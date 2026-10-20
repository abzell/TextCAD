//! Kernel-independent geometry: frames, 2D profile segments, profiles and paths.

use std::f64::consts::PI;

/// A 2D point in a frame's x/y plane (mm).
pub type P2 = [f64; 2];
/// A 3D point or vector (mm).
pub type V3 = [f64; 3];

/// Dot product.
pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Cross product.
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Length of a vector.
pub fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// A vector scaled to unit length (the zero vector stays zero).
pub fn unit(a: V3) -> V3 {
    let n = norm(a);
    if n == 0.0 {
        a
    } else {
        [a[0] / n, a[1] / n, a[2] / n]
    }
}

/// Sum of two vectors.
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Difference of two vectors.
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// A vector scaled by a factor.
pub fn scale(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// Rotates `v` about the unit axis `k` by `angle` radians (Rodrigues' formula).
pub fn rotate(v: V3, k: V3, angle: f64) -> V3 {
    let (s, c) = (libm::sin(angle), libm::cos(angle));
    add(
        add(scale(v, c), scale(cross(k, v), s)),
        scale(k, dot(k, v) * (1.0 - c)),
    )
}

/// A right-handed orthonormal frame. A sketch lies in the x/y plane of its frame; `z` is the
/// plane normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// Origin.
    pub origin: V3,
    /// Unit x direction.
    pub x: V3,
    /// Unit y direction.
    pub y: V3,
    /// Unit normal, `x cross y`.
    pub z: V3,
}

impl Frame {
    /// Builds a frame from an origin and two directions; `y` is made orthogonal to `x`.
    pub fn new(origin: V3, x: V3, y_hint: V3) -> Frame {
        let x = unit(x);
        let z = unit(cross(x, y_hint));
        let y = cross(z, x);
        Frame { origin, x, y, z }
    }

    /// The base plane XY: x = X, y = Y, normal = +Z.
    pub fn xy() -> Frame {
        Frame::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
    }

    /// The base plane YZ: x = Y, y = Z, normal = +X.
    pub fn yz() -> Frame {
        Frame::new([0.0; 3], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0])
    }

    /// The base plane XZ: x = X, y = Z, normal = -Y.
    pub fn xz() -> Frame {
        Frame::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
    }

    /// Maps a point of the frame's plane to world coordinates.
    pub fn to_world(&self, p: P2) -> V3 {
        add(self.origin, add(scale(self.x, p[0]), scale(self.y, p[1])))
    }

    /// Maps a world point to the frame's (x, y, height above the plane).
    pub fn to_local(&self, w: V3) -> V3 {
        let d = sub(w, self.origin);
        [dot(d, self.x), dot(d, self.y), dot(d, self.z)]
    }

    /// The frame moved along its normal.
    pub fn offset(&self, distance: f64) -> Frame {
        Frame {
            origin: add(self.origin, scale(self.z, distance)),
            ..*self
        }
    }

    /// The frame rotated about an axis (a point and a unit direction) by `angle` radians.
    pub fn rotated(&self, axis_origin: V3, axis_dir: V3, angle: f64) -> Frame {
        let k = unit(axis_dir);
        let turn = |v: V3| rotate(v, k, angle);
        let o = add(axis_origin, turn(sub(self.origin, axis_origin)));
        Frame {
            origin: o,
            x: turn(self.x),
            y: turn(self.y),
            z: turn(self.z),
        }
    }
}

