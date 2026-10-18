//! Runs every golden corpus entry (`corpus/<name>/expected.json`).

use serde_json::Value;
use std::path::PathBuf;
use tcad_diag::Severity;
use tcad_model::{Geometry, analyze};

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

fn point_of(g: &Geometry, role: &str) -> Option<[f64; 2]> {
    Some(match (g, role) {
        (Geometry::Point(p), "at") => *p,
        (Geometry::Line { start, .. }, "start") => *start,
        (Geometry::Line { end, .. }, "end") => *end,
        (Geometry::Circle { center, .. }, "center")
        | (Geometry::Arc { center, .. }, "center")
        | (Geometry::Ellipse { center, .. }, "center") => *center,
        (Geometry::Arc { start, .. }, "start") => *start,
        (Geometry::Arc { end, .. }, "end") => *end,
        (Geometry::Rect { bottom_left, .. }, "bottom_left") => *bottom_left,
        (Geometry::Rect { bottom_right, .. }, "bottom_right") => *bottom_right,
        (Geometry::Rect { top_right, .. }, "top_right") => *top_right,
        (Geometry::Rect { top_left, .. }, "top_left") => *top_left,
        (Geometry::Slot { start, .. }, "start") => *start,
        (Geometry::Slot { end, .. }, "end") => *end,
        (Geometry::Spline { points, .. }, r) => {
            *points.get(r.strip_prefix('p')?.parse::<usize>().ok()?)?
        }
        _ => return None,
    })
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn every_corpus_entry_matches_its_expectations() {
    let mut names: Vec<String> = std::fs::read_dir(corpus_dir())
        .expect("corpus dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("expected.json").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert!(
        names.len() >= 6,
        "expected at least the six sketch parts, found {names:?}"
    );

    for name in names {
        let dir = corpus_dir().join(&name);
        let src = std::fs::read_to_string(dir.join("parts").join(format!("{name}.tcad")))
            .expect("source");
        let want: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("expected.json")).expect("expected"),
        )
        .expect("json");
        let a = analyze(&src);

        let got: Vec<&str> = a
            .diagnostics
            .iter()
            .filter(|d| d.severity != Severity::Info)
            .map(|d| d.code.as_str())
            .collect();
        assert_eq!(
            got,
            strings(&want["codes"]),
            "{name}: diagnostics {:#?}",
            a.diagnostics
        );
        let info: Vec<&str> = a
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Info)
            .map(|d| d.code.as_str())
            .collect();
        assert_eq!(info, strings(&want["info"]), "{name}: information");

        if let Some(conflict) = a.diagnostics.iter().find(|d| d.code.as_str() == "E4101") {
            for s in strings(&want["conflict"]) {
                assert!(
                    conflict.message.contains(&s),
                    "{name}: `{s}` missing from {}",
                    conflict.message
                );
            }
            for s in strings(&want["not_named"]) {
                assert!(
                    !conflict.message.contains(&s),
                    "{name}: `{s}` wrongly named: {}",
                    conflict.message
                );
            }
        }
        if want["fails"].as_bool() == Some(true) {
            assert!(a.has_errors(), "{name} must fail");
            continue;
        }
        assert!(!a.has_errors(), "{name}: {:#?}", a.diagnostics);

        let model = a.model.expect("model");
        let start = std::time::Instant::now();
        let sketches = model.solve_sketches(None).expect("solve");
        let elapsed = start.elapsed();
        let sketch = &sketches[0];
        if let Some(dof) = want["dof"].as_u64() {
            assert_eq!(sketch.dof as u64, dof, "{name}: degrees of freedom");
        }
        if let Some(n) = want["entity_count"].as_u64() {
            assert_eq!(sketch.entities.len() as u64, n, "{name}: entity count");
        }
        if let Some(ents) = want["entities"].as_object() {
            for (en, spec) in ents {
                let e = sketch
                    .entities
                    .iter()
                    .find(|e| &e.name == en)
                    .unwrap_or_else(|| panic!("{name}: no {en}"));
                if let Some(d) = spec["dof"].as_u64() {
                    assert_eq!(e.dof as u64, d, "{name}: dof of {en}");
                }
            }
        }
        if let Some(points) = want["points"].as_object() {
            for (key, xy) in points {
                let (en, role) = key.split_once('.').expect("entity.role");
                let e = sketch
                    .entities
                    .iter()
                    .find(|e| e.name == en)
                    .unwrap_or_else(|| panic!("{name}: no {en}"));
                let p = point_of(&e.geometry, role)
                    .unwrap_or_else(|| panic!("{name}: {key} is not a point"));
                let want_xy: Vec<f64> = xy
                    .as_array()
                    .expect("pair")
                    .iter()
                    .filter_map(Value::as_f64)
                    .collect();
                for (g, w) in p.iter().zip(&want_xy) {
                    assert!(
                        (g - w).abs() < 1e-6,
                        "{name}: {key} is {p:?}, expected {want_xy:?}"
                    );
                }
            }
        }
        if name == "sketch_200_entities" && !cfg!(debug_assertions) {
            assert!(elapsed.as_millis() < 50, "{name} took {elapsed:?}");
        }
    }
}
