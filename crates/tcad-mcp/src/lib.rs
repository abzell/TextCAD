//! `tcad-mcp`: a Model Context Protocol server that lets an agent create, edit, build and inspect
//! TextCAD parts. It is a thin layer over the same crates as the CLI: every tool returns structured
//! data and diagnostics, edits are validated before they are written, and a rejected edit leaves the
//! files untouched.
//!
//! Transport: JSON-RPC 2.0, one message per line, on standard input and output.

mod edit;
mod explain;

pub use edit::{Edit, apply, apply_all};

use serde_json::{Map, Value, json};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use tcad_cache::{cache_dir, load_bundle, read_manifest};
use tcad_diag::{LineIndex, render_json};
use tcad_kernel::Kernel;
use tcad_model::analyze;

/// Working name of the format. Defined once per crate so it can be renamed later
/// (see `spec/name.md`).
pub const FORMAT_NAME: &str = "TextCAD";

/// File extension of a part/assembly source file.
pub const FILE_EXTENSION: &str = "tcad";

/// File extension of an archived bundle.
pub const BUNDLE_EXTENSION: &str = "tcadz";

const PROTOCOL_VERSION: &str = "2025-03-26";

/// The result of a tool call.
struct Outcome {
    ok: bool,
    data: Value,
    image: Option<Vec<u8>>,
}

impl Outcome {
    fn ok(data: Value) -> Outcome {
        Outcome {
            ok: true,
            data,
            image: None,
        }
    }

    fn err(message: impl Into<String>, mut data: Value) -> Outcome {
        if let Some(o) = data.as_object_mut() {
            o.insert("error".into(), json!(message.into()));
        }
        Outcome {
            ok: false,
            data,
            image: None,
        }
    }

    fn fail(message: impl Into<String>) -> Outcome {
        Outcome::err(message, json!({}))
    }
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn tool(name: &str, description: &str, props: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": props,
            "required": required,
            "additionalProperties": false,
        },
    })
}

fn s(description: &str) -> Value {
    json!({"type": "string", "description": description})
}

