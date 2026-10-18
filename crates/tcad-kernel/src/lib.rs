//! `tcad-kernel`: the abstract kernel operation set.
//!
//! The spec defines what each operation must produce; a backend (OpenCASCADE in `tcad-occt`, the
//! analytic [`mock::MockKernel`] here) is one implementation of that contract. Nothing in the
//! format may depend on a particular backend; see `spec/kernel.md`.

pub mod geometry;
pub mod mock;

pub use geometry::{Frame, Loop, P2, Path, Profile, Segment, V3};

/// Working name of the format. Defined once per crate so it can be renamed later
/// (see `spec/name.md`).
pub const FORMAT_NAME: &str = "TextCAD";

/// File extension of a part/assembly source file.
pub const FILE_EXTENSION: &str = "tcad";

/// File extension of an archived bundle.
pub const BUNDLE_EXTENSION: &str = "tcadz";

/// A kernel operation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KernelError(pub String);

impl std::fmt::Display for KernelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for KernelError {}

/// Result of a kernel operation.
pub type KResult<T> = Result<T, KernelError>;

/// Boolean operations on solids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolOp {
    /// `a` united with `b`.
    Union,
    /// `a` with `b` removed.
    Subtract,
    /// The common part of `a` and `b`.
    Intersect,
}

/// Counts of topological entities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Topology {
    /// Solids.
    pub solids: usize,
    /// Faces.
    pub faces: usize,
    /// Edges.
    pub edges: usize,
    /// Vertices.
    pub vertices: usize,
}

/// Geometric properties of a shape for unit density.
#[derive(Clone, Debug, PartialEq)]
pub struct Properties {
    /// Volume (mm^3).
    pub volume: f64,
    /// Surface area (mm^2).
    pub area: f64,
    /// Center of mass (mm).
    pub center_of_mass: V3,
    /// Inertia tensor about the center of mass, axes parallel to the world axes, for unit density
    /// (mm^5), if the backend supplies it.
    pub inertia_com: Option<[[f64; 3]; 3]>,
    /// Axis-aligned bounding box minimum (mm).
    pub bbox_min: V3,
    /// Axis-aligned bounding box maximum (mm).
    pub bbox_max: V3,
    /// Topology counts.
    pub topology: Topology,
}

/// The operations a geometry kernel provides (requirements 8.1). Lengths are in millimetres and
/// angles in radians. Every operation is a pure function of its inputs: no hidden state, no
/// parallel reductions, fixed iteration orders.
pub trait Kernel {
    /// The backend's shape handle (cheap to clone).
    type Shape: Clone;

    /// Backend identifier and version, stored in the cache manifest (for example `occt-7.6.3`).
    fn id(&self) -> String;

    /// Sweeps a profile along `dir` (a unit vector) by `distance` (mm, may be negative). A
    /// positive `draft` (radians) tapers the sides inward along the sweep.
    fn extrude(
        &self,
        profile: &Profile,
        dir: V3,
        distance: f64,
        draft: f64,
    ) -> KResult<Self::Shape>;

    /// Revolves a profile about the axis through `axis_origin` with direction `axis_dir`.
    fn revolve(
        &self,
        profile: &Profile,
        axis_origin: V3,
        axis_dir: V3,
        angle: f64,
    ) -> KResult<Self::Shape>;

    /// Sweeps a profile along a path, optionally controlled by a guide curve.
    fn sweep(&self, profile: &Profile, path: &Path, guide: Option<&Path>) -> KResult<Self::Shape>;

    /// Lofts through the sections in order (smooth, or ruled with straight sides).
    fn loft(&self, sections: &[Profile], ruled: bool) -> KResult<Self::Shape>;

    /// Combines two solids. `tolerance` is the modeling tolerance of the part.
    fn boolean(
        &self,
        op: BoolOp,
        a: &Self::Shape,
        b: &Self::Shape,
        tolerance: f64,
    ) -> KResult<Self::Shape>;

    /// Measures a shape.
    fn properties(&self, shape: &Self::Shape) -> KResult<Properties>;

    /// A canonical serialization of the shape: identical bytes for identical shapes built by the
    /// same backend on the same platform (determinism tier T1). Also the cache format.
    fn canonical_brep(&self, shape: &Self::Shape) -> KResult<Vec<u8>>;

    /// Reads a shape back from [`Kernel::canonical_brep`].
    #[allow(clippy::wrong_self_convention)]
    fn from_brep(&self, bytes: &[u8]) -> KResult<Self::Shape>;
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
