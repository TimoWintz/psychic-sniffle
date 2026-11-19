//! The avatar's body: a jointed rig of blocks and the code that animates it.
//!
//! The rig is built from the same blocks the avatar always had, hung on joints (hips, knees,
//! shoulders, elbows, spine, neck) so they can move. Poses are plain data produced by small
//! functions (idle, gait, air, typing, cheer, slump) and blended by weights that follow what the
//! avatar is doing. On top of that the head turns toward what is worth looking at: a test in flight,
//! the open console, or the place Lumen is pointing at.
//!
//! Nothing here moves the avatar: `player` owns its position, and the rig only follows.

use crate::carry::Carry;
use crate::companion::{Companion, Landmarks};
use crate::island::Sites;
use crate::player::{Player, RUN_SPEED, WALK_SPEED};
use crate::sim::Sim;
use crate::state::{Mode, Ui};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

const PELVIS: usize = 0;
const SPINE: usize = 1;
const NECK: usize = 2;
const HIP_L: usize = 3;
const KNEE_L: usize = 4;
const HIP_R: usize = 5;
const KNEE_R: usize = 6;
const SHOULDER_L: usize = 7;
const ELBOW_L: usize = 8;
const SHOULDER_R: usize = 9;
const ELBOW_R: usize = 10;
const JOINTS: usize = 11;

/// Each joint's parent (`None`: the body root) and its place in the parent's frame at rest.
const PARENT: [Option<usize>; JOINTS] = [None, Some(PELVIS), Some(SPINE), Some(PELVIS), Some(HIP_L), Some(PELVIS), Some(HIP_R), Some(SPINE), Some(SHOULDER_L), Some(SPINE), Some(SHOULDER_R)];
const REST: [Vec3; JOINTS] = [
    Vec3::new(0.0, 0.8, 0.0),     // pelvis, at the top of the legs
    Vec3::new(0.0, 0.04, 0.0),    // spine, at the waist
    Vec3::new(0.0, 0.6, 0.0),     // neck, at the top of the chest
    Vec3::new(-0.14, 0.0, 0.0),   // left hip
    Vec3::new(0.0, -0.42, 0.0),   // left knee
    Vec3::new(0.14, 0.0, 0.0),    // right hip
    Vec3::new(0.0, -0.42, 0.0),   // right knee
    Vec3::new(-0.37, 0.5, 0.0),   // left shoulder
    Vec3::new(0.0, -0.28, 0.0),   // left elbow
    Vec3::new(0.37, 0.5, 0.0),    // right shoulder
    Vec3::new(0.0, -0.28, 0.0),   // right elbow
];

/// Underside of a boot, front and back, in the knee's frame.
const SOLE: [Vec3; 2] = [Vec3::new(0.0, -0.38, -0.19), Vec3::new(0.0, -0.38, 0.13)];
/// Middle of a glove, in the elbow's frame.
const HAND: Vec3 = Vec3::new(0.0, -0.3, 0.0);
/// Middle of the helmet, in the neck's frame.
const HEAD: Vec3 = Vec3::new(0.0, 0.17, 0.0);
/// Height of the eyes above the feet at rest.
const EYE_HEIGHT: f32 = 1.64;

#[derive(Clone, Copy)]
enum Paint {
    Suit,
    Dark,
    Visor,
}

