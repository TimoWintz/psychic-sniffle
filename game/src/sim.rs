//! Demonstrations and tests played out in the world.
//!
//! The outcome of a test is decided by `physics_core` at the moment of commit, and the path of every
//! demonstration body by `physics_core::demo`. This module only shows them: an object follows the
//! exact path until the judged event (the landing or the strike), and is then handed to the physics
//! engine, which bounces and settles it. Nothing the engine does can change the result.

use crate::carry::{Carry, DropLedge, QUARRY_STONE_SIZE, Slot};
use crate::island::{GROUND, LaunchSite, SEA_LEVEL, Sites};
use crate::player::OrbitCamera;
use crate::props::{self, Finishes, Vehicle};
use crate::ruler::Ruler;
use crate::state::{BuildWorld, Game, Mode, PageEvent, RECHARGE_SECONDS, Ui, WorldEntity, not_paused};
use crate::voxel::VoxelWorld;
use avian3d::prelude::*;
use bevy::prelude::*;
use physics_core::ballistics::{Flight, Projectile};
use physics_core::demo::{DemoPlayback, Shot};
use physics_core::expr::evaluate;
use physics_core::notebook::source;
use physics_core::progress::CommittedValue;
use physics_core::puzzle::{Ch1Layout, InputSource, Inputs, Outcome, Playback, PuzzleKind};
use physics_core::units::Quantity;

/// Something the interface asks the simulation to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Request {
    /// Run a station's conceptual "Try it first" demonstration (free, repeatable, writes handbook
    /// pages; does not record instrument readings).
    Demo(usize),
    /// Run a station's built-in measuring instrument (free, repeatable): timing the crawler past
    /// the posts at the haul track.
    Measure(usize),
    /// Release a loose quarry drop-stone straight down from rest off a sea-cliff ledge (triggered
    /// directly in the world with F; free, repeatable, logs fall time only).
    DropStone(DropLedge),
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
    /// The conceptual strobe demonstration at the sea-cliff quarry: a flash marks a stone falling
    /// down the cliff face at each of `flashes`.
    Strobe {
        puzzle: usize,
        t: f32,
        flashes: Vec<f64>,
        shown: usize,
        guess: Option<usize>,
    },
    /// Dropping a loose quarry stone off either ledge with F: falls from rest to the bench or
    /// tidal slab and notes `t` in the notebook via Lumen's stopwatch.
    QuarryDrop {
        ledge: DropLedge,
        t: f32,
    },
    /// Two demonstration bodies fly their exact paths from a launch site.
    Pair {
        puzzle: usize,
        t: f32,
        shots: [Shot; 2],
        site: LaunchSite,
        offsets: [Vec3; 2],
        size: f32,
        landed: [bool; 2],
        guess: Option<usize>,
    },
    /// Verification drop from the outcrop overhang: the stone shutter leaves over the tidal slab's
    /// quartz socket slide open at `predicted` and either catch the counterweight or let it strike
    /// the closed stone.
    OverhangDrop {
        puzzle: usize,
        t: f32,
        predicted: f32,
        actual: f32,
        landed: bool,
        verdict: Verdict,
    },
    /// A launcher's load in flight; `landed` once it has been handed to the physics engine.
    Flight {
        puzzle: usize,
        t: f32,
        projectile: Projectile,
        flight: Flight,
        site: LaunchSite,
        landed: bool,
        verdict: Verdict,
    },
    Slide {
        puzzle: usize,
        t: f32,
        distance: f32,
        duration: f32,
        verdict: Verdict,
    },
    /// The conceptual demonstration at the haul track: cart and crawler set off at steady speeds
    /// with strobe afterimages.
    Pace {
        puzzle: usize,
        t: f32,
        flashes: Vec<f64>,
        shown: usize,
        guess: Option<usize>,
        run: Run,
    },
    /// Timing the crawler solo past both timing posts with the station's stopwatch.
    TimeCrawler {
        t: f32,
        run: Run,
    },
    /// The haul cart sent after the crawler, until it reaches it or the crawler reaches the buffer.
    Chase {
        puzzle: usize,
        t: f32,
        run: Run,
        verdict: Verdict,
    },
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
    outcome: Outcome,
}

/// How many previous test trails at the same station stay visible as faint ghost arcs.
const MAX_PAST_TRAILS: usize = 3;

#[derive(Resource, Default)]
pub struct Sim {
    pub request: Option<Request>,
    running: Option<Running>,
    /// Paths of the last test or demonstration with their colours, drawn until the next one.
    pub trails: Vec<(Vec<Vec3>, [f32; 3])>,
    /// Whether `trails` currently holds a committed test trail (eligible to be archived as a ghost arc).
    trails_from_test: bool,
    /// Which puzzle `past_trails` (and `trails` when `trails_from_test`) belong to.
    trail_puzzle: Option<usize>,
    /// Faint ghost trails from earlier tests at the same station, oldest first.
    past_trails: Vec<Vec<Vec3>>,
    /// Where the strobe caught the test mass (its centre), drawn as ticks until the next release.
    strobe: Vec<Vec3>,
    /// Seconds until the camera stops watching the test.
    watch_left: f32,
    /// Where the moving object of the running test or experiment is.
    action: Option<Vec3>,
    /// Counts of completed quarry stone drops per ledge (read by Lumen to comment on each drop).
    pub low_drops: u32,
    pub cliff_drops: u32,
    /// Tracks whether the rail cart and tiller have already transitioned to their post-solve places.
    relayed: Relayed,
}

impl Sim {
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    /// Whether the camera is still on a test that has ended.
    pub fn watching(&self) -> bool {
        self.watch_left > 0.0
    }

    /// The puzzle whose test is running, if one is (a demonstration or measurement is not a test).
    pub fn testing(&self) -> Option<usize> {
        match &self.running {
            Some(Running::OverhangDrop { puzzle, .. } | Running::Flight { puzzle, .. } | Running::Slide { puzzle, .. } | Running::Chase { puzzle, .. }) => {
                Some(*puzzle)
            }
            Some(Running::Strobe { .. } | Running::QuarryDrop { .. } | Running::Pair { .. } | Running::Pace { .. } | Running::TimeCrawler { .. }) | None => {
                None
            }
        }
    }

    /// Where the object a test or an experiment moves is now, while one runs.
    pub fn action_point(&self) -> Option<Vec3> {
        self.running.as_ref().and(self.action)
    }

    /// The puzzle whose conceptual demonstration is running, if one is.
    pub fn demo(&self) -> Option<usize> {
        match &self.running {
            Some(Running::Strobe { puzzle, .. } | Running::Pair { puzzle, .. } | Running::Pace { puzzle, .. }) => Some(*puzzle),
            _ => None,
        }
    }

    /// Whether a built-in measuring run (crawler stopwatch or quarry stone drop) is under way.
    pub fn measuring(&self) -> bool {
        matches!(&self.running, Some(Running::QuarryDrop { .. } | Running::TimeCrawler { .. }))
    }

    fn archive_current_test_trails(&mut self) {
        if !self.trails_from_test {
            return;
        }
        for (trail, _) in self.trails.drain(..) {
            if trail.len() >= 2 {
                self.past_trails.push(trail);
            }
        }
        if self.past_trails.len() > MAX_PAST_TRAILS {
            let drop = self.past_trails.len() - MAX_PAST_TRAILS;
            self.past_trails.drain(..drop);
        }
        self.trails_from_test = false;
    }

    /// Begin a new test trail at `puzzle`, archiving any previous test trail at the same station as
    /// a faint ghost arc so successive attempts can be compared in 3D.
    fn start_test_trail(&mut self, puzzle: usize, colour: [f32; 3]) {
        if self.trail_puzzle == Some(puzzle) {
            self.archive_current_test_trails();
        } else {
            self.past_trails.clear();
            self.trail_puzzle = Some(puzzle);
        }
        self.trails = vec![(Vec::new(), colour)];
        self.trails_from_test = true;
    }

    /// Begin measuring-instrument trails at `puzzle`, preserving any archived test ghost arcs from
    /// the same station without archiving the measurement trail itself.
    fn start_measure_trails(&mut self, puzzle: usize, trails: Vec<(Vec<Vec3>, [f32; 3])>) {
        if self.trail_puzzle == Some(puzzle) {
            self.archive_current_test_trails();
        } else {
            self.past_trails.clear();
            self.trail_puzzle = Some(puzzle);
        }
        self.trails = trails;
        self.trails_from_test = false;
    }

    /// Begin demonstration trails, clearing any archived test ghost arcs.
    fn start_demo_trails(&mut self, trails: Vec<(Vec<Vec3>, [f32; 3])>) {
        self.past_trails.clear();
        self.trail_puzzle = None;
        self.trails_from_test = false;
        self.trails = trails;
    }
}

