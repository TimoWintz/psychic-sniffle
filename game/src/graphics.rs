//! How the picture is made: the quality setting, and what the camera does with the light it gathers
//! (high dynamic range, tonemapping, the grade, bloom, anti-aliasing, ambient occlusion). How bright
//! each glowing thing is lives here too, so the order of what glows is decided in one place.
//!
//! The order, brightest first: the builders' quartz once it is awake, then the wild quartz, then the
//! sky's bodies. The castaway's marks (orange paint, the yellow pins, the handles) never glow: they
//! are lit like everything else, and the grade keeps them the most saturated colours on screen.

use bevy::anti_alias::smaa::{Smaa, SmaaPreset};
use bevy::asset::embedded_asset;
use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, ScreenSpaceAmbientOcclusion};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection, Msaa};
use bevy::shader::ShaderRef;
use serde::Deserialize;
use std::path::Path;

/// How much the picture costs to draw. `Low` runs on integrated graphics; `High` adds ambient
/// occlusion, finer shadows and a sharper edge filter.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    #[default]
    Low,
    High,
}

#[derive(Debug, Default, Deserialize)]
struct SettingsFile {
    graphics: Option<GraphicsSettings>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct GraphicsSettings {
    quality: Option<Quality>,
}

impl Quality {
    /// The quality a settings file asks for: `[graphics] quality = "low"` or `"high"`. Anything else,
    /// or no file, is `Low`; a file that can't be read says why on the console.
    pub fn parse(source: &str) -> Result<Quality, String> {
        let file: SettingsFile = toml::from_str(source).map_err(|e| e.message().to_string())?;
        Ok(file.graphics.and_then(|g| g.quality).unwrap_or_default())
    }

    /// `GAME_QUALITY` (`low` or `high`) wins over the settings file, so a benchmark can run both.
    pub fn load(path: &Path) -> Quality {
        if let Ok(name) = std::env::var("GAME_QUALITY") {
            return Quality::parse(&format!("[graphics]\nquality = \"{name}\"")).unwrap_or_else(|e| {
                warn!("GAME_QUALITY: {e}");
                Quality::Low
            });
        }
        match std::fs::read_to_string(path) {
            Ok(text) => Quality::parse(&text).unwrap_or_else(|e| {
                warn!("{}: {e}", path.display());
                Quality::Low
            }),
            Err(_) => Quality::Low,
        }
    }
}

/// How many times brighter than its colour a glowing thing is drawn. Above 1 the camera's bloom
/// spreads it into a halo; at 1 or below it is simply bright.
pub mod glow {
    /// The builders' quartz, awake: in the road, the machines, the consoles, the lamps, the cables.
    pub const WAY: f32 = 2.2;
    /// Quartz that grows wild on the island: softer, so the way stays the brightest thread.
    pub const WILD: f32 = 1.4;
    /// The survey orange set into the stone (markers, strips, plates): bright, never a halo.
    pub const PAINT: f32 = 0.9;
    /// The sun's disc.
    pub const SUN: f32 = 6.0;

    // The way outshines the wild quartz, and the paint never glows.
    const _: () = assert!(WAY > WILD && WILD > 1.0 && PAINT <= 1.0);
}

/// How much more saturated a glowing colour is drawn than its material's colour. Bright light fades
/// towards white on screen; the quartz's pale teal would glow white without this.
const GLOW_SATURATION: f32 = 2.6;

/// A colour given in sRGB, drawn `strength` times brighter than itself: for unlit materials and the
/// glowing blocks' vertex colours. Above 1 it is also saturated, so it glows in its own colour.
pub fn glowing(srgb: [f32; 3], strength: f32) -> Color {
    let srgb = if strength > 1.0 { saturated(srgb, GLOW_SATURATION) } else { srgb };
    let c = Color::srgb(srgb[0], srgb[1], srgb[2]).to_linear();
    Color::LinearRgba(LinearRgba::rgb(c.red * strength, c.green * strength, c.blue * strength))
}

/// An sRGB colour pushed `k` times further from its own grey, scaled back to fit if a channel
/// passes 1, so its hue holds.
fn saturated(c: [f32; 3], k: f32) -> [f32; 3] {
    let grey = (c[0] + c[1] + c[2]) / 3.0;
    let pushed = c.map(|x| (grey + (x - grey) * k).max(0.0));
    let top = pushed.into_iter().fold(1.0, f32::max);
    pushed.map(|x| x / top)
}

/// The blocks' surfaces: the standard material, with a fine pattern for each material drawn inside
/// every face (`blocks.wesl`). A mesh tells it each vertex's material in its first UVs
/// (`block_uv`); a mesh without UVs is drawn plain.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct BlockSurface {
    /// How strongly the pattern shows, 0 to 1.
    #[uniform(100)]
    pub detail: f32,
}

impl MaterialExtension for BlockSurface {
    fn fragment_shader() -> ShaderRef {
        "embedded://game/blocks.wesl".into()
    }
}

pub type BlockMaterial = ExtendedMaterial<StandardMaterial, BlockSurface>;

