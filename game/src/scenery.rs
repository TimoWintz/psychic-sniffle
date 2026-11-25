//! Everything that is looked at and never touched: the island's meshes, the sea, the sun, a sky with a
//! ringed giant and a moon in it, haze, islands on the horizon and the small plants in the turf. The
//! sun, the sky and the haze follow the chapter's light (`Sky::of`).

use crate::carry::DirtyCells;
use crate::island::{GROUND, SEA_LEVEL, Sites};
use crate::state::Game;
use crate::voxel::{VoxelWorld, fbm, hash01, tint};
use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use physics_core::content::Light;
use physics_core::materials::Material;

/// Columns per terrain mesh along each horizontal axis.
const CHUNK: i32 = 32;

/// Radius of the sky dome; inside the camera's far plane.
const SKY: f32 = 900.0;

/// One terrain mesh: the columns from `x0`, `z0` to the next chunk, its glowing blocks or the rest.
#[derive(Component)]
struct TerrainChunk {
    x0: i32,
    z0: i32,
    glow: bool,
}

/// Moves with the camera, so the sky and what is in it never come closer.
#[derive(Component)]
struct SkyAnchor;

/// The sky's dome, which the intro hides while it shows space.
#[derive(Component)]
pub struct SkyDome;

/// The light of a chapter: where the sun stands, its colour, the sky's and the haze's. Colours are sRGB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    /// From the island towards the sun.
    pub toward_sun: Vec3,
    pub sun: Color,
    /// The sun's light on a surface facing it, in lux.
    pub illuminance: f32,
    /// What the sky lights from every side: the colour of the shadows.
    pub fill: Color,
    pub fill_brightness: f32,
    pub zenith: Color,
    /// The sky at the horizon, which the haze fades everything towards.
    pub horizon: Color,
    /// The horizon under the sun, warmed by it.
    pub glow: Color,
    /// The haze in the sun's direction, lit by it; its alpha is how strongly.
    pub haze_in_the_sun: Color,
    /// How far one can see before the haze takes everything, in metres.
    pub visibility: f32,
}

impl Sky {
    pub fn of(light: Light) -> Sky {
        match light {
            // A low sun in the south-west throws long shadows to the north-east; its warm light
            // against the cool blue of the sky in the shadows is the look of a late afternoon. The
            // sun is only a little warm, so the turf stays teal: most of the gold is in the sky
            // and the haze around it.
            Light::Afternoon => Sky {
                // About 35 degrees up: the tops of things get less of its light than under a high
                // sun, and the faces turned to it more, so it is brighter than a noon sun.
                toward_sun: Vec3::new(-0.70, 0.60, 0.48).normalize(),
                sun: Color::srgb(1.0, 0.92, 0.80),
                illuminance: 12_000.0,
                fill: Color::srgb(0.58, 0.75, 1.0),
                fill_brightness: 700.0,
                zenith: Color::srgb(0.20, 0.52, 0.72),
                horizon: Color::srgb(0.72, 0.87, 0.86),
                glow: Color::srgb(1.0, 0.88, 0.72),
                haze_in_the_sun: Color::srgba(1.0, 0.84, 0.62, 0.4),
                visibility: 820.0,
            },
        }
    }
}

pub struct SceneryPlugin;

impl Plugin for SceneryPlugin {
    fn build(&self, app: &mut App) {
        let sky = Sky::of(app.world().resource::<Game>().chapter.light);
        app.insert_resource(ClearColor(sky.horizon))
            .add_systems(Startup, spawn)
            .add_systems(Update, rebuild_chunks)
            .add_systems(PostUpdate, follow_camera.before(TransformSystems::Propagate));
    }
}

