//! `tcad-build`: regenerates a part. Resolves a configuration, solves the sketches, evaluates the
//! feature statements in order and drives a [`Kernel`] to produce the result.
//!
//! Regeneration stops at the first feature that fails, with a diagnostic naming it; it never
//! silently skips or substitutes (requirements 6.4).

mod detail;
mod holes;
mod imports;
mod patterns;
pub mod profile;
mod select;

use profile::NamedProfile;
use std::collections::HashMap;
use tcad_diag::{Code, Diagnostic, Span, codes};
use tcad_expr::{Arg, Dim, Env, EvalError, Expr, ExprKind, Reference, Value, eval};
use tcad_kernel::geometry::{Frame, P2, Path, Profile, Segment, V3, cross, dot, norm, sub, unit};
use tcad_kernel::{BoolOp, Kernel, KernelError, Role, TopoInfo, Topology};
use tcad_model::{Item, ItemKind, Model, SolvedSketch};
use tcad_naming::Names;

/// Working name of the format. Defined once per crate so it can be renamed later
/// (see `spec/name.md`).
pub const FORMAT_NAME: &str = "TextCAD";

/// File extension of a part/assembly source file.
pub const FILE_EXTENSION: &str = "tcad";

/// File extension of an archived bundle.
pub const BUNDLE_EXTENSION: &str = "tcadz";

/// What one feature added to the result.
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureRecord {
    /// Feature name.
    pub name: String,
    /// Statement keyword.
    pub keyword: String,
    /// Topology of the result after this feature.
    pub topology: Topology,
    /// Volume of the result after this feature (mm^3).
    pub volume: f64,
    /// Faces and edges of the result after this feature with their persistent names.
    pub topology_map: Vec<TopoEntry>,
    /// Thread designation recorded by a tapped hole (cosmetic only).
    pub thread: Option<String>,
}

/// One face or edge of a result with its persistent name (spec/naming.md).
#[derive(Clone, Debug, PartialEq)]
pub struct TopoEntry {
    /// `face` or `edge`.
    pub kind: &'static str,
    /// The primary semantic ID, ordinal included.
    pub id: String,
    /// The other IDs of the entity (merged features).
    pub aliases: Vec<String>,
    /// Surface or curve type word.
    pub geometry: String,
    /// Area (mm^2) or length (mm).
    pub size: f64,
    /// Centroid of a face, midpoint of an edge (mm).
    pub at: V3,
}

/// A reference met while regenerating, and what it resolved to.
#[derive(Clone, Debug, PartialEq)]
pub struct RefRecord {
    /// The feature that used the reference.
    pub feature: String,
    /// Location in the source.
    pub span: Span,
    /// The reference as written.
    pub text: String,
    /// The names it resolved to, or the failure.
    pub resolved: Result<Vec<String>, String>,
}

/// The outcome of a regeneration.
pub struct Built<S> {
    /// The resulting solid(s), if any feature produced one.
    pub shape: Option<S>,
    /// One record per solid-producing feature, in order.
    pub features: Vec<FeatureRecord>,
    /// The modeling tolerance of the part (mm).
    pub tolerance: f64,
    /// Material name and density (kg/mm^3), if declared.
    pub material: Option<(String, Option<f64>)>,
    /// The configuration that was built (`None`: the base part).
    pub config: Option<String>,
    /// Every face/edge reference that was resolved, in order.
    pub refs: Vec<RefRecord>,
}

/// Regenerates `model` under a configuration (`None`: the default configuration if the part has
/// one, otherwise the base part).
pub fn build<K: Kernel>(
    kernel: &K,
    model: &Model,
    config: Option<&str>,
) -> Result<Built<K::Shape>, Vec<Diagnostic>> {
    build_traced(kernel, model, config).0
}

/// The outcome of a build and the references it resolved.
pub type Traced<S> = (Result<Built<S>, Vec<Diagnostic>>, Vec<RefRecord>);

/// Reads a bundle file (an imported STEP file) by its path in the source.
pub type Loader<'a> = &'a dyn Fn(&str) -> Result<Vec<u8>, String>;

/// Like [`build`], and also returns every face/edge reference that was resolved on the way, even
/// when the build fails. Imports fail: use [`build_with`] to supply the files.
pub fn build_traced<K: Kernel>(
    kernel: &K,
    model: &Model,
    config: Option<&str>,
) -> Traced<K::Shape> {
    build_with(kernel, model, config, None)
}