/// The blocks: joint they hang on, size, centre in the joint's frame, paint. The front is −Z.
const PARTS: [(usize, Vec3, Vec3, Paint); 16] = [
    (HIP_L, Vec3::new(0.22, 0.44, 0.26), Vec3::new(0.0, -0.2, 0.0), Paint::Dark),
    (KNEE_L, Vec3::new(0.2, 0.3, 0.24), Vec3::new(0.0, -0.15, 0.0), Paint::Dark),
    (KNEE_L, Vec3::new(0.22, 0.1, 0.32), Vec3::new(0.0, -0.33, -0.03), Paint::Dark),
    (HIP_R, Vec3::new(0.22, 0.44, 0.26), Vec3::new(0.0, -0.2, 0.0), Paint::Dark),
    (KNEE_R, Vec3::new(0.2, 0.3, 0.24), Vec3::new(0.0, -0.15, 0.0), Paint::Dark),
    (KNEE_R, Vec3::new(0.22, 0.1, 0.32), Vec3::new(0.0, -0.33, -0.03), Paint::Dark),
    (SPINE, Vec3::new(0.56, 0.62, 0.32), Vec3::new(0.0, 0.27, 0.0), Paint::Suit),
    (SPINE, Vec3::new(0.34, 0.42, 0.14), Vec3::new(0.0, 0.31, 0.23), Paint::Dark),
    (SHOULDER_L, Vec3::new(0.16, 0.32, 0.2), Vec3::new(0.0, -0.13, 0.0), Paint::Suit),
    (ELBOW_L, Vec3::new(0.15, 0.27, 0.19), Vec3::new(0.0, -0.12, 0.0), Paint::Suit),
    (SHOULDER_R, Vec3::new(0.16, 0.32, 0.2), Vec3::new(0.0, -0.13, 0.0), Paint::Suit),
    (ELBOW_R, Vec3::new(0.15, 0.27, 0.19), Vec3::new(0.0, -0.12, 0.0), Paint::Suit),
    (NECK, Vec3::new(0.4, 0.38, 0.4), HEAD, Paint::Suit),
    (NECK, Vec3::new(0.3, 0.16, 0.06), Vec3::new(0.0, 0.2, -0.2), Paint::Visor),
    (ELBOW_L, Vec3::new(0.13, 0.09, 0.15), HAND, Paint::Dark),
    (ELBOW_R, Vec3::new(0.13, 0.09, 0.15), HAND, Paint::Dark),
];

/// Full gait cycle (two steps) at walking and at running speed, metres.
const STRIDE_WALK: f32 = 1.9;
const STRIDE_RUN: f32 = 2.8;
/// How long the avatar must be off the ground before it takes the air pose (a step down is shorter).
const AIR_DELAY: f32 = 0.1;
/// Lumen's gaze is followed only to places this close.
const GAZE_REACH: f32 = 30.0;
const REACTION_SECONDS: f32 = 1.8;

/// What the avatar is doing, as the animation sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Activity {
    #[default]
    Idle,
    Walk,
    Run,
    Air,
    Typing,
    Watching,
    Cheer,
    Slump,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reaction {
    Cheer,
    Slump,
}

/// The rig's joints, found from the avatar.
#[derive(Component)]
pub struct Rig {
    body: Entity,
    joints: [Entity; JOINTS],
}

/// Animation state kept between frames.
#[derive(Component, Default)]
pub struct AvatarState {
    pub activity: Activity,
    clock: f32,
    last_feet: Option<Vec3>,
    /// Smoothed speed over the ground, m/s.
    speed: f32,
    /// Gait cycle position, 0..1.
    phase: f32,
    airborne_for: f32,
    /// Vertical speed in the last frame off the ground.
    air_vy: f32,
    /// Seconds since landing and how hard it was (0..1).
    landing: Option<(f32, f32)>,
    /// Height the body still lags behind the feet after a step up (negative).
    step: f32,
    reaction: Option<(Reaction, f32)>,
    results_seen: Vec<bool>,
    // Smoothed layer weights.
    w_move: f32,
    w_air: f32,
    w_typing: f32,
    w_carry: f32,
    w_reaction: f32,
    w_look: f32,
    /// Smoothed aim of the head, relative to the body: yaw and pitch, radians.
    look: Vec2,
}

pub struct AvatarPlugin;

impl Plugin for AvatarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, animate.after(crate::player::move_player).run_if(in_state(Mode::Playing)));
    }
}

/// Hang the avatar's blocks on joints under `player`.
pub fn attach(commands: &mut Commands, player: Entity, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
    let suit = materials.add(StandardMaterial { base_color: Color::srgb(0.85, 0.45, 0.12), perceptual_roughness: 0.8, ..default() });
    let dark = materials.add(StandardMaterial { base_color: Color::srgb(0.16, 0.18, 0.22), perceptual_roughness: 0.8, ..default() });
    let visor = materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.8, 0.95), perceptual_roughness: 0.2, ..default() });
    let body = commands.spawn((Transform::default(), Visibility::default())).id();
    commands.entity(player).add_child(body);
    let mut joints = [Entity::PLACEHOLDER; JOINTS];
    for j in 0..JOINTS {
        let parent = PARENT[j].map_or(body, |p| joints[p]);
        joints[j] = commands.spawn((Transform::from_translation(REST[j]), Visibility::default())).id();
        commands.entity(parent).add_child(joints[j]);
    }
    for (joint, size, centre, paint) in PARTS {
        let material = match paint {
            Paint::Suit => suit.clone(),
            Paint::Dark => dark.clone(),
            Paint::Visor => visor.clone(),
        };
        let part = commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(material), Transform::from_translation(centre))).id();
        commands.entity(joints[joint]).add_child(part);
    }
    commands.entity(player).insert((Rig { body, joints }, AvatarState::default()));
}

