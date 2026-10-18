//! The `tcad-lsp` language server on standard input and output.

use std::path::PathBuf;
use tcad_lsp::{NamedEntity, Provider, Server};

/// Reads the persistent names of the last build from the bundle's cache.
struct CacheProvider;

fn file_path(uri: &str) -> Option<PathBuf> {
    let p = uri.strip_prefix("file://")?;
    // `file:///C:/x` on Windows
    let p = p
        .strip_prefix('/')
        .filter(|r| r.as_bytes().get(1) == Some(&b':'))
        .unwrap_or(p);
    Some(PathBuf::from(p.replace("%20", " ")))
}

impl Provider for CacheProvider {
    fn topology(&self, uri: &str) -> Vec<NamedEntity> {
        let Some(path) = file_path(uri) else {
            return Vec::new();
        };
        let Ok(bundle) = tcad_cache::load_bundle(&path) else {
            return Vec::new();
        };
        let Ok(src) = std::fs::read_to_string(&path) else {
            return Vec::new();
        };
        let Some(part) = tcad_model::analyze(&src).model.map(|m| m.part.name) else {
            return Vec::new();
        };
        let dir = tcad_cache::cache_dir(&bundle.root, &part);
        tcad_cache::read_manifest(&dir)
            .and_then(|m| m.features.last().map(|f| f.topology.clone()))
            .unwrap_or_default()
            .into_iter()
            .map(|t| NamedEntity {
                kind: t.kind,
                id: t.id,
                aliases: t.aliases,
                geometry: t.geometry,
                size: t.size,
            })
            .collect()
    }
}

fn main() {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("tcad-lsp {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut server = Server::new(CacheProvider);
    if let Err(e) = server.serve(stdin.lock(), &mut stdout) {
        eprintln!("tcad-lsp: {e}");
        std::process::exit(1);
    }
    std::process::exit(if server.is_shutdown() { 0 } else { 1 });
}