/// Like [`build_traced`] with a loader for imported files (paths are relative to the bundle).
pub fn build_with<K: Kernel>(
    kernel: &K,
    model: &Model,
    config: Option<&str>,
    loader: Option<Loader<'_>>,
) -> Traced<K::Shape> {
    let config = config
        .map(str::to_string)
        .or_else(|| model.default_config().map(|c| c.name.clone()));
    let resolved = match model.resolve(config.as_deref()) {
        Ok(r) => r,
        Err(d) => return (Err(d), Vec::new()),
    };
    let sketches: HashMap<String, SolvedSketch> = match model.solve_sketches(config.as_deref()) {
        Ok(v) => v.into_iter().map(|s| (s.name.clone(), s)).collect(),
        Err(d) => return (Err(d), Vec::new()),
    };
    let env: HashMap<String, Value> = resolved.params.iter().cloned().collect();
    let mut cx = Cx {
        k: kernel,
        env,
        sketches,
        frames: HashMap::new(),
        axes: HashMap::new(),
        points: HashMap::new(),
        sketch_frames: HashMap::new(),
        body: None,
        body_topo: None,
        names: Names::default(),
        tags: HashMap::new(),
        tools: HashMap::new(),
        imports: HashMap::new(),
        loader,
        rank: 0,
        current: String::new(),
        refs: Vec::new(),
        diags: Vec::new(),
        features: Vec::new(),
        tol: model.part.tolerance,
    };
    for item in &model.part.items {
        if resolved.suppressed.contains(&item.name) {
            continue;
        }
        if cx.item(item).is_err() {
            break;
        }
    }
    if tcad_diag::has_errors(&cx.diags) {
        return (Err(cx.diags), cx.refs);
    }
    let refs = cx.refs.clone();
    (
        Ok(Built {
            shape: cx.body,
            features: cx.features,
            tolerance: model.part.tolerance,
            material: model
                .part
                .material
                .as_ref()
                .map(|m| (m.name.clone(), m.density)),
            config: resolved.config,
            refs: cx.refs,
        }),
        refs,
    )
}

struct Values<'a>(&'a HashMap<String, Value>);

impl Env for Values<'_> {
    fn get(&self, name: &str) -> Option<Value> {
        self.0.get(name).cloned()
    }
}

pub(crate) type Res<T = ()> = Result<T, ()>;

/// A solid made by one statement before it is combined with the body, with the names of its faces.
pub(crate) struct Tool<S> {
    shape: S,
    /// `None` if the kernel cannot list topology (the mock): then nothing is named.
    topo: Option<TopoInfo>,
    names: Names,
}

/// What a pattern copies: the tool pieces of a feature, with the role text of every face.
pub(crate) struct ToolRec<S> {
    op: BoolOp,
    pieces: Vec<ToolPiece<S>>,
}

pub(crate) struct ToolPiece<S> {
    shape: S,
    /// For each face index of `shape`, the role text after the feature name.
    roles: Vec<String>,
}

pub(crate) struct Cx<'a, K: Kernel> {
    k: &'a K,
    env: HashMap<String, Value>,
    sketches: HashMap<String, SolvedSketch>,
    frames: HashMap<String, Frame>,
    axes: HashMap<String, (V3, V3)>,
    points: HashMap<String, V3>,
    sketch_frames: HashMap<String, Frame>,
    body: Option<K::Shape>,
    body_topo: Option<TopoInfo>,
    names: Names,
    tags: HashMap<String, Reference>,
    tools: HashMap<String, ToolRec<K::Shape>>,
    imports: HashMap<String, Tool<K::Shape>>,
    loader: Option<Loader<'a>>,
    rank: u32,
    current: String,
    refs: Vec<RefRecord>,
    diags: Vec<Diagnostic>,
    features: Vec<FeatureRecord>,
    tol: f64,
}

const OPS: &[&str] = &["add", "subtract", "intersect"];

/// Role text of a face made by sweeping a profile whose segments have `names`.
pub(crate) fn sweep_role(names: &[Vec<String>]) -> impl Fn(Role) -> Option<String> + '_ {
    move |r| match r {
        Role::CapStart => Some("cap_start".to_string()),
        Role::CapEnd => Some("cap_end".to_string()),
        Role::Side { lp, seg } => {
            let n = names
                .get(lp as usize)
                .and_then(|l| l.get(seg as usize))
                .filter(|n| !n.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("loop{lp}.{seg}"));
            Some(format!("side({n})"))
        }
        _ => None,
    }
}

