//! Sketches: turning the IR of a sketch into a solver problem, solving it, and reporting.
//!
//! Positions given as coordinates (`center=(1 mm, 2 mm)`) are *soft*: they are starting values, and
//! the solver moves them as little as the constraints allow. Dimensions given as arguments (`r`, `d`,
//! `w`, `h`, `a`, `b`, `angle` of an ellipse) and everything stated with `constrain` are *hard*.
//! References to another entity's point (`start=a.end`) share that point. Values the file leaves
//! out are taken from the entity's `@hint(...)`, then from a deterministic default.
//! See `spec/sketch.md`.

use crate::ir::{Constraint as IrConstraint, Entity, Sketch};
use std::collections::HashMap;
use tcad_diag::{Diagnostic, Span, codes};
use tcad_expr::{Arg, Dim, Env, EvalError, Expr, ExprKind, Reference, Value, eval};
use tcad_solver::{
    Arc, Circle, Constraint as C, Curve, Ellipse, Line, Point, Polygon, Problem, Rect, Slot,
};

/// Solved geometry of one entity, in millimetres and radians.
#[derive(Clone, Debug, PartialEq)]
#[allow(missing_docs)] // the variants and fields are self-describing
pub enum Geometry {
    Point([f64; 2]),
    Line {
        start: [f64; 2],
        end: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        r: f64,
    },
    /// Counterclockwise from `start` to `end` about `center`.
    Arc {
        center: [f64; 2],
        start: [f64; 2],
        end: [f64; 2],
        r: f64,
    },
    Ellipse {
        center: [f64; 2],
        a: f64,
        b: f64,
        theta: f64,
    },
    Rect {
        bottom_left: [f64; 2],
        bottom_right: [f64; 2],
        top_right: [f64; 2],
        top_left: [f64; 2],
    },
    Polygon {
        center: [f64; 2],
        r: f64,
        vertices: Vec<[f64; 2]>,
    },
    Slot {
        start: [f64; 2],
        end: [f64; 2],
        width: f64,
    },
    Spline {
        points: Vec<[f64; 2]>,
        interpolated: bool,
        closed: bool,
    },
    /// Geometry projected from the model; resolved by the kernel.
    Projected,
}

/// A solved entity.
#[derive(Clone, Debug)]
pub struct SolvedEntity {
    /// Name.
    pub name: String,
    /// Entity keyword (`line`, `circle`, ...).
    pub kind: String,
    /// Construction geometry (not part of the profile).
    pub construction: bool,
    /// Solved geometry.
    pub geometry: Geometry,
    /// Remaining degrees of freedom of this entity.
    pub dof: usize,
}

impl SolvedEntity {
    /// The values written by `tcad fmt --solve` as `@hint(...)`: `(value, is_angle)` in the
    /// entity's variable order (see `spec/sketch.md`).
    pub fn hint_values(&self) -> Vec<(f64, bool)> {
        let l = |v: f64| (v, false);
        let pt = |p: &[f64; 2]| [l(p[0]), l(p[1])];
        match &self.geometry {
            Geometry::Point(p) => pt(p).to_vec(),
            Geometry::Line { start, end } => [pt(start), pt(end)].concat(),
            Geometry::Circle { center, r } => [pt(center).to_vec(), vec![l(*r)]].concat(),
            Geometry::Arc {
                center, start, end, ..
            } => [pt(center), pt(start), pt(end)].concat(),
            Geometry::Ellipse {
                center,
                a,
                b,
                theta,
            } => [pt(center).to_vec(), vec![l(*a), l(*b), (*theta, true)]].concat(),
            Geometry::Rect {
                bottom_left,
                bottom_right,
                top_right,
                top_left,
            } => [
                pt(bottom_left),
                pt(bottom_right),
                pt(top_right),
                pt(top_left),
            ]
            .concat(),
            Geometry::Polygon {
                center,
                r,
                vertices,
            } => {
                let phase = vertices
                    .first()
                    .map_or(0.0, |v| libm::atan2(v[1] - center[1], v[0] - center[0]));
                [pt(center).to_vec(), vec![l(*r), (phase, true)]].concat()
            }
            Geometry::Slot { start, end, width } => {
                [pt(start).to_vec(), pt(end).to_vec(), vec![l(*width)]].concat()
            }
            Geometry::Spline { points, .. } => points.iter().flat_map(pt).collect(),
            Geometry::Projected => Vec::new(),
        }
    }
}

/// A solved sketch.
#[derive(Clone, Debug)]
pub struct SolvedSketch {
    /// Sketch name.
    pub name: String,
    /// Entities in declaration order.
    pub entities: Vec<SolvedEntity>,
    /// Total remaining degrees of freedom.
    pub dof: usize,
    /// True if all constraints are satisfied.
    pub solved: bool,
}

struct Values<'a>(&'a HashMap<String, Value>);

impl Env for Values<'_> {
    fn get(&self, name: &str) -> Option<Value> {
        self.0.get(name).cloned()
    }
}

#[derive(Clone)]
enum Ent {
    Point(Point),
    Line(Line),
    Circle(Circle),
    Arc(Arc),
    Ellipse(Ellipse),
    Rect(Rect),
    Polygon(Polygon),
    Slot(Slot),
    Spline(Vec<Point>, bool, bool),
    Skipped,
}

struct TagInfo {
    span: Span,
    text: String,
}

struct Build<'a> {
    p: Problem,
    env: Values<'a>,
    src: &'a str,
    diags: Vec<Diagnostic>,
    ents: HashMap<String, usize>,
    list: Vec<(String, String, bool, Ent, Option<usize>)>,
    tags: Vec<TagInfo>,
    counter: f64,
    /// Points known to coincide (from `coincident` statements), as a union-find over x variables.
    same: HashMap<usize, usize>,
}

/// How strongly a starting value resists being moved by the solver: what the author wrote with
/// coordinates is stiff, a stored hint is neutral, and values the tool had to invent are soft.
const EXPLICIT: f64 = 1.0e4;
const HINTED: f64 = 1.0;
const DERIVED: f64 = 0.01;

/// Starting values taken from an entity's `@hint(...)`.
struct Hint(Vec<f64>);

impl Hint {
    fn get(&self, i: usize) -> Option<f64> {
        self.0.get(i).copied()
    }
}

/// Only `closed` is a flag; every other entity argument needs a value.
fn is_flag(a: &Arg) -> bool {
    a.flag && a.name == "closed"
}

fn first_value(a: &Arg) -> &Expr {
    &a.values[0]
}

