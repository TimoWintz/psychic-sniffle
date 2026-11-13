//! The island, generated from the same seeded layout the puzzles read.
//!
//! Every distance a puzzle depends on is built here in whole blocks from [`Ch1Layout`], so the number
//! the player measures and the number the simulator uses are the same.

use crate::state::PUZZLES;
use crate::voxel::{VoxelWorld, hash01};
use bevy::prelude::*;
use physics_core::materials::Material;
use physics_core::puzzle::Ch1Layout;

pub const WORLD_SIZE: IVec3 = IVec3::new(176, 72, 128);
/// Height of the ground surface over most of the island.
pub const GROUND: i32 = 20;
pub const SEA_LEVEL: f32 = 8.6;

/// A named point the ruler snaps to.
#[derive(Clone, Debug)]
pub struct Pin {
    pub position: Vec3,
    pub label: &'static str,
}

/// A launcher: where the load is released and which way it flies.
#[derive(Clone, Copy, Debug)]
pub struct LaunchSite {
    /// Release point of the load's underside.
    pub origin: Vec3,
    pub direction: Vec3,
}

impl LaunchSite {
    /// World position for a point `x` metres along the launch line at height `y` above the far
    /// ground level of the puzzle.
    pub fn at(&self, x: f64, y: f64, ground: f32) -> Vec3 {
        let mut p = self.origin + self.direction * x as f32;
        p.y = ground + y as f32;
        p
    }
}

/// Where everything on the island is.
#[derive(Resource, Clone, Debug)]
pub struct Sites {
    pub spawn: Vec3,
    /// Underside centre of the rail cart where it starts; it runs along +x.
    pub rail_start: Vec3,
    /// Underside of the test mass at rest, and where it lands.
    pub tower_release: Vec3,
    #[cfg_attr(not(test), expect(dead_code, reason = "the game reads it through its pin; tests check it directly"))]
    pub tower_plate: Vec3,
    pub mast_release: Vec3,
    pub mast_plate: Vec3,
    pub canyon_launch: LaunchSite,
    pub ridge_launch: LaunchSite,
    /// Console block of each station, in puzzle order.
    pub stations: [IVec3; PUZZLES],
    pub pins: Vec<Pin>,
    pub wreck: Vec3,
    /// Centre of the shipyard floor, where the ship is assembled.
    pub shipyard: Vec3,
}

/// Block the rail cart starts on, and the row the supply rail runs along (eastward).
const RAIL_X: i32 = 28;
const RAIL_ROW: i32 = 47;
const TOWER: IVec3 = IVec3::new(36, GROUND, 30);
const MAST: IVec3 = IVec3::new(28, GROUND, 22);
/// Near (west) rim of the canyon, where the ledge launcher releases.
const CANYON_X: i32 = 70;
const CANYON_ROW: i32 = 56;
const CANYON_Z: (i32, i32) = (28, 84);
/// Release point of the tilting launcher.
const RIDGE_LAUNCH_X: i32 = 40;
const RIDGE_ROW: i32 = 100;

