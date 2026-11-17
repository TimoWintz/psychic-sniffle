//! The intro's timeline: where the camera and the three parts of the ship are at any moment of the
//! loop, what the screen shows, and when things happen. Plain data and pure functions, so the whole
//! loop can be tested without the engine (see [`crate::intro`] for the side that draws it).
//!
//! The intro goes back through the levels, last to first, seen from above:
//!
//! - **III, orbit** (placeholder): the ship comes in too fast and breaks up; its drive section is
//!   left drifting.
//! - **II, the mainland across the strait** (placeholder): the main hull comes down.
//! - **I, the island** (chapter 1.1): the forward section crosses the strait and comes down at the
//!   wreck, and the castaway climbs out.
//!
//! The section dives steeply into the wreck rather than skidding in: the wreck's walls and the
//! tutorial's camp beside it leave no clear run to skid along. The model gives way to the voxel wreck
//! under the dust of the impact.

use crate::island::SEA_LEVEL;
use crate::voxel::{VoxelWorld, smoothstep};
use bevy::prelude::*;
use bevy_egui::egui::{self, Align2, Color32, FontId, pos2, vec2};

/// A colour with an alpha, from sRGB components in `[0, 1]`.
fn rgba(c: [f32; 3], a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Cover the screen in a colour, `fade` of the way to opaque.
pub fn paint_veil(painter: &egui::Painter, screen: egui::Rect, fade: f32, colour: [f32; 3]) {
    if fade > 0.0 {
        painter.rect_filled(screen, 0.0, rgba(colour, fade));
    }
}

/// Letterbox bars, `bars` of their full height, with a caption in the lower one: a small gold
/// heading over a line, both at `alpha`. Returns whether any bar shows.
pub fn paint_letterbox(painter: &egui::Painter, screen: egui::Rect, bars: f32, heading: Option<&str>, line: Option<&str>, alpha: f32) -> bool {
    let bar = screen.height() * 0.11 * bars;
    if bar <= 0.5 {
        return false;
    }
    painter.rect_filled(egui::Rect::from_min_size(screen.min, vec2(screen.width(), bar)), 0.0, Color32::BLACK);
    let bottom = egui::Rect::from_min_max(pos2(screen.left(), screen.bottom() - bar), screen.max);
    painter.rect_filled(bottom, 0.0, Color32::BLACK);
    if let Some(heading) = heading {
        let gold = Color32::from_rgba_unmultiplied(230, 196, 120, (alpha.clamp(0.0, 1.0) * 255.0) as u8);
        painter.text(pos2(screen.center().x, bottom.top() + bar * 0.32), Align2::CENTER_CENTER, heading, FontId::proportional(13.0), gold);
    }
    if let Some(line) = line {
        let white = Color32::from_rgba_unmultiplied(235, 238, 242, (alpha.clamp(0.0, 1.0) * 255.0) as u8);
        painter.text(pos2(screen.center().x, bottom.top() + bar * 0.62), Align2::CENTER_CENTER, line, FontId::proportional(17.0), white);
    }
    true
}

/// Length of one pass of the loop, in seconds.
pub const LOOP: f32 = 48.0;
/// How long the hand-over from the intro to the player's own view takes.
pub const GLIDE: f32 = 2.2;

// When things happen, in seconds from the start of the loop.
const BREAKUP: f32 = 5.0;
/// The falling hull starts to burn.
const ENTRY: f32 = 9.5;
const ORBIT_END: f32 = 13.0;
const HULL_IMPACT: f32 = 19.0;
/// The camera turns from the mainland to follow the section.
const CHASE: (f32, f32) = (19.5, 24.0);
/// The section starts its final approach.
const DIVE: f32 = 26.5;
const TOUCHDOWN: f32 = 29.0;
const CASTAWAY: f32 = 31.0;
/// The camera rises over the wreck, then settles on the view the intro holds.
const OVERHEAD: (f32, f32) = (30.0, 35.5);
const HOLD: (f32, f32) = (35.5, 40.0);
const FADE_OUT: f32 = 47.0;

/// How high above the island the ship breaks up: far beyond the camera's reach from the island,
/// so the orbit set and the island never share a picture.
pub const ORBIT_ALTITUDE: f32 = 4000.0;
/// The placeholder planet seen from orbit: its radius, and how far its centre is below the ship.
pub const PLANET_RADIUS: f32 = 600.0;
pub const PLANET_BELOW: f32 = 820.0;
const ORBIT_SPEED: f32 = 14.0;
/// Horizontal length of the section's final dive, and its least steepness (rise over run).
const DIVE_RUN: f32 = 60.0;
const DIVE_SLOPE: f32 = 0.7;
/// Height of the section's centre above the ground at impact, and the least height it keeps above
/// the ground on the way in, except over the last few metres.
const IMPACT_HEIGHT: f32 = 4.5;
const CLEARANCE: f32 = 7.0;

/// The levels the intro goes back through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Orbit,
    Mainland,
    Island,
}

