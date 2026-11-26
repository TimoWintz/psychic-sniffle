//! The way wakes: the ancient road's machines sleep at the start and wake one by one as the
//! chapter's puzzles pass.
//!
//! Everything follows from one number, the **stage**: how many of the chapter's puzzles are solved
//! (they open in order, so that is also the index of the next one). At stage `k`:
//!
//! - machine `i` (the five stations, then the north gate) has woken when `1 ≤ i ≤ k`: the light has
//!   run to it, its **wake** quartz is lit and its moving quartz glows;
//! - its job is done when `i < k`: its **done** quartz is lit (the marker's studs, the crane, the
//!   tower to the top, both rims, the landing circle);
//! - the stretch of inlay from machine `i` to the next is lit when `i < k`;
//! - console `i` (each machine has its own, a stone of the builders' with a quartz face) and the
//!   lamps beside it are awake when `i ≤ k`: the first is the only one awake at the start;
//! - the stone gate's leaf is down from stage 1, the ramp onto the outcrop stands from stage 3, the
//!   bridge over the channel stands from stage 4, and at stage 5, when the skiff takes its helm, the
//!   seal in the pass through the spine is down and the north gate's beam shines;
//! - the launchers' sight is off the crawler and at the crane's foot from stage 2, and set on the
//!   ledge launcher from stage 3.
//!
//! The island is built awake (`island::build`); [`settle`] puts it to the stage the save is at
//! before anything is meshed or collides. The world shows the stage in [`Waking::shown`], which
//! trails the save's while a wake plays.

use crate::carry::DirtyCells;
use crate::companion::Landmarks;
use crate::island::{Cable, MACHINES, Sites, Stonework, Way};
use crate::player::OrbitCamera;
use crate::sim::{Sim, TerrainReshaped};
use crate::state::{Game, Mode, PUZZLES, Ui};
use crate::vessel::{SIGHT_SIZE, Sight};
use crate::voxel::{DARK_QUARTZ, VoxelWorld};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use physics_core::materials::Material;

/// The stage the save is at: how many of the chapter's puzzles are solved.
pub fn stage(game: &Game) -> usize {
    game.next_unsolved().unwrap_or(PUZZLES)
}

/// Whether the light has reached a machine and it has woken.
pub fn wake_lit(machine: usize, stage: usize) -> bool {
    machine >= 1 && machine <= stage
}

/// Whether a machine's own job has been done.
pub fn done_lit(machine: usize, stage: usize) -> bool {
    machine < stage
}

pub fn run_lit(run: usize, stage: usize) -> bool {
    run < stage
}

/// Whether machine `i`'s console and lamps are awake (the north gate's lamps for `i` = 5).
pub fn console_awake(machine: usize, stage: usize) -> bool {
    machine <= stage
}

pub fn beam_shines(stage: usize) -> bool {
    stage >= 5
}

/// Whether the crane has lifted the sight off the crawler, and whether it is set on the ledge
/// launcher.
pub fn sight_lifted(stage: usize) -> bool {
    stage >= 2
}

pub fn sight_fitted(stage: usize) -> bool {
    stage >= 3
}

/// What changed in the world: cells to mesh again, and whether any block came or went (rather than
/// just lighting), so the physics engine's view of the island must be built again.
#[derive(Default, Debug)]
pub struct Changes {
    pub cells: Vec<IVec3>,
    pub reshaped: bool,
}

/// Light or put out blocks of the way's quartz, listing the ones that change.
pub fn light(world: &mut VoxelWorld, cells: impl IntoIterator<Item = IVec3>, lit: bool, changes: &mut Changes) {
    for cell in cells {
        if world.is_dark(cell) == lit {
            world.set_dark(cell, !lit);
            changes.cells.push(cell);
        }
    }
}

/// Stand the first `steps` steps of stonework and take away the rest.
fn stand_steps(world: &mut VoxelWorld, work: &Stonework, steps: i32, changes: &mut Changes) {
    for (i, &(cell, material)) in work.cells.iter().enumerate() {
        let want = if work.step(i) < steps { material } else { Material::Air };
        if world.get(cell) != want {
            world.set(cell, want);
            changes.cells.push(cell);
            changes.reshaped = true;
        }
    }
}

/// How many of its steps stonework stands `moved` of the way through its move (0 as it was, 1 done).
fn standing_steps(work: &Stonework, moved: f32) -> i32 {
    let all = work.step_count();
    let gone = (moved.clamp(0.0, 1.0) * all as f32).ceil() as i32;
    if work.rises { gone } else { all - gone }
}

/// Stonework `moved` of the way through its move (0 as it was, 1 done): rising step by step (layer
/// by layer from the ground, unless it says otherwise), or sinking from the top.
pub fn shape(world: &mut VoxelWorld, work: &Stonework, moved: f32, changes: &mut Changes) {
    stand_steps(world, work, standing_steps(work, moved), changes);
}

/// Put the world in the state of a stage at once: the quartz lit or dark, the stonework standing or
/// sunk.
pub fn settle(world: &mut VoxelWorld, way: &Way, stage: usize) -> Changes {
    let mut changes = Changes::default();
    for machine in 0..MACHINES {
        light(world, way.wake[machine].iter().copied(), wake_lit(machine, stage), &mut changes);
        light(world, way.done[machine].iter().copied(), done_lit(machine, stage), &mut changes);
    }
    for (run, inlay) in way.runs.iter().enumerate() {
        light(world, inlay.cells.iter().map(|&(c, _)| c), run_lit(run, stage), &mut changes);
    }
    for work in way.stonework() {
        stand_steps(world, work, if work.stands(stage) { work.step_count() } else { 0 }, &mut changes);
    }
    changes
}

/// The parts of one wake, in order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// The machine whose job was just done lights up (and the crane lifts the sight off, the bridge
    /// rises, or the seal sinks).
    Glow,
    /// The light runs up the road to the next machine.
    Run,
    /// The next machine wakes: its quartz lights and its parts move (the gate's leaf sinks, the
    /// ramp rises, the north gate's beam reaches out).
    Wake,
    /// Its console has lit with it; the camera glides back to the player's own view.
    Home,
    Done,
}

