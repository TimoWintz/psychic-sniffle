//! The materials table: the physical properties of every block type.
//!
//! This is both the in-game handbook and the data the simulators read. Values are representative
//! room-temperature figures in SI units.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Material {
    Air = 0,
    Rock,
    Soil,
    Grass,
    Sand,
    Water,
    Steel,
    Copper,
    Timber,
    Concrete,
    /// Painted survey block: steel with a bright coating, used for markers and targets.
    Marker,
    /// Hull plating salvaged from the wreck.
    HullAlloy,
}

#[derive(Clone, Copy, Debug)]
pub struct Properties {
    pub name: &'static str,
    /// kg/m^3
    pub density: f64,
    /// J/(kg K)
    pub specific_heat: f64,
    /// W/(m K)
    pub thermal_conductivity: f64,
    /// ohm m; `f64::INFINITY` for insulators
    pub resistivity: f64,
    /// Linear RGB, 0 to 1.
    pub colour: [f32; 3],
    pub solid: bool,
}

impl Material {
    pub const ALL: [Material; 12] = [
        Material::Air,
        Material::Rock,
        Material::Soil,
        Material::Grass,
        Material::Sand,
        Material::Water,
        Material::Steel,
        Material::Copper,
        Material::Timber,
        Material::Concrete,
        Material::Marker,
        Material::HullAlloy,
    ];

    pub fn from_id(id: u8) -> Material {
        Material::ALL.get(id as usize).copied().unwrap_or(Material::Air)
    }

    pub fn id(self) -> u8 {
        self as u8
    }

    pub fn is_solid(self) -> bool {
        self.properties().solid
    }

    /// Mass of one block (1 m^3) in kilograms.
    pub fn block_mass(self) -> f64 {
        self.properties().density
    }

    pub fn properties(self) -> Properties {
        let p = |name, density, specific_heat, thermal_conductivity, resistivity, colour, solid| Properties {
            name,
            density,
            specific_heat,
            thermal_conductivity,
            resistivity,
            colour,
            solid,
        };
        const INSULATOR: f64 = f64::INFINITY;
        match self {
            Material::Air => p("Air", 1.2, 1005.0, 0.026, INSULATOR, [0.0, 0.0, 0.0], false),
            Material::Rock => p("Rock", 2700.0, 790.0, 2.9, INSULATOR, [0.36, 0.35, 0.36], true),
            Material::Soil => p("Soil", 1500.0, 800.0, 0.5, INSULATOR, [0.30, 0.20, 0.12], true),
            Material::Grass => p("Turf", 1300.0, 900.0, 0.4, INSULATOR, [0.20, 0.42, 0.14], true),
            Material::Sand => p("Sand", 1600.0, 830.0, 0.3, INSULATOR, [0.72, 0.64, 0.42], true),
            Material::Water => p("Water", 1000.0, 4186.0, 0.6, 2.0e5, [0.10, 0.30, 0.55], false),
            Material::Steel => p("Steel", 7850.0, 490.0, 50.0, 1.4e-7, [0.45, 0.48, 0.52], true),
            Material::Copper => p("Copper", 8960.0, 385.0, 400.0, 1.7e-8, [0.72, 0.40, 0.20], true),
            Material::Timber => p("Timber", 600.0, 1700.0, 0.15, INSULATOR, [0.50, 0.34, 0.18], true),
            Material::Concrete => p("Concrete", 2400.0, 880.0, 1.4, INSULATOR, [0.62, 0.62, 0.60], true),
            Material::Marker => p("Survey marker", 7850.0, 490.0, 50.0, 1.4e-7, [0.95, 0.55, 0.05], true),
            Material::HullAlloy => p("Hull alloy", 4500.0, 520.0, 7.0, 1.7e-6, [0.70, 0.74, 0.80], true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for m in Material::ALL {
            assert_eq!(Material::from_id(m.id()), m);
        }
        assert_eq!(Material::from_id(250), Material::Air);
    }

    #[test]
    fn only_air_and_water_are_passable() {
        for m in Material::ALL {
            assert_eq!(m.is_solid(), !matches!(m, Material::Air | Material::Water));
        }
    }
}
