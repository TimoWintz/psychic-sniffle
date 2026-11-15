//! Launcher rigs: the launchers show what the blueprint says before anything is committed.
//!
//! The tilting launcher's guide arm swings to the angle typed in, and its gauge fills to the speed
//! typed in, measured against the launcher's rating. This is the player's own number made visible;
//! nothing here previews a path or judges anything. The ledge launcher's governor is jammed: its
//! gauge stands full at its rated speed, and the launcher rides on a cradle in a jack frame, on top
//! of however many crates the player has slid in (see `carry`).

use crate::island::{LaunchSite, Sites};
use crate::sim::Sim;
use crate::state::{Game, Ui};
use bevy::prelude::*;
use physics_core::expr::evaluate;
use physics_core::puzzle::{Ch1Layout, PuzzleKind};
use physics_core::units::{Dim, Quantity};

/// Length of the tilting launcher's guide arm, from its hinge at the release point.
const ARM_LENGTH: f32 = 2.2;
/// Angle of the arm when the blueprint has no usable angle in it.
const ARM_REST: f32 = 20.0 * std::f32::consts::PI / 180.0;
/// Height of a gauge's scale, from empty to the launcher's rating.
const GAUGE_HEIGHT: f32 = 2.0;
const FILL: Color = Color::srgb(1.0, 0.56, 0.10);
const OVER: Color = Color::srgb(1.0, 0.16, 0.10);

#[derive(Component)]
struct Arm {
    puzzle: usize,
    /// Current angle above the horizontal, radians; eases toward the blueprint's.
    angle: f32,
    base: Quat,
    axis: Vec3,
}

/// The ledge launcher's cradle, on top of the crates in the jack frame.
#[derive(Component)]
struct Cradle;

#[derive(Component)]
struct GaugeFill {
    puzzle: usize,
    /// A launcher that fires at one speed only shows that speed, whatever is typed.
    fixed: Option<f64>,
    /// Speed at the top of the scale, m/s: the launcher's rating.
    limit: f64,
    /// Current fill, as a fraction of the scale; eases toward the blueprint's.
    level: f32,
    /// Centre of the empty scale's bottom edge, in the gauge's frame.
    bottom: Vec3,
}

pub struct RigPlugin;

impl Plugin for RigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn).add_systems(Update, (follow_blueprint, ride_the_stack));
    }
}

fn puzzle_of(game: &Game, kind: PuzzleKind) -> usize {
    game.chapter.puzzles.iter().position(|p| p.kind == kind).expect("the chapter has this kind of puzzle")
}