/// One piece of a 2D wire in a frame's plane.
#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    /// Straight line.
    Line {
        /// Start.
        a: P2,
        /// End.
        b: P2,
    },
    /// Circular arc from `start` through `mid` to `end` (so the direction is unambiguous).
    Arc {
        /// Center.
        center: P2,
        /// Start point.
        start: P2,
        /// A point halfway along the arc.
        mid: P2,
        /// End point.
        end: P2,
    },
    /// A full circle.
    Circle {
        /// Center.
        center: P2,
        /// Radius.
        r: f64,
    },
    /// A full ellipse; the `a` axis is rotated by `theta` from the frame's x axis.
    Ellipse {
        /// Center.
        center: P2,
        /// Semi-axis along the rotated x direction.
        a: f64,
        /// Semi-axis along the rotated y direction.
        b: f64,
        /// Rotation in radians.
        theta: f64,
    },
    /// A spline through (`interpolated`) or controlled by the points.
    Spline {
        /// Points.
        points: Vec<P2>,
        /// True if the curve passes through the points.
        interpolated: bool,
        /// True if the curve is closed.
        closed: bool,
    },
}

impl Segment {
    /// First point of the segment.
    pub fn start(&self) -> P2 {
        match self {
            Segment::Line { a, .. } => *a,
            Segment::Arc { start, .. } => *start,
            Segment::Circle { center, r } => [center[0] + r, center[1]],
            Segment::Ellipse {
                center, a, theta, ..
            } => [
                center[0] + a * libm::cos(*theta),
                center[1] + a * libm::sin(*theta),
            ],
            Segment::Spline { points, .. } => points.first().copied().unwrap_or([0.0; 2]),
        }
    }

    /// Last point of the segment.
    pub fn end(&self) -> P2 {
        match self {
            Segment::Line { b, .. } => *b,
            Segment::Arc { end, .. } => *end,
            Segment::Spline {
                points,
                closed: false,
                ..
            } => points.last().copied().unwrap_or([0.0; 2]),
            other => other.start(),
        }
    }

    /// True if the segment closes on itself.
    pub fn is_closed(&self) -> bool {
        matches!(
            self,
            Segment::Circle { .. } | Segment::Ellipse { .. } | Segment::Spline { closed: true, .. }
        )
    }

    /// Points on the segment for containment tests and bounding boxes (approximate for curves).
    pub fn sample(&self, per_curve: usize) -> Vec<P2> {
        match self {
            Segment::Line { a, b } => vec![*a, *b],
            Segment::Arc {
                center,
                start,
                mid,
                end,
            } => {
                let ang = |p: &P2| libm::atan2(p[1] - center[1], p[0] - center[0]);
                let (a0, am, a1) = (ang(start), ang(mid), ang(end));
                let wrap = |d: f64| d.rem_euclid(2.0 * PI);
                // direction from the start through the middle
                let ccw = wrap(am - a0) < wrap(a1 - a0) || wrap(a1 - a0) == 0.0;
                let sweep = if ccw { wrap(a1 - a0) } else { -wrap(a0 - a1) };
                let r = libm::hypot(start[0] - center[0], start[1] - center[1]);
                (0..=per_curve)
                    .map(|i| {
                        let t = a0 + sweep * i as f64 / per_curve as f64;
                        [center[0] + r * libm::cos(t), center[1] + r * libm::sin(t)]
                    })
                    .collect()
            }
            Segment::Circle { center, r } => (0..per_curve)
                .map(|i| {
                    let t = 2.0 * PI * i as f64 / per_curve as f64;
                    [center[0] + r * libm::cos(t), center[1] + r * libm::sin(t)]
                })
                .collect(),
            Segment::Ellipse {
                center,
                a,
                b,
                theta,
            } => (0..per_curve)
                .map(|i| {
                    let t = 2.0 * PI * i as f64 / per_curve as f64;
                    let (u, v) = (a * libm::cos(t), b * libm::sin(t));
                    let (c, s) = (libm::cos(*theta), libm::sin(*theta));
                    [center[0] + u * c - v * s, center[1] + u * s + v * c]
                })
                .collect(),
            Segment::Spline { points, .. } => points.clone(),
        }
    }
}

/// A closed wire.
#[derive(Clone, Debug, PartialEq)]
pub struct Loop {
    /// Segments in order; consecutive segments share an end point.
    pub segments: Vec<Segment>,
}

