#!/usr/bin/env bash
# Builds OpenCASCADE for the browser module (docs/adr/0014): the 8.0.1 sources compiled with
# Emscripten into static libraries; tools/wasm/link-occt.sh then links them into the side module.
#
#   tools/wasm/build-occt.sh OUT_DIR
#
# Needs emcc/emcmake (emsdk 4.0.9), cmake and ninja on PATH. Produces OUT_DIR/include/opencascade
# (headers for the shim), OUT_DIR/lib/libTK*.a and OUT_DIR/lib/libtcad_math.a.
set -euo pipefail
out=$(realpath -m "${1:?usage: build-occt.sh OUT_DIR}")
version=V8_0_1
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

curl -sSfL "https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/${version}.tar.gz" -o "$work/occt.tar.gz"
tar -xzf "$work/occt.tar.gz" -C "$work"
src=$(echo "$work"/OCCT-*)

# Same floating-point and exception model as the shim and Rust (.cargo/config.toml).
# -UOCC_CONVERT_SIGNALS: OpenCASCADE's CMake defines it on every non-Windows platform, turning
# OCC_CATCH_SIGNALS into setjmp/longjmp, which Emscripten 4.0.9 lowers to invalid code next to wasm
# exceptions (ShapeCustom_BSplineRestriction, ShapeUpgrade_ShapeDivide). WebAssembly has no OS
# signals to convert, the Windows build does not define it either, and the shim is compiled
# without it.
# tcad_math.h: every transcendental function from the vendored musl library, as on Windows
# (third_party/musl-math, docs/adr/0014).
math=$(realpath "$(dirname "$(realpath "$0")")/../../third_party/musl-math")
flags="-fPIC -fwasm-exceptions -ffp-contract=off -UOCC_CONVERT_SIGNALS -include $math/include/tcad_math.h"
emcmake cmake -S "$src" -B "$work/build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_EXPORT_COMPILE_COMMANDS=ON \
  -DCMAKE_C_FLAGS="$flags" -DCMAKE_CXX_FLAGS="$flags" \
  -DBUILD_LIBRARY_TYPE=Static \
  -DINSTALL_DIR="$out" -DCMAKE_INSTALL_PREFIX="$out" \
  -DBUILD_MODULE_Draw=OFF \
  -DBUILD_MODULE_DETools=OFF \
  -DBUILD_DOC_Overview=OFF \
  -DBUILD_SAMPLES_QT=OFF \
  -DUSE_FREETYPE=OFF -DUSE_OPENGL=OFF -DUSE_GLES2=OFF -DUSE_FREEIMAGE=OFF \
  -DUSE_RAPIDJSON=OFF -DUSE_DRACO=OFF -DUSE_TBB=OFF -DUSE_VTK=OFF -DUSE_TK=OFF \
  -DUSE_FFMPEG=OFF -DUSE_OPENVR=OFF
cmake --build "$work/build"
# Recompile the few objects the compiler gets wrong (see repair-objects.mjs); the second build
# re-archives the libraries that contain them.
node "$(dirname "$(realpath "$0")")/repair-objects.mjs" "$work/build"
cmake --build "$work/build"
cmake --install "$work/build"

# The math library goes into the side module next to the toolkits (tools/wasm/link-occt.sh).
mkdir -p "$work/math"
for f in "$math"/src/*.c; do
  emcc -std=c11 -O2 -fPIC -ffp-contract=off -c "$f" -o "$work/math/$(basename "$f" .c).o"
done
emar rcs "$out/lib/libtcad_math.a" "$work"/math/*.o
test -f "$out/include/opencascade/Standard_Version.hxx"
