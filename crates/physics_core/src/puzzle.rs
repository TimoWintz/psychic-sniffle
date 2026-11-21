//! Puzzles: seeded numbers, judging a committed attempt, worked solutions.
//!
//! A puzzle *kind* is code (this file). Its words — title, brief, concept cards — are data in
//! `assets/curriculum`. The numbers come from a per-save layout which the island generator also reads,
//! so what the player measures in the world is what the simulator uses.

use crate::ballistics::{Flight, FlightEnd, Profile, Projectile, fall_time, fly};
use crate::units::{Dim, Quantity, sig};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Small deterministic generator (SplitMix64) so that a save's seed fixes every number.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Uniform whole number in `lo..=hi`.
    pub fn int(&mut self, lo: u32, hi: u32) -> u32 {
        lo + (self.unit() * (hi - lo + 1) as f64) as u32
    }
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.unit() * (hi - lo)
    }
}

/// The seeded numbers of chapter 1.1. Lengths are whole metres so they can be checked by counting
/// blocks; half-metre centres arise from odd-width targets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ch1Layout {
    /// Gravitational acceleration of the planet, m/s^2. Never close to 9.81.
    pub g: f64,

    /// Puzzle 0 (the tutorial): distance along the supply rail from the cart's starting point to the
    /// centre of the marker.
    pub rail_target: u32,

    /// Puzzle 1: the survey crawler's one speed along the haul track, m/s.
    pub crawler_speed: f64,
    /// Puzzle 1: how far ahead of the haul cart the crawler starts, along the track.
    pub head_start: u32,
    /// Puzzle 1: distance along the track from the haul cart's start to the transfer marker's centre.
    pub transfer_at: u32,
    /// Puzzle 1: distance between the two timing posts the stopwatch times the crawler between.
    pub post_gap: u32,

    /// Puzzle 2: height of the drop tower's release point above the ground.
    pub tower_h: u32,
    /// Puzzle 2: height of the survey mast's release point, used for the test drop.
    pub mast_h: u32,

    /// Puzzle 3: height of the ledge above the far plateau, before any blocks raise the launcher.
    pub ledge_h: u32,
    /// Puzzle 3: width of the channel between the islands, from the ledge edge to the far bank.
    pub channel_width: u32,
    /// Puzzle 3: distance from the ledge edge to the centre of the landing platform.
    pub platform_centre: f64,
    pub platform_half: f64,
    /// Puzzle 3: the speeds the ledge launcher's damaged dial turns between, m/s. From the bare ledge
    /// the casket would need more than the top of the band, so the launcher has to be raised on blocks
    /// before a speed inside it will do.
    pub channel_speed_min: f64,
    pub channel_speed_max: f64,

    /// Puzzle 4: distance from the launcher to the centre of the landing circle (level ground).
    pub pad_centre: f64,
    pub pad_half: f64,
    /// Puzzle 4: distance from the launcher to the near face of the ridge (one block thick).
    pub ridge_x: u32,
    /// Puzzle 4: height of the ridge above the launcher.
    pub ridge_h: u32,
    /// Puzzle 4: the launcher's rated maximum speed, m/s.
    pub v_max: f64,
}

impl Ch1Layout {
    pub const G_RANGE: (f64, f64) = (6.5, 13.5);
    /// Values this close to Earth's are skipped, so the familiar number never works.
    pub const G_EXCLUDED: (f64, f64) = (9.3, 10.3);
    /// Length of the supply rail: the furthest the cart can be sent.
    pub const RAIL_LENGTH: u32 = 24;
    /// Half the width of the rail marker along the rail (the marker is one block).
    pub const RAIL_MARKER_HALF: f64 = 0.5;
    /// Puzzle 1: length of the haul track, from the haul cart's start to the buffer at its end.
    pub const HAUL_LENGTH: u32 = 40;
    /// Puzzle 1: the haul cart's rated top speed, m/s.
    pub const CART_TOP_SPEED: f64 = 8.0;
    /// Puzzle 1: half the width of the transfer marker along the track (the marker is one block).
    pub const TRANSFER_HALF: f64 = 0.5;
    /// Puzzle 1: the first timing post stands this far past the crawler's start, so the crawler is
    /// well under way when the stopwatch starts.
    pub const POST_LEAD: u32 = 2;
    /// Puzzle 1: the speeds that put the coupling on the marker span at least this fraction either side
    /// of the speed needed, so the crawler's speed worked out from the stopwatch (good to about 0.12 %)
    /// always does.
    pub const CHASE_WINDOW_MIN: f64 = 0.006;
    /// Puzzle 3: how many blocks the jack frame under the ledge launcher holds (and how many lie in
    /// the pile of blocks on the ledge). Each raises the launcher one metre.
    pub const MAX_STACK: u32 = 8;
    /// Puzzle 3: no stack's needed speed comes closer than this fraction to either end of the
    /// launcher's band, so a player's own measurement of g never moves a stack in or out of it.
    pub const BAND_MARGIN: f64 = 0.015;
    /// Puzzle 3: half the length of the catch strip on the far side, along the launch line. One block:
    /// a worked-out speed lands within a few centimetres; a guessed one rarely does.
    pub const CATCH_HALF: f64 = 0.5;
    /// Puzzle 3: metres of flat ground the island keeps beyond the rim, which the furthest shot the
    /// launcher can make must come down on.
    pub const CHANNEL_RUN: usize = 70;
    /// Puzzle 3: how far the water in the channel lies below the far bank, which is the top of the far
    /// island. A casket that falls short ends its flight on the water.
    pub const WATER_DROP: f64 = 11.4;

    pub fn generate(seed: u64) -> Self {
        let mut rng = Rng::new(seed ^ 0x1_0001);

        // Draw from the allowed range with the excluded band cut out.
        let (lo, hi) = Self::G_RANGE;
        let (cut_lo, cut_hi) = Self::G_EXCLUDED;
        let span = (cut_lo - lo) + (hi - cut_hi);
        let pick = rng.unit() * span;
        let g = if pick < cut_lo - lo { lo + pick } else { cut_hi + (pick - (cut_lo - lo)) };
        let g = (g * 1000.0).round() / 1000.0;

        let tower_h = rng.int(12, 24);
        let mast_h = tower_h + rng.int(8, 14);

        // The canyon's ledge height and platform distance used to be drawn here (the channel was a canyon then). They are drawn anew
        // below, but these draws are kept so that every other number stays what it was for older saves.
        rng.int(6, 12);
        let channel_width = rng.int(12, 20);
        rng.int(4, 8);

        let pad_centre = rng.int(40, 56) as f64 + 0.5;
        let ridge_x = (pad_centre * rng.range(0.20, 0.30)).round() as u32;
        // Choose the ridge height so that the shallowest shot that clears it and still reaches the
        // pad is between 50 and 56 degrees: a 45 degree shot never works.
        let target_min_angle = rng.range(50.0, 56.0f64).to_radians();
        let shape = ridge_x as f64 * (1.0 - ridge_x as f64 / pad_centre);
        let ridge_h = (shape * target_min_angle.tan()).round() as u32;
        let min_angle = (ridge_h as f64 / shape).atan();
        // The speed limit rules out shots steeper than about twelve degrees above that.
        let max_angle = (min_angle + 12f64.to_radians()).min(74f64.to_radians());
        let v_max = (g * pad_centre / (2.0 * max_angle).sin()).sqrt();
        let v_max = (v_max * 10.0).round() / 10.0;

        // Drawn last, so the numbers above stay what they were for saves made before the tutorial.
        let rail_target = rng.int(9, 17);

        // The channel, drawn after the rest. The platform is one block long now, and the band is put between the speeds the stacks need, so
        // that the first two stacks never do and one or two stacks after them do. Draws that crowd an
        // end of the band are drawn again.
        let round = |v: f64| (v * 10.0).round() / 10.0;
        let (ledge_h, platform_centre, channel_speed_min, channel_speed_max) = loop {
            let ledge_h = rng.int(3, 5);
            let first = rng.int(1, 5);
            let last = first + rng.int(0, 1);
            let platform_centre = rng.int(channel_width + 10, channel_width + 22) as f64 + 0.5;
            let need = |stack: u32| platform_centre / fall_time((ledge_h + stack) as f64, g);
            let top = round((need(first - 1) + need(first)) / 2.0);
            let bottom = round((need(last) + need(last + 1)) / 2.0);
            let trial = Ch1Layout {
                g,
                rail_target,
                crawler_speed: 0.0,
                head_start: 0,
                transfer_at: 0,
                post_gap: 0,
                tower_h,
                mast_h,
                ledge_h,
                channel_width,
                platform_centre,
                platform_half: Self::CATCH_HALF,
                channel_speed_min: bottom,
                channel_speed_max: top,
                pad_centre,
                pad_half: 2.5,
                ridge_x,
                ridge_h,
                v_max,
            };
            if trial.channel_stacks() == (first..=last) && trial.channel_is_fair() {
                break (ledge_h, platform_centre, bottom, top);
            }
        };

        // The haul track, drawn last of all, so every number above stays what it was for older saves.
        // Draws whose window of passing speeds is too narrow for a stopwatch reading are drawn again.
        let (crawler_speed, head_start, transfer_at, post_gap) = loop {
            let crawler_speed = (rng.range(1.2, 2.4) * 1000.0).round() / 1000.0;
            let post_gap = rng.int(10, 14);
            let head_start = rng.int(5, 10);
            let transfer_at = rng.int(22, 32);
            if chase_window(head_start, transfer_at) >= Self::CHASE_WINDOW_MIN {
                break (crawler_speed, head_start, transfer_at, post_gap);
            }
        };

        Ch1Layout {
            g,
            rail_target,
            crawler_speed,
            head_start,
            transfer_at,
            post_gap,
            tower_h,
            mast_h,
            ledge_h,
            channel_width,
            platform_centre,
            platform_half: Self::CATCH_HALF,
            channel_speed_min,
            channel_speed_max,
            pad_centre,
            pad_half: 2.5,
            ridge_x,
            ridge_h,
            v_max,
        }
    }