pub fn build(layout: &Ch1Layout) -> (VoxelWorld, Sites) {
    let mut world = VoxelWorld::new(WORLD_SIZE);
    terrain(&mut world);

    let ground = GROUND as f32;
    let mut pins = Vec::new();

    // --- Puzzle 0 (tutorial): the supply rail ------------------------------------------------
    // A steel rail flush with the grass, a buffer stop past its far end, a start mark and a marker
    // across the rail. The survey pins sit on the near side of the rail, where the cart does not hide
    // them, so the ruler reads the run along the rail exactly.
    let rail_len = Ch1Layout::RAIL_LENGTH as i32;
    world.fill(IVec3::new(RAIL_X - 2, GROUND - 1, RAIL_ROW), IVec3::new(RAIL_X + rail_len, GROUND - 1, RAIL_ROW), Material::Steel);
    world.set(IVec3::new(RAIL_X + rail_len + 1, GROUND, RAIL_ROW), Material::Steel);
    world.set(IVec3::new(RAIL_X, GROUND - 1, RAIL_ROW - 1), Material::Concrete);
    let marker_x = RAIL_X + layout.rail_target as i32;
    world.fill(IVec3::new(marker_x, GROUND - 1, RAIL_ROW - 1), IVec3::new(marker_x, GROUND - 1, RAIL_ROW + 1), Material::Marker);
    let rail_start = Vec3::new(RAIL_X as f32 + 0.5, ground, RAIL_ROW as f32 + 0.5);
    let beside = Vec3::new(0.0, 0.0, -1.0);
    let rail_start_pin = Pin { position: rail_start + beside, label: "Rail cart: starting point" };
    let rail_marker_pin = Pin { position: rail_start + beside + Vec3::X * layout.rail_target as f32, label: "Rail marker: centre" };
    let station0 = IVec3::new(RAIL_X - 1, GROUND, RAIL_ROW - 3);

    // --- Puzzle 1: drop tower and survey mast -------------------------------------------------
    let gantry = |world: &mut VoxelWorld, base: IVec3, height: i32| -> (Vec3, Vec3) {
        // A steel column with an arm; the mass hangs under the end of the arm, two blocks out.
        let top = base.y + height + 1;
        world.fill(base, IVec3::new(base.x, top, base.z), Material::Steel);
        world.fill(IVec3::new(base.x, top, base.z), IVec3::new(base.x + 2, top, base.z), Material::Steel);
        let plate = IVec3::new(base.x + 2, base.y - 1, base.z);
        world.set(plate, Material::Marker);
        let foot = Vec3::new(plate.x as f32 + 0.5, base.y as f32, plate.z as f32 + 0.5);
        (foot + Vec3::Y * height as f32, foot)
    };
    let (tower_release, tower_plate) = gantry(&mut world, TOWER, layout.tower_h as i32);
    let (mast_release, mast_plate) = gantry(&mut world, MAST, layout.mast_h as i32);
    pins.push(Pin { position: tower_release, label: "Drop tower: release point" });
    pins.push(Pin { position: tower_plate, label: "Drop tower: landing plate" });
    pins.push(Pin { position: mast_release, label: "Survey mast: release point" });
    pins.push(Pin { position: mast_plate, label: "Survey mast: landing plate" });
    let station1 = IVec3::new(TOWER.x + 1, GROUND, TOWER.z + 5);

    // --- Puzzle 2: ledge, canyon, platform ----------------------------------------------------
    let ledge_top = GROUND + layout.ledge_h as i32;
    let far_rim = CANYON_X + layout.canyon_width as i32;
    // The canyon, with a floor that climbs in steps at its south end so nobody is trapped in it.
    for z in CANYON_Z.0..CANYON_Z.1 {
        let climb = (z - (CANYON_Z.1 - layout.canyon_depth as i32)).max(0);
        let floor = GROUND - layout.canyon_depth as i32 + climb;
        for x in CANYON_X..far_rim {
            world.fill(IVec3::new(x, floor, z), IVec3::new(x, WORLD_SIZE.y - 1, z), Material::Air);
            world.set(IVec3::new(x, floor - 1, z), Material::Sand);
        }
    }
    // The rock outcrop that carries the launcher, and steps up its west side.
    world.fill(IVec3::new(56, GROUND, 50), IVec3::new(CANYON_X - 1, ledge_top - 1, 62), Material::Rock);
    for step in 0..layout.ledge_h as i32 {
        let x = 55 - step;
        world.fill(IVec3::new(x, GROUND, 54), IVec3::new(x, ledge_top - 2 - step, 58), Material::Rock);
    }
    // Launcher rail along the launch line, ending at the rim.
    world.fill(IVec3::new(CANYON_X - 4, ledge_top - 1, CANYON_ROW), IVec3::new(CANYON_X - 1, ledge_top - 1, CANYON_ROW), Material::Steel);
    let canyon_launch = LaunchSite {
        origin: Vec3::new(CANYON_X as f32, ledge_top as f32, CANYON_ROW as f32 + 0.5),
        direction: Vec3::X,
    };
    // Landing platform: three by three marker blocks set flush into the far plateau.
    let platform_mid = CANYON_X + layout.platform_centre.floor() as i32;
    world.fill(
        IVec3::new(platform_mid - 1, GROUND - 1, CANYON_ROW - 1),
        IVec3::new(platform_mid + 1, GROUND - 1, CANYON_ROW + 1),
        Material::Marker,
    );
    pins.push(Pin { position: canyon_launch.origin, label: "Ledge launcher: release point" });
    pins.push(Pin { position: canyon_launch.at(layout.platform_centre, 0.0, ground), label: "Platform: centre" });
    let station2 = IVec3::new(CANYON_X - 6, ledge_top, CANYON_ROW + 3);

    // --- Puzzle 3: tilting launcher, ridge, pad -----------------------------------------------
    let ridge_x = RIDGE_LAUNCH_X + layout.ridge_x as i32;
    world.fill(
        IVec3::new(ridge_x, GROUND, RIDGE_ROW - 10),
        IVec3::new(ridge_x, GROUND + layout.ridge_h as i32 - 1, RIDGE_ROW + 11),
        Material::Rock,
    );
    let pad_mid = RIDGE_LAUNCH_X + layout.pad_centre.floor() as i32;
    world.fill(
        IVec3::new(pad_mid - 2, GROUND - 1, RIDGE_ROW - 2),
        IVec3::new(pad_mid + 2, GROUND - 1, RIDGE_ROW + 2),
        Material::Marker,
    );
    world.fill(
        IVec3::new(RIDGE_LAUNCH_X - 3, GROUND - 1, RIDGE_ROW),
        IVec3::new(RIDGE_LAUNCH_X - 1, GROUND - 1, RIDGE_ROW),
        Material::Steel,
    );
    let ridge_launch = LaunchSite {
        origin: Vec3::new(RIDGE_LAUNCH_X as f32, ground, RIDGE_ROW as f32 + 0.5),
        direction: Vec3::X,
    };
    pins.push(Pin { position: ridge_launch.origin, label: "Tilting launcher: release point" });
    pins.push(Pin {
        position: ridge_launch.at(layout.ridge_x as f64, layout.ridge_h as f64, ground),
        label: "Ridge: top, near edge",
    });
    pins.push(Pin { position: ridge_launch.at(layout.ridge_x as f64, 0.0, ground), label: "Ridge: foot, near side" });
    pins.push(Pin { position: ridge_launch.at(layout.pad_centre, 0.0, ground), label: "Shipyard pad: centre" });
    let station3 = IVec3::new(RIDGE_LAUNCH_X - 3, GROUND, RIDGE_ROW + 4);

    // --- Shipyard floor beyond the pad --------------------------------------------------------
    let yard_x = pad_mid + 7;
    world.fill(IVec3::new(yard_x, GROUND - 1, RIDGE_ROW - 7), IVec3::new(yard_x + 19, GROUND - 1, RIDGE_ROW + 7), Material::Concrete);
    let shipyard = Vec3::new(yard_x as f32 + 10.0, ground, RIDGE_ROW as f32 + 0.5);

    // --- The wreck: a broken hull section near the start --------------------------------------
    let wreck = IVec3::new(16, GROUND, 40);
    for i in 0..9 {
        let height = [2, 3, 4, 4, 3, 4, 2, 1, 1][i as usize];
        world.fill(wreck + IVec3::new(i, 0, 0), wreck + IVec3::new(i, height - 1, 0), Material::HullAlloy);
        world.fill(wreck + IVec3::new(i, 0, 5), wreck + IVec3::new(i, (height - 1).max(1) - 1, 5), Material::HullAlloy);
    }
    world.fill(wreck + IVec3::new(0, 0, 1), wreck + IVec3::new(0, 2, 4), Material::HullAlloy);
    world.fill(wreck + IVec3::new(1, -1, 1), wreck + IVec3::new(7, -1, 4), Material::HullAlloy);

    // Pins are numbered in the order they were added (the scripted tour picks them by number), so the
    // tutorial's go last.
    pins.push(rail_start_pin);
    pins.push(rail_marker_pin);

    // Station consoles.
    let stations = [station0, station1, station2, station3];
    for console in stations {
        world.set(console, Material::Steel);
    }

    let sites = Sites {
        spawn: Vec3::new(30.5, ground, 40.5),
        rail_start,
        tower_release,
        tower_plate,
        mast_release,
        mast_plate,
        canyon_launch,
        ridge_launch,
        stations,
        pins,
        wreck: wreck.as_vec3() + Vec3::new(4.5, 0.0, 3.0),
        shipyard,
    };
    (world, sites)
}

