//! Topology, history and the detail operations of the OpenCASCADE backend.
#![cfg(feature = "occt")]

use std::f64::consts::PI;
use tcad_kernel::geometry::{Frame, Loop, Profile, Segment};
use tcad_kernel::{
    BoolOp, ChamferEdge, CurveKind, FilletEdge, Kernel, Role, SurfaceKind, Transform,
};
use tcad_occt::OcctKernel;

fn near(a: f64, b: f64, rel: f64, what: &str) {
    assert!(
        (a - b).abs() <= rel * b.abs().max(1.0),
        "{what}: {a} vs {b}"
    );
}

fn rect(w: f64, h: f64) -> Profile {
    let p = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
    Profile {
        frame: Frame::xy(),
        outer: Loop {
            segments: (0..4)
                .map(|i| Segment::Line {
                    a: p[i],
                    b: p[(i + 1) % 4],
                })
                .collect(),
        },
        holes: vec![],
    }
}

fn block(k: &OcctKernel, w: f64, h: f64, d: f64) -> <OcctKernel as Kernel>::Shape {
    k.extrude(&rect(w, h), [0.0, 0.0, 1.0], d, 0.0).unwrap()
}

#[test]
fn block_topology_and_lineage() {
    let k = OcctKernel::new();
    let b = block(&k, 20.0, 10.0, 5.0);
    let t = k.topology(&b).unwrap();
    assert_eq!(t.faces.len(), 6);
    assert_eq!(t.edges.len(), 12);
    assert!(t.faces.iter().all(|f| f.kind == SurfaceKind::Plane));
    assert!(
        t.edges
            .iter()
            .all(|e| e.faces.len() == 2 && e.kind == CurveKind::Line)
    );
    let lin = k.lineage(&b);
    assert!(lin.inherited.is_empty());
    let roles: Vec<Role> = lin.created.iter().map(|c| c.1).collect();
    assert!(roles.contains(&Role::CapStart) && roles.contains(&Role::CapEnd));
    let sides: Vec<(u32, u32)> = roles
        .iter()
        .filter_map(|r| match r {
            Role::Side { lp, seg } => Some((*lp, *seg)),
            _ => None,
        })
        .collect();
    assert_eq!(sides.len(), 4);
    // the cap at the end faces +Z, the cap at the start -Z
    for (f, r) in &lin.created {
        let n = t.faces[*f as usize].normal.unwrap();
        match r {
            Role::CapStart => assert!(n[2] < -0.99),
            Role::CapEnd => assert!(n[2] > 0.99),
            Role::Side { seg, .. } => {
                // segment 0 runs along +X at y=0: its side faces -Y
                if *seg == 0 {
                    assert!(n[1] < -0.99, "{n:?}");
                }
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn boolean_lineage_follows_faces() {
    let k = OcctKernel::new();
    let a = block(&k, 20.0, 10.0, 5.0);
    // a tool through the middle of the top face
    let tool = k
        .extrude(
            &Profile {
                frame: Frame::xy().offset(-1.0),
                outer: Loop {
                    segments: vec![Segment::Circle {
                        center: [10.0, 5.0],
                        r: 2.0,
                    }],
                },
                holes: vec![],
            },
            [0.0, 0.0, 1.0],
            10.0,
            0.0,
        )
        .unwrap();
    let r = k.boolean(BoolOp::Subtract, &a, &tool, 1e-6).unwrap();
    let t = k.topology(&r).unwrap();
    assert_eq!(t.faces.len(), 7);
    let lin = k.lineage(&r);
    // each of the six faces of the block is still there (the top and bottom are trimmed)
    let from_a: Vec<_> = lin.inherited.iter().filter(|i| i.1 == 0).collect();
    assert_eq!(from_a.len(), 6);
    // the hole wall comes from the tool's side face
    let from_tool: Vec<_> = lin.inherited.iter().filter(|i| i.1 == 1).collect();
    assert_eq!(from_tool.len(), 1);
    let wall = from_tool[0].0 as usize;
    assert_eq!(t.faces[wall].kind, SurfaceKind::Cylinder);
    near(t.faces[wall].radius.unwrap(), 2.0, 1e-9, "wall radius");
}

#[test]
fn merged_faces_have_several_origins() {
    let k = OcctKernel::new();
    let a = block(&k, 20.0, 10.0, 5.0);
    let moved = k
        .transform(&a, &Transform::Translate([20.0, 0.0, 0.0]))
        .unwrap();
    let u = k.boolean(BoolOp::Union, &a, &moved, 1e-6).unwrap();
    let t = k.topology(&u).unwrap();
    assert_eq!(t.faces.len(), 6, "coplanar faces merge");
    let lin = k.lineage(&u);
    // the merged top face comes from both blocks
    let top = t
        .faces
        .iter()
        .position(|f| f.normal.is_some_and(|n| n[2] > 0.99))
        .unwrap() as u32;
    let origins: Vec<_> = lin.inherited.iter().filter(|i| i.0 == top).collect();
    assert_eq!(origins.len(), 2);
    assert!(origins.iter().any(|o| o.1 == 0) && origins.iter().any(|o| o.1 == 1));
}

fn edge_between(
    k: &OcctKernel,
    s: &<OcctKernel as Kernel>::Shape,
    n1: [f64; 3],
    n2: [f64; 3],
) -> u32 {
    let t = k.topology(s).unwrap();
    let hit = |f: u32, n: [f64; 3]| {
        let m = t.faces[f as usize].normal.unwrap();
        (m[0] - n[0]).abs() + (m[1] - n[1]).abs() + (m[2] - n[2]).abs() < 1e-6
    };
    t.edges
        .iter()
        .position(|e| {
            e.faces.len() == 2
                && ((hit(e.faces[0], n1) && hit(e.faces[1], n2))
                    || (hit(e.faces[0], n2) && hit(e.faces[1], n1)))
        })
        .expect("edge") as u32
}

#[test]
fn fillet_and_chamfer() {
    let k = OcctKernel::new();
    let a = block(&k, 20.0, 10.0, 5.0);
    let e = edge_between(&k, &a, [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]);
    let f = k
        .fillet(
            &a,
            &[FilletEdge {
                edge: e,
                r1: 2.0,
                r2: 2.0,
            }],
        )
        .unwrap();
    let p = k.properties(&f).unwrap();
    // removed: (1 - pi/4) r^2 L
    near(
        p.volume,
        20.0 * 10.0 * 5.0 - (1.0 - PI / 4.0) * 4.0 * 20.0,
        1e-9,
        "fillet volume",
    );
    let t = k.topology(&f).unwrap();
    assert_eq!(t.faces.len(), 7);
    let lin = k.lineage(&f);
    let made: Vec<_> = lin
        .created
        .iter()
        .filter(|c| matches!(c.1, Role::FromEdge(i) if i == e))
        .collect();
    assert_eq!(made.len(), 1);
    assert_eq!(t.faces[made[0].0 as usize].kind, SurfaceKind::Cylinder);
    assert_eq!(lin.inherited.len(), 6);

    let tt = k.topology(&a).unwrap();
    let face = tt.edges[e as usize].faces[0];
    let c = k
        .chamfer(
            &a,
            &[ChamferEdge {
                edge: e,
                face,
                d1: 2.0,
                d2: 3.0,
            }],
        )
        .unwrap();
    near(
        k.properties(&c).unwrap().volume,
        1000.0 - 0.5 * 2.0 * 3.0 * 20.0,
        1e-9,
        "chamfer volume",
    );
}

#[test]
fn shell_and_draft() {
    let k = OcctKernel::new();
    let a = block(&k, 20.0, 20.0, 10.0);
    let t = k.topology(&a).unwrap();
    let top = t
        .faces
        .iter()
        .position(|f| f.normal.is_some_and(|n| n[2] > 0.99))
        .unwrap() as u32;
    let s = k.shell(&a, &[top], 2.0, false).unwrap();
    near(
        k.properties(&s).unwrap().volume,
        20.0 * 20.0 * 10.0 - 16.0 * 16.0 * 8.0,
        1e-9,
        "shell volume",
    );
    let st = k.topology(&s).unwrap();
    assert_eq!(st.faces.len(), 11);
    let lin = k.lineage(&s);
    let n_inner = lin
        .created
        .iter()
        .filter(|c| matches!(c.1, Role::FromFace(_)))
        .count();
    assert_eq!(n_inner, 5, "inner walls {:?}", lin.created);

    // draft the four sides by 5 degrees about the bottom plane, pulling along +Z
    let sides: Vec<u32> = t
        .faces
        .iter()
        .enumerate()
        .filter(|(_, f)| f.normal.is_some_and(|n| n[2].abs() < 1e-6))
        .map(|(i, _)| i as u32)
        .collect();
    let d = k
        .draft(
            &a,
            &sides,
            [0.0, 0.0, 1.0],
            5.0_f64.to_radians(),
            [0.0; 3],
            [0.0, 0.0, 1.0],
        )
        .unwrap();
    let p = k.properties(&d).unwrap();
    let w = 20.0 - 2.0 * 10.0 * 5.0_f64.to_radians().tan();
    near(p.bbox_max[0] - p.bbox_min[0], 20.0, 1e-9, "base stays");
    let top_t = k.topology(&d).unwrap();
    let top_face = top_t
        .faces
        .iter()
        .find(|f| f.normal.is_some_and(|n| n[2] > 0.99))
        .unwrap();
    near(
        top_face.area,
        w * w,
        1e-6,
        "narrower top (positive angle narrows along the pull)",
    );
}

#[test]
fn transforms_keep_the_faces() {
    let k = OcctKernel::new();
    let a = block(&k, 20.0, 10.0, 5.0);
    for by in [
        Transform::Translate([3.0, 4.0, 5.0]),
        Transform::Rotate {
            origin: [0.0; 3],
            dir: [0.0, 0.0, 1.0],
            angle: PI / 3.0,
        },
        Transform::Mirror {
            origin: [0.0; 3],
            normal: [1.0, 0.0, 0.0],
        },
    ] {
        let m = k.transform(&a, &by).unwrap();
        let p = k.properties(&m).unwrap();
        near(p.volume, 1000.0, 1e-9, "volume");
        let lin = k.lineage(&m);
        assert_eq!(lin.inherited.len(), 6);
        let ta = k.topology(&a).unwrap();
        let tm = k.topology(&m).unwrap();
        for (r, _, i) in &lin.inherited {
            near(
                tm.faces[*r as usize].area,
                ta.faces[*i as usize].area,
                1e-9,
                "area",
            );
        }
    }
    let m = k
        .transform(
            &a,
            &Transform::Mirror {
                origin: [0.0; 3],
                normal: [1.0, 0.0, 0.0],
            },
        )
        .unwrap();
    let p = k.properties(&m).unwrap();
    near(p.center_of_mass[0], -10.0, 1e-9, "mirrored x");
}

#[test]
fn indices_survive_the_cache() {
    let k = OcctKernel::new();
    let a = block(&k, 20.0, 10.0, 5.0);
    let b = k
        .boolean(BoolOp::Subtract, &a, &block(&k, 4.0, 4.0, 20.0), 1e-6)
        .unwrap();
    let again = k.from_brep(&k.canonical_brep(&b).unwrap()).unwrap();
    assert_eq!(k.topology(&b).unwrap(), k.topology(&again).unwrap());
}

#[test]
fn revolve_roles() {
    let k = OcctKernel::new();
    // half-section of a hole: axis, floor, wall, top
    let pts = [[0.0, 0.5], [0.0, -4.0], [3.0, -4.0], [3.0, 0.5]];
    let lp = Loop {
        segments: (0..4)
            .map(|i| Segment::Line {
                a: pts[i],
                b: pts[(i + 1) % 4],
            })
            .collect(),
    };
    let f = Frame {
        origin: [0.0; 3],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, 1.0],
        z: [0.0, -1.0, 0.0],
    };
    let tool = k
        .revolve(
            &Profile {
                frame: f,
                outer: lp,
                holes: vec![],
            },
            [0.0; 3],
            [0.0, 0.0, 1.0],
            2.0 * PI,
        )
        .unwrap();
    let lin = k.lineage(&tool);
    let t = k.topology(&tool).unwrap();
    eprintln!(
        "{:?}\n{:?}",
        lin.created,
        t.faces.iter().map(|f| (f.kind, f.area)).collect::<Vec<_>>()
    );
    assert_eq!(lin.created.len(), 3, "floor, wall, top: {:?}", lin.created);
}
