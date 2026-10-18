//! Bit-level determinism of everything that does not need the geometry kernel: the canonical text and
//! the solved sketches of every corpus part, for every configuration. The digests in
//! `corpus/determinism.txt` are the same on Windows, Linux and WebAssembly (the CI test jobs and the
//! `wasm-run` job compare against this one file). Regenerate with `UPDATE_DETERMINISM=1 cargo test -p
//! tcad-model --test determinism` after an intended change.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tcad_model::analyze;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

fn fnv(h: &mut u64, text: &str) {
    for b in text.bytes() {
        *h ^= u64::from(b);
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }
}

fn sources(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .expect("corpus")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("expected.json").is_file())
        .collect();
    names.sort();
    for bundle in names {
        let mut files: Vec<_> = std::fs::read_dir(bundle.join("parts"))
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "tcad"))
            .collect();
        files.sort();
        for f in files {
            let name = format!(
                "{}/{}",
                bundle.file_name().unwrap().to_string_lossy(),
                f.file_name().unwrap().to_string_lossy()
            );
            out.push((name, std::fs::read_to_string(&f).expect("read")));
        }
    }
    out
}

fn digest(src: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let formatted = tcad_syntax::format_source(src).unwrap_or_else(|_| src.to_string());
    fnv(&mut h, &formatted);
    let Some(model) = analyze(src).model else {
        return h;
    };
    let mut configs: Vec<Option<String>> = vec![None];
    configs.extend(model.part.configs.iter().map(|c| Some(c.name.clone())));
    for config in configs {
        fnv(&mut h, &format!("|{config:?}|"));
        // Debug prints floats as the shortest text that round-trips, so equal text means equal bits.
        match model.solve_sketches(config.as_deref()) {
            Ok(solved) => {
                for s in solved {
                    fnv(&mut h, &s.name);
                    for e in s.entities {
                        fnv(&mut h, &format!("{}:{:?};", e.name, e.geometry));
                    }
                }
            }
            Err(d) => fnv(&mut h, &format!("{} diagnostics", d.len())),
        }
    }
    h
}

#[test]
fn corpus_digests_match_the_golden_file() {
    let golden = corpus_dir().join("determinism.txt");
    let found: BTreeMap<String, u64> = sources(&corpus_dir())
        .into_iter()
        .map(|(n, s)| (n, digest(&s)))
        .collect();
    assert!(found.len() >= 40, "{} parts", found.len());
    let text: String = found
        .iter()
        .map(|(n, h)| format!("{n} {h:016x}\n"))
        .collect();
    if std::env::var_os("UPDATE_DETERMINISM").is_some() {
        std::fs::write(&golden, text).expect("write golden");
        return;
    }
    let expected = std::fs::read_to_string(&golden).expect("corpus/determinism.txt");
    let diff: Vec<String> = text
        .lines()
        .zip(expected.lines())
        .filter(|(a, b)| a != b)
        .map(|(a, b)| format!("found {a}, expected {b}"))
        .collect();
    assert!(
        diff.is_empty() && text.lines().count() == expected.lines().count(),
        "digests differ:\n{}",
        diff.join("\n")
    );
}
