//! Everything that is looked at and never touched: the island's meshes, the sea, the sun, a sky with a
//! ringed giant and a moon in it, haze, islands on the horizon and the small plants in the turf.

use crate::island::{GROUND, SEA_LEVEL, Sites};
use crate::voxel::{VoxelWorld, fbm, hash01, tint};
use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use physics_core::materials::Material;

/// Columns per terrain mesh along each horizontal axis.
const CHUNK: i32 = 32;

/// Colour of the sky at the horizon, which the haze fades everything towards.
const HORIZON: Color = Color::srgb(0.70, 0.86, 0.88);
const ZENITH: Color = Color::srgb(0.30, 0.56, 0.78);
/// Radius of the sky dome; inside the camera's far plane.
const SKY: f32 = 900.0;

/// Moves with the camera, so the sky and what is in it never come closer.
#[derive(Component)]
struct SkyAnchor;

pub struct SceneryPlugin;

impl Plugin for SceneryPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(HORIZON)).add_systems(Startup, spawn).add_systems(PostUpdate, follow_camera.before(TransformSystems::Propagate));
    }
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    world: Res<VoxelWorld>,
    sites: Res<Sites>,
) {
    // Block colours are per vertex, so one material serves the whole island, and one unlit material
    // serves everything that glows.
    let blocks = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, ..default() });
    let glow = materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() });
    for z0 in (0..world.size.z).step_by(CHUNK as usize) {
        for x0 in (0..world.size.x).step_by(CHUNK as usize) {
            let (x1, z1) = ((x0 + CHUNK).min(world.size.x), (z0 + CHUNK).min(world.size.z));
            commands.spawn((Mesh3d(meshes.add(world.mesh(x0, x1, z0, z1, false))), MeshMaterial3d(blocks.clone())));
            commands.spawn((Mesh3d(meshes.add(world.mesh(x0, x1, z0, z1, true))), MeshMaterial3d(glow.clone()), NotShadowCaster));
        }
    }
    commands.spawn((Mesh3d(meshes.add(tufts(&world, &sites))), MeshMaterial3d(blocks.clone()), NotShadowCaster));

    // Islands on the horizon, larger than life so the haze softens them.
    for (i, (centre, scale)) in [
        (Vec3::new(-190.0, 0.0, 10.0), 3.0),
        (Vec3::new(330.0, 0.0, -150.0), 4.0),
        (Vec3::new(120.0, 0.0, -300.0), 3.5),
        (Vec3::new(-120.0, 0.0, 330.0), 4.5),
    ]
    .into_iter()
    .enumerate()
    {
        let distant = distant_island(i as i32, false);
        let half = distant.size.as_vec3() * scale / 2.0;
        let mesh = meshes.add(distant.mesh(0, distant.size.x, 0, distant.size.z, false));
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(blocks.clone()),
            // Its own sea level (blocks up to 8.6 m) sits on the island's.
            Transform::from_translation(centre + Vec3::new(-half.x, SEA_LEVEL - SEA_LEVEL * scale, -half.z)).with_scale(Vec3::splat(scale)),
            NotShadowCaster,
        ));
    }
    // One rock that floats: the familiar world, not quite.
    let floating = distant_island(9, true);
    commands.spawn((
        Mesh3d(meshes.add(floating.mesh(0, floating.size.x, 0, floating.size.z, false))),
        MeshMaterial3d(blocks.clone()),
        Transform::from_xyz(260.0, 70.0, 150.0).with_scale(Vec3::splat(2.5)),
        NotShadowCaster,
    ));

    let size = world.size.as_vec3();
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(1800.0, 1800.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.09, 0.38, 0.48, 0.88),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.18,
            ..default()
        })),
        Transform::from_xyz(size.x / 2.0, SEA_LEVEL, size.z / 2.0),
        NotShadowCaster,
    ));

    // A pale-gold sun, high in the south-west of the sky.
    let sun = Transform::from_xyz(60.0, 100.0, 30.0).looking_at(Vec3::new(88.0, 20.0, 64.0), Vec3::Y);
    commands.spawn((DirectionalLight { illuminance: 9_500.0, color: Color::srgb(1.0, 0.95, 0.86), shadow_maps_enabled: true, ..default() }, sun));
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(0.78, 0.88, 1.0), brightness: 380.0, ..default() });

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(20.0, 30.0, 60.0),
        DistanceFog {
            color: HORIZON,
            directional_light_color: Color::srgba(1.0, 0.92, 0.75, 0.45),
            directional_light_exponent: 24.0,
            falloff: FogFalloff::Linear { start: 140.0, end: 760.0 },
        },
    ));

    // The sky: a dome shaded from the horizon up, the sun's disc, a ringed giant and a small moon.
    let sky_material = |materials: &mut Assets<StandardMaterial>, colour: Color, alpha: bool| {
        materials.add(StandardMaterial {
            base_color: colour,
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            alpha_mode: if alpha { AlphaMode::Blend } else { AlphaMode::Opaque },
            ..default()
        })
    };
    let toward_sun = -sun.forward().as_vec3();
    let giant = Vec3::new(0.55, 0.28, -0.79).normalize();
    let moon = Vec3::new(-0.62, 0.42, -0.66).normalize();
    commands.spawn((SkyAnchor, Transform::default(), Visibility::default())).with_children(|sky| {
        sky.spawn((Mesh3d(meshes.add(dome())), MeshMaterial3d(sky_material(&mut materials, Color::WHITE, false)), NotShadowCaster));
        sky.spawn((
            Mesh3d(meshes.add(Sphere::new(26.0).mesh().uv(24, 12))),
            MeshMaterial3d(sky_material(&mut materials, Color::srgb(1.0, 0.97, 0.88), false)),
            Transform::from_translation(toward_sun * (SKY - 60.0)),
            NotShadowCaster,
        ));
        let at = giant * (SKY - 160.0);
        sky.spawn((
            Mesh3d(meshes.add(banded_sphere(150.0))),
            MeshMaterial3d(sky_material(&mut materials, Color::WHITE, false)),
            Transform::from_translation(at).with_rotation(Quat::from_rotation_z(0.35)),
            NotShadowCaster,
        ));
        sky.spawn((
            Mesh3d(meshes.add(Annulus::new(190.0, 290.0))),
            MeshMaterial3d(sky_material(&mut materials, Color::srgba(0.92, 0.84, 0.90, 0.55), true)),
            Transform::from_translation(at).with_rotation(Quat::from_rotation_z(0.35) * Quat::from_rotation_x(-1.32)),
            NotShadowCaster,
        ));
        sky.spawn((
            Mesh3d(meshes.add(Sphere::new(30.0).mesh().uv(32, 16))),
            MeshMaterial3d(sky_material(&mut materials, Color::srgb(0.86, 0.84, 0.92), false)),
            Transform::from_translation(moon * (SKY - 120.0)),
            NotShadowCaster,
        ));
    });
}

