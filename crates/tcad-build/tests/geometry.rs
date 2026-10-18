//! End-to-end regeneration against OpenCASCADE. Needs `--features occt`.
#![cfg(feature = "occt")]

use std::f64::consts::PI;
use tcad_build::build;
use tcad_diag::Diagnostic;
use tcad_kernel::{Kernel, Properties};
use tcad_model::analyze;
use tcad_occt::OcctKernel;

fn part(body: &str) -> String {
    format!("tcad 0.1\n\npart p {{\n{body}\n}}\n")
}

fn run(body: &str) -> Result<Properties, Vec<Diagnostic>> {
    let a = analyze(&part(body));
    assert!(
        !a.has_errors(),
        "model errors: {:?}\n{body}",
        a.diagnostics
            .iter()
            .filter(|d| d.severity == tcad_diag::Severity::Error)
            .map(|d| format!("{} {} @{}", d.code, d.message, d.span.start))
            .collect::<Vec<_>>()
    );
    let k = OcctKernel::new();
    let built = build(&k, &a.model.expect("model"), None)?;
    Ok(k.properties(&built.shape.expect("a shape"))
        .expect("properties"))
}

fn props(body: &str) -> Properties {
    run(body).unwrap_or_else(|d| {
        panic!(
            "{:?}",
            d.iter()
                .map(|x| format!("{} {} @{}", x.code, x.message, x.span.start))
                .collect::<Vec<_>>()
        )
    })
}

fn codes(body: &str) -> Vec<&'static str> {
    run(body)
        .expect_err("must fail")
        .iter()
        .map(|d| d.code.as_str())
        .collect()
}

fn near(a: f64, b: f64, what: &str) {
    assert!(
        (a - b).abs() <= 1e-9 * b.abs().max(1.0),
        "{what}: {a} vs {b}"
    );
}

#[test]
fn block_cylinder_hex_tube_slot() {
    let p = props(
        "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=2 mm h=3 mm\n  }\n  extrude e from s.r distance=4 mm",
    );
    near(p.volume, 24.0, "block");
    assert_eq!(
        (p.topology.faces, p.topology.edges, p.topology.vertices),
        (6, 12, 8)
    );

    let p = props(
        "  sketch s on plane.XY {\n    circle c center=(0 mm, 0 mm) r=5 mm\n  }\n  extrude e from s distance=10 mm",
    );
    near(p.volume, 250.0 * PI, "cylinder");

    let p = props(
        "  sketch s on plane.XY {\n    polygon g center=(0 mm, 0 mm) sides=6 r=4 mm\n  }\n  extrude e from s distance=5 mm",
    );
    near(p.volume, 3.0 * 3f64.sqrt() / 2.0 * 16.0 * 5.0, "hex prism");

    let p = props(
        "  sketch s on plane.XY {\n    circle outer center=(0 mm, 0 mm) r=5 mm\n    circle inner center=(0 mm, 0 mm) r=3 mm\n  }\n  extrude e from s distance=10 mm",
    );
    near(p.volume, PI * 16.0 * 10.0, "tube");

    let p = props(
        "  sketch s on plane.XY {\n    slot sl start=(0 mm, 0 mm) end=(10 mm, 0 mm) w=4 mm\n  }\n  extrude e from s distance=2 mm",
    );
    near(p.volume, (40.0 + 4.0 * PI) * 2.0, "slot");
}

