//! Scalar constraint equations.
//!
//! Every residual is length-like (millimetres) or dimensionless (sine, cosine, radians), so one
//! absolute tolerance is meaningful for all of them.

use crate::dual::{Dual, N};
use std::f64::consts::PI;

/// The form of an equation. The variable list of the equation is given in the order the form
/// documents.
#[derive(Clone, Debug)]
#[allow(missing_docs)] // the variant docs describe the fields
pub enum Kind {
    /// `sum(c_i * v_i) - k`.
    Linear { coeffs: Vec<f64>, k: f64 },
    /// `[x1 y1 x2 y2]`: `|p2 - p1| - d`.
    Dist { d: f64 },
    /// `[a b c d]` (points; vectors `b - a` and `d - c`): sine of the angle between them.
    Sine,
    /// Same variables: cosine of the angle between them.
    Cosine,
    /// Same variables: `wrap(atan2(cross, dot) - theta)`.
    Angle { theta: f64 },
    /// Same variables: `|b - a| - |d - c|`.
    LenDiff,
    /// `[p a b]`: signed distance of `p` from the line through `a` and `b`.
    PointLine,
    /// `[p a b]`: `sign * signed_distance - d`.
    PointLineDist { d: f64, sign: f64 },
    /// `[p c r]`: `|p - c| - r`.
    PointCircle,
    /// `[p c a b theta]`: scaled `(sqrt(u^2/a^2 + v^2/b^2) - 1)` in the ellipse frame.
    PointEllipse,
    /// `[p q a b]`: signed distance of the midpoint of `p q` from the line `a b`.
    MidOnLine,
    /// `[a b c r]`: `sign * signed_distance(c, line ab) - r`.
    TangentLineCircle { sign: f64 },
    /// `[c1 r1 c2 r2]`: `|c1 - c2| - s * (r1 + k * r2)`.
    TangentCircles { k: f64, s: f64 },
}

impl Kind {
    /// Number of variables the equation takes.
    pub fn arity(&self) -> usize {
        match self {
            Kind::Linear { coeffs, .. } => coeffs.len(),
            Kind::Dist { .. } => 4,
            Kind::Sine | Kind::Cosine | Kind::Angle { .. } | Kind::LenDiff | Kind::MidOnLine => 8,
            Kind::PointLine | Kind::PointLineDist { .. } => 6,
            Kind::PointCircle => 5,
            Kind::PointEllipse => 7,
            Kind::TangentLineCircle { .. } => 7,
            Kind::TangentCircles { .. } => 6,
        }
    }
}

fn norm(x: Dual, y: Dual) -> Dual {
    Dual::hypot(x, y)
}

/// Signed distance from `p` to the line through `a` and `b`.
fn signed_distance(p: [Dual; 2], a: [Dual; 2], b: [Dual; 2]) -> Dual {
    let (ux, uy) = (b[0] - a[0], b[1] - a[1]);
    let (wx, wy) = (p[0] - a[0], p[1] - a[1]);
    (ux * wy - uy * wx) / norm(ux, uy)
}