fn follow_camera(camera: Single<&Transform, (With<Camera3d>, Without<SkyAnchor>)>, mut sky: Single<&mut Transform, With<SkyAnchor>>) {
    sky.translation = camera.translation;
}

/// The inside of a sphere, shaded from the horizon to the zenith.
fn dome() -> Mesh {
    let mut mesh = Sphere::new(SKY).mesh().uv(48, 24);
    let horizon = HORIZON.to_linear().to_vec3();
    let zenith = ZENITH.to_linear().to_vec3();
    let below = Color::srgb(0.46, 0.66, 0.70).to_linear().to_vec3();
    let colours: Vec<[f32; 4]> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(positions)) => positions
            .iter()
            .map(|p| {
                let up = p[1] / SKY;
                let c = if up >= 0.0 { horizon.lerp(zenith, up.powf(0.6)) } else { horizon.lerp(below, (-up * 4.0).min(1.0)) };
                [c.x, c.y, c.z, 1.0]
            })
            .collect(),
        _ => Vec::new(),
    };
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh
}

/// A gas giant: a sphere in soft bands of cream, rose and violet.
fn banded_sphere(radius: f32) -> Mesh {
    let mut mesh = Sphere::new(radius).mesh().uv(48, 32);
    const BANDS: [[f32; 3]; 5] = [[0.95, 0.86, 0.80], [0.86, 0.68, 0.72], [0.78, 0.70, 0.88], [0.94, 0.80, 0.70], [0.88, 0.76, 0.84]];
    let colours: Vec<[f32; 4]> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(positions)) => positions
            .iter()
            .map(|p| {
                let latitude = p[1] / radius;
                let band = ((latitude * 3.2 + (latitude * 11.0).sin() * 0.25 + 2.5).max(0.0) as usize).min(4);
                let c = Color::srgb_from_array(BANDS[band]).to_linear();
                [c.red, c.green, c.blue, 1.0]
            })
            .collect(),
        _ => Vec::new(),
    };
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh
}

/// A small island for the horizon: a peak or two of rock under turf and a few groves. Floating, it is
/// turned into a crag with its roots hanging below.
fn distant_island(seed: i32, floating: bool) -> VoxelWorld {
    let size = IVec3::new(48, 40, 48);
    let mut world = VoxelWorld::new(size);
    let base = if floating { 20 } else { 0 };
    for z in 0..size.z {
        for x in 0..size.x {
            let (xf, zf) = (x as f32, z as f32);
            let d = Vec2::new(xf - 24.0, zf - 24.0).length() / 22.0 + (fbm(xf / 9.0, zf / 9.0, 900 + seed, 3) - 0.5) * 0.5;
            if d >= 1.0 {
                continue;
            }
            let rise = (1.0 - d).powf(1.3);
            let top = base + 9 + (rise * (6.0 + 14.0 * fbm(xf / 7.0, zf / 7.0, 950 + seed, 2))) as i32;
            let bottom = if floating { base - (rise * 16.0 * fbm(xf / 4.0, zf / 4.0, 970 + seed, 2)) as i32 } else { 0 };
            for y in bottom.max(0)..top {
                let material = if y == top - 1 { if top > base + 20 { Material::Rock } else { Material::Grass } } else { Material::Rock };
                world.set(IVec3::new(x, y, z), material);
            }
            if y_has_tree(x, z, seed) && top < base + 18 {
                world.fill(IVec3::new(x, top, z), IVec3::new(x, top + 2, z), Material::Timber);
                world.fill(IVec3::new(x - 1, top + 3, z - 1), IVec3::new(x + 1, top + 3, z + 1), Material::Foliage);
            }
        }
    }
    world
}