pub(crate) fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sites: Res<Sites>, game: Res<Game>) {
    let steel = materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.57, 0.62), metallic: 0.6, perceptual_roughness: 0.4, ..default() });
    let dark = materials.add(StandardMaterial { base_color: Color::srgb(0.10, 0.11, 0.14), perceptual_roughness: 0.7, ..default() });
    let mark = materials.add(StandardMaterial { base_color: Color::srgb(0.92, 0.95, 1.0), unlit: true, ..default() });

    // The tilting launcher's guide arm: two rails hinged at the release point, which the beam rides
    // up on its way out. They lie along the start of the path the beam flies.
    let ridge = puzzle_of(&game, PuzzleKind::Ridge);
    let site = sites.ridge_launch;
    let across = site.direction.cross(Vec3::Y).normalize();
    let base = Quat::from_rotation_arc(Vec3::X, site.direction);
    let rail = meshes.add(Cuboid::new(ARM_LENGTH, 0.1, 0.08));
    let hinge = meshes.add(Cuboid::new(0.25, 0.25, 0.9));
    commands.spawn((Mesh3d(hinge), MeshMaterial3d(dark.clone()), Transform::from_translation(site.origin - Vec3::Y * 0.1).with_rotation(base)));
    commands
        .spawn((
            Arm { puzzle: ridge, angle: ARM_REST, base, axis: across },
            Transform::from_translation(site.origin).with_rotation(Quat::from_axis_angle(across, ARM_REST) * base),
            Visibility::default(),
        ))
        .with_children(|arm| {
            for side in [-0.33, 0.33] {
                arm.spawn((Mesh3d(rail.clone()), MeshMaterial3d(steel.clone()), Transform::from_xyz(ARM_LENGTH / 2.0, 0.05, side)));
            }
        });

    // The jack frame at the rim: two uprights either side of the column the crates stack in, as
    // tall as the frame holds, and the launcher's cradle riding on the stack.
    let frame_foot = sites.canyon_well.as_vec3();
    let tall = Ch1Layout::MAX_STACK as f32 + 0.6;
    let upright = meshes.add(Cuboid::new(0.14, tall, 0.14));
    for (x, z) in [(0.07, -0.08), (0.93, -0.08), (0.07, 1.08), (0.93, 1.08)] {
        commands.spawn((Mesh3d(upright.clone()), MeshMaterial3d(steel.clone()), Transform::from_translation(frame_foot + Vec3::new(x, tall / 2.0, z))));
    }
    let cradle = meshes.add(Cuboid::new(1.0, 0.12, 1.0));
    let back_stop = meshes.add(Cuboid::new(0.1, 0.5, 0.9));
    commands
        .spawn((Cradle, Mesh3d(cradle), MeshMaterial3d(dark.clone()), Transform::from_translation(cradle_at(&sites)), Visibility::default()))
        .with_children(|c| {
            c.spawn((Mesh3d(back_stop), MeshMaterial3d(steel.clone()), Transform::from_xyz(-0.45, 0.3, 0.0)));
        });

    // A speed gauge beside each launcher, on the station's side. The tilting launcher's is rated to
    // what it accepts and follows the blueprint; the ledge launcher's stands full at its one speed.
    let canyon = puzzle_of(&game, PuzzleKind::Canyon);
    let gauges = [
        (canyon, sites.canyon_launch, game.layout.canyon_speed, Some(game.layout.canyon_speed)),
        (ridge, sites.ridge_launch, game.layout.v_max, None),
    ];
    let post = meshes.add(Cuboid::new(0.12, GAUGE_HEIGHT + 0.7, 0.12));
    let panel = meshes.add(Cuboid::new(0.42, GAUGE_HEIGHT + 0.16, 0.06));
    let fill = meshes.add(Cuboid::new(0.28, 1.0, 0.09));
    let line = meshes.add(Cuboid::new(0.56, 0.05, 0.1));
    for (puzzle, site, limit, fixed) in gauges {
        let foot = gauge_foot(&site);
        let frame = Transform::from_translation(foot).with_rotation(Quat::from_rotation_arc(Vec3::X, site.direction));
        let bottom = Vec3::new(0.0, 0.6, 0.02);
        let material = materials.add(StandardMaterial { base_color: FILL, unlit: true, ..default() });
        commands.spawn((frame, Visibility::default())).with_children(|gauge| {
            gauge.spawn((Mesh3d(post.clone()), MeshMaterial3d(dark.clone()), Transform::from_xyz(0.0, (GAUGE_HEIGHT + 0.7) / 2.0, -0.08)));
            gauge.spawn((Mesh3d(panel.clone()), MeshMaterial3d(dark.clone()), Transform::from_translation(bottom + Vec3::new(0.0, GAUGE_HEIGHT / 2.0, -0.02))));
            // The rating: the top of the scale.
            gauge.spawn((Mesh3d(line.clone()), MeshMaterial3d(mark.clone()), Transform::from_translation(bottom + Vec3::new(0.0, GAUGE_HEIGHT, 0.02))));
            gauge.spawn((
                GaugeFill { puzzle, fixed, limit, level: 0.0, bottom },
                Mesh3d(fill.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(bottom).with_scale(Vec3::new(1.0, 0.001, 1.0)),
            ));
        });
    }
}

