//! The island, generated from the same seeded layout the puzzles read.
//!
//! Every distance a puzzle depends on is built here in whole blocks from [`Ch1Layout`], so the number
//! the player measures and the number the simulator uses are the same. Around the puzzles the land is
//! free to be a landscape: a ragged coast of beaches and cliffs, rolling turf, hills to the west and
//! the east, groves of parasol trees. The puzzle sites themselves are kept exactly flat (the "zones"
//! below), and the ground the tests' side-view cameras look across is kept low.
//!
//! The chapter is one route, from the wreck on the south coast to the shipyard on the north shore,
//! which faces the strait and the main hull beyond it (see [`Route`]). It follows an ancient road of
//! pale stone with quartz down its middle (see [`Way`]), whose builders' machines are the stations:
//!
//! - one railway runs east from the wreck: the supply rail, the stone gate, then the haul track;
//! - the drop tower and the survey mast stand at the line's end, under the outcrop;
//! - a ramp climbs north onto the outcrop, whose ledge launcher fires north across the canyon;
//! - the beam lands beside the tilting launcher, which fires it north over the ridge onto the pad;
//! - past the pad, the shipyard floor and a slipway run down to the strait, through the north gate.

use crate::state::PUZZLES;
use crate::voxel::{VoxelWorld, fbm, hash01, smoothstep};
use bevy::prelude::*;
use physics_core::materials::Material;
use physics_core::puzzle::Ch1Layout;

pub const WORLD_SIZE: IVec3 = IVec3::new(232, 72, 232);
/// Height of the ground surface over most of the island.
pub const GROUND: i32 = 20;
pub const SEA_LEVEL: f32 = 8.6;

/// A named point the ruler snaps to.
#[derive(Clone, Debug)]
pub struct Pin {
    pub position: Vec3,
    pub label: &'static str,
}

impl Pin {
    /// Where a pin is put while what it marks is not in the world (no gate on the tower).
    pub const AWAY: Vec3 = Vec3::new(0.0, -1000.0, 0.0);

    /// Whether the pin marks something in the world now.
    pub fn present(&self) -> bool {
        self.position.y > -100.0
    }
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
    /// Underside centre of the rail cart where it stands now: at the start, or where the last test
    /// stopped it (see `sim`).
    pub cart: Vec3,
    /// Where the cart's cargo crate stands at the start, beside the rail.
    pub cargo_cell: IVec3,
    /// Corners (inclusive) of the ground beside the rail start where the cargo crate may be put down.
    pub cargo_yard: [IVec3; 2],
    /// The haul track: the point on the track (at ground level, on its centre line) where the haul
    /// cart's front coupler stands at the start, and the way the cart and the crawler run. Distances
    /// along the track are measured from here (see [`Sites::haul_at`]). It continues the supply rail,
    /// past the stone gate: once the tools are delivered, the same cart waits here.
    pub haul_start: Vec3,
    pub haul_direction: Vec3,
    /// Where the crawler's rear coupler starts, in metres along the track (its head start).
    pub crawler_start: f32,
    /// Underside of the test mass at rest, and where it lands.
    pub tower_release: Vec3,
    pub tower_plate: Vec3,
    /// Foot of the light gate's stand by the drop tower's station, where the gate rests.
    pub gate_stand: Vec3,
    /// Index in `pins` of the light gate, which moves with the gate (out of sight while none is
    /// clamped on the tower).
    pub gate_pin: usize,
    pub mast_release: Vec3,
    pub mast_plate: Vec3,
    /// The ledge launcher. Its origin rises with the crates stacked under it (see `carry`); the
    /// island is built with none.
    pub canyon_launch: LaunchSite,
    /// Lowest cell of the jack frame under the ledge launcher, at the rim; crates stack up from it.
    pub canyon_well: IVec3,
    /// Cells of the salvage pile on the ledge: where the movable crates lie at the start.
    pub crate_pile: Vec<IVec3>,
    /// Corners (inclusive) of the space where crates may be put down: the ledge west of the
    /// launcher's rail, up to three crates high. Off the launch lines, clear of the station.
    pub crate_yard: [IVec3; 2],
    /// Index in `pins` of the ledge launcher's release point, which moves with the launcher.
    pub canyon_release_pin: usize,
    pub ridge_launch: LaunchSite,
    /// Console block of each station, in puzzle order.
    pub stations: [IVec3; PUZZLES],
    pub pins: Vec<Pin>,
    pub wreck: Vec3,
    /// Centre of the shipyard floor, where the ship is assembled. The ship points north, down the
    /// slipway into the strait.
    pub shipyard: Vec3,
    /// The last dry ground from the shipyard towards the main hull: where the slipway meets the strait.
    pub shore: Vec3,
    /// Ground at the foot of the transfer crane, beyond the haul track: where the sight is
    /// set down once the crawler is caught.
    pub crane_foot: Vec3,
    /// Feet of the survey stakes that flag the corners of each target.
    pub stakes: Vec<Vec3>,
    /// Feet of the lamp posts at the camps and the shipyard.
    pub lanterns: Vec<Vec3>,
    /// Ground where a piece of scrap lies, thrown clear of the wreck or kept at a camp. The pieces
    /// are shown, not built: nothing stands on them or measures them.
    pub scrap: Vec<Vec3>,
    /// Corners of the puzzle sites, kept as clean turf: no tufts in the way of a measurement.
    pub sites: Vec<[IVec2; 2]>,
    /// The ancient road and what wakes along it.
    pub way: Way,
}

/// How many machines stand along the way: the five stations, then the north gate.
pub const MACHINES: usize = PUZZLES + 1;

/// The ancient road from the wreck to the north gate, and its machines. The island is built with all
/// of it awake; `waking` puts it to sleep and wakes it again as the puzzles pass.
#[derive(Clone, Debug)]
pub struct Way {
    /// Where each machine stands, where the light running up the road ends: the five consoles, then
    /// the north gate.
    pub machines: [Vec3; MACHINES],
    /// Each machine's own quartz, which lights when it wakes.
    pub quartz: [Vec<IVec3>; MACHINES],
    /// Run `i` of the inlay carries the light from machine `i` to machine `i + 1`.
    pub runs: [Inlay; MACHINES - 1],
    /// The ramp onto the outcrop as built: its blocks above the ground, and what each one is.
    pub ramp: Vec<(IVec3, Material)>,
    /// Foot of the stone gate's leaf, in the rail between its posts.
    pub stone_gate: Vec3,
    /// Foot of the seal across the road at the ridge's east end.
    pub ridge_seal: Vec3,
    /// Foot of the north gate, between its posts on the slipway, and its keystone's centre.
    pub north_gate: Vec3,
    pub keystone: Vec3,
}

/// One stretch of the way's quartz inlay, down the middle of the road: the line it follows (column
/// centres), and the blocks set into it, each with how far along the line it lies.
#[derive(Clone, Debug, Default)]
pub struct Inlay {
    pub line: Vec<Vec2>,
    pub cells: Vec<(IVec3, f32)>,
}

impl Inlay {
    pub fn length(&self) -> f32 {
        self.line.windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    /// The point `s` metres along the line, held at its ends.
    pub fn at(&self, s: f32) -> Vec2 {
        let mut left = s.max(0.0);
        for w in self.line.windows(2) {
            let d = w[0].distance(w[1]);
            if left <= d && d > 0.0 {
                return w[0].lerp(w[1], left / d);
            }
            left -= d;
        }
        self.line.last().copied().unwrap_or_default()
    }

    /// Carry on along another stretch, which starts where this one ends.
    fn then(mut self, more: Inlay) -> Inlay {
        let offset = self.length() + self.line.last().zip(more.line.first()).map_or(0.0, |(a, b)| a.distance(*b));
        self.cells.extend(more.cells.into_iter().map(|(c, s)| (c, s + offset)));
        self.line.extend(more.line);
        self
    }
}

impl Sites {
    /// A point on the haul track's centre line, `s` metres along it from the haul cart's start.
    pub fn haul_at(&self, s: f64) -> Vec3 {
        self.haul_start + self.haul_direction * s as f32
    }
}

/// Column of the tilting launcher's line, which runs north to the pad and the shipyard.
const RIDGE_X: i32 = 108;
/// Row of the pad's centre. Everything else is laid back south from it, by the seed's distances.
const PAD_Z: i32 = 60;
/// How far east of the tilting launcher's line the ledge launcher's line runs: far enough that the
/// ridge stays off the canyon shot's level run, near enough that the beam lands beside the launcher.
const CANYON_OFFSET: i32 = 9;
/// The canyon's length, across its launch line: it closes at the west end and climbs out at the east.
const CANYON_ENDS: (i32, i32) = (-28, 28);
/// Depth of the ledge's flat top, back from the rim.
const LEDGE_DEPTH: i32 = 14;
/// How far south of the rim the railway runs.
const LINE_BACK: i32 = 38;

/// A launch line's own grid of columns: `along` the way the load flies, `across` to its right, the
/// station's side. Column (0, 0) is the first one past the release point.
#[derive(Clone, Copy, Debug)]
struct Frame {
    origin: IVec2,
    along: IVec2,
    across: IVec2,
}

impl Frame {
    /// A line that fires north; its right is east.
    const fn north(origin: IVec2) -> Self {
        Frame { origin, along: IVec2::new(0, -1), across: IVec2::new(1, 0) }
    }

