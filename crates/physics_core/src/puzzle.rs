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

    /// Puzzle 1: height of the drop tower's release point above the ground.
    pub tower_h: u32,
    /// Puzzle 1: height of the survey mast's release point, used for the test drop.
    pub mast_h: u32,

    /// Puzzle 2: height of the ledge above the far plateau.
    pub ledge_h: u32,
    /// Puzzle 2: width of the canyon, from the ledge edge to the far rim.
    pub canyon_width: u32,
    /// Puzzle 2: depth of the canyon floor below the far plateau.
    pub canyon_depth: u32,
    /// Puzzle 2: distance from the ledge edge to the centre of the landing platform.
    pub platform_centre: f64,
    pub platform_half: f64,

    /// Puzzle 3: distance from the launcher to the centre of the shipyard pad (level ground).
    pub pad_centre: f64,
    pub pad_half: f64,
    /// Puzzle 3: distance from the launcher to the near face of the ridge (one block thick).
    pub ridge_x: u32,
    /// Puzzle 3: height of the ridge above the launcher.
    pub ridge_h: u32,
    /// Puzzle 3: the launcher's rated maximum speed, m/s.
    pub v_max: f64,
}

impl Ch1Layout {
    pub const G_RANGE: (f64, f64) = (6.5, 13.5);
    /// Values this close to Earth's are skipped, so the familiar number never works.
    pub const G_EXCLUDED: (f64, f64) = (9.3, 10.3);

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

        let ledge_h = rng.int(6, 12);
        let canyon_width = rng.int(12, 20);
        let platform_centre = (canyon_width + rng.int(4, 8)) as f64 + 0.5;

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

