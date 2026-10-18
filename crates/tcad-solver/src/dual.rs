//! Forward-mode automatic differentiation over at most [`N`] local variables, so every
//! constraint equation has an exact Jacobian row without hand-written derivatives.

use std::ops::{Add, Div, Mul, Neg, Sub};

/// Maximum number of variables of one equation.
pub const N: usize = 8;

/// A value with its partial derivatives with respect to the equation's local variables.
#[derive(Clone, Copy, Debug)]
pub struct Dual {
    /// Value.
    pub v: f64,
    /// Partial derivatives.
    pub d: [f64; N],
}

impl Dual {
    /// A constant.
    pub fn constant(v: f64) -> Self {
        Dual { v, d: [0.0; N] }
    }

    /// The `i`-th local variable with value `v`.
    pub fn var(v: f64, i: usize) -> Self {
        let mut d = [0.0; N];
        d[i] = 1.0;
        Dual { v, d }
    }

    fn map(self, v: f64, dv: f64) -> Dual {
        let mut d = self.d;
        d.iter_mut().for_each(|x| *x *= dv);
        Dual { v, d }
    }

    /// Square root (`self` must be positive).
    pub fn sqrt(self) -> Dual {
        let s = self.v.sqrt();
        self.map(s, 0.5 / s)
    }

    /// Sine (pure-Rust `libm`).
    pub fn sin(self) -> Dual {
        self.map(libm::sin(self.v), libm::cos(self.v))
    }

    /// Cosine (pure-Rust `libm`).
    pub fn cos(self) -> Dual {
        self.map(libm::cos(self.v), -libm::sin(self.v))
    }

    /// `atan2(self, x)` with `self` as y.
    pub fn atan2(self, x: Dual) -> Dual {
        let den = self.v * self.v + x.v * x.v;
        let mut d = [0.0; N];
        for (i, di) in d.iter_mut().enumerate() {
            *di = (x.v * self.d[i] - self.v * x.d[i]) / den;
        }
        Dual {
            v: libm::atan2(self.v, x.v),
            d,
        }
    }

    /// `sqrt(x^2 + y^2)` that stays differentiable at the origin.
    pub fn hypot(x: Dual, y: Dual) -> Dual {
        (x * x + y * y + Dual::constant(1e-24)).sqrt()
    }
}

impl Add for Dual {
    type Output = Dual;
    fn add(self, o: Dual) -> Dual {
        let mut d = self.d;
        d.iter_mut().zip(o.d).for_each(|(a, b)| *a += b);
        Dual { v: self.v + o.v, d }
    }
}

impl Sub for Dual {
    type Output = Dual;
    fn sub(self, o: Dual) -> Dual {
        let mut d = self.d;
        d.iter_mut().zip(o.d).for_each(|(a, b)| *a -= b);
        Dual { v: self.v - o.v, d }
    }
}

impl Mul for Dual {
    type Output = Dual;
    fn mul(self, o: Dual) -> Dual {
        let mut d = [0.0; N];
        for (i, di) in d.iter_mut().enumerate() {
            *di = self.d[i] * o.v + self.v * o.d[i];
        }
        Dual { v: self.v * o.v, d }
    }
}

impl Div for Dual {
    type Output = Dual;
    fn div(self, o: Dual) -> Dual {
        let mut d = [0.0; N];
        let den = o.v * o.v;
        for (i, di) in d.iter_mut().enumerate() {
            *di = (self.d[i] * o.v - self.v * o.d[i]) / den;
        }
        Dual { v: self.v / o.v, d }
    }
}

impl Neg for Dual {
    type Output = Dual;
    fn neg(self) -> Dual {
        self.map(-self.v, -1.0)
    }
}

impl Add<f64> for Dual {
    type Output = Dual;
    fn add(self, o: f64) -> Dual {
        Dual {
            v: self.v + o,
            d: self.d,
        }
    }
}

impl Sub<f64> for Dual {
    type Output = Dual;
    fn sub(self, o: f64) -> Dual {
        Dual {
            v: self.v - o,
            d: self.d,
        }
    }
}

impl Mul<f64> for Dual {
    type Output = Dual;
    fn mul(self, o: f64) -> Dual {
        self.map(self.v * o, o)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numeric(f: impl Fn(f64, f64) -> f64, x: f64, y: f64) -> (f64, f64) {
        let h = 1e-6;
        (
            (f(x + h, y) - f(x - h, y)) / (2.0 * h),
            (f(x, y + h) - f(x, y - h)) / (2.0 * h),
        )
    }

    #[test]
    fn derivatives_match_finite_differences() {
        let (x0, y0) = (0.7, -1.3);
        let f =
            |x: Dual, y: Dual| (x * y + x.sin() * y.cos()) / (Dual::hypot(x, y) + 2.0) + y.atan2(x);
        let r = f(Dual::var(x0, 0), Dual::var(y0, 1));
        let g = |x: f64, y: f64| f(Dual::constant(x), Dual::constant(y)).v;
        let (nx, ny) = numeric(g, x0, y0);
        assert!((r.d[0] - nx).abs() < 1e-6, "{} {}", r.d[0], nx);
        assert!((r.d[1] - ny).abs() < 1e-6, "{} {}", r.d[1], ny);
        assert_eq!(r.d[2], 0.0);
    }

    #[test]
    fn hypot_is_smooth_at_the_origin() {
        let h = Dual::hypot(Dual::var(0.0, 0), Dual::var(0.0, 1));
        assert!(h.v < 1e-11 && h.d[0].abs() < 1e-9 && h.d[1].abs() < 1e-9);
    }
}
