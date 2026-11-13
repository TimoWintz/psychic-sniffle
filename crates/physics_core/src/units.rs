//! Physical quantities with dimensions.
//!
//! A [`Quantity`] is a value in SI base units plus a [`Dim`]. Arithmetic checks dimensions, so adding
//! a length to a time is an error rather than a number.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Number of SI base units tracked: metre, kilogram, second, ampere, kelvin.
pub const BASES: usize = 5;
const BASE_SYMBOLS: [&str; BASES] = ["m", "kg", "s", "A", "K"];

/// Exponents of the SI base units, stored in halves so that square roots stay exact
/// (`Dim([2, 0, 0, 0, 0])` is a length, `Dim([1, 0, 0, 0, 0])` is the square root of a length).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct Dim(pub [i8; BASES]);

impl Dim {
    pub const NONE: Dim = Dim([0, 0, 0, 0, 0]);
    pub const LENGTH: Dim = Dim([2, 0, 0, 0, 0]);
    pub const MASS: Dim = Dim([0, 2, 0, 0, 0]);
    pub const TIME: Dim = Dim([0, 0, 2, 0, 0]);
    pub const CURRENT: Dim = Dim([0, 0, 0, 2, 0]);
    pub const TEMPERATURE: Dim = Dim([0, 0, 0, 0, 2]);
    pub const SPEED: Dim = Dim([2, 0, -2, 0, 0]);
    pub const ACCELERATION: Dim = Dim([2, 0, -4, 0, 0]);
    pub const FORCE: Dim = Dim([2, 2, -4, 0, 0]);
    pub const ENERGY: Dim = Dim([4, 2, -4, 0, 0]);
    pub const POWER: Dim = Dim([4, 2, -6, 0, 0]);
    pub const PRESSURE: Dim = Dim([-2, 2, -4, 0, 0]);
    pub const FREQUENCY: Dim = Dim([0, 0, -2, 0, 0]);
    pub const CHARGE: Dim = Dim([0, 0, 2, 2, 0]);
    pub const VOLTAGE: Dim = Dim([4, 2, -6, -2, 0]);
    pub const RESISTANCE: Dim = Dim([4, 2, -6, -4, 0]);
    pub const AREA: Dim = Dim([4, 0, 0, 0, 0]);
    pub const VOLUME: Dim = Dim([6, 0, 0, 0, 0]);
    pub const DENSITY: Dim = Dim([-6, 2, 0, 0, 0]);

    pub fn is_none(self) -> bool {
        self == Dim::NONE
    }

    pub fn mul(self, other: Dim) -> Dim {
        let mut out = [0i8; BASES];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = self.0[i] + other.0[i];
        }
        Dim(out)
    }

    pub fn div(self, other: Dim) -> Dim {
        let mut out = [0i8; BASES];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = self.0[i] - other.0[i];
        }
        Dim(out)
    }

    /// Raise to the power `num / den`. `None` if an exponent would not be a whole number of halves.
    pub fn pow_ratio(self, num: i32, den: i32) -> Option<Dim> {
        let mut out = [0i8; BASES];
        for (i, slot) in out.iter_mut().enumerate() {
            let scaled = self.0[i] as i32 * num;
            if scaled % den != 0 {
                return None;
            }
            *slot = i8::try_from(scaled / den).ok()?;
        }
        Some(Dim(out))
    }

    /// Everyday name of the dimension if it has one ("speed"), otherwise `None`.
    pub fn name(self) -> Option<&'static str> {
        NAMED.iter().find(|n| n.dim == self).map(|n| n.name)
    }

    /// Preferred SI unit symbol ("m/s" for a speed, "" for a pure number).
    pub fn unit_symbol(self) -> String {
        if let Some(n) = NAMED.iter().find(|n| n.dim == self) {
            return n.unit.to_string();
        }
        let part = |want_positive: bool| -> Vec<String> {
            let mut parts = Vec::new();
            for (i, &halves) in self.0.iter().enumerate() {
                if halves == 0 || (halves > 0) != want_positive {
                    continue;
                }
                let mag = halves.unsigned_abs();
                parts.push(match (mag / 2, mag % 2) {
                    (1, 0) => BASE_SYMBOLS[i].to_string(),
                    (whole, 0) => format!("{}^{}", BASE_SYMBOLS[i], whole),
                    _ => format!("{}^({}/2)", BASE_SYMBOLS[i], mag),
                });
            }
            parts
        };
        let (top, bottom) = (part(true), part(false));
        match (top.is_empty(), bottom.is_empty()) {
            (true, true) => String::new(),
            (false, true) => top.join(" "),
            (true, false) => format!("1/{}", wrap(&bottom)),
            (false, false) => format!("{}/{}", top.join(" "), wrap(&bottom)),
        }
    }

    /// Text for messages to the player: "a speed (m/s)", "a pure number", "a quantity in kg/s".
    pub fn describe(self) -> String {
        if self.is_none() {
            return "a pure number".into();
        }
        match self.name() {
            Some(name) => format!("{} {} ({})", article(name), name, self.unit_symbol()),
            None => format!("a quantity in {}", self.unit_symbol()),
        }
    }
}

