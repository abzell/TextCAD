# x64-windows with one portable implementation of the transcendental functions (docs/adr/0014):
# every OpenCASCADE source is compiled with third_party/musl-math/include/tcad_math.h force-included,
# and every DLL links the vendored musl library (build it first:
#   cmake -S third_party/musl-math -B build-tcad-math -A x64
#   cmake --build build-tcad-math --config Release)
# so Windows computes sin, cos, pow, ... exactly as the WebAssembly build does.
set(VCPKG_TARGET_ARCHITECTURE x64)
set(VCPKG_CRT_LINKAGE dynamic)
set(VCPKG_LIBRARY_LINKAGE dynamic)

get_filename_component(TCAD_ROOT "${CMAKE_CURRENT_LIST_DIR}/../.." ABSOLUTE)
set(VCPKG_C_FLAGS "/FI${TCAD_ROOT}/third_party/musl-math/include/tcad_math.h")
set(VCPKG_CXX_FLAGS "/FI${TCAD_ROOT}/third_party/musl-math/include/tcad_math.h")
set(VCPKG_LINKER_FLAGS "${TCAD_ROOT}/build-tcad-math/Release/tcad_math.lib")
