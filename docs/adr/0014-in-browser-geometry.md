# 0014. In-browser geometry (Phase 1b): OpenCASCADE in WebAssembly

- Status: proposed
- Date: 2026-10-18

## Context

Phase 1b (requirements section 13) moves regeneration into the browser: "OCCT in WASM, live
regeneration in the viewer. Gate: all corpus parts regenerate in the browser at T1 against the
Windows build." Section 11 asks for "an Emscripten build of OCCT bridged to the Rust WASM module" and
for this ADR before work starts. Section 12 allows LGPL "only for OCCT and only dynamically linked",
and asks that the WASM linking approach be checked against OCCT's LGPL-2.1 + exception.

Three facts shape the design:

1. **T1 today is per platform** (spec/kernel.md: "same backend id and version, same platform
   build"). The gate needs more: the browser build must reproduce the Windows build's canonical
   B-rep byte for byte. Windows uses OpenCASCADE 8.0.1 (vcpkg); Linux CI uses 7.6.3 (apt).
2. **Floating point across compilers.** MSVC (Windows) and clang (Emscripten) both use IEEE
   doubles with SSE/wasm semantics, but clang may contract `a*b+c` into an FMA unless told not to,
   and the C libraries differ (MSVC CRT vs Emscripten's musl) in transcendental functions, which
   are not correctly rounded. Either can change the last bit of a coordinate and so the B-rep text.
3. **OpenCASCADE's hashed containers** key on shape addresses; any result that depended on their
   iteration order would already break T1 between two runs on one platform, which the corpus test
   shows does not happen for our operations.

## Options

1. **opencascade.js** (prebuilt npm package): JavaScript bindings over OCCT 7.6, its own API, not
   our shim. Our kernel code (`tcad-occt/cpp/shim.cc`, naming, history) would have to be rewritten
   in JavaScript, and the version would not match Windows. Rejected.
2. **Rust `wasm32-unknown-emscripten` + Emscripten build of OCCT 8.0.1 + the same shim.** One code
   path for native and browser: `tcad-build`, `tcad-naming`, `tcad-io` and `tcad-occt` compile
   unchanged; only the build script learns the Emscripten target. Chosen.
3. **`wasm32-unknown-unknown` + a separate Emscripten OCCT module** called through JavaScript glue:
   two runtimes, copies of every shape across the boundary, two allocators. Rejected.

For linking OCCT into the module:

- **a. Static** into one `.wasm`: simplest and fastest, but contradicts "only dynamically linked".
- **b. Emscripten dynamic linking**: OCCT built as a side module (`-sSIDE_MODULE=1`, `occt.wasm`)
  loaded at run time by the main module (Rust + shim, `-sMAIN_MODULE=2`). The user can replace
  `occt.wasm` with a modified build, which is what LGPL-2.1 section 6 asks of a "work that uses the
  Library". Chosen, with (a) only as a measured fallback that needs the maintainer's agreement.

## Decision

- **One OpenCASCADE version everywhere: 8.0.1.** Windows already uses it; Linux CI moves from apt
  7.6.3 to conda-forge `occt 8.0.1`; the browser build compiles the 8.0.1 sources. The backend id
  stays `occt-<version>`, so caches never mix versions.
- **Browser module**: a new crate `tcad-web` (target `wasm32-unknown-emscripten`) exporting a small C
  ABI: build a part from source text and configuration and return the manifest (JSON), the canonical
  B-rep and the GLB, or the diagnostics. It runs in a Web Worker; the viewer gets an in-browser mode
  (edit the text, regenerate on each change) and keeps `tcad serve` as the native path.
- **Emscripten pinned** to 4.0.9 from the official emsdk, whose LLVM and binaryen are released
  together (conda-forge's `emscripten 4.0.9` pairs with binaryen 117, which rejects the wasm
  features emcc asks for when it optimises the side module); OCCT built with `emcmake`, only the toolkits we link (FoundationClasses, ModelingData,
  ModelingAlgorithms, the STEP part of DataExchange), no threads, no visualization, cached in CI.
  `OCC_CONVERT_SIGNALS` is undefined for this build (as on Windows): its setjmp/longjmp handlers
  made Emscripten emit invalid code next to wasm exceptions, and WebAssembly has no OS signals to
  convert. `tools/wasm/repair-objects.mjs` validates every object after the build and recompiles
  any the engine rejects. `occt.wasm` is linked without the link-time `wasm-opt` pass (the objects
  are already optimised) and the main module is linked with `-sMAIN_MODULE=2`.
- **Floating point**: OCCT and the shim are compiled with `-ffp-contract=off` on every clang build
  (Emscripten, Linux), matching MSVC's `/fp:precise`. The C library difference is measured first
  (the gate harness reports, per corpus part, the first differing line of the canonical B-rep). If
  transcendental functions differ, all three builds are routed to one portable implementation (the
  musl `libm` sources, already the basis of the `libm` crate our expression evaluator uses):
  Emscripten through link-time wrapping, Windows through a vcpkg overlay port with a forced-include
  header. That step is taken only if the measurement shows it is needed.

  **Measured (2026-10-19)** and taken: with every transcendental call in our own code on the `libm`
  crate and rotations passed as quaternions, 30 of 37 corpus parts were identical on Windows and in
  the browser; the other 7 differed in the last bits of sin/cos results inside OpenCASCADE (amplified
  to 1e-8..1e-6 in fitted fillet and blend surfaces), with identical topology and line counts. The
  browser matched Linux (glibc) on several of them: the Microsoft C runtime rounds differently.
  `third_party/musl-math` (musl 1.2.5's math, MIT) is therefore compiled into both builds and
  `third_party/musl-math/include/tcad_math.h` is force-included into every OpenCASCADE and shim
  source: on Windows through the vcpkg triplet `ci/vcpkg-triplets/x64-windows-tcad.cmake` (`/FI`, the
  library on every DLL's link line; CI and the release use it), in the browser through
  `tools/wasm/build-occt.sh` (`-include`, the library inside `occt.wasm`). The library gives identical
  bits natively and in WebAssembly (checked over 200 000 arguments per function). Linux keeps
  conda-forge's OpenCASCADE and glibc and reports its differences without failing.
- **Licence check (LGPL-2.1 with the OCCT exception 1.0)**: OCCT stays a separately replaceable
  component in the browser (`occt.wasm`), its sources and our build script are published, and our
  own code is Apache-2.0 with complete sources, so a user can rebuild or relink with a modified OCCT.
  The OCCT exception additionally permits use of OCCT's header-only templates and inline code in our
  shim without LGPL obligations. The static fallback, if ever needed, would ship our object files and
  link script with each release to keep relinking possible, and would be recorded in a new ADR.
- **Gate test**: the Windows job writes the SHA-256 of every corpus part's canonical B-rep to
  `corpus/brep-t1.txt` (committed, like `corpus/determinism.txt`); a CI job regenerates every part
  in the browser module (Node and headless Chromium) and must match every line. The Linux 8.0.1 job
  checks the same file, which tells compiler/C-library differences apart from WebAssembly ones.

## Consequences

- spec/kernel.md gains a cross-platform clause for T1 under one OCCT version, and the browser
  backend; the gate is reproducible from the committed hash file.
- The first Emscripten build of OCCT is long (comparable to the vcpkg build) and is cached by OCCT
  version and Emscripten version.
- If bit identity across MSVC and musl cannot be reached even with a shared libm, the gate as
  written cannot pass; that outcome is reported with the per-part differences rather than worked
  around by loosening T1.
