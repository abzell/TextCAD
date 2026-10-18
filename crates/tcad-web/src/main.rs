//! Entry point of the WebAssembly module. The work is done by the exported functions of the
//! library (`tcad_build`, `tcad_free`, `tcad_glb_ptr`, `tcad_glb_len`), called from JavaScript; the
//! runtime stays alive after `main` returns.

fn main() {
    // keep the exported functions in the executable: nothing in Rust calls them
    std::hint::black_box([
        tcad_web::tcad_build as *const (),
        tcad_web::tcad_free as *const (),
        tcad_web::tcad_glb_ptr as *const (),
        tcad_web::tcad_glb_len as *const (),
        tcad_web::tcad_brep_ptr as *const (),
        tcad_web::tcad_brep_len as *const (),
    ]);
}
