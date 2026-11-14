//! The island, generated from the same seeded layout the puzzles read.
//!
//! Every distance a puzzle depends on is built here in whole blocks from [`Ch1Layout`], so the number
//! the player measures and the number the simulator uses are the same. Around the puzzles the land is
//! free to be a landscape: a ragged coast of beaches and cliffs, rolling turf, a mountain range to the
//! north-east, groves of parasol trees. The puzzle sites themselves are kept exactly flat (the
//! "zones" below), and the ground the tests' side-view cameras look across is kept low.

use crate::state::PUZZLES;
use crate::voxel::{VoxelWorld, fbm, hash01, smoothstep};
use bevy::prelude::*;
use physics_core::materials::Material;
use physics_core::puzzle::Ch1Layout;

/// Where the layout's own coordinates start in the world. The margin around them leaves room for the
/// coast to find its own shape, and for beaches wide enough to slope gently down to the water.
pub const LAYOUT_ORIGIN: IVec3 = IVec3::new(OX, 0, OZ);
const OX: i32 = 40;
const OZ: i32 = 34;
pub const WORLD_SIZE: IVec3 = IVec3::new(232, 72, 196);
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
    /// Feet of the survey stakes that flag the corners of each target.
    pub stakes: Vec<Vec3>,
    /// Feet of the lamp posts at the camps and the shipyard.
    pub lanterns: Vec<Vec3>,
    /// Corners of the puzzle sites, kept as clean turf: no tufts in the way of a measurement.
    pub sites: Vec<[IVec2; 2]>,
}

/// Block the rail cart starts on, and the row the supply rail runs along (eastward).
const RAIL_X: i32 = OX + 28;
const RAIL_ROW: i32 = OZ + 47;
const TOWER: IVec3 = IVec3::new(OX + 36, GROUND, OZ + 30);
const MAST: IVec3 = IVec3::new(OX + 28, GROUND, OZ + 22);
/// Near (west) rim of the canyon, where the ledge launcher releases.
const CANYON_X: i32 = OX + 70;
const CANYON_ROW: i32 = OZ + 56;
const CANYON_Z: (i32, i32) = (OZ + 28, OZ + 84);
/// Release point of the tilting launcher.
const RIDGE_LAUNCH_X: i32 = OX + 40;
const RIDGE_ROW: i32 = OZ + 100;
/// West edge of the outcrop's flat top; the ramp climbs to it.
const LEDGE_X: i32 = CANYON_X - 14;

/// An axis-aligned rectangle of columns, both corners inclusive.
#[derive(Clone, Copy, Debug)]
struct Rect {
    min: IVec2,
    max: IVec2,
}

impl Rect {
    const fn new(x0: i32, z0: i32, x1: i32, z1: i32) -> Self {
        Rect { min: IVec2::new(x0, z0), max: IVec2::new(x1, z1) }
    }

    /// A rectangle given in the layout's own coordinates.
    const fn local(x0: i32, z0: i32, x1: i32, z1: i32) -> Self {
        Rect::new(OX + x0, OZ + z0, OX + x1, OZ + z1)
    }

    /// Distance in blocks from a column to the rectangle (0 inside).
    fn distance(&self, x: i32, z: i32) -> f32 {
        let dx = (self.min.x - x).max(x - self.max.x).max(0);
        let dz = (self.min.y - z).max(z - self.max.y).max(0);
        ((dx * dx + dz * dz) as f32).sqrt()
    }
}

/// What the landscape has to leave alone.
struct Plan {
    /// Puzzle sites: kept at exactly [`GROUND`], with land around them.
    zones: Vec<Rect>,
    /// Ground the tests' side-view cameras look across: kept low, no tall plants.
    corridors: Vec<Rect>,
    /// Where the rail test's camera looks from: nothing taller than a crystal.
    rail_view: Rect,
}