/// Where the ledge launcher's cradle sits: on top of the jack frame's crates, under the load.
fn cradle_at(sites: &Sites) -> Vec3 {
    sites.canyon_launch.origin - sites.canyon_launch.direction * 0.5 - Vec3::Y * 0.06
}

fn ride_the_stack(sites: Res<Sites>, mut cradle: Single<&mut Transform, With<Cradle>>) {
    if sites.is_changed() {
        cradle.translation = cradle_at(&sites);
    }
}

/// Where a launcher's gauge stands: behind the release point, toward the station's side.
fn gauge_foot(site: &LaunchSite) -> Vec3 {
    let across = site.direction.cross(Vec3::Y).normalize();
    site.origin - site.direction * 1.5 + across * 1.6
}

/// The blueprint's value for an input of a puzzle, if what is typed there is a quantity of the
/// right kind.
fn typed(game: &Game, ui: &Ui, puzzle: usize, key: &str, dim: Dim) -> Option<Quantity> {
    let index = game.instance(puzzle).inputs().iter().position(|spec| spec.key == key)?;
    let text = ui.drafts[puzzle].get(index)?.trim();
    if text.is_empty() {
        return None;
    }
    evaluate(text, &game.save.notebook).ok().filter(|q| q.dim == dim && q.value.is_finite())
}

fn follow_blueprint(
    time: Res<Time>,
    game: Res<Game>,
    ui: Res<Ui>,
    sim: Res<Sim>,
    mut arms: Query<(&mut Arm, &mut Transform)>,
    mut fills: Query<(&mut GaugeFill, &mut Transform, &MeshMaterial3d<StandardMaterial>), Without<Arm>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // While a test or an experiment runs, the launchers hold what was committed.
    if sim.busy() {
        return;
    }
    let ease = (time.delta_secs() * 5.0).min(1.0);
    for (mut arm, mut transform) in &mut arms {
        let target = typed(&game, &ui, arm.puzzle, "angle", Dim::NONE)
            .map(|q| q.value as f32)
            .filter(|a| (1f32.to_radians()..=89f32.to_radians()).contains(a))
            .unwrap_or(ARM_REST);
        arm.angle += (target - arm.angle) * ease;
        transform.rotation = Quat::from_axis_angle(arm.axis, arm.angle) * arm.base;
    }
    for (mut gauge, mut transform, material) in &mut fills {
        let speed = gauge.fixed.unwrap_or_else(|| typed(&game, &ui, gauge.puzzle, "speed", Dim::SPEED).map_or(0.0, |q| q.value.max(0.0)));
        let over = speed > gauge.limit;
        let target = (speed / gauge.limit).min(1.0) as f32;
        gauge.level += (target - gauge.level) * ease;
        let height = (gauge.level * GAUGE_HEIGHT).max(0.001);
        transform.scale.y = height;
        transform.translation = gauge.bottom + Vec3::Y * height / 2.0;
        if let Some(mut m) = materials.get_mut(&material.0) {
            let colour = if over { OVER } else { FILL };
            if m.base_color != colour {
                m.base_color = colour;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::island;
    use crate::voxel::VoxelWorld;
    use physics_core::puzzle::Ch1Layout;

    fn solid_below(world: &VoxelWorld, at: Vec3) -> bool {
        world.is_solid((at - Vec3::Y * 0.5).floor().as_ivec3())
    }

    #[test]
    fn gauges_stand_on_solid_ground_beside_their_launchers() {
        for seed in 0..40 {
            let layout = Ch1Layout::generate(seed);
            let (world, sites) = island::build(&layout);
            for site in [sites.canyon_launch, sites.ridge_launch] {
                let foot = gauge_foot(&site);
                assert!(solid_below(&world, foot), "seed {seed}: the gauge at {foot} has nothing under it");
                assert!(!world.is_solid((foot + Vec3::Y * 0.5).floor().as_ivec3()), "seed {seed}: the gauge at {foot} is buried");
            }
        }
    }
}
