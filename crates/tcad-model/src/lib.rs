//! `tcad-model`: the typed intermediate representation of a part and its semantic checks.
//!
//! [`analyze`] parses a source text, lowers the syntax tree to the IR ([`Model`]) and checks scopes
//! (references only point backward), units and dimensions, parameter ranges, configurations and
//! enumerated arguments. [`Model::resolve`] evaluates the part under a configuration.

mod check;
mod hints;
pub mod ir;
mod lower;
pub mod sketch;

pub use check::{Analysis, analyze};
pub use hints::solve_hints;
pub use ir::{
    Config, Constraint, Entity, Item, ItemKind, Material, Model, Override, Param, Part, Resolved,
    Sketch,
};
pub use lower::DEFAULT_TOLERANCE_MM;
pub use sketch::{Geometry, SolvedEntity, SolvedSketch};

/// Working name of the format. Defined once per crate so it can be renamed later
/// (see `spec/name.md`).
pub const FORMAT_NAME: &str = "TextCAD";

/// File extension of a part/assembly source file.
pub const FILE_EXTENSION: &str = "tcad";

/// File extension of an archived bundle.
pub const BUNDLE_EXTENSION: &str = "tcadz";
