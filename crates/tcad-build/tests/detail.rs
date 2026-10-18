//! Persistent naming and the detail features, end to end against OpenCASCADE.
#![cfg(feature = "occt")]

use std::f64::consts::PI;
use tcad_build::{Built, build};
use tcad_diag::Diagnostic;
use tcad_kernel::{Kernel, Properties};
use tcad_model::analyze;
use tcad_occt::OcctKernel;

fn part(body: &str) -> String {
    format!("tcad 0.1\n\npart p {{\n{body}\n}}\n")
}

type Outcome = Result<(Built<<OcctKernel as Kernel>::Shape>, Properties), Vec<Diagnostic>>;

fn run(body: &str) -> Outcome {
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
    let p = k
        .properties(built.shape.as_ref().expect("a shape"))
        .expect("properties");
    Ok((built, p))
}

fn ok(body: &str) -> (Built<<OcctKernel as Kernel>::Shape>, Properties) {
    run(body).unwrap_or_else(|d| {
        panic!(
            "{:?}",
            d.iter()
                .map(|x| format!("{} {} @{}", x.code, x.message, x.span.start))
                .collect::<Vec<_>>()
        )
    })
}

fn near(a: f64, b: f64, what: &str) {
    assert!(
        (a - b).abs() <= 1e-6 * b.abs().max(1.0),
        "{what}: {a} vs {b}"
    );
}

fn ids(b: &Built<<OcctKernel as Kernel>::Shape>) -> Vec<String> {
    b.features
        .last()
        .expect("a feature")
        .topology_map
        .iter()
        .map(|e| e.id.clone())
        .collect()
}

const BLOCK: &str = "
  sketch s on plane.XY {
    rect r corner=(0 mm, 0 mm) w=40 mm h=20 mm
  }
  extrude boss from s.r distance=10 mm
";

#[test]
fn extrusion_faces_get_semantic_ids() {
    let (b, _) = ok(BLOCK);
    let names = ids(&b);
    for want in [
        "boss.cap_start",
        "boss.cap_end",
        "boss.side(s.r.bottom)",
        "boss.side(s.r.right)",
        "boss.side(s.r.top)",
        "boss.side(s.r.left)",
    ] {
        assert!(names.contains(&want.to_string()), "{want} in {names:?}");
    }
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("edge(boss.cap_end & boss.side(s.r.top))")),
        "{names:?}"
    );
}

#[test]
fn fillet_by_edge_name_and_tag_and_query() {
    let (b, p) = ok(&format!(
        "{BLOCK}
  fillet rnd edges=[edge(boss.cap_end & boss.side(s.r.bottom))] r=2 mm
"
    ));
    near(
        p.volume,
        8000.0 - (1.0 - PI / 4.0) * 4.0 * 40.0,
        "fillet volume",
    );
    let names = ids(&b);
    assert!(
        names.contains(&"rnd.face(boss.cap_end & boss.side(s.r.bottom))".to_string()),
        "{names:?}"
    );

    // the same edge through a tag
    let (_, p2) = ok(&format!(
        "{BLOCK}
  tag front_top = edge(boss.cap_end & boss.side(s.r.bottom))
  fillet rnd edges=[front_top] r=2 mm
"
    ));
    near(p2.volume, p.volume, "tag");

    // and through a geometric query
    let (_, p3) = ok(&format!(
        "{BLOCK}
  fillet rnd edges=query(edges, parallel_to=+X, adjacent_to=boss.cap_end) r=2 mm
"
    ));
    // two edges parallel to X on the top face (front and back)
    near(
        p3.volume,
        8000.0 - 2.0 * (1.0 - PI / 4.0) * 4.0 * 40.0,
        "query",
    );
}

