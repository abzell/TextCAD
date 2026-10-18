//! `tcad-lsp`: a language server for TextCAD.
//!
//! The core ([`Server`]) is pure Rust over the parser and the model, with no file system access of its
//! own: everything it needs from the outside (the persistent names of the last build, for example)
//! comes through a [`Provider`]. It therefore builds for WebAssembly and runs in the browser and in
//! Theia as well as natively (the binary supplies a provider that reads the bundle's cache).
//!
//! Features: diagnostics, completion (keywords, parameters, sketch entities, semantic IDs, tags),
//! hover (a parameter's resolved value with unit, a reference's entity, statements), go to definition,
//! rename, formatting, code actions from fix hints and document symbols.

mod text;

use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use tcad_diag::{Diagnostic, Severity, Span};
use tcad_model::{ItemKind, Model, analyze};
use tcad_syntax::token::{Token, TokenKind};
use tcad_syntax::{format_source, keywords as kw, lexer};
use text::Doc;

/// Working name of the format. Defined once per crate so it can be renamed later
/// (see `spec/name.md`).
pub const FORMAT_NAME: &str = "TextCAD";

/// File extension of a part/assembly source file.
pub const FILE_EXTENSION: &str = "tcad";

/// File extension of an archived bundle.
pub const BUNDLE_EXTENSION: &str = "tcadz";

/// What the server asks of its host.
pub trait Provider {
    /// The persistent names of the faces and edges of the last build of the document, as
    /// `(id, aliases, geometry, size)`; empty if there is none.
    fn topology(&self, _uri: &str) -> Vec<NamedEntity> {
        Vec::new()
    }
}

/// A face or edge of the last build.
#[derive(Clone, Debug)]
pub struct NamedEntity {
    /// `face` or `edge`.
    pub kind: String,
    /// Primary semantic ID.
    pub id: String,
    /// Other IDs.
    pub aliases: Vec<String>,
    /// Surface or curve type.
    pub geometry: String,
    /// Area (mm^2) or length (mm).
    pub size: f64,
}

/// A provider with nothing to offer.
pub struct NoProvider;
impl Provider for NoProvider {}

/// The language server state: open documents.
pub struct Server<P: Provider> {
    docs: HashMap<String, Doc>,
    provider: P,
    shutdown: bool,
}

const SEVERITY: fn(Severity) -> u8 = |s| match s {
    Severity::Error => 1,
    Severity::Warning => 2,
    Severity::Info => 3,
};

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

impl<P: Provider> Server<P> {
    /// A server with a provider.
    pub fn new(provider: P) -> Self {
        Server {
            docs: HashMap::new(),
            provider,
            shutdown: false,
        }
    }

    /// True after `shutdown` was requested.
    pub fn is_shutdown(&self) -> bool {
        self.shutdown
    }

