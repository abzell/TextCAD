//! `tcad diff`: what changed between two versions of a part, in terms of the design (parameters,
//! features, references) and of the geometry.

use crate::geom::{Options, Source, prepare, regenerate, report_failure};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use tcad_kernel::Kernel;

/// Options of `diff`.
pub struct DiffOptions<'a> {
    /// The old version.
    pub a: &'a str,
    /// The new version.
    pub b: &'a str,
    /// Configuration to compare (default: each part's default).
    pub config: Option<&'a str>,
    /// Compare only the text, without regenerating geometry.
    pub no_geometry: bool,
    /// JSON output.
    pub json: bool,
}

/// Canonical text of every statement by name, in source order.
fn statements(s: &Source) -> Vec<(String, String)> {
    let canonical = tcad_syntax::format_source(&s.src).unwrap_or_else(|_| s.src.clone());
    let model = tcad_model::analyze(&canonical).model;
    let Some(model) = model else {
        return Vec::new();
    };
    model
        .part
        .items
        .iter()
        .map(|i| {
            let text = canonical
                .get(i.span.start..i.span.end)
                .unwrap_or("")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            (format!("{} {}", i.keyword, i.name), text)
        })
        .collect()
}

fn params(s: &Source) -> BTreeMap<String, String> {
    s.model
        .part
        .params
        .iter()
        .map(|p| (p.name.clone(), p.value.to_string()))
        .collect()
}

fn num_delta(a: f64, b: f64) -> Value {
    json!({ "from": a, "to": b, "delta": b - a })
}