impl Stop {
    pub fn numeral(self) -> &'static str {
        match self {
            Stop::Orbit => "III",
            Stop::Mainland => "II",
            Stop::Island => "I",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Stop::Orbit => "Orbit",
            Stop::Mainland => "The mainland",
            Stop::Island => "The island",
        }
    }
}

/// A line of text under the picture, and how visible it is.
#[derive(Clone, Copy, Debug)]
pub struct Caption {
    pub stop: Stop,
    pub line: &'static str,
    pub alpha: f32,
}

const CAPTIONS: [(f32, f32, Stop, &str); 4] = [
    (1.5, 11.5, Stop::Orbit, "The ship came in too fast. Its drive section tore loose, and it is up there still."),
    (14.0, 22.0, Stop::Mainland, "The main hull came down on the far side of the strait."),
    (30.5, 38.5, Stop::Island, "The forward section came down here, with you in it."),
    (39.0, 46.5, Stop::Island, "Lumen, the ship's computer, came through as well. It can see everything and touch nothing."),
];

/// The three pieces the ship breaks into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Drive,
    Hull,
    Section,
}

impl Part {
    pub const ALL: [Part; 3] = [Part::Drive, Part::Hull, Part::Section];

    /// Width, height and length. The length runs along local Z, with the nose towards -Z.
    pub fn size(self) -> Vec3 {
        match self {
            Part::Drive => Vec3::new(5.0, 4.0, 8.0),
            Part::Hull => Vec3::new(6.0, 4.5, 14.0),
            // As big as the broken section on the island.
            Part::Section => Vec3::new(6.0, 4.0, 9.0),
        }
    }

    /// Centre of the part in the whole ship, from the hull's centre.
    fn joined(self) -> Vec3 {
        match self {
            Part::Drive => Vec3::Z * 11.0,
            Part::Hull => Vec3::ZERO,
            Part::Section => Vec3::NEG_Z * 11.5,
        }
    }
}

/// What a moving part leaves behind it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trail {
    None,
    Fire,
    Smoke,
    Dust,
}

/// Where a part is (its centre) and which way it faces.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub at: Vec3,
    pub rotation: Quat,
    pub shown: bool,
    pub trail: Trail,
}

const HIDDEN: Pose = Pose { at: Vec3::ZERO, rotation: Quat::IDENTITY, shown: false, trail: Trail::None };

/// Moments the drawing side reacts to (dust, physics, the castaway).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Breakup,
    HullImpact,
    Touchdown,
    CastawayAppears,
}

const EVENTS: [(f32, Event); 4] = [
    (BREAKUP, Event::Breakup),
    (HULL_IMPACT, Event::HullImpact),
    (TOUCHDOWN, Event::Touchdown),
    (CASTAWAY, Event::CastawayAppears),
];