#[test]
fn revolved_primitives() {
    let cone = props(
        "  sketch s on plane.XZ {\n    line a start=(0 mm, 0 mm) end=(4 mm, 0 mm)\n    line b start=(4 mm, 0 mm) end=(0 mm, 6 mm)\n    line c start=(0 mm, 6 mm) end=(0 mm, 0 mm)\n  }\n  revolve r from s axis=axis.Z",
    );
    near(cone.volume, PI * 16.0 * 6.0 / 3.0, "cone");

    let sphere = props(
        "  sketch s on plane.XZ {\n    arc a start=(0 mm, -3 mm) end=(0 mm, 3 mm) through=(3 mm, 0 mm)\n    line l start=(0 mm, 3 mm) end=(0 mm, -3 mm)\n  }\n  revolve r from s axis=axis.Z",
    );
    near(sphere.volume, 4.0 / 3.0 * PI * 27.0, "sphere");
    near(sphere.area, 4.0 * PI * 9.0, "sphere area");

    let torus = props(
        "  sketch s on plane.XZ {\n    circle c center=(7 mm, 0 mm) r=2 mm\n  }\n  revolve r from s axis=axis.Z",
    );
    near(torus.volume, 2.0 * PI * PI * 7.0 * 4.0, "torus");

    // revolve about a construction line drawn in the sketch, half a turn
    let half = props(
        "  sketch s on plane.XZ {\n    rect r corner=(2 mm, 0 mm) w=1 mm h=1 mm\n    line ax start=(0 mm, 0 mm) end=(0 mm, 1 mm) construction\n  }\n  revolve v from s.r axis=s.ax angle=180 deg",
    );
    near(half.volume, PI * 5.0 / 2.0, "half ring");
}

#[test]
fn end_conditions_and_booleans() {
    let base = "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=10 mm h=10 mm\n  }\n  extrude block from s distance=10 mm\n  sketch h on plane.XY {\n    circle c center=(5 mm, 5 mm) r=2 mm\n  }\n";
    let hole = |end: &str| props(&format!("{base}  extrude cut from h {end} op=subtract"));
    near(
        hole("distance=10 mm").volume,
        1000.0 - PI * 40.0,
        "blind cut through the whole block",
    );
    near(
        hole("distance=4 mm").volume,
        1000.0 - PI * 16.0,
        "blind cut",
    );
    // the cut starts at the bottom face (z = 0); through_all keeps going the same way
    near(
        hole("through_all").volume,
        1000.0 - PI * 40.0,
        "through_all",
    );
    // symmetric: 8 mm centered on z = 0 removes the part that overlaps the block (4 mm)
    near(
        hole("distance=8 mm symmetric").volume,
        1000.0 - PI * 16.0,
        "symmetric",
    );
    // two-sided: 3 mm up and 5 mm down from the sketch plane
    near(
        hole("distance=3 mm distance2=5 mm").volume,
        1000.0 - PI * 12.0,
        "two-sided",
    );
    // up to a plane
    let to = format!(
        "{base}  plane top offset=6 mm from=plane.XY\n  extrude cut from h to=top op=subtract"
    );
    near(props(&to).volume, 1000.0 - PI * 24.0, "to plane");

    let union = props(&format!(
        "{base}  sketch w on plane.XY {{\n    rect r corner=(5 mm, 0 mm) w=10 mm h=10 mm\n  }}\n  extrude wing from w distance=10 mm"
    ));
    near(union.volume, 1500.0, "union of two overlapping blocks");
    assert_eq!(union.topology.faces, 6, "coplanar faces merge");

    let common = props(&format!(
        "{base}  sketch w on plane.XY {{\n    rect r corner=(5 mm, 0 mm) w=10 mm h=10 mm\n  }}\n  extrude both from w distance=10 mm op=intersect"
    ));
    near(common.volume, 500.0, "intersection");
}

#[test]
fn draft_and_inline_profiles() {
    let d = 5f64.to_radians();
    let p = props(
        "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=10 mm h=10 mm\n  }\n  extrude e from s.r distance=10 mm draft=5 deg",
    );
    let top = 10.0 - 2.0 * 10.0 * d.tan();
    near(
        p.volume,
        10.0 / 3.0 * (100.0 + top * top + 10.0 * top),
        "draft",
    );

    let p = props(
        "  extrude e from rect(on=plane.XZ, center=(0 mm, 0 mm), w=4 mm, h=6 mm) distance=2 mm",
    );
    near(p.volume, 48.0, "inline rect");
    assert!(p.bbox_min[1] < -1.99, "extruded along the XZ normal (-Y)");
    let p = props("  extrude e from circle(on=plane.XY, d=6 mm) distance=2 mm");
    near(p.volume, PI * 9.0 * 2.0, "inline circle");
}