/// Runs the comparison and prints it. Exit code 0: no differences, 1: differences, 2: error.
pub fn diff_command<K: Kernel>(
    k: &K,
    o: &DiffOptions<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let load = |path| {
        prepare(&Options {
            path,
            config: o.config,
            force: false,
            json: o.json,
        })
    };
    let (a, b) = match (load(o.a), load(o.b)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(f), _) | (_, Err(f)) => return report_failure(f, o.json, out, err),
    };

    // parameters
    let (pa, pb) = (params(&a), params(&b));
    let mut param_changes = Vec::new();
    for name in pa
        .keys()
        .chain(pb.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        let (x, y) = (pa.get(name), pb.get(name));
        if x != y {
            param_changes.push(json!({ "name": name, "from": x, "to": y }));
        }
    }

    // statements
    let (sa, sb) = (statements(&a), statements(&b));
    let (ma, mb): (BTreeMap<_, _>, BTreeMap<_, _>) =
        (sa.iter().cloned().collect(), sb.iter().cloned().collect());
    let mut feature_changes = Vec::new();
    for (key, _) in &sa {
        match mb.get(key) {
            None => feature_changes.push(json!({ "change": "removed", "statement": key })),
            Some(t) if t != &ma[key] => feature_changes.push(json!({
                "change": "changed", "statement": key, "from": ma[key], "to": t,
            })),
            _ => {}
        }
    }
    for (key, _) in &sb {
        if !ma.contains_key(key) {
            feature_changes.push(json!({ "change": "added", "statement": key }));
        }
    }
    let order = |v: &[(String, String)], other: &BTreeMap<String, String>| -> Vec<String> {
        v.iter()
            .filter(|(k, _)| other.contains_key(k))
            .map(|(k, _)| k.clone())
            .collect()
    };
    if order(&sa, &mb) != order(&sb, &ma) {
        feature_changes.push(json!({ "change": "reordered" }));
    }

    // geometry and references
    let mut ref_changes = Vec::new();
    let mut geometry = Value::Null;
    let mut feature_volumes = Vec::new();
    if !o.no_geometry {
        let (fa, fb) = match (regenerate(k, &a), regenerate(k, &b)) {
            (Ok(x), Ok(y)) => (x, y),
            // a part that does not build is reported with its diagnostics
            (Err(f), _) | (_, Err(f)) => return report_failure(f, o.json, out, err),
        };
        let key = |r: &tcad_build::RefRecord| format!("{} {}", r.feature, r.text);
        let to_map = |refs: &[tcad_build::RefRecord]| -> BTreeMap<String, Vec<String>> {
            refs.iter()
                .map(|r| (key(r), r.resolved.clone().unwrap_or_default()))
                .collect()
        };
        let (ra, rb) = (to_map(&fa.refs), to_map(&fb.refs));
        for (kname, v) in &ra {
            match rb.get(kname) {
                None => ref_changes.push(json!({ "change": "removed", "reference": kname })),
                Some(w) if w != v => ref_changes.push(json!({
                    "change": "now resolves differently", "reference": kname, "from": v, "to": w,
                })),
                _ => {}
            }
        }
        for kname in rb.keys() {
            if !ra.contains_key(kname) {
                ref_changes.push(json!({ "change": "added", "reference": kname }));
            }
        }
        let (r, n) = (&fa.manifest.result, &fb.manifest.result);
        let vec3 = |x: [f64; 3], y: [f64; 3]| json!({ "from": x, "to": y, "delta": [y[0] - x[0], y[1] - x[1], y[2] - x[2]] });
        geometry = json!({
            "volume_mm3": num_delta(r.volume_mm3, n.volume_mm3),
            "area_mm2": num_delta(r.area_mm2, n.area_mm2),
            "center_of_mass_mm": vec3(r.center_of_mass_mm, n.center_of_mass_mm),
            "bbox_min_mm": vec3(r.bbox_min_mm, n.bbox_min_mm),
            "bbox_max_mm": vec3(r.bbox_max_mm, n.bbox_max_mm),
            "faces": num_delta(r.faces as f64, n.faces as f64),
        });
        let fvb: BTreeMap<&str, f64> = fb
            .manifest
            .features
            .iter()
            .map(|f| (f.name.as_str(), f.volume_mm3))
            .collect();
        for f in &fa.manifest.features {
            if let Some(&vb) = fvb.get(f.name.as_str())
                && (vb - f.volume_mm3).abs() > 1e-6 * f.volume_mm3.abs().max(1.0)
            {
                feature_volumes.push(json!({
                    "feature": f.name, "volume_mm3": num_delta(f.volume_mm3, vb),
                }));
            }
        }
    }
    let geometry_changed = geometry.as_object().is_some_and(|g| {
        g.values()
            .any(|v| v["delta"] != json!(0.0) && v["delta"] != json!([0.0, 0.0, 0.0]))
    });
    let different = !param_changes.is_empty()
        || !feature_changes.is_empty()
        || !ref_changes.is_empty()
        || geometry_changed;
    if o.json {
        let v = json!({
            "command": "diff", "ok": true, "different": different,
            "parameters": param_changes, "features": feature_changes,
            "references": ref_changes, "geometry": geometry,
            "feature_volumes": feature_volumes,
        });
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&v).unwrap_or_default()
        );
    } else if !different {
        let _ = writeln!(out, "no differences");
    } else {
        let s = |v: &Value| v.as_str().map_or_else(|| v.to_string(), str::to_string);
        if !param_changes.is_empty() {
            let _ = writeln!(out, "parameters:");
            for c in &param_changes {
                let _ = writeln!(
                    out,
                    "  {}: {} -> {}",
                    s(&c["name"]),
                    s(&c["from"]),
                    s(&c["to"])
                );
            }
        }
        if !feature_changes.is_empty() {
            let _ = writeln!(out, "features:");
            for c in &feature_changes {
                let mark = match c["change"].as_str().unwrap_or("") {
                    "added" => "+",
                    "removed" => "-",
                    "changed" => "~",
                    _ => "*",
                };
                let _ = writeln!(out, "  {mark} {}", s(&c["statement"]));
                if c["change"] == "changed" {
                    let _ = writeln!(out, "      was: {}", s(&c["from"]));
                    let _ = writeln!(out, "      now: {}", s(&c["to"]));
                }
            }
        }
        if !ref_changes.is_empty() {
            let _ = writeln!(out, "references:");
            for c in &ref_changes {
                let _ = writeln!(out, "  {} {}", s(&c["change"]), s(&c["reference"]));
                if c.get("from").is_some() {
                    let _ = writeln!(out, "      was: {}", s(&c["from"]));
                    let _ = writeln!(out, "      now: {}", s(&c["to"]));
                }
            }
        }
        if geometry_changed {
            let _ = writeln!(out, "geometry:");
            for key in ["volume_mm3", "area_mm2", "faces"] {
                let g = &geometry[key];
                if g["delta"] != json!(0.0) {
                    let _ = writeln!(
                        out,
                        "  {key}: {} -> {} ({:+})",
                        s(&g["from"]),
                        s(&g["to"]),
                        g["delta"].as_f64().unwrap_or(0.0)
                    );
                }
            }
            for key in ["center_of_mass_mm", "bbox_min_mm", "bbox_max_mm"] {
                let g = &geometry[key];
                if g["delta"] != json!([0.0, 0.0, 0.0]) {
                    let _ = writeln!(out, "  {key}: {} -> {}", g["from"], g["to"]);
                }
            }
            for f in &feature_volumes {
                let g = &f["volume_mm3"];
                let _ = writeln!(
                    out,
                    "  after {}: volume {} -> {}",
                    s(&f["feature"]),
                    s(&g["from"]),
                    s(&g["to"])
                );
            }
        }
    }
    i32::from(different)
}
