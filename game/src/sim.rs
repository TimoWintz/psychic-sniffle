//! Demonstrations and tests played out in the world.
//!
//! The outcome of a test is decided by `physics_core` at the moment of commit, and the path of every
//! demonstration body by `physics_core::demo`. This module only shows them: an object follows the
//! exact path until the judged event (the landing or the strike), and is then handed to the physics
//! engine, which bounces and settles it. Nothing the engine does can change the result.

use crate::carry::{Carry, Slot};
use crate::island::{GROUND, LaunchSite, Sites};
use crate::player::OrbitCamera;
use crate::props::{self, Finishes, Vehicle};
use crate::state::{Game, PageEvent, RECHARGE_SECONDS, Ui};
use crate::voxel::VoxelWorld;
use avian3d::prelude::*;
use bevy::prelude::*;
use physics_core::ballistics::{Flight, Projectile};
use physics_core::demo::{DemoPlayback, Shot};
use physics_core::expr::evaluate;
use physics_core::materials::Material;
use physics_core::progress::CommittedValue;
use physics_core::puzzle::{InputSource, Inputs, Outcome, Playback, PuzzleKind};
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
    /// The test mass falls from the drop tower; a flash marks it at each of `flashes`. With the
    /// light gate clamped `gate` metres below the release (0 for none), the stopwatch stops there.
    Strobe { puzzle: usize, t: f32, flashes: Vec<f64>, shown: usize, guess: Option<usize>, gate: u32 },
    /// Two demonstration bodies fly their exact paths from a launch site.
    Pair { puzzle: usize, t: f32, shots: [Shot; 2], site: LaunchSite, offsets: [Vec3; 2], size: f32, landed: [bool; 2], guess: Option<usize> },
    MastDrop { puzzle: usize, t: f32, predicted: f32, actual: f32, verdict: Verdict },
    Flight { puzzle: usize, t: f32, projectile: Projectile, flight: Flight, site: LaunchSite, verdict: Verdict },
    Slide { puzzle: usize, t: f32, distance: f32, duration: f32, verdict: Verdict },
    /// The haul cart and the crawler set off from their marks at steady speeds; a flash marks both at
    /// each of `flashes`. Positions are along the track (the cart's front coupler, the crawler's rear).
    Pace { puzzle: usize, t: f32, flashes: Vec<f64>, shown: usize, guess: Option<usize>, run: Run },
    /// The haul cart sent after the crawler, until it reaches it or the crawler reaches the buffer.
    Chase { puzzle: usize, t: f32, run: Run, verdict: Verdict },
}

/// The haul cart and the crawler on the move: both from their marks, each at its steady speed, for
/// `end` seconds.
#[derive(Clone, Copy)]
struct Run {
    cart_speed: f32,
    crawler_speed: f32,
    head_start: f32,
    end: f32,
}

impl Run {
    /// Where the cart's front coupler and the crawler's rear coupler are at `t`, along the track.
    fn at(&self, t: f32) -> (f32, f32) {
        let t = t.clamp(0.0, self.end);
        (self.cart_speed * t, self.head_start + self.crawler_speed * t)
    }
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
    /// Where the moving object of the running test or experiment is.
    action: Option<Vec3>,
    /// The test mass has passed the light gate since it was last released: its beam shows it.
    pub gate_tripped: bool,
}

impl Sim {
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    /// The puzzle whose test is running, if one is (a demonstration is not a test).
    pub fn testing(&self) -> Option<usize> {
        match &self.running {
            Some(
                Running::MastDrop { puzzle, .. } | Running::Flight { puzzle, .. } | Running::Slide { puzzle, .. } | Running::Chase { puzzle, .. },
            ) => Some(*puzzle),
            Some(Running::Strobe { .. } | Running::Pair { .. } | Running::Pace { .. }) | None => None,
        }
    }

    /// Where the object a test or an experiment moves is now, while one runs.
    pub fn action_point(&self) -> Option<Vec3> {
        self.running.as_ref().and(self.action)
    }

