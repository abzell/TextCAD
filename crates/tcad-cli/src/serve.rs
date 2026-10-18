//! `tcad serve`: a local HTTP server for the browser viewer (spec/viewer.md).
//!
//! The server only reads: it builds parts through the same commands as the command line (so the
//! cache is shared), serves the result as GLB with the persistent name of every face, and tells
//! the page when a source file changed so it can reload. It listens on the loopback interface only
//! and serves only the `.tcad` files of the bundle it was started on.

use serde_json::{Value, json};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tcad_kernel::Kernel;

const VIEWER_HTML: &str = include_str!("../assets/viewer.html");
// Built from `viewer/` (`npm run build`); CI checks that the checked-in bundle is current.
const VIEWER_JS: &str = include_str!("../assets/viewer.js");

/// Options of `serve`.
pub struct ServeOptions<'a> {
    /// A bundle directory or a `.tcad` file.
    pub path: &'a str,
    /// TCP port on 127.0.0.1 (0: pick a free one).
    pub port: u16,
}

/// Binds the port, announces the address and serves until the process ends.
pub fn serve_command<K: Kernel>(
    k: &K,
    o: &ServeOptions<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let listener = match TcpListener::bind(("127.0.0.1", o.port)) {
        Ok(l) => l,
        Err(e) => {
            let _ = writeln!(err, "tcad serve: cannot listen on port {}: {e}", o.port);
            return 2;
        }
    };
    let addr = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    let _ = writeln!(out, "serving {} at http://{addr}/ (Ctrl-C to stop)", o.path);
    let _ = out.flush();
    serve_on(k, &listener, o.path, None);
    0
}

/// Serves requests from `listener`; `limit` bounds the number of connections (for tests).
pub fn serve_on<K: Kernel>(k: &K, listener: &TcpListener, path: &str, limit: Option<usize>) {
    let mut served = 0;
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        handle(k, stream, path);
        served += 1;
        if limit.is_some_and(|n| served >= n) {
            return;
        }
    }
}

fn handle<K: Kernel>(k: &K, mut stream: TcpStream, root: &str) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 16384 {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let mut words = head.lines().next().unwrap_or("").split_whitespace();
    let (method, target) = (words.next().unwrap_or(""), words.next().unwrap_or("/"));
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (status, ctype, body) = if method != "GET" {
        ("405 Method Not Allowed", "text/plain", b"GET only".to_vec())
    } else {
        route(k, root, path, query)
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}

type Reply = (&'static str, &'static str, Vec<u8>);

fn json_reply(v: &Value) -> Reply {
    ("200 OK", "application/json", v.to_string().into_bytes())
}

fn not_found(what: &str) -> Reply {
    ("404 Not Found", "text/plain", what.as_bytes().to_vec())
}

fn route<K: Kernel>(k: &K, root: &str, path: &str, query: &str) -> Reply {
    match path {
        "/" | "/index.html" => (
            "200 OK",
            "text/html; charset=utf-8",
            VIEWER_HTML.as_bytes().to_vec(),
        ),
        "/viewer.js" => (
            "200 OK",
            "text/javascript; charset=utf-8",
            VIEWER_JS.as_bytes().to_vec(),
        ),
        "/api/parts" => json_reply(
            &json!({"parts": parts(root).iter().map(|(rel, _)| rel).collect::<Vec<_>>()}),
        ),
        "/api/version" => json_reply(&json!({"version": version(root)})),
        "/api/part" | "/api/model.glb" => {
            let Some(file) = param(query, "file").and_then(|rel| {
                parts(root)
                    .into_iter()
                    .find(|(r, _)| *r == rel)
                    .map(|(_, p)| p)
            }) else {
                return not_found("no such part");
            };
            let config = param(query, "config").filter(|c| !c.is_empty());
            if path == "/api/part" {
                json_reply(&part_info(k, &file, config.as_deref()))
            } else {
                match model(k, &file, config.as_deref()) {
                    Ok(b) => ("200 OK", "model/gltf-binary", b),
                    Err(m) => ("500 Internal Server Error", "text/plain", m.into_bytes()),
                }
            }
        }
        _ => not_found("not found"),
    }
}

/// The `.tcad` files of the bundle as (path relative to the bundle, absolute path).
fn parts(root: &str) -> Vec<(String, PathBuf)> {
    let base = Path::new(root);
    let dir = if base.is_file() {
        base.parent().unwrap_or(base)
    } else {
        base
    };
    let Ok(files) = crate::collect_files(&[root]) else {
        return Vec::new();
    };
    files
        .into_iter()
        .map(|f| {
            let rel = f
                .strip_prefix(dir)
                .unwrap_or(&f)
                .to_string_lossy()
                .replace('\\', "/");
            (rel, f)
        })
        .collect()
}

/// Changes whenever a source file or the set of files changes.
fn version(root: &str) -> String {
    let mut h = DefaultHasher::new();
    for (rel, f) in parts(root) {
        rel.hash(&mut h);
        if let Ok(m) = std::fs::metadata(&f) {
            m.len().hash(&mut h);
            m.modified().ok().hash(&mut h);
        }
    }
    format!("{:016x}", h.finish())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(v) => {
                    out.push(v);
                    i += 3;
                    continue;
                }
                Err(_) => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn param(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == name).then(|| percent_decode(v))
    })
}

fn run<K: Kernel>(k: &K, args: &[&str]) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = crate::run_with(k, &args, &mut out, &mut err);
    (
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

fn part_info<K: Kernel>(k: &K, file: &Path, config: Option<&str>) -> Value {
    let source = std::fs::read_to_string(file).unwrap_or_default();
    let path = file.to_string_lossy();
    let mut args = vec!["build", "--json"];
    if let Some(c) = config {
        args.extend(["--config", c]);
    }
    args.push(&path);
    let (code, out, err) = run(k, &args);
    let built: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
    let configs = tcad_model::analyze(&source)
        .model
        .map(|m| {
            m.part
                .configs
                .iter()
                .map(|c| c.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if code == 0 {
        json!({"ok": true, "source": source, "configs": configs, "build": built})
    } else {
        // failures print their diagnostics as JSON on standard output
        json!({"ok": false, "source": source, "configs": configs, "error": err.trim(), "diagnostics": built})
    }
}

fn model<K: Kernel>(k: &K, file: &Path, config: Option<&str>) -> Result<Vec<u8>, String> {
    let tmp = std::env::temp_dir().join(format!(
        "tcad-serve-{}-{}.glb",
        std::process::id(),
        version(&file.to_string_lossy())
    ));
    let (tmp_s, path) = (
        tmp.to_string_lossy().into_owned(),
        file.to_string_lossy().into_owned(),
    );
    let mut args = vec!["export", "--to", "glb", "--out", &tmp_s];
    if let Some(c) = config {
        args.extend(["--config", c]);
    }
    args.push(&path);
    let (code, _, err) = run(k, &args);
    let bytes = std::fs::read(&tmp).map_err(|e| format!("{err} ({e})"));
    let _ = std::fs::remove_file(&tmp);
    if code == 0 { bytes } else { Err(err) }
}