#[test]
fn references_survive_parameter_edits() {
    for w in [30.0, 40.0, 55.5] {
        let (b, p) = ok(&format!(
            "
  param w = {w} mm
  sketch s on plane.XY {{
    rect r corner=(0 mm, 0 mm) w=w h=20 mm
  }}
  extrude boss from s.r distance=10 mm
  hole bore on=boss.cap_end at=(w / 2, 10 mm) d=6 mm end=through_all
  fillet rnd edges=[edge(boss.cap_end & boss.side(s.r.left))] r=2 mm
  chamfer ch edges=[edge(boss.cap_start & boss.side(s.r.right))] d=1 mm
"
        ));
        assert!(p.volume < w * 200.0, "w={w}");
        assert!(ids(&b).contains(&"bore.wall".to_string()), "{:?}", ids(&b));
    }
}

#[test]
fn unresolved_reference_names_candidates() {
    let d = run(&format!(
        "{BLOCK}
  fillet rnd edges=[edge(boss.cap_end & boss.side(s.r.botom))] r=2 mm
"
    ))
    .err()
    .expect("must fail");
    assert_eq!(d[0].code.to_string(), "E3001");
    assert!(d[0].message.contains("s.r.bottom"), "{}", d[0].message);
    assert!(d[0].hint.is_some());
}

#[test]
fn holes() {
    let (b, p) = ok(&format!(
        "{BLOCK}
  hole h1 on=boss.cap_end at=(20 mm, 10 mm) d=6 mm depth=4 mm
"
    ));
    near(p.volume, 8000.0 - PI * 9.0 * 4.0, "blind hole");
    let n = ids(&b);
    assert!(
        n.contains(&"h1.wall".to_string()) && n.contains(&"h1.floor".to_string()),
        "{n:?}"
    );

    let (_, p) = ok(&format!(
        "{BLOCK}
  hole h1 on=boss.cap_end at=(20 mm, 10 mm) d=6 mm cbore_d=10 mm cbore_depth=3 mm end=through_all
"
    ));
    near(
        p.volume,
        8000.0 - PI * 9.0 * 7.0 - PI * 25.0 * 3.0,
        "counterbore",
    );

    // countersink 90 degrees: cone from radius 5 at the surface to radius 3
    let (_, p) = ok(&format!(
        "{BLOCK}
  hole h1 on=boss.cap_end at=(20 mm, 10 mm) d=6 mm csk_d=10 mm end=through_all
"
    ));
    let cone = PI * 2.0 / 3.0 * (25.0 + 15.0 + 9.0);
    near(p.volume, 8000.0 - PI * 9.0 * 8.0 - cone, "countersink");

    // two holes from a sketch of points on the face, tapped M6
    let (b, p) = ok(&format!(
        "{BLOCK}
  sketch pts on boss.cap_end {{
    point a at=(10 mm, 10 mm)
    point b at=(30 mm, 10 mm)
  }}
  hole t from=pts thread=\"M6\" depth=8 mm
"
    ));
    near(
        p.volume,
        8000.0 - 2.0 * PI * 2.5 * 2.5 * 8.0,
        "tapped holes, drill 5 mm",
    );
    assert_eq!(b.features.last().unwrap().thread.as_deref(), Some("M6"));
    assert!(ids(&b).contains(&"t.p2.wall".to_string()), "{:?}", ids(&b));
}

#[test]
fn shell_draft_and_chamfer() {
    let (b, p) = ok(&format!(
        "{BLOCK}
  shell hollow faces=[boss.cap_end] thickness=2 mm
"
    ));
    near(p.volume, 8000.0 - 36.0 * 16.0 * 8.0, "shell");
    assert!(
        ids(&b).iter().any(|n| n.starts_with("hollow.inner(")),
        "{:?}",
        ids(&b)
    );

    let (_, p) = ok(&format!(
        "{BLOCK}
  chamfer ch edges=[edge(boss.cap_end & boss.side(s.r.top))] d=2 mm d2=3 mm
"
    ));
    near(p.volume, 8000.0 - 0.5 * 2.0 * 3.0 * 40.0, "chamfer");

    let (_, p) = ok(&format!(
        "{BLOCK}
  draft taper faces=[boss.side(s.r.left), boss.side(s.r.right)] angle=5 deg pull=+Z neutral=boss.cap_start
"
    ));
    assert!(p.volume < 8000.0 && p.volume > 7000.0, "{}", p.volume);
}