#[test]
fn reference_planes_axes_and_points() {
    // offset plane
    let p = props(
        "  plane up offset=5 mm from=plane.XY\n  sketch s on up {\n    rect r corner=(0 mm, 0 mm) w=2 mm h=2 mm\n  }\n  extrude e from s distance=3 mm",
    );
    near(p.bbox_min[2], 5.0, "offset plane");
    near(p.bbox_max[2], 8.0, "offset plane top");
    // plane through three points: the YZ plane shifted to x = 4
    let p = props(
        "  point a at=(4 mm, 0 mm, 0 mm)\n  point b at=(4 mm, 1 mm, 0 mm)\n  point c at=(4 mm, 0 mm, 1 mm)\n  plane pl through=[a, b, c]\n  sketch s on pl {\n    rect r corner=(0 mm, 0 mm) w=2 mm h=2 mm\n  }\n  extrude e from s distance=1 mm",
    );
    near(p.bbox_min[0], 4.0, "three-point plane");
    // angled plane: XY rotated 90 degrees about the X axis is the XZ plane (normal -Y or +Y)
    let p = props(
        "  plane tilted from=plane.XY angle=90 deg about=axis.X\n  sketch s on tilted {\n    rect r corner=(0 mm, 0 mm) w=2 mm h=3 mm\n  }\n  extrude e from s distance=1 mm",
    );
    near(p.volume, 6.0, "angled plane");
    assert!(
        p.bbox_max[2] > 2.99 || p.bbox_min[2] < -2.99,
        "the 3 mm side stands up"
    );
    // axis through two points, used for a revolve
    let p = props(
        "  point a at=(0 mm, 0 mm, 0 mm)\n  point b at=(0 mm, 0 mm, 1 mm)\n  axis up through=[a, b]\n  sketch s on plane.XZ {\n    circle c center=(7 mm, 0 mm) r=2 mm\n  }\n  revolve r from s axis=up",
    );
    near(
        p.volume,
        2.0 * PI * PI * 7.0 * 4.0,
        "revolve about a named axis",
    );
    // coordinate system
    let p = props(
        "  csys cs origin=(0 mm, 0 mm, 10 mm) x=(1, 0, 0) y=(0, 1, 0)\n  sketch s on cs {\n    rect r corner=(0 mm, 0 mm) w=1 mm h=1 mm\n  }\n  extrude e from s distance=1 mm",
    );
    near(p.bbox_min[2], 10.0, "csys");
}

#[test]
fn sweep_and_loft() {
    // a straight pipe
    let p = props(
        "  sketch path on plane.XZ {\n    line l start=(0 mm, 0 mm) end=(0 mm, 10 mm)\n  }\n  sketch prof on plane.XY {\n    circle c center=(0 mm, 0 mm) r=1 mm\n  }\n  sweep pipe from prof path=path",
    );
    near(p.volume, 10.0 * PI, "pipe");
    // a quarter bend (Pappus): the profile stands on a plane perpendicular to the path start
    let p = props(
        "  sketch path on plane.XY {\n    arc a start=(5 mm, 0 mm) end=(0 mm, 5 mm) center=(0 mm, 0 mm)\n  }\n  plane start from=plane.XZ offset=0 mm\n  sketch prof on plane.XZ {\n    circle c center=(5 mm, 0 mm) r=1 mm\n  }\n  sweep elbow from prof path=path",
    );
    near(p.volume, PI * 5.0 * PI / 2.0, "elbow");

    let p = props(
        "  plane top offset=2 mm from=plane.XY\n  sketch lo on plane.XY {\n    rect r corner=(-1 mm, -1 mm) w=2 mm h=2 mm\n  }\n  sketch hi on top {\n    rect r corner=(-0.5 mm, -0.5 mm) w=1 mm h=1 mm\n  }\n  loft f sections=[lo, hi] ruled",
    );
    near(p.volume, 2.0 / 3.0 * 7.0, "ruled loft");
    let smooth = props(
        "  plane top offset=4 mm from=plane.XY\n  sketch lo on plane.XY {\n    circle c center=(0 mm, 0 mm) r=2 mm\n  }\n  sketch hi on top {\n    circle c center=(0 mm, 0 mm) r=1 mm\n  }\n  loft f sections=[lo, hi]",
    );
    // a smooth loft of two coaxial circles is a cone frustum
    near(smooth.volume, PI * 4.0 / 3.0 * (4.0 + 1.0 + 2.0), "frustum");
}

