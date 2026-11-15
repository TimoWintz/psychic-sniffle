//! Carrying salvage: pick up a crate, carry it, put it down.
//!
//! The crates on the canyon ledge are real blocks of the island: they can be stood on and measured.
//! `F` picks up the crate in front of the avatar and puts it down again, on the ledge north of the
//! launcher's rail. Put down facing the jack frame at the rim, a crate slides into the frame at its
//! foot and raises the ledge launcher one metre; `F` there takes one out again. The number of crates
//! in the frame is what the canyon station commits, and it is kept in the save. Loose crates are not:
//! they are back on the pile the next time the game starts.

use crate::island::Sites;
use crate::player::Player;
use crate::sim::Sim;
use crate::state::{Game, Mode, Ui};
use crate::voxel::VoxelWorld;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use physics_core::materials::Material;
use physics_core::puzzle::{Ch1Layout, PuzzleKind};

/// How far in front of the avatar's feet a crate is picked up or put down.
const AHEAD: f32 = 1.1;
/// How close to the jack frame's foot the avatar must stand to use it.
const FRAME_REACH: f32 = 1.9;
/// Where the carried crate is held, in the avatar's frame (its front is −Z).
pub const HELD_AT: Vec3 = Vec3::new(0.0, 1.05, -0.62);
const HELD_SIZE: f32 = 0.8;
const HANDLE: Color = Color::srgb(1.0, 0.56, 0.10);

/// What `F` would do now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Pick up the loose crate in this cell.
    Take(IVec3),
    /// Take a crate out of the jack frame.
    TakeFromFrame,
    /// Put the crate down in this cell.
    Put(IVec3),
    /// Slide the crate into the jack frame.
    PutInFrame,
    /// Nothing can be done here, and why.
    Blocked(&'static str),
}

/// The movable crates and the one being carried.
#[derive(Resource, Default)]
pub struct Carry {
    /// Cells of the crates lying loose (not in the jack frame, not carried).
    pub loose: Vec<IVec3>,
    /// Crates in the jack frame.
    pub stack: u32,
    pub holding: bool,
    /// What `F` would do, worked out each frame from where the avatar stands and faces.
    pub target: Option<Target>,
    /// The canyon puzzle's index in the chapter.
    canyon: usize,
}

/// Cells whose blocks changed since the terrain was last meshed; `scenery` rebuilds their chunks.
#[derive(Resource, Default)]
pub struct DirtyCells(pub Vec<IVec3>);

/// The orange handle and the physics engine's collider of one movable crate.
#[derive(Component)]
struct CrateMark(usize);

/// The crate in the avatar's hands.
#[derive(Component)]
struct Held;

pub struct CarryPlugin;

impl Plugin for CarryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Carry>().init_resource::<DirtyCells>().add_systems(
            Startup,
            lay_out.after(crate::scenery::spawn).after(crate::sim::setup).after(crate::rig::spawn),
        );
        app.add_systems(Update, (aim, act).chain().after(crate::player::move_player).run_if(in_state(Mode::Playing)))
            .add_systems(Update, (place_marks, hold, draw_target));
    }
}

/// Put the crates where the save says: as many in the jack frame as were left there, the rest on
/// the pile (the top layer leaves the pile first).
fn lay_out(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut world: ResMut<VoxelWorld>,
    mut sites: ResMut<Sites>,
    mut carry: ResMut<Carry>,
    mut dirty: ResMut<DirtyCells>,
    game: Res<Game>,
) {
    carry.canyon = game.chapter.puzzles.iter().position(|p| p.kind == PuzzleKind::Canyon).expect("the chapter has a canyon");
    let stack = game.placed(carry.canyon).min(Ch1Layout::MAX_STACK);
    let mut pile = sites.crate_pile.clone();
    // Bottom layer first, so the crates taken away are the top ones.
    pile.sort_by_key(|c| (c.y, c.z, c.x));
    pile.truncate(pile.len() - stack as usize);
    for cell in &pile {
        world.set(*cell, Material::Timber);
        dirty.0.push(*cell);
    }
    carry.loose = pile;
    set_stack(&mut world, &mut sites, &mut dirty, stack);
    carry.stack = stack;

    let handle = meshes.add(Cuboid::new(0.62, 0.07, 0.14));
    let glow = materials.add(StandardMaterial { base_color: HANDLE, emissive: LinearRgba::rgb(2.0, 0.9, 0.1), ..default() });
    for i in 0..Ch1Layout::MAX_STACK as usize {
        commands.spawn((
            CrateMark(i),
            RigidBody::Static,
            Collider::cuboid(1.0, 1.0, 1.0),
            Transform::from_translation(Vec3::NEG_Y * 100.0),
            Visibility::default(),
            children![(Mesh3d(handle.clone()), MeshMaterial3d(glow.clone()), Transform::from_xyz(0.0, 0.53, 0.0))],
        ));
    }
    let body = meshes.add(Cuboid::from_length(HELD_SIZE));
    let timber = materials.add(StandardMaterial { base_color: colour(Material::Timber), perceptual_roughness: 0.9, ..default() });
    commands.spawn((
        Held,
        Mesh3d(body),
        MeshMaterial3d(timber),
        Transform::default(),
        Visibility::Hidden,
        children![(Mesh3d(handle), MeshMaterial3d(glow), Transform::from_xyz(0.0, HELD_SIZE / 2.0 + 0.03, 0.0).with_scale(Vec3::splat(HELD_SIZE)))],
    ));
}

