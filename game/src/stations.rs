//! Stations: the builders' consoles where puzzles are worked on, one beside each machine, and their
//! lamps.

use crate::island::{GROUND, MACHINES, Sites};
use crate::player::{OrbitCamera, Player};
use crate::sim::Sim;
use crate::state::{BuildWorld, Game, Mode, PUZZLES, Ui, WorldEntity, not_paused};
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
        app.add_systems(BuildWorld, spawn).add_systems(
            Update,
            ((proximity, frame_station).chain().run_if(in_state(Mode::Playing)).run_if(not_paused), colour_beacons, light_consoles.run_if(not_paused)),
        );
    }
}

/// The quartz in each machine's console face and lamps: dark while it sleeps, lit once it wakes.
#[derive(Resource)]
struct ConsoleQuartz {
    materials: [Handle<StandardMaterial>; MACHINES],
    fade_shown: Option<[f32; MACHINES]>,
    frame_shown: Option<usize>,
    beacons_shown: Option<usize>,
}

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sites: Res<Sites>) {
    let stone = crate::waking::stone(&mut materials);
    let quartz =
        ConsoleQuartz { materials: std::array::from_fn(|_| materials.add(MachineTrim::look(false))), fade_shown: None, frame_shown: None, beacons_shown: None };

    // Each machine's console: a pillar of the builders' stone with a sloping top, and a quartz face
    // set into it.
    let pillar = meshes.add(Cuboid::new(0.6, 1.0, 0.6));
    let top = meshes.add(Cuboid::new(0.86, 0.14, 0.7));
    let face = meshes.add(Cuboid::new(0.62, 0.05, 0.46));
    let beam = meshes.add(Cuboid::new(0.12, 14.0, 0.12));
    let slope = Quat::from_rotation_x(-0.4);
    for (i, console) in sites.stations.iter().enumerate() {
        let foot = console.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        commands.spawn((WorldEntity, Mesh3d(pillar.clone()), MeshMaterial3d(stone.clone()), Transform::from_translation(foot + Vec3::Y * 0.5)));
        let slab = Transform::from_translation(foot + Vec3::Y * 1.05).with_rotation(slope);
        commands.spawn((WorldEntity, Mesh3d(top.clone()), MeshMaterial3d(stone.clone()), slab));
        commands.spawn((
            WorldEntity,
            Mesh3d(face.clone()),
            MeshMaterial3d(quartz.materials[i].clone()),
            slab.with_translation(slab.translation + slab.rotation * Vec3::Y * 0.08),
        ));
        // A thin column of light, once it is awake, so stations can be found from across the island.
        let material = materials.add(StandardMaterial { unlit: true, ..default() });
        commands.spawn((WorldEntity, Beacon(i), Mesh3d(beam.clone()), MeshMaterial3d(material), Transform::from_translation(foot + Vec3::Y * 9.0)));
    }

    // Survey stakes at the corners of each target: a dark post with an orange pennant.
    let post = meshes.add(Cuboid::new(0.09, 1.4, 0.09));
    let pennant = meshes.add(Cuboid::new(0.03, 0.22, 0.34));
    let post_material = materials.add(StandardMaterial { base_color: Color::srgb(0.20, 0.21, 0.25), perceptual_roughness: 0.6, ..default() });
    let pennant_material = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.56, 0.10), unlit: true, ..default() });
    for foot in &sites.stakes {
        commands.spawn((WorldEntity, Mesh3d(post.clone()), MeshMaterial3d(post_material.clone()), Transform::from_translation(*foot + Vec3::Y * 0.7)));
        commands.spawn((
            WorldEntity,
            Mesh3d(pennant.clone()),
            MeshMaterial3d(pennant_material.clone()),
            Transform::from_translation(*foot + Vec3::new(0.0, 1.28, 0.2)),
        ));
    }

    // The builders' lamps: a stone post, a block of quartz and a stone cap. Each lights as its
    // machine wakes.
    let pole = meshes.add(Cuboid::new(0.22, 2.3, 0.22));
    let lamp = meshes.add(Cuboid::new(0.3, 0.38, 0.3));
    let cap = meshes.add(Cuboid::new(0.46, 0.1, 0.46));
    for (foot, &machine) in sites.lanterns.iter().zip(&sites.lantern_machine) {
        commands.spawn((WorldEntity, Mesh3d(pole.clone()), MeshMaterial3d(stone.clone()), Transform::from_translation(*foot + Vec3::Y * 1.15)));
        commands.spawn((
            WorldEntity,
            Mesh3d(lamp.clone()),
            MeshMaterial3d(quartz.materials[machine].clone()),
            Transform::from_translation(*foot + Vec3::Y * 2.49),
        ));
        commands.spawn((WorldEntity, Mesh3d(cap.clone()), MeshMaterial3d(stone.clone()), Transform::from_translation(*foot + Vec3::Y * 2.73)));
    }
    commands.insert_resource(quartz);
}