impl Plan {
    fn new(layout: &Ch1Layout) -> Self {
        let far_rim = CANYON_X + layout.canyon_width as i32;
        let ridge_x = RIDGE_LAUNCH_X + layout.ridge_x as i32;
        let yard_x = RIDGE_LAUNCH_X + layout.pad_centre.floor() as i32 + 7;
        let zones = vec![
            // Landing site: wreck, spawn, drop tower, survey mast.
            Rect::local(10, 16, 46, 50),
            // Supply rail.
            Rect::local(22, 40, 58, 52),
            // Ramp foot, outcrop and the launch line out past the canyon.
            Rect::local(34, 50, 142, 62),
            // The canyon, rim to rim.
            Rect::new(CANYON_X - 6, CANYON_Z.0 - 4, far_rim + 4, CANYON_Z.1 + 4),
            // Tilting launcher, ridge, pad and the line beyond.
            Rect::new(OX + 32, RIDGE_ROW - 6, OX + 140, RIDGE_ROW + 6),
            Rect::new(ridge_x - 4, RIDGE_ROW - 12, ridge_x + 5, RIDGE_ROW + 12),
            // Shipyard.
            Rect::new(yard_x - 3, RIDGE_ROW - 10, yard_x + 23, RIDGE_ROW + 10),
        ];
        let corridors = vec![
            Rect::local(18, 48, 62, 100),
            Rect::local(20, 23, 46, 60),
            Rect::local(60, 59, 148, 140),
            Rect::new(OX + 30, RIDGE_ROW + 3, OX + 142, WORLD_SIZE.z),
        ];
        Plan { zones, corridors, rail_view: Rect::local(18, 48, 62, 100) }
    }

    fn zone_distance(&self, x: i32, z: i32) -> f32 {
        self.zones.iter().map(|r| r.distance(x, z)).fold(f32::INFINITY, f32::min)
    }

    /// 0 on a puzzle site, rising to 1 a dozen blocks away from every site.
    fn flatness(&self, x: i32, z: i32) -> f32 {
        smoothstep(0.0, 12.0, self.zone_distance(x, z))
    }

    /// 1 where a test camera looks across, falling to 0 well away from it.
    fn lowland(&self, x: i32, z: i32) -> f32 {
        let d = self.corridors.iter().map(|r| r.distance(x, z)).fold(f32::INFINITY, f32::min);
        1.0 - smoothstep(0.0, 16.0, d)
    }

    /// Roughly how many blocks inland a column is; negative out at sea.
    fn inland(&self, x: i32, z: i32) -> f32 {
        let (xf, zf) = ((x - OX) as f32, (z - OZ) as f32);
        let ellipse = (((xf - 88.0) / 112.0).powi(2) + ((zf - 64.0) / 88.0).powi(2)).sqrt();
        let wobble = (fbm(xf / 30.0, zf / 30.0, 1, 3) - 0.5) * 0.6;
        let coast = (1.0 - ellipse - wobble) * 60.0;
        // Puzzle sites always have a beach's width of land around them, more in places, so the coast
        // never simply traces their outline. The sea always reaches the world's edge.
        let margin = BEACH + 2.0 + 12.0 * fbm(xf / 16.0, zf / 16.0, 7, 2);
        let edge = x.min(WORLD_SIZE.x - 1 - x).min(z).min(WORLD_SIZE.z - 1 - z) as f32;
        let fray = 6.0 * fbm(xf / 11.0, zf / 11.0, 6, 2);
        coast.max(margin - self.zone_distance(x, z)).min(edge - 3.0 - fray)
    }
}

