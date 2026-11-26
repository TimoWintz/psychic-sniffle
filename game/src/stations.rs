//! Stations: the builders' consoles where puzzles are worked on, one beside each machine, and their
//! lamps.

use crate::island::{GROUND, MACHINES, Sites};
use crate::player::{OrbitCamera, Player};
use crate::sim::Sim;
use crate::state::{Game, Mode, PUZZLES, Ui};
use crate::waking::{MachineTrim, Waking};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use physics_core::puzzle::PuzzleKind;

/// How close the avatar must be to use a station.
const REACH: f32 = 4.5;

#[derive(Component)]
struct Beacon(usize);

pub struct StationsPlugin;

impl Plugin for StationsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn)
            .add_systems(Update, ((proximity, frame_station).chain().run_if(in_state(Mode::Playing)), colour_beacons, light_consoles));
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
fn light_consoles(
    time: Res<Time>,
    waking: Res<Waking>,
    quartz: Res<ConsoleQuartz>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shown: Local<Option<[f32; MACHINES]>>,
) {
    let lit: [bool; MACHINES] = std::array::from_fn(|machine| waking.console_awake(machine));
    for (machine, f) in crate::waking::fade_towards(&mut shown, lit, time.delta_secs()) {
        if let Some(mut material) = materials.get_mut(&quartz.0[machine]) {
            *material = MachineTrim::look_at(f);
        }
    }
}

/// Whether a station opens: its puzzle is unlocked and its console is awake (once the machine has
/// woken). The hint under the screen says the same.
pub fn ready(game: &Game, waking: &Waking, puzzle: usize) -> bool {
    game.unlocked(puzzle) && waking.console_awake(puzzle)
}

/// The machine a puzzle's station works, as the hints name it.
pub fn machine_name(kind: PuzzleKind) -> &'static str {
    match kind {
        PuzzleKind::Rail => "the supply rail",
        PuzzleKind::Chase => "the haul track",
        PuzzleKind::Gravity => "the drop tower",
        PuzzleKind::Channel => "the ledge launcher",
        PuzzleKind::Ridge => "the tilting launcher",
    }
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
    ui.near_station = sites.stations.iter().position(|console| (console.as_vec3() + Vec3::splat(0.5)).distance(position + Vec3::Y * 0.5) <= REACH);
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
        ui.station = if ui.station == Some(near) || !ready(&game, &waking, near) { None } else { Some(near) };
    }
}

/// The smallest box that holds every point: its lowest and its highest corner.
pub fn bounds(points: &[Vec3]) -> (Vec3, Vec3) {
    points.iter().fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |(lo, hi), p| (lo.min(*p), hi.max(*p)))
}

/// What the camera shows while a puzzle's station is open: the box from `min` to `max`, seen from
/// the side with `along` running left to right. It holds what the ruler measures there and where the
/// test runs, so the player can work from the console.
pub fn station_view(kind: PuzzleKind, game: &Game, sites: &Sites) -> (Vec3, Vec3, Vec3) {
    let layout = &game.layout;
    let ground = GROUND as f32;
    let span = bounds;
    match kind {
        PuzzleKind::Rail => {
            let (min, max) = span(&[sites.rail_start - Vec3::X * 1.5, sites.rail_start + Vec3::new(layout.rail_target as f32 + 2.0, 2.0, 0.0)]);
            (min, max, Vec3::X)
        }
        PuzzleKind::Chase => {
            let reach = layout.transfer_at.max(layout.posts().1) as f64 + 2.0;
            haul_view(sites, reach as f32)
        }
        PuzzleKind::Gravity => {
            let (min, max) = span(&[sites.tower_plate, sites.tower_release, sites.mast_plate, sites.mast_release, sites.gate_stand]);
            (min, max, Vec3::X)
        }
        PuzzleKind::Channel => {
            let site = sites.channel_launch;
            let (min, max) = span(&[site.at(-2.0, 0.0, ground), site.origin + Vec3::Y * 2.0, site.at(layout.platform_centre + 2.0, 0.0, ground)]);
            (min, max, site.direction)
        }
        PuzzleKind::Ridge => {
            let site = sites.ridge_launch;
            let (min, max) = span(&[site.at(-2.0, 0.0, ground), site.at(layout.pad_centre + 3.0, layout.ridge_h as f64 + 2.0, ground)]);
            (min, max, site.direction)
        }
    }
}

/// The haul track from the side, from just behind the haul cart's start to `reach` metres along it.
pub fn haul_view(sites: &Sites, reach: f32) -> (Vec3, Vec3, Vec3) {
    let (a, b) = (sites.haul_at(-1.5), sites.haul_at(reach as f64 + 0.5));
    (a.min(b), a.max(b) + Vec3::Y * 2.0, sites.haul_direction)
}

/// Turn the camera to the puzzle's view, gliding, while the station is open.
pub fn look_at_station(orbit: &mut OrbitCamera, game: &Game, sites: &Sites, puzzle: usize) {
    let (min, max, along) = station_view(game.def(puzzle).kind, game, sites);
    orbit.frame_side_view(min, max, along, true);
}

/// While a station is open the camera shows its puzzle, and it glides back to the avatar when the
/// player leaves. A test borrows it meanwhile (`sim` hands it back), and a wake closes the station
/// and borrows it itself.
fn frame_station(
    game: Res<Game>,
    sites: Res<Sites>,
    ui: Res<Ui>,
    sim: Res<Sim>,
    waking: Res<Waking>,
    mut orbit: ResMut<OrbitCamera>,
    mut shown: Local<Option<usize>>,
) {
    if ui.station == *shown {
        return;
    }
    if sim.busy() || sim.watching() {
        // The test keeps the camera; `sim` brings it back to the station, or home, when it is done.
        *shown = ui.station;
        return;
    }
    match ui.station {
        Some(puzzle) => look_at_station(&mut orbit, &game, &sites, puzzle),
        None if !waking.playing() => orbit.ease_home(),
        None => {}
    }
    *shown = ui.station;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::SIDE_VIEW_FURTHEST;

    #[test]
    fn every_station_view_fits_on_screen() {
        // Each station's view stands back far enough to show its whole box left of the panel: the
        // distance it asks for is within what a side view may use.
        for seed in 0..20 {
            let game = Game::for_test(seed);
            let (_, sites) = crate::island::build(&game.layout);
            for puzzle in 0..PUZZLES {
                let kind = game.def(puzzle).kind;
                let (min, max, along) = station_view(kind, &game, &sites);
                let mut orbit = OrbitCamera::default();
                orbit.frame_side_view(min, max, along, false);
                assert!(orbit.distance < SIDE_VIEW_FURTHEST, "seed {seed}: {kind:?} wants {} m", orbit.distance);
            }
        }
    }
}