fn wrap(parts: &[String]) -> String {
    if parts.len() == 1 {
        parts[0].clone()
    } else {
        format!("({})", parts.join(" "))
    }
}

fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    }
}

struct Named {
    dim: Dim,
    name: &'static str,
    unit: &'static str,
}

const NAMED: &[Named] = &[
    Named { dim: Dim::NONE, name: "pure number", unit: "" },
    Named { dim: Dim::LENGTH, name: "length", unit: "m" },
    Named { dim: Dim::MASS, name: "mass", unit: "kg" },
    Named { dim: Dim::TIME, name: "time", unit: "s" },
    Named { dim: Dim::CURRENT, name: "current", unit: "A" },
    Named { dim: Dim::TEMPERATURE, name: "temperature", unit: "K" },
    Named { dim: Dim::SPEED, name: "speed", unit: "m/s" },
    Named { dim: Dim::ACCELERATION, name: "acceleration", unit: "m/s^2" },
    Named { dim: Dim::FORCE, name: "force", unit: "N" },
    Named { dim: Dim::ENERGY, name: "energy", unit: "J" },
    Named { dim: Dim::POWER, name: "power", unit: "W" },
    Named { dim: Dim::PRESSURE, name: "pressure", unit: "Pa" },
    Named { dim: Dim::FREQUENCY, name: "frequency", unit: "Hz" },
    Named { dim: Dim::CHARGE, name: "charge", unit: "C" },
    Named { dim: Dim::VOLTAGE, name: "voltage", unit: "V" },
    Named { dim: Dim::RESISTANCE, name: "resistance", unit: "ohm" },
    Named { dim: Dim::AREA, name: "area", unit: "m^2" },
    Named { dim: Dim::VOLUME, name: "volume", unit: "m^3" },
    Named { dim: Dim::DENSITY, name: "density", unit: "kg/m^3" },
];

/// A value in SI base units together with its dimension.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Quantity {
    pub value: f64,
    pub dim: Dim,
}

impl Quantity {
    pub const fn new(value: f64, dim: Dim) -> Self {
        Quantity { value, dim }
    }
    pub const fn number(value: f64) -> Self {
        Quantity { value, dim: Dim::NONE }
    }
    pub const fn metres(v: f64) -> Self {
        Quantity::new(v, Dim::LENGTH)
    }
    pub const fn seconds(v: f64) -> Self {
        Quantity::new(v, Dim::TIME)
    }
    pub const fn speed(v: f64) -> Self {
        Quantity::new(v, Dim::SPEED)
    }
    pub const fn acceleration(v: f64) -> Self {
        Quantity::new(v, Dim::ACCELERATION)
    }
    /// An angle given in degrees. Angles are pure numbers (radians) internally.
    pub fn degrees(v: f64) -> Self {
        Quantity::number(v.to_radians())
    }

    /// Format in the preferred SI unit with four significant figures: "12.50 m/s".
    pub fn display(&self) -> String {
        let unit = self.dim.unit_symbol();
        if unit.is_empty() {
            sig(self.value, 4)
        } else {
            format!("{} {}", sig(self.value, 4), unit)
        }
    }

