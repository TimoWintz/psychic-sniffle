//! The block grid: storage, queries, ray casting and meshing. One block is one cubic metre.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use physics_core::materials::Material;

#[derive(Resource, Clone)]
pub struct VoxelWorld {
    pub size: IVec3,
    cells: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub cell: IVec3,
    /// Outward normal of the face that was hit.
    pub normal: IVec3,
    pub point: Vec3,
    pub distance: f32,
}

impl VoxelWorld {
    pub fn new(size: IVec3) -> Self {
        VoxelWorld { size, cells: vec![0; (size.x * size.y * size.z) as usize] }
    }

    fn index(&self, p: IVec3) -> Option<usize> {
        let inside = p.cmpge(IVec3::ZERO).all() && p.cmplt(self.size).all();
        inside.then(|| ((p.y * self.size.z + p.z) * self.size.x + p.x) as usize)
    }

    pub fn get(&self, p: IVec3) -> Material {
        self.index(p).map_or(Material::Air, |i| Material::from_id(self.cells[i]))
    }

    pub fn set(&mut self, p: IVec3, material: Material) {
        if let Some(i) = self.index(p) {
            self.cells[i] = material.id();
        }
    }

    pub fn is_solid(&self, p: IVec3) -> bool {
        self.get(p).is_solid()
    }

    /// Fill the box of blocks from `min` to `max`, both inclusive.
    pub fn fill(&mut self, min: IVec3, max: IVec3, material: Material) {
        for y in min.y..=max.y {
            for z in min.z..=max.z {
                for x in min.x..=max.x {
                    self.set(IVec3::new(x, y, z), material);
                }
            }
        }
    }

    /// Height of the top surface of the highest solid block in a column (0 if the column is empty).
    #[cfg_attr(not(test), expect(dead_code, reason = "used by the island's consistency tests"))]
    pub fn surface(&self, x: i32, z: i32) -> i32 {
        (0..self.size.y).rev().find(|&y| self.is_solid(IVec3::new(x, y, z))).map_or(0, |y| y + 1)
    }