    /// Terrain along the ledge launcher's line, with the near bank at `x = 0` and the far bank at
    /// `y = 0`. Over the channel the surface is the water's.
    pub fn channel_profile(&self) -> Profile {
        // Long enough for the furthest shot the launcher can make, with room to spare.
        let furthest = self.channel_speed_max * fall_time((self.ledge_h + Self::MAX_STACK) as f64, self.g);
        let length = (furthest.max(self.platform_centre + self.platform_half) + 6.0).ceil() as usize;
        let mut p = Profile::flat(0.0, length, 0.0);
        p.set(0.0, self.channel_width as f64, -Self::WATER_DROP);
        p
    }

    /// The ledge launcher's shot with `stack` blocks under it, at `speed`.
    pub fn channel_shot(&self, stack: u32, speed: f64) -> (Projectile, Flight) {
        let projectile = Projectile::from_angle(0.0, (self.ledge_h + stack) as f64, speed, 0.0, self.g);
        (projectile, fly(&projectile, &self.channel_profile()))
    }

    /// The speed that puts the casket on the platform centre with `stack` blocks under the launcher.
    pub fn channel_speed_needed(&self, stack: u32) -> f64 {
        self.platform_centre / fall_time((self.ledge_h + stack) as f64, self.g)
    }

    /// The stacks for which the speed needed is inside the launcher's band.
    pub fn channel_stacks(&self) -> std::ops::RangeInclusive<u32> {
        let inside = |n: &u32| (self.channel_speed_min..=self.channel_speed_max).contains(&self.channel_speed_needed(*n));
        let first = (0..=Self::MAX_STACK).find(inside).unwrap_or(Self::MAX_STACK + 1);
        let last = (0..=Self::MAX_STACK).rev().find(inside).unwrap_or(0);
        first..=last
    }

    /// Whether the band is within the launcher's limits and no stack's speed crowds an end of it,
    /// and every stack inside it, at its speed, lands on the platform past the far rim.
    fn channel_is_fair(&self) -> bool {
        let (low, high) = (self.channel_speed_min, self.channel_speed_max);
        if high >= LAUNCHER_HARD_LIMIT || low >= high || self.channel_profile().heights.len() > Self::CHANNEL_RUN {
            return false;
        }
        let clear = |n: u32| {
            let v = self.channel_speed_needed(n);
            ((v - low) / v).abs() >= Self::BAND_MARGIN && ((v - high) / v).abs() >= Self::BAND_MARGIN
        };
        let stacks = self.channel_stacks();
        !stacks.is_empty()
            && (0..=Self::MAX_STACK).all(clear)
            && stacks.clone().all(|n| {
                let (projectile, flight) = self.channel_shot(n, self.channel_speed_needed(n));
                flight.end == FlightEnd::Landed
                    && (flight.x - self.platform_centre).abs() < 0.1
                    && projectile.height_at_x(self.channel_width as f64).is_some_and(|y| y > 1.0)
            })
    }

    /// Puzzle 1: where the timing posts stand, as distances along the track from the haul cart's start.
    pub fn posts(&self) -> (u32, u32) {
        let a = self.head_start + Self::POST_LEAD;
        (a, a + self.post_gap)
    }

    /// Puzzle 1: the crawler's time from the first timing post to the second, as the stopwatch shows it
    /// (hundredths of a second).
    pub fn crawler_time(&self) -> f64 {
        (self.post_gap as f64 / self.crawler_speed * 100.0).round() / 100.0
    }

    /// Puzzle 1: the haul cart sent after the crawler at `speed`. Both start together and run at their
    /// steady speeds; the run ends when the cart reaches the crawler, or when the crawler reaches the
    /// buffer at the end of the track first.
    pub fn chase(&self, speed: f64) -> ChaseRun {
        let (u, d, end) = (self.crawler_speed, self.head_start as f64, Self::HAUL_LENGTH as f64);
        if speed > u {
            let t = d / (speed - u);
            let x = speed * t;
            if x <= end {
                return ChaseRun { t, cart: x, crawler: x, coupled: true };
            }
        }
        let t = (end - d) / u;
        ChaseRun { t, cart: speed * t, crawler: end, coupled: false }
    }

    /// Puzzle 1: the cart speed that reaches the crawler on the marker's centre.
    pub fn chase_speed_needed(&self) -> f64 {
        let (d, m) = (self.head_start as f64, self.transfer_at as f64);
        self.crawler_speed * m / (m - d)
    }

    /// Fall time from the drop tower as the stopwatch shows it (hundredths of a second).
    pub fn tower_fall_time(&self) -> f64 {
        self.gate_time(self.tower_h)
    }

    /// Puzzle 2: the light gate's reading with the gate clamped `drop` metres below the release point
    /// (one of the notches `1..=tower_h`; the last is at the landing plate): the time from the release
    /// until the mass reaches the gate, as the stopwatch shows it (hundredths of a second).
    pub fn gate_time(&self, drop: u32) -> f64 {
        (fall_time(drop as f64, self.g) * 100.0).round() / 100.0
    }

    /// Puzzle 2: the notches the light gate can be clamped at, as drops below the release point.
    pub fn gate_notches(&self) -> std::ops::RangeInclusive<u32> {
        1..=self.tower_h
    }

    /// Launch angles (radians) between which a shot aimed at the centre of the circle clears the ridge and
    /// stays within the launcher's rating.
    pub fn ridge_angle_window(&self) -> (f64, f64) {
        let shape = self.ridge_x as f64 * (1.0 - self.ridge_x as f64 / self.pad_centre);
        let min = (self.ridge_h as f64 / shape).atan();
        // v^2 = g D / sin(2 theta) <= v_max^2, on the steep side of 45 degrees.
        let s = (self.g * self.pad_centre / (self.v_max * self.v_max)).min(1.0);
        let max = (std::f64::consts::PI - s.asin()) / 2.0;
        (min, max)
    }
}

/// How far either side of the speed needed, as a fraction of it, the cart's speed may be and still
/// reach the crawler on the transfer marker. The coupling point is `x = v D / (v - u)`, so a change in
/// `v` moves it by `(M - D)^2 / (u D)` metres per m/s at the marker; the marker's half-width over that,
/// relative to `v = u M / (M - D)`, leaves the crawler's speed out.
fn chase_window(head_start: u32, transfer_at: u32) -> f64 {
    let (d, m) = (head_start as f64, transfer_at as f64);
    Ch1Layout::TRANSFER_HALF * d / ((m - d) * m)
}

/// A run of the haul cart after the crawler.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChaseRun {
    /// When the run ended, s.
    pub t: f64,
    /// Where the cart and the crawler were then, along the track from the cart's start.
    pub cart: f64,
    pub crawler: f64,
    /// Whether the cart reached the crawler (otherwise the crawler reached the buffer first).
    pub coupled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PuzzleKind {
    /// The tutorial: send a cart along a rail. Measure, enter, commit; no calculation.
    Rail,
    /// Catch a crawler running at a steady speed: velocity and relative velocity.
    Chase,
    Gravity,
    Channel,
    Ridge,
}

/// Where a committed value comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSource {
    /// Typed into the blueprint: a number with its unit, or an expression over the notebook.
    Typed,
    /// Read from the world: what the player has built or placed there (the blocks under the ledge
    /// launcher). Shown in the blueprint, not typed.
    Placed,
}

/// One value the player must commit.
#[derive(Clone, Debug, PartialEq)]
pub struct InputSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub dim: Dim,
    /// Unit the value is shown in and suggested to the player ("m/s", "deg").
    pub unit: &'static str,
    pub example: &'static str,
    pub source: InputSource,
}

/// A fact about the world that the player can measure.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Given {
    pub label: String,
    pub quantity: Quantity,
}