/// The events after `from` up to and including `to`, both times in the loop. `to < from` means the
/// loop wrapped in between.
pub fn events_between(from: f32, to: f32) -> Vec<Event> {
    let inside = |t: f32| if to >= from { t > from && t <= to } else { t > from || t <= to };
    EVENTS.iter().filter(|(t, _)| inside(*t)).map(|(_, e)| *e).collect()
}

/// Everything about one moment of the loop.
#[derive(Clone, Debug)]
pub struct Frame {
    pub eye: Vec3,
    pub look: Vec3,
    /// In the order of [`Part::ALL`].
    pub parts: [Pose; 3],
    /// 0: clear; 1: the picture is covered in `fade_colour` (sRGB).
    pub fade: f32,
    pub fade_colour: [f32; 3],
    /// A white flash over the picture, 0 to 1.
    pub flash: f32,
    /// How hard the camera shakes, roughly in metres.
    pub shake: f32,
    /// Out in space: no sky, no haze.
    pub space: bool,
    pub castaway: bool,
    pub caption: Option<Caption>,
}

const BLACK: [f32; 3] = [0.02, 0.025, 0.04];
const WHITE: [f32; 3] = [1.0, 0.96, 0.88];

/// Where the intro's sets are and the paths between them. Built once from the island and the
/// mainland; everything else is worked out from it.
#[derive(Clone, Debug)]
pub struct Sets {
    /// Centre of the ship's hull at the moment it breaks up.
    pub orbit: Vec3,
    orbit_heading: Vec3,
    /// Centre of the main hull where it comes to rest on the mainland.
    pub hull_rest: Vec3,
    /// The wreck on the island, on the ground.
    pub wreck: Vec3,
    /// Centre of the section where it hits the ground, and where its final dive starts.
    pub touchdown: Vec3,
    dive: Vec3,
    /// Horizontal direction of flight from the mainland to the island, and its right-hand side.
    heading: Vec3,
    side: Vec3,
    hull_entry: Vec3,
    section_entry: Vec3,
    /// The view the intro settles on (eye, look), and the player's own view.
    pub hold: (Vec3, Vec3),
    pub play: (Vec3, Vec3),
}

impl Sets {
    /// `wreck` and `mainland_wreck` are on the ground; `play` is the player's view at the start.
    pub fn new(island: &VoxelWorld, wreck: Vec3, spawn: Vec3, mainland_wreck: Vec3, play: (Vec3, Vec3)) -> Self {
        let hull_rest = mainland_wreck + Vec3::Y * Part::Hull.size().y / 2.0;
        let heading = (wreck - hull_rest).with_y(0.0).normalize();
        let side = Vec3::new(-heading.z, 0.0, heading.x);
        let touchdown = wreck + Vec3::Y * IMPACT_HEIGHT;

        // The final approach is a straight dive, as steep as it has to be to clear everything on the
        // way in, over the section's full width.
        let ground = |p: Vec3| (island.surface(p.x.floor() as i32, p.z.floor() as i32) as f32).max(SEA_LEVEL);
        let mut slope = DIVE_SLOPE;
        for d in 10..=300 {
            let back = touchdown - heading * d as f32;
            for across in [-4.0, 0.0, 4.0] {
                slope = slope.max((ground(back + side * across) + CLEARANCE - touchdown.y) / d as f32);
            }
        }
        let dive = touchdown - heading * DIVE_RUN + Vec3::Y * DIVE_RUN * slope;

        let hull_entry = hull_rest - heading * 260.0 + Vec3::Y * 190.0;
        let section_entry = hull_entry - heading * 30.0 + Vec3::Y * 40.0 - side * 14.0;
        let centre = Vec3::new(island.size.x as f32 / 2.0, ORBIT_ALTITUDE, island.size.z as f32 / 2.0);

        let look = (wreck + spawn) / 2.0 + Vec3::Y * 1.5;
        let hold = (look + Vec3::new(10.0, 13.0, 24.0), look);
        Sets {
            orbit: centre,
            orbit_heading: Vec3::new(1.0, 0.0, 0.35).normalize(),
            hull_rest,
            wreck,
            touchdown,
            dive,
            heading,
            side,
            hull_entry,
            section_entry,
            hold,
            play,
        }
    }

