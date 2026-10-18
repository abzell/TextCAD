//! A scripted agent builds the rod clevis end to end using only MCP tools: the same part as
//! `corpus/hyd_rod_clevis`, checked against its expected properties.
#![cfg(feature = "occt")]

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tcad_mcp::Server;
use tcad_occt::OcctKernel;

struct Root(PathBuf);
impl Root {
    fn new() -> Root {
        let p = std::env::temp_dir().join(format!("tcad-mcp-clevis-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("mkdir");
        Root(p)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Agent {
    server: Server<OcctKernel>,
    next: u64,
}

impl Agent {
    fn rpc(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let reply = self
            .server
            .handle(&json!({"jsonrpc": "2.0", "id": self.next, "method": method, "params": params}))
            .expect("a reply");
        assert_eq!(reply["id"], self.next);
        reply
    }

    /// Calls a tool; returns (is_error, structured result, full result).
    fn tool(&mut self, name: &str, args: Value) -> (bool, Value, Value) {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let result = r["result"].clone();
        (
            result["isError"].as_bool().expect("isError"),
            result["structuredContent"].clone(),
            result,
        )
    }

    fn ok(&mut self, name: &str, args: Value) -> Value {
        let (err, data, _) = self.tool(name, args);
        assert!(!err, "{name} failed: {data:#}");
        data
    }

    fn edit(&mut self, edits: Value) -> Value {
        self.ok("apply_edit", json!({"path": "clevis", "edits": edits}))
    }
}

fn add(text: &str) -> Value {
    json!({"op": "add_statement", "text": text})
}

fn near(a: f64, b: f64, rel: f64, what: &str) {
    assert!(
        (a - b).abs() <= rel * b.abs().max(1.0),
        "{what}: {a} vs {b}"
    );
}

#[test]
fn an_agent_builds_the_rod_clevis_with_mcp_tools_only() {
    let root = Root::new();
    let mut agent = Agent {
        server: Server::new(OcctKernel::new(), root.0.clone()),
        next: 0,
    };

    // handshake and the tool list
    let init = agent.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "script", "version": "1"}}));
    assert_eq!(init["result"]["serverInfo"]["name"], "tcad-mcp");
    assert!(
        agent
            .server
            .handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .is_none()
    );
    let tools = agent.rpc("tools/list", json!({}));
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|t| t["name"].as_str().expect("name"))
        .collect();
    for want in [
        "create_part",
        "read_part",
        "apply_edit",
        "set_param",
        "build",
        "get_properties",
        "query_topology",
        "resolve_reference",
        "export",
        "render_view",
        "explain_error",
    ] {
        assert!(names.contains(&want), "tool {want} in {names:?}");
    }
    assert_eq!(agent.rpc("nonsense", json!({}))["error"]["code"], -32601);

    // 1. an empty part
    agent.ok(
        "create_part",
        json!({"name": "clevis", "title": "Rod clevis, 20 mm pin"}),
    );

    // 2. parameters and material
    agent.edit(json!([
        add("material \"AISI 1045\" density=7.85 g/cm^3"),
        {"op": "add_param", "name": "pin_dia", "value": "20 mm", "range": "10 mm, 60 mm"},
        {"op": "add_param", "name": "jaw_gap", "value": "26 mm"},
        {"op": "add_param", "name": "jaw_thk", "value": "16 mm"},
        {"op": "add_param", "name": "body_len", "value": "70 mm"},
        {"op": "add_param", "name": "thread", "value": "\"M24x2\""},
    ]));

    // 3. the body
    agent.edit(json!([
        add("sketch side on plane.XY {\n  rect outline corner=(0 mm, -(jaw_gap / 2 + jaw_thk)) w=body_len h=jaw_gap + 2 * jaw_thk\n}"),
        add("extrude body from side.outline distance=40 mm symmetric"),
    ]));
    let props = agent.ok("get_properties", json!({"path": "clevis"}));
    near(
        props["result"]["volume_mm3"].as_f64().unwrap(),
        70.0 * 58.0 * 40.0,
        1e-9,
        "block",
    );

    // 4. the fork slot, tags and the rounded gap root
    agent.edit(json!([
        add("sketch fork on plane.XY {\n  rect gap corner=(30 mm, -(jaw_gap / 2)) w=body_len - 30 mm + 5 mm h=jaw_gap\n}"),
        add("extrude slot from fork.gap through_all symmetric op=subtract"),
        add("tag jaw_inner_a = slot.side(fork.gap.top)"),
        add("tag jaw_inner_b = slot.side(fork.gap.bottom)"),
        add("tag gap_floor = slot.side(fork.gap.left)"),
        add("fillet root_round edges=[edge(gap_floor & jaw_inner_a), edge(gap_floor & jaw_inner_b)] r=4 mm"),
    ]));

    // 5. a wrong reference is rejected, with the diagnostics, and the file stays as it was
    let file = root.0.join("clevis/parts/clevis.tcad");
    let before = std::fs::read_to_string(&file).unwrap();
    let (err, data, _) = agent.tool(
        "apply_edit",
        json!({"path": "clevis", "edits": [add("fillet oops edges=[edge(body.side(side.outline.botom) & body.cap_end)] r=1 mm")]}),
    );
    assert!(err);
    assert_eq!(data["rejected"], true);
    assert_eq!(data["diagnostics"][0]["code"], "E3001", "{data:#}");
    assert!(
        data["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("side.outline.bottom")
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    let help = agent.ok("explain_error", json!({"code": "E3001"}));
    assert!(
        help["how_to_fix"]
            .as_str()
            .unwrap()
            .contains("resolve_reference")
    );
    // a parameter outside its range is rejected too
    let (err, data, _) = agent.tool(
        "set_param",
        json!({"path": "clevis", "name": "pin_dia", "value": "5 mm"}),
    );
    assert!(err, "{data:#}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);

    // 6. the agent asks what a reference means before using it
    let r = agent.ok(
        "resolve_reference",
        json!({"path": "clevis", "reference": "body.side(side.outline.right)#2"}),
    );
    assert_eq!(r["resolves_to"][0], "body.side(side.outline.right)#2");
    let (err, data, _) = agent.tool(
        "resolve_reference",
        json!({"path": "clevis", "reference": "body.side(side.outline.right)"}),
    );
    assert!(err);
    assert!(
        data["error"].as_str().unwrap().contains("ordinal"),
        "{data:#}"
    );

    // 7. nose, bore, thread and chamfers
    agent.edit(json!([
        add("fillet nose_round edges=[edge(body.side(side.outline.right)#2 & body.side(side.outline.top)), edge(body.side(side.outline.right)#1 & body.side(side.outline.bottom))] r=4 mm"),
        add("hole pin_bore on=body.side(side.outline.top) at=(-(body_len - 20 mm), 0 mm) d=pin_dia end=through_all"),
        add("hole rod_thread on=body.side(side.outline.left) at=(0 mm, 0 mm) thread=thread depth=28 mm"),
        add("chamfer bore_edges edges=[edge(body.side(side.outline.top) & pin_bore.wall#2), edge(body.side(side.outline.bottom) & pin_bore.wall#1)] d=1.5 mm"),
        add("config heavy { pin_dia = 25 mm; jaw_thk = 20 mm; thread = \"M30x2\" }"),
        add("config default standard { }"),
    ]));

    // 8. the result matches the corpus part at tier T2
    let built = agent.ok("build", json!({"path": "clevis"}));
    assert_eq!(built["features"].as_array().unwrap().len(), 7, "{built:#}");
    let props = agent.ok("get_properties", json!({"path": "clevis"}));
    let want: Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/hyd_rod_clevis/expected.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let got = &props["result"];
    near(
        got["volume_mm3"].as_f64().unwrap(),
        want["volume_mm3"].as_f64().unwrap(),
        1e-6,
        "volume",
    );
    near(
        got["area_mm2"].as_f64().unwrap(),
        want["area_mm2"].as_f64().unwrap(),
        1e-6,
        "area",
    );
    for key in ["center_of_mass_mm", "bbox_min_mm", "bbox_max_mm"] {
        for i in 0..3 {
            let (a, b) = (
                got[key][i].as_f64().unwrap(),
                want[key][i].as_f64().unwrap(),
            );
            assert!((a - b).abs() <= 1e-5, "{key}[{i}]: {a} vs {b}");
        }
    }
    assert!(got["mass_kg"].as_f64().unwrap() > 0.5);

    // 9. look at it
    let topo = agent.ok(
        "query_topology",
        json!({"path": "clevis", "kind": "face", "contains": "pin_bore", "geometry": "cylinder"}),
    );
    assert_eq!(topo["count"], 2, "{topo:#}");
    let (err, data, full) = agent.tool(
        "render_view",
        json!({"path": "clevis", "view": "iso", "size": 256, "highlight": ["pin_bore"]}),
    );
    assert!(!err, "{data:#}");
    assert!(data["highlighted_faces"].as_u64().unwrap() >= 2);
    let image = full["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["type"] == "image")
        .expect("an image");
    assert_eq!(image["mimeType"], "image/png");
    assert!(image["data"].as_str().unwrap().starts_with("iVBORw0KGgo"));

    // 10. change a parameter and export
    agent.ok(
        "set_param",
        json!({"path": "clevis", "name": "body_len", "value": "80 mm"}),
    );
    let longer = agent.ok("get_properties", json!({"path": "clevis"}));
    assert!(longer["result"]["volume_mm3"].as_f64().unwrap() > got["volume_mm3"].as_f64().unwrap());
    let heavy = agent.ok(
        "get_properties",
        json!({"path": "clevis", "config": "heavy"}),
    );
    assert!(
        heavy["result"]["volume_mm3"].as_f64().unwrap()
            > longer["result"]["volume_mm3"].as_f64().unwrap()
    );
    let exp = agent.ok("export", json!({"path": "clevis", "to": "step"}));
    assert_eq!(exp["file"], "clevis/exports/clevis.step");
    assert!(root.0.join("clevis/exports/clevis.step").is_file());
    // paths cannot leave the root
    let (err, _, _) = agent.tool("read_part", json!({"path": "../etc"}));
    assert!(err);
    let part = agent.ok("read_part", json!({"path": "clevis"}));
    assert_eq!(part["part"]["name"], "clevis");
    assert_eq!(part["statements"].as_array().unwrap().len(), 12);
    assert_eq!(part["configurations"].as_array().unwrap().len(), 2);
}
