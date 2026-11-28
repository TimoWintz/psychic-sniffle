//! What moves in the sky: clouds drifting high over the island on the wind, and a few flocks of
//! flyers circling far out over the sea. Neither comes between the player and a puzzle: the clouds
//! are high and cast no shadows, and the flocks keep well away from the island.

use crate::island::WORLD_SIZE;
use crate::state::not_paused;
use crate::voxel::hash01;
use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// The way the wind blows, across the ground (the same as the tufts sway to, in `sway.wesl`).
const WIND: Vec2 = Vec2::new(0.8, 0.6);
/// How fast the clouds drift, in metres per second.
const DRIFT: f32 = 1.6;
/// How far from the island's middle the clouds are kept; one that drifts further comes back in on
/// the other side.
const CLOUD_FIELD: f32 = 650.0;
/// How high the clouds float.
const CLOUD_HEIGHT: (f32, f32) = (80.0, 115.0);
const CLOUDS: i32 = 30;

/// The flocks: how far out their circles lie, how wide they are, how high they fly.
const FLOCKS: usize = 3;
const FLOCK_OUT: (f32, f32) = (215.0, 265.0);
const FLOCK_CIRCLE: f32 = 35.0;
const FLOCK_HEIGHT: (f32, f32) = (28.0, 48.0);
/// The island's own reach from its middle, which no flyer comes within.
const ISLAND_REACH: f32 = 150.0;

// The clouds float well above the island's highest ground, and the island's reach takes in all of
// it but the far corners of the block grid, which are open sea.
const _: () = assert!(CLOUD_HEIGHT.0 > WORLD_SIZE.y as f32 && ISLAND_REACH > WORLD_SIZE.x as f32 / 2.0 * 1.2);

#[derive(Component)]
struct Cloud;

#[derive(Component)]
struct Flyer {
    flock: usize,
    /// Its place in the flock, behind and beside the leader.
    offset: Vec3,
    beat: f32,
}

#[derive(Component)]
struct Wing(f32);

pub struct AloftPlugin;

impl Plugin for AloftPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn).add_systems(Update, (drift_clouds, fly).run_if(not_paused));
    }
}

fn middle() -> Vec2 {
    Vec2::new(WORLD_SIZE.x as f32, WORLD_SIZE.z as f32) / 2.0
}

/// A cloud: a few flattened blocks of white, piled loosely.
fn cloud(seed: i32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let r = |k: i32| hash01(IVec3::new(seed, 700 + k, 11));
    let parts = 3 + (r(0) * 5.0) as i32;
    for i in 0..parts {
        let q = |k: i32| hash01(IVec3::new(seed, 720 + k, i));
        let size = Vec3::new(12.0 + 22.0 * q(0), 3.0 + 4.0 * q(1), 9.0 + 16.0 * q(2));
        let centre = Vec3::new((q(3) - 0.5) * 30.0, q(4) * 4.0 - if i == 0 { 0.0 } else { 1.5 }, (q(5) - 0.5) * 22.0);
        let mesh = Cuboid::from_size(size).mesh().build();
        let first = positions.len() as u32;
        if let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            positions.extend(p.iter().map(|v| (Vec3::from_array(*v) + centre).to_array()));
        }
        if let Some(bevy::mesh::VertexAttributeValues::Float32x3(n)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
            normals.extend(n.iter().copied());
        }
        if let Some(Indices::U32(ix)) = mesh.indices() {
            indices.extend(ix.iter().map(|k| k + first));
        } else if let Some(Indices::U16(ix)) = mesh.indices() {
            indices.extend(ix.iter().map(|&k| k as u32 + first));
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices))
}

