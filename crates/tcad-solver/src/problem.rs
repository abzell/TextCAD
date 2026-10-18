//! Building a constraint problem: variables, geometric primitives and constraints.

use crate::equation::Kind;
use std::f64::consts::PI;

/// Index of a variable.
pub type Var = usize;

/// A 2D point as two variables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    /// X variable.
    pub x: Var,
    /// Y variable.
    pub y: Var,
}

/// A line segment between two points.
#[derive(Clone, Copy, Debug)]
pub struct Line {
    /// Start.
    pub a: Point,
    /// End.
    pub b: Point,
}

/// A circle.
#[derive(Clone, Copy, Debug)]
pub struct Circle {
    /// Center.
    pub c: Point,
    /// Radius variable.
    pub r: Var,
}

/// A circular arc, counterclockwise from `s` to `e` around `c`.
#[derive(Clone, Copy, Debug)]
pub struct Arc {
    /// Center.
    pub c: Point,
    /// Start point.
    pub s: Point,
    /// End point.
    pub e: Point,
    /// Radius variable.
    pub r: Var,
}

/// An ellipse with semi-axes `a` and `b`, the `a` axis rotated by `theta` (radians).
#[derive(Clone, Copy, Debug)]
pub struct Ellipse {
    /// Center.
    pub c: Point,
    /// Semi-axis along the rotated x direction.
    pub a: Var,
    /// Semi-axis along the rotated y direction.
    pub b: Var,
    /// Rotation.
    pub theta: Var,
}

/// An axis-aligned rectangle by its corners.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    /// Bottom-left corner.
    pub bl: Point,
    /// Bottom-right corner.
    pub br: Point,
    /// Top-right corner.
    pub tr: Point,
    /// Top-left corner.
    pub tl: Point,
}

/// A regular polygon.
#[derive(Clone, Debug)]
pub struct Polygon {
    /// Center.
    pub c: Point,
    /// Circumradius variable.
    pub r: Var,
    /// Vertices in counterclockwise order.
    pub verts: Vec<Point>,
}

/// A slot: a centerline with a width (rounded ends are derived).
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    /// Centerline start.
    pub a: Point,
    /// Centerline end.
    pub b: Point,
    /// Width variable.
    pub w: Var,
}

/// A curve a point can lie on or that can be tangent.
#[derive(Clone, Copy, Debug)]
pub enum Curve {
    /// Line (infinite for point-on-curve and tangency).
    Line(Line),
    /// Circle.
    Circle(Circle),
    /// Arc (treated as its full circle).
    Arc(Arc),
    /// Ellipse.
    Ellipse(Ellipse),
}

impl Curve {
    fn circle(self) -> Option<(Point, Var)> {
        match self {
            Curve::Circle(c) => Some((c.c, c.r)),
            Curve::Arc(a) => Some((a.c, a.r)),
            _ => None,
        }
    }
}

/// A user-level constraint. Angles are in radians and lengths in the problem's length unit.
#[derive(Clone, Copy, Debug)]
pub enum Constraint {
    /// Two points coincide.
    Coincident(Point, Point),
    /// Two points share a y coordinate.
    Horizontal(Point, Point),
    /// Two points share an x coordinate.
    Vertical(Point, Point),
    /// Distance between points.
    Distance(Point, Point, f64),
    /// `q.x - p.x = d`.
    HDistance(Point, Point, f64),
    /// `q.y - p.y = d`.
    VDistance(Point, Point, f64),
    /// Perpendicular distance from a point to the line through a segment.
    DistancePointLine(Point, Line, f64),
    /// Two lines are parallel.
    Parallel(Line, Line),
    /// Two lines are perpendicular.
    Perpendicular(Line, Line),
    /// Angle from the first line to the second.
    Angle(Line, Line, f64),
    /// Two lines have equal length.
    EqualLength(Line, Line),
    /// Two circles or arcs have equal radius.
    EqualRadius(Curve, Curve),
    /// Two curves share a center.
    Concentric(Curve, Curve),
    /// Two curves touch tangentially.
    Tangent(Curve, Curve),
    /// Two curves are tangent at a contact point that is known to lie on both (an end point
    /// shared by a line and an arc, or two arcs). Using the contact avoids the first-order
    /// degeneracy of the general form, so such chains count their degrees of freedom correctly.
    TangentAt(Curve, Curve, Point),
    /// A point is the midpoint of a segment.
    Midpoint(Point, Line),
    /// Two points are mirror images in a line.
    Symmetric(Point, Point, Line),
    /// A point is fixed at coordinates.
    Fix(Point, f64, f64),
    /// A point lies on a curve.
    PointOnCurve(Point, Curve),
    /// Radius of a circle or arc.
    Radius(Curve, f64),
    /// Semi-axes `(a, b)` of an ellipse.
    EllipseAxes(Ellipse, f64, f64),
    /// Rotation of an ellipse in radians.
    EllipseAngle(Ellipse, f64),
}

