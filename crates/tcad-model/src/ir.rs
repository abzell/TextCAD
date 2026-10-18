//! The typed intermediate representation of a part.

use std::collections::BTreeSet;
use tcad_diag::Span;
use tcad_expr::{Arg, Expr, Quantity, Reference, Value};

/// A checked source file.
#[derive(Clone, Debug)]
pub struct Model {
    /// Format version from the header.
    pub version: (u32, u32),
    /// The single part of the file.
    pub part: Part,
}

/// A part: parameters, an ordered feature graph and configurations.
#[derive(Clone, Debug)]
pub struct Part {
    /// Identifier.
    pub name: String,
    /// Display name (the quoted string after the identifier).
    pub display_name: Option<String>,
    /// Source span of the name.
    pub name_span: Span,
    /// Modeling tolerance in mm (default 1e-6 mm).
    pub tolerance: f64,
    /// Material, if declared.
    pub material: Option<Material>,
    /// Parameters in declaration order.
    pub params: Vec<Param>,
    /// Sketches, reference geometry, features, tags and imports in declaration order (the
    /// feature order).
    pub items: Vec<Item>,
    /// Configurations in declaration order.
    pub configs: Vec<Config>,
}

/// A material.
#[derive(Clone, Debug)]
pub struct Material {
    /// Name, e.g. `AISI 1045`.
    pub name: String,
    /// Density in kg/mm^3 (canonical units), if given.
    pub density: Option<f64>,
}

/// A parameter.
#[derive(Clone, Debug)]
pub struct Param {
    /// Identifier.
    pub name: String,
    /// Defining expression.
    pub expr: Expr,
    /// Declared `range(lo, hi)` bounds.
    pub range: Option<(Expr, Expr)>,
    /// Documentation string.
    pub doc: Option<String>,
    /// Value with no configuration applied.
    pub value: Value,
    /// Span of the whole declaration.
    pub span: Span,
    /// Span of the name.
    pub name_span: Span,
}

/// What an [`Item`] is.
#[derive(Clone, Debug)]
pub enum ItemKind {
    /// `plane`, `axis`, `point`, `csys`.
    RefGeom,
    /// A feature (`extrude`, `hole`, ...).
    Feature,
    /// A sketch.
    Sketch(Sketch),
    /// `tag name = reference`.
    Tag(Reference),
    /// `import name = step("path")`.
    Import(String),
}

/// A statement of the feature graph.
#[derive(Clone, Debug)]
pub struct Item {
    /// What it is.
    pub kind: ItemKind,
    /// The statement keyword (`extrude`, `sketch`, `tag`, ...).
    pub keyword: String,
    /// Declared name.
    pub name: String,
    /// Arguments in source order (`from` is an argument named `from`).
    pub args: Vec<Arg>,
    /// `suppress if <bool expr>`.
    pub suppress_if: Option<Expr>,
    /// Span of the whole statement.
    pub span: Span,
    /// Span of the name.
    pub name_span: Span,
}

/// A sketch.
#[derive(Clone, Debug)]
pub struct Sketch {
    /// The plane or face it lies on.
    pub plane: Reference,
    /// Entities in declaration order.
    pub entities: Vec<Entity>,
    /// Constraints in declaration order.
    pub constraints: Vec<Constraint>,
}

/// A sketch entity.
#[derive(Clone, Debug)]
pub struct Entity {
    /// `line`, `arc`, `circle`, ...
    pub kind: String,
    /// Name.
    pub name: String,
    /// Arguments.
    pub args: Vec<Arg>,
    /// Construction geometry flag.
    pub construction: bool,
    /// Solver hint values (`@hint(...)`).
    pub hint: Option<Vec<Expr>>,
    /// Span.
    pub span: Span,
    /// Span of the name.
    pub name_span: Span,
}

/// A sketch constraint or dimension.
#[derive(Clone, Debug)]
pub struct Constraint {
    /// Constraint kind, e.g. `coincident`, `distance`.
    pub kind: String,
    /// Arguments (references or values).
    pub args: Vec<Expr>,
    /// Dimension value after `=`.
    pub value: Option<Expr>,
    /// Span.
    pub span: Span,
}

/// A configuration (variant).
#[derive(Clone, Debug)]
pub struct Config {
    /// Name.
    pub name: String,
    /// Marked `default`.
    pub is_default: bool,
    /// Parameter overrides.
    pub overrides: Vec<Override>,
    /// Features suppressed in this configuration.
    pub suppress: Vec<Reference>,
    /// Span.
    pub span: Span,
    /// Span of the name.
    pub name_span: Span,
}

/// A parameter override in a configuration.
#[derive(Clone, Debug)]
pub struct Override {
    /// Parameter name.
    pub param: String,
    /// New defining expression.
    pub expr: Expr,
    /// Span.
    pub span: Span,
}

/// The state of a part under one configuration.
#[derive(Clone, Debug)]
pub struct Resolved {
    /// Configuration name; `None` for the base part.
    pub config: Option<String>,
    /// Parameter values in declaration order.
    pub params: Vec<(String, Value)>,
    /// Names of suppressed items.
    pub suppressed: BTreeSet<String>,
}

impl Resolved {
    /// The value of a parameter.
    pub fn param(&self, name: &str) -> Option<&Value> {
        self.params.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    /// A length parameter in mm.
    pub fn length_mm(&self, name: &str) -> Option<f64> {
        match self.param(name)? {
            Value::Num(Quantity { value, dim }) if *dim == tcad_expr::Dim::LENGTH => Some(*value),
            _ => None,
        }
    }
}
