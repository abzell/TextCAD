# 0008. OpenCASCADE linking, license and WASM

Status: accepted (M5)

## Context
The kernel is OpenCASCADE Technology (OCCT), LGPL-2.1 with the OCCT exception. TextCAD itself is
Apache-2.0. Phase 1b needs a browser build.

## Decision
- OCCT is **always linked dynamically** (shared libraries), so the LGPL relinking requirement is met
  without extra steps. Release bundles ship the OCCT shared libraries beside the executable together
  with the OCCT license text.
- The Rust bridge is `cxx`; the only unsafe/C++ code lives in `tcad-occt`, behind the `occt` feature.
  Every other crate keeps `unsafe_code = forbid`.
- Without the `occt` feature, the workspace builds and tests with no C++ toolchain (mock backend).
  `tcad-kernel` and `tcad-cache` stay free of native dependencies so they build for `wasm32`.
- The browser build (Phase 1b) will compile OCCT to WASM (Emscripten) behind the same `Kernel` trait.
  T2 equivalence (spec/kernel.md) is the contract between the native and the WASM build.

## Consequences
CI needs OCCT installed on every OS that runs OCCT tests (apt on Linux, vcpkg on Windows).
