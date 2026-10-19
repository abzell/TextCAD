# musl-math: one implementation of the transcendental functions

The OpenCASCADE builds that must agree bit for bit (Windows, the reference, and the WebAssembly
module in the browser) take `sin`, `cos`, `pow`, `exp`, `log`, ... from this library instead of the
platform's C library, whose results differ in the last bit between Microsoft's runtime, glibc and
musl. See docs/adr/0014-in-browser-geometry.md.

## Source

The math functions of [musl](https://musl.libc.org/) 1.2.5 (`src/math`, plus `src/internal/libm.h`),
taken from the copy that Emscripten 4.0.9 ships in `system/lib/libc/musl`. MIT licence: see
`COPYRIGHT`. The functions are the ones OpenCASCADE 8.0.1 imports whose results are not exactly
specified by IEEE 754 (`sqrt`, `fmod`, `floor`, `round`, `trunc`, `modf`, `remainder` and `scalbn`
are exact everywhere and stay the platform's):

    sin cos tan asin acos atan atan2 sinh cosh tanh asinh acosh atanh
    exp expm1 log log10 log1p pow cbrt sinf cosf tanf acosf powf

and the internal functions and tables they use.

## Changes from musl

- Every `.c` file starts with `#include "tcad_musl_names.h"`, which includes the platform's
  `<math.h>` first and then renames the public functions to `tcad_*` and the internal ones to
  `tcad_musl_*`, so they never collide with the C library.
- `src/libm.h` is musl's `src/internal/libm.h` without the long double layout (no long double function
  is vendored), `<endian.h>` and `fp_arch.h`; `hidden` is empty; `__FP_FAST_FMA` is undefined so `log`
  and `pow` take the same non-FMA path on every compiler; `FORCE_EVAL` is the same on every target.

No algorithm is changed. The library must be compiled without floating-point contraction
(`-ffp-contract=off`; MSVC's default `/fp:precise`) and without fast-math options.

## Use

`include/tcad_math.h` is force-included into every OpenCASCADE and shim source file of the Windows
build (`ci/vcpkg-triplets/x64-windows-tcad.cmake`, `/FI`) and the WebAssembly build
(`tools/wasm/build-occt.sh`, `-include`). It declares the `tcad_*` functions and defines `sin` as
`tcad_sin` and so on (`std::sin` included), so every call goes to this library.

`CMakeLists.txt` builds the static library for Windows; `tools/wasm/build-occt.sh` and
`crates/tcad-occt/build.rs` compile the same sources directly.

## Checked

Compiled natively (gcc, x86-64, `-ffp-contract=off`) and to WebAssembly (Emscripten 4.0.9), the
library gives identical bits for 200 000 arguments per function (including `sin` up to 1e22), and
C++ code compiled with `tcad_math.h` force-included (`std::sin(float)`, `std::pow(x, 2)`,
`std::valarray`) gives identical results under libstdc++ and libc++. A build with FMA contraction
does not, which is why contraction must stay off.
