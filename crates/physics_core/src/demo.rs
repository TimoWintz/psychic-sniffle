//! Demonstrations: the free "try it first" experiment at each physics station.
//!
//! A demonstration shows the principle a puzzle rests on, in the world, before any calculation. Like a
//! test, what happens is decided here, in closed form; the game only plays it back. Unlike a test it
//! costs nothing, judges nothing, and is built so that it gives nothing away: the canyon crates fall
//! further than the ledge's drop to the platform, and the ridge slugs land well short of the ridge.
//!
//! The question, the choices and what the player should notice are words, kept with the chapter in
//! `assets/curriculum`.

use crate::ballistics::{Flight, Projectile, fall_time, fly};
use crate::puzzle::{Instance, PuzzleKind};

/// Number of equal intervals the strobe divides the tower drop into.
pub const STROBE_INTERVALS: u32 = 7;
/// Where the canyon's test crates start: this far out over the lip, clear of the wall.
pub const CANYON_DEMO_LIP: f64 = 0.5;
/// The fired test crate lands this far across the canyon floor, as a fraction of the canyon's width.
pub const CANYON_DEMO_REACH: f64 = 0.5;
/// The two launch angles of the ridge demonstration, in degrees. Equally far either side of 45.
pub const RIDGE_DEMO_ANGLES: [f64; 2] = [30.0, 60.0];
/// The ridge's test slugs land this far out, as a fraction of the distance to the ridge.
pub const RIDGE_DEMO_REACH: f64 = 0.6;

/// One body's exact path and how it ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub projectile: Projectile,
    pub flight: Flight,
}

impl Shot {
    fn new(projectile: Projectile, flight: Flight) -> Self {
        Shot { projectile, flight }
    }
}

/// What the world shows for a demonstration.
#[derive(Clone, Debug, PartialEq)]
pub enum DemoPlayback {
    /// The test mass falls `height` from the drop tower; a flash marks it every `interval` seconds.
    Strobe { height: f64, g: f64, interval: f64, fall_time: f64 },
    /// Two test crates leave the ledge together: one dropped, one fired sideways.
    DropAndFire { dropped: Shot, fired: Shot },
    /// Two test slugs at the same speed and complementary angles.
    TwoAngles { flat: Shot, steep: Shot },
}

impl DemoPlayback {
    /// Moments at which the strobe flashes, from the release to the landing. Empty for the others.
    pub fn flash_times(&self) -> Vec<f64> {
        match self {
            DemoPlayback::Strobe { interval, fall_time, .. } => {
                (0..).map(|k| k as f64 * interval).take_while(|t| *t <= fall_time + 1e-9).collect()
            }
            _ => Vec::new(),
        }
    }

    /// Time from the start until everything has come down.
    pub fn duration(&self) -> f64 {
        match self {
            DemoPlayback::Strobe { fall_time, .. } => *fall_time,
            DemoPlayback::DropAndFire { dropped, fired } => dropped.flight.t.max(fired.flight.t),
            DemoPlayback::TwoAngles { flat, steep } => flat.flight.t.max(steep.flight.t),
        }
    }
}