pub(crate) fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut block_materials: ResMut<Assets<crate::graphics::BlockMaterial>>,
    world: Res<VoxelWorld>,
    sites: Res<Sites>,
    game: Res<Game>,
) {
    let sky = Sky::of(game.chapter.light);
    // Block colours are per vertex, so one material serves the whole island (with the pattern inside
    // each face), and one unlit material serves everything that glows.
    let blocks = crate::graphics::block_material(&mut block_materials);
    let glow = materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() });
    for z0 in (0..world.size.z).step_by(CHUNK as usize) {
        for x0 in (0..world.size.x).step_by(CHUNK as usize) {
            let (x1, z1) = ((x0 + CHUNK).min(world.size.x), (z0 + CHUNK).min(world.size.z));
            commands.spawn((TerrainChunk { x0, z0, glow: false }, Mesh3d(meshes.add(world.mesh(x0, x1, z0, z1, false))), MeshMaterial3d(blocks.clone())));
            commands.spawn((
                TerrainChunk { x0, z0, glow: true },
                Mesh3d(meshes.add(world.mesh(x0, x1, z0, z1, true))),
                MeshMaterial3d(glow.clone()),
                NotShadowCaster,
            ));
        }
    }
    let plain = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, ..default() });
    commands.spawn((Mesh3d(meshes.add(tufts(&world, &sites))), MeshMaterial3d(plain), NotShadowCaster));

    // Islands on the horizon, larger than life so the haze softens them.
    for (i, (centre, scale)) in
        [(Vec3::new(-180.0, 0.0, 40.0), 3.0), (Vec3::new(380.0, 0.0, -140.0), 4.0), (Vec3::new(150.0, 0.0, -280.0), 3.5), (Vec3::new(-110.0, 0.0, 380.0), 4.5)]
            .into_iter()
            .enumerate()
    {
        let distant = distant_island(i as i32);
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

    // The sea bed out past the island, level with the deepest floor the world holds.
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(1800.0, 1800.0))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.40, 0.38, 0.48), perceptual_roughness: 1.0, ..default() })),
        Transform::from_xyz(size.x / 2.0, 0.98, size.z / 2.0),
        NotShadowCaster,
    ));

    let sun = Transform::default().looking_to(-sky.toward_sun, Vec3::Y);
    commands.spawn((DirectionalLight { illuminance: sky.illuminance, color: sky.sun, shadow_maps_enabled: true, ..default() }, sun));
    commands.insert_resource(GlobalAmbientLight { color: sky.fill, brightness: sky.fill_brightness, ..default() });

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(20.0, 30.0, 60.0),
        DistanceFog {
            color: sky.horizon,
            directional_light_color: sky.haze_in_the_sun,
            directional_light_exponent: 12.0,
            // Clear near the avatar, thickening with the square of the distance: the haze takes the
            // far shore and the islands on the horizon, not the puzzle in front of the player.
            falloff: FogFalloff::from_visibility_squared(sky.visibility),
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
    let toward_sun = sky.toward_sun;
    let giant = Vec3::new(0.55, 0.28, -0.79).normalize();
    let moon = Vec3::new(-0.62, 0.42, -0.66).normalize();
    commands.spawn((SkyAnchor, Transform::default(), Visibility::default())).with_children(|heavens| {
        heavens.spawn((SkyDome, Mesh3d(meshes.add(dome(&sky))), MeshMaterial3d(sky_material(&mut materials, Color::WHITE, false)), NotShadowCaster));
        let disc = sky.sun.to_srgba();
        heavens.spawn((
            Mesh3d(meshes.add(Sphere::new(26.0).mesh().uv(24, 12))),
            MeshMaterial3d(sky_material(&mut materials, crate::graphics::glowing([disc.red, disc.green, disc.blue], crate::graphics::glow::SUN), false)),
            Transform::from_translation(toward_sun * (SKY - 60.0)),
            NotShadowCaster,
        ));
        let at = giant * (SKY - 160.0);
        heavens.spawn((
            Mesh3d(meshes.add(banded_sphere(150.0))),
            MeshMaterial3d(sky_material(&mut materials, Color::WHITE, false)),
            Transform::from_translation(at).with_rotation(Quat::from_rotation_z(0.35)),
            NotShadowCaster,
        ));
        heavens.spawn((
            Mesh3d(meshes.add(Annulus::new(190.0, 290.0))),
            MeshMaterial3d(sky_material(&mut materials, Color::srgba(0.92, 0.84, 0.90, 0.55), true)),
            Transform::from_translation(at).with_rotation(Quat::from_rotation_z(0.35) * Quat::from_rotation_x(-1.32)),
            NotShadowCaster,
        ));
        heavens.spawn((
            Mesh3d(meshes.add(Sphere::new(30.0).mesh().uv(32, 16))),
            MeshMaterial3d(sky_material(&mut materials, Color::srgb(0.86, 0.84, 0.92), false)),
            Transform::from_translation(moon * (SKY - 120.0)),
            NotShadowCaster,
        ));
    });
}

