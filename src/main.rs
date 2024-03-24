use bevy::{input::mouse::MouseMotion, prelude::*};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, mouse_orbit)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let camera = Camera3dBundle::default();
    commands.spawn(camera);

    let cube = PbrBundle {
        mesh: meshes.add(Cuboid::default()),
        transform: Transform::from_xyz(0.0, 0.0, -2.0),
        ..default()
    };
    commands.spawn(cube);
}

fn mouse_orbit(
    mut mouse_events: EventReader<MouseMotion>,
    mut camera_q: Query<&mut Transform, With<Camera3d>>,
) {
    let mut camera = camera_q.single_mut();
    let mut mouse_delta = Vec2::ZERO;
    for mouse_event in mouse_events.read() {
        mouse_delta += mouse_event.delta;
    }
    mouse_delta *= 0.001;
    let (mut yaw, mut pitch, _) = camera.rotation.to_euler(EulerRot::YXZ);
    yaw -= mouse_delta.x;
    pitch -= mouse_delta.y;
    camera.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
}
