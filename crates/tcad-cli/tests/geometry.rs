//! `tcad build` and `tcad props`, with the mock kernel (always) and OpenCASCADE (`--features occt`).

use serde_json::Value;
use std::path::{Path, PathBuf};
use tcad_cli::run_with_kernel;
use tcad_kernel::mock::MockKernel;

struct Dir(PathBuf);
impl Dir {
    fn new(tag: &str) -> Dir {
        let p = std::env::temp_dir().join(format!("tcad-geom-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("mkdir");
        Dir(p)
    }
    fn write(&self, name: &str, text: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        std::fs::write(&p, text).expect("write");
        p
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn part(h: &str) -> String {
    format!(
        "tcad 0.1\n\npart blk {{\n  material \"steel\" density=7.85 g/cm^3\n  param h = {h}\n  sketch s on plane.XY {{\n    rect r corner=(0 mm, 0 mm) w=2 mm h=3 mm\n  }}\n  extrude e from s.r distance=h\n  config default tall {{ h = 40 mm }}\n}}\n"
    )
}

fn exec(cmd: &str, path: &Path, config: Option<&str>, json: bool) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_with_kernel(
        &MockKernel,
        cmd,
        &path.display().to_string(),
        config,
        json,
        &mut out,
        &mut err,
    );
    (
        code,
        String::from_utf8(out).expect("utf8"),
        String::from_utf8(err).expect("utf8"),
    )
}

#[test]
fn build_writes_the_cache_then_reports_it_up_to_date_and_notices_edits() {
    let d = Dir::new("cache");
    let f = d.write("blk.tcad", &part("4 mm"));
    let (code, out, err) = exec("build", &f, None, true);
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(v["fresh"], false);
    assert_eq!(v["config"], "tall", "the default configuration is built");
    assert_eq!(v["result"]["volume_mm3"], 240.0, "2 x 3 x 40");
    let cache = d.0.join(".cache/blk");
    assert!(cache.join("brep.brep").is_file() && cache.join("manifest.json").is_file());

    let (_, out, _) = exec("build", &f, None, true);
    let v: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(v["fresh"], true, "nothing changed");

    // another configuration is a different source hash
    let (_, out, _) = exec("build", &f, Some("tall"), true);
    assert_eq!(
        serde_json::from_str::<Value>(&out).expect("json")["fresh"],
        true
    );

    // an edit invalidates the cache
    d.write("blk.tcad", &part("5 mm").replace("h = 40 mm", "h = 50 mm"));
    let (_, out, _) = exec("build", &f, None, true);
    let v: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(v["fresh"], false);
    assert_eq!(v["result"]["volume_mm3"], 300.0);

    // a damaged B-rep invalidates it too
    std::fs::write(cache.join("brep.brep"), b"damaged").expect("write");
    let (_, out, _) = exec("build", &f, None, true);
    assert_eq!(
        serde_json::from_str::<Value>(&out).expect("json")["fresh"],
        false
    );
}

#[test]
fn props_reports_mass_from_the_material() {
    let d = Dir::new("props");
    let f = d.write("blk.tcad", &part("4 mm"));
    let (code, out, _) = exec("props", &f, Some("tall"), true);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).expect("json");
    let mass = v["result"]["mass_kg"].as_f64().expect("mass");
    // 240 mm^3 of 7.85 g/cm^3 steel
    assert!((mass - 240.0 * 7.85e-6).abs() < 1e-12, "{mass}");
    let (_, text, _) = exec("props", &f, Some("tall"), false);
    assert!(text.contains("volume    240.000000 mm^3"), "{text}");
    assert!(text.contains("mass      "), "{text}");
}

#[test]
fn bundles_are_found_through_bundle_toml() {
    let d = Dir::new("bundle");
    d.write(
        "bundle.toml",
        "format = \"0.1\"\nentry = \"parts/blk.tcad\"\n",
    );
    d.write("parts/blk.tcad", &part("4 mm"));
    let (code, _, err) = exec("build", &d.0, None, false);
    assert_eq!(code, 0, "{err}");
    assert!(d.0.join(".cache/blk/manifest.json").is_file());
    // pointing at the part file finds the same bundle root
    let (_, out, _) = exec("build", &d.0.join("parts/blk.tcad"), None, false);
    assert!(out.starts_with("up to date blk"), "{out}");
}

#[test]
fn failures_are_reported_with_codes_and_exit_codes() {
    let d = Dir::new("fail");
    // syntax error
    let f = d.write("bad.tcad", "tcad 0.1\npart p {\n  param a 5\n}\n");
    let (code, _, err) = exec("build", &f, None, false);
    assert_eq!(code, 1);
    assert!(err.contains("error[E1102]"), "{err}");
    // a feature the kernel cannot do (the mock has no booleans)
    let src = "tcad 0.1\n\npart p {\n  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=2 mm h=2 mm\n  }\n  extrude a from s.r distance=2 mm\n  extrude b from s.r distance=1 mm op=subtract\n}\n";
    let f = d.write("two.tcad", src);
    let (code, out, _) = exec("build", &f, None, true);
    assert_eq!(code, 1);
    let v: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(v["ok"], false);
    assert_eq!(v["diagnostics"][0]["code"], "E5004");
    assert!(
        v["diagnostics"][0]["message"]
            .as_str()
            .expect("message")
            .contains("`b`")
    );
    // unknown configuration, missing file
    let f = d.write("blk.tcad", &part("4 mm"));
    assert_eq!(exec("build", &f, Some("nope"), false).0, 2);
    assert_eq!(exec("build", &d.0.join("missing.tcad"), None, false).0, 2);
}

#[cfg(feature = "occt")]
mod with_occt {
    use super::*;
    use tcad_occt::OcctKernel;

    #[test]
    fn a_real_build_and_a_stable_brep_hash() {
        let d = Dir::new("occt");
        let f = d.write("blk.tcad", &part("4 mm"));
        let go = |force: bool| {
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let o = format!("{}", f.display());
            let args: Vec<String> = ["build", "--json", "--config", "tall"]
                .iter()
                .chain(if force { &["--force"][..] } else { &[][..] })
                .map(|s| s.to_string())
                .chain([o])
                .collect();
            let code = tcad_cli::run(&args, &mut out, &mut err);
            assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
            serde_json::from_slice::<Value>(&out).expect("json")
        };
        let first = go(false);
        assert!(
            first["kernel"]
                .as_str()
                .expect("kernel")
                .starts_with("occt-7.")
        );
        let v = first["result"]["volume_mm3"].as_f64().expect("volume");
        assert!((v - 240.0).abs() < 1e-9);
        let hash = |name: &str| {
            let m: Value = serde_json::from_slice(
                &std::fs::read(d.0.join(".cache/blk").join(name)).expect("read"),
            )
            .expect("json");
            m["brep_sha256"].as_str().expect("hash").to_string()
        };
        let h1 = hash("manifest.json");
        // a forced rebuild yields the identical canonical B-rep (determinism tier T1)
        let second = go(true);
        assert_eq!(second["fresh"], false);
        assert_eq!(hash("manifest.json"), h1);
        let _ = OcctKernel::new();
    }
}