    /// The puzzle whose demonstration is running, if one is.
    pub fn demo(&self) -> Option<usize> {
        match &self.running {
            Some(Running::Strobe { puzzle, .. } | Running::Pair { puzzle, .. } | Running::Pace { puzzle, .. }) => Some(*puzzle),
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
    /// The cart's cargo crate, while it is loaded: it rides on the cart.
    Cargo,
    /// The haul cart, behind its front coupler.
    HaulCart,
    /// The survey crawler, ahead of its rear coupler.
    Crawler,
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
    /// What the ledge launcher and the tilting launcher throw: a crate and the frame beam.
    crate_mesh: Handle<Mesh>,
    beam: Handle<Mesh>,
    /// The haul cart's and the crawler's shapes, for their afterimages.
    haul: [Vehicle; 2],
}

const MASS_SIZE: Vec3 = Vec3::new(0.8, 0.8, 0.8);
const CRATE_SIZE: Vec3 = Vec3::new(0.9, 0.9, 0.9);
const BEAM_SIZE: Vec3 = Vec3::new(2.4, 0.5, 0.5);
const CART_SIZE: Vec3 = Vec3::new(0.9, 0.7, 0.9);
const CARGO_SIZE: Vec3 = Vec3::new(0.8, 0.8, 0.8);
/// The haul cart and the crawler, as long along the track as the first number. The crawler's body
/// ends short of the buffer when its rear coupler is at the track's length.
const HAUL_CART_SIZE: Vec3 = Vec3::new(0.9, 0.7, 0.9);
const CRAWLER_SIZE: Vec3 = Vec3::new(1.4, 0.9, 0.9);

/// Centre of the haul cart with its front coupler `s` metres along the track.
fn haul_cart_centre(sites: &Sites, s: f32) -> Vec3 {
    sites.haul_at((s - HAUL_CART_SIZE.x / 2.0) as f64) + Vec3::Y * HAUL_CART_SIZE.y / 2.0
}

/// Centre of the crawler with its rear coupler `s` metres along the track.
fn crawler_centre(sites: &Sites, s: f32) -> Vec3 {
    sites.haul_at((s + CRAWLER_SIZE.x / 2.0) as f64) + Vec3::Y * CRAWLER_SIZE.y / 2.0
}

fn prop_size(prop: Prop) -> Vec3 {
    match prop {
        Prop::TowerMass | Prop::MastMass | Prop::Ghost => MASS_SIZE,
        Prop::Crate => CRATE_SIZE,
        Prop::Beam => BEAM_SIZE,
        Prop::Cart => CART_SIZE,
        Prop::Cargo => CARGO_SIZE,
        Prop::HaulCart => HAUL_CART_SIZE,
        Prop::Crawler => CRAWLER_SIZE,
    }
}

pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Sim>()
            .add_systems(Startup, setup)
            .add_systems(Update, (handle_requests, advance, settle, recharge, draw_trail, follow_launcher, ride_the_cart, props::spin_wheels).chain());
    }
}

/// Where a prop rests when nothing is happening: transform of its centre.
fn rest_pose(prop: Prop, sites: &Sites) -> Vec3 {
    let half = Vec3::Y * prop_size(prop).y / 2.0;
    match prop {
        Prop::TowerMass => sites.tower_release + half,
        Prop::MastMass | Prop::Ghost => sites.mast_release + half,
        // On the ledge launcher's cradle, which rides on the crates in the jack frame.
        Prop::Crate => sites.canyon_launch.origin - sites.canyon_launch.direction * 0.5 + half,
        Prop::Beam => sites.ridge_launch.origin - sites.ridge_launch.direction * 1.5 + half,
        Prop::Cart => sites.rail_start + half,
        Prop::Cargo => sites.rail_start + Vec3::Y * CART_SIZE.y + half,
        Prop::HaulCart => haul_cart_centre(sites, 0.0),
        Prop::Crawler => crawler_centre(sites, sites.crawler_start),
    }
}

