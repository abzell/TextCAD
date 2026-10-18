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

    #[test]
    fn refs_lists_what_every_reference_resolves_to_and_why_one_fails() {
        let d = Dir::new("refs");
        let good = "tcad 0.1\n\npart p {\n  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=20 mm h=10 mm\n  }\n  extrude boss from s.r distance=5 mm\n  fillet f edges=[edge(boss.cap_end & boss.side(s.r.bottom))] r=1 mm\n}\n";
        let f = d.write("p.tcad", good);
        let k = OcctKernel::new();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with_kernel(
            &k,
            "refs",
            &f.display().to_string(),
            None,
            true,
            &mut out,
            &mut err,
        );
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
        let v: Value = serde_json::from_slice(&out).expect("json");
        assert_eq!(v["refs"][0]["feature"], "f");
        assert_eq!(
            v["refs"][0]["resolved"][0],
            "edge(boss.cap_end & boss.side(s.r.bottom))"
        );
        let bad = good.replace("s.r.bottom", "s.r.botom");
        let f = d.write("q.tcad", &bad);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with_kernel(
            &k,
            "refs",
            &f.display().to_string(),
            None,
            true,
            &mut out,
            &mut err,
        );
        assert_eq!(code, 1, "{}", String::from_utf8_lossy(&err));
        let v: Value = serde_json::from_slice(&out).expect("json");
        assert!(
            v["refs"][0]["error"]
                .as_str()
                .expect("error")
                .contains("s.r.bottom")
        );
        assert_eq!(v["diagnostics"][0]["code"], "E3001");
    }

    fn tcad(args: &[&str]) -> (i32, String, String) {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = tcad_cli::run(&args, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).expect("utf8"),
            String::from_utf8(err).expect("utf8"),
        )
    }

    const PLATE: &str = "tcad 0.1\n\npart plate {\n  material \"steel\" density=7.85 g/cm^3\n  param w = 30 mm\n  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=w h=20 mm\n  }\n  extrude body from s.r distance=10 mm\n  hole bore on=body.cap_end at=(10 mm, 10 mm) d=6 mm end=through_all\n}\n";

    #[test]
    fn exports_in_every_format_and_the_cache_has_step_and_glb() {
        let d = Dir::new("export");
        let f = d.write("plate.tcad", PLATE);
        let path = f.display().to_string();
        let (code, _, err) = tcad(&["build", &path]);
        assert_eq!(code, 0, "{err}");
        assert!(d.0.join(".cache/plate/result.step").is_file());
        assert!(d.0.join(".cache/plate/mesh.glb").is_file());
        for (fmt, magic) in [
            ("step", &b"ISO-10303"[..]),
            ("glb", b"glTF"),
            ("3mf", b"PK\x03\x04"),
        ] {
            let out = d.0.join(format!("out/plate.{fmt}"));
            let (code, stdout, err) = tcad(&[
                "export",
                &path,
                "--to",
                fmt,
                "--out",
                &out.display().to_string(),
            ]);
            assert_eq!(code, 0, "{fmt}: {err}");
            assert!(stdout.contains("wrote"), "{stdout}");
            let bytes = std::fs::read(&out).expect("file");
            assert!(bytes.starts_with(magic), "{fmt}");
        }
        let stl = d.0.join("plate.stl");
        let (code, _, err) = tcad(&[
            "export",
            &path,
            "--to",
            "stl",
            "--out",
            &stl.display().to_string(),
            "--linear",
            "0.01",
        ]);
        assert_eq!(code, 0, "{err}");
        let bytes = std::fs::read(&stl).expect("stl");
        let tris = u32::from_le_bytes(bytes[80..84].try_into().expect("count")) as usize;
        assert_eq!(bytes.len(), 84 + 50 * tris);
        assert!(tris > 12);
        // errors are reported, not guessed
        assert_eq!(tcad(&["export", &path, "--to", "iges"]).0, 2);
        assert_eq!(tcad(&["export", &path]).0, 2);
        assert_eq!(
            tcad(&["export", &path, "--to", "stl", "--linear", "-1"]).0,
            2
        );
    }

    #[test]
    fn step_round_trips_through_the_import_statement() {
        let d = Dir::new("roundtrip");
        let f = d.write("plate.tcad", PLATE);
        let path = f.display().to_string();
        let (_, props, _) = tcad(&["props", "--json", &path]);
        let want: Value = serde_json::from_str(&props).expect("json");
        let step = d.0.join("imports/plate.step");
        assert_eq!(
            tcad(&[
                "export",
                &path,
                "--to",
                "step",
                "--out",
                &step.display().to_string()
            ])
            .0,
            0
        );
        let g = d.write(
            "copy.tcad",
            "tcad 0.1\n\npart copy {\n  import src = step(\"imports/plate.step\")\n  combine whole with=src\n}\n",
        );
        let (code, props, err) = tcad(&["props", "--json", &g.display().to_string()]);
        assert_eq!(code, 0, "{err}");
        let got: Value = serde_json::from_str(&props).expect("json");
        for key in ["volume_mm3", "area_mm2"] {
            let (a, b) = (
                got["result"][key].as_f64().unwrap(),
                want["result"][key].as_f64().unwrap(),
            );
            assert!((a - b).abs() <= 1e-6 * b, "{key}: {a} vs {b}");
        }
        for key in ["center_of_mass_mm", "bbox_min_mm", "bbox_max_mm"] {
            for i in 0..3 {
                let (a, b) = (
                    got["result"][key][i].as_f64().unwrap(),
                    want["result"][key][i].as_f64().unwrap(),
                );
                assert!((a - b).abs() <= 1e-5, "{key}[{i}]: {a} vs {b}");
            }
        }
        // the cache is fresh until the imported file changes
        let copy = g.display().to_string();
        let (_, first, _) = tcad(&["build", "--json", &copy]);
        let first: Value = serde_json::from_str(&first).expect("json");
        assert_eq!(first["fresh"], true);
        let wide = d.write("wide/plate.tcad", &PLATE.replace("w = 30 mm", "w = 40 mm"));
        assert_eq!(
            tcad(&[
                "export",
                &wide.display().to_string(),
                "--to",
                "step",
                "--out",
                &step.display().to_string()
            ])
            .0,
            0
        );
        let (_, again, _) = tcad(&["build", "--json", &copy]);
        let again: Value = serde_json::from_str(&again).expect("json");
        assert_eq!(again["fresh"], false);
        let (v1, v2) = (
            first["result"]["volume_mm3"].as_f64().unwrap(),
            again["result"]["volume_mm3"].as_f64().unwrap(),
        );
        assert!(v2 > v1 + 100.0, "{v1} -> {v2}");
        // an import outside the bundle is refused
        let h = d.write(
            "bad.tcad",
            "tcad 0.1\n\npart bad {\n  import src = step(\"../x.step\")\n  combine whole with=src\n}\n",
        );
        assert_eq!(tcad(&["build", &h.display().to_string()]).0, 2);
    }

    #[test]
    fn props_reports_mass_and_inertia() {
        let d = Dir::new("mass");
        let f = d.write("plate.tcad", PLATE);
        let (code, out, err) = tcad(&["props", "--json", &f.display().to_string()]);
        assert_eq!(code, 0, "{err}");
        let v: Value = serde_json::from_str(&out).expect("json");
        let vol = 30.0 * 20.0 * 10.0 - std::f64::consts::PI * 9.0 * 10.0;
        let mass = v["result"]["mass_kg"].as_f64().expect("mass");
        assert!((mass - vol * 7.85e-6).abs() < 1e-9, "{mass}");
        let i = &v["result"]["inertia_kg_mm2"];
        assert!(i[0][0].as_f64().unwrap() > 0.0 && i[2][2].as_f64().unwrap() > 0.0);
        // symmetric tensor
        assert!((i[0][1].as_f64().unwrap() - i[1][0].as_f64().unwrap()).abs() < 1e-9);
        let (_, text, _) = tcad(&["props", &f.display().to_string()]);
        assert!(text.contains("inertia"), "{text}");
    }

    #[test]
    fn verify_reports_tiers_and_diff_reports_changes() {
        let d = Dir::new("verify");
        let f = d.write("plate.tcad", PLATE);
        let path = f.display().to_string();
        // no cache yet
        assert_eq!(tcad(&["verify", &path]).0, 2);
        assert_eq!(tcad(&["build", &path]).0, 0);
        let (code, out, _) = tcad(&["verify", "--json", &path]);
        assert_eq!(code, 0);
        let v: Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["tier"], "T1");
        // a corrupted cache is not silently accepted
        let brep = d.0.join(".cache/plate/brep.brep");
        let mut bytes = std::fs::read(&brep).expect("brep");
        let n = bytes.len() / 2;
        bytes[n] = bytes[n].wrapping_add(1);
        std::fs::write(&brep, bytes).expect("write");
        let (code, out, _) = tcad(&["verify", "--json", &path]);
        let v: Value = serde_json::from_str(&out).expect("json");
        assert_eq!(code, 1, "{out}");
        assert_eq!(v["tier"], "T3");
        // an edited source makes the cache stale
        let g = d.write("plate.tcad", &PLATE.replace("w = 30 mm", "w = 40 mm"));
        let (code, _, err) = tcad(&["verify", &g.display().to_string()]);
        assert_eq!(code, 2);
        assert!(err.contains("different source"), "{err}");

        // diff
        let a = d.write("a/plate.tcad", PLATE);
        let b = d.write(
            "b/plate.tcad",
            &PLATE
                .replace("w = 30 mm", "w = 40 mm")
                .replace("d=6 mm", "d=8 mm"),
        );
        let (code, out, err) = tcad(&[
            "diff",
            "--json",
            &a.display().to_string(),
            &b.display().to_string(),
        ]);
        assert_eq!(code, 1, "{err}");
        let v: Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["parameters"][0]["name"], "w");
        assert_eq!(v["features"][0]["change"], "changed");
        assert!(v["geometry"]["volume_mm3"]["delta"].as_f64().unwrap() > 0.0);
        assert_eq!(v["references"].as_array().unwrap().len(), 0);
        let (code, out, _) = tcad(&["diff", &a.display().to_string(), &a.display().to_string()]);
        assert_eq!(code, 0);
        assert!(out.contains("no differences"), "{out}");
        let (code, out, _) = tcad(&[
            "diff",
            "--no-geometry",
            &a.display().to_string(),
            &b.display().to_string(),
        ]);
        assert_eq!(code, 1);
        assert!(out.contains("w: 30 mm -> 40 mm"), "{out}");
    }
}
