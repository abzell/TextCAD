//! `tcad-occt`: the OpenCASCADE implementation of [`tcad_kernel::Kernel`].
//!
//! The C++ lives in `cpp/shim.cc` and is bridged with `cxx`. The `occt` feature builds and links it
//! (dynamically; OCCT is LGPL-2.1 with an exception). Without the feature the crate is empty
//! except for [`AVAILABLE`], so tools can report that no geometry kernel is present.

#[cfg(feature = "occt")]
mod bridge;
#[cfg(feature = "occt")]
mod kernel;

#[cfg(feature = "occt")]
pub use kernel::{OcctKernel, OcctShape};

/// True if this build links OpenCASCADE.
pub const AVAILABLE: bool = cfg!(feature = "occt");

/// Working name of the format. Defined once per crate so it can be renamed later
/// (see `spec/name.md`).
pub const FORMAT_NAME: &str = "TextCAD";

/// File extension of a part/assembly source file.
pub const FILE_EXTENSION: &str = "tcad";

/// File extension of an archived bundle.
pub const BUNDLE_EXTENSION: &str = "tcadz";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_constants() {
        assert_eq!(FORMAT_NAME, "TextCAD");
        assert_eq!(FILE_EXTENSION, "tcad");
        assert_eq!(BUNDLE_EXTENSION, "tcadz");
    }

    #[test]
    fn availability_matches_the_feature() {
        assert_eq!(AVAILABLE, cfg!(feature = "occt"));
    }
}
