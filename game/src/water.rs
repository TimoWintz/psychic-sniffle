//! The sea: one plane at the water's surface, drawn by its own shader (`water.wesl`). What the shader
//! needs to know about the island under the water is baked into a small map, one texel per column
//! of the block grid: how deep the water stands there, and how near the nearest shore or cliff is.
//! From those it colours shallows and depths, and draws foam where the water meets the land. The map
//! follows the world: when blocks at the water's level change (the bridge rising), the texels around
//! them are worked out again.

use crate::carry::DirtyCells;
use crate::graphics::Quality;
use crate::island::SEA_LEVEL;
use crate::scenery::Sky;
use crate::state::{BuildWorld, Game};
use crate::voxel::VoxelWorld;
use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::image::ImageSampler;
use bevy::light::NotShadowCaster;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;

/// The deepest water the map holds; deeper reads as this deep.
const DEEPEST: f32 = 12.0;
/// How far from the land the foam and the shallows' light reach, in metres.
const REACH: f32 = 6.0;

/// The water's colours and the map's size, as the shader reads them. Colours are linear, with the
/// water's opacity in alpha.
#[derive(ShaderType, Reflect, Debug, Clone, Copy)]
pub struct WaterLook {
    pub shallow: Vec4,
    pub deep: Vec4,
    pub foam: Vec4,
    /// The sky the water reflects: at the horizon and overhead.
    pub horizon: Vec4,
    pub zenith: Vec4,
    /// The map's width and depth in metres, the depth that its red channel's 1 stands for, and the
    /// reach of its green channel.
    pub extent: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WaterSurface {
    #[uniform(100)]
    pub look: WaterLook,
    /// Red: the water's depth over `DEEPEST`. Green: 1 at the shore, falling to 0 at `REACH`.
    #[texture(101)]
    #[sampler(102)]
    pub map: Handle<Image>,
}

impl MaterialExtension for WaterSurface {
    fn fragment_shader() -> ShaderRef {
        "embedded://game/water.wesl".into()
    }
}

pub type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterSurface>;

/// The map's image, and which columns are dry, for working parts of it out again.
#[derive(Resource)]
struct WaterMap {
    image: Handle<Image>,
    dry: Vec<bool>,
}

pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "water.wesl");
        app.add_plugins(MaterialPlugin::<WaterMaterial>::default())
            .add_systems(Startup, spawn)
            .add_systems(BuildWorld, refresh_all)
            .add_systems(Update, follow_the_land.before(crate::scenery::rebuild_chunks));
    }
}

/// Whether a column stands above the water: a block of it reaches the surface.
fn is_dry(world: &VoxelWorld, x: i32, z: i32) -> bool {
    world.is_solid(IVec3::new(x, SEA_LEVEL.floor() as i32, z))
}

/// How deep the water stands over a column: from the surface down to the first block under it.
pub fn depth(world: &VoxelWorld, x: i32, z: i32) -> f32 {
    let top = (0..SEA_LEVEL.floor() as i32).rev().find(|&y| world.is_solid(IVec3::new(x, y, z)));
    top.map_or(SEA_LEVEL, |y| SEA_LEVEL - (y + 1) as f32).max(0.0)
}

/// How far a column's centre is from the nearest dry column's edge, up to `REACH`.
fn distance_to_land(dry: &[bool], size: IVec2, x: i32, z: i32) -> f32 {
    let r = REACH.ceil() as i32 + 1;
    let mut nearest = REACH;
    for dz in -r..=r {
        for dx in -r..=r {
            let (nx, nz) = (x + dx, z + dz);
            if nx < 0 || nz < 0 || nx >= size.x || nz >= size.y || !dry[(nz * size.x + nx) as usize] {
                continue;
            }
            // From this centre to the nearest point of that column's square.
            let gap = Vec2::new((dx.abs() as f32 - 0.5).max(0.0), (dz.abs() as f32 - 0.5).max(0.0)).length();
            nearest = nearest.min(gap);
        }
    }
    nearest
}

/// One texel of the map: depth, then nearness to land.
pub fn texel(world: &VoxelWorld, dry: &[bool], x: i32, z: i32) -> [u8; 4] {
    let size = IVec2::new(world.size.x, world.size.z);
    if dry[(z * size.x + x) as usize] {
        return [0, 255, 0, 255];
    }
    let deep = (depth(world, x, z) / DEEPEST).clamp(0.0, 1.0);
    let near = 1.0 - distance_to_land(dry, size, x, z) / REACH;
    [(deep * 255.0).round() as u8, (near.clamp(0.0, 1.0) * 255.0).round() as u8, 0, 255]
}