/// The tool definitions (name, description and JSON schema of the arguments).
pub fn tool_definitions() -> Vec<Value> {
    let path = s("Bundle directory or part file, relative to the server's root.");
    let config = s("Configuration to use (default: the part's default).");
    vec![
        tool(
            "create_part",
            "Create a new bundle `<name>/` with an empty part, or from complete source text.",
            json!({
                "name": s("Part and bundle name: lowercase letters, digits and underscores."),
                "title": s("Human-readable title."),
                "source": s("Complete .tcad source (optional; validated before it is written)."),
            }),
            &["name"],
        ),
        tool(
            "read_part",
            "Read a part: source, canonical form, parameters, statements with line numbers, configurations and diagnostics.",
            json!({"path": path}),
            &["path"],
        ),
        tool(
            "check_part",
            "Parse and check a part (names, units, references that do not need geometry). Returns diagnostics.",
            json!({"path": path}),
            &["path"],
        ),
        tool(
            "apply_edit",
            "Apply structured edits and, if the result is valid (and builds), write it. Edits: \
             {op:add_statement,text,after?,before?} {op:replace_statement,name,text} {op:remove_statement,name} \
             {op:set_arg,name,entity?,arg,value} {op:remove_arg,name,entity?,arg} \
             {op:add_param,name,value,range?,doc?,after?} {op:set_param,name,value} {op:add_to_sketch,sketch,text}. \
             A rejected edit leaves the files untouched and returns the diagnostics.",
            json!({
                "path": path,
                "edits": {"type": "array", "items": {"type": "object"}, "description": "Edits applied in order."},
                "validate_geometry": {"type": "boolean", "description": "Also regenerate the part and reject the edit if it fails (default true)."},
            }),
            &["path", "edits"],
        ),
        tool(
            "set_param",
            "Change the defining expression of a parameter, for example value \"25 mm\".",
            json!({"path": path, "name": s("Parameter name."), "value": s("New expression, with units.")}),
            &["path", "name", "value"],
        ),
        tool(
            "build",
            "Regenerate the part and fill the cache. Returns per-feature topology and the result's properties.",
            json!({"path": path, "config": config, "force": {"type": "boolean"}}),
            &["path"],
        ),
        tool(
            "get_properties",
            "Volume, area, mass, center of mass, inertia tensor, bounding box and topology counts.",
            json!({"path": path, "config": config}),
            &["path"],
        ),
        tool(
            "query_topology",
            "List the faces and edges of the result with their persistent names, types, sizes and positions.",
            json!({
                "path": path, "config": config,
                "kind": {"type": "string", "enum": ["face", "edge"]},
                "feature": s("Show the shape after this feature (default: the final result)."),
                "contains": s("Only names containing this text."),
                "geometry": s("Only this surface/curve type (plane, cylinder, line, circle ...)."),
            }),
            &["path"],
        ),
        tool(
            "resolve_reference",
            "Resolve a face or edge reference (name, tag, edge(a & b), query(...)) and say what it denotes, or why it does not resolve.",
            json!({
                "path": path, "config": config,
                "reference": s("The reference as written in a statement."),
                "after": s("Resolve in the shape after this statement (default: the end of the part)."),
            }),
            &["path", "reference"],
        ),
        tool(
            "export",
            "Export the result as step, stl, 3mf or glb.",
            json!({
                "path": path, "config": config,
                "to": {"type": "string", "enum": ["step", "stl", "3mf", "glb"]},
                "out": s("Output file relative to the root (default: <bundle>/exports/<part>.<ext>)."),
            }),
            &["path", "to"],
        ),
        tool(
            "render_view",
            "Render a PNG of a named view (iso, front, back, left, right, top, bottom); faces whose name contains one of `highlight` are orange.",
            json!({
                "path": path, "config": config,
                "view": s("iso (default), front, back, left, right, top or bottom."),
                "size": {"type": "integer", "description": "Image size in pixels (default 512)."},
                "highlight": {"type": "array", "items": {"type": "string"}},
            }),
            &["path"],
        ),
        tool(
            "explain_error",
            "Explain a diagnostic code such as E3002: what it means and how to fix it.",
            json!({"code": s("A diagnostic code.")}),
            &["code"],
        ),
    ]
}

/// The server: a kernel and the directory that contains the bundles.
pub struct Server<K: Kernel> {
    kernel: K,
    root: PathBuf,
    geometry: bool,
}

impl<K: Kernel> Server<K> {
    /// A server for bundles below `root`.
    pub fn new(kernel: K, root: PathBuf) -> Self {
        Server {
            kernel,
            root,
            geometry: true,
        }
    }

    /// Tell the server whether the kernel can build real geometry. Without it, edits are validated
    /// by the checker only.
    pub fn with_geometry(mut self, geometry: bool) -> Self {
        self.geometry = geometry;
        self
    }

