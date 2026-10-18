//! `tcad-web`: regeneration in the browser (Phase 1b, docs/adr/0014).
//!
//! The same build code as the command line, behind a small C interface for JavaScript. Compiled for
//! `wasm32-unknown-emscripten` with the `occt` feature, OpenCASCADE is a separate side module
//! (`occt.wasm`) loaded at start-up. Without the feature the mock kernel is used, which keeps the
//! crate testable everywhere.
//!
//! JavaScript calls [`tcad_build`] with the source text and a configuration name and receives JSON
//! (see [`regenerate`]); the GLB of the last successful build is read with [`tcad_glb_ptr`] and
//! [`tcad_glb_len`].

use serde_json::{Value, json};
use std::ffi::{CString, c_char};
use std::sync::Mutex;
use tcad_build::build;
use tcad_diag::Severity;
use tcad_io::{FaceLabel, ModelInfo};
use tcad_kernel::Kernel;

/// Chordal deflection of the viewer mesh (mm), as `tcad export --to glb`.
pub const LINEAR_DEFLECTION_MM: f64 = 0.05;
/// Angular deflection of the viewer mesh (radians).
pub const ANGULAR_DEFLECTION_RAD: f64 = 10.0 * std::f64::consts::PI / 180.0;

/// Builds `src` (configuration `config`, `None`: the default) and describes the result as JSON:
/// `{ok, kernel, part, config, features, result, brep_sha256, topology}` or
/// `{ok: false, diagnostics}`. The GLB of a successful build is returned separately.
pub fn regenerate<K: Kernel>(k: &K, src: &str, config: Option<&str>) -> (Value, Option<Vec<u8>>) {
    let analysis = tcad_model::analyze(src);
    let fail = |diags: &[tcad_diag::Diagnostic]| {
        json!({
            "ok": false,
            "kernel": k.id(),
            "diagnostics": diags.iter().map(|d| json!({
                "code": d.code.as_str(),
                "severity": match d.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                    Severity::Info => "info",
                },
                "message": d.message,
                "start": d.span.start,
                "end": d.span.end,
            })).collect::<Vec<_>>(),
        })
    };
    let Some(model) = analysis.model.as_ref().filter(|_| !analysis.has_errors()) else {
        return (fail(&analysis.diagnostics), None);
    };
    let built = match build(k, model, config) {
        Ok(b) => b,
        Err(d) => return (fail(&d), None),
    };
    let features: Vec<Value> = built
        .features
        .iter()
        .map(|f| {
            json!({
                "name": f.name, "keyword": f.keyword, "faces": f.topology.faces,
                "edges": f.topology.edges, "volume_mm3": f.volume,
            })
        })
        .collect();
    let topology: Vec<Value> = built
        .features
        .last()
        .map(|f| {
            f.topology_map
                .iter()
                .map(|t| json!({"kind": t.kind, "id": t.id, "aliases": t.aliases}))
                .collect()
        })
        .unwrap_or_default();
    let Some(shape) = built.shape.as_ref() else {
        let v = json!({"ok": true, "kernel": k.id(), "part": model.part.name,
            "config": built.config, "features": features, "result": Value::Null});
        return (v, None);
    };
    let result = match k.properties(shape) {
        Ok(p) => json!({
            "volume_mm3": p.volume, "area_mm2": p.area, "center_of_mass_mm": p.center_of_mass,
            "bbox_min_mm": p.bbox_min, "bbox_max_mm": p.bbox_max,
            "solids": p.topology.solids, "faces": p.topology.faces,
            "edges": p.topology.edges, "vertices": p.topology.vertices,
        }),
        Err(e) => json!({"error": e.to_string()}),
    };
    let brep = k.canonical_brep(shape).ok();
    let labels: Vec<FaceLabel> = built
        .features
        .last()
        .map(|f| {
            f.topology_map
                .iter()
                .filter(|t| t.kind == "face")
                .map(|t| FaceLabel {
                    id: t.id.clone(),
                    aliases: t.aliases.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    let material = built.material.as_ref().map(|(n, _)| n.as_str());
    let glb = k
        .mesh(shape, LINEAR_DEFLECTION_MM, ANGULAR_DEFLECTION_RAD)
        .ok()
        .map(|mesh| {
            let info = ModelInfo {
                name: &model.part.name,
                configuration: built.config.as_deref(),
                material,
            };
            tcad_io::glb(&mesh, &labels, &info)
        });
    let v = json!({
        "ok": true,
        "kernel": k.id(),
        "part": model.part.name,
        "config": built.config,
        "features": features,
        "result": result,
        "brep_sha256": brep.as_deref().map(tcad_cache::sha256_hex),
        "topology": topology,
    });
    (v, glb)
}

#[cfg(feature = "occt")]
fn kernel() -> &'static tcad_occt::OcctKernel {
    static K: std::sync::OnceLock<tcad_occt::OcctKernel> = std::sync::OnceLock::new();
    K.get_or_init(tcad_occt::OcctKernel::new)
}

#[cfg(not(feature = "occt"))]
fn kernel() -> &'static tcad_kernel::mock::MockKernel {
    &tcad_kernel::mock::MockKernel
}

static LAST_GLB: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// # Safety
/// `ptr` must point to `len` readable bytes (or be null with `len` 0).
unsafe fn text<'a>(ptr: *const u8, len: usize) -> &'a str {
    if ptr.is_null() || len == 0 {
        return "";
    }
    // SAFETY: the caller guarantees `len` readable bytes at `ptr`.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(bytes).unwrap_or("")
}

/// Builds a part. `src`/`config` are UTF-8 byte ranges (an empty configuration means the default).
/// Returns a NUL-terminated JSON string to be released with [`tcad_free`].
///
/// # Safety
/// Both ranges must be readable for their lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcad_build(
    src: *const u8,
    src_len: usize,
    config: *const u8,
    config_len: usize,
) -> *mut c_char {
    // SAFETY: forwarded from the caller's guarantee.
    let (src, config) = unsafe { (text(src, src_len), text(config, config_len)) };
    let config = (!config.is_empty()).then_some(config);
    let (json, glb) = regenerate(kernel(), src, config);
    if let Ok(mut last) = LAST_GLB.lock() {
        *last = glb.unwrap_or_default();
    }
    CString::new(json.to_string())
        .unwrap_or_default()
        .into_raw()
}