impl<'a> Build<'a> {
    fn err(&mut self, code: tcad_diag::Code, span: Span, msg: impl Into<String>) {
        self.diags.push(Diagnostic::error(code, span, msg));
    }

    fn snippet(&self, span: Span) -> String {
        let text = self.src.get(span.start..span.end).unwrap_or("");
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn tag(&mut self, span: Span, prefix: &str) -> usize {
        let text = if prefix.is_empty() {
            self.snippet(span)
        } else {
            format!("{prefix} {}", self.snippet(span))
        };
        self.tags.push(TagInfo { span, text });
        self.tags.len() - 1
    }

    fn add(&mut self, c: C, tag: usize, span: Span) {
        if let Err(e) = self.p.constrain(c, tag) {
            self.err(codes::E4003, span, e.0);
        }
    }

    fn value(&mut self, e: &Expr) -> Option<Value> {
        match eval(e, &self.env) {
            Ok(v) => Some(v),
            Err(EvalError::Diag(d)) => {
                self.diags.push(d);
                None
            }
            Err(EvalError::NotAValue(span, what)) => {
                self.err(codes::E4001, span, format!("`{what}` is not a value here"));
                None
            }
        }
    }

    fn quantity(&mut self, e: &Expr, dim: Dim, what: &str) -> Option<f64> {
        match self.value(e)? {
            Value::Num(q) if q.dim == dim => Some(q.value),
            other => {
                self.err(
                    codes::E4001,
                    e.span,
                    format!(
                        "{what} must be {}, found {}",
                        dim_name(dim),
                        other.type_name()
                    ),
                );
                None
            }
        }
    }

    fn xy(&mut self, e: &Expr) -> Option<(f64, f64)> {
        match self.value(e)? {
            Value::Tuple(v) if v.len() == 2 => match (&v[0], &v[1]) {
                (Value::Num(a), Value::Num(b)) if a.dim == Dim::LENGTH && b.dim == Dim::LENGTH => {
                    Some((a.value, b.value))
                }
                _ => {
                    self.err(
                        codes::E4001,
                        e.span,
                        "a point needs two lengths, like (1 mm, 2 mm)",
                    );
                    None
                }
            },
            other => {
                self.err(
                    codes::E4001,
                    e.span,
                    format!(
                        "expected a point like (1 mm, 2 mm), found {}",
                        other.type_name()
                    ),
                );
                None
            }
        }
    }

    // ---- entity references ----

    fn lookup(&self, r: &Reference) -> Option<&Ent> {
        self.ents.get(r.head()).map(|&i| &self.list[i].3)
    }

    fn point_of(ent: &Ent, role: Option<&str>) -> Result<Point, String> {
        let bad = |roles: &str| {
            Err(format!(
                "no such point; the points of this entity are {roles}"
            ))
        };
        match (ent, role) {
            (Ent::Point(p), None | Some("at")) => Ok(*p),
            (Ent::Point(_), _) => bad("(the entity itself)"),
            (Ent::Line(l), Some("start")) => Ok(l.a),
            (Ent::Line(l), Some("end")) => Ok(l.b),
            (Ent::Line(_), _) => bad("start, end"),
            (Ent::Circle(c), Some("center")) => Ok(c.c),
            (Ent::Circle(_), _) => bad("center"),
            (Ent::Arc(a), Some("center")) => Ok(a.c),
            (Ent::Arc(a), Some("start")) => Ok(a.s),
            (Ent::Arc(a), Some("end")) => Ok(a.e),
            (Ent::Arc(_), _) => bad("center, start, end"),
            (Ent::Ellipse(e), Some("center")) => Ok(e.c),
            (Ent::Ellipse(_), _) => bad("center"),
            (Ent::Rect(r), Some("bottom_left")) => Ok(r.bl),
            (Ent::Rect(r), Some("bottom_right")) => Ok(r.br),
            (Ent::Rect(r), Some("top_right")) => Ok(r.tr),
            (Ent::Rect(r), Some("top_left")) => Ok(r.tl),
            (Ent::Rect(_), _) => bad("bottom_left, bottom_right, top_right, top_left"),
            (Ent::Polygon(g), Some("center")) => Ok(g.c),
            (Ent::Polygon(g), Some(r)) => r
                .strip_prefix('v')
                .and_then(|n| n.parse::<usize>().ok())
                .and_then(|i| g.verts.get(i).copied())
                .ok_or_else(|| format!("no such vertex; use center or v0..v{}", g.verts.len() - 1)),
            (Ent::Polygon(_), None) => bad("center, v0, v1, ..."),
            (Ent::Slot(s), Some("start")) => Ok(s.a),
            (Ent::Slot(s), Some("end")) => Ok(s.b),
            (Ent::Slot(_), _) => bad("start, end"),
            (Ent::Spline(pts, _, _), Some(r)) => r
                .strip_prefix('p')
                .and_then(|n| n.parse::<usize>().ok())
                .and_then(|i| pts.get(i).copied())
                .ok_or_else(|| format!("no such point; use p0..p{}", pts.len() - 1)),
            (Ent::Spline(..), None) => bad("p0, p1, ..."),
            (Ent::Skipped, _) => Err("projected geometry is not available yet".into()),
        }
    }

    fn curve_of(ent: &Ent, role: Option<&str>) -> Result<Curve, String> {
        match (ent, role) {
            (Ent::Line(l), None) => Ok(Curve::Line(*l)),
            (Ent::Circle(c), None) => Ok(Curve::Circle(*c)),
            (Ent::Arc(a), None) => Ok(Curve::Arc(*a)),
            (Ent::Ellipse(e), None) => Ok(Curve::Ellipse(*e)),
            (Ent::Slot(s), None) => Ok(Curve::Line(Line { a: s.a, b: s.b })),
            (Ent::Rect(r), Some(edge)) => match edge {
                "bottom" => Ok(Curve::Line(Line { a: r.bl, b: r.br })),
                "right" => Ok(Curve::Line(Line { a: r.br, b: r.tr })),
                "top" => Ok(Curve::Line(Line { a: r.tr, b: r.tl })),
                "left" => Ok(Curve::Line(Line { a: r.tl, b: r.bl })),
                _ => Err("a rectangle's edges are bottom, right, top, left".into()),
            },
            (Ent::Polygon(g), Some(e)) => e
                .strip_prefix('e')
                .and_then(|n| n.parse::<usize>().ok())
                .filter(|&i| i < g.verts.len())
                .map(|i| Curve::Line(Line { a: g.verts[i], b: g.verts[(i + 1) % g.verts.len()] }))
                .ok_or_else(|| format!("a polygon's edges are e0..e{}", g.verts.len() - 1)),
            _ => Err("this is not a curve (use a line, circle, arc, ellipse, slot, or an edge such as r.bottom)".into()),
        }
    }

    /// Resolves an expression that names an entity point, if it is a reference to an entity.
    fn entity_point(&mut self, e: &Expr) -> Option<Option<Point>> {
        let ExprKind::Ref(r) = &e.kind else {
            return None;
        };
        let ent = self.lookup(r)?.clone();
        if r.segments.len() > 2 || r.segments.iter().any(|s| s.args.is_some()) {
            self.err(codes::E4003, r.span, "unsupported reference in a sketch");
            return Some(None);
        }
        let role = r.segments.get(1).map(|s| s.name.as_str());
        match Self::point_of(&ent, role) {
            Ok(p) => Some(Some(p)),
            Err(m) => {
                self.err(codes::E4003, r.span, format!("`{}`: {m}", r.dotted()));
                Some(None)
            }
        }
    }

    fn point_expr(&mut self, e: &Expr) -> Option<Point> {
        match self.entity_point(e) {
            Some(p) => p,
            None => {
                let (x, y) = self.xy(e)?;
                let p = self.p.point(x, y);
                self.p.pin_point(p, EXPLICIT);
                Some(p)
            }
        }
    }

    fn curve_expr(&mut self, e: &Expr) -> Option<Curve> {
        let ExprKind::Ref(r) = &e.kind else {
            self.err(
                codes::E4003,
                e.span,
                "expected a curve (a line, circle, arc, ...)",
            );
            return None;
        };
        let Some(ent) = self.lookup(r).cloned() else {
            self.err(
                codes::E4003,
                r.span,
                format!("`{}` is not an entity of this sketch", r.dotted()),
            );
            return None;
        };
        let role = r.segments.get(1).map(|s| s.name.as_str());
        match Self::curve_of(&ent, role) {
            Ok(c) => Some(c),
            Err(m) => {
                self.err(codes::E4003, r.span, format!("`{}`: {m}", r.dotted()));
                None
            }
        }
    }

    fn line_expr(&mut self, e: &Expr) -> Option<Line> {
        match self.curve_expr(e)? {
            Curve::Line(l) => Some(l),
            _ => {
                self.err(codes::E4003, e.span, "expected a line");
                None
            }
        }
    }

    // ---- a point argument: coordinates, a reference, or the hint, or a default ----

    fn next_default(&mut self) -> (f64, f64) {
        self.counter += 1.0;
        (self.counter, 0.0)
    }

    fn point_arg(&mut self, a: Option<&Arg>, hint: &Hint, at: usize) -> Option<Point> {
        match a {
            Some(a) => self.point_expr(first_value(a)),
            None => {
                let (x, y, weight) = match (hint.get(at), hint.get(at + 1)) {
                    (Some(x), Some(y)) => (x, y, HINTED),
                    _ => {
                        let (x, y) = self.next_default();
                        (x, y, DERIVED)
                    }
                };
                let p = self.p.point(x, y);
                self.p.set_point_weight(p, weight);
                Some(p)
            }
        }
    }

    fn register(&mut self, e: &Entity, ent: Ent, vars: Vec<usize>) {
        let idx = self.p.entity(e.name.clone(), vars);
        self.ents.insert(e.name.clone(), self.list.len());
        self.list.push((
            e.name.clone(),
            e.kind.clone(),
            e.construction,
            ent,
            Some(idx),
        ));
    }

    fn pvars(ps: &[Point]) -> Vec<usize> {
        ps.iter().flat_map(|p| [p.x, p.y]).collect()
    }

    fn entity(&mut self, e: &Entity) {
        let hint = Hint(match &e.hint {
            Some(h) => {
                let vals: Vec<f64> = h
                    .iter()
                    .filter_map(|x| match self.value(x) {
                        Some(Value::Num(q)) => Some(q.value),
                        _ => None,
                    })
                    .collect();
                vals
            }
            None => Vec::new(),
        });
        let mut args: Vec<(&Arg, bool)> = e.args.iter().map(|a| (a, false)).collect();
        let allowed: &[&str] = match e.kind.as_str() {
            "point" => &["at"],
            "line" => &["start", "end"],
            "circle" => &["center", "r", "d"],
            "arc" => &["center", "start", "end", "through", "tangent_to", "r"],
            "ellipse" => &["center", "a", "b", "angle"],
            "spline" => &["points", "through", "closed"],
            "slot" => &["start", "end", "w"],
            "rect" => &["corner", "center", "w", "h"],
            "polygon" => &["center", "sides", "r", "d", "angle"],
            "project" => &["edges"],
            _ => &[],
        };
        for (a, _) in &args {
            if !allowed.contains(&a.name.as_str()) {
                self.err(
                    codes::E4001,
                    a.span,
                    format!(
                        "`{}` does not take an argument `{}` (it takes: {})",
                        e.kind,
                        a.name,
                        allowed.join(", ")
                    ),
                );
            } else if a.values.is_empty() && !is_flag(a) {
                self.err(codes::E4001, a.span, format!("`{}` needs a value", a.name));
            }
        }
        args.retain(|(a, _)| {
            allowed.contains(&a.name.as_str()) && (is_flag(a) || !a.values.is_empty())
        });
        let get = |name: &str| args.iter().find(|(a, _)| a.name == name).map(|(a, _)| *a);
        for (a, _) in &args {
            if a.values.len() > 1 && !matches!(a.name.as_str(), "through" | "points") {
                self.err(
                    codes::E4001,
                    a.span,
                    format!("`{}` takes a single value", a.name),
                );
            }
        }
        match e.kind.as_str() {
            "point" => {
                let Some(p) = self.point_arg(get("at"), &hint, 0) else {
                    return;
                };
                self.register(e, Ent::Point(p), Self::pvars(&[p]));
            }
            "line" => {
                let (Some(a), Some(b)) = (
                    self.point_arg(get("start"), &hint, 0),
                    self.point_arg(get("end"), &hint, 2),
                ) else {
                    return;
                };
                self.register(e, Ent::Line(Line { a, b }), Self::pvars(&[a, b]));
            }
            "circle" => self.circle(e, &hint, get("center"), get("r"), get("d")),
            "arc" => self.arc(e, &hint, &get),
            "ellipse" => self.ellipse(e, &hint, &get),
            "spline" => self.spline(e, &get),
            "slot" => {
                let (Some(a), Some(b)) = (
                    self.point_arg(get("start"), &hint, 0),
                    self.point_arg(get("end"), &hint, 2),
                ) else {
                    return;
                };
                let w = match get("w") {
                    Some(arg) => self.quantity(first_value(arg), Dim::LENGTH, "the width"),
                    None => Some(hint.get(4).unwrap_or(1.0)),
                };
                let Some(w) = w else { return };
                let slot: Slot = self.p.slot(a, b, w);
                self.p.set_weight(
                    slot.w,
                    if get("w").is_some() {
                        EXPLICIT
                    } else {
                        DERIVED
                    },
                );
                if let Some(arg) = get("w") {
                    let tag = self.tag(arg.span, &format!("slot `{}`", e.name));
                    self.add(
                        C::Radius(Curve::Circle(Circle { c: a, r: slot.w }), w),
                        tag,
                        arg.span,
                    );
                }
                let mut vars = Self::pvars(&[a, b]);
                vars.push(slot.w);
                self.register(e, Ent::Slot(slot), vars);
            }
            "rect" => self.rect(e, &hint, &get),
            "polygon" => self.polygon(e, &hint, &get),
            _ => {
                self.diags.push(Diagnostic::warning(
                    codes::W4601,
                    e.span,
                    "projected geometry is resolved by the kernel; this entity is ignored by the sketch solver",
                ));
                let i = self.list.len();
                self.ents.insert(e.name.clone(), i);
                self.list.push((
                    e.name.clone(),
                    e.kind.clone(),
                    e.construction,
                    Ent::Skipped,
                    None,
                ));
            }
        }
    }

    /// The hard radius from `r` or `d`: `None` on error, `Some(None)` if neither is given.
    fn radius_args(&mut self, r: Option<&Arg>, d: Option<&Arg>) -> Option<Option<(f64, Span)>> {
        match (r, d) {
            (Some(_), Some(d)) => {
                self.err(codes::E4001, d.span, "give either `r` or `d`, not both");
                None
            }
            (Some(a), None) => self
                .quantity(first_value(a), Dim::LENGTH, "the radius")
                .map(|v| Some((v, a.span))),
            (None, Some(a)) => self
                .quantity(first_value(a), Dim::LENGTH, "the diameter")
                .map(|v| Some((v / 2.0, a.span))),
            (None, None) => Some(None),
        }
    }

    fn circle(
        &mut self,
        e: &Entity,
        hint: &Hint,
        center: Option<&Arg>,
        r: Option<&Arg>,
        d: Option<&Arg>,
    ) {
        let Some(c) = self.point_arg(center, hint, 0) else {
            return;
        };
        let Some(rad) = self.radius_args(r, d) else {
            return;
        };
        let r0 = rad.map(|x| x.0).or(hint.get(2)).unwrap_or(1.0);
        let k = self.p.circle(c, r0);
        self.p.set_weight(
            k.r,
            if rad.is_some() {
                EXPLICIT
            } else if hint.get(2).is_some() {
                HINTED
            } else {
                DERIVED
            },
        );
        if let Some((v, span)) = rad {
            let tag = self.tag(span, &format!("circle `{}`", e.name));
            self.add(C::Radius(Curve::Circle(k), v), tag, span);
        }
        let mut vars = Self::pvars(&[c]);
        vars.push(k.r);
        self.register(e, Ent::Circle(k), vars);
    }

    fn arc<'x>(&mut self, e: &Entity, hint: &Hint, get: &dyn Fn(&str) -> Option<&'x Arg>) {
        // `through=P, Q` with two values means start and end; this is how the examples write a
        // tangent arc between two points.
        let through = get("through");
        let two_through = through.is_some_and(|a| a.values.len() == 2);
        let start_arg = get("start");
        let end_arg = get("end");
        let (s, en, mid): (Option<Point>, Option<Point>, Option<&Expr>) =
            if two_through && start_arg.is_none() {
                let t = through.map(|a| &a.values);
                let t = t.expect("checked");
                (self.point_expr(&t[0]), self.point_expr(&t[1]), None)
            } else {
                (
                    self.point_arg(start_arg, hint, 2),
                    self.point_arg(end_arg, hint, 4),
                    through.and_then(|a| a.values.first()),
                )
            };
        let (Some(s), Some(en)) = (s, en) else { return };
        let (sx, sy, ex, ey) = (
            self.p.initial(s.x),
            self.p.initial(s.y),
            self.p.initial(en.x),
            self.p.initial(en.y),
        );
        let tangent = get("tangent_to");
        let center_arg = get("center");

        // Starting center: given, or from the through point, or from the tangent direction, or
        // from the hint.
        let c = if let Some(ca) = center_arg {
            self.point_expr(first_value(ca))
        } else {
            let guess = if let Some(m) = mid {
                let mp = match self.entity_point(m) {
                    Some(Some(p)) => Some((self.p.initial(p.x), self.p.initial(p.y))),
                    Some(None) => return,
                    None => self.xy(m),
                };
                mp.and_then(|(mx, my)| circumcenter((sx, sy), (mx, my), (ex, ey)))
            } else if let Some(t) = tangent {
                self.tangent_center(t, (sx, sy), (ex, ey))
            } else {
                None
            };
            let guess = guess.or(match (hint.get(0), hint.get(1)) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            });
            let (x, y) = guess.unwrap_or(((sx + ex) / 2.0, (sy + ey) / 2.0 + 1e-3));
            let p = self.p.point(x, y);
            self.p.set_point_weight(p, DERIVED);
            Some(p)
        };
        let Some(c) = c else { return };
        let arc = self.p.arc(c, s, en);
        self.p.set_weight(arc.r, DERIVED);
        if let Some(ra) = get("r")
            && let Some(v) = self.quantity(first_value(ra), Dim::LENGTH, "the radius")
        {
            let tag = self.tag(ra.span, &format!("arc `{}`", e.name));
            self.add(C::Radius(Curve::Arc(arc), v), tag, ra.span);
        }
        // References (not coordinates) are hard: the arc passes through the point / is tangent.
        if let Some(m) = mid
            && matches!(m.kind, ExprKind::Ref(_))
            && let Some(Some(p)) = self.entity_point(m)
        {
            let tag = self.tag(m.span, &format!("arc `{}` through", e.name));
            self.add(C::PointOnCurve(p, Curve::Arc(arc)), tag, m.span);
        }
        if let Some(t) = tangent
            && let Some(tv) = t.values.first()
            && let Some(curve) = self.curve_expr(tv)
        {
            let tag = self.tag(t.span, &format!("arc `{}`", e.name));
            self.add(C::PointOnCurve(s, curve), tag, t.span);
            // the arc starts on the curve, so the start is the contact point
            self.add(C::TangentAt(Curve::Arc(arc), curve, s), tag, t.span);
        }
        let mut vars = Self::pvars(&[c, s, en]);
        vars.push(arc.r);
        self.register(e, Ent::Arc(arc), vars);
    }

    /// Starting center of an arc that begins at `s`, ends at `en` and is tangent to a curve at `s`.
    fn tangent_center(&mut self, t: &Arg, s: (f64, f64), en: (f64, f64)) -> Option<(f64, f64)> {
        let curve = self.curve_expr(t.values.first()?)?;
        let dir = match curve {
            Curve::Line(l) => (
                self.p.initial(l.b.x) - self.p.initial(l.a.x),
                self.p.initial(l.b.y) - self.p.initial(l.a.y),
            ),
            Curve::Circle(Circle { c, .. }) | Curve::Arc(Arc { c, .. }) => {
                let (rx, ry) = (s.0 - self.p.initial(c.x), s.1 - self.p.initial(c.y));
                (-ry, rx)
            }
            Curve::Ellipse(_) => return None,
        };
        let len = libm::hypot(dir.0, dir.1);
        if len == 0.0 {
            return None;
        }
        let n = (-dir.1 / len, dir.0 / len);
        let (vx, vy) = (en.0 - s.0, en.1 - s.1);
        let denom = 2.0 * (vx * n.0 + vy * n.1);
        if denom.abs() < 1e-12 {
            return None;
        }
        let rho = (vx * vx + vy * vy) / denom;
        Some((s.0 + n.0 * rho, s.1 + n.1 * rho))
    }

    fn ellipse<'x>(&mut self, e: &Entity, hint: &Hint, get: &dyn Fn(&str) -> Option<&'x Arg>) {
        let Some(c) = self.point_arg(get("center"), hint, 0) else {
            return;
        };
        let (ga, gb, gt) = (get("a"), get("b"), get("angle"));
        if ga.is_some() != gb.is_some() {
            self.err(
                codes::E4001,
                e.span,
                "an ellipse needs both `a` and `b`, or neither",
            );
            return;
        }
        let a = ga.and_then(|x| self.quantity(first_value(x), Dim::LENGTH, "the semi-axis `a`"));
        let b = gb.and_then(|x| self.quantity(first_value(x), Dim::LENGTH, "the semi-axis `b`"));
        let th = gt.and_then(|x| self.quantity(first_value(x), Dim::ANGLE, "the rotation"));
        if (ga.is_some() && (a.is_none() || b.is_none())) || (gt.is_some() && th.is_none()) {
            return;
        }
        let soft = if hint.get(2).is_some() {
            HINTED
        } else {
            DERIVED
        };
        let ell: Ellipse = self.p.ellipse(
            c,
            a.or(hint.get(2)).unwrap_or(2.0),
            b.or(hint.get(3)).unwrap_or(1.0),
            th.or(hint.get(4)).unwrap_or(0.0),
        );
        for v in [ell.a, ell.b, ell.theta] {
            self.p.set_weight(v, soft);
        }
        if let (Some(a), Some(b), Some(arg)) = (a, b, ga) {
            let tag = self.tag(arg.span, &format!("ellipse `{}` axes", e.name));
            self.add(C::EllipseAxes(ell, a, b), tag, arg.span);
        }
        if let (Some(t), Some(arg)) = (th, gt) {
            let tag = self.tag(arg.span, &format!("ellipse `{}`", e.name));
            self.add(C::EllipseAngle(ell, t), tag, arg.span);
        }
        let mut vars = Self::pvars(&[c]);
        vars.extend([ell.a, ell.b, ell.theta]);
        self.register(e, Ent::Ellipse(ell), vars);
    }

    fn spline<'x>(&mut self, e: &Entity, get: &dyn Fn(&str) -> Option<&'x Arg>) {
        let (pts, interpolated) = match (get("points"), get("through")) {
            (Some(a), None) => (a, false),
            (None, Some(a)) => (a, true),
            _ => {
                self.err(
                    codes::E4001,
                    e.span,
                    "a spline needs `points` (control points) or `through` (interpolated)",
                );
                return;
            }
        };
        let values: Vec<&Expr> = pts
            .values
            .iter()
            .flat_map(|v| match &v.kind {
                ExprKind::List(items) => items.iter().collect::<Vec<_>>(),
                _ => vec![v],
            })
            .collect();
        if values.len() < 2 {
            self.err(codes::E4001, pts.span, "a spline needs at least two points");
            return;
        }
        let points: Vec<Point> = values.iter().filter_map(|v| self.point_expr(v)).collect();
        if points.len() != values.len() {
            return;
        }
        let closed = get("closed").is_some();
        let vars = Self::pvars(&points);
        self.register(e, Ent::Spline(points, interpolated, closed), vars);
    }

    fn rect<'x>(&mut self, e: &Entity, hint: &Hint, get: &dyn Fn(&str) -> Option<&'x Arg>) {
        let (gw, gh) = (get("w"), get("h"));
        let w = gw.and_then(|a| self.quantity(first_value(a), Dim::LENGTH, "the width"));
        let h = gh.and_then(|a| self.quantity(first_value(a), Dim::LENGTH, "the height"));
        if (gw.is_some() && w.is_none()) || (gh.is_some() && h.is_none()) {
            return;
        }
        let from_hint = hint.0.len() >= 8;
        let (w0, h0) = match (w, h) {
            (Some(w), Some(h)) => (w, h),
            _ if from_hint => (hint.0[2] - hint.0[0], hint.0[7] - hint.0[1]),
            (w, h) => (w.unwrap_or(10.0), h.unwrap_or(10.0)),
        };
        let origin = if let Some(c) = get("corner") {
            self.xy(first_value(c))
        } else if let Some(c) = get("center") {
            self.xy(first_value(c))
                .map(|(x, y)| (x - w0 / 2.0, y - h0 / 2.0))
        } else if from_hint {
            Some((hint.0[0], hint.0[1]))
        } else {
            let d = self.next_default();
            Some((d.0, d.1))
        };
        let Some((x0, y0)) = origin else { return };
        let r = self.p.rect(x0, y0, x0 + w0, y0 + h0);
        let weight = if get("corner").is_some() || get("center").is_some() {
            EXPLICIT
        } else if from_hint {
            HINTED
        } else {
            DERIVED
        };
        for pt in [r.bl, r.br, r.tr, r.tl] {
            if weight == EXPLICIT {
                self.p.pin_point(pt, weight);
            } else {
                self.p.set_point_weight(pt, weight);
            }
        }
        if let (Some(w), Some(arg)) = (w, gw) {
            let tag = self.tag(arg.span, &format!("rect `{}`", e.name));
            self.add(C::HDistance(r.bl, r.br, w), tag, arg.span);
        }
        if let (Some(h), Some(arg)) = (h, gh) {
            let tag = self.tag(arg.span, &format!("rect `{}`", e.name));
            self.add(C::VDistance(r.bl, r.tl, h), tag, arg.span);
        }
        let vars = Self::pvars(&[r.bl, r.br, r.tr, r.tl]);
        self.register(e, Ent::Rect(r), vars);
    }

    fn polygon<'x>(&mut self, e: &Entity, hint: &Hint, get: &dyn Fn(&str) -> Option<&'x Arg>) {
        let Some(sides_arg) = get("sides") else {
            self.err(codes::E4001, e.span, "a polygon needs `sides=<count>`");
            return;
        };
        let n = match self.value(first_value(sides_arg)) {
            Some(Value::Num(q))
                if q.dim.is_none()
                    && q.value.fract() == 0.0
                    && (3.0..=256.0).contains(&q.value) =>
            {
                q.value as usize
            }
            Some(_) => {
                self.err(
                    codes::E4001,
                    sides_arg.span,
                    "`sides` must be a whole number from 3 to 256",
                );
                return;
            }
            None => return,
        };
        let center = get("center").and_then(|c| self.xy(first_value(c)));
        if get("center").is_some() && center.is_none() {
            return;
        }
        let Some(rad) = self.radius_args(get("r"), get("d")) else {
            return;
        };
        let phase_arg =
            get("angle").and_then(|a| self.quantity(first_value(a), Dim::ANGLE, "the rotation"));
        if get("angle").is_some() && phase_arg.is_none() {
            return;
        }
        let (cx, cy) = center
            .or(match (hint.get(0), hint.get(1)) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            })
            .unwrap_or_else(|| self.next_default());
        let r0 = rad.map(|x| x.0).or(hint.get(2)).unwrap_or(5.0);
        let phase = phase_arg.or(hint.get(3)).unwrap_or(0.0);
        let g: Polygon = self.p.polygon(n, cx, cy, r0, phase);
        self.p
            .set_weight(g.r, if rad.is_some() { EXPLICIT } else { DERIVED });
        if center.is_some() {
            self.p.pin_point(g.c, EXPLICIT);
        } else {
            let w = if hint.get(0).is_some() {
                HINTED
            } else {
                DERIVED
            };
            self.p.set_point_weight(g.c, w);
        }
        for v in &g.verts {
            self.p.set_point_weight(*v, DERIVED);
        }
        if let Some((v, span)) = rad {
            let tag = self.tag(span, &format!("polygon `{}`", e.name));
            self.add(
                C::Radius(Curve::Circle(Circle { c: g.c, r: g.r }), v),
                tag,
                span,
            );
        }
        let mut vars = Self::pvars(&[g.c]);
        vars.push(g.r);
        vars.extend(Self::pvars(&g.verts));
        self.register(e, Ent::Polygon(g), vars);
    }

    // ---- constraints ----

    fn find(&mut self, v: usize) -> usize {
        let mut root = v;
        while let Some(&p) = self.same.get(&root) {
            if p == root {
                break;
            }
            root = p;
        }
        root
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.same.insert(ra.max(rb), ra.min(rb));
        }
    }

    /// Learns which points coincide (shared variables or `coincident`) before tangents are built.
    fn learn_coincidences(&mut self, constraints: &[IrConstraint]) {
        for c in constraints
            .iter()
            .filter(|c| c.kind == "coincident" && c.args.len() == 2)
        {
            if let (Some(p), Some(q)) = (
                self.entity_point_quiet(&c.args[0]),
                self.entity_point_quiet(&c.args[1]),
            ) {
                self.union(p.x, q.x);
            }
        }
    }

    fn same_point(&mut self, p: Point, q: Point) -> bool {
        p == q || self.find(p.x) == self.find(q.x)
    }

    /// The end point two curves share, if they have one.
    fn contact(&mut self, a: Curve, b: Curve) -> Option<Point> {
        let ends = |c: Curve| match c {
            Curve::Line(l) => vec![l.a, l.b],
            Curve::Arc(arc) => vec![arc.s, arc.e],
            _ => Vec::new(),
        };
        let (ea, eb) = (ends(a), ends(b));
        for &p in &ea {
            for &q in &eb {
                if self.same_point(p, q) {
                    // prefer the arc's own end point as the contact
                    return Some(if matches!(b, Curve::Arc(_)) { q } else { p });
                }
            }
        }
        None
    }

    fn constraint(&mut self, c: &IrConstraint) {
        let tag = self.tag(c.span, "");
        let span = c.span;
        let n = c.args.len();
        let arity = |this: &mut Self, want: &str, ok: bool| -> bool {
            if !ok {
                this.err(
                    codes::E4003,
                    span,
                    format!(
                        "`{}` takes {want}, found {n} argument{}",
                        c.kind,
                        if n == 1 { "" } else { "s" }
                    ),
                );
            }
            ok
        };
        let dimensional = matches!(
            c.kind.as_str(),
            "distance" | "hdistance" | "vdistance" | "radius" | "diameter" | "angle"
        );
        if dimensional && c.value.is_none() {
            self.err(
                codes::E4003,
                span,
                format!(
                    "`{}` is a dimension and needs a value, e.g. `{}(...) = 10 mm`",
                    c.kind, c.kind
                ),
            );
            return;
        }
        if !dimensional && c.value.is_some() {
            self.err(codes::E4003, span, format!("`{}` takes no value", c.kind));
            return;
        }
        let a = &c.args;
        let built: Option<C> = match c.kind.as_str() {
            "coincident" if arity(self, "two points", n == 2) => {
                self.two_points(a).map(|(p, q)| C::Coincident(p, q))
            }
            "horizontal" | "vertical" => {
                let horizontal = c.kind == "horizontal";
                let pair = if n == 1 {
                    self.line_expr(&a[0]).map(|l| (l.a, l.b))
                } else if arity(self, "a line or two points", n == 2) {
                    self.two_points(a)
                } else {
                    None
                };
                pair.map(|(p, q)| {
                    if horizontal {
                        C::Horizontal(p, q)
                    } else {
                        C::Vertical(p, q)
                    }
                })
            }
            "parallel" | "perpendicular" if arity(self, "two lines", n == 2) => {
                match (self.line_expr(&a[0]), self.line_expr(&a[1])) {
                    (Some(l), Some(m)) if c.kind == "parallel" => Some(C::Parallel(l, m)),
                    (Some(l), Some(m)) => Some(C::Perpendicular(l, m)),
                    _ => None,
                }
            }
            "equal" if arity(self, "two curves", n == 2) => {
                match (self.curve_expr(&a[0]), self.curve_expr(&a[1])) {
                    (Some(Curve::Line(l)), Some(Curve::Line(m))) => Some(C::EqualLength(l, m)),
                    (Some(x), Some(y))
                        if !matches!(x, Curve::Line(_)) && !matches!(y, Curve::Line(_)) =>
                    {
                        Some(C::EqualRadius(x, y))
                    }
                    (Some(_), Some(_)) => {
                        self.err(
                            codes::E4003,
                            span,
                            "`equal` compares two lines (length) or two circles/arcs (radius)",
                        );
                        None
                    }
                    _ => None,
                }
            }
            "tangent" | "concentric" if arity(self, "two curves", n == 2) => {
                match (self.curve_expr(&a[0]), self.curve_expr(&a[1])) {
                    (Some(x), Some(y)) if c.kind == "tangent" => match self.contact(x, y) {
                        Some(at) => Some(C::TangentAt(x, y, at)),
                        None => Some(C::Tangent(x, y)),
                    },
                    (Some(x), Some(y)) => Some(C::Concentric(x, y)),
                    _ => None,
                }
            }
            "midpoint" if arity(self, "a point and a line", n == 2) => {
                match (self.point_expr_existing(&a[0]), self.line_expr(&a[1])) {
                    (Some(p), Some(l)) => Some(C::Midpoint(p, l)),
                    _ => None,
                }
            }
            "symmetric" if arity(self, "two points and a line", n == 3) => {
                match (
                    self.point_expr_existing(&a[0]),
                    self.point_expr_existing(&a[1]),
                    self.line_expr(&a[2]),
                ) {
                    (Some(p), Some(q), Some(l)) => Some(C::Symmetric(p, q, l)),
                    _ => None,
                }
            }
            "fix" if arity(self, "one point", n == 1) => self
                .point_expr_existing(&a[0])
                .map(|p| C::Fix(p, self.p.initial(p.x), self.p.initial(p.y))),
            "on_curve" | "point_on_curve" if arity(self, "a point and a curve", n == 2) => {
                match (self.point_expr_existing(&a[0]), self.curve_expr(&a[1])) {
                    (Some(p), Some(cv)) => Some(C::PointOnCurve(p, cv)),
                    _ => None,
                }
            }
            "distance" => self.dist(c),
            "hdistance" | "vdistance" if arity(self, "two points", n == 2) => {
                let d = c
                    .value
                    .as_ref()
                    .and_then(|v| self.quantity(v, Dim::LENGTH, "the distance"));
                match (self.two_points(a), d) {
                    (Some((p, q)), Some(d)) if c.kind == "hdistance" => Some(C::HDistance(p, q, d)),
                    (Some((p, q)), Some(d)) => Some(C::VDistance(p, q, d)),
                    _ => None,
                }
            }
            "radius" | "diameter" if arity(self, "one circle or arc", n == 1) => {
                let v = c
                    .value
                    .as_ref()
                    .and_then(|v| self.quantity(v, Dim::LENGTH, "the dimension"));
                match (self.curve_expr(&a[0]), v) {
                    (Some(cv), Some(v)) => Some(C::Radius(
                        cv,
                        if c.kind == "diameter" { v / 2.0 } else { v },
                    )),
                    _ => None,
                }
            }
            "angle" if arity(self, "two lines", n == 2) => {
                let v = c
                    .value
                    .as_ref()
                    .and_then(|v| self.quantity(v, Dim::ANGLE, "the angle"));
                match (self.line_expr(&a[0]), self.line_expr(&a[1]), v) {
                    (Some(l), Some(m), Some(v)) => Some(C::Angle(l, m, v)),
                    _ => None,
                }
            }
            "coincident" | "parallel" | "perpendicular" | "equal" | "tangent" | "concentric"
            | "midpoint" | "symmetric" | "fix" | "on_curve" | "point_on_curve" | "hdistance"
            | "vdistance" | "radius" | "diameter" | "angle" => None, // arity already reported
            other => {
                self.err(
                    codes::E4002,
                    span,
                    format!(
                        "unknown constraint `{other}`; known: coincident, horizontal, vertical, parallel, perpendicular, \
                         tangent, equal, symmetric, concentric, midpoint, fix, on_curve, distance, hdistance, vdistance, \
                         radius, diameter, angle"
                    ),
                );
                None
            }
        };
        if let Some(built) = built {
            self.add(built, tag, span);
        }
    }

    /// A point that must already exist (a constraint never creates geometry).
    fn point_expr_existing(&mut self, e: &Expr) -> Option<Point> {
        match self.entity_point(e) {
            Some(p) => p,
            None => {
                self.err(
                    codes::E4003,
                    e.span,
                    "expected a point of the sketch, like `a.end` or `c.center`",
                );
                None
            }
        }
    }

    fn two_points(&mut self, a: &[Expr]) -> Option<(Point, Point)> {
        match (
            self.point_expr_existing(&a[0]),
            self.point_expr_existing(&a[1]),
        ) {
            (Some(p), Some(q)) => Some((p, q)),
            _ => None,
        }
    }

    fn dist(&mut self, c: &IrConstraint) -> Option<C> {
        let d = c
            .value
            .as_ref()
            .and_then(|v| self.quantity(v, Dim::LENGTH, "the distance"))?;
        match c.args.len() {
            1 => {
                let l = self.line_expr(&c.args[0])?;
                Some(C::Distance(l.a, l.b, d))
            }
            2 => {
                // point-point, or point-line
                let first_is_point = self.entity_point_quiet(&c.args[0]).is_some();
                let second_is_point = self.entity_point_quiet(&c.args[1]).is_some();
                if first_is_point && second_is_point {
                    let (p, q) = self.two_points(&c.args)?;
                    Some(C::Distance(p, q, d))
                } else if first_is_point {
                    let p = self.point_expr_existing(&c.args[0])?;
                    let l = self.line_expr(&c.args[1])?;
                    Some(C::DistancePointLine(p, l, d))
                } else {
                    let p = self.point_expr_existing(&c.args[1])?;
                    let l = self.line_expr(&c.args[0])?;
                    Some(C::DistancePointLine(p, l, d))
                }
            }
            n => {
                self.err(
                    codes::E4003,
                    c.span,
                    format!("`distance` takes a line, two points, or a point and a line, found {n} arguments"),
                );
                None
            }
        }
    }

    /// Like `entity_point` but silent: used to classify an argument.
    fn entity_point_quiet(&self, e: &Expr) -> Option<Point> {
        let ExprKind::Ref(r) = &e.kind else {
            return None;
        };
        let ent = self.lookup(r)?;
        Self::point_of(ent, r.segments.get(1).map(|s| s.name.as_str())).ok()
    }
}