    /// Whether an axis-aligned box overlaps any solid block.
    pub fn box_collides(&self, min: Vec3, max: Vec3) -> bool {
        let lo = min.floor().as_ivec3();
        // A box that ends exactly on a block boundary does not reach into the next block.
        let hi = (max - Vec3::splat(1e-4)).floor().as_ivec3();
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    if self.is_solid(IVec3::new(x, y, z)) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// First solid block along a ray (grid traversal, exact).
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<RayHit> {
        let dir = direction.normalize_or_zero();
        if dir == Vec3::ZERO {
            return None;
        }
        let mut cell = origin.floor().as_ivec3();
        let step = IVec3::new(dir.x.signum() as i32, dir.y.signum() as i32, dir.z.signum() as i32);
        // Distance along the ray to the next block boundary on each axis, and between boundaries.
        let mut t_max = Vec3::ZERO;
        let mut t_delta = Vec3::ZERO;
        for axis in 0..3 {
            if dir[axis] == 0.0 {
                t_max[axis] = f32::INFINITY;
                t_delta[axis] = f32::INFINITY;
            } else {
                let boundary = if dir[axis] > 0.0 { cell[axis] as f32 + 1.0 } else { cell[axis] as f32 };
                t_max[axis] = (boundary - origin[axis]) / dir[axis];
                t_delta[axis] = 1.0 / dir[axis].abs();
            }
        }
        let mut distance = 0.0;
        let mut normal = IVec3::ZERO;
        while distance <= max_distance {
            if self.is_solid(cell) {
                return Some(RayHit { cell, normal, point: origin + dir * distance, distance });
            }
            let axis = if t_max.x <= t_max.y && t_max.x <= t_max.z {
                0
            } else if t_max.y <= t_max.z {
                1
            } else {
                2
            };
            distance = t_max[axis];
            t_max[axis] += t_delta[axis];
            cell[axis] += step[axis];
            normal = IVec3::ZERO;
            normal[axis] = -step[axis];
        }
        None
    }

    /// Solid blocks that touch a passable one: the only blocks anything can collide with.
    pub fn exposed_blocks(&self) -> Vec<IVec3> {
        let mut out = Vec::new();
        for y in 0..self.size.y {
            for z in 0..self.size.z {
                for x in 0..self.size.x {
                    let p = IVec3::new(x, y, z);
                    if self.is_solid(p) && FACES.iter().any(|f| !self.is_solid(p + f.normal)) {
                        out.push(p);
                    }
                }
            }
        }
        out
    }

    /// Mesh of the visible faces of the columns `x0..x1`, `z0..z1`, coloured per block.
    pub fn mesh(&self, x0: i32, x1: i32, z0: i32, z1: i32) -> Mesh {
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut colours: Vec<[f32; 4]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        for y in 0..self.size.y {
            for z in z0..z1 {
                for x in x0..x1 {
                    let p = IVec3::new(x, y, z);
                    let material = self.get(p);
                    if !material.is_solid() {
                        continue;
                    }
                    // Vertex colours are linear; the table holds sRGB. A little variation per block
                    // keeps flat ground reading as blocks.
                    let [r, g, b] = material.properties().colour;
                    let shade = 0.88 + 0.24 * hash01(p);
                    let base = Color::srgb(r, g, b).to_linear();
                    let colour = [base.red * shade, base.green * shade, base.blue * shade, 1.0];
                    for face in &FACES {
                        if self.is_solid(p + face.normal) {
                            continue;
                        }
                        let first = positions.len() as u32;
                        for corner in face.corners {
                            positions.push((p.as_vec3() + corner).to_array());
                            normals.push(face.normal.as_vec3().to_array());
                            colours.push(colour);
                        }
                        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
                    }
                }
            }
        }
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
            .with_inserted_indices(Indices::U32(indices))
    }
}

struct Face {
    normal: IVec3,
    /// Corners in counter-clockwise order seen from outside.
    corners: [Vec3; 4],
}

const fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

const FACES: [Face; 6] = [
    Face { normal: IVec3::X, corners: [v(1., 0., 0.), v(1., 1., 0.), v(1., 1., 1.), v(1., 0., 1.)] },
    Face { normal: IVec3::NEG_X, corners: [v(0., 0., 0.), v(0., 0., 1.), v(0., 1., 1.), v(0., 1., 0.)] },
    Face { normal: IVec3::Y, corners: [v(0., 1., 0.), v(0., 1., 1.), v(1., 1., 1.), v(1., 1., 0.)] },
    Face { normal: IVec3::NEG_Y, corners: [v(0., 0., 0.), v(1., 0., 0.), v(1., 0., 1.), v(0., 0., 1.)] },
    Face { normal: IVec3::Z, corners: [v(0., 0., 1.), v(1., 0., 1.), v(1., 1., 1.), v(0., 1., 1.)] },
    Face { normal: IVec3::NEG_Z, corners: [v(0., 0., 0.), v(0., 1., 0.), v(1., 1., 0.), v(1., 0., 0.)] },
];

/// Deterministic value in `[0, 1)` from a block position.
pub fn hash01(p: IVec3) -> f32 {
    let mut h = (p.x as u32).wrapping_mul(0x9E37_79B1) ^ (p.y as u32).wrapping_mul(0x85EB_CA77) ^ (p.z as u32).wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65536.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slab() -> VoxelWorld {
        let mut world = VoxelWorld::new(IVec3::new(8, 8, 8));
        world.fill(IVec3::new(0, 0, 0), IVec3::new(7, 2, 7), Material::Rock);
        world
    }

    #[test]
    fn faces_wind_outward() {
        for face in &FACES {
            let [a, b, c, d] = face.corners;
            assert_eq!((b - a).cross(c - a), face.normal.as_vec3());
            assert_eq!((c - a).cross(d - a), face.normal.as_vec3());
        }
    }

    #[test]
    fn surface_and_collision() {
        let world = slab();
        assert_eq!(world.surface(3, 3), 3);
        assert!(!world.box_collides(Vec3::new(1.0, 3.0, 1.0), Vec3::new(1.6, 4.8, 1.6)), "standing on the slab");
        assert!(world.box_collides(Vec3::new(1.0, 2.9, 1.0), Vec3::new(1.6, 4.7, 1.6)), "sunk into the slab");
        assert_eq!(world.get(IVec3::new(-1, 0, 0)), Material::Air);
    }

    #[test]
    fn ray_hits_the_top_face() {
        let world = slab();
        let hit = world.raycast(Vec3::new(2.5, 7.0, 2.5), Vec3::NEG_Y, 20.0).unwrap();
        assert_eq!(hit.cell, IVec3::new(2, 2, 2));
        assert_eq!(hit.normal, IVec3::Y);
        assert!((hit.point.y - 3.0).abs() < 1e-5);
        assert!(world.raycast(Vec3::new(2.5, 7.0, 2.5), Vec3::Y, 20.0).is_none());
    }

    #[test]
    fn only_visible_faces_are_meshed() {
        // An 8x3x8 slab on the world floor: top, four sides, and the underside.
        let mesh = slab().mesh(0, 8, 0, 8);
        let faces = 64 + 4 * 24 + 64;
        assert_eq!(mesh.count_vertices(), faces * 4);
        assert_eq!(slab().exposed_blocks().len(), 8 * 8 * 3 - 6 * 6);
    }
}