/// The objects that tests move.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum Prop {
    /// Smaller quarry drop-stone shown during the strobe demo or an F drop off either ledge.
    CliffMass,
    /// Heavy builders' counterweight hanging under the outcrop overhang.
    Counterweight,
    Casket,
    Tiller,
    /// The rail cart, on the tutorial's stretch of the line.
    Cart,
    /// The block the cart carries, while it is loaded: it rides on the cart.
    Cargo,
    /// The same cart once it has carried its block to the marker and gone on through the stone
    /// gate: the haul cart, behind its front coupler.
    HaulCart,
    /// The survey crawler, ahead of its rear coupler.
    Crawler,
}

/// One of the two sliding pale-stone shutter leaves covering the quartz socket on the tidal slab:
/// `-1.0` for the west leaf, `+1.0` for the east leaf.
#[derive(Component, Clone, Copy)]
struct ShutterLeaf(f32);

/// A body handed to the physics engine after a test; removed when its time is up.
#[derive(Component)]
struct Settling {
    left: f32,
    after: AfterSettling,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum AfterSettling {
    /// Removed, and the prop put back where it rests (on its launcher, or wherever a pass has left it).
    Restore(Prop),
    /// Removed, and the prop shown where the body came to rest (the casket beside the tilting
    /// launcher).
    Rest(Prop),
    /// Left where it came to rest (a load on its target).
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
    stone: Handle<StandardMaterial>,
    pair: [Handle<StandardMaterial>; 2],
    afterimage: Handle<StandardMaterial>,
    /// Smaller quarry drop-stone mesh (`QUARRY_STONE_SIZE`), used for strobe & ledge drops.
    quarry_stone: Handle<Mesh>,
    /// Counterweight mesh (`MASS_SIZE`), used for the outcrop overhang verification drop.
    mass: Handle<Mesh>,
    /// What the ledge launcher and the tilting launcher throw: the helm's casket and the tiller,
    /// each a stone body and its quartz, with the quartz's material.
    casket: (Handle<Mesh>, Handle<Mesh>, Handle<StandardMaterial>),
    tiller: (Handle<Mesh>, Handle<Mesh>, Handle<StandardMaterial>),
    /// The haul cart's and the crawler's shapes, for their afterimages.
    haul: [Vehicle; 2],
    /// A ring of foam, for where something falls into the water.
    ring: (Handle<Mesh>, Handle<StandardMaterial>),
}

/// A ring of foam spreading on the water where something fell in. It sinks out of sight as it
/// spreads.
#[derive(Component)]
struct Splash {
    age: f32,
    delay: f32,
}

/// How long a ring of foam spreads, and how wide it gets.
const SPLASH_TIME: f32 = 1.3;
const SPLASH_REACH: f32 = 3.2;

/// Whether a flight came down on the water rather than the ground.
fn on_water(flight: &Flight) -> bool {
    (flight.surface + Ch1Layout::WATER_DROP).abs() < 1e-9
}

/// Rings of foam where something falls into the water at `at`, and the water it throws up.
fn splash(commands: &mut Commands, assets: &PropAssets, at: Vec3) {
    crate::particles::burst(commands, crate::particles::Kind::Splash, at.with_y(SEA_LEVEL), 1.0);
    for delay in [0.0, 0.25] {
        commands.spawn((
            WorldEntity,
            Splash { age: 0.0, delay },
            Mesh3d(assets.ring.0.clone()),
            MeshMaterial3d(assets.ring.1.clone()),
            Transform::from_translation(at.with_y(SEA_LEVEL + 0.03)).with_scale(Vec3::splat(0.01)),
            bevy::light::NotShadowCaster,
        ));
    }
}

/// Rings of foam where the avatar fell into deep water.
fn splash_the_avatar(mut commands: Commands, assets: Res<PropAssets>, mut shore: ResMut<crate::player::Shore>) {
    if let Some(at) = shore.splash.take() {
        splash(&mut commands, &assets, at);
    }
}

/// Spread each ring of foam, sink it under the water as it fades, and clear it away.
fn spread_splashes(time: Res<Time>, mut commands: Commands, mut splashes: Query<(Entity, &mut Splash, &mut Transform)>) {
    for (entity, mut splash, mut transform) in &mut splashes {
        splash.age += time.delta_secs();
        let u = ((splash.age - splash.delay) / SPLASH_TIME).clamp(0.0, 1.0);
        if u >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let reach = 0.3 + (SPLASH_REACH - 0.3) * (1.0 - (1.0 - u) * (1.0 - u));
        transform.scale = Vec3::splat(if splash.age < splash.delay { 0.01 } else { reach });
        transform.translation.y = SEA_LEVEL + 0.03 - 0.3 * ((u - 0.6) / 0.4).max(0.0);
    }
}

const MASS_SIZE: Vec3 = Vec3::new(0.8, 0.8, 0.8);
const STONE_SIZE: Vec3 = Vec3::splat(QUARRY_STONE_SIZE);
const CASKET_SIZE: Vec3 = Vec3::new(0.9, 0.9, 0.9);
pub const TILLER_SIZE: Vec3 = Vec3::new(2.4, 0.5, 0.5);
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
        Prop::CliffMass => STONE_SIZE,
        Prop::Counterweight => MASS_SIZE,
        Prop::Casket => CASKET_SIZE,
        Prop::Tiller => TILLER_SIZE,
        Prop::Cart => CART_SIZE,
        Prop::Cargo => CARGO_SIZE,
        Prop::HaulCart => HAUL_CART_SIZE,
        Prop::Crawler => CRAWLER_SIZE,
    }
}

pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Sim>().init_resource::<TerrainReshaped>().add_systems(BuildWorld, setup).add_systems(
            Update,
            (
                (handle_requests, advance, pose_shutters, settle, recharge, props::spin_wheels, props::step_legs, splash_the_avatar, spread_splashes)
                    .chain()
                    .run_if(in_state(Mode::Playing))
                    .run_if(not_paused),
                relay.run_if(not_paused),
                (draw_trail.run_if(in_state(Mode::Playing)), follow_launcher, ride_the_cart, reshape_terrain).chain(),
            )
                .chain(),
        );
    }
}

/// The physics engine's view of the island's blocks.
#[derive(Component)]
struct TerrainCollider;

/// Set when blocks of the island have come or gone for good (the way's stonework rising or sinking,
/// a new game putting it back to sleep), so the physics engine's view of the island is built again.
#[derive(Resource, Default)]
pub struct TerrainReshaped(pub bool);

fn reshape_terrain(mut commands: Commands, mut reshaped: ResMut<TerrainReshaped>, world: Res<VoxelWorld>, terrain: Single<Entity, With<TerrainCollider>>) {
    if !reshaped.0 {
        return;
    }
    reshaped.0 = false;
    commands.entity(*terrain).insert(Collider::voxels(Vec3::ONE, &world.exposed_blocks()));
}

/// How a prop rests when nothing is happening: where its centre is, and the tiller lying along its
/// launcher's line.
fn rest_transform(prop: Prop, sites: &Sites, game: &Game) -> Transform {
    let transform = Transform::from_translation(rest_pose(prop, sites, game));
    if prop == Prop::Tiller { transform.with_rotation(Quat::from_rotation_arc(Vec3::X, sites.ridge_launch.direction)) } else { transform }
}

/// Whether a prop is in the world when nothing is happening. The cart runs the tutorial's stretch
/// until it has carried its block to the marker, then waits past the stone gate as the haul cart;
/// the casket rides the ledge launcher, then rests on the platform it was thrown to; the tiller is on
/// the tilting launcher from the casket's landing until it is thrown into the circle (and from there
/// the skiff takes it).
fn shown(prop: Prop, game: &Game) -> bool {
    match prop {
        Prop::CliffMass | Prop::Cargo => false,
        Prop::Cart => !game.kind_solved(PuzzleKind::Rail),
        Prop::HaulCart => game.kind_solved(PuzzleKind::Rail),
        Prop::Tiller => game.kind_solved(PuzzleKind::Channel) && !game.kind_solved(PuzzleKind::Ridge),
        _ => true,
    }
}

