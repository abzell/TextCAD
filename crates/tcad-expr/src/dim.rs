//! Dimensions and the unit table.
//!
//! Values are stored in canonical units: millimetres, kilograms and radians. A dimension is a
//! vector of integer exponents of length, mass and angle; angle is its own dimension so that
//! `30 deg` and a bare `30` cannot be mixed.

use std::fmt;

/// Exponents of the base dimensions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Dim {
    /// Exponent of length.
    pub length: i8,
    /// Exponent of mass.
    pub mass: i8,
    /// Exponent of angle.
    pub angle: i8,
}

impl Dim {
    /// No dimension (counts, ratios).
    pub const NONE: Dim = Dim {
        length: 0,
        mass: 0,
        angle: 0,
    };
    /// Length.
    pub const LENGTH: Dim = Dim {
        length: 1,
        mass: 0,
        angle: 0,
    };
    /// Mass.
    pub const MASS: Dim = Dim {
        length: 0,
        mass: 1,
        angle: 0,
    };
    /// Angle.
    pub const ANGLE: Dim = Dim {
        length: 0,
        mass: 0,
        angle: 1,
    };
    /// Mass per volume.
    pub const DENSITY: Dim = Dim {
        length: -3,
        mass: 1,
        angle: 0,
    };

    /// True for dimensionless.
    pub fn is_none(self) -> bool {
        self == Dim::NONE
    }

    /// Product of two dimensions; `None` on exponent overflow.
    pub fn checked_mul(self, o: Dim) -> Option<Dim> {
        Some(Dim {
            length: self.length.checked_add(o.length)?,
            mass: self.mass.checked_add(o.mass)?,
            angle: self.angle.checked_add(o.angle)?,
        })
    }

    /// Quotient of two dimensions; `None` on exponent overflow.
    pub fn checked_div(self, o: Dim) -> Option<Dim> {
        self.checked_mul(o.pow(-1)?)
    }

    /// Integer power; `None` on exponent overflow.
    pub fn pow(self, n: i32) -> Option<Dim> {
        let f = |e: i8| i8::try_from(i32::from(e).checked_mul(n)?).ok();
        Some(Dim {
            length: f(self.length)?,
            mass: f(self.mass)?,
            angle: f(self.angle)?,
        })
    }

    /// Square root; `None` if any exponent is odd.
    pub fn sqrt(self) -> Option<Dim> {
        if self.length % 2 == 0 && self.mass % 2 == 0 && self.angle % 2 == 0 {
            Some(Dim {
                length: self.length / 2,
                mass: self.mass / 2,
                angle: self.angle / 2,
            })
        } else {
            None
        }
    }
}

impl fmt::Display for Dim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_none() {
            return f.write_str("a dimensionless number");
        }
        match *self {
            Dim::LENGTH => return f.write_str("length"),
            Dim::MASS => return f.write_str("mass"),
            Dim::ANGLE => return f.write_str("angle"),
            Dim::DENSITY => return f.write_str("density"),
            _ => {}
        }
        let mut parts = Vec::new();
        for (name, e) in [
            ("length", self.length),
            ("mass", self.mass),
            ("angle", self.angle),
        ] {
            match e {
                0 => {}
                1 => parts.push(name.to_string()),
                e => parts.push(format!("{name}^{e}")),
            }
        }
        f.write_str(&parts.join("*"))
    }
}

/// A resolved unit: a factor to canonical units and a dimension.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unit {
    /// Multiply a value in this unit by `factor` to get canonical units.
    pub factor: f64,
    /// Dimension.
    pub dim: Dim,
}

impl Unit {
    /// The unit of a bare number.
    pub const NONE: Unit = Unit {
        factor: 1.0,
        dim: Dim::NONE,
    };

    /// Product of units.
    pub fn checked_mul(self, o: Unit) -> Option<Unit> {
        Some(Unit {
            factor: self.factor * o.factor,
            dim: self.dim.checked_mul(o.dim)?,
        })
    }

    /// Quotient of units.
    pub fn checked_div(self, o: Unit) -> Option<Unit> {
        Some(Unit {
            factor: self.factor / o.factor,
            dim: self.dim.checked_div(o.dim)?,
        })
    }

    /// Integer power of a unit.
    pub fn pow(self, n: i32) -> Option<Unit> {
        Some(Unit {
            factor: powi(self.factor, n),
            dim: self.dim.pow(n)?,
        })
    }
}

/// Integer power by repeated squaring (exact and identical on every platform).
pub fn powi(base: f64, n: i32) -> f64 {
    let mut result = 1.0;
    let mut b = base;
    let mut e = n.unsigned_abs();
    while e > 0 {
        if e & 1 == 1 {
            result *= b;
        }
        b *= b;
        e >>= 1;
    }
    if n < 0 { 1.0 / result } else { result }
}

/// Looks up a base unit by name (requirements 4.4).
pub fn base_unit(name: &str) -> Option<Unit> {
    let (factor, dim) = match name {
        "mm" => (1.0, Dim::LENGTH),
        "cm" => (10.0, Dim::LENGTH),
        "m" => (1000.0, Dim::LENGTH),
        "in" => (25.4, Dim::LENGTH),
        "ft" => (304.8, Dim::LENGTH),
        "deg" => (std::f64::consts::PI / 180.0, Dim::ANGLE),
        "rad" => (1.0, Dim::ANGLE),
        "kg" => (1.0, Dim::MASS),
        "g" => (0.001, Dim::MASS),
        "lb" => (0.453_592_37, Dim::MASS),
        _ => return None,
    };
    Some(Unit { factor, dim })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_units() {
        let kg_m3 = base_unit("kg")
            .unwrap()
            .checked_div(base_unit("m").unwrap().pow(3).unwrap())
            .unwrap();
        let g_cm3 = base_unit("g")
            .unwrap()
            .checked_div(base_unit("cm").unwrap().pow(3).unwrap())
            .unwrap();
        assert_eq!(kg_m3.dim, Dim::DENSITY);
        assert_eq!(g_cm3.dim, Dim::DENSITY);
        // 1 g/cm^3 = 1000 kg/m^3
        assert!((g_cm3.factor / kg_m3.factor - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn dim_algebra() {
        assert_eq!(
            Dim::LENGTH.checked_mul(Dim::LENGTH),
            Some(Dim {
                length: 2,
                mass: 0,
                angle: 0
            })
        );
        assert_eq!(Dim::LENGTH.checked_div(Dim::LENGTH), Some(Dim::NONE));
        assert_eq!(Dim::LENGTH.sqrt(), None);
        assert_eq!(Dim::LENGTH.pow(2).unwrap().sqrt(), Some(Dim::LENGTH));
        assert_eq!(
            Dim {
                length: 100,
                mass: 0,
                angle: 0
            }
            .pow(2),
            None
        );
    }

    #[test]
    fn display() {
        assert_eq!(Dim::NONE.to_string(), "a dimensionless number");
        assert_eq!(Dim::LENGTH.to_string(), "length");
        assert_eq!(Dim::LENGTH.pow(2).unwrap().to_string(), "length^2");
        assert_eq!(
            Dim {
                length: -3,
                mass: 1,
                angle: 0
            }
            .to_string(),
            "density"
        );
    }

    #[test]
    fn powi_is_exact_for_small_cases() {
        assert_eq!(powi(10.0, 3), 1000.0);
        assert_eq!(powi(2.0, -2), 0.25);
        assert_eq!(powi(7.0, 0), 1.0);
    }
}
