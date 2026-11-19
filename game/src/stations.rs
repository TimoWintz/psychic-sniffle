//! Stations: the builders' consoles where puzzles are worked on, one beside each machine, and their
//! lamps.

use crate::island::{MACHINES, Sites};
use crate::player::Player;
use crate::state::{Game, Mode, PUZZLES, Ui};
use crate::waking::{MachineTrim, Waking};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;

/// How close the avatar must be to use a station.
const REACH: f32 = 4.5;

#[derive(Component)]
struct Beacon(usize);

pub struct StationsPlugin;

impl Plugin for StationsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn).add_systems(Update, (proximity.run_if(in_state(Mode::Playing)), colour_beacons, light_consoles));
    }
}

/// The quartz in each machine's console face and lamps: dark while it sleeps, lit once it wakes.
#[derive(Resource)]
struct ConsoleQuartz([Handle<StandardMaterial>; MACHINES]);

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sites: Res<Sites>) {
    let stone = crate::waking::stone(&mut materials);
    let quartz = ConsoleQuartz(std::array::from_fn(|_| materials.add(MachineTrim::look(false))));

    // Each machine's console: a pillar of the builders' stone with a sloping top, and a quartz face
    // set into it.
    let pillar = meshes.add(Cuboid::new(0.6, 1.0, 0.6));
    let top = meshes.add(Cuboid::new(0.86, 0.14, 0.7));
    let face = meshes.add(Cuboid::new(0.62, 0.05, 0.46));
    let beam = meshes.add(Cuboid::new(0.12, 14.0, 0.12));
    let slope = Quat::from_rotation_x(-0.4);
    for (i, console) in sites.stations.iter().enumerate() {
        let foot = console.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        commands.spawn((Mesh3d(pillar.clone()), MeshMaterial3d(stone.clone()), Transform::from_translation(foot + Vec3::Y * 0.5)));
        let slab = Transform::from_translation(foot + Vec3::Y * 1.05).with_rotation(slope);
        commands.spawn((Mesh3d(top.clone()), MeshMaterial3d(stone.clone()), slab));
        commands.spawn((Mesh3d(face.clone()), MeshMaterial3d(quartz.0[i].clone()), slab.with_translation(slab.translation + slab.rotation * Vec3::Y * 0.08)));
        // A thin column of light, once it is awake, so stations can be found from across the island.
        let material = materials.add(StandardMaterial { unlit: true, ..default() });
        commands.spawn((Beacon(i), Mesh3d(beam.clone()), MeshMaterial3d(material), Transform::from_translation(foot + Vec3::Y * 9.0)));
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

    // The builders' lamps: a stone post, a block of quartz and a stone cap. Each lights as its
    // machine wakes.
    let pole = meshes.add(Cuboid::new(0.22, 2.3, 0.22));
    let lamp = meshes.add(Cuboid::new(0.3, 0.38, 0.3));
    let cap = meshes.add(Cuboid::new(0.46, 0.1, 0.46));
    for (foot, &machine) in sites.lanterns.iter().zip(&sites.lantern_machine) {
        commands.spawn((Mesh3d(pole.clone()), MeshMaterial3d(stone.clone()), Transform::from_translation(*foot + Vec3::Y * 1.15)));
        commands.spawn((Mesh3d(lamp.clone()), MeshMaterial3d(quartz.0[machine].clone()), Transform::from_translation(*foot + Vec3::Y * 2.49)));
        commands.spawn((Mesh3d(cap.clone()), MeshMaterial3d(stone.clone()), Transform::from_translation(*foot + Vec3::Y * 2.73)));
    }
    commands.insert_resource(quartz);
}

/// Light each machine's console face and lamps as it wakes.
fn light_consoles(waking: Res<Waking>, quartz: Res<ConsoleQuartz>, mut materials: ResMut<Assets<StandardMaterial>>, mut lit: Local<Option<[bool; MACHINES]>>) {
    let now: [bool; MACHINES] = std::array::from_fn(|machine| waking.console_awake(machine));
    if *lit == Some(now) {
        return;
    }
    for (machine, handle) in quartz.0.iter().enumerate() {
        if lit.is_none_or(|before| before[machine] != now[machine])
            && let Some(mut material) = materials.get_mut(handle)
        {
            *material = MachineTrim::look(now[machine]);
        }
    }
    *lit = Some(now);
}

fn proximity(
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    sites: Res<Sites>,
    game: Res<Game>,
    waking: Res<Waking>,
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
    // While the way wakes, E skips the wake (see `waking`) rather than opening a station.
    if wants.wants_any_keyboard_input() || waking.playing() {
        return;
    }
    if keys.just_pressed(KeyCode::KeyE)
        && let Some(near) = ui.near_station
    {
        // A station opens once its console is awake: once the machine has woken.
        let ready = game.unlocked(near) && waking.console_awake(near);
        ui.station = if ui.station == Some(near) || !ready { None } else { Some(near) };
    }
}

/// A console's column of light is there once its machine has woken: green once its job is done,
/// orange while it is open.
fn colour_beacons(
    game: Res<Game>,
    waking: Res<Waking>,
    mut beacons: Query<(&Beacon, &MeshMaterial3d<StandardMaterial>, &mut Visibility)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shown: Local<Option<usize>>,
) {
    // A wake changes the way every frame; the columns only when a console wakes.
    let awake = (0..PUZZLES).filter(|&i| waking.console_awake(i)).count();
    if !game.is_changed() && *shown == Some(awake) {
        return;
    }
    *shown = Some(awake);
    for (beacon, handle, mut visibility) in &mut beacons {
        visibility.set_if_neq(if waking.console_awake(beacon.0) { Visibility::Inherited } else { Visibility::Hidden });
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
