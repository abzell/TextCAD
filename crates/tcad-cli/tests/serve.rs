//! The viewer server: routes, version tracking, and the files it will and will not serve.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use tcad_kernel::mock::MockKernel;

const PART: &str = "tcad 0.1\n\npart blk {\n  param h = 3 mm\n  sketch s on plane.XY {\n    rect r corner=(0 mm, 0 mm) w=2 mm h=3 mm\n  }\n  extrude e from s.r distance=h\n}\n";

fn get(addr: &str, target: &str) -> (String, Vec<u8>) {
    let mut s = TcpStream::connect(addr).expect("connect");
    write!(s, "GET {target} HTTP/1.1\r\nHost: x\r\n\r\n").expect("send");
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).expect("read");
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("header end");
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    (head, raw[split + 4..].to_vec())
}

#[test]
fn serves_the_viewer_the_parts_and_the_changes() {
    let dir = std::env::temp_dir().join(format!("tcad-serve-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("parts")).expect("mkdir");
    std::fs::write(
        dir.join("bundle.toml"),
        "format = \"0.1\"\nname = \"blk\"\nentry = \"parts/blk.tcad\"\nunits = \"mm\"\nlicense = \"Apache-2.0\"\n",
    )
    .expect("bundle");
    let file = dir.join("parts").join("blk.tcad");
    std::fs::write(&file, PART).expect("write");
    std::fs::write(dir.join("secret.txt"), "no").expect("write");

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr").to_string();
    let root = dir.display().to_string();
    let server =
        std::thread::spawn(move || tcad_cli::serve_on(&MockKernel, &listener, &root, Some(9)));

    let (head, body) = get(&addr, "/");
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert!(String::from_utf8_lossy(&body).contains("<canvas id=\"view\""));
    let (head, body) = get(&addr, "/viewer.js");
    assert!(
        head.contains("text/javascript") && body.len() > 10_000,
        "{head}"
    );

    let (_, body) = get(&addr, "/api/parts");
    let parts: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(parts["parts"][0], "parts/blk.tcad");

    let (_, body) = get(&addr, "/api/part?file=parts%2Fblk.tcad&config=");
    let info: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(info["ok"], true, "{info}");
    assert_eq!(info["build"]["features"][0]["name"], "e");
    assert!(info["source"].as_str().expect("source").contains("rect r"));

    let (head, body) = get(&addr, "/api/model.glb?file=parts%2Fblk.tcad");
    // the mock kernel cannot mesh; the real GLB is checked by the browser test in viewer/
    assert!(
        (head.starts_with("HTTP/1.1 200") && body.starts_with(b"glTF"))
            || head.starts_with("HTTP/1.1 500"),
        "{head}"
    );

    // only the bundle's .tcad files are served
    let (head, _) = get(&addr, "/api/part?file=..%2Fsecret.txt");
    assert!(head.starts_with("HTTP/1.1 404"), "{head}");
    let (head, _) = get(&addr, "/api/model.glb?file=%2Fetc%2Fpasswd");
    assert!(head.starts_with("HTTP/1.1 404"), "{head}");

    // saving changes the version
    let (_, a) = get(&addr, "/api/version");
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&file, PART.replace("3 mm\n  sketch", "4 mm\n  sketch")).expect("write");
    let (_, b) = get(&addr, "/api/version");
    assert_ne!(a, b);
    server.join().expect("server");
    let _ = std::fs::remove_dir_all(&dir);
}
