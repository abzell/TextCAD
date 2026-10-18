//! `tcad-expr`: expression AST, unit algebra and evaluation.

pub mod ast;
pub mod dim;
pub mod eval;
pub mod value;

pub use ast::{Arg, BinOp, Direction, Expr, ExprKind, GenArg, Reference, Segment, UnOp};
pub use dim::{Dim, Unit, base_unit};
pub use eval::{Env, EvalError, eval};
pub use value::{Quantity, Value};

/// Working name of the format. Defined once per crate so it can be renamed later
/// (see `spec/name.md`).
pub const FORMAT_NAME: &str = "TextCAD";

/// File extension of a part/assembly source file.
pub const FILE_EXTENSION: &str = "tcad";

/// File extension of an archived bundle.
pub const BUNDLE_EXTENSION: &str = "tcadz";