/// Committed values by [`InputSpec::key`].
pub type Inputs = BTreeMap<String, Quantity>;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum InputError {
    #[error("{label} hasn't been entered yet.")]
    Missing { label: String },
    #[error("{label} needs to be {expected}, but you entered {got}.")]
    Dimension { label: String, expected: String, got: String },
    #[error("{label}: {reason}")]
    OutOfRange { label: String, reason: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    pub label: String,
    /// What the player's values imply, or the target they were aiming for.
    pub expected: Quantity,
    pub observed: Quantity,
}

/// What the world should show for a test.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Playback {
    /// A mass released from rest at `height`; a ghost follows the player's prediction.
    Drop { height: f64, g: f64, predicted_time: f64, actual_time: f64 },
    Flight { projectile: Projectile, flight: Flight },
    /// The rail cart drives `distance` metres along its rail and stops.
    Slide { distance: f64 },
    /// The haul cart sets off after the crawler, each at its steady speed, both from their marks, for
    /// `t` seconds: until the cart reaches the crawler, or the crawler reaches the buffer.
    Chase { cart_speed: f64, crawler_speed: f64, head_start: f64, t: f64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    pub pass: bool,
    pub headline: String,
    /// What happened, in physical terms. Never the answer.
    pub detail: String,
    pub comparisons: Vec<Comparison>,
    pub playback: Playback,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Answer {
    pub key: String,
    pub label: String,
    pub quantity: Quantity,
    pub unit: String,
}

/// A worked solution. For the tutor and for tests; never shown to the player.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Solution {
    pub steps: Vec<String>,
    /// One set of values that passes. Where several do (the ridge), `note` says so.
    pub answers: Vec<Answer>,
    pub note: String,
}

impl Solution {
    pub fn inputs(&self) -> Inputs {
        self.answers.iter().map(|a| (a.key.clone(), a.quantity)).collect()
    }
}

/// A puzzle with its numbers filled in.
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub kind: PuzzleKind,
    pub layout: Ch1Layout,
}

/// Fraction by which the player's `g` may differ from the true value.
pub const GRAVITY_TOLERANCE: f64 = 0.02;
/// Highest speed any launcher accepts, m/s.
pub const LAUNCHER_HARD_LIMIT: f64 = 60.0;

impl Instance {
    pub fn new(kind: PuzzleKind, layout: &Ch1Layout) -> Self {
        Instance { kind, layout: layout.clone() }
    }

    pub fn inputs(&self) -> Vec<InputSpec> {
        let speed = InputSpec {
            key: "speed",
            label: "Launch speed",
            dim: Dim::SPEED,
            unit: "m/s",
            example: "14.2 m/s",
            source: InputSource::Typed,
        };
        match self.kind {
            PuzzleKind::Rail => vec![InputSpec {
                key: "distance",
                label: "Run distance",
                dim: Dim::LENGTH,
                unit: "m",
                example: "d1   or   12 m",
                source: InputSource::Typed,
            }],
            PuzzleKind::Chase => vec![InputSpec {
                key: "speed",
                label: "Cart speed",
                dim: Dim::SPEED,
                unit: "m/s",
                example: "2.6 m/s",
                source: InputSource::Typed,
            }],
            PuzzleKind::Gravity => vec![InputSpec {
                key: "g",
                label: "Gravitational acceleration",
                dim: Dim::ACCELERATION,
                unit: "m/s^2",
                example: "3.7 m/s^2",
                source: InputSource::Typed,
            }],
            PuzzleKind::Channel => vec![
                InputSpec {
                    key: "stack",
                    label: "Launcher raised by",
                    dim: Dim::LENGTH,
                    unit: "m",
                    example: "",
                    source: InputSource::Placed,
                },
                speed,
            ],
            PuzzleKind::Ridge => vec![
                InputSpec {
                    key: "angle",
                    label: "Launch angle",
                    dim: Dim::NONE,
                    unit: "deg",
                    example: "30 deg",
                    source: InputSource::Typed,
                },
                speed,
            ],
        }
    }

    /// The measurable facts of this puzzle, with their true values.
    pub fn givens(&self) -> Vec<Given> {
        let l = &self.layout;
        let given = |label: &str, quantity| Given { label: label.to_string(), quantity };
        match self.kind {
            PuzzleKind::Rail => vec![
                given("distance from the cart to the marker centre", Quantity::metres(l.rail_target as f64)),
                given("length of the rail", Quantity::metres(Ch1Layout::RAIL_LENGTH as f64)),
            ],
            PuzzleKind::Chase => {
                let (a, b) = l.posts();
                vec![
                    given("distance from the haul cart to the crawler (its head start)", Quantity::metres(l.head_start as f64)),
                    given("distance from the haul cart to the transfer marker centre", Quantity::metres(l.transfer_at as f64)),
                    given("distance between the two timing posts", Quantity::metres((b - a) as f64)),
                    given("crawler's time from the first timing post to the second", Quantity::seconds(l.crawler_time())),
                    given("haul cart's top speed", Quantity::speed(Ch1Layout::CART_TOP_SPEED)),
                    given("length of the haul track", Quantity::metres(Ch1Layout::HAUL_LENGTH as f64)),
                ]
            }
            PuzzleKind::Gravity => vec![
                given("height of the drop tower", Quantity::metres(l.tower_h as f64)),
                given("fall time from the drop tower", Quantity::seconds(l.tower_fall_time())),
                given("height of the survey mast", Quantity::metres(l.mast_h as f64)),
            ],
            PuzzleKind::Channel => vec![
                given("drop from the ledge to the platform, with no blocks under the launcher", Quantity::metres(l.ledge_h as f64)),
                given("horizontal distance to the platform centre", Quantity::metres(l.platform_centre)),
                given("length of the platform", Quantity::metres(2.0 * l.platform_half)),
                given("lowest speed the ledge launcher's dial turns to", Quantity::speed(l.channel_speed_min)),
                given("highest speed the ledge launcher's dial turns to", Quantity::speed(l.channel_speed_max)),
                given("height of one block", Quantity::metres(1.0)),
            ],
            PuzzleKind::Ridge => vec![
                given("distance to the centre of the circle", Quantity::metres(l.pad_centre)),
                given("length of the landing circle", Quantity::metres(2.0 * l.pad_half)),
                given("distance to the near face of the ridge", Quantity::metres(l.ridge_x as f64)),
                given("height of the ridge", Quantity::metres(l.ridge_h as f64)),
                given("maximum launch speed", Quantity::speed(l.v_max)),
            ],
        }
    }

    /// Terrain along the launch line, with the release point at `x = 0`. `None` for the drop.
    pub fn profile(&self) -> Option<Profile> {
        let l = &self.layout;
        match self.kind {
            PuzzleKind::Rail | PuzzleKind::Chase | PuzzleKind::Gravity => None,
            PuzzleKind::Channel => Some(l.channel_profile()),
            PuzzleKind::Ridge => {
                let length = (l.pad_centre + l.pad_half) as usize + 40;
                let mut p = Profile::flat(0.0, length, 0.0);
                p.set(l.ridge_x as f64, l.ridge_x as f64 + 1.0, l.ridge_h as f64);
                Some(p)
            }
        }
    }

    /// The tilting launcher's projectile for a speed and an angle.
    fn projectile(&self, speed: f64, angle: f64) -> Projectile {
        Projectile::from_angle(0.0, 0.0, speed, angle, self.layout.g)
    }

    /// What stands at this puzzle's station in the world, given what the save says is placed there
    /// (`placed`: blocks in the jack frame, the cart's block on the cart, the light gate's notch), in
    /// words for the station panel and the tutor. `None` where nothing is placed (the haul track, the
    /// ridge).
    pub fn built(&self, placed: u32) -> Option<String> {
        let plural = |n: u32| if n == 1 { "" } else { "s" };
        Some(match self.kind {
            PuzzleKind::Rail if placed == 0 => "the cart is empty, and its block is beside the rail".into(),
            PuzzleKind::Rail => "the block is on the cart".into(),
            PuzzleKind::Gravity if placed == 0 => "no light gate on the drop tower".into(),
            PuzzleKind::Gravity => format!("the light gate is clamped {placed} m below the release point"),
            PuzzleKind::Channel => format!("{placed} block{} in the jack frame", plural(placed)),
            PuzzleKind::Chase | PuzzleKind::Ridge => return None,
        })
    }

    /// Why the station will not run a test with what is placed in the world, before anything typed
    /// is read: the supply rail's cart must carry its cargo. Refused for free, like a value of the
    /// wrong kind, but it is not a value committed, so it writes no handbook page.
    pub fn unready(&self, placed: u32) -> Option<String> {
        match self.kind {
            PuzzleKind::Rail if placed == 0 => {
                Some("Load the cart first. Pick up the block beside the rail (F), then put it on the cart (F).".into())
            }
            _ => None,
        }
    }

    /// Why the station will not run its experiment with what is placed in the world: the drop tower
    /// times nothing until the light gate is clamped on it. Refused for free; nothing is noted.
    pub fn experiment_unready(&self, placed: u32) -> Option<String> {
        match self.kind {
            PuzzleKind::Gravity if placed == 0 => Some(
                "The experiment needs the light gate on the tower. Leave the console, pick the gate up from its stand (F), then point at a notch on the tower and press F."
                    .into(),
            ),
            _ => None,
        }
    }