/// The material every block of the world is drawn with: its colours come from the vertices.
pub fn block_material(materials: &mut Assets<BlockMaterial>) -> Handle<BlockMaterial> {
    materials.add(BlockMaterial {
        base: StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, ..default() },
        extension: BlockSurface { detail: 1.0 },
    })
}

/// A vertex's first UVs on a block: its material, for the pattern to choose by.
pub fn block_uv(material: physics_core::materials::Material) -> [f32; 2] {
    [material.id() as f32 / 32.0, 0.0]
}

pub struct GraphicsPlugin;

impl Plugin for GraphicsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "blocks.wesl");
        app.add_plugins(MaterialPlugin::<BlockMaterial>::default());
        // `GAME_SETTINGS` overrides where the settings file is looked for, as for the tutor.
        let path = std::env::var("GAME_SETTINGS").unwrap_or_else(|_| "settings.toml".into());
        let quality = Quality::load(Path::new(&path));
        info!("graphics quality: {quality:?}");
        app.insert_resource(quality)
            .insert_resource(DirectionalLightShadowMap { size: if quality == Quality::High { 4096 } else { 2048 } })
            .add_systems(PostStartup, (dress_camera, shape_shadows));
    }
}

/// The camera's look: everything here works on any machine, apart from what `High` adds.
fn dress_camera(mut commands: Commands, quality: Res<Quality>, camera: Single<Entity, With<Camera3d>>) {
    let mut camera = commands.entity(*camera);
    camera.insert((Hdr, Tonemapping::TonyMcMapface, Bloom { intensity: 0.18, low_frequency_boost: 0.6, ..Bloom::NATURAL }, grade()));
    match *quality {
        Quality::Low => {
            camera.insert(Msaa::Sample4);
        }
        Quality::High => {
            // Ambient occlusion reads the depth and normals of every pixel, which multisampling
            // can't give it: the edges are smoothed afterwards instead.
            camera.insert((Msaa::Off, Smaa { preset: SmaaPreset::High }, ScreenSpaceAmbientOcclusion::default()));
        }
    }
}

/// The tonemapper keeps bright colours from clipping but leaves the picture a little soft: give back
/// some contrast and colour, most of it in the midtones where the turf and the stone are.
pub fn grade() -> ColorGrading {
    ColorGrading {
        global: ColorGradingGlobal { exposure: -0.1, post_saturation: 1.05, ..default() },
        shadows: ColorGradingSection { saturation: 1.05, contrast: 1.0, gamma: 1.0, gain: 1.0, lift: -0.01 },
        midtones: ColorGradingSection { saturation: 1.15, contrast: 1.1, gamma: 1.0, gain: 1.0, lift: 0.0 },
        highlights: ColorGradingSection { saturation: 1.0, contrast: 1.0, gamma: 1.0, gain: 1.0, lift: 0.0 },
    }
}

/// The sun's shadows: sharp near the avatar, out to where the haze takes over.
fn shape_shadows(mut commands: Commands, quality: Res<Quality>, sun: Single<Entity, With<DirectionalLight>>) {
    let config = CascadeShadowConfigBuilder {
        num_cascades: if *quality == Quality::High { 4 } else { 3 },
        first_cascade_far_bound: 18.0,
        maximum_distance: 220.0,
        ..default()
    };
    commands.entity(*sun).insert(config.build());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_is_read_from_the_graphics_section() {
        assert_eq!(Quality::parse("[graphics]\nquality = \"high\""), Ok(Quality::High));
        assert_eq!(Quality::parse("[graphics]\nquality = \"low\""), Ok(Quality::Low));
        assert_eq!(Quality::parse("[tutor]\nbase_url = \"x\"\nmodel = \"y\""), Ok(Quality::Low));
        assert_eq!(Quality::parse(""), Ok(Quality::Low));
        assert!(Quality::parse("[graphics]\nquality = \"ultra\"").is_err());
        assert!(Quality::parse("[graphics]\nqualty = \"high\"").is_err());
    }

    #[test]
    fn every_material_reaches_the_blocks_shader() {
        // The shader reads the id back as round(uv.x * 32): every id must survive that, and stay
        // within 0 to 1 in case the UVs are stored compressed.
        for material in physics_core::materials::Material::ALL {
            let [u, _] = block_uv(material);
            assert!((0.0..1.0).contains(&u));
            assert_eq!((u * 32.0).round() as u8, material.id());
        }
    }

    #[test]
    fn a_glow_keeps_its_hue_and_what_does_not_glow_keeps_its_colour() {
        let quartz = [0.62, 0.96, 0.92];
        let lit = glowing(quartz, glow::WAY).to_linear();
        // Green leads, red falls further behind than in the material's own colour.
        assert!(lit.green > lit.blue && lit.blue > lit.red);
        assert!(lit.red / lit.green < Color::srgb(0.62, 0.96, 0.92).to_linear().red / Color::srgb(0.62, 0.96, 0.92).to_linear().green);
        assert_eq!(glowing(quartz, 1.0), Color::LinearRgba(Color::srgb(0.62, 0.96, 0.92).to_linear()));
    }
}
