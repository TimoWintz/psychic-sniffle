//! The builders' skiff, which lies asleep on its slipway on the north shore, and what the way
//! carries towards it: the launchers' sight, which rides the crawler until it is caught. (The helm's
//! own journey to the skiff, in its casket across the channel and then over the ridge, is `sim`'s.)
//!
//! - The haul track: the sight sits on the crawler, then at the foot of the transfer crane.
//! - Local gravity: calibrated, the sight is fitted beside the ledge launcher.
//! - The ridge (the chapter's capstone): the tiller lands in the circle on the quay, and once the
//!   north gate has woken the skiff takes it: it settles into the socket at the stern, and the helm's
//!   quartz lights. The rest of the skiff stays dark: its power, hull and drive are later chapters'.

use crate::island::Sites;
use crate::sim::TILLER_SIZE;
use crate::state::Mode;
use crate::waking::{MachineTrim, Waking, beam_shines};
use bevy::prelude::*;
use physics_core::materials::Material;

/// The skiff's length along the slipway, and its half-beam amidships at the gunwale.
const LENGTH: f32 = 14.0;
const HALF_BEAM: f32 = 2.4;
/// The top of the floor, and of the sides amidships, above the slipway's stone.
const FLOOR: f32 = 1.3;
const GUNWALE: f32 = 2.7;
/// How far the sides lean outward from upright, in radians.
const FLARE: f32 = 0.35;
/// Where the helm's socket is, in the skiff's own frame (+x is the bow, down the slipway).
const SOCKET: Vec3 = Vec3::new(-LENGTH / 2.0 + 0.9, GUNWALE + 0.6, 0.0);

/// The tiller, delivered to the landing circle: the skiff takes it once the north gate wakes.
#[derive(Component)]
pub struct Delivered;

/// The skiff, and the quartz of its helm: its socket, and the tiller once it is in.
#[derive(Component)]
struct Skiff;

#[derive(Resource)]
struct HelmQuartz(Handle<StandardMaterial>);

/// The tiller in the skiff's stern, settling into its socket: seconds since it started.
#[derive(Component)]
struct Settling(f32);
const SETTLE: f32 = 1.6;

/// The launchers' sight. One rides on the crawler (a child of it, so it goes along in
/// experiments and tests) until the crane lifts it off; the other is the same sight once it is off.
#[derive(Component)]
pub struct Sight {
    pub riding: bool,
}

pub const SIGHT_SIZE: Vec3 = Vec3::new(0.7, 0.4, 0.55);

/// The sight's look: a block of the builders' pale stone with a lens of their quartz on top, which
/// glows whatever the machines around it are doing.
pub fn sight_parts(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> [(Mesh3d, MeshMaterial3d<StandardMaterial>, Transform); 2] {
    let stone = crate::waking::stone(materials);
    let q = Material::Quartz.properties().colour;
    let glow = materials.add(StandardMaterial { base_color: Color::srgb(q[0], q[1], q[2]), unlit: true, ..default() });
    [
        (Mesh3d(meshes.add(Cuboid::from_size(SIGHT_SIZE))), MeshMaterial3d(stone), Transform::default()),
        (Mesh3d(meshes.add(Cuboid::new(0.28, 0.06, 0.28))), MeshMaterial3d(glow), Transform::from_xyz(0.0, SIGHT_SIZE.y / 2.0 + 0.03, 0.0)),
    ]
}

pub struct VesselPlugin;

impl Plugin for VesselPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_skiff).add_systems(Update, ((take_helm, place_sight).run_if(in_state(Mode::Playing)), settle_helm));
    }
}

/// The skiff's stone: a weathered shade of the builders' pale stone.
fn hull_stone(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial { base_color: Color::srgb(0.80, 0.76, 0.67), perceptual_roughness: 0.9, ..default() })
}

/// Half the skiff's beam at the gunwale `x` along it: widest amidships, narrowing to the stern and
/// more sharply to the bow.
fn half_beam(x: f32) -> f32 {
    let u = x / (LENGTH / 2.0);
    HALF_BEAM * (1.0 - 0.7 * u * u).max(0.15) * if x < 0.0 { 1.0 } else { 1.0 - 0.6 * u * u }
}

/// The top of the sides at `x`: rising towards the bow, and a little towards the stern.
fn sheer(x: f32) -> f32 {
    let u = x / (LENGTH / 2.0);
    GUNWALE + if x > 0.0 { 1.0 } else { 0.5 } * u * u
}