/// Where a prop rests when nothing is happening: transform of its centre.
fn rest_pose(prop: Prop, sites: &Sites, game: &Game) -> Vec3 {
    let half = Vec3::Y * prop_size(prop).y / 2.0;
    match prop {
        Prop::CliffMass => sites.cliff_drop + half,
        Prop::Counterweight => sites.overhang_release + half,
        // Once it has crossed, on the platform beside the tilting launcher.
        Prop::Casket if game.kind_solved(PuzzleKind::Channel) => sites.channel_launch.at(game.layout.platform_centre, 0.0, GROUND as f32) + half,
        Prop::Casket | Prop::Tiller => launch_pose(prop, sites),
        Prop::Cart => sites.rail_start + half,
        Prop::Cargo => sites.rail_start + Vec3::Y * CART_SIZE.y + half,
        Prop::HaulCart => haul_cart_centre(sites, 0.0),
        Prop::Crawler => crawler_centre(sites, sites.crawler_start),
    }
}

/// Where a launcher's load sits before it is thrown: the casket on the ledge launcher's cradle,
/// which rides on the blocks in the jack frame, the tiller on the tilting launcher.
fn launch_pose(prop: Prop, sites: &Sites) -> Vec3 {
    let half = Vec3::Y * prop_size(prop).y / 2.0;
    match prop {
        Prop::Casket => sites.channel_launch.origin - sites.channel_launch.direction * 0.5 + half,
        _ => sites.ridge_launch.origin - sites.ridge_launch.direction * 1.5 + half,
    }
}

