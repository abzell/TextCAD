//! The commands that need a geometry kernel: `build` and `props`.

use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use tcad_cache::{
    FeatureEntry, Manifest, ResultEntry, TopologyEntry, cache_dir, is_fresh, load_bundle,
    sha256_hex, source_hash, write_cache,
};
use tcad_diag::{Diagnostic, render_json, render_text};
use tcad_kernel::Kernel;

/// Options of `build` and `props`.
pub struct Options<'a> {
    /// A bundle directory or a `.tcad` file.
    pub path: &'a str,
    /// Configuration to build (default: the part's default).
    pub config: Option<&'a str>,
    /// Rebuild even if the cache is fresh.
    pub force: bool,
    /// JSON output.
    pub json: bool,
}

struct Produced {
    manifest: Manifest,
    fresh: bool,
    dir: PathBuf,
    density: Option<f64>,
    material: Option<String>,
}

enum Failure {
    /// Diagnostics for a source text.
    Diagnostics(String, String, Vec<Diagnostic>),
    /// Anything else (I/O, missing bundle), exit code 2.
    Other(String),
}

fn produce<K: Kernel>(k: &K, o: &Options<'_>) -> Result<Produced, Failure> {
    let bundle = load_bundle(std::path::Path::new(o.path)).map_err(Failure::Other)?;
    let name = bundle.entry.display().to_string();
    let src = std::fs::read_to_string(&bundle.entry)
        .map_err(|e| Failure::Other(format!("cannot read {name}: {e}")))?;
    let analysis = tcad_model::analyze(&src);
    let failed = analysis.has_errors();
    let Some(model) = analysis.model.filter(|_| !failed) else {
        return Err(Failure::Diagnostics(name, src, analysis.diagnostics));
    };
    let config = o
        .config
        .map(str::to_string)
        .or_else(|| model.default_config().map(|c| c.name.clone()));
    if let Some(c) = &config
        && !model.part.configs.iter().any(|x| &x.name == c)
    {
        return Err(Failure::Other(format!(
            "the part has no configuration `{c}`"
        )));
    }
    let canonical = tcad_syntax::format_source(&src).unwrap_or_else(|_| src.clone());
    let hash = source_hash(&canonical, config.as_deref(), &[]);
    let dir = cache_dir(&bundle.root, &model.part.name);
    let density = model.part.material.as_ref().and_then(|m| m.density);
    let material = model.part.material.as_ref().map(|m| m.name.clone());
    if !o.force
        && let Some(manifest) = is_fresh(&dir, &hash, &k.id())
    {
        return Ok(Produced {
            manifest,
            fresh: true,
            dir,
            density,
            material,
        });
    }
    let built = tcad_build::build(k, &model, config.as_deref())
        .map_err(|d| Failure::Diagnostics(name.clone(), src.clone(), d))?;
    let Some(shape) = &built.shape else {
        return Err(Failure::Other("the part has no solid feature".into()));
    };
    let io = |what: &str, e: tcad_kernel::KernelError| Failure::Other(format!("{what}: {e}"));
    let props = k
        .properties(shape)
        .map_err(|e| io("measuring the result", e))?;
    let brep = k
        .canonical_brep(shape)
        .map_err(|e| io("serializing the result", e))?;
    let manifest = Manifest {
        format: "0.1".into(),
        part: model.part.name.clone(),
        config: built.config.clone(),
        source_hash: hash,
        kernel: k.id(),
        tolerance_mm: built.tolerance,
        features: built
            .features
            .iter()
            .map(|f| FeatureEntry {
                name: f.name.clone(),
                keyword: f.keyword.clone(),
                solids: f.topology.solids,
                faces: f.topology.faces,
                edges: f.topology.edges,
                vertices: f.topology.vertices,
                volume_mm3: f.volume,
                thread: f.thread.clone(),
                topology: f
                    .topology_map
                    .iter()
                    .map(|t| TopologyEntry {
                        kind: t.kind.to_string(),
                        id: t.id.clone(),
                        aliases: t.aliases.clone(),
                        geometry: t.geometry.clone(),
                        size: t.size,
                        at: t.at,
                    })
                    .collect(),
            })
            .collect(),
        result: ResultEntry {
            solids: props.topology.solids,
            faces: props.topology.faces,
            edges: props.topology.edges,
            vertices: props.topology.vertices,
            volume_mm3: props.volume,
            area_mm2: props.area,
            center_of_mass_mm: props.center_of_mass,
            bbox_min_mm: props.bbox_min,
            bbox_max_mm: props.bbox_max,
        },
        brep_sha256: sha256_hex(&brep),
    };
    write_cache(&dir, &manifest, &brep)
        .map_err(|e| Failure::Other(format!("cannot write the cache {}: {e}", dir.display())))?;
    Ok(Produced {
        manifest,
        fresh: false,
        dir,
        density,
        material,
    })
}