/// A constraint that cannot be expressed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsupported(pub &'static str);

/// One scalar equation.
#[derive(Clone, Debug)]
pub struct Equation {
    /// The form.
    pub kind: Kind,
    /// Variables, in the order the form documents.
    pub vars: Vec<Var>,
    /// The user constraint it came from; `None` for the internal structure of a primitive.
    pub tag: Option<usize>,
}

/// A named group of variables, for per-entity degrees of freedom.
#[derive(Clone, Debug)]
pub struct Entity {
    /// Caller's name.
    pub name: String,
    /// Variables that define the entity.
    pub vars: Vec<Var>,
}

/// A constraint problem.
#[derive(Clone, Debug, Default)]
pub struct Problem {
    pub(crate) x0: Vec<f64>,
    pub(crate) weights: Vec<f64>,
    pub(crate) pinned: Vec<bool>,
    pub(crate) equations: Vec<Equation>,
    pub(crate) entities: Vec<Entity>,
}

impl Problem {
    /// An empty problem.
    pub fn new() -> Self {
        Problem::default()
    }

    /// Adds a variable with an initial value.
    pub fn var(&mut self, init: f64) -> Var {
        self.x0.push(init);
        self.weights.push(1.0);
        self.pinned.push(false);
        self.x0.len() - 1
    }

    /// Sets how strongly a variable resists moving from its initial value (default 1). The solver
    /// takes the step that minimizes the weighted movement, so a variable of weight 100 moves
    /// 100 times less than one of weight 1 when the constraints could be met either way.
    pub fn set_weight(&mut self, v: Var, weight: f64) {
        self.weights[v] = weight.max(1e-12);
    }

    /// Pins a variable: the solver first tries to keep it exactly at its initial value (it then
    /// is a constant and does not couple the equations it appears in). If the rest cannot be
    /// satisfied that way, the pin is released and the variable becomes an ordinary one with the
    /// given (large) `soft_weight`. Degrees of freedom and conflicts never depend on pins.
    pub fn pin(&mut self, v: Var, soft_weight: f64) {
        self.pinned[v] = true;
        self.set_weight(v, soft_weight);
    }

    /// Pins both coordinates of a point.
    pub fn pin_point(&mut self, p: Point, soft_weight: f64) {
        self.pin(p.x, soft_weight);
        self.pin(p.y, soft_weight);
    }

    /// Sets the weight of both coordinates of a point.
    pub fn set_point_weight(&mut self, p: Point, weight: f64) {
        self.set_weight(p.x, weight);
        self.set_weight(p.y, weight);
    }

    /// Adds a point.
    pub fn point(&mut self, x: f64, y: f64) -> Point {
        Point {
            x: self.var(x),
            y: self.var(y),
        }
    }

    /// Initial value of a variable.
    pub fn initial(&self, v: Var) -> f64 {
        self.x0[v]
    }

    /// Number of variables.
    pub fn num_vars(&self) -> usize {
        self.x0.len()
    }