const GLOW: f32 = 1.0;
/// At the crawler's pass, the crane lifts the sight off while its quartz lights.
const GLOW_LIFT: f32 = 3.2;
/// At the casket's pass the bridge over the channel rises while both banks light, and at the
/// tiller's the seal in the pass sinks while the landing circle lights.
const GLOW_MOVE: f32 = 4.0;
/// How fast the light runs, and the shortest and longest a run may take.
const RUN_SPEED: f32 = 14.0;
const RUN_TIME: (f32, f32) = (2.0, 5.0);
const WAKE: f32 = 2.5;
/// At the first wake the cart rolls on through the opened gate before the camera goes home.
const WAKE_FIRST: f32 = 4.6;
/// How far into its wake a machine's console lights.
const CONSOLE_LIGHTS: f32 = 0.45;
/// How long the camera takes to glide back to the player's view.
const HOME: f32 = 1.2;
/// How long the letterbox bars take to come in, and to go.
const BARS: f32 = 0.6;
/// How long a pass's result may be read at its console before the wake starts anyway; stepping away
/// from the console starts it at once.
const READ_RESULT: f32 = 8.0;
/// How far into its wake the stone gate's leaf is down far enough for the cart to roll through.
const GATE_CLEAR: f32 = 0.35;

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// When everything happens in the wake to one stage, in seconds from the moment it starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timeline {
    pub stage: usize,
    run_length: f32,
    glow: f32,
    run: f32,
    wake: f32,
    home: f32,
}

impl Timeline {
    pub fn new(stage: usize, way: &Way) -> Self {
        assert!((1..=PUZZLES).contains(&stage), "there is no wake to stage {stage}");
        let run_length = way.runs[stage - 1].length();
        Timeline {
            stage,
            run_length,
            glow: if sight_lifted(stage) && !sight_lifted(stage - 1) {
                GLOW_LIFT
            } else if way.stonework().into_iter().any(|work| work.with_glow && work.stage == stage) {
                GLOW_MOVE
            } else {
                GLOW
            },
            run: (run_length / RUN_SPEED).clamp(RUN_TIME.0, RUN_TIME.1),
            wake: if stage == 1 { WAKE_FIRST } else { WAKE },
            home: HOME,
        }
    }

    pub fn length(&self) -> f32 {
        self.glow + self.run + self.wake + self.home
    }

    /// When the machine starts to wake, once the light has run.
    pub fn wake_starts(&self) -> f32 {
        self.glow + self.run
    }

    /// When the waking machine's console lights.
    pub fn console_lights(&self) -> f32 {
        self.wake_starts() + CONSOLE_LIGHTS * self.wake
    }

    /// When the machine has woken and the camera starts home.
    fn home_starts(&self) -> f32 {
        self.glow + self.run + self.wake
    }

    /// The phase at `t`, and how far through it (0 to 1).
    pub fn phase(&self, t: f32) -> (Phase, f32) {
        let mut start = 0.0;
        for (phase, length) in [(Phase::Glow, self.glow), (Phase::Run, self.run), (Phase::Wake, self.wake), (Phase::Home, self.home)] {
            if t < start + length {
                return (phase, ((t - start) / length).clamp(0.0, 1.0));
            }
            start += length;
        }
        (Phase::Done, 1.0)
    }

    /// How far the machine whose job was done has lit, from 0 to 1.
    pub fn glowed(&self, t: f32) -> f32 {
        (t / self.glow).clamp(0.0, 1.0)
    }

    /// How far along its stretch of inlay the light has run, in metres.
    pub fn lit_to(&self, t: f32) -> f32 {
        ease((t - self.glow) / self.run) * self.run_length
    }

    /// How far the waking machine has come, from 0 to 1.
    pub fn woken(&self, t: f32) -> f32 {
        ((t - self.glow - self.run) / self.wake).clamp(0.0, 1.0)
    }

    /// Whether the machine has woken and the world shows its stage: from the moment the camera
    /// starts home.
    pub fn woke(&self, t: f32) -> bool {
        t >= self.home_starts()
    }

    /// How far the letterbox bars are in, from 0 to 1.
    pub fn bars(&self, t: f32) -> f32 {
        (t / BARS).min((self.length() - t) / BARS).clamp(0.0, 1.0)
    }
}

/// A wake as it plays.
#[derive(Clone, Copy, Debug)]
pub struct Play {
    pub timeline: Timeline,
    pub t: f32,
}

/// What the world shows of the way.
#[derive(Resource, Debug)]
pub struct Waking {
    /// The stage the world shows: up to the machine that is waking, until it has woken.
    pub shown: usize,
    /// The wake playing now, if one is.
    pub play: Option<Play>,
    /// Seconds a wake has been due while the player read the result at the console.
    waited: f32,
    /// Where the sight was on the crawler when the crane began to lift it.
    lift_from: Option<Vec3>,
}

impl Waking {
    pub fn new(shown: usize) -> Self {
        Waking { shown, play: None, waited: 0.0, lift_from: None }
    }

    pub fn playing(&self) -> bool {
        self.play.is_some()
    }

    /// Whether machine `i`'s console and lamps are awake: once the world shows its stage, or as its
    /// wake reaches them.
    pub fn console_awake(&self, machine: usize) -> bool {
        console_awake(machine, self.shown) || self.play.is_some_and(|p| p.timeline.stage == machine && p.t >= p.timeline.console_lights())
    }

    /// How far stonework has come through its move: 1 once the world shows it moved, and during the
    /// wake that moves it, as far as it has got.
    pub fn moved(&self, work: &Stonework) -> f32 {
        let before = (work.stands(self.shown) != work.stands(0)) as u8 as f32;
        match self.play {
            Some(play) if play.timeline.stage == work.stage && before == 0.0 && work.with_glow => ease(play.timeline.glowed(play.t) / 0.9),
            Some(play) if play.timeline.stage == work.stage && before == 0.0 => {
                let reach = if work.rises {
                    0.8
                } else if work.stage == 1 {
                    GATE_CLEAR
                } else {
                    0.6
                };
                ease(play.timeline.woken(play.t) / reach)
            }
            _ => before,
        }
    }