/// Joint rotations and the body's offset from the feet.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pose {
    rot: [Quat; JOINTS],
    body: Vec3,
}

impl Default for Pose {
    fn default() -> Self {
        Pose { rot: [Quat::IDENTITY; JOINTS], body: Vec3::ZERO }
    }
}

impl Pose {
    fn blend(&self, other: &Pose, w: f32) -> Pose {
        if w <= 0.0 {
            return *self;
        }
        if w >= 1.0 {
            return *other;
        }
        let mut out = *self;
        for j in 0..JOINTS {
            out.rot[j] = self.rot[j].slerp(other.rot[j], w);
        }
        out.body = self.body.lerp(other.body, w);
        out
    }

    /// Position and rotation of a joint in the avatar's frame (feet at the origin).
    fn joint(&self, j: usize) -> (Vec3, Quat) {
        let (at, turn) = match PARENT[j] {
            Some(p) => self.joint(p),
            None => (self.body, Quat::IDENTITY),
        };
        (at + turn * REST[j], turn * self.rot[j])
    }

    fn point(&self, j: usize, local: Vec3) -> Vec3 {
        let (at, turn) = self.joint(j);
        at + turn * local
    }

    /// Height of the lowest point of either boot.
    fn lowest_sole(&self) -> f32 {
        [KNEE_L, KNEE_R].iter().flat_map(|&k| SOLE.map(|s| self.point(k, s).y)).fold(f32::INFINITY, f32::min)
    }

    /// Move the body up or down so the lowest boot rests on the ground.
    fn plant_offset(&self) -> f32 {
        -self.lowest_sole()
    }
}

fn rx(a: f32) -> Quat {
    Quat::from_rotation_x(a)
}

/// A limb's rotation: `swing` forward (positive) or back about the side axis, then `out` away from
/// the body (`side` is −1 for left, +1 for right), then `twist` about the vertical.
fn limb(side: f32, swing: f32, out: f32, twist: f32) -> Quat {
    Quat::from_rotation_y(side * twist) * Quat::from_rotation_z(side * out) * rx(swing)
}

const SIDES: [(usize, usize, usize, usize, f32); 2] = [(HIP_L, KNEE_L, SHOULDER_L, ELBOW_L, -1.0), (HIP_R, KNEE_R, SHOULDER_R, ELBOW_R, 1.0)];

/// Standing: breathing and a little sway. `t` in seconds.
fn idle(t: f32) -> Pose {
    let mut p = Pose::default();
    let breath = (TAU * t / 3.6).sin();
    p.rot[SPINE] = rx(0.02 * breath);
    p.rot[NECK] = rx(-0.02 * breath);
    for (_, _, shoulder, elbow, side) in SIDES {
        p.rot[shoulder] = limb(side, 0.02 * breath, 0.07 + 0.015 * breath, 0.0);
        p.rot[elbow] = rx(0.14);
    }
    p
}

/// Walking (`run` 0) to running (`run` 1), at `phase` of the gait cycle (0..1).
fn gait(phase: f32, run: f32) -> Pose {
    let mut p = Pose::default();
    let angle = TAU * phase;
    let leg = 0.5 + 0.3 * run;
    let knee = 0.95 + 0.7 * run;
    let arm = 0.45 + 0.35 * run;
    let elbow = 0.35 + 1.0 * run;
    for (k, (hip, knee_j, shoulder, elbow_j, side)) in SIDES.into_iter().enumerate() {
        // The right side is half a cycle behind the left.
        let a = angle + k as f32 * PI;
        let swing = a.sin();
        // The knee folds while the leg swings forward, most as it passes under the body.
        let fold = a.cos().max(0.0);
        p.rot[hip] = rx(leg * swing);
        p.rot[knee_j] = rx(-knee * fold - 0.12 * run);
        p.rot[shoulder] = limb(side, -arm * swing, 0.08, 0.0);
        p.rot[elbow_j] = rx(elbow);
    }
    let twist = angle.sin();
    p.rot[PELVIS] = Quat::from_rotation_y(0.07 * twist);
    p.rot[SPINE] = Quat::from_rotation_y(-0.12 * twist) * rx(-0.06 - 0.22 * run);
    p.rot[NECK] = rx(0.04 + 0.16 * run);
    p
}

