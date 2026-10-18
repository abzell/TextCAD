//! The OpenCASCADE backend against closed-form results. Needs `--features occt`.
#![cfg(feature = "occt")]

use std::f64::consts::PI;
use tcad_kernel::geometry::{Frame, Loop, Path, Profile, Segment};
use tcad_kernel::{BoolOp, Kernel};
use tcad_occt::OcctKernel;

fn near(a: f64, b: f64, rel: f64, what: &str) {
    assert!(
        (a - b).abs() <= rel * b.abs().max(1.0),
        "{what}: {a} vs {b}"
    );
}

fn rect_loop(x0: f64, y0: f64, x1: f64, y1: f64) -> Loop {
    let p = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    Loop {
        segments: (0..4)
            .map(|i| Segment::Line {
                a: p[i],
                b: p[(i + 1) % 4],
            })
            .collect(),
    }
}

fn poly_loop(pts: &[[f64; 2]]) -> Loop {
    let n = pts.len();
    Loop {
        segments: (0..n)
            .map(|i| Segment::Line {
                a: pts[i],
                b: pts[(i + 1) % n],
            })
            .collect(),
    }
}

fn circle(cx: f64, cy: f64, r: f64) -> Loop {
    Loop {
        segments: vec![Segment::Circle {
            center: [cx, cy],
            r,
        }],
    }
}

fn prof(frame: Frame, outer: Loop) -> Profile {
    Profile {
        frame,
        outer,
        holes: vec![],
    }
}

const Z: [f64; 3] = [0.0, 0.0, 1.0];

#[test]
fn block_properties() {
    let k = OcctKernel::new();
    let b = k
        .extrude(
            &prof(Frame::xy(), rect_loop(0.0, 0.0, 2.0, 3.0)),
            Z,
            4.0,
            0.0,
        )
        .unwrap();
    let p = k.properties(&b).unwrap();
    near(p.volume, 24.0, 1e-12, "volume");
    near(p.area, 52.0, 1e-12, "area");
    for (g, w) in p.center_of_mass.iter().zip([1.0, 1.5, 2.0]) {
        near(*g, w, 1e-12, "com");
    }
    for (g, w) in p.bbox_min.iter().zip([0.0; 3]) {
        near(*g, w, 1e-6, "bbox min");
    }
    for (g, w) in p.bbox_max.iter().zip([2.0, 3.0, 4.0]) {
        near(*g, w, 1e-6, "bbox max");
    }
    assert_eq!(
        (
            p.topology.solids,
            p.topology.faces,
            p.topology.edges,
            p.topology.vertices
        ),
        (1, 6, 12, 8)
    );
    // inertia is reported about the center of mass: Ixx = m (b^2 + c^2) / 12
    let i = p.inertia_com.expect("inertia");
    near(i[0][0], 24.0 * (9.0 + 16.0) / 12.0, 1e-9, "Ixx");
    near(i[1][1], 24.0 * (4.0 + 16.0) / 12.0, 1e-9, "Iyy");
    near(i[2][2], 24.0 * (4.0 + 9.0) / 12.0, 1e-9, "Izz");
}