/// Releases a string returned by [`tcad_build`].
///
/// # Safety
/// `p` must come from [`tcad_build`] and not have been released yet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcad_free(p: *mut c_char) {
    if !p.is_null() {
        // SAFETY: `p` was produced by `CString::into_raw` in `tcad_build`.
        drop(unsafe { CString::from_raw(p) });
    }
}

/// Address of the GLB of the last successful build (valid until the next build).
#[unsafe(no_mangle)]
pub extern "C" fn tcad_glb_ptr() -> *const u8 {
    LAST_GLB
        .lock()
        .map(|g| g.as_ptr())
        .unwrap_or(std::ptr::null())
}

/// Length of the GLB of the last successful build (0 if there is none).
#[unsafe(no_mangle)]
pub extern "C" fn tcad_glb_len() -> usize {
    LAST_GLB.lock().map(|g| g.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: &str = "tcad 0.1\n\npart blk \"B\" {\n  param h = 3 mm\n  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=2 mm h=5 mm\n  }\n  extrude e from s.r distance=h\n}\n";

    #[test]
    fn a_block_builds_through_the_c_interface() {
        let cfg = "";
        // SAFETY: both ranges are valid for their lengths.
        let out = unsafe { tcad_build(BLOCK.as_ptr(), BLOCK.len(), cfg.as_ptr(), 0) };
        // SAFETY: `out` is a NUL-terminated string from `tcad_build`.
        let json = unsafe { std::ffi::CStr::from_ptr(out) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: released once.
        unsafe { tcad_free(out) };
        let v: Value = serde_json::from_str(&json).expect("json");
        assert_eq!(v["ok"], true, "{v}");
        assert_eq!(v["part"], "blk");
        assert!((v["result"]["volume_mm3"].as_f64().unwrap() - 30.0).abs() < 1e-9);
        assert_eq!(v["features"][0]["name"], "e");
    }

    #[test]
    fn errors_come_back_as_diagnostics() {
        let src = "tcad 0.1\n\npart p {\n  param h = nosuch\n}\n";
        let (v, glb) = regenerate(kernel(), src, None);
        assert_eq!(v["ok"], false);
        assert!(
            v["diagnostics"][0]["code"]
                .as_str()
                .unwrap()
                .starts_with('E')
        );
        assert!(glb.is_none());
    }
}
