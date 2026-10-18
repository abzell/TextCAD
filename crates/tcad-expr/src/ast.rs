//! The expression AST, independent of the concrete syntax tree.

use crate::dim::Dim;
use tcad_diag::Span;

/// A reference to a named thing: `side.pin.center`, `boss.side(profile.c1)#2`, `plane.XZ`.
#[derive(Clone, Debug, PartialEq)]
pub struct Reference {
    /// Dot-separated segments.
    pub segments: Vec<Segment>,
    /// Ordinal suffix (`#2`).
    pub ordinal: Option<u32>,
    /// Source span.
    pub span: Span,
}

impl Reference {
    /// The name if this is a plain single identifier (no generator, no ordinal).
    pub fn simple_name(&self) -> Option<&str> {
        match self.segments.as_slice() {
            [s] if s.args.is_none() && self.ordinal.is_none() => Some(&s.name),
            _ => None,
        }
    }

    /// The first segment's name.
    pub fn head(&self) -> &str {
        self.segments.first().map_or("", |s| s.name.as_str())
    }

    /// Dotted text without generators, for messages: `side.pin.center`.
    pub fn dotted(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}

/// One segment of a reference with optional generator arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    /// Name (`side`, `XZ`).
    pub name: String,
    /// Generator arguments, `Some` if parentheses were written.
    pub args: Option<Vec<GenArg>>,
    /// Source span.
    pub span: Span,
}

/// A generator argument: a reference or a direction. `&` separators are not represented; the list
/// is the intersection of its elements.
#[derive(Clone, Debug, PartialEq)]
pub enum GenArg {
    /// A nested reference.
    Ref(Reference),
    /// `+Y`, `-Z`.
    Dir(Direction),
}

/// A signed axis direction such as `+Y`.
#[derive(Clone, Debug, PartialEq)]
pub struct Direction {
    /// True for `-`.
    pub negative: bool,
    /// Axis or plane name (`X`, `Y`, `Z`).
    pub axis: String,
    /// Source span.
    pub span: Span,
}

/// A named argument or flag of a statement, call, profile or query.
#[derive(Clone, Debug, PartialEq)]
pub struct Arg {
    /// Key (or flag name, or `from`).
    pub name: String,
    /// Values; empty for a flag.
    pub values: Vec<Expr>,
    /// True for a flag (no `=`).
    pub flag: bool,
    /// Source span.
    pub span: Span,
}

/// Binary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// Unary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    /// Arithmetic negation.
    Neg,
    /// Boolean negation.
    Not,
}

/// An expression with its source span.
#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    /// The expression.
    pub kind: ExprKind,
    /// Source span.
    pub span: Span,
}

/// Expression forms.
#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    /// A dimensionless number.
    Number(f64),
    /// A quantity, already converted to canonical units (mm, kg, rad).
    Quantity(f64, Dim),
    /// A string.
    Str(String),
    /// `true` or `false`.
    Bool(bool),
    /// A reference (a parameter name or a geometric reference).
    Ref(Reference),
    /// A signed axis direction.
    Direction(Direction),
    /// Unary operation.
    Unary(UnOp, Box<Expr>),
    /// Binary operation.
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// Built-in function call.
    Call(String, Vec<Expr>),
    /// `(a, b, ...)`
    Tuple(Vec<Expr>),
    /// `[a, b, ...]`
    List(Vec<Expr>),
    /// `query(faces, ...)`
    Query(String, Vec<Arg>),
    /// `rect(on=..., ...)` used as a profile.
    Profile(String, Vec<Arg>),
}

impl Expr {
    /// Visits every expression in the tree, including nested arguments, in source order.
    pub fn walk<'a>(&'a self, f: &mut dyn FnMut(&'a Expr)) {
        f(self);
        match &self.kind {
            ExprKind::Unary(_, e) => e.walk(f),
            ExprKind::Binary(_, a, b) => {
                a.walk(f);
                b.walk(f);
            }
            ExprKind::Call(_, es) | ExprKind::Tuple(es) | ExprKind::List(es) => {
                es.iter().for_each(|e| e.walk(f))
            }
            ExprKind::Query(_, args) | ExprKind::Profile(_, args) => {
                args.iter().flat_map(|a| &a.values).for_each(|e| e.walk(f))
            }
            _ => {}
        }
    }

    /// All references in the tree (not descending into generator arguments), with the expression
    /// that contains them.
    pub fn references(&self) -> Vec<&Reference> {
        let mut out = Vec::new();
        self.walk(&mut |e| {
            if let ExprKind::Ref(r) = &e.kind {
                out.push(r);
            }
        });
        out
    }
}