/// Mesh again the chunks whose blocks have changed (blocks picked up and put down), and their
/// neighbours, whose faces against the changed blocks may now show or hide.
fn rebuild_chunks(mut dirty: ResMut<DirtyCells>, world: Res<VoxelWorld>, mut meshes: ResMut<Assets<Mesh>>, chunks: Query<(&TerrainChunk, &Mesh3d)>) {
    if dirty.0.is_empty() {
        return;
    }
    let mut touched: Vec<IVec2> = Vec::new();
    for cell in dirty.0.drain(..) {
        for (dx, dz) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
            let chunk = IVec2::new((cell.x + dx).div_euclid(CHUNK) * CHUNK, (cell.z + dz).div_euclid(CHUNK) * CHUNK);
            if !touched.contains(&chunk) {
                touched.push(chunk);
            }
        }
    }
    for (chunk, handle) in &chunks {
        if !touched.contains(&IVec2::new(chunk.x0, chunk.z0)) {
            continue;
        }
        let (x1, z1) = ((chunk.x0 + CHUNK).min(world.size.x), (chunk.z0 + CHUNK).min(world.size.z));
        if let Some(mut mesh) = meshes.get_mut(&handle.0) {
            *mesh = world.mesh(chunk.x0, x1, chunk.z0, z1, chunk.glow);
        }
    }
}

fn follow_camera(camera: Single<&Transform, (With<Camera3d>, Without<SkyAnchor>)>, mut sky: Single<&mut Transform, With<SkyAnchor>>) {
    sky.translation = camera.translation;
}

/// The inside of a sphere, shaded from the horizon to the zenith, and warmed low in the sky around
/// the sun.
fn dome(sky: &Sky) -> Mesh {
    let mut mesh = Sphere::new(SKY).mesh().uv(64, 32);
    let horizon = sky.horizon.to_linear().to_vec3();
    let zenith = sky.zenith.to_linear().to_vec3();
    let glow = sky.glow.to_linear().to_vec3();
    let below = Color::srgb(0.46, 0.66, 0.70).to_linear().to_vec3();
    let sun_bearing = sky.toward_sun.with_y(0.0).normalize();
    let colours: Vec<[f32; 4]> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(positions)) => positions
            .iter()
            .map(|p| {
                let up = p[1] / SKY;
                // How much of the sun's warmth reaches this part of the sky: most under the sun, and
                // fading towards the zenith and round to the far side.
                let facing = Vec3::new(p[0], 0.0, p[2]).normalize_or_zero().dot(sun_bearing).max(0.0);
                let warmth = facing.powf(3.0) * (1.0 - up.abs()).powf(2.5) * 0.85;
                let low = horizon.lerp(glow, warmth);
                let c = if up >= 0.0 { low.lerp(zenith, up.powf(0.6)) } else { low.lerp(below, (-up * 4.0).min(1.0)) };
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

/// A small island for the horizon: a peak or two of rock under turf and a few groves.
fn distant_island(seed: i32) -> VoxelWorld {
    let size = IVec3::new(48, 40, 48);
    let mut world = VoxelWorld::new(size);
    for z in 0..size.z {
        for x in 0..size.x {
            let (xf, zf) = (x as f32, z as f32);
            let d = Vec2::new(xf - 24.0, zf - 24.0).length() / 22.0 + (fbm(xf / 9.0, zf / 9.0, 900 + seed, 3) - 0.5) * 0.5;
            if d >= 1.0 {
                continue;
            }
            let rise = (1.0 - d).powf(1.3);
            let top = 9 + (rise * (6.0 + 14.0 * fbm(xf / 7.0, zf / 7.0, 950 + seed, 2))) as i32;
            for y in 0..top {
                let material = if y == top - 1 { if top > 20 { Material::Rock } else { Material::Grass } } else { Material::Rock };
                world.set(IVec3::new(x, y, z), material);
            }
            if y_has_tree(x, z, seed) && top < 18 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_afternoon_sun_is_low_in_the_west() {
        let sky = Sky::of(Light::Afternoon);
        let elevation = sky.toward_sun.y.asin().to_degrees();
        assert!((20.0..40.0).contains(&elevation), "{elevation} degrees");
        assert!(sky.toward_sun.x < 0.0, "west is -x");
        // The sun is warmer than the sky's light in the shadows.
        let (sun, fill) = (sky.sun.to_srgba(), sky.fill.to_srgba());
        assert!(sun.red > sun.blue && fill.blue > fill.red);
    }
}
