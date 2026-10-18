//! The `tcad-mcp` server: MCP over standard input and output.
//!
//! `tcad-mcp [--root DIR]` serves the bundles below DIR (default: the current directory).

fn main() {
    let mut root = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => match args.next() {
                Some(d) => root = d.into(),
                None => {
                    eprintln!("tcad-mcp: --root needs a directory");
                    std::process::exit(2);
                }
            },
            "--help" | "-h" => {
                println!(
                    "Usage: tcad-mcp [--root DIR]\n\nA Model Context Protocol server (JSON-RPC lines on stdin/stdout)."
                );
                return;
            }
            "--version" | "-V" => {
                println!("tcad-mcp {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            other => {
                eprintln!("tcad-mcp: unknown argument `{other}`");
                std::process::exit(2);
            }
        }
    }
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    #[cfg(feature = "occt")]
    let server = tcad_mcp::Server::new(tcad_occt::OcctKernel::new(), root);
    #[cfg(not(feature = "occt"))]
    let server = tcad_mcp::Server::new(tcad_kernel::mock::MockKernel, root).with_geometry(false);
    if let Err(e) = server.serve(stdin.lock(), &mut stdout) {
        eprintln!("tcad-mcp: {e}");
        std::process::exit(1);
    }
}
