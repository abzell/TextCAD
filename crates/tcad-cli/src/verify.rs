//! `tcad verify`: regenerate and compare with the cache (determinism tiers, spec/kernel.md).

use crate::geom::{Options, Source, prepare, regenerate, report_failure};
use serde_json::{Value, json};
use std::io::Write;
use tcad_cache::{BREP_FILE, Manifest, read_manifest, sha256_hex};
use tcad_kernel::Kernel;

fn rel_close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * b.abs().max(1.0)
}

/// Compares two manifests at tier T2. Returns what differs, per feature and for the result.
fn compare(cached: &Manifest, fresh: &Manifest) -> Vec<String> {
    let tol = (10.0 * fresh.tolerance_mm).max(1e-5);
    let mut problems = Vec::new();
    if cached.features.len() != fresh.features.len() {
        problems.push(format!(
            "the number of features changed: {} cached, {} now",
            cached.features.len(),
            fresh.features.len()
        ));
    }
    for (a, b) in cached.features.iter().zip(&fresh.features) {
        let counts = |f: &tcad_cache::FeatureEntry| [f.solids, f.faces, f.edges, f.vertices];
        if a.name != b.name || a.keyword != b.keyword {
            problems.push(format!(
                "feature `{}` ({}) is now `{}` ({})",
                a.name, a.keyword, b.name, b.keyword
            ));
        } else if counts(a) != counts(b) {
            problems.push(format!(
                "feature `{}`: topology {:?} cached, {:?} now (solids, faces, edges, vertices)",
                a.name,
                counts(a),
                counts(b)
            ));
        } else if !rel_close(b.volume_mm3, a.volume_mm3) {
            problems.push(format!(
                "feature `{}`: volume {} cached, {} now",
                a.name, a.volume_mm3, b.volume_mm3
            ));
        }
    }
    let (r, n) = (&cached.result, &fresh.result);
    if !rel_close(n.volume_mm3, r.volume_mm3) {
        problems.push(format!(
            "volume {} cached, {} now",
            r.volume_mm3, n.volume_mm3
        ));
    }
    if !rel_close(n.area_mm2, r.area_mm2) {
        problems.push(format!("area {} cached, {} now", r.area_mm2, n.area_mm2));
    }
    for (name, a, b) in [
        ("center of mass", r.center_of_mass_mm, n.center_of_mass_mm),
        ("bounding box minimum", r.bbox_min_mm, n.bbox_min_mm),
        ("bounding box maximum", r.bbox_max_mm, n.bbox_max_mm),
    ] {
        if a.iter().zip(&b).any(|(x, y)| (x - y).abs() > tol) {
            problems.push(format!("{name} {a:?} cached, {b:?} now"));
        }
    }
    problems
}

/// Regenerates the part and compares with the cache: T1 (identical canonical B-rep), T2
/// (equivalent within tolerance), T3 (failed, with a per-feature report).
pub fn verify_command<K: Kernel>(
    k: &K,
    o: &Options<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let s: Source = match prepare(o) {
        Ok(s) => s,
        Err(f) => return report_failure(f, o.json, out, err),
    };
    let Some(cached) = read_manifest(&s.dir) else {
        let _ = writeln!(
            err,
            "tcad verify: there is no cache for `{}`; run `tcad build` first",
            s.model.part.name
        );
        return 2;
    };
    let cached_brep = std::fs::read(s.dir.join(BREP_FILE)).unwrap_or_default();
    let intact = sha256_hex(&cached_brep) == cached.brep_sha256;
    let (tier, problems, stale): (&str, Vec<String>, bool);
    if cached.source_hash != s.hash {
        (tier, problems, stale) = ("stale", Vec::new(), true);
    } else {
        stale = false;
        let fresh = match regenerate(k, &s) {
            Ok(f) => f,
            Err(f) => return report_failure(f, o.json, out, err),
        };
        let identical =
            intact && cached.kernel == fresh.manifest.kernel && cached_brep == fresh.brep;
        if identical {
            (tier, problems) = ("T1", Vec::new());
        } else {
            let mut p = compare(&cached, &fresh.manifest);
            if !intact {
                p.insert(
                    0,
                    "the cached B-rep does not match its recorded hash".into(),
                );
            }
            if p.is_empty() {
                (tier, problems) = ("T2", p);
            } else {
                (tier, problems) = ("T3", p);
            }
        }
    }
    let ok = matches!(tier, "T1" | "T2");
    if o.json {
        let v: Value = json!({
            "command": "verify", "ok": ok, "tier": tier, "stale": stale,
            "part": s.model.part.name, "cached_kernel": cached.kernel,
            "problems": problems,
        });
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&v).unwrap_or_default()
        );
    } else {
        match tier {
            "T1" => {
                let _ = writeln!(
                    out,
                    "{}: T1 identical (canonical B-rep matches byte for byte)",
                    s.model.part.name
                );
            }
            "T2" => {
                let _ = writeln!(
                    out,
                    "{}: T2 equivalent (same topology, properties within tolerance; the B-rep differs, for example across kernel versions)",
                    s.model.part.name
                );
            }
            "stale" => {
                let _ = writeln!(
                    err,
                    "{}: the cache was built from a different source; run `tcad build`",
                    s.model.part.name
                );
            }
            _ => {
                let _ = writeln!(out, "{}: T3 failed", s.model.part.name);
                for p in &problems {
                    let _ = writeln!(out, "  {p}");
                }
            }
        }
    }
    if stale { 2 } else { i32::from(!ok) }
}