    /// All equations.
    pub fn equations(&self) -> &[Equation] {
        &self.equations
    }

    /// The registered entities.
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Registers an entity for degree-of-freedom reporting; returns its index.
    pub fn entity(&mut self, name: impl Into<String>, vars: Vec<Var>) -> usize {
        self.entities.push(Entity {
            name: name.into(),
            vars,
        });
        self.entities.len() - 1
    }

    fn pt(&self, p: Point) -> (f64, f64) {
        (self.x0[p.x], self.x0[p.y])
    }

    fn push(&mut self, kind: Kind, vars: Vec<Var>, tag: Option<usize>) {
        self.equations.push(Equation { kind, vars, tag });
    }

    fn linear(&mut self, terms: &[(Var, f64)], k: f64, tag: Option<usize>) {
        let (vars, coeffs) = terms.iter().copied().unzip();
        self.push(Kind::Linear { coeffs, k }, vars, tag);
    }

    /// A line segment (two points, no internal structure).
    pub fn line(&self, a: Point, b: Point) -> Line {
        Line { a, b }
    }

    /// A circle with a new radius variable.
    pub fn circle(&mut self, c: Point, r: f64) -> Circle {
        Circle { c, r: self.var(r) }
    }

    /// An arc: the radius variable is tied to both endpoints.
    pub fn arc(&mut self, c: Point, s: Point, e: Point) -> Arc {
        let (cx, cy) = self.pt(c);
        let (sx, sy) = self.pt(s);
        let r = self.var((sx - cx).hypot(sy - cy));
        for p in [s, e] {
            self.push(Kind::PointCircle, vec![p.x, p.y, c.x, c.y, r], None);
        }
        Arc { c, s, e, r }
    }

    /// An ellipse.
    pub fn ellipse(&mut self, c: Point, a: f64, b: f64, theta: f64) -> Ellipse {
        Ellipse {
            c,
            a: self.var(a),
            b: self.var(b),
            theta: self.var(theta),
        }
    }

