//! The server binary speaks JSON-RPC lines on standard input and output.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

struct Io {
    stdin: std::process::ChildStdin,
    out: BufReader<std::process::ChildStdout>,
}

impl Io {
    fn ask(&mut self, line: &str) -> Value {
        writeln!(self.stdin, "{line}").expect("write");
        self.stdin.flush().expect("flush");
        let mut reply = String::new();
        self.out.read_line(&mut reply).expect("read");
        serde_json::from_str(&reply).unwrap_or_else(|e| panic!("{e}: {reply}"))
    }
}

fn call(io: &mut Io, v: Value) -> Value {
    io.ask(&v.to_string())
}

#[test]
fn the_binary_answers_json_rpc_lines() {
    let root = std::env::temp_dir().join(format!("tcad-mcp-stdio-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("mkdir");
    let mut child = Command::new(env!("CARGO_BIN_EXE_tcad-mcp"))
        .args(["--root", &root.display().to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut io = Io {
        stdin: child.stdin.take().expect("stdin"),
        out: BufReader::new(child.stdout.take().expect("stdout")),
    };
    let r = call(
        &mut io,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );
    assert_eq!(r["result"]["serverInfo"]["name"], "tcad-mcp");
    let r = call(
        &mut io,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    );
    assert!(r["result"]["tools"].as_array().expect("tools").len() >= 11);
    let r = call(
        &mut io,
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {"name": "create_part", "arguments": {"name": "bar", "title": "A bar"}}}),
    );
    assert_eq!(r["result"]["isError"], false, "{r}");
    let r = call(
        &mut io,
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {"name": "read_part", "arguments": {"path": "bar"}}}),
    );
    assert_eq!(r["result"]["structuredContent"]["part"]["name"], "bar");
    // a malformed line is a parse error, not a crash
    let r = io.ask("this is not json");
    assert_eq!(r["error"]["code"], -32700);
    // an unknown tool is a tool error
    let r = call(
        &mut io,
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "frobnicate", "arguments": {}}}),
    );
    assert_eq!(r["result"]["isError"], true);
    drop(io);
    assert!(child.wait().expect("exit").success());
    let _ = std::fs::remove_dir_all(&root);
}