    /// Handles one JSON-RPC message and returns the messages to send (replies and notifications).
    pub fn handle(&mut self, msg: &Value) -> Vec<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let reply = |result: Value| json!({"jsonrpc": "2.0", "id": id.clone().unwrap_or(Value::Null), "result": result});
        let mut out = Vec::new();
        match method {
            "initialize" => out.push(reply(json!({
                "capabilities": {
                    "textDocumentSync": {"openClose": true, "change": 1},
                    "completionProvider": {"triggerCharacters": [".", "(", "=", "&"]},
                    "hoverProvider": true,
                    "definitionProvider": true,
                    "renameProvider": true,
                    "documentFormattingProvider": true,
                    "codeActionProvider": {"codeActionKinds": ["quickfix"]},
                    "documentSymbolProvider": true,
                },
                "serverInfo": {"name": "tcad-lsp", "version": env!("CARGO_PKG_VERSION")},
            }))),
            "initialized" | "exit" | "$/cancelRequest" => {}
            "shutdown" => {
                self.shutdown = true;
                out.push(reply(Value::Null));
            }
            "textDocument/didOpen" => {
                let td = &params["textDocument"];
                if let (Some(uri), Some(text)) = (td["uri"].as_str(), td["text"].as_str()) {
                    self.docs.insert(uri.to_string(), Doc::new(text));
                    out.push(self.diagnostics(uri));
                }
            }
            "textDocument/didChange" => {
                let uri = params["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                if let Some(text) = params["contentChanges"]
                    .as_array()
                    .and_then(|c| c.last())
                    .and_then(|c| c["text"].as_str())
                {
                    self.docs.insert(uri.clone(), Doc::new(text));
                    out.push(self.diagnostics(&uri));
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = params["textDocument"]["uri"].as_str() {
                    self.docs.remove(uri);
                    out.push(
                        json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
                        "params": {"uri": uri, "diagnostics": []}}),
                    );
                }
            }
            "textDocument/completion" => out.push(reply(self.completion(&params))),
            "textDocument/hover" => out.push(reply(self.hover(&params))),
            "textDocument/definition" => out.push(reply(self.definition(&params))),
            "textDocument/rename" => {
                let r = self.rename(&params);
                out.push(match r {
                    Ok(edit) => reply(edit),
                    Err(m) => json!({"jsonrpc": "2.0", "id": id.clone().unwrap_or(Value::Null),
                        "error": {"code": -32602, "message": m}}),
                });
            }
            "textDocument/formatting" => out.push(reply(self.formatting(&params))),
            "textDocument/codeAction" => out.push(reply(self.code_actions(&params))),
            "textDocument/documentSymbol" => out.push(reply(self.symbols(&params))),
            _ => {
                if id.is_some() {
                    out.push(json!({"jsonrpc": "2.0", "id": id.clone().unwrap_or(Value::Null),
                        "error": {"code": -32601, "message": format!("method not found: {method}")}}));
                }
            }
        }
        out
    }

    /// Reads `Content-Length` framed messages and answers them until `exit` or the end of input.
    pub fn serve<R: BufRead, W: Write>(
        &mut self,
        mut input: R,
        out: &mut W,
    ) -> std::io::Result<()> {
        loop {
            let mut length = None;
            loop {
                let mut line = String::new();
                if input.read_line(&mut line)? == 0 {
                    return Ok(());
                }
                let line = line.trim_end();
                if line.is_empty() {
                    break;
                }
                if let Some(v) = line.strip_prefix("Content-Length:") {
                    length = v.trim().parse::<usize>().ok();
                }
            }
            let Some(n) = length else { continue };
            let mut body = vec![0u8; n];
            input.read_exact(&mut body)?;
            let Ok(msg) = serde_json::from_slice::<Value>(&body) else {
                continue;
            };
            let exiting = msg["method"] == "exit";
            for reply in self.handle(&msg) {
                let text = reply.to_string();
                write!(out, "Content-Length: {}\r\n\r\n{}", text.len(), text)?;
                out.flush()?;
            }
            if exiting {
                return Ok(());
            }
        }
    }

    // ---- helpers ----

    fn doc(&self, params: &Value) -> Option<(&str, &Doc)> {
        let uri = params["textDocument"]["uri"].as_str()?;
        self.docs.get_key_value(uri).map(|(k, d)| (k.as_str(), d))
    }

    fn offset(doc: &Doc, params: &Value) -> Option<usize> {
        let p = &params["position"];
        Some(doc.offset(
            p["line"].as_u64()? as usize,
            p["character"].as_u64()? as usize,
        ))
    }

    fn range(doc: &Doc, span: Span) -> Value {
        let (a, b) = (doc.position(span.start), doc.position(span.end));
        json!({"start": {"line": a.0, "character": a.1}, "end": {"line": b.0, "character": b.1}})
    }

    fn diagnostics(&self, uri: &str) -> Value {
        let doc = &self.docs[uri];
        let a = analyze(&doc.text);
        let list: Vec<Value> = a
            .diagnostics
            .iter()
            .map(|d| {
                let mut v = json!({
                    "range": Self::range(doc, d.span),
                    "severity": SEVERITY(d.severity),
                    "code": d.code.as_str(),
                    "source": "tcad",
                    "message": d.message,
                });
                if !d.related.is_empty() {
                    v["relatedInformation"] = json!(
                        d.related
                            .iter()
                            .map(|r| json!({
                                "location": {"uri": uri, "range": Self::range(doc, r.span)},
                                "message": r.message,
                            }))
                            .collect::<Vec<_>>()
                    );
                }
                v
            })
            .collect();
        json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
            "params": {"uri": uri, "diagnostics": list}})
    }

    /// The identifier (with a dotted path) around `at`: `(tokens of the path, index of the one at
    /// the cursor)`.
    fn path_at(tokens: &[Token], at: usize) -> Option<(Vec<&Token>, usize)> {
        let i = tokens.iter().position(|t| {
            t.span.start <= at
                && at <= t.span.end
                && matches!(t.kind, TokenKind::Ident | TokenKind::Upper)
        })?;
        let mut lo = i;
        while lo >= 2
            && tokens[lo - 1].kind == TokenKind::Dot
            && matches!(tokens[lo - 2].kind, TokenKind::Ident | TokenKind::Upper)
        {
            lo -= 2;
        }
        let mut hi = i;
        while hi + 2 < tokens.len()
            && tokens[hi + 1].kind == TokenKind::Dot
            && matches!(tokens[hi + 2].kind, TokenKind::Ident | TokenKind::Upper)
        {
            hi += 2;
        }
        let path: Vec<&Token> = tokens[lo..=hi].iter().step_by(2).collect();
        Some((path.clone(), (i - lo) / 2))
    }

    fn model(&self, uri: &str) -> Option<Model> {
        analyze(&self.docs.get(uri)?.text).model
    }

    // ---- completion ----

    fn completion(&self, params: &Value) -> Value {
        let Some((uri, doc)) = self.doc(params) else {
            return json!([]);
        };
        let Some(at) = Self::offset(doc, params) else {
            return json!([]);
        };
        let text = &doc.text;
        let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
        let before = &text[line_start..at];
        let model = self.model(uri);
        let item = |label: &str, kind: u8, detail: &str| json!({"label": label, "kind": kind, "detail": detail});
        let mut out: Vec<Value> = Vec::new();

        // after a dot: members of what comes before it
        if let Some(prefix) = before
            .rsplit(|c: char| !(is_ident_char(c) || c == '.'))
            .next()
            .filter(|p| p.contains('.'))
        {
            let head: Vec<&str> = prefix.split('.').collect();
            let owner = head[..head.len() - 1].join(".");
            let first = head[0];
            if let Some(m) = &model
                && let Some(sk) =
                    m.part
                        .items
                        .iter()
                        .find(|i| i.name == first)
                        .and_then(|i| match &i.kind {
                            ItemKind::Sketch(s) => Some(s),
                            _ => None,
                        })
            {
                if head.len() == 2 {
                    for e in &sk.entities {
                        out.push(item(&e.name, 5, &format!("{} in sketch {first}", e.kind)));
                    }
                } else {
                    let roles: &[&str] = match sk
                        .entities
                        .iter()
                        .find(|e| e.name == head[1])
                        .map(|e| e.kind.as_str())
                    {
                        Some("rect") => &[
                            "bottom_left",
                            "bottom_right",
                            "top_right",
                            "top_left",
                            "bottom",
                            "right",
                            "top",
                            "left",
                        ],
                        Some("line") => &["start", "end"],
                        Some("circle" | "ellipse") => &["center"],
                        Some("arc") => &["center", "start", "end"],
                        Some("slot") => &["start", "end", "right", "end_cap", "left", "start_cap"],
                        _ => &[],
                    };
                    for r in roles {
                        out.push(item(r, 5, "point or curve of the entity"));
                    }
                }
                return json!(out);
            }
            // a feature or tag: roles and the persistent names of the last build
            let known = self.provider.topology(uri);
            for e in known
                .iter()
                .filter(|e| e.id.starts_with(&format!("{owner}.")))
            {
                let rest = &e.id[owner.len() + 1..];
                out.push(item(
                    rest,
                    5,
                    &format!("{} {} ({:.3})", e.kind, e.geometry, e.size),
                ));
            }
            for role in [
                "cap_start",
                "cap_end",
                "side(",
                "wall",
                "floor",
                "face(",
                "inner(",
                "rim(",
            ] {
                if !out.iter().any(|o| o["label"] == role) {
                    out.push(item(role, 5, "role"));
                }
            }
            return json!(out);
        }

        let in_sketch = model.as_ref().is_some_and(|m| {
            m.part.items.iter().any(|i| {
                matches!(i.kind, ItemKind::Sketch(_)) && i.span.start < at && at < i.span.end
            })
        });
        let words_before = before.trim_start();
        let at_statement_start =
            !words_before.contains(char::is_whitespace) || words_before.is_empty();
        if at_statement_start {
            let list: Vec<&str> = if in_sketch {
                kw::ENTITY_KW.iter().copied().chain(["constrain"]).collect()
            } else {
                kw::DECL_KW
                    .iter()
                    .chain(kw::FEATURE_KW)
                    .chain(kw::REF_GEOM_KW)
                    .copied()
                    .filter(|k| *k != "part" && *k != "constrain")
                    .collect()
            };
            for k in list {
                out.push(item(k, 14, "keyword"));
            }
            return json!(out);
        }
        // inside a statement: parameters, names, tags, planes, functions, units
        if let Some(m) = &model {
            for p in &m.part.params {
                out.push(item(&p.name, 6, &format!("param = {}", p.value)));
            }
            for i in &m.part.items {
                if i.span.end <= at {
                    out.push(item(&i.name, 6, &i.keyword));
                }
            }
        }
        for f in kw::FUNC_NAMES {
            out.push(item(f, 3, "function"));
        }
        for u in kw::UNIT_NAMES {
            out.push(item(u, 21, "unit"));
        }
        for p in [
            "plane.XY", "plane.YZ", "plane.XZ", "axis.X", "axis.Y", "axis.Z", "origin",
        ] {
            out.push(item(p, 12, "built-in"));
        }
        json!(out)
    }

    // ---- hover ----

    fn hover(&self, params: &Value) -> Value {
        let Some((uri, doc)) = self.doc(params) else {
            return Value::Null;
        };
        let Some(at) = Self::offset(doc, params) else {
            return Value::Null;
        };
        let (tokens, _) = lexer::lex(&doc.text);
        let Some((path, idx)) = Self::path_at(&tokens, at) else {
            return Value::Null;
        };
        let model = self.model(uri);
        let name = path[idx].text.as_str();
        let span = path[idx].span;
        let md = |text: String| json!({"contents": {"kind": "markdown", "value": text}, "range": Self::range(doc, span)});
        if let Some(m) = &model
            && idx == 0
        {
            if let Some(p) = m.part.params.iter().find(|p| p.name == name) {
                let mut t = format!("**{}** = `{}`", p.name, p.value);
                if let Some(d) = &p.doc {
                    t.push_str(&format!("\n\n{d}"));
                }
                return md(t);
            }
            if let Some(i) = m.part.items.iter().find(|i| i.name == name) {
                return md(format!("**{}** `{}`", i.name, i.keyword));
            }
        }
        let known = self.provider.topology(uri);
        // the whole path as a persistent name
        let joined = path[..=idx]
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(".");
        if let Some(e) = known.iter().find(|e| {
            e.id == joined || e.aliases.contains(&joined) || e.id.starts_with(&format!("{joined}("))
        }) {
            return md(format!(
                "**{}** — {} ({}), size {:.3}",
                e.id, e.kind, e.geometry, e.size
            ));
        }
        if kw::is_keyword(name, true) || kw::is_keyword(name, false) {
            return md(format!("`{name}`: {}", keyword_help(name)));
        }
        Value::Null
    }

    // ---- definition ----

    /// The declaration span of the name at `at`.
    fn declaration(&self, uri: &str, at: usize) -> Option<(Span, String)> {
        let doc = self.docs.get(uri)?;
        let (tokens, _) = lexer::lex(&doc.text);
        let (path, idx) = Self::path_at(&tokens, at)?;
        let model = self.model(uri)?;
        let first = path[0].text.as_str();
        let top = |name: &str| -> Option<(Span, String)> {
            model
                .part
                .params
                .iter()
                .find(|p| p.name == name)
                .map(|p| (p.name_span, p.name.clone()))
                .or_else(|| {
                    model
                        .part
                        .items
                        .iter()
                        .find(|i| i.name == name)
                        .map(|i| (i.name_span, i.name.clone()))
                })
        };
        // an entity of the sketch the cursor is in
        let enclosing = model.part.items.iter().find(|i| {
            matches!(i.kind, ItemKind::Sketch(_)) && i.span.start < at && at < i.span.end
        });
        if idx == 0 {
            if let Some(ItemKind::Sketch(sk)) = enclosing.map(|i| &i.kind)
                && let Some(e) = sk.entities.iter().find(|e| e.name == first)
            {
                return Some((e.name_span, e.name.clone()));
            }
            return top(first);
        }
        // `sketch.entity...`
        if idx == 1
            && let Some(ItemKind::Sketch(sk)) = model
                .part
                .items
                .iter()
                .find(|i| i.name == first)
                .map(|i| &i.kind)
            && let Some(e) = sk.entities.iter().find(|e| e.name == path[1].text)
        {
            return Some((e.name_span, e.name.clone()));
        }
        top(first)
    }

    fn definition(&self, params: &Value) -> Value {
        let Some((uri, doc)) = self.doc(params) else {
            return Value::Null;
        };
        let Some(at) = Self::offset(doc, params) else {
            return Value::Null;
        };
        match self.declaration(uri, at) {
            Some((span, _)) => json!({"uri": uri, "range": Self::range(doc, span)}),
            None => Value::Null,
        }
    }

    // ---- rename ----

    fn rename(&self, params: &Value) -> Result<Value, String> {
        let (uri, doc) = self.doc(params).ok_or("the document is not open")?;
        let at = Self::offset(doc, params).ok_or("no position")?;
        let new = params["newName"].as_str().ok_or("no new name")?;
        let valid = !new.is_empty()
            && new
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
            && new
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !valid
            || kw::is_reserved(new)
            || kw::is_keyword(new, true)
            || kw::is_keyword(new, false)
            || kw::is_func(new)
        {
            return Err(format!(
                "`{new}` is not a valid name (lowercase letters, digits and underscores; not a keyword)"
            ));
        }
        let (decl, old) = self
            .declaration(uri, at)
            .ok_or("there is nothing to rename here")?;
        let model = self
            .model(uri)
            .ok_or("fix the syntax errors before renaming")?;
        if model.part.params.iter().any(|p| p.name == new)
            || model.part.items.iter().any(|i| i.name == new)
        {
            return Err(format!("`{new}` is already declared"));
        }
        let (tokens, _) = lexer::lex(&doc.text);
        // is the declaration a sketch entity? then references are local to the sketch or `sketch.entity`
        let owner: Option<(&str, Span)> = model.part.items.iter().find_map(|i| match &i.kind {
            ItemKind::Sketch(sk) if sk.entities.iter().any(|e| e.name_span == decl) => {
                Some((i.name.as_str(), i.span))
            }
            _ => None,
        });
        let mut edits = Vec::new();
        for (n, t) in tokens.iter().enumerate() {
            if t.kind != TokenKind::Ident || t.text != old {
                continue;
            }
            let prev_dot = n > 0 && tokens[n - 1].kind == TokenKind::Dot;
            let key = tokens.get(n + 1).is_some_and(|x| x.kind == TokenKind::Eq)
                && !prev_dot
                && t.span != decl
                && !in_config(&tokens, n);
            let hit = match owner {
                None => !prev_dot && !key,
                Some((sketch, range)) => {
                    let inside = range.start <= t.span.start && t.span.end <= range.end;
                    let qualified = prev_dot
                        && n >= 2
                        && tokens[n - 2].kind == TokenKind::Ident
                        && tokens[n - 2].text == sketch
                        && !(n >= 3 && tokens[n - 3].kind == TokenKind::Dot);
                    (inside && !prev_dot && !key) || qualified
                }
            };
            if hit {
                edits.push(json!({"range": Self::range(doc, t.span), "newText": new}));
            }
        }
        // the result must still analyze without new errors
        let mut text = doc.text.clone();
        let mut spans: Vec<Span> = tokens
            .iter()
            .filter(|t| edits.iter().any(|e| e["range"] == Self::range(doc, t.span)))
            .map(|t| t.span)
            .collect();
        spans.sort_by_key(|s| std::cmp::Reverse(s.start));
        for s in spans {
            text.replace_range(s.start..s.end, new);
        }
        let before = analyze(&doc.text)
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .count();
        let after = analyze(&text)
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .count();
        if after > before {
            let first = analyze(&text)
                .diagnostics
                .into_iter()
                .find(|d| d.is_error())
                .map(|d| d.message)
                .unwrap_or_default();
            return Err(format!(
                "renaming `{old}` would introduce errors ({first}); nothing was changed"
            ));
        }
        Ok(json!({"changes": {uri: edits}}))
    }

    // ---- formatting, actions, symbols ----

    fn formatting(&self, params: &Value) -> Value {
        let Some((_, doc)) = self.doc(params) else {
            return json!([]);
        };
        match format_source(&doc.text) {
            Ok(f) if f != doc.text => {
                let end = doc.position(doc.text.len());
                json!([{"range": {"start": {"line": 0, "character": 0}, "end": {"line": end.0, "character": end.1}}, "newText": f}])
            }
            _ => json!([]),
        }
    }

    fn code_actions(&self, params: &Value) -> Value {
        let Some((uri, doc)) = self.doc(params) else {
            return json!([]);
        };
        let (Some(lo), Some(hi)) = (
            params["range"]["start"]["line"].as_u64().map(|l| {
                (
                    l as usize,
                    params["range"]["start"]["character"].as_u64().unwrap_or(0) as usize,
                )
            }),
            params["range"]["end"]["line"].as_u64().map(|l| {
                (
                    l as usize,
                    params["range"]["end"]["character"].as_u64().unwrap_or(0) as usize,
                )
            }),
        ) else {
            return json!([]);
        };
        let (lo, hi) = (doc.offset(lo.0, lo.1), doc.offset(hi.0, hi.1));
        let a = analyze(&doc.text);
        let mut out = Vec::new();
        for d in a
            .diagnostics
            .iter()
            .filter(|d| d.span.start <= hi && lo <= d.span.end)
        {
            if let Some(h) = &d.hint {
                out.push(self.fix_action(uri, doc, d, h));
            }
        }
        json!(out)
    }

    fn fix_action(&self, uri: &str, doc: &Doc, d: &Diagnostic, h: &tcad_diag::FixHint) -> Value {
        let edits: Vec<Value> = h
            .edits
            .iter()
            .map(|e| json!({"range": Self::range(doc, e.span), "newText": e.replacement}))
            .collect();
        json!({
            "title": h.message,
            "kind": "quickfix",
            "diagnostics": [{"range": Self::range(doc, d.span), "code": d.code.as_str(), "message": d.message}],
            "edit": {"changes": {uri: edits}},
        })
    }

    fn symbols(&self, params: &Value) -> Value {
        let Some((uri, doc)) = self.doc(params) else {
            return json!([]);
        };
        let Some(m) = self.model(uri) else {
            return json!([]);
        };
        let mut out = Vec::new();
        for p in &m.part.params {
            out.push(json!({"name": p.name, "detail": p.value.to_string(), "kind": 13,
                "range": Self::range(doc, p.span), "selectionRange": Self::range(doc, p.name_span)}));
        }
        for i in &m.part.items {
            let kind = match i.kind {
                ItemKind::Sketch(_) => 5,
                ItemKind::Tag(_) => 14,
                _ => 12,
            };
            let mut s = json!({"name": i.name, "detail": i.keyword, "kind": kind,
                "range": Self::range(doc, i.span), "selectionRange": Self::range(doc, i.name_span)});
            if let ItemKind::Sketch(sk) = &i.kind {
                s["children"] = json!(sk.entities.iter().map(|e| json!({
                    "name": e.name, "detail": e.kind, "kind": 13,
                    "range": Self::range(doc, e.span), "selectionRange": Self::range(doc, e.name_span),
                })).collect::<Vec<_>>());
            }
            out.push(s);
        }
        json!(out)
    }
}

