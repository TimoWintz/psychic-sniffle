//! The avatar and the camera that follows it.

use crate::island::Sites;
use crate::ruler::Ruler;
use crate::state::{Game, Mode, Ui};
use crate::voxel::VoxelWorld;
use crate::waking::Waking;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bevy_egui::input::EguiWantsInput;
use std::sync::OnceLock;

/// Half-width and height of the avatar's collision box. Its transform is at its feet.
const HALF_WIDTH: f32 = 0.3;
const HEIGHT: f32 = 1.8;
/// How many blocks the avatar is lifted to stand on something that rose where it was standing.
const LIFT_OUT: i32 = 16;
pub(crate) const WALK_SPEED: f32 = 5.0;
pub(crate) const RUN_SPEED: f32 = 9.0;
/// Take-off speed of a jump. The height reached depends on the planet: v^2 / (2 g).
pub(crate) const JUMP_SPEED: f32 = 6.0;
/// Grace period after stepping off a ledge during which jumping still works.
const COYOTE_SECONDS: f32 = 0.12;
/// Window before landing during which a Space press is remembered and fires on touchdown.
const JUMP_BUFFER_SECONDS: f32 = 0.12;

#[derive(Component, Default)]
pub struct Player {
    pub velocity: Vec3,
    pub grounded: bool,
    /// Seconds left of coyote time after walking off an edge.
    coyote: f32,
    /// Seconds left of a buffered jump press.
    jump_buffer: f32,
    /// Negative vertical offset after a 1-block step-up, decayed smoothly so the camera glides up.
    pub step_offset: f32,
}

#[derive(Resource)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    /// When set, the camera looks at this point instead of the avatar (used to follow a test).
    pub watch: Option<Vec3>,
    focus: Vec3,
    /// The player's yaw, pitch and distance while a test, a station or a wake has borrowed the camera.
    saved: Option<(f32, f32, f32)>,
    /// The player's yaw, pitch and distance the camera is easing back to, after a wake or a station.
    returning: Option<(f32, f32, f32)>,
    /// Remaining distance glide after manual rotation cancels a return glide (e.g. zooming back in
    /// from a wide station view while the player is already turning to walk away).
    returning_distance: Option<f32>,
    /// The yaw, pitch and distance the camera is turning to: to show something past the avatar, or a
    /// station's puzzle.
    turning: Option<(f32, f32, f32)>,
    /// The camera is on a side view (a test, or a station's puzzle), which may stand further back
    /// than the player zooms out.
    side_view: bool,
    /// Whether the cursor is hidden and grabbed for button-free mouse look while exploring.
    pub free_look: bool,
    /// Frames left to ignore mouse motion after entering `free_look` (avoids OS grab spikes).
    lock_settle: u8,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        OrbitCamera {
            yaw: -0.6,
            pitch: 0.45,
            distance: 14.0,
            watch: None,
            focus: Vec3::ZERO,
            saved: None,
            returning: None,
            returning_distance: None,
            turning: None,
            side_view: false,
            free_look: false,
            lock_settle: 0,
        }
    }
}

/// Whether button-free mouse look may engage: focused window, exploring with no panel, ruler, or
/// waking scene active.
pub fn can_free_look(window_focused: bool, at_console: bool, panel_open: bool, ruler_in_use: bool, waking: bool) -> bool {
    window_focused && !at_console && !panel_open && !ruler_in_use && !waking
}

/// Automated screenshot/test scripts drive the camera themselves and should not lock the OS cursor.
/// `prefill` is played by hand and behaves like a normal game.
fn scripted_run() -> bool {
    static SCRIPTED: OnceLock<bool> = OnceLock::new();
    *SCRIPTED.get_or_init(|| std::env::var("GAME_SCRIPT").is_ok_and(|s| s != "prefill"))
}

/// The camera's nearest and furthest distance from what it looks at.
const NEAREST: f32 = 3.0;
const FURTHEST: f32 = 80.0;
/// How far back a side view may stand, to fit a whole test (the ridge's is about 95 m): further than
/// the player zooms out, and well inside the fog.
pub(crate) const SIDE_VIEW_FURTHEST: f32 = 120.0;

/// Tangent of half the camera's vertical field of view (Bevy's default is 45 degrees).
const TAN_HALF_FOV: f32 = 0.4142;
const ASPECT: f32 = 16.0 / 9.0;

