//! `tcad-solver`: an in-house 2D sketch constraint solver (no GPL code).
//!
//! A [`Problem`] holds variables, primitives (points, lines, circles, arcs, ellipses, rectangles,
//! polygons, slots) and constraints. [`Problem::solve`] starts from the initial values, splits the
//! equations into independent clusters, and solves each with Newton steps (damped CGLS, so an
//! under-determined cluster moves the *least* distance) with Levenberg-Marquardt damping as the
//! fallback. Jacobians are exact (forward-mode autodiff). A cluster that cannot be solved yields a
//! minimal [`Conflict`] naming the constraints involved; [`Problem::analyze`] reports degrees of
//! freedom per entity and redundant constraints.

pub mod dual;
pub mod equation;
// numerical kernels index several arrays in lock step; iterator forms would obscure them
#[allow(clippy::needless_range_loop)]
pub mod linalg;
pub mod problem;
pub mod solve;

pub use problem::{
    Arc, Circle, Constraint, Curve, Ellipse, Entity, Line, Point, Polygon, Problem, Rect, Slot,
    Unsupported, Var,
};
pub use solve::{Analysis, Cluster, ClusterResult, Conflict, SOLVED_TOL, Solution};

/// Working name of the format. Defined once per crate so it can be renamed later
/// (see `spec/name.md`).
pub const FORMAT_NAME: &str = "TextCAD";

/// File extension of a part/assembly source file.
pub const FILE_EXTENSION: &str = "tcad";

/// File extension of an archived bundle.
pub const BUNDLE_EXTENSION: &str = "tcadz";