    /// Check that every input is present, has the right dimension and is physically usable. Passing
    /// this check is what "the launcher accepts the values" means; it costs no prototype charge.
    pub fn validate(&self, inputs: &Inputs) -> Result<(), InputError> {
        for spec in self.inputs() {
            let label = spec.label.to_string();
            let Some(q) = inputs.get(spec.key) else {
                return Err(InputError::Missing { label });
            };
            if q.dim != spec.dim {
                return Err(InputError::Dimension {
                    label,
                    expected: spec.dim.describe(),
                    got: q.dim.describe(),
                });
            }
            let out_of_range = |reason: String| Err(InputError::OutOfRange { label: label.clone(), reason });
            match spec.key {
                "distance" => {
                    if q.value <= 0.0 {
                        return out_of_range("the cart only runs forward, so enter a distance greater than zero.".into());
                    }
                    if q.value > Ch1Layout::RAIL_LENGTH as f64 {
                        return out_of_range(format!(
                            "the rail is {} m long, and the cart won't run past its end.",
                            Ch1Layout::RAIL_LENGTH
                        ));
                    }
                }
                "stack" => {
                    if q.value < -1e-9 {
                        return out_of_range("the launcher can't stand lower than the ledge.".into());
                    }
                    if (q.value - q.value.round()).abs() > 1e-6 {
                        return out_of_range("the jack frame only takes whole blocks, each one metre high.".into());
                    }
                    if q.value.round() > Ch1Layout::MAX_STACK as f64 {
                        return out_of_range(format!("the jack frame holds at most {} blocks.", Ch1Layout::MAX_STACK));
                    }
                }
                "g" if q.value <= 0.0 => {
                    return out_of_range("gravity pulls downwards, so enter its size as a positive value.".into());
                }
                "speed" => {
                    let (limit, what, verb) = match self.kind {
                        PuzzleKind::Ridge => (self.layout.v_max, "launcher", "fire"),
                        PuzzleKind::Chase => (Ch1Layout::CART_TOP_SPEED, "haul cart", "run"),
                        _ => (LAUNCHER_HARD_LIMIT, "launcher", "fire"),
                    };
                    if q.value <= 0.0 {
                        return out_of_range("the speed has to be greater than zero.".into());
                    }
                    if self.kind == PuzzleKind::Channel {
                        let (low, high) = (self.layout.channel_speed_min, self.layout.channel_speed_max);
                        if !(low..=high).contains(&q.value) {
                            return out_of_range(format!(
                                "the launcher's damaged dial only turns from {} to {}, and {} is outside that range.",
                                Quantity::speed(low).display(),
                                Quantity::speed(high).display(),
                                Quantity::speed(q.value).display()
                            ));
                        }
                    }
                    if q.value > limit {
                        return out_of_range(format!(
                            "the {what} is rated to {} and won't {verb} any faster.",
                            Quantity::speed(limit).display()
                        ));
                    }
                }
                "angle" => {
                    let deg = q.value.to_degrees();
                    if q.value > std::f64::consts::FRAC_PI_2 * 1.0001 && q.value <= 90.0 {
                        return out_of_range(format!(
                            "{} radians isn't a launch angle. Did you mean {} deg?",
                            sig(q.value, 4),
                            sig(q.value, 4)
                        ));
                    }
                    if !(1.0..=89.0).contains(&deg) {
                        return out_of_range("the launcher tilts between 1 and 89 degrees.".into());
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Judge a committed attempt.
    pub fn evaluate(&self, inputs: &Inputs) -> Result<Outcome, InputError> {
        self.validate(inputs)?;
        let l = &self.layout;
        let value = |key: &str| inputs[key].value;
        Ok(match self.kind {
            PuzzleKind::Rail => {
                let target = l.rail_target as f64;
                let run = value("distance");
                let miss = run - target;
                let pass = miss.abs() <= Ch1Layout::RAIL_MARKER_HALF;
                let (headline, detail) = if pass {
                    (
                        "On the marker".to_string(),
                        format!("The cart ran {} m and stopped on the marker.", sig(run, 3)),
                    )
                } else {
                    let (headline, side) = if miss < 0.0 { ("Stopped short", "short of") } else { ("Overshot", "beyond") };
                    (
                        headline.to_string(),
                        format!(
                            "The cart ran {} m and stopped {} m {side} the marker centre.",
                            sig(run, 3),
                            sig(miss.abs(), 3)
                        ),
                    )
                };
                Outcome {
                    pass,
                    headline,
                    detail,
                    comparisons: vec![Comparison {
                        label: "Distance run".into(),
                        expected: Quantity::metres(target),
                        observed: Quantity::metres(run),
                    }],
                    playback: Playback::Slide { distance: run },
                }
            }
            PuzzleKind::Chase => {
                let speed = value("speed");
                let run = l.chase(speed);
                let target = l.transfer_at as f64;
                let miss = run.cart - target;
                let pass = run.coupled && miss.abs() <= Ch1Layout::TRANSFER_HALF;
                let (headline, detail) = if pass {
                    (
                        "Coupled on the marker".to_string(),
                        format!(
                            "The cart reached the crawler after {} s, {} m from the marker centre.",
                            sig(run.t, 3),
                            sig(miss.abs(), 2)
                        ),
                    )
                } else if run.coupled {
                    let (headline, side) = if miss < 0.0 { ("Caught it too soon", "before") } else { ("Caught it too late", "past") };
                    (
                        headline.to_string(),
                        format!(
                            "The cart reached the crawler after {} s, {} m {side} the marker centre.",
                            sig(run.t, 3),
                            sig(miss.abs(), 3)
                        ),
                    )
                } else {
                    (
                        "Never caught it".to_string(),
                        format!(
                            "The crawler reached the buffer at the end of the track after {} s, still {} m ahead of the cart.",
                            sig(run.t, 3),
                            sig(run.crawler - run.cart, 3)
                        ),
                    )
                };
                let comparisons = if run.coupled {
                    vec![Comparison {
                        label: "Where the cart reached the crawler".into(),
                        expected: Quantity::metres(target),
                        observed: Quantity::metres(run.cart),
                    }]
                } else {
                    vec![Comparison {
                        label: "Gap left when the crawler stopped".into(),
                        expected: Quantity::metres(0.0),
                        observed: Quantity::metres(run.crawler - run.cart),
                    }]
                };
                Outcome {
                    pass,
                    headline,
                    detail,
                    comparisons,
                    playback: Playback::Chase {
                        cart_speed: speed,
                        crawler_speed: l.crawler_speed,
                        head_start: l.head_start as f64,
                        t: run.t,
                    },
                }
            }
            PuzzleKind::Gravity => {
                let g_player = value("g");
                let height = l.mast_h as f64;
                let predicted_time = fall_time(height, g_player);
                let actual_time = fall_time(height, l.g);
                let pass = ((g_player - l.g) / l.g).abs() <= GRAVITY_TOLERANCE;
                let (headline, detail) = if pass {
                    (
                        "Calibrated".to_string(),
                        format!(
                            "The mass fell from the {} m mast in {} s, and your value predicted {} s.",
                            l.mast_h,
                            sig(actual_time, 3),
                            sig(predicted_time, 3)
                        ),
                    )
                } else {
                    let (speed_word, strength_word) =
                        if actual_time < predicted_time { ("sooner", "stronger") } else { ("later", "weaker") };
                    (
                        "Prediction doesn't match".to_string(),
                        format!(
                            "The mass fell from the {} m mast in {} s, but your value predicted {} s. It landed \
                             {speed_word} than predicted, so gravity here is {strength_word} than your value.",
                            l.mast_h,
                            sig(actual_time, 3),
                            sig(predicted_time, 3)
                        ),
                    )
                };
                Outcome {
                    pass,
                    headline,
                    detail,
                    comparisons: vec![Comparison {
                        label: format!("Fall time from the mast ({} m)", l.mast_h),
                        expected: Quantity::seconds(predicted_time),
                        observed: Quantity::seconds(actual_time),
                    }],
                    playback: Playback::Drop { height, g: l.g, predicted_time, actual_time },
                }
            }
            PuzzleKind::Channel => {
                let (projectile, flight) = l.channel_shot(value("stack").round() as u32, value("speed"));
                let miss = flight.x - l.platform_centre;
                let on_plateau = flight.end == FlightEnd::Landed && flight.surface == 0.0;
                let pass = on_plateau && miss.abs() <= l.platform_half;
                let (headline, detail) = if pass {
                    (
                        "On the platform".to_string(),
                        format!(
                            "The casket was in the air for {} s and landed {} m from the platform centre.",
                            sig(flight.t, 3),
                            sig(miss.abs(), 2)
                        ),
                    )
                } else if on_plateau {
                    let side = if miss < 0.0 { "short of" } else { "beyond" };
                    (
                        "Missed the platform".to_string(),
                        format!(
                            "The casket was in the air for {} s and landed {} m {side} the platform centre.",
                            sig(flight.t, 3),
                            sig(miss.abs(), 3)
                        ),
                    )
                } else if flight.end == FlightEnd::StruckFace {
                    (
                        "Fell short".to_string(),
                        format!(
                            "The casket fell short and struck the far bank {} m below the top.",
                            sig(-flight.y, 3)
                        ),
                    )
                } else {
                    (
                        "Fell short".to_string(),
                        format!(
                            "The casket fell into the channel {} m out from the ledge.",
                            sig(flight.x, 3)
                        ),
                    )
                };
                Outcome {
                    pass,
                    headline,
                    detail,
                    comparisons: vec![Comparison {
                        label: "Horizontal distance travelled".into(),
                        expected: Quantity::metres(l.platform_centre),
                        observed: Quantity::metres(flight.x),
                    }],
                    playback: Playback::Flight { projectile, flight },
                }
            }
            PuzzleKind::Ridge => {
                let projectile = self.projectile(value("speed"), value("angle"));
                let flight = fly(&projectile, &self.profile().expect("ridge has terrain"));
                let ridge_x = l.ridge_x as f64;
                let ridge_h = l.ridge_h as f64;
                let miss = flight.x - l.pad_centre;
                let landed_level = flight.end == FlightEnd::Landed && flight.surface == 0.0;
                let pass = landed_level && miss.abs() <= l.pad_half;
                let (headline, detail) = if pass {
                    (
                        "In the circle".to_string(),
                        format!(
                            "The tiller cleared the ridge by {} m and landed {} m from the centre of the circle.",
                            sig(projectile.height_at_x(ridge_x).unwrap_or(0.0) - ridge_h, 2),
                            sig(miss.abs(), 2)
                        ),
                    )
                } else if flight.end == FlightEnd::StruckFace {
                    (
                        "Struck the ridge".to_string(),
                        format!("The tiller hit the ridge {} m below its top.", sig(ridge_h - flight.y, 3)),
                    )
                } else if flight.end == FlightEnd::Landed && flight.surface > 0.0 {
                    ("Landed on the ridge".to_string(), "The tiller came down on top of the ridge.".to_string())
                } else if flight.x < ridge_x {
                    (
                        "Fell short".to_string(),
                        format!("The tiller came down {} m before the ridge.", sig(ridge_x - flight.x, 3)),
                    )
                } else {
                    let side = if miss < 0.0 { "short of" } else { "beyond" };
                    (
                        "Missed the circle".to_string(),
                        format!(
                            "The tiller cleared the ridge and landed {} m {side} the centre of the circle.",
                            sig(miss.abs(), 3)
                        ),
                    )
                };
                let mut comparisons = Vec::new();
                if let Some(y) = projectile.height_at_x(ridge_x)
                    && flight.x >= ridge_x
                {
                    comparisons.push(Comparison {
                        label: "Height on reaching the ridge (must exceed the first value)".into(),
                        expected: Quantity::metres(ridge_h),
                        observed: Quantity::metres(y),
                    });
                }
                comparisons.push(Comparison {
                    label: "Horizontal distance travelled".into(),
                    expected: Quantity::metres(l.pad_centre),
                    observed: Quantity::metres(flight.x),
                });
                Outcome { pass, headline, detail, comparisons, playback: Playback::Flight { projectile, flight } }
            }
        })
    }

    pub fn solution(&self) -> Solution {
        let l = &self.layout;
        let answer = |key: &str, label: &str, quantity: Quantity, unit: &str| Answer {
            key: key.into(),
            label: label.into(),
            quantity,
            unit: unit.into(),
        };
        match self.kind {
            PuzzleKind::Rail => {
                let d = l.rail_target;
                Solution {
                    steps: vec![
                        "Nothing to calculate: the cart's motor drives it exactly the distance entered.".into(),
                        format!(
                            "Measure with the ruler from the cart's starting point to the centre of the marker: {d} m \
                             horizontally. Note it, then enter it with its unit (or by its notebook name)."
                        ),
                    ],
                    answers: vec![answer("distance", "Run distance", Quantity::metres(d as f64), "m")],
                    note: format!(
                        "The marker is one block, so anything within {} m of {d} m passes. The point of this puzzle is \
                         the routine: ruler, notebook, units, commit, read the result.",
                        Ch1Layout::RAIL_MARKER_HALF
                    ),
                }
            }
            PuzzleKind::Chase => {
                let (s, t) = (l.post_gap as f64, l.crawler_time());
                let (d, m) = (l.head_start as f64, l.transfer_at as f64);
                let u = s / t;
                let meet = (m - d) / u;
                let v = m / meet;
                // A slower cart reaches the crawler further on: the slowest that passes does at the
                // marker's far edge.
                let far = m + Ch1Layout::TRANSFER_HALF;
                let window = (v - u * far / (far - d)) / v;
                Solution {
                    steps: vec![
                        format!(
                            "The crawler runs at a steady speed: the distance between the timing posts over the \
                             time it takes, u = {s} m / {t} s = {} m/s.",
                            sig(u, 4)
                        ),
                        format!(
                            "It starts D = {d} m ahead. Running the same way, the cart gains on it at v - u, so \
                             the gap closes in D / (v - u). By then the crawler must be at the marker, {} m further \
                             on from its start: that takes (M - D) / u = {} s.",
                            m - d,
                            sig(meet, 4)
                        ),
                        format!(
                            "In that time the cart covers the whole M = {m} m to the marker: v = M / t = u M / (M - D) \
                             = {} m/s.",
                            sig(v, 4)
                        ),
                    ],
                    answers: vec![answer("speed", "Cart speed", Quantity::speed(v), "m/s")],
                    note: format!(
                        "The true crawler speed is {} m/s. The marker is one block, so the cart may reach the \
                         crawler within {} m of {m} m: speeds within about {}% of the one needed pass. The \
                         stopwatch reading of the crawler is good to about 0.1%.",
                        l.crawler_speed,
                        Ch1Layout::TRANSFER_HALF,
                        sig(window * 100.0, 2)
                    ),
                }
            }
            PuzzleKind::Gravity => {
                let (h, t) = (l.tower_h as f64, l.tower_fall_time());
                Solution {
                    steps: vec![
                        "A mass released from rest falls a height h in a time t with h = (1/2) g t^2.".into(),
                        "Rearranged: g = 2 h / t^2.".into(),
                        format!(
                            "With the light gate at the plate notch, h = {h} m (the whole tower) and the timed fall t = {t} s: g = 2 x {h} / {t}^2 = {} m/s^2.",
                            sig(2.0 * h / (t * t), 4)
                        ),
                    ],
                    answers: vec![answer("g", "Gravitational acceleration", Quantity::acceleration(l.g), "m/s^2")],
                    note: format!(
                        "The true value is {} m/s^2. The stopwatch reads to 0.01 s, so the player's result differs \
                         slightly; anything within {}% passes. The experiment only runs with the light gate clamped \
                         at a notch, and the same relation holds for the drop h to the gate and the time t to it. Short drops are worth \
                         less: the rounding is a bigger share of a short time, so a reading of a second or more \
                         gives g to 1%, and fitting several notches shows that doubling the drop does not double \
                         the time.",
                        l.g,
                        GRAVITY_TOLERANCE * 100.0
                    ),
                }
            }
            PuzzleKind::Channel => {
                let (low, high, d, ledge) = (l.channel_speed_min, l.channel_speed_max, l.platform_centre, l.ledge_h as f64);
                let stacks = l.channel_stacks();
                let stack = *stacks.start();
                let v = l.channel_speed_needed(stack);
                // Release heights at which the band's two ends reach the platform centre.
                let at = |speed: f64| l.g * d * d / (2.0 * speed * speed);
                let others: Vec<String> = stacks.clone().skip(1).map(|n| n.to_string()).collect();
                Solution {
                    steps: vec![
                        "Horizontal and vertical motion are independent. The launcher fires horizontally, so the \
                         block starts with no vertical speed: from a release height H it falls for t = sqrt(2 H / g), \
                         and in that time covers d = v t."
                            .into(),
                        format!(
                            "So the speed needed to reach d = {d} m from height H is v = d / sqrt(2 H / g). The dial \
                             turns from {low} to {high} m/s, which allows H from g d^2 / (2 v^2): between {} m (at \
                             {high} m/s) and {} m (at {low} m/s).",
                            sig(at(high), 3),
                            sig(at(low), 3)
                        ),
                        format!(
                            "The ledge is {ledge} m above the platform and blocks come in whole metres, so {} \
                             block{} will do.",
                            if others.is_empty() { stack.to_string() } else { format!("{stack} or {}", others.join(" or ")) },
                            if stacks.end() == &1 { "" } else { "s" }
                        ),
                        format!(
                            "With {stack} blocks H = {} m, t = {} s and v = d / t = {} m/s.",
                            ledge + stack as f64,
                            sig(fall_time(ledge + stack as f64, l.g), 4),
                            sig(v, 4)
                        ),
                    ],
                    answers: vec![
                        answer("stack", "Launcher raised by", Quantity::metres(stack as f64), "m"),
                        answer("speed", "Launch speed", Quantity::speed(v), "m/s"),
                    ],
                    note: format!(
                        "Fewer blocks would need more than {high} m/s, more would need less than {low} m/s. {}The \
                         platform is {} m long, so landing within {} m of the centre passes.",
                        if others.is_empty() {
                            String::new()
                        } else {
                            format!("{} blocks with the matching speed works too. ", others.join(" or "))
                        },
                        2.0 * l.platform_half,
                        l.platform_half
                    ),
                }
            }
            PuzzleKind::Ridge => {
                let (min, max) = l.ridge_angle_window();
                let angle = (min + max) / 2.0;
                let v = (l.g * l.pad_centre / (2.0 * angle).sin()).sqrt();
                let (d, xr, hr) = (l.pad_centre, l.ridge_x as f64, l.ridge_h as f64);
                Solution {
                    steps: vec![
                        "Components: x = v cos(theta) t and y = v sin(theta) t - (1/2) g t^2.".into(),
                        format!(
                            "Launcher and circle are level, so the range is R = v^2 sin(2 theta) / g and must equal \
                             D = {d} m."
                        ),
                        format!(
                            "Eliminating v for a shot that lands at D gives the path y = x tan(theta) (1 - x/D). At \
                             the ridge (x = {xr} m) this must exceed {hr} m, so tan(theta) > {hr} / ({xr} (1 - {xr}/{d})), \
                             i.e. theta > {} deg.",
                            sig(min.to_degrees(), 3)
                        ),
                        format!(
                            "The speed limit: v^2 = g D / sin(2 theta) <= {}^2 rules out theta > {} deg.",
                            l.v_max,
                            sig(max.to_degrees(), 3)
                        ),
                        format!(
                            "Pick an angle inside the window with margin, e.g. {} deg, then v = sqrt(g D / sin(2 theta)) = {} m/s.",
                            sig(angle.to_degrees(), 3),
                            sig(v, 4)
                        ),
                    ],
                    answers: vec![
                        answer("angle", "Launch angle", Quantity::number(angle), "deg"),
                        answer("speed", "Launch speed", Quantity::speed(v), "m/s"),
                    ],
                    note: format!(
                        "Any angle between about {} and {} deg works with the matching speed; the values above \
                         are one example. A 45 degree shot hits the ridge.",
                        sig(min.to_degrees(), 3),
                        sig(max.to_degrees(), 3)
                    ),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEEDS: u64 = 1000;
    const ALL_KINDS: [PuzzleKind; 5] = [PuzzleKind::Rail, PuzzleKind::Chase, PuzzleKind::Gravity, PuzzleKind::Channel, PuzzleKind::Ridge];

    fn inputs(pairs: &[(&str, Quantity)]) -> Inputs {
        pairs.iter().map(|(k, q)| (k.to_string(), *q)).collect()
    }

    #[test]
    fn layout_is_deterministic_and_in_range() {
        assert_eq!(Ch1Layout::generate(7), Ch1Layout::generate(7));
        assert_ne!(Ch1Layout::generate(7), Ch1Layout::generate(8));
        for seed in 0..SEEDS {
            let l = Ch1Layout::generate(seed);
            assert!((6.5..=13.5).contains(&l.g), "g {}", l.g);
            assert!(!(9.3..10.3).contains(&l.g), "g {} is too close to Earth's", l.g);
            assert!(l.mast_h > l.tower_h);
            assert!(l.platform_centre - l.platform_half >= l.channel_width as f64 + 2.0);
            assert!((l.ridge_x as f64) < l.pad_centre / 2.0);
            let (min, max) = l.ridge_angle_window();
            assert!(min.to_degrees() > 47.0, "seed {seed}: 45 degrees must not work ({})", min.to_degrees());
            assert!((max - min).to_degrees() >= 8.0, "seed {seed}: window {}", (max - min).to_degrees());
            assert!(l.v_max < LAUNCHER_HARD_LIMIT);
            assert!((9..=17).contains(&l.rail_target), "rail target {}", l.rail_target);
            assert!(l.rail_target + 4 <= Ch1Layout::RAIL_LENGTH);
            assert!((3..=5).contains(&l.ledge_h), "seed {seed}: ledge {}", l.ledge_h);
            let (low, high) = (l.channel_speed_min, l.channel_speed_max);
            assert!(0.0 < low && low < high && high < LAUNCHER_HARD_LIMIT, "seed {seed}: {low} to {high}");
            for v in [low, high] {
                assert_eq!(v, (v * 10.0).round() / 10.0, "the rating plate reads to 0.1 m/s");
            }
            // The launch line, out to the furthest shot, fits on the flat ground the island keeps for it.
            assert!(l.channel_profile().heights.len() <= Ch1Layout::CHANNEL_RUN, "seed {seed}: {} m", l.channel_profile().heights.len());
            for stack in 0..=Ch1Layout::MAX_STACK {
                assert_ne!(l.channel_shot(stack, high).1.end, FlightEnd::LeftTerrain, "seed {seed}: {stack} blocks");
            }
            // The haul track: whole metres, the posts and the marker on the track, a speed the cart can make.
            assert!((1.2..=2.4).contains(&l.crawler_speed), "seed {seed}: crawler {}", l.crawler_speed);
            assert!((5..=10).contains(&l.head_start) && (22..=32).contains(&l.transfer_at), "seed {seed}");
            let (a, b) = l.posts();
            assert!(a > l.head_start && b < Ch1Layout::HAUL_LENGTH, "seed {seed}: posts at {a} and {b} m");
            assert!((10..=14).contains(&(b - a)), "seed {seed}");
            assert!(l.transfer_at + 4 <= Ch1Layout::HAUL_LENGTH);
            let v = l.chase_speed_needed();
            assert!(v < Ch1Layout::CART_TOP_SPEED * 0.75, "seed {seed}: {v} m/s");
            assert!(v > l.crawler_speed * 1.25, "seed {seed}: the cart must clearly outrun the crawler");
            // The crawler takes several seconds between the posts, so the stopwatch's rounding is small.
            assert!(l.crawler_time() >= 4.0, "seed {seed}: {} s", l.crawler_time());
        }
    }

    #[test]
    fn older_saves_keep_every_number_but_the_channels() {
        // Values from before the channel was reworked: (seed, g, rail, tower, mast, channel width, pad,
        // ridge distance, ridge height, rated speed of the tilting launcher).
        let before = [
            (0, 8.734, 17, 20, 30, 15, 46.5, 10, 10, 22.7),
            (7, 10.891, 9, 23, 34, 12, 53.5, 15, 14, 27.3),
            (42, 8.789, 11, 23, 32, 13, 54.5, 12, 11, 23.9),
            (999, 7.945, 10, 23, 34, 13, 41.5, 9, 9, 20.4),
        ];
        for (seed, g, rail, tower, mast, width, pad, ridge_x, ridge_h, v_max) in before {
            let l = Ch1Layout::generate(seed);
            assert_eq!(
                (l.g, l.rail_target, l.tower_h, l.mast_h, l.channel_width, l.pad_centre, l.ridge_x, l.ridge_h, l.v_max),
                (g, rail, tower, mast, width, pad, ridge_x, ridge_h, v_max),
                "seed {seed}"
            );
        }
    }

    #[test]
    fn the_band_admits_one_or_two_stacks_and_never_none() {
        for seed in 0..SEEDS {
            let l = Ch1Layout::generate(seed);
            let stacks = l.channel_stacks();
            let count = stacks.clone().count();
            assert!((1..=2).contains(&count), "seed {seed}: {count} stacks");
            assert!(*stacks.start() >= 1 && *stacks.end() <= 6, "seed {seed}: {stacks:?}");
            // No stack's speed sits near an end of the band, where a measured g could tip it over.
            for n in 0..=Ch1Layout::MAX_STACK {
                let v = l.channel_speed_needed(n);
                for end in [l.channel_speed_min, l.channel_speed_max] {
                    assert!(((v - end) / v).abs() >= Ch1Layout::BAND_MARGIN, "seed {seed}: {n} blocks need {v}, the band ends at {end}");
                }
            }
            let channel = Instance::new(PuzzleKind::Channel, &l);
            let attempt = |n: u32, v: f64| inputs(&[("stack", Quantity::metres(n as f64)), ("speed", Quantity::speed(v))]);
            // Every stack in the band, at its speed, lands; off by a tenth, it misses.
            for n in stacks.clone() {
                let v = l.channel_speed_needed(n);
                assert!(channel.evaluate(&attempt(n, v)).unwrap().pass, "seed {seed}: {n} blocks");
                for factor in [0.9, 1.1] {
                    match channel.evaluate(&attempt(n, v * factor)) {
                        Ok(outcome) => assert!(!outcome.pass, "seed {seed}: {n} blocks at x{factor} passed"),
                        Err(InputError::OutOfRange { .. }) => {}
                        Err(other) => panic!("seed {seed}: {other}"),
                    }
                }
            }
            // Without blocks, nothing the dial allows reaches the platform.
            for v in [l.channel_speed_min, l.channel_speed_max] {
                let outcome = channel.evaluate(&attempt(0, v)).unwrap();
                assert!(!outcome.pass, "seed {seed}: no blocks at {v} passed");
            }
        }
    }

    #[test]
    fn the_jack_frame_takes_whole_crates_and_the_dial_its_band() {
        let l = Ch1Layout::generate(2);
        let channel = Instance::new(PuzzleKind::Channel, &l);
        let v = l.channel_speed_min;
        for bad in [-1.0, 2.5, 9.0] {
            let err = channel.evaluate(&inputs(&[("stack", Quantity::metres(bad)), ("speed", Quantity::speed(v))])).unwrap_err();
            assert!(matches!(err, InputError::OutOfRange { .. }), "{bad}: {err}");
        }
        assert!(channel.evaluate(&inputs(&[("stack", Quantity::metres(0.0)), ("speed", Quantity::speed(v))])).is_ok(), "no blocks is a valid try");
        for speed in [l.channel_speed_min - 0.5, l.channel_speed_max + 0.5] {
            let err = channel.evaluate(&inputs(&[("stack", Quantity::metres(3.0)), ("speed", Quantity::speed(speed))])).unwrap_err();
            assert!(err.to_string().contains("dial only turns from"), "{err}");
        }
        assert_eq!(channel.inputs()[0].source, InputSource::Placed);
        assert_eq!(channel.inputs()[1].source, InputSource::Typed);
    }

    #[test]
    fn every_puzzle_is_solvable_for_every_seed() {
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            for kind in ALL_KINDS {
                let puzzle = Instance::new(kind, &layout);
                let outcome = puzzle.evaluate(&puzzle.solution().inputs()).unwrap();
                assert!(outcome.pass, "seed {seed} {kind:?}: {} — {}", outcome.headline, outcome.detail);
            }
        }
    }

    #[test]
    fn ten_percent_errors_fail() {
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            for kind in ALL_KINDS {
                let puzzle = Instance::new(kind, &layout);
                let key = puzzle.inputs().last().unwrap().key;
                for factor in [0.9, 1.1] {
                    let mut wrong = puzzle.solution().inputs();
                    wrong.get_mut(key).unwrap().value *= factor;
                    match puzzle.evaluate(&wrong) {
                        Ok(outcome) => assert!(!outcome.pass, "seed {seed} {kind:?} x{factor} passed"),
                        // Too fast for the launcher: refused, which is also not a pass.
                        Err(InputError::OutOfRange { .. }) => {}
                        Err(other) => panic!("seed {seed} {kind:?}: {other}"),
                    }
                }
            }
        }
    }

    #[test]
    fn the_players_own_measurement_of_g_is_good_enough() {
        // g worked out from the whole-metre tower height and the 0.01 s stopwatch must pass, and
        // must then be accurate enough to solve the channel.
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            let t = layout.tower_fall_time();
            let g_measured = 2.0 * layout.tower_h as f64 / (t * t);
            let gravity = Instance::new(PuzzleKind::Gravity, &layout);
            assert!(gravity.evaluate(&inputs(&[("g", Quantity::acceleration(g_measured))])).unwrap().pass);

            // The channel from the player's g, the ruler's readings and the rating plate: the lowest
            // release height the band allows, rounded up to whole blocks, then the speed for it.
            let d = layout.platform_centre;
            let lowest = g_measured * d * d / (2.0 * layout.channel_speed_max.powi(2)) - layout.ledge_h as f64;
            let stack = lowest.ceil();
            let speed = d / fall_time(layout.ledge_h as f64 + stack, g_measured);
            let channel = Instance::new(PuzzleKind::Channel, &layout);
            let attempt = inputs(&[("stack", Quantity::metres(stack)), ("speed", Quantity::speed(speed))]);
            assert!(channel.evaluate(&attempt).unwrap().pass, "seed {seed}");

            let (min, max) = layout.ridge_angle_window();
            let angle = (min + max) / 2.0;
            let v = (g_measured * layout.pad_centre / (2.0 * angle).sin()).sqrt();
            let ridge = Instance::new(PuzzleKind::Ridge, &layout);
            let attempt = inputs(&[("angle", Quantity::number(angle)), ("speed", Quantity::speed(v))]);
            assert!(ridge.evaluate(&attempt).unwrap().pass, "seed {seed}");
        }
    }

    #[test]
    fn the_light_gate_reads_the_fall_to_every_notch() {
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            let notches = layout.gate_notches();
            assert_eq!((*notches.start(), *notches.end()), (1, layout.tower_h), "from a metre below the release down to the plate");
            for drop in notches {
                let t = layout.gate_time(drop);
                assert!((t - fall_time(drop as f64, layout.g)).abs() <= 0.005 + 1e-12, "seed {seed}: {drop} m reads {t} s");
                assert_eq!(t, (t * 100.0).round() / 100.0, "the stopwatch reads to 0.01 s");
            }
            // At the plate the gate reads what the stopwatch read for the whole drop.
            assert_eq!(layout.gate_time(layout.tower_h), layout.tower_fall_time());
            // Doubling the drop does not double the time.
            let half = layout.tower_h / 2;
            assert!(layout.gate_time(2 * half) < 1.5 * layout.gate_time(half), "seed {seed}");
        }
    }

    #[test]
    fn g_from_a_good_enough_notch_passes() {
        // The rule the brief gives: a reading of a second or more gives g to 1%. Shorter drops are worth
        // less, and at the shortest the rounding alone can push g past the 2% a pass allows.
        let mut a_short_drop_failed = false;
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            let gravity = Instance::new(PuzzleKind::Gravity, &layout);
            let g_at = |drop: u32| 2.0 * drop as f64 / layout.gate_time(drop).powi(2);
            let mut good = 0;
            for drop in layout.gate_notches() {
                let g = g_at(drop);
                let passes = gravity.evaluate(&inputs(&[("g", Quantity::acceleration(g))])).unwrap().pass;
                if layout.gate_time(drop) >= 1.0 {
                    good += 1;
                    assert!(((g - layout.g) / layout.g).abs() <= 0.01, "seed {seed}: {drop} m gives g = {g}, true {}", layout.g);
                    assert!(passes, "seed {seed}: {drop} m");
                } else {
                    a_short_drop_failed |= !passes;
                }
            }
            assert!(good >= 5, "seed {seed}: only {good} notches time a second or more");
            // A straight line through the origin fitted to every pair (h against t^2) passes too.
            let (num, den) = layout.gate_notches().fold((0.0, 0.0), |(n, d), drop| {
                let t2 = layout.gate_time(drop).powi(2);
                (n + drop as f64 * t2, d + t2 * t2)
            });
            let g_fit = 2.0 * num / den;
            assert!(((g_fit - layout.g) / layout.g).abs() <= 0.01, "seed {seed}: the fit gives {g_fit}");
        }
        assert!(a_short_drop_failed, "a one-metre drop should sometimes be too short to pass");
    }

    #[test]
    fn the_cart_must_be_loaded_and_the_gate_clamped() {
        let layout = Ch1Layout::generate(6);
        let rail = Instance::new(PuzzleKind::Rail, &layout);
        let reason = rail.unready(0).expect("an empty cart is refused");
        assert!(reason.starts_with("Load the cart first"), "{reason}");
        assert_eq!(rail.unready(1), None);
        for kind in [PuzzleKind::Chase, PuzzleKind::Gravity, PuzzleKind::Channel, PuzzleKind::Ridge] {
            assert_eq!(Instance::new(kind, &layout).unready(0), None, "{kind:?}");
        }
        assert_eq!(rail.built(1).as_deref(), Some("the block is on the cart"));
        let gravity = Instance::new(PuzzleKind::Gravity, &layout);
        let reason = gravity.experiment_unready(0).expect("the tower times nothing without the gate");
        assert!(reason.starts_with("The experiment needs the light gate"), "{reason}");
        assert_eq!(gravity.experiment_unready(3), None);
        for kind in [PuzzleKind::Rail, PuzzleKind::Chase, PuzzleKind::Channel, PuzzleKind::Ridge] {
            assert_eq!(Instance::new(kind, &layout).experiment_unready(0), None, "{kind:?}");
        }
        assert_eq!(Instance::new(PuzzleKind::Gravity, &layout).built(7).as_deref(), Some("the light gate is clamped 7 m below the release point"));
        assert_eq!(Instance::new(PuzzleKind::Channel, &layout).built(1).as_deref(), Some("1 block in the jack frame"));
        assert_eq!(Instance::new(PuzzleKind::Ridge, &layout).built(0), None);
        assert_eq!(Instance::new(PuzzleKind::Chase, &layout).built(0), None);
    }

    #[test]
    fn forty_five_degrees_never_clears_the_ridge() {
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            let v = (layout.g * layout.pad_centre).sqrt(); // sin(90 deg) = 1
            let ridge = Instance::new(PuzzleKind::Ridge, &layout);
            let attempt = inputs(&[("angle", Quantity::degrees(45.0)), ("speed", Quantity::speed(v))]);
            let outcome = ridge.evaluate(&attempt).unwrap();
            assert!(!outcome.pass);
            assert_eq!(outcome.headline, "Struck the ridge", "seed {seed}");
        }
    }

    #[test]
    fn wrong_dimension_is_refused_with_an_explanation() {
        let puzzle = Instance::new(PuzzleKind::Ridge, &Ch1Layout::generate(1));
        let err = puzzle.evaluate(&inputs(&[("angle", Quantity::degrees(60.0)), ("speed", Quantity::seconds(3.0))])).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Launch speed needs to be a speed (m/s), but you entered a time (s)."
        );
        assert!(matches!(puzzle.evaluate(&Inputs::new()), Err(InputError::Missing { .. })));
    }

    #[test]
    fn ridge_refuses_overspeed_and_bare_degrees() {
        let layout = Ch1Layout::generate(3);
        let puzzle = Instance::new(PuzzleKind::Ridge, &layout);
        let too_fast = inputs(&[
            ("angle", Quantity::degrees(60.0)),
            ("speed", Quantity::speed(layout.v_max + 0.5)),
        ]);
        assert!(matches!(puzzle.evaluate(&too_fast), Err(InputError::OutOfRange { .. })));
        // "58" typed without "deg" is 58 radians.
        let bare = inputs(&[("angle", Quantity::number(58.0)), ("speed", Quantity::speed(10.0))]);
        let err = puzzle.evaluate(&bare).unwrap_err().to_string();
        assert!(err.contains("Did you mean 58.00 deg?"), "{err}");
    }

    #[test]
    fn failures_describe_what_happened_without_the_answer() {
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            let channel = Instance::new(PuzzleKind::Channel, &layout);
            let try_ = |n: u32, v: f64| channel.evaluate(&inputs(&[("stack", Quantity::metres(n as f64)), ("speed", Quantity::speed(v))])).unwrap();
            let low = try_(0, layout.channel_speed_max);
            assert!(!low.pass);
            assert!(low.headline == "Fell short" || low.detail.contains("short of the platform centre"), "seed {seed}: {}", low.detail);
            let high = try_(Ch1Layout::MAX_STACK, layout.channel_speed_min);
            assert!(high.detail.contains("beyond the platform centre"), "seed {seed}: {}", high.detail);
            let right = layout.channel_speed_needed(*layout.channel_stacks().start());
            for outcome in [low, high] {
                assert!(!outcome.detail.contains(&sig(right, 3)), "{}", outcome.detail);
            }
        }
    }

    /// Tests a player needs who guesses instead of calculating, but uses every miss well: they halve
    /// the dial's band on whichever side the miss points to, and move a block when the band runs out.
    fn tests_by_guessing(layout: &Ch1Layout) -> u32 {
        let channel = Instance::new(PuzzleKind::Channel, layout);
        let (mut stack, mut low, mut high) = (Ch1Layout::MAX_STACK / 2, layout.channel_speed_min, layout.channel_speed_max);
        for tests in 1..=60 {
            let v = (low + high) / 2.0;
            let outcome = channel.evaluate(&inputs(&[("stack", Quantity::metres(stack as f64)), ("speed", Quantity::speed(v))])).unwrap();
            if outcome.pass {
                return tests;
            }
            let short = outcome.headline == "Fell short" || outcome.detail.contains("short of");
            if short { low = v } else { high = v }
            if (high - low) / high < 0.003 {
                stack = if short { stack + 1 } else { stack - 1 }.min(Ch1Layout::MAX_STACK);
                (low, high) = (layout.channel_speed_min, layout.channel_speed_max);
            }
        }
        60
    }

    /// For comparison, the same player at the old canyon (speed typed, 3 m platform) needed 3.9 tests on
    /// average and was done within three 31% of the time; at the first block canyon (fixed speed, blocks
    /// only) 2.8 and 75%.
    #[test]
    fn guessing_takes_more_tests_than_a_station_has_charges() {
        let tests: Vec<u32> = (0..SEEDS).map(|seed| tests_by_guessing(&Ch1Layout::generate(seed))).collect();
        let mean = tests.iter().sum::<u32>() as f64 / tests.len() as f64;
        let lucky = tests.iter().filter(|t| **t <= 3).count() as f64 / tests.len() as f64;
        println!("guessing: {mean:.2} tests on average, {:.0}% within three", lucky * 100.0);
        assert!(mean >= 6.0, "a guesser needs only {mean:.2} tests on average");
        assert!(lucky <= 0.35, "a guesser lands within three tests {:.0}% of the time", lucky * 100.0);
    }

    #[test]
    fn the_players_own_timing_of_the_crawler_is_good_enough() {
        // u from the whole-metre post gap and the 0.01 s stopwatch, then v = u M / (M - D), passes with
        // room to spare: the cart reaches the crawler well inside the marker.
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            let u = layout.post_gap as f64 / layout.crawler_time();
            let (d, m) = (layout.head_start as f64, layout.transfer_at as f64);
            let v = u * m / (m - d);
            let chase = Instance::new(PuzzleKind::Chase, &layout);
            let outcome = chase.evaluate(&inputs(&[("speed", Quantity::speed(v))])).unwrap();
            assert!(outcome.pass, "seed {seed}: {}", outcome.detail);
            let run = layout.chase(v);
            assert!((run.cart - m).abs() < 0.3, "seed {seed}: reached it {} m from the marker", run.cart - m);
        }
    }

