//! `tcad-cache`: bundles and the derived-data cache (`spec/bundle.md`).
//!
//! A design is a directory: the `.tcad` text files are the source of truth and everything under
//! `.cache/` is derived and may be deleted. A cache entry is valid only if its source hash and the
//! kernel id/version both match.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Working name of the format. Defined once per crate so it can be renamed later
/// (see `spec/name.md`).
pub const FORMAT_NAME: &str = "TextCAD";

/// File extension of a part/assembly source file.
pub const FILE_EXTENSION: &str = "tcad";

/// File extension of an archived bundle.
pub const BUNDLE_EXTENSION: &str = "tcadz";

/// Name of the bundle description file.
pub const BUNDLE_FILE: &str = "bundle.toml";

/// Name of the cache directory inside a bundle.
pub const CACHE_DIR: &str = ".cache";

/// File names inside a part's cache directory.
pub const BREP_FILE: &str = "brep.brep";
/// The manifest file name.
pub const MANIFEST_FILE: &str = "manifest.json";

/// The contents of `bundle.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleMeta {
    /// Format version of the bundle layout, e.g. `0.1`.
    pub format: String,
    /// Human-readable name.
    #[serde(default)]
    pub name: Option<String>,
    /// The entry part, relative to the bundle root.
    pub entry: String,
    /// Default units for display, e.g. `mm`.
    #[serde(default)]
    pub units: Option<String>,
    /// License of the design.
    #[serde(default)]
    pub license: Option<String>,
}

/// A located bundle.
#[derive(Clone, Debug)]
pub struct Bundle {
    /// The bundle directory.
    pub root: PathBuf,
    /// The entry part file.
    pub entry: PathBuf,
    /// The metadata, if the directory has a `bundle.toml`.
    pub meta: Option<BundleMeta>,
}

/// Locates a bundle from a path: a bundle directory, or a `.tcad` file (inside a bundle, or alone).
pub fn load_bundle(path: &Path) -> Result<Bundle, String> {
    if path.is_dir() {
        let file = path.join(BUNDLE_FILE);
        let text = std::fs::read_to_string(&file)
            .map_err(|e| format!("{} has no readable {BUNDLE_FILE}: {e}", path.display()))?;
        let meta: BundleMeta =
            toml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        let entry = path.join(&meta.entry);
        if !entry.is_file() {
            return Err(format!("the entry part {} does not exist", entry.display()));
        }
        return Ok(Bundle {
            root: path.to_path_buf(),
            entry,
            meta: Some(meta),
        });
    }
    if path.is_file() {
        // inside a bundle: .../parts/x.tcad with a bundle.toml one or two levels up
        let mut dir = path.parent();
        for _ in 0..2 {
            let Some(d) = dir else { break };
            if d.join(BUNDLE_FILE).is_file() {
                let mut b = load_bundle(d)?;
                b.entry = path.to_path_buf();
                return Ok(b);
            }
            dir = d.parent();
        }
        let root = path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        return Ok(Bundle {
            root,
            entry: path.to_path_buf(),
            meta: None,
        });
    }
    Err(format!("no such file or directory: {}", path.display()))
}

/// The cache directory of a part: `<bundle>/.cache/<part>/`.
pub fn cache_dir(root: &Path, part: &str) -> PathBuf {
    root.join(CACHE_DIR).join(part)
}

/// SHA-256 of bytes as lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// The source hash of a part: SHA-256 over the canonical text, the resolved configuration and the
/// bytes of imported files. Each piece is length-prefixed so pieces cannot run together.
pub fn source_hash(canonical: &str, config: Option<&str>, imports: &[(String, Vec<u8>)]) -> String {
    let mut h = Sha256::new();
    let mut piece = |label: &str, bytes: &[u8]| {
        h.update((label.len() as u64).to_le_bytes());
        h.update(label.as_bytes());
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    };
    piece("tcad-source-v1", b"");
    piece("text", canonical.as_bytes());
    piece("config", config.unwrap_or("").as_bytes());
    for (name, bytes) in imports {
        piece(&format!("import:{name}"), bytes);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// What one feature contributed (topology and volume after it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeatureEntry {
    /// Feature name.
    pub name: String,
    /// Statement keyword.
    pub keyword: String,
    /// Solids after this feature.
    pub solids: usize,
    /// Faces after this feature.
    pub faces: usize,
    /// Edges after this feature.
    pub edges: usize,
    /// Vertices after this feature.
    pub vertices: usize,
    /// Volume after this feature (mm^3).
    pub volume_mm3: f64,
}

/// Measurements of the result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResultEntry {
    /// Solids.
    pub solids: usize,
    /// Faces.
    pub faces: usize,
    /// Edges.
    pub edges: usize,
    /// Vertices.
    pub vertices: usize,
    /// Volume (mm^3).
    pub volume_mm3: f64,
    /// Surface area (mm^2).
    pub area_mm2: f64,
    /// Center of mass (mm).
    pub center_of_mass_mm: [f64; 3],
    /// Bounding box minimum (mm).
    pub bbox_min_mm: [f64; 3],
    /// Bounding box maximum (mm).
    pub bbox_max_mm: [f64; 3],
}

/// `manifest.json`: what the cache holds and what it was built from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Format version.
    pub format: String,
    /// Part name.
    pub part: String,
    /// Configuration that was built (`null` for the base part).
    pub config: Option<String>,
    /// Source hash (see [`source_hash`]).
    pub source_hash: String,
    /// Kernel id and version, e.g. `occt-7.6.3`.
    pub kernel: String,
    /// Modeling tolerance (mm).
    pub tolerance_mm: f64,
    /// Per-feature topology and volume.
    pub features: Vec<FeatureEntry>,
    /// Measurements of the result.
    pub result: ResultEntry,
    /// SHA-256 of `brep.brep`.
    pub brep_sha256: String,
}