fn dim_name(d: Dim) -> String {
    d.to_string()
}

/// The center of the circle through three points.
fn circumcenter(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<(f64, f64)> {
    let d = 2.0 * (a.0 * (b.1 - c.1) + b.0 * (c.1 - a.1) + c.0 * (a.1 - b.1));
    if d.abs() < 1e-12 {
        return None;
    }
    let (a2, b2, c2) = (
        a.0 * a.0 + a.1 * a.1,
        b.0 * b.0 + b.1 * b.1,
        c.0 * c.0 + c.1 * c.1,
    );
    let x = (a2 * (b.1 - c.1) + b2 * (c.1 - a.1) + c2 * (a.1 - b.1)) / d;
    let y = (a2 * (c.0 - b.0) + b2 * (a.0 - c.0) + c2 * (b.0 - a.0)) / d;
    Some((x, y))
}

/// What to report besides errors.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Report {
    /// Errors, conflicts and redundancy only.
    Problems,
    /// Also an informational note when the sketch is under-constrained.
    WithDegreesOfFreedom,
}

/// Solves one sketch with the given parameter values and appends diagnostics.
pub fn solve_sketch(
    name: &str,
    name_span: Span,
    sketch: &Sketch,
    params: &HashMap<String, Value>,
    src: &str,
    report: Report,
    diags: &mut Vec<Diagnostic>,
) -> SolvedSketch {
    let mut b = Build {
        p: Problem::new(),
        env: Values(params),
        src,
        diags: Vec::new(),
        ents: HashMap::new(),
        list: Vec::new(),
        tags: Vec::new(),
        counter: 0.0,
        same: HashMap::new(),
    };
    for e in &sketch.entities {
        b.entity(e);
    }
    b.learn_coincidences(&sketch.constraints);
    for c in &sketch.constraints {
        b.constraint(c);
    }
    let sol = b.p.solve();
    let analysis = b.p.analyze(&sol);
    let mut out = std::mem::take(&mut b.diags);

    for conflict in &sol.conflicts {
        match conflict.tags.split_last() {
            Some((&last, rest)) => {
                let names: Vec<String> = conflict
                    .tags
                    .iter()
                    .map(|&t| format!("`{}`", b.tags[t].text))
                    .collect();
                let mut d = Diagnostic::error(
                    codes::E4101,
                    b.tags[last].span,
                    format!(
                        "sketch `{name}`: these constraints cannot all be satisfied: {}",
                        names.join(" and ")
                    ),
                );
                for &t in rest {
                    d = d.with_related(
                        b.tags[t].span,
                        format!("conflicts with `{}`", b.tags[last].text),
                    );
                }
                out.push(d);
            }
            None => out.push(Diagnostic::error(
                codes::E4301,
                name_span,
                format!("sketch `{name}` cannot be solved near its starting geometry"),
            )),
        }
    }
    if sol.ok() {
        for &t in &analysis.redundant {
            out.push(Diagnostic::warning(
                codes::W4102,
                b.tags[t].span,
                format!(
                    "`{}` is redundant: the other constraints already imply it",
                    b.tags[t].text
                ),
            ));
        }
        if report == Report::WithDegreesOfFreedom && analysis.dof > 0 {
            let parts: Vec<String> = b
                .list
                .iter()
                .filter_map(|(n, _, _, _, idx)| idx.map(|i| (n, analysis.entity_dof[i])))
                .filter(|(_, d)| *d > 0)
                .take(8)
                .map(|(n, d)| format!("{n}: {d}"))
                .collect();
            out.push(Diagnostic::info(
                codes::I4201,
                name_span,
                format!(
                    "sketch `{name}` is under-constrained: {} degree{} of freedom ({})",
                    analysis.dof,
                    if analysis.dof == 1 { "" } else { "s" },
                    parts.join(", ")
                ),
            ));
        }
    }
    diags.append(&mut out);

    let pt = |p: Point| [sol.x[p.x], sol.x[p.y]];
    let entities = b
        .list
        .iter()
        .map(|(n, kind, construction, ent, idx)| {
            let geometry = match ent {
                Ent::Point(p) => Geometry::Point(pt(*p)),
                Ent::Line(l) => Geometry::Line {
                    start: pt(l.a),
                    end: pt(l.b),
                },
                Ent::Circle(c) => Geometry::Circle {
                    center: pt(c.c),
                    r: sol.x[c.r],
                },
                Ent::Arc(a) => Geometry::Arc {
                    center: pt(a.c),
                    start: pt(a.s),
                    end: pt(a.e),
                    r: sol.x[a.r],
                },
                Ent::Ellipse(e) => Geometry::Ellipse {
                    center: pt(e.c),
                    a: sol.x[e.a],
                    b: sol.x[e.b],
                    theta: sol.x[e.theta],
                },
                Ent::Rect(r) => Geometry::Rect {
                    bottom_left: pt(r.bl),
                    bottom_right: pt(r.br),
                    top_right: pt(r.tr),
                    top_left: pt(r.tl),
                },
                Ent::Polygon(g) => Geometry::Polygon {
                    center: pt(g.c),
                    r: sol.x[g.r],
                    vertices: g.verts.iter().map(|&v| pt(v)).collect(),
                },
                Ent::Slot(s) => Geometry::Slot {
                    start: pt(s.a),
                    end: pt(s.b),
                    width: sol.x[s.w],
                },
                Ent::Spline(pts, interpolated, closed) => Geometry::Spline {
                    points: pts.iter().map(|&p| pt(p)).collect(),
                    interpolated: *interpolated,
                    closed: *closed,
                },
                Ent::Skipped => Geometry::Projected,
            };
            SolvedEntity {
                name: n.clone(),
                kind: kind.clone(),
                construction: *construction,
                geometry,
                dof: idx.map_or(0, |i| analysis.entity_dof[i]),
            }
        })
        .collect();
    SolvedSketch {
        name: name.to_string(),
        entities,
        dof: analysis.dof,
        solved: sol.ok(),
    }
}
