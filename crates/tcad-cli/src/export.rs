//! `tcad export`: STEP, STL, 3MF and GLB from the (cached) result.

use crate::geom::{Options, face_labels, produce, report_failure};
use serde_json::json;
use std::io::Write;
use tcad_io::ModelInfo;
use tcad_kernel::{Kernel, StepMeta};

/// Options of `export`.
pub struct ExportOptions<'a> {
    /// Which part, configuration and output style.
    pub base: Options<'a>,
    /// `step`, `stl`, `3mf` or `glb`.
    pub to: &'a str,
    /// Output file (default: `<part>[.<configuration>].<ext>` in the current directory).
    pub out: Option<&'a str>,
    /// Chordal deflection for the mesh formats (mm).
    pub linear: f64,
    /// Angular deflection for the mesh formats (degrees).
    pub angular_deg: f64,
}

/// Writes the export and reports where.
pub fn export_command<K: Kernel>(
    k: &K,
    o: &ExportOptions<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let ext = match o.to {
        "step" | "stp" => "step",
        "stl" => "stl",
        "3mf" => "3mf",
        "glb" => "glb",
        other => {
            let _ = writeln!(
                err,
                "tcad export: unknown format `{other}`; use step, stl, 3mf or glb"
            );
            return 2;
        }
    };
    if !(o.linear > 0.0 && o.angular_deg > 0.0) {
        let _ = writeln!(err, "tcad export: the deflections must be positive");
        return 2;
    }
    let p = match produce(k, &o.base) {
        Ok(p) => p,
        Err(f) => return report_failure(f, o.base.json, out, err),
    };
    let m = &p.manifest;
    let shape = match std::fs::read(p.dir.join(tcad_cache::BREP_FILE))
        .map_err(|e| e.to_string())
        .and_then(|b| k.from_brep(&b).map_err(|e| e.to_string()))
    {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(err, "tcad: cannot read the cached result: {e}");
            return 2;
        }
    };
    let info = ModelInfo {
        name: &m.part,
        configuration: m.config.as_deref(),
        material: p.material.as_deref(),
    };
    let bytes: Result<Vec<u8>, String> = match ext {
        "step" => k
            .export_step(
                &shape,
                &StepMeta {
                    name: &m.part,
                    configuration: m.config.as_deref(),
                    material: p
                        .material
                        .as_deref()
                        .map(|n| (n, p.density.map(|d| d * 1e6))),
                },
            )
            .map_err(|e| e.to_string()),
        _ => k
            .mesh(&shape, o.linear, o.angular_deg.to_radians())
            .map_err(|e| e.to_string())
            .map(|mesh| match ext {
                "stl" => tcad_io::stl_binary(&mesh, &m.part),
                "3mf" => tcad_io::threemf(&mesh, &info),
                _ => tcad_io::glb(&mesh, &face_labels(&m.features), &info),
            }),
    };
    let bytes = match bytes {
        Ok(b) => b,
        Err(e) => {
            let _ = writeln!(err, "tcad export: {e}");
            return 2;
        }
    };
    let path = o
        .out
        .map(str::to_string)
        .unwrap_or_else(|| match &m.config {
            Some(c) => format!("{}.{c}.{ext}", m.part),
            None => format!("{}.{ext}", m.part),
        });
    if let Some(dir) = std::path::Path::new(&path).parent()
        && !dir.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&path, &bytes) {
        let _ = writeln!(err, "tcad: cannot write {path}: {e}");
        return 2;
    }
    if o.base.json {
        let v = json!({
            "command": "export", "ok": true, "part": m.part, "config": m.config,
            "format": ext, "file": path, "bytes": bytes.len(), "kernel": m.kernel,
        });
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&v).unwrap_or_default()
        );
    } else {
        let _ = writeln!(out, "wrote {path} ({} bytes, {ext})", bytes.len());
    }
    0
}