    fn col(&self, a: i32, c: i32) -> IVec2 {
        self.origin + self.along * a + self.across * c
    }

    fn cell(&self, a: i32, c: i32, y: i32) -> IVec3 {
        let p = self.col(a, c);
        IVec3::new(p.x, y, p.y)
    }

    /// The columns between two corners given in the frame.
    fn rect(&self, a0: i32, c0: i32, a1: i32, c1: i32) -> Rect {
        let (p, q) = (self.col(a0, c0), self.col(a1, c1));
        Rect { min: p.min(q), max: p.max(q) }
    }

    /// Fill the box of blocks between two corners given in the frame, heights inclusive.
    fn fill(&self, world: &mut VoxelWorld, (a0, c0, y0): (i32, i32, i32), (a1, c1, y1): (i32, i32, i32), material: Material) {
        let (p, q) = (self.cell(a0, c0, y0), self.cell(a1, c1, y1));
        world.fill(p.min(q), p.max(q), material);
    }

    /// The launcher whose release point is the face behind column (0, 0), at height `y`.
    fn launch(&self, y: f32) -> LaunchSite {
        let along = Vec3::new(self.along.x as f32, 0.0, self.along.y as f32);
        let centre = Vec3::new(self.origin.x as f32 + 0.5, y, self.origin.y as f32 + 0.5);
        LaunchSite { origin: centre - along * 0.5, direction: along }
    }
}

/// Where the chapter's route lies for a layout. It is anchored at the shipyard pad and laid back
/// south from it, so the shipyard always stands on the north shore and the seed's distances decide
/// how far south the canyon, the outcrop and the railway reach.
struct Route {
    /// Row of the railway, and the block the rail cart starts on (it runs east).
    line_z: i32,
    rail_x: i32,
    /// Block the haul cart's front coupler starts on, past the stone gate on the same row.
    haul_x: i32,
    tower: IVec3,
    mast: IVec3,
    /// The ledge launcher's line: column (0, 0) is the canyon's first column past the rim.
    canyon: Frame,
    /// The tilting launcher's line.
    ridge: Frame,
}

impl Route {
    fn new(layout: &Ch1Layout) -> Self {
        let ridge_first = PAD_Z + layout.pad_centre.floor() as i32;
        // The platform's centre is two blocks behind the tilting launcher's release point, where the
        // beam waits for it.
        let rim = ridge_first + 2 + layout.platform_centre.floor() as i32;
        let canyon_x = RIDGE_X + CANYON_OFFSET;
        // The line ends at a buffer three blocks short of the canyon's line, under the outcrop.
        let rail_x = canyon_x - 72;
        Route {
            line_z: rim + LINE_BACK,
            rail_x,
            haul_x: rail_x + Ch1Layout::RAIL_LENGTH as i32 + 3,
            tower: IVec3::new(canyon_x - 10, GROUND, rim + 30),
            mast: IVec3::new(canyon_x - 19, GROUND, rim + 30),
            canyon: Frame::north(IVec2::new(canyon_x, rim)),
            ridge: Frame::north(IVec2::new(RIDGE_X, ridge_first)),
        }
    }

    /// Row of the ridge's one-block crest on the tilting launcher's line.
    fn ridge_row(&self, layout: &Ch1Layout) -> i32 {
        self.ridge.col(layout.ridge_x as i32, 0).y
    }

    /// The first row of the shipyard floor past the pad, and its last, at the end of the tilting
    /// launcher's level run (which the simulator needs for a beam thrown too far); the slipway runs
    /// on from there down the shore to the water.
    fn shipyard_rows(&self) -> (i32, i32) {
        (PAD_Z - 6, PAD_Z - 42)
    }
}

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
    /// Where the railway's test cameras look from: nothing taller than a crystal.
    views: Vec<Rect>,
    /// Rows south of this are where the railway's cameras stand: no sea stacks off that coast.
    camera_coast: i32,
}