fn report_failure(f: Failure, json: bool, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match f {
        Failure::Diagnostics(name, src, diags) => {
            if json {
                let mut v = render_json(&name, &src, &diags);
                v["ok"] = json!(false);
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&v).unwrap_or_default()
                );
            } else {
                let _ = write!(err, "{}", render_text(&name, &src, &diags));
            }
            1
        }
        Failure::Other(m) => {
            let _ = writeln!(err, "tcad: {m}");
            2
        }
    }
}

fn result_json(m: &Manifest, density: Option<f64>) -> Value {
    let r = &m.result;
    let mut v = json!({
        "solids": r.solids, "faces": r.faces, "edges": r.edges, "vertices": r.vertices,
        "volume_mm3": r.volume_mm3, "area_mm2": r.area_mm2,
        "center_of_mass_mm": r.center_of_mass_mm,
        "bbox_min_mm": r.bbox_min_mm, "bbox_max_mm": r.bbox_max_mm,
    });
    if let Some(d) = density {
        v["mass_kg"] = json!(r.volume_mm3 * d);
    }
    v
}

/// `tcad build`: regenerate and fill the cache.
pub fn build_command<K: Kernel>(
    k: &K,
    o: &Options<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let p = match produce(k, o) {
        Ok(p) => p,
        Err(f) => return report_failure(f, o.json, out, err),
    };
    let m = &p.manifest;
    if o.json {
        let v = json!({
            "command": "build", "ok": true, "part": m.part, "config": m.config,
            "kernel": m.kernel, "fresh": p.fresh, "source_hash": m.source_hash,
            "cache_dir": p.dir.display().to_string(),
            "features": m.features.iter().map(|f| json!({
                "name": f.name, "keyword": f.keyword, "solids": f.solids,
                "faces": f.faces, "edges": f.edges, "vertices": f.vertices,
                "volume_mm3": f.volume_mm3,
            })).collect::<Vec<_>>(),
            "result": result_json(m, p.density),
        });
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&v).unwrap_or_default()
        );
    } else {
        let cfg = m
            .config
            .as_ref()
            .map(|c| format!(" (configuration {c})"))
            .unwrap_or_default();
        let verb = if p.fresh { "up to date" } else { "built" };
        let _ = writeln!(
            out,
            "{verb} {}{cfg}: {} solid{}, {} faces, volume {} mm^3 [{}] -> {}",
            m.part,
            m.result.solids,
            if m.result.solids == 1 { "" } else { "s" },
            m.result.faces,
            m.result.volume_mm3,
            m.kernel,
            p.dir.display()
        );
    }
    0
}

