use core_affinity;
use rtrb::{Consumer, Producer, RingBuffer};
use std::ffi::OsStr;
use std::mem::{size_of, MaybeUninit};
use std::os::windows::prelude::OsStrExt;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::{mem, thread};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HID_USAGE_GENERIC_MOUSE, HID_USAGE_PAGE_GENERIC,
};
use windows::Win32::Foundation::{HANDLE, HWND, WAIT_OBJECT_0};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::Threading::{CreateEventW, SetEvent, INFINITE};
use windows::Win32::UI::Input::{
    GetRawInputBuffer, RegisterRawInputDevices, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
    RIM_TYPEMOUSE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetQueueStatus, MsgWaitForMultipleObjects, QS_RAWINPUT,
    WINDOW_EX_STYLE, WINDOW_STYLE,
};
use windows::{self, core::PCWSTR};

#[derive(Debug, Copy, Clone)]
pub struct MouseRawEvent {
    pub dx: i32,
    pub dy: i32,
    pub t: i64,
}

pub struct MouseRawInputManager {
    timer_frequency: i64,
    exit_handle: HANDLE,
    pub receiver: Option<Arc<Mutex<Consumer<MouseRawEvent>>>>,
    joiner: Option<JoinHandle<()>>,
}

fn create_window_class() -> HWND {
    unsafe {
        let classname = OsStr::new("Message")
            .encode_wide()
            .chain(Some(0).into_iter())
            .collect::<Vec<_>>();
        let windowname = OsStr::new("RawInput Message")
            .encode_wide()
            .chain(Some(0).into_iter())
            .collect::<Vec<_>>();

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR::from_raw(classname.as_ptr()),
            PCWSTR::from_raw(windowname.as_ptr()),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            None,
            None,
            None,
            None,
        );
        if hwnd.0 == 0 {
            panic!("Window Creation Failed!");
        }
        hwnd
    }
}

fn register_mouse_raw_input(hwnd: HWND) {
    let flags = windows::Win32::UI::Input::RAWINPUTDEVICE_FLAGS::default();
    let devices = [RAWINPUTDEVICE {
        usUsagePage: HID_USAGE_PAGE_GENERIC,
        usUsage: HID_USAGE_GENERIC_MOUSE,
        dwFlags: flags,
        hwndTarget: hwnd,
    }];
    let device_size = size_of::<RAWINPUTDEVICE>() as u32;
    unsafe {
        let succ = RegisterRawInputDevices(&devices, device_size) == true;
        if !succ {
            panic!("Register Mouse Raw Input Failed!");
        }
    }
}

fn win_get_event(sender: &mut Producer<MouseRawEvent>, last_time: &mut i64) -> u32 {
    unsafe {
        let mut buffer = [MaybeUninit::<RAWINPUT>::uninit(); 1000];
        let mut buffer_size = mem::size_of_val(&buffer) as u32;
        let mut buffer_ptr = buffer.as_mut_ptr() as *mut RAWINPUT;
        let mut total_elements = 0;
        loop {
            let element_count = GetRawInputBuffer(
                Some(buffer_ptr),
                &mut buffer_size,
                mem::size_of::<RAWINPUTHEADER>() as u32,
            );

            if element_count as i32 == -1 {
                panic!("GetRawInputBuffer Error");
            }

            if element_count == 0 {
                return total_elements;
            }

            total_elements += element_count;

            let mut now = MaybeUninit::<i64>::uninit();
            QueryPerformanceCounter(now.as_mut_ptr());
            let now = now.assume_init();
            let mut increment: i64 = 0;
            if element_count > 0 {
                increment = (now - *last_time) / element_count as i64
            };
            for _ in 0..element_count {
                *last_time += increment;
                let first_elem = *buffer_ptr;
                if first_elem.header.dwType == RIM_TYPEMOUSE.0 {
                    let raw_data = first_elem.data.mouse;
                    let data = MouseRawEvent {
                        dx: raw_data.lLastX,
                        dy: raw_data.lLastY,
                        t: *last_time,
                    };
                    match sender.push(data) {
                        Ok(_) => {}
                        Err(_) => println!("push error"),
                    };
                }
                buffer_ptr = (buffer_ptr as *mut u8).offset(first_elem.header.dwSize as isize)
                    as *mut RAWINPUT;
            }
            *last_time = now;
        }
    }
}

impl MouseRawInputManager {
    pub fn start(&mut self) {
        let exit_handle = self.exit_handle;

        let (sender, receiver) = RingBuffer::new(1024);
        self.receiver = Some(Arc::new(Mutex::new(receiver)));

        let joiner = thread::spawn(move || {
            let cores = core_affinity::get_core_ids().unwrap();
            core_affinity::set_for_current(cores[cores.len() - 1]);
            let hwnd = create_window_class();
            register_mouse_raw_input(hwnd);

            let mut sender = sender;

            unsafe {
                let mut done_event: [HANDLE; 1] = [HANDLE::default()];
                done_event[0] = exit_handle;
                let mut last_time = 0;
                QueryPerformanceCounter(&mut last_time);
                loop {
                    if MsgWaitForMultipleObjects(Some(&done_event), false, INFINITE, QS_RAWINPUT)
                        != WAIT_OBJECT_0.0 + 1
                    {
                        break;
                    }
                    GetQueueStatus(QS_RAWINPUT);
                    win_get_event(&mut sender, &mut last_time);
                }
            }
            unsafe {
                DestroyWindow(hwnd);
            }
        });
        self.joiner = Some(joiner);
    }

    pub fn new() -> MouseRawInputManager {
        let mut p_freq = MaybeUninit::<i64>::uninit();
        unsafe {
            QueryPerformanceFrequency(p_freq.as_mut_ptr());
        }
        let exit_handle = unsafe { CreateEventW(None, false, false, None).unwrap() };

        MouseRawInputManager {
            timer_frequency: unsafe { p_freq.assume_init() },
            receiver: None,
            exit_handle,
            joiner: None,
        }
    }

    pub fn get_events(&self) -> Vec<MouseRawEvent> {
        let mut res = Vec::new();
        if let Some(receiver) = &self.receiver {
            let mut receiver = receiver.lock().unwrap();
            while let Ok(data) = receiver.pop() {
                res.push(data);
            }
        }
        return res;
    }
}

impl Drop for MouseRawInputManager {
    fn drop(&mut self) {
        unsafe {
            SetEvent(self.exit_handle);
        }
        self.joiner.take().unwrap().join().unwrap();
    }
}