    /// Reads messages line by line and answers them until the input ends.
    pub fn serve<R: BufRead, W: Write>(&self, input: R, out: &mut W) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let reply = match serde_json::from_str::<Value>(&line) {
                Ok(msg) => self.handle(&msg),
                Err(e) => Some(json!({
                    "jsonrpc": "2.0", "id": null,
                    "error": {"code": -32700, "message": format!("parse error: {e}")},
                })),
            };
            if let Some(r) = reply {
                writeln!(out, "{r}")?;
                out.flush()?;
            }
        }
        Ok(())
    }

    /// Handles one JSON-RPC message; notifications get no answer.
    pub fn handle(&self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let reply = |result: Value| {
            id.clone()
                .map(|id| json!({"jsonrpc": "2.0", "id": id, "result": result}))
        };
        let error = |code: i64, message: String| {
            id.clone().map(|id| json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}))
        };
        match method {
            "initialize" => reply(json!({
                "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "tcad-mcp", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "TextCAD parts are text. Create a part, add parameters, sketches and features with apply_edit, build, then inspect with get_properties, query_topology and render_view. Faces and edges are referred to by persistent names such as body.cap_end or edge(a & b); use resolve_reference when unsure. Diagnostics carry stable codes; explain_error describes them.",
            })),
            "ping" => reply(json!({})),
            "tools/list" => reply(json!({"tools": tool_definitions()})),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let outcome = self.call(name, &args);
                let mut content = vec![
                    json!({"type": "text", "text": serde_json::to_string_pretty(&outcome.data).unwrap_or_default()}),
                ];
                if let Some(png) = &outcome.image {
                    content.push(
                        json!({"type": "image", "data": base64(png), "mimeType": "image/png"}),
                    );
                }
                reply(
                    json!({"content": content, "structuredContent": outcome.data, "isError": !outcome.ok}),
                )
            }
            m if m.starts_with("notifications/") => None,
            "" => error(-32600, "invalid request".into()),
            other => error(-32601, format!("method not found: {other}")),
        }
    }

    fn call(&self, name: &str, args: &Value) -> Outcome {
        let get = |k: &str| args.get(k).and_then(Value::as_str);
        let need = |k: &str| -> Result<&str, Outcome> {
            get(k).ok_or_else(|| Outcome::fail(format!("`{name}` needs `{k}`")))
        };
        macro_rules! tri {
            ($e:expr) => {
                match $e {
                    Ok(v) => v,
                    Err(o) => return o,
                }
            };
        }
        match name {
            "create_part" => self.create_part(tri!(need("name")), get("title"), get("source")),
            "read_part" => self.read_part(tri!(need("path"))),
            "check_part" => self.check_part(tri!(need("path"))),
            "apply_edit" => {
                let Some(list) = args.get("edits").and_then(Value::as_array) else {
                    return Outcome::fail("`apply_edit` needs `edits`, a list");
                };
                let mut edits = Vec::new();
                for e in list {
                    match Edit::from_json(e) {
                        Ok(e) => edits.push(e),
                        Err(m) => return Outcome::fail(m),
                    }
                }
                let validate = args
                    .get("validate_geometry")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                self.commit(tri!(need("path")), &edits, validate)
            }
            "set_param" => {
                let edit = Edit::SetParam {
                    name: tri!(need("name")).to_string(),
                    value: tri!(need("value")).to_string(),
                };
                self.commit(tri!(need("path")), &[edit], true)
            }
            "build" => {
                let mut a = vec!["build", "--json"];
                if args.get("force").and_then(Value::as_bool) == Some(true) {
                    a.push("--force");
                }
                self.cli_tool(&a, tri!(need("path")), get("config"))
            }
            "get_properties" => {
                self.cli_tool(&["props", "--json"], tri!(need("path")), get("config"))
            }
            "query_topology" => self.query_topology(args),
            "resolve_reference" => self.resolve_reference(
                tri!(need("path")),
                tri!(need("reference")),
                get("after"),
                get("config"),
            ),
            "export" => self.export(
                tri!(need("path")),
                tri!(need("to")),
                get("out"),
                get("config"),
            ),
            "render_view" => self.render_view(args),
            "explain_error" => {
                let code = tri!(need("code")).trim().to_uppercase();
                match explain::explain(&code) {
                    Some((title, meaning, fix)) => Outcome::ok(
                        json!({"code": code, "title": title, "meaning": meaning, "how_to_fix": fix}),
                    ),
                    None => Outcome::err(
                        format!("unknown diagnostic code `{code}`"),
                        json!({"known": explain::codes().collect::<Vec<_>>()}),
                    ),
                }
            }
            other => Outcome::fail(format!("unknown tool `{other}`")),
        }
    }

    // ---- files ----

    /// A path below the root (no absolute paths, no `..`).
    fn inside(&self, rel: &str) -> Result<PathBuf, String> {
        let p = Path::new(rel);
        if p.is_absolute()
            || p.components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(format!("`{rel}` must be a path inside the server's root"));
        }
        Ok(self.root.join(p))
    }

    fn bundle(&self, rel: &str) -> Result<tcad_cache::Bundle, Outcome> {
        let path = self.inside(rel).map_err(Outcome::fail)?;
        load_bundle(&path).map_err(Outcome::fail)
    }

    fn diagnostics(file: &str, src: &str, diags: &[tcad_diag::Diagnostic]) -> Value {
        render_json(file, src, diags)
            .get("diagnostics")
            .cloned()
            .unwrap_or_else(|| json!([]))
    }

    fn create_part(&self, name: &str, title: Option<&str>, source: Option<&str>) -> Outcome {
        let valid = !name.is_empty()
            && name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !valid {
            return Outcome::fail(
                "the name must be lowercase letters, digits and underscores, starting with a letter",
            );
        }
        let dir = match self.inside(name) {
            Ok(d) => d,
            Err(m) => return Outcome::fail(m),
        };
        if dir.exists() {
            return Outcome::fail(format!("`{name}` already exists"));
        }
        let text = match source {
            Some(t) => t.to_string(),
            None => {
                let title = title
                    .map(|t| format!(" \"{}\"", t.replace('"', "'")))
                    .unwrap_or_default();
                format!("tcad 0.1\n\npart {name}{title} {{\n  tolerance 1e-6 mm\n}}\n")
            }
        };
        let a = analyze(&text);
        if a.has_errors() {
            return Outcome::err(
                "the source has errors; nothing was written",
                json!({"diagnostics": Self::diagnostics(name, &text, &a.diagnostics)}),
            );
        }
        let canonical = tcad_syntax::format_source(&text).unwrap_or(text);
        let entry = format!("parts/{name}.tcad");
        let manifest = format!(
            "format = \"0.1\"\nname = \"{name}\"\nentry = \"{entry}\"\nunits = \"mm\"\nlicense = \"Apache-2.0\"\n"
        );
        let written = std::fs::create_dir_all(dir.join("parts"))
            .and_then(|()| std::fs::write(dir.join("bundle.toml"), manifest))
            .and_then(|()| std::fs::write(dir.join(&entry), &canonical));
        match written {
            Ok(()) => Outcome::ok(
                json!({"path": name, "entry": format!("{name}/{entry}"), "diagnostics": Self::diagnostics(name, &canonical, &a.diagnostics)}),
            ),
            Err(e) => Outcome::fail(format!("cannot write {name}: {e}")),
        }
    }

    fn read_part(&self, rel: &str) -> Outcome {
        let b = match self.bundle(rel) {
            Ok(b) => b,
            Err(o) => return o,
        };
        let src = match std::fs::read_to_string(&b.entry) {
            Ok(s) => s,
            Err(e) => return Outcome::fail(format!("cannot read {}: {e}", b.entry.display())),
        };
        let a = analyze(&src);
        let mut data = json!({
            "path": rel,
            "source": src,
            "canonical": tcad_syntax::format_source(&src).ok(),
            "diagnostics": Self::diagnostics(rel, &src, &a.diagnostics),
        });
        if let Some(m) = &a.model {
            let lines = LineIndex::new(&src);
            data["part"] = json!({
                "name": m.part.name,
                "title": m.part.display_name,
                "tolerance_mm": m.part.tolerance,
                "material": m.part.material.as_ref().map(|x| x.name.clone()),
            });
            data["parameters"] = json!(
                m.part
                    .params
                    .iter()
                    .map(|p| json!({
                        "name": p.name, "value": p.value.to_string(),
                        "range": p.range.is_some(), "doc": p.doc,
                        "line": lines.line_col(p.span.start).0,
                    }))
                    .collect::<Vec<_>>()
            );
            data["statements"] = json!(m.part.items.iter().map(|i| json!({
                "keyword": i.keyword, "name": i.name, "line": lines.line_col(i.span.start).0,
            })).collect::<Vec<_>>());
            data["configurations"] = json!(m.part.configs.iter().map(|c| json!({
                "name": c.name, "default": c.is_default,
                "overrides": c.overrides.iter().map(|o| o.param.clone()).collect::<Vec<_>>(),
            })).collect::<Vec<_>>());
        }
        Outcome::ok(data)
    }

    fn check_part(&self, rel: &str) -> Outcome {
        let b = match self.bundle(rel) {
            Ok(b) => b,
            Err(o) => return o,
        };
        let src = std::fs::read_to_string(&b.entry).unwrap_or_default();
        let a = analyze(&src);
        let diags = Self::diagnostics(rel, &src, &a.diagnostics);
        if a.has_errors() {
            Outcome::err("the part has errors", json!({"diagnostics": diags}))
        } else {
            Outcome::ok(json!({"diagnostics": diags}))
        }
    }

    /// Applies edits, validates the result and writes it only if it is valid.
    fn commit(&self, rel: &str, edits: &[Edit], validate_geometry: bool) -> Outcome {
        let b = match self.bundle(rel) {
            Ok(b) => b,
            Err(o) => return o,
        };
        let src = match std::fs::read_to_string(&b.entry) {
            Ok(s) => s,
            Err(e) => return Outcome::fail(format!("cannot read {}: {e}", b.entry.display())),
        };
        let text = match apply_all(&src, edits) {
            Ok(t) => t,
            Err(m) => {
                return Outcome::err(
                    format!("{m}; nothing was written"),
                    json!({"rejected": true}),
                );
            }
        };
        let a = analyze(&text);
        let diags = Self::diagnostics(rel, &text, &a.diagnostics);
        let Some(model) = a.model.as_ref().filter(|_| !a.has_errors()) else {
            return Outcome::err(
                "the edit produces errors; nothing was written",
                json!({"rejected": true, "diagnostics": diags}),
            );
        };
        let canonical = tcad_syntax::format_source(&text).unwrap_or_else(|_| text.clone());
        let mut build = Value::Null;
        if validate_geometry && self.geometry {
            let loader = |p: &str| -> Result<Vec<u8>, String> {
                let q = Path::new(p);
                if q.is_absolute()
                    || q.components()
                        .any(|c| matches!(c, std::path::Component::ParentDir))
                {
                    return Err("imports must stay inside the bundle".into());
                }
                std::fs::read(b.root.join(q)).map_err(|e| e.to_string())
            };
            let (result, _) = tcad_build::build_with(&self.kernel, model, None, Some(&loader));
            match result {
                Ok(built) => {
                    build = json!({
                        "features": built.features.iter().map(|f| json!({"name": f.name, "keyword": f.keyword, "volume_mm3": f.volume})).collect::<Vec<_>>(),
                    });
                }
                Err(d) => {
                    return Outcome::err(
                        "the edit does not regenerate; nothing was written",
                        json!({"rejected": true, "diagnostics": Self::diagnostics(rel, &canonical, &d)}),
                    );
                }
            }
        }
        if let Err(e) = std::fs::write(&b.entry, &canonical) {
            return Outcome::fail(format!("cannot write {}: {e}", b.entry.display()));
        }
        Outcome::ok(
            json!({"applied": edits.len(), "source": canonical, "diagnostics": diags, "build": build}),
        )
    }

    // ---- commands through the CLI ----

    fn run_cli(&self, args: &[String]) -> (i32, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = tcad_cli::run_with(&self.kernel, args, &mut out, &mut err);
        (
            code,
            String::from_utf8_lossy(&out).into_owned(),
            String::from_utf8_lossy(&err).into_owned(),
        )
    }

    fn cli_tool(&self, args: &[&str], rel: &str, config: Option<&str>) -> Outcome {
        let path = match self.inside(rel) {
            Ok(p) => p,
            Err(m) => return Outcome::fail(m),
        };
        let mut a: Vec<String> = args.iter().map(|x| x.to_string()).collect();
        if let Some(c) = config {
            a.push("--config".into());
            a.push(c.into());
        }
        a.push(path.display().to_string());
        let (code, out, err) = self.run_cli(&a);
        let data = serde_json::from_str::<Value>(&out).unwrap_or_else(|_| json!({"output": out}));
        if code == 0 {
            Outcome::ok(data)
        } else {
            let mut data = if data.is_object() {
                data
            } else {
                json!({"output": data})
            };
            if !err.trim().is_empty() {
                data["stderr"] = json!(err);
            }
            Outcome::err(format!("the command failed (exit code {code})"), data)
        }
    }

    fn export(&self, rel: &str, to: &str, out: Option<&str>, config: Option<&str>) -> Outcome {
        let b = match self.bundle(rel) {
            Ok(b) => b,
            Err(o) => return o,
        };
        let src = std::fs::read_to_string(&b.entry).unwrap_or_default();
        let part = analyze(&src)
            .model
            .map(|m| m.part.name)
            .unwrap_or_else(|| "part".into());
        let target = match out {
            Some(o) => match self.inside(o) {
                Ok(p) => p,
                Err(m) => return Outcome::fail(m),
            },
            None => b.root.join("exports").join(format!("{part}.{to}")),
        };
        let mut a = vec![
            "export".to_string(),
            "--json".into(),
            "--to".into(),
            to.into(),
            "--out".into(),
            target.display().to_string(),
        ];
        if let Some(c) = config {
            a.push("--config".into());
            a.push(c.into());
        }
        a.push(b.root.display().to_string());
        let (code, stdout, stderr) = self.run_cli(&a);
        let mut data = serde_json::from_str::<Value>(&stdout).unwrap_or_else(|_| json!({}));
        if code == 0 {
            if let Ok(rel_out) = target.strip_prefix(&self.root) {
                data["file"] = json!(rel_out.display().to_string());
            }
            Outcome::ok(data)
        } else {
            data["stderr"] = json!(stderr);
            Outcome::err(format!("the export failed (exit code {code})"), data)
        }
    }

    // ---- inspection ----

    /// Builds (if needed) and returns the manifest of the cache.
    fn built(
        &self,
        rel: &str,
        config: Option<&str>,
    ) -> Result<(tcad_cache::Bundle, tcad_cache::Manifest, PathBuf), Outcome> {
        let built = self.cli_tool(&["build", "--json"], rel, config);
        if !built.ok {
            return Err(built);
        }
        let b = self.bundle(rel)?;
        let src = std::fs::read_to_string(&b.entry).unwrap_or_default();
        let part = analyze(&src)
            .model
            .map(|m| m.part.name)
            .ok_or_else(|| Outcome::fail("the part has errors"))?;
        let dir = cache_dir(&b.root, &part);
        let m = read_manifest(&dir).ok_or_else(|| Outcome::fail("the cache has no manifest"))?;
        Ok((b, m, dir))
    }

    fn query_topology(&self, args: &Value) -> Outcome {
        let get = |k: &str| args.get(k).and_then(Value::as_str);
        let Some(rel) = get("path") else {
            return Outcome::fail("`query_topology` needs `path`");
        };
        let (_, m, _) = match self.built(rel, get("config")) {
            Ok(x) => x,
            Err(o) => return o,
        };
        let feature = match get("feature") {
            Some(f) => match m.features.iter().find(|x| x.name == f) {
                Some(x) => x,
                None => return Outcome::fail(format!("no feature `{f}` in the result")),
            },
            None => match m.features.last() {
                Some(x) => x,
                None => return Outcome::fail("the part has no solid feature"),
            },
        };
        let kind = get("kind");
        let contains = get("contains");
        let geometry = get("geometry");
        let list: Vec<Value> = feature
            .topology
            .iter()
            .filter(|t| kind.is_none_or(|k| t.kind == k))
            .filter(|t| contains.is_none_or(|c| t.id.contains(c) || t.aliases.iter().any(|a| a.contains(c))))
            .filter(|t| geometry.is_none_or(|g| t.geometry == g))
            .take(500)
            .map(|t| json!({"kind": t.kind, "id": t.id, "aliases": t.aliases, "geometry": t.geometry, "size": t.size, "at": t.at}))
            .collect();
        Outcome::ok(json!({"feature": feature.name, "count": list.len(), "entities": list}))
    }

    fn resolve_reference(
        &self,
        rel: &str,
        reference: &str,
        after: Option<&str>,
        config: Option<&str>,
    ) -> Outcome {
        let b = match self.bundle(rel) {
            Ok(b) => b,
            Err(o) => return o,
        };
        let src = std::fs::read_to_string(&b.entry).unwrap_or_default();
        let edit = Edit::AddStatement {
            text: format!("tag __probe = {reference}"),
            after: after.map(str::to_string),
            before: None,
        };
        let text = match apply(&src, &edit) {
            Ok(t) => t,
            Err(m) => return Outcome::fail(m),
        };
        let a = analyze(&text);
        let Some(model) = a.model.as_ref().filter(|_| !a.has_errors()) else {
            return Outcome::err(
                "the reference is not valid syntax or names something undefined",
                json!({"diagnostics": Self::diagnostics(rel, &text, &a.diagnostics)}),
            );
        };
        let loader = |p: &str| -> Result<Vec<u8>, String> {
            let q = Path::new(p);
            if q.is_absolute()
                || q.components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err("imports must stay inside the bundle".into());
            }
            std::fs::read(b.root.join(q)).map_err(|e| e.to_string())
        };
        let (result, refs) = tcad_build::build_with(&self.kernel, model, config, Some(&loader));
        let probe = refs.iter().find(|r| r.feature == "__probe");
        match (probe, result) {
            (Some(r), _) => match &r.resolved {
                Ok(names) => Outcome::ok(json!({"reference": reference, "resolves_to": names})),
                Err(m) => Outcome::err(m.clone(), json!({"reference": reference})),
            },
            (None, Err(d)) => Outcome::err(
                "the part does not regenerate before the reference",
                json!({"diagnostics": Self::diagnostics(rel, &text, &d)}),
            ),
            (None, Ok(_)) => {
                Outcome::fail("the reference was not evaluated (is there a body yet?)")
            }
        }
    }

    fn render_view(&self, args: &Value) -> Outcome {
        let get = |k: &str| args.get(k).and_then(Value::as_str);
        let Some(rel) = get("path") else {
            return Outcome::fail("`render_view` needs `path`");
        };
        let view = get("view").unwrap_or("iso");
        let size = args
            .get("size")
            .and_then(Value::as_u64)
            .unwrap_or(512)
            .clamp(64, 2048) as u32;
        let highlight: Vec<String> = args
            .get("highlight")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let (_, m, dir) = match self.built(rel, get("config")) {
            Ok(x) => x,
            Err(o) => return o,
        };
        let shape = match std::fs::read(dir.join(tcad_cache::BREP_FILE))
            .map_err(|e| e.to_string())
            .and_then(|b| self.kernel.from_brep(&b).map_err(|e| e.to_string()))
        {
            Ok(s) => s,
            Err(e) => return Outcome::fail(format!("cannot read the cached result: {e}")),
        };
        let mesh = match self.kernel.mesh(&shape, 0.05, 0.2) {
            Ok(x) => x,
            Err(e) => return Outcome::fail(format!("meshing failed: {e}")),
        };
        let labels: Vec<(String, Vec<String>)> = m
            .features
            .last()
            .map(|f| {
                f.topology
                    .iter()
                    .filter(|t| t.kind == "face")
                    .map(|t| (t.id.clone(), t.aliases.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let hit = |face: u32| -> bool {
            labels.get(face as usize).is_some_and(|(id, aliases)| {
                highlight.iter().any(|h| {
                    id.contains(h.as_str()) || aliases.iter().any(|a| a.contains(h.as_str()))
                })
            })
        };
        let highlighted: Vec<u32> = mesh
            .faces
            .iter()
            .map(|f| f.face)
            .filter(|f| hit(*f))
            .collect();
        match tcad_io::render_png(&mesh, view, size, &hit) {
            Ok(png) => {
                let mut o = Outcome::ok(
                    json!({"view": view, "size": size, "bytes": png.len(), "highlighted_faces": highlighted.len()}),
                );
                o.image = Some(png);
                o
            }
            Err(m) => Outcome::fail(m),
        }
    }
}

/// A convenience for tests and hosts: the tool arguments as an object.
pub fn args(pairs: &[(&str, Value)]) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    Value::Object(m)
}
