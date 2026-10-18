//! The resolver and the query filters on a synthetic box (no geometry kernel involved).

use std::collections::HashMap;
use tcad_diag::{Span, codes};
use tcad_expr::{GenArg, Reference, Segment};
use tcad_kernel::{CurveKind, EdgeInfo, FaceInfo, Lineage, Role, SurfaceKind, TopoInfo};
use tcad_naming::{Entity, Filter, Names, Query, QueryKind, Resolver, run_query};

const SPAN: Span = Span { start: 0, end: 0 };

/// A 10 x 10 x 10 box with faces 0..6 = -z, +z, -y, +y, -x, +x and its 12 edges.
fn cube() -> TopoInfo {
    let n = [
        [0.0, 0.0, -1.0],
        [0.0, 0.0, 1.0],
        [0.0, -1.0, 0.0],
        [0.0, 1.0, 0.0],
        [-1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
    ];
    let faces: Vec<FaceInfo> = n
        .iter()
        .map(|n| FaceInfo {
            kind: SurfaceKind::Plane,
            normal: Some(*n),
            axis: None,
            radius: None,
            centroid: [5.0 + 5.0 * n[0], 5.0 + 5.0 * n[1], 5.0 + 5.0 * n[2]],
            area: 100.0,
            edges: vec![],
        })
        .collect();
    let mut edges = Vec::new();
    for a in 0..6u32 {
        for b in (a + 1)..6 {
            let (na, nb) = (n[a as usize], n[b as usize]);
            let dot: f64 = (0..3).map(|i| na[i] * nb[i]).sum();
            if dot.abs() > 0.5 {
                continue; // opposite faces
            }
            let mid = [
                5.0 + 5.0 * (na[0] + nb[0]),
                5.0 + 5.0 * (na[1] + nb[1]),
                5.0 + 5.0 * (na[2] + nb[2]),
            ];
            let dir = [
                na[1] * nb[2] - na[2] * nb[1],
                na[2] * nb[0] - na[0] * nb[2],
                na[0] * nb[1] - na[1] * nb[0],
            ];
            edges.push(EdgeInfo {
                kind: CurveKind::Line,
                ends: (mid, mid),
                midpoint: mid,
                length: 10.0,
                radius: None,
                direction: Some(dir),
                faces: vec![a, b],
            });
        }
    }
    let mut topo = TopoInfo { faces, edges };
    let links: Vec<(usize, Vec<u32>)> = topo
        .edges
        .iter()
        .enumerate()
        .map(|(i, e)| (i, e.faces.clone()))
        .collect();
    for (i, fs) in links {
        for f in fs {
            topo.faces[f as usize].edges.push(i as u32);
        }
    }
    topo
}

fn names() -> Names {
    let lineage = Lineage {
        inherited: vec![],
        created: (0..6u32)
            .map(|i| {
                (
                    i,
                    match i {
                        0 => Role::CapStart,
                        1 => Role::CapEnd,
                        k => Role::Side { lp: 0, seg: k - 2 },
                    },
                )
            })
            .collect(),
    };
    let t = ["bottom", "top", "left", "right"];
    Names::derive("boss", 0, &cube(), &lineage, &[], &|r| match r {
        Role::CapStart => Some("cap_start".into()),
        Role::CapEnd => Some("cap_end".into()),
        Role::Side { seg, .. } => Some(format!("side(s.{})", t[seg as usize])),
        _ => None,
    })
}

fn r(segments: &[&str]) -> Reference {
    Reference {
        segments: segments
            .iter()
            .map(|s| Segment {
                name: s.to_string(),
                args: None,
                span: SPAN,
            })
            .collect(),
        ordinal: None,
        span: SPAN,
    }
}

fn face_ref(text: &str) -> Reference {
    // "boss.side(s.top)" -> segments boss, side(s.top)
    let (head, rest) = text.split_once('.').unwrap();
    let mut out = r(&[head]);
    if let Some((name, arg)) = rest.split_once('(') {
        let arg = arg.trim_end_matches(')');
        let parts: Vec<&str> = arg.split('.').collect();
        out.segments.push(Segment {
            name: name.to_string(),
            args: Some(vec![GenArg::Ref(r(&parts))]),
            span: SPAN,
        });
    } else {
        out.segments.push(Segment {
            name: rest.to_string(),
            args: None,
            span: SPAN,
        });
    }
    out
}

fn edge_ref(a: &str, b: &str) -> Reference {
    Reference {
        segments: vec![Segment {
            name: "edge".into(),
            args: Some(vec![GenArg::Ref(face_ref(a)), GenArg::Ref(face_ref(b))]),
            span: SPAN,
        }],
        ordinal: None,
        span: SPAN,
    }
}

#[test]
fn faces_and_edges_resolve_by_name() {
    let topo = cube();
    let names = names();
    let tags = HashMap::new();
    let res = Resolver::new(&topo, &names, &tags);
    assert_eq!(res.face(&face_ref("boss.cap_end")).unwrap(), 1);
    assert_eq!(res.face(&face_ref("boss.side(s.left)")).unwrap(), 4);
    match res
        .resolve(&edge_ref("boss.cap_end", "boss.side(s.top)"))
        .unwrap()
    {
        Entity::Edges(v) => {
            assert_eq!(v.len(), 1);
            let e = &topo.edges[v[0] as usize];
            assert!(e.faces.contains(&1) && e.faces.contains(&3));
        }
        other => panic!("{other:?}"),
    }
    // the order inside edge(a & b) does not matter
    assert_eq!(
        res.resolve(&edge_ref("boss.side(s.top)", "boss.cap_end"))
            .unwrap(),
        res.resolve(&edge_ref("boss.cap_end", "boss.side(s.top)"))
            .unwrap()
    );
}

#[test]
fn unknown_names_list_the_nearest() {
    let topo = cube();
    let names = names();
    let tags = HashMap::new();
    let res = Resolver::new(&topo, &names, &tags);
    let e = res.face(&face_ref("boss.cap_edn")).unwrap_err();
    assert_eq!(e.code, codes::E3001);
    assert_eq!(e.candidates[0], "boss.cap_end");
}

#[test]
fn tags_resolve_through_the_current_names() {
    let topo = cube();
    let names = names();
    let mut tags = HashMap::new();
    tags.insert("top_face".to_string(), face_ref("boss.cap_end"));
    tags.insert("front".to_string(), face_ref("boss.side(s.bottom)"));
    tags.insert("loop_a".to_string(), r(&["loop_b"]));
    tags.insert("loop_b".to_string(), r(&["loop_a"]));
    let res = Resolver::new(&topo, &names, &tags);
    assert_eq!(res.face(&r(&["top_face"])).unwrap(), 1);
    assert_eq!(res.face(&r(&["front"])).unwrap(), 2);
    assert_eq!(res.resolve(&r(&["loop_a"])).unwrap_err().code, codes::E3001);
}

#[test]
fn queries_filter_and_pick() {
    let topo = cube();
    let names = names();
    let q = |kind, filters| run_query(&Query { kind, filters }, &topo, &names).unwrap();
    assert_eq!(
        q(QueryKind::Faces, vec![Filter::Normal([0.0, 0.0, 1.0])]),
        vec![1]
    );
    assert_eq!(
        q(QueryKind::Faces, vec![Filter::ParallelTo([0.0, 0.0, 1.0])]).len(),
        2
    );
    assert_eq!(
        q(QueryKind::Faces, vec![Filter::Type("plane".into())]).len(),
        6
    );
    assert!(q(QueryKind::Faces, vec![Filter::Type("cylinder".into())]).is_empty());
    assert_eq!(
        q(QueryKind::Faces, vec![Filter::CreatedBy("boss".into())]).len(),
        6
    );
    assert!(q(QueryKind::Faces, vec![Filter::CreatedBy("other".into())]).is_empty());
    // faces next to the top face
    assert_eq!(
        q(QueryKind::Faces, vec![Filter::AdjacentTo(vec![1])]).len(),
        4
    );
    // edges parallel to Z: the four vertical ones
    assert_eq!(
        q(QueryKind::Edges, vec![Filter::ParallelTo([0.0, 0.0, 1.0])]).len(),
        4
    );
    // nearest to a point beside the +x face picks that face
    assert_eq!(
        q(QueryKind::Faces, vec![Filter::Nearest([20.0, 5.0, 5.0])]),
        vec![5]
    );
    assert!(
        run_query(
            &Query {
                kind: QueryKind::Faces,
                filters: vec![Filter::Type("blob".into())]
            },
            &topo,
            &names
        )
        .is_err()
    );
}