    /// Centre of the placeholder planet below the orbit set.
    pub fn planet(&self) -> Vec3 {
        self.orbit - Vec3::Y * PLANET_BELOW
    }

    /// Position and velocity of the main hull, from the cut to the mainland until it hits.
    fn hull_motion(&self, t: f32) -> (Vec3, Vec3) {
        let duration = HULL_IMPACT - ORBIT_END;
        let arrive = self.heading * 40.0 - Vec3::Y * 14.0;
        hermite(self.hull_entry, self.heading * 44.0 - Vec3::Y * 30.0, self.hull_rest, arrive, duration, (t - ORBIT_END) / duration)
    }

    /// Position and velocity of the forward section from the cut to the mainland until it hits;
    /// after that, the point of impact and the velocity it hit with.
    fn section_motion(&self, t: f32) -> (Vec3, Vec3) {
        let down = (self.touchdown - self.dive).normalize();
        let speed = self.touchdown.distance(self.dive) / (TOUCHDOWN - DIVE);
        if t < DIVE {
            let duration = DIVE - ORBIT_END;
            hermite(self.section_entry, self.heading * 48.0 - Vec3::Y * 20.0, self.dive, down * speed, duration, (t - ORBIT_END) / duration)
        } else if t < TOUCHDOWN {
            let u = (t - DIVE) / (TOUCHDOWN - DIVE);
            (self.dive.lerp(self.touchdown, u), down * speed)
        } else {
            (self.touchdown, down * speed)
        }
    }
}

/// A cubic Hermite segment from `p0` (velocity `v0`) to `p1` (velocity `v1`) taking `duration`
/// seconds: the position and velocity at the fraction `u` of the way.
fn hermite(p0: Vec3, v0: Vec3, p1: Vec3, v1: Vec3, duration: f32, u: f32) -> (Vec3, Vec3) {
    let u = u.clamp(0.0, 1.0);
    let (u2, u3) = (u * u, u * u * u);
    let (m0, m1) = (v0 * duration, v1 * duration);
    let p = p0 * (2.0 * u3 - 3.0 * u2 + 1.0) + m0 * (u3 - 2.0 * u2 + u) + p1 * (-2.0 * u3 + 3.0 * u2) + m1 * (u3 - u2);
    let d = p0 * (6.0 * u2 - 6.0 * u) + m0 * (3.0 * u2 - 4.0 * u + 1.0) + p1 * (6.0 * u - 6.0 * u2) + m1 * (3.0 * u2 - 2.0 * u);
    (p, d / duration)
}

/// The rotation that turns a part's nose (-Z) along `direction`, keeping it upright.
fn facing(direction: Vec3) -> Quat {
    Transform::default().looking_to(direction.normalize_or(Vec3::NEG_Z), Vec3::Y).rotation
}

fn blend(a: (Vec3, Vec3), b: (Vec3, Vec3), w: f32) -> (Vec3, Vec3) {
    (a.0.lerp(b.0, w), a.1.lerp(b.1, w))
}

/// A decaying pulse after an event at `at`.
fn pulse(t: f32, at: f32, strength: f32, decay: f32) -> f32 {
    if t < at { 0.0 } else { strength * (-(t - at) / decay).exp() }
}

