//! `tcad-naming`: persistent naming (spec/naming.md).
//!
//! Every face a feature creates gets a **semantic ID** `<feature>.<role>[(<generator>)]`, derived from
//! the history of the operations (the [`Lineage`] a kernel reports), never from kernel indices.
//! Later features keep the IDs of the faces they trim, split or merge:
//!
//! * a face that is merged with others keeps the ID of the earliest contributing feature; the other
//!   IDs become aliases,
//! * faces that a later feature splits share one ID and are told apart by an ordinal `#k`, ordered by
//!   a kernel-independent rule (centroid x, y, z, then area).
//!
//! Edges are named by the faces they join: `edge(<face id> & <face id>)`. Tags and geometric queries
//! complete the resolver.

mod query;
mod resolve;

pub use query::{Filter, Query, QueryKind, run_query};
pub use resolve::{Entity, NameError, Resolver, canonical_text};

use std::collections::HashMap;
use tcad_kernel::{Lineage, Role, TopoInfo};

/// Working name of the format. Defined once per crate so it can be renamed later
/// (see `spec/name.md`).
pub const FORMAT_NAME: &str = "TextCAD";

/// File extension of a part/assembly source file.
pub const FILE_EXTENSION: &str = "tcad";

/// File extension of an archived bundle.
pub const BUNDLE_EXTENSION: &str = "tcadz";

/// One semantic ID of a face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Id {
    /// The text, without ordinal: `boss.side(profile.r1.top)`.
    pub text: String,
    /// Position of the creating feature in the feature order (earlier is smaller).
    pub rank: u32,
}

/// The IDs of every face of a shape, by face index. The first ID of a face is its primary ID.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Names {
    /// IDs per face.
    pub faces: Vec<Vec<Id>>,
}

/// Rounds so that centroids that differ by numerical noise sort the same way.
fn q(v: f64) -> i64 {
    (v * 1e6).round() as i64
}

impl Names {
    /// Names for the result of an operation: `inputs` are the names of the operands, indexed by the
    /// operand number in the lineage. `role` turns a created role into the text after the feature
    /// name; a face that has neither an origin nor a role is named `<feature>.extra`.
    pub fn derive(
        feature: &str,
        rank: u32,
        topo: &TopoInfo,
        lineage: &Lineage,
        inputs: &[&Names],
        role: &dyn Fn(Role) -> Option<String>,
    ) -> Names {
        let mut faces: Vec<Vec<Id>> = vec![Vec::new(); topo.faces.len()];
        for &(f, r) in &lineage.created {
            if let Some(text) = role(r) {
                let id = Id {
                    text: format!("{feature}.{text}"),
                    rank,
                };
                if let Some(slot) = faces.get_mut(f as usize)
                    && !slot.contains(&id)
                {
                    slot.push(id);
                }
            }
        }
        for &(f, operand, input) in &lineage.inherited {
            let Some(src) = inputs
                .get(operand as usize)
                .and_then(|n| n.faces.get(input as usize))
            else {
                continue;
            };
            if let Some(slot) = faces.get_mut(f as usize) {
                for id in src {
                    if !slot.contains(id) {
                        slot.push(id.clone());
                    }
                }
            }
        }
        for slot in &mut faces {
            if slot.is_empty() {
                slot.push(Id {
                    text: format!("{feature}.extra"),
                    rank,
                });
            }
            // the earliest feature is primary; equal ranks order by text so the choice never
            // depends on the kernel's enumeration
            slot.sort_by(|a, b| (a.rank, &a.text).cmp(&(b.rank, &b.text)));
        }
        Names { faces }
    }

    /// Faces sharing an ID, sorted by the ordinal rule, for every ID text.
    pub fn groups(&self, topo: &TopoInfo) -> HashMap<String, Vec<u32>> {
        let mut groups: HashMap<String, Vec<u32>> = HashMap::new();
        for (i, ids) in self.faces.iter().enumerate() {
            for id in ids {
                groups.entry(id.text.clone()).or_default().push(i as u32);
            }
        }
        for list in groups.values_mut() {
            list.sort_by_key(|&f| {
                let fi = &topo.faces[f as usize];
                (
                    q(fi.centroid[0]),
                    q(fi.centroid[1]),
                    q(fi.centroid[2]),
                    q(fi.area),
                    f,
                )
            });
            list.dedup();
        }
        groups
    }

    /// The primary ID of every face with its ordinal if the ID is shared: `boss.cap_end`,
    /// `boss.side(profile.c1)#2`.
    pub fn primary_texts(&self, topo: &TopoInfo) -> Vec<String> {
        let groups = self.groups(topo);
        self.faces
            .iter()
            .enumerate()
            .map(|(i, ids)| ordinal_text(&groups, &ids[0].text, i as u32))
            .collect()
    }