/// In the air: legs tucked and arms up while rising, legs reaching and arms out while falling.
fn air(vy: f32) -> Pose {
    let rising = Pose {
        rot: {
            let mut r = [Quat::IDENTITY; JOINTS];
            r[HIP_L] = rx(0.9);
            r[KNEE_L] = rx(-1.5);
            r[HIP_R] = rx(0.25);
            r[KNEE_R] = rx(-0.7);
            r[SHOULDER_L] = limb(-1.0, 1.1, 0.25, 0.0);
            r[ELBOW_L] = rx(0.8);
            r[SHOULDER_R] = limb(1.0, -0.4, 0.35, 0.0);
            r[ELBOW_R] = rx(0.6);
            r[SPINE] = rx(-0.1);
            r
        },
        body: Vec3::ZERO,
    };
    let falling = Pose {
        rot: {
            let mut r = [Quat::IDENTITY; JOINTS];
            r[HIP_L] = rx(0.35);
            r[KNEE_L] = rx(-0.45);
            r[HIP_R] = rx(-0.1);
            r[KNEE_R] = rx(-0.25);
            r[SHOULDER_L] = limb(-1.0, 0.2, 0.95, 0.0);
            r[ELBOW_L] = rx(0.3);
            r[SHOULDER_R] = limb(1.0, 0.2, 0.95, 0.0);
            r[ELBOW_R] = rx(0.3);
            r[NECK] = rx(-0.2);
            r
        },
        body: Vec3::ZERO,
    };
    let up = (vy / crate::player::JUMP_SPEED * 0.5 + 0.5).clamp(0.0, 1.0);
    falling.blend(&rising, up)
}

/// Working a console: hands forward at the panel, tapping.
fn typing(t: f32) -> Pose {
    let mut p = idle(t);
    for (k, (_, _, shoulder, elbow, side)) in SIDES.into_iter().enumerate() {
        let tap = (TAU * 2.7 * t + k as f32 * 2.1).sin().max(0.0) * (TAU * 0.4 * t + k as f32).sin().max(0.0);
        p.rot[shoulder] = limb(side, 0.85 + 0.06 * tap, 0.0, 0.32);
        p.rot[elbow] = rx(0.75 - 0.2 * tap);
    }
    p.rot[SPINE] = rx(-0.08);
    p
}

/// Carrying a crate: both arms forward and bent, hands either side of it at chest height. Only the
/// arms and the back; the legs keep whatever they are doing.
fn carrying(under: &Pose) -> Pose {
    let mut p = *under;
    for (_, _, shoulder, elbow, side) in SIDES {
        p.rot[shoulder] = limb(side, 1.05, 0.22, 0.0);
        p.rot[elbow] = rx(0.5);
    }
    p.rot[SPINE] = rx(0.06) * under.rot[SPINE];
    p
}

/// A pass: both arms up, a hop, a wave.
fn cheer(t: f32) -> Pose {
    let mut p = Pose::default();
    let wave = (TAU * 2.2 * t).sin();
    for (_, _, shoulder, elbow, side) in SIDES {
        p.rot[shoulder] = limb(side, 2.75, -0.45 - 0.12 * wave * side, 0.0);
        p.rot[elbow] = rx(0.25);
    }
    p.rot[SPINE] = rx(0.08);
    p.rot[NECK] = rx(0.25);
    if t < 0.42 {
        p.body.y = 0.28 * (PI * t / 0.42).sin();
    }
    p
}

/// A miss: head down, one hand at the helmet, a shake of the head.
fn slump(t: f32) -> Pose {
    let mut p = Pose::default();
    p.rot[SPINE] = rx(-0.22);
    let shake = if t < 1.0 { 0.28 * (TAU * 2.0 * t).sin() * (1.0 - t) } else { 0.0 };
    p.rot[NECK] = Quat::from_rotation_y(shake) * rx(-0.45);
    // The elbow up and a little out, so the forearm folds back onto the side of the helmet. (A twist
    // would not help: it turns the elbow in but swings the folded-back hand out, away from the head.)
    p.rot[SHOULDER_R] = limb(1.0, 2.25, 0.3, 0.0);
    p.rot[ELBOW_R] = rx(2.1);
    p.rot[SHOULDER_L] = limb(-1.0, -0.05, 0.05, 0.0);
    p.rot[ELBOW_L] = rx(0.1);
    for (hip, knee, _, _, _) in SIDES {
        p.rot[hip] = rx(0.12);
        p.rot[knee] = rx(-0.22);
    }
    p
}

