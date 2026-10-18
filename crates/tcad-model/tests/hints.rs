//! `fmt --solve`: writing solved coordinates back as hints.

use tcad_model::{Geometry, analyze, solve_hints};

const CLEVIS: &str = include_str!("fixtures/clevis_sketch.tcad");

fn geometries(src: &str) -> Vec<(String, Geometry)> {
    let a = analyze(src);
    assert!(!a.has_errors(), "{:#?}", a.diagnostics);
    a.model
        .expect("model")
        .solve_sketches(None)
        .expect("solve")
        .remove(0)
        .entities
        .into_iter()
        .map(|e| (e.name, e.geometry))
        .collect()
}

#[test]
fn every_entity_gets_a_hint_and_the_result_is_stable() {
    let once = solve_hints(CLEVIS).expect("solve_hints");
    for name in ["outline", "pin", "nose"] {
        let line = once
            .lines()
            .find(|l| l.contains(&format!(" {name} ")))
            .expect(name);
        assert!(line.contains("@hint("), "{line}");
        assert!(line.trim_end().ends_with(')'), "hint must be last: {line}");
    }
    // formatting the result changes nothing, and solving it again changes nothing
    assert_eq!(tcad_syntax::format_source(&once).expect("format"), once);
    assert_eq!(solve_hints(&once).expect("again"), once);
}

#[test]
fn hints_do_not_change_the_solution() {
    let before = geometries(CLEVIS);
    let after = geometries(&solve_hints(CLEVIS).expect("solve_hints"));
    assert_eq!(before.len(), after.len());
    let flat = |g: &Geometry| -> Vec<f64> {
        match g {
            Geometry::Rect {
                bottom_left,
                bottom_right,
                top_right,
                top_left,
            } => [bottom_left, bottom_right, top_right, top_left]
                .iter()
                .flat_map(|p| p.to_vec())
                .collect(),
            Geometry::Circle { center, r } => vec![center[0], center[1], *r],
            Geometry::Arc {
                center,
                start,
                end,
                r,
            } => {
                vec![center[0], center[1], start[0], start[1], end[0], end[1], *r]
            }
            _ => Vec::new(),
        }
    };
    for ((n1, g1), (n2, g2)) in before.iter().zip(&after) {
        assert_eq!(n1, n2);
        for (a, b) in flat(g1).iter().zip(flat(g2)) {
            assert!((a - b).abs() < 1e-6, "{n1}: {a} vs {b}");
        }
    }
}

#[test]
fn the_stored_branch_survives_a_parameter_change() {
    // apex is the intersection of two circles; the file stores the lower solution
    let src = |d: &str| {
        format!(
            "tcad 0.1\n\npart p {{\n  param d = {d}\n  sketch s on plane.XY {{\n    \
             line base start=(0 mm, 0 mm) end=(10 mm, 0 mm)\n    point apex at=(5 mm, -3 mm)\n    \
             constrain fix(base.start)\n    constrain fix(base.end)\n    \
             constrain distance(base.start, apex) = d\n    constrain distance(base.end, apex) = 7 mm\n  }}\n}}\n"
        )
    };
    let solved = solve_hints(&src("8 mm")).expect("solve_hints");
    assert!(solved.contains("@hint(5"), "{solved}");
    // keep only the stored hint: remove the explicit coordinates so the hint decides the branch
    let hinted = solved.replace("at=(5 mm, -3 mm) ", "");
    let hinted = hinted.replace("param d = 8 mm", "param d = 8.5 mm");
    let Geometry::Point(p) = geometries(&hinted)
        .into_iter()
        .find(|(n, _)| n == "apex")
        .expect("apex")
        .1
    else {
        panic!()
    };
    assert!(p[1] < 0.0, "the apex flipped: {p:?}");
}

#[test]
fn invalid_files_are_refused() {
    let bad = "tcad 0.1\n\npart p {\n  sketch s on plane.XY {\n    circle c bogus=1 mm\n  }\n}\n";
    assert!(solve_hints(bad).is_err());
}
