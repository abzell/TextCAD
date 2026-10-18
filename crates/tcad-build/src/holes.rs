//! The `hole` feature: simple, counterbore, countersink and tapped holes.

use crate::{Cx, Res};
use tcad_diag::codes;
use tcad_expr::{Dim, ExprKind, Value};
use tcad_kernel::geometry::{Frame, Loop, P2, Profile, Segment, add, cross, scale};
use tcad_kernel::{BoolOp, Kernel, Role};
use tcad_model::{Geometry, Item};

/// How far a hole tool sticks out above the surface so the cut is clean (mm).
const OVERSHOOT: f64 = 0.5;

/// Metric coarse pitches (mm) for `M8`-style designations.
const METRIC_COARSE: &[(f64, f64)] = &[
    (3.0, 0.5),
    (4.0, 0.7),
    (5.0, 0.8),
    (6.0, 1.0),
    (8.0, 1.25),
    (10.0, 1.5),
    (12.0, 1.75),
    (14.0, 2.0),
    (16.0, 2.0),
    (20.0, 2.5),
    (24.0, 3.0),
    (30.0, 3.5),
];

/// Tap drill diameter (mm) for a thread designation, if it is `M8`, `M8x1.25` or `1/2-13 UNC`.
pub(crate) fn tap_drill(thread: &str) -> Option<f64> {
    let t = thread.trim();
    if let Some(rest) = t.strip_prefix('M') {
        let (nom, pitch) = match rest.split_once('x') {
            Some((n, p)) => (n.parse::<f64>().ok()?, p.parse::<f64>().ok()?),
            None => {
                let n = rest.parse::<f64>().ok()?;
                let p = METRIC_COARSE.iter().find(|(d, _)| (*d - n).abs() < 1e-9)?.1;
                (n, p)
            }
        };
        return Some(nom - pitch);
    }
    // inch series: "1/2-13 UNC", "0.5-13 UNC"
    let head = t.split_whitespace().next()?;
    let (size, tpi) = head.split_once('-')?;
    let inches = match size.split_once('/') {
        Some((a, b)) => a.parse::<f64>().ok()? / b.parse::<f64>().ok()?,
        None => size.parse::<f64>().ok()?,
    };
    let tpi: f64 = tpi.parse().ok()?;
    Some((inches - 1.0 / tpi) * 25.4)
}

