# The vendored musl math functions as a static library (README.md). Used by the Windows build of
# OpenCASCADE; the WebAssembly build compiles the same sources in tools/wasm/build-occt.sh.
cmake_minimum_required(VERSION 3.16)
project(tcad_math C)
file(GLOB sources ${CMAKE_CURRENT_SOURCE_DIR}/src/*.c)
add_library(tcad_math STATIC ${sources})
set_target_properties(tcad_math PROPERTIES C_STANDARD 11 C_STANDARD_REQUIRED ON
  MSVC_RUNTIME_LIBRARY "MultiThreadedDLL" POSITION_INDEPENDENT_CODE ON)
# One floating-point model everywhere: no contraction into FMA, no fast math.
if(MSVC)
  target_compile_options(tcad_math PRIVATE /fp:precise /W3 /wd4116 /wd4146 /wd4244)
else()
  target_compile_options(tcad_math PRIVATE -ffp-contract=off)
endif()