fn parts_at(sets: &Sets, t: f32) -> [Pose; 3] {
    let mut parts = [HIDDEN; 3];
    let [drive, hull, section] = &mut parts;
    if t < ORBIT_END {
        let heading = sets.orbit_heading;
        let tau = (t - BREAKUP).max(0.0);
        let base = sets.orbit + heading * ORBIT_SPEED * (t - BREAKUP);
        let whole = facing(heading);
        // The drive section carries on as it was, turning over slowly, and drifts aside.
        let tumble = Quat::from_axis_angle(Vec3::new(1.0, 0.4, 0.2).normalize(), 0.12 * tau);
        let aside = heading.cross(Vec3::Y).normalize() * 0.9 + Vec3::Y * 0.5;
        *drive = Pose { at: base + whole * Part::Drive.joined() + aside * tau, rotation: whole * tumble, shown: true, trail: Trail::None };
        // The rest noses down and falls towards the planet, burning once it reaches the air.
        let rotation = whole * Quat::from_rotation_x(-0.55 * smoothstep(0.0, 4.0, tau));
        let at = base + heading * 1.5 * tau - Vec3::Y * 2.5 * tau * tau;
        let trail = if t > ENTRY { Trail::Fire } else { Trail::None };
        *hull = Pose { at, rotation, shown: true, trail };
        *section = Pose { at: at + rotation * Part::Section.joined(), rotation, shown: true, trail: Trail::None };
        return parts;
    }
    if t < HULL_IMPACT {
        let (at, velocity) = sets.hull_motion(t);
        *hull = Pose { at, rotation: facing(velocity), shown: true, trail: Trail::Fire };
    }
    if t < TOUCHDOWN {
        let (at, velocity) = sets.section_motion(t);
        let trail = if t < DIVE { Trail::Fire } else { Trail::Smoke };
        *section = Pose { at, rotation: facing(velocity), shown: true, trail };
    }
    parts
}

fn camera_at(sets: &Sets, t: f32, parts: &[Pose; 3]) -> (Vec3, Vec3) {
    if t < ORBIT_END {
        // High above and behind the ship, the planet below it; then down after the falling hull.
        let heading = sets.orbit_heading;
        let base = sets.orbit + heading * ORBIT_SPEED * (t - BREAKUP);
        let sway = (t * 0.25).sin() * 4.0;
        let eye = base - heading * 58.0 + Vec3::Y * 36.0 + heading.cross(Vec3::Y).normalize() * (16.0 + sway);
        let look = (base + heading * 12.0 - Vec3::Y * 22.0).lerp(parts[1].at, smoothstep(5.5, 10.0, t) * 0.8);
        return (eye, look);
    }
    // High on the right-hand side of the flight path (the west), so both pieces cross the picture
    // from left to right, towards the island beyond.
    let overview = (
        sets.hull_rest + sets.side * 230.0 + Vec3::Y * 160.0 + sets.heading * (3.0 * (t - ORBIT_END) - 20.0),
        sets.hull_rest + Vec3::Y * 25.0 + sets.heading * 40.0,
    );
    let (at, velocity) = sets.section_motion(t);
    let direction = velocity.normalize_or(sets.heading);
    // Behind, above and off to the side, so the section's own smoke is not in the way.
    let chase = (at - direction * 40.0 + sets.side * 22.0 + Vec3::Y * 16.0, at + direction * 20.0);
    let overhead = (sets.wreck - sets.heading * 8.0 + Vec3::Y * 42.0, sets.wreck);
    let mut camera = blend(overview, chase, smoothstep(CHASE.0, CHASE.1, t));
    camera = blend(camera, overhead, smoothstep(OVERHEAD.0, OVERHEAD.1, t));
    blend(camera, sets.hold, smoothstep(HOLD.0, HOLD.1, t))
}

fn fade_at(t: f32) -> (f32, [f32; 3]) {
    if t < 1.0 {
        (1.0 - smoothstep(0.0, 1.0, t), BLACK)
    } else if t < ORBIT_END {
        (smoothstep(ORBIT_END - 1.2, ORBIT_END, t), WHITE)
    } else if t < ORBIT_END + 1.2 {
        (1.0 - smoothstep(ORBIT_END, ORBIT_END + 1.2, t), WHITE)
    } else {
        (smoothstep(FADE_OUT, LOOP, t), BLACK)
    }
}