#[test]
fn cylinder_cone_sphere_torus() {
    let k = OcctKernel::new();
    let cyl = k
        .extrude(&prof(Frame::xy(), circle(0.0, 0.0, 5.0)), Z, 10.0, 0.0)
        .unwrap();
    let p = k.properties(&cyl).unwrap();
    near(p.volume, 250.0 * PI, 1e-9, "cylinder volume");
    near(p.area, 150.0 * PI, 1e-9, "cylinder area");
    assert_eq!(
        (p.topology.faces, p.topology.edges, p.topology.vertices),
        (3, 3, 2)
    );

    // the XZ frame's y is world Z, so the world Z axis is the profile's vertical axis
    let xz = Frame::xz();
    let cone_profile = prof(xz, poly_loop(&[[0.0, 0.0], [4.0, 0.0], [0.0, 6.0]]));
    let cone = k.revolve(&cone_profile, [0.0; 3], Z, 2.0 * PI).unwrap();
    let p = k.properties(&cone).unwrap();
    near(p.volume, PI * 16.0 * 6.0 / 3.0, 1e-9, "cone volume");
    let slant = (16.0f64 + 36.0).sqrt();
    near(p.area, PI * 4.0 * (4.0 + slant), 1e-9, "cone area");

    let r = 3.0;
    let half_disk = Loop {
        segments: vec![
            Segment::Arc {
                center: [0.0, 0.0],
                start: [0.0, -r],
                mid: [r, 0.0],
                end: [0.0, r],
            },
            Segment::Line {
                a: [0.0, r],
                b: [0.0, -r],
            },
        ],
    };
    let sphere = k
        .revolve(&prof(xz, half_disk), [0.0; 3], Z, 2.0 * PI)
        .unwrap();
    let p = k.properties(&sphere).unwrap();
    near(p.volume, 4.0 / 3.0 * PI * 27.0, 1e-9, "sphere volume");
    near(p.area, 4.0 * PI * 9.0, 1e-9, "sphere area");

    let torus = k
        .revolve(&prof(xz, circle(7.0, 0.0, 2.0)), [0.0; 3], Z, 2.0 * PI)
        .unwrap();
    let p = k.properties(&torus).unwrap();
    near(p.volume, 2.0 * PI * PI * 7.0 * 4.0, 1e-9, "torus volume");
    near(p.area, 4.0 * PI * PI * 7.0 * 2.0, 1e-9, "torus area");
}

#[test]
fn tube_hex_wedge_and_partial_revolve() {
    let k = OcctKernel::new();
    let tube = Profile {
        frame: Frame::xy(),
        outer: circle(0.0, 0.0, 5.0),
        holes: vec![circle(0.0, 0.0, 3.0)],
    };
    let t = k.extrude(&tube, Z, 10.0, 0.0).unwrap();
    near(
        k.properties(&t).unwrap().volume,
        PI * (25.0 - 9.0) * 10.0,
        1e-9,
        "tube",
    );

    let hex: Vec<[f64; 2]> = (0..6)
        .map(|i| {
            [
                4.0 * (PI / 3.0 * i as f64).cos(),
                4.0 * (PI / 3.0 * i as f64).sin(),
            ]
        })
        .collect();
    let h = k
        .extrude(&prof(Frame::xy(), poly_loop(&hex)), Z, 5.0, 0.0)
        .unwrap();
    let p = k.properties(&h).unwrap();
    near(
        p.volume,
        3.0 * 3f64.sqrt() / 2.0 * 16.0 * 5.0,
        1e-9,
        "hex volume",
    );
    assert_eq!(p.topology.faces, 8);

    let wedge = k
        .extrude(
            &prof(
                Frame::xz(),
                poly_loop(&[[0.0, 0.0], [6.0, 0.0], [0.0, 4.0]]),
            ),
            Frame::xz().z,
            3.0,
            0.0,
        )
        .unwrap();
    let p = k.properties(&wedge).unwrap();
    near(p.volume, 36.0, 1e-9, "wedge volume");
    // extruded along -Y from the XZ plane
    assert!(p.bbox_min[1] < -2.9 && p.bbox_max[1] < 1e-6);

    // a half revolution of a unit square about the Z axis: V = pi * (3^2 - 2^2) * 1 / 2
    let q = k
        .revolve(
            &prof(Frame::xz(), rect_loop(2.0, 0.0, 3.0, 1.0)),
            [0.0; 3],
            Z,
            PI,
        )
        .unwrap();
    near(
        k.properties(&q).unwrap().volume,
        PI * 5.0 / 2.0,
        1e-9,
        "half ring",
    );
}

