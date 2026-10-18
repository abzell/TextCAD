//! `tcad-kernel`: the abstract kernel operation set.
//!
//! The spec defines what each operation must produce; a backend (OpenCASCADE in `tcad-occt`, the
//! analytic [`mock::MockKernel`] here) is one implementation of that contract. Nothing in the
//! format may depend on a particular backend; see `spec/kernel.md`.

pub mod geometry;
pub mod mock;
pub mod topo;

pub use geometry::{Frame, Loop, P2, Path, Profile, Segment, V3};
pub use topo::{CurveKind, EdgeInfo, FaceInfo, Lineage, Role, SurfaceKind, TopoInfo};

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

/// An edge to fillet: the radius changes linearly from `r1` at the edge start to `r2` at its end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilletEdge {
    /// Edge index.
    pub edge: u32,
    /// Radius at the start (mm).
    pub r1: f64,
    /// Radius at the end (mm).
    pub r2: f64,
}

/// An edge to chamfer: `d1` on the reference `face`, `d2` on the other adjacent face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChamferEdge {
    /// Edge index.
    pub edge: u32,
    /// Reference face index (one of the two faces adjacent to the edge).
    pub face: u32,
    /// Distance on the reference face (mm).
    pub d1: f64,
    /// Distance on the other face (mm).
    pub d2: f64,
}

/// A rigid motion or a mirror image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transform {
    /// Move by a vector (mm).
    Translate(V3),
    /// Turn about the axis through `origin` with unit direction `dir` by `angle` (radians).
    Rotate {
        /// A point on the axis.
        origin: V3,
        /// Unit direction of the axis.
        dir: V3,
        /// Angle (radians).
        angle: f64,
    },
    /// Reflect in the plane through `origin` with unit normal `normal`.
    Mirror {
        /// A point on the plane.
        origin: V3,
        /// Unit normal of the plane.
        normal: V3,
    },
}

/// What a STEP export records about the part.
#[derive(Clone, Copy, Debug, Default)]
pub struct StepMeta<'a> {
    /// Part name.
    pub name: &'a str,
    /// Configuration that was built, if any.
    pub configuration: Option<&'a str>,
    /// Material name and density (g/cm^3), if declared.
    pub material: Option<(&'a str, Option<f64>)>,
}

/// The triangles of one face. Indices refer to the face's own vertices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshFace {
    /// Index of the face in the shape's enumeration (see [`TopoInfo`]).
    pub face: u32,
    /// Vertex positions, x y z interleaved (mm).
    pub positions: Vec<f32>,
    /// Unit normals, x y z interleaved, one per vertex.
    pub normals: Vec<f32>,
    /// Triangle corner indices, counterclockwise seen from outside.
    pub triangles: Vec<u32>,
}

/// A tessellated shape.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    /// One entry per face that has triangles.
    pub faces: Vec<MeshFace>,
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

    /// Lofts through sections placed at the vertices of `guide`, in order from its start to its end,
    /// following the guide as the spine of the loft. The sections are interpolated along the guide.
    fn loft_guided(&self, sections: &[Profile], guide: &Path) -> KResult<Self::Shape>;

    /// Combines two solids. `tolerance` is the modeling tolerance of the part.
    fn boolean(
        &self,
        op: BoolOp,
        a: &Self::Shape,
        b: &Self::Shape,
        tolerance: f64,
    ) -> KResult<Self::Shape>;

    /// Rounds edges. The faces next to the edges are trimmed; each edge gets a new face.
    fn fillet(&self, shape: &Self::Shape, edges: &[FilletEdge]) -> KResult<Self::Shape>;

    /// Bevels edges.
    fn chamfer(&self, shape: &Self::Shape, edges: &[ChamferEdge]) -> KResult<Self::Shape>;

    /// Hollows a solid: the faces in `remove` are opened and the walls get `thickness` (mm),
    /// growing inward, or outward if `outward`.
    fn shell(
        &self,
        shape: &Self::Shape,
        remove: &[u32],
        thickness: f64,
        outward: bool,
    ) -> KResult<Self::Shape>;

    /// Tilts faces by `angle` (radians) about their intersection with the neutral plane
    /// (through `neutral_origin`, normal `neutral_normal`), relative to the pull direction.
    fn draft(
        &self,
        shape: &Self::Shape,
        faces: &[u32],
        pull: V3,
        angle: f64,
        neutral_origin: V3,
        neutral_normal: V3,
    ) -> KResult<Self::Shape>;

    /// Moves a copy of a shape. The lineage maps every face to itself.
    fn transform(&self, shape: &Self::Shape, by: &Transform) -> KResult<Self::Shape>;

    /// Lists the faces and edges of a shape.
    fn topology(&self, shape: &Self::Shape) -> KResult<TopoInfo>;

    /// Where the faces of `shape` came from, as recorded when the operation that made it ran.
    /// Empty for shapes read from the cache.
    fn lineage(&self, shape: &Self::Shape) -> Lineage;

    /// Writes the shape as STEP AP242 (millimetres), with the part name, material and configuration.
    fn export_step(&self, shape: &Self::Shape, meta: &StepMeta<'_>) -> KResult<Vec<u8>>;

    /// Reads the solids of a STEP file as one shape (a "dead" solid: no history).
    fn import_step(&self, bytes: &[u8]) -> KResult<Self::Shape>;

    /// Tessellates the shape. `linear` is the chordal deflection (mm), `angular` the angular
    /// deflection (radians).
    fn mesh(&self, shape: &Self::Shape, linear: f64, angular: f64) -> KResult<Mesh>;

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