    /// Every ID of every face, ordinals included (primary first).
    pub fn all_texts(&self, topo: &TopoInfo) -> Vec<Vec<String>> {
        let groups = self.groups(topo);
        self.faces
            .iter()
            .enumerate()
            .map(|(i, ids)| {
                ids.iter()
                    .map(|id| ordinal_text(&groups, &id.text, i as u32))
                    .collect()
            })
            .collect()
    }

    /// The name of an edge: `edge(<face> & <face>)`, with an ordinal when several edges join the
    /// same faces. One entry per edge of `topo`.
    pub fn edge_texts(&self, topo: &TopoInfo) -> Vec<String> {
        let faces = self.primary_texts(topo);
        let base: Vec<String> = topo
            .edges
            .iter()
            .map(|e| {
                let mut ids: Vec<&str> = e
                    .faces
                    .iter()
                    .map(|&f| faces[f as usize].as_str())
                    .collect();
                ids.sort_unstable();
                ids.dedup();
                format!("edge({})", ids.join(" & "))
            })
            .collect();
        let mut groups: HashMap<&str, Vec<u32>> = HashMap::new();
        for (i, b) in base.iter().enumerate() {
            groups.entry(b.as_str()).or_default().push(i as u32);
        }
        for list in groups.values_mut() {
            list.sort_by_key(|&e| {
                let m = topo.edges[e as usize].midpoint;
                (q(m[0]), q(m[1]), q(m[2]), e)
            });
        }
        base.iter()
            .enumerate()
            .map(|(i, b)| {
                let g = &groups[b.as_str()];
                if g.len() > 1 {
                    let k = g.iter().position(|&e| e == i as u32).unwrap_or(0) + 1;
                    format!("{b}#{k}")
                } else {
                    b.clone()
                }
            })
            .collect()
    }
}

fn ordinal_text(groups: &HashMap<String, Vec<u32>>, text: &str, face: u32) -> String {
    match groups.get(text) {
        Some(g) if g.len() > 1 => {
            let k = g.iter().position(|&f| f == face).unwrap_or(0) + 1;
            format!("{text}#{k}")
        }
        _ => text.to_string(),
    }
}

/// The generator of an edge name: `a & b` of `edge(a & b)#2`.
pub fn edge_generator(text: &str) -> &str {
    let t = text.strip_prefix("edge(").unwrap_or(text);
    match t.rfind(')') {
        Some(i) => &t[..i],
        None => t,
    }
}

/// The feature part of an ID text (`fillet_1` of `fillet_1.face(...)`).
pub fn feature_of(text: &str) -> &str {
    text.split(['.', '(']).next().unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcad_kernel::{FaceInfo, SurfaceKind};

    fn face(cx: f64, area: f64) -> FaceInfo {
        FaceInfo {
            kind: SurfaceKind::Plane,
            normal: Some([0.0, 0.0, 1.0]),
            axis: None,
            radius: None,
            centroid: [cx, 0.0, 0.0],
            area,
            edges: vec![],
        }
    }

    #[test]
    fn name_constants() {
        assert_eq!(FORMAT_NAME, "TextCAD");
        assert_eq!(FILE_EXTENSION, "tcad");
        assert_eq!(BUNDLE_EXTENSION, "tcadz");
    }

    #[test]
    fn created_inherited_and_merged() {
        let topo = TopoInfo {
            faces: vec![face(0.0, 1.0), face(1.0, 1.0), face(2.0, 1.0)],
            edges: vec![],
        };
        let first = Names::derive(
            "a",
            0,
            &topo,
            &Lineage {
                inherited: vec![],
                created: vec![(0, Role::CapStart), (1, Role::CapEnd)],
            },
            &[],
            &|r| match r {
                Role::CapStart => Some("cap_start".into()),
                Role::CapEnd => Some("cap_end".into()),
                _ => None,
            },
        );
        assert_eq!(first.faces[0][0].text, "a.cap_start");
        assert_eq!(first.faces[2][0].text, "a.extra");
        // a later feature merges face 0 and face 1 into face 0 and splits face 2 in two
        let topo2 = TopoInfo {
            faces: vec![face(0.0, 2.0), face(2.0, 0.5), face(3.0, 0.5)],
            edges: vec![],
        };
        let second = Names::derive(
            "b",
            1,
            &topo2,
            &Lineage {
                inherited: vec![(0, 0, 1), (0, 0, 0), (1, 0, 2), (2, 0, 2)],
                created: vec![],
            },
            &[&first],
            &|_| None,
        );
        assert_eq!(second.faces[0].len(), 2, "merged face keeps both IDs");
        let all = second.all_texts(&topo2);
        assert_eq!(all[1][0], "a.extra#1");
        assert_eq!(all[2][0], "a.extra#2");
        assert_eq!(all[0][0], "a.cap_end");
    }
}