/// Half the skiff's width at the floor, where the sides meet it: the sides lean out from there to
/// the gunwale.
fn bottom(x: f32) -> f32 {
    (half_beam(x) - (sheer(x) - FLOOR) * FLARE.tan()).max(0.12)
}

/// The skiff, built along x and turned to lie along the slipway, which runs north to the strait
/// (its bow towards the water): a stone cradle, a keel, a floor and two sides of stone in segments,
/// a stern seat with the helm's empty socket, and quartz along the gunwales that sleeps.
fn spawn_skiff(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sites: Res<Sites>) {
    let stone = crate::waking::stone(&mut materials);
    let hull = hull_stone(&mut materials);
    let dark = materials.add(MachineTrim::look(false));
    let helm = HelmQuartz(materials.add(MachineTrim::look(false)));
    let along_slipway = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    // The sides, in planks along the skiff's line, each leaning outward from the floor.
    const PLANKS: i32 = 20;
    let step = LENGTH / PLANKS as f32;
    let mut planks = Vec::new();
    for i in 0..PLANKS {
        let (xa, xb) = (-LENGTH / 2.0 + step * i as f32, -LENGTH / 2.0 + step * (i + 1) as f32);
        let height = (sheer((xa + xb) / 2.0) - FLOOR) / FLARE.cos();
        for side in [-1.0_f32, 1.0] {
            let (a, b) = (Vec2::new(xa, side * bottom(xa)), Vec2::new(xb, side * bottom(xb)));
            let along = b - a;
            let turn = Quat::from_rotation_y(f32::atan2(-along.y, along.x)) * Quat::from_rotation_x(side * FLARE);
            let foot = Vec3::new((a.x + b.x) / 2.0, FLOOR, (a.y + b.y) / 2.0);
            let size = Vec3::new(along.length() + 0.06, height, 0.24);
            let at = Transform::from_translation(foot + turn * Vec3::Y * height / 2.0).with_rotation(turn);
            planks.push((at, height, meshes.add(Cuboid::from_size(size)), meshes.add(Cuboid::new(size.x * 0.85, 0.07, 0.14))));
        }
    }
    let mut part = |size: Vec3, at: Vec3, material: &Handle<StandardMaterial>| {
        (Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(material.clone()), Transform::from_translation(at))
    };
    commands.spawn((Skiff, Transform::from_translation(sites.slipway).with_rotation(along_slipway), Visibility::default())).with_children(|skiff| {
        // The cradle: two stone blocks the keel rests in.
        for x in [-4.0, 4.0] {
            skiff.spawn(part(Vec3::new(0.9, 0.8, 3.2), Vec3::new(x, 0.4, 0.0), &stone));
        }
        // The keel, then the hull in planks: a floor, and on each hand a side that follows the
        // skiff's line, leans outward and rises towards the bow and the stern.
        skiff.spawn(part(Vec3::new(LENGTH, 0.5, 0.6), Vec3::new(0.0, FLOOR - 0.25, 0.0), &hull));
        for i in 0..PLANKS {
            let (xa, xb) = (-LENGTH / 2.0 + step * i as f32, -LENGTH / 2.0 + step * (i + 1) as f32);
            let xm = (xa + xb) / 2.0;
            skiff.spawn(part(Vec3::new(step + 0.04, 0.3, 2.0 * bottom(xm) + 0.3), Vec3::new(xm, FLOOR - 0.15, 0.0), &hull));
        }
        for (at, height, plank, quartz) in planks {
            // The gunwale's quartz, dark until the skiff has power.
            let gunwale = Transform::from_xyz(0.0, height / 2.0 + 0.035, 0.0);
            skiff.spawn((Mesh3d(plank), MeshMaterial3d(hull.clone()), at)).with_child((Mesh3d(quartz), MeshMaterial3d(dark.clone()), gunwale));
        }
        // The transom across the stern.
        let stern = -LENGTH / 2.0;
        skiff.spawn(part(Vec3::new(0.24, sheer(stern) - FLOOR, 2.0 * half_beam(stern)), Vec3::new(stern - 0.05, (sheer(stern) + FLOOR) / 2.0, 0.0), &hull));
        // The stern seat, and on it the helm's socket: a stone block with quartz set in its top.
        let seat = SOCKET.y - 0.5 - FLOOR;
        skiff.spawn(part(Vec3::new(1.2, seat, 2.0 * bottom(SOCKET.x)), Vec3::new(SOCKET.x + 0.1, FLOOR + seat / 2.0, 0.0), &stone));
        skiff.spawn(part(Vec3::new(0.8, 0.5, 0.8), SOCKET - Vec3::Y * 0.25, &stone));
        skiff.spawn(part(Vec3::new(0.5, 0.06, 0.5), SOCKET + Vec3::Y * 0.02, &helm.0));
        // A carved prow post at the bow, standing above the sheer.
        let bow = LENGTH / 2.0;
        skiff.spawn(part(Vec3::new(0.45, sheer(bow) + 0.9 - FLOOR, 0.45), Vec3::new(bow - 0.1, (sheer(bow) + 0.9 + FLOOR) / 2.0, 0.0), &stone));
    });
    commands.insert_resource(helm);
}

