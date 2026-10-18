//! Tests for sketch solving through the language: entities, constraints, diagnostics.

use tcad_diag::Severity;
use tcad_model::{Analysis, Geometry, analyze};

const CLEVIS: &str = include_str!("../../../spec/examples/rod_clevis.tcad");

fn sketch(body: &str) -> String {
    format!("tcad 0.1\n\npart p {{\n  sketch s on plane.XY {{\n{body}\n  }}\n}}\n")
}

fn codes(a: &Analysis) -> Vec<&'static str> {
    a.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

fn problems(a: &Analysis) -> Vec<&'static str> {
    a.diagnostics
        .iter()
        .filter(|d| d.severity != Severity::Info)
        .map(|d| d.code.as_str())
        .collect()
}

fn check(body: &str) -> Vec<&'static str> {
    problems(&analyze(&sketch(body)))
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-7, "{a} != {b}");
}

fn solved(src: &str) -> Vec<tcad_model::SolvedEntity> {
    let a = analyze(src);
    assert!(problems(&a).is_empty(), "{:#?}", a.diagnostics);
    a.model
        .expect("model")
        .solve_sketches(None)
        .expect("solve")
        .remove(0)
        .entities
}

#[test]
fn clevis_sketch_follows_its_constraints() {
    let ents = solved(CLEVIS);
    let find = |n: &str| ents.iter().find(|e| e.name == n).expect(n).geometry.clone();
    let Geometry::Rect {
        bottom_left,
        top_right,
        ..
    } = find("outline")
    else {
        panic!()
    };
    near(bottom_left[0], 0.0);
    near(bottom_left[1], -29.0);
    near(top_right[0], 70.0);
    near(top_right[1], 29.0);
    let Geometry::Circle { center, r } = find("pin") else {
        panic!()
    };
    near(center[0], 50.0);
    near(r, 10.0);
    // the nose is concentric with the pin and passes through the rectangle's right corners
    let Geometry::Arc {
        center: ac,
        start,
        end,
        r: ar,
    } = find("nose")
    else {
        panic!()
    };
    near(ac[0], center[0]);
    near(ac[1], center[1]);
    near(start[0], 70.0);
    near(end[1], 29.0);
    near(ar, (20f64.powi(2) + 29f64.powi(2)).sqrt());
}

#[test]
fn dimensions_follow_parameters_and_configurations() {
    let src = "tcad 0.1\n\npart p {\n  param w = 40 mm\n  sketch s on plane.XY {\n    \
               rect r corner=(0 mm, 0 mm) w=w h=10 mm\n  }\n  config default wide { w = 90 mm }\n}\n";
    let a = analyze(src);
    assert!(problems(&a).is_empty(), "{:#?}", a.diagnostics);
    let m = a.model.expect("model");
    let width = |cfg: Option<&str>| {
        let s = m.solve_sketches(cfg).expect("solve");
        let Geometry::Rect { bottom_right, .. } = s[0].entities[0].geometry else {
            panic!()
        };
        bottom_right[0]
    };
    near(width(None), 40.0);
    near(width(Some("wide")), 90.0);
}

#[test]
fn conflicting_constraints_are_named() {
    let a = analyze(&sketch(
        "    line l start=(0 mm, 0 mm) end=(10 mm, 0 mm)\n    constrain fix(l.start)\n    \
         constrain distance(l.start, l.end) = 10 mm\n    constrain distance(l.start, l.end) = 20 mm",
    ));
    assert_eq!(problems(&a), ["E4101"]);
    let d = a
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E4101")
        .expect("conflict");
    assert!(
        d.message.contains("distance(l.start, l.end) = 10 mm"),
        "{}",
        d.message
    );
    assert!(
        d.message.contains("distance(l.start, l.end) = 20 mm"),
        "{}",
        d.message
    );
    assert!(
        !d.message.contains("fix"),
        "the innocent constraint must not be named: {}",
        d.message
    );
    assert_eq!(d.related.len(), 1);
    assert!(
        d.span.start > d.related[0].span.start,
        "primary is the later constraint"
    );
}

#[test]
fn conflicts_that_only_exist_in_a_configuration_are_reported_with_it() {
    let src = "tcad 0.1\n\npart p {\n  param d = 10 mm\n  sketch s on plane.XY {\n    \
               line l start=(0 mm, 0 mm) end=(10 mm, 0 mm)\n    constrain fix(l.start)\n    \
               constrain distance(l.start, l.end) = d\n    constrain distance(l.start, l.end) = 10 mm\n  }\n  \
               config default odd { d = 12 mm }\n}\n";
    let a = analyze(src);
    let c = a
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E4101")
        .expect("conflict");
    assert!(
        c.message.contains("in configuration `odd`"),
        "{}",
        c.message
    );
}

#[test]
fn redundant_constraints_warn() {
    let a = analyze(&sketch(
        "    line l start=(0 mm, 0 mm) end=(10 mm, 0.5 mm)\n    constrain horizontal(l)\n    \
         constrain horizontal(l.start, l.end)",
    ));
    assert_eq!(problems(&a), ["W4102"]);
}