pub fn build(layout: &Ch1Layout) -> (VoxelWorld, Sites) {
    let plan = Plan::new(layout);
    let mut world = VoxelWorld::new(WORLD_SIZE);
    terrain(&mut world, &plan);
    sea_stacks(&mut world, &plan);

    let ground = GROUND as f32;
    let mut pins = Vec::new();
    let mut stakes = Vec::new();
    let mut lanterns = Vec::new();

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
    for z in [RAIL_ROW as f32 - 1.7, RAIL_ROW as f32 + 2.7] {
        stakes.push(Vec3::new(marker_x as f32 + 0.5, ground, z));
    }

    // --- Puzzle 1: drop tower and survey mast -------------------------------------------------
    let gantry = |world: &mut VoxelWorld, base: IVec3, height: i32| -> (Vec3, Vec3) {
        // A steel column with an arm; the mass hangs under the end of the arm, two blocks out. It
        // stands on a poured footing that runs out under the landing plate, with three steel feet.
        world.fill(base + IVec3::new(-1, -1, -1), base + IVec3::new(3, -1, 1), Material::Concrete);
        for foot in [IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
            world.set(base + foot, Material::Steel);
        }
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
    outcrop(&mut world, layout.ledge_h as i32);
    canyon(&mut world, layout);
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
    stakes.extend(corners(platform_mid - 1, platform_mid + 1, CANYON_ROW - 1, CANYON_ROW + 1, ground));
    pins.push(Pin { position: canyon_launch.origin, label: "Ledge launcher: release point" });
    pins.push(Pin { position: canyon_launch.at(layout.platform_centre, 0.0, ground), label: "Platform: centre" });
    let station2 = IVec3::new(CANYON_X - 6, ledge_top, CANYON_ROW + 3);

    // --- Puzzle 3: tilting launcher, ridge, pad -----------------------------------------------
    let ridge_x = RIDGE_LAUNCH_X + layout.ridge_x as i32;
    ridge(&mut world, ridge_x, layout.ridge_h as i32);
    let pad_mid = RIDGE_LAUNCH_X + layout.pad_centre.floor() as i32;
    world.fill(
        IVec3::new(pad_mid - 2, GROUND - 1, RIDGE_ROW - 2),
        IVec3::new(pad_mid + 2, GROUND - 1, RIDGE_ROW + 2),
        Material::Marker,
    );
    stakes.extend(corners(pad_mid - 2, pad_mid + 2, RIDGE_ROW - 2, RIDGE_ROW + 2, ground));
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

    // --- Shipyard floor beyond the pad, lit at its corners -----------------------------------
    let yard_x = pad_mid + 7;
    world.fill(IVec3::new(yard_x, GROUND - 1, RIDGE_ROW - 7), IVec3::new(yard_x + 19, GROUND - 1, RIDGE_ROW + 7), Material::Concrete);
    for (x, z) in [(yard_x - 1, RIDGE_ROW - 8), (yard_x + 20, RIDGE_ROW - 8), (yard_x - 1, RIDGE_ROW + 8), (yard_x + 20, RIDGE_ROW + 8)] {
        lanterns.push(Vec3::new(x as f32 + 0.5, ground, z as f32 + 0.5));
    }
    let shipyard = Vec3::new(yard_x as f32 + 10.0, ground, RIDGE_ROW as f32 + 0.5);

    // --- The wreck: a broken hull section near the start, in a scorched patch -----------------
    let wreck = IVec3::new(OX + 16, GROUND, OZ + 40);
    for dz in -5..=10 {
        for dx in -6..=14 {
            let p = wreck + IVec3::new(dx, -1, dz);
            let from_centre = Vec2::new(dx as f32 - 4.0, dz as f32 - 2.5).length();
            if hash01(p) * 9.0 > from_centre - 2.0 && world.get(p) == Material::Grass {
                world.set(p, Material::Soil);
            }
        }
    }
    for i in 0..9 {
        let height = [2, 3, 4, 4, 3, 4, 2, 1, 1][i as usize];
        world.fill(wreck + IVec3::new(i, 0, 0), wreck + IVec3::new(i, height - 1, 0), Material::HullAlloy);
        world.fill(wreck + IVec3::new(i, 0, 5), wreck + IVec3::new(i, (height - 1).max(1) - 1, 5), Material::HullAlloy);
    }
    world.fill(wreck + IVec3::new(0, 0, 1), wreck + IVec3::new(0, 2, 4), Material::HullAlloy);
    world.fill(wreck + IVec3::new(1, -1, 1), wreck + IVec3::new(7, -1, 4), Material::HullAlloy);
    // Plating thrown clear in the crash.
    for offset in [IVec3::new(-3, 0, -2), IVec3::new(-4, 0, 7), IVec3::new(11, 0, -3), IVec3::new(2, 0, 8)] {
        world.set(wreck + offset, Material::HullAlloy);
    }

    // Pins are numbered in the order they were added (the scripted tour picks them by number), so the
    // tutorial's go last.
    pins.push(rail_start_pin);
    pins.push(rail_marker_pin);

    // Worn paths from the wreck past each station in turn, then on to the shipyard.
    let ramp_foot = LEDGE_X - layout.ledge_h as i32 - 2;
    path(&mut world, &[(OX + 24, OZ + 46), (OX + 30, OZ + 42), (OX + 34, OZ + 38), (station1.x, station1.z + 1)]);
    path(&mut world, &[(station1.x + 1, station1.z + 1), (OX + 42, OZ + 43), (OX + 46, OZ + 50), (ramp_foot, CANYON_ROW)]);
    path(&mut world, &[(ramp_foot, CANYON_ROW + 1), (ramp_foot - 2, OZ + 66), (OX + 42, OZ + 82), (OX + 38, OZ + 94), (station3.x + 1, station3.z - 1)]);
    path(&mut world, &[(station3.x + 2, RIDGE_ROW - 3), (ridge_x - 3, RIDGE_ROW - 14), (ridge_x + 6, RIDGE_ROW - 14), (pad_mid - 3, RIDGE_ROW - 4), (yard_x, RIDGE_ROW)]);

    // Station consoles, each in a small camp of salvage: a plated apron, a quartz lantern and crates.
    let stations = [station0, station1, station2, station3];
    let lamps = [IVec2::new(-2, -2), IVec2::new(2, 1), IVec2::new(-2, 2), IVec2::new(-2, 2)];
    let crates = [IVec2::new(2, -1), IVec2::new(-2, 1), IVec2::new(2, 1), IVec2::new(2, 2)];
    for (i, console) in stations.into_iter().enumerate() {
        camp(&mut world, console, crates[i]);
        lanterns.push(console.as_vec3() + Vec3::new(lamps[i].x as f32 + 0.5, 0.0, lamps[i].y as f32 + 0.5));
    }

    flora(&mut world, &plan);

    let sites = Sites {
        spawn: Vec3::new(OX as f32 + 30.5, ground, OZ as f32 + 40.5),
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
        stakes,
        lanterns,
        sites: plan.zones.iter().map(|r| [r.min, r.max]).collect(),
    };
    (world, sites)
}

/// Feet of four stakes just outside the corners of a rectangle of blocks.
fn corners(x0: i32, x1: i32, z0: i32, z1: i32, ground: f32) -> [Vec3; 4] {
    let (w, e) = (x0 as f32 - 0.3, x1 as f32 + 1.3);
    let (n, s) = (z0 as f32 - 0.3, z1 as f32 + 1.3);
    [Vec3::new(w, ground, n), Vec3::new(e, ground, n), Vec3::new(w, ground, s), Vec3::new(e, ground, s)]
}

/// Set a block only where there is air, so decoration never overwrites the puzzles.
fn put(world: &mut VoxelWorld, p: IVec3, material: Material) {
    if world.get(p) == Material::Air {
        world.set(p, material);
    }
}

/// Coast, beaches, cliffs, hills and the north-eastern range, in layers of turf, soil and rock.
/// Width of a beach, from the turf down to the water.
const BEACH: f32 = 24.0;

fn terrain(world: &mut VoxelWorld, plan: &Plan) {
    let (sx, sz) = (WORLD_SIZE.x, WORLD_SIZE.z);
    let mut heights = vec![0; (sx * sz) as usize];
    let mut beachy = vec![false; (sx * sz) as usize];
    for z in 0..sz {
        for x in 0..sx {
            let (xf, zf) = ((x - OX) as f32, (z - OZ) as f32);
            let flat = plan.flatness(x, z);
            let low = plan.lowland(x, z);
            let rolling = (fbm(xf / 26.0, zf / 26.0, 2, 3) - 0.5) * 7.0 * (1.0 - 0.55 * low);
            // The range: strongest to the north-east and along the east end.
            let north_east = ((xf - 95.0) / 50.0).clamp(0.0, 1.0) * ((45.0 - zf) / 35.0 + 0.4).clamp(0.0, 1.0);
            let east = ((xf - 145.0) / 20.0).clamp(0.0, 1.0);
            let west = ((22.0 - xf) / 14.0).clamp(0.0, 1.0) * 0.6;
            let massif = smoothstep(0.3, 0.8, fbm(xf / 42.0, zf / 42.0, 3, 3) * 0.55 + north_east.max(east).max(west) * 0.6);
            let crest = 1.0 - (fbm(xf / 18.0, zf / 18.0, 4, 3) * 2.0 - 1.0).abs();
            let mountain = massif * (8.0 + 18.0 * crest) * (1.0 - low);
            let land = GROUND as f32 + flat * (rolling + mountain);

            // Where the coast is cliff the land runs to the water's edge; elsewhere it slopes to a beach,
            // wide enough that it comes down in broad, low terraces rather than a staircase.
            let inland = plan.inland(x, z);
            let cliff = smoothstep(0.42, 0.62, fbm(xf / 40.0, zf / 40.0, 5, 2));
            let slope = BEACH - (BEACH - 1.5) * cliff;
            let top = if inland >= slope {
                land
            } else if inland > 0.0 {
                // A little noise, so the terraces' edges wander instead of running in parallel lines.
                let ripple = (fbm(xf / 7.0, zf / 7.0, 8, 2) - 0.5) * 2.8;
                (9.0 + (land - 9.0) * (inland / slope).powf(1.2) + ripple).clamp(8.6, land)
            } else {
                (8.0 + inland * 0.7).max(0.0)
            };
            let mut top = (top.round() as i32).clamp(1, WORLD_SIZE.y - 24);
            if plan.zone_distance(x, z) == 0.0 {
                top = GROUND;
            }
            let i = (z * sx + x) as usize;
            heights[i] = top;
            beachy[i] = top <= 10 || (inland < slope + 2.0 && cliff < 0.5 && top < GROUND + 2);
        }
    }
    let height = |x: i32, z: i32| heights[(z.clamp(0, sz - 1) * sx + x.clamp(0, sx - 1)) as usize];
    for z in 0..sz {
        for x in 0..sx {
            let top = height(x, z);
            let steep = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().map(|(dx, dz)| (top - height(x + dx, z + dz)).abs()).max().unwrap_or(0);
            let high = top > GROUND + 14 && hash01(IVec3::new(x, 1, z)) < 0.6;
            // Deep water has a floor of bare rock; the sea bed beyond the world's edge matches it.
            let surface = if top <= 4 {
                Material::Rock
            } else if beachy[(z * sx + x) as usize] {
                Material::Sand
            } else if steep >= 3 || high {
                Material::Rock
            } else {
                Material::Grass
            };
            for y in 0..top {
                let depth = top - 1 - y;
                let material = match (depth, surface) {
                    (0, _) => surface,
                    (1..=3, Material::Grass) => Material::Soil,
                    (1..=3, Material::Sand) => Material::Sand,
                    _ => Material::Rock,
                };
                world.set(IVec3::new(x, y, z), material);
            }
        }
    }
}

/// Rock stacks standing in the sea off the north, west and east coasts.
fn sea_stacks(world: &mut VoxelWorld, plan: &Plan) {
    const CELL: i32 = 14;
    for gz in (0..WORLD_SIZE.z).step_by(CELL as usize) {
        for gx in (0..WORLD_SIZE.x).step_by(CELL as usize) {
            let x = gx + (hash01(IVec3::new(gx, 201, gz)) * CELL as f32) as i32;
            let z = gz + (hash01(IVec3::new(gx, 202, gz)) * CELL as f32) as i32;
            let inland = plan.inland(x, z);
            // Not off the south coast: that is where the test cameras stand.
            let edge = x.min(WORLD_SIZE.x - 1 - x).min(z).min(WORLD_SIZE.z - 1 - z);
            if !(-13.0..-5.0).contains(&inland) || z - OZ > 90 || edge < 6 || hash01(IVec3::new(gx, 203, gz)) < 0.35 {
                continue;
            }
            let radius = 1.3 + 1.4 * hash01(IVec3::new(x, 204, z));
            let peak = GROUND - 6 + (hash01(IVec3::new(x, 205, z)) * 12.0) as i32;
            let r = radius.ceil() as i32 + 1;
            for dz in -r..=r {
                for dx in -r..=r {
                    let d = Vec2::new(dx as f32, dz as f32).length() + hash01(IVec3::new(x + dx, 206, z + dz)) * 0.8;
                    if d > radius {
                        continue;
                    }
                    let top = peak - (d * 1.5) as i32;
                    let column = IVec3::new(x + dx, 0, z + dz);
                    for y in 2..top {
                        world.set(column.with_y(y), Material::Rock);
                    }
                    world.set(column.with_y(top), if d < radius * 0.6 { Material::Grass } else { Material::Rock });
                }
            }
            if hash01(IVec3::new(x, 207, z)) < 0.5 {
                quartz(world, IVec3::new(x, peak + 1, z), hash01(IVec3::new(x, 208, z)));
            }
        }
    }
}

/// The outcrop that carries the ledge launcher: a steep-sided mesa with a gully ramp up its west side.
fn outcrop(world: &mut VoxelWorld, ledge_h: i32) {
    let (cx, cz) = ((CANYON_X - 6) as f32, CANYON_ROW as f32);
    // The supply rail runs just north of it, so the mesa is shallower on that side.
    for z in RAIL_ROW + 2..=OZ + 74 {
        for x in OX + 42..CANYON_X {
            let (xf, zf) = (x as f32, z as f32);
            let reach = if zf < cz { 8.5 } else { 15.0 };
            let ellipse = (((xf - cx) / 19.0).powi(2) + ((zf - cz) / reach).powi(2)).sqrt();
            let rim = ellipse + (fbm(xf / 7.0, zf / 7.0, 61, 2) - 0.5) * 0.35;
            let mut height = (((1.0 - rim) * 13.0 * 3.0).floor() as i32).clamp(0, ledge_h);
            // The flat top that holds the launcher and the station.
            if (LEDGE_X..CANYON_X).contains(&x) && (CANYON_ROW - 4..=CANYON_ROW + 6).contains(&z) {
                height = ledge_h;
            }
            // The ramp: one block up per block east, five wide, cut into the mesa's west side.
            let ramp = (CANYON_ROW - 2..=CANYON_ROW + 2).contains(&z) && x < LEDGE_X;
            if ramp {
                height = (ledge_h - (LEDGE_X - x)).max(0);
            }
            if height == 0 {
                continue;
            }
            let top = GROUND + height - 1;
            world.fill(IVec3::new(x, GROUND - 1, z), IVec3::new(x, top - 1, z), Material::Rock);
            let surface = if ramp {
                Material::Soil
            } else if height == ledge_h || hash01(IVec3::new(x, top, z)) < 0.3 {
                Material::Grass
            } else {
                Material::Rock
            };
            world.set(IVec3::new(x, top, z), surface);
        }
    }
}

/// The canyon: exact width on the launch line, its walls wandering outward away from it, a rounded
/// north end, a floor that climbs in steps at the south end so nobody is trapped, rubble and quartz.
fn canyon(world: &mut VoxelWorld, layout: &Ch1Layout) {
    let depth = layout.canyon_depth as i32;
    let far_rim = CANYON_X + layout.canyon_width as i32;
    for z in CANYON_Z.0..CANYON_Z.1 {
        let zf = z as f32;
        let off_line = (((z - CANYON_ROW).abs() - 6).max(0) as f32 / 6.0).min(1.0);
        let mut west = CANYON_X - (fbm(zf / 6.0, 0.5, 71, 2) * 4.0 * off_line).round() as i32;
        let mut east = far_rim - 1 + (fbm(zf / 6.0, 0.5, 73, 2) * 4.0 * off_line).round() as i32;
        // The north end closes in a curve.
        let pinch = ((CANYON_Z.0 + 5 - z).max(0) as f32).powi(2) / 3.0;
        west += pinch as i32;
        east -= pinch as i32;
        let climb = (z - (CANYON_Z.1 - depth)).max(0);
        let floor = GROUND - depth + climb;
        for x in west..=east {
            world.fill(IVec3::new(x, floor, z), IVec3::new(x, WORLD_SIZE.y - 1, z), Material::Air);
            world.set(IVec3::new(x, floor - 1, z), Material::Sand);
            // Away from the launch line: fallen rock, and quartz growing at the foot of the walls.
            // Kept off the west side, where anyone who walks off the rim lands and walks out.
            let away = (z - CANYON_ROW).abs() > 6 && climb == 0 && x > CANYON_X + 1;
            let h = hash01(IVec3::new(x, 301, z));
            if away && h < 0.06 {
                world.set(IVec3::new(x, floor, z), Material::Rock);
            } else if away && (x == west || x == east) && h > 0.9 {
                quartz(world, IVec3::new(x, floor, z), h);
            }
        }
    }
}

/// The ridge: a fin of rock. Where the beam crosses it, it is exactly one block thick and `height`
/// tall; further along it rises and falls, thickens behind, and sinks into the ground at both ends.
fn ridge(world: &mut VoxelWorld, x: i32, height: i32) {
    for dz in -11..=11 {
        let z = RIDGE_ROW + dz;
        let along = dz.abs();
        let crest = if along <= 2 {
            height
        } else {
            let t = (along - 2) as f32 / 10.0;
            let jag = (hash01(IVec3::new(x, 91, z)) * 3.0) as i32 - 1;
            ((height as f32 * (1.0 - t * t)).round() as i32 + jag).clamp(0, height + 2)
        };
        if crest > 0 {
            world.fill(IVec3::new(x, GROUND, z), IVec3::new(x, GROUND + crest - 1, z), Material::Rock);
        }
        if along >= 3 {
            for out in 1..=3 {
                let h = crest - 3 * out - (hash01(IVec3::new(x + out, 92, z)) * 2.0) as i32;
                if h > 0 {
                    world.fill(IVec3::new(x + out, GROUND, z), IVec3::new(x + out, GROUND + h - 1, z), Material::Rock);
                }
            }
        }
        if along >= 4 && crest > 4 {
            world.fill(IVec3::new(x - 1, GROUND, z), IVec3::new(x - 1, GROUND + crest - 5, z), Material::Rock);
        }
        if along >= 5 && hash01(IVec3::new(x, 93, z)) < 0.3 {
            quartz(world, IVec3::new(x + 1, GROUND, z), hash01(IVec3::new(x, 94, z)));
        }
    }
}

/// A trodden path through the turf along a line of columns.
fn path(world: &mut VoxelWorld, points: &[(i32, i32)]) {
    for pair in points.windows(2) {
        let (a, b) = (Vec2::new(pair[0].0 as f32, pair[0].1 as f32), Vec2::new(pair[1].0 as f32, pair[1].1 as f32));
        let steps = (b - a).length().ceil() as i32 * 2;
        let across = (b - a).normalize_or_zero().perp();
        for i in 0..=steps {
            let centre = a.lerp(b, i as f32 / steps.max(1) as f32);
            for side in -1..=1 {
                let c = (centre + across * side as f32).round().as_ivec2();
                if side != 0 && hash01(IVec3::new(c.x, 401, c.y)) < 0.35 {
                    continue;
                }
                let y = world.surface(c.x, c.y) - 1;
                let p = IVec3::new(c.x, y, c.y);
                if world.get(p) == Material::Grass {
                    world.set(p, Material::Soil);
                }
            }
        }
    }
}

/// A camp around a station console: hull plates laid on the turf and salvage crates.
fn camp(world: &mut VoxelWorld, console: IVec3, crate_at: IVec2) {
    for dz in -2_i32..=2 {
        for dx in -2_i32..=2 {
            let corner = dx.abs() == 2_i32 && dz.abs() == 2_i32;
            let p = console + IVec3::new(dx, -1, dz);
            if (!corner || hash01(p) < 0.5) && matches!(world.get(p), Material::Grass | Material::Soil) {
                world.set(p, Material::HullAlloy);
            }
        }
    }
    world.set(console - IVec3::Y, Material::HullAlloy);
    let crate_ = console + IVec3::new(crate_at.x, 0, crate_at.y);
    put(world, crate_, Material::Timber);
    put(world, crate_ + IVec3::new(crate_at.x.signum(), 0, 0), Material::HullAlloy);
}

/// Trees in groves, and in the open meadows shrubs, boulders and quartz.
fn flora(world: &mut VoxelWorld, plan: &Plan) {
    const CELL: i32 = 5;
    for gz in (0..WORLD_SIZE.z).step_by(CELL as usize) {
        for gx in (0..WORLD_SIZE.x).step_by(CELL as usize) {
            let x = gx + (hash01(IVec3::new(gx, 501, gz)) * CELL as f32) as i32;
            let z = gz + (hash01(IVec3::new(gx, 502, gz)) * CELL as f32) as i32;
            if x < 3 || z < 3 || x >= WORLD_SIZE.x - 3 || z >= WORLD_SIZE.z - 3 || plan.zone_distance(x, z) < 7.0 {
                continue;
            }
            let y = world.surface(x, z);
            if world.get(IVec3::new(x, y - 1, z)) != Material::Grass {
                continue;
            }
            // Level footing, and no path or structure close by.
            let mut level = true;
            for dz in -2..=2 {
                for dx in -2..=2 {
                    let s = world.surface(x + dx, z + dz);
                    let below = world.get(IVec3::new(x + dx, s - 1, z + dz));
                    level &= (s - y).abs() <= 1 && !matches!(below, Material::Soil | Material::HullAlloy | Material::Concrete | Material::Steel);
                }
            }
            if !level {
                continue;
            }
            let base = IVec3::new(x, y, z);
            let roll = hash01(IVec3::new(x, 503, z));
            let shape = hash01(IVec3::new(x, 504, z));
            let grove = fbm(x as f32 / 26.0, z as f32 / 26.0, 81, 2);
            if plan.rail_view.distance(x, z) < 6.0 {
                if roll < 0.12 {
                    quartz(world, base, 0.0);
                }
            } else if plan.lowland(x, z) > 0.25 {
                // Meadow: nothing taller than a few blocks.
                match roll {
                    r if r < 0.22 => shrub(world, base, shape),
                    r if r < 0.30 => boulder(world, base, shape),
                    r if r < 0.36 => quartz(world, base, shape),
                    _ => {}
                }
            } else if grove > 0.47 {
                match roll {
                    r if r < 0.55 => parasol(world, base, shape),
                    r if r < 0.72 => spire(world, base, shape),
                    r if r < 0.85 => shrub(world, base, shape),
                    _ => boulder(world, base, shape),
                }
            } else {
                match roll {
                    r if r < 0.12 => parasol(world, base, shape),
                    r if r < 0.22 => spire(world, base, shape),
                    r if r < 0.40 => shrub(world, base, shape),
                    r if r < 0.50 => boulder(world, base, shape),
                    r if r < 0.56 => quartz(world, base, shape),
                    _ => {}
                }
            }
        }
    }
}

/// A parasol tree: a thin trunk that kinks once, under a wide flat crown with a drooping rim.
fn parasol(world: &mut VoxelWorld, base: IVec3, shape: f32) {
    let height = 6 + (shape * 5.0) as i32;
    let lean = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z][(hash01(base + IVec3::Y * 7) * 4.0) as usize];
    let mut trunk = base;
    for i in 0..height {
        if i == height / 2 {
            trunk += lean;
        }
        put(world, trunk.with_y(base.y + i), Material::Timber);
    }
    let crown = trunk.with_y(base.y + height);
    let radius = 2.6 + 1.6 * hash01(base + IVec3::Y * 9);
    let r = radius.ceil() as i32;
    for dz in -r..=r {
        for dx in -r..=r {
            let d = Vec2::new(dx as f32, dz as f32).length();
            let p = crown + IVec3::new(dx, 0, dz);
            if d <= radius {
                put(world, p, Material::Foliage);
                if d > radius - 1.2 && hash01(p) < 0.35 {
                    put(world, p - IVec3::Y, Material::Foliage);
                }
            }
            if d <= radius - 1.6 {
                put(world, p + IVec3::Y, Material::Foliage);
            }
        }
    }
}

