//! The way wakes: the ancient road's machines sleep at the start and wake one by one as the
//! chapter's puzzles pass.
//!
//! Everything follows from one number, the **stage**: how many of the chapter's puzzles are solved
//! (they open in order, so that is also the index of the next one). At stage `k`:
//!
//! - machine `i` (the five stations, then the north gate) is lit when `k ≥ 1` and `i ≤ k`: the one
//!   whose job was done and the one the light ran on to; the first sleeps until its job is done,
//!   though its console works;
//! - the stretch of inlay from machine `i` to the next is lit when `i < k`;
//! - console `i` is wired in when `i ≤ k` (the station is open);
//! - the stone gate is open from stage 1, the ramp onto the outcrop stands from stage 3, the seal at
//!   the ridge is down from stage 4, and the north gate's beam shines at stage 5.
//!
//! The island is built awake (`island::build`); [`settle`] puts it to the stage the save is at
//! before anything is meshed or collides. The world shows the stage in [`Waking::shown`], which
//! trails the save's while a wake plays.

use crate::carry::DirtyCells;
use crate::companion::Landmarks;
use crate::island::{MACHINES, Way};
use crate::player::OrbitCamera;
use crate::sim::Sim;
use crate::state::{Game, Mode, PUZZLES, Ui};
use crate::voxel::VoxelWorld;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use physics_core::materials::Material;

/// The stage the save is at: how many of the chapter's puzzles are solved.
pub fn stage(game: &Game) -> usize {
    game.next_unsolved().unwrap_or(PUZZLES)
}

pub fn machine_lit(machine: usize, stage: usize) -> bool {
    stage >= 1 && machine <= stage
}

pub fn run_lit(run: usize, stage: usize) -> bool {
    run < stage
}

pub fn console_shown(station: usize, stage: usize) -> bool {
    station <= stage
}

pub fn gate_open(stage: usize) -> bool {
    stage >= 1
}

pub fn ramp_up(stage: usize) -> bool {
    stage >= 3
}

pub fn seal_open(stage: usize) -> bool {
    stage >= 4
}

pub fn beam_shines(stage: usize) -> bool {
    stage >= 5
}

/// Light or put out blocks of the way's quartz, listing the ones that change.
pub fn light(world: &mut VoxelWorld, cells: impl IntoIterator<Item = IVec3>, lit: bool, changed: &mut Vec<IVec3>) {
    for cell in cells {
        if world.is_dark(cell) == lit {
            world.set_dark(cell, !lit);
            changed.push(cell);
        }
    }
}

/// Raise the ramp's blocks up to `height` blocks above the ground (all of it for `i32::MAX`), and
/// take away the rest, listing the cells that change.
pub fn raise_ramp(world: &mut VoxelWorld, way: &Way, height: i32, changed: &mut Vec<IVec3>) {
    for &(cell, material) in &way.ramp {
        let want = if cell.y - crate::island::GROUND < height { material } else { Material::Air };
        if world.get(cell) != want {
            world.set(cell, want);
            changed.push(cell);
        }
    }
}

/// Put the world in the state of a stage at once: the quartz lit or dark, the ramp up or down.
/// Returns the cells that changed, for meshing again.
pub fn settle(world: &mut VoxelWorld, way: &Way, stage: usize) -> Vec<IVec3> {
    let mut changed = Vec::new();
    for machine in 0..MACHINES {
        light(world, way.quartz[machine].iter().copied(), machine_lit(machine, stage), &mut changed);
    }
    for (run, inlay) in way.runs.iter().enumerate() {
        light(world, inlay.cells.iter().map(|&(c, _)| c), run_lit(run, stage), &mut changed);
    }
    raise_ramp(world, way, if ramp_up(stage) { i32::MAX } else { 0 }, &mut changed);
    changed
}

/// The parts of one wake, in order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// The machine whose job was just done lights up.
    Glow,
    /// The light runs up the road to the next machine.
    Run,
    /// The next machine wakes: its quartz lights and its parts move (the gate's leaf sinks, the
    /// ramp rises, the seal sinks, the north gate's beam reaches out).
    Wake,
    /// A fade to dusk while the castaway wires its console in, with a caption.
    Rig,
    Done,
}