    /// An axis-aligned rectangle from two opposite corners.
    pub fn rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        let bl = self.point(x0, y0);
        let br = self.point(x1, y0);
        let tr = self.point(x1, y1);
        let tl = self.point(x0, y1);
        self.linear(&[(bl.y, 1.0), (br.y, -1.0)], 0.0, None);
        self.linear(&[(br.x, 1.0), (tr.x, -1.0)], 0.0, None);
        self.linear(&[(tr.y, 1.0), (tl.y, -1.0)], 0.0, None);
        self.linear(&[(tl.x, 1.0), (bl.x, -1.0)], 0.0, None);
        Rect { bl, br, tr, tl }
    }

    /// A regular polygon with `n >= 3` sides.
    pub fn polygon(&mut self, n: usize, cx: f64, cy: f64, radius: f64, phase: f64) -> Polygon {
        let c = self.point(cx, cy);
        let r = self.var(radius);
        let verts: Vec<Point> = (0..n)
            .map(|i| {
                let a = phase + 2.0 * PI * i as f64 / n as f64;
                self.point(cx + radius * libm::cos(a), cy + radius * libm::sin(a))
            })
            .collect();
        for v in &verts {
            self.push(Kind::PointCircle, vec![v.x, v.y, c.x, c.y, r], None);
        }
        for i in 1..n {
            let (p, q) = (verts[i], verts[(i + 1) % n]);
            let (a, b) = (verts[0], verts[1]);
            self.push(
                Kind::LenDiff,
                vec![p.x, p.y, q.x, q.y, a.x, a.y, b.x, b.y],
                None,
            );
        }
        Polygon { c, r, verts }
    }

    /// A slot.
    pub fn slot(&mut self, a: Point, b: Point, width: f64) -> Slot {
        Slot {
            a,
            b,
            w: self.var(width),
        }
    }

    fn eq_points(&mut self, p: Point, q: Point, tag: usize) {
        self.linear(&[(p.x, 1.0), (q.x, -1.0)], 0.0, Some(tag));
        self.linear(&[(p.y, 1.0), (q.y, -1.0)], 0.0, Some(tag));
    }

    fn side_sign(&self, p: Point, l: Line) -> f64 {
        let (px, py) = self.pt(p);
        let (ax, ay) = self.pt(l.a);
        let (bx, by) = self.pt(l.b);
        let cross = (bx - ax) * (py - ay) - (by - ay) * (px - ax);
        if cross < 0.0 { -1.0 } else { 1.0 }
    }

    fn four(a: Point, b: Point, c: Point, d: Point) -> Vec<Var> {
        vec![a.x, a.y, b.x, b.y, c.x, c.y, d.x, d.y]
    }

    /// Adds a constraint, tagged with the caller's identifier for reporting.
    pub fn constrain(&mut self, c: Constraint, tag: usize) -> Result<(), Unsupported> {
        let t = Some(tag);
        match c {
            Constraint::Coincident(p, q) => self.eq_points(p, q, tag),
            Constraint::Horizontal(p, q) => self.linear(&[(p.y, 1.0), (q.y, -1.0)], 0.0, t),
            Constraint::Vertical(p, q) => self.linear(&[(p.x, 1.0), (q.x, -1.0)], 0.0, t),
            Constraint::Distance(p, q, d) => {
                self.push(Kind::Dist { d }, vec![p.x, p.y, q.x, q.y], t)
            }
            Constraint::HDistance(p, q, d) => self.linear(&[(q.x, 1.0), (p.x, -1.0)], d, t),
            Constraint::VDistance(p, q, d) => self.linear(&[(q.y, 1.0), (p.y, -1.0)], d, t),
            Constraint::DistancePointLine(p, l, d) => {
                let sign = self.side_sign(p, l);
                self.push(
                    Kind::PointLineDist { d, sign },
                    vec![p.x, p.y, l.a.x, l.a.y, l.b.x, l.b.y],
                    t,
                );
            }
            Constraint::Parallel(l, m) => self.push(Kind::Sine, Self::four(l.a, l.b, m.a, m.b), t),
            Constraint::Perpendicular(l, m) => {
                self.push(Kind::Cosine, Self::four(l.a, l.b, m.a, m.b), t)
            }
            Constraint::Angle(l, m, theta) => {
                self.push(Kind::Angle { theta }, Self::four(l.a, l.b, m.a, m.b), t)
            }
            Constraint::EqualLength(l, m) => {
                self.push(Kind::LenDiff, Self::four(l.a, l.b, m.a, m.b), t)
            }
            Constraint::EqualRadius(a, b) => match (a.circle(), b.circle()) {
                (Some((_, r1)), Some((_, r2))) => self.linear(&[(r1, 1.0), (r2, -1.0)], 0.0, t),
                _ => return Err(Unsupported("equal radius needs two circles or arcs")),
            },
            Constraint::Concentric(a, b) => match (center(a), center(b)) {
                (Some(p), Some(q)) => self.eq_points(p, q, tag),
                _ => return Err(Unsupported("concentric needs curves with a center")),
            },
            Constraint::Tangent(a, b) => self.tangent(a, b, tag)?,
            Constraint::TangentAt(a, b, at) => match (a, b) {
                (Curve::Line(l), other) | (other, Curve::Line(l)) if other.circle().is_some() => {
                    let (c, _) = other.circle().ok_or(Unsupported("tangent"))?;
                    // the radius to the contact is perpendicular to the line
                    self.push(Kind::Cosine, Self::four(at, c, l.a, l.b), t);
                }
                (x, y) if x.circle().is_some() && y.circle().is_some() => {
                    let ((c1, _), (c2, _)) = (
                        x.circle().ok_or(Unsupported("tangent"))?,
                        y.circle().ok_or(Unsupported("tangent"))?,
                    );
                    // both centers lie on the line through the contact
                    self.push(Kind::Sine, Self::four(at, c1, at, c2), t);
                }
                _ => {
                    return Err(Unsupported(
                        "tangent needs a line and a circle/arc, or two circles/arcs",
                    ));
                }
            },
            Constraint::Midpoint(p, l) => {
                self.linear(&[(p.x, 2.0), (l.a.x, -1.0), (l.b.x, -1.0)], 0.0, t);
                self.linear(&[(p.y, 2.0), (l.a.y, -1.0), (l.b.y, -1.0)], 0.0, t);
            }
            Constraint::Symmetric(p, q, l) => {
                self.push(Kind::Cosine, Self::four(p, q, l.a, l.b), t);
                self.push(Kind::MidOnLine, Self::four(p, q, l.a, l.b), t);
            }
            Constraint::Fix(p, x, y) => {
                self.linear(&[(p.x, 1.0)], x, t);
                self.linear(&[(p.y, 1.0)], y, t);
            }
            Constraint::PointOnCurve(p, curve) => match curve {
                Curve::Line(l) => self.push(
                    Kind::PointLine,
                    vec![p.x, p.y, l.a.x, l.a.y, l.b.x, l.b.y],
                    t,
                ),
                Curve::Circle(Circle { c, r }) | Curve::Arc(Arc { c, r, .. }) => {
                    self.push(Kind::PointCircle, vec![p.x, p.y, c.x, c.y, r], t)
                }
                Curve::Ellipse(e) => self.push(
                    Kind::PointEllipse,
                    vec![p.x, p.y, e.c.x, e.c.y, e.a, e.b, e.theta],
                    t,
                ),
            },
            Constraint::EllipseAxes(e, a, b) => {
                self.linear(&[(e.a, 1.0)], a, t);
                self.linear(&[(e.b, 1.0)], b, t);
            }
            Constraint::EllipseAngle(e, theta) => self.linear(&[(e.theta, 1.0)], theta, t),
            Constraint::Radius(curve, radius) => match curve.circle() {
                Some((_, r)) => self.linear(&[(r, 1.0)], radius, t),
                None => return Err(Unsupported("radius needs a circle or an arc")),
            },
        }
        Ok(())
    }

    fn tangent(&mut self, a: Curve, b: Curve, tag: usize) -> Result<(), Unsupported> {
        let t = Some(tag);
        match (a, b) {
            (Curve::Line(l), other) | (other, Curve::Line(l)) if other.circle().is_some() => {
                let (c, r) = other.circle().ok_or(Unsupported("tangent"))?;
                let sign = self.side_sign(c, l);
                self.push(
                    Kind::TangentLineCircle { sign },
                    vec![l.a.x, l.a.y, l.b.x, l.b.y, c.x, c.y, r],
                    t,
                );
                Ok(())
            }
            (x, y) if x.circle().is_some() && y.circle().is_some() => {
                let ((c1, r1), (c2, r2)) = (
                    x.circle().ok_or(Unsupported("tangent"))?,
                    y.circle().ok_or(Unsupported("tangent"))?,
                );
                let (p, q) = (self.pt(c1), self.pt(c2));
                let d = (p.0 - q.0).hypot(p.1 - q.1);
                let (ra, rb) = (self.x0[r1], self.x0[r2]);
                let (k, s) = if (d - (ra + rb)).abs() <= (d - (ra - rb).abs()).abs() {
                    (1.0, 1.0)
                } else {
                    (-1.0, if ra >= rb { 1.0 } else { -1.0 })
                };
                self.push(
                    Kind::TangentCircles { k, s },
                    vec![c1.x, c1.y, r1, c2.x, c2.y, r2],
                    t,
                );
                Ok(())
            }
            _ => Err(Unsupported(
                "tangent needs a line and a circle/arc, or two circles/arcs",
            )),
        }
    }
}

fn center(c: Curve) -> Option<Point> {
    match c {
        Curve::Circle(c) => Some(c.c),
        Curve::Arc(a) => Some(a.c),
        Curve::Ellipse(e) => Some(e.c),
        Curve::Line(_) => None,
    }
}
