//! The way wakes: the ancient road's machines sleep at the start and wake one by one as the
//! chapter's puzzles pass.
//!
//! Everything follows from one number, the **stage**: how many of the chapter's puzzles are solved
//! (they open in order, so that is also the index of the next one). At stage `k`:
//!
//! - machine `i` (the five stations, then the north gate) is lit when `k ≥ 1` and `i ≤ k`: the one
//!   whose job was done and the one the light ran on to; the first sleeps until its job is done,
//!   though its console works;
//! - the stretch of inlay from machine `i` to the next is lit when `i < k`;
//! - console `i` is wired in when `i ≤ k` (the station is open);
//! - the stone gate is open from stage 1, the ramp onto the outcrop stands from stage 3, the seal at
//!   the ridge is down from stage 4, and the north gate's beam shines at stage 5.
//!
//! The island is built awake (`island::build`); [`settle`] puts it to the stage the save is at
//! before anything is meshed or collides. The world shows the stage in [`Waking::shown`], which
//! trails the save's while a wake plays.

use crate::carry::DirtyCells;
use crate::island::{MACHINES, Way};
use crate::sim::Sim;
use crate::state::{Game, PUZZLES};
use crate::voxel::VoxelWorld;
use bevy::prelude::*;
use physics_core::materials::Material;

/// The stage the save is at: how many of the chapter's puzzles are solved.
pub fn stage(game: &Game) -> usize {
    game.next_unsolved().unwrap_or(PUZZLES)
}

pub fn machine_lit(machine: usize, stage: usize) -> bool {
    stage >= 1 && machine <= stage
}

pub fn run_lit(run: usize, stage: usize) -> bool {
    run < stage
}

pub fn console_shown(station: usize, stage: usize) -> bool {
    station <= stage
}

pub fn gate_open(stage: usize) -> bool {
    stage >= 1
}

pub fn ramp_up(stage: usize) -> bool {
    stage >= 3
}

pub fn seal_open(stage: usize) -> bool {
    stage >= 4
}

pub fn beam_shines(stage: usize) -> bool {
    stage >= 5
}

/// Light or put out blocks of the way's quartz, listing the ones that change.
pub fn light(world: &mut VoxelWorld, cells: impl IntoIterator<Item = IVec3>, lit: bool, changed: &mut Vec<IVec3>) {
    for cell in cells {
        if world.is_dark(cell) == lit {
            world.set_dark(cell, !lit);
            changed.push(cell);
        }
    }
}

/// Raise the ramp's blocks up to `height` blocks above the ground (all of it for `i32::MAX`), and
/// take away the rest, listing the cells that change.
pub fn raise_ramp(world: &mut VoxelWorld, way: &Way, height: i32, changed: &mut Vec<IVec3>) {
    for &(cell, material) in &way.ramp {
        let want = if cell.y - crate::island::GROUND < height { material } else { Material::Air };
        if world.get(cell) != want {
            world.set(cell, want);
            changed.push(cell);
        }
    }
}

/// Put the world in the state of a stage at once: the quartz lit or dark, the ramp up or down.
/// Returns the cells that changed, for meshing again.
pub fn settle(world: &mut VoxelWorld, way: &Way, stage: usize) -> Vec<IVec3> {
    let mut changed = Vec::new();
    for machine in 0..MACHINES {
        light(world, way.quartz[machine].iter().copied(), machine_lit(machine, stage), &mut changed);
    }
    for (run, inlay) in way.runs.iter().enumerate() {
        light(world, inlay.cells.iter().map(|&(c, _)| c), run_lit(run, stage), &mut changed);
    }
    raise_ramp(world, way, if ramp_up(stage) { i32::MAX } else { 0 }, &mut changed);
    changed
}

/// What the world shows of the way.
#[derive(Resource, Debug)]
pub struct Waking {
    /// The stage the world shows.
    pub shown: usize,
}

impl Waking {
    pub fn console_shown(&self, station: usize) -> bool {
        console_shown(station, self.shown)
    }

    pub fn gate_is_open(&self) -> bool {
        gate_open(self.shown)
    }
}

/// The way's parts that move rather than light: the stone gate's leaf and the seal at the ridge,
/// which sink into the ground, and the north gate's beam, which shines across the strait.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
enum Part {
    GateLeaf,
    RidgeSeal,
    NorthBeam,
}

impl Part {
    /// How far open, from 0 (shut, dark) to 1 (sunk, shining), at a stage.
    fn open_at(self, stage: usize) -> f32 {
        let open = match self {
            Part::GateLeaf => gate_open(stage),
            Part::RidgeSeal => seal_open(stage),
            Part::NorthBeam => beam_shines(stage),
        };
        open as u8 as f32
    }
}

