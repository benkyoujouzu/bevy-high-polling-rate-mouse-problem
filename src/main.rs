use bevy::prelude::*;
use rawinput_mouse::MouseRawInputManager;

fn main() {
    App::new()
    .insert_resource(MouseInputReader{manager: MouseRawInputManager::new()})
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, mouse_orbit)
        .run();
}

#[derive(Resource)]
struct MouseInputReader {
    manager: MouseRawInputManager,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mouse_reader: ResMut<MouseInputReader>,
) {
    let camera = Camera3dBundle::default();
    commands.spawn(camera);

    let cube = PbrBundle {
        mesh: meshes.add(Cuboid::default()),
        transform: Transform::from_xyz(0.0, 0.0, -2.0),
        ..default()
    };
    commands.spawn(cube);

    mouse_reader.manager.start();
}

fn mouse_orbit(
    mouse_reader: Res<MouseInputReader>,
    mut camera_q: Query<&mut Transform, With<Camera3d>>,
) {
    let mut camera = camera_q.single_mut();
    let mouse_events = mouse_reader.manager.get_events();
    let mut mouse_delta = Vec2::ZERO;
    for mouse_event in mouse_events {
        mouse_delta += Vec2::new(mouse_event.dx as f32, mouse_event.dy as f32);
    }
    mouse_delta *= 0.001;
    let (mut yaw, mut pitch, _) = camera.rotation.to_euler(EulerRot::YXZ);
    yaw -= mouse_delta.x;
    pitch -= mouse_delta.y;
    camera.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
}
