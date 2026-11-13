//! Renders one frame of a lit cube and saves it. Used to check that rendering works on a machine.
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, capture)
        .run();
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::default())),
        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.5, 0.2))),
    ));
    commands.spawn((DirectionalLight::default(), Transform::from_xyz(3.0, 5.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.spawn((Camera3d::default(), Transform::from_xyz(-2.0, 2.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y)));
}

fn capture(mut commands: Commands, mut frame: Local<u32>, mut exit: MessageWriter<AppExit>) {
    *frame += 1;
    if *frame == 150 {
        let path = std::env::var("SHOT").unwrap_or_else(|_| "render_check.png".into());
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if *frame == 180 {
        exit.write(AppExit::Success);
    }
}