fn colour(material: Material) -> Color {
    let [r, g, b] = material.properties().colour;
    Color::srgb(r, g, b)
}

/// Fill the jack frame with `stack` crates and raise the launcher (and its survey pin) onto them.
pub fn set_stack(world: &mut VoxelWorld, sites: &mut Sites, dirty: &mut DirtyCells, stack: u32) {
    for k in 0..=Ch1Layout::MAX_STACK as i32 {
        let cell = sites.canyon_well + IVec3::Y * k;
        world.set(cell, if k < stack as i32 { Material::Timber } else { Material::Air });
        dirty.0.push(cell);
    }
    sites.canyon_launch.origin.y = (sites.canyon_well.y + stack as i32) as f32;
    let pin = sites.canyon_release_pin;
    sites.pins[pin].position = sites.canyon_launch.origin;
}

/// Whether a cell may take a crate that is put down: inside the yard, empty, on something solid.
fn can_put(world: &VoxelWorld, sites: &Sites, cell: IVec3) -> bool {
    let [low, high] = sites.crate_yard;
    cell.cmpge(low).all() && cell.cmple(high).all() && !world.is_solid(cell) && world.is_solid(cell - IVec3::Y)
}

/// Whether the avatar's body overlaps a block cell.
fn overlaps_avatar(feet: Vec3, cell: IVec3) -> bool {
    let (low, high) = (cell.as_vec3(), cell.as_vec3() + Vec3::ONE);
    let (a, b) = (feet - Vec3::new(0.3, 0.0, 0.3), feet + Vec3::new(0.3, 1.8, 0.3));
    a.x < high.x && b.x > low.x && a.y < high.y && b.y > low.y && a.z < high.z && b.z > low.z
}

/// What `F` would do for an avatar at `feet` facing along `forward` (horizontal, unit length).
pub fn target_for(world: &VoxelWorld, sites: &Sites, carry: &Carry, feet: Vec3, forward: Vec3) -> Option<Target> {
    let frame_foot = sites.canyon_well.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    let to_frame = (frame_foot - feet).with_y(0.0);
    let at_frame = to_frame.length() < FRAME_REACH && (feet.y - frame_foot.y).abs() < 0.6 && to_frame.normalize_or_zero().dot(forward) > 0.5;
    let level = (feet.y + 0.01).floor() as i32;
    let ahead = (feet + forward * AHEAD).floor().as_ivec3();
    if carry.holding {
        if at_frame {
            return Some(if carry.stack < Ch1Layout::MAX_STACK { Target::PutInFrame } else { Target::Blocked("The jack frame is full.") });
        }
        // On the ground in front, or on top of a crate there.
        for y in [level, level + 1] {
            let cell = IVec3::new(ahead.x, y, ahead.z);
            if can_put(world, sites, cell) && !overlaps_avatar(feet, cell) {
                return Some(Target::Put(cell));
            }
        }
        let [low, high] = sites.crate_yard;
        let in_yard = (low.x..=high.x).contains(&ahead.x) && (low.z..=high.z).contains(&ahead.z);
        return Some(Target::Blocked(if in_yard { "No room to put the crate down here." } else { "Crates go on the ledge, north of the launcher's rail." }));
    }
    if at_frame && carry.stack > 0 {
        return Some(Target::TakeFromFrame);
    }
    // The nearest loose crate in front with nothing on top of it, within reach of the hands.
    let reach_point = feet + forward * 0.9;
    carry
        .loose
        .iter()
        .filter(|c| !carry.loose.contains(&(**c + IVec3::Y)) && (level - 1..=level + 1).contains(&c.y))
        .map(|c| ((c.as_vec3() + Vec3::new(0.5, 0.0, 0.5) - reach_point).with_y(0.0).length(), *c))
        .filter(|(d, _)| *d < 1.2)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| Target::Take(c))
}