/// Evaluates the residual of an equation with its partial derivatives.
pub fn residual(kind: &Kind, v: &[Dual]) -> Dual {
    debug_assert!(v.len() <= N && v.len() == kind.arity());
    match kind {
        Kind::Linear { coeffs, k } => {
            let mut s = Dual::constant(-k);
            for (c, x) in coeffs.iter().zip(v) {
                s = s + *x * *c;
            }
            s
        }
        Kind::Dist { d } => norm(v[2] - v[0], v[3] - v[1]) - *d,
        Kind::Sine | Kind::Cosine | Kind::Angle { .. } | Kind::LenDiff => {
            let (ux, uy) = (v[2] - v[0], v[3] - v[1]);
            let (wx, wy) = (v[6] - v[4], v[7] - v[5]);
            let cross = ux * wy - uy * wx;
            let dot = ux * wx + uy * wy;
            match kind {
                Kind::Sine => cross / (norm(ux, uy) * norm(wx, wy)),
                Kind::Cosine => dot / (norm(ux, uy) * norm(wx, wy)),
                Kind::LenDiff => norm(ux, uy) - norm(wx, wy),
                Kind::Angle { theta } => {
                    let diff = cross.atan2(dot) - *theta;
                    let shift = 2.0 * PI * libm::round(diff.v / (2.0 * PI));
                    diff - shift
                }
                _ => unreachable!(),
            }
        }
        Kind::PointLine => signed_distance([v[0], v[1]], [v[2], v[3]], [v[4], v[5]]),
        Kind::PointLineDist { d, sign } => {
            signed_distance([v[0], v[1]], [v[2], v[3]], [v[4], v[5]]) * *sign - *d
        }
        Kind::PointCircle => norm(v[0] - v[2], v[1] - v[3]) - v[4],
        Kind::PointEllipse => {
            let (dx, dy) = (v[0] - v[2], v[1] - v[3]);
            let (c, s) = (v[6].cos(), v[6].sin());
            let (u, w) = (dx * c + dy * s, dy * c - dx * s);
            let (a, b) = (v[4], v[5]);
            let n = ((u / a) * (u / a) + (w / b) * (w / b) + Dual::constant(1e-24)).sqrt();
            (n - 1.0) * ((a + b) * 0.5)
        }
        Kind::MidOnLine => {
            let mid = [(v[0] + v[2]) * 0.5, (v[1] + v[3]) * 0.5];
            signed_distance(mid, [v[4], v[5]], [v[6], v[7]])
        }
        Kind::TangentLineCircle { sign } => {
            signed_distance([v[4], v[5]], [v[0], v[1]], [v[2], v[3]]) * *sign - v[6]
        }
        Kind::TangentCircles { k, s } => norm(v[0] - v[3], v[1] - v[4]) - (v[2] + v[5] * *k) * *s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(kind: &Kind, x: &[f64]) -> f64 {
        let v: Vec<Dual> = x
            .iter()
            .enumerate()
            .map(|(i, &c)| Dual::var(c, i))
            .collect();
        residual(kind, &v).v
    }

    #[test]
    fn residuals_vanish_on_satisfied_configurations() {
        // distance 5 (3-4-5 triangle)
        assert!(eval(&Kind::Dist { d: 5.0 }, &[0.0, 0.0, 3.0, 4.0]).abs() < 1e-9);
        // parallel / perpendicular
        let par = [0.0, 0.0, 1.0, 1.0, 5.0, 5.0, 7.0, 7.0];
        assert!(eval(&Kind::Sine, &par).abs() < 1e-12);
        let perp = [0.0, 0.0, 1.0, 1.0, 5.0, 5.0, 4.0, 6.0];
        assert!(eval(&Kind::Cosine, &perp).abs() < 1e-12);
        // angle of 90 degrees, and wrap-around (-270 degrees is the same angle)
        assert!(eval(&Kind::Angle { theta: PI / 2.0 }, &perp).abs() < 1e-12);
        assert!(eval(&Kind::Angle { theta: -1.5 * PI }, &perp).abs() < 1e-12);
        // point on line y = x; midpoint on it
        assert!(eval(&Kind::PointLine, &[2.0, 2.0, 0.0, 0.0, 1.0, 1.0]).abs() < 1e-12);
        assert!(eval(&Kind::MidOnLine, &[0.0, 2.0, 2.0, 0.0, 0.0, 0.0, 1.0, 1.0]).abs() < 1e-12);
        // point on circle, tangent line (y = 1 touches a unit circle at the origin)
        assert!(eval(&Kind::PointCircle, &[0.0, 1.0, 0.0, 0.0, 1.0]).abs() < 1e-9);
        let t = Kind::TangentLineCircle { sign: -1.0 };
        assert!(eval(&t, &[-3.0, 1.0, 3.0, 1.0, 0.0, 0.0, 1.0]).abs() < 1e-9);
        // circles externally tangent
        let ext = Kind::TangentCircles { k: 1.0, s: 1.0 };
        assert!(eval(&ext, &[0.0, 0.0, 1.0, 3.0, 0.0, 2.0]).abs() < 1e-9);
        // point on an ellipse a=3,b=2 rotated by 90 degrees: (0, 3)
        let x = [0.0, 3.0, 0.0, 0.0, 3.0, 2.0, PI / 2.0];
        assert!(eval(&Kind::PointEllipse, &x).abs() < 1e-9);
    }

    #[test]
    fn arity_matches_residual() {
        for (k, n) in [
            (Kind::Sine, 8),
            (Kind::PointLine, 6),
            (Kind::PointEllipse, 7),
        ] {
            assert_eq!(k.arity(), n);
        }
    }
}
