//! Builds every corpus part that states measured properties and compares them with
//! `expected.json`: volume and area to 1e-6 relative, center of mass and bounding box to 1e-5 mm
//! (determinism tier T2), topology counts exactly, and the canonical B-rep byte for byte across
//! two builds (tier T1). Needs `--features occt`.
#![cfg(feature = "occt")]

use serde_json::Value;
use std::path::PathBuf;
use tcad_build::build;
use tcad_kernel::Kernel;
use tcad_model::analyze;
use tcad_occt::OcctKernel;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

fn vec3(v: &Value) -> [f64; 3] {
    let a = v.as_array().expect("array");
    [
        a[0].as_f64().unwrap(),
        a[1].as_f64().unwrap(),
        a[2].as_f64().unwrap(),
    ]
}

/// Persistent names of the faces and edges after the last feature, sorted.
fn named(b: &tcad_build::Built<<OcctKernel as Kernel>::Shape>) -> Vec<String> {
    let mut v: Vec<String> = b
        .features
        .last()
        .map(|f| f.topology_map.iter().map(|e| e.id.clone()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Replaces the value of `param NAME = ...` lines (keeping a trailing `range(...)`).
fn with_params(src: &str, set: &serde_json::Map<String, Value>) -> String {
    src.lines()
        .map(|line| {
            let t = line.trim_start();
            if let Some(rest) = t.strip_prefix("param ") {
                let name = rest.split_whitespace().next().unwrap_or("");
                if let Some(v) = set.get(name) {
                    let indent = &line[..line.len() - t.len()];
                    let tail = rest.find(" range(").map_or("", |i| &rest[i..]);
                    return format!("{indent}param {name} = {}{tail}", v.as_str().unwrap());
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// Naming-robustness sweep: at every swept parameter set the part must build (so every reference
/// resolves), keep exactly the same persistent names and match the closed-form volume and box.
fn sweep(
    k: &OcctKernel,
    name: &str,
    src: &str,
    base: &tcad_build::Built<<OcctKernel as Kernel>::Shape>,
    sweeps: &[Value],
) {
    let base_names = named(base);
    assert!(!base_names.is_empty(), "{name}: no names recorded");
    for sw in sweeps {
        let set = sw["set"].as_object().unwrap();
        let text = with_params(src, set);
        let a = analyze(&text);
        assert!(!a.has_errors(), "{name} {set:?}: {:#?}", a.diagnostics);
        let built =
            build(k, &a.model.unwrap(), None).unwrap_or_else(|d| panic!("{name} {set:?}: {d:#?}"));
        let p = k.properties(built.shape.as_ref().unwrap()).unwrap();
        let exp = sw["volume_mm3"].as_f64().unwrap();
        assert!(
            (p.volume - exp).abs() <= 1e-6 * exp.max(1.0),
            "{name} {set:?}: volume {} vs {exp}",
            p.volume
        );
        for (key, got) in [("bbox_min_mm", p.bbox_min), ("bbox_max_mm", p.bbox_max)] {
            for (g, e) in got.iter().zip(vec3(&sw[key])) {
                assert!((g - e).abs() <= 1e-5, "{name} {set:?}: {key} {got:?}");
            }
        }
        assert_eq!(
            named(&built),
            base_names,
            "{name} {set:?}: the names changed"
        );
    }
}

#[test]
fn every_measured_corpus_part_matches() {
    let k = OcctKernel::new();
    let mut names: Vec<String> = std::fs::read_dir(corpus_dir())
        .expect("corpus")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| corpus_dir().join(n).join("expected.json").is_file())
        .collect();
    names.sort();
    let mut checked = 0;
    for name in names {
        let dir = corpus_dir().join(&name);
        let want: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("expected.json")).unwrap())
                .unwrap();
        if want.get("volume_mm3").is_none() {
            continue;
        }
        let src = std::fs::read_to_string(dir.join("parts").join(format!("{name}.tcad"))).unwrap();
        let a = analyze(&src);
        assert!(!a.has_errors(), "{name}: {:#?}", a.diagnostics);
        let model = a.model.unwrap();
        let built = build(&k, &model, None).unwrap_or_else(|d| panic!("{name}: {d:#?}"));
        let shape = built.shape.as_ref().expect("shape");
        let p = k.properties(shape).unwrap();

        let rel = |got: f64, exp: f64, what: &str| {
            assert!(
                (got - exp).abs() <= 1e-6 * exp.abs().max(1.0),
                "{name}: {what} {got} vs {exp}"
            );
        };
        rel(p.volume, want["volume_mm3"].as_f64().unwrap(), "volume");
        rel(p.area, want["area_mm2"].as_f64().unwrap(), "area");
        for (key, got) in [
            ("center_of_mass_mm", p.center_of_mass),
            ("bbox_min_mm", p.bbox_min),
            ("bbox_max_mm", p.bbox_max),
        ] {
            for (g, e) in got.iter().zip(vec3(&want[key])) {
                assert!(
                    (g - e).abs() <= 1e-5,
                    "{name}: {key} {got:?} vs {:?}",
                    want[key]
                );
            }
        }
        if let Some(t) = want.get("topology") {
            let c = p.topology;
            let got = [c.solids, c.faces, c.edges, c.vertices];
            for (k, g) in ["solids", "faces", "edges", "vertices"].iter().zip(got) {
                if let Some(e) = t.get(*k).and_then(Value::as_u64) {
                    assert_eq!(g, e as usize, "{name}: {k}");
                }
            }
        }
        if let Some(sweeps) = want.get("sweeps").and_then(Value::as_array) {
            sweep(&k, &name, &src, &built, sweeps);
        }
        // round trip: export STEP, import it again as a dead solid, properties within T2
        let step = k
            .export_step(
                shape,
                &tcad_kernel::StepMeta {
                    name: &name,
                    ..Default::default()
                },
            )
            .unwrap_or_else(|e| panic!("{name}: STEP export: {e}"));
        let back = k
            .import_step(&step)
            .unwrap_or_else(|e| panic!("{name}: STEP import: {e}"));
        let q = k.properties(&back).unwrap();
        rel(q.volume, p.volume, "STEP round trip volume");
        rel(q.area, p.area, "STEP round trip area");
        for i in 0..3 {
            assert!(
                (q.center_of_mass[i] - p.center_of_mass[i]).abs() <= 1e-5,
                "{name}: STEP round trip center of mass"
            );
            assert!(
                (q.bbox_min[i] - p.bbox_min[i]).abs() <= 1e-5
                    && (q.bbox_max[i] - p.bbox_max[i]).abs() <= 1e-5,
                "{name}: STEP round trip box"
            );
        }
        // T1: building again gives the same canonical B-rep, byte for byte (three builds in all)
        let first = k.canonical_brep(shape).unwrap();
        for _ in 0..2 {
            let again = build(&k, &model, None).unwrap().shape.unwrap();
            assert_eq!(
                first,
                k.canonical_brep(&again).unwrap(),
                "{name}: not deterministic"
            );
        }
        checked += 1;
    }
    assert!(checked >= 27, "only {checked} measured parts found");
}