/// Height of the leaves that sink: the stone gate's fills it to its lintel.
const LEAF_HEIGHT: f32 = 3.0;
const SEAL_HEIGHT: f32 = 2.4;
/// How far the north gate's beam reaches towards the main hull.
const BEAM_LENGTH: f32 = 260.0;

pub struct WakingPlugin;

impl Plugin for WakingPlugin {
    fn build(&self, app: &mut App) {
        let shown = stage(app.world().resource::<Game>());
        app.insert_resource(Waking { shown }).add_systems(Startup, spawn).add_systems(Update, (catch_up, pose_parts).chain());
    }
}

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sites: Res<crate::island::Sites>) {
    let way = &sites.way;
    let stone = Material::Limestone.properties().colour;
    let stone = materials.add(StandardMaterial { base_color: Color::srgb(stone[0], stone[1], stone[2]), perceptual_roughness: 0.9, ..default() });
    let quartz = Material::Quartz.properties().colour;
    let light = materials.add(StandardMaterial { base_color: Color::srgba(quartz[0], quartz[1], quartz[2], 0.55), unlit: true, alpha_mode: AlphaMode::Add, ..default() });
    // The stone gate's leaf fills the rail between its posts, up to the lintel.
    commands.spawn((Part::GateLeaf, Mesh3d(meshes.add(Cuboid::new(0.5, LEAF_HEIGHT, 1.0))), MeshMaterial3d(stone.clone()), Transform::from_translation(way.stone_gate)));
    // The seal across the road at the ridge's east end: a slab between two standing stones.
    commands.spawn((Part::RidgeSeal, Mesh3d(meshes.add(Cuboid::new(3.2, SEAL_HEIGHT, 0.45))), MeshMaterial3d(stone.clone()), Transform::from_translation(way.ridge_seal)));
    let standing = meshes.add(Cuboid::new(0.8, 3.0, 0.8));
    for side in [-2.2, 2.2] {
        commands.spawn((Mesh3d(standing.clone()), MeshMaterial3d(stone.clone()), Transform::from_translation(way.ridge_seal + Vec3::new(side, 1.5, 0.0))));
    }
    // The north gate's beam, from its keystone out over the strait towards the main hull.
    let towards = (crate::intro::Mainland::wreck() - way.keystone).with_y(0.0).normalize() + Vec3::Y * 0.08;
    let rotation = Quat::from_rotation_arc(Vec3::Y, towards.normalize());
    commands.spawn((
        Part::NorthBeam,
        Mesh3d(meshes.add(Cylinder::new(0.35, BEAM_LENGTH))),
        MeshMaterial3d(light),
        Transform::from_translation(way.keystone + towards.normalize() * BEAM_LENGTH / 2.0).with_rotation(rotation),
        bevy::light::NotShadowCaster,
    ));
}

/// Put the moving parts where the shown stage has them.
fn pose_parts(waking: Res<Waking>, sites: Res<crate::island::Sites>, mut parts: Query<(&Part, &mut Transform, &mut Visibility)>) {
    if !waking.is_changed() {
        return;
    }
    let way = &sites.way;
    for (part, mut transform, mut visibility) in &mut parts {
        let open = part.open_at(waking.shown);
        match part {
            Part::GateLeaf => transform.translation = way.stone_gate + Vec3::Y * (LEAF_HEIGHT / 2.0 - (LEAF_HEIGHT + 0.1) * open),
            Part::RidgeSeal => transform.translation = way.ridge_seal + Vec3::Y * (SEAL_HEIGHT / 2.0 - (SEAL_HEIGHT + 0.1) * open),
            Part::NorthBeam => {}
        }
        // A leaf sunk all the way is out of sight; the beam shows once it shines at all.
        let seen = if *part == Part::NorthBeam { open > 0.0 } else { open < 1.0 };
        visibility.set_if_neq(if seen { Visibility::Inherited } else { Visibility::Hidden });
    }
}

