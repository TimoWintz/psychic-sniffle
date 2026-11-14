//! Demonstrations and tests played out in the world.
//!
//! The outcome of a test is decided by `physics_core` at the moment of commit, and the path of every
//! demonstration body by `physics_core::demo`. This module only shows them: an object follows the
//! exact path until the judged event (the landing or the strike), and is then handed to the physics
//! engine, which bounces and settles it. Nothing the engine does can change the result.

use crate::island::{GROUND, LaunchSite, Sites};
use crate::player::OrbitCamera;
use crate::state::{Game, PageEvent, RECHARGE_SECONDS, Ui};
use crate::voxel::VoxelWorld;
use avian3d::prelude::*;
use bevy::prelude::*;
use physics_core::ballistics::{Flight, Projectile};
use physics_core::demo::{DemoPlayback, Shot};
use physics_core::expr::evaluate;
use physics_core::materials::Material;
use physics_core::progress::CommittedValue;
use physics_core::puzzle::{Inputs, Outcome, Playback, PuzzleKind};
use physics_core::units::Quantity;

/// Something the interface asks the simulation to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Request {
    /// Run a station's demonstration (free, repeatable). At the drop tower it is also the stopwatch.
    Demo(usize),
    /// Commit the blueprint of a puzzle and run the test.
    Commit(usize),
}

/// Seconds the two demonstration bodies wait side by side before they go, so both can be seen.
const PAIR_HOLD: f32 = 0.7;
/// Colours of the two demonstration bodies and their trails.
const PAIR_COLOURS: [[f32; 3]; 2] = [[0.35, 0.85, 1.0], [1.0, 0.4, 0.75]];
/// Colour of a test's trail.
const TRAIL: [f32; 3] = [1.0, 0.85, 0.2];

enum Running {
    /// The test mass falls from the drop tower; a flash marks it at each of `flashes`.
    Strobe { puzzle: usize, t: f32, flashes: Vec<f64>, shown: usize, guess: Option<usize> },
    /// Two demonstration bodies fly their exact paths from a launch site.
    Pair { puzzle: usize, t: f32, shots: [Shot; 2], site: LaunchSite, offsets: [Vec3; 2], size: f32, landed: [bool; 2], guess: Option<usize> },
    MastDrop { puzzle: usize, t: f32, predicted: f32, actual: f32, verdict: Verdict },
    Flight { puzzle: usize, t: f32, projectile: Projectile, flight: Flight, site: LaunchSite, verdict: Verdict },
    Slide { puzzle: usize, t: f32, distance: f32, duration: f32, verdict: Verdict },
}

/// A judged attempt, kept back until the world has shown what happened.
#[derive(Clone)]
struct Verdict {
    values: Vec<CommittedValue>,
    outcome: Outcome,
}

#[derive(Resource, Default)]
pub struct Sim {
    pub request: Option<Request>,
    running: Option<Running>,
    /// Paths of the last test or demonstration with their colours, drawn until the next one.
    pub trails: Vec<(Vec<Vec3>, [f32; 3])>,
    /// Where the strobe caught the test mass (its centre), drawn as ticks until the next release.
    strobe: Vec<Vec3>,
    /// Seconds until the camera stops watching the test.
    watch_left: f32,
}

impl Sim {
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    /// The puzzle whose test is running, if one is (a demonstration is not a test).
    pub fn testing(&self) -> Option<usize> {
        match &self.running {
            Some(Running::MastDrop { puzzle, .. } | Running::Flight { puzzle, .. } | Running::Slide { puzzle, .. }) => Some(*puzzle),
            Some(Running::Strobe { .. } | Running::Pair { .. }) | None => None,
        }
    }

    /// The puzzle whose demonstration is running, if one is.
    pub fn demo(&self) -> Option<usize> {
        match &self.running {
            Some(Running::Strobe { puzzle, .. } | Running::Pair { puzzle, .. }) => Some(*puzzle),
            _ => None,
        }
    }

    fn start_trail(&mut self, colour: [f32; 3]) {
        self.trails = vec![(Vec::new(), colour)];
    }
}

/// The objects that tests move.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum Prop {
    TowerMass,
    MastMass,
    /// Follows the player's prediction during the mast test.
    Ghost,
    Crate,
    Beam,
    /// The tutorial's rail cart.
    Cart,
}

