//! The avatar and the camera that follows it.

use crate::island::Sites;
use crate::state::{Game, Mode, Ui};
use crate::voxel::VoxelWorld;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;

/// Half-width and height of the avatar's collision box. Its transform is at its feet.
const HALF_WIDTH: f32 = 0.3;
const HEIGHT: f32 = 1.8;
/// How many blocks the avatar is lifted to stand on something that rose where it was standing.
const LIFT_OUT: i32 = 16;
pub(crate) const WALK_SPEED: f32 = 5.0;
pub(crate) const RUN_SPEED: f32 = 9.0;
/// Take-off speed of a jump. The height reached depends on the planet: v^2 / (2 g).
pub(crate) const JUMP_SPEED: f32 = 6.0;

#[derive(Component, Default)]
pub struct Player {
    pub velocity: Vec3,
    pub grounded: bool,
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
    /// The yaw, pitch and distance the camera is gliding to: the player's own after a wake or a
    /// station (then `saved` is empty), or a station's view.
    easing: Option<(f32, f32, f32)>,
    /// The camera is on a side view (a test, or a station's puzzle), which may stand further back
    /// than the player zooms out.
    side_view: bool,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        OrbitCamera { yaw: -0.6, pitch: 0.45, distance: 14.0, watch: None, focus: Vec3::ZERO, saved: None, easing: None, side_view: false }
    }
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
        self.keep_home();
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
            self.easing = Some(view);
        } else {
            (self.yaw, self.pitch, self.distance) = view;
            self.easing = None;
        }
        let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
        self.side_view = true;
        self.watch = Some((min + max) / 2.0 + right * 0.42 * half_screen_width * distance);
    }

    /// Keep the player's own view, the first time something borrows the camera. If the camera is
    /// still gliding home, home is where it was going.
    fn keep_home(&mut self) {
        if self.saved.is_none() {
            self.saved = Some(self.easing.take().unwrap_or((self.yaw, self.pitch, self.distance)));
        }
    }

    /// Take the camera to watch something with this pitch and distance (set `watch` to say what),
    /// keeping the player's own view to come back with [`OrbitCamera::release_view`].
    pub fn borrow(&mut self, pitch: f32, distance: f32) {
        self.keep_home();
        self.easing = None;
        self.side_view = false;
        self.pitch = pitch;
        self.distance = distance;
    }

    /// Set the yaw, pitch and distance outright, cancelling any glide (the scripts' `Look`).
    pub fn set_view(&mut self, yaw: f32, pitch: f32, distance: f32) {
        (self.yaw, self.pitch, self.distance) = (yaw, pitch, distance);
        self.easing = None;
    }

    pub fn release_view(&mut self) {
        self.watch = None;
        self.easing = None;
        self.side_view = false;
        if let Some((yaw, pitch, distance)) = self.saved.take() {
            (self.yaw, self.pitch, self.distance) = (yaw, pitch, distance);
        }
    }

    /// Like [`OrbitCamera::release_view`], but the camera glides back to the player's own view over
    /// a moment instead of cutting to it.
    pub fn ease_home(&mut self) {
        self.watch = None;
        self.side_view = false;
        if let Some(home) = self.saved.take() {
            self.easing = Some(home);
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

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OrbitCamera>()
            .add_systems(Startup, spawn)
            .add_systems(Update, (move_player, orbit_camera).chain().run_if(in_state(Mode::Playing)));
    }
}

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sites: Res<Sites>, mut camera: ResMut<OrbitCamera>) {
    let player = commands.spawn((Player::default(), Transform::from_translation(sites.spawn), Visibility::default())).id();
    crate::avatar::attach(&mut commands, player, &mut meshes, &mut materials);
    camera.focus = sites.spawn + Vec3::Y * 1.5;
}