impl OrbitCamera {
    /// Turn to a side view that shows the box from `min` to `max`, with `along` running left to
    /// right. The box is placed in the left part of the screen, clear of the station panel. The
    /// player's own view is kept and comes back with [`OrbitCamera::release_view`] or
    /// [`OrbitCamera::ease_home`]. With `glide`, the camera turns to the view over a moment (a
    /// station's view); without, it cuts to it (a test).
    pub fn frame_side_view(&mut self, min: Vec3, max: Vec3, along: Vec3, glide: bool) {
        self.keep_view();
        // Either corner may come first.
        let (min, max) = (min.min(max), min.max(max));
        let size = max - min;
        let width = size.with_y(0.0).length() + 8.0;
        let height = size.y + 6.0;
        let half_screen_width = ASPECT * TAN_HALF_FOV;
        // The box may use the left half of the screen's width and most of its height.
        let distance = (width / half_screen_width).max(height / (1.4 * TAN_HALF_FOV)).clamp(18.0, SIDE_VIEW_FURTHEST);
        let yaw = f32::atan2(-along.z, along.x);
        let view = (yaw, 0.18, distance);
        if glide {
            self.turning = Some(view);
        } else {
            (self.yaw, self.pitch, self.distance) = view;
        }
        let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
        self.side_view = true;
        self.watch = Some((min + max) / 2.0 + right * 0.42 * half_screen_width * distance);
    }

    /// Take the camera to watch something with this pitch and distance (set `watch` to say what),
    /// keeping the player's own view to come back with [`OrbitCamera::release_view`].
    pub fn borrow(&mut self, pitch: f32, distance: f32) {
        self.keep_view();
        self.side_view = false;
        self.pitch = pitch;
        self.distance = distance;
    }

    /// Turn the camera, over a moment, to look past `from` (the avatar) towards `to`, with this
    /// pitch and distance, keeping the player's own view to come back with
    /// [`OrbitCamera::ease_home`].
    pub fn face(&mut self, from: Vec3, to: Vec3, pitch: f32, distance: f32) {
        self.keep_view();
        self.side_view = false;
        self.turning = Some((yaw_towards(from, to), pitch, distance));
    }

    /// Keep the player's own view to come back to, unless it is kept already. Whatever the camera
    /// was turning to or easing back to stops: it is taken from here.
    fn keep_view(&mut self) {
        if self.saved.is_none() {
            let fallback = (self.yaw, self.pitch, self.returning_distance.unwrap_or(self.distance));
            self.saved = Some(self.returning.take().unwrap_or(fallback));
        }
        self.returning_distance = None;
        self.turning = None;
    }

    /// Set the yaw, pitch and distance outright, cancelling any glide (the scripts' `Look`).
    pub fn set_view(&mut self, yaw: f32, pitch: f32, distance: f32) {
        (self.yaw, self.pitch, self.distance) = (yaw, pitch, distance);
        self.returning = None;
        self.returning_distance = None;
        self.turning = None;
    }

    pub fn release_view(&mut self) {
        self.watch = None;
        self.returning = None;
        self.returning_distance = None;
        self.turning = None;
        self.side_view = false;
        if let Some((yaw, pitch, distance)) = self.saved.take() {
            (self.yaw, self.pitch, self.distance) = (yaw, pitch, distance);
        }
    }

    /// Like [`OrbitCamera::release_view`], but the camera glides back to the player's own view over
    /// a moment instead of cutting to it.
    pub fn ease_home(&mut self) {
        self.watch = None;
        self.turning = None;
        self.returning_distance = None;
        self.side_view = false;
        self.returning = self.saved.take();
    }

    /// The player rotated the camera manually: stop any yaw/pitch glide so the camera never fights
    /// input. A return glide's zoom still finishes smoothly unless the player also scrolls. While
    /// exploring (`watch.is_none()`), also drop any saved return angle so a finished story cue does
    /// not yank the camera back later.
    fn manual_rotate(&mut self) {
        if let Some((_, _, distance)) = self.returning.take() {
            self.returning_distance = Some(distance);
        }
        self.turning = None;
        if self.watch.is_none() {
            self.saved = None;
        }
    }

