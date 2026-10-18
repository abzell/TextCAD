//! `tcad-cli`: the `tcad` command-line tool. All commands support `--json` and return a non-zero
//! exit code on failure (1: the input is invalid or would change, 2: usage or I/O error).

use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use tcad_diag::{Diagnostic, has_errors, render_json, render_text};

/// Working name of the format. Defined once per crate so it can be renamed later
/// (see `spec/name.md`).
pub const FORMAT_NAME: &str = "TextCAD";

/// File extension of a part/assembly source file.
pub const FILE_EXTENSION: &str = "tcad";

/// File extension of an archived bundle.
pub const BUNDLE_EXTENSION: &str = "tcadz";

const USAGE: &str = "\
Usage: tcad <command> [options] <path>...

Commands:
  check   Parse, resolve names, units and configurations; report diagnostics (no geometry)
  fmt     Rewrite files in canonical form; with --check only report differences

Options:
  --json     Machine-readable output
  --check    (fmt) do not write, exit 1 if any file would change
  --help     Show this help
  --version  Show the version

A path is a .tcad file or a directory searched recursively for .tcad files.
";

/// Runs the CLI with the given arguments (without the program name); returns the exit code.
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut json = false;
    let mut check_only = false;
    let mut command: Option<&str> = None;
    let mut paths: Vec<&str> = Vec::new();
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            "--check" => check_only = true,
            "--help" | "-h" => {
                let _ = write!(out, "{USAGE}");
                return 0;
            }
            "--version" | "-V" => {
                let _ = writeln!(out, "tcad {}", env!("CARGO_PKG_VERSION"));
                return 0;
            }
            s if s.starts_with("--") => {
                let _ = writeln!(err, "tcad: unknown option `{s}`\n\n{USAGE}");
                return 2;
            }
            s if command.is_none() => command = Some(s),
            s => paths.push(s),
        }
    }
    let Some(command) = command else {
        let _ = write!(err, "{USAGE}");
        return 2;
    };
    if !matches!(command, "check" | "fmt") {
        let _ = writeln!(err, "tcad: unknown command `{command}`\n\n{USAGE}");
        return 2;
    }
    if check_only && command != "fmt" {
        let _ = writeln!(err, "tcad: --check only applies to `fmt`");
        return 2;
    }
    if paths.is_empty() {
        let _ = writeln!(err, "tcad {command}: expected at least one path");
        return 2;
    }
    let files = match collect_files(&paths) {
        Ok(f) => f,
        Err(e) => {
            let _ = writeln!(err, "tcad: {e}");
            return 2;
        }
    };

    let mut failed = false;
    let mut io_error = false;
    let mut reports: Vec<Value> = Vec::new();
    for file in &files {
        let name = file.display().to_string();
        let src = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                let _ = writeln!(err, "tcad: cannot read {name}: {e}");
                io_error = true;
                continue;
            }
        };
        let (status, diags) = match command {
            "check" => check_file(&src),
            _ => match fmt_file(file, &src, check_only) {
                Ok(r) => r,
                Err(e) => {
                    let _ = writeln!(err, "tcad: cannot write {name}: {e}");
                    io_error = true;
                    continue;
                }
            },
        };
        let bad = has_errors(&diags) || status == "would_change";
        failed |= bad;
        if json {
            let mut r = render_json(&name, &src, &diags);
            r["status"] = json!(status);
            reports.push(r);
        } else {
            let _ = write!(err, "{}", render_text(&name, &src, &diags));
            if status == "would_change" {
                let _ = writeln!(out, "{name}: would reformat");
            } else if status == "formatted" {
                let _ = writeln!(out, "{name}: formatted");
            }
        }
    }
    if json {
        let report = json!({ "command": command, "ok": !failed && !io_error, "files": reports });
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
    }
    if io_error { 2 } else { i32::from(failed) }
}

fn check_file(src: &str) -> (&'static str, Vec<Diagnostic>) {
    let analysis = tcad_model::analyze(src);
    let status = if analysis.has_errors() { "error" } else { "ok" };
    (status, analysis.diagnostics)
}