    #[test]
    fn chase_misses_point_the_right_way_without_the_answer() {
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            let chase = Instance::new(PuzzleKind::Chase, &layout);
            let try_ = |v: f64| chase.evaluate(&inputs(&[("speed", Quantity::speed(v))])).unwrap();
            let right = layout.chase_speed_needed();
            let fast = try_(right * 1.1);
            assert_eq!(fast.headline, "Caught it too soon", "seed {seed}");
            assert!(fast.detail.contains("before the marker centre"), "seed {seed}: {}", fast.detail);
            let slow = try_(right * 0.95);
            assert!(!slow.pass);
            assert!(slow.detail.contains("past the marker centre") || slow.headline == "Never caught it", "seed {seed}: {}", slow.detail);
            // No faster than the crawler: it is never caught, and reaches the buffer.
            let crawl = try_(layout.crawler_speed);
            assert_eq!(crawl.headline, "Never caught it", "seed {seed}");
            assert!(crawl.detail.contains("ahead of the cart"), "{}", crawl.detail);
            for outcome in [fast, slow, crawl] {
                assert!(!outcome.detail.contains("m/s"), "seed {seed}: no speed is reported: {}", outcome.detail);
                let Playback::Chase { t, cart_speed, crawler_speed, head_start } = outcome.playback else { panic!("a chase") };
                // The playback ends where the verdict says: together, or the crawler at the buffer.
                let gap = head_start + crawler_speed * t - cart_speed * t;
                assert!(gap >= -1e-9, "seed {seed}: the cart never passes the crawler");
            }
        }
    }

    #[test]
    fn the_haul_cart_has_a_top_speed() {
        let chase = Instance::new(PuzzleKind::Chase, &Ch1Layout::generate(5));
        let err = chase.evaluate(&inputs(&[("speed", Quantity::speed(Ch1Layout::CART_TOP_SPEED + 0.5))])).unwrap_err();
        let rating = Quantity::speed(Ch1Layout::CART_TOP_SPEED).display();
        assert!(err.to_string().contains(&format!("the haul cart is rated to {rating} and won't run any faster")), "{err}");
        assert!(chase.evaluate(&inputs(&[("speed", Quantity::speed(0.0))])).is_err());
        let err = chase.evaluate(&inputs(&[("speed", Quantity::seconds(5.0))])).unwrap_err();
        assert_eq!(err.to_string(), "Cart speed needs to be a speed (m/s), but you entered a time (s).");
    }

    /// Tests a player needs who guesses the cart's speed instead of working it out, but uses every miss
    /// well: they halve what is left of the cart's range on the side the miss points to.
    fn chase_tests_by_guessing(layout: &Ch1Layout) -> u32 {
        let chase = Instance::new(PuzzleKind::Chase, layout);
        let (mut low, mut high) = (0.0, Ch1Layout::CART_TOP_SPEED);
        for tests in 1..=60 {
            let v = (low + high) / 2.0;
            let outcome = chase.evaluate(&inputs(&[("speed", Quantity::speed(v))])).unwrap();
            if outcome.pass {
                return tests;
            }
            if outcome.headline == "Caught it too soon" { high = v } else { low = v }
        }
        60
    }

    #[test]
    fn guessing_the_cart_speed_takes_more_tests_than_a_station_has_charges() {
        let tests: Vec<u32> = (0..SEEDS).map(|seed| chase_tests_by_guessing(&Ch1Layout::generate(seed))).collect();
        let mean = tests.iter().sum::<u32>() as f64 / tests.len() as f64;
        let lucky = tests.iter().filter(|t| **t <= 3).count() as f64 / tests.len() as f64;
        println!("guessing the cart speed: {mean:.2} tests on average, {:.0}% within three", lucky * 100.0);
        assert!(mean >= 6.0, "a guesser needs only {mean:.2} tests on average");
        assert!(lucky <= 0.15, "a guesser catches the crawler within three tests {:.0}% of the time", lucky * 100.0);
    }

    #[test]
    fn the_tutorial_needs_no_calculation() {
        // The ruler's reading, entered as it is, passes: by number with its unit or by notebook name.
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            let rail = Instance::new(PuzzleKind::Rail, &layout);
            let reading = Quantity::metres(layout.rail_target as f64);
            let outcome = rail.evaluate(&inputs(&[("distance", reading)])).unwrap();
            assert!(outcome.pass, "seed {seed}: {}", outcome.detail);
            assert_eq!(outcome.playback, Playback::Slide { distance: layout.rail_target as f64 });
        }
    }

    #[test]
    fn the_tutorial_explains_misses_and_refusals() {
        let layout = Ch1Layout::generate(4);
        let rail = Instance::new(PuzzleKind::Rail, &layout);
        let d = layout.rail_target as f64;

        let short = rail.evaluate(&inputs(&[("distance", Quantity::metres(d - 3.0))])).unwrap();
        assert_eq!(short.headline, "Stopped short");
        assert!(short.detail.ends_with("stopped 3.00 m short of the marker centre."), "{}", short.detail);
        let long = rail.evaluate(&inputs(&[("distance", Quantity::metres(d + 2.0))])).unwrap();
        assert_eq!(long.headline, "Overshot");
        assert!(long.detail.contains("beyond the marker"), "{}", long.detail);
        // At the edge of the marker block still counts.
        assert!(rail.evaluate(&inputs(&[("distance", Quantity::metres(d + 0.5))])).unwrap().pass);

        // A bare number is refused, for free, with the reason.
        let err = rail.evaluate(&inputs(&[("distance", Quantity::number(d))])).unwrap_err();
        assert_eq!(err.to_string(), "Run distance needs to be a length (m), but you entered a pure number.");
        let err = rail.evaluate(&inputs(&[("distance", Quantity::metres(30.0))])).unwrap_err();
        assert!(err.to_string().contains("the rail is 24 m long"), "{err}");
        assert!(rail.evaluate(&inputs(&[("distance", Quantity::metres(-4.0))])).is_err());
    }

    #[test]
    fn gravity_feedback_points_the_right_way() {
        let layout = Ch1Layout::generate(5);
        let puzzle = Instance::new(PuzzleKind::Gravity, &layout);
        let low = puzzle.evaluate(&inputs(&[("g", Quantity::acceleration(layout.g * 0.8))])).unwrap();
        assert!(low.detail.contains("stronger than your value"));
        let high = puzzle.evaluate(&inputs(&[("g", Quantity::acceleration(layout.g * 1.2))])).unwrap();
        assert!(high.detail.contains("weaker than your value"));
    }
}