    /// The player zoomed manually: stop any distance glide so zoom stays where it was set.
    fn manual_zoom(&mut self) {
        self.returning_distance = None;
        if let Some((_, _, distance)) = &mut self.returning {
            *distance = self.distance;
        }
        if let Some((_, _, distance)) = &mut self.turning {
            *distance = self.distance;
        }
    }

    /// Where the camera stands to look at `focus` with this yaw, pitch and distance, when nothing is
    /// in the way.
    pub fn eye(&self, focus: Vec3) -> Vec3 {
        focus + self.back() * self.distance
    }

    /// Direction from what the camera looks at back to the camera.
    fn back(&self) -> Vec3 {
        Quat::from_euler(EulerRot::YXZ, self.yaw, -self.pitch, 0.0) * Vec3::Z
    }
}

/// The camera's yaw that looks from `from` towards `to`: the way forward is `to`'s way.
fn yaw_towards(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    f32::atan2(-d.x, -d.z)
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OrbitCamera>()
            .init_resource::<Shore>()
            .add_systems(Startup, spawn)
            .add_systems(Update, (move_player, orbit_camera).chain().run_if(in_state(Mode::Playing)));
    }
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sites: Res<Sites>,
    mut camera: ResMut<OrbitCamera>,
    mut shore: ResMut<Shore>,
) {
    shore.dry = sites.spawn;
    let player = commands.spawn((Player::default(), Transform::from_translation(sites.spawn), Visibility::default())).id();
    crate::avatar::attach(&mut commands, player, &mut meshes, &mut materials);
    camera.focus = sites.spawn + Vec3::Y * 1.5;
}

fn collides(world: &VoxelWorld, feet: Vec3) -> bool {
    world.box_collides(feet - Vec3::new(HALF_WIDTH, 0.0, HALF_WIDTH), feet + Vec3::new(HALF_WIDTH, HEIGHT, HALF_WIDTH))
}

/// How near the place it last stood on dry ground the avatar must be to get back onto dry ground
/// from over deep water. Less than a block, so it can't jump a strip of deep water either.
const REACH_BACK: f32 = 1.0;

/// Whether the water under `p` is too deep to wade.
pub(crate) fn over_deep_water(world: &VoxelWorld, p: Vec3) -> bool {
    crate::island::too_deep(world.surface(p.x.floor() as i32, p.z.floor() as i32))
}

/// Whether a step from `from` to `to` is barred by deep water. Out over it, nothing holds the avatar
/// up, so it can only step back onto the dry ground it came from (`dry` is where it last stood
/// there). Anywhere else it falls in, even where a jump would carry it across to the far bank.
pub(crate) fn barred_by_water(world: &VoxelWorld, dry: Vec3, from: Vec3, to: Vec3) -> bool {
    over_deep_water(world, from) && !over_deep_water(world, to) && to.xz().distance(dry.xz()) >= REACH_BACK
}

/// Whether the avatar at `feet` has fallen into deep water: it is in the water, or something other
/// than the edge it stepped off holds it up out there (the lip of the far bank, under the edge of
/// its box).
pub(crate) fn fallen_in(world: &VoxelWorld, dry: Vec3, feet: Vec3, grounded: bool) -> bool {
    over_deep_water(world, feet) && (feet.y <= crate::island::SEA_LEVEL || grounded && feet.xz().distance(dry.xz()) >= REACH_BACK)
}