/// Flat-topped island with a sandy shore, layered soil over rock, and a few trees and boulders
/// well away from the launch lines.
fn terrain(world: &mut VoxelWorld) {
    for z in 0..WORLD_SIZE.z {
        for x in 0..WORLD_SIZE.x {
            // Distance in blocks from the island's edge, negative out at sea.
            let inset = (x.min(WORLD_SIZE.x - 1 - x)).min(z.min(WORLD_SIZE.z - 1 - z)) - 6;
            let wobble = (hash01(IVec3::new(x / 3, 0, z / 3)) * 2.0) as i32;
            let top = (GROUND - (9 - inset - wobble).max(0)).clamp(3, GROUND);
            for y in 0..top {
                let depth = top - 1 - y;
                let material = if top < GROUND - 3 {
                    Material::Sand
                } else if depth == 0 {
                    if top == GROUND { Material::Grass } else { Material::Sand }
                } else if depth < 4 {
                    Material::Soil
                } else {
                    Material::Rock
                };
                world.set(IVec3::new(x, y, z), material);
            }
        }
    }
    let trees = [(14, 14), (22, 70), (48, 14), (100, 16), (130, 30), (150, 60), (140, 14), (12, 96), (24, 116), (120, 70), (158, 100), (96, 40)];
    for (i, (x, z)) in trees.into_iter().enumerate() {
        let trunk = 4 + (i as i32 % 3);
        world.fill(IVec3::new(x, GROUND, z), IVec3::new(x, GROUND + trunk - 1, z), Material::Timber);
        world.fill(IVec3::new(x - 2, GROUND + trunk, z - 2), IVec3::new(x + 2, GROUND + trunk + 1, z + 2), Material::Grass);
        world.fill(IVec3::new(x - 1, GROUND + trunk + 2, z - 1), IVec3::new(x + 1, GROUND + trunk + 2, z + 1), Material::Grass);
    }
    let boulders = [(44, 70), (110, 24), (136, 84), (60, 20), (20, 84)];
    for (x, z) in boulders {
        world.fill(IVec3::new(x, GROUND, z), IVec3::new(x + 2, GROUND + 1, z + 1), Material::Rock);
        world.set(IVec3::new(x + 1, GROUND + 2, z), Material::Rock);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics_core::puzzle::{Instance, PuzzleKind};

    /// Surface heights of the block columns along a launch line, relative to the ground.
    fn columns(world: &VoxelWorld, site: &LaunchSite, count: usize) -> Vec<f64> {
        (0..count)
            .map(|i| {
                let p = site.origin + site.direction * (i as f32 + 0.5);
                (world.surface(p.x.floor() as i32, p.z.floor() as i32) - GROUND) as f64
            })
            .collect()
    }

    #[test]
    fn the_island_matches_what_the_simulator_assumes() {
        for seed in 0..200 {
            let layout = Ch1Layout::generate(seed);
            let (world, sites) = build(&layout);

            let canyon = Instance::new(PuzzleKind::Canyon, &layout).profile().unwrap();
            assert_eq!(columns(&world, &sites.canyon_launch, canyon.heights.len()), canyon.heights, "seed {seed}: canyon");
            assert_eq!(sites.canyon_launch.origin.y, (GROUND + layout.ledge_h as i32) as f32);

            let ridge = Instance::new(PuzzleKind::Ridge, &layout).profile().unwrap();
            assert_eq!(columns(&world, &sites.ridge_launch, ridge.heights.len()), ridge.heights, "seed {seed}: ridge");

            // The drops are clear all the way down and exactly as tall as the layout says.
            for (release, plate, height) in [
                (sites.tower_release, sites.tower_plate, layout.tower_h),
                (sites.mast_release, sites.mast_plate, layout.mast_h),
            ] {
                assert_eq!(release.y - plate.y, height as f32);
                let hit = world.raycast(release + Vec3::Y * 0.5, Vec3::NEG_Y, 80.0).unwrap();
                assert!((hit.point.y - plate.y).abs() < 1e-4, "seed {seed}: something is in the way of the drop");
                assert_eq!(world.get(hit.cell), Material::Marker);
            }
        }
    }

    #[test]
    fn targets_are_marked_and_stations_are_reachable() {
        for seed in 0..200 {
            let layout = Ch1Layout::generate(seed);
            let (world, sites) = build(&layout);
            let ground = GROUND as f32;
            let below = |p: Vec3| world.get((p - Vec3::Y * 0.5).floor().as_ivec3());

            let platform = sites.canyon_launch.at(layout.platform_centre, 0.0, ground);
            let pad = sites.ridge_launch.at(layout.pad_centre, 0.0, ground);
            for offset in [-layout.platform_half + 0.1, 0.0, layout.platform_half - 0.1] {
                assert_eq!(below(platform + Vec3::X * offset as f32), Material::Marker, "seed {seed}");
            }
            for offset in [-layout.pad_half + 0.1, 0.0, layout.pad_half - 0.1] {
                assert_eq!(below(pad + Vec3::X * offset as f32), Material::Marker, "seed {seed}");
            }
            assert_ne!(below(platform + Vec3::X * (layout.platform_half as f32 + 0.1)), Material::Marker);

            // Each console stands on solid ground with open space beside it.
            for console in sites.stations {
                assert!(world.is_solid(console - IVec3::Y), "seed {seed}: console at {console} floats");
                assert!(!world.is_solid(console + IVec3::Y));
                assert!(!world.is_solid(console - IVec3::X), "seed {seed}: console at {console} is walled in");
            }
            assert!(world.is_solid(sites.spawn.floor().as_ivec3() - IVec3::Y));
            assert!(!world.is_solid(sites.spawn.floor().as_ivec3()));
            assert!(sites.shipyard.x + 12.0 < WORLD_SIZE.x as f32 - 8.0, "seed {seed}: shipyard runs off the island");
        }
    }

    #[test]
    fn the_supply_rail_is_clear_and_measures_true() {
        for seed in 0..200 {
            let layout = Ch1Layout::generate(seed);
            let (world, sites) = build(&layout);
            let pin = |label: &str| sites.pins.iter().find(|p| p.label == label).unwrap().position;
            let (start, marker) = (pin("Rail cart: starting point"), pin("Rail marker: centre"));

            // The ruler's horizontal reading between the two pins is the distance the puzzle judges.
            assert_eq!((marker - start).with_y(0.0).length(), layout.rail_target as f32, "seed {seed}");
            assert_eq!(world.get((marker - Vec3::Y * 0.5).floor().as_ivec3()), Material::Marker, "seed {seed}");
            let cart_stop = sites.rail_start + Vec3::X * layout.rail_target as f32;
            assert_eq!(world.get((cart_stop - Vec3::Y * 0.5).floor().as_ivec3()), Material::Marker, "seed {seed}");

            // Nothing stands in the cart's way over the whole length of the rail.
            for x in 0..=Ch1Layout::RAIL_LENGTH {
                let p = sites.rail_start + Vec3::X * x as f32;
                assert_eq!(world.surface(p.x.floor() as i32, p.z.floor() as i32), GROUND, "seed {seed}: rail blocked at {x} m");
                assert!(!world.box_collides(p + Vec3::new(-0.45, 0.01, -0.45), p + Vec3::new(0.45, 0.9, 0.45)), "seed {seed}: cart hits something at {x} m");
            }
        }
    }
}