fn y_has_tree(x: i32, z: i32, seed: i32) -> bool {
    x % 4 == 1 && z % 4 == 2 && hash01(IVec3::new(x, 990 + seed, z)) < 0.35
}

/// Thin blades and fronds standing in the turf, away from the puzzle sites. They are decoration:
/// nothing collides with them.
fn tufts(world: &VoxelWorld, sites: &Sites) -> Mesh {
    let mut out = Boxes::default();
    for z in 0..world.size.z {
        for x in 0..world.size.x {
            let near_site = sites.sites.iter().any(|[min, max]| x >= min.x - 2 && x <= max.x + 2 && z >= min.y - 2 && z <= max.y + 2);
            if near_site {
                continue;
            }
            let top = world.surface(x, z);
            let ground = IVec3::new(x, top - 1, z);
            if world.get(ground) != Material::Grass || world.is_solid(ground + IVec3::Y) {
                continue;
            }
            let h = hash01(IVec3::new(x, 601, z));
            // Denser in the low meadows than on the mountain.
            let density = if top <= GROUND + 3 { 0.16 } else { 0.07 };
            if h > density {
                continue;
            }
            let colour = Vec3::from_array(tint(Material::Grass, ground)) * 1.18;
            let blades = 1 + (h * 20.0) as i32 % 3;
            for b in 0..blades {
                let r = |k: i32| hash01(IVec3::new(x * 7 + b, 610 + k, z * 7));
                let foot = Vec3::new(x as f32 + 0.15 + 0.7 * r(0), top as f32, z as f32 + 0.15 + 0.7 * r(1));
                let height = 0.15 + 0.3 * r(2);
                let shade = colour * (0.85 + 0.3 * r(3));
                out.push(foot - Vec3::new(0.03, 0.0, 0.03), foot + Vec3::new(0.03, height, 0.03), shade);
            }
            // Now and then a frond with a pale violet tip.
            if h < 0.008 {
                let foot = Vec3::new(x as f32 + 0.5, top as f32, z as f32 + 0.5);
                out.push(foot - Vec3::new(0.05, 0.0, 0.05), foot + Vec3::new(0.05, 0.8, 0.05), colour);
                out.push(foot + Vec3::new(-0.11, 0.8, -0.11), foot + Vec3::new(0.11, 1.0, 0.11), Vec3::new(0.78, 0.66, 0.95));
            }
        }
    }
    out.mesh()
}

/// Axis-aligned boxes gathered into one vertex-coloured mesh.
#[derive(Default)]
struct Boxes {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colours: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Boxes {
    /// A box from `min` to `max`, sRGB `colour`; the bottom face is left out (it rests on the ground).
    fn push(&mut self, min: Vec3, max: Vec3, colour: Vec3) {
        let c = Color::srgb(colour.x.min(1.0), colour.y.min(1.0), colour.z.min(1.0)).to_linear();
        let faces: [(Vec3, [Vec3; 4]); 5] = [
            (Vec3::X, [Vec3::new(1., 0., 0.), Vec3::new(1., 1., 0.), Vec3::new(1., 1., 1.), Vec3::new(1., 0., 1.)]),
            (Vec3::NEG_X, [Vec3::new(0., 0., 0.), Vec3::new(0., 0., 1.), Vec3::new(0., 1., 1.), Vec3::new(0., 1., 0.)]),
            (Vec3::Y, [Vec3::new(0., 1., 0.), Vec3::new(0., 1., 1.), Vec3::new(1., 1., 1.), Vec3::new(1., 1., 0.)]),
            (Vec3::Z, [Vec3::new(0., 0., 1.), Vec3::new(1., 0., 1.), Vec3::new(1., 1., 1.), Vec3::new(0., 1., 1.)]),
            (Vec3::NEG_Z, [Vec3::new(0., 0., 0.), Vec3::new(0., 1., 0.), Vec3::new(1., 1., 0.), Vec3::new(1., 0., 0.)]),
        ];
        for (normal, corners) in faces {
            let first = self.positions.len() as u32;
            for corner in corners {
                self.positions.push((min + (max - min) * corner).to_array());
                self.normals.push(normal.to_array());
                // Darker at the root, as if shaded by the turf around it.
                let light = 0.8 + 0.2 * corner.y;
                self.colours.push([c.red * light, c.green * light, c.blue * light, 1.0]);
            }
            self.indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        }
    }

    fn mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colours)
            .with_inserted_indices(Indices::U32(self.indices))
    }
}