/// Bend at hips and knees by `amount` (0..1), as when landing.
fn crouch(pose: &mut Pose, amount: f32) {
    for (hip, knee, _, _, _) in SIDES {
        pose.rot[hip] = rx(0.75 * amount) * pose.rot[hip];
        pose.rot[knee] = rx(-1.3 * amount) * pose.rot[knee];
    }
    pose.rot[SPINE] = rx(-0.35 * amount) * pose.rot[SPINE];
}

/// Limits of a look: the chest turns a little, the neck does the rest.
const SPINE_YAW: f32 = 0.45;
const NECK_YAW: f32 = 1.2;
const NECK_UP: f32 = 0.6;
const NECK_DOWN: f32 = 0.75;

/// Split an aim (yaw, pitch relative to the body) between chest and neck, within their limits.
fn split_look(yaw: f32, pitch: f32) -> (f32, f32, f32) {
    let spine = (yaw * 0.35).clamp(-SPINE_YAW, SPINE_YAW);
    let neck = (yaw - spine).clamp(-NECK_YAW, NECK_YAW);
    (spine, neck, pitch.clamp(-NECK_DOWN, NECK_UP))
}

/// Turn the head (and a little of the chest) toward an aim, by `weight`.
fn look(pose: &mut Pose, aim: Vec2, weight: f32) {
    if weight <= 0.0 {
        return;
    }
    let (spine, neck, pitch) = split_look(aim.x, aim.y);
    pose.rot[SPINE] = Quat::from_rotation_y(spine * weight) * pose.rot[SPINE];
    let head = Quat::from_rotation_y(neck) * rx(pitch);
    pose.rot[NECK] = pose.rot[NECK].slerp(head, weight);
}