/// The moment `t` seconds into the loop.
pub fn frame_at(sets: &Sets, t: f32) -> Frame {
    let t = t.rem_euclid(LOOP);
    let parts = parts_at(sets, t);
    let (eye, look) = camera_at(sets, t, &parts);
    let (fade, fade_colour) = fade_at(t);
    let caption = CAPTIONS.iter().find(|(from, to, _, _)| t >= *from && t < *to).map(|&(from, to, stop, line)| Caption {
        stop,
        line,
        alpha: smoothstep(from, from + 0.8, t) * (1.0 - smoothstep(to - 0.8, to, t)),
    });
    Frame {
        eye,
        look,
        parts,
        fade,
        fade_colour,
        flash: pulse(t, BREAKUP, 0.8, 0.3) + pulse(t, HULL_IMPACT, 0.3, 0.25) + pulse(t, TOUCHDOWN, 0.45, 0.3),
        shake: pulse(t, BREAKUP, 0.25, 0.5) + pulse(t, HULL_IMPACT, 0.4, 0.4) + pulse(t, TOUCHDOWN, 0.9, 0.5),
        space: t < ORBIT_END,
        castaway: t >= CASTAWAY,
        caption,
    }
}

/// Whether Start can glide straight from the intro's camera to the player's view: the camera is
/// over the island, the section has come down and the castaway is out.
pub fn settled(t: f32) -> bool {
    let t = t.rem_euclid(LOOP);
    (CASTAWAY..FADE_OUT - 0.5).contains(&t)
}