/// A body handed to the physics engine after a test; removed when its time is up.
#[derive(Component)]
struct Settling {
    left: f32,
    after: AfterSettling,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum AfterSettling {
    /// Removed, and the prop put back on its launcher.
    Restore(Prop),
    /// Left where it came to rest (the beam on the pad).
    Stay,
    /// Removed (a demonstration body).
    Vanish,
}

/// A demonstration body while it follows its exact path.
#[derive(Component)]
struct DemoBody(usize);

/// A strobe flash at the drop tower: kept until the next release.
#[derive(Component)]
struct StrobeMark;

#[derive(Resource)]
struct PropAssets {
    alloy: Handle<StandardMaterial>,
    pair: [Handle<StandardMaterial>; 2],
    afterimage: Handle<StandardMaterial>,
    mass: Handle<Mesh>,
}

const MASS_SIZE: Vec3 = Vec3::new(0.8, 0.8, 0.8);
const CRATE_SIZE: Vec3 = Vec3::new(0.9, 0.9, 0.9);
const BEAM_SIZE: Vec3 = Vec3::new(2.4, 0.5, 0.5);
const CART_SIZE: Vec3 = Vec3::new(0.9, 0.7, 0.9);

fn prop_size(prop: Prop) -> Vec3 {
    match prop {
        Prop::TowerMass | Prop::MastMass | Prop::Ghost => MASS_SIZE,
        Prop::Crate => CRATE_SIZE,
        Prop::Beam => BEAM_SIZE,
        Prop::Cart => CART_SIZE,
    }
}

pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Sim>()
            .add_systems(Startup, setup)
            .add_systems(Update, (handle_requests, advance, settle, recharge, draw_trail).chain());
    }
}

