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
    /// Part number.
    pub number: Option<&'a str>,
    /// Revision.
    pub revision: Option<&'a str>,
    /// Display colour, sRGB components in 0..1.
    pub color: Option<[f64; 3]>,
}

/// `#RRGGBB` for an sRGB colour with components in 0..1.
pub(crate) fn hex_color(c: [f64; 3]) -> String {
    let byte = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", byte(c[0]), byte(c[1]), byte(c[2]))
}

/// An sRGB component (0..1) in linear light, as glTF's `baseColorFactor` expects.
pub(crate) fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        libm::pow((c + 0.055) / 1.055, 2.4)
    }
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
