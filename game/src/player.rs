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
    /// The player's yaw, pitch and distance the camera is easing back to, after a wake or a station.
    returning: Option<(f32, f32, f32)>,
    /// The yaw, pitch and distance the camera is turning to: to show something past the avatar, or a
    /// station's puzzle.
    turning: Option<(f32, f32, f32)>,
    /// The camera is on a side view (a test, or a station's puzzle), which may stand further back
    /// than the player zooms out.
    side_view: bool,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        OrbitCamera { yaw: -0.6, pitch: 0.45, distance: 14.0, watch: None, focus: Vec3::ZERO, saved: None, returning: None, turning: None, side_view: false }
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
            self.saved = Some(self.returning.take().unwrap_or((self.yaw, self.pitch, self.distance)));
        }
        self.turning = None;
    }

    /// Set the yaw, pitch and distance outright, cancelling any glide (the scripts' `Look`).
    pub fn set_view(&mut self, yaw: f32, pitch: f32, distance: f32) {
        (self.yaw, self.pitch, self.distance) = (yaw, pitch, distance);
        self.returning = None;
        self.turning = None;
    }

    pub fn release_view(&mut self) {
        self.watch = None;
        self.returning = None;
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
        self.side_view = false;
        self.returning = self.saved.take();
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

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sites: Res<Sites>, mut camera: ResMut<OrbitCamera>) {
    let player = commands.spawn((Player::default(), Transform::from_translation(sites.spawn), Visibility::default())).id();
    crate::avatar::attach(&mut commands, player, &mut meshes, &mut materials);
    camera.focus = sites.spawn + Vec3::Y * 1.5;
}

fn collides(world: &VoxelWorld, feet: Vec3) -> bool {
    world.box_collides(feet - Vec3::new(HALF_WIDTH, 0.0, HALF_WIDTH), feet + Vec3::new(HALF_WIDTH, HEIGHT, HALF_WIDTH))
}

/// Whether a step from `from` to `to` would take the avatar into water too deep to wade, from
/// somewhere that is not: the avatar stops at the water's edge, or at the top of a bank over deep
/// water, as at a wall. Out of deep water it may always go.
pub(crate) fn into_deep_water(world: &VoxelWorld, from: Vec3, to: Vec3) -> bool {
    let deep = |p: Vec3| crate::island::too_deep(world.surface(p.x.floor() as i32, p.z.floor() as i32));
    deep(to) && !deep(from)
}

/// Whether deep water has stopped the avatar this session, which Lumen remarks on once.
#[derive(Resource, Default)]
pub struct Shore {
    pub stopped: bool,
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
        if into_deep_water(&world, feet, next) {
            if !shore.stopped {
                shore.stopped = true;
            }
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
    // Back to the player's view, or round to show something or a station's puzzle: slower, so the
    // eye can follow.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_avatar_wades_the_shallows_and_stops_at_deep_water() {
        use crate::island::{GROUND, SEA_LEVEL};
        use physics_core::materials::Material;
        // A bank at the ground's height, then a shelf the water barely covers, then deep water.
        let mut world = VoxelWorld::new(IVec3::new(3, 32, 1));
        let shallow = SEA_LEVEL.floor() as i32;
        world.fill(IVec3::ZERO, IVec3::new(0, GROUND - 1, 0), Material::Rock);
        world.fill(IVec3::new(1, 0, 0), IVec3::new(1, shallow - 1, 0), Material::Sand);
        world.fill(IVec3::new(2, 0, 0), IVec3::new(2, 2, 0), Material::Rock);
        let at = |x: i32, y: i32| Vec3::new(x as f32 + 0.5, y as f32, 0.5);
        assert!(!into_deep_water(&world, at(0, GROUND), at(1, shallow)), "the shallows can be waded");
        assert!(into_deep_water(&world, at(1, shallow), at(2, 3)), "deep water stops the avatar");
        assert!(into_deep_water(&world, at(0, GROUND), at(2, 3)), "so does the top of a bank over it");
        assert!(!into_deep_water(&world, at(2, 3), at(1, shallow)), "out of deep water it may always go");
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
}