pub(crate) fn setup(
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
    let timber_cargo = timber.clone();
    let alloy = solid(&mut materials, Material::HullAlloy.properties().colour);
    let crawler = solid(&mut materials, [0.92, 0.78, 0.22]);
    let finishes = Finishes::new(&mut materials);
    // The carts roll on spoked wheels; the crawler on treads, running west along the haul track.
    let rail_cart = props::cart(&mut meshes, CART_SIZE, false);
    let haul_cart = props::cart(&mut meshes, HAUL_CART_SIZE, true);
    let crawler_shape = props::crawler(&mut meshes, CRAWLER_SIZE, sites.haul_direction.x.signum());
    let crate_mesh = meshes.add(Cuboid::from_size(CRATE_SIZE));
    let beam = meshes.add(props::i_beam(BEAM_SIZE));
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
        (Prop::Cargo, timber_cargo, false),
        (Prop::HaulCart, steel.clone(), true),
        (Prop::Crawler, crawler, true),
    ] {
        let vehicle = match prop {
            Prop::Cart => Some(&rail_cart),
            Prop::HaulCart => Some(&haul_cart),
            Prop::Crawler => Some(&crawler_shape),
            _ => None,
        };
        let mesh = match prop {
            Prop::Crate => crate_mesh.clone(),
            Prop::Beam => beam.clone(),
            _ => vehicle.map_or_else(|| meshes.add(Cuboid::from_size(prop_size(prop))), |v| v.body.clone()),
        };
        let mut body = commands.spawn((
            prop,
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(rest_pose(prop, &sites)),
            if visible { Visibility::Visible } else { Visibility::Hidden },
        ));
        if let Some(vehicle) = vehicle {
            body.with_children(|parts| vehicle.spawn_parts(parts, |paint| finishes.paint(&material, paint), true));
        }
        if prop == Prop::Cargo {
            // The orange handle of salvage that can be carried: it comes off the cart again with F.
            let glow = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.56, 0.10), emissive: LinearRgba::rgb(2.0, 0.9, 0.1), ..default() });
            let handle = meshes.add(Cuboid::new(0.5, 0.06, 0.12));
            body.with_child((Mesh3d(handle), MeshMaterial3d(glow), Transform::from_xyz(0.0, CARGO_SIZE.y / 2.0 + 0.03, 0.0)));
        }
        if prop == Prop::Crawler {
            // The launchers' guidance unit rides on top until the crawler is caught.
            let [unit, lens] = crate::shipyard::guidance_parts(&mut meshes, &mut materials);
            let on_top = Transform::from_xyz(0.0, (CRAWLER_SIZE.y + crate::shipyard::GUIDANCE_SIZE.y) / 2.0, 0.0);
            body.with_children(|crawler| {
                crawler.spawn((crate::shipyard::GuidanceUnit { riding: true }, unit.0, unit.1, on_top, Visibility::Inherited)).with_child(lens);
            });
        }
    }

    // The physics engine's view of the island: every block anything could touch.
    commands.insert_resource(Gravity(Vec3::NEG_Y * game.layout.g as f32));
    commands.spawn((RigidBody::Static, Collider::voxels(Vec3::ONE, &world.exposed_blocks()), Transform::default()));

    // Loose wreckage, left to the engine: it falls and settles where it will. Bent hull plating,
    // conduit, girder, a thruster bell and a console, thrown clear in the crash.
    for i in 0..10 {
        let offset = Vec3::new((i as f32 * 1.7) % 6.0 - 3.0, 3.0 + i as f32 * 0.9, (i as f32 * 2.3) % 5.0 - 6.5);
        let piece = props::debris(&mut meshes, &finishes, i);
        let turned = Quat::from_rotation_y(i as f32 * 1.3) * Quat::from_rotation_z(i as f32 * 0.7);
        commands
            .spawn((
                RigidBody::Dynamic,
                piece.collider,
                Mesh3d(piece.mesh),
                MeshMaterial3d(piece.material),
                Transform::from_translation(sites.wreck + offset).with_rotation(turned),
            ))
            .with_children(|trim| {
                for (mesh, material, transform) in piece.trim {
                    trim.spawn((Mesh3d(mesh), MeshMaterial3d(material), transform));
                }
            });
    }
    // More of it lies where it was thrown clear, or where it was dragged to a camp: still, resting on
    // the ground, a little larger than the loose pieces. The loose pieces can land on it.
    const SCRAP_SCALE: f32 = 1.35;
    for (j, ground) in sites.scrap.iter().enumerate() {
        let piece = props::debris(&mut meshes, &finishes, j);
        let lift = Vec3::Y * piece.depth(&meshes) * SCRAP_SCALE;
        commands
            .spawn((
                RigidBody::Static,
                piece.collider,
                Mesh3d(piece.mesh),
                MeshMaterial3d(piece.material),
                Transform::from_translation(*ground + lift).with_rotation(Quat::from_rotation_y(j as f32 * 2.1)).with_scale(Vec3::splat(SCRAP_SCALE)),
            ))
            .with_children(|trim| {
                for (mesh, material, transform) in piece.trim {
                    trim.spawn((Mesh3d(mesh), MeshMaterial3d(material), transform));
                }
            });
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
        crate_mesh,
        beam,
        haul: [haul_cart, crawler_shape],
    });
}