/// Reads a manifest, if there is a valid one.
pub fn read_manifest(dir: &Path) -> Option<Manifest> {
    let text = std::fs::read_to_string(dir.join(MANIFEST_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Writes the B-rep and the manifest (creating the directory).
pub fn write_cache(dir: &Path, manifest: &Manifest, brep: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(BREP_FILE), brep)?;
    let mut text = serde_json::to_string_pretty(manifest)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    text.push('\n');
    std::fs::write(dir.join(MANIFEST_FILE), text)
}

/// True if the cache in `dir` was built from this source by this kernel and is intact.
pub fn is_fresh(dir: &Path, source_hash: &str, kernel_id: &str) -> Option<Manifest> {
    let m = read_manifest(dir)?;
    if m.source_hash != source_hash || m.kernel != kernel_id {
        return None;
    }
    let brep = std::fs::read(dir.join(BREP_FILE)).ok()?;
    (sha256_hex(&brep) == m.brep_sha256).then_some(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tmp(PathBuf);
    impl Tmp {
        fn new(tag: &str) -> Tmp {
            let p = std::env::temp_dir().join(format!("tcad-cache-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).expect("mkdir");
            Tmp(p)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn manifest(hash: &str, kernel: &str, brep: &[u8]) -> Manifest {
        Manifest {
            format: "0.1".into(),
            part: "p".into(),
            config: None,
            source_hash: hash.into(),
            kernel: kernel.into(),
            tolerance_mm: 1e-6,
            features: vec![],
            result: ResultEntry {
                solids: 1,
                faces: 6,
                edges: 12,
                vertices: 8,
                volume_mm3: 1.0,
                area_mm2: 6.0,
                center_of_mass_mm: [0.5; 3],
                bbox_min_mm: [0.0; 3],
                bbox_max_mm: [1.0; 3],
            },
            brep_sha256: sha256_hex(brep),
        }
    }

    #[test]
    fn sha256_known_answer() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn source_hash_depends_on_every_piece_and_pieces_do_not_run_together() {
        let base = source_hash("text", None, &[]);
        assert_eq!(base, source_hash("text", None, &[]));
        assert_ne!(base, source_hash("text2", None, &[]));
        assert_ne!(base, source_hash("text", Some("heavy"), &[]));
        assert_ne!(
            base,
            source_hash("text", None, &[("a.step".into(), vec![1])])
        );
        assert_ne!(
            source_hash("ab", Some("c"), &[]),
            source_hash("a", Some("bc"), &[]),
            "length prefixes keep the pieces apart"
        );
    }

    #[test]
    fn freshness_needs_matching_hash_kernel_and_intact_brep() {
        let t = Tmp::new("fresh");
        let dir = cache_dir(&t.0, "p");
        let brep = b"BREP";
        write_cache(&dir, &manifest("h1", "occt-7.6.3", brep), brep).unwrap();
        assert!(is_fresh(&dir, "h1", "occt-7.6.3").is_some());
        assert!(
            is_fresh(&dir, "h2", "occt-7.6.3").is_none(),
            "source changed"
        );
        assert!(
            is_fresh(&dir, "h1", "occt-7.9.0").is_none(),
            "kernel changed"
        );
        std::fs::write(dir.join(BREP_FILE), b"tampered").unwrap();
        assert!(
            is_fresh(&dir, "h1", "occt-7.6.3").is_none(),
            "damaged B-rep"
        );
        assert!(is_fresh(&t.0.join("nowhere"), "h1", "k").is_none());
    }

    #[test]
    fn loading_bundles_and_loose_files() {
        let t = Tmp::new("bundle");
        std::fs::create_dir_all(t.0.join("parts")).unwrap();
        std::fs::write(
            t.0.join(BUNDLE_FILE),
            "format = \"0.1\"\nname = \"demo\"\nentry = \"parts/demo.tcad\"\nunits = \"mm\"\n",
        )
        .unwrap();
        std::fs::write(t.0.join("parts/demo.tcad"), "tcad 0.1\n").unwrap();
        let b = load_bundle(&t.0).unwrap();
        assert_eq!(b.meta.as_ref().unwrap().name.as_deref(), Some("demo"));
        assert_eq!(b.entry, t.0.join("parts/demo.tcad"));
        // a file inside the bundle finds the bundle
        let b = load_bundle(&t.0.join("parts/demo.tcad")).unwrap();
        assert_eq!(b.root, t.0);
        // a loose file is its own bundle
        let loose = Tmp::new("loose");
        std::fs::write(loose.0.join("x.tcad"), "tcad 0.1\n").unwrap();
        let b = load_bundle(&loose.0.join("x.tcad")).unwrap();
        assert!(b.meta.is_none());
        assert_eq!(b.root, loose.0);
        // errors
        assert!(
            load_bundle(&loose.0).is_err(),
            "directory without bundle.toml"
        );
        assert!(load_bundle(&t.0.join("missing")).is_err());
        std::fs::write(
            t.0.join(BUNDLE_FILE),
            "format = \"0.1\"\nentry = \"parts/nope.tcad\"\n",
        )
        .unwrap();
        assert!(load_bundle(&t.0).unwrap_err().contains("does not exist"));
    }
}
