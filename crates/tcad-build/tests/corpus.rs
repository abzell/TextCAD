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
        let shape = built.shape.expect("shape");
        let p = k.properties(&shape).unwrap();

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
            let counts = [c.solids, c.faces, c.edges, c.vertices];
            let exp =
                ["solids", "faces", "edges", "vertices"].map(|k| t[k].as_u64().unwrap() as usize);
            assert_eq!(counts, exp, "{name}: topology");
        }
        // T1: building again gives the same canonical B-rep, byte for byte
        let again = build(&k, &model, None).unwrap().shape.unwrap();
        assert_eq!(
            k.canonical_brep(&shape).unwrap(),
            k.canonical_brep(&again).unwrap(),
            "{name}: not deterministic"
        );
        checked += 1;
    }
    assert!(checked >= 8, "only {checked} measured parts found");
}