#[test]
fn configurations_change_the_geometry() {
    let src = part(
        "  param h = 10 mm\n  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=2 mm h=2 mm\n  }\n  extrude e from s distance=h\n  config default tall { h = 30 mm }\n  config short { h = 1 mm }",
    );
    let a = analyze(&src);
    assert!(!a.has_errors(), "{:#?}", a.diagnostics);
    let m = a.model.expect("model");
    let k = OcctKernel::new();
    let volume = |cfg: Option<&str>| {
        let b = build(&k, &m, cfg).expect("build");
        k.properties(&b.shape.expect("shape"))
            .expect("props")
            .volume
    };
    near(volume(None), 120.0, "the default configuration is used");
    near(volume(Some("short")), 4.0, "explicit configuration");
    near(volume(Some("tall")), 120.0, "named default");
}

#[test]
fn suppressed_features_are_skipped() {
    let p = props(
        "  param n = 0\n  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=2 mm h=2 mm\n  }\n  extrude a from s distance=1 mm\n  extrude b from s distance=5 mm suppress if n == 0",
    );
    near(p.volume, 4.0, "suppressed");
}

#[test]
fn failures_name_the_feature_and_stop() {
    let sk = "  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=2 mm h=2 mm\n  }\n";
    assert_eq!(
        codes(&format!("{sk}  extrude e from s distance=1 mm op=subtract")),
        ["E5006"]
    );
    assert_eq!(codes(&format!("{sk}  extrude e from s")), ["E5001"]);
    assert_eq!(
        codes(&format!("{sk}  extrude e from s distance=1 mm through_all")),
        ["E5001"]
    );
    assert_eq!(
        codes(&format!("{sk}  extrude e from s distance=1 mm bogus=1 mm")),
        ["E5001"]
    );
    assert_eq!(
        codes(&format!(
            "{sk}  extrude e from s distance=1 mm draft=5 deg symmetric"
        )),
        ["E5001"]
    );
    assert_eq!(
        codes(&format!("{sk}  extrude e from s through_all")),
        ["E5006"]
    );
    assert_eq!(codes(&format!("{sk}  hole h on=s.r d=1 mm")), ["E5003"]);
    let open = "  sketch o on plane.XY {\n    line a start=(0 mm, 0 mm) end=(5 mm, 0 mm)\n    line b start=(5 mm, 0 mm) end=(5 mm, 5 mm)\n  }\n  extrude e from o distance=1 mm";
    assert_eq!(codes(open), ["E5005"]);
    let d = run(open).err().unwrap();
    assert!(d[0].message.contains("not closed"), "{}", d[0].message);
    // a sketch on a face needs persistent naming
    assert_eq!(
        codes(&format!(
            "{sk}  extrude e from s distance=1 mm\n  sketch t on e.cap_end {{\n    circle c center=(1 mm, 1 mm) r=0.5 mm\n  }}\n  extrude f from t distance=1 mm"
        )),
        ["E5002"]
    );
}