    /// Whether the stone gate is open, or opening far enough for the cart to roll through it.
    pub fn gate_is_open(&self, way: &Way) -> bool {
        self.moved(&way.leaf) >= 1.0
    }

    #[cfg(test)]
    fn woke_now(&self) -> bool {
        self.play.is_some_and(|p| p.timeline.woke(p.t))
    }

    /// Whether a machine's moving quartz glows: once it has woken, or as the wake reaches it.
    pub fn glows(&self, machine: usize) -> bool {
        if wake_lit(machine, self.shown) {
            return true;
        }
        self.play.is_some_and(|p| machine == p.timeline.stage && !matches!(p.timeline.phase(p.t).0, Phase::Glow | Phase::Run))
    }

    /// Whether the light is on its way up the road.
    pub fn light_running(&self) -> bool {
        self.play.is_some_and(|p| matches!(p.timeline.phase(p.t).0, Phase::Glow | Phase::Run))
    }

    /// Where the light is on the road, while it runs.
    pub fn head(&self, way: &Way) -> Option<Vec2> {
        let play = self.play?;
        match play.timeline.phase(play.t).0 {
            Phase::Glow | Phase::Run => Some(way.runs[play.timeline.stage - 1].at(play.timeline.lit_to(play.t))),
            _ => None,
        }
    }

    /// How far through lifting the sight off the crawler the crane is, while it does.
    pub fn lift(&self) -> Option<f32> {
        let play = self.play?;
        let lifting = sight_lifted(play.timeline.stage) && !sight_lifted(play.timeline.stage - 1);
        (lifting && play.timeline.phase(play.t).0 == Phase::Glow).then(|| play.timeline.glowed(play.t))
    }

    /// Where the sight is: on the crawler (`None`), or where it rests off it.
    pub fn sight(&self, sites: &Sites) -> Option<Vec3> {
        if let (Some(u), Some(from)) = (self.lift(), self.lift_from) {
            return Some(lift_pose(u, from, sites.way.crane_pivot, rest_on(sites.crane_foot)).1);
        }
        let past_lift = self.play.is_some_and(|p| sight_lifted(p.timeline.stage) && self.lift().is_none());
        if sight_fitted(self.shown) {
            Some(rest_on(crate::vessel::fitted_at(sites)))
        } else if sight_lifted(self.shown) || past_lift {
            Some(rest_on(sites.crane_foot))
        } else {
            None
        }
    }

    /// How far the north gate's beam reaches, from 0 to 1.
    fn beam(&self) -> f32 {
        match self.play {
            Some(play) if beam_shines(play.timeline.stage) && !beam_shines(self.shown) => ease(play.timeline.woken(play.t) / 0.85),
            _ => beam_shines(self.shown) as u8 as f32,
        }
    }

    /// The crane's arm: how far it has turned from over the track towards the sight's resting place.
    fn crane_turn(&self) -> f32 {
        match (self.lift(), self.lift_from) {
            (Some(u), Some(from)) => lift_pose(u, from, Vec3::ZERO, Vec3::NEG_Z).0,
            _ if self.sight_is_off() => std::f32::consts::PI,
            _ => 0.0,
        }
    }

    fn sight_is_off(&self) -> bool {
        sight_lifted(self.shown) || self.play.is_some_and(|p| sight_lifted(p.timeline.stage) && self.lift().is_none())
    }
}

/// A sight's centre resting on the ground at `foot`.
fn rest_on(foot: Vec3) -> Vec3 {
    foot + Vec3::Y * SIGHT_SIZE.y / 2.0
}

/// How far below the crane's arm the sight hangs on its hook.
const HANG: f32 = 1.0;

/// The crane lifting the sight, `u` of the way through (0 to 1): how far its arm has turned (0 over
/// the track, π over the sight's resting place, behind the mast), and where the sight is. It rises
/// from the crawler to the hook, the arm swings round with it, and it is let down to `to`.
pub fn lift_pose(u: f32, from: Vec3, pivot: Vec3, to: Vec3) -> (f32, Vec3) {
    use std::f32::consts::PI;
    let reach_from = (from - pivot).with_y(0.0).length().max(0.5);
    let reach_to = (to - pivot).with_y(0.0).length().max(0.5);
    let hook = |turn: f32, reach: f32| pivot + Quat::from_rotation_y(turn) * Vec3::Z * reach - Vec3::Y * HANG;
    if u < 0.3 {
        let k = ease(u / 0.3);
        (0.0, from.lerp(hook(0.0, reach_from), k))
    } else if u < 0.7 {
        let k = ease((u - 0.3) / 0.4);
        (PI * k, hook(PI * k, reach_from + (reach_to - reach_from) * k))
    } else {
        let k = ease((u - 0.7) / 0.3);
        (PI, hook(PI, reach_to).lerp(to, k))
    }
}

/// The way's parts that move rather than light or stand as blocks: the north gate's beam, which
/// shines across the strait, the transfer crane's arm and the line of its hook, and the bridge's
/// cables.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
enum Part {
    NorthBeam,
    CraneArm,
    HookLine,
    /// A segment of one of the bridge's cables.
    Cable {
        cable: usize,
        segment: usize,
    },
}

/// How thick the bridge's cables are.
const CABLE_RADIUS: f32 = 0.12;

/// How far the north gate's beam reaches towards the main hull.
const BEAM_LENGTH: f32 = 260.0;
/// The crane's arm: how far it reaches from the mast's middle, and how far behind it.
const ARM_REACH: f32 = 2.3;
const ARM_TAIL: f32 = 0.3;

/// The builders' pale stone, for the parts of their machines that are not blocks.
pub fn stone(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    let c = Material::Limestone.properties().colour;
    materials.add(StandardMaterial { base_color: Color::srgb(c[0], c[1], c[2]), perceptual_roughness: 0.85, ..default() })
}

/// The quartz set into each machine's moving parts (the carts, the launchers): one material per
/// machine, dark while it sleeps and lit once it wakes.
#[derive(Resource, Clone)]
pub struct MachineTrim(pub [Handle<StandardMaterial>; MACHINES]);