/// Where the avatar last stood on dry ground, which is where it comes back to after falling into
/// deep water, and how often it has fallen in this session (Lumen remarks on the first time).
#[derive(Resource, Default)]
pub struct Shore {
    pub dry: Vec3,
    pub fallen_in: u32,
    /// Where it fell in, for the splash.
    pub splash: Option<Vec3>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    world: Res<VoxelWorld>,
    sites: Res<Sites>,
    game: Res<Game>,
    ui: Res<Ui>,
    waking: Res<crate::waking::Waking>,
    camera: Res<OrbitCamera>,
    mut shore: ResMut<Shore>,
    mut player: Single<(&mut Player, &mut Transform)>,
    mut jump_armed: Local<bool>,
) {
    let (player, transform) = &mut *player;
    let dt = time.delta_secs().min(1.0 / 20.0);
    // At an open station (or the beam's console) the avatar stays there: the keys are for the panel.
    // While the way wakes, the avatar waits too.
    let still = wants.wants_any_keyboard_input() || ui.at_console() || waking.playing();

    // Walk relative to where the camera looks.
    let mut wish = Vec3::ZERO;
    if !still {
        let forward = Vec3::new(-camera.yaw.sin(), 0.0, -camera.yaw.cos());
        let right = Vec3::new(forward.z, 0.0, -forward.x) * -1.0;
        for (key, direction) in [(KeyCode::KeyW, forward), (KeyCode::KeyS, -forward), (KeyCode::KeyD, right), (KeyCode::KeyA, -right)] {
            if keys.pressed(key) {
                wish += direction;
            }
        }
    }
    let speed = if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) { RUN_SPEED } else { WALK_SPEED };
    let horizontal = wish.normalize_or_zero() * speed;
    player.velocity.x = horizontal.x;
    player.velocity.z = horizontal.z;
    if horizontal != Vec3::ZERO {
        let facing = Quat::from_rotation_y(f32::atan2(-horizontal.x, -horizontal.z));
        transform.rotation = transform.rotation.slerp(facing, (dt * 12.0).min(1.0));
    }

    // The avatar falls with the planet's own gravity.
    player.velocity.y = (player.velocity.y - game.layout.g as f32 * dt).max(-40.0);
    // Space also starts the game from the menu: a press still held from there is not a jump.
    if !keys.pressed(KeyCode::Space) {
        *jump_armed = true;
    }
    if player.grounded {
        player.coyote = COYOTE_SECONDS;
    } else {
        player.coyote = (player.coyote - dt).max(0.0);
    }
    if !still && *jump_armed && keys.just_pressed(KeyCode::Space) {
        player.jump_buffer = JUMP_BUFFER_SECONDS;
    } else {
        player.jump_buffer = (player.jump_buffer - dt).max(0.0);
    }
    let can_jump = player.grounded || player.coyote > 0.0;
    let wants_jump = player.jump_buffer > 0.0 || (player.grounded && keys.pressed(KeyCode::Space));
    if can_jump && wants_jump && !still && *jump_armed {
        player.velocity.y = JUMP_SPEED;
        player.grounded = false;
        player.coyote = 0.0;
        player.jump_buffer = 0.0;
    }

    player.step_offset *= (-14.0 * dt).exp();
    if player.step_offset.abs() < 1e-4 {
        player.step_offset = 0.0;
    }

    let mut feet = transform.translation;
    for axis in [0, 2] {
        let mut next = feet;
        next[axis] += player.velocity[axis] * dt;
        if barred_by_water(&world, shore.dry, feet, next) {
            continue;
        }
        if !collides(&world, next) {
            feet = next;
            continue;
        }
        // Blocked: step up one block if there is room on top of it.
        let mut stepped = next;
        stepped.y = (feet.y + 1e-3).floor() + 1.0;
        if player.grounded && !collides(&world, stepped) && !collides(&world, feet.with_y(stepped.y)) {
            player.step_offset = (player.step_offset - (stepped.y - feet.y)).clamp(-1.5, 0.0);
            feet = stepped;
        }
    }
    let mut next = feet;
    next.y += player.velocity.y * dt;
    player.grounded = false;
    if collides(&world, next) {
        if player.velocity.y <= 0.0 {
            // Land on top of the block below.
            next.y = next.y.floor() + 1.0;
            player.grounded = true;
        } else {
            next.y = feet.y;
        }
        player.velocity.y = 0.0;
        if collides(&world, next) {
            next = feet;
        }
    }
    feet = next;

    // Something rose where the avatar stands (the ramp's steps, a block put down): stand on top of it.
    // The lowest free place straight above, so an overhang further up is not where it ends up.
    if collides(&world, feet)
        && let Some(lifted) = (1..=LIFT_OUT).map(|up| feet.with_y(feet.y.floor() + up as f32)).find(|p| !collides(&world, *p))
    {
        feet = lifted;
        player.velocity.y = 0.0;
    }
    // Into deep water: a splash, and the avatar is back on the bank where it last stood.
    if fallen_in(&world, shore.dry, feet, player.grounded) {
        shore.fallen_in += 1;
        shore.splash = Some(feet);
        feet = shore.dry;
        player.velocity = Vec3::ZERO;
        player.step_offset = 0.0;
        player.coyote = 0.0;
        player.jump_buffer = 0.0;
    }
    // Out of the world, or walled in: back to the start.
    if feet.y < 1.0 || collides(&world, feet) {
        feet = sites.spawn;
        player.velocity = Vec3::ZERO;
        player.step_offset = 0.0;
        player.coyote = 0.0;
        player.jump_buffer = 0.0;
    }
    if player.grounded && !over_deep_water(&world, feet) {
        shore.dry = feet;
    }
    transform.translation = feet;
}