/// Once the north gate has woken (the ridge's pass, and its wake), the skiff takes its helm: the
/// tiller in the landing circle is gone, one settles into the socket at the stern, and the helm's
/// quartz lights. A save that is past it starts with the helm in.
#[allow(clippy::too_many_arguments)]
fn take_helm(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    waking: Res<Waking>,
    helm: Res<HelmQuartz>,
    skiff: Single<Entity, With<Skiff>>,
    delivered: Query<Entity, With<Delivered>>,
    mut taken: Local<bool>,
) {
    let due = beam_shines(waking.shown);
    if due == *taken {
        return;
    }
    *taken = due;
    if let Some(mut material) = materials.get_mut(&helm.0) {
        *material = MachineTrim::look(due);
    }
    if !due {
        return;
    }
    for tiller in &delivered {
        commands.entity(tiller).despawn();
    }
    let stone = crate::waking::stone(&mut materials);
    let [shaft, core] = crate::props::tiller(TILLER_SIZE).map(|m| meshes.add(m));
    // The blade hangs over the stern and down into the water's line; the grip reaches forward over
    // the seat.
    let fitted = Transform::from_translation(SOCKET + Vec3::new(-0.2, 0.35, 0.0))
        .with_rotation(Quat::from_rotation_z(0.5) * Quat::from_rotation_y(std::f32::consts::PI));
    commands.entity(*skiff).with_children(|skiff| {
        skiff
            .spawn((Settling(0.0), Mesh3d(shaft), MeshMaterial3d(stone), fitted.with_translation(fitted.translation + Vec3::Y * 2.0)))
            .with_child((Mesh3d(core), MeshMaterial3d(helm.0.clone())));
    });
}

/// The tiller settles into its socket.
fn settle_helm(time: Res<Time>, mut commands: Commands, mut helm: Query<(Entity, &mut Settling, &mut Transform)>) {
    for (entity, mut settling, mut transform) in &mut helm {
        let before = 2.0 * (1.0 - settling.0 / SETTLE).max(0.0).powi(3);
        settling.0 += time.delta_secs();
        let after = 2.0 * (1.0 - settling.0 / SETTLE).max(0.0).powi(3);
        transform.translation.y -= before - after;
        if settling.0 >= SETTLE {
            commands.entity(entity).remove::<Settling>();
        }
    }
}

/// The sight rides the crawler until the crane lifts it off in the wake after the haul track's pass,
/// then waits at the crane's foot, and once the launchers know this planet's g it is set beside the
/// ledge launcher (once the ramp has risen to it). Where it is is the way's to say (see `waking`).
fn place_sight(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    waking: Res<Waking>,
    sites: Res<Sites>,
    mut units: Query<(&Sight, &mut Visibility, &mut Transform)>,
    mut landed: Local<bool>,
) {
    let off = waking.sight(&sites);
    if let Some(at) = off
        && !*landed
    {
        *landed = true;
        let [body, lens] = sight_parts(&mut meshes, &mut materials);
        commands.spawn((Sight { riding: false }, body.0, body.1, Transform::from_translation(at), Visibility::Inherited)).with_child(lens);
    }
    for (unit, mut visibility, mut transform) in &mut units {
        let show = unit.riding == off.is_none();
        visibility.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
        if let Some(at) = off
            && !unit.riding
            && transform.translation != at
        {
            transform.translation = at;
        }
    }
}

/// Where the calibrated sight sits: on the ledge, beside the ledge launcher's rail, on the
/// station's side and behind its gauge.
pub fn fitted_at(sites: &Sites) -> Vec3 {
    let ledge = sites.channel_launch;
    let side = ledge.direction.cross(Vec3::Y).normalize();
    let mut at = ledge.origin - ledge.direction * 3.0 + side * 1.0;
    at.y = sites.channel_well.y as f32;
    at
}