impl<K: Kernel> Cx<'_, K> {
    fn point2(&mut self, e: &tcad_expr::Expr) -> Res<P2> {
        match self.value(e)? {
            Value::Tuple(t) if t.len() == 2 => match (&t[0], &t[1]) {
                (Value::Num(a), Value::Num(b)) if a.dim == Dim::LENGTH && b.dim == Dim::LENGTH => {
                    Ok([a.value, b.value])
                }
                _ => self.fail(
                    codes::E5001,
                    e.span,
                    "expected two lengths like (10 mm, 5 mm)",
                ),
            },
            _ => self.fail(
                codes::E5001,
                e.span,
                "expected a position like (10 mm, 5 mm)",
            ),
        }
    }

    pub(crate) fn hole(&mut self, item: &Item) -> Res {
        self.only(
            item,
            &[
                "on",
                "at",
                "from",
                "type",
                "d",
                "depth",
                "end",
                "cbore_d",
                "cbore_depth",
                "csk_d",
                "csk_angle",
                "thread",
                "flip",
            ],
        )?;
        if self.body.is_none() {
            return self.fail(codes::E5006, item.span, "`hole` needs an existing body");
        }
        // where: a frame (a face of the body, a plane or the plane of a sketch) and positions in it
        let mut frame: Option<Frame> = None;
        if let Some(a) = Self::arg(item, "on") {
            frame = Some(self.plane_expr(&a.values[0])?);
        }
        let mut positions: Vec<P2> = Vec::new();
        if let Some(a) = Self::arg(item, "at") {
            for e in Self::flat(a) {
                positions.push(self.point2(e)?);
            }
        }
        if let Some(a) = Self::arg(item, "from") {
            let ExprKind::Ref(r) = &a.values[0].kind else {
                return self.fail(codes::E5002, a.span, "`from` names a sketch of points");
            };
            let name = r.segments[0].name.clone();
            let Some(sk) = self.sketches.get(&name) else {
                return self.fail(codes::E5002, r.span, format!("`{name}` is not a sketch"));
            };
            for e in &sk.entities {
                if let Geometry::Point(p) = &e.geometry {
                    positions.push(*p);
                }
            }
            if frame.is_none() {
                frame = Some(
                    self.sketch_frames
                        .get(&name)
                        .copied()
                        .unwrap_or_else(Frame::xy),
                );
            }
        }
        let Some(mut frame) = frame else {
            return self.fail(
                codes::E5001,
                item.span,
                "`hole` needs `on=<face or plane>` (or `from=<sketch>`)",
            );
        };
        if positions.is_empty() {
            return self.fail(
                codes::E5001,
                item.span,
                "`hole` needs `at=(x, y)` or `from=<sketch of points>`",
            );
        }
        if Self::arg(item, "flip").is_some() {
            frame = Frame {
                origin: frame.origin,
                x: frame.x,
                y: scale(frame.y, -1.0),
                z: scale(frame.z, -1.0),
            };
        }
        // what: kind and dimensions
        let thread = match Self::arg(item, "thread") {
            Some(a) => match self.value(&a.values[0])? {
                Value::Str(s) => Some(s),
                _ => {
                    return self.fail(
                        codes::E5001,
                        a.span,
                        "`thread` is a string like \"M8x1.25\"",
                    );
                }
            },
            None => None,
        };
        let kind = match Self::arg(item, "type") {
            Some(a) => match &a.values[0].kind {
                ExprKind::Ref(r) => r.simple_name().unwrap_or("").to_string(),
                _ => String::new(),
            },
            None if thread.is_some() => "tapped".to_string(),
            None if Self::arg(item, "cbore_d").is_some() => "counterbore".to_string(),
            None if Self::arg(item, "csk_d").is_some() => "countersink".to_string(),
            None => "simple".to_string(),
        };
        let diameter = match Self::arg(item, "d") {
            Some(a) => self.quantity(a, Dim::LENGTH, "the hole diameter")?,
            None => match thread.as_deref().and_then(tap_drill) {
                Some(d) => d,
                None => {
                    return self.fail(
                        codes::E5001,
                        item.span,
                        "`hole` needs `d=<diameter>` (or a `thread` the tap drill can be derived from)",
                    );
                }
            },
        };
        if diameter <= 0.0 {
            return self.fail(
                codes::E5001,
                item.span,
                "the hole diameter must be positive",
            );
        }
        let r = diameter / 2.0;
        let through = match (Self::arg(item, "end"), Self::arg(item, "depth")) {
            (Some(a), _) => match &a.values[0].kind {
                ExprKind::Ref(x) if x.simple_name() == Some("through_all") => true,
                ExprKind::Ref(x) if x.simple_name() == Some("blind") => false,
                _ => return self.fail(codes::E2501, a.span, "`end` is through_all or blind"),
            },
            (None, Some(_)) => false,
            (None, None) => true,
        };
        let depth = if through {
            2.0 * self.body_diagonal() + 1.0
        } else {
            match Self::arg(item, "depth") {
                Some(a) => self.quantity(a, Dim::LENGTH, "the depth")?,
                None => {
                    return self.fail(codes::E5001, item.span, "a blind hole needs `depth=...`");
                }
            }
        };
        if depth <= 0.0 {
            return self.fail(codes::E5001, item.span, "the hole depth must be positive");
        }
        let e = OVERSHOOT;
        // half-section (radial u, height v above the surface), counterclockwise, axis first
        let (pts, roles): (Vec<P2>, Vec<Option<&str>>) = match kind.as_str() {
            "simple" | "tapped" => (
                vec![[0.0, e], [0.0, -depth], [r, -depth], [r, e]],
                vec![None, Some("floor"), Some("wall"), None],
            ),
            "counterbore" => {
                let (Some(cd), Some(cdepth)) =
                    (Self::arg(item, "cbore_d"), Self::arg(item, "cbore_depth"))
                else {
                    return self.fail(
                        codes::E5001,
                        item.span,
                        "a counterbore needs `cbore_d` and `cbore_depth`",
                    );
                };
                let big = self.quantity(cd, Dim::LENGTH, "the counterbore diameter")? / 2.0;
                let bd = self.quantity(cdepth, Dim::LENGTH, "the counterbore depth")?;
                if big <= r || bd <= 0.0 || bd >= depth {
                    return self.fail(
                        codes::E5001,
                        item.span,
                        "a counterbore must be wider than the hole and shallower than it",
                    );
                }
                (
                    vec![
                        [0.0, e],
                        [0.0, -depth],
                        [r, -depth],
                        [r, -bd],
                        [big, -bd],
                        [big, e],
                    ],
                    vec![
                        None,
                        Some("floor"),
                        Some("wall"),
                        Some("cbore_floor"),
                        Some("cbore_wall"),
                        None,
                    ],
                )
            }
            "countersink" => {
                let Some(cd) = Self::arg(item, "csk_d") else {
                    return self.fail(codes::E5001, item.span, "a countersink needs `csk_d`");
                };
                let rs = self.quantity(cd, Dim::LENGTH, "the countersink diameter")? / 2.0;
                let half = match Self::arg(item, "csk_angle") {
                    Some(a) => self.quantity(a, Dim::ANGLE, "the countersink angle")? / 2.0,
                    None => std::f64::consts::FRAC_PI_4,
                };
                if rs <= r || !(half > 0.0 && half < std::f64::consts::FRAC_PI_2) {
                    return self.fail(codes::E5001, item.span, "a countersink must be wider than the hole, with an included angle between 0 and 180 degrees");
                }
                let ds = (rs - r) / half.tan();
                if ds >= depth {
                    return self.fail(
                        codes::E5001,
                        item.span,
                        "the countersink is deeper than the hole",
                    );
                }
                (
                    vec![
                        [0.0, e],
                        [0.0, -depth],
                        [r, -depth],
                        [r, -ds],
                        [rs + e * half.tan(), e],
                    ],
                    vec![None, Some("floor"), Some("wall"), Some("csk"), None],
                )
            }
            other => {
                return self.fail(
                    codes::E2501,
                    item.span,
                    format!("unknown hole type `{other}`; expected simple, counterbore, countersink or tapped"),
                );
            }
        };
        let segs: Vec<Segment> = (0..pts.len())
            .map(|i| Segment::Line {
                a: pts[i],
                b: pts[(i + 1) % pts.len()],
            })
            .collect();
        let lp = Loop { segments: segs };
        let multiple = positions.len() > 1;
        let mut tools = Vec::new();
        for (idx, p) in positions.iter().enumerate() {
            let center = add(
                add(frame.origin, scale(frame.x, p[0])),
                scale(frame.y, p[1]),
            );
            let pf = Frame {
                origin: center,
                x: frame.x,
                y: frame.z,
                z: cross(frame.x, frame.z),
            };
            let profile = Profile {
                frame: pf,
                outer: lp.clone(),
                holes: vec![],
            };
            let shape = match self
                .k
                .revolve(&profile, center, frame.z, 2.0 * std::f64::consts::PI)
            {
                Ok(s) => s,
                Err(err) => return self.kernel_error(item, err),
            };
            let roles = roles.clone();
            let tool = self.make_tool(item, shape, &move |role| match role {
                Role::Side { seg, .. } => roles.get(seg as usize).copied().flatten().map(|n| {
                    if multiple {
                        format!("p{}.{n}", idx + 1)
                    } else {
                        n.to_string()
                    }
                }),
                _ => None,
            });
            tools.push(tool);
        }
        let tool = self.unite(item, tools)?;
        self.combine_as(item, BoolOp::Subtract, tool)?;
        if let Some(t) = thread
            && let Some(f) = self.features.last_mut()
        {
            f.thread = Some(t);
        }
        Ok(())
    }
}