#[allow(clippy::too_many_arguments)]
fn orbit_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    wants: Res<EguiWantsInput>,
    world: Res<VoxelWorld>,
    ui: Res<Ui>,
    ruler: Res<Ruler>,
    waking: Res<Waking>,
    mut orbit: ResMut<OrbitCamera>,
    window: Single<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
    player: Single<(&Player, &Transform), Without<Camera3d>>,
    mut camera: Single<&mut Transform, With<Camera3d>>,
) {
    let dt = time.delta_secs();
    #[allow(unused_mut)]
    let (mut win, mut cursor) = window.into_inner();
    let panel_open = ui.notebook_open || ui.handbook_open || ui.tutor_open || ui.controls_open;
    let want_free_look = !scripted_run() && can_free_look(win.focused, ui.at_console(), panel_open, ruler.in_use(), waking.playing());
    if want_free_look && !orbit.free_look {
        orbit.lock_settle = 2;
    } else if !want_free_look {
        orbit.lock_settle = 0;
    }
    orbit.free_look = want_free_look;

    let locked_grab = if cfg!(target_os = "windows") { CursorGrabMode::Confined } else { CursorGrabMode::Locked };
    let want_grab = if orbit.free_look { locked_grab } else { CursorGrabMode::None };
    let want_visible = !orbit.free_look;
    if cursor.grab_mode != want_grab {
        cursor.grab_mode = want_grab;
    }
    if cursor.visible != want_visible {
        cursor.visible = want_visible;
    }
    let settling = orbit.lock_settle > 0;
    if orbit.lock_settle > 0 {
        orbit.lock_settle -= 1;
    }
    #[cfg(target_os = "windows")]
    if orbit.free_look && !settling {
        // On Windows, `winit` confines rather than locks the cursor; pin the hidden pointer to the
        // window centre so it never drifts onto HUD widgets and starts centered when a panel opens.
        let center = win.size() / 2.0;
        if win.cursor_position().is_none_or(|p| (p - center).length_squared() > 16.0) {
            win.set_cursor_position(Some(center));
        }
    }

    // A side view may stand further back than the player zooms out, to fit its whole test.
    let furthest = if orbit.side_view && orbit.watch.is_some() { SIDE_VIEW_FURTHEST } else { FURTHEST };
    let mut rotated = false;
    let mut zoomed = false;
    if orbit.free_look {
        if !settling && motion.delta != Vec2::ZERO {
            orbit.yaw -= motion.delta.x * 0.0035;
            orbit.pitch += motion.delta.y * 0.0035;
            rotated = true;
        }
        if scroll.delta.y != 0.0 {
            orbit.distance = (orbit.distance * (1.0 - scroll.delta.y * 0.1)).clamp(NEAREST, furthest);
            zoomed = true;
        }
    } else if !wants.wants_any_pointer_input() {
        let left_orbit = !ruler.active && buttons.pressed(MouseButton::Left);
        if (left_orbit || buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Middle)) && motion.delta != Vec2::ZERO {
            orbit.yaw -= motion.delta.x * 0.005;
            orbit.pitch += motion.delta.y * 0.005;
            rotated = true;
        }
        if scroll.delta.y != 0.0 {
            orbit.distance = (orbit.distance * (1.0 - scroll.delta.y * 0.1)).clamp(NEAREST, furthest);
            zoomed = true;
        }
    }
    if !wants.wants_any_keyboard_input() {
        let turn = keys.pressed(KeyCode::ArrowLeft) as i32 - keys.pressed(KeyCode::ArrowRight) as i32;
        let tilt = keys.pressed(KeyCode::ArrowUp) as i32 - keys.pressed(KeyCode::ArrowDown) as i32;
        if turn != 0 || tilt != 0 {
            orbit.yaw += turn as f32 * 1.8 * dt;
            orbit.pitch += tilt as f32 * 1.2 * dt;
            rotated = true;
        }
    }
    if rotated {
        orbit.manual_rotate();
    }
    if zoomed {
        orbit.manual_zoom();
    }

    let blend = (dt * 6.0).min(1.0);
    // Back to the player's view, or round to show something or a station's puzzle: slower, so the
    // eye can follow. Yielded immediately above whenever the player turns the camera themselves.
    let easing = orbit.returning.map(|view| (view, blend)).or(orbit.turning.map(|view| (view, (dt * 2.5).min(1.0))));
    if let Some(((yaw, pitch, distance), blend)) = easing {
        // The shorter way round.
        let turn = (yaw - orbit.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        orbit.yaw += turn * blend;
        orbit.pitch += (pitch - orbit.pitch) * blend;
        orbit.distance += (distance - orbit.distance) * blend;
        if turn.abs() < 1e-3 && (pitch - orbit.pitch).abs() < 1e-3 && (distance - orbit.distance).abs() < 1e-2 {
            (orbit.yaw, orbit.pitch, orbit.distance) = (yaw, pitch, distance);
            orbit.returning = None;
            orbit.turning = None;
        }
    } else if let Some(distance) = orbit.returning_distance {
        orbit.distance += (distance - orbit.distance) * blend;
        if (distance - orbit.distance).abs() < 1e-2 {
            orbit.distance = distance;
            orbit.returning_distance = None;
        }
    }
    orbit.pitch = orbit.pitch.clamp(-0.3, 1.45);

    let (player_state, player_transform) = *player;
    let target = orbit.watch.unwrap_or(player_transform.translation + Vec3::Y * (1.5 + player_state.step_offset));
    orbit.focus = orbit.focus.lerp(target, blend);

    // A focus that is itself inside terrain (a fixed test view can be) is left alone.
    let open_focus = !world.is_solid(orbit.focus.floor().as_ivec3());
    let back = orbit.back();
    // Pull the camera in if terrain is between it and what it looks at.
    let reach = match world.raycast(orbit.focus, back, orbit.distance) {
        Some(hit) if open_focus => (hit.distance - 0.4).max(1.0),
        _ => orbit.distance,
    };
    camera.translation = orbit.focus + back * reach;
    camera.look_at(orbit.focus, Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_avatar_wades_the_shallows_and_falls_into_deep_water() {
        use crate::island::{GROUND, SEA_LEVEL};
        use physics_core::materials::Material;
        // A bank at the ground's height, a shelf the water barely covers, deep water, and a far bank.
        let mut world = VoxelWorld::new(IVec3::new(6, 32, 1));
        let shallow = SEA_LEVEL.floor() as i32;
        world.fill(IVec3::ZERO, IVec3::new(0, GROUND - 1, 0), Material::Rock);
        world.fill(IVec3::new(1, 0, 0), IVec3::new(1, shallow - 1, 0), Material::Sand);
        world.fill(IVec3::new(2, 0, 0), IVec3::new(4, 2, 0), Material::Rock);
        world.fill(IVec3::new(5, 0, 0), IVec3::new(5, GROUND - 1, 0), Material::Rock);
        let at = |x: f32, y: i32| Vec3::new(x, y as f32, 0.5);
        assert!(!over_deep_water(&world, at(1.5, shallow)), "the shallows can be waded");
        assert!(over_deep_water(&world, at(2.5, 3)));

        // It may step out over deep water, from the shallows or the top of a bank.
        let shelf = at(1.9, shallow);
        assert!(!barred_by_water(&world, shelf, shelf, at(2.1, shallow)));
        // Wading, it is in the water already, so a step off the shelf takes it under.
        assert!(fallen_in(&world, shelf, at(2.1, shallow), true), "a step off the shelf takes it under");
        // At the top of a bank over deep water, it isn't in yet, and it may step back.
        let mut cliff = VoxelWorld::new(IVec3::new(2, 32, 1));
        cliff.fill(IVec3::ZERO, IVec3::new(0, GROUND - 1, 0), Material::Rock);
        let bank = at(0.9, GROUND);
        assert!(!barred_by_water(&cliff, bank, bank, at(1.1, GROUND)));
        assert!(!fallen_in(&cliff, bank, at(1.1, GROUND), true), "it stands at the edge, not in the water");
        assert!(!barred_by_water(&cliff, bank, at(1.1, GROUND), bank), "it may step back the way it came");
        assert!(!fallen_in(&cliff, bank, at(1.3, GROUND - 4), false), "falling, it isn't in yet");
        assert!(fallen_in(&cliff, bank, at(1.3, shallow), false), "at the water, it is");
        // A jump from the bank doesn't carry it across to the far bank.
        assert!(barred_by_water(&world, bank, at(4.9, GROUND + 1), at(5.1, GROUND + 1)), "it can't jump across to the far bank");
        // Held up by the lip of the far bank, it falls in all the same.
        assert!(fallen_in(&world, bank, at(4.8, GROUND), true), "the far bank's lip doesn't hold it up");
        // Nor over a strip of deep water a block wide.
        let mut strip = VoxelWorld::new(IVec3::new(3, 32, 1));
        strip.fill(IVec3::ZERO, IVec3::new(0, GROUND - 1, 0), Material::Rock);
        strip.fill(IVec3::new(2, 0, 0), IVec3::new(2, GROUND - 1, 0), Material::Rock);
        let edge = at(0.95, GROUND);
        assert!(barred_by_water(&strip, edge, at(1.95, GROUND + 1), at(2.05, GROUND + 1)), "it can't jump a strip of deep water");
    }

    #[test]
    fn facing_a_place_puts_it_ahead_and_the_camera_behind() {
        let from = Vec3::new(40.0, 20.0, 170.0);
        for to in [Vec3::new(108.0, 20.0, 28.0), Vec3::new(-112.0, 15.0, -328.0), Vec3::new(0.0, 20.0, 200.0)] {
            let mut orbit = OrbitCamera::default();
            orbit.face(from, to, 0.12, 9.0);
            let (yaw, ..) = orbit.turning.unwrap();
            // Walking forward, as `move_player` reads the yaw, heads for the place.
            let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
            assert!(forward.dot((to - from).with_y(0.0).normalize()) > 0.999, "{to}");
            // And the camera stands on the far side of the avatar from it.
            orbit.yaw = yaw;
            assert!(orbit.back().with_y(0.0).normalize().dot(forward) < -0.999);
        }
        // The player's own view comes back afterwards.
        let mut orbit = OrbitCamera::default();
        let own = (orbit.yaw, orbit.pitch, orbit.distance);
        orbit.face(from, Vec3::ZERO, 0.12, 9.0);
        orbit.ease_home();
        assert_eq!(orbit.returning, Some(own));
        assert_eq!(orbit.turning, None);
    }

    #[test]
    fn free_look_engages_only_when_exploring_with_focus() {
        assert!(can_free_look(true, false, false, false, false));
        assert!(!can_free_look(false, false, false, false, false), "unfocused window releases cursor");
        assert!(!can_free_look(true, true, false, false, false), "open station releases cursor");
        assert!(!can_free_look(true, false, true, false, false), "open dock panel releases cursor");
        assert!(!can_free_look(true, false, false, true, false), "ruler in use releases cursor");
        assert!(!can_free_look(true, false, false, false, true), "waking scene releases cursor");
    }

    #[test]
    fn manual_rotation_cancels_camera_glides_without_fighting_input() {
        // During a story look (exploring, no watch), manual rotation stops both the turn and any
        // delayed snap-back to the old saved angle.
        let mut orbit = OrbitCamera::default();
        orbit.face(Vec3::ZERO, Vec3::new(20.0, 0.0, 0.0), 0.12, 9.0);
        assert!(orbit.turning.is_some());
        assert!(orbit.saved.is_some());
        orbit.manual_rotate();
        assert_eq!(orbit.turning, None);
        assert_eq!(orbit.saved, None);

        // Leaving a wide station side view: manual rotation cancels yaw/pitch snap-back while
        // keeping the distance return glide until the player scrolls.
        let mut orbit = OrbitCamera { distance: 14.0, ..OrbitCamera::default() };
        orbit.frame_side_view(Vec3::ZERO, Vec3::new(30.0, 10.0, 0.0), Vec3::X, true);
        // Rotating while the station is open stops the side-view turn but keeps `saved` for exit.
        orbit.manual_rotate();
        assert_eq!(orbit.turning, None);
        assert!(orbit.saved.is_some());
        orbit.ease_home();
        assert!(orbit.returning.is_some());
        orbit.manual_rotate();
        assert_eq!(orbit.returning, None);
        assert_eq!(orbit.returning_distance, Some(14.0));
        orbit.manual_zoom();
        assert_eq!(orbit.returning_distance, None);
    }
}