/// Work out the map's texels in the columns from `lo` to `hi`, and the dryness they depend on.
fn refresh(world: &VoxelWorld, dry: &mut [bool], data: &mut [u8], lo: IVec2, hi: IVec2) {
    let size = IVec2::new(world.size.x, world.size.z);
    let reach = REACH.ceil() as i32 + 1;
    let (lo, hi) = (lo.max(IVec2::ZERO), hi.min(size - 1));
    for z in lo.y..=hi.y {
        for x in lo.x..=hi.x {
            dry[(z * size.x + x) as usize] = is_dry(world, x, z);
        }
    }
    // A change of dryness moves the foam up to the reach away.
    let (lo, hi) = ((lo - reach).max(IVec2::ZERO), (hi + reach).min(size - 1));
    for z in lo.y..=hi.y {
        for x in lo.x..=hi.x {
            let i = ((z * size.x + x) * 4) as usize;
            data[i..i + 4].copy_from_slice(&texel(world, dry, x, z));
        }
    }
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    world: Res<VoxelWorld>,
    game: Res<Game>,
    quality: Res<Quality>,
) {
    let size = IVec2::new(world.size.x, world.size.z);
    let mut dry = vec![false; (size.x * size.y) as usize];
    let mut data = vec![0u8; (size.x * size.y * 4) as usize];
    refresh(&world, &mut dry, &mut data, IVec2::ZERO, size - 1);
    let mut map = Image::new(
        Extent3d { width: size.x as u32, height: size.y as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    map.sampler = ImageSampler::linear();
    let image = images.add(map);

    let sky = Sky::of(game.chapter.light);
    let linear = |c: Color, alpha: f32| c.to_linear().to_vec3().extend(alpha);
    let look = WaterLook {
        shallow: linear(Color::srgb(0.30, 0.80, 0.76), 0.45),
        deep: linear(Color::srgb(0.05, 0.33, 0.42), 0.92),
        foam: linear(Color::srgb(0.93, 0.97, 0.96), 1.0),
        horizon: linear(sky.horizon, 1.0),
        zenith: linear(sky.zenith, 1.0),
        extent: Vec4::new(size.x as f32, size.y as f32, DEEPEST, REACH),
    };
    let material = materials.add(WaterMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: if *quality == Quality::High { 0.06 } else { 0.08 },
            reflectance: 0.3,
            ..default()
        },
        extension: WaterSurface { look, map: image.clone() },
    });
    // Finely divided near the island, so the waves' light is smooth; the map's edge holds beyond it.
    let plane = Plane3d::default().mesh().size(1800.0, 1800.0).subdivisions(8);
    commands.spawn((
        Mesh3d(meshes.add(plane)),
        MeshMaterial3d(material),
        Transform::from_xyz(size.x as f32 / 2.0, SEA_LEVEL, size.y as f32 / 2.0),
        NotShadowCaster,
    ));
    commands.insert_resource(WaterMap { image, dry });
}

/// Refresh the entire water depth/foam map after a world rebuild.
fn refresh_all(world: Res<VoxelWorld>, mut map: ResMut<WaterMap>, mut images: ResMut<Assets<Image>>) {
    let size = IVec2::new(world.size.x, world.size.z);
    let WaterMap { image, dry } = &mut *map;
    if let Some(mut image) = images.get_mut(image.id())
        && let Some(data) = image.data.as_mut()
    {
        refresh(&world, dry, data, IVec2::ZERO, size - 1);
    }
}

/// When blocks at the water's level change, work the map out again around them.
fn follow_the_land(dirty: Res<DirtyCells>, world: Res<VoxelWorld>, mut map: ResMut<WaterMap>, mut images: ResMut<Assets<Image>>) {
    let level = SEA_LEVEL.floor() as i32;
    let mut changed = dirty.0.iter().filter(|c| c.y <= level).map(|c| IVec2::new(c.x, c.z));
    let Some(first) = changed.next() else { return };
    let (lo, hi) = changed.fold((first, first), |(lo, hi), c| (lo.min(c), hi.max(c)));
    let WaterMap { image, dry } = &mut *map;
    if let Some(mut image) = images.get_mut(image.id())
        && let Some(data) = image.data.as_mut()
    {
        refresh(&world, dry, data, lo, hi);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics_core::materials::Material;

    /// A sea floor two blocks deep, with an island of three columns at one side.
    fn bay() -> VoxelWorld {
        let level = SEA_LEVEL.floor() as i32;
        let mut world = VoxelWorld::new(IVec3::new(16, level + 4, 16));
        world.fill(IVec3::new(0, 0, 0), IVec3::new(15, level - 2, 15), Material::Sand);
        world.fill(IVec3::new(0, 0, 0), IVec3::new(2, level + 1, 15), Material::Rock);
        world
    }

    #[test]
    fn the_map_knows_the_depth_and_the_shore() {
        let world = bay();
        let size = IVec2::new(16, 16);
        let mut dry = vec![false; 256];
        let mut data = vec![0u8; 256 * 4];
        refresh(&world, &mut dry, &mut data, IVec2::ZERO, size - 1);
        let at = |x: i32| {
            let i = ((8 * 16 + x) * 4) as usize;
            (data[i], data[i + 1])
        };
        // On the land: dry, with the foam's full strength at its edge.
        assert_eq!(at(1), (0, 255));
        // The water's depth: from the surface to the top of the floor.
        let floor_top = (SEA_LEVEL.floor() as i32 - 1) as f32;
        assert!((depth(&world, 8, 8) - (SEA_LEVEL - floor_top)).abs() < 1e-5);
        // Beside the land the foam is strongest; it fades out to the reach.
        let (_, next_to_land) = at(3);
        let (_, further) = at(6);
        let (_, beyond) = at(12);
        assert!(next_to_land > further && further > beyond, "{next_to_land} {further} {beyond}");
        assert_eq!(beyond, 0);
    }

    #[test]
    fn a_column_rising_out_of_the_water_brings_foam_round_it() {
        let mut world = bay();
        let size = IVec2::new(16, 16);
        let mut dry = vec![false; 256];
        let mut data = vec![0u8; 256 * 4];
        refresh(&world, &mut dry, &mut data, IVec2::ZERO, size - 1);
        let near = |data: &[u8], x: i32, z: i32| data[((z * 16 + x) * 4 + 1) as usize];
        assert_eq!(near(&data, 13, 8), 0);
        // A pylon rises at (12, 8), as the bridge's do.
        world.fill(IVec3::new(12, 0, 8), IVec3::new(12, SEA_LEVEL.floor() as i32 + 2, 8), Material::Limestone);
        refresh(&world, &mut dry, &mut data, IVec2::new(12, 8), IVec2::new(12, 8));
        assert!(near(&data, 13, 8) > 200, "foam beside the pylon");
    }
}