pub(crate) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    world: Res<VoxelWorld>,
    sites: Res<Sites>,
    game: Res<Game>,
    trim: Res<crate::waking::MachineTrim>,
) {
    let solid = |materials: &mut Assets<StandardMaterial>, c: [f32; 3]| {
        materials.add(StandardMaterial { base_color: Color::srgb(c[0], c[1], c[2]), perceptual_roughness: 0.6, ..default() })
    };
    // The carts, the crawler, the test masses, the cart's block, the helm's casket and the tiller
    // are the builders': their pale stone, the crawler a weathered shade of it.
    let stone = crate::waking::stone(&mut materials);
    let crawler = solid(&mut materials, [0.72, 0.68, 0.58]);
    let finishes = Finishes::new(&mut materials);
    let crawler_quartz = crate::waking::lit_quartz(&mut materials);
    // The carts roll on stone wheels in the groove; the crawler walks, west along the haul track.
    let rail_cart = props::cart(&mut meshes, CART_SIZE, false);
    let haul_cart = props::cart(&mut meshes, HAUL_CART_SIZE, true);
    let crawler_shape = props::crawler(&mut meshes, CRAWLER_SIZE, sites.haul_direction.x.signum());
    // The casket's quartz band and the tiller's core light as their launchers wake.
    let [casket_body, casket_band] = props::casket(CASKET_SIZE).map(|m| meshes.add(m));
    let [tiller_shaft, tiller_core] = props::tiller(TILLER_SIZE).map(|m| meshes.add(m));
    let casket = (casket_body, casket_band, trim.0[3].clone());
    let tiller = (tiller_shaft, tiller_core, trim.0[4].clone());
    let quarry_quartz = trim.0[2].clone();
    for (prop, material) in [
        (Prop::CliffMass, stone.clone()),
        (Prop::Counterweight, stone.clone()),
        (Prop::Casket, stone.clone()),
        (Prop::Tiller, stone.clone()),
        (Prop::Cart, stone.clone()),
        (Prop::Cargo, stone.clone()),
        (Prop::HaulCart, stone.clone()),
        (Prop::Crawler, crawler),
    ] {
        let vehicle = match prop {
            Prop::Cart => Some(&rail_cart),
            Prop::HaulCart => Some(&haul_cart),
            Prop::Crawler => Some(&crawler_shape),
            _ => None,
        };
        let mesh = match prop {
            Prop::Casket => casket.0.clone(),
            Prop::Tiller => tiller.0.clone(),
            _ => vehicle.map_or_else(|| meshes.add(Cuboid::from_size(prop_size(prop))), |v| v.body.clone()),
        };
        let mut body = commands.spawn((
            WorldEntity,
            prop,
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            rest_transform(prop, &sites, &game),
            if shown(prop, &game) { Visibility::Visible } else { Visibility::Hidden },
        ));
        if prop == Prop::Counterweight {
            // A band of builders' quartz around the counterweight, lighting as the quarry wakes.
            let band = meshes.add(Cuboid::new(MASS_SIZE.x + 0.04, 0.14, MASS_SIZE.z + 0.04));
            body.with_child((Mesh3d(band), MeshMaterial3d(quarry_quartz.clone())));
        }
        if let Some(vehicle) = vehicle {
            // The cart's quartz is the supply rail's machine's, and sleeps and wakes with it. The
            // crawler never slept: it still walks its rounds, so its quartz is lit.
            let quartz = if prop == Prop::Crawler { crawler_quartz.clone() } else { trim.0[0].clone() };
            body.with_children(|parts| {
                vehicle.spawn_parts(parts, |paint| if paint == props::Paint::Trim { quartz.clone() } else { finishes.paint(&material, paint) }, true)
            });
        }
        if let Some((_, quartz, lit)) = match prop {
            Prop::Casket => Some(&casket),
            Prop::Tiller => Some(&tiller),
            _ => None,
        } {
            body.with_child((Mesh3d(quartz.clone()), MeshMaterial3d(lit.clone())));
        }
        if prop == Prop::Cargo {
            // The orange handle we painted on what can be carried: it comes off the cart again with F.
            let glow = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.56, 0.10), emissive: LinearRgba::rgb(0.5, 0.2, 0.02), ..default() });
            let handle = meshes.add(Cuboid::new(0.5, 0.06, 0.12));
            body.with_child((Mesh3d(handle), MeshMaterial3d(glow), Transform::from_xyz(0.0, CARGO_SIZE.y / 2.0 + 0.03, 0.0)));
        }
        if prop == Prop::Crawler {
            // The launchers' sight rides on top until the crawler is caught.
            let [unit, lens] = crate::vessel::sight_parts(&mut meshes, &mut materials);
            let on_top = Transform::from_xyz(0.0, (CRAWLER_SIZE.y + crate::vessel::SIGHT_SIZE.y) / 2.0, 0.0);
            body.with_children(|crawler| {
                crawler.spawn((crate::vessel::Sight { riding: true }, unit.0, unit.1, on_top, Visibility::Inherited)).with_child(lens);
            });
        }
    }

    // The two sliding pale-stone shutter leaves over the tidal slab's quartz socket. They slide
    // apart along +/-X at the player's predicted arrival time during the verification drop.
    let leaf_mesh = meshes.add(Cuboid::new(SHUTTER_WIDTH, SHUTTER_HEIGHT, 1.08));
    let leaf_trim = meshes.add(Cuboid::new(0.08, SHUTTER_HEIGHT + 0.02, 1.1));
    let solved_gravity = game.kind_solved(PuzzleKind::Gravity);
    let initial_open = if solved_gravity { 1.0 } else { 0.0 };
    for sign in [-1.0f32, 1.0] {
        commands
            .spawn((
                WorldEntity,
                ShutterLeaf(sign),
                Mesh3d(leaf_mesh.clone()),
                MeshMaterial3d(stone.clone()),
                Transform::from_translation(shutter_pose(sites.overhang_socket, sign, initial_open)),
            ))
            .with_child((
                Mesh3d(leaf_trim.clone()),
                MeshMaterial3d(quarry_quartz.clone()),
                Transform::from_xyz(-sign * (SHUTTER_WIDTH / 2.0 - 0.05), 0.0, 0.0),
            ));
    }

    // The physics engine's view of the island: every block anything could touch.
    commands.insert_resource(Gravity(Vec3::NEG_Y * game.layout.g as f32));
    commands.spawn((WorldEntity, TerrainCollider, RigidBody::Static, Collider::voxels(Vec3::ONE, &world.exposed_blocks()), Transform::default()));

    // Loose wreckage, left to the engine: it falls and settles where it will. Bent hull plating,
    // conduit, girder, a thruster bell and a console, thrown clear in the crash.
    for i in 0..10 {
        let offset = Vec3::new((i as f32 * 1.7) % 6.0 - 3.0, 3.0 + i as f32 * 0.9, (i as f32 * 2.3) % 5.0 - 6.5);
        let piece = props::debris(&mut meshes, &finishes, i);
        let turned = Quat::from_rotation_y(i as f32 * 1.3) * Quat::from_rotation_z(i as f32 * 0.7);
        commands
            .spawn((
                WorldEntity,
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
    // More of it lies where it was thrown clear: still, resting on the ground, a little larger than
    // the loose pieces. The loose pieces can land on it.
    const SCRAP_SCALE: f32 = 1.35;
    for (j, ground) in sites.scrap.iter().enumerate() {
        let piece = props::debris(&mut meshes, &finishes, j);
        let lift = Vec3::Y * piece.depth(&meshes) * SCRAP_SCALE;
        commands
            .spawn((
                WorldEntity,
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
    let afterimage = materials.add(StandardMaterial { base_color: Color::srgba(1.0, 0.72, 0.25, 0.5), alpha_mode: AlphaMode::Blend, unlit: true, ..default() });
    commands.insert_resource(PropAssets {
        stone,
        pair,
        afterimage,
        quarry_stone: meshes.add(Cuboid::from_size(STONE_SIZE)),
        mass: meshes.add(Cuboid::from_size(MASS_SIZE)),
        casket,
        tiller,
        haul: [haul_cart, crawler_shape],
        ring: (
            meshes.add(Annulus::new(0.8, 1.0).mesh().resolution(40).build().rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))),
            materials.add(StandardMaterial { base_color: Color::srgb(0.9, 0.97, 1.0), unlit: true, ..default() }),
        ),
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
            return Err(format!("{} hasn't been entered yet.", spec.label));
        }
        let quantity = evaluate(text, &game.save.notebook).map_err(|e| format!("{}: {e}", spec.label))?;
        inputs.insert(spec.key.to_string(), quantity);
        values.push(CommittedValue { label: spec.label.to_string(), quantity, unit: spec.unit.to_string() });
    }
    Ok((inputs, values))
}

/// A request the station has accepted: what the world is to show.
enum Accepted {
    Demo { puzzle: usize, playback: DemoPlayback, guess: Option<usize> },
    Measure { puzzle: usize, kind: PuzzleKind },
    DropStone(DropLedge),
    Commit { puzzle: usize, outcome: Outcome },
}

/// Whether a request runs: a demonstration needs its station awake; a commit is judged and may be
/// refused for free. Nothing in the world changes for a request that does not run, so the last
/// test's result stays where it can be looked at. A commit that runs has used its charge from this
/// moment (see [`Game::begin_test`]).
fn accept(game: &mut Game, ui: &mut Ui, request: Request) -> Option<Accepted> {
    match request {
        Request::Demo(puzzle) => {
            if !game.unlocked(puzzle) {
                return None;
            }
            let instance = game.instance(puzzle);
            let playback = instance.demo_raised(game.placed(puzzle))?;
            // The guess counts only before the first run.
            let guess = if game.save.demo_watched(&game.def(puzzle).id) { None } else { ui.guess[puzzle] };
            Some(Accepted::Demo { puzzle, playback, guess })
        }
        Request::Measure(puzzle) => {
            if !game.unlocked(puzzle) {
                return None;
            }
            let kind = game.def(puzzle).kind;
            if kind != PuzzleKind::Chase {
                return None;
            }
            if game.instance(puzzle).measure_unready(game.placed(puzzle)).is_some() {
                return None;
            }
            Some(Accepted::Measure { puzzle, kind })
        }
        Request::DropStone(ledge) => Some(Accepted::DropStone(ledge)),
        Request::Commit(puzzle) => {
            let (outcome, values) = judge_commit(game, ui, puzzle)?;
            game.begin_test(puzzle, values, outcome.clone());
            Some(Accepted::Commit { puzzle, outcome })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_requests(
    mut commands: Commands,
    mut sim: ResMut<Sim>,
    mut game: ResMut<Game>,
    mut ui: ResMut<Ui>,
    mut orbit: ResMut<OrbitCamera>,
    mut ruler: ResMut<Ruler>,
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
    let Some(accepted) = accept(&mut game, &mut ui, request) else { return };

    // Put back anything still lying around from the previous run, so each run starts loaded.
    for (entity, body) in &settling {
        if body.after != AfterSettling::Stay {
            commands.entity(entity).despawn();
        }
    }
    for (prop, mut transform, mut visibility) in &mut props {
        if *prop != Prop::Cargo {
            *transform = rest_transform(*prop, &sites, &game);
            *visibility = if shown(*prop, &game) { Visibility::Visible } else { Visibility::Hidden };
        }
    }
    if sites.cart != sites.rail_start {
        sites.cart = sites.rail_start;
    }
    let ground = GROUND as f32;
    sim.running = Some(match accepted {
        Accepted::Demo { puzzle, playback, guess } => match playback {
            DemoPlayback::Strobe { .. } => {
                for mark in &marks {
                    commands.entity(mark).despawn();
                }
                sim.strobe.clear();
                sim.start_demo_trails(Vec::new());
                for (prop, mut transform, mut visibility) in &mut props {
                    if *prop == Prop::CliffMass {
                        transform.translation = sites.cliff_drop + Vec3::Y * STONE_SIZE.y / 2.0;
                        *visibility = Visibility::Visible;
                    }
                }
                orbit.frame_side_view(sites.cliff_landing, sites.cliff_drop, Vec3::X, false);
                Running::Strobe { puzzle, t: 0.0, flashes: playback.flash_times(), shown: 0, guess }
            }
            DemoPlayback::Pace { cart_speed, crawler_speed, head_start, duration, .. } => {
                for mark in &marks {
                    commands.entity(mark).despawn();
                }
                sim.strobe.clear();
                let run = Run { cart_speed: cart_speed as f32, crawler_speed: crawler_speed as f32, head_start: head_start as f32, end: duration as f32 };
                let (_, crawler_end) = run.at(run.end);
                frame_track(&mut orbit, &sites, crawler_end + CRAWLER_SIZE.x);
                sim.start_demo_trails(PAIR_COLOURS.iter().map(|c| (Vec::new(), *c)).collect());
                Running::Pace { puzzle, t: 0.0, flashes: playback.flash_times(), shown: 0, guess, run }
            }
            DemoPlayback::DropAndFire { dropped, fired } => {
                let site = sites.channel_launch;
                let low = site.at(0.0, dropped.flight.y, ground);
                orbit.frame_side_view(low, site.at(fired.flight.x, dropped.projectile.y0, ground), site.direction, false);
                let mesh = meshes.add(Cuboid::from_length(0.4));
                start_pair(&mut commands, &mut sim, &assets, puzzle, mesh, [dropped, fired], site, 0.4, guess)
            }
            DemoPlayback::TwoAngles { flat, steep } => {
                let site = sites.ridge_launch;
                let (_, _, apex) = steep.projectile.apex();
                orbit.frame_side_view(site.at(-2.0, 0.0, ground), site.at(steep.flight.x + 2.0, apex, ground), site.direction, false);
                let mesh = meshes.add(Cuboid::from_length(0.3));
                start_pair(&mut commands, &mut sim, &assets, puzzle, mesh, [flat, steep], site, 0.3, guess)
            }
        },
        Accepted::Measure { puzzle, kind } => match kind {
            PuzzleKind::Chase => {
                for mark in &marks {
                    commands.entity(mark).despawn();
                }
                sim.strobe.clear();
                let l = &game.layout;
                let (_, post_b) = l.posts();
                let head_start = l.head_start as f32;
                let crawler_speed = l.crawler_speed as f32;
                let end = (post_b as f32 + physics_core::demo::PACE_OVERRUN as f32 - head_start) / crawler_speed;
                let run = Run { cart_speed: 0.0, crawler_speed, head_start, end };
                let (_, crawler_end) = run.at(run.end);
                frame_track(&mut orbit, &sites, crawler_end + CRAWLER_SIZE.x);
                sim.start_measure_trails(puzzle, vec![(Vec::new(), PAIR_COLOURS[1])]);
                Running::TimeCrawler { t: 0.0, run }
            }
            _ => unreachable!("only Chase accepts Request::Measure"),
        },
        Accepted::DropStone(ledge) => {
            for mark in &marks {
                commands.entity(mark).despawn();
            }
            sim.strobe.clear();
            let gravity = game.puzzle_of(PuzzleKind::Gravity);
            sim.start_measure_trails(gravity, Vec::new());
            let release = ledge.release(&sites);
            let landing = ledge.landing(&sites);
            for (prop, mut transform, mut visibility) in &mut props {
                if *prop == Prop::CliffMass {
                    transform.translation = release + Vec3::Y * STONE_SIZE.y / 2.0;
                    *visibility = Visibility::Visible;
                }
            }
            match ledge {
                DropLedge::Low => orbit.frame_side_view(landing - Vec3::Y * 1.0, release + Vec3::Y * 1.5, Vec3::X, false),
                DropLedge::Cliff => orbit.frame_side_view(landing, release, Vec3::X, false),
            }
            Running::QuarryDrop { ledge, t: 0.0 }
        }
        Accepted::Commit { puzzle, outcome } => {
            ruler.put_away();
            let instance = game.instance(puzzle);
            // A run of the tutorial after it is solved fetches the cart back down the line for it.
            if instance.kind == PuzzleKind::Rail {
                for (prop, _, mut visibility) in &mut props {
                    match prop {
                        Prop::Cart => *visibility = Visibility::Visible,
                        Prop::HaulCart => *visibility = Visibility::Hidden,
                        _ => {}
                    }
                }
            }
            // The verdict exists now, but it is shown only when the test has played out.
            ui.results[puzzle] = None;
            sim.start_test_trail(puzzle, TRAIL);
            let verdict = Verdict { outcome: outcome.clone() };
            match outcome.playback {
                Playback::Drop { predicted_time, actual_time, .. } => {
                    orbit.frame_side_view(sites.overhang_socket, sites.overhang_release, Vec3::X, false);
                    Running::OverhangDrop { puzzle, t: 0.0, predicted: predicted_time as f32, actual: actual_time as f32, landed: false, verdict }
                }
                Playback::Flight { projectile, flight } => {
                    let site = if instance.kind == PuzzleKind::Channel { sites.channel_launch } else { sites.ridge_launch };
                    // The box holds the release, the top of the arc and the landing. A launch that
                    // climbs has its top halfway along, so all three are needed.
                    let (_, apex_x, apex_y) = projectile.apex();
                    let points = [site.at(0.0, projectile.y0, ground), site.at(apex_x, apex_y, ground), site.at(flight.x, flight.y, ground)];
                    let (low, high) = crate::stations::bounds(&points);
                    orbit.frame_side_view(low, high, site.direction, false);
                    Running::Flight { puzzle, t: 0.0, projectile, flight, site, landed: false, verdict }
                }
                Playback::Chase { cart_speed, crawler_speed, head_start, t } => {
                    // The experiment's afterimages would stand on the track the test runs along.
                    for mark in &marks {
                        commands.entity(mark).despawn();
                    }
                    let run = Run { cart_speed: cart_speed as f32, crawler_speed: crawler_speed as f32, head_start: head_start as f32, end: t as f32 };
                    let (_, crawler_end) = run.at(run.end);
                    frame_track(&mut orbit, &sites, crawler_end.max(game.layout.transfer_at as f32) + CRAWLER_SIZE.x);
                    Running::Chase { puzzle, t: 0.0, run, verdict }
                }
                Playback::Slide { distance } => {
                    let reach = distance.max(game.layout.rail_target as f64) as f32;
                    orbit.frame_side_view(sites.rail_start, sites.rail_start + Vec3::new(reach, 2.0, 0.0), Vec3::X, false);
                    let duration = 1.0 + distance as f32 / 6.0;
                    Running::Slide { puzzle, t: 0.0, distance: distance as f32, duration, verdict }
                }
            }
        }
    });
}

/// Look at the haul track from the side, from just behind the cart's start to `reach` metres along it.
fn frame_track(orbit: &mut OrbitCamera, sites: &Sites, reach: f32) {
    let (min, max, along) = crate::stations::haul_view(sites, reach);
    orbit.frame_side_view(min, max, along, false);
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
    sim.start_demo_trails(PAIR_COLOURS.iter().map(|c| (Vec::new(), *c)).collect());
    for (k, shot) in shots.iter().enumerate() {
        let (x, y) = shot.projectile.pos(0.0);
        commands.spawn((
            WorldEntity,
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
fn hand_over(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    size: Vec3,
    centre: Vec3,
    velocity: Vec3,
    material: Handle<StandardMaterial>,
    after: AfterSettling,
) -> Entity {
    commands
        .spawn((
            WorldEntity,
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
        ))
        .id()
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
    // that points the camera keeps it). At an open station it goes back to the station's view.
    if sim.running.is_none() && sim.watch_left > 0.0 {
        sim.watch_left -= dt;
        if sim.watch_left <= 0.0 {
            match ui.station {
                Some(station) => crate::stations::look_at_station(&mut orbit, &game, &sites, station),
                None => orbit.release_view(),
            }
        }
    }
    sim.action = None;
    let Some(mut running) = sim.running.take() else { return };

    let mut finished: Option<(usize, Verdict)> = None;

    match &mut running {
        Running::Strobe { puzzle, t, flashes, shown, guess } => {
            *t += dt;
            let height = game.layout.cliff_h as f32;
            let fall_time = (2.0 * height / g).sqrt();
            let start = sites.cliff_drop + Vec3::Y * STONE_SIZE.y / 2.0;
            // Each flash leaves an afterimage of the quarry stone and a tick beside its path.
            while *shown < flashes.len() && *t >= flashes[*shown] as f32 {
                let below = fallen(g, flashes[*shown] as f32, height);
                let centre = start - Vec3::Y * below;
                commands.spawn((
                    WorldEntity,
                    StrobeMark,
                    Mesh3d(assets.quarry_stone.clone()),
                    MeshMaterial3d(assets.afterimage.clone()),
                    Transform::from_translation(centre),
                ));
                sim.strobe.push(centre);
                *shown += 1;
            }
            for (prop, mut transform, mut visibility, material) in &mut props {
                if *prop != Prop::CliffMass {
                    continue;
                }
                transform.translation = start - Vec3::Y * fallen(g, *t, height);
                sim.action = Some(transform.translation);
                if *t >= fall_time {
                    *visibility = Visibility::Hidden;
                    crate::particles::burst(&mut commands, crate::particles::Kind::Dust, sites.cliff_landing, 0.8);
                    hand_over(
                        &mut commands,
                        assets.quarry_stone.clone(),
                        STONE_SIZE,
                        transform.translation,
                        Vec3::NEG_Y * g * fall_time,
                        material.0.clone(),
                        AfterSettling::Vanish,
                    );
                }
            }
            if *t >= fall_time {
                let (puzzle, guess) = (*puzzle, *guess);
                finish_demo(&mut game, &mut ui, puzzle, guess);
                sim.running = None;
                sim.watch_left = 2.5;
                return;
            }
        }
        Running::QuarryDrop { ledge, t } => {
            *t += dt;
            let release = ledge.release(&sites);
            let landing = ledge.landing(&sites);
            let height = (release.y - landing.y).max(0.1);
            let fall_time = (2.0 * height / g).sqrt();
            let start = release + Vec3::Y * STONE_SIZE.y / 2.0;
            for (prop, mut transform, mut visibility, material) in &mut props {
                if *prop != Prop::CliffMass {
                    continue;
                }
                transform.translation = start - Vec3::Y * fallen(g, *t, height);
                sim.action = Some(transform.translation);
                if *t >= fall_time {
                    *visibility = Visibility::Hidden;
                    crate::particles::burst(&mut commands, crate::particles::Kind::Dust, landing, 0.9);
                    hand_over(
                        &mut commands,
                        assets.quarry_stone.clone(),
                        STONE_SIZE,
                        transform.translation,
                        Vec3::NEG_Y * g * fall_time,
                        material.0.clone(),
                        AfterSettling::Vanish,
                    );
                }
            }
            if *t >= fall_time {
                let ledge = *ledge;
                note_quarry_drop(&mut game, ledge);
                game.persist();
                match ledge {
                    DropLedge::Low => sim.low_drops += 1,
                    DropLedge::Cliff => sim.cliff_drops += 1,
                }
                sim.running = None;
                sim.watch_left = 1.6;
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
                let (puzzle, guess) = (*puzzle, *guess);
                finish_demo(&mut game, &mut ui, puzzle, guess);
                sim.running = None;
                sim.watch_left = 2.5;
                return;
            }
        }
        Running::TimeCrawler { t, run, .. } => {
            *t += dt;
            let (_, crawler) = run.at(*t);
            if let Some((trail, _)) = sim.trails.first_mut() {
                trail.push(sites.haul_at(crawler as f64) + Vec3::Y * 0.05);
            }
            let pos = crawler_centre(&sites, crawler);
            for (prop, mut transform, _, _) in props.iter_mut() {
                if *prop == Prop::Crawler {
                    transform.translation = pos;
                }
            }
            sim.action = Some(pos);
            if *t >= run.end {
                note_crawler(&mut game);
                game.persist();
                sim.running = None;
                sim.watch_left = 2.0;
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
                    if on_water(&shot.flight) {
                        splash(&mut commands, &assets, centre);
                    }
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
        Running::OverhangDrop { puzzle, t, predicted, actual, landed, verdict } => {
            *t += dt;
            let height = game.layout.overhang_h as f32;
            let pass = verdict.outcome.pass;
            let end_t = if pass { *actual + 0.35 } else { (*actual).max((*predicted).min(*actual + 1.2)) + 0.45 };
            for (prop, mut transform, mut visibility, material) in &mut props {
                if *prop != Prop::Counterweight {
                    continue;
                }
                let start = rest_pose(Prop::Counterweight, &sites, &game);
                let stop_drop = if pass { height + 0.15 } else { height - SHUTTER_HEIGHT };
                transform.translation = start - Vec3::Y * fallen(g, *t, stop_drop);
                sim.action = Some(transform.translation);
                if *t >= *actual && !*landed {
                    *landed = true;
                    crate::particles::burst(&mut commands, crate::particles::Kind::Dust, sites.overhang_socket, 1.2);
                    if !pass {
                        *visibility = Visibility::Hidden;
                        hand_over(
                            &mut commands,
                            assets.mass.clone(),
                            MASS_SIZE,
                            transform.translation,
                            Vec3::new(0.4, -g * *actual * 0.4, 0.3),
                            material.0.clone(),
                            AfterSettling::Restore(Prop::Counterweight),
                        );
                    }
                }
            }
            if *t >= end_t {
                finished = Some((*puzzle, verdict.clone()));
            }
        }
        Running::Flight { puzzle, t, projectile, flight, site, landed, verdict } => {
            // A short run-up along the rail before release, so the launch reads as a launch.
            const RUN_UP: f32 = 0.35;
            *t += dt;
            let prop = if game.def(*puzzle).kind == PuzzleKind::Channel { Prop::Casket } else { Prop::Tiller };
            let half = Vec3::Y * prop_size(prop).y / 2.0;
            let flight_t = (*t - RUN_UP).clamp(0.0, flight.t as f32) as f64;
            let (x, y) = projectile.pos(flight_t);
            let mut centre = site.at(x, y, ground) + half;
            if *t < RUN_UP {
                // From the launcher, even when the load has been resting elsewhere since a pass.
                let start = launch_pose(prop, &sites);
                let progress = *t / RUN_UP;
                centre = start.lerp(site.at(0.0, projectile.y0, ground) + half, progress * progress);
            } else if let Some((trail, _)) = sim.trails.first_mut() {
                trail.push(centre - half);
            }
            sim.action = Some(centre);
            let (vx, vy) = projectile.vel(flight_t);
            let velocity = site.direction * vx as f32 + Vec3::Y * vy as f32;
            for (candidate, mut transform, mut visibility, _) in &mut props {
                if *candidate != prop {
                    continue;
                }
                transform.translation = centre;
                if prop == Prop::Tiller {
                    // The tiller points along its path.
                    transform.rotation = Quat::from_rotation_arc(Vec3::X, velocity.normalize_or(Vec3::X));
                }
                if *landed {
                    continue;
                }
                // In flight even where it is not shown at rest (a test run again after a pass).
                visibility.set_if_neq(Visibility::Visible);
                if *t - RUN_UP >= flight.t as f32 {
                    *landed = true;
                    *visibility = Visibility::Hidden;
                    if on_water(flight) {
                        splash(&mut commands, &assets, centre);
                    }
                    // A casket that lands on its target rests where it came down, beside the tilting
                    // launcher. The tiller's first pass stays in the landing circle until the skiff
                    // takes it; the skiff takes it once, so a pass run again only goes back. A miss is
                    // returned to its launcher.
                    let first_pass = verdict.outcome.pass && !game.solved(*puzzle);
                    let after = match prop {
                        Prop::Casket if verdict.outcome.pass => AfterSettling::Rest(prop),
                        Prop::Tiller if first_pass => AfterSettling::Stay,
                        _ => AfterSettling::Restore(prop),
                    };
                    let (body, quartz, lit) = if prop == Prop::Tiller { &assets.tiller } else { &assets.casket };
                    let load = hand_over(&mut commands, body.clone(), prop_size(prop), centre, velocity, assets.stone.clone(), after);
                    commands.entity(load).with_child((Mesh3d(quartz.clone()), MeshMaterial3d(lit.clone())));
                    if prop == Prop::Tiller && first_pass {
                        commands.entity(load).insert(crate::vessel::Delivered);
                    }
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
            let start = rest_pose(Prop::Cart, &sites, &game);
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

    let Some((puzzle, Verdict { outcome })) = finished else {
        sim.running = Some(running);
        return;
    };
    // The test has played out: now it counts (the save file has had it since the commit).
    sim.watch_left = 2.5;
    let outcome = game.finish_test().unwrap_or(outcome);
    ui.results[puzzle] = Some(outcome);
    ui.scroll_to_result[puzzle] = true;
}

/// Dimensions of each sliding stone shutter leaf covering the tidal slab's quartz socket.
const SHUTTER_WIDTH: f32 = 0.54;
const SHUTTER_HEIGHT: f32 = 0.16;
const SHUTTER_SLIDE: f32 = 0.66;

fn shutter_pose(socket: Vec3, sign: f32, open: f32) -> Vec3 {
    let closed_offset = sign * (SHUTTER_WIDTH / 2.0);
    socket + Vec3::new(closed_offset + sign * SHUTTER_SLIDE * open.clamp(0.0, 1.0), SHUTTER_HEIGHT / 2.0, 0.0)
}

/// How far open (`0.0` closed .. `1.0` open) the stone shutters are at `t` during a verification
/// drop. The visual timing strictly mirrors the pass/fail verdict: on a pass they slide open to
/// receive the counterweight; on a miss they either open after impact (too late) or snap shut
/// before impact (too early).
fn shutter_open(t: f32, predicted: f32, actual: f32, pass: bool) -> f32 {
    let smooth = |x: f32| {
        let u = x.clamp(0.0, 1.0);
        u * u * (3.0 - 2.0 * u)
    };
    if pass {
        let start = (actual - 0.22).max(0.0);
        smooth((t - start) / 0.18)
    } else if actual < predicted {
        // Gravity stronger than predicted: shutters stay firmly closed at impact, then open late.
        let open_at = predicted.min(actual + 1.0).max(actual + 0.12);
        if t < open_at {
            0.0
        } else if t < open_at + 0.14 {
            smooth((t - open_at) / 0.14)
        } else if t < open_at + 0.30 {
            1.0
        } else {
            1.0 - smooth((t - (open_at + 0.30)) / 0.14)
        }
    } else {
        // Gravity weaker than predicted: shutters open early and snap shut before impact.
        let close_by = (actual - 0.06).max(0.18);
        let open_at = (predicted - 0.14).clamp(0.04, (close_by - 0.22).max(0.04));
        let mid = (open_at + close_by) / 2.0;
        if t < open_at || t > close_by {
            0.0
        } else if t < mid {
            smooth((t - open_at) / (mid - open_at).max(0.05))
        } else {
            1.0 - smooth((t - mid) / (close_by - mid).max(0.05))
        }
    }
}

fn pose_shutters(sim: Res<Sim>, sites: Res<Sites>, game: Res<Game>, mut leaves: Query<(&ShutterLeaf, &mut Transform)>) {
    let open = match &sim.running {
        Some(Running::OverhangDrop { t, predicted, actual, verdict, .. }) => shutter_open(*t, *predicted, *actual, verdict.outcome.pass),
        _ if game.kind_solved(PuzzleKind::Gravity) => 1.0,
        _ => 0.0,
    };
    for (leaf, mut transform) in &mut leaves {
        transform.translation = shutter_pose(sites.overhang_socket, leaf.0, open);
    }
}

/// What the stopwatch's reading is called in the notebook at the haul track.
pub const CRAWLER_TIME: &str = "crawler's time between the timing posts";
/// What the stopwatch's readings are called in the notebook at the sea-cliff quarry.
pub const LOW_DROP_TIME: &str = "low quarry step: fall time";
pub const CLIFF_DROP_TIME: &str = "cliff rim: fall time";

/// Write the stopwatch's reading of the crawler, from the first timing post to the second, into the
/// notebook. Every run gives the same reading, so it is noted once.
pub fn note_crawler(game: &mut Game) {
    let reading = Quantity::seconds(game.layout.crawler_time());
    if !game.save.notebook.has_reading(CRAWLER_TIME, reading) {
        game.save.notebook.add_numbered("t", CRAWLER_TIME, reading, source::STOPWATCH);
    }
}

/// Write Lumen's stopwatch reading of a quarry stone dropped off `ledge` into the notebook: time
/// only (`t1`, `t2`, ...), so the player measures the drop height with the ruler. A reading
/// already in the notebook is not noted again.
pub fn note_quarry_drop(game: &mut Game, ledge: DropLedge) {
    let (label, seconds) = match ledge {
        DropLedge::Low => (LOW_DROP_TIME, game.layout.low_fall_time()),
        DropLedge::Cliff => (CLIFF_DROP_TIME, game.layout.cliff_fall_time()),
    };
    let reading = Quantity::seconds(seconds);
    if !game.save.notebook.has_reading(label, reading) {
        game.save.notebook.add_numbered("t", label, reading, source::STOPWATCH);
    }
}

/// Seconds the cart takes to roll on from the tutorial's marker, through the stone gate, to the haul
/// cart's mark.
const ROLL_ON: f32 = 3.0;

/// What the relay of the chapter has done so far, as the world shows it.
#[derive(Default)]
struct Relayed {
    /// The tutorial's solved state and the channel's, as last shown; `None` before the first look.
    shown: Option<(bool, bool)>,
    /// The cart rolling on up the line after carrying its block to the marker: seconds since it set off, and
    /// where from.
    rolling: Option<(f32, Vec3)>,
    busy: bool,
}

/// Keep the line's one cart and the tiller where the chapter has got them to. When the tutorial
/// is solved, the cart rolls on through the stone gate to wait for the crawler; when the channel is,
/// the tiller is on the tilting launcher. Once the world shows a stage it is left alone, so a tiller
/// that has just landed in the circle is not put back on the launcher.
fn relay(
    time: Res<Time>,
    (game, sites, waking): (Res<Game>, Res<Sites>, Res<crate::waking::Waking>),
    mut sim: ResMut<Sim>,
    mut carry: ResMut<Carry>,
    mut props: Query<(&Prop, &mut Transform, &mut Visibility)>,
) {
    // The cart rolls on once its block is at the marker and the stone gate has opened for it.
    let (rail, channel) = (game.kind_solved(PuzzleKind::Rail) && waking.gate_is_open(&sites.way), game.kind_solved(PuzzleKind::Channel));
    let busy = sim.busy();
    let relayed = &mut sim.relayed;
    let finished = std::mem::replace(&mut relayed.busy, busy) && !busy;
    if busy {
        return;
    }
    let first = relayed.shown.is_none();
    let (was_rail, was_channel) = relayed.shown.unwrap_or((rail, channel));
    relayed.shown = Some((rail, channel));
    let start = rest_pose(Prop::HaulCart, &sites, &game);
    if rail && !was_rail {
        let from = props.iter().find(|(p, _, _)| **p == Prop::Cart).map_or(start, |(_, t, _)| t.translation);
        relayed.rolling = Some((0.0, from));
    }
    let mut swap = first || finished;
    if let Some((t, from)) = relayed.rolling.as_mut() {
        *t += time.delta_secs();
        let progress = (*t / ROLL_ON).min(1.0);
        let eased = progress * progress * (3.0 - 2.0 * progress);
        let at = from.lerp(start.with_y(from.y), eased);
        for (prop, mut transform, _) in &mut props {
            if *prop == Prop::Cart {
                transform.translation = at;
            }
        }
        if progress < 1.0 {
            return;
        }
        relayed.rolling = None;
        swap = true;
    }
    carry.cart_gone = rail;
    for (prop, mut transform, mut visibility) in &mut props {
        let show = match prop {
            Prop::Cart if swap => !rail,
            Prop::HaulCart if swap => rail,
            Prop::Tiller if first || channel != was_channel => shown(*prop, &game),
            _ => continue,
        };
        if show && *prop == Prop::Tiller {
            *transform = rest_transform(*prop, &sites, &game);
        }
        visibility.set_if_neq(if show { Visibility::Visible } else { Visibility::Hidden });
    }
}

/// The block rides on the cart while it is loaded, until the cart has gone on up the line.
fn ride_the_cart(carry: Res<Carry>, mut props: Query<(&Prop, &mut Transform, &mut Visibility)>) {
    let Some(cart) = props.iter().find(|(p, _, _)| **p == Prop::Cart).map(|(_, t, _)| t.translation) else { return };
    let loaded = carry.holds(Slot::Cart) > 0 && !carry.cart_gone;
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

/// Remove settled bodies when their time is up and put the launcher's load back, or leave it where
/// it came to rest.
#[allow(clippy::type_complexity)]
fn settle(
    time: Res<Time>,
    mut commands: Commands,
    sites: Res<Sites>,
    game: Res<Game>,
    mut settling: Query<(Entity, &mut Settling, &Transform), Without<Prop>>,
    mut props: Query<(&Prop, &mut Transform, &mut Visibility)>,
) {
    for (entity, mut body, at) in &mut settling {
        body.left -= time.delta_secs();
        if body.left > 0.0 {
            continue;
        }
        let restores = match body.after {
            AfterSettling::Stay => {
                // A load on its target stays where it came to rest.
                commands.entity(entity).remove::<Settling>();
                continue;
            }
            AfterSettling::Vanish => {
                commands.entity(entity).despawn();
                continue;
            }
            AfterSettling::Rest(prop) => {
                commands.entity(entity).despawn();
                for (candidate, mut transform, mut visibility) in &mut props {
                    if *candidate == prop {
                        *transform = *at;
                        *visibility = Visibility::Visible;
                    }
                }
                continue;
            }
            AfterSettling::Restore(prop) => prop,
        };
        commands.entity(entity).despawn();
        for (prop, mut transform, mut visibility) in &mut props {
            if *prop == restores {
                *transform = rest_transform(restores, &sites, &game);
                *visibility = if shown(restores, &game) { Visibility::Visible } else { Visibility::Hidden };
            }
        }
    }
}

/// Keep the ledge launcher's load on its cradle when blocks go into or out of the jack frame, until
/// it has been thrown across.
fn follow_launcher(sim: Res<Sim>, sites: Res<Sites>, game: Res<Game>, mut props: Query<(&Prop, &mut Transform, &Visibility)>) {
    if !sites.is_changed() || sim.busy() || game.kind_solved(PuzzleKind::Channel) {
        return;
    }
    for (prop, mut transform, visibility) in &mut props {
        if *prop == Prop::Casket && *visibility != Visibility::Hidden {
            transform.translation = launch_pose(Prop::Casket, &sites);
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

/// Split a polyline into world-space dashed segments of length `dash` separated by `gap` metres, so
/// ghost trails look consistent at any frame rate.
fn dashed_segments(trail: &[Vec3], dash: f32, gap: f32) -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    let period = dash + gap;
    if trail.len() < 2 || dash <= 0.0 || period <= 0.0 {
        return out;
    }
    let mut dist = 0.0;
    for pair in trail.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let seg_len = a.distance(b);
        if seg_len < 1e-5 {
            continue;
        }
        let mut s = 0.0;
        while s < seg_len {
            let phase = (dist + s).rem_euclid(period);
            if phase < dash {
                let take = (dash - phase).min(seg_len - s);
                out.push((a.lerp(b, s / seg_len), a.lerp(b, (s + take) / seg_len)));
                s += take;
            } else {
                s += (period - phase).min(seg_len - s);
            }
        }
        dist += seg_len;
    }
    out
}

fn draw_trail(sim: Res<Sim>, mut gizmos: Gizmos) {
    // Lines keep their width on screen, so the strobe's ticks read from a distance.
    for at in &sim.strobe {
        let base = *at - Vec3::Y * STONE_SIZE.y / 2.0;
        gizmos.line(base + Vec3::X * (STONE_SIZE.x / 2.0 + 0.2), base + Vec3::X * 2.6, Color::srgb(1.0, 0.85, 0.3));
    }
    // Faint dashed ghost arcs from earlier tests at the same station, older ones dimmer.
    let past_count = sim.past_trails.len();
    for (idx, trail) in sim.past_trails.iter().enumerate() {
        if trail.len() < 2 {
            continue;
        }
        let recency = (idx + 1) as f32 / past_count as f32;
        let alpha = 0.22 + 0.28 * recency;
        let colour = Color::srgba(0.78, 0.72, 0.58, alpha);
        for (a, b) in dashed_segments(trail, 0.45, 0.30) {
            gizmos.line(a, b, colour);
        }
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

    /// Mark the puzzle of `kind` solved.
    fn solve(game: &mut Game, kind: PuzzleKind) {
        let id = game.chapter.puzzles.iter().find(|p| p.kind == kind).unwrap().id.clone();
        game.save.puzzles.entry(id).or_default().solved = true;
    }

    #[test]
    fn the_helm_is_in_one_place_at_a_time() {
        let mut game = game(13);
        let (_, sites) = crate::island::build(&game.layout);
        let platform = sites.channel_launch.at(game.layout.platform_centre, 0.0, GROUND as f32);
        // Before the channel: the casket rides the ledge launcher, and the tiller is in it.
        assert!(shown(Prop::Casket, &game) && !shown(Prop::Tiller, &game));
        assert_eq!(rest_pose(Prop::Casket, &sites, &game), launch_pose(Prop::Casket, &sites));
        // Across: the casket rests on the platform, and the tiller waits on the tilting launcher.
        for kind in [PuzzleKind::Rail, PuzzleKind::Chase, PuzzleKind::Gravity, PuzzleKind::Channel] {
            solve(&mut game, kind);
        }
        assert!(shown(Prop::Casket, &game) && shown(Prop::Tiller, &game));
        assert!(rest_pose(Prop::Casket, &sites, &game).xz().distance(platform.xz()) < 1e-4, "the casket is on the platform");
        assert_ne!(rest_pose(Prop::Casket, &sites, &game), launch_pose(Prop::Casket, &sites));
        // Over the ridge: the skiff has the tiller, so the launcher is empty.
        solve(&mut game, PuzzleKind::Ridge);
        assert!(!shown(Prop::Tiller, &game), "the tiller is in the skiff, not on the launcher");
        assert!(shown(Prop::Casket, &game));
    }

    #[test]
    fn a_test_counts_in_the_save_file_from_the_moment_it_is_committed() {
        let mut game = game(14);
        let gravity = game.chapter.puzzles.iter().position(|p| p.kind == PuzzleKind::Gravity).unwrap();
        let mut ui = Ui::default();
        ui.drafts[gravity] = vec![format!("{} m/s^2", game.layout.g)];
        for kind in [PuzzleKind::Rail, PuzzleKind::Chase] {
            solve(&mut game, kind);
        }
        let Some(Accepted::Commit { outcome, .. }) = accept(&mut game, &mut ui, Request::Commit(gravity)) else { panic!("the commit runs") };
        assert!(outcome.pass);
        // The charge is used at once, but the game shows the pass only once the test has played out.
        assert_eq!(game.charges_left(gravity), 2);
        assert!(!game.solved(gravity) && game.save.notebook.get("g").is_none());
        // Quitting now: the save file already holds the whole test.
        let on_disk = physics_core::progress::Save::from_ron(&std::fs::read_to_string(&game.save_path).unwrap()).unwrap();
        assert!(on_disk.is_solved(&game.def(gravity).id), "a test cannot be taken back by quitting");
        assert_eq!(on_disk.progress(&game.def(gravity).id).attempts.len(), 1);
        assert!(on_disk.notebook.get("g").is_some(), "the calibrated g is noted with it");
        // Played out: the game counts it, once.
        assert_eq!(game.finish_test().map(|o| o.pass), Some(true));
        assert!(game.solved(gravity) && game.save.notebook.get("g").is_some());
        assert_eq!(game.save.progress(&game.def(gravity).id).attempts.len(), 1);
        assert_eq!(game.charges_left(gravity), 2);
        assert_eq!(physics_core::progress::Save::from_ron(&std::fs::read_to_string(&game.save_path).unwrap()).unwrap(), game.save);
        assert!(game.finish_test().is_none());
        let _ = std::fs::remove_file(&game.save_path);
    }

    #[test]
    fn a_refused_request_runs_nothing() {
        let mut game = game(15);
        let mut ui = Ui::default();
        // The tutorial, solved, with its block taken off the cart: refused, and nothing is charged.
        solve(&mut game, PuzzleKind::Rail);
        ui.drafts[0] = vec![format!("{} m", game.layout.rail_target)];
        assert!(accept(&mut game, &mut ui, Request::Commit(0)).is_none());
        assert!(ui.refusal.is_some());
        assert!(game.pending.is_none());
        assert_eq!(game.charges_left(0), 3);
        // At the sea-cliff quarry, the conceptual demo runs from the console; measurements happen
        // directly in the world via Request::DropStone (not Request::Measure).
        let gravity = game.chapter.puzzles.iter().position(|p| p.kind == PuzzleKind::Gravity).unwrap();
        solve(&mut game, PuzzleKind::Chase);
        assert!(matches!(accept(&mut game, &mut ui, Request::Demo(gravity)), Some(Accepted::Demo { .. })));
        assert!(accept(&mut game, &mut ui, Request::Measure(gravity)).is_none());
        assert!(matches!(accept(&mut game, &mut ui, Request::DropStone(DropLedge::Cliff)), Some(Accepted::DropStone(DropLedge::Cliff))));
        let _ = std::fs::remove_file(&game.save_path);
    }

    #[test]
    fn quarry_drops_note_time_only_and_once_per_ledge() {
        let mut game = game(12);
        note_quarry_drop(&mut game, DropLedge::Low);
        note_quarry_drop(&mut game, DropLedge::Cliff);
        let nb = &game.save.notebook;
        assert!(nb.get("h1").is_none(), "height must be measured with the ruler, not logged by the drop");
        assert_eq!(nb.get("t1").unwrap().quantity, Quantity::seconds(game.layout.low_fall_time()));
        assert_eq!(nb.get("t1").unwrap().source, source::STOPWATCH);
        assert_eq!(nb.get("t2").unwrap().quantity, Quantity::seconds(game.layout.cliff_fall_time()));
        note_quarry_drop(&mut game, DropLedge::Cliff);
        assert!(game.save.notebook.get("t3").is_none(), "the same drop time is noted once");
    }

    #[test]
    fn stone_shutters_match_the_verdict_at_impact() {
        // On a pass the shutters are wide open when the counterweight arrives.
        assert!(shutter_open(1.5, 1.5, 1.5, true) > 0.95);
        // On a miss where gravity is stronger (arrives early), shutters are closed at impact and open later.
        assert_eq!(shutter_open(1.5, 1.7, 1.5, false), 0.0);
        assert!(shutter_open(1.85, 1.7, 1.5, false) > 0.9);
        // On a miss where gravity is weaker (arrives late), shutters open early and are closed at impact.
        assert!(shutter_open(1.3, 1.3, 1.6, false) > 0.5);
        assert_eq!(shutter_open(1.6, 1.3, 1.6, false), 0.0);
    }

    #[test]
    fn the_crawler_is_timed_into_the_notebook_once() {
        let mut game = game(12);
        note_crawler(&mut game);
        note_crawler(&mut game);
        let times: Vec<_> = game.save.notebook.entries.iter().filter(|e| e.label == CRAWLER_TIME).collect();
        assert_eq!(times.len(), 1);
        assert_eq!(times[0].name, "t1");
    }

    #[test]
    fn past_test_trails_are_kept_per_station_and_capped() {
        let mut sim = Sim::default();
        for n in 0..5 {
            sim.start_test_trail(3, TRAIL);
            sim.trails[0].0.extend([Vec3::new(0.0, n as f32, 0.0), Vec3::new(1.0, n as f32, 0.0)]);
        }
        assert_eq!(sim.past_trails.len(), MAX_PAST_TRAILS);
        assert_eq!(sim.past_trails[0][0].y, 1.0, "oldest trail beyond the cap is dropped");
        assert_eq!(sim.past_trails[2][0].y, 3.0);
        // Switching stations clears the archived trails.
        sim.start_test_trail(4, TRAIL);
        assert!(sim.past_trails.is_empty());
        // Running a measurement at the same station preserves prior test trails and does not
        // archive the measurement's own trail.
        sim.trails[0].0.extend([Vec3::ZERO, Vec3::X]);
        sim.start_measure_trails(4, vec![(vec![Vec3::Y, Vec3::Y + Vec3::X], PAIR_COLOURS[1])]);
        assert_eq!(sim.past_trails.len(), 1);
        sim.start_test_trail(4, TRAIL);
        assert_eq!(sim.past_trails.len(), 1, "measurement trail is not archived as a test ghost");
        // Running a demo clears archived test trails.
        sim.start_demo_trails(Vec::new());
        assert!(sim.past_trails.is_empty());
    }

    #[test]
    fn dashed_segments_use_world_space_length_independent_of_sample_density() {
        let coarse = vec![Vec3::ZERO, Vec3::X * 3.0];
        let dense: Vec<Vec3> = (0..=60).map(|i| Vec3::X * (3.0 * i as f32 / 60.0)).collect();
        let total_len = |segs: &[(Vec3, Vec3)]| segs.iter().map(|(a, b)| a.distance(*b)).sum::<f32>();
        let l_coarse = total_len(&dashed_segments(&coarse, 0.45, 0.30));
        let l_dense = total_len(&dashed_segments(&dense, 0.45, 0.30));
        assert!((l_coarse - l_dense).abs() < 1e-3, "coarse={l_coarse}, dense={l_dense}");
        assert!((l_coarse - 1.8).abs() < 1e-3, "4 dashes of 0.45 m across 3.0 m = 1.8 m, got {l_coarse}");
    }
}