#[allow(clippy::too_many_arguments)]
fn aim(
    world: Res<VoxelWorld>,
    sites: Res<Sites>,
    ui: Res<Ui>,
    sim: Res<Sim>,
    mut carry: ResMut<Carry>,
    player: Single<&Transform, With<Player>>,
) {
    let busy = ui.station.is_some() || sim.busy();
    let forward = (player.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
    let target = if busy { None } else { target_for(&world, &sites, &carry, player.translation, forward) };
    if carry.target != target {
        carry.target = target;
    }
}

#[allow(clippy::too_many_arguments)]
fn act(
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    mut world: ResMut<VoxelWorld>,
    mut sites: ResMut<Sites>,
    mut carry: ResMut<Carry>,
    mut dirty: ResMut<DirtyCells>,
    mut game: ResMut<Game>,
) {
    if wants.wants_any_keyboard_input() || !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let Some(target) = carry.target else { return };
    apply(&mut world, &mut sites, &mut carry, &mut dirty, &mut game, target);
}

/// Do what a target says. Also used by the scripted tour.
pub fn apply(world: &mut VoxelWorld, sites: &mut Sites, carry: &mut Carry, dirty: &mut DirtyCells, game: &mut Game, target: Target) {
    match target {
        Target::Take(cell) => {
            let Some(i) = carry.loose.iter().position(|c| *c == cell) else { return };
            carry.loose.swap_remove(i);
            world.set(cell, Material::Air);
            dirty.0.push(cell);
            carry.holding = true;
        }
        Target::Put(cell) => {
            world.set(cell, Material::Timber);
            dirty.0.push(cell);
            carry.loose.push(cell);
            carry.holding = false;
        }
        Target::TakeFromFrame | Target::PutInFrame => {
            let stack = if target == Target::PutInFrame { carry.stack + 1 } else { carry.stack - 1 };
            set_stack(world, sites, dirty, stack);
            carry.stack = stack;
            carry.holding = target == Target::TakeFromFrame;
            let id = game.def(carry.canyon).id.clone();
            game.save.placed.insert(id, stack);
            game.persist();
        }
        Target::Blocked(_) => return,
    }
    carry.target = None;
}

/// Keep the handles and colliders on the crates: the loose ones, then the ones in the frame.
fn place_marks(carry: Res<Carry>, sites: Res<Sites>, mut marks: Query<(&CrateMark, &mut Transform, &mut Visibility)>) {
    if !carry.is_changed() {
        return;
    }
    let cells: Vec<IVec3> = carry.loose.iter().copied().chain((0..carry.stack as i32).map(|k| sites.canyon_well + IVec3::Y * k)).collect();
    for (mark, mut transform, mut visibility) in &mut marks {
        match cells.get(mark.0) {
            Some(cell) => {
                transform.translation = cell.as_vec3() + Vec3::splat(0.5);
                *visibility = Visibility::Inherited;
            }
            None => {
                transform.translation = Vec3::NEG_Y * 100.0;
                *visibility = Visibility::Hidden;
            }
        }
    }
}

/// The carried crate rides in front of the avatar, between its hands.
fn hold(carry: Res<Carry>, player: Query<&Transform, (With<Player>, Without<Held>)>, mut held: Single<(&mut Transform, &mut Visibility), With<Held>>) {
    let (transform, visibility) = &mut *held;
    let Ok(avatar) = player.single() else { return };
    **visibility = if carry.holding { Visibility::Inherited } else { Visibility::Hidden };
    transform.translation = avatar.translation + avatar.rotation * HELD_AT;
    transform.rotation = avatar.rotation;
}

/// Outline where `F` would act: the crate to lift, or the ghost of the one to put down.
fn draw_target(carry: Res<Carry>, sites: Res<Sites>, mut gizmos: Gizmos) {
    let Some(target) = carry.target else { return };
    let (cell, colour) = match target {
        Target::Take(cell) => (cell, Color::srgb(1.0, 0.85, 0.3)),
        Target::Put(cell) => (cell, Color::srgb(0.4, 0.9, 1.0)),
        Target::PutInFrame => (sites.canyon_well, Color::srgb(0.4, 0.9, 1.0)),
        Target::TakeFromFrame => (sites.canyon_well + IVec3::Y * (carry.stack as i32 - 1), Color::srgb(1.0, 0.85, 0.3)),
        Target::Blocked(_) => return,
    };
    gizmos.cube(Transform::from_translation(cell.as_vec3() + Vec3::splat(0.5)).with_scale(Vec3::splat(1.04)), colour);
}

/// The line the hints bar shows for what `F` would do.
pub fn hint(carry: &Carry) -> Option<String> {
    Some(match carry.target? {
        Target::Take(_) => "F — pick up the crate".into(),
        Target::TakeFromFrame => "F — take a crate out of the jack frame (the launcher comes down a metre)".into(),
        Target::Put(_) => "F — put the crate down".into(),
        Target::PutInFrame => "F — slide the crate into the jack frame (the launcher goes up a metre)".into(),
        Target::Blocked(why) => format!("Carrying a crate. {why}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::island;

    fn laid_out(seed: u64) -> (VoxelWorld, Sites, Carry) {
        let (mut world, sites) = island::build(&Ch1Layout::generate(seed));
        for cell in &sites.crate_pile {
            world.set(*cell, Material::Timber);
        }
        let carry = Carry { loose: sites.crate_pile.clone(), ..default() };
        (world, sites, carry)
    }

    #[test]
    fn a_crate_from_the_pile_goes_into_the_frame_and_raises_the_launcher() {
        for seed in [0, 7, 42] {
            let (mut world, mut sites, mut carry) = laid_out(seed);
            let base = sites.canyon_launch.origin;
            let mut dirty = DirtyCells::default();

            // Facing the pile from the yard's west side: the near crate on top comes first.
            let pile_west = sites.crate_pile.iter().map(|c| c.x).min().unwrap();
            let row = sites.crate_pile[0].z;
            let feet = Vec3::new(pile_west as f32 - 0.6, base.y, row as f32 + 0.5);
            let Some(Target::Take(cell)) = target_for(&world, &sites, &carry, feet, Vec3::X) else { panic!("seed {seed}: nothing to take") };
            assert_eq!(cell.y, base.y as i32 + 1, "the top crate is taken first");
            world.set(cell, Material::Air);
            carry.loose.retain(|c| *c != cell);
            carry.holding = true;

            // At the frame's foot, facing it.
            let foot = sites.canyon_well.as_vec3() + Vec3::new(-0.9, 0.0, 0.5);
            assert_eq!(target_for(&world, &sites, &carry, foot, Vec3::X), Some(Target::PutInFrame), "seed {seed}");
            // Facing away from it, the crate cannot be put on the rail.
            assert!(matches!(target_for(&world, &sites, &carry, foot, Vec3::Z), Some(Target::Blocked(_))), "seed {seed}");

            set_stack(&mut world, &mut sites, &mut dirty, 3);
            assert_eq!(sites.canyon_launch.origin, base + Vec3::Y * 3.0);
            assert_eq!(sites.pins[sites.canyon_release_pin].position, sites.canyon_launch.origin);
            assert!(world.is_solid(sites.canyon_well + IVec3::Y * 2));
            assert!(!world.is_solid(sites.canyon_well + IVec3::Y * 3));
            set_stack(&mut world, &mut sites, &mut dirty, 0);
            assert_eq!(sites.canyon_launch.origin, base);
        }
    }

    #[test]
    fn crates_are_put_down_only_in_the_yard_and_never_on_the_avatar() {
        let (world, sites, mut carry) = laid_out(3);
        carry.holding = true;
        let [low, _] = sites.crate_yard;
        let feet = Vec3::new(low.x as f32 + 2.5, low.y as f32, low.z as f32 + 0.5);
        assert_eq!(target_for(&world, &sites, &carry, feet, Vec3::Z), Some(Target::Put(IVec3::new(low.x + 2, low.y, low.z + 1))));
        // Facing out of the yard (north, off its edge).
        assert!(matches!(target_for(&world, &sites, &carry, feet, Vec3::NEG_Z), Some(Target::Blocked(_))));
        assert!(overlaps_avatar(feet, feet.floor().as_ivec3()));
    }
}