fn collides(world: &VoxelWorld, feet: Vec3) -> bool {
    world.box_collides(feet - Vec3::new(HALF_WIDTH, 0.0, HALF_WIDTH), feet + Vec3::new(HALF_WIDTH, HEIGHT, HALF_WIDTH))
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
    mut player: Single<(&mut Player, &mut Transform)>,
) {
    let (player, transform) = &mut *player;
    let dt = time.delta_secs().min(1.0 / 20.0);
    // At an open station the avatar stays at the console: the keys are for the panel.
    // While the way wakes, the avatar waits too.
    let still = wants.wants_any_keyboard_input() || ui.station.is_some() || waking.playing();

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
    let speed = if keys.pressed(KeyCode::ShiftLeft) { RUN_SPEED } else { WALK_SPEED };
    let horizontal = wish.normalize_or_zero() * speed;
    player.velocity.x = horizontal.x;
    player.velocity.z = horizontal.z;
    if horizontal != Vec3::ZERO {
        let facing = Quat::from_rotation_y(f32::atan2(-horizontal.x, -horizontal.z));
        transform.rotation = transform.rotation.slerp(facing, (dt * 12.0).min(1.0));
    }

    // The avatar falls with the planet's own gravity.
    player.velocity.y = (player.velocity.y - game.layout.g as f32 * dt).max(-40.0);
    if player.grounded && !still && keys.pressed(KeyCode::Space) {
        player.velocity.y = JUMP_SPEED;
    }

    let mut feet = transform.translation;
    for axis in [0, 2] {
        let mut next = feet;
        next[axis] += player.velocity[axis] * dt;
        if !collides(&world, next) {
            feet = next;
            continue;
        }
        // Blocked: step up one block if there is room on top of it.
        let mut stepped = next;
        stepped.y = (feet.y + 1e-3).floor() + 1.0;
        if player.grounded && !collides(&world, stepped) && !collides(&world, feet.with_y(stepped.y)) {
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
    // Out of the world, or walled in: back to the start.
    if feet.y < 1.0 || collides(&world, feet) {
        feet = sites.spawn;
        player.velocity = Vec3::ZERO;
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
    mut orbit: ResMut<OrbitCamera>,
    player: Single<&Transform, (With<Player>, Without<Camera3d>)>,
    mut camera: Single<&mut Transform, With<Camera3d>>,
) {
    let dt = time.delta_secs();
    let blend = (dt * 6.0).min(1.0);
    if let Some((yaw, pitch, distance)) = orbit.easing {
        // The shorter way round to the yaw it is gliding to.
        let turn = (yaw - orbit.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        orbit.yaw += turn * blend;
        orbit.pitch += (pitch - orbit.pitch) * blend;
        orbit.distance += (distance - orbit.distance) * blend;
        if turn.abs() < 1e-3 && (pitch - orbit.pitch).abs() < 1e-3 && (distance - orbit.distance).abs() < 1e-2 {
            (orbit.yaw, orbit.pitch, orbit.distance) = (yaw, pitch, distance);
            orbit.easing = None;
        }
    }
    if !wants.wants_any_pointer_input() {
        if buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Middle) {
            orbit.yaw -= motion.delta.x * 0.005;
            orbit.pitch += motion.delta.y * 0.005;
        }
        // A side view may stand further back than the player zooms out, to fit its whole test.
        let furthest = if orbit.side_view && orbit.watch.is_some() { SIDE_VIEW_FURTHEST } else { FURTHEST };
        orbit.distance = (orbit.distance * (1.0 - scroll.delta.y * 0.1)).clamp(NEAREST, furthest);
    }
    if !wants.wants_any_keyboard_input() {
        let turn = keys.pressed(KeyCode::ArrowLeft) as i32 - keys.pressed(KeyCode::ArrowRight) as i32;
        let tilt = keys.pressed(KeyCode::ArrowUp) as i32 - keys.pressed(KeyCode::ArrowDown) as i32;
        orbit.yaw += turn as f32 * 1.8 * dt;
        orbit.pitch += tilt as f32 * 1.2 * dt;
    }
    orbit.pitch = orbit.pitch.clamp(-0.3, 1.45);

    let target = orbit.watch.unwrap_or(player.translation + Vec3::Y * 1.5);
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