    /// Format in a named unit ("deg", "km"). Falls back to [`Quantity::display`] if the unit is unknown
    /// or has a different dimension.
    pub fn display_in(&self, unit: &str) -> String {
        match lookup_unit(unit) {
            Some(u) if u.dim == self.dim && u.value != 0.0 => {
                format!("{} {}", sig(self.value / u.value, 4), unit)
            }
            _ => self.display(),
        }
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

/// Format with `digits` significant figures, without scientific notation for everyday magnitudes.
pub fn sig(value: f64, digits: usize) -> String {
    if value == 0.0 || !value.is_finite() {
        return format!("{value}");
    }
    let magnitude = value.abs().log10().floor() as i32;
    if !(-4..9).contains(&magnitude) {
        return format!("{:.*e}", digits.saturating_sub(1), value);
    }
    let decimals = (digits as i32 - 1 - magnitude).max(0) as usize;
    format!("{value:.decimals$}")
}

/// A unit or constant by symbol. The value is the factor to SI ("km" is 1000 with [`Dim::LENGTH`]).
///
/// The gram is spelled `gram`, not `g`: `g` is reserved for gravitational acceleration in the notebook.
pub fn lookup_unit(symbol: &str) -> Option<Quantity> {
    let q = |v: f64, d: Dim| Some(Quantity::new(v, d));
    match symbol {
        "m" => q(1.0, Dim::LENGTH),
        "cm" => q(1e-2, Dim::LENGTH),
        "mm" => q(1e-3, Dim::LENGTH),
        "km" => q(1e3, Dim::LENGTH),
        "s" => q(1.0, Dim::TIME),
        "ms" => q(1e-3, Dim::TIME),
        "min" => q(60.0, Dim::TIME),
        "h" | "hr" => q(3600.0, Dim::TIME),
        "kg" => q(1.0, Dim::MASS),
        "gram" => q(1e-3, Dim::MASS),
        "t" | "tonne" => q(1e3, Dim::MASS),
        "N" => q(1.0, Dim::FORCE),
        "kN" => q(1e3, Dim::FORCE),
        "J" => q(1.0, Dim::ENERGY),
        "kJ" => q(1e3, Dim::ENERGY),
        "MJ" => q(1e6, Dim::ENERGY),
        "W" => q(1.0, Dim::POWER),
        "kW" => q(1e3, Dim::POWER),
        "Pa" => q(1.0, Dim::PRESSURE),
        "kPa" => q(1e3, Dim::PRESSURE),
        "Hz" => q(1.0, Dim::FREQUENCY),
        "A" => q(1.0, Dim::CURRENT),
        "mA" => q(1e-3, Dim::CURRENT),
        "V" => q(1.0, Dim::VOLTAGE),
        "ohm" | "Ω" => q(1.0, Dim::RESISTANCE),
        "C" => q(1.0, Dim::CHARGE),
        "K" => q(1.0, Dim::TEMPERATURE),
        "rad" => q(1.0, Dim::NONE),
        "deg" | "°" => q(std::f64::consts::PI / 180.0, Dim::NONE),
        "pi" | "π" => q(std::f64::consts::PI, Dim::NONE),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_dimensions_compose() {
        assert_eq!(Dim::LENGTH.div(Dim::TIME), Dim::SPEED);
        assert_eq!(Dim::SPEED.div(Dim::TIME), Dim::ACCELERATION);
        assert_eq!(Dim::MASS.mul(Dim::ACCELERATION), Dim::FORCE);
        assert_eq!(Dim::FORCE.mul(Dim::LENGTH), Dim::ENERGY);
        assert_eq!(Dim::VOLTAGE.div(Dim::CURRENT), Dim::RESISTANCE);
        assert_eq!(Dim::VOLTAGE.mul(Dim::CURRENT), Dim::POWER);
    }

    #[test]
    fn square_root_of_dimension() {
        assert_eq!(Dim::AREA.pow_ratio(1, 2), Some(Dim::LENGTH));
        // sqrt(length / acceleration) is a time
        assert_eq!(Dim::LENGTH.div(Dim::ACCELERATION).pow_ratio(1, 2), Some(Dim::TIME));
        // a cube root of a length is not representable
        assert_eq!(Dim::LENGTH.pow_ratio(1, 3), None);
    }

    #[test]
    fn symbols_and_descriptions() {
        assert_eq!(Dim::SPEED.unit_symbol(), "m/s");
        assert_eq!(Dim([2, 2, -2, 0, 0]).unit_symbol(), "m kg/s");
        assert_eq!(Dim([0, 0, -4, 0, 0]).unit_symbol(), "1/s^2");
        assert_eq!(Dim::ACCELERATION.describe(), "an acceleration (m/s^2)");
        assert_eq!(Dim::NONE.describe(), "a pure number");
    }

    #[test]
    fn formatting() {
        assert_eq!(Quantity::speed(12.5).display(), "12.50 m/s");
        assert_eq!(Quantity::degrees(55.0).display_in("deg"), "55.00 deg");
        assert_eq!(Quantity::metres(1500.0).display_in("km"), "1.500 km");
        assert_eq!(sig(0.012345, 3), "0.0123");
        assert_eq!(sig(123456.0, 4), "123456");
    }
}
