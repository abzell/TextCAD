//! Builds the C++ shim and links OpenCASCADE when the `occt` feature is on.
//!
//! OCCT is located through environment variables, in this order:
//! `OCCT_INCLUDE_DIR` / `OCCT_LIB_DIR`; then vcpkg (`VCPKG_INSTALLATION_ROOT` or `VCPKG_ROOT`, with
//! `VCPKG_TARGET_TRIPLET` defaulting to `x64-windows`); then the Linux system paths
//! (`/usr/include/opencascade`, `/usr/lib/x86_64-linux-gnu`). OCCT is always linked dynamically
//! (it is LGPL-2.1 with an exception; see docs/adr/0008-occt-linking-and-license.md).

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
    "TKSTEPBase",
    "TKSTEPAttr",
    "TKSTEP209",
    "TKSTEP",
    "TKLCAF",
    "TKCDF",
    "TKCAF",
    "TKXCAF",
    "TKXDESTEP",
];

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
        build.compile("tcad_occt_shim");
        println!("cargo:rustc-link-search=native={}", lib.display());
        for l in LIBS {
            println!("cargo:rustc-link-lib=dylib={l}");
        }
    }
}