const GLOW: f32 = 1.0;
/// How fast the light runs, and the shortest and longest a run may take.
const RUN_SPEED: f32 = 14.0;
const RUN_TIME: (f32, f32) = (2.0, 5.0);
const WAKE: f32 = 2.5;
/// At the first wake the cart rolls on through the opened gate before the fade.
const WAKE_FIRST: f32 = 4.6;
const FADE: f32 = 0.8;
const CAPTION: f32 = 1.8;
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
    run: f32,
    wake: f32,
    rig: f32,
}

impl Timeline {
    pub fn new(stage: usize, way: &Way) -> Self {
        assert!((1..=PUZZLES).contains(&stage), "there is no wake to stage {stage}");
        let run_length = way.runs[stage - 1].length();
        Timeline {
            stage,
            run_length,
            run: (run_length / RUN_SPEED).clamp(RUN_TIME.0, RUN_TIME.1),
            wake: if stage == 1 { WAKE_FIRST } else { WAKE },
            // Past the last machine there is no console to wire in: the chapter's closing follows.
            rig: if stage < PUZZLES { 2.0 * FADE + CAPTION } else { 0.0 },
        }
    }

    pub fn length(&self) -> f32 {
        GLOW + self.run + self.wake + self.rig
    }

    /// When the machine starts to wake, once the light has run.
    pub fn wake_starts(&self) -> f32 {
        GLOW + self.run
    }

    /// When the screen is dark and the caption shows, while the console goes in.
    pub fn rigging(&self) -> f32 {
        self.rig_start() + FADE + CAPTION / 2.0
    }

    fn rig_start(&self) -> f32 {
        GLOW + self.run + self.wake
    }

    /// The phase at `t`, and how far through it (0 to 1).
    pub fn phase(&self, t: f32) -> (Phase, f32) {
        let mut start = 0.0;
        for (phase, length) in [(Phase::Glow, GLOW), (Phase::Run, self.run), (Phase::Wake, self.wake), (Phase::Rig, self.rig)] {
            if t < start + length {
                return (phase, ((t - start) / length).clamp(0.0, 1.0));
            }
            start += length;
        }
        (Phase::Done, 1.0)
    }

    /// How far along its stretch of inlay the light has run, in metres.
    pub fn lit_to(&self, t: f32) -> f32 {
        ease((t - GLOW) / self.run) * self.run_length
    }

    /// How far the waking machine has come, from 0 to 1.
    pub fn woken(&self, t: f32) -> f32 {
        ((t - GLOW - self.run) / self.wake).clamp(0.0, 1.0)
    }

    /// How dark the screen is, from 0 to 1, while the console is wired in.
    pub fn veil(&self, t: f32) -> f32 {
        if self.rig == 0.0 {
            return 0.0;
        }
        let r = t - self.rig_start();
        (r / FADE).min((self.rig - r) / FADE).clamp(0.0, 1.0)
    }

    /// The caption's opacity: it shows while the screen is dark.
    pub fn caption(&self, t: f32) -> f32 {
        if self.rig == 0.0 {
            return 0.0;
        }
        let r = t - self.rig_start() - FADE * 0.6;
        (r / 0.4).min((CAPTION + FADE * 0.8 - r) / 0.4).clamp(0.0, 1.0)
    }

    /// Whether the console is in: from the moment the screen is dark, or the wake's end.
    pub fn rigged(&self, t: f32) -> bool {
        t >= self.rig_start() + FADE.min(self.rig)
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
    /// The stage the world shows: up to the console being wired in.
    pub shown: usize,
    /// The wake playing now, if one is.
    pub play: Option<Play>,
    /// Seconds a wake has been due while the player read the result at the console.
    waited: f32,
}

impl Waking {
    pub fn new(shown: usize) -> Self {
        Waking { shown, play: None, waited: 0.0 }
    }

    pub fn playing(&self) -> bool {
        self.play.is_some()
    }

    pub fn console_shown(&self, station: usize) -> bool {
        console_shown(station, self.shown)
    }

    /// Whether the stone gate is open, or opening far enough for the cart to roll through it.
    pub fn gate_is_open(&self) -> bool {
        gate_open(self.shown) || self.play.is_some_and(|p| gate_open(p.timeline.stage) && p.timeline.woken(p.t) >= GATE_CLEAR)
    }

    #[cfg(test)]
    fn rigged_now(&self) -> bool {
        self.play.is_some_and(|p| p.timeline.rigged(p.t))
    }