#[test]
fn degrees_of_freedom_are_reported() {
    let a = analyze(&sketch(
        "    line l start=(0 mm, 0 mm) end=(10 mm, 0 mm)\n    circle c center=(5 mm, 5 mm) d=2 mm",
    ));
    let info = a
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "I4201")
        .expect("info");
    assert_eq!(info.severity, Severity::Info);
    assert!(
        info.message.contains("6 degrees of freedom"),
        "{}",
        info.message
    );
    assert!(
        info.message.contains("l: 4") && info.message.contains("c: 2"),
        "{}",
        info.message
    );

    let full = analyze(&sketch(
        "    line l start=(0 mm, 0 mm) end=(10 mm, 0 mm)\n    constrain fix(l.start)\n    \
         constrain horizontal(l)\n    constrain distance(l) = 10 mm",
    ));
    assert!(!codes(&full).contains(&"I4201"), "{:?}", full.diagnostics);
    let s = full
        .model
        .expect("model")
        .solve_sketches(None)
        .expect("solve");
    assert_eq!(s[0].dof, 0);
}

#[test]
fn entity_forms() {
    let src = sketch(
        "    point p at=(1 mm, 1 mm)\n    line l start=(0 mm, 0 mm) end=p\n    \
         circle c center=l.end d=4 mm\n    arc a1 center=(0 mm, 0 mm) start=(3 mm, 0 mm) end=(0 mm, 3 mm)\n    \
         arc a2 start=(0 mm, 10 mm) end=(10 mm, 10 mm) through=(5 mm, 12 mm)\n    \
         line t start=(20 mm, 0 mm) end=(30 mm, 0 mm)\n    \
         arc a3 start=(20 mm, 0 mm) end=(25 mm, 5 mm) tangent_to=t\n    \
         ellipse e center=(0 mm, 20 mm) a=4 mm b=2 mm angle=30 deg\n    \
         slot sl start=(0 mm, 30 mm) end=(10 mm, 30 mm) w=3 mm\n    \
         polygon g center=(30 mm, 30 mm) sides=6 r=5 mm\n    \
         spline sp points=[(0 mm, 40 mm), (5 mm, 45 mm), (10 mm, 40 mm)]\n    \
         spline si through=[(0 mm, 50 mm), (5 mm, 55 mm)] closed",
    );
    let ents = solved(&src);
    let get = |n: &str| ents.iter().find(|e| e.name == n).expect(n).geometry.clone();
    // a line ending at a point shares it; a circle centered on the line's end shares that point
    let Geometry::Line { end, .. } = get("l") else {
        panic!()
    };
    near(end[0], 1.0);
    let Geometry::Circle { center, r } = get("c") else {
        panic!()
    };
    near(center[0], 1.0);
    near(r, 2.0);
    // three-point arc: radius from the circle through the points
    let Geometry::Arc { r, .. } = get("a2") else {
        panic!()
    };
    assert!(r > 5.0 && r < 10.0, "{r}");
    // tangent arc leaves the line tangentially: the center lies straight above the start
    let Geometry::Arc { center, start, .. } = get("a3") else {
        panic!()
    };
    near(center[0], start[0]);
    // ellipse, slot, polygon
    let Geometry::Ellipse { a, b, theta, .. } = get("e") else {
        panic!()
    };
    near(a, 4.0);
    near(b, 2.0);
    near(theta, 30f64.to_radians());
    let Geometry::Slot { width, .. } = get("sl") else {
        panic!()
    };
    near(width, 3.0);
    let Geometry::Polygon {
        vertices, center, ..
    } = get("g")
    else {
        panic!()
    };
    assert_eq!(vertices.len(), 6);
    for v in &vertices {
        near((v[0] - center[0]).hypot(v[1] - center[1]), 5.0);
    }
    let Geometry::Spline {
        points,
        interpolated,
        closed,
    } = get("si")
    else {
        panic!()
    };
    assert_eq!((points.len(), interpolated, closed), (2, true, true));
    let Geometry::Spline {
        interpolated,
        closed,
        ..
    } = get("sp")
    else {
        panic!()
    };
    assert!(!interpolated && !closed);
}