#[test]
fn booleans() {
    let k = OcctKernel::new();
    let block = k
        .extrude(
            &prof(Frame::xy(), rect_loop(0.0, 0.0, 10.0, 10.0)),
            Z,
            10.0,
            0.0,
        )
        .unwrap();
    let hole = k
        .extrude(&prof(Frame::xy(), circle(5.0, 5.0, 2.0)), Z, 10.0, 0.0)
        .unwrap();
    let cut = k.boolean(BoolOp::Subtract, &block, &hole, 1e-6).unwrap();
    let p = k.properties(&cut).unwrap();
    near(p.volume, 1000.0 - PI * 4.0 * 10.0, 1e-9, "block with hole");
    // 6 block faces + the hole wall; the seam is one extra edge
    assert_eq!(p.topology.solids, 1);
    assert_eq!(p.topology.faces, 7);

    let other = k
        .extrude(
            &prof(Frame::xy(), rect_loop(5.0, 0.0, 15.0, 10.0)),
            Z,
            10.0,
            0.0,
        )
        .unwrap();
    let u = k.boolean(BoolOp::Union, &block, &other, 1e-6).unwrap();
    let p = k.properties(&u).unwrap();
    near(p.volume, 1500.0, 1e-9, "union");
    assert_eq!(p.topology.faces, 6, "coplanar faces of a union are merged");

    let c = k.boolean(BoolOp::Intersect, &block, &other, 1e-6).unwrap();
    near(
        k.properties(&c).unwrap().volume,
        500.0,
        1e-9,
        "intersection",
    );

    // disjoint solids stay separate
    let far = k
        .extrude(
            &prof(Frame::xy(), rect_loop(100.0, 0.0, 101.0, 1.0)),
            Z,
            1.0,
            0.0,
        )
        .unwrap();
    let two = k.boolean(BoolOp::Union, &block, &far, 1e-6).unwrap();
    assert_eq!(k.properties(&two).unwrap().topology.solids, 2);
}