/// `tcad props`: the properties of the result (building it first if the cache is stale).
pub fn props_command<K: Kernel>(
    k: &K,
    o: &Options<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let p = match produce(k, o) {
        Ok(p) => p,
        Err(f) => return report_failure(f, o.json, out, err),
    };
    let m = &p.manifest;
    if o.json {
        let v = json!({
            "command": "props", "ok": true, "part": m.part, "config": m.config,
            "kernel": m.kernel, "material": p.material,
            "result": result_json(m, p.density),
            "features": m.features.iter().map(|f| json!({
                "name": f.name, "keyword": f.keyword, "solids": f.solids,
                "faces": f.faces, "edges": f.edges, "vertices": f.vertices,
            })).collect::<Vec<_>>(),
        });
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&v).unwrap_or_default()
        );
        return 0;
    }
    let r = &m.result;
    let _ = writeln!(
        out,
        "part      {}{}",
        m.part,
        m.config
            .as_ref()
            .map(|c| format!(" (configuration {c})"))
            .unwrap_or_default()
    );
    let _ = writeln!(out, "volume    {:.6} mm^3", r.volume_mm3);
    let _ = writeln!(out, "area      {:.6} mm^2", r.area_mm2);
    if let (Some(d), Some(mat)) = (p.density, &p.material) {
        let _ = writeln!(out, "mass      {:.9} kg ({mat})", r.volume_mm3 * d);
    }
    let c = r.center_of_mass_mm;
    let _ = writeln!(out, "center    ({:.6}, {:.6}, {:.6}) mm", c[0], c[1], c[2]);
    let (lo, hi) = (r.bbox_min_mm, r.bbox_max_mm);
    let _ = writeln!(
        out,
        "bbox      ({:.6}, {:.6}, {:.6}) to ({:.6}, {:.6}, {:.6}) mm",
        lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
    );
    let _ = writeln!(
        out,
        "topology  {} solid(s), {} faces, {} edges, {} vertices",
        r.solids, r.faces, r.edges, r.vertices
    );
    for f in &m.features {
        let _ = writeln!(
            out,
            "  {:<10} {:<16} {} faces, {} edges",
            f.keyword, f.name, f.faces, f.edges
        );
    }
    0
}

/// `tcad refs`: every face/edge reference of the part and the persistent name(s) it resolves to.
/// Unresolved references are listed too (with the reason), so this also explains a failed build.
pub fn refs_command<K: Kernel>(
    k: &K,
    o: &Options<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let bundle = match load_bundle(std::path::Path::new(o.path)) {
        Ok(b) => b,
        Err(m) => {
            let _ = writeln!(err, "tcad: {m}");
            return 2;
        }
    };
    let name = bundle.entry.display().to_string();
    let src = match std::fs::read_to_string(&bundle.entry) {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(err, "tcad: cannot read {name}: {e}");
            return 2;
        }
    };
    let analysis = tcad_model::analyze(&src);
    let failed = analysis.has_errors();
    let Some(model) = analysis.model.filter(|_| !failed) else {
        return report_failure(
            Failure::Diagnostics(name, src, analysis.diagnostics),
            o.json,
            out,
            err,
        );
    };
    let (result, refs) = tcad_build::build_traced(k, &model, o.config);
    let index = tcad_diag::LineIndex::new(&src);
    let ok = result.is_ok();
    if o.json {
        let list: Vec<Value> = refs
            .iter()
            .map(|r| {
                let (line, col) = index.line_col(r.span.start);
                match &r.resolved {
                    Ok(names) => json!({
                        "feature": r.feature, "line": line, "column": col, "text": r.text,
                        "resolved": names,
                    }),
                    Err(m) => json!({
                        "feature": r.feature, "line": line, "column": col, "text": r.text,
                        "error": m,
                    }),
                }
            })
            .collect();
        let mut v = json!({ "command": "refs", "ok": ok, "file": name, "refs": list });
        if let Err(d) = &result {
            v["diagnostics"] = render_json(&name, &src, d)["diagnostics"].clone();
        }
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&v).unwrap_or_default()
        );
    } else {
        for r in &refs {
            let (line, col) = index.line_col(r.span.start);
            match &r.resolved {
                Ok(names) => {
                    let _ = writeln!(
                        out,
                        "{name}:{line}:{col}: {} {} -> {}",
                        r.feature,
                        r.text,
                        names.join(", ")
                    );
                }
                Err(m) => {
                    let _ = writeln!(
                        out,
                        "{name}:{line}:{col}: {} {} -> error: {m}",
                        r.feature, r.text
                    );
                }
            }
        }
        if let Err(d) = &result {
            let _ = write!(err, "{}", render_text(&name, &src, d));
        }
    }
    i32::from(!ok)
}