/// Light each machine's console face and lamps as it wakes.
fn light_consoles(time: Res<Time>, waking: Res<Waking>, mut quartz: ResMut<ConsoleQuartz>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let lit: [bool; MACHINES] = std::array::from_fn(|machine| waking.console_awake(machine));
    let quartz = &mut *quartz;
    for (machine, f) in crate::waking::fade_towards(&mut quartz.fade_shown, lit, time.delta_secs()) {
        if let Some(mut material) = materials.get_mut(&quartz.materials[machine]) {
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

#[allow(clippy::too_many_arguments)]
fn proximity(
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    sites: Res<Sites>,
    game: Res<Game>,
    sim: Res<Sim>,
    beam: Res<crate::beam::Beam>,
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
    // While the HUD is hidden (wake, station run, or beam swing), E cannot open or close a station.
    if wants.wants_any_keyboard_input() || crate::ui::hud_hidden(&waking, &sim, &beam) {
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

/// Frame a puzzle's station view in `orbit`. The ridge stands across the tilting launcher's line
/// with a coast-to-coast spine, so its station view looks from south-east and higher up to keep the
/// launcher's release point, the ridge's near foot and crest, and the landing circle all in sight.
pub fn frame_station_view(orbit: &mut OrbitCamera, kind: PuzzleKind, game: &Game, sites: &Sites, glide: bool) {
    let (min, max, along) = station_view(kind, game, sites);
    match kind {
        PuzzleKind::Ridge => orbit.frame_angled_view(min, max, along, -0.62, 0.34, glide),
        _ => orbit.frame_side_view(min, max, along, glide),
    }
}

/// Turn the camera to the puzzle's view, gliding, while the station is open.
pub fn look_at_station(orbit: &mut OrbitCamera, game: &Game, sites: &Sites, puzzle: usize) {
    frame_station_view(orbit, game.def(puzzle).kind, game, sites, true);
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
    mut quartz: ResMut<ConsoleQuartz>,
) {
    if ui.station == quartz.frame_shown {
        return;
    }
    if sim.busy() || sim.watching() {
        // The test keeps the camera; `sim` brings it back to the station, or home, when it is done.
        quartz.frame_shown = ui.station;
        return;
    }
    match ui.station {
        Some(puzzle) => look_at_station(&mut orbit, &game, &sites, puzzle),
        None if !waking.playing() => orbit.ease_home(),
        None => {}
    }
    quartz.frame_shown = ui.station;
}

/// A console's column of light is there once its machine has woken: green once its job is done,
/// orange while it is open.
fn colour_beacons(
    game: Res<Game>,
    waking: Res<Waking>,
    mut beacons: Query<(&Beacon, &MeshMaterial3d<StandardMaterial>, &mut Visibility)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut quartz: ResMut<ConsoleQuartz>,
) {
    // A wake changes the way every frame; the columns only when a console wakes.
    let awake = (0..PUZZLES).filter(|&i| waking.console_awake(i)).count();
    if !game.is_changed() && quartz.beacons_shown == Some(awake) {
        return;
    }
    quartz.beacons_shown = Some(awake);
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
                let mut orbit = OrbitCamera::default();
                frame_station_view(&mut orbit, kind, &game, &sites, false);
                assert!(orbit.distance < SIDE_VIEW_FURTHEST, "seed {seed}: {kind:?} wants {} m", orbit.distance);
            }
        }
    }

    /// Ray-AABB intersection returning `(t_enter, t_exit)` for `t >= 0` if the ray hits the box.
    fn ray_aabb(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<(f32, f32)> {
        let mut tmin = 0.0_f32;
        let mut tmax = f32::INFINITY;
        for axis in 0..3 {
            if dir[axis].abs() < 1e-8 {
                if origin[axis] < min[axis] || origin[axis] > max[axis] {
                    return None;
                }
            } else {
                let inv = 1.0 / dir[axis];
                let mut t1 = (min[axis] - origin[axis]) * inv;
                let mut t2 = (max[axis] - origin[axis]) * inv;
                if t1 > t2 {
                    std::mem::swap(&mut t1, &mut t2);
                }
                tmin = tmin.max(t1);
                tmax = tmax.min(t2);
                if tmin > tmax {
                    return None;
                }
            }
        }
        Some((tmin, tmax))
    }

    fn aabb(centre: Vec3, size: Vec3) -> (Vec3, Vec3) {
        (centre - size / 2.0, centre + size / 2.0)
    }

    fn project(eye: Vec3, focus: Vec3, p: Vec3) -> Option<Vec2> {
        let cam = Transform::from_translation(eye).looking_at(focus, Vec3::Y);
        let local = cam.rotation.inverse() * (p - eye);
        let depth = -local.z;
        if depth <= 0.1 {
            return None;
        }
        let tan_half = (std::f32::consts::FRAC_PI_4 / 2.0).tan();
        let aspect = 16.0 / 9.0;
        let ndc_x = local.x / (depth * tan_half * aspect);
        let ndc_y = local.y / (depth * tan_half);
        Some(Vec2::new((ndc_x + 1.0) * 0.5 * 1920.0, (1.0 - ndc_y) * 0.5 * 1080.0))
    }

    /// Clean regression test verifying that for every puzzle's station view, the camera is not
    /// pulled in by terrain, every survey pin stays on screen clear of the station panel and HUD
    /// corners, and no terrain block, vehicle, rig part, console, lamp, or stake occludes any pin.
    #[test]
    fn every_station_pin_is_visible_and_unoccluded() {
        use physics_core::materials::Material;
        use physics_core::puzzle::Ch1Layout;
        use std::collections::BTreeMap;

        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut examples: BTreeMap<String, String> = BTreeMap::new();
        let mut note = |key: String, detail: String| {
            *counts.entry(key.clone()).or_default() += 1;
            examples.entry(key).or_insert(detail);
        };

        let seeds = 80u64;
        for seed in 0..seeds {
            let game = Game::for_test(seed);
            let (base_world, base_sites) = crate::island::build(&game.layout);

            // Check the Beam side-machine view as well.
            {
                let mut world = base_world.clone();
                crate::waking::settle(&mut world, &base_sites.way, 0);
                let (min, max) = crate::beam::view(&base_sites.beam);
                let mut orbit = OrbitCamera::default();
                orbit.frame_side_view(min, max, Vec3::NEG_X, false);
                let focus = orbit.watch.unwrap();
                orbit.follow(focus, 1.0);
                let reach = orbit.reach(&world);
                if reach < orbit.distance - 0.1 {
                    note("Beam: camera pulled in by terrain".into(), format!("seed {seed}: want {:.1}, got {:.1}", orbit.distance, reach));
                }
                let eye = focus + crate::player::back(orbit.yaw, orbit.pitch) * reach;
                for seat_x in [-5.0, 0.0, 5.0] {
                    let pos = base_sites.beam.pivot + Vec3::X * seat_x;
                    let to = pos - eye;
                    let d = to.length();
                    if let Some(hit) = world.raycast(eye, to / d, d - 0.5) {
                        note(
                            format!("Beam: voxel occlusion ({:?})", world.get(hit.cell)),
                            format!("seed {seed}: hit {:?} at {:?} checking seat {seat_x}", world.get(hit.cell), hit.cell),
                        );
                    }
                }
            }

            for puzzle in 0..PUZZLES {
                let kind = game.def(puzzle).kind;
                let mut world = base_world.clone();
                let sites = base_sites.clone();
                crate::waking::settle(&mut world, &sites.way, puzzle);

                // Place movable loose blocks as carry::lay_out does for each stage.
                if puzzle == 0 {
                    world.set(sites.cargo_cell, Material::Limestone);
                }
                for &cell in &sites.block_pile {
                    world.set(cell, Material::Limestone);
                }

                // Build entity bounding boxes present at this stage.
                let mut boxes: Vec<(String, Vec3, Vec3)> = Vec::new();
                for (i, console) in sites.stations.iter().enumerate() {
                    let foot = console.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
                    let (lo, hi) = aabb(foot + Vec3::Y * 0.6, Vec3::new(0.86, 1.2, 0.7));
                    boxes.push((format!("console_{i}"), lo, hi));
                }
                // Avatar standing beside the open station's console (both script & approach spots).
                {
                    let c = sites.stations[puzzle].as_vec3() + Vec3::new(0.5, 0.0, 0.5);
                    for (tag, offset) in [("avatar_south", Vec3::new(0.0, 0.9, 1.2)), ("avatar_west", Vec3::new(-1.5, 0.9, 0.0))] {
                        let (lo, hi) = aabb(c + offset, Vec3::new(0.6, 1.8, 0.6));
                        boxes.push((tag.into(), lo, hi));
                    }
                }
                for (i, &foot) in sites.lanterns.iter().enumerate() {
                    let (lo, hi) = aabb(foot + Vec3::Y * 1.4, Vec3::new(0.46, 2.8, 0.46));
                    boxes.push((format!("lantern_{i}"), lo, hi));
                }
                for (i, &foot) in sites.stakes.iter().enumerate() {
                    let (lo, hi) = aabb(foot + Vec3::Y * 0.7, Vec3::new(0.09, 1.4, 0.09));
                    boxes.push((format!("stake_{i}_post"), lo, hi));
                    let (lo, hi) = aabb(foot + Vec3::new(0.0, 1.28, 0.2), Vec3::new(0.03, 0.22, 0.34));
                    boxes.push((format!("stake_{i}_pennant"), lo, hi));
                }

                if puzzle == 0 {
                    let (lo, hi) = aabb(sites.rail_start + Vec3::Y * 0.35, Vec3::new(0.9, 0.7, 0.9));
                    boxes.push(("rail_cart".into(), lo, hi));
                    let (lo, hi) = aabb(sites.rail_start + Vec3::Y * 1.1, Vec3::new(0.8, 0.8, 0.8));
                    boxes.push(("rail_cargo".into(), lo, hi));
                } else {
                    let hc = sites.haul_at(-0.45) + Vec3::Y * 0.35;
                    let (lo, hi) = aabb(hc, Vec3::new(0.9, 0.7, 0.9));
                    boxes.push(("haul_cart".into(), lo, hi));
                }
                let cr = sites.haul_at(sites.crawler_start as f64 + 0.7) + Vec3::Y * 0.45;
                let (lo, hi) = aabb(cr, Vec3::new(1.4, 0.9, 0.9));
                boxes.push(("crawler".into(), lo, hi));
                if puzzle < 2 {
                    let (lo, hi) = aabb(cr + Vec3::Y * 0.65, Vec3::new(0.7, 0.4, 0.55));
                    boxes.push(("sight_on_crawler".into(), lo, hi));
                }
                let (lo, hi) = aabb(sites.tower_release + Vec3::Y * 0.4, Vec3::splat(0.8));
                boxes.push(("tower_mass".into(), lo, hi));
                let (lo, hi) = aabb(sites.mast_release + Vec3::Y * 0.4, Vec3::splat(0.8));
                boxes.push(("mast_mass".into(), lo, hi));
                let (lo, hi) = aabb(sites.gate_stand + Vec3::Y * 0.52, Vec3::new(0.5, 1.04, 0.5));
                boxes.push(("gate_stand".into(), lo, hi));

                // Ledge launcher rig (jack frame + gauge + fitted sight).
                let ledge = sites.channel_launch;
                let (out, side) = (ledge.direction, ledge.direction.cross(Vec3::Y).normalize());
                let frame_centre = sites.channel_well.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
                let tall = Ch1Layout::MAX_STACK as f32 + 0.6;
                for (u, v) in [(-0.43, -0.58), (0.43, -0.58), (-0.43, 0.58), (0.43, 0.58)] {
                    let at = frame_centre + out * u + side * v + Vec3::Y * tall / 2.0;
                    let (lo, hi) = aabb(at, Vec3::new(0.14, tall + 0.16, 0.14));
                    boxes.push((format!("jack_upright_{u}_{v}"), lo, hi));
                }
                let gauge_channel = ledge.origin - ledge.direction * 1.5 + side * 1.6;
                let (lo, hi) = aabb(gauge_channel + Vec3::Y * 1.35, Vec3::new(0.2, 2.7, 0.56));
                boxes.push(("channel_gauge".into(), lo, hi));
                if puzzle >= 3 {
                    let fit = crate::vessel::fitted_at(&sites);
                    let (lo, hi) = aabb(fit + Vec3::Y * 0.2, crate::vessel::SIGHT_SIZE);
                    boxes.push(("fitted_sight".into(), lo, hi));
                }
                if puzzle >= 4 {
                    let casket_c = ledge.at(game.layout.platform_centre, 0.0, GROUND as f32) + Vec3::Y * 0.45;
                    let (lo, hi) = aabb(casket_c, Vec3::splat(0.9));
                    boxes.push(("casket_on_platform".into(), lo, hi));
                }

                // Tilting launcher rig (hinge + guide arm rails at rest + gauge + tiller).
                let ridge = sites.ridge_launch;
                let r_side = ridge.direction.cross(Vec3::Y).normalize();
                let (lo, hi) = aabb(ridge.origin - Vec3::Y * 0.1, Vec3::new(0.92, 0.27, 0.25));
                boxes.push(("ridge_hinge".into(), lo, hi));
                // Sample segments along the tilted guide arm rails at resting angle (20 deg).
                let rest = 20.0_f32.to_radians();
                let rail_dir = ridge.direction * rest.cos() + Vec3::Y * rest.sin();
                for rail_side in [-0.33_f32, 0.33] {
                    for k in 0..10 {
                        let s = (k as f32 + 0.5) * 0.22;
                        let pt = ridge.origin + r_side * rail_side + Vec3::Y * 0.05 + rail_dir * s;
                        let (lo, hi) = aabb(pt, Vec3::new(0.08, 0.12, 0.22));
                        boxes.push((format!("ridge_rail_{rail_side}"), lo, hi));
                    }
                }
                let gauge_ridge = ridge.origin - ridge.direction * 1.5 + r_side * 1.6;
                let (lo, hi) = aabb(gauge_ridge + Vec3::Y * 1.35, Vec3::new(0.2, 2.7, 0.56));
                boxes.push(("ridge_gauge".into(), lo, hi));
                if puzzle == 4 {
                    let tiller_c = ridge.origin - ridge.direction * 1.5 + Vec3::Y * 0.25;
                    let (lo, hi) = aabb(tiller_c, Vec3::new(0.5, 0.5, 2.4));
                    boxes.push(("tiller".into(), lo, hi));
                }

                let check_view = |world: &crate::voxel::VoxelWorld,
                                  sites: &Sites,
                                  extra_boxes: &[(String, Vec3, Vec3)],
                                  pins_to_check: &[(&str, Vec3)],
                                  note: &mut dyn FnMut(String, String)| {
                    let mut orbit = OrbitCamera::default();
                    frame_station_view(&mut orbit, kind, &game, sites, false);
                    let focus = orbit.watch.unwrap();
                    orbit.follow(focus, 1.0);
                    let reach = orbit.reach(world);
                    if reach < orbit.distance - 0.1 {
                        note(
                            format!("{kind:?}: camera pulled in by terrain"),
                            format!("seed {seed}: want {:.1} m, got {:.1} m (focus={focus})", orbit.distance, reach),
                        );
                    }
                    let eye = focus + crate::player::back(orbit.yaw, orbit.pitch) * reach;

                    let mut screen_pts: Vec<(&str, Vec2)> = Vec::new();
                    for &(label, pos) in pins_to_check {
                        match project(eye, focus, pos) {
                            None => note(format!("{kind:?} | {label}: behind camera"), format!("seed {seed}")),
                            Some(s) => {
                                screen_pts.push((label, s));
                                if s.x < 20.0 || s.x > 1420.0 || s.y < 20.0 || s.y > 1020.0 {
                                    note(
                                        format!("{kind:?} | {label}: off-screen or under station/hint panel"),
                                        format!("seed {seed}: screen=({:.0}, {:.0})", s.x, s.y),
                                    );
                                }
                                if s.x < 480.0 && s.y < 150.0 {
                                    note(format!("{kind:?} | {label}: under top-left dock"), format!("seed {seed}: screen=({:.0}, {:.0})", s.x, s.y));
                                }
                                if s.x < 360.0 && s.y > 900.0 {
                                    note(format!("{kind:?} | {label}: under bottom-left ruler panel"), format!("seed {seed}: screen=({:.0}, {:.0})", s.x, s.y));
                                }
                            }
                        }

                        let to_pin = pos - eye;
                        let dist = to_pin.length();
                        let dir = to_pin / dist;

                        if let Some(hit) = world.raycast(eye, dir, dist - 0.17) {
                            let mat = world.get(hit.cell);
                            note(
                                format!("{kind:?} | {label}: voxel occlusion ({mat:?})"),
                                format!("seed {seed}: hit {:?} at {:?} (d={:.2} vs pin {:.2}, eye={eye:.1}, pin={pos:.1})", mat, hit.cell, hit.distance, dist),
                            );
                        }

                        for (name, lo, hi) in boxes.iter().chain(extra_boxes.iter()) {
                            if let Some((t_enter, _)) = ray_aabb(eye, dir, *lo, *hi)
                                && t_enter < dist - 0.17
                            {
                                note(
                                    format!("{kind:?} | {label}: entity occlusion ({name})"),
                                    format!("seed {seed}: hit {name} at t={:.2} vs pin {:.2} (eye={eye:.1}, pin={pos:.1})", t_enter, dist),
                                );
                            }
                        }
                    }

                    for i in 0..screen_pts.len() {
                        for j in i + 1..screen_pts.len() {
                            if screen_pts[i].0 == "Light gate" || screen_pts[j].0 == "Light gate" {
                                continue;
                            }
                            let d = screen_pts[i].1.distance(screen_pts[j].1);
                            if d < 30.0 {
                                note(
                                    format!("{kind:?} | pins too close on screen: {} & {}", screen_pts[i].0, screen_pts[j].0),
                                    format!("seed {seed}: dist={d:.1}px"),
                                );
                            }
                        }
                    }
                };

                let pin = |s: &Sites, label: &str| s.pins.iter().find(|p| p.label == label).unwrap().position;

                match kind {
                    PuzzleKind::Rail => {
                        let pins = [
                            ("Rail cart: starting point", pin(&sites, "Rail cart: starting point")),
                            ("Rail marker: centre", pin(&sites, "Rail marker: centre")),
                        ];
                        check_view(&world, &sites, &[], &pins, &mut note);
                    }
                    PuzzleKind::Chase => {
                        let pins = [
                            ("Haul cart: front coupler", pin(&sites, "Haul cart: front coupler")),
                            ("Crawler: rear coupler", pin(&sites, "Crawler: rear coupler")),
                            ("Transfer marker: centre", pin(&sites, "Transfer marker: centre")),
                            ("Timing post A", pin(&sites, "Timing post A")),
                            ("Timing post B", pin(&sites, "Timing post B")),
                        ];
                        check_view(&world, &sites, &[], &pins, &mut note);
                    }
                    PuzzleKind::Gravity => {
                        // Test every notch 1..=tower_h on the drop tower.
                        for notch in 1..=game.layout.tower_h {
                            let gate_pos = crate::carry::gate_at(&sites, notch);
                            let mut gate_boxes = Vec::new();
                            let (lo, hi) = aabb(gate_pos + Vec3::new(-2.0, 0.0, 0.0), Vec3::new(1.2, 0.4, 1.6));
                            gate_boxes.push(("light_gate_clamp".into(), lo, hi));
                            for side_z in [-1.0_f32, 1.0] {
                                let (lo, hi) = aabb(gate_pos + Vec3::new(-1.0, 0.0, side_z * 0.68), Vec3::new(2.0, 0.12, 0.12));
                                gate_boxes.push((format!("light_gate_arm_{side_z}"), lo, hi));
                                let (lo, hi) = aabb(gate_pos + Vec3::new(0.0, 0.0, side_z * 0.68), Vec3::new(0.24, 0.24, 0.2));
                                gate_boxes.push((format!("light_gate_head_{side_z}"), lo, hi));
                                let (lo, hi) = aabb(gate_pos + Vec3::new(0.0, 0.0, side_z * 0.56), Vec3::new(0.1, 0.1, 0.04));
                                gate_boxes.push((format!("light_gate_eye_{side_z}"), lo, hi));
                            }
                            let pins = [
                                ("Drop tower: release point", pin(&sites, "Drop tower: release point")),
                                ("Drop tower: landing plate", pin(&sites, "Drop tower: landing plate")),
                                ("Survey mast: release point", pin(&sites, "Survey mast: release point")),
                                ("Survey mast: landing plate", pin(&sites, "Survey mast: landing plate")),
                                ("Light gate", gate_pos),
                            ];
                            check_view(&world, &sites, &gate_boxes, &pins, &mut note);
                        }
                    }
                    PuzzleKind::Channel => {
                        // Test every possible block stack 0..=MAX_STACK in the jack frame.
                        for stack in 0..=Ch1Layout::MAX_STACK {
                            let mut w = world.clone();
                            let mut s = sites.clone();
                            let mut dirty = crate::carry::DirtyCells::default();
                            crate::carry::set_stack(&mut w, &mut s, &mut dirty, stack);
                            let ledge = s.channel_launch;
                            let cradle_c = ledge.origin - ledge.direction * 0.5 + Vec3::Y * (0.02 - 0.14 / 2.0);
                            let (clo, chi) = aabb(cradle_c, Vec3::new(1.06, 0.14, 1.06));
                            let casket_c = ledge.origin - ledge.direction * 0.5 + Vec3::Y * 0.45;
                            let (klo, khi) = aabb(casket_c, Vec3::splat(0.9));
                            let extra = vec![("cradle".into(), clo, chi), ("casket_on_launcher".into(), klo, khi)];
                            let pins =
                                [("Ledge launcher: release point", s.pins[s.channel_release_pin].position), ("Platform: centre", pin(&s, "Platform: centre"))];
                            check_view(&w, &s, &extra, &pins, &mut note);
                        }
                    }
                    PuzzleKind::Ridge => {
                        let pins = [
                            ("Tilting launcher: release point", pin(&sites, "Tilting launcher: release point")),
                            ("Ridge: top, near edge", pin(&sites, "Ridge: top, near edge")),
                            ("Ridge: foot, near side", pin(&sites, "Ridge: foot, near side")),
                            ("Landing circle: centre", pin(&sites, "Landing circle: centre")),
                        ];
                        check_view(&world, &sites, &[], &pins, &mut note);
                    }
                }
            }
        }

        if !counts.is_empty() {
            let mut report = format!("Station visibility issues across {seeds} seeds:\n");
            for (k, v) in &counts {
                report.push_str(&format!("  [{v}] {k} (e.g. {})\n", examples[k]));
            }
            panic!("{report}");
        }
    }
}