/// A spire: a tall fleshy stalk with collars of fronds and a quartz bud at the tip.
fn spire(world: &mut VoxelWorld, base: IVec3, shape: f32) {
    let height = 4 + (shape * 7.0) as i32;
    for i in 0..height {
        let p = base + IVec3::Y * i;
        put(world, p, Material::Foliage);
        if i > 1 && i % 3 == 0 {
            for side in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                put(world, p + side, Material::Foliage);
            }
        }
    }
    put(world, base + IVec3::Y * height, Material::Quartz);
}

/// A low cushion of fronds, now and then with a glowing bud.
fn shrub(world: &mut VoxelWorld, base: IVec3, shape: f32) {
    for dz in -1..=1 {
        for dx in -1..=1 {
            if dx * dz == 0 || shape > 0.5 {
                put(world, base + IVec3::new(dx, 0, dz), Material::Foliage);
            }
        }
    }
    put(world, base + IVec3::Y, Material::Foliage);
    if hash01(base + IVec3::Y * 3) < 0.3 {
        put(world, base + IVec3::Y * 2, Material::Quartz);
    }
}

/// A rounded boulder half sunk in the turf.
fn boulder(world: &mut VoxelWorld, base: IVec3, shape: f32) {
    let radius = 1.2 + shape * 1.3;
    let r = radius.ceil() as i32;
    for dy in 0..=r {
        for dz in -r..=r {
            for dx in -r..=r {
                if Vec3::new(dx as f32, dy as f32 * 1.4 + 0.5, dz as f32).length() <= radius {
                    put(world, base + IVec3::new(dx, dy, dz), Material::Rock);
                }
            }
        }
    }
}

