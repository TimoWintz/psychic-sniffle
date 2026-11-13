//! The avatar and the camera that follows it.

use crate::island::Sites;
use crate::state::Game;
use crate::voxel::VoxelWorld;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;

/// Half-width and height of the avatar's collision box. Its transform is at its feet.
const HALF_WIDTH: f32 = 0.3;
const HEIGHT: f32 = 1.8;
const WALK_SPEED: f32 = 5.0;
const RUN_SPEED: f32 = 9.0;
/// Take-off speed of a jump. The height reached depends on the planet: v^2 / (2 g).
const JUMP_SPEED: f32 = 6.0;

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
    /// The player's yaw, pitch and distance while a test has borrowed the camera.
    saved: Option<(f32, f32, f32)>,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        OrbitCamera { yaw: -0.6, pitch: 0.45, distance: 14.0, watch: None, focus: Vec3::ZERO, saved: None }
    }
}

/// Tangent of half the camera's vertical field of view (Bevy's default is 45 degrees).
const TAN_HALF_FOV: f32 = 0.4142;
const ASPECT: f32 = 16.0 / 9.0;

impl OrbitCamera {
    /// Turn to a side view that shows the box from `min` to `max`, with `along` running left to
    /// right. The box is placed in the left part of the screen, clear of the station panel. The
    /// player's own view is kept and comes back with [`OrbitCamera::release_view`].
    pub fn frame_side_view(&mut self, min: Vec3, max: Vec3, along: Vec3) {
        if self.saved.is_none() {
            self.saved = Some((self.yaw, self.pitch, self.distance));
        }
        let size = max - min;
        let width = size.with_y(0.0).length() + 8.0;
        let height = size.y + 6.0;
        let half_screen_width = ASPECT * TAN_HALF_FOV;
        // The box may use the left half of the screen's width and most of its height.
        self.distance = (width / half_screen_width).max(height / (1.4 * TAN_HALF_FOV)).max(18.0);
        self.yaw = f32::atan2(-along.z, along.x);
        self.pitch = 0.18;
        let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
        self.watch = Some((min + max) / 2.0 + right * 0.42 * half_screen_width * self.distance);
    }

    pub fn release_view(&mut self) {
        self.watch = None;
        if let Some((yaw, pitch, distance)) = self.saved.take() {
            (self.yaw, self.pitch, self.distance) = (yaw, pitch, distance);
        }
    }
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OrbitCamera>()
            .add_systems(Startup, spawn)
            .add_systems(Update, (move_player, orbit_camera).chain());
    }
}

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sites: Res<Sites>, mut camera: ResMut<OrbitCamera>) {
    let suit = materials.add(StandardMaterial { base_color: Color::srgb(0.85, 0.45, 0.12), perceptual_roughness: 0.8, ..default() });
    let dark = materials.add(StandardMaterial { base_color: Color::srgb(0.16, 0.18, 0.22), perceptual_roughness: 0.8, ..default() });
    let visor = materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.8, 0.95), perceptual_roughness: 0.2, ..default() });
    let mut part = |size: Vec3, centre: Vec3, material: &Handle<StandardMaterial>| {
        (Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(material.clone()), Transform::from_translation(centre))
    };
    commands
        .spawn((Player::default(), Transform::from_translation(sites.spawn), Visibility::default()))
        .with_children(|body| {
            body.spawn(part(Vec3::new(0.22, 0.8, 0.26), Vec3::new(-0.14, 0.4, 0.0), &dark));
            body.spawn(part(Vec3::new(0.22, 0.8, 0.26), Vec3::new(0.14, 0.4, 0.0), &dark));
            body.spawn(part(Vec3::new(0.56, 0.62, 0.32), Vec3::new(0.0, 1.11, 0.0), &suit));
            body.spawn(part(Vec3::new(0.16, 0.6, 0.2), Vec3::new(-0.37, 1.1, 0.0), &suit));
            body.spawn(part(Vec3::new(0.16, 0.6, 0.2), Vec3::new(0.37, 1.1, 0.0), &suit));
            body.spawn(part(Vec3::new(0.4, 0.38, 0.4), Vec3::new(0.0, 1.61, 0.0), &suit));
            // The visor marks the front (-Z).
            body.spawn(part(Vec3::new(0.3, 0.16, 0.06), Vec3::new(0.0, 1.64, -0.2), &visor));
            body.spawn(part(Vec3::new(0.34, 0.42, 0.14), Vec3::new(0.0, 1.15, 0.23), &dark));
        });
    camera.focus = sites.spawn + Vec3::Y * 1.5;
}

fn collides(world: &VoxelWorld, feet: Vec3) -> bool {
    world.box_collides(feet - Vec3::new(HALF_WIDTH, 0.0, HALF_WIDTH), feet + Vec3::new(HALF_WIDTH, HEIGHT, HALF_WIDTH))
}

#[allow(clippy::too_many_arguments)]
fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    world: Res<VoxelWorld>,
    sites: Res<Sites>,
    game: Res<Game>,
    camera: Res<OrbitCamera>,
    mut player: Single<(&mut Player, &mut Transform)>,
) {
    let (player, transform) = &mut *player;
    let dt = time.delta_secs().min(1.0 / 20.0);
    let typing = wants.wants_any_keyboard_input();

    // Walk relative to where the camera looks.
    let mut wish = Vec3::ZERO;
    if !typing {
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
    if player.grounded && !typing && keys.pressed(KeyCode::Space) {
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

    // Out of the world: back to the start.
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
    if !wants.wants_any_pointer_input() {
        if buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Middle) {
            orbit.yaw -= motion.delta.x * 0.005;
            orbit.pitch += motion.delta.y * 0.005;
        }
        orbit.distance = (orbit.distance * (1.0 - scroll.delta.y * 0.1)).clamp(3.0, 80.0);
    }
    if !wants.wants_any_keyboard_input() {
        let turn = keys.pressed(KeyCode::ArrowLeft) as i32 - keys.pressed(KeyCode::ArrowRight) as i32;
        let tilt = keys.pressed(KeyCode::ArrowUp) as i32 - keys.pressed(KeyCode::ArrowDown) as i32;
        orbit.yaw += turn as f32 * 1.8 * dt;
        orbit.pitch += tilt as f32 * 1.2 * dt;
    }
    orbit.pitch = orbit.pitch.clamp(-0.3, 1.45);

    let target = orbit.watch.unwrap_or(player.translation + Vec3::Y * 1.5);
    let blend = (dt * 6.0).min(1.0);
    orbit.focus = orbit.focus.lerp(target, blend);

    let back = Quat::from_euler(EulerRot::YXZ, orbit.yaw, -orbit.pitch, 0.0) * Vec3::Z;
    // Pull the camera in if terrain is between it and what it looks at.
    // A focus that is itself inside terrain (a fixed test view can be) is left alone.
    let reach = match world.raycast(orbit.focus, back, orbit.distance) {
        Some(hit) if !world.is_solid(orbit.focus.floor().as_ivec3()) => (hit.distance - 0.4).max(1.0),
        _ => orbit.distance,
    };
    camera.translation = orbit.focus + back * reach;
    camera.look_at(orbit.focus, Vec3::Y);
}
