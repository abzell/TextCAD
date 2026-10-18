#!/usr/bin/env bash
# Links the OpenCASCADE static libraries from tools/wasm/build-occt.sh into one side module,
# OUT_DIR/occt.wasm, which the main module (crates/tcad-web) loads at start-up and a user can
# replace with their own build (docs/adr/0014). Undefined C/C++ runtime symbols are resolved from
# the main module at load time.
#
#   tools/wasm/link-occt.sh OUT_DIR
set -euo pipefail
out=$(realpath "${1:?usage: link-occt.sh OUT_DIR}")
libs=$(find "$out/lib" -name 'libTK*.a' | sort)
em++ -sSIDE_MODULE=1 -fwasm-exceptions -O2 \
  -Wl,--whole-archive $libs -Wl,--no-whole-archive \
  -o "$out/occt.wasm"
ls -la "$out/occt.wasm"
