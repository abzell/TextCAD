//! Resolving references (tag, then semantic ID, then query) against the current shape.

use crate::Names;
use std::collections::HashMap;
use tcad_diag::{Code, Span, codes};
use tcad_expr::{GenArg, Reference};
use tcad_kernel::TopoInfo;

/// What a reference denotes in the current shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entity {
    /// One or more faces (indices, in kernel numbering).
    Faces(Vec<u32>),
    /// One or more edges.
    Edges(Vec<u32>),
}

/// A reference that does not resolve to what the caller needs.
#[derive(Clone, Debug, PartialEq)]
pub struct NameError {
    /// `E3xxx`.
    pub code: Code,
    /// The reference.
    pub span: Span,
    /// What went wrong.
    pub message: String,
    /// Nearest names, best first, for the diagnostic hint.
    pub candidates: Vec<String>,
}

impl NameError {
    fn new(code: Code, span: Span, message: String, candidates: Vec<String>) -> Self {
        NameError {
            code,
            span,
            message,
            candidates,
        }
    }
}

/// The text of a reference as it appears in IDs: segments joined by `.`, generators sorted and
/// joined by ` & `, ordinal as `#k`. Whitespace and the order inside `a & b` do not matter.
pub fn canonical_text(r: &Reference) -> String {
    let mut s = canonical_no_ordinal(r);
    if let Some(k) = r.ordinal {
        s.push_str(&format!("#{k}"));
    }
    s
}