/// True if token `n` sits directly in a `config name { ... }` block, where `name = value` entries
/// refer to parameters.
fn in_config(tokens: &[Token], n: usize) -> bool {
    let mut depth = 0usize;
    for i in (0..n).rev() {
        match tokens[i].kind {
            TokenKind::RBrace => depth += 1,
            TokenKind::LBrace if depth > 0 => depth -= 1,
            TokenKind::LBrace => {
                return i >= 2 && tokens[i - 2].text == "config";
            }
            _ => {}
        }
    }
    false
}

fn keyword_help(word: &str) -> &'static str {
    match word {
        "param" => "a named, typed value: `param w = 10 mm range(5 mm, 20 mm)`",
        "sketch" => "a 2D drawing with constraints on a plane or planar face",
        "extrude" => {
            "sweeps a profile along its normal; end conditions: distance, symmetric, distance2, through_all, to"
        }
        "revolve" => "turns a profile about an axis",
        "sweep" => "moves a profile along a path, optionally controlled by a guide",
        "loft" => "a solid through sections, smooth or ruled, optionally along a guide",
        "hole" => "simple, counterbore, countersink or tapped hole on a face or plane",
        "fillet" => "rounds edges; `r` may be a pair for a variable radius",
        "chamfer" => "bevels edges with one or two distances or a distance and an angle",
        "shell" => "hollows the body, optionally opening faces",
        "draft" => "tilts faces about a neutral plane",
        "rib" => "a thin web between a centerline and a plane",
        "linear_pattern" => "copies a feature along a direction, with an optional skip list",
        "circular_pattern" => "copies a feature about an axis",
        "mirror" => "copies a feature in a plane",
        "combine" => "adds, subtracts or intersects an imported STEP solid",
        "tag" => "binds a stable name to a face or edge",
        "config" => "a variant: parameter overrides and suppressed features",
        "constrain" => "a sketch constraint or dimension",
        _ => "a TextCAD keyword (see spec/grammar.ebnf)",
    }
}
