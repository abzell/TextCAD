#!/usr/bin/env bash
# Assembles the static in-browser viewer (docs/adr/0014) into OUT_DIR: page, viewer bundle,
# worker, the WebAssembly module (tcad-web.js + its .wasm) and occt.wasm, plus an example part.
#
#   tools/wasm/site.sh MODULE_DIR OUT_DIR
#
# MODULE_DIR holds tcad-web.js, tcad_web.wasm and occt.wasm (e.g. target/wasm32-unknown-emscripten/release).
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
mod=${1:?usage: site.sh MODULE_DIR OUT_DIR}
out=${2:?usage: site.sh MODULE_DIR OUT_DIR}
mkdir -p "$out"
cp "$root/viewer/web/index.html" "$out/index.html"
cp "$root/crates/tcad-cli/assets/viewer.js" "$out/viewer.js"
cp "$root/viewer/dist/worker.js" "$out/worker.js"
cp "$mod/tcad-web.js" "$out/"
cp "$mod"/*.wasm "$out/"
cp "$root/spec/examples/rod_clevis.tcad" "$out/example.tcad"
ls -la "$out"
