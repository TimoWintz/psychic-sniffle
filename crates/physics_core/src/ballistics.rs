//! Exact projectile motion in a vertical plane, and flight over block terrain.
//!
//! `x` is horizontal distance along the launch direction, `y` is height. The tracked point is the
//! bottom centre of the launched object, so it starts on a surface and lands when it meets one.
//! Everything here is closed form: outcomes do not depend on a time step.

use serde::{Deserialize, Serialize};

/// Time to fall a height `h` from rest.
pub fn fall_time(h: f64, g: f64) -> f64 {
    (2.0 * h / g).sqrt()
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Projectile {
    pub x0: f64,
    pub y0: f64,
    pub vx: f64,
    pub vy: f64,
    pub g: f64,
}

impl Projectile {
    /// Launch from `(x0, y0)` at `speed`, `angle` radians above the horizontal.
    pub fn from_angle(x0: f64, y0: f64, speed: f64, angle: f64, g: f64) -> Self {
        Projectile { x0, y0, vx: speed * angle.cos(), vy: speed * angle.sin(), g }
    }

    pub fn pos(&self, t: f64) -> (f64, f64) {
        (self.x0 + self.vx * t, self.y0 + self.vy * t - 0.5 * self.g * t * t)
    }

    pub fn vel(&self, t: f64) -> (f64, f64) {
        (self.vx, self.vy - self.g * t)
    }

    /// Time at which the projectile is at horizontal position `x` (requires forward motion).
    pub fn time_at_x(&self, x: f64) -> Option<f64> {
        if self.vx <= 0.0 {
            return None;
        }
        let t = (x - self.x0) / self.vx;
        (t >= 0.0).then_some(t)
    }

    pub fn height_at_x(&self, x: f64) -> Option<f64> {
        self.time_at_x(x).map(|t| self.pos(t).1)
    }

    /// Time at which the projectile comes down through height `y` (the later of the two crossings).
    pub fn time_descending_through(&self, y: f64) -> Option<f64> {
        let disc = self.vy * self.vy - 2.0 * self.g * (y - self.y0);
        if disc < 0.0 {
            return None;
        }
        let t = (self.vy + disc.sqrt()) / self.g;
        (t >= 0.0).then_some(t)
    }

    /// Time, distance and height of the highest point (time is 0 for a launch at or below horizontal).
    pub fn apex(&self) -> (f64, f64, f64) {
        let t = (self.vy / self.g).max(0.0);
        let (x, y) = self.pos(t);
        (t, x, y)
    }
}

/// Block terrain along the launch line: column `i` covers `x` in `[x_start + i, x_start + i + 1)` and
/// its top surface is at `heights[i]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub x_start: f64,
    pub heights: Vec<f64>,
}

impl Profile {
    pub fn flat(x_start: f64, columns: usize, height: f64) -> Self {
        Profile { x_start, heights: vec![height; columns] }
    }

    pub fn x_end(&self) -> f64 {
        self.x_start + self.heights.len() as f64
    }

    /// Set the surface height for the columns covering `[from, to)`.
    pub fn set(&mut self, from: f64, to: f64, height: f64) {
        let a = ((from - self.x_start).round().max(0.0)) as usize;
        let b = ((to - self.x_start).round().max(0.0) as usize).min(self.heights.len());
        for h in &mut self.heights[a.min(b)..b] {
            *h = height;
        }
    }

    pub fn height_at(&self, x: f64) -> Option<f64> {
        let i = (x - self.x_start).floor();
        (i >= 0.0).then(|| self.heights.get(i as usize).copied()).flatten()
    }
}

/// How a flight ended.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum FlightEnd {
    /// Came down onto the top of a column.
    Landed,
    /// Flew into the vertical face of a taller column.
    StruckFace,
    /// Left the surveyed terrain without touching anything.
    LeftTerrain,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flight {
    pub end: FlightEnd,
    pub t: f64,
    pub x: f64,
    pub y: f64,
    /// Top-surface height of the column that was landed on or struck (the last column if the
    /// projectile left the terrain).
    pub surface: f64,
}