/// The hand-over: from where the camera is (eye, look) to the player's view, `t` seconds in. Ends
/// exactly on `to`.
pub fn glide(from: (Vec3, Vec3), to: (Vec3, Vec3), t: f32) -> (Vec3, Vec3) {
    if t >= GLIDE {
        return to;
    }
    blend(from, to, smoothstep(0.0, GLIDE, t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intro::Mainland;
    use crate::island::build;
    use crate::player::OrbitCamera;
    use physics_core::puzzle::Ch1Layout;

    const STEP: f32 = 1.0 / 60.0;

    fn sets(seed: u64) -> (VoxelWorld, Mainland, Sets) {
        let (world, sites) = build(&Ch1Layout::generate(seed));
        let mainland = Mainland::new(sites.wreck);
        let focus = sites.spawn + Vec3::Y * 1.5;
        let sets = Sets::new(&world, sites.wreck, sites.spawn, Mainland::wreck(), (OrbitCamera::default().eye(focus), focus));
        (world, mainland, sets)
    }

    fn ground(world: &VoxelWorld, p: Vec3) -> f32 {
        (world.surface(p.x.floor() as i32, p.z.floor() as i32) as f32).max(SEA_LEVEL)
    }

    fn times() -> impl Iterator<Item = f32> {
        (0..(LOOP / STEP) as usize).map(|i| i as f32 * STEP)
    }

    #[test]
    fn the_pieces_come_down_at_their_wrecks() {
        let (_, mainland, sets) = sets(7);
        let section = |t: f32| frame_at(&sets, t).parts[2];
        let hull = |t: f32| frame_at(&sets, t).parts[1];
        let impact = section(TOUCHDOWN - 1e-3).at;
        assert!(impact.with_y(0.0).distance(sets.wreck.with_y(0.0)) < 0.1, "the section comes down at {impact}");
        assert!(impact.y - sets.wreck.y < IMPACT_HEIGHT + 0.1);
        assert!(!section(TOUCHDOWN + 1e-3).shown, "the model gives way to the wreck in the island");
        assert!(hull(HULL_IMPACT - 1e-3).at.distance(sets.hull_rest) < 0.1);
        assert!(!hull(HULL_IMPACT + 1e-3).shown);
        // The main hull comes down on the mainland, at the foot of its wreck (two blocks of two metres).
        let rest = sets.hull_rest - Vec3::Y * Part::Hull.size().y / 2.0;
        let above = mainland.ground(rest) - rest.y;
        assert!((0.0..=4.01).contains(&above) && rest.y > SEA_LEVEL + 2.0, "the hull comes down at {rest}, ground {}", mainland.ground(rest));
    }

    #[test]
    fn nothing_flies_through_the_ground() {
        for seed in (0..50).step_by(7) {
            let (world, mainland, sets) = sets(seed);
            for t in times().filter(|t| *t >= ORBIT_END) {
                let frame = frame_at(&sets, t);
                if frame.fade < 0.98 {
                    let eye = frame.eye;
                    assert!(eye.y > ground(&world, eye) + 3.0, "seed {seed}, {t:.2} s: the camera at {eye} is in the island");
                    assert!(eye.y > mainland.ground(eye) + 3.0, "seed {seed}, {t:.2} s: the camera at {eye} is in the mainland");
                }
                let section = frame.parts[2];
                // Over the last few metres it is allowed into the ground: that is the impact.
                if section.shown && t < TOUCHDOWN - 0.3 {
                    let half = Part::Section.size() / 2.0;
                    for end in [-half.z, 0.0, half.z] {
                        let p = section.at + section.rotation * Vec3::Z * end;
                        let bottom = p.y - half.y;
                        assert!(bottom >= ground(&world, p) - 0.01, "seed {seed}, {t:.2} s: the section at {p} is in the island");
                        assert!(bottom >= mainland.ground(p) + 1.0, "seed {seed}, {t:.2} s: the section at {p} is in the mainland");
                    }
                }
                let hull = frame.parts[1];
                if hull.shown && t < HULL_IMPACT - 0.6 {
                    let bottom = hull.at.y - Part::Hull.size().y / 2.0;
                    assert!(bottom >= mainland.ground(hull.at) + 1.0, "seed {seed}, {t:.2} s: the hull at {} is in the mainland", hull.at);
                }
            }
        }
    }

    #[test]
    fn the_camera_never_jumps_in_plain_sight() {
        let (_, _, sets) = sets(7);
        for t in times() {
            let (a, b) = (frame_at(&sets, t), frame_at(&sets, t + STEP));
            if a.fade > 0.98 || b.fade > 0.98 {
                continue;
            }
            let moved = a.eye.distance(b.eye);
            let turned = (a.look - a.eye).angle_between(b.look - b.eye);
            assert!(moved < 4.0, "{t:.2} s: the camera moves {moved} m in one frame");
            assert!(turned < 0.06, "{t:.2} s: the camera turns {turned} rad in one frame");
        }
        // Cuts between sets are hidden: the picture is covered at the cut and where the loop wraps.
        assert!(frame_at(&sets, ORBIT_END).fade > 0.999);
        assert!(frame_at(&sets, 0.0).fade > 0.999);
        assert!(frame_at(&sets, LOOP - 1e-3).fade > 0.99);
    }

    #[test]
    fn events_are_seen_once_including_across_the_wrap() {
        assert_eq!(events_between(4.0, 5.5), vec![Event::Breakup]);
        assert_eq!(events_between(5.0, 5.5), vec![]);
        assert_eq!(events_between(LOOP - 0.1, 0.1), vec![]);
        assert_eq!(events_between(28.0, 4.0), vec![Event::Touchdown, Event::CastawayAppears]);
        let mut seen = Vec::new();
        let mut last = 0.0;
        for t in times().skip(1).chain([0.0, STEP]) {
            seen.extend(events_between(last, t));
            last = t;
        }
        assert_eq!(seen.len(), EVENTS.len());
    }

    #[test]
    fn the_hand_over_ends_on_the_players_view() {
        let (_, _, sets) = sets(7);
        let from = (frame_at(&sets, 42.0).eye, frame_at(&sets, 42.0).look);
        assert_eq!(glide(from, sets.play, GLIDE), sets.play);
        assert_eq!(glide(from, sets.play, 0.0), from);
        assert!(settled(42.0) && !settled(20.0) && !settled(47.5));
    }
}