        Ch1Layout {
            g,
            tower_h,
            mast_h,
            ledge_h,
            canyon_width,
            canyon_depth: 10,
            platform_centre,
            platform_half: 1.5,
            pad_centre,
            pad_half: 2.5,
            ridge_x,
            ridge_h,
            v_max,
        }
    }

    /// Fall time from the drop tower as the stopwatch shows it (hundredths of a second).
    pub fn tower_fall_time(&self) -> f64 {
        (fall_time(self.tower_h as f64, self.g) * 100.0).round() / 100.0
    }

    /// Launch angles (radians) between which a shot aimed at the pad centre clears the ridge and
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PuzzleKind {
    Gravity,
    Canyon,
    Ridge,
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
    #[error("{label} has not been entered.")]
    Missing { label: String },
    #[error("{label} must be {expected}, but what you entered is {got}.")]
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
        };
        match self.kind {
            PuzzleKind::Gravity => vec![InputSpec {
                key: "g",
                label: "Gravitational acceleration",
                dim: Dim::ACCELERATION,
                unit: "m/s^2",
                example: "2*h1/t1^2",
            }],
            PuzzleKind::Canyon => vec![speed],
            PuzzleKind::Ridge => vec![
                InputSpec {
                    key: "angle",
                    label: "Launch angle",
                    dim: Dim::NONE,
                    unit: "deg",
                    example: "58 deg",
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
            PuzzleKind::Gravity => vec![
                given("height of the drop tower", Quantity::metres(l.tower_h as f64)),
                given("fall time from the drop tower", Quantity::seconds(l.tower_fall_time())),
                given("height of the survey mast", Quantity::metres(l.mast_h as f64)),
            ],
            PuzzleKind::Canyon => vec![
                given("drop from the ledge to the platform", Quantity::metres(l.ledge_h as f64)),
                given("horizontal distance to the platform centre", Quantity::metres(l.platform_centre)),
                given("length of the platform", Quantity::metres(2.0 * l.platform_half)),
            ],
            PuzzleKind::Ridge => vec![
                given("distance to the pad centre", Quantity::metres(l.pad_centre)),
                given("length of the pad", Quantity::metres(2.0 * l.pad_half)),
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
            PuzzleKind::Gravity => None,
            PuzzleKind::Canyon => {
                let length = (l.platform_centre + l.platform_half) as usize + 40;
                let mut p = Profile::flat(0.0, length, 0.0);
                p.set(0.0, l.canyon_width as f64, -(l.canyon_depth as f64));
                Some(p)
            }
            PuzzleKind::Ridge => {
                let length = (l.pad_centre + l.pad_half) as usize + 40;
                let mut p = Profile::flat(0.0, length, 0.0);
                p.set(l.ridge_x as f64, l.ridge_x as f64 + 1.0, l.ridge_h as f64);
                Some(p)
            }
        }
    }

    /// The projectile a set of inputs produces, if the puzzle launches one.
    fn projectile(&self, speed: f64, angle: f64) -> Projectile {
        let y0 = match self.kind {
            PuzzleKind::Canyon => self.layout.ledge_h as f64,
            _ => 0.0,
        };
        Projectile::from_angle(0.0, y0, speed, angle, self.layout.g)
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
                "g" if q.value <= 0.0 => {
                    return out_of_range("gravity pulls downward; enter its size as a positive value.".into());
                }
                "speed" => {
                    let limit = match self.kind {
                        PuzzleKind::Ridge => self.layout.v_max,
                        _ => LAUNCHER_HARD_LIMIT,
                    };
                    if q.value <= 0.0 {
                        return out_of_range("the speed must be greater than zero.".into());
                    }
                    if q.value > limit {
                        return out_of_range(format!(
                            "the launcher is rated to {} and will not fire faster.",
                            Quantity::speed(limit).display()
                        ));
                    }
                }
                "angle" => {
                    let deg = q.value.to_degrees();
                    if q.value > std::f64::consts::FRAC_PI_2 * 1.0001 && q.value <= 90.0 {
                        return out_of_range(format!(
                            "{} radians is not a launch angle. Did you mean {} deg?",
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
                            "The mass fell from the {} m mast in {} s; your value predicted {} s.",
                            l.mast_h,
                            sig(actual_time, 3),
                            sig(predicted_time, 3)
                        ),
                    )
                } else {
                    let (speed_word, strength_word) =
                        if actual_time < predicted_time { ("sooner", "stronger") } else { ("later", "weaker") };
                    (
                        "Prediction does not match".to_string(),
                        format!(
                            "The mass fell from the {} m mast in {} s; your value predicted {} s. It landed \
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
            PuzzleKind::Canyon => {
                let projectile = self.projectile(value("speed"), 0.0);
                let flight = fly(&projectile, &self.profile().expect("canyon has terrain"));
                let miss = flight.x - l.platform_centre;
                let on_plateau = flight.end == FlightEnd::Landed && flight.surface == 0.0;
                let pass = on_plateau && miss.abs() <= l.platform_half;
                let (headline, detail) = if pass {
                    (
                        "On the platform".to_string(),
                        format!(
                            "The crate was in the air for {} s and landed {} m from the platform centre.",
                            sig(flight.t, 3),
                            sig(miss.abs(), 2)
                        ),
                    )
                } else if on_plateau {
                    let side = if miss < 0.0 { "short of" } else { "beyond" };
                    (
                        "Missed the platform".to_string(),
                        format!(
                            "The crate was in the air for {} s and landed {} m {side} the platform centre.",
                            sig(flight.t, 3),
                            sig(miss.abs(), 3)
                        ),
                    )
                } else if flight.end == FlightEnd::StruckFace {
                    (
                        "Fell short".to_string(),
                        format!(
                            "The crate dropped into the canyon and struck the far wall {} m below the rim.",
                            sig(-flight.y, 3)
                        ),
                    )
                } else {
                    (
                        "Fell short".to_string(),
                        format!(
                            "The crate dropped into the canyon and hit the floor {} m out from the ledge.",
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
                        "On the pad".to_string(),
                        format!(
                            "The beam cleared the ridge by {} m and landed {} m from the pad centre.",
                            sig(projectile.height_at_x(ridge_x).unwrap_or(0.0) - ridge_h, 2),
                            sig(miss.abs(), 2)
                        ),
                    )
                } else if flight.end == FlightEnd::StruckFace {
                    (
                        "Struck the ridge".to_string(),
                        format!("The beam hit the ridge {} m below its top.", sig(ridge_h - flight.y, 3)),
                    )
                } else if flight.end == FlightEnd::Landed && flight.surface > 0.0 {
                    ("Landed on the ridge".to_string(), "The beam came down on top of the ridge.".to_string())
                } else if flight.x < ridge_x {
                    (
                        "Fell short".to_string(),
                        format!("The beam came down {} m before the ridge.", sig(ridge_x - flight.x, 3)),
                    )
                } else {
                    let side = if miss < 0.0 { "short of" } else { "beyond" };
                    (
                        "Missed the pad".to_string(),
                        format!(
                            "The beam cleared the ridge and landed {} m {side} the pad centre.",
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
            PuzzleKind::Gravity => {
                let (h, t) = (l.tower_h as f64, l.tower_fall_time());
                Solution {
                    steps: vec![
                        "A mass released from rest falls a height h in a time t with h = (1/2) g t^2.".into(),
                        "Rearranged: g = 2 h / t^2.".into(),
                        format!(
                            "With the tower height h = {h} m and the timed fall t = {t} s: g = 2 x {h} / {t}^2 = {} m/s^2.",
                            sig(2.0 * h / (t * t), 4)
                        ),
                    ],
                    answers: vec![answer("g", "Gravitational acceleration", Quantity::acceleration(l.g), "m/s^2")],
                    note: format!(
                        "The true value is {} m/s^2. The stopwatch reads to 0.01 s, so the player's result differs \
                         slightly; anything within {}% passes.",
                        l.g,
                        GRAVITY_TOLERANCE * 100.0
                    ),
                }
            }
            PuzzleKind::Canyon => {
                let (h, d) = (l.ledge_h as f64, l.platform_centre);
                let t = fall_time(h, l.g);
                let v = d / t;
                Solution {
                    steps: vec![
                        "Horizontal and vertical motion are independent. Launched horizontally, the crate starts \
                         with no vertical speed."
                            .into(),
                        format!(
                            "Vertical: it falls h = {h} m, so h = (1/2) g t^2 and t = sqrt(2 h / g) = {} s.",
                            sig(t, 4)
                        ),
                        format!(
                            "Horizontal: constant speed, so d = v t. To cover d = {d} m: v = d / t = {} m/s.",
                            sig(v, 4)
                        ),
                    ],
                    answers: vec![answer("speed", "Launch speed", Quantity::speed(v), "m/s")],
                    note: format!(
                        "The platform is {} m long, so landing within {} m of the centre passes.",
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
                            "Launcher and pad are level, so the range is R = v^2 sin(2 theta) / g and must equal \
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
            assert!(l.platform_centre - l.platform_half >= l.canyon_width as f64 + 2.0);
            assert!((l.ridge_x as f64) < l.pad_centre / 2.0);
            let (min, max) = l.ridge_angle_window();
            assert!(min.to_degrees() > 47.0, "seed {seed}: 45 degrees must not work ({})", min.to_degrees());
            assert!((max - min).to_degrees() >= 8.0, "seed {seed}: window {}", (max - min).to_degrees());
            assert!(l.v_max < LAUNCHER_HARD_LIMIT);
        }
    }

    #[test]
    fn every_puzzle_is_solvable_for_every_seed() {
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            for kind in [PuzzleKind::Gravity, PuzzleKind::Canyon, PuzzleKind::Ridge] {
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
            for kind in [PuzzleKind::Gravity, PuzzleKind::Canyon, PuzzleKind::Ridge] {
                let puzzle = Instance::new(kind, &layout);
                let key = if kind == PuzzleKind::Gravity { "g" } else { "speed" };
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
        // must then be accurate enough to solve the canyon.
        for seed in 0..SEEDS {
            let layout = Ch1Layout::generate(seed);
            let t = layout.tower_fall_time();
            let g_measured = 2.0 * layout.tower_h as f64 / (t * t);
            let gravity = Instance::new(PuzzleKind::Gravity, &layout);
            assert!(gravity.evaluate(&inputs(&[("g", Quantity::acceleration(g_measured))])).unwrap().pass);

            let v = layout.platform_centre / fall_time(layout.ledge_h as f64, g_measured);
            let canyon = Instance::new(PuzzleKind::Canyon, &layout);
            assert!(canyon.evaluate(&inputs(&[("speed", Quantity::speed(v))])).unwrap().pass, "seed {seed}");

            let (min, max) = layout.ridge_angle_window();
            let angle = (min + max) / 2.0;
            let v = (g_measured * layout.pad_centre / (2.0 * angle).sin()).sqrt();
            let ridge = Instance::new(PuzzleKind::Ridge, &layout);
            let attempt = inputs(&[("angle", Quantity::number(angle)), ("speed", Quantity::speed(v))]);
            assert!(ridge.evaluate(&attempt).unwrap().pass, "seed {seed}");
        }
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
        let puzzle = Instance::new(PuzzleKind::Canyon, &Ch1Layout::generate(1));
        let err = puzzle.evaluate(&inputs(&[("speed", Quantity::seconds(3.0))])).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Launch speed must be a speed (m/s), but what you entered is a time (s)."
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
        let layout = Ch1Layout::generate(11);
        let canyon = Instance::new(PuzzleKind::Canyon, &layout);
        let right = canyon.solution().answers[0].quantity.value;

        let slow = canyon.evaluate(&inputs(&[("speed", Quantity::speed(right * 0.3))])).unwrap();
        assert_eq!(slow.headline, "Fell short");
        let fast = canyon.evaluate(&inputs(&[("speed", Quantity::speed(right * 1.3))])).unwrap();
        assert!(fast.detail.contains("beyond the platform centre"), "{}", fast.detail);
        for outcome in [slow, fast] {
            assert!(!outcome.detail.contains(&sig(right, 3)), "{}", outcome.detail);
        }
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
