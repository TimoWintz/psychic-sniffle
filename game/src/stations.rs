//! Stations: the consoles where puzzles are worked on.

use crate::island::Sites;
use crate::player::Player;
use crate::state::{Game, Ui};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use physics_core::materials::Material;

/// How close the avatar must be to use a station.
const REACH: f32 = 4.5;

#[derive(Component)]
struct Beacon(usize);

pub struct StationsPlugin;

impl Plugin for StationsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn).add_systems(Update, (proximity, colour_beacons));
    }
}

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sites: Res<Sites>) {
    let screen = meshes.add(Cuboid::new(0.8, 0.5, 0.1));
    let beam = meshes.add(Cuboid::new(0.12, 14.0, 0.12));
    for (i, console) in sites.stations.iter().enumerate() {
        let top = console.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
        let material = materials.add(StandardMaterial { unlit: true, ..default() });
        commands.spawn((Beacon(i), Mesh3d(screen.clone()), MeshMaterial3d(material.clone()), Transform::from_translation(top + Vec3::Y * 0.3)));
        // A thin column of light so stations can be found from across the island.
        commands.spawn((Beacon(i), Mesh3d(beam.clone()), MeshMaterial3d(material), Transform::from_translation(top + Vec3::Y * 8.0)));
    }

    // Survey stakes at the corners of each target: a dark post with an orange pennant.
    let post = meshes.add(Cuboid::new(0.09, 1.4, 0.09));
    let pennant = meshes.add(Cuboid::new(0.03, 0.22, 0.34));
    let post_material = materials.add(StandardMaterial { base_color: Color::srgb(0.20, 0.21, 0.25), perceptual_roughness: 0.6, ..default() });
    let pennant_material = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.56, 0.10), unlit: true, ..default() });
    for foot in &sites.stakes {
        commands.spawn((Mesh3d(post.clone()), MeshMaterial3d(post_material.clone()), Transform::from_translation(*foot + Vec3::Y * 0.7)));
        commands.spawn((Mesh3d(pennant.clone()), MeshMaterial3d(pennant_material.clone()), Transform::from_translation(*foot + Vec3::new(0.0, 1.28, 0.2))));
    }

    // Lamp posts: a slim pole, a quartz lamp and a cap salvaged from the hull.
    let pole = meshes.add(Cuboid::new(0.12, 2.3, 0.12));
    let lamp = meshes.add(Cuboid::new(0.3, 0.38, 0.3));
    let cap = meshes.add(Cuboid::new(0.46, 0.07, 0.46));
    let quartz = Material::Quartz.properties().colour;
    let lamp_material = materials.add(StandardMaterial { base_color: Color::srgb(quartz[0], quartz[1], quartz[2]), unlit: true, ..default() });
    for foot in &sites.lanterns {
        commands.spawn((Mesh3d(pole.clone()), MeshMaterial3d(post_material.clone()), Transform::from_translation(*foot + Vec3::Y * 1.15)));
        commands.spawn((Mesh3d(lamp.clone()), MeshMaterial3d(lamp_material.clone()), Transform::from_translation(*foot + Vec3::Y * 2.45)));
        commands.spawn((Mesh3d(cap.clone()), MeshMaterial3d(post_material.clone()), Transform::from_translation(*foot + Vec3::Y * 2.68)));
    }
}

fn proximity(
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    sites: Res<Sites>,
    game: Res<Game>,
    mut ui: ResMut<Ui>,
    player: Single<&Transform, With<Player>>,
) {
    let position = player.translation;
    ui.near_station = sites
        .stations
        .iter()
        .position(|console| (console.as_vec3() + Vec3::splat(0.5)).distance(position + Vec3::Y * 0.5) <= REACH);
    // Walking away closes the panel.
    if ui.station.is_some() && ui.station != ui.near_station {
        ui.station = None;
    }
    if wants.wants_any_keyboard_input() {
        return;
    }
    if keys.just_pressed(KeyCode::KeyE)
        && let Some(near) = ui.near_station
    {
        ui.station = if ui.station == Some(near) || !game.unlocked(near) { None } else { Some(near) };
    }
    if keys.just_pressed(KeyCode::Escape) {
        ui.station = None;
    }
}

fn colour_beacons(game: Res<Game>, beacons: Query<(&Beacon, &MeshMaterial3d<StandardMaterial>)>, mut materials: ResMut<Assets<StandardMaterial>>) {
    if !game.is_changed() {
        return;
    }
    for (beacon, handle) in &beacons {
        let colour = if game.solved(beacon.0) {
            Color::srgb(0.25, 0.95, 0.4)
        } else if game.unlocked(beacon.0) {
            Color::srgb(1.0, 0.65, 0.1)
        } else {
            Color::srgb(0.35, 0.35, 0.38)
        };
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.base_color = colour;
        }
    }
}