impl MachineTrim {
    pub fn look(lit: bool) -> StandardMaterial {
        Self::look_at(lit as u8 as f32)
    }

    /// Quartz part of the way from dark (0) to fully lit (1), as it fades in when its machine wakes.
    pub fn look_at(lit: f32) -> StandardMaterial {
        let dark = Color::srgb(DARK_QUARTZ[0], DARK_QUARTZ[1], DARK_QUARTZ[2]);
        if lit <= 0.0 {
            return StandardMaterial { base_color: dark, perceptual_roughness: 0.5, ..default() };
        }
        let q = Material::Quartz.properties().colour;
        let glowing = crate::graphics::glowing(q, crate::graphics::glow::WAY).to_linear();
        let colour = dark.to_linear().mix(&glowing, ease(lit));
        StandardMaterial { base_color: Color::LinearRgba(colour), unlit: true, ..default() }
    }
}

/// How long a machine's quartz takes to fade in as it wakes, in seconds.
pub const FADE_IN: f32 = 0.7;

/// Move each machine's quartz a step towards lit or dark, fading in as it wakes and going dark at
/// once (only a new game puts it out). Returns the machines whose look changed. The first call
/// shows the world as it is, without fading.
pub fn fade_towards(shown: &mut Option<[f32; MACHINES]>, lit: [bool; MACHINES], dt: f32) -> Vec<(usize, f32)> {
    let mut changed = Vec::new();
    let first = shown.is_none();
    let now = shown.get_or_insert([0.0; MACHINES]);
    for machine in 0..MACHINES {
        let target = lit[machine] as u8 as f32;
        let next = if first || target < now[machine] { target } else { (now[machine] + dt / FADE_IN).min(target) };
        if first || next != now[machine] {
            now[machine] = next;
            changed.push((machine, next));
        }
    }
    changed
}

/// Quartz on a moving part that is always lit (the crawler, which never slept).
pub fn lit_quartz(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(MachineTrim::look(true))
}

/// The place Lumen's eye follows while the light runs (a landmark that moves).
pub const LIGHT_ON_THE_ROAD: &str = "The light on the road";

pub struct WakingPlugin;

impl Plugin for WakingPlugin {
    fn build(&self, app: &mut App) {
        let shown = stage(app.world().resource::<Game>());
        let trim = {
            let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
            MachineTrim(std::array::from_fn(|machine| materials.add(MachineTrim::look(wake_lit(machine, shown)))))
        };
        app.insert_resource(Waking::new(shown))
            .insert_resource(trim)
            .add_systems(Startup, spawn)
            .add_systems(Update, (play.run_if(in_state(Mode::Playing)), pose_parts, light_trim).chain())
            .add_systems(EguiPrimaryContextPass, letterbox.after(crate::ui::style).run_if(in_state(Mode::Playing)));
    }
}

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sites: Res<Sites>) {
    let way = &sites.way;
    let stone = stone(&mut materials);
    let quartz = Material::Quartz.properties().colour;
    let light = materials.add(StandardMaterial {
        base_color: crate::graphics::glowing(quartz, crate::graphics::glow::WAY).with_alpha(0.55),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..default()
    });
    // The north gate's beam, from its keystone out over the strait towards the main hull.
    let towards = (crate::intro::Mainland::wreck() - way.keystone).with_y(0.0).normalize() + Vec3::Y * 0.08;
    let rotation = Quat::from_rotation_arc(Vec3::Y, towards.normalize());
    commands.spawn((
        Part::NorthBeam,
        Mesh3d(meshes.add(Cylinder::new(0.35, BEAM_LENGTH))),
        MeshMaterial3d(light),
        Transform::from_translation(way.keystone + towards.normalize() * BEAM_LENGTH / 2.0).with_rotation(rotation),
        bevy::light::NotShadowCaster,
    ));
    // The transfer crane's arm: a stone boom on the mast's top, which turns about the mast.
    let arm = Mesh::from(Cuboid::new(0.3, 0.3, ARM_REACH + ARM_TAIL)).translated_by(Vec3::Z * (ARM_REACH - ARM_TAIL) / 2.0);
    commands.spawn((Part::CraneArm, Mesh3d(meshes.add(arm)), MeshMaterial3d(stone.clone()), Transform::from_translation(way.crane_pivot)));
    // The line from the arm down to the sight while it hangs there.
    commands.spawn((Part::HookLine, Mesh3d(meshes.add(Cylinder::new(0.02, 1.0))), MeshMaterial3d(stone), Transform::default(), Visibility::Hidden));
    // The bridge's cables, a chain of glowing segments each, which come with the deck.
    let glowing = lit_quartz(&mut materials);
    let segment = meshes.add(Cylinder::new(CABLE_RADIUS, 1.0));
    for (c, cable) in way.cables.iter().enumerate() {
        for (i, ends) in cable.points.windows(2).enumerate() {
            commands.spawn((
                Part::Cable { cable: c, segment: i },
                Mesh3d(segment.clone()),
                MeshMaterial3d(glowing.clone()),
                segment_between(ends[0], ends[1]),
                Visibility::Hidden,
                bevy::light::NotShadowCaster,
            ));
        }
    }
}

/// A unit cylinder stretched from `a` to `b`, overlapping a little at each end so a chain of them
/// reads as one line.
fn segment_between(a: Vec3, b: Vec3) -> Transform {
    let along = b - a;
    Transform::from_translation((a + b) / 2.0).with_rotation(Quat::from_rotation_arc(Vec3::Y, along.normalize())).with_scale(Vec3::new(
        1.0,
        along.length() + CABLE_RADIUS,
        1.0,
    ))
}

/// Whether a segment of one of the bridge's cables is there: once the bridge stands far enough.
fn cable_shown(cable: &Cable, segment: usize, bridge: &Stonework, moved: f32) -> bool {
    moved > 0.0 && cable.steps[segment] < standing_steps(bridge, moved)
}

/// Where the camera looks while a machine wakes: what moves there.
fn wake_focus(stage: usize, sites: &Sites) -> Vec3 {
    let way = &sites.way;
    match stage {
        // Over the gate's lintel, so the camera is not drawn in between its posts.
        1 => way.stone_gate + Vec3::Y * 4.5,
        3 => way.ramp.centre(),
        5 => way.keystone,
        k => way.machines[k] + Vec3::Y * 1.5,
    }
}

