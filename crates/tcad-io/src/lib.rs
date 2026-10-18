//! `tcad-io`: mesh exports (binary STL, 3MF, GLB) from a tessellated shape. STEP is written and read
//! by the kernel itself. Everything here is pure Rust and builds for WebAssembly.
//!
//! The writers are deterministic: the same mesh gives the same bytes (no timestamps).

mod glb;
mod png;
mod render;
mod stl;
mod threemf;
mod zip;

pub use glb::{FaceLabel, glb};
pub use png::png_rgb;
pub use render::{VIEWS, render_png};
pub use stl::stl_binary;
pub use threemf::threemf;

/// Working name of the format. Defined once per crate so it can be renamed later
/// (see `spec/name.md`).
pub const FORMAT_NAME: &str = "TextCAD";

/// File extension of a part/assembly source file.
pub const FILE_EXTENSION: &str = "tcad";

/// File extension of an archived bundle.
pub const BUNDLE_EXTENSION: &str = "tcadz";

/// Names recorded in the exported files.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModelInfo<'a> {
    /// Part name.
    pub name: &'a str,
    /// Configuration that was built.
    pub configuration: Option<&'a str>,
    /// Material name.
    pub material: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_constants() {
        assert_eq!(FORMAT_NAME, "TextCAD");
        assert_eq!(FILE_EXTENSION, "tcad");
        assert_eq!(BUNDLE_EXTENSION, "tcadz");
    }
}