    /// Whether a machine's quartz glows: once it is lit, or as the wake reaches it.
    pub fn glows(&self, machine: usize) -> bool {
        if machine_lit(machine, self.shown) {
            return true;
        }
        self.play.is_some_and(|p| {
            let k = p.timeline.stage;
            machine + 1 == k || (machine == k && !matches!(p.timeline.phase(p.t).0, Phase::Glow | Phase::Run))
        })
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

    /// How far open a moving part is, from 0 to 1.
    fn open(&self, part: Part) -> f32 {
        let before = part.open_at(self.shown);
        match self.play {
            Some(play) if part.open_at(play.timeline.stage) > before => {
                let reach = match part {
                    Part::GateLeaf => GATE_CLEAR,
                    Part::RidgeSeal => 0.6,
                    Part::NorthBeam => 0.85,
                };
                ease(play.timeline.woken(play.t) / reach)
            }
            _ => before,
        }
    }
}

/// The way's parts that move rather than light: the stone gate's leaf and the seal at the ridge,
/// which sink into the ground, and the north gate's beam, which shines across the strait.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
enum Part {
    GateLeaf,
    RidgeSeal,
    NorthBeam,
}

impl Part {
    /// How far open, from 0 (shut, dark) to 1 (sunk, shining), at a stage.
    fn open_at(self, stage: usize) -> f32 {
        let open = match self {
            Part::GateLeaf => gate_open(stage),
            Part::RidgeSeal => seal_open(stage),
            Part::NorthBeam => beam_shines(stage),
        };
        open as u8 as f32
    }
}

/// Height of the leaves that sink: the stone gate's fills it to its lintel.
const LEAF_HEIGHT: f32 = 3.0;
const SEAL_HEIGHT: f32 = 2.4;
/// How far the north gate's beam reaches towards the main hull.
const BEAM_LENGTH: f32 = 260.0;

/// The builders' pale stone, for the parts of their machines that are not blocks.
pub fn stone(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    let c = Material::Limestone.properties().colour;
    materials.add(StandardMaterial { base_color: Color::srgb(c[0], c[1], c[2]), perceptual_roughness: 0.85, ..default() })
}

/// The quartz set into each machine's moving parts (the carts, the crawler, the launchers): one
/// material per machine, dark while it sleeps and lit once it wakes.
#[derive(Resource, Clone)]
pub struct MachineTrim(pub [Handle<StandardMaterial>; MACHINES]);

impl MachineTrim {
    fn look(lit: bool) -> StandardMaterial {
        if lit {
            let q = Material::Quartz.properties().colour;
            StandardMaterial { base_color: Color::srgb(q[0], q[1], q[2]), unlit: true, ..default() }
        } else {
            StandardMaterial { base_color: Color::srgb(DARK_TRIM[0], DARK_TRIM[1], DARK_TRIM[2]), perceptual_roughness: 0.5, ..default() }
        }
    }
}

/// Sleeping quartz on a moving part: the colour the voxel world gives its dark quartz.
const DARK_TRIM: [f32; 3] = [0.17, 0.24, 0.27];

/// The place Lumen's eye follows while the light runs (a landmark that moves).
pub const LIGHT_ON_THE_ROAD: &str = "The light on the road";

pub struct WakingPlugin;

impl Plugin for WakingPlugin {
    fn build(&self, app: &mut App) {
        let shown = stage(app.world().resource::<Game>());
        let trim = {
            let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
            MachineTrim(std::array::from_fn(|machine| materials.add(MachineTrim::look(machine_lit(machine, shown)))))
        };
        app.insert_resource(Waking::new(shown))
            .insert_resource(trim)
            .add_systems(Startup, spawn)
            .add_systems(Update, (play.run_if(in_state(Mode::Playing)), pose_parts, light_trim).chain())
            .add_systems(EguiPrimaryContextPass, veil.after(crate::ui::style).run_if(in_state(Mode::Playing)));
    }
}

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sites: Res<crate::island::Sites>) {
    let way = &sites.way;
    let stone = Material::Limestone.properties().colour;
    let stone = materials.add(StandardMaterial { base_color: Color::srgb(stone[0], stone[1], stone[2]), perceptual_roughness: 0.9, ..default() });
    let quartz = Material::Quartz.properties().colour;
    let light = materials.add(StandardMaterial { base_color: Color::srgba(quartz[0], quartz[1], quartz[2], 0.55), unlit: true, alpha_mode: AlphaMode::Add, ..default() });
    // The stone gate's leaf fills the rail between its posts, up to the lintel.
    commands.spawn((Part::GateLeaf, Mesh3d(meshes.add(Cuboid::new(0.5, LEAF_HEIGHT, 1.0))), MeshMaterial3d(stone.clone()), Transform::from_translation(way.stone_gate)));
    // The seal across the road at the ridge's east end: a slab between two standing stones.
    commands.spawn((Part::RidgeSeal, Mesh3d(meshes.add(Cuboid::new(3.2, SEAL_HEIGHT, 0.45))), MeshMaterial3d(stone.clone()), Transform::from_translation(way.ridge_seal)));
    let standing = meshes.add(Cuboid::new(0.8, 3.0, 0.8));
    for side in [-2.2, 2.2] {
        commands.spawn((Mesh3d(standing.clone()), MeshMaterial3d(stone.clone()), Transform::from_translation(way.ridge_seal + Vec3::new(side, 1.5, 0.0))));
    }
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
}

/// Where the camera looks while a machine wakes: what moves there.
fn wake_focus(stage: usize, sites: &crate::island::Sites) -> Vec3 {
    let way = &sites.way;
    match stage {
        // Over the gate's lintel, so the camera is not drawn in between its posts.
        1 => way.stone_gate + Vec3::Y * 4.5,
        3 => way.ramp.iter().map(|(c, _)| c.as_vec3()).sum::<Vec3>() / way.ramp.len().max(1) as f32,
        5 => way.keystone,
        k => way.machines[k] + Vec3::Y * 1.5,
    }
}

/// Play the wake to the save's stage, one stage at a time, once a pass has been watched.
#[allow(clippy::too_many_arguments)]
fn play(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    game: Res<Game>,
    sim: Res<Sim>,
    sites: Res<crate::island::Sites>,
    mut waking: ResMut<Waking>,
    mut world: ResMut<VoxelWorld>,
    mut dirty: ResMut<DirtyCells>,
    (mut orbit, mut ui, mut landmarks): (ResMut<OrbitCamera>, ResMut<Ui>, ResMut<Landmarks>),
) {
    let way = &sites.way;
    let saved = stage(&game);
    if waking.play.is_none() {
        if saved < waking.shown {
            // The save went back (a new game): no wake for that.
            dirty.0.extend(settle(&mut world, way, saved));
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
        waking.play = Some(Play { timeline: Timeline::new(waking.shown + 1, way), t: 0.0 });
        // The panel steps aside and the camera goes to watch.
        ui.station = None;
        orbit.borrow(0.55, 28.0);
    }
    let Some(mut now) = waking.play else { return };
    let skip = keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyE);
    now.t = if skip { now.timeline.length() } else { now.t + time.delta_secs().min(0.25) };
    let (timeline, t, k) = (now.timeline, now.t, now.timeline.stage);
    let (phase, _) = timeline.phase(t);

    let mut changed = Vec::new();
    light(&mut world, way.quartz[k - 1].iter().copied(), true, &mut changed);
    let reach = timeline.lit_to(t);
    light(&mut world, way.runs[k - 1].cells.iter().filter(|(_, s)| *s <= reach).map(|&(c, _)| c), true, &mut changed);
    if !matches!(phase, Phase::Glow | Phase::Run) {
        light(&mut world, way.quartz[k].iter().copied(), true, &mut changed);
        if ramp_up(k) && !ramp_up(k - 1) {
            // The steps come up out of the ground, layer by layer.
            let top = way.ramp.iter().map(|(c, _)| c.y - crate::island::GROUND + 1).max().unwrap_or(0);
            raise_ramp(&mut world, way, (ease(timeline.woken(t) / 0.8) * top as f32).ceil() as i32, &mut changed);
        }
    }
    dirty.0.extend(changed);

    let surface = |p: Vec2| Vec3::new(p.x, world.surface(p.x.floor() as i32, p.y.floor() as i32) as f32 + 0.6, p.y);
    let head = waking.head(way).map(surface);
    if let Some(head) = head {
        landmarks.set(LIGHT_ON_THE_ROAD, head);
    }
    orbit.watch = match phase {
        Phase::Glow | Phase::Run => head,
        Phase::Wake => Some(wake_focus(k, &sites)),
        Phase::Rig | Phase::Done => orbit.watch,
    };
    if timeline.rigged(t) && waking.shown < k {
        // Dark, or done: the console is in, and the player's own view comes back behind the fade.
        dirty.0.extend(settle(&mut world, way, k));
        waking.shown = k;
        orbit.release_view();
    }
    waking.play = (phase != Phase::Done).then_some(now);
}

/// The fade to dusk and the letterbox while a wake plays, with the caption for the console.
fn veil(mut contexts: EguiContexts, waking: Res<Waking>, game: Res<Game>) -> Result {
    let Some(play) = waking.play else { return Ok(()) };
    let ctx = contexts.ctx_mut()?.clone();
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("waking")));
    let (timeline, t) = (play.timeline, play.t);
    crate::cinematic::paint_veil(&painter, screen, timeline.veil(t), [0.03, 0.035, 0.06]);
    let caption = game.chapter.puzzles.get(timeline.stage).map(|def| def.rigged.as_str()).filter(|c| !c.is_empty());
    crate::cinematic::paint_letterbox(&painter, screen, timeline.bars(t), None, caption, timeline.caption(t));
    Ok(())
}