#[test]
fn loft_sweep_and_draft() {
    let k = OcctKernel::new();
    let low = prof(Frame::xy(), rect_loop(-1.0, -1.0, 1.0, 1.0));
    let high = prof(Frame::xy().offset(2.0), rect_loop(-0.5, -0.5, 0.5, 0.5));
    let frustum = k.loft(&[low, high], true).unwrap();
    near(
        k.properties(&frustum).unwrap().volume,
        2.0 / 3.0 * (4.0 + 1.0 + 2.0),
        1e-9,
        "ruled loft",
    );

    // a straight pipe: circle r=1 along 10 mm
    let along_z = Path {
        frame: Frame::xz(),
        segments: vec![Segment::Line {
            a: [0.0, 0.0],
            b: [0.0, 10.0],
        }],
    };
    let pipe = k
        .sweep(&prof(Frame::xy(), circle(0.0, 0.0, 1.0)), &along_z, None)
        .unwrap();
    near(
        k.properties(&pipe).unwrap().volume,
        10.0 * PI,
        1e-9,
        "straight sweep",
    );

    // a quarter ring (Pappus): area pi r^2 times the centroid path length R * pi / 2
    let bend = Path {
        frame: Frame::xy(),
        segments: vec![Segment::Arc {
            center: [0.0, 0.0],
            start: [5.0, 0.0],
            mid: [5.0 * (PI / 4.0).cos(), 5.0 * (PI / 4.0).sin()],
            end: [0.0, 5.0],
        }],
    };
    // profile plane at the path start, perpendicular to its tangent (+Y)
    let profile_frame = Frame::new([5.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let elbow = k
        .sweep(&prof(profile_frame, circle(0.0, 0.0, 1.0)), &bend, None)
        .unwrap();
    near(
        k.properties(&elbow).unwrap().volume,
        PI * 5.0 * PI / 2.0,
        1e-6,
        "elbow",
    );

    // draft: 10 x 10 base, 10 high, 5 degrees inward
    let d = 5f64.to_radians();
    let drafted = k
        .extrude(
            &prof(Frame::xy(), rect_loop(0.0, 0.0, 10.0, 10.0)),
            Z,
            10.0,
            d,
        )
        .unwrap();
    let top = 10.0 - 2.0 * 10.0 * d.tan();
    near(
        k.properties(&drafted).unwrap().volume,
        10.0 / 3.0 * (100.0 + top * top + 10.0 * top),
        1e-6,
        "drafted prism",
    );
}

#[test]
fn splines_and_ellipses() {
    let k = OcctKernel::new();
    let e = k
        .extrude(
            &prof(
                Frame::xy(),
                Loop {
                    segments: vec![Segment::Ellipse {
                        center: [0.0, 0.0],
                        a: 4.0,
                        b: 2.0,
                        theta: 0.0,
                    }],
                },
            ),
            Z,
            3.0,
            0.0,
        )
        .unwrap();
    near(
        k.properties(&e).unwrap().volume,
        PI * 8.0 * 3.0,
        1e-9,
        "ellipse prism",
    );
    // minor axis given first, rotated: same shape
    let e2 = k
        .extrude(
            &prof(
                Frame::xy(),
                Loop {
                    segments: vec![Segment::Ellipse {
                        center: [0.0, 0.0],
                        a: 2.0,
                        b: 4.0,
                        theta: PI / 2.0,
                    }],
                },
            ),
            Z,
            3.0,
            0.0,
        )
        .unwrap();
    near(
        k.properties(&e2).unwrap().volume,
        PI * 8.0 * 3.0,
        1e-9,
        "rotated ellipse prism",
    );

    // a closed interpolated spline through four points of a unit circle: area close to pi
    let pts: Vec<[f64; 2]> = (0..4)
        .map(|i| [(PI / 2.0 * i as f64).cos(), (PI / 2.0 * i as f64).sin()])
        .collect();
    let sp = k
        .extrude(
            &prof(
                Frame::xy(),
                Loop {
                    segments: vec![Segment::Spline {
                        points: pts,
                        interpolated: true,
                        closed: true,
                    }],
                },
            ),
            Z,
            1.0,
            0.0,
        )
        .unwrap();
    let v = k.properties(&sp).unwrap().volume;
    assert!(v > 2.9 && v < 3.3, "{v}");
}

#[test]
fn canonical_serialization_is_deterministic_and_round_trips() {
    let build = || {
        let k = OcctKernel::new();
        let block = k
            .extrude(
                &prof(Frame::xy(), rect_loop(0.0, 0.0, 10.0, 10.0)),
                Z,
                10.0,
                0.0,
            )
            .unwrap();
        let hole = k
            .extrude(&prof(Frame::xy(), circle(5.0, 5.0, 2.0)), Z, 10.0, 0.0)
            .unwrap();
        let s = k.boolean(BoolOp::Subtract, &block, &hole, 1e-6).unwrap();
        (k.canonical_brep(&s).unwrap(), s)
    };
    let (a, shape) = build();
    let (b, _) = build();
    assert_eq!(a, b, "same inputs, same bytes");
    assert!(!a.is_empty());
    let k = OcctKernel::new();
    let back = k.from_brep(&a).unwrap();
    let (p, q) = (k.properties(&shape).unwrap(), k.properties(&back).unwrap());
    near(q.volume, p.volume, 1e-12, "round-trip volume");
    assert_eq!(p.topology, q.topology);
    assert!(k.id().starts_with("occt-"), "{}", k.id());
}

#[test]
fn invalid_input_is_an_error_not_a_crash() {
    let k = OcctKernel::new();
    let degenerate = prof(
        Frame::xy(),
        Loop {
            segments: vec![Segment::Line {
                a: [0.0, 0.0],
                b: [0.0, 0.0],
            }],
        },
    );
    assert!(k.extrude(&degenerate, Z, 1.0, 0.0).is_err());
    let open = prof(
        Frame::xy(),
        Loop {
            segments: vec![
                Segment::Line {
                    a: [0.0, 0.0],
                    b: [1.0, 0.0],
                },
                Segment::Line {
                    a: [5.0, 5.0],
                    b: [6.0, 5.0],
                },
            ],
        },
    );
    assert!(k.extrude(&open, Z, 1.0, 0.0).is_err());
    assert!(k.from_brep(b"not a brep").is_err());
}
