//! Builds the C++ shim and links OpenCASCADE when the `occt` feature is on.
//!
//! OCCT is located through environment variables, in this order:
//! `OCCT_INCLUDE_DIR` / `OCCT_LIB_DIR`; then vcpkg (`VCPKG_INSTALLATION_ROOT` or `VCPKG_ROOT`, with
//! `VCPKG_TARGET_TRIPLET` defaulting to `x64-windows`); then the Linux system paths
//! (`/usr/include/opencascade`, `/usr/lib/x86_64-linux-gnu`). OCCT is always linked dynamically
//! (it is LGPL-2.1 with an exception; see docs/adr/0008-occt-linking-and-license.md).
//!
//! On Windows and in the browser the shim is compiled like OpenCASCADE itself there: with
//! `third_party/musl-math/include/tcad_math.h` force-included, so every transcendental function
//! comes from one portable implementation (docs/adr/0014). On Windows the shim links its own copy
//! of that library; in the browser it comes from `occt.wasm`.

use std::path::{Path, PathBuf};

#[cfg_attr(not(feature = "occt"), allow(dead_code))]
const LIBS: &[&str] = &[
    "TKernel",
    "TKMath",
    "TKG2d",
    "TKG3d",
    "TKGeomBase",
    "TKBRep",
    "TKGeomAlgo",
    "TKTopAlgo",
    "TKPrim",
    "TKBO",
    "TKBool",
    "TKShHealing",
    "TKOffset",
    "TKFillet",
    "TKFeat",
    "TKMesh",
    "TKXSBase",
    "TKLCAF",
    "TKCDF",
    "TKCAF",
    "TKXCAF",
];

/// The STEP toolkits: OpenCASCADE up to 7.x splits them into five libraries, 8.0 merges them into
/// `TKDESTEP` (and drops `TKXDESTEP`).
#[cfg_attr(not(feature = "occt"), allow(dead_code))]
const STEP_LEGACY: &[&str] = &[
    "TKSTEPBase",
    "TKSTEPAttr",
    "TKSTEP209",
    "TKSTEP",
    "TKXDESTEP",
];

#[cfg_attr(not(feature = "occt"), allow(dead_code))]
fn has_lib(dir: &Path, name: &str) -> bool {
    ["", "lib"].iter().any(|prefix| {
        ["lib", "so", "dylib", "a"]
            .iter()
            .any(|ext| dir.join(format!("{prefix}{name}.{ext}")).is_file())
    })
}

/// The C files of the vendored math library.
#[cfg_attr(not(feature = "occt"), allow(dead_code))]
fn c_sources(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("third_party/musl-math/src")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "c"))
        .collect();
    files.sort();
    files
}

#[cfg_attr(not(feature = "occt"), allow(dead_code))]
fn locate() -> (PathBuf, PathBuf) {
    if let (Some(i), Some(l)) = (
        std::env::var_os("OCCT_INCLUDE_DIR"),
        std::env::var_os("OCCT_LIB_DIR"),
    ) {
        return (i.into(), l.into());
    }
    let vcpkg =
        std::env::var_os("VCPKG_INSTALLATION_ROOT").or_else(|| std::env::var_os("VCPKG_ROOT"));
    if let Some(root) = vcpkg {
        let triplet =
            std::env::var("VCPKG_TARGET_TRIPLET").unwrap_or_else(|_| "x64-windows".into());
        let base = Path::new(&root).join("installed").join(triplet);
        if base.join("include").join("opencascade").is_dir() {
            return (base.join("include").join("opencascade"), base.join("lib"));
        }
    }
    (
        PathBuf::from("/usr/include/opencascade"),
        PathBuf::from("/usr/lib/x86_64-linux-gnu"),
    )
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/bridge.rs");
    println!("cargo:rerun-if-changed=cpp/shim.cc");
    println!("cargo:rerun-if-changed=cpp/shim.h");
    println!("cargo:rerun-if-env-changed=OCCT_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=OCCT_LIB_DIR");
    #[cfg(feature = "occt")]
    {
        let (include, lib) = locate();
        assert!(
            include.join("Standard_Version.hxx").is_file(),
            "OpenCASCADE headers not found in {} (set OCCT_INCLUDE_DIR and OCCT_LIB_DIR)",
            include.display()
        );
        let mut build = cxx_build::bridge("src/bridge.rs");
        build
            .file("cpp/shim.cc")
            .include(&include)
            .include(".")
            .std("c++17")
            .warnings(false);
        // MSVC runs destructors during exception unwinding only with /EHsc. Without it a C++
        // exception thrown inside a `std::lock_guard` scope leaves the mutex locked (and leaks
        // scratch files): the next STEP export then waits forever.
        let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
        let emscripten = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten");
        if msvc {
            build.flag("/EHsc");
        }
        if msvc || emscripten {
            let math = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/musl-math");
            let header = math.join("include").join("tcad_math.h");
            println!("cargo:rerun-if-changed={}", math.display());
            if msvc {
                build.flag(format!("/FI{}", header.display()));
                let mut libm = cc::Build::new();
                libm.files(c_sources(&math.join("src")))
                    .std("c11")
                    .flag("/fp:precise")
                    .warnings(false);
                libm.compile("tcad_math");
            } else {
                build.flag("-include").flag(header.display().to_string());
            }
        }
        build.compile("tcad_occt_shim");
        // In the browser OpenCASCADE is a side module (occt.wasm) that the final executable
        // (crates/tcad-web) loads at start-up: nothing to link here (docs/adr/0014).
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten") {
            return;
        }
        println!("cargo:rustc-link-search=native={}", lib.display());
        for l in LIBS {
            println!("cargo:rustc-link-lib=dylib={l}");
        }
        let step: Vec<&str> = if STEP_LEGACY.iter().all(|l| has_lib(&lib, l)) {
            STEP_LEGACY.to_vec()
        } else if has_lib(&lib, "TKDESTEP") {
            vec!["TKDESTEP"]
        } else {
            STEP_LEGACY.to_vec()
        };
        for l in step {
            println!("cargo:rustc-link-lib=dylib={l}");
        }
    }
}