/// Where a prop rests when nothing is happening: transform of its centre.
fn rest_pose(prop: Prop, sites: &Sites) -> Vec3 {
    let half = Vec3::Y * prop_size(prop).y / 2.0;
    match prop {
        Prop::TowerMass => sites.tower_release + half,
        Prop::MastMass | Prop::Ghost => sites.mast_release + half,
        Prop::Crate => sites.canyon_launch.origin - sites.canyon_launch.direction * 2.5 + half,
        Prop::Beam => sites.ridge_launch.origin - sites.ridge_launch.direction * 1.5 + half,
        Prop::Cart => sites.rail_start + half,
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    world: Res<VoxelWorld>,
    sites: Res<Sites>,
    game: Res<Game>,
) {
    let solid = |materials: &mut Assets<StandardMaterial>, c: [f32; 3]| {
        materials.add(StandardMaterial { base_color: Color::srgb(c[0], c[1], c[2]), perceptual_roughness: 0.6, ..default() })
    };
    let steel = solid(&mut materials, [0.75, 0.16, 0.12]);
    let timber = solid(&mut materials, Material::Timber.properties().colour);
    let alloy = solid(&mut materials, Material::HullAlloy.properties().colour);
    let ghost = materials.add(StandardMaterial {
        base_color: Color::srgba(0.4, 0.8, 1.0, 0.35),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for (prop, material, visible) in [
        (Prop::TowerMass, steel.clone(), true),
        (Prop::MastMass, steel.clone(), true),
        (Prop::Ghost, ghost, false),
        (Prop::Crate, timber, true),
        (Prop::Beam, alloy.clone(), true),
        (Prop::Cart, steel.clone(), true),
    ] {
        commands.spawn((
            prop,
            Mesh3d(meshes.add(Cuboid::from_size(prop_size(prop)))),
            MeshMaterial3d(material),
            Transform::from_translation(rest_pose(prop, &sites)),
            if visible { Visibility::Visible } else { Visibility::Hidden },
        ));
    }

    // The physics engine's view of the island: every block anything could touch.
    commands.insert_resource(Gravity(Vec3::NEG_Y * game.layout.g as f32));
    commands.spawn((RigidBody::Static, Collider::voxels(Vec3::ONE, &world.exposed_blocks()), Transform::default()));

    // Loose wreckage, left to the engine: it falls and settles where it will.
    for i in 0..10 {
        let size = 0.5 + 0.25 * (i % 3) as f32;
        let offset = Vec3::new((i as f32 * 1.7) % 6.0 - 3.0, 3.0 + i as f32 * 0.9, (i as f32 * 2.3) % 5.0 - 6.5);
        commands.spawn((
            RigidBody::Dynamic,
            Collider::cuboid(size, size, size),
            Mesh3d(meshes.add(Cuboid::from_size(Vec3::splat(size)))),
            MeshMaterial3d(alloy.clone()),
            Transform::from_translation(sites.wreck + offset).with_rotation(Quat::from_rotation_z(i as f32 * 0.7)),
        ));
    }
    let unlit = |materials: &mut Assets<StandardMaterial>, c: [f32; 3]| {
        materials.add(StandardMaterial { base_color: Color::srgb(c[0], c[1], c[2]), unlit: true, ..default() })
    };
    let pair = PAIR_COLOURS.map(|c| unlit(&mut materials, c));
    let afterimage = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.72, 0.25, 0.5),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.insert_resource(PropAssets {
        alloy,
        pair,
        afterimage,
        mass: meshes.add(Cuboid::from_size(MASS_SIZE)),
    });
}

/// Read the blueprint: evaluate each typed expression against the notebook.
pub fn read_blueprint(game: &Game, ui: &Ui, puzzle: usize) -> Result<(Inputs, Vec<CommittedValue>), String> {
    let mut inputs = Inputs::new();
    let mut values = Vec::new();
    for (i, spec) in game.instance(puzzle).inputs().iter().enumerate() {
        let text = ui.drafts[puzzle].get(i).map_or("", String::as_str);
        if text.trim().is_empty() {
            return Err(format!("{} has not been entered.", spec.label));
        }
        let quantity = evaluate(text, &game.save.notebook).map_err(|e| format!("{}: {e}", spec.label))?;
        inputs.insert(spec.key.to_string(), quantity);
        values.push(CommittedValue { label: spec.label.to_string(), quantity, unit: spec.unit.to_string() });
    }
    Ok((inputs, values))
}

#[allow(clippy::too_many_arguments)]
fn handle_requests(
    mut commands: Commands,
    mut sim: ResMut<Sim>,
    mut game: ResMut<Game>,
    mut ui: ResMut<Ui>,
    mut orbit: ResMut<OrbitCamera>,
    sites: Res<Sites>,
    assets: Res<PropAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut props: Query<(&Prop, &mut Transform, &mut Visibility)>,
    settling: Query<(Entity, &Settling)>,
    marks: Query<Entity, With<StrobeMark>>,
) {
    let Some(request) = sim.request.take() else { return };
    if sim.busy() {
        return;
    }
    // Put back anything still lying around from the previous run, so each run starts loaded.
    for (entity, body) in &settling {
        if body.after != AfterSettling::Stay {
            commands.entity(entity).despawn();
        }
    }
    for (prop, mut transform, mut visibility) in &mut props {
        if *prop != Prop::Ghost {
            *transform = Transform::from_translation(rest_pose(*prop, &sites));
            *visibility = Visibility::Visible;
        }
    }
    match request {
        Request::Demo(puzzle) => {
            if !game.unlocked(puzzle) {
                return;
            }
            let instance = game.instance(puzzle);
            let Some(playback) = instance.demo() else { return };
            // The guess counts only before the first run.
            let guess = if game.save.demo_watched(&game.def(puzzle).id) { None } else { ui.guess[puzzle] };
            let ground = GROUND as f32;
            sim.running = Some(match playback {
                DemoPlayback::Strobe { .. } => {
                    for mark in &marks {
                        commands.entity(mark).despawn();
                    }
                    sim.strobe.clear();
                    orbit.frame_side_view(sites.tower_plate, sites.tower_release, Vec3::X);
                    Running::Strobe { puzzle, t: 0.0, flashes: playback.flash_times(), shown: 0, guess }
                }
                DemoPlayback::DropAndFire { dropped, fired } => {
                    let site = sites.canyon_launch;
                    let low = site.at(0.0, dropped.flight.y, ground);
                    orbit.frame_side_view(low, site.at(fired.flight.x, dropped.projectile.y0, ground), site.direction);
                    let mesh = meshes.add(Cuboid::from_length(0.4));
                    start_pair(&mut commands, &mut sim, &assets, puzzle, mesh, [dropped, fired], site, 0.4, guess)
                }
                DemoPlayback::TwoAngles { flat, steep } => {
                    let site = sites.ridge_launch;
                    let (_, _, apex) = steep.projectile.apex();
                    orbit.frame_side_view(site.at(-2.0, 0.0, ground), site.at(steep.flight.x + 2.0, apex, ground), site.direction);
                    let mesh = meshes.add(Cuboid::from_length(0.3));
                    start_pair(&mut commands, &mut sim, &assets, puzzle, mesh, [flat, steep], site, 0.3, guess)
                }
            });
        }
        Request::Commit(puzzle) => {
            ui.refusal = None;
            if !game.unlocked(puzzle) {
                return;
            }
            // A first value committed here may write a page, whether it is tested or refused.
            if ui.drafts[puzzle].iter().any(|d| !d.trim().is_empty()) && game.find_pages(&mut ui, puzzle, PageEvent::Commit) > 0 {
                game.persist();
            }
            if game.charges_left(puzzle) == 0 {
                ui.refusal = Some("The station is recharging.".into());
                return;
            }
            let instance = game.instance(puzzle);
            let (inputs, values) = match read_blueprint(&game, &ui, puzzle) {
                Ok(read) => read,
                Err(problem) => {
                    ui.refusal = Some(problem);
                    return;
                }
            };
            // Wrong dimensions and impossible settings are refused here, before a charge is used.
            let outcome = match instance.evaluate(&inputs) {
                Ok(outcome) => outcome,
                Err(problem) => {
                    ui.refusal = Some(problem.to_string());
                    return;
                }
            };
            // The verdict exists now, but it is recorded and shown only when the test has played out.
            ui.results[puzzle] = None;
            sim.start_trail(TRAIL);
            let ground = GROUND as f32;
            sim.running = Some(match outcome.playback.clone() {
                Playback::Drop { predicted_time, actual_time, .. } => {
                    for (prop, _, mut visibility) in &mut props {
                        if *prop == Prop::Ghost {
                            *visibility = Visibility::Visible;
                        }
                    }
                    orbit.frame_side_view(sites.mast_plate, sites.mast_release, Vec3::X);
                    Running::MastDrop { puzzle, t: 0.0, predicted: predicted_time as f32, actual: actual_time as f32, verdict: Verdict { values, outcome } }
                }
                Playback::Flight { projectile, flight } => {
                    let site = if instance.kind == PuzzleKind::Canyon { sites.canyon_launch } else { sites.ridge_launch };
                    let (_, apex_x, apex_y) = projectile.apex();
                    let highest = site.at(apex_x, apex_y, ground);
                    let end = site.at(flight.x, flight.y, ground);
                    orbit.frame_side_view(site.at(0.0, projectile.y0, ground).min(end), highest.max(end), site.direction);
                    Running::Flight { puzzle, t: 0.0, projectile, flight, site, verdict: Verdict { values, outcome } }
                }
                Playback::Slide { distance } => {
                    let reach = distance.max(game.layout.rail_target as f64) as f32;
                    orbit.frame_side_view(sites.rail_start, sites.rail_start + Vec3::new(reach, 2.0, 0.0), Vec3::X);
                    let duration = 1.0 + distance as f32 / 6.0;
                    Running::Slide { puzzle, t: 0.0, distance: distance as f32, duration, verdict: Verdict { values, outcome } }
                }
            });
        }
    }
}

/// Set two demonstration bodies at the launch site, side by side inside the launch line's block row.
#[allow(clippy::too_many_arguments)]
fn start_pair(
    commands: &mut Commands,
    sim: &mut Sim,
    assets: &PropAssets,
    puzzle: usize,
    mesh: Handle<Mesh>,
    shots: [Shot; 2],
    site: LaunchSite,
    size: f32,
    guess: Option<usize>,
) -> Running {
    let across = site.direction.cross(Vec3::Y).normalize();
    let offsets = [across * 0.22, -across * 0.22];
    let ground = GROUND as f32;
    sim.trails = PAIR_COLOURS.iter().map(|c| (Vec::new(), *c)).collect();
    for (k, shot) in shots.iter().enumerate() {
        let (x, y) = shot.projectile.pos(0.0);
        commands.spawn((
            DemoBody(k),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(assets.pair[k].clone()),
            Transform::from_translation(site.at(x, y, ground) + offsets[k] + Vec3::Y * size / 2.0),
        ));
    }
    Running::Pair { puzzle, t: 0.0, shots, site, offsets, size, landed: [false; 2], guess }
}

/// A demonstration has played out: keep the guess, write its pages.
fn finish_demo(game: &mut Game, ui: &mut Ui, puzzle: usize, guess: Option<usize>) {
    let id = game.def(puzzle).id.clone();
    game.save.record_demo(&id, guess);
    game.find_pages(ui, puzzle, PageEvent::Demo);
    game.persist();
}

/// Give a body to the physics engine with the velocity it has at the judged moment.
fn hand_over(commands: &mut Commands, meshes: &mut Assets<Mesh>, size: Vec3, centre: Vec3, velocity: Vec3, material: Handle<StandardMaterial>, after: AfterSettling) {
    commands.spawn((
        RigidBody::Dynamic,
        Collider::cuboid(size.x, size.y, size.z),
        LinearVelocity(velocity),
        AngularVelocity(Vec3::new(0.3, 0.2, -1.2)),
        Restitution::new(0.25),
        Friction::new(0.8),
        Mesh3d(meshes.add(Cuboid::from_size(size))),
        MeshMaterial3d(material),
        Transform::from_translation(centre),
        Settling { left: 6.0, after },
    ));
}

/// Height fallen from rest after `t` seconds, stopping at `limit`.
fn fallen(g: f32, t: f32, limit: f32) -> f32 {
    (0.5 * g * t * t).min(limit)
}

#[allow(clippy::too_many_arguments)]
fn advance(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<PropAssets>,
    mut sim: ResMut<Sim>,
    mut game: ResMut<Game>,
    mut ui: ResMut<Ui>,
    mut orbit: ResMut<OrbitCamera>,
    sites: Res<Sites>,
    mut props: Query<(&Prop, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>)>,
    mut bodies: Query<(Entity, &DemoBody, &mut Transform), Without<Prop>>,
) {
    let dt = time.delta_secs();
    let g = game.layout.g as f32;
    let ground = GROUND as f32;

    // Let go of the camera a little after a test or an experiment ends (and only then: anything else
    // that points the camera keeps it).
    if sim.running.is_none() && sim.watch_left > 0.0 {
        sim.watch_left -= dt;
        if sim.watch_left <= 0.0 {
            orbit.release_view();
        }
    }
    let Some(mut running) = sim.running.take() else { return };

    let mut finished: Option<(usize, Verdict)> = None;

    match &mut running {
        Running::Strobe { puzzle, t, flashes, shown, guess } => {
            *t += dt;
            let height = game.layout.tower_h as f32;
            let fall_time = (2.0 * height / g).sqrt();
            // Each flash leaves an afterimage of the mass and a tick beside its path.
            while *shown < flashes.len() && *t >= flashes[*shown] as f32 {
                let below = fallen(g, flashes[*shown] as f32, height);
                let centre = rest_pose(Prop::TowerMass, &sites) - Vec3::Y * below;
                commands.spawn((StrobeMark, Mesh3d(assets.mass.clone()), MeshMaterial3d(assets.afterimage.clone()), Transform::from_translation(centre)));
                sim.strobe.push(centre);
                *shown += 1;
            }
            for (prop, mut transform, mut visibility, material) in &mut props {
                if *prop != Prop::TowerMass {
                    continue;
                }
                transform.translation = rest_pose(Prop::TowerMass, &sites) - Vec3::Y * fallen(g, *t, height);
                if *t >= fall_time {
                    *visibility = Visibility::Hidden;
                    hand_over(&mut commands, &mut meshes, MASS_SIZE, transform.translation, Vec3::NEG_Y * g * fall_time, material.0.clone(), AfterSettling::Restore(Prop::TowerMass));
                }
            }
            if *t >= fall_time {
                let reading = Quantity::seconds(game.layout.tower_fall_time());
                game.save.notebook.add_numbered("t", "fall time from the drop tower", reading, "stopwatch");
                let (puzzle, guess) = (*puzzle, *guess);
                finish_demo(&mut game, &mut ui, puzzle, guess);
                sim.running = None;
                sim.watch_left = 2.5;
                return;
            }
        }
        Running::Pair { puzzle, t, shots, site, offsets, size, landed, guess } => {
            *t += dt;
            let flying = (*t - PAIR_HOLD).max(0.0) as f64;
            for (entity, body, mut transform) in &mut bodies {
                let k = body.0;
                let shot = shots[k];
                let at = flying.min(shot.flight.t);
                let (x, y) = shot.projectile.pos(at);
                let centre = site.at(x, y, ground) + offsets[k] + Vec3::Y * *size / 2.0;
                transform.translation = centre;
                if flying > 0.0
                    && let Some((trail, _)) = sim.trails.get_mut(k)
                {
                    trail.push(centre - Vec3::Y * *size / 2.0);
                }
                if flying >= shot.flight.t && !landed[k] {
                    landed[k] = true;
                    let (vx, vy) = shot.projectile.vel(shot.flight.t);
                    commands.entity(entity).remove::<DemoBody>().insert((
                        RigidBody::Dynamic,
                        Collider::cuboid(*size, *size, *size),
                        LinearVelocity(site.direction * vx as f32 + Vec3::Y * vy as f32),
                        Restitution::new(0.25),
                        Friction::new(0.8),
                        Settling { left: 5.0, after: AfterSettling::Vanish },
                    ));
                }
            }
            if landed.iter().all(|l| *l) {
                let (puzzle, guess) = (*puzzle, *guess);
                finish_demo(&mut game, &mut ui, puzzle, guess);
                sim.running = None;
                sim.watch_left = 3.0;
                return;
            }
        }
        Running::MastDrop { puzzle, t, predicted, actual, verdict } => {
            *t += dt;
            let height = game.layout.mast_h as f32;
            let g_player = 2.0 * height / (*predicted * *predicted);
            for (prop, mut transform, mut visibility, material) in &mut props {
                match *prop {
                    Prop::MastMass => {
                        transform.translation = rest_pose(Prop::MastMass, &sites) - Vec3::Y * fallen(g, *t, height);
                        if *t >= *actual && *visibility != Visibility::Hidden {
                            *visibility = Visibility::Hidden;
                            hand_over(&mut commands, &mut meshes, MASS_SIZE, transform.translation, Vec3::NEG_Y * g * *actual, material.0.clone(), AfterSettling::Restore(Prop::MastMass));
                        }
                    }
                    Prop::Ghost => {
                        transform.translation = rest_pose(Prop::Ghost, &sites) - Vec3::Y * fallen(g_player, *t, height);
                    }
                    _ => {}
                }
            }
            if *t >= predicted.max(*actual) {
                finished = Some((*puzzle, verdict.clone()));
            }
        }
        Running::Flight { puzzle, t, projectile, flight, site, verdict } => {
            // A short run-up along the rail before release, so the launch reads as a launch.
            const RUN_UP: f32 = 0.35;
            *t += dt;
            let prop = if game.def(*puzzle).kind == PuzzleKind::Canyon { Prop::Crate } else { Prop::Beam };
            let half = Vec3::Y * prop_size(prop).y / 2.0;
            let flight_t = (*t - RUN_UP).clamp(0.0, flight.t as f32) as f64;
            let (x, y) = projectile.pos(flight_t);
            let mut centre = site.at(x, y, ground) + half;
            if *t < RUN_UP {
                let start = rest_pose(prop, &sites);
                let progress = *t / RUN_UP;
                centre = start.lerp(site.at(0.0, projectile.y0, ground) + half, progress * progress);
            } else if let Some((trail, _)) = sim.trails.first_mut() {
                trail.push(centre - half);
            }
            let (vx, vy) = projectile.vel(flight_t);
            let velocity = site.direction * vx as f32 + Vec3::Y * vy as f32;
            for (candidate, mut transform, mut visibility, material) in &mut props {
                if *candidate != prop {
                    continue;
                }
                transform.translation = centre;
                if prop == Prop::Beam {
                    // The beam points along its path.
                    transform.rotation = Quat::from_rotation_arc(Vec3::X, velocity.normalize_or(Vec3::X));
                }
                if *t - RUN_UP >= flight.t as f32 && *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                    // A beam that lands on the pad stays down; anything else is returned to its launcher.
                    let after = if verdict.outcome.pass && prop == Prop::Beam { AfterSettling::Stay } else { AfterSettling::Restore(prop) };
                    let material = if prop == Prop::Beam { assets.alloy.clone() } else { material.0.clone() };
                    hand_over(&mut commands, &mut meshes, prop_size(prop), centre, velocity, material, after);
                }
            }
            if *t - RUN_UP >= flight.t as f32 {
                finished = Some((*puzzle, verdict.clone()));
            }
        }
        Running::Slide { puzzle, t, distance, duration, verdict } => {
            // The motor eases the cart up to speed and brakes it to a stop at the set distance. The
            // cart stays where it stopped, so a miss can be looked at, until the next run.
            *t += dt;
            let progress = (*t / *duration).min(1.0);
            let eased = progress * progress * (3.0 - 2.0 * progress);
            let start = rest_pose(Prop::Cart, &sites);
            let centre = start + Vec3::X * *distance * eased;
            if let Some((trail, _)) = sim.trails.first_mut() {
                trail.push(centre - Vec3::Y * (CART_SIZE.y / 2.0 - 0.05));
            }
            for (prop, mut transform, _, _) in &mut props {
                if *prop == Prop::Cart {
                    transform.translation = centre;
                }
            }
            if progress >= 1.0 {
                finished = Some((*puzzle, verdict.clone()));
            }
        }
    }

    let Some((puzzle, Verdict { values, outcome })) = finished else {
        sim.running = Some(running);
        return;
    };
    // The test has played out: now it counts.
    sim.watch_left = 2.5;
    if outcome.pass && game.def(puzzle).kind == PuzzleKind::Gravity {
        // The player's own measurement becomes the g that later puzzles use.
        game.save.notebook.set("g", "gravitational acceleration (your calibration)", values[0].quantity, "calibrated");
    }
    let id = game.def(puzzle).id.clone();
    game.save.record(&id, values, &outcome);
    game.persist();
    for (prop, _, mut visibility, _) in &mut props {
        if *prop == Prop::Ghost {
            *visibility = Visibility::Hidden;
        }
    }
    ui.results[puzzle] = Some(outcome);
}

/// Remove settled bodies when their time is up and put the launcher's load back.
fn settle(time: Res<Time>, mut commands: Commands, sites: Res<Sites>, mut settling: Query<(Entity, &mut Settling)>, mut props: Query<(&Prop, &mut Transform, &mut Visibility)>) {
    for (entity, mut body) in &mut settling {
        body.left -= time.delta_secs();
        if body.left > 0.0 {
            continue;
        }
        let restores = match body.after {
            AfterSettling::Stay => {
                // The beam on the pad stays where it came to rest.
                commands.entity(entity).remove::<Settling>();
                continue;
            }
            AfterSettling::Vanish => {
                commands.entity(entity).despawn();
                continue;
            }
            AfterSettling::Restore(prop) => prop,
        };
        commands.entity(entity).despawn();
        for (prop, mut transform, mut visibility) in &mut props {
            if *prop == restores {
                *transform = Transform::from_translation(rest_pose(restores, &sites));
                *visibility = Visibility::Visible;
            }
        }
    }
}

/// A station with no charges left recharges after a wait.
fn recharge(time: Res<Time>, mut game: ResMut<Game>, mut ui: ResMut<Ui>, sim: Res<Sim>) {
    for puzzle in 0..game.chapter.puzzles.len() {
        if game.charges_left(puzzle) > 0 || sim.busy() {
            continue;
        }
        let left = ui.recharging[puzzle].unwrap_or(RECHARGE_SECONDS) - time.delta_secs();
        if left > 0.0 {
            ui.recharging[puzzle] = Some(left);
        } else {
            ui.recharging[puzzle] = None;
            let id = game.def(puzzle).id.clone();
            game.save.recharge(&id);
            game.persist();
        }
    }
}

fn draw_trail(sim: Res<Sim>, mut gizmos: Gizmos) {
    // Lines keep their width on screen, so the strobe's ticks read from a distance.
    for at in &sim.strobe {
        let base = *at - Vec3::Y * MASS_SIZE.y / 2.0;
        gizmos.line(base + Vec3::X * (MASS_SIZE.x / 2.0 + 0.2), base + Vec3::X * 3.2, Color::srgb(1.0, 0.85, 0.3));
    }
    for (trail, c) in &sim.trails {
        if trail.len() >= 2 {
            gizmos.linestrip(trail.iter().copied(), Color::srgb(c[0], c[1], c[2]));
        }
    }
}