/// Fly `p` over `profile` until it touches terrain. Requires `p.vx > 0`.
///
/// For each column in turn: entering it below its top is a strike on its face; otherwise, if the
/// projectile comes down to the column's top while still over it, it lands there.
pub fn fly(p: &Projectile, profile: &Profile) -> Flight {
    assert!(p.vx > 0.0, "flight needs forward motion");
    const EPS: f64 = 1e-9;
    let first = ((p.x0 - profile.x_start) + EPS).floor().max(0.0) as usize;
    for i in first..profile.heights.len() {
        let surface = profile.heights[i];
        let left = profile.x_start + i as f64;
        let right = left + 1.0;
        let t_in = ((left - p.x0) / p.vx).max(0.0);
        let t_out = (right - p.x0) / p.vx;

        if i > first {
            let (_, y_in) = p.pos(t_in);
            if y_in < surface - EPS {
                return Flight { end: FlightEnd::StruckFace, t: t_in, x: left, y: y_in, surface };
            }
        }
        if let Some(t_land) = p.time_descending_through(surface) {
            // `t_land > EPS` keeps a projectile that starts on a surface from landing at once.
            if t_land > EPS && t_land >= t_in - EPS && t_land < t_out {
                let (x, _) = p.pos(t_land);
                return Flight { end: FlightEnd::Landed, t: t_land, x, y: surface, surface };
            }
        }
    }
    let x = profile.x_end();
    let t = (x - p.x0) / p.vx;
    let surface = profile.heights.last().copied().unwrap_or(0.0);
    Flight { end: FlightEnd::LeftTerrain, t, x, y: p.pos(t).1, surface }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn free_fall() {
        assert!(close(fall_time(20.0, 10.0), 2.0));
        assert!(close(fall_time(4.9, 9.8), 1.0));
    }

    #[test]
    fn level_range_matches_closed_form() {
        // R = v^2 sin(2 theta) / g
        for (v, deg, g) in [(20.0, 30.0_f64, 9.81), (15.0, 60.0, 7.2), (31.0, 45.0, 12.9)] {
            let p = Projectile::from_angle(0.0, 0.0, v, deg.to_radians(), g);
            let expected = v * v * (2.0 * deg.to_radians()).sin() / g;
            let flight = fly(&p, &Profile::flat(0.0, 400, 0.0));
            assert_eq!(flight.end, FlightEnd::Landed);
            assert!(close(flight.x, expected), "{} vs {}", flight.x, expected);
            // time of flight = 2 v sin(theta) / g
            assert!(close(flight.t, 2.0 * v * deg.to_radians().sin() / g));
        }
    }

    #[test]
    fn apex_matches_closed_form() {
        let p = Projectile::from_angle(0.0, 0.0, 20.0, 60f64.to_radians(), 10.0);
        let (t, x, y) = p.apex();
        assert!(close(t, 20.0 * 60f64.to_radians().sin() / 10.0));
        assert!(close(y, (20.0 * 60f64.to_radians().sin()).powi(2) / 20.0));
        assert!(close(x, 20.0 * 60f64.to_radians().cos() * t));
    }

    #[test]
    fn horizontal_launch_from_a_ledge() {
        // Ledge 20 m above a plain, g = 10: falls for 2 s, lands 2 v away.
        let mut profile = Profile::flat(0.0, 100, 0.0);
        profile.set(0.0, 5.0, -30.0); // a canyon right after the ledge
        let p = Projectile { x0: 0.0, y0: 20.0, vx: 12.0, vy: 0.0, g: 10.0 };
        let flight = fly(&p, &profile);
        assert_eq!(flight.end, FlightEnd::Landed);
        assert!(close(flight.t, 2.0));
        assert!(close(flight.x, 24.0));
    }

    #[test]
    fn too_slow_falls_into_the_canyon_and_hits_the_far_wall() {
        let mut profile = Profile::flat(0.0, 100, 0.0);
        profile.set(0.0, 10.0, -30.0);
        // Reaches y = 0 at x = 8, still over the canyon; meets the far wall at x = 10 below the rim.
        let p = Projectile { x0: 0.0, y0: 20.0, vx: 4.0, vy: 0.0, g: 10.0 };
        let flight = fly(&p, &profile);
        assert_eq!(flight.end, FlightEnd::StruckFace);
        assert!(close(flight.x, 10.0));
        assert!(flight.y < 0.0);
    }

    #[test]
    fn wall_is_struck_or_cleared() {
        let mut profile = Profile::flat(0.0, 100, 0.0);
        profile.set(10.0, 11.0, 6.0);
        let low = Projectile::from_angle(0.0, 0.0, 22.0, 20f64.to_radians(), 10.0);
        let struck = fly(&low, &profile);
        assert_eq!(struck.end, FlightEnd::StruckFace);
        assert!(close(struck.x, 10.0));
        assert!(close(struck.y, low.height_at_x(10.0).unwrap()));

        let high = Projectile::from_angle(0.0, 0.0, 22.0, 50f64.to_radians(), 10.0);
        let cleared = fly(&high, &profile);
        assert_eq!(cleared.end, FlightEnd::Landed);
        assert!(close(cleared.x, 22.0 * 22.0 * (100f64.to_radians()).sin() / 10.0));
    }

    #[test]
    fn landing_on_top_of_a_wall() {
        let mut profile = Profile::flat(0.0, 100, 0.0);
        profile.set(20.0, 30.0, 5.0);
        let p = Projectile::from_angle(0.0, 0.0, 17.0, 60f64.to_radians(), 10.0);
        let flight = fly(&p, &profile);
        assert_eq!(flight.end, FlightEnd::Landed);
        assert!(close(flight.y, 5.0));
        assert!((20.0..30.0).contains(&flight.x));
    }

    #[test]
    fn leaving_the_terrain() {
        let p = Projectile::from_angle(0.0, 0.0, 50.0, 45f64.to_radians(), 10.0);
        assert_eq!(fly(&p, &Profile::flat(0.0, 20, 0.0)).end, FlightEnd::LeftTerrain);
    }
}