/// Fraction of the way to close on a target this frame, at `rate` per second.
fn approach(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

fn ease(value: &mut f32, target: f32, rate: f32, dt: f32) {
    *value += (target - *value) * approach(rate, dt);
}

/// Yaw that faces along `d` (the avatar's front is −Z).
fn facing(d: Vec3) -> f32 {
    f32::atan2(-d.x, -d.z)
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

#[allow(clippy::too_many_arguments)]
fn animate(
    time: Res<Time>,
    ui: Res<Ui>,
    sim: Res<Sim>,
    sites: Res<Sites>,
    carry: Res<Carry>,
    companion: Res<Companion>,
    landmarks: Res<Landmarks>,
    mut avatar: Single<(&Player, &mut Transform, &Rig, &mut AvatarState)>,
    mut parts: Query<&mut Transform, Without<Player>>,
) {
    let (player, transform, rig, state) = &mut *avatar;
    // A plain reference, so separate fields can be borrowed at once.
    let state: &mut AvatarState = state;
    let dt = time.delta_secs().min(1.0 / 20.0);
    state.clock += dt;
    let feet = transform.translation;

    // Movement as seen from where the feet actually went (a wall stops the legs too).
    let moved = feet - state.last_feet.unwrap_or(feet);
    state.last_feet = Some(feet);
    if moved.length() > 3.0 {
        // Put somewhere else (a respawn or a script): start over standing.
        let results_seen = std::mem::take(&mut state.results_seen);
        *state = AvatarState { last_feet: Some(feet), results_seen, ..default() };
        return;
    }
    let ground_speed = if dt > 0.0 { moved.with_y(0.0).length() / dt } else { 0.0 };
    ease(&mut state.speed, ground_speed, 12.0, dt);
    let run = ((state.speed - WALK_SPEED) / (RUN_SPEED - WALK_SPEED)).clamp(0.0, 1.0);
    let stride = STRIDE_WALK + (STRIDE_RUN - STRIDE_WALK) * run;
    state.phase = (state.phase + ground_speed * dt / stride).fract();
    let still = state.speed < 0.4;

    if player.grounded {
        if state.airborne_for > AIR_DELAY {
            let impact = (-state.air_vy / 14.0).clamp(0.2, 1.0);
            state.landing = Some((0.0, impact));
        } else if moved.y > 0.3 && state.airborne_for == 0.0 {
            // A step up: the feet were lifted a block at once; the body catches up.
            state.step -= moved.y;
        }
        state.airborne_for = 0.0;
    } else {
        state.airborne_for += dt;
        state.air_vy = player.velocity.y;
    }
    ease(&mut state.step, 0.0, 16.0, dt);
    if let Some((t, _)) = &mut state.landing {
        *t += dt;
    }
    if state.landing.is_some_and(|(t, _)| t > 0.4) {
        state.landing = None;
    }

    // A test that has just been judged gets a reaction.
    state.results_seen.resize(ui.results.len(), false);
    for (seen, result) in state.results_seen.iter_mut().zip(&ui.results) {
        if !*seen && let Some(outcome) = result {
            state.reaction = Some((if outcome.pass { Reaction::Cheer } else { Reaction::Slump }, 0.0));
        }
        *seen = result.is_some();
    }
    if let Some((_, t)) = &mut state.reaction {
        *t += dt;
    }
    if state.reaction.is_some_and(|(_, t)| t > REACTION_SECONDS) || state.airborne_for > AIR_DELAY {
        state.reaction = None;
    }

    // What to look at, most important first.
    let console = ui.station.map(|i| sites.stations[i].as_vec3() + Vec3::new(0.5, 1.3, 0.5));
    let action = sim.action_point();
    let gaze = companion.gaze.as_deref().and_then(|name| landmarks.position(name)).filter(|p| p.distance(feet) < GAZE_REACH);
    let target = action.or(console).or(if run < 0.5 { gaze } else { None });
    let typing_now = console.is_some() && action.is_none() && still && state.reaction.is_none();
    // Hands full: no cheering or slumping, the block stays held.
    if carry.holding.is_some() {
        state.reaction = None;
    }

    // Turn the whole body to the console, or to anything the head cannot reach while standing.
    if still && player.grounded {
        let face = if typing_now { console } else { target };
        if let Some(face) = face {
            let d = (face - feet).with_y(0.0);
            let off = wrap(facing(d) - transform.rotation.to_euler(EulerRot::YXZ).0);
            if d.length() > 0.3 && (typing_now || off.abs() > 1.75) {
                let want = Quat::from_rotation_y(facing(d));
                transform.rotation = transform.rotation.slerp(want, approach(5.0, dt));
            }
        }
    }

    let air_now = state.airborne_for > AIR_DELAY;
    state.activity = if air_now {
        Activity::Air
    } else if let Some((reaction, _)) = state.reaction {
        if reaction == Reaction::Cheer { Activity::Cheer } else { Activity::Slump }
    } else if action.is_some() {
        Activity::Watching
    } else if typing_now {
        Activity::Typing
    } else if state.speed > (WALK_SPEED + RUN_SPEED) / 2.0 {
        Activity::Run
    } else if !still {
        Activity::Walk
    } else {
        Activity::Idle
    };

    let move_target = (state.speed / WALK_SPEED * 1.5).clamp(0.0, 1.0);
    ease(&mut state.w_move, move_target, 10.0, dt);
    ease(&mut state.w_air, air_now as u8 as f32, 12.0, dt);
    ease(&mut state.w_typing, typing_now as u8 as f32, 6.0, dt);
    ease(&mut state.w_carry, carry.holding.is_some() as u8 as f32, 10.0, dt);
    ease(&mut state.w_reaction, state.reaction.is_some_and(|(_, t)| t < REACTION_SECONDS - 0.3) as u8 as f32, 9.0, dt);
    ease(&mut state.w_look, target.is_some() as u8 as f32, 4.0, dt);
    if let Some(target) = target {
        let d = transform.rotation.inverse() * (target - (feet + Vec3::Y * EYE_HEIGHT));
        let aim = Vec2::new(facing(d), f32::atan2(d.y, d.with_y(0.0).length()));
        state.look.x += wrap(aim.x - state.look.x) * approach(8.0, dt);
        state.look.y += (aim.y - state.look.y) * approach(8.0, dt);
    }

    let t = state.clock;
    let mut pose = idle(t).blend(&gait(state.phase, run), state.w_move);
    pose = pose.blend(&typing(t), state.w_typing);
    pose = pose.blend(&carrying(&pose), state.w_carry);
    if let Some((reaction, rt)) = state.reaction {
        let react = if reaction == Reaction::Cheer { cheer(rt) } else { slump(rt) };
        pose = pose.blend(&react, state.w_reaction);
    }
    if let Some((lt, impact)) = state.landing {
        // Down fast, back up slower.
        let curve = if lt < 0.08 { lt / 0.08 } else { 1.0 - (lt - 0.08) / 0.32 };
        crouch(&mut pose, 0.55 * impact * curve.clamp(0.0, 1.0));
    }
    pose = pose.blend(&air(state.air_vy), state.w_air);
    pose.body.y += pose.plant_offset() * (1.0 - state.w_air) + state.step;
    look(&mut pose, state.look, state.w_look);

    if let Ok(mut body) = parts.get_mut(rig.body) {
        body.translation = pose.body;
    }
    for (j, entity) in rig.joints.iter().enumerate() {
        if let Ok(mut joint) = parts.get_mut(*entity) {
            joint.rotation = pose.rot[j];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planted(mut pose: Pose) -> Pose {
        pose.body.y += pose.plant_offset();
        pose
    }

    fn angle(q: Quat) -> f32 {
        q.to_axis_angle().1.min(TAU - q.to_axis_angle().1)
    }

    #[test]
    fn carrying_holds_the_hands_forward_either_side_of_the_crate() {
        for under in [idle(0.0), gait(0.3, 0.0)] {
            let pose = planted(carrying(&under));
            let (left, right) = (pose.point(ELBOW_L, HAND), pose.point(ELBOW_R, HAND));
            for hand in [left, right] {
                assert!(hand.z < -0.25, "hands in front of the body: {hand}");
                assert!((0.75..1.35).contains(&hand.y), "hands at chest height: {hand}");
            }
            assert!((left.x - right.x).abs() > 0.4, "hands apart, either side of the crate: {left} {right}");
            // The legs keep walking.
            assert_eq!(pose.rot[HIP_L], planted(under).rot[HIP_L]);
        }
    }

    #[test]
    fn at_rest_the_boots_stand_on_the_ground_and_the_head_is_on_top() {
        let rest = Pose::default();
        assert!(rest.lowest_sole().abs() < 1e-5);
        let top = rest.point(NECK, HEAD).y + 0.19;
        assert!((top - 1.8).abs() < 0.01, "the avatar is 1.8 m tall, its collision box says so: {top}");
    }

    #[test]
    fn a_planted_gait_keeps_a_boot_on_the_ground_and_bobs_a_little() {
        for run in [0.0, 0.5, 1.0] {
            let heights: Vec<f32> = (0..64).map(|i| planted(gait(i as f32 / 64.0, run)).body.y).collect();
            let (low, high) = heights.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), y| (l.min(*y), h.max(*y)));
            // A walk barely bobs; a run's longer stride, with a boot kept on the ground, dips more.
            let bob = 0.08 + 0.1 * run;
            assert!(high - low < bob, "bob of {} m at run {run}", high - low);
            assert!(low > -0.2, "the body sinks {low} m at run {run}");
            for i in 0..64 {
                let pose = planted(gait(i as f32 / 64.0, run));
                assert!(pose.lowest_sole().abs() < 1e-4);
            }
        }
    }

    #[test]
    fn the_gait_mirrors_after_half_a_cycle() {
        for i in 0..16 {
            let p = i as f32 / 16.0;
            let (a, b) = (gait(p, 0.3), gait(p + 0.5, 0.3));
            for (l, r) in [(HIP_L, HIP_R), (KNEE_L, KNEE_R), (ELBOW_L, ELBOW_R)] {
                // Not `angle_between`: it takes the acos of a dot product, and in f32 a dot that
                // rounds to 0.99999994 already reads as ~7e-4 rad, so equal rotations look apart.
                assert!(a.rot[l].abs_diff_eq(b.rot[r], 1e-5), "{l} {r} at {p}: {} {}", a.rot[l], b.rot[r]);
            }
            // Arms swing the same way in the side plane; their outward lean is mirrored.
            let fwd = |q: Quat| (q * Vec3::NEG_Y).z;
            assert!((fwd(a.rot[SHOULDER_L]) - fwd(b.rot[SHOULDER_R])).abs() < 1e-4);
        }
    }

    #[test]
    fn arms_swing_against_the_legs() {
        let pose = gait(0.25, 0.0);
        let foot_l = pose.point(KNEE_L, SOLE[0]).z;
        let hand_l = pose.point(ELBOW_L, HAND).z;
        let hand_r = pose.point(ELBOW_R, HAND).z;
        assert!(foot_l < 0.0, "the left leg is forward at a quarter cycle");
        assert!(hand_l > hand_r, "so the left hand is back and the right forward");
    }

    #[test]
    fn looks_stay_within_the_neck_and_chest() {
        for yaw in [-3.0f32, -1.5, -0.4, 0.0, 0.7, 2.0, 3.1] {
            for pitch in [-1.5f32, -0.3, 0.0, 0.5, 1.5] {
                let (spine, neck, p) = split_look(yaw, pitch);
                assert!(spine.abs() <= SPINE_YAW + 1e-6 && neck.abs() <= NECK_YAW + 1e-6);
                assert!((-NECK_DOWN..=NECK_UP).contains(&p));
                if yaw.abs() < 1.5 {
                    assert!((spine + neck - yaw).abs() < 1e-5, "a reachable aim is reached");
                }
            }
        }
        // The head ends up facing the aim.
        let mut pose = Pose::default();
        look(&mut pose, Vec2::new(0.8, -0.3), 1.0);
        let (_, head) = pose.joint(NECK);
        let ahead = head * Vec3::NEG_Z;
        assert!((facing(ahead) - 0.8).abs() < 1e-3 && (ahead.y.asin() + 0.3).abs() < 1e-3);
    }

    #[test]
    fn blending_ends_at_either_pose() {
        let (a, b) = (idle(0.3), cheer(0.2));
        assert_eq!(a.blend(&b, 0.0), a);
        assert_eq!(a.blend(&b, 1.0), b);
        let half = a.blend(&b, 0.5);
        assert!((angle(half.rot[SHOULDER_L].inverse() * a.rot[SHOULDER_L]) - angle(half.rot[SHOULDER_L].inverse() * b.rot[SHOULDER_L])).abs() < 1e-3);
    }

    #[test]
    fn starting_to_walk_does_not_jolt() {
        // The gait at its start is close to standing, so the blend in from idle is smooth.
        let (a, b) = (idle(0.0), gait(0.0, 0.0));
        for (l, r) in [(HIP_L, HIP_R), (SHOULDER_L, SHOULDER_R)] {
            assert!(angle(a.rot[l].inverse() * b.rot[l]) < 0.25 && angle(a.rot[r].inverse() * b.rot[r]) < 0.25);
        }
    }

    #[test]
    fn typing_hands_are_at_the_panel_in_front() {
        let pose = typing(0.0);
        for elbow in [ELBOW_L, ELBOW_R] {
            let hand = pose.point(elbow, HAND);
            assert!(hand.z < -0.35, "hand in front of the chest: {hand}");
            assert!((0.95..1.35).contains(&hand.y), "hand at console height: {hand}");
            assert!(hand.x.abs() < 0.35, "hands drawn in toward the middle: {hand}");
        }
    }

    #[test]
    fn a_cheer_raises_both_hands_above_the_head_apart() {
        // Through the hop and the wave, where one arm leans out further and so reaches less high.
        for t in [0.1, 0.5, 0.8, 1.0, 1.3] {
            let pose = cheer(t);
            let (left, right) = (pose.point(ELBOW_L, HAND), pose.point(ELBOW_R, HAND));
            let top = pose.point(NECK, HEAD).y + 0.19;
            assert!(left.y > top && right.y > top, "at {t}: {left} {right}, top of the helmet {top}");
            assert!(left.x < -0.3 && right.x > 0.3, "at {t}: {left} {right}");
        }
    }

    #[test]
    fn a_slump_puts_a_hand_to_the_helmet() {
        let pose = slump(1.2);
        let hand = pose.point(ELBOW_R, HAND);
        let head = pose.point(NECK, HEAD);
        assert!(hand.distance(head) < 0.4, "hand {hand}, head {head}");
        assert!(pose.point(ELBOW_L, HAND).y < 1.0, "the other hand hangs");
    }

    #[test]
    fn the_air_pose_tucks_while_rising_and_reaches_while_falling() {
        let rising = planted(air(6.0));
        let falling = planted(air(-12.0));
        // How high the boot is drawn up under the body, by the middle of its sole: the toe would
        // understate it, as a tucked boot tips toe down.
        let tuck = |p: &Pose| p.point(KNEE_L, (SOLE[0] + SOLE[1]) / 2.0).y - p.body.y;
        assert!(tuck(&rising) > tuck(&falling) + 0.15, "rising {} falling {}", tuck(&rising), tuck(&falling));
    }
}