/// Where a flock's leader is at time `t`: on its circle far out over the sea.
pub fn flock_at(flock: usize, t: f32) -> Vec3 {
    let f = flock as i32;
    let bearing = hash01(IVec3::new(f, 800, 3)) * std::f32::consts::TAU;
    let out = FLOCK_OUT.0 + (FLOCK_OUT.1 - FLOCK_OUT.0) * hash01(IVec3::new(f, 801, 3));
    let height = FLOCK_HEIGHT.0 + (FLOCK_HEIGHT.1 - FLOCK_HEIGHT.0) * hash01(IVec3::new(f, 802, 3));
    let speed = 0.09 + 0.05 * hash01(IVec3::new(f, 803, 3));
    let centre = middle() + Vec2::from_angle(bearing) * out;
    let a = t * speed * if flock.is_multiple_of(2) { 1.0 } else { -1.0 } + f as f32;
    let at = centre + Vec2::from_angle(a) * FLOCK_CIRCLE;
    Vec3::new(at.x, height + 3.0 * (t * 0.3 + f as f32).sin(), at.y)
}

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let white = materials.add(StandardMaterial { base_color: Color::srgb(0.98, 0.97, 0.96), perceptual_roughness: 1.0, ..default() });
    for i in 0..CLOUDS {
        let r = |k: i32| hash01(IVec3::new(i, 750 + k, 5));
        let at = middle() + Vec2::new(r(0) - 0.5, r(1) - 0.5) * 2.0 * CLOUD_FIELD;
        let height = CLOUD_HEIGHT.0 + (CLOUD_HEIGHT.1 - CLOUD_HEIGHT.0) * r(2);
        commands.spawn((
            Cloud,
            Mesh3d(meshes.add(cloud(i))),
            MeshMaterial3d(white.clone()),
            Transform::from_xyz(at.x, height, at.y).with_rotation(Quat::from_rotation_y(r(3) * 0.6)),
            NotShadowCaster,
        ));
    }

    // The flyers: dark against the sky, a body and two wings that beat.
    let dark = materials.add(StandardMaterial { base_color: Color::srgb(0.18, 0.16, 0.24), perceptual_roughness: 0.9, ..default() });
    // Larger than our birds: a wingspan of about four metres.
    let body = meshes.add(Cuboid::new(0.45, 0.35, 1.5));
    let wing = meshes.add(Mesh::from(Cuboid::new(1.9, 0.08, 0.7)).translated_by(Vec3::X * 0.95));
    for flock in 0..FLOCKS {
        let size = 4 + (hash01(IVec3::new(flock as i32, 810, 3)) * 3.0) as usize;
        for k in 0..size {
            let side = if k.is_multiple_of(2) { 1.0 } else { -1.0 };
            let rank = k.div_ceil(2) as f32;
            let offset = Vec3::new(side * rank * 4.0, rank * 0.8, rank * 4.5);
            commands
                .spawn((
                    Flyer { flock, offset, beat: hash01(IVec3::new(flock as i32, 820, k as i32)) * 6.0 },
                    Mesh3d(body.clone()),
                    MeshMaterial3d(dark.clone()),
                    Transform::default(),
                    NotShadowCaster,
                ))
                .with_children(|bird| {
                    for side in [1.0, -1.0] {
                        bird.spawn((
                            Wing(side),
                            Mesh3d(wing.clone()),
                            MeshMaterial3d(dark.clone()),
                            Transform::from_scale(Vec3::new(side, 1.0, 1.0)),
                            NotShadowCaster,
                        ));
                    }
                });
        }
    }
}

fn drift_clouds(time: Res<Time>, mut clouds: Query<&mut Transform, With<Cloud>>) {
    let step = WIND.normalize() * DRIFT * time.delta_secs();
    let middle = middle();
    for mut transform in &mut clouds {
        let mut at = transform.translation.xz() + step;
        let from = at - middle;
        // Gone too far downwind: come back in on the other side.
        if from.dot(WIND.normalize()) > CLOUD_FIELD {
            at -= WIND.normalize() * 2.0 * CLOUD_FIELD;
        }
        transform.translation.x = at.x;
        transform.translation.z = at.y;
    }
}

fn fly(time: Res<Time>, mut flyers: Query<(&Flyer, &mut Transform, &Children)>, mut wings: Query<(&Wing, &mut Transform), Without<Flyer>>) {
    let t = time.elapsed_secs();
    for (flyer, mut transform, children) in &mut flyers {
        let here = flock_at(flyer.flock, t);
        let ahead = flock_at(flyer.flock, t + 0.5);
        let heading = (ahead - here).with_y(0.0).normalize_or(Vec3::Z);
        let turn = Quat::from_rotation_arc(Vec3::NEG_Z, heading);
        transform.translation = here + turn * flyer.offset;
        transform.rotation = turn;
        // Beating, now and then gliding.
        let glide = ((t * 0.4 + flyer.beat).sin() > 0.6) as u8 as f32;
        let flap = (t * 7.0 + flyer.beat).sin() * 0.7 * (1.0 - glide) + 0.1 * glide;
        for &child in children {
            if let Ok((wing, mut w)) = wings.get_mut(child) {
                w.rotation = Quat::from_rotation_z(flap * wing.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flocks_never_come_over_the_island() {
        for flock in 0..FLOCKS {
            for i in 0..2000 {
                let at = flock_at(flock, i as f32 * 0.37);
                // The leader, and the furthest a follower trails it.
                let reach = at.xz().distance(middle()) - 20.0;
                assert!(reach > ISLAND_REACH, "flock {flock} comes {reach} m from the island's middle");
                assert!(at.y > 20.0, "flock {flock} flies at {}", at.y);
            }
        }
    }
}
