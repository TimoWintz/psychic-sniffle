//! First Principles: a physics game. See `docs/` for the design.

mod devscript;
mod island;
mod player;
mod ruler;
mod shipyard;
mod sim;
mod state;
mod stations;
mod tutor_bridge;
mod ui;
mod voxel;

use avian3d::prelude::PhysicsPlugins;
use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use state::Game;

/// Columns per terrain mesh along each horizontal axis.
const CHUNK: i32 = 32;

fn main() {
    let game = Game::load_or_new();
    let (world, sites) = island::build(&game.layout);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "First Principles".into(), resolution: (1280, 720).into(), ..default() }),
            ..default()
        }))
        .add_plugins((EguiPlugin::default(), PhysicsPlugins::default()))
        .insert_resource(ClearColor(Color::srgb(0.53, 0.72, 0.90)))
        .insert_resource(game)
        .insert_resource(world)
        .insert_resource(sites)
        .add_systems(Startup, scenery)
        .add_plugins((
            player::PlayerPlugin,
            stations::StationsPlugin,
            sim::SimPlugin,
            ruler::RulerPlugin,
            shipyard::ShipyardPlugin,
            tutor_bridge::TutorPlugin,
            ui::UiPlugin,
            devscript::DevScriptPlugin,
        ))
        .run();
}

/// Terrain meshes, sea, sun and camera.
fn scenery(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, world: Res<voxel::VoxelWorld>) {
    // Block colours are per vertex, so one white material serves the whole island.
    let blocks = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, ..default() });
    for z0 in (0..world.size.z).step_by(CHUNK as usize) {
        for x0 in (0..world.size.x).step_by(CHUNK as usize) {
            let mesh = world.mesh(x0, (x0 + CHUNK).min(world.size.x), z0, (z0 + CHUNK).min(world.size.z));
            commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(blocks.clone())));
        }
    }

    let size = world.size.as_vec3();
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(size.x * 6.0, size.z * 6.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.10, 0.36, 0.60, 0.82),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.25,
            ..default()
        })),
        Transform::from_xyz(size.x / 2.0, island::SEA_LEVEL, size.z / 2.0),
    ));

    commands.spawn((
        DirectionalLight { illuminance: 9_000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(60.0, 100.0, 30.0).looking_at(Vec3::new(88.0, 20.0, 64.0), Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { brightness: 320.0, ..default() });
    commands.spawn((Camera3d::default(), Transform::from_xyz(20.0, 30.0, 60.0)));
}