/// A cluster of quartz crystals, one to four, up to three blocks tall.
fn quartz(world: &mut VoxelWorld, base: IVec3, shape: f32) {
    let count = 1 + (shape * 4.0) as usize;
    for (i, offset) in [IVec3::ZERO, IVec3::X, IVec3::NEG_Z, IVec3::new(-1, 0, 1)].into_iter().take(count).enumerate() {
        let p = base + offset;
        if !world.is_solid(p - IVec3::Y) {
            continue;
        }
        let height = 3 - i.min(2) as i32 - (hash01(p) < 0.5) as i32;
        for y in 0..height.max(1) {
            put(world, p + IVec3::Y * y, Material::Quartz);
        }
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
    fn the_island_is_ringed_by_sea_and_plants_keep_off_the_sites() {
        for seed in [0, 7, 42, 199] {
            let layout = Ch1Layout::generate(seed);
            let (world, sites) = build(&layout);
            for x in 0..WORLD_SIZE.x {
                for z in [0, WORLD_SIZE.z - 1] {
                    assert!((world.surface(x, z) as f32) < SEA_LEVEL, "seed {seed}: land at the world's edge ({x}, {z})");
                }
            }
            for z in 0..WORLD_SIZE.z {
                for x in [0, WORLD_SIZE.x - 1] {
                    assert!((world.surface(x, z) as f32) < SEA_LEVEL, "seed {seed}: land at the world's edge ({x}, {z})");
                }
            }
            for [min, max] in &sites.sites {
                for z in min.y..=max.y {
                    for x in min.x..=max.x {
                        for y in GROUND..WORLD_SIZE.y {
                            assert_ne!(world.get(IVec3::new(x, y, z)), Material::Foliage, "seed {seed}: a plant on a puzzle site at ({x}, {y}, {z})");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_ramp_up_the_outcrop_climbs_one_block_at_a_time() {
        for seed in 0..200 {
            let layout = Ch1Layout::generate(seed);
            let (world, sites) = build(&layout);
            let ledge = sites.canyon_launch.origin;
            let mut last = world.surface(ledge.x as i32 - 30, ledge.z as i32);
            assert_eq!(last, GROUND);
            for x in ledge.x as i32 - 29..ledge.x as i32 {
                let here = world.surface(x, ledge.z as i32);
                assert!((0..=1).contains(&(here - last)), "seed {seed}: a step of {} at x = {x}", here - last);
                last = here;
            }
            assert_eq!(last as f32, ledge.y);
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
