//! STEP export and import and tessellation in the OpenCASCADE backend.
#![cfg(feature = "occt")]

use tcad_kernel::geometry::{Frame, Loop, Profile, Segment};
use tcad_kernel::{BoolOp, Kernel, StepMeta};
use tcad_occt::OcctKernel;

fn block(k: &OcctKernel) -> <OcctKernel as Kernel>::Shape {
    let p = [[0.0, 0.0], [30.0, 0.0], [30.0, 20.0], [0.0, 20.0]];
    let profile = Profile {
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
    };
    let b = k.extrude(&profile, [0.0, 0.0, 1.0], 10.0, 0.0).unwrap();
    let hole = Profile {
        frame: Frame::xy().offset(-1.0),
        outer: Loop {
            segments: vec![Segment::Circle {
                center: [15.0, 10.0],
                r: 4.0,
            }],
        },
        holes: vec![],
    };
    let tool = k.extrude(&hole, [0.0, 0.0, 1.0], 12.0, 0.0).unwrap();
    k.boolean(BoolOp::Subtract, &b, &tool, 1e-6).unwrap()
}

#[test]
fn step_round_trip_keeps_the_geometry() {
    let k = OcctKernel::new();
    let a = block(&k);
    eprintln!("step_round_trip: block built");
    let bytes = k
        .export_step(
            &a,
            &StepMeta {
                name: "bracket",
                configuration: Some("heavy"),
                material: Some(("AISI 1045", Some(7.85))),
            },
        )
        .unwrap();
    eprintln!("step_round_trip: exported {} bytes", bytes.len());
    let text = String::from_utf8_lossy(&bytes).to_string();
    assert!(
        text.starts_with("ISO-10303-21"),
        "{}",
        &text[..40.min(text.len())]
    );
    assert!(text.contains("AP242") || text.contains("ap242"), "schema");
    assert!(text.contains("bracket"), "part name");
    assert!(text.contains("AISI 1045"), "material");
    assert!(text.contains("configuration heavy"), "configuration");
    let b = k.import_step(&bytes).unwrap();
    eprintln!("step_round_trip: imported");
    let (pa, pb) = (k.properties(&a).unwrap(), k.properties(&b).unwrap());
    assert!((pa.volume - pb.volume).abs() <= 1e-6 * pa.volume);
    assert!((pa.area - pb.area).abs() <= 1e-6 * pa.area);
    for i in 0..3 {
        assert!((pa.center_of_mass[i] - pb.center_of_mass[i]).abs() <= 1e-5);
        assert!((pa.bbox_min[i] - pb.bbox_min[i]).abs() <= 1e-5);
        assert!((pa.bbox_max[i] - pb.bbox_max[i]).abs() <= 1e-5);
    }
    assert_eq!(pa.topology.faces, pb.topology.faces);
}

#[test]
fn garbage_is_not_a_step_file() {
    let k = OcctKernel::new();
    assert!(k.import_step(b"this is not STEP").is_err());
}

#[test]
fn mesh_covers_every_face_with_outward_triangles() {
    let k = OcctKernel::new();
    let a = block(&k);
    let m = k.mesh(&a, 0.05, 0.2).unwrap();
    let topo = k.topology(&a).unwrap();
    assert_eq!(m.faces.len(), topo.faces.len());
    // the mesh area matches the exact area (planar faces and a fine cylinder)
    let mut area = 0.0;
    let mut volume = 0.0;
    for f in &m.faces {
        assert_eq!(f.positions.len(), f.normals.len());
        // the face index refers to the same face as in the topology listing
        let mut face_area = 0.0;
        for t in f.triangles.chunks_exact(3) {
            let p = |i: u32| {
                let j = 3 * i as usize;
                [
                    f64::from(f.positions[j]),
                    f64::from(f.positions[j + 1]),
                    f64::from(f.positions[j + 2]),
                ]
            };
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            face_area += 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        }
        let exact = topo.faces[f.face as usize].area;
        assert!(
            (face_area - exact).abs() / exact < 5e-3,
            "face {}: {face_area} vs {exact}",
            f.face
        );
        for t in f.triangles.chunks_exact(3) {
            let p = |i: u32| {
                let j = 3 * i as usize;
                [
                    f64::from(f.positions[j]),
                    f64::from(f.positions[j + 1]),
                    f64::from(f.positions[j + 2]),
                ]
            };
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            area += 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            // signed volume of the tetrahedron with the origin: positive for outward winding
            volume += (a[0] * n[0] + a[1] * n[1] + a[2] * n[2]) / 6.0;
        }
    }
    let exact = k.properties(&a).unwrap();
    assert!(
        (area - exact.area).abs() / exact.area < 2e-3,
        "{area} vs {}",
        exact.area
    );
    assert!(
        (volume - exact.volume).abs() / exact.volume < 2e-3,
        "{volume} vs {}",
        exact.volume
    );
}