/// Read the blueprint: evaluate each typed expression against the notebook, and read what is built
/// in the world.
pub fn read_blueprint(game: &Game, ui: &Ui, puzzle: usize) -> Result<(Inputs, Vec<CommittedValue>), String> {
    let mut inputs = Inputs::new();
    let mut values = Vec::new();
    for (i, spec) in game.instance(puzzle).inputs().iter().enumerate() {
        if spec.source == InputSource::Placed {
            let quantity = Quantity::metres(game.placed(puzzle) as f64);
            inputs.insert(spec.key.to_string(), quantity);
            values.push(CommittedValue { label: spec.label.to_string(), quantity, unit: spec.unit.to_string() });
            continue;
        }
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
    mut sites: ResMut<Sites>,
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
        if !matches!(*prop, Prop::Ghost | Prop::Cargo) {
            *transform = Transform::from_translation(rest_pose(*prop, &sites));
            *visibility = Visibility::Visible;
        }
    }
    if sites.cart != sites.rail_start {
        sites.cart = sites.rail_start;
    }
    match request {
        Request::Demo(puzzle) => {
            if !game.unlocked(puzzle) {
                return;
            }
            let instance = game.instance(puzzle);
            let Some(playback) = instance.demo_raised(game.placed(puzzle)) else { return };
            // The guess counts only before the first run.
            let guess = if game.save.demo_watched(&game.def(puzzle).id) { None } else { ui.guess[puzzle] };
            let ground = GROUND as f32;
            sim.running = Some(match playback {
                DemoPlayback::Strobe { .. } => {
                    for mark in &marks {
                        commands.entity(mark).despawn();
                    }
                    sim.strobe.clear();
                    sim.gate_tripped = false;
                    orbit.frame_side_view(sites.tower_plate, sites.tower_release, Vec3::X);
                    Running::Strobe { puzzle, t: 0.0, flashes: playback.flash_times(), shown: 0, guess, gate: game.placed(puzzle) }
                }
                DemoPlayback::Pace { cart_speed, crawler_speed, head_start, duration, .. } => {
                    for mark in &marks {
                        commands.entity(mark).despawn();
                    }
                    sim.strobe.clear();
                    let run = Run { cart_speed: cart_speed as f32, crawler_speed: crawler_speed as f32, head_start: head_start as f32, end: duration as f32 };
                    let (_, crawler_end) = run.at(run.end);
                    frame_track(&mut orbit, &sites, crawler_end + CRAWLER_SIZE.x);
                    sim.trails = PAIR_COLOURS.iter().map(|c| (Vec::new(), *c)).collect();
                    Running::Pace { puzzle, t: 0.0, flashes: playback.flash_times(), shown: 0, guess, run }
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
            let Some((outcome, values)) = judge_commit(&mut game, &mut ui, puzzle) else { return };
            let instance = game.instance(puzzle);
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
                Playback::Chase { cart_speed, crawler_speed, head_start, t } => {
                    // The experiment's afterimages would stand on the track the test runs along.
                    for mark in &marks {
                        commands.entity(mark).despawn();
                    }
                    let run = Run { cart_speed: cart_speed as f32, crawler_speed: crawler_speed as f32, head_start: head_start as f32, end: t as f32 };
                    let (_, crawler_end) = run.at(run.end);
                    frame_track(&mut orbit, &sites, crawler_end.max(game.layout.transfer_at as f32) + CRAWLER_SIZE.x);
                    Running::Chase { puzzle, t: 0.0, run, verdict: Verdict { values, outcome } }
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

/// Look at the haul track from the side, from just behind the cart's start to `reach` metres along it.
fn frame_track(orbit: &mut OrbitCamera, sites: &Sites, reach: f32) {
    let (a, b) = (sites.haul_at(-1.5), sites.haul_at(reach as f64 + 0.5));
    orbit.frame_side_view(a.min(b), a.max(b) + Vec3::Y * 2.0, sites.haul_direction);
}

/// Put the haul cart and the crawler where a run has them at `t`. Returns where the action is: between
/// the two, which is where the eye should be.
fn place_haul(sites: &Sites, run: &Run, t: f32, props: &mut Query<(&Prop, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>)>) -> Vec3 {
    let (cart, crawler) = run.at(t);
    let (cart, crawler) = (haul_cart_centre(sites, cart), crawler_centre(sites, crawler));
    for (prop, mut transform, _, _) in props.iter_mut() {
        match prop {
            Prop::HaulCart => transform.translation = cart,
            Prop::Crawler => transform.translation = crawler,
            _ => {}
        }
    }
    (cart + crawler) / 2.0
}

/// A commit at a station, up to the verdict: refused for free (the reason goes to `ui.refusal`), or
/// judged, with the values committed. Nothing is recorded yet: that waits until the test has played
/// out in the world.
pub fn judge_commit(game: &mut Game, ui: &mut Ui, puzzle: usize) -> Option<(Outcome, Vec<CommittedValue>)> {
    ui.refusal = None;
    if !game.unlocked(puzzle) {
        return None;
    }
    let instance = game.instance(puzzle);
    // What has to stand in the world comes first (the cart's cargo). Refused for it, nothing typed
    // has been committed, so no page is written.
    if let Some(reason) = instance.unready(game.placed(puzzle)) {
        ui.refusal = Some(reason);
        return None;
    }
    // A first value committed here may write a page, whether it is tested or refused.
    if ui.drafts[puzzle].iter().any(|d| !d.trim().is_empty()) && game.find_pages(ui, puzzle, PageEvent::Commit) > 0 {
        game.persist();
    }
    if game.charges_left(puzzle) == 0 {
        ui.refusal = Some("The station is recharging.".into());
        return None;
    }
    let (inputs, values) = match read_blueprint(game, ui, puzzle) {
        Ok(read) => read,
        Err(problem) => {
            ui.refusal = Some(problem);
            return None;
        }
    };
    // Wrong dimensions and impossible settings are refused here, before a charge is used.
    match instance.evaluate(&inputs) {
        Ok(outcome) => Some((outcome, values)),
        Err(problem) => {
            ui.refusal = Some(problem.to_string());
            None
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
/// Hand a body over to the physics engine: `mesh` inside a box of `size`, moving at `velocity`.
fn hand_over(commands: &mut Commands, mesh: Handle<Mesh>, size: Vec3, centre: Vec3, velocity: Vec3, material: Handle<StandardMaterial>, after: AfterSettling) {
    commands.spawn((
        RigidBody::Dynamic,
        Collider::cuboid(size.x, size.y, size.z),
        LinearVelocity(velocity),
        AngularVelocity(Vec3::new(0.3, 0.2, -1.2)),
        Restitution::new(0.25),
        Friction::new(0.8),
        Mesh3d(mesh),
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
    assets: Res<PropAssets>,
    mut sim: ResMut<Sim>,
    mut game: ResMut<Game>,
    mut ui: ResMut<Ui>,
    mut orbit: ResMut<OrbitCamera>,
    mut sites: ResMut<Sites>,
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
    sim.action = None;
    let Some(mut running) = sim.running.take() else { return };

    let mut finished: Option<(usize, Verdict)> = None;

    match &mut running {
        Running::Strobe { puzzle, t, flashes, shown, guess, gate } => {
            *t += dt;
            let height = game.layout.tower_h as f32;
            let fall_time = (2.0 * height / g).sqrt();
            if *gate > 0 && fallen(g, *t, height) >= *gate as f32 {
                sim.gate_tripped = true;
            }
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
                sim.action = Some(transform.translation);
                if *t >= fall_time {
                    *visibility = Visibility::Hidden;
                    hand_over(&mut commands, assets.mass.clone(), MASS_SIZE, transform.translation, Vec3::NEG_Y * g * fall_time, material.0.clone(), AfterSettling::Restore(Prop::TowerMass));
                }
            }
            if *t >= fall_time {
                note_reading(&mut game, *gate);
                let (puzzle, guess) = (*puzzle, *guess);
                finish_demo(&mut game, &mut ui, puzzle, guess);
                sim.running = None;
                sim.watch_left = 2.5;
                return;
            }
        }
        Running::Pace { puzzle, t, flashes, shown, guess, run } => {
            *t += dt;
            // Each flash leaves an afterimage of both, side by side in time along the track.
            while *shown < flashes.len() && *t >= flashes[*shown] as f32 {
                let (cart, crawler) = run.at(flashes[*shown] as f32);
                for (mesh, centre) in [(0, haul_cart_centre(&sites, cart)), (1, crawler_centre(&sites, crawler))] {
                    let shape = &assets.haul[mesh];
                    let ghost = assets.afterimage.clone();
                    commands
                        .spawn((StrobeMark, Mesh3d(shape.body.clone()), MeshMaterial3d(ghost.clone()), Transform::from_translation(centre)))
                        .with_children(|parts| shape.spawn_parts(parts, |_| ghost.clone(), false));
                }
                *shown += 1;
            }
            let (cart, crawler) = run.at(*t);
            for (k, s) in [cart, crawler].into_iter().enumerate() {
                if let Some((trail, _)) = sim.trails.get_mut(k) {
                    trail.push(sites.haul_at(s as f64) + Vec3::Y * 0.05);
                }
            }
            sim.action = Some(place_haul(&sites, run, *t, &mut props));
            if *t >= run.end {
                note_crawler(&mut game);
                let (puzzle, guess) = (*puzzle, *guess);
                finish_demo(&mut game, &mut ui, puzzle, guess);
                sim.running = None;
                sim.watch_left = 2.5;
                return;
            }
        }
        Running::Chase { puzzle, t, run, verdict } => {
            // Both run at their steady speeds from the moment they start: no easing. They stay where
            // the run ended, so a miss can be looked at, until the next run.
            *t += dt;
            let (cart, _) = run.at(*t);
            if let Some((trail, _)) = sim.trails.first_mut() {
                trail.push(sites.haul_at(cart as f64) + Vec3::Y * 0.05);
            }
            sim.action = Some(place_haul(&sites, run, *t, &mut props));
            if *t >= run.end {
                finished = Some((*puzzle, verdict.clone()));
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
                // Both bodies fly side by side: look between them.
                sim.action = Some(sim.action.map_or(centre, |other| (other + centre) / 2.0));
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
                        sim.action = Some(transform.translation);
                        if *t >= *actual && *visibility != Visibility::Hidden {
                            *visibility = Visibility::Hidden;
                            hand_over(&mut commands, assets.mass.clone(), MASS_SIZE, transform.translation, Vec3::NEG_Y * g * *actual, material.0.clone(), AfterSettling::Restore(Prop::MastMass));
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
            sim.action = Some(centre);
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
                    let mesh = if prop == Prop::Beam { assets.beam.clone() } else { assets.crate_mesh.clone() };
                    hand_over(&mut commands, mesh, prop_size(prop), centre, velocity, material, after);
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
            sim.action = Some(centre);
            if let Some((trail, _)) = sim.trails.first_mut() {
                trail.push(centre - Vec3::Y * (CART_SIZE.y / 2.0 - 0.05));
            }
            for (prop, mut transform, _, _) in &mut props {
                if *prop == Prop::Cart {
                    transform.translation = centre;
                }
            }
            if progress >= 1.0 {
                // It stays where it stopped, and that is where its cargo can be reached.
                sites.cart = start - Vec3::Y * CART_SIZE.y / 2.0 + Vec3::X * *distance;
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

/// What the stopwatch's readings are called in the notebook, at the haul track and at the drop tower.
pub const CRAWLER_TIME: &str = "crawler's time between the timing posts";
pub const TOWER_TIME: &str = "fall time from the drop tower";

/// Write the stopwatch's reading of the crawler, from the first timing post to the second, into the
/// notebook.
pub fn note_crawler(game: &mut Game) {
    let reading = Quantity::seconds(game.layout.crawler_time());
    game.save.notebook.add_numbered("t", CRAWLER_TIME, reading, "stopwatch");
}

/// Write the stopwatch's reading of a release from the drop tower into the notebook: with the light
/// gate clamped `gate` metres below the release, the drop and the time to the gate as a pair (`h2`,
/// `t2`); without, the time of the whole drop.
pub fn note_reading(game: &mut Game, gate: u32) {
    if gate == 0 {
        let reading = Quantity::seconds(game.layout.tower_fall_time());
        game.save.notebook.add_numbered("t", TOWER_TIME, reading, "stopwatch");
    } else {
        let (drop, time) = (Quantity::metres(gate as f64), Quantity::seconds(game.layout.gate_time(gate)));
        game.save.notebook.add_pair(("h", "drop from the release point to the light gate", drop), ("t", "fall time to the light gate", time), "light gate");
    }
}

/// The cargo crate rides on the cart while it is loaded.
fn ride_the_cart(carry: Res<Carry>, mut props: Query<(&Prop, &mut Transform, &mut Visibility)>) {
    let Some(cart) = props.iter().find(|(p, _, _)| **p == Prop::Cart).map(|(_, t, _)| t.translation) else { return };
    let loaded = carry.holds(Slot::Cart) > 0;
    for (prop, mut transform, mut visibility) in &mut props {
        if *prop == Prop::Cargo {
            transform.translation = cart + Vec3::Y * (CART_SIZE.y + CARGO_SIZE.y) / 2.0;
            let want = if loaded { Visibility::Visible } else { Visibility::Hidden };
            if *visibility != want {
                *visibility = want;
            }
        }
    }
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

/// Keep the ledge launcher's load on its cradle when crates go into or out of the jack frame.
fn follow_launcher(sim: Res<Sim>, sites: Res<Sites>, mut props: Query<(&Prop, &mut Transform, &Visibility)>) {
    if !sites.is_changed() || sim.busy() {
        return;
    }
    for (prop, mut transform, visibility) in &mut props {
        if *prop == Prop::Crate && *visibility != Visibility::Hidden {
            transform.translation = rest_pose(Prop::Crate, &sites);
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

fn draw_trail(sim: Res<Sim>, sites: Res<Sites>, mut gizmos: Gizmos) {
    // The light gate's beam across the fall line, bright once the mass has passed it.
    let gate = &sites.pins[sites.gate_pin];
    if gate.present() {
        let colour = if sim.gate_tripped { Color::srgb(1.0, 0.95, 0.5) } else { Color::srgb(1.0, 0.2, 0.15) };
        gizmos.line(gate.position - Vec3::Z * 0.56, gate.position + Vec3::Z * 0.56, colour);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn game(seed: u64) -> Game {
        Game::for_test(seed)
    }

    #[test]
    fn an_empty_cart_is_refused_for_free_and_writes_no_page() {
        let mut game = game(11);
        let mut ui = Ui::default();
        ui.drafts[0] = vec![game.layout.rail_target.to_string()];
        assert!(judge_commit(&mut game, &mut ui, 0).is_none());
        assert!(ui.refusal.as_deref().is_some_and(|r| r.starts_with("Load the cart first")), "{:?}", ui.refusal);
        assert!(!game.save.found.contains("units"), "refused for the empty cart, nothing was committed");
        assert_eq!(game.charges_left(0), 3);

        // Loaded, the bare number is refused for its missing unit, and that first commit writes the page.
        game.save.placed.insert("1.1.0".into(), 1);
        assert!(judge_commit(&mut game, &mut ui, 0).is_none());
        assert!(ui.refusal.as_deref().is_some_and(|r| r.contains("pure number")), "{:?}", ui.refusal);
        assert!(game.save.found.contains("units"));
        assert_eq!(ui.unread_pages, ["units"]);
        assert_eq!(game.charges_left(0), 3, "a refusal costs no charge");

        ui.drafts[0] = vec![format!("{} m", game.layout.rail_target)];
        let (outcome, values) = judge_commit(&mut game, &mut ui, 0).expect("a loaded cart runs");
        assert!(outcome.pass && ui.refusal.is_none());
        assert_eq!(values.len(), 1);
        let _ = std::fs::remove_file(&game.save_path);
    }

    #[test]
    fn the_light_gate_notes_a_pair_and_the_bare_tower_a_time() {
        let mut game = game(12);
        game.save.notebook.add_numbered("h", "tower height", Quantity::metres(game.layout.tower_h as f64), "ruler");
        note_reading(&mut game, 0);
        let nb = &game.save.notebook;
        assert_eq!(nb.get("t1").unwrap().quantity, Quantity::seconds(game.layout.tower_fall_time()));
        note_reading(&mut game, 6);
        note_reading(&mut game, 9);
        let nb = &game.save.notebook;
        assert_eq!(nb.get("h2").unwrap().quantity, Quantity::metres(6.0));
        assert_eq!(nb.get("t2").unwrap().quantity, Quantity::seconds(game.layout.gate_time(6)));
        assert_eq!(nb.get("h3").unwrap().quantity, Quantity::metres(9.0));
        assert_eq!(nb.get("t3").unwrap().source, "light gate");
    }
}