fn canonical_no_ordinal(r: &Reference) -> String {
    r.segments
        .iter()
        .map(|seg| match &seg.args {
            None => seg.name.clone(),
            Some(args) => {
                let mut parts: Vec<String> = args
                    .iter()
                    .map(|a| match a {
                        GenArg::Ref(r) => canonical_text(r),
                        GenArg::Dir(d) => {
                            format!("{}{}", if d.negative { '-' } else { '+' }, d.axis)
                        }
                    })
                    .collect();
                parts.sort();
                format!("{}({})", seg.name, parts.join(" & "))
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Resolves references against one state of the shape.
pub struct Resolver<'a> {
    topo: &'a TopoInfo,
    groups: HashMap<String, Vec<u32>>,
    face_texts: Vec<Vec<String>>,
    edge_texts: Vec<String>,
    tags: &'a HashMap<String, Reference>,
}

impl<'a> Resolver<'a> {
    /// Prepares the lookup tables for a shape and its names. `tags` maps a tag name to the
    /// reference it was bound to.
    pub fn new(topo: &'a TopoInfo, names: &'a Names, tags: &'a HashMap<String, Reference>) -> Self {
        Resolver {
            topo,
            groups: names.groups(topo),
            face_texts: names.all_texts(topo),
            edge_texts: names.edge_texts(topo),
            tags,
        }
    }

    /// The primary name of every face and edge, for diagnostics and the manifest.
    pub fn face_name(&self, f: u32) -> &str {
        &self.face_texts[f as usize][0]
    }

    /// The name of an edge.
    pub fn edge_name(&self, e: u32) -> &str {
        &self.edge_texts[e as usize]
    }

    fn near(&self, text: &str, edges: bool) -> Vec<String> {
        let pool: Vec<&String> = if edges {
            self.edge_texts.iter().collect()
        } else {
            self.face_texts.iter().flatten().collect()
        };
        let mut scored: Vec<(usize, &String)> =
            pool.iter().map(|c| (distance(text, c), *c)).collect();
        scored.sort();
        scored.dedup_by(|a, b| a.1 == b.1);
        scored.into_iter().take(3).map(|(_, c)| c.clone()).collect()
    }

    /// Resolves a reference to its entities (several if the reference is a tag on more).
    /// Used for faces and edges alike; see [`Resolver::face`].
    pub fn resolve(&self, r: &Reference) -> Result<Entity, NameError> {
        self.resolve_depth(r, 0, false)
    }

    fn resolve_depth(&self, r: &Reference, depth: u32, loose: bool) -> Result<Entity, NameError> {
        if depth > 8 {
            return Err(NameError::new(
                codes::E3001,
                r.span,
                format!("`{}` leads back to itself through tags", canonical_text(r)),
                vec![],
            ));
        }
        if r.segments.len() == 1
            && r.segments[0].args.is_none()
            && let Some(target) = self.tags.get(&r.segments[0].name)
        {
            let e = self.resolve_depth(target, depth + 1, loose)?;
            return self.apply_ordinal(r, e);
        }
        if r.segments.len() == 1 && r.segments[0].name == "edge" && r.segments[0].args.is_some() {
            return self.edge_by_faces(r, depth);
        }
        let text = canonical_no_ordinal(r);
        let Some(list) = self.groups.get(&text) else {
            let cands = self.near(&canonical_text(r), false);
            let mut msg = format!(
                "`{}` does not exist in the current shape",
                canonical_text(r)
            );
            if let Some(c) = cands.first() {
                msg.push_str(&format!("; nearest: `{c}`"));
            }
            return Err(NameError::new(codes::E3001, r.span, msg, cands));
        };
        match r.ordinal {
            Some(k) => match list.get((k as usize).wrapping_sub(1)) {
                Some(&f) => Ok(Entity::Faces(vec![f])),
                None => Err(NameError::new(
                    codes::E3001,
                    r.span,
                    format!("`{text}` has {} face(s); there is no #{k}", list.len()),
                    (1..=list.len()).map(|i| format!("{text}#{i}")).collect(),
                )),
            },
            None if list.len() == 1 || loose => Ok(Entity::Faces(list.clone())),
            None => Err(NameError::new(
                codes::E3002,
                r.span,
                format!(
                    "`{text}` names {} faces (a later feature split it); add an ordinal",
                    list.len()
                ),
                (1..=list.len()).map(|i| format!("{text}#{i}")).collect(),
            )),
        }
    }

    fn apply_ordinal(&self, r: &Reference, e: Entity) -> Result<Entity, NameError> {
        let Some(k) = r.ordinal else { return Ok(e) };
        let (list, faces) = match e {
            Entity::Faces(v) => (v, true),
            Entity::Edges(v) => (v, false),
        };
        match list.get((k as usize).wrapping_sub(1)) {
            Some(&x) => Ok(if faces {
                Entity::Faces(vec![x])
            } else {
                Entity::Edges(vec![x])
            }),
            None => Err(NameError::new(
                codes::E3001,
                r.span,
                format!(
                    "`{}` has {} entities; there is no #{k}",
                    r.head(),
                    list.len()
                ),
                vec![],
            )),
        }
    }

    fn edge_by_faces(&self, r: &Reference, depth: u32) -> Result<Entity, NameError> {
        let args = r.segments[0].args.as_deref().unwrap_or(&[]);
        if args.is_empty() || args.len() > 2 {
            return Err(NameError::new(
                codes::E3005,
                r.span,
                "`edge(...)` names an edge by one or two faces: `edge(a & b)`".into(),
                vec![],
            ));
        }
        let mut sets: Vec<Vec<u32>> = Vec::new();
        for a in args {
            let GenArg::Ref(fr) = a else {
                return Err(NameError::new(
                    codes::E3005,
                    r.span,
                    "`edge(...)` takes face references".into(),
                    vec![],
                ));
            };
            match self.resolve_depth(fr, depth + 1, true)? {
                Entity::Faces(v) => sets.push(v),
                Entity::Edges(_) => {
                    return Err(NameError::new(
                        codes::E3005,
                        fr.span,
                        format!(
                            "`{}` is an edge; `edge(...)` takes faces",
                            canonical_text(fr)
                        ),
                        vec![],
                    ));
                }
            }
        }
        let (a, b) = (&sets[0], sets.get(1).unwrap_or(&sets[0]));
        let mut hits: Vec<u32> = (0..self.topo.edges.len() as u32)
            .filter(|&e| {
                let fs = &self.topo.edges[e as usize].faces;
                if sets.len() == 2 {
                    fs.iter().any(|f| a.contains(f))
                        && fs.iter().any(|f| b.contains(f))
                        && (fs.len() == 2 || a == b)
                } else {
                    fs.iter().any(|f| a.contains(f))
                }
            })
            .collect();
        let key = |e: &u32| {
            let m = self.topo.edges[*e as usize].midpoint;
            (
                (m[0] * 1e6).round() as i64,
                (m[1] * 1e6).round() as i64,
                (m[2] * 1e6).round() as i64,
                *e,
            )
        };
        hits.sort_by_key(key);
        let text = canonical_text(r);
        if hits.is_empty() {
            let cands = self.near(&text, true);
            let mut msg = format!("no edge joins the faces of `{text}`");
            if let Some(c) = cands.first() {
                msg.push_str(&format!("; nearest: `{c}`"));
            }
            return Err(NameError::new(codes::E3001, r.span, msg, cands));
        }
        match r.ordinal {
            Some(k) => match hits.get((k as usize).wrapping_sub(1)) {
                Some(&e) => Ok(Entity::Edges(vec![e])),
                None => Err(NameError::new(
                    codes::E3001,
                    r.span,
                    format!("`{text}` matches {} edge(s); there is no #{k}", hits.len()),
                    vec![],
                )),
            },
            None if hits.len() == 1 => Ok(Entity::Edges(hits)),
            None => {
                let base = canonical_no_ordinal(r);
                Err(NameError::new(
                    codes::E3002,
                    r.span,
                    format!("`{base}` matches {} edges; add an ordinal", hits.len()),
                    (1..=hits.len()).map(|i| format!("{base}#{i}")).collect(),
                ))
            }
        }
    }

    /// Resolves to exactly one face.
    pub fn face(&self, r: &Reference) -> Result<u32, NameError> {
        match self.resolve(r)? {
            Entity::Faces(v) if v.len() == 1 => Ok(v[0]),
            Entity::Faces(v) => Err(NameError::new(
                codes::E3002,
                r.span,
                format!(
                    "`{}` names {} faces; one is needed",
                    canonical_text(r),
                    v.len()
                ),
                vec![],
            )),
            Entity::Edges(_) => Err(NameError::new(
                codes::E3005,
                r.span,
                format!("`{}` is an edge; a face is needed", canonical_text(r)),
                vec![],
            )),
        }
    }

    /// The name of a face by index for use inside generators of new IDs.
    pub fn face_text(&self, f: u32) -> String {
        self.face_texts[f as usize][0].clone()
    }
}