/// Play the wake to the save's stage, one stage at a time, once a pass has been watched.
#[allow(clippy::too_many_arguments)]
fn play(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    game: Res<Game>,
    sim: Res<Sim>,
    sites: Res<Sites>,
    sights: Query<(&Sight, &GlobalTransform)>,
    mut waking: ResMut<Waking>,
    mut world: ResMut<VoxelWorld>,
    (mut dirty, mut reshaped): (ResMut<DirtyCells>, ResMut<TerrainReshaped>),
    (mut orbit, mut ui, mut landmarks): (ResMut<OrbitCamera>, ResMut<Ui>, ResMut<Landmarks>),
) {
    let way = &sites.way;
    let saved = stage(&game);
    if waking.play.is_none() {
        if saved < waking.shown {
            // The save went back (a new game): no wake for that.
            let changes = settle(&mut world, way, saved);
            reshaped.0 |= changes.reshaped;
            dirty.0.extend(changes.cells);
            waking.shown = saved;
        }
        if saved <= waking.shown || sim.busy() || sim.watching() {
            if waking.waited != 0.0 {
                waking.waited = 0.0;
            }
            return;
        }
        // The result can be read first: the wake starts once the player steps away from the console,
        // or after a while.
        waking.waited += time.delta_secs();
        if ui.station.is_some() && waking.waited < READ_RESULT {
            return;
        }
        waking.waited = 0.0;
        let timeline = Timeline::new(waking.shown + 1, way);
        waking.play = Some(Play { timeline, t: 0.0 });
        // The crane lifts the sight from wherever the crawler left it.
        waking.lift_from = sights.iter().find(|(sight, _)| sight.riding).map(|(_, at)| at.translation());
        // The panel steps aside and the camera goes to watch. Nothing more this frame: the key that
        // closed the console (E or Esc) must not also skip the wake it starts.
        ui.station = None;
        orbit.borrow(0.55, 28.0);
        return;
    }
    let Some(mut now) = waking.play else { return };
    // Typing in a panel (the chat with Lumen) is not a skip.
    let skip = !wants.wants_any_keyboard_input() && (keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyE));
    now.t = if skip { now.timeline.length() } else { now.t + time.delta_secs().min(0.25) };
    let (timeline, t, k) = (now.timeline, now.t, now.timeline.stage);
    let (phase, _) = timeline.phase(t);
    // What follows reads the wake as it is this frame.
    waking.play = Some(now);

    let mut changes = Changes::default();
    // The machine whose job was done lights from the bottom up.
    let done = &way.done[k - 1];
    let glowed = (timeline.glowed(t) * done.len() as f32).ceil() as usize;
    light(&mut world, done.iter().take(glowed).copied(), true, &mut changes);
    let reach = timeline.lit_to(t);
    light(&mut world, way.runs[k - 1].cells.iter().filter(|(_, s)| *s <= reach).map(|&(c, _)| c), true, &mut changes);
    let waking_now = !matches!(phase, Phase::Glow | Phase::Run);
    if waking_now {
        light(&mut world, way.wake[k].iter().copied(), true, &mut changes);
    }
    for work in way.stonework() {
        if work.stage == k && (waking_now || work.with_glow) {
            shape(&mut world, work, waking.moved(work), &mut changes);
        }
    }
    dirty.0.extend(changes.cells);

    let surface = |p: Vec2| Vec3::new(p.x, world.surface(p.x.floor() as i32, p.y.floor() as i32) as f32 + 0.6, p.y);
    let head = waking.head(way).map(surface);
    if let Some(head) = head {
        landmarks.set(LIGHT_ON_THE_ROAD, head);
    }
    let building = way.stonework().into_iter().find(|work| work.with_glow && work.stage == k);
    orbit.watch = match phase {
        Phase::Glow if waking.lift().is_some() => Some(way.crane_pivot - Vec3::Y * 1.5),
        Phase::Glow if building.is_some() => building.map(Stonework::centre),
        Phase::Glow | Phase::Run => head,
        Phase::Wake => Some(wake_focus(k, &sites)),
        Phase::Home | Phase::Done => orbit.watch,
    };
    if timeline.woke(t) && waking.shown < k {
        // Woken (or skipped): the world shows the stage, and the camera glides back to the player.
        let changes = settle(&mut world, way, k);
        reshaped.0 |= changes.reshaped;
        dirty.0.extend(changes.cells);
        waking.shown = k;
        if skip { orbit.release_view() } else { orbit.ease_home() }
    }
    if phase == Phase::Done {
        waking.play = None;
        waking.lift_from = None;
    }
}

/// The letterbox while a wake plays.
fn letterbox(mut contexts: EguiContexts, waking: Res<Waking>) -> Result {
    let Some(play) = waking.play else { return Ok(()) };
    let ctx = contexts.ctx_mut()?.clone();
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("waking")));
    crate::cinematic::paint_letterbox(&painter, screen, play.timeline.bars(play.t), None, None, 0.0);
    Ok(())
}

/// Light the quartz on each machine's moving parts as the machine wakes.
fn light_trim(
    time: Res<Time>,
    waking: Res<Waking>,
    trim: Res<MachineTrim>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shown: Local<Option<[f32; MACHINES]>>,
) {
    let lit: [bool; MACHINES] = std::array::from_fn(|machine| waking.glows(machine));
    for (machine, f) in fade_towards(&mut shown, lit, time.delta_secs()) {
        if let Some(mut material) = materials.get_mut(&trim.0[machine]) {
            *material = MachineTrim::look_at(f);
        }
    }
}

