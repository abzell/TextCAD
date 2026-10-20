# Building TextCAD

```
cargo build --workspace            # no C++ needed: mock kernel only
cargo test --workspace
```

## With OpenCASCADE (real geometry)

Install OCCT 7.6 or newer, then build with the `occt` feature (`cargo build -p tcad-cli --features occt`).

- **Linux (Debian/Ubuntu):** `sudo apt install libocct-foundation-dev libocct-modeling-data-dev
  libocct-modeling-algorithms-dev libocct-data-exchange-dev libocct-ocaf-dev g++`
- **Windows:** `vcpkg install opencascade:x64-windows`, set `VCPKG_INSTALLATION_ROOT`.
- **Elsewhere:** set `OCCT_INCLUDE_DIR` and `OCCT_LIB_DIR`.

OCCT is linked dynamically (docs/adr/0008-occt-linking-and-license.md).

## Cross-check against build123d (optional)

`pip install build123d && python3 corpus/tools/compare_reference.py corpus`

To read the STEP exports of the corpus with an independent reader:
`python3 corpus/tools/check_step_exports.py target/debug/tcad corpus`.

## Viewer and editor extension

`tcad serve <bundle>` needs a `tcad` built with `--features occt`. The viewer's JavaScript bundle is
checked in (`crates/tcad-cli/assets/viewer.js`), so Rust builds need no Node. To change the viewer:

```
cd viewer
npm ci
npm run check && npm run build      # regenerates the bundle; commit it
TCAD=../target/debug/tcad npm test  # Chromium end-to-end test (Playwright)
```

The VS Code / Theia extension: `cd editors/vscode && npm ci && npm run build`, then load the folder as an
extension (or `npx @vscode/vsce package`). It starts `tcad-lsp` and `tcad` from the `PATH`; the paths
are settings (`textcad.languageServerPath`, `textcad.cliPath`).

## Cross-platform determinism

`corpus/determinism.txt` is checked by `cargo test -p tcad-model --test determinism` on every
platform. To run the non-geometry tests under WebAssembly locally:

```
pip install wasmtime
rustup target add wasm32-wasip1
CARGO_TARGET_WASM32_WASIP1_RUNNER="python3 $PWD/corpus/tools/wasi_run.py" \
  cargo test --target wasm32-wasip1 -p tcad-syntax -p tcad-expr -p tcad-solver -p tcad-model
```
After an intended change run `UPDATE_DETERMINISM=1 cargo test -p tcad-model --test determinism`.

## Browser module (Phase 1b)

Needs the official emsdk 4.0.9 (`emsdk install 4.0.9 && emsdk activate 4.0.9`), CMake and Ninja.

```
tools/wasm/build-occt.sh occt-wasm          # OpenCASCADE 8.0.1 for WebAssembly (~45 min)
tools/wasm/link-occt.sh occt-wasm           # occt.wasm, the replaceable side module
OCCT_INCLUDE_DIR=$PWD/occt-wasm/include/opencascade OCCT_LIB_DIR=$PWD/occt-wasm/lib \
TCAD_OCCT_WASM=$PWD/occt-wasm/occt.wasm \
  cargo build --release -p tcad-web --features occt --target wasm32-unknown-emscripten
cp occt-wasm/occt.wasm target/wasm32-unknown-emscripten/release/
node tools/wasm/gate.mjs target/wasm32-unknown-emscripten/release/tcad-web.js corpus
tools/wasm/site.sh target/wasm32-unknown-emscripten/release site   # the in-browser viewer
```

The gate compares every corpus part's canonical B-rep with `corpus/brep-t1.txt`, which the Windows
build writes (`UPDATE_BREP_T1=1 cargo test -p tcad-build --features occt --test corpus` on Windows
with the `x64-windows-tcad` vcpkg triplet). `BREP_T1_DUMP=DIR` keeps the B-reps of either side for
`python3 tools/brep-diff.py A B`.

