//! Resolving references, tags and queries to faces and edges of the current body.

use crate::{Cx, RefRecord, Res};
use tcad_diag::{Diagnostic, Edit, FixHint, Span, codes};
use tcad_expr::{Arg, Dim, Expr, ExprKind, Reference, Value};
use tcad_kernel::geometry::{Frame, V3, cross, dot, norm, scale, unit};
use tcad_kernel::{Kernel, SurfaceKind};
use tcad_naming::{
    Entity, Filter, NameError, Query, QueryKind, Resolver, canonical_text, run_query,
};

/// What a reference must denote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Want {
    Faces,
    Edges,
}

impl Want {
    fn word(self) -> &'static str {
        match self {
            Want::Faces => "face",
            Want::Edges => "edge",
        }
    }
}

impl<K: Kernel> Cx<'_, K> {
    fn name_error<T>(&mut self, e: NameError) -> Res<T> {
        let mut d = Diagnostic::error(e.code, e.span, e.message);
        if let Some(c) = e.candidates.first() {
            d = d.with_hint(FixHint {
                message: format!("replace with `{c}`"),
                edits: vec![Edit {
                    span: e.span,
                    replacement: c.clone(),
                }],
            });
        }
        self.diags.push(d);
        Err(())
    }

    fn log_ref(&mut self, span: Span, text: String, resolved: Result<Vec<String>, String>) {
        self.refs.push(RefRecord {
            feature: self.current.clone(),
            span,
            text,
            resolved,
        });
    }

    /// The topology of the body or a diagnostic saying why there is none.
    fn need_topology(&mut self, span: Span, what: &str) -> Res {
        if self.body.is_none() {
            return self.fail(
                codes::E5006,
                span,
                format!("{what} refers to existing geometry, but there is no body yet"),
            );
        }
        if self.body_topo.is_none() {
            return self.fail(
                codes::E5003,
                span,
                format!(
                    "{what} needs a kernel that can list faces and edges (the mock kernel cannot)"
                ),
            );
        }
        Ok(())
    }

    /// A direction from `+Z`, `(0, 0, 1)`, an axis or a plane (its normal).
    pub(crate) fn direction_of(&mut self, e: &Expr) -> Res<V3> {
        match &e.kind {
            ExprKind::Direction(d) => {
                let v = match d.axis.as_str() {
                    "X" => [1.0, 0.0, 0.0],
                    "Y" => [0.0, 1.0, 0.0],
                    "Z" => [0.0, 0.0, 1.0],
                    other => {
                        return self.fail(
                            codes::E5001,
                            d.span,
                            format!("`{other}` is not an axis direction; use +X, -Y, +Z ..."),
                        );
                    }
                };
                Ok(if d.negative { scale(v, -1.0) } else { v })
            }
            ExprKind::Ref(r) => {
                let names: Vec<&str> = r.segments.iter().map(|s| s.name.as_str()).collect();
                if matches!(names.as_slice(), ["axis", _])
                    || r.simple_name().is_some_and(|n| self.axes.contains_key(n))
                {
                    let (_, d) = self.axis_expr(e)?;
                    return Ok(d);
                }
                let f = self.plane_expr(e)?;
                Ok(f.z)
            }
            ExprKind::Tuple(_) => {
                let v = self.triple(e, true)?;
                if norm(v) == 0.0 {
                    return self.fail(codes::E5001, e.span, "a direction cannot be zero");
                }
                Ok(unit(v))
            }
            _ => self.fail(
                codes::E5001,
                e.span,
                "expected a direction such as +Z, (0, 0, 1), an axis or a plane",
            ),
        }
    }

    fn build_query(&mut self, kind: QueryKind, args: &[Arg]) -> Res<Query> {
        let mut filters = Vec::new();
        for a in args {
            let first = a.values.first();
            let f = match (a.name.as_str(), first) {
                ("largest", None) => Filter::Largest,
                ("smallest", None) => Filter::Smallest,
                ("type", Some(e)) => match &e.kind {
                    ExprKind::Ref(r) if r.simple_name().is_some() => {
                        Filter::Type(r.simple_name().unwrap_or("").to_string())
                    }
                    _ => {
                        return self.fail(
                            codes::E3004,
                            e.span,
                            "`type` takes a word like plane or cylinder",
                        );
                    }
                },
                ("normal", Some(e)) => Filter::Normal(self.direction_of(e)?),
                ("parallel_to", Some(e)) => Filter::ParallelTo(self.direction_of(e)?),
                ("axis", Some(e)) => Filter::Axis(self.direction_of(e)?),
                ("radius", Some(_)) => {
                    Filter::Radius(self.quantity(a, Dim::LENGTH, "the radius")?)
                }
                ("created_by", Some(e)) => match &e.kind {
                    ExprKind::Ref(r) if r.simple_name().is_some() => {
                        Filter::CreatedBy(r.simple_name().unwrap_or("").to_string())
                    }
                    _ => {
                        return self.fail(
                            codes::E3004,
                            e.span,
                            "`created_by` takes a feature name",
                        );
                    }
                },
                ("adjacent_to", Some(e)) => {
                    let faces = self.select(e, Want::Faces, "`adjacent_to`")?;
                    Filter::AdjacentTo(faces)
                }
                ("nearest", Some(e)) => Filter::Nearest(self.triple(e, false)?),
                (other, _) => {
                    return self.fail(
                        codes::E3004,
                        a.span,
                        format!(
                            "unknown query filter `{other}`; filters are type, normal, parallel_to, axis, radius, created_by, adjacent_to, nearest, largest, smallest"
                        ),
                    );
                }
            };
            filters.push(f);
        }
        Ok(Query { kind, filters })
    }

    fn edges_of_faces(&self, faces: &[u32]) -> Vec<u32> {
        let Some(topo) = &self.body_topo else {
            return Vec::new();
        };
        let mut out: Vec<u32> = faces
            .iter()
            .flat_map(|&f| topo.faces[f as usize].edges.iter().copied())
            .filter(|&e| !topo.edges[e as usize].faces.is_empty())
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Resolves a reference, tag or query to faces or edges. Faces expand to their edges when
    /// edges are wanted. Records the resolution for `tcad refs`.
    pub(crate) fn select(&mut self, e: &Expr, want: Want, what: &str) -> Res<Vec<u32>> {
        self.need_topology(e.span, what)?;
        match &e.kind {
            ExprKind::List(items) => {
                let mut out = Vec::new();
                for it in items {
                    out.extend(self.select(it, want, what)?);
                }
                out.sort_unstable();
                out.dedup();
                Ok(out)
            }
            ExprKind::Ref(r) => self.select_ref(r, want, what),
            ExprKind::Query(kind, args) => {
                let qk = match kind.as_str() {
                    "faces" => QueryKind::Faces,
                    "edges" => QueryKind::Edges,
                    other => {
                        return self.fail(
                            codes::E5003,
                            e.span,
                            format!("queries on `{other}` are not supported; use faces or edges"),
                        );
                    }
                };
                let q = self.build_query(qk, args)?;
                let (found, listing) = {
                    let (Some(topo), names) = (&self.body_topo, &self.names) else {
                        return Err(());
                    };
                    let found = run_query(&q, topo, names);
                    let tags = std::collections::HashMap::new();
                    let r = Resolver::new(topo, names, &tags);
                    let listing = |v: &[u32]| -> Vec<String> {
                        v.iter()
                            .map(|&i| match qk {
                                QueryKind::Faces => r.face_name(i).to_string(),
                                QueryKind::Edges => r.edge_name(i).to_string(),
                            })
                            .collect()
                    };
                    let l = found.as_ref().ok().map(|v| listing(v));
                    (found, l)
                };
                let text = format!("query({kind})");
                let found = match found {
                    Ok(v) => v,
                    Err(m) => {
                        self.log_ref(e.span, text, Err(m.clone()));
                        return self.fail(codes::E3004, e.span, m);
                    }
                };
                if found.is_empty() {
                    let m = format!("{what}: the query matches nothing");
                    self.log_ref(e.span, text, Err(m.clone()));
                    return self.fail(codes::E3003, e.span, m);
                }
                self.log_ref(e.span, text, Ok(listing.unwrap_or_default()));
                match (qk, want) {
                    (QueryKind::Faces, Want::Faces) | (QueryKind::Edges, Want::Edges) => Ok(found),
                    (QueryKind::Faces, Want::Edges) => Ok(self.edges_of_faces(&found)),
                    (QueryKind::Edges, Want::Faces) => self.fail(
                        codes::E3005,
                        e.span,
                        format!("{what} needs faces, but the query selects edges"),
                    ),
                }
            }
            _ => self.fail(
                codes::E3005,
                e.span,
                format!("{what} needs a {} reference or a query", want.word()),
            ),
        }
    }

    fn select_ref(&mut self, r: &Reference, want: Want, what: &str) -> Res<Vec<u32>> {
        let text = canonical_text(r);
        let (result, names) = {
            let (Some(topo), names) = (&self.body_topo, &self.names) else {
                return Err(());
            };
            let resolver = Resolver::new(topo, names, &self.tags);
            let result = resolver.resolve(r);
            let listing = match &result {
                Ok(Entity::Faces(v)) => v
                    .iter()
                    .map(|&f| resolver.face_name(f).to_string())
                    .collect(),
                Ok(Entity::Edges(v)) => v
                    .iter()
                    .map(|&e| resolver.edge_name(e).to_string())
                    .collect(),
                Err(_) => Vec::new(),
            };
            (result, listing)
        };
        match result {
            Err(e) => {
                self.log_ref(r.span, text, Err(e.message.clone()));
                self.name_error(e)
            }
            Ok(Entity::Faces(v)) => {
                self.log_ref(r.span, text, Ok(names));
                match want {
                    Want::Faces => Ok(v),
                    Want::Edges => Ok(self.edges_of_faces(&v)),
                }
            }
            Ok(Entity::Edges(v)) => {
                self.log_ref(r.span, text.clone(), Ok(names));
                match want {
                    Want::Edges => Ok(v),
                    Want::Faces => self.fail(
                        codes::E3005,
                        r.span,
                        format!("{what}: `{text}` is an edge, but a face is needed"),
                    ),
                }
            }
        }
    }

    /// Resolves the values of a list argument (`edges=[a, b]`, `edges=a, b`) to distinct entities.
    pub(crate) fn select_list(&mut self, a: &Arg, want: Want, what: &str) -> Res<Vec<u32>> {
        let mut out = Vec::new();
        for e in Self::flat(a) {
            out.extend(self.select(e, want, what)?);
        }
        out.sort_unstable();
        out.dedup();
        if out.is_empty() {
            return self.fail(codes::E3003, a.span, format!("{what} selects nothing"));
        }
        Ok(out)
    }

    /// Resolves to exactly one face.
    pub(crate) fn select_face(&mut self, e: &Expr, what: &str) -> Res<u32> {
        let v = self.select(e, Want::Faces, what)?;
        if v.len() != 1 {
            let topo_names: Vec<String> = match &self.body_topo {
                Some(t) => {
                    let all = self.names.primary_texts(t);
                    v.iter().take(8).map(|&f| all[f as usize].clone()).collect()
                }
                None => Vec::new(),
            };
            return self.fail(
                codes::E3003,
                e.span,
                format!(
                    "{what} needs exactly one face, but this selects {}: {}",
                    v.len(),
                    topo_names.join(", ")
                ),
            );
        }
        Ok(v[0])
    }

    /// The frame of a planar face: origin where the plane is closest to the world origin, z along
    /// the outward normal, x horizontal (`Z cross n`) or world X for horizontal faces.
    pub(crate) fn face_frame(&mut self, f: u32, span: Span, what: &str) -> Res<Frame> {
        let Some(topo) = &self.body_topo else {
            return Err(());
        };
        let fi = &topo.faces[f as usize];
        let (Some(n), SurfaceKind::Plane) = (fi.normal, fi.kind) else {
            return self.fail(codes::E5002, span, format!("{what} needs a planar face"));
        };
        let d = dot(n, fi.centroid);
        let origin = scale(n, d);
        let x = if n[2].abs() < 1.0 - 1e-9 {
            unit(cross([0.0, 0.0, 1.0], n))
        } else {
            [1.0, 0.0, 0.0]
        };
        let y = cross(n, x);
        Ok(Frame { origin, x, y, z: n })
    }

    /// A plane from a face reference or query.
    pub(crate) fn face_plane_expr(&mut self, e: &Expr, what: &str) -> Res<Frame> {
        let f = self.select_face(e, what)?;
        self.face_frame(f, e.span, what)
    }

    /// Value of a numeric argument that is a plain count.
    pub(crate) fn count_of(&mut self, a: &Arg, what: &str) -> Res<u32> {
        let Some(e) = a.values.first() else {
            return self.fail(codes::E5001, a.span, format!("`{}` needs a value", a.name));
        };
        match self.value(e)? {
            Value::Num(q) if q.dim.is_none() && q.value >= 1.0 && q.value.fract() == 0.0 => {
                Ok(q.value as u32)
            }
            _ => self.fail(
                codes::E5001,
                e.span,
                format!("{what} must be a whole number of at least 1"),
            ),
        }
    }
}
