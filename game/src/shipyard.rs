//! What solving puts in the world: the launchers' sight, which rides the crawler until it is
//! caught, and the ship, which grows on the shipyard floor on the north shore. (The frame beam's own
//! journey there, across the canyon and over the ridge, is `sim`'s.)
//!
//! - The haul track: the sight sits on the crawler, then at the foot of the transfer crane.
//! - Local gravity: calibrated, the sight is fitted beside the ledge launcher.
//! - The ridge (the chapter's capstone): the frame itself, a keel, ribs and two rails, in its cradle,
//!   pointing down the slipway into the strait.

use crate::island::Sites;
use crate::state::{Game, Mode};
use crate::waking::Waking;
use bevy::prelude::*;
use physics_core::materials::Material;

/// Something on the shipyard floor that rises into place when it first appears.
#[derive(Component)]
struct Rising {
    /// 0 when it starts to appear, 1 when fully in place.
    built: f32,
}

#[derive(Component)]
struct SkiffFrame;

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

pub struct ShipyardPlugin;

impl Plugin for ShipyardPlugin {
    fn build(&self, app: &mut App) {
        // What solving put there appears as the game begins, not during the crash.
        app.add_systems(Update, ((install_frame, place_sight).run_if(in_state(Mode::Playing)), assemble));
    }
}


fn alloy(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    let c = Material::HullAlloy.properties().colour;
    materials.add(StandardMaterial { base_color: Color::srgb(c[0], c[1], c[2]), metallic: 0.6, perceptual_roughness: 0.4, ..default() })
}

/// What rises on the shipyard floor is built along x and turned to lie along the slipway, which runs
/// north to the strait.
fn rising_at(sites: &Sites) -> (Rising, Transform, Visibility) {
    let along_slipway = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    (Rising { built: 0.0 }, Transform::from_translation(sites.shipyard).with_rotation(along_slipway).with_scale(Vec3::new(1.0, 0.01, 1.0)), Visibility::default())
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
        // The cradle: two blocks for the keel to rest in.
        for x in [-4.0, 4.0] {
            frame.spawn(bar(Vec3::new(0.8, 0.8, 3.0), Vec3::new(x, 0.4, 0.0)));
        }
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

/// The sight rides the crawler until the crane lifts it off in the wake after the haul track's pass,
/// then waits at the crane's foot, and once the launchers know this planet's g it is set beside the
/// ledge launcher (by evening, behind the fade). Where it is is the way's to say (see `waking`).
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
    let ledge = sites.canyon_launch;
    let side = ledge.direction.cross(Vec3::Y).normalize();
    let mut at = ledge.origin - ledge.direction * 3.0 + side * 1.0;
    at.y = sites.canyon_well.y as f32;
    at
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
