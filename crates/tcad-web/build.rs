//! Link settings of the browser module (docs/adr/0014): a main module that loads OpenCASCADE as a
//! side module, exported to JavaScript as the factory `createTcad`. `TCAD_OCCT_WASM` names the
//! side module (built by tools/wasm/build-occt.sh); without it the module uses the mock kernel.

fn main() {
    println!("cargo:rerun-if-env-changed=TCAD_OCCT_WASM");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("emscripten") {
        return;
    }
    for arg in [
        "-sMODULARIZE=1",
        "-sEXPORT_NAME=createTcad",
        "-sINVOKE_RUN=0",
        "-sALLOW_MEMORY_GROWTH=1",
        "-sSTACK_SIZE=4MB",
        "-sEXPORTED_RUNTIME_METHODS=ccall,UTF8ToString,stringToNewUTF8,HEAPU8",
        "-sEXPORTED_FUNCTIONS=_main,_tcad_build,_tcad_free,_tcad_glb_ptr,_tcad_glb_len,_malloc,_free",
        "-fwasm-exceptions",
    ] {
        println!("cargo:rustc-link-arg={arg}");
    }
    if let Ok(side) = std::env::var("TCAD_OCCT_WASM") {
        println!("cargo:rustc-link-arg=-sMAIN_MODULE=1");
        println!("cargo:rustc-link-arg={side}");
        println!("cargo:rerun-if-changed={side}");
    }
}