fn fmt_file(
    path: &Path,
    src: &str,
    check_only: bool,
) -> Result<(&'static str, Vec<Diagnostic>), std::io::Error> {
    match tcad_syntax::format_source(src) {
        Err(e) => Ok(("error", e.diagnostics)),
        Ok(formatted) => {
            let warnings = tcad_syntax::parse(src).diagnostics;
            if formatted == src {
                Ok(("unchanged", warnings))
            } else if check_only {
                Ok(("would_change", warnings))
            } else {
                std::fs::write(path, formatted)?;
                Ok(("formatted", warnings))
            }
        }
    }
}

fn collect_files(paths: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for p in paths {
        let path = Path::new(p);
        if path.is_dir() {
            walk(path, &mut files).map_err(|e| format!("cannot read {p}: {e}"))?;
        } else if path.is_file() {
            files.push(path.to_path_buf());
        } else {
            return Err(format!("no such file or directory: {p}"));
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if name != ".cache" && name != "target" && !name.starts_with('.') {
                walk(&path, out)?;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some(FILE_EXTENSION) {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);
    impl Dir {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!("tcad-cli-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).expect("create temp dir");
            Dir(p)
        }
        fn write(&self, name: &str, text: &str) -> String {
            let path = self.0.join(name);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, text).expect("write");
            path.display().to_string()
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn run_args(args: &[&str]) -> (i32, String, String) {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&args, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).expect("utf8"),
            String::from_utf8(err).expect("utf8"),
        )
    }

    const GOOD: &str = "tcad 0.1\n\npart p {\n  param a = 1 mm\n}\n";
    const UGLY: &str = "tcad 0.1\npart p {\nparam a=1.0mm\n}\n";

    #[test]
    fn check_ok_and_error() {
        let d = Dir::new("check");
        let good = d.write("good.tcad", GOOD);
        let bad = d.write("bad.tcad", "tcad 0.1\npart p {\n  param a 5\n}\n");
        assert_eq!(run_args(&["check", &good]).0, 0);
        let (code, _, err) = run_args(&["check", &bad]);
        assert_eq!(code, 1);
        assert!(err.contains("error[E1102]"), "{err}");
    }

    #[test]
    fn check_reports_semantic_errors() {
        let d = Dir::new("sem");
        let f = d.write(
            "units.tcad",
            "tcad 0.1\npart p {\n  param a = 1 mm + 1 deg\n}\n",
        );
        let (code, _, err) = run_args(&["check", &f]);
        assert_eq!(code, 1);
        assert!(err.contains("error[E2201]"), "{err}");
        let (_, out, _) = run_args(&["check", "--json", &f]);
        let v: Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["files"][0]["diagnostics"][0]["code"], "E2201");
    }

    #[test]
    fn json_output() {
        let d = Dir::new("json");
        let bad = d.write("bad.tcad", "tcad 0.1\npart p {\n  param a 5\n}\n");
        let (code, out, _) = run_args(&["check", "--json", &bad]);
        assert_eq!(code, 1);
        let v: Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["ok"], false);
        assert_eq!(v["files"][0]["diagnostics"][0]["code"], "E1102");
    }

    #[test]
    fn fmt_check_then_write() {
        let d = Dir::new("fmt");
        let f = d.write("ugly.tcad", UGLY);
        let (code, out, _) = run_args(&["fmt", "--check", &f]);
        assert_eq!(code, 1);
        assert!(out.contains("would reformat"));
        assert_eq!(std::fs::read_to_string(&f).expect("read"), UGLY);
        assert_eq!(run_args(&["fmt", &f]).0, 0);
        assert_eq!(std::fs::read_to_string(&f).expect("read"), GOOD);
        assert_eq!(run_args(&["fmt", "--check", &f]).0, 0);
    }

    #[test]
    fn directories_are_searched_but_cache_is_skipped() {
        let d = Dir::new("dir");
        d.write("parts/a.tcad", GOOD);
        d.write(".cache/skip.tcad", "garbage");
        let root = d.0.display().to_string();
        let (code, out, _) = run_args(&["check", "--json", &root]);
        assert_eq!(code, 0);
        let v: Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["files"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn usage_errors() {
        assert_eq!(run_args(&[]).0, 2);
        assert_eq!(run_args(&["frob", "x"]).0, 2);
        assert_eq!(run_args(&["check"]).0, 2);
        assert_eq!(run_args(&["check", "/no/such/path.tcad"]).0, 2);
        assert_eq!(run_args(&["check", "--check", "x"]).0, 2);
        assert_eq!(run_args(&["--version"]).0, 0);
    }
}