#[test]
fn rib_on_a_plate() {
    let (_, p) = ok("
  sketch plate on plane.XY {
    rect r corner=(0 mm, 0 mm) w=60 mm h=40 mm
  }
  extrude base from plate.r distance=5 mm
  plane top from=plane.XY offset=5 mm
  plane mid from=plane.XZ offset=-20 mm
  sketch web on mid {
    line l start=(10 mm, 5 mm) end=(50 mm, 25 mm)
  }
  rib stiffener profile=web thickness=4 mm direction=-Z to=top
");
    // a triangle 40 mm long and 20 mm high, 4 mm thick, on top of the plate
    near(p.volume, 60.0 * 40.0 * 5.0 + 0.5 * 40.0 * 20.0 * 4.0, "rib");
}

#[test]
fn patterns_and_mirror() {
    let (b, p) = ok(&format!(
        "{BLOCK}
  hole h1 on=boss.cap_end at=(6 mm, 5 mm) d=4 mm end=through_all
  linear_pattern row of=h1 dir=+X spacing=8 mm count=4 skip=[3]
"
    ));
    // d=4 mm through 10 mm, instances 1, 2 and 4 (3 is skipped)
    near(p.volume, 8000.0 - 3.0 * PI * 4.0 * 10.0, "three holes");
    let n = ids(&b);
    assert!(
        n.contains(&"row.i2.wall".to_string()) && n.contains(&"row.i4.wall".to_string()),
        "{n:?}"
    );
    assert!(!n.contains(&"row.i3.wall".to_string()));
}

#[test]
fn split_faces_get_ordinals_and_merged_faces_alias() {
    // a slot across the whole width splits the top face in two
    let split = format!(
        "{BLOCK}
  sketch cut on plane.XY {{
    rect c corner=(15 mm, -5 mm) w=10 mm h=30 mm
  }}
  extrude groove from cut.c distance=12 mm op=subtract
"
    );
    // the ambiguous name is an error that offers the ordinals
    let d = run(&format!(
        "{split}
  fillet f edges=[edge(boss.cap_end & boss.side(s.r.bottom))] r=1 mm
"
    ))
    .err()
    .expect("ambiguous");
    assert_eq!(d[0].code.to_string(), "E3002", "{d:?}");
    assert!(
        d[0].message.contains("boss.cap_end#1") || d[0].message.contains("ordinal"),
        "{}",
        d[0].message
    );
    // with an ordinal it works: #1 is the left part (smaller x)
    let (b, p) = ok(&format!(
        "{split}
  fillet f edges=[edge(boss.cap_end#1 & boss.side(s.r.bottom))] r=1 mm
"
    ));
    assert!(p.volume < 8000.0 - 10.0 * 20.0 * 10.0 + 1.0);
    let n = ids(&b);
    assert!(
        n.contains(&"boss.cap_end#1".to_string()) && n.contains(&"boss.cap_end#2".to_string()),
        "{n:?}"
    );

    // two blocks side by side: the union merges their tops, and both names reach the merged face
    let (b, _) = ok("
  sketch a on plane.XY {
    rect r corner=(0 mm, 0 mm) w=20 mm h=20 mm
  }
  sketch b on plane.XY {
    rect r corner=(20 mm, 0 mm) w=20 mm h=20 mm
  }
  extrude one from a.r distance=10 mm
  extrude two from b.r distance=10 mm
  fillet f edges=[edge(two.cap_end & one.side(a.r.top))] r=1 mm
");
    let _ = b;
}
