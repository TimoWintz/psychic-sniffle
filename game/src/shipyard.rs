//! What solving puts in the world: the ship, which grows on the shipyard floor as puzzles are
//! solved, and the launchers' guidance unit, which rides the crawler until it is caught.
//!
//! - The haul track: the guidance unit sits on the crawler, then at the foot of the transfer crane.
//! - The canyon: the frame beam's cradle appears on the shipyard floor.
//! - The ridge (the chapter's capstone): the frame itself, a keel, ribs and two rails, on the cradle.

use crate::island::Sites;
use crate::state::{Game, Mode};
use bevy::prelude::*;
use physics_core::materials::Material;
use physics_core::puzzle::PuzzleKind;

/// Something on the shipyard floor that rises into place when it first appears.
#[derive(Component)]
struct Rising {
    /// 0 when it starts to appear, 1 when fully in place.
    built: f32,
}

#[derive(Component)]
struct SkiffFrame;

#[derive(Component)]
struct Cradle;

/// The launchers' guidance unit. One rides on the crawler (a child of it, so it goes along in
/// experiments and tests) until the haul track is solved; the other waits at the crane's foot after.
#[derive(Component)]
pub struct GuidanceUnit {
    pub riding: bool,
}

pub const GUIDANCE_SIZE: Vec3 = Vec3::new(0.7, 0.4, 0.55);

/// The guidance unit's look: hull alloy with Lumen's cyan glowing in it.
pub fn guidance_parts(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> [(Mesh3d, MeshMaterial3d<StandardMaterial>, Transform); 2] {
    let c = Material::HullAlloy.properties().colour;
    let alloy = materials.add(StandardMaterial { base_color: Color::srgb(c[0], c[1], c[2]), metallic: 0.6, perceptual_roughness: 0.4, ..default() });
    let glow = materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.84, 1.0), emissive: LinearRgba::rgb(0.6, 2.2, 3.0), ..default() });
    [
        (Mesh3d(meshes.add(Cuboid::from_size(GUIDANCE_SIZE))), MeshMaterial3d(alloy), Transform::default()),
        // A lens on top, the colour of Lumen's eye.
        (Mesh3d(meshes.add(Cuboid::new(0.28, 0.06, 0.28))), MeshMaterial3d(glow), Transform::from_xyz(0.0, GUIDANCE_SIZE.y / 2.0 + 0.03, 0.0)),
    ]
}

pub struct ShipyardPlugin;

impl Plugin for ShipyardPlugin {
    fn build(&self, app: &mut App) {
        // What solving put there appears as the game begins, not during the crash.
        app.add_systems(Update, ((install_cradle, install_frame, place_guidance_unit).run_if(in_state(Mode::Playing)), assemble));
    }
}

fn solved(game: &Game, kind: PuzzleKind) -> bool {
    game.chapter.puzzles.iter().position(|p| p.kind == kind).is_some_and(|i| game.solved(i))
}

fn alloy(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    let c = Material::HullAlloy.properties().colour;
    materials.add(StandardMaterial { base_color: Color::srgb(c[0], c[1], c[2]), metallic: 0.6, perceptual_roughness: 0.4, ..default() })
}

fn rising_at(sites: &Sites) -> (Rising, Transform, Visibility) {
    (Rising { built: 0.0 }, Transform::from_translation(sites.shipyard).with_scale(Vec3::new(1.0, 0.01, 1.0)), Visibility::default())
}

/// The canyon's delivery: two cradle blocks for the keel to rest in.
fn install_cradle(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    game: Res<Game>,
    sites: Res<Sites>,
    existing: Query<(), With<Cradle>>,
) {
    if !existing.is_empty() || !solved(&game, PuzzleKind::Canyon) {
        return;
    }
    let alloy = alloy(&mut materials);
    let block = meshes.add(Cuboid::new(0.8, 0.8, 3.0));
    commands.spawn((Cradle, rising_at(&sites))).with_children(|cradle| {
        for x in [-4.0, 4.0] {
            cradle.spawn((Mesh3d(block.clone()), MeshMaterial3d(alloy.clone()), Transform::from_xyz(x, 0.4, 0.0)));
        }
    });
}

/// Chapter 1.1's part: the skiff's frame, a keel, ribs and two rails, laid in the cradle.
fn install_frame(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    game: Res<Game>,
    sites: Res<Sites>,
    existing: Query<(), With<SkiffFrame>>,
) {
    if !existing.is_empty() || !game.save.chapter_complete(&game.chapter) {
        return;
    }
    let alloy = alloy(&mut materials);
    let mut bar = |size: Vec3, centre: Vec3| (Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(alloy.clone()), Transform::from_translation(centre));
    commands.spawn((SkiffFrame, rising_at(&sites))).with_children(|frame| {
        // Keel, then ribs widest amidships, joined by a rail on each side.
        frame.spawn(bar(Vec3::new(14.0, 0.5, 0.5), Vec3::new(0.0, 1.05, 0.0)));
        for i in 0..7 {
            let x = -6.0 + 2.0 * i as f32;
            let half_beam = 2.6 * (1.0 - (x / 7.5).powi(2));
            frame.spawn(bar(Vec3::new(0.3, 0.3, 2.0 * half_beam), Vec3::new(x, 1.45, 0.0)));
            for side in [-1.0, 1.0] {
                frame.spawn(bar(Vec3::new(0.3, 1.6, 0.3), Vec3::new(x, 2.2, side * half_beam)));
            }
        }
        for side in [-1.0, 1.0] {
            frame.spawn(bar(Vec3::new(12.4, 0.25, 0.25), Vec3::new(0.0, 3.0, side * 2.2)));
        }
    });
}

/// The guidance unit rides the crawler until the haul track is solved, then waits at the crane.
fn place_guidance_unit(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    game: Res<Game>,
    sites: Res<Sites>,
    mut units: Query<(&GuidanceUnit, &mut Visibility)>,
    mut landed: Local<bool>,
) {
    let caught = solved(&game, PuzzleKind::Chase);
    for (unit, mut visibility) in &mut units {
        let show = unit.riding != caught;
        visibility.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
    if caught && !*landed {
        *landed = true;
        let [body, lens] = guidance_parts(&mut meshes, &mut materials);
        let at = sites.crane_foot + Vec3::Y * GUIDANCE_SIZE.y / 2.0;
        commands.spawn((GuidanceUnit { riding: false }, body.0, body.1, Transform::from_translation(at), Visibility::Inherited)).with_child(lens);
    }
}

/// What appears on the shipyard floor rises into place over a couple of seconds.
fn assemble(time: Res<Time>, mut parts: Query<(&mut Rising, &mut Transform)>) {
    for (mut part, mut transform) in &mut parts {
        if part.built < 1.0 {
            part.built = (part.built + time.delta_secs() / 2.5).min(1.0);
            let eased = 1.0 - (1.0 - part.built).powi(3);
            transform.scale = Vec3::new(1.0, eased.max(0.01), 1.0);
        }
    }
}