/// Light the quartz on each machine's moving parts as the machine wakes.
fn light_trim(waking: Res<Waking>, trim: Res<MachineTrim>, mut materials: ResMut<Assets<StandardMaterial>>, mut lit: Local<Option<[bool; MACHINES]>>) {
    let now: [bool; MACHINES] = std::array::from_fn(|machine| waking.glows(machine));
    if *lit == Some(now) {
        return;
    }
    for (machine, handle) in trim.0.iter().enumerate() {
        if lit.is_none_or(|before| before[machine] != now[machine])
            && let Some(mut material) = materials.get_mut(handle)
        {
            *material = MachineTrim::look(now[machine]);
        }
    }
    *lit = Some(now);
}

/// Put the moving parts where the way has them: shut, sinking, sunk; the beam reaching out.
fn pose_parts(waking: Res<Waking>, sites: Res<crate::island::Sites>, mut parts: Query<(&Part, &mut Transform, &mut Visibility)>) {
    if !waking.is_changed() {
        return;
    }
    let way = &sites.way;
    for (part, mut transform, mut visibility) in &mut parts {
        let open = waking.open(*part);
        match part {
            Part::GateLeaf => transform.translation = way.stone_gate + Vec3::Y * (LEAF_HEIGHT / 2.0 - (LEAF_HEIGHT + 0.1) * open),
            Part::RidgeSeal => transform.translation = way.ridge_seal + Vec3::Y * (SEAL_HEIGHT / 2.0 - (SEAL_HEIGHT + 0.1) * open),
            Part::NorthBeam => {
                let along = transform.rotation * Vec3::Y;
                transform.scale = Vec3::new(1.0, open.max(0.001), 1.0);
                transform.translation = way.keystone + along * BEAM_LENGTH * open / 2.0;
            }
        }
        // A leaf sunk all the way is out of sight; the beam shows once it shines at all.
        let seen = if *part == Part::NorthBeam { open > 0.0 } else { open < 1.0 };
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
    fn the_rules_at_each_stage() {
        // Stage 0: everything asleep, only the first console in.
        assert!((0..MACHINES).all(|m| !machine_lit(m, 0)));
        assert!(console_shown(0, 0) && !console_shown(1, 0));
        // The first pass lights the first machine, the run from it and the second machine.
        assert!(machine_lit(0, 1) && machine_lit(1, 1) && !machine_lit(2, 1));
        assert!(run_lit(0, 1) && !run_lit(1, 1));
        assert!(gate_open(1) && !gate_open(0));
        assert!(!ramp_up(2) && ramp_up(3));
        assert!(!seal_open(3) && seal_open(4));
        assert!(!beam_shines(4) && beam_shines(5));
        // At the end all of it is lit.
        assert!((0..MACHINES).all(|m| machine_lit(m, 5)));
        assert!((0..MACHINES - 1).all(|r| run_lit(r, 5)));
    }

    #[test]
    fn a_wake_runs_in_order_and_ends_with_the_console_in() {
        for seed in 0..20 {
            let (_, sites) = island::build(&Ch1Layout::generate(seed));
            let way = &sites.way;
            for stage in 1..=PUZZLES {
                let timeline = Timeline::new(stage, way);
                let length = timeline.length();
                assert!(length > 3.0 && length < 16.0, "seed {seed}: the wake to stage {stage} takes {length} s");
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
                let expected: Vec<Phase> = if stage < PUZZLES {
                    vec![Phase::Glow, Phase::Run, Phase::Wake, Phase::Rig, Phase::Done]
                } else {
                    vec![Phase::Glow, Phase::Run, Phase::Wake, Phase::Done]
                };
                assert_eq!(seen, expected, "seed {seed}: stage {stage}");
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
                // The console goes in while the screen is dark; past the last machine, at the end.
                let rigged = (0..=200).map(|i| length * i as f32 / 200.0).find(|&t| timeline.rigged(t)).expect("the console goes in");
                if stage < PUZZLES {
                    assert!(timeline.veil(rigged + 0.01) > 0.95, "seed {seed}: stage {stage}: the console appears in plain view");
                    assert!(timeline.caption(rigged + 0.5) > 0.5, "seed {seed}: no caption");
                } else {
                    assert_eq!(timeline.veil(length * 0.99), 0.0);
                }
                assert!(timeline.bars(0.0) == 0.0 && timeline.bars(length / 2.0) == 1.0 && timeline.bars(length) == 0.0);
            }
        }
    }

    #[test]
    fn the_cart_rolls_through_the_gate_once_its_leaf_is_down() {
        let (_, sites) = island::build(&Ch1Layout::generate(3));
        let timeline = Timeline::new(1, &sites.way);
        let at = |t: f32| Waking { play: Some(Play { timeline, t }), ..Waking::new(0) };
        assert!(!at(0.0).gate_is_open());
        let open = (0..400).map(|i| i as f32 * 0.05).find(|&t| at(t).gate_is_open()).expect("the gate opens");
        assert!((at(open).open(Part::GateLeaf) - 1.0).abs() < 1e-3, "the leaf is down when the cart may pass");
        assert!(!at(open).rigged_now(), "the cart rolls before the fade");
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
            assert!(!settle(&mut world, &sites.way, 0).is_empty(), "seed {seed}: nothing sleeps");
            for &(cell, _) in &sites.way.ramp {
                assert_eq!(world.get(cell), Material::Air, "seed {seed}: the ramp stands asleep");
            }
            for stage in 1..=PUZZLES {
                settle(&mut world, &sites.way, stage);
                // Settling again changes nothing.
                assert!(settle(&mut world, &sites.way, stage).is_empty(), "seed {seed}");
            }
            assert!(snapshot(&world) == snapshot(&built), "seed {seed}: awake, the island is not as built");
        }
    }

    /// Where the avatar can stand, starting from `from`: a block with two clear above it, reached
    /// from a neighbouring column by climbing at most `climb` blocks or dropping any distance.
    fn reachable(world: &VoxelWorld, from: IVec3, climb: i32) -> HashSet<IVec3> {
        let stands = |p: IVec3| world.is_solid(p - IVec3::Y) && !world.is_solid(p) && !world.is_solid(p + IVec3::Y);
        let mut seen = HashSet::from([from]);
        let mut queue = VecDeque::from([from]);
        while let Some(p) = queue.pop_front() {
            for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                let n = p + d;
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

    #[test]
    fn the_ledge_is_out_of_reach_until_the_ramp_rises() {
        for seed in 0..24 {
            let layout = Ch1Layout::generate(seed);
            let (mut world, sites) = island::build(&layout);
            // The highest a jump lifts the avatar's feet, on this planet, and so the highest block
            // it can get onto.
            let apex = crate::player::JUMP_SPEED.powi(2) / (2.0 * layout.g as f32);
            let climb = (apex.floor() as i32).max(1);
            let spawn = sites.spawn.floor().as_ivec3();
            let ledge = sites.canyon_well;
            settle(&mut world, &sites.way, 2);
            assert!(!reachable(&world, spawn, climb).contains(&ledge), "seed {seed}: the ledge can be climbed before the ramp rises (climb {climb})");
            settle(&mut world, &sites.way, 3);
            assert!(reachable(&world, spawn, climb).contains(&ledge), "seed {seed}: the ramp does not lead onto the ledge");
            assert_eq!(ledge.y, GROUND + layout.ledge_h as i32);
        }
    }
}