impl Plan {
    fn new(layout: &Ch1Layout, route: &Route) -> Self {
        let (canyon, ridge) = (&route.canyon, &route.ridge);
        let line = route.line_z;
        let rim = canyon.origin.y;
        let line_end = route.haul_x + Ch1Layout::HAUL_LENGTH as i32 + 4;
        let pad = layout.pad_centre.floor() as i32;
        let width = layout.canyon_width as i32;
        let zones = vec![
            // Landing site: wreck, spawn, the rail's start and its station.
            Rect::new(route.rail_x - 22, line - 14, route.rail_x + 4, line + 3),
            // The railway, from the wreck to the buffer past the crane.
            Rect::new(route.rail_x - 3, line - 6, line_end, line + 4),
            // Drop tower, survey mast and their station, between the line and the outcrop.
            Rect::new(route.mast.x - 5, route.tower.z - 4, route.tower.x + 8, line - 2),
            // Ramp foot, outcrop, and the ledge launcher's line out past the canyon to the end of its
            // level run.
            canyon.rect(-LEDGE_DEPTH - 18, -6, Ch1Layout::CANYON_RUN as i32 + 10, 6),
            // The canyon, rim to rim.
            canyon.rect(-4, CANYON_ENDS.0 - 4, width + 4, CANYON_ENDS.1 + 4),
            // The platform, the dolly rail, the tilting launcher and its station.
            Rect::new(RIDGE_X - 3, ridge.origin.y - 2, canyon.origin.x + 3, ridge.origin.y + 8),
            // The tilting launcher's line: ridge, pad, shipyard and on to the end of its level run.
            ridge.rect(-6, -6, pad + 43, 6),
            ridge.rect(layout.ridge_x as i32 - 4, -12, layout.ridge_x as i32 + 5, 8),
            // The shipyard floor, on the north shore.
            Rect::new(RIDGE_X - 11, route.shipyard_rows().1 - 3, RIDGE_X + 11, PAD_Z + 3),
        ];
        // Each side view looks across its line from one side: the railway's from the south, the
        // launchers' (which fire north) from the east.
        let corridors = vec![
            Rect::new(route.rail_x - 8, line, line_end + 4, WORLD_SIZE.z),
            Rect::new(route.mast.x - 6, rim + 20, route.tower.x + 10, line),
            canyon.rect(-6, 0, Ch1Layout::CANYON_RUN as i32 + 6, 90),
            ridge.rect(-6, 0, pad + 43, 90),
        ];
        let views = vec![Rect::new(route.rail_x - 6, line + 2, line_end + 4, WORLD_SIZE.z)];
        Plan { zones, corridors, views, camera_coast: line - 10 }
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
        let (xf, zf) = (x as f32, z as f32);
        let ellipse = (((xf - 116.0) / 110.0).powi(2) + ((zf - 116.0) / 108.0).powi(2)).sqrt();
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
    let route = Route::new(layout);
    let plan = Plan::new(layout, &route);
    let mut world = VoxelWorld::new(WORLD_SIZE);
    terrain(&mut world, &plan);
    sea_stacks(&mut world, &plan);

    let ground = GROUND as f32;
    let mut pins = Vec::new();
    let mut stakes = Vec::new();
    let mut lanterns = Vec::new();
    let (rail_x, row) = (route.rail_x, route.line_z);

    // --- The railway ------------------------------------------------------------------------
    // One steel line flush with the grass, from the wreck east to a buffer under the outcrop. The
    // stone gate splits it: west of it the supply rail (the tutorial), east of it the haul track,
    // where the same cart, once the tools are delivered, chases the crawler.
    let rail_len = Ch1Layout::RAIL_LENGTH as i32;
    let haul_len = Ch1Layout::HAUL_LENGTH as i32;
    let hx = route.haul_x;
    world.fill(IVec3::new(rail_x - 2, GROUND - 1, row), IVec3::new(hx + haul_len + 1, GROUND - 1, row), Material::Steel);

    // --- Puzzle 0 (tutorial): the supply rail ------------------------------------------------
    // A start mark and a marker across the rail; the stone gate past its far end stops the cart: two
    // posts either side of the rail under a lintel, quartz in their caps, and a leaf between them
    // (an entity, which sinks when the gate wakes). The survey pins sit on the near side of the rail,
    // where the cart does not hide them, so the ruler reads the run along the rail exactly.
    let gate_x = rail_x + rail_len + 1;
    let mut gate_quartz = Vec::new();
    for z in [row - 1, row + 1] {
        world.fill(IVec3::new(gate_x, GROUND, z), IVec3::new(gate_x, GROUND + 2, z), Material::Limestone);
        world.set(IVec3::new(gate_x, GROUND + 3, z), Material::Quartz);
        gate_quartz.push(IVec3::new(gate_x, GROUND + 3, z));
    }
    world.set(IVec3::new(gate_x, GROUND + 3, row), Material::Limestone);
    let stone_gate = Vec3::new(gate_x as f32 + 0.5, ground, row as f32 + 0.5);
    world.set(IVec3::new(rail_x, GROUND - 1, row - 1), Material::Concrete);
    let marker_x = rail_x + layout.rail_target as i32;
    world.fill(IVec3::new(marker_x, GROUND - 1, row - 1), IVec3::new(marker_x, GROUND - 1, row + 1), Material::Marker);
    let rail_start = Vec3::new(rail_x as f32 + 0.5, ground, row as f32 + 0.5);
    let beside = Vec3::new(0.0, 0.0, -1.0);
    let rail_start_pin = Pin { position: rail_start + beside, label: "Rail cart: starting point" };
    // The cargo crate stands on the station's side of the rail, two blocks short of the start mark so
    // it never hides the pin, with room around it to be put down again.
    let cargo_cell = IVec3::new(rail_x - 2, GROUND, row - 1);
    let cargo_yard = [IVec3::new(rail_x - 3, GROUND, row - 2), IVec3::new(rail_x - 1, GROUND, row - 1)];
    let rail_marker_pin = Pin { position: rail_start + beside + Vec3::X * layout.rail_target as f32, label: "Rail marker: centre" };
    let station0 = IVec3::new(rail_x - 1, GROUND, row - 3);
    for z in [row as f32 - 1.7, row as f32 + 2.7] {
        stakes.push(Vec3::new(marker_x as f32 + 0.5, ground, z));
    }

    // --- Puzzle 1: the haul track ---------------------------------------------------------------
    // The line past the stone gate, east to a buffer. The haul cart's front coupler and the crawler's
    // rear coupler start on concrete marks; the transfer marker crosses the track. The two timing
    // posts stand on the far (north) side from the test's camera, with the transfer crane's mast. The
    // survey pins sit on the near side of the track, where the cart and the crawler do not hide them.
    // Block whose centre is `s` metres along the track (the track runs towards +x).
    let haul_block = |s: u32| hx + s as i32;
    // The crawler stops with its rear coupler at the track's length; its body reaches a metre and a
    // half further, to the buffer.
    world.set(IVec3::new(hx + haul_len + 2, GROUND, row), Material::Steel);
    world.set(IVec3::new(hx, GROUND - 1, row + 1), Material::Concrete);
    world.set(IVec3::new(haul_block(layout.head_start), GROUND - 1, row + 1), Material::Concrete);
    let transfer_x = haul_block(layout.transfer_at);
    world.fill(IVec3::new(transfer_x, GROUND - 1, row - 1), IVec3::new(transfer_x, GROUND - 1, row + 1), Material::Marker);
    for z in [row as f32 - 1.7, row as f32 + 2.7] {
        stakes.push(Vec3::new(transfer_x as f32 + 0.5, ground, z));
    }
    // The transfer crane: a stone mast beyond the far side of the marker, its jib reaching to the
    // track's edge, quartz in its cap.
    world.fill(IVec3::new(transfer_x, GROUND, row - 2), IVec3::new(transfer_x, GROUND + 4, row - 2), Material::Limestone);
    world.set(IVec3::new(transfer_x, GROUND + 4, row - 1), Material::Limestone);
    let crane_quartz = vec![IVec3::new(transfer_x, GROUND + 5, row - 2), IVec3::new(transfer_x, GROUND + 2, row - 2)];
    for &q in &crane_quartz {
        world.set(q, Material::Quartz);
    }
    let crane_foot = Vec3::new(transfer_x as f32 + 0.5, ground, row as f32 - 2.5);
    let (post_a, post_b) = layout.posts();
    for post in [post_a, post_b] {
        world.fill(IVec3::new(haul_block(post), GROUND, row - 1), IVec3::new(haul_block(post), GROUND + 1, row - 1), Material::Steel);
        world.set(IVec3::new(haul_block(post), GROUND + 2, row - 1), Material::Marker);
    }
    let haul_start = Vec3::new(hx as f32 + 0.5, ground, row as f32 + 0.5);
    let haul_direction = Vec3::X;
    let along = |s: u32| haul_start + haul_direction * s as f32;
    let near = Vec3::Z;
    let haul_pins = [
        Pin { position: along(0) + near, label: "Haul cart: front coupler" },
        Pin { position: along(layout.head_start) + near, label: "Crawler: rear coupler" },
        Pin { position: along(layout.transfer_at) + near, label: "Transfer marker: centre" },
        // On top of the posts, where they can be seen and clicked.
        Pin { position: along(post_a) + Vec3::new(0.0, 3.0, -1.0), label: "Timing post A" },
        Pin { position: along(post_b) + Vec3::new(0.0, 3.0, -1.0), label: "Timing post B" },
    ];
    // Past the stone gate, on the station side of the rail.
    let station_haul = IVec3::new(hx - 2, GROUND, row - 3);

    // --- Puzzle 2: drop tower and survey mast, at the line's end -----------------------------
    let gantry = |world: &mut VoxelWorld, base: IVec3, height: i32, quartz: &mut Vec<IVec3>| -> (Vec3, Vec3) {
        // A stone column with an arm; the mass hangs under the end of the arm, two blocks out. It
        // stands on a dressed footing that runs out under the landing plate, with three stone feet,
        // and quartz bands the column every few blocks.
        world.fill(base + IVec3::new(-1, -1, -1), base + IVec3::new(3, -1, 1), Material::Limestone);
        for foot in [IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
            world.set(base + foot, Material::Limestone);
        }
        let top = base.y + height + 1;
        world.fill(base, IVec3::new(base.x, top, base.z), Material::Limestone);
        world.fill(IVec3::new(base.x, top, base.z), IVec3::new(base.x + 2, top, base.z), Material::Limestone);
        for y in (base.y + 2..top).step_by(3) {
            world.set(IVec3::new(base.x, y, base.z), Material::Quartz);
            quartz.push(IVec3::new(base.x, y, base.z));
        }
        let plate = IVec3::new(base.x + 2, base.y - 1, base.z);
        world.set(plate, Material::Marker);
        let foot = Vec3::new(plate.x as f32 + 0.5, base.y as f32, plate.z as f32 + 0.5);
        (foot + Vec3::Y * height as f32, foot)
    };
    let (tower, mast) = (route.tower, route.mast);
    let mut tower_quartz = Vec::new();
    let (tower_release, tower_plate) = gantry(&mut world, tower, layout.tower_h as i32, &mut tower_quartz);
    let (mast_release, mast_plate) = gantry(&mut world, mast, layout.mast_h as i32, &mut tower_quartz);
    pins.push(Pin { position: tower_release, label: "Drop tower: release point" });
    pins.push(Pin { position: tower_plate, label: "Drop tower: landing plate" });
    pins.push(Pin { position: mast_release, label: "Survey mast: release point" });
    pins.push(Pin { position: mast_plate, label: "Survey mast: landing plate" });
    let station1 = IVec3::new(tower.x + 1, GROUND, tower.z + 5);
    // The light gate rests on a stand on the station's apron until it is clamped on the tower.
    let gate_stand = Vec3::new(tower.x as f32 + 3.5, ground, tower.z as f32 + 3.5);

    // --- Puzzle 3: ledge, canyon, platform ----------------------------------------------------
    let canyon_frame = route.canyon;
    let ledge_top = GROUND + layout.ledge_h as i32;
    outcrop(&mut world, &canyon_frame, layout.ledge_h as i32);
    canyon(&mut world, &canyon_frame, layout);
    // Launcher rail along the launch line, ending at the rim.
    canyon_frame.fill(&mut world, (-4, 0, ledge_top - 1), (-1, 0, ledge_top - 1), Material::Steel);
    let canyon_launch = canyon_frame.launch(ledge_top as f32);
    // Landing platform: marker blocks set flush into the far plateau, as long along the launch line as
    // the layout says (one block) and three wide across it.
    let platform_mid = layout.platform_centre.floor() as i32;
    let reach = (layout.platform_half - 0.5).round() as i32;
    canyon_frame.fill(&mut world, (platform_mid - reach, -1, GROUND - 1), (platform_mid + reach, 1, GROUND - 1), Material::Marker);
    // Both rims light when the crate lands: quartz either side of the jack frame on the ledge, and
    // in front of and behind the platform on the far plateau (the dolly rail leaves it sideways).
    let mut canyon_quartz = Vec::new();
    for c in [-2, 2] {
        canyon_quartz.push(canyon_frame.cell(-1, c, ledge_top - 1));
    }
    for a in [platform_mid - reach - 1, platform_mid + reach + 1] {
        for c in -1..=1 {
            canyon_quartz.push(canyon_frame.cell(a, c, GROUND - 1));
        }
    }
    for &q in &canyon_quartz {
        world.set(q, Material::Quartz);
    }
    stakes.extend(corners(canyon_frame.rect(platform_mid - reach, -1, platform_mid + reach, 1), ground));
    let canyon_release_pin = pins.len();
    pins.push(Pin { position: canyon_launch.origin, label: "Ledge launcher: release point" });
    pins.push(Pin { position: canyon_launch.at(layout.platform_centre, 0.0, ground), label: "Platform: centre" });
    let station2 = canyon_frame.cell(-6, 3, ledge_top);
    // The jack frame stands on the last block of the launcher's rail, at the rim. The salvage pile
    // lies on the ledge west of the rail, two crates high: the crates themselves are put there when
    // the game starts (see `carry`), since they move.
    let canyon_well = canyon_frame.cell(-1, 0, ledge_top);
    let crate_pile: Vec<IVec3> =
        (0..Ch1Layout::MAX_STACK as i32).map(|i| canyon_frame.cell(-4 + (i / 2) % 2, -3 + i / 4, ledge_top + i % 2)).collect();
    let yard = canyon_frame.rect(-(LEDGE_DEPTH - 1), -4, -2, -1);
    let crate_yard = [IVec3::new(yard.min.x, ledge_top, yard.min.y), IVec3::new(yard.max.x, ledge_top + 2, yard.max.y)];

    // --- Puzzle 4: tilting launcher, ridge, pad -----------------------------------------------
    let ridge_frame = route.ridge;
    ridge(&mut world, &ridge_frame, layout.ridge_x as i32, layout.ridge_h as i32);
    let pad_mid = layout.pad_centre.floor() as i32;
    ridge_frame.fill(&mut world, (pad_mid - 2, -2, GROUND - 1), (pad_mid + 2, 2, GROUND - 1), Material::Marker);
    // The landing circle's rim of quartz, either side of the pad, and either side of the breech.
    let mut ridge_quartz: Vec<IVec3> = (pad_mid - 2..=pad_mid + 2).flat_map(|a| [ridge_frame.cell(a, -3, GROUND - 1), ridge_frame.cell(a, 3, GROUND - 1)]).collect();
    ridge_quartz.extend([ridge_frame.cell(-3, -1, GROUND - 1), ridge_frame.cell(-3, 1, GROUND - 1)]);
    for &q in &ridge_quartz {
        world.set(q, Material::Quartz);
    }
    stakes.extend(corners(ridge_frame.rect(pad_mid - 2, -2, pad_mid + 2, 2), ground));
    ridge_frame.fill(&mut world, (-3, 0, GROUND - 1), (-1, 0, GROUND - 1), Material::Steel);
    // The dolly rail: the beam, landed on the platform, rides it to the launcher's breech.
    let platform_cell = canyon_frame.cell(platform_mid, 0, GROUND - 1);
    let breech = ridge_frame.cell(-2, 0, GROUND - 1);
    debug_assert_eq!(platform_cell.z, breech.z, "the platform is level with the breech");
    world.fill(IVec3::new(breech.x + 1, GROUND - 1, breech.z), IVec3::new(platform_cell.x - 2, GROUND - 1, breech.z), Material::Steel);
    let ridge_launch = ridge_frame.launch(ground);
    pins.push(Pin { position: ridge_launch.origin, label: "Tilting launcher: release point" });
    pins.push(Pin {
        position: ridge_launch.at(layout.ridge_x as f64, layout.ridge_h as f64, ground),
        label: "Ridge: top, near edge",
    });
    pins.push(Pin { position: ridge_launch.at(layout.ridge_x as f64, 0.0, ground), label: "Ridge: foot, near side" });
    pins.push(Pin { position: ridge_launch.at(layout.pad_centre, 0.0, ground), label: "Shipyard pad: centre" });
    let station3 = ridge_frame.cell(-5, 4, GROUND);

    // --- Shipyard floor past the pad, lit at its corners, and the slipway to the strait --------
    let (yard_south, yard_north) = route.shipyard_rows();
    world.fill(IVec3::new(RIDGE_X - 8, GROUND - 1, yard_north), IVec3::new(RIDGE_X + 8, GROUND - 1, yard_south), Material::Concrete);
    for (x, z) in [(RIDGE_X - 10, yard_south + 1), (RIDGE_X + 10, yard_south + 1), (RIDGE_X - 10, yard_north - 1), (RIDGE_X + 10, yard_north - 1)] {
        lanterns.push(Vec3::new(x as f32 + 0.5, ground, z as f32 + 0.5));
    }
    // The ship goes together at the floor's north end, at the head of the slipway.
    let shipyard = Vec3::new(RIDGE_X as f32 + 0.5, ground, yard_north as f32 + 10.5);
    // Two slipway rails run from the floor's north edge down the shore into the water.
    for x in [RIDGE_X - 3, RIDGE_X + 3] {
        for z in (0..yard_north).rev() {
            let top = world.surface(x, z) - 1;
            if (top as f32) < SEA_LEVEL - 1.0 {
                break;
            }
            world.set(IVec3::new(x, top, z), Material::Steel);
        }
    }
    // The north gate: an arch over the slipway at the floor's north end, facing the strait, with a
    // quartz keystone. Its posts stand on whatever the shore is there and rise to one lintel.
    let gate_z = yard_north - 3;
    let lintel = GROUND + 7;
    for x in [RIDGE_X - 6, RIDGE_X + 6] {
        let foot = world.surface(x, gate_z);
        world.fill(IVec3::new(x, foot.min(GROUND) - 1, gate_z), IVec3::new(x, lintel - 1, gate_z), Material::Limestone);
    }
    world.fill(IVec3::new(RIDGE_X - 6, lintel, gate_z), IVec3::new(RIDGE_X + 6, lintel, gate_z), Material::Limestone);
    let keystone_cell = IVec3::new(RIDGE_X, lintel, gate_z);
    world.set(keystone_cell, Material::Quartz);
    let mut north_quartz = vec![keystone_cell];
    for x in [RIDGE_X - 6, RIDGE_X + 6] {
        for y in [GROUND + 2, GROUND + 5] {
            world.set(IVec3::new(x, y, gate_z), Material::Quartz);
            north_quartz.push(IVec3::new(x, y, gate_z));
        }
    }
    let north_gate = Vec3::new(RIDGE_X as f32 + 0.5, ground, gate_z as f32 + 0.5);
    let keystone = keystone_cell.as_vec3() + Vec3::splat(0.5);

    // --- The wreck: a broken hull section at the line's west end, in a scorched patch ---------
    let wreck = IVec3::new(rail_x - 14, GROUND, row - 8);
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
    // Scrap thrown clear in the crash.
    let mut scrap: Vec<Vec3> = [IVec3::new(-3, 0, -2), IVec3::new(-4, 0, 7), IVec3::new(11, 0, -3), IVec3::new(2, 0, 8)]
        .into_iter()
        .map(|offset| ground_at(&world, wreck + offset))
        .collect();

    // Pins are numbered in the order they were added (the scripted tour picks them by number), so the
    // tutorial's go last, and the light gate's after them.
    pins.push(rail_start_pin);
    pins.push(rail_marker_pin);
    let gate_pin = pins.len();
    pins.push(Pin { position: Pin::AWAY, label: "Light gate" });
    // The haul track's came last of all.
    pins.extend(haul_pins);

    // Station consoles, each in a small camp of salvage: a plated apron, a quartz lantern and crates.
    let stations = [station0, station_haul, station1, station2, station3];
    let lamps = [IVec2::new(-2, -2), IVec2::new(2, -2), IVec2::new(2, 1), IVec2::new(2, 2), IVec2::new(2, 2)];
    let crates = [IVec2::new(2, -1), IVec2::new(-2, -1), IVec2::new(-2, 1), IVec2::new(2, -1), IVec2::new(2, -1)];
    for (i, console) in stations.into_iter().enumerate() {
        camp(&mut world, console, crates[i]);
        // Beside the crate, a piece of scrap salvaged from the wreck.
        let beside = console + IVec3::new(crates[i].x + crates[i].x.signum(), 0, crates[i].y);
        scrap.push(ground_at(&world, beside));
        lanterns.push(console.as_vec3() + Vec3::new(lamps[i].x as f32 + 0.5, 0.0, lamps[i].y as f32 + 0.5));
    }

    // The road, laid after the camps so their plates lie over it: from the wreck beside the railway
    // to the stone gate and its end, past the drop tower, up the ramp onto the ledge; down again and
    // round the canyon's east end to the platform and the tilting launcher; round the ridge's east end,
    // across the shipyard floor and down the slipway to the north gate. Quartz runs down its middle,
    // one stretch from each machine to the next.
    let rim = canyon_frame.origin.y;
    let ramp_foot = canyon_frame.col(-LEDGE_DEPTH - layout.ledge_h as i32 - 2, 0);
    let ledge_end = canyon_frame.col(-6, 0);
    let canyon_east = canyon_frame.col(0, CANYON_ENDS.1 + 3).x;
    let far_side = rim - layout.canyon_width as i32 - 5;
    let ridge_row = route.ridge_row(layout);
    let launcher = ridge_frame.col(-3, 2);
    let prelude = road(&mut world, &[(wreck.x + 9, row - 3), (rail_x + 2, row - 2)]);
    let runs = [
        road(&mut world, &[(rail_x + 2, row - 2), (hx - 1, row - 2)]),
        road(&mut world, &[(hx - 1, row - 2), (hx + haul_len - 6, row - 2), (station1.x, station1.z + 1)]),
        road(&mut world, &[(station1.x + 1, station1.z - 1), (ramp_foot.x - 1, tower.z - 3), (ramp_foot.x, ramp_foot.y), (ledge_end.x, ledge_end.y)]),
        road(&mut world, &[(ledge_end.x, ledge_end.y), (ramp_foot.x, ramp_foot.y)]).then(road(
            &mut world,
            &[(ramp_foot.x + 3, ramp_foot.y), (canyon_east, rim + 6), (canyon_east, far_side), (canyon_frame.origin.x + 3, far_side - 4), (canyon_frame.origin.x + 3, launcher.y), (launcher.x + 3, launcher.y)],
        )),
        road(&mut world, &[(launcher.x + 3, launcher.y - 2), (RIDGE_X + 10, ridge_row + 5), (RIDGE_X + 10, ridge_row - 6), (RIDGE_X + 4, PAD_Z + 4), (RIDGE_X + 3, yard_south), (RIDGE_X, yard_north), (RIDGE_X, gate_z + 1)]),
    ];
    let ridge_seal = Vec3::new(RIDGE_X as f32 + 10.5, ground, ridge_row as f32 + 0.5);
    // The ramp as built, above the ground: what sleeps until gravity is measured.
    let mut ramp = Vec::new();
    for a in (-LEDGE_DEPTH - layout.ledge_h as i32 - 1)..-LEDGE_DEPTH {
        for c in -2..=2 {
            let col = canyon_frame.col(a, c);
            for y in GROUND..world.surface(col.x, col.y) {
                ramp.push((IVec3::new(col.x, y, col.y), world.get(IVec3::new(col.x, y, col.y))));
            }
        }
    }
    let mut first_quartz = gate_quartz;
    first_quartz.extend(prelude.cells.iter().map(|&(c, _)| c));
    let way = Way {
        machines: [
            stations[0].as_vec3() + Vec3::new(0.5, 0.0, 0.5),
            stations[1].as_vec3() + Vec3::new(0.5, 0.0, 0.5),
            stations[2].as_vec3() + Vec3::new(0.5, 0.0, 0.5),
            stations[3].as_vec3() + Vec3::new(0.5, 0.0, 0.5),
            stations[4].as_vec3() + Vec3::new(0.5, 0.0, 0.5),
            north_gate,
        ],
        quartz: [first_quartz, crane_quartz, tower_quartz, canyon_quartz, ridge_quartz, north_quartz],
        runs,
        ramp,
        stone_gate,
        ridge_seal,
        north_gate,
        keystone,
    };

    flora(&mut world, &plan);
    let shore = shore(&world, shipyard, crate::intro::Mainland::wreck());

    let sites = Sites {
        spawn: Vec3::new(rail_x as f32 + 2.5, ground, row as f32 - 6.5),
        rail_start,
        cart: rail_start,
        cargo_cell,
        cargo_yard,
        haul_start,
        haul_direction,
        crawler_start: layout.head_start as f32,
        tower_release,
        tower_plate,
        gate_stand,
        gate_pin,
        mast_release,
        mast_plate,
        canyon_launch,
        canyon_well,
        crate_pile,
        crate_yard,
        canyon_release_pin,
        ridge_launch,
        stations,
        pins,
        wreck: wreck.as_vec3() + Vec3::new(4.5, 0.0, 3.0),
        shipyard,
        shore,
        crane_foot,
        stakes,
        lanterns,
        scrap,
        sites: plan.zones.iter().map(|r| [r.min, r.max]).collect(),
        way,
    };
    (world, sites)
}

/// Walking from `from` towards `towards`, the last column of dry land before the sea, on its surface.
fn shore(world: &VoxelWorld, from: Vec3, towards: Vec3) -> Vec3 {
    let step = (towards - from).with_y(0.0).normalize();
    let mut last = from;
    for i in 0..400 {
        let p = from + step * i as f32;
        let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
        if x < 0 || z < 0 || x >= WORLD_SIZE.x || z >= WORLD_SIZE.z {
            break;
        }
        let top = world.surface(x, z) as f32;
        if top <= SEA_LEVEL {
            break;
        }
        last = Vec3::new(x as f32 + 0.5, top, z as f32 + 0.5);
    }
    last
}

/// Feet of four stakes just outside the corners of a rectangle of blocks.
fn corners(r: Rect, ground: f32) -> [Vec3; 4] {
    let (w, e) = (r.min.x as f32 - 0.3, r.max.x as f32 + 1.3);
    let (n, s) = (r.min.y as f32 - 0.3, r.max.y as f32 + 1.3);
    [Vec3::new(w, ground, n), Vec3::new(e, ground, n), Vec3::new(w, ground, s), Vec3::new(e, ground, s)]
}

/// Set a block only where there is air, so decoration never overwrites the puzzles.
fn put(world: &mut VoxelWorld, p: IVec3, material: Material) {
    if world.get(p) == Material::Air {
        world.set(p, material);
    }
}

/// Coast, beaches, cliffs, hills and the ranges to the west and the east, in layers of turf, soil
/// and rock.
/// Width of a beach, from the turf down to the water.
const BEACH: f32 = 24.0;

fn terrain(world: &mut VoxelWorld, plan: &Plan) {
    let (sx, sz) = (WORLD_SIZE.x, WORLD_SIZE.z);
    let mut heights = vec![0; (sx * sz) as usize];
    let mut beachy = vec![false; (sx * sz) as usize];
    for z in 0..sz {
        for x in 0..sx {
            let (xf, zf) = (x as f32, z as f32);
            let flat = plan.flatness(x, z);
            let low = plan.lowland(x, z);
            let rolling = (fbm(xf / 26.0, zf / 26.0, 2, 3) - 0.5) * 7.0 * (1.0 - 0.55 * low);
            // The ranges: one in the west, between the railway and the north shore (off the line of
            // sight from the shipyard to the main hull), and one along the east end.
            let west = ((92.0 - xf) / 40.0).clamp(0.0, 1.0) * ((zf - 50.0) / 30.0).clamp(0.0, 1.0) * ((170.0 - zf) / 30.0).clamp(0.0, 1.0);
            let east = ((xf - 190.0) / 20.0).clamp(0.0, 1.0);
            let massif = smoothstep(0.3, 0.8, fbm(xf / 42.0, zf / 42.0, 3, 3) * 0.55 + west.max(east) * 0.6);
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
            // Not off the south coast: that is where the railway's test cameras stand.
            let edge = x.min(WORLD_SIZE.x - 1 - x).min(z).min(WORLD_SIZE.z - 1 - z);
            if !(-13.0..-5.0).contains(&inland) || z > plan.camera_coast || edge < 6 || hash01(IVec3::new(gx, 203, gz)) < 0.35 {
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

/// The outcrop that carries the ledge launcher, on the canyon's near rim: a sheer-sided mesa with a
/// gully ramp of stone steps up the back of it, climbing towards the rim from the railway's end.
fn outcrop(world: &mut VoxelWorld, frame: &Frame, ledge_h: i32) {
    // The railway's end and the drop tower lie behind it, so the mesa is shallower on the west.
    for c in -7..=18 {
        for a in -28..0 {
            let (af, cf) = (a as f32, c as f32);
            let col = frame.col(a, c);
            let (xf, zf) = (col.x as f32, col.y as f32);
            let reach = if c < 0 { 8.5 } else { 11.0 };
            let ellipse = (((af + 6.0) / 19.0).powi(2) + (cf / reach).powi(2)).sqrt();
            let rim = ellipse + (fbm(xf / 7.0, zf / 7.0, 61, 2) - 0.5) * 0.35;
            // Sheer all round, at the ledge's full height: the ramp is the only way up, so while it
            // sleeps the ledge cannot be climbed (see `waking`). Boulders stand on the top near its
            // edge, never on the flat the launcher and the station use.
            let mut height = if rim < 1.0 { ledge_h + (rim > 0.82 && hash01(IVec3::new(col.x, 62, col.y)) < 0.25) as i32 } else { 0 };
            // The flat top that holds the launcher and the station.
            if (-LEDGE_DEPTH..0).contains(&a) && (-4..=6).contains(&c) {
                height = ledge_h;
            }
            // The ramp: one block up per block towards the rim, five wide, cut into the mesa's back.
            let ramp = (-2..=2).contains(&c) && a < -LEDGE_DEPTH;
            if ramp {
                height = (ledge_h - (-LEDGE_DEPTH - a)).max(0);
            }
            if height == 0 {
                continue;
            }
            let top = GROUND + height - 1;
            world.fill(IVec3::new(col.x, GROUND - 1, col.y), IVec3::new(col.x, top - 1, col.y), Material::Rock);
            let surface = if ramp {
                Material::Limestone
            } else if height == ledge_h || hash01(IVec3::new(col.x, top, col.y)) < 0.3 {
                Material::Grass
            } else {
                Material::Rock
            };
            world.set(IVec3::new(col.x, top, col.y), surface);
        }
    }
}

/// The canyon, across the ledge launcher's line: exact width on the line, its walls wandering
/// outward away from it, a rounded west end, a floor that climbs in steps at the east end so nobody
/// is trapped (and the way round it is a walk), rubble and quartz.
fn canyon(world: &mut VoxelWorld, frame: &Frame, layout: &Ch1Layout) {
    let depth = layout.canyon_depth as i32;
    let width = layout.canyon_width as i32;
    let (first, last) = CANYON_ENDS;
    for c in first..last {
        let cf = c as f32;
        let off_line = ((c.abs() - 6).max(0) as f32 / 6.0).min(1.0);
        let mut near = -(fbm(cf / 6.0, 0.5, 71, 2) * 4.0 * off_line).round() as i32;
        let mut far = width - 1 + (fbm(cf / 6.0, 0.5, 73, 2) * 4.0 * off_line).round() as i32;
        // The west end closes in a curve.
        let pinch = ((first + 5 - c).max(0) as f32).powi(2) / 3.0;
        near += pinch as i32;
        far -= pinch as i32;
        let climb = (c - (last - depth)).max(0);
        let floor = GROUND - depth + climb;
        for a in near..=far {
            let cell = frame.cell(a, c, floor);
            world.fill(cell, cell.with_y(WORLD_SIZE.y - 1), Material::Air);
            world.set(cell - IVec3::Y, Material::Sand);
            // Away from the launch line: fallen rock, and quartz growing at the foot of the walls.
            // Kept off the near side, where anyone who walks off the rim lands and walks out.
            let away = c.abs() > 6 && climb == 0 && a > 1;
            let h = hash01(cell.with_y(301));
            if away && h < 0.06 {
                world.set(cell, Material::Rock);
            } else if away && (a == near || a == far) && h > 0.9 {
                quartz(world, cell, h);
            }
        }
    }
}

/// The ridge, across the tilting launcher's line: a fin of rock. Where the beam crosses it, it is
/// exactly one block thick and `height` tall; further along it rises and falls, thickens behind, and
/// sinks into the ground at both ends. The east end, towards the ledge launcher's line, sinks quickly
/// so that line's level run stays clear.
fn ridge(world: &mut VoxelWorld, frame: &Frame, along: i32, height: i32) {
    for c in -11_i32..=6 {
        let side = c.abs();
        let span = if c > 0 { 5.0 } else { 10.0 };
        let column = frame.cell(along, c, GROUND);
        let crest = if side <= 2 {
            height
        } else {
            let t = (side - 2) as f32 / span;
            let jag = (hash01(column.with_y(91)) * 3.0) as i32 - 1;
            ((height as f32 * (1.0 - t * t)).round() as i32 + jag).clamp(0, height + 2)
        };
        if crest > 0 {
            frame.fill(world, (along, c, GROUND), (along, c, GROUND + crest - 1), Material::Rock);
        }
        if side >= 3 {
            for out in 1..=3 {
                let h = crest - 3 * out - (hash01(frame.cell(along + out, c, 92)) * 2.0) as i32;
                if h > 0 {
                    frame.fill(world, (along + out, c, GROUND), (along + out, c, GROUND + h - 1), Material::Rock);
                }
            }
        }
        if side >= 4 && crest > 4 {
            frame.fill(world, (along - 1, c, GROUND), (along - 1, c, GROUND + crest - 5), Material::Rock);
        }
        if side >= 5 && hash01(column.with_y(93)) < 0.3 {
            quartz(world, frame.cell(along + 1, c, GROUND), hash01(column.with_y(94)));
        }
    }
}

/// The ancient road along a line of columns: dressed stone three blocks wide, worn through to the
/// turf here and there at its edges, with quartz set down its middle. It lies flush in the ground,
/// over turf and the ramp's steps; the castaway's plates, the rails and the marks are left as they
/// are. Returns the stretch of inlay it laid, in order along the line.
fn road(world: &mut VoxelWorld, points: &[(i32, i32)]) -> Inlay {
    let line: Vec<Vec2> = points.iter().map(|&(x, z)| Vec2::new(x as f32 + 0.5, z as f32 + 0.5)).collect();
    let mut inlay = Inlay { line: line.clone(), cells: Vec::new() };
    let mut walked = 0.0;
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let steps = (b - a).length().ceil() as i32 * 2;
        let across = (b - a).normalize_or_zero().perp();
        for i in 0..=steps {
            let t = i as f32 / steps.max(1) as f32;
            let centre = a.lerp(b, t);
            for side in -1..=1 {
                let c = (centre + across * side as f32 - Vec2::splat(0.5)).round().as_ivec2();
                if side != 0 && hash01(IVec3::new(c.x, 401, c.y)) < 0.15 {
                    continue;
                }
                let p = IVec3::new(c.x, world.surface(c.x, c.y) - 1, c.y);
                let on = world.get(p);
                if side == 0 && matches!(on, Material::Grass | Material::Soil | Material::Limestone) {
                    world.set(p, Material::Quartz);
                    inlay.cells.push((p, walked + (b - a).length() * t));
                } else if matches!(on, Material::Grass | Material::Soil) {
                    world.set(p, Material::Limestone);
                }
            }
        }
        walked += (b - a).length();
    }
    inlay
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
}

/// The middle of the top of the ground in the column of `cell`.
fn ground_at(world: &VoxelWorld, cell: IVec3) -> Vec3 {
    Vec3::new(cell.x as f32 + 0.5, world.surface(cell.x, cell.z) as f32, cell.z as f32 + 0.5)
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
                    level &= (s - y).abs() <= 1
                        && !matches!(below, Material::Soil | Material::HullAlloy | Material::Concrete | Material::Steel | Material::Limestone | Material::Quartz);
                }
            }
            if !level {
                continue;
            }
            let base = IVec3::new(x, y, z);
            let roll = hash01(IVec3::new(x, 503, z));
            let shape = hash01(IVec3::new(x, 504, z));
            let grove = fbm(x as f32 / 26.0, z as f32 / 26.0, 81, 2);
            if plan.views.iter().any(|v| v.distance(x, z) < 6.0) {
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

            let (canyon, ridge) = (sites.canyon_launch, sites.ridge_launch);
            let platform = canyon.at(layout.platform_centre, 0.0, ground);
            let pad = ridge.at(layout.pad_centre, 0.0, ground);
            for offset in [-layout.platform_half + 0.1, 0.0, layout.platform_half - 0.1] {
                assert_eq!(below(platform + canyon.direction * offset as f32), Material::Marker, "seed {seed}");
            }
            for offset in [-layout.pad_half + 0.1, 0.0, layout.pad_half - 0.1] {
                assert_eq!(below(pad + ridge.direction * offset as f32), Material::Marker, "seed {seed}");
            }
            assert_ne!(below(platform + canyon.direction * (layout.platform_half as f32 + 0.1)), Material::Marker);

            // Each console stands on solid ground with open space beside it.
            for console in sites.stations {
                assert!(world.is_solid(console - IVec3::Y), "seed {seed}: console at {console} floats");
                assert!(!world.is_solid(console + IVec3::Y));
                assert!(!world.is_solid(console - IVec3::X), "seed {seed}: console at {console} is walled in");
            }
            assert!(world.is_solid(sites.spawn.floor().as_ivec3() - IVec3::Y));
            assert!(!world.is_solid(sites.spawn.floor().as_ivec3()));
        }
    }

    #[test]
    fn the_route_leads_from_the_wreck_to_the_shipyard_on_the_strait() {
        let hull = crate::intro::Mainland::wreck();
        for seed in 0..200 {
            let layout = Ch1Layout::generate(seed);
            let (world, sites) = build(&layout);
            let flat = |p: Vec3| p.xz();

            // Each station is nearer the shipyard than the one before it, and the wreck is the farthest
            // point of all: walking the chapter is walking towards the goal.
            let mut last = flat(sites.wreck).distance(flat(sites.shipyard));
            for (i, console) in sites.stations.iter().enumerate() {
                let here = console.as_vec3().xz().distance(flat(sites.shipyard));
                assert!(here < last, "seed {seed}: station {i} is no nearer the shipyard ({here} after {last})");
                last = here;
            }

            // The shipyard is the end of the route nearest the main hull, and the strait is just past it.
            assert!(flat(sites.shipyard).distance(flat(hull)) < flat(sites.wreck).distance(flat(hull)), "seed {seed}");
            for console in sites.stations {
                assert!(flat(sites.shipyard).distance(flat(hull)) < console.as_vec3().xz().distance(flat(hull)), "seed {seed}");
            }
            assert!(flat(sites.shore).distance(flat(sites.shipyard)) < 32.0, "seed {seed}: the shipyard is {} from the strait", flat(sites.shore).distance(flat(sites.shipyard)));
            assert!(sites.shore.z < sites.shipyard.z, "seed {seed}: the shore is not north of the shipyard");

            // The beam lands beside the tilting launcher, and the dolly rail between them is flush.
            let platform = sites.canyon_launch.at(layout.platform_centre, 0.0, GROUND as f32);
            let breech = sites.ridge_launch.origin - sites.ridge_launch.direction * 1.5;
            assert!((platform.z - breech.z).abs() < 0.01, "seed {seed}: the platform is not level with the breech");
            assert!(platform.distance(breech) < 10.0, "seed {seed}");
            for x in breech.x as i32 + 1..platform.x as i32 - 1 {
                let cell = IVec3::new(x, GROUND - 1, breech.z.floor() as i32);
                assert_eq!(world.get(cell), Material::Steel, "seed {seed}: the dolly rail is broken at {cell}");
                assert_eq!(world.surface(x, cell.z), GROUND, "seed {seed}");
            }
        }
    }

    #[test]
    fn one_railway_carries_the_cart_from_the_supply_rail_to_the_haul_track() {
        for seed in 0..200 {
            let layout = Ch1Layout::generate(seed);
            let (world, sites) = build(&layout);
            assert_eq!(sites.haul_start.z, sites.rail_start.z, "the haul track continues the rail");
            assert_eq!(sites.haul_direction, Vec3::X, "and runs the same way");
            // The rail is steel all the way from the start to the buffer.
            let row = sites.rail_start.z.floor() as i32;
            let end = sites.haul_at(Ch1Layout::HAUL_LENGTH as f64 + 1.0).x.floor() as i32;
            for x in sites.rail_start.x.floor() as i32..=end {
                // Steel, or a mark painted across it.
                let rail = world.get(IVec3::new(x, GROUND - 1, row));
                assert!(matches!(rail, Material::Steel | Material::Marker), "seed {seed}: the line is broken at x = {x} ({rail:?})");
            }
            // The stone gate stands past the furthest the tutorial can send the cart, and short of where
            // the haul cart waits: posts either side of the rail, the rail itself clear for the leaf.
            let gate = sites.rail_start.x.floor() as i32 + Ch1Layout::RAIL_LENGTH as i32 + 1;
            for z in [row - 1, row + 1] {
                assert!(world.is_solid(IVec3::new(gate, GROUND, z)), "seed {seed}: no stone gate post");
            }
            assert_eq!(sites.way.stone_gate.x.floor() as i32, gate, "seed {seed}");
            assert!(!world.is_solid(IVec3::new(gate, GROUND, row)), "seed {seed}: the leaf is an entity, not a block");
            assert!((gate as f32) < sites.haul_at(-0.9).x, "seed {seed}: the gate is in the haul cart's way");
        }
    }

    #[test]
    fn the_island_is_ringed_by_sea_and_plants_keep_off_the_sites() {
        // The extremes of how far the route reaches south, and a few in between.
        let reach = |seed: &u64| {
            let l = Ch1Layout::generate(*seed);
            (l.pad_centre + l.platform_centre) as i64
        };
        let seeds: Vec<u64> = (0..200).collect();
        let longest = *seeds.iter().max_by_key(|s| reach(s)).unwrap();
        let shortest = *seeds.iter().min_by_key(|s| reach(s)).unwrap();
        for seed in [0, 7, 42, 199, longest, shortest] {
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
    fn the_crates_have_room_on_the_ledge() {
        for seed in 0..200 {
            let layout = Ch1Layout::generate(seed);
            let (world, sites) = build(&layout);
            let ledge_top = GROUND + layout.ledge_h as i32;
            let out = sites.canyon_launch.direction.as_ivec3();
            // The jack frame's column is clear all the way up, and stands on the ledge at the rim.
            assert!(world.is_solid(sites.canyon_well - IVec3::Y), "seed {seed}");
            assert!(!world.is_solid(sites.canyon_well + out - IVec3::Y), "seed {seed}: the rim must be the next block out");
            for k in 0..=Ch1Layout::MAX_STACK as i32 + 2 {
                assert!(!world.is_solid(sites.canyon_well + IVec3::Y * k), "seed {seed}: the frame's column is blocked at {k}");
            }
            // The pile lies on the ledge's flat top, every cell free, inside the yard.
            assert_eq!(sites.crate_pile.len(), Ch1Layout::MAX_STACK as usize);
            let [low, high] = sites.crate_yard;
            for x in low.x..=high.x {
                for z in low.z..=high.z {
                    assert!(world.is_solid(IVec3::new(x, low.y - 1, z)), "seed {seed}: the yard has a hole at ({x}, {z})");
                    for y in low.y..=high.y + 2 {
                        assert!(!world.is_solid(IVec3::new(x, y, z)), "seed {seed}: the yard is blocked at ({x}, {y}, {z})");
                    }
                }
            }
            for cell in &sites.crate_pile {
                assert!(cell.cmpge(low).all() && cell.cmple(high).all(), "seed {seed}: {cell} is outside the yard");
                let bottom = IVec3::new(cell.x, ledge_top, cell.z);
                assert!(world.is_solid(bottom - IVec3::Y), "seed {seed}: the pile at {cell} is not on the ledge");
            }
        }
    }

    #[test]
    fn the_ramp_up_the_outcrop_climbs_one_block_at_a_time() {
        for seed in 0..200 {
            let layout = Ch1Layout::generate(seed);
            let (world, sites) = build(&layout);
            let ledge = sites.canyon_launch.origin;
            let back = -sites.canyon_launch.direction;
            // Columns walking up the ramp from 30 blocks back to the rim.
            let column = |k: i32| {
                let p = ledge + back * (k as f32 - 0.5);
                world.surface(p.x.floor() as i32, p.z.floor() as i32)
            };
            let mut last = column(30);
            assert_eq!(last, GROUND, "seed {seed}");
            for k in (1..30).rev() {
                let here = column(k);
                assert!((0..=1).contains(&(here - last)), "seed {seed}: a step of {} {k} blocks back", here - last);
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

            // The cargo crate's cell is free and on the ground, in its yard, off the rail and clear of
            // the pins; every cell of the yard can take it.
            let [low, high] = sites.cargo_yard;
            let cell = sites.cargo_cell;
            assert!(cell.cmpge(low).all() && cell.cmple(high).all(), "seed {seed}: the cargo crate starts outside its yard");
            assert_eq!(cell.y, GROUND);
            for x in low.x..=high.x {
                for z in low.z..=high.z {
                    let c = IVec3::new(x, low.y, z);
                    assert!(world.is_solid(c - IVec3::Y), "seed {seed}: nothing under the cargo yard at {c}");
                    assert!(!world.is_solid(c) && !world.is_solid(c + IVec3::Y), "seed {seed}: the cargo yard is blocked at {c}");
                    assert_ne!(z, sites.rail_start.z.floor() as i32, "the yard is off the rail");
                    for p in [start, marker] {
                        assert_ne!(p.floor().as_ivec3().with_y(c.y), c, "seed {seed}: the yard covers a pin");
                    }
                }
            }
            assert_eq!(sites.cart, sites.rail_start, "the island is built with the cart at its start");
        }
    }

    #[test]
    fn the_haul_track_is_clear_and_measures_true() {
        for seed in 0..200 {
            let layout = Ch1Layout::generate(seed);
            let (world, sites) = build(&layout);
            let pin = |label: &str| sites.pins.iter().find(|p| p.label == label).unwrap().position;
            let across = |a: Vec3, b: Vec3| (b - a).with_y(0.0).dot(sites.haul_direction);
            let start = pin("Haul cart: front coupler");
            // The ruler's readings between the pins are the distances the puzzle judges.
            assert_eq!(across(start, pin("Crawler: rear coupler")), layout.head_start as f32, "seed {seed}");
            assert_eq!(across(start, pin("Transfer marker: centre")), layout.transfer_at as f32, "seed {seed}");
            assert_eq!((pin("Timing post B") - pin("Timing post A")).with_y(0.0).length(), layout.post_gap as f32, "seed {seed}");
            assert_eq!(across(start, pin("Timing post A")), layout.posts().0 as f32, "seed {seed}");
            let marker = sites.haul_at(layout.transfer_at as f64);
            assert_eq!(world.get((marker - Vec3::Y * 0.5).floor().as_ivec3()), Material::Marker, "seed {seed}");
            for s in [-0.4, 0.4] {
                let edge = sites.haul_at(layout.transfer_at as f64 + s);
                assert_eq!(world.get((edge - Vec3::Y * 0.5).floor().as_ivec3()), Material::Marker, "seed {seed}");
            }

            // Nothing stands in the way of the cart or the crawler over the whole track, up to the buffer.
            let end = Ch1Layout::HAUL_LENGTH as f64 + 1.4;
            for k in -2..=(end * 2.0) as i32 {
                let p = sites.haul_at(k as f64 / 2.0);
                assert_eq!(world.surface(p.x.floor() as i32, p.z.floor() as i32), GROUND, "seed {seed}: track blocked at {} m", k as f64 / 2.0);
                assert!(!world.box_collides(p + Vec3::new(-0.45, 0.01, -0.45), p + Vec3::new(0.45, 1.0, 0.45)), "seed {seed}: something on the track at {} m", k as f64 / 2.0);
            }
            let buffer = sites.haul_at(end + 0.6).floor().as_ivec3();
            assert!(world.is_solid(buffer), "seed {seed}: no buffer at the track's end");
        }
    }

    #[test]
    fn the_light_gate_has_a_stand_and_a_pin() {
        for seed in 0..200 {
            let layout = Ch1Layout::generate(seed);
            let (world, sites) = build(&layout);
            let stand = sites.gate_stand.floor().as_ivec3();
            assert!(world.is_solid(stand - IVec3::Y), "seed {seed}: the stand floats");
            assert!(!world.is_solid(stand) && !world.is_solid(stand + IVec3::Y), "seed {seed}: the stand is buried");
            let pin = &sites.pins[sites.gate_pin];
            assert_eq!(pin.label, "Light gate");
            assert!(!pin.present(), "no gate is on the tower when the island is built");
            assert!(sites.pins.iter().enumerate().all(|(i, p)| i == sites.gate_pin || p.present()));
        }
    }

    #[test]
    fn the_way_runs_from_each_machine_to_the_next() {
        for seed in 0..200 {
            let layout = Ch1Layout::generate(seed);
            let (world, sites) = build(&layout);
            let way = &sites.way;
            for (i, run) in way.runs.iter().enumerate() {
                let start = run.at(0.0);
                let end = run.at(run.length());
                let (from, to) = (way.machines[i].xz(), way.machines[i + 1].xz());
                assert!(start.distance(from) < 6.0, "seed {seed}: run {i} starts {} m from its machine", start.distance(from));
                assert!(end.distance(to) < 6.0, "seed {seed}: run {i} ends {} m from the next machine", end.distance(to));
                assert!(run.cells.len() as f32 > run.length() * 0.5, "seed {seed}: run {i} is mostly gaps");
                // The cells are in order along the line, set flush in the road, and quartz.
                assert!(run.cells.windows(2).all(|w| w[0].1 <= w[1].1), "seed {seed}: run {i} is out of order");
                for &(cell, _) in &run.cells {
                    assert_eq!(world.get(cell), Material::Quartz, "seed {seed}: run {i} at {cell}");
                    assert!(!world.is_solid(cell + IVec3::Y), "seed {seed}: run {i} is buried at {cell}");
                }
            }
            for (i, quartz) in way.quartz.iter().enumerate() {
                assert!(!quartz.is_empty(), "seed {seed}: machine {i} has no quartz");
                assert!(quartz.iter().all(|&q| world.get(q) == Material::Quartz), "seed {seed}: machine {i}");
                // Set into something: nothing hangs in the air.
                for &q in quartz {
                    let held = [IVec3::NEG_Y, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z].iter().any(|&d| world.is_solid(q + d));
                    assert!(held, "seed {seed}: machine {i}'s quartz at {q} floats");
                }
            }
            // No cell belongs to two stretches, or to a stretch and a machine.
            let mut seen = std::collections::HashSet::new();
            let all = way.runs.iter().flat_map(|r| r.cells.iter().map(|&(c, _)| c)).chain(way.quartz.iter().flatten().copied());
            for cell in all {
                assert!(seen.insert(cell), "seed {seed}: {cell} is lit twice");
            }
            // The north gate stands north of the pad, over the slipway, on the way to the strait.
            assert!(way.north_gate.z < sites.ridge_launch.at(layout.pad_centre, 0.0, 0.0).z - 20.0, "seed {seed}");
            assert!(way.north_gate.z > sites.shore.z, "seed {seed}: the north gate is in the sea");
            assert!(!way.ramp.is_empty(), "seed {seed}: no ramp");
        }
    }
}