/// Put the moving parts where the way has them: the beam reaching out, the crane's arm turned, its
/// hook's line down to the sight while it hangs there, the bridge's cables as far as its deck.
fn pose_parts(waking: Res<Waking>, sites: Res<Sites>, mut parts: Query<(&Part, &mut Transform, &mut Visibility)>) {
    if !waking.is_changed() {
        return;
    }
    let way = &sites.way;
    let turn = waking.crane_turn();
    let hanging = waking.lift().and(waking.sight(&sites));
    let bridge = waking.moved(&way.bridge);
    for (part, mut transform, mut visibility) in &mut parts {
        let seen = match part {
            Part::NorthBeam => {
                let open = waking.beam();
                let along = transform.rotation * Vec3::Y;
                transform.scale = Vec3::new(1.0, open.max(0.001), 1.0);
                transform.translation = way.keystone + along * BEAM_LENGTH * open / 2.0;
                open > 0.0
            }
            Part::CraneArm => {
                transform.rotation = Quat::from_rotation_y(turn);
                true
            }
            Part::HookLine => match hanging {
                Some(sight) => {
                    let top = sight + Vec3::Y * SIGHT_SIZE.y / 2.0;
                    let hook = top.with_y(way.crane_pivot.y);
                    let length = (hook.y - top.y).max(0.01);
                    *transform = Transform::from_translation((hook + top) / 2.0).with_scale(Vec3::new(1.0, length, 1.0));
                    true
                }
                None => false,
            },
            Part::Cable { cable, segment } => cable_shown(&way.cables[*cable], *segment, &way.bridge, bridge),
        };
        visibility.set_if_neq(if seen { Visibility::Inherited } else { Visibility::Hidden });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::island::{self, GROUND};
    use physics_core::puzzle::Ch1Layout;
    use std::collections::{HashSet, VecDeque};

    #[test]
    fn quartz_fades_in_as_its_machine_wakes_and_shows_a_loaded_game_at_once() {
        let mut shown = None;
        let mut lit = [false; MACHINES];
        lit[0] = true;
        // A game loaded: what is lit shows lit, from the first frame.
        let first = fade_towards(&mut shown, lit, 0.016);
        assert_eq!(first.len(), MACHINES);
        assert_eq!(shown.unwrap()[0], 1.0);
        // Machine 1 wakes: it fades in over FADE_IN, and nothing else changes.
        lit[1] = true;
        let step = fade_towards(&mut shown, lit, FADE_IN / 2.0);
        assert_eq!(step, vec![(1, 0.5)]);
        fade_towards(&mut shown, lit, FADE_IN);
        assert_eq!(shown.unwrap()[1], 1.0);
        assert!(fade_towards(&mut shown, lit, 0.016).is_empty(), "nothing left to change");
        // A new game puts it out at once.
        let dark = fade_towards(&mut shown, [false; MACHINES], 0.016);
        assert!(dark.iter().all(|&(_, f)| f == 0.0) && dark.len() == 2);
    }

    #[test]
    fn the_rules_at_each_stage() {
        // Stage 0: everything asleep, only the first console awake.
        assert!((0..MACHINES).all(|m| !wake_lit(m, 0) && !done_lit(m, 0)));
        assert!(console_awake(0, 0) && !console_awake(1, 0));
        // The first pass: the supply rail's job is done; the light runs to the haul track, which wakes.
        assert!(done_lit(0, 1) && wake_lit(1, 1) && !done_lit(1, 1) && !wake_lit(2, 1));
        assert!(run_lit(0, 1) && !run_lit(1, 1));
        assert!(!sight_lifted(1) && sight_lifted(2) && !sight_fitted(2) && sight_fitted(3));
        assert!(!beam_shines(4) && beam_shines(5));
        // At the end all of it is lit.
        assert!((1..MACHINES).all(|m| wake_lit(m, 5)));
        assert!((0..MACHINES - 1).all(|m| done_lit(m, 5)));
        assert!((0..MACHINES - 1).all(|r| run_lit(r, 5)));
    }

    /// Each row of the design's table: what a machine shows when the light reaches it, and what it
    /// shows only once its own job is done.
    #[test]
    fn a_machine_lights_up_when_its_own_job_is_done() {
        for seed in 0..12 {
            let (built, sites) = island::build(&Ch1Layout::generate(seed));
            let way = &sites.way;
            let mut world = built.clone();
            let dark = |world: &VoxelWorld, cells: &[IVec3]| cells.iter().all(|&c| world.is_dark(c));
            let lit = |world: &VoxelWorld, cells: &[IVec3]| cells.iter().all(|&c| !world.is_dark(c));
            for machine in 0..PUZZLES {
                assert!(!way.done[machine].is_empty(), "seed {seed}: machine {machine} has nothing to light when its job is done");
                // Woken, its job not yet done: its own quartz is still dark.
                settle(&mut world, way, machine);
                assert!(dark(&world, &way.done[machine]), "seed {seed}: machine {machine} lit before its job was done");
                assert!(lit(&world, &way.wake[machine]), "seed {seed}: machine {machine} woken but dark");
                settle(&mut world, way, machine + 1);
                assert!(lit(&world, &way.done[machine]), "seed {seed}: machine {machine} done but dark");
            }
            // The stone gate opens and the haul track wakes at the first pass; the tower's foot lights
            // when the light reaches it, the rest of it at its own pass.
            assert_eq!(way.leaf.stage, 1);
            assert_eq!(way.wake[2].len(), 2, "seed {seed}: the tower's and the mast's feet");
            assert!(way.done[2].iter().all(|c| c.y > way.wake[2][0].y), "seed {seed}: the tower does not light to the top");
            assert!(way.done[2].windows(2).all(|w| w[0].y <= w[1].y), "seed {seed}: the tower lights out of order");
            assert!(!way.wake[MACHINES - 1].is_empty() && way.done[MACHINES - 1].is_empty());
        }
    }

    #[test]
    fn a_wake_runs_in_order_and_its_console_lights_in_plain_view() {
        for seed in 0..20 {
            let (_, sites) = island::build(&Ch1Layout::generate(seed));
            let way = &sites.way;
            for stage in 1..=PUZZLES {
                let timeline = Timeline::new(stage, way);
                let length = timeline.length();
                assert!(length > 3.0 && length < 18.0, "seed {seed}: the wake to stage {stage} takes {length} s");
                // The phases come in order, and each is reached.
                let mut seen = Vec::new();
                let mut t = 0.0;
                while t <= length + 0.1 {
                    let (phase, _) = timeline.phase(t);
                    if seen.last() != Some(&phase) {
                        seen.push(phase);
                    }
                    t += 0.05;
                }
                assert_eq!(seen, vec![Phase::Glow, Phase::Run, Phase::Wake, Phase::Home, Phase::Done], "seed {seed}: stage {stage}");
                // The light starts at the machine whose job was done and ends at the one it wakes.
                let run = &way.runs[stage - 1];
                let (from, to) = (run.at(timeline.lit_to(0.0)), run.at(timeline.lit_to(length)));
                assert!(from.distance(way.machines[stage - 1].xz()) < 6.0, "seed {seed}: stage {stage}");
                assert!(to.distance(way.machines[stage].xz()) < 6.0, "seed {seed}: stage {stage}");
                // It only ever moves on.
                let mut last = 0.0;
                for i in 0..=100 {
                    let lit = timeline.lit_to(length * i as f32 / 100.0);
                    assert!(lit >= last - 1e-4, "seed {seed}: the light ran back");
                    last = lit;
                }
                // The console lights while its machine wakes, under the letterbox, before the camera
                // goes home.
                let lights = timeline.console_lights();
                assert_eq!(timeline.phase(lights).0, Phase::Wake, "seed {seed}: stage {stage}");
                assert!(!timeline.woke(lights) && timeline.woke(length), "seed {seed}: stage {stage}");
                let w = |t: f32| Waking { play: Some(Play { timeline, t }), ..Waking::new(stage - 1) };
                assert!(!w(lights - 0.05).console_awake(stage) && w(lights + 0.05).console_awake(stage), "seed {seed}: stage {stage}");
                assert!(timeline.bars(lights) == 1.0, "seed {seed}: stage {stage}");
                assert!(timeline.bars(0.0) == 0.0 && timeline.bars(length / 2.0) == 1.0 && timeline.bars(length) == 0.0);
            }
        }
    }

    #[test]
    fn the_cart_rolls_through_the_gate_once_its_leaf_is_down() {
        let (_, sites) = island::build(&Ch1Layout::generate(3));
        let timeline = Timeline::new(1, &sites.way);
        let at = |t: f32| Waking { play: Some(Play { timeline, t }), ..Waking::new(0) };
        assert!(!at(0.0).gate_is_open(&sites.way));
        let open = (0..400).map(|i| i as f32 * 0.05).find(|&t| at(t).gate_is_open(&sites.way)).expect("the gate opens");
        // The leaf's blocks are gone by then.
        let mut world = island::build(&Ch1Layout::generate(3)).0;
        settle(&mut world, &sites.way, 0);
        assert!(sites.way.leaf.cells.iter().all(|&(c, _)| world.is_solid(c)), "the leaf stands while the gate sleeps");
        shape(&mut world, &sites.way.leaf, at(open).moved(&sites.way.leaf), &mut Changes::default());
        assert!(sites.way.leaf.cells.iter().all(|&(c, _)| !world.is_solid(c)), "the leaf is down when the cart may pass");
        assert!(!at(open).woke_now(), "the cart rolls before the camera goes home");
    }

    #[test]
    fn the_crane_lifts_the_sight_off_the_crawler_and_sets_it_down() {
        let (_, sites) = island::build(&Ch1Layout::generate(5));
        let pivot = sites.way.crane_pivot;
        let to = rest_on(sites.crane_foot);
        let from = Vec3::new(pivot.x + 0.3, GROUND as f32 + 1.1, pivot.z + 2.0);
        let (turn, at) = lift_pose(0.0, from, pivot, to);
        assert_eq!(turn, 0.0);
        assert!(at.distance(from) < 1e-4, "it starts on the crawler");
        let (turn, at) = lift_pose(1.0, from, pivot, to);
        assert!((turn - std::f32::consts::PI).abs() < 1e-4);
        assert!(at.distance(to) < 1e-4, "it ends at the crane's foot");
        // On the way it hangs under the arm, clear of the ground and of the mast.
        let mut last = from;
        for i in 1..=100 {
            let (_, at) = lift_pose(i as f32 / 100.0, from, pivot, to);
            assert!(at.y >= to.y - 1e-4, "it goes below the ground");
            assert!(at.distance(last) < 0.25, "it jumps");
            last = at;
        }
        // In the game: on the crawler before the crawler's pass, off it after.
        let w = Waking::new(1);
        assert!(w.sight(&sites).is_none());
        let w = Waking::new(2);
        assert!(w.sight(&sites).is_some_and(|at| at.distance(to) < 1e-4));
        let w = Waking::new(3);
        assert!(w.sight(&sites).is_some_and(|at| at.distance(rest_on(crate::vessel::fitted_at(&sites))) < 1e-4));
    }

    /// Every cell of the world, and which are dark.
    fn snapshot(world: &VoxelWorld) -> (Vec<Material>, Vec<IVec3>) {
        let mut cells = Vec::new();
        let mut dark = Vec::new();
        for y in 0..world.size.y {
            for z in 0..world.size.z {
                for x in 0..world.size.x {
                    let p = IVec3::new(x, y, z);
                    cells.push(world.get(p));
                    if world.is_dark(p) {
                        dark.push(p);
                    }
                }
            }
        }
        (cells, dark)
    }

    #[test]
    fn put_to_sleep_and_woken_stage_by_stage_it_is_the_island_as_built() {
        for seed in 0..12 {
            let (built, sites) = island::build(&Ch1Layout::generate(seed));
            let mut world = built.clone();
            assert!(!settle(&mut world, &sites.way, 0).cells.is_empty(), "seed {seed}: nothing sleeps");
            for &(cell, _) in &sites.way.ramp.cells {
                assert_eq!(world.get(cell), Material::Air, "seed {seed}: the ramp stands asleep");
            }
            for work in [&sites.way.leaf, &sites.way.seal] {
                assert!(work.cells.iter().all(|&(c, m)| world.get(c) == m), "seed {seed}: the gate or the seal is open asleep");
            }
            for stage in 1..=PUZZLES {
                settle(&mut world, &sites.way, stage);
                // Settling again changes nothing.
                assert!(settle(&mut world, &sites.way, stage).cells.is_empty(), "seed {seed}");
            }
            assert!(snapshot(&world) == snapshot(&built), "seed {seed}: awake, the island is not as built");
        }
    }

    /// Where the avatar can stand, starting from `from`: a block with two clear above it, reached
    /// from a neighbouring column by climbing at most `climb` blocks or dropping any distance. It
    /// wades the shallows, and goes no further where the water gets too deep: the avatar falls in
    /// there and comes back where it stepped off.
    fn reachable(world: &VoxelWorld, from: IVec3, climb: i32) -> HashSet<IVec3> {
        let stands = |p: IVec3| world.is_solid(p - IVec3::Y) && !world.is_solid(p) && !world.is_solid(p + IVec3::Y);
        let deep = |p: IVec3| island::too_deep(world.surface(p.x, p.z));
        let mut seen = HashSet::from([from]);
        let mut queue = VecDeque::from([from]);
        while let Some(p) = queue.pop_front() {
            for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                let n = p + d;
                if deep(n) && !deep(p) {
                    continue;
                }
                let mut next = None;
                // Up: clear overhead from where it stands to where it lands.
                for up in (1..=climb).rev() {
                    let q = n + IVec3::Y * up;
                    if stands(q) && (1..=up + 1).all(|h| !world.is_solid(p + IVec3::Y * (h + 1))) {
                        next = Some(q);
                        break;
                    }
                }
                // Level, or walking off an edge and falling.
                if next.is_none() && !world.is_solid(n) && !world.is_solid(n + IVec3::Y) {
                    let mut q = n;
                    while q.y > 0 && !world.is_solid(q - IVec3::Y) {
                        q -= IVec3::Y;
                    }
                    if stands(q) {
                        next = Some(q);
                    }
                }
                if let Some(q) = next
                    && seen.insert(q)
                {
                    queue.push_back(q);
                }
            }
        }
        seen
    }

    /// The highest a jump lifts the avatar's feet on this planet, and so the highest block it can
    /// get onto.
    fn climb(layout: &Ch1Layout) -> i32 {
        let apex = crate::player::JUMP_SPEED.powi(2) / (2.0 * layout.g as f32);
        (apex.floor() as i32).max(1)
    }

    #[test]
    fn the_beam_and_its_pile_can_be_reached_from_the_start() {
        // A side-track open from the first moment: nothing of the way needs to wake for it.
        for seed in 0..12 {
            let layout = Ch1Layout::generate(seed);
            let (mut world, sites) = island::build(&layout);
            settle(&mut world, &sites.way, 0);
            let reach = reachable(&world, sites.spawn.floor().as_ivec3(), climb(&layout));
            let beam = &sites.beam;
            let beside = |x: f32, dz: i32| IVec3::new(x.floor() as i32, GROUND, beam.row() + dz);
            assert!(reach.contains(&beside(beam.pivot.x + 3.0, 2)) && reach.contains(&beside(beam.pivot.x - 3.0, -2)), "seed {seed}");
            assert!(reach.contains(&(beam.console + IVec3::Z)), "seed {seed}: its console");
            for cell in &beam.pile {
                assert!([IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z].iter().any(|d| reach.contains(&(cell.with_y(GROUND) + *d))), "seed {seed}: the pile");
            }
        }
    }

    #[test]
    fn the_ledge_is_out_of_reach_until_the_ramp_rises() {
        for seed in 0..24 {
            let layout = Ch1Layout::generate(seed);
            let (mut world, sites) = island::build(&layout);
            let climb = climb(&layout);
            let spawn = sites.spawn.floor().as_ivec3();
            let ledge = sites.channel_well;
            settle(&mut world, &sites.way, 2);
            assert!(!reachable(&world, spawn, climb).contains(&ledge), "seed {seed}: the ledge can be climbed before the ramp rises (climb {climb})");
            settle(&mut world, &sites.way, 3);
            assert!(reachable(&world, spawn, climb).contains(&ledge), "seed {seed}: the ramp does not lead onto the ledge");
            assert_eq!(ledge.y, GROUND + layout.ledge_h as i32);
        }
    }

    /// The channel keeps the castaway off the north island, where the platform and the tilting
    /// launcher are, until the casket is across and the bridge stands.
    #[test]
    fn the_north_island_is_out_of_reach_until_the_bridge_rises() {
        for seed in 0..24 {
            let layout = Ch1Layout::generate(seed);
            let (mut world, sites) = island::build(&layout);
            let climb = climb(&layout);
            let spawn = sites.spawn.floor().as_ivec3();
            let platform = sites.channel_launch.at(layout.platform_centre, 0.0, GROUND as f32).floor().as_ivec3();
            let console = sites.stations[PUZZLES - 1] + IVec3::NEG_X;
            settle(&mut world, &sites.way, 3);
            let before = reachable(&world, spawn, climb);
            assert!(
                !before.contains(&platform) && !before.contains(&console),
                "seed {seed}: the north island can be reached before the bridge stands (climb {climb})"
            );
            settle(&mut world, &sites.way, 4);
            let after = reachable(&world, spawn, climb);
            assert!(after.contains(&platform), "seed {seed}: the bridge does not lead to the platform");
            assert!(after.contains(&console), "seed {seed}: the bridge does not lead to the tilting launcher's console");
        }
    }

    /// The seal stands in the castaway's way: the spine runs from coast to coast, and nothing goes
    /// round it or over it until the tiller is in the landing circle, even with the bridge up.
    #[test]
    fn the_skiff_is_out_of_reach_until_the_seal_sinks() {
        for seed in 0..24 {
            let layout = Ch1Layout::generate(seed);
            let (mut world, sites) = island::build(&layout);
            let climb = climb(&layout);
            let spawn = sites.spawn.floor().as_ivec3();
            let yard = (sites.slipway + Vec3::Y * 0.5).floor().as_ivec3();
            settle(&mut world, &sites.way, 4);
            assert!(!reachable(&world, spawn, climb).contains(&yard), "seed {seed}: the skiff can be reached before the seal sinks (climb {climb})");
            settle(&mut world, &sites.way, 5);
            assert!(reachable(&world, spawn, climb).contains(&yard), "seed {seed}: the pass does not lead to the skiff");
        }
    }
}