/// Once a pass has been watched, bring the world up to the save's stage.
fn catch_up(game: Res<Game>, sim: Res<Sim>, mut waking: ResMut<Waking>, mut world: ResMut<VoxelWorld>, sites: Res<crate::island::Sites>, mut dirty: ResMut<DirtyCells>) {
    let saved = stage(&game);
    if saved == waking.shown || sim.busy() || sim.watching() {
        return;
    }
    dirty.0.extend(settle(&mut world, &sites.way, saved));
    waking.shown = saved;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::island::{self, GROUND};
    use physics_core::puzzle::Ch1Layout;
    use std::collections::{HashSet, VecDeque};

    #[test]
    fn the_rules_at_each_stage() {
        // Stage 0: everything asleep, only the first console in.
        assert!((0..MACHINES).all(|m| !machine_lit(m, 0)));
        assert!(console_shown(0, 0) && !console_shown(1, 0));
        // The first pass lights the first machine, the run from it and the second machine.
        assert!(machine_lit(0, 1) && machine_lit(1, 1) && !machine_lit(2, 1));
        assert!(run_lit(0, 1) && !run_lit(1, 1));
        assert!(gate_open(1) && !gate_open(0));
        assert!(!ramp_up(2) && ramp_up(3));
        assert!(!seal_open(3) && seal_open(4));
        assert!(!beam_shines(4) && beam_shines(5));
        // At the end all of it is lit.
        assert!((0..MACHINES).all(|m| machine_lit(m, 5)));
        assert!((0..MACHINES - 1).all(|r| run_lit(r, 5)));
    }

    /// Every cell of the world, and which are dark.
    fn snapshot(world: &VoxelWorld) -> (Vec<Material>, Vec<IVec3>) {
        let mut cells = Vec::new();
        let mut dark = Vec::new();
        for y in 0..world.size.y {
            for z in 0..world.size.z {
                for x in 0..world.size.x {
                    let p = IVec3::new(x, y, z);
                    cells.push(world.get(p));
                    if world.is_dark(p) {
                        dark.push(p);
                    }
                }
            }
        }
        (cells, dark)
    }

    #[test]
    fn put_to_sleep_and_woken_stage_by_stage_it_is_the_island_as_built() {
        for seed in 0..12 {
            let (built, sites) = island::build(&Ch1Layout::generate(seed));
            let mut world = built.clone();
            assert!(!settle(&mut world, &sites.way, 0).is_empty(), "seed {seed}: nothing sleeps");
            for &(cell, _) in &sites.way.ramp {
                assert_eq!(world.get(cell), Material::Air, "seed {seed}: the ramp stands asleep");
            }
            for stage in 1..=PUZZLES {
                settle(&mut world, &sites.way, stage);
                // Settling again changes nothing.
                assert!(settle(&mut world, &sites.way, stage).is_empty(), "seed {seed}");
            }
            assert!(snapshot(&world) == snapshot(&built), "seed {seed}: awake, the island is not as built");
        }
    }

    /// Where the avatar can stand, starting from `from`: a block with two clear above it, reached
    /// from a neighbouring column by climbing at most `climb` blocks or dropping any distance.
    fn reachable(world: &VoxelWorld, from: IVec3, climb: i32) -> HashSet<IVec3> {
        let stands = |p: IVec3| world.is_solid(p - IVec3::Y) && !world.is_solid(p) && !world.is_solid(p + IVec3::Y);
        let mut seen = HashSet::from([from]);
        let mut queue = VecDeque::from([from]);
        while let Some(p) = queue.pop_front() {
            for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                let n = p + d;
                let mut next = None;
                // Up: clear overhead from where it stands to where it lands.
                for up in (1..=climb).rev() {
                    let q = n + IVec3::Y * up;
                    if stands(q) && (1..=up + 1).all(|h| !world.is_solid(p + IVec3::Y * (h + 1))) {
                        next = Some(q);
                        break;
                    }
                }
                // Level, or walking off an edge and falling.
                if next.is_none() && !world.is_solid(n) && !world.is_solid(n + IVec3::Y) {
                    let mut q = n;
                    while q.y > 0 && !world.is_solid(q - IVec3::Y) {
                        q -= IVec3::Y;
                    }
                    if stands(q) {
                        next = Some(q);
                    }
                }
                if let Some(q) = next
                    && seen.insert(q)
                {
                    queue.push_back(q);
                }
            }
        }
        seen
    }

    #[test]
    fn the_ledge_is_out_of_reach_until_the_ramp_rises() {
        for seed in 0..24 {
            let layout = Ch1Layout::generate(seed);
            let (mut world, sites) = island::build(&layout);
            // The highest a jump lifts the avatar's feet, on this planet, and so the highest block
            // it can get onto.
            let apex = crate::player::JUMP_SPEED.powi(2) / (2.0 * layout.g as f32);
            let climb = (apex.floor() as i32).max(1);
            let spawn = sites.spawn.floor().as_ivec3();
            let ledge = sites.canyon_well;
            settle(&mut world, &sites.way, 2);
            assert!(!reachable(&world, spawn, climb).contains(&ledge), "seed {seed}: the ledge can be climbed before the ramp rises (climb {climb})");
            settle(&mut world, &sites.way, 3);
            assert!(reachable(&world, spawn, climb).contains(&ledge), "seed {seed}: the ramp does not lead onto the ledge");
            assert_eq!(ledge.y, GROUND + layout.ledge_h as i32);
        }
    }
}
