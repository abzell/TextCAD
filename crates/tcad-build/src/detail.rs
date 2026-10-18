//! Detail features: fillet, chamfer, shell, draft and rib.

use crate::select::Want;
use crate::{Cx, Res, Tool};
use tcad_diag::codes;
use tcad_expr::{Dim, ExprKind, Value};
use tcad_kernel::geometry::{Frame, Loop, P2, Profile, Segment, dot, sub};
use tcad_kernel::{ChamferEdge, FilletEdge, Kernel, Role};
use tcad_model::Item;
use tcad_naming::{Names, edge_generator};

impl<K: Kernel> Cx<'_, K> {
    /// Installs the result of an operation on the body and names its faces from the body's names.
    pub(crate) fn replace_body(
        &mut self,
        item: &Item,
        shape: K::Shape,
        role: &dyn Fn(Role) -> Option<String>,
    ) -> Res {
        let topo = match self.k.topology(&shape) {
            Ok(t) => t,
            Err(e) => return self.kernel_error(item, e),
        };
        let lin = self.k.lineage(&shape);
        let prev = std::mem::take(&mut self.names);
        let names = Names::derive(&item.name, self.rank, &topo, &lin, &[&prev], role);
        self.body = Some(shape);
        self.body_topo = Some(topo);
        self.names = names;
        self.finish_feature(item)
    }

    /// `(a, b)` from a length or a pair of lengths.
    fn length_pair(&mut self, a: &tcad_expr::Arg, what: &str) -> Res<(f64, f64)> {
        let Some(e) = a.values.first() else {
            return self.fail(codes::E5001, a.span, format!("`{}` needs a value", a.name));
        };
        match self.value(e)? {
            Value::Num(q) if q.dim == Dim::LENGTH => Ok((q.value, q.value)),
            Value::Tuple(t) if t.len() == 2 => match (&t[0], &t[1]) {
                (Value::Num(x), Value::Num(y)) if x.dim == Dim::LENGTH && y.dim == Dim::LENGTH => {
                    Ok((x.value, y.value))
                }
                _ => self.fail(codes::E5001, e.span, format!("{what} must be two lengths")),
            },
            _ => self.fail(
                codes::E5001,
                e.span,
                format!(
                    "{what} must be a length, or two lengths (start, end) for a variable radius"
                ),
            ),
        }
    }

    pub(crate) fn fillet(&mut self, item: &Item) -> Res {
        self.only(item, &["edges", "r"])?;
        let edges_arg = self.require(item, "edges")?;
        let edges = self.select_list(edges_arg, Want::Edges, "`edges`")?;
        let r = self.require(item, "r")?;
        let (r1, r2) = self.length_pair(r, "the radius")?;
        if r1 <= 0.0 || r2 <= 0.0 {
            return self.fail(codes::E5001, r.span, "the fillet radius must be positive");
        }
        let list: Vec<FilletEdge> = edges
            .iter()
            .map(|&edge| FilletEdge { edge, r1, r2 })
            .collect();
        let texts = self.edge_generators();
        let result = match self.body.as_ref() {
            Some(b) => self.k.fillet(b, &list),
            None => return Err(()),
        };
        let shape = match result {
            Ok(s) => s,
            Err(e) => return self.kernel_error(item, e),
        };
        self.replace_body(item, shape, &|role| match role {
            Role::FromEdge(i) => texts.get(i as usize).map(|g| format!("face({g})")),
            _ => None,
        })
    }

    pub(crate) fn chamfer(&mut self, item: &Item) -> Res {
        self.only(item, &["edges", "d", "d2", "angle"])?;
        let edges_arg = self.require(item, "edges")?;
        let edges = self.select_list(edges_arg, Want::Edges, "`edges`")?;
        let d1 = {
            let a = self.require(item, "d")?;
            self.quantity(a, Dim::LENGTH, "the chamfer distance")?
        };
        let d2 = match (Self::arg(item, "d2"), Self::arg(item, "angle")) {
            (Some(_), Some(a)) => {
                return self.fail(codes::E5001, a.span, "give `d2` or `angle`, not both");
            }
            (Some(a), None) => self.quantity(a, Dim::LENGTH, "the second distance")?,
            (None, Some(a)) => {
                let ang = self.quantity(a, Dim::ANGLE, "the chamfer angle")?;
                if !(ang > 0.0 && ang < std::f64::consts::FRAC_PI_2) {
                    return self.fail(
                        codes::E5001,
                        a.span,
                        "the chamfer angle must be between 0 and 90 degrees",
                    );
                }
                d1 * ang.tan()
            }
            (None, None) => d1,
        };
        if d1 <= 0.0 || d2 <= 0.0 {
            return self.fail(
                codes::E5001,
                item.span,
                "chamfer distances must be positive",
            );
        }
        let list = {
            let (Some(topo), names) = (&self.body_topo, &self.names) else {
                return Err(());
            };
            let faces = names.primary_texts(topo);
            let list: Vec<ChamferEdge> = edges
                .iter()
                .map(|&edge| {
                    // the reference face is the one whose name sorts first, so the choice does not
                    // depend on the kernel's numbering
                    let fs = &topo.edges[edge as usize].faces;
                    let face = fs
                        .iter()
                        .copied()
                        .min_by(|a, b| faces[*a as usize].cmp(&faces[*b as usize]))
                        .unwrap_or(0);
                    ChamferEdge { edge, face, d1, d2 }
                })
                .collect();
            list
        };
        let gens = self.edge_generators();
        let result = match self.body.as_ref() {
            Some(b) => self.k.chamfer(b, &list),
            None => return Err(()),
        };
        let shape = match result {
            Ok(s) => s,
            Err(e) => return self.kernel_error(item, e),
        };
        self.replace_body(item, shape, &|role| match role {
            Role::FromEdge(i) => gens.get(i as usize).map(|g| format!("face({g})")),
            _ => None,
        })
    }

    pub(crate) fn shell(&mut self, item: &Item) -> Res {
        self.only(item, &["faces", "thickness", "outward"])?;
        let remove = match Self::arg(item, "faces") {
            Some(a) => self.select_list(a, Want::Faces, "`faces`")?,
            None => {
                if self.body.is_none() {
                    return self.fail(codes::E5006, item.span, "`shell` needs an existing body");
                }
                Vec::new()
            }
        };
        let thickness = {
            let a = self.require(item, "thickness")?;
            self.quantity(a, Dim::LENGTH, "the wall thickness")?
        };
        if thickness <= 0.0 {
            return self.fail(
                codes::E5001,
                item.span,
                "the wall thickness must be positive",
            );
        }
        let outward = Self::arg(item, "outward").is_some();
        let (faces, edges) = match &self.body_topo {
            Some(t) => (self.names.primary_texts(t), self.edge_generators()),
            None => return Err(()),
        };
        let result = match self.body.as_ref() {
            Some(b) => self.k.shell(b, &remove, thickness, outward),
            None => return Err(()),
        };
        let shape = match result {
            Ok(s) => s,
            Err(e) => return self.kernel_error(item, e),
        };
        self.replace_body(item, shape, &|role| match role {
            Role::FromFace(i) => faces.get(i as usize).map(|f| format!("inner({f})")),
            Role::FromEdge(i) => edges.get(i as usize).map(|g| format!("rim({g})")),
            _ => None,
        })
    }

    pub(crate) fn draft(&mut self, item: &Item) -> Res {
        self.only(item, &["faces", "angle", "pull", "neutral"])?;
        let faces_arg = self.require(item, "faces")?;
        let faces = self.select_list(faces_arg, Want::Faces, "`faces`")?;
        let angle = {
            let a = self.require(item, "angle")?;
            self.quantity(a, Dim::ANGLE, "the draft angle")?
        };
        let pull_arg = self.require(item, "pull")?;
        let pull = self.direction_of(&pull_arg.values[0])?;
        let neutral_arg = self.require(item, "neutral")?;
        let neutral = self.plane_expr(&neutral_arg.values[0])?;
        let result = match self.body.as_ref() {
            Some(b) => self
                .k
                .draft(b, &faces, pull, angle, neutral.origin, neutral.z),
            None => return Err(()),
        };
        let shape = match result {
            Ok(s) => s,
            Err(e) => return self.kernel_error(item, e),
        };
        self.replace_body(item, shape, &|_| None)
    }

    /// `a & b` for every edge of the body, in kernel numbering, for naming fillet and shell faces.
    fn edge_generators(&self) -> Vec<String> {
        match &self.body_topo {
            Some(t) => self
                .names
                .edge_texts(t)
                .iter()
                .map(|s| edge_generator(s).to_string())
                .collect(),
            None => Vec::new(),
        }
    }

    pub(crate) fn rib(&mut self, item: &Item) -> Res {
        self.only(item, &["profile", "thickness", "direction", "to", "reach"])?;
        if self.body.is_none() {
            return self.fail(
                codes::E5006,
                item.span,
                "`rib` needs an existing body to attach to",
            );
        }
        let prof = self.require(item, "profile")?;
        let thickness = {
            let a = self.require(item, "thickness")?;
            self.quantity(a, Dim::LENGTH, "the rib thickness")?
        };
        if thickness <= 0.0 {
            return self.fail(
                codes::E5001,
                item.span,
                "the rib thickness must be positive",
            );
        }
        let dir_arg = self.require(item, "direction")?;
        let dir = self.direction_of(&dir_arg.values[0])?;
        let ExprKind::Ref(r) = &prof.values[0].kind else {
            return self.fail(
                codes::E5002,
                prof.values[0].span,
                "`profile` names a sketch (or a sketch entity) holding the rib's centerline",
            );
        };
        let sk = r.segments[0].name.clone();
        let Some(sketch) = self.sketches.get(&sk) else {
            return self.fail(codes::E5002, r.span, format!("`{sk}` is not a sketch"));
        };
        let frame = self
            .sketch_frames
            .get(&sk)
            .copied()
            .unwrap_or_else(Frame::xy);
        let only = r.segments.get(1).map(|s| s.name.clone());
        let path = match crate::profile::path_of(sketch, frame, only.as_deref(), self.tol.max(1e-6))
        {
            Ok(p) => p,
            Err(m) => return self.fail(codes::E5005, r.span, m),
        };
        // points of the centerline
        let mut pts: Vec<P2> = Vec::new();
        for s in &path.segments {
            let Segment::Line { a, b } = s else {
                return self.fail(codes::E5003, r.span, "a rib centerline made of arcs or splines is not supported yet; use straight lines");
            };
            if pts.is_empty() {
                pts.push(*a);
            }
            pts.push(*b);
        }
        // the direction must lie in the sketch plane
        if dot(dir, frame.z).abs() > 1e-6 {
            return self.fail(
                codes::E5001,
                dir_arg.span,
                "`direction` must lie in the plane of the rib sketch",
            );
        }
        let d2: P2 = [dot(dir, frame.x), dot(dir, frame.y)];
        let project = |p: P2, this: &mut Self| -> Res<P2> {
            if let Some(to) = Self::arg(item, "to") {
                let pl = this.plane_expr(&to.values[0])?;
                let w = [
                    frame.origin[0] + p[0] * frame.x[0] + p[1] * frame.y[0],
                    frame.origin[1] + p[0] * frame.x[1] + p[1] * frame.y[1],
                    frame.origin[2] + p[0] * frame.x[2] + p[1] * frame.y[2],
                ];
                let denom = dot(dir, pl.z);
                if denom.abs() < 1e-9 {
                    return this.fail(
                        codes::E5001,
                        to.span,
                        "`direction` is parallel to the `to` plane",
                    );
                }
                let t = dot(sub(pl.origin, w), pl.z) / denom;
                Ok([p[0] + t * d2[0], p[1] + t * d2[1]])
            } else {
                let reach = match Self::arg(item, "reach") {
                    Some(a) => this.quantity(a, Dim::LENGTH, "the reach")?,
                    None => {
                        return this.fail(
                            codes::E5001,
                            item.span,
                            "a rib needs `to=<plane or face>` or `reach=<length>`",
                        );
                    }
                };
                Ok([p[0] + reach * d2[0], p[1] + reach * d2[1]])
            }
        };
        let first = pts[0];
        let last = *pts.last().unwrap_or(&first);
        let (q_first, q_last) = (project(first, self)?, project(last, self)?);
        let mut names: Vec<String> = Vec::new();
        let mut segs: Vec<Segment> = Vec::new();
        let mut edges: Vec<(P2, P2, String)> = pts
            .windows(2)
            .enumerate()
            .map(|(j, w)| (w[0], w[1], format!("top{j}")))
            .collect();
        edges.push((last, q_last, "end1".into()));
        edges.push((q_last, q_first, "foot".into()));
        edges.push((q_first, first, "end0".into()));
        // a closing edge collapses when the centerline already touches the base
        for (a, b, n) in edges {
            if (a[0] - b[0]).hypot(a[1] - b[1]) > 1e-9 {
                segs.push(Segment::Line { a, b });
                names.push(n);
            }
        }
        let mut lp = Loop { segments: segs };
        if lp.signed_area() < 0.0 {
            lp = Loop {
                segments: lp
                    .segments
                    .iter()
                    .rev()
                    .map(|s| match s {
                        Segment::Line { a, b } => Segment::Line { a: *b, b: *a },
                        other => other.clone(),
                    })
                    .collect(),
            };
            names.reverse();
        }
        if lp.signed_area().abs() < 1e-9 {
            return self.fail(
                codes::E5005,
                r.span,
                "the rib region has no area: the centerline is parallel to `direction`",
            );
        }
        let profile = Profile {
            frame: frame.offset(-thickness / 2.0),
            outer: lp,
            holes: vec![],
        };
        let shape = match self.k.extrude(&profile, frame.z, thickness, 0.0) {
            Ok(s) => s,
            Err(e) => return self.kernel_error(item, e),
        };
        let all = vec![names];
        let tool: Tool<K::Shape> = self.make_tool(item, shape, &crate::sweep_role(&all));
        self.combine(item, tool)
    }
}
