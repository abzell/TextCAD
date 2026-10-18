//! Language server features over the rod clevis example.

use serde_json::{Value, json};
use tcad_lsp::{NamedEntity, Provider, Server};

const URI: &str = "file:///x/part.tcad";
const SRC: &str = include_str!("../../../spec/examples/rod_clevis.tcad");

struct Names;
impl Provider for Names {
    fn topology(&self, _: &str) -> Vec<NamedEntity> {
        vec![NamedEntity {
            kind: "face".into(),
            id: "body.cap_end".into(),
            aliases: vec![],
            geometry: "plane".into(),
            size: 12.5,
        }]
    }
}

fn open(src: &str) -> (Server<Names>, Vec<Value>) {
    let mut s = Server::new(Names);
    let out = s.handle(&json!({"jsonrpc":"2.0","method":"textDocument/didOpen",
        "params":{"textDocument":{"uri":URI,"text":src}}}));
    (s, out)
}

fn req(s: &mut Server<Names>, method: &str, params: Value) -> Value {
    let mut r = s.handle(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}));
    r.remove(0)
}

fn pos_of(src: &str, needle: &str, nth: usize) -> Value {
    let off = src.match_indices(needle).nth(nth).expect("needle").0;
    let line = src[..off].matches('\n').count();
    let col = off - src[..off].rfind('\n').map_or(0, |i| i + 1);
    json!({"line": line, "character": col})
}

fn td() -> Value {
    json!({"uri": URI})
}

#[test]
fn clean_documents_publish_no_diagnostics_and_broken_ones_do() {
    let (_, out) = open(SRC);
    assert!(
        out[0]["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["severity"] == 3),
        "{}",
        out[0]
    );
    let (_, out) = open("part p {\n  param w = 1 mm\n  extrude e = nosuch\n}\n");
    let d = &out[0]["params"]["diagnostics"];
    assert!(!d.as_array().unwrap().is_empty());
    assert_eq!(d[0]["source"], "tcad");
    assert!(d[0]["code"].as_str().unwrap().starts_with('E'));
}

#[test]
fn definition_and_hover_find_parameters() {
    let (mut s, _) = open(SRC);
    let name = SRC
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("param ")
                .and_then(|r| r.split_whitespace().next())
        })
        .expect("a param")
        .to_string();
    let uses = SRC.matches(name.as_str()).count();
    assert!(uses >= 2, "{name} is used");
    let at = pos_of(SRC, &name, 1);
    let r = req(
        &mut s,
        "textDocument/definition",
        json!({"textDocument":td(),"position":at}),
    );
    assert_eq!(r["result"]["uri"], URI);
    let decl = pos_of(SRC, &name, 0);
    assert_eq!(r["result"]["range"]["start"], decl);
    let h = req(
        &mut s,
        "textDocument/hover",
        json!({"textDocument":td(),"position":at}),
    );
    let v = h["result"]["contents"]["value"]
        .as_str()
        .expect("hover text");
    assert!(v.contains(&name) && v.contains('='), "{v}");
}

#[test]
fn rename_changes_every_use_and_rejects_bad_names() {
    let (mut s, _) = open(SRC);
    let name = SRC
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("param ")
                .and_then(|r| r.split_whitespace().next())
        })
        .unwrap()
        .to_string();
    let at = pos_of(SRC, &name, 0);
    let r = req(
        &mut s,
        "textDocument/rename",
        json!({"textDocument":td(),"position":at,"newName":"renamed_value"}),
    );
    let edits = r["result"]["changes"][URI]
        .as_array()
        .unwrap_or_else(|| panic!("{r}"))
        .len();
    assert!(edits >= 2, "{r}");
    let bad = req(
        &mut s,
        "textDocument/rename",
        json!({"textDocument":td(),"position":at,"newName":"extrude"}),
    );
    assert!(bad.get("error").is_some());
    let bad = req(
        &mut s,
        "textDocument/rename",
        json!({"textDocument":td(),"position":at,"newName":"Bad Name"}),
    );
    assert!(bad.get("error").is_some());
}

#[test]
fn completion_offers_keywords_params_and_names() {
    let (mut s, _) = open("part p {\n  param w = 10 mm\n  \n}\n");
    let r = req(
        &mut s,
        "textDocument/completion",
        json!({"textDocument":td(),"position":{"line":2,"character":2}}),
    );
    let labels: Vec<&str> = r["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["label"].as_str().unwrap())
        .collect();
    assert!(
        labels.contains(&"extrude") && labels.contains(&"sketch"),
        "{labels:?}"
    );
    let src = "tcad 0.1\n\npart p \"P\" {\n  param w = 10 mm\n  extrude e = w + body.\n}\n";
    let (mut s, _) = open(src);
    let r = req(
        &mut s,
        "textDocument/completion",
        json!({"textDocument":td(),"position":{"line":4,"character":24}}),
    );
    let labels: Vec<&str> = r["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["label"].as_str().unwrap())
        .collect();
    assert!(labels.contains(&"cap_end"), "{labels:?}");
}

#[test]
fn formatting_and_symbols() {
    let (mut s, _) = open("tcad 0.1\n\npart p \"P\" {\n  param   w=10 mm\n}\n");
    let r = req(
        &mut s,
        "textDocument/formatting",
        json!({"textDocument":td(),"options":{}}),
    );
    let text = r["result"][0]["newText"].as_str().expect("an edit");
    assert!(text.contains("param w = 10 mm"), "{text}");
    let (mut s, _) = open(SRC);
    let r = req(
        &mut s,
        "textDocument/documentSymbol",
        json!({"textDocument":td()}),
    );
    assert!(r["result"].as_array().unwrap().len() >= 5);
}

#[test]
fn framing_round_trip() {
    let mut s = Server::new(Names);
    let body = json!({"jsonrpc":"2.0","id":7,"method":"initialize","params":{}}).to_string();
    let input = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
    let mut out = Vec::new();
    s.serve(std::io::Cursor::new(input), &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with("Content-Length: "));
    assert!(text.contains("\"renameProvider\":true"));
}
