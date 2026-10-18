//! Geometric queries: the fallback when no semantic ID or tag names what a feature needs.

use crate::{Names, feature_of};
use tcad_kernel::geometry::{V3, dot, norm, sub};
use tcad_kernel::{CurveKind, SurfaceKind, TopoInfo};

/// What a query selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryKind {
    /// Faces.
    Faces,
    /// Edges.
    Edges,
}

/// One filter of a query; all filters of a query must hold.
#[derive(Clone, Debug, PartialEq)]
pub enum Filter {
    /// Surface or curve type (`plane`, `cylinder`, `cone`, `sphere`, `torus`, `bspline`; for edges
    /// `line`, `circle`, `ellipse`, `bspline`).
    Type(String),
    /// A planar face whose outward normal points this way.
    Normal(V3),
    /// A planar face whose normal is parallel to the direction, or a straight edge parallel to it.
    ParallelTo(V3),
    /// A cylinder, cone or torus whose axis is parallel to the direction.
    Axis(V3),
    /// Radius of a cylinder, cone, sphere, torus or circular edge (mm).
    Radius(f64),
    /// Entities that a feature created.
    CreatedBy(String),
    /// Faces sharing an edge with these faces; edges of these faces.
    AdjacentTo(Vec<u32>),
    /// The single entity closest to a point.
    Nearest(V3),
    /// The entity or entities of greatest area (faces) or length (edges).
    Largest,
    /// The entity or entities of least area or length.
    Smallest,
}

/// A query: `query(faces, created_by=boss, normal=+Z)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    /// What to select.
    pub kind: QueryKind,
    /// The filters.
    pub filters: Vec<Filter>,
}

fn parallel(a: V3, b: V3) -> bool {
    let (la, lb) = (norm(a), norm(b));
    la > 0.0 && lb > 0.0 && (dot(a, b) / (la * lb)).abs() > 1.0 - 1e-9
}

fn same_way(a: V3, b: V3) -> bool {
    let (la, lb) = (norm(a), norm(b));
    la > 0.0 && lb > 0.0 && dot(a, b) / (la * lb) > 1.0 - 1e-9
}

fn kind_word(k: SurfaceKind) -> &'static str {
    k.word()
}

fn curve_word(k: CurveKind) -> &'static str {
    match k {
        CurveKind::Line => "line",
        CurveKind::Circle => "circle",
        CurveKind::Ellipse => "ellipse",
        CurveKind::BSpline => "bspline",
    }
}

/// Runs a query against the current shape. The result is ordered by position (centroid or
/// midpoint, then size), not by kernel index. `Err` explains an unusable filter.
pub fn run_query(q: &Query, topo: &TopoInfo, names: &Names) -> Result<Vec<u32>, String> {
    let faces = q.kind == QueryKind::Faces;
    let n = if faces {
        topo.faces.len()
    } else {
        topo.edges.len()
    };
    let mut keep: Vec<u32> = (0..n as u32).collect();
    let position = |i: u32| -> V3 {
        if faces {
            topo.faces[i as usize].centroid
        } else {
            topo.edges[i as usize].midpoint
        }
    };
    let size = |i: u32| -> f64 {
        if faces {
            topo.faces[i as usize].area
        } else {
            topo.edges[i as usize].length
        }
    };
    let creators = |i: u32| -> Vec<String> {
        let face_ids = |f: u32| -> Vec<String> {
            names.faces[f as usize]
                .iter()
                .map(|id| feature_of(&id.text).to_string())
                .collect()
        };
        if faces {
            face_ids(i)
        } else {
            topo.edges[i as usize]
                .faces
                .iter()
                .flat_map(|&f| face_ids(f))
                .collect()
        }
    };
    for f in &q.filters {
        match f {
            Filter::Type(t) => {
                let ok = if faces {
                    ["plane", "cylinder", "cone", "sphere", "torus", "bspline"]
                        .contains(&t.as_str())
                } else {
                    ["line", "circle", "ellipse", "bspline"].contains(&t.as_str())
                };
                if !ok {
                    return Err(format!("unknown type `{t}`"));
                }
                keep.retain(|&i| {
                    if faces {
                        kind_word(topo.faces[i as usize].kind) == t
                    } else {
                        curve_word(topo.edges[i as usize].kind) == t
                    }
                });
            }
            Filter::Normal(v) => {
                if !faces {
                    return Err("`normal` selects faces".into());
                }
                keep.retain(|&i| {
                    topo.faces[i as usize]
                        .normal
                        .is_some_and(|n| same_way(n, *v))
                });
            }
            Filter::ParallelTo(v) => {
                keep.retain(|&i| {
                    if faces {
                        topo.faces[i as usize]
                            .normal
                            .is_some_and(|n| parallel(n, *v))
                    } else {
                        topo.edges[i as usize]
                            .direction
                            .is_some_and(|d| parallel(d, *v))
                    }
                });
            }
            Filter::Axis(v) => {
                if !faces {
                    return Err("`axis` selects faces".into());
                }
                keep.retain(|&i| {
                    topo.faces[i as usize]
                        .axis
                        .is_some_and(|(_, d)| parallel(d, *v))
                });
            }
            Filter::Radius(r) => {
                keep.retain(|&i| {
                    let rad = if faces {
                        topo.faces[i as usize].radius
                    } else {
                        topo.edges[i as usize].radius
                    };
                    rad.is_some_and(|x| (x - r).abs() <= 1e-6 * r.abs().max(1.0))
                });
            }
            Filter::CreatedBy(name) => keep.retain(|&i| creators(i).iter().any(|c| c == name)),
            Filter::AdjacentTo(set) => {
                if faces {
                    keep.retain(|&i| {
                        !set.contains(&i)
                            && topo.faces[i as usize].edges.iter().any(|e| {
                                topo.edges[*e as usize]
                                    .faces
                                    .iter()
                                    .any(|f| set.contains(f))
                            })
                    });
                } else {
                    keep.retain(|&i| topo.edges[i as usize].faces.iter().any(|f| set.contains(f)));
                }
            }
            Filter::Nearest(_) | Filter::Largest | Filter::Smallest => {}
        }
    }
    for f in &q.filters {
        match f {
            Filter::Largest | Filter::Smallest => {
                let best = keep.iter().map(|&i| size(i)).fold(
                    if matches!(f, Filter::Largest) {
                        f64::MIN
                    } else {
                        f64::MAX
                    },
                    |a, b| {
                        if matches!(f, Filter::Largest) {
                            a.max(b)
                        } else {
                            a.min(b)
                        }
                    },
                );
                keep.retain(|&i| (size(i) - best).abs() <= 1e-9 * best.abs().max(1.0));
            }
            Filter::Nearest(p) => {
                let best = keep
                    .iter()
                    .map(|&i| norm(sub(position(i), *p)))
                    .fold(f64::MAX, f64::min);
                keep.retain(|&i| (norm(sub(position(i), *p)) - best).abs() <= 1e-9 * best.max(1.0));
                // an exact tie is broken by position so the answer is one entity
                keep.sort_by_key(|&i| {
                    let m = position(i);
                    (
                        (m[0] * 1e6).round() as i64,
                        (m[1] * 1e6).round() as i64,
                        (m[2] * 1e6).round() as i64,
                        i,
                    )
                });
                keep.truncate(1);
            }
            _ => {}
        }
    }
    keep.sort_by_key(|&i| {
        let m = position(i);
        (
            (m[0] * 1e6).round() as i64,
            (m[1] * 1e6).round() as i64,
            (m[2] * 1e6).round() as i64,
            (size(i) * 1e6).round() as i64,
            i,
        )
    });
    Ok(keep)
}