impl Instance {
    /// The station's demonstration, if it has one. The tutorial needs none.
    pub fn demo(&self) -> Option<DemoPlayback> {
        let l = &self.layout;
        match self.kind {
            PuzzleKind::Rail => None,
            PuzzleKind::Gravity => {
                let height = l.tower_h as f64;
                let fall_time = fall_time(height, l.g);
                Some(DemoPlayback::Strobe { height, g: l.g, interval: fall_time / STROBE_INTERVALS as f64, fall_time })
            }
            PuzzleKind::Canyon => {
                let profile = self.profile().expect("the canyon has terrain");
                let y0 = l.ledge_h as f64;
                let floor_drop = y0 + l.canyon_depth as f64;
                let reach = CANYON_DEMO_REACH * l.canyon_width as f64;
                let speed = (reach - CANYON_DEMO_LIP) / fall_time(floor_drop, l.g);
                // "Dropped" has to move forward for the terrain walk; a millimetre an hour will do.
                let dropped = Projectile { x0: CANYON_DEMO_LIP, y0, vx: 1e-6, vy: 0.0, g: l.g };
                let fired = Projectile { x0: CANYON_DEMO_LIP, y0, vx: speed, vy: 0.0, g: l.g };
                Some(DemoPlayback::DropAndFire {
                    dropped: Shot::new(dropped, fly(&dropped, &profile)),
                    fired: Shot::new(fired, fly(&fired, &profile)),
                })
            }
            PuzzleKind::Ridge => {
                let profile = self.profile().expect("the ridge has terrain");
                let reach = RIDGE_DEMO_REACH * l.ridge_x as f64;
                let [flat_deg, steep_deg] = RIDGE_DEMO_ANGLES;
                // R = v^2 sin(2 theta) / g, the same for both angles.
                let speed = (reach * l.g / (2.0 * flat_deg.to_radians()).sin()).sqrt();
                let shot = |deg: f64| {
                    let p = Projectile::from_angle(0.0, 0.0, speed, deg.to_radians(), l.g);
                    Shot::new(p, fly(&p, &profile))
                };
                Some(DemoPlayback::TwoAngles { flat: shot(flat_deg), steep: shot(steep_deg) })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ballistics::FlightEnd;
    use crate::puzzle::Ch1Layout;

    const SEEDS: u64 = 1000;

    #[test]
    fn the_tutorial_has_no_demonstration_and_the_others_do() {
        let layout = Ch1Layout::generate(3);
        assert!(Instance::new(PuzzleKind::Rail, &layout).demo().is_none());
        for kind in [PuzzleKind::Gravity, PuzzleKind::Canyon, PuzzleKind::Ridge] {
            assert!(Instance::new(kind, &layout).demo().is_some(), "{kind:?}");
        }
    }

    #[test]
    fn the_strobe_marks_equal_moments_and_growing_gaps() {
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            let demo = Instance::new(PuzzleKind::Gravity, &layout).demo().unwrap();
            let DemoPlayback::Strobe { g, height, .. } = demo else { panic!("gravity is a strobe") };
            let times = demo.flash_times();
            assert!((6..=9).contains(&times.len()), "seed {seed}: {} flashes", times.len());
            let fallen: Vec<f64> = times.iter().map(|t| 0.5 * g * t * t).collect();
            assert!(fallen.last().unwrap() <= &(height + 1e-6));
            let gaps: Vec<f64> = fallen.windows(2).map(|w| w[1] - w[0]).collect();
            assert!(gaps.windows(2).all(|w| w[1] > w[0]), "seed {seed}: gaps must grow");
        }
    }

    #[test]
    fn the_dropped_and_the_fired_crate_reach_the_floor_together() {
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            let Some(DemoPlayback::DropAndFire { dropped, fired }) = Instance::new(PuzzleKind::Canyon, &layout).demo() else {
                panic!("the canyon drops and fires")
            };
            let floor = -(layout.canyon_depth as f64);
            for shot in [dropped, fired] {
                assert_eq!(shot.flight.end, FlightEnd::Landed, "seed {seed}");
                assert_eq!(shot.flight.surface, floor, "seed {seed}: must land on the canyon floor");
            }
            assert!((dropped.flight.t - fired.flight.t).abs() < 1e-9, "seed {seed}");
            assert!(fired.flight.x > dropped.flight.x + 3.0, "seed {seed}: the fired crate must visibly travel");
            assert!(fired.flight.x < layout.canyon_width as f64 - 1.0, "seed {seed}: it must stay inside the canyon");
            // It falls further than the ledge's drop to the platform, so its time is not the puzzle's.
            let to_platform = fall_time(layout.ledge_h as f64, layout.g);
            assert!(fired.flight.t > to_platform * 1.1, "seed {seed}");
        }
    }

    #[test]
    fn complementary_angles_land_together_short_of_the_ridge() {
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            let Some(DemoPlayback::TwoAngles { flat, steep }) = Instance::new(PuzzleKind::Ridge, &layout).demo() else {
                panic!("the ridge fires two angles")
            };
            for shot in [flat, steep] {
                assert_eq!(shot.flight.end, FlightEnd::Landed, "seed {seed}");
                assert_eq!(shot.flight.surface, 0.0, "seed {seed}");
                assert!(shot.flight.x < layout.ridge_x as f64 - 1.0, "seed {seed}: must land before the ridge");
                assert!(shot.flight.x > 3.0, "seed {seed}: must land far enough out to see");
            }
            assert!((flat.flight.x - steep.flight.x).abs() < 1e-6, "seed {seed}");
            assert!(flat.flight.t < steep.flight.t, "seed {seed}: the flat shot comes down first");
            assert!(steep.projectile.apex().2 > flat.projectile.apex().2 * 2.0, "seed {seed}");
        }
    }
}
