#!/usr/bin/env bash
# Diagnoses invalid WebAssembly in the OpenCASCADE build: lists every object file in the static
# libraries that the engine rejects, then recompiles each one's source with candidate compiler flags
# and reports which produce valid code.
#
#   tools/wasm/probe-objects.sh OCCT_WASM_DIR
set -uo pipefail
out=$(realpath "${1:?usage: probe-objects.sh OCCT_WASM_DIR}")
here=$(dirname "$(realpath "$0")")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

for lib in "$out"/lib/libTK*.a; do
  d="$work/objs/$(basename "$lib" .a)"
  mkdir -p "$d"
  (cd "$d" && emar x "$lib")
done
node "$here/validate-objects.mjs" "$work/objs" > "$work/invalid.txt"
cat "$work/invalid.txt"
[ -s "$work/invalid.txt" ] || exit 0

curl -sSfL "https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/V8_0_1.tar.gz" -o "$work/occt.tar.gz"
tar -xzf "$work/occt.tar.gz" -C "$work"
src=$(echo "$work"/OCCT-*)
base="-DNDEBUG -fPIC -fwasm-exceptions -ffp-contract=off -I$out/include/opencascade"
while read -r obj _; do
  file=$(basename "$obj" .o)
  source=$(find "$src/src" -name "$file" | head -1)
  [ -n "$source" ] || { echo "no source for $obj"; continue; }
  for flags in "-O3" "-O3 -mno-multivalue" "-O2" "-O1" "-O3 -mno-reference-types" "-Os"; do
    if em++ $flags $base -I"$(dirname "$source")" -c "$source" -o "$work/probe.o" 2>"$work/err.txt"; then
      node -e 'process.stdout.write(WebAssembly.validate(require("fs").readFileSync(process.argv[1])) ? "valid" : "INVALID")' "$work/probe.o"
    else
      printf 'compile error: %s' "$(head -c 200 "$work/err.txt")"
    fi
    echo "  $file [$flags]"
  done
done < "$work/invalid.txt"
