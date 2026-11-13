//! Experiments and tests played out in the world.
//!
//! The outcome of a test is decided by `physics_core` at the moment of commit. This module only shows
//! it: an object follows the exact path until the judged event (the landing or the strike), and is
//! then handed to the physics engine, which bounces and settles it. Nothing the engine does can
//! change the result.

use crate::island::{GROUND, LaunchSite, Sites};
use crate::player::OrbitCamera;
use crate::state::{Game, RECHARGE_SECONDS, Ui};
use crate::voxel::VoxelWorld;
use avian3d::prelude::*;
use bevy::prelude::*;
use physics_core::ballistics::{Flight, Projectile};
use physics_core::expr::evaluate;
use physics_core::materials::Material;
use physics_core::progress::CommittedValue;
use physics_core::puzzle::{Inputs, Outcome, Playback, PuzzleKind};
use physics_core::units::Quantity;

/// Something the interface asks the simulation to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Request {
    /// Release the drop tower's test mass and time it (free, repeatable).
    ReleaseTowerMass,
    /// Commit the blueprint of a puzzle and run the test.
    Commit(usize),
}

enum Running {
    TowerDrop { t: f32 },
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
    /// Path of the last flight, drawn in the world until the next one.
    pub trail: Vec<Vec3>,
    /// Seconds until the camera stops watching the test.
    watch_left: f32,
}

impl Sim {
    pub fn busy(&self) -> bool {
        self.running.is_some()
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
    restores: Option<Prop>,
}

#[derive(Resource)]
struct PropAssets {
    alloy: Handle<StandardMaterial>,
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
    commands.insert_resource(PropAssets { alloy });
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
    game: Res<Game>,
    mut ui: ResMut<Ui>,
    mut orbit: ResMut<OrbitCamera>,
    sites: Res<Sites>,
    mut props: Query<(&Prop, &mut Transform, &mut Visibility)>,
    settling: Query<(Entity, &Settling)>,
) {
    let Some(request) = sim.request.take() else { return };
    if sim.busy() {
        return;
    }
    // Put back anything still lying around from the previous run, so each run starts loaded.
    for (entity, body) in &settling {
        if body.restores.is_some() {
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
        Request::ReleaseTowerMass => sim.running = Some(Running::TowerDrop { t: 0.0 }),
        Request::Commit(puzzle) => {
            ui.refusal = None;
            if !game.unlocked(puzzle) {
                return;
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
            sim.trail.clear();
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
) {
    let dt = time.delta_secs();
    let g = game.layout.g as f32;
    let ground = GROUND as f32;

    // Let go of the camera a little after a test ends.
    if sim.running.is_none() && orbit.watch.is_some() {
        sim.watch_left -= dt;
        if sim.watch_left <= 0.0 {
            orbit.release_view();
        }
    }
    let Some(mut running) = sim.running.take() else { return };

    let mut hand_over = |prop: Prop, centre: Vec3, velocity: Vec3, material: Handle<StandardMaterial>, restores: Option<Prop>| {
        let size = prop_size(prop);
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
            Settling { left: 6.0, restores },
        ));
    };
    let mut finished: Option<(usize, Verdict)> = None;

    match &mut running {
        Running::TowerDrop { t } => {
            *t += dt;
            let height = game.layout.tower_h as f32;
            let fall_time = (2.0 * height / g).sqrt();
            for (prop, mut transform, mut visibility, material) in &mut props {
                if *prop != Prop::TowerMass {
                    continue;
                }
                transform.translation = rest_pose(Prop::TowerMass, &sites) - Vec3::Y * fallen(g, *t, height);
                if *t >= fall_time {
                    *visibility = Visibility::Hidden;
                    hand_over(Prop::TowerMass, transform.translation, Vec3::NEG_Y * g * fall_time, material.0.clone(), Some(Prop::TowerMass));
                }
            }
            if *t >= fall_time {
                let reading = Quantity::seconds(game.layout.tower_fall_time());
                game.save.notebook.add_numbered("t", "fall time from the drop tower", reading, "stopwatch");
                game.persist();
                sim.running = None;
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
                            hand_over(Prop::MastMass, transform.translation, Vec3::NEG_Y * g * *actual, material.0.clone(), Some(Prop::MastMass));
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
            } else {
                sim.trail.push(centre - half);
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
                    let restores = (!(verdict.outcome.pass && prop == Prop::Beam)).then_some(prop);
                    let material = if prop == Prop::Beam { assets.alloy.clone() } else { material.0.clone() };
                    hand_over(prop, centre, velocity, material, restores);
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
            sim.trail.push(centre - Vec3::Y * (CART_SIZE.y / 2.0 - 0.05));
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
        let Some(restores) = body.restores else {
            // The beam on the pad stays where it came to rest.
            commands.entity(entity).remove::<Settling>();
            continue;
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
    #[expect(clippy::needless_range_loop, reason = "the index names the puzzle in three places")]
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
    if sim.trail.len() >= 2 {
        gizmos.linestrip(sim.trail.iter().copied(), Color::srgb(1.0, 0.85, 0.2));
    }
}