impl<K: Kernel> Cx<'_, K> {
    pub(crate) fn fail<T>(&mut self, code: Code, span: Span, msg: impl Into<String>) -> Res<T> {
        self.diags.push(Diagnostic::error(code, span, msg));
        Err(())
    }

    pub(crate) fn kernel_error<T>(&mut self, item: &Item, e: KernelError) -> Res<T> {
        self.fail(
            codes::E5004,
            item.span,
            format!(
                "{} `{}` failed in the geometry kernel: {e}",
                item.keyword, item.name
            ),
        )
    }

    // ---- evaluating arguments ----

    pub(crate) fn value(&mut self, e: &Expr) -> Res<Value> {
        match eval(e, &Values(&self.env)) {
            Ok(v) => Ok(v),
            Err(EvalError::Diag(d)) => {
                self.diags.push(d);
                Err(())
            }
            Err(EvalError::NotAValue(span, what)) => {
                self.fail(codes::E5001, span, format!("`{what}` is not a value here"))
            }
        }
    }

    pub(crate) fn quantity(&mut self, a: &Arg, dim: Dim, what: &str) -> Res<f64> {
        let Some(e) = a.values.first() else {
            return self.fail(codes::E5001, a.span, format!("`{}` needs a value", a.name));
        };
        match self.value(e)? {
            Value::Num(q) if q.dim == dim => Ok(q.value),
            other => self.fail(
                codes::E5001,
                e.span,
                format!("{what} must be {}, found {}", dim, other.type_name()),
            ),
        }
    }

    pub(crate) fn triple(&mut self, e: &Expr, allow_numbers: bool) -> Res<V3> {
        if let ExprKind::Ref(r) = &e.kind
            && let Some(name) = r.simple_name()
            && let Some(p) = self.points.get(name)
        {
            return Ok(*p);
        }
        let v = self.value(e)?;
        if let Value::Tuple(items) = &v
            && items.len() == 3
        {
            let mut out = [0.0; 3];
            for (o, it) in out.iter_mut().zip(items) {
                match it {
                    Value::Num(q) if q.dim == Dim::LENGTH || (allow_numbers && q.dim.is_none()) => {
                        *o = q.value
                    }
                    _ => {
                        return self.fail(
                            codes::E5001,
                            e.span,
                            "expected three lengths like (1 mm, 2 mm, 3 mm)",
                        );
                    }
                }
            }
            return Ok(out);
        }
        self.fail(
            codes::E5001,
            e.span,
            "expected a 3D point like (1 mm, 2 mm, 3 mm) or the name of a `point`",
        )
    }

    /// The arguments of `item` must come from `allowed`.
    pub(crate) fn only(&mut self, item: &Item, allowed: &[&str]) -> Res {
        for a in &item.args {
            if !allowed.contains(&a.name.as_str()) {
                return self.fail(
                    codes::E5001,
                    a.span,
                    format!(
                        "`{}` does not take `{}` (it takes: {})",
                        item.keyword,
                        a.name,
                        allowed.join(", ")
                    ),
                );
            }
        }
        Ok(())
    }

    /// The values of an argument with `[a, b, c]` lists flattened (`through=[a, b]` and
    /// `through=a, b` mean the same).
    pub(crate) fn flat(a: &Arg) -> Vec<&Expr> {
        a.values
            .iter()
            .flat_map(|v| match &v.kind {
                ExprKind::List(items) => items.iter().collect::<Vec<_>>(),
                _ => vec![v],
            })
            .collect()
    }

    pub(crate) fn arg<'i>(item: &'i Item, name: &str) -> Option<&'i Arg> {
        item.args.iter().find(|a| a.name == name)
    }

    pub(crate) fn require<'i>(&mut self, item: &'i Item, name: &str) -> Res<&'i Arg> {
        match Self::arg(item, name) {
            Some(a) => Ok(a),
            None => self.fail(
                codes::E5001,
                item.span,
                format!("`{}` `{}` needs `{name}=...`", item.keyword, item.name),
            ),
        }
    }

    // ---- references to geometry ----

    pub(crate) fn plane_of(&mut self, r: &Reference) -> Res<Frame> {
        let names: Vec<&str> = r.segments.iter().map(|s| s.name.as_str()).collect();
        match names.as_slice() {
            ["plane", "XY"] => Ok(Frame::xy()),
            ["plane", "YZ"] => Ok(Frame::yz()),
            ["plane", "XZ"] => Ok(Frame::xz()),
            [name] if self.frames.contains_key(*name) => Ok(self.frames[*name]),
            [name] if self.sketches.contains_key(*name) => {
                // a sketch's own plane
                Ok(self.sketch_frames.get(*name).copied().unwrap_or_else(Frame::xy))
            }
            _ if self.body.is_some() => {
                // a planar face of the body, by name, tag or query
                let e = Expr {
                    kind: ExprKind::Ref(r.clone()),
                    span: r.span,
                };
                self.face_plane_expr(&e, "a plane")
            }
            _ => self.fail(
                codes::E5002,
                r.span,
                format!(
                    "`{}` is not a plane: use plane.XY, plane.YZ, plane.XZ, a `plane` feature or a planar face of an existing body",
                    r.dotted()
                ),
            ),
        }
    }

    pub(crate) fn plane_expr(&mut self, e: &Expr) -> Res<Frame> {
        match &e.kind {
            ExprKind::Ref(r) => self.plane_of(r),
            ExprKind::Query(..) if self.body.is_some() => self.face_plane_expr(e, "a plane"),
            _ => self.fail(codes::E5002, e.span, "expected a plane"),
        }
    }

    pub(crate) fn axis_expr(&mut self, e: &Expr) -> Res<(V3, V3)> {
        let ExprKind::Ref(r) = &e.kind else {
            return self.fail(codes::E5002, e.span, "expected an axis");
        };
        let names: Vec<&str> = r.segments.iter().map(|s| s.name.as_str()).collect();
        match names.as_slice() {
            ["axis", "X"] => Ok(([0.0; 3], [1.0, 0.0, 0.0])),
            ["axis", "Y"] => Ok(([0.0; 3], [0.0, 1.0, 0.0])),
            ["axis", "Z"] => Ok(([0.0; 3], [0.0, 0.0, 1.0])),
            [name] if self.axes.contains_key(*name) => Ok(self.axes[*name]),
            [sk, line] if self.sketches.contains_key(*sk) => {
                let frame = self.sketch_frames.get(*sk).copied().unwrap_or_else(Frame::xy);
                let found = self.sketches[*sk]
                    .entities
                    .iter()
                    .find(|en| en.name == *line)
                    .and_then(|en| match &en.geometry {
                        tcad_model::Geometry::Line { start, end } => Some((*start, *end)),
                        _ => None,
                    });
                match found {
                    Some((a, b)) => {
                        let (wa, wb) = (frame.to_world(a), frame.to_world(b));
                        Ok((wa, unit(sub(wb, wa))))
                    }
                    None => self.fail(codes::E5002, r.span, format!("`{}` is not a line of a sketch", r.dotted())),
                }
            }
            _ => self.fail(
                codes::E5002,
                r.span,
                format!("`{}` is not an axis: use axis.X, axis.Y, axis.Z, an `axis` feature or a sketch line", r.dotted()),
            ),
        }
    }

    /// Profiles named by a `from=` / `sections=` value: `sketch`, `sketch.entity`, or an inline
    /// `rect(...)` / `circle(...)`.
    pub(crate) fn profiles(&mut self, e: &Expr) -> Res<Vec<NamedProfile>> {
        match &e.kind {
            ExprKind::Ref(r) => {
                let Some(sk) = r.segments.first().map(|s| s.name.clone()) else {
                    return self.fail(codes::E5002, r.span, "empty reference");
                };
                let Some(sketch) = self.sketches.get(&sk) else {
                    return self.fail(codes::E5002, r.span, format!("`{sk}` is not a sketch"));
                };
                let only = r.segments.get(1).map(|s| s.name.clone());
                let frame = self
                    .sketch_frames
                    .get(&sk)
                    .copied()
                    .unwrap_or_else(Frame::xy);
                let loops = match profile::loops_named(sketch, only.as_deref(), self.tol.max(1e-6))
                {
                    Ok(l) => l,
                    Err(m) => return self.fail(codes::E5005, r.span, m),
                };
                Ok(profile::nest_named(frame, loops))
            }
            ExprKind::Profile(kind, args) => self.inline_profile(e, kind, args),
            _ => self.fail(
                codes::E5002,
                e.span,
                "expected a sketch or an inline profile like rect(on=plane.XY, ...)",
            ),
        }
    }

    pub(crate) fn inline_profile(
        &mut self,
        e: &Expr,
        kind: &str,
        args: &[Arg],
    ) -> Res<Vec<NamedProfile>> {
        let get = |n: &str| args.iter().find(|a| a.name == n);
        let Some(on) = get("on").and_then(|a| a.values.first()) else {
            return self.fail(
                codes::E5001,
                e.span,
                format!("`{kind}(...)` needs `on=<plane>`"),
            );
        };
        let frame = self.plane_expr(on)?;
        let len = |this: &mut Self, name: &str| -> Res<Option<f64>> {
            match get(name) {
                Some(a) => Ok(Some(this.quantity(a, Dim::LENGTH, name)?)),
                None => Ok(None),
            }
        };
        let xy = |this: &mut Self, name: &str| -> Res<Option<P2>> {
            match get(name).and_then(|a| a.values.first()) {
                Some(v) => match this.value(v)? {
                    Value::Tuple(t) if t.len() == 2 => match (&t[0], &t[1]) {
                        (Value::Num(a), Value::Num(b))
                            if a.dim == Dim::LENGTH && b.dim == Dim::LENGTH =>
                        {
                            Ok(Some([a.value, b.value]))
                        }
                        _ => this.fail(codes::E5001, v.span, "expected two lengths"),
                    },
                    _ => this.fail(codes::E5001, v.span, "expected a point like (1 mm, 2 mm)"),
                },
                None => Ok(None),
            }
        };
        let names: Vec<String>;
        let outer = match kind {
            "rect" => {
                let (Some(w), Some(h)) = (len(self, "w")?, len(self, "h")?) else {
                    return self.fail(codes::E5001, e.span, "`rect(...)` needs `w` and `h`");
                };
                let o = match (xy(self, "corner")?, xy(self, "center")?) {
                    (Some(c), None) => c,
                    (None, Some(c)) => [c[0] - w / 2.0, c[1] - h / 2.0],
                    (None, None) => [-w / 2.0, -h / 2.0],
                    _ => {
                        return self.fail(
                            codes::E5001,
                            e.span,
                            "give `corner` or `center`, not both",
                        );
                    }
                };
                let p = [o, [o[0] + w, o[1]], [o[0] + w, o[1] + h], [o[0], o[1] + h]];
                names = ["rect.bottom", "rect.right", "rect.top", "rect.left"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                tcad_kernel::Loop {
                    segments: (0..4)
                        .map(|i| Segment::Line {
                            a: p[i],
                            b: p[(i + 1) % 4],
                        })
                        .collect(),
                }
            }
            "circle" => {
                let r = match (len(self, "r")?, len(self, "d")?) {
                    (Some(r), None) => r,
                    (None, Some(d)) => d / 2.0,
                    _ => return self.fail(codes::E5001, e.span, "`circle(...)` needs `r` or `d`"),
                };
                let c = xy(self, "center")?.unwrap_or([0.0, 0.0]);
                names = vec!["circle".to_string()];
                tcad_kernel::Loop {
                    segments: vec![Segment::Circle { center: c, r }],
                }
            }
            other => {
                return self.fail(
                    codes::E5003,
                    e.span,
                    format!("an inline `{other}(...)` profile is not supported; use `rect` or `circle`, or draw a sketch"),
                );
            }
        };
        Ok(vec![NamedProfile {
            profile: Profile {
                frame,
                outer,
                holes: vec![],
            },
            names: vec![names],
        }])
    }

    // ---- statements ----

    pub(crate) fn item(&mut self, item: &Item) -> Res {
        self.current = item.name.clone();
        match &item.kind {
            ItemKind::Sketch(sk) => {
                let frame = self.plane_of(&sk.plane)?;
                self.sketch_frames.insert(item.name.clone(), frame);
                Ok(())
            }
            ItemKind::Tag(target) => {
                // a tag must resolve where it is declared
                let e = Expr {
                    kind: ExprKind::Ref(target.clone()),
                    span: target.span,
                };
                if self.body.is_some() {
                    self.select(&e, select::Want::Faces, "a tag")
                        .or_else(|()| {
                            // not a face: it may be an edge tag
                            self.diags.pop();
                            self.select(&e, select::Want::Edges, "a tag")
                        })
                        .map(|_| ())?;
                }
                self.tags.insert(item.name.clone(), target.clone());
                Ok(())
            }
            ItemKind::Import(path) => {
                self.rank += 1;
                self.import_step(item, path)
            }
            ItemKind::RefGeom => match item.keyword.as_str() {
                "plane" => self.plane_feature(item),
                "axis" => self.axis_feature(item),
                "point" => self.point_feature(item),
                "csys" => self.csys_feature(item),
                other => self.fail(
                    codes::E5003,
                    item.span,
                    format!("`{other}` is not supported yet"),
                ),
            },
            ItemKind::Feature => {
                self.rank += 1;
                match item.keyword.as_str() {
                    "extrude" => self.extrude(item),
                    "revolve" => self.revolve(item),
                    "sweep" => self.sweep(item),
                    "loft" => self.loft(item),
                    "hole" => self.hole(item),
                    "fillet" => self.fillet(item),
                    "chamfer" => self.chamfer(item),
                    "shell" => self.shell(item),
                    "draft" => self.draft(item),
                    "rib" => self.rib(item),
                    "linear_pattern" => self.linear_pattern(item),
                    "circular_pattern" => self.circular_pattern(item),
                    "mirror" => self.mirror(item),
                    "combine" => self.combine_import(item),
                    other => self.fail(
                        codes::E5003,
                        item.span,
                        format!("`{other}` is not supported"),
                    ),
                }
            }
        }
    }

    pub(crate) fn point_feature(&mut self, item: &Item) -> Res {
        self.only(item, &["at"])?;
        let at = self.require(item, "at")?;
        let p = self.triple(&at.values[0], false)?;
        self.points.insert(item.name.clone(), p);
        Ok(())
    }

    pub(crate) fn axis_feature(&mut self, item: &Item) -> Res {
        self.only(item, &["through", "from", "direction"])?;
        if let Some(t) = Self::arg(item, "through") {
            let pts = Self::flat(t);
            if pts.len() != 2 {
                return self.fail(codes::E5001, t.span, "`through` needs two points");
            }
            let a = self.triple(pts[0], false)?;
            let b = self.triple(pts[1], false)?;
            if norm(sub(b, a)) < 1e-12 {
                return self.fail(codes::E5001, t.span, "the two points of an axis coincide");
            }
            self.axes.insert(item.name.clone(), (a, unit(sub(b, a))));
            return Ok(());
        }
        let from = self.require(item, "from")?;
        let o = self.triple(&from.values[0], false)?;
        let dir = self.require(item, "direction")?;
        let d = match &dir.values[0].kind {
            ExprKind::Ref(_) => self.axis_expr(&dir.values[0])?.1,
            _ => self.triple(&dir.values[0], true)?,
        };
        self.axes.insert(item.name.clone(), (o, unit(d)));
        Ok(())
    }

    pub(crate) fn plane_feature(&mut self, item: &Item) -> Res {
        self.only(item, &["offset", "from", "angle", "about", "through"])?;
        if let Some(t) = Self::arg(item, "through") {
            let pts = Self::flat(t);
            if pts.len() != 3 {
                return self.fail(codes::E5001, t.span, "`through` needs three points");
            }
            let p: Vec<V3> = pts
                .into_iter()
                .map(|v| self.triple(v, false))
                .collect::<Res<_>>()?;
            let (u, v) = (sub(p[1], p[0]), sub(p[2], p[0]));
            if norm(cross(u, v)) < 1e-9 {
                return self.fail(codes::E5001, t.span, "the three points are on a line");
            }
            self.frames
                .insert(item.name.clone(), Frame::new(p[0], u, v));
            return Ok(());
        }
        let from = self.require(item, "from")?;
        let mut f = self.plane_expr(&from.values[0])?;
        if let Some(o) = Self::arg(item, "offset") {
            f = f.offset(self.quantity(o, Dim::LENGTH, "the offset")?);
        }
        if let Some(a) = Self::arg(item, "angle") {
            let angle = self.quantity(a, Dim::ANGLE, "the angle")?;
            let about = self.require(item, "about")?;
            let (o, d) = self.axis_expr(&about.values[0])?;
            f = f.rotated(o, d, angle);
        } else if Self::arg(item, "about").is_some() {
            return self.fail(codes::E5001, item.span, "`about` needs an `angle`");
        }
        if Self::arg(item, "offset").is_none() && Self::arg(item, "angle").is_none() {
            return self.fail(
                codes::E5001,
                item.span,
                "a plane needs `offset`, `angle` and `about`, or `through`",
            );
        }
        self.frames.insert(item.name.clone(), f);
        Ok(())
    }

    pub(crate) fn csys_feature(&mut self, item: &Item) -> Res {
        self.only(item, &["origin", "x", "y"])?;
        let o = self.require(item, "origin")?;
        let origin = self.triple(&o.values[0], false)?;
        let x = self.require(item, "x")?;
        let xd = self.triple(&x.values[0], true)?;
        let y = self.require(item, "y")?;
        let yd = self.triple(&y.values[0], true)?;
        if norm(cross(xd, yd)) < 1e-9 {
            return self.fail(
                codes::E5001,
                item.span,
                "the x and y directions are parallel",
            );
        }
        self.frames
            .insert(item.name.clone(), Frame::new(origin, xd, yd));
        Ok(())
    }

    pub(crate) fn op_of(&mut self, item: &Item) -> Res<&'static str> {
        match Self::arg(item, "op") {
            None => Ok("add"),
            Some(a) => {
                let name = match a.values.first().map(|e| &e.kind) {
                    Some(ExprKind::Ref(r)) => r.simple_name().unwrap_or(""),
                    _ => "",
                };
                match OPS.iter().find(|o| **o == name) {
                    Some(o) => Ok(o),
                    None => self.fail(
                        codes::E5001,
                        a.span,
                        "`op` must be add, subtract or intersect",
                    ),
                }
            }
        }
    }

    /// Names the faces of a freshly made solid from the roles its construction reported.
    pub(crate) fn make_tool(
        &mut self,
        item: &Item,
        shape: K::Shape,
        role: &dyn Fn(Role) -> Option<String>,
    ) -> Tool<K::Shape> {
        match self.k.topology(&shape) {
            Ok(topo) => {
                let lin = self.k.lineage(&shape);
                let names = Names::derive(&item.name, self.rank, &topo, &lin, &[], role);
                Tool {
                    shape,
                    topo: Some(topo),
                    names,
                }
            }
            Err(_) => Tool {
                shape,
                topo: None,
                names: Names::default(),
            },
        }
    }

    /// Names the faces of the result of a boolean from the names of its operands.
    pub(crate) fn derived_tool(
        &mut self,
        item: &Item,
        shape: K::Shape,
        inputs: &[&Names],
    ) -> Tool<K::Shape> {
        match self.k.topology(&shape) {
            Ok(topo) => {
                let lin = self.k.lineage(&shape);
                let names = Names::derive(&item.name, self.rank, &topo, &lin, inputs, &|_| None);
                Tool {
                    shape,
                    topo: Some(topo),
                    names,
                }
            }
            Err(_) => Tool {
                shape,
                topo: None,
                names: Names::default(),
            },
        }
    }

    /// Unites several solids made by one feature.
    pub(crate) fn unite(
        &mut self,
        item: &Item,
        mut tools: Vec<Tool<K::Shape>>,
    ) -> Res<Tool<K::Shape>> {
        let mut acc = tools.remove(0);
        for t in tools {
            let shape = match self
                .k
                .boolean(BoolOp::Union, &acc.shape, &t.shape, self.tol)
            {
                Ok(v) => v,
                Err(e) => return self.kernel_error(item, e),
            };
            acc = self.derived_tool(item, shape, &[&acc.names, &t.names]);
        }
        Ok(acc)
    }

    /// Combines a tool with the body, naming the result. Does not record the feature.
    pub(crate) fn merge_tool(&mut self, item: &Item, op: BoolOp, tool: Tool<K::Shape>) -> Res {
        let next = match self.body.take() {
            None if op == BoolOp::Union => {
                self.body = Some(tool.shape);
                self.body_topo = tool.topo;
                self.names = tool.names;
                return Ok(());
            }
            None => {
                return self.fail(
                    codes::E5006,
                    item.span,
                    format!(
                        "`{}` `{}` needs a body to modify; the first solid feature must add",
                        item.keyword, item.name
                    ),
                );
            }
            Some(b) => match self.k.boolean(op, &b, &tool.shape, self.tol) {
                Ok(s) => s,
                Err(e) => {
                    self.body = Some(b);
                    return self.kernel_error(item, e);
                }
            },
        };
        let prev = std::mem::take(&mut self.names);
        let t = self.derived_tool(item, next, &[&prev, &tool.names]);
        self.body = Some(t.shape);
        self.body_topo = t.topo;
        self.names = t.names;
        Ok(())
    }

    /// Checks the body after a feature and records the feature.
    pub(crate) fn finish_feature(&mut self, item: &Item) -> Res {
        let Some(body) = self.body.as_ref() else {
            return self.fail(codes::E5006, item.span, "there is no body");
        };
        let props = match self.k.properties(body) {
            Ok(p) => p,
            Err(e) => return self.kernel_error(item, e),
        };
        if props.topology.solids == 0 || props.volume.abs() < 1e-12 {
            return self.fail(
                codes::E5004,
                item.span,
                format!("`{}` `{}` leaves no solid", item.keyword, item.name),
            );
        }
        let topology = self.topology_entries();
        self.features.push(FeatureRecord {
            name: item.name.clone(),
            keyword: item.keyword.clone(),
            topology: props.topology,
            volume: props.volume,
            topology_map: topology,
            thread: None,
        });
        Ok(())
    }

    /// Faces and edges of the body with their persistent names.
    pub(crate) fn topology_entries(&self) -> Vec<TopoEntry> {
        let Some(topo) = &self.body_topo else {
            return Vec::new();
        };
        let faces = self.names.all_texts(topo);
        let edges = self.names.edge_texts(topo);
        let mut out = Vec::new();
        for (i, f) in topo.faces.iter().enumerate() {
            out.push(TopoEntry {
                kind: "face",
                id: faces[i][0].clone(),
                aliases: faces[i][1..].to_vec(),
                geometry: f.kind.word().to_string(),
                size: f.area,
                at: f.centroid,
            });
        }
        for (i, e) in topo.edges.iter().enumerate() {
            if e.faces.is_empty() {
                continue;
            }
            out.push(TopoEntry {
                kind: "edge",
                id: edges[i].clone(),
                aliases: Vec::new(),
                geometry: match e.kind {
                    tcad_kernel::CurveKind::Line => "line",
                    tcad_kernel::CurveKind::Circle => "circle",
                    tcad_kernel::CurveKind::Ellipse => "ellipse",
                    tcad_kernel::CurveKind::BSpline => "bspline",
                }
                .to_string(),
                size: e.length,
                at: e.midpoint,
            });
        }
        out
    }

    /// Remembers the tool pieces of a feature so patterns can copy them.
    pub(crate) fn register_tool(
        &mut self,
        item: &Item,
        op: BoolOp,
        tools: Vec<(K::Shape, Vec<String>)>,
    ) {
        self.tools.insert(
            item.name.clone(),
            ToolRec {
                op,
                pieces: tools
                    .into_iter()
                    .map(|(shape, roles)| ToolPiece { shape, roles })
                    .collect(),
            },
        );
    }

    /// The role text of every face of a tool: its primary ID without the feature name.
    pub(crate) fn roles_of(&self, item: &Item, tool: &Tool<K::Shape>) -> Vec<String> {
        let prefix = format!("{}.", item.name);
        tool.names
            .faces
            .iter()
            .map(|ids| {
                let t = &ids[0].text;
                t.strip_prefix(&prefix).unwrap_or(t).to_string()
            })
            .collect()
    }

    /// Adds a tool to the body (or subtracts, or intersects), records the feature and remembers the
    /// tool for patterns.
    pub(crate) fn combine(&mut self, item: &Item, tool: Tool<K::Shape>) -> Res {
        let op = match self.op_of(item)? {
            "add" => BoolOp::Union,
            "subtract" => BoolOp::Subtract,
            _ => BoolOp::Intersect,
        };
        self.combine_as(item, op, tool)
    }

    /// Like [`Cx::combine`] with the operation fixed by the feature kind.
    pub(crate) fn combine_as(&mut self, item: &Item, op: BoolOp, tool: Tool<K::Shape>) -> Res {
        let roles = self.roles_of(item, &tool);
        let copy = tool.shape.clone();
        self.merge_tool(item, op, tool)?;
        self.finish_feature(item)?;
        self.register_tool(item, op, vec![(copy, roles)]);
        Ok(())
    }

    pub(crate) fn body_diagonal(&self) -> f64 {
        self.body
            .as_ref()
            .and_then(|b| self.k.properties(b).ok())
            .map_or(0.0, |p| norm(sub(p.bbox_max, p.bbox_min)))
    }

    pub(crate) fn extrude(&mut self, item: &Item) -> Res {
        self.only(
            item,
            &[
                "from",
                "distance",
                "distance2",
                "symmetric",
                "through_all",
                "to",
                "draft",
                "op",
            ],
        )?;
        let from = self.require(item, "from")?;
        let profiles = self.profiles(&from.values[0])?;
        let ends = ["distance", "through_all", "to"]
            .iter()
            .filter(|n| Self::arg(item, n).is_some())
            .count();
        if ends != 1 {
            return self.fail(
                codes::E5001,
                item.span,
                "an extrusion needs exactly one end condition: `distance=...`, `through_all` or `to=<plane>`",
            );
        }
        let symmetric = Self::arg(item, "symmetric").is_some();
        let distance = match Self::arg(item, "distance") {
            Some(a) => Some(self.quantity(a, Dim::LENGTH, "the distance")?),
            None => None,
        };
        let distance2 = match Self::arg(item, "distance2") {
            Some(a) => Some(self.quantity(a, Dim::LENGTH, "the second distance")?),
            None => None,
        };
        let draft = match Self::arg(item, "draft") {
            Some(a) => self.quantity(a, Dim::ANGLE, "the draft angle")?,
            None => 0.0,
        };
        if distance2.is_some() && distance.is_none() {
            return self.fail(codes::E5001, item.span, "`distance2` goes with `distance`");
        }
        if draft != 0.0 && (symmetric || distance2.is_some()) {
            return self.fail(
                codes::E5001,
                item.span,
                "`draft` cannot be combined with `symmetric` or `distance2`",
            );
        }
        let target = match Self::arg(item, "to") {
            Some(a) => Some(self.plane_expr(&a.values[0])?),
            None => None,
        };
        let through_all = Self::arg(item, "through_all").is_some();
        if through_all && self.body.is_none() {
            return self.fail(
                codes::E5006,
                item.span,
                "`through_all` needs an existing body",
            );
        }
        let reach = 2.0 * self.body_diagonal() + 1.0;

        let mut shapes = Vec::new();
        for np in &profiles {
            let p = &np.profile;
            let n = p.frame.z;
            let (start, len) = if let Some(d) = distance {
                if symmetric {
                    (-d / 2.0, d)
                } else if let Some(d2) = distance2 {
                    (-d2, d + d2)
                } else {
                    (0.0, d)
                }
            } else if through_all {
                if symmetric {
                    (-reach, 2.0 * reach)
                } else {
                    (0.0, reach)
                }
            } else {
                let t = target.expect("end condition counted");
                if dot(t.z, n).abs() < 1.0 - 1e-9 {
                    return self.fail(
                        codes::E5001,
                        item.span,
                        "`to` must be a plane parallel to the profile",
                    );
                }
                (0.0, dot(sub(t.origin, p.frame.origin), n))
            };
            let moved = Profile {
                frame: p.frame.offset(start),
                ..p.clone()
            };
            match self.k.extrude(&moved, n, len, draft) {
                Ok(s) => shapes.push(self.make_tool(item, s, &sweep_role(&np.names))),
                Err(e) => return self.kernel_error(item, e),
            }
        }
        let tool = self.unite(item, shapes)?;
        self.combine(item, tool)
    }

    pub(crate) fn revolve(&mut self, item: &Item) -> Res {
        self.only(item, &["from", "axis", "angle", "op"])?;
        let from = self.require(item, "from")?;
        let profiles = self.profiles(&from.values[0])?;
        let ax = self.require(item, "axis")?;
        let (origin, dir) = self.axis_expr(&ax.values[0])?;
        let angle = match Self::arg(item, "angle") {
            Some(a) => self.quantity(a, Dim::ANGLE, "the angle")?,
            None => 2.0 * std::f64::consts::PI,
        };
        let mut shapes = Vec::new();
        for np in &profiles {
            match self.k.revolve(&np.profile, origin, dir, angle) {
                Ok(s) => shapes.push(self.make_tool(item, s, &sweep_role(&np.names))),
                Err(e) => return self.kernel_error(item, e),
            }
        }
        let tool = self.unite(item, shapes)?;
        self.combine(item, tool)
    }

    pub(crate) fn path(&mut self, e: &Expr) -> Res<Path> {
        let ExprKind::Ref(r) = &e.kind else {
            return self.fail(
                codes::E5002,
                e.span,
                "expected a sketch (or a sketch entity) holding the path",
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
        match profile::path_of(sketch, frame, only.as_deref(), self.tol.max(1e-6)) {
            Ok(p) => Ok(p),
            Err(m) => self.fail(codes::E5005, r.span, m),
        }
    }

    pub(crate) fn sweep(&mut self, item: &Item) -> Res {
        self.only(item, &["from", "path", "guide", "op"])?;
        let from = self.require(item, "from")?;
        let profiles = self.profiles(&from.values[0])?;
        let path = {
            let a = self.require(item, "path")?;
            self.path(&a.values[0])?
        };
        let guide = match Self::arg(item, "guide") {
            Some(a) => Some(self.path(&a.values[0])?),
            None => None,
        };
        let mut shapes = Vec::new();
        for np in &profiles {
            match self.k.sweep(&np.profile, &path, guide.as_ref()) {
                Ok(s) => shapes.push(self.make_tool(item, s, &sweep_role(&np.names))),
                Err(e) => return self.kernel_error(item, e),
            }
        }
        let tool = self.unite(item, shapes)?;
        self.combine(item, tool)
    }

    pub(crate) fn loft(&mut self, item: &Item) -> Res {
        self.only(item, &["sections", "ruled", "op"])?;
        let secs = self.require(item, "sections")?;
        let refs = Self::flat(secs);
        if refs.len() < 2 {
            return self.fail(
                codes::E5001,
                secs.span,
                "a loft needs at least two sections",
            );
        }
        let mut sections = Vec::new();
        let mut first_names: Vec<Vec<String>> = Vec::new();
        for r in refs {
            let mut ps = self.profiles(r)?;
            if ps.len() != 1 {
                return self.fail(
                    codes::E5005,
                    r.span,
                    "each loft section must be exactly one closed profile",
                );
            }
            let np = ps.remove(0);
            if first_names.is_empty() {
                first_names = np.names.clone();
            }
            sections.push(np.profile);
        }
        let ruled = Self::arg(item, "ruled").is_some();
        let shape = match self.k.loft(&sections, ruled) {
            Ok(s) => s,
            Err(e) => return self.kernel_error(item, e),
        };
        let tool = self.make_tool(item, shape, &sweep_role(&first_names));
        self.combine(item, tool)
    }
}
