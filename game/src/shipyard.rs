//! The ship, which grows on the shipyard floor as chapters are completed.

use crate::island::Sites;
use crate::state::Game;
use bevy::prelude::*;
use physics_core::materials::Material;

#[derive(Component)]
struct SkiffFrame {
    /// 0 when it starts to appear, 1 when fully assembled.
    built: f32,
}

pub struct ShipyardPlugin;

impl Plugin for ShipyardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (install_frame, assemble));
    }
}

/// Chapter 1.1's part: the skiff's frame — a keel, ribs and two rails.
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
    let c = Material::HullAlloy.properties().colour;
    let alloy = materials.add(StandardMaterial {
        base_color: Color::srgb(c[0], c[1], c[2]),
        metallic: 0.6,
        perceptual_roughness: 0.4,
        ..default()
    });
    let mut bar = |size: Vec3, centre: Vec3| (Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(alloy.clone()), Transform::from_translation(centre));
    commands
        .spawn((SkiffFrame { built: 0.0 }, Transform::from_translation(sites.shipyard).with_scale(Vec3::new(1.0, 0.01, 1.0)), Visibility::default()))
        .with_children(|frame| {
            // Cradle blocks, keel, then ribs widest amidships, joined by a rail on each side.
            for x in [-4.0, 4.0] {
                frame.spawn(bar(Vec3::new(0.8, 0.8, 3.0), Vec3::new(x, 0.4, 0.0)));
            }
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

/// The frame rises into place over a couple of seconds.
fn assemble(time: Res<Time>, mut frames: Query<(&mut SkiffFrame, &mut Transform)>) {
    for (mut frame, mut transform) in &mut frames {
        if frame.built < 1.0 {
            frame.built = (frame.built + time.delta_secs() / 2.5).min(1.0);
            let eased = 1.0 - (1.0 - frame.built).powi(3);
            transform.scale = Vec3::new(1.0, eased.max(0.01), 1.0);
        }
    }
}