impl Loop {
    /// Approximate polygon of the loop.
    pub fn polygon(&self) -> Vec<P2> {
        let mut out = Vec::new();
        for s in &self.segments {
            out.extend(s.sample(32));
        }
        out
    }

    /// Signed area of the approximate polygon (positive counterclockwise).
    pub fn signed_area(&self) -> f64 {
        let p = self.polygon();
        let n = p.len();
        (0..n)
            .map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1])
            .sum::<f64>()
            / 2.0
    }
}

/// A planar face to sweep: an outer loop with optional holes, in a frame.
///
/// Every loop, holes included, runs counterclockwise in the frame (seen from the normal); a kernel
/// turns the holes around itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    /// Plane of the profile.
    pub frame: Frame,
    /// Outer boundary.
    pub outer: Loop,
    /// Holes.
    pub holes: Vec<Loop>,
}

/// An open or closed chain of segments in a frame (a sweep path or guide).
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    /// Plane of the chain.
    pub frame: Frame,
    /// Segments in order.
    pub segments: Vec<Segment>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_planes_are_right_handed() {
        for f in [Frame::xy(), Frame::yz(), Frame::xz()] {
            assert!(norm(sub(cross(f.x, f.y), f.z)) < 1e-15);
        }
        assert_eq!(Frame::xy().z, [0.0, 0.0, 1.0]);
        assert_eq!(Frame::yz().z, [1.0, 0.0, 0.0]);
        assert_eq!(Frame::xz().z, [0.0, -1.0, 0.0]);
        assert_eq!(Frame::xz().to_world([2.0, 3.0]), [2.0, 0.0, 3.0]);
    }

    #[test]
    fn local_and_world_round_trip() {
        let f = Frame::new([1.0, 2.0, 3.0], [1.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        let w = f.to_world([4.0, 5.0]);
        let l = f.to_local(w);
        assert!((l[0] - 4.0).abs() < 1e-12 && (l[1] - 5.0).abs() < 1e-12 && l[2].abs() < 1e-12);
    }

    #[test]
    fn rotation_and_offset() {
        let f = Frame::xy().rotated([0.0; 3], [1.0, 0.0, 0.0], PI / 2.0);
        assert!(norm(sub(f.z, [0.0, -1.0, 0.0])) < 1e-12);
        assert_eq!(Frame::xy().offset(5.0).origin, [0.0, 0.0, 5.0]);
    }

    #[test]
    fn arc_sampling_follows_the_middle_point() {
        // the lower semicircle of a unit circle, start (1,0) -> end (-1,0) via (0,-1) is clockwise
        let s = Segment::Arc {
            center: [0.0, 0.0],
            start: [1.0, 0.0],
            mid: [0.0, -1.0],
            end: [-1.0, 0.0],
        };
        let pts = s.sample(8);
        assert!(pts.iter().all(|p| p[1] <= 1e-12), "{pts:?}");
        let ccw = Segment::Arc {
            center: [0.0, 0.0],
            start: [1.0, 0.0],
            mid: [0.0, 1.0],
            end: [-1.0, 0.0],
        };
        assert!(ccw.sample(8).iter().all(|p| p[1] >= -1e-12));
    }

    #[test]
    fn loop_area() {
        let l = Loop {
            segments: vec![
                Segment::Line {
                    a: [0.0, 0.0],
                    b: [2.0, 0.0],
                },
                Segment::Line {
                    a: [2.0, 0.0],
                    b: [2.0, 3.0],
                },
                Segment::Line {
                    a: [2.0, 3.0],
                    b: [0.0, 3.0],
                },
                Segment::Line {
                    a: [0.0, 3.0],
                    b: [0.0, 0.0],
                },
            ],
        };
        assert!((l.signed_area() - 6.0).abs() < 1e-12);
        let c = Loop {
            segments: vec![Segment::Circle {
                center: [0.0; 2],
                r: 1.0,
            }],
        };
        assert!((c.signed_area() - PI).abs() < 0.03); // a 32-gon, good enough for containment tests
    }
}