#[test]
fn constraints_cover_the_phase_1_set() {
    let src = sketch(
        "    line a start=(0 mm, 0 mm) end=(10 mm, 1 mm)\n    line b start=(0 mm, 5 mm) end=(9 mm, 7 mm)\n    \
         line c start=(0 mm, 10 mm) end=(2 mm, 14 mm)\n    circle k center=(20 mm, 5 mm) r=3 mm\n    \
         circle m center=(26 mm, 5 mm) r=2 mm\n    point q at=(3 mm, 3 mm)\n    point u at=(8 mm, 9 mm)\n    \
         constrain fix(a.start)\n    constrain horizontal(a)\n    constrain distance(a) = 10 mm\n    \
         constrain fix(b.start)\n    constrain parallel(a, b)\n    constrain equal(a, b)\n    \
         constrain fix(c.start)\n    constrain perpendicular(a, c)\n    constrain distance(c) = 4 mm\n    \
         constrain fix(k.center)\n    constrain tangent(k, m)\n    constrain horizontal(k.center, m.center)\n    \
         constrain midpoint(q, a)\n    constrain on_curve(u, k)\n    constrain hdistance(k.center, u) = 1 mm\n    \
         constrain angle(a, c) = 90 deg",
    );
    let a = analyze(&src);
    // `angle(a, c) = 90 deg` repeats `perpendicular(a, c)`, so exactly one redundancy is expected
    assert_eq!(problems(&a), ["W4102"], "{:#?}", a.diagnostics);
    let ents = a
        .model
        .expect("model")
        .solve_sketches(None)
        .expect("solve")
        .remove(0)
        .entities;
    let get = |n: &str| ents.iter().find(|e| e.name == n).expect(n).geometry.clone();
    let Geometry::Point(q) = get("q") else {
        panic!()
    };
    near(q[0], 5.0);
    near(q[1], 0.0);
    let Geometry::Circle { center, .. } = get("m") else {
        panic!()
    };
    near(center[0], 25.0);
    let Geometry::Line { start, end } = get("b") else {
        panic!()
    };
    near(end[1], start[1]);
    near(end[0] - start[0], 10.0);
    let Geometry::Line { start, end } = get("c") else {
        panic!()
    };
    near(end[0], start[0]);
    near((end[1] - start[1]).abs(), 4.0);
}

#[test]
fn stored_hints_select_the_branch_and_stay_nearest() {
    let two_distances = |hint: &str| {
        sketch(&format!(
            "    line base start=(0 mm, 0 mm) end=(10 mm, 0 mm)\n    constrain fix(base.start)\n    \
             constrain fix(base.end)\n    point apex {hint}\n    \
             constrain distance(base.start, apex) = 8 mm\n    constrain distance(base.end, apex) = 7 mm"
        ))
    };
    let y_of = |src: &str| {
        let e = solved(src);
        let Geometry::Point(p) = e.iter().find(|e| e.name == "apex").expect("apex").geometry else {
            panic!()
        };
        p[1]
    };
    assert!(y_of(&two_distances("@hint(5 mm, 3 mm)")) > 0.0);
    assert!(y_of(&two_distances("@hint(5 mm, -3 mm)")) < 0.0);
    // coordinates in the arguments win over a stale hint
    assert!(y_of(&two_distances("at=(5 mm, 3 mm) @hint(5 mm, -3 mm)")) > 0.0);
}

#[test]
fn invalid_sketches_are_reported() {
    assert_eq!(check("    line l start=(0 mm, 0 mm) bogus=1 mm"), ["E4001"]);
    assert_eq!(
        check("    circle c center=(0 mm, 0 mm) r=1 mm d=2 mm"),
        ["E4001"]
    );
    assert_eq!(check("    circle c center=(0, 0) r=1 mm"), ["E4001"]);
    assert_eq!(check("    circle c center=(0 mm, 0 mm) r=1 deg"), ["E4001"]);
    assert_eq!(check("    polygon g sides=2 r=1 mm"), ["E4001"]);
    assert_eq!(
        check("    arc a start=(0 mm, 0 mm) end=(1 mm, 1 mm)"),
        Vec::<&str>::new()
    );
    assert_eq!(check("    ellipse e center=(0 mm, 0 mm) a=1 mm"), ["E4001"]);
    assert_eq!(check("    spline s points=[(0 mm, 0 mm)]"), ["E4001"]);
    let c = "    line l start=(0 mm, 0 mm) end=(1 mm, 0 mm)\n";
    assert_eq!(check(&format!("{c}    constrain frobnicate(l)")), ["E4002"]);
    assert_eq!(check(&format!("{c}    constrain parallel(l)")), ["E4003"]);
    assert_eq!(check(&format!("{c}    constrain distance(l)")), ["E4003"]);
    assert_eq!(
        check(&format!("{c}    constrain horizontal(l) = 1 mm")),
        ["E4003"]
    );
    assert_eq!(
        check(&format!("{c}    constrain distance(l) = 1 deg")),
        ["E4001"]
    );
    assert_eq!(check(&format!("{c}    constrain fix(l.middle)")), ["E4003"]);
    assert_eq!(check(&format!("{c}    constrain tangent(l, l)")), ["E4003"]);
}

#[test]
fn projected_geometry_is_deferred() {
    let a = analyze(&sketch("    project edge edges=origin"));
    assert!(codes(&a).contains(&"W4601"), "{:?}", codes(&a));
}

#[test]
fn hints_round_trip_through_hint_values() {
    let ents = solved(CLEVIS);
    let pin = ents.iter().find(|e| e.name == "pin").expect("pin");
    let v = pin.hint_values();
    assert_eq!(v.len(), 3);
    near(v[0].0, 50.0);
    near(v[2].0, 10.0);
    let outline = ents.iter().find(|e| e.name == "outline").expect("outline");
    assert_eq!(outline.hint_values().len(), 8);
}
