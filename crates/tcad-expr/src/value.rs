//! Evaluated values.

use crate::dim::Dim;
use std::fmt;

/// A number with a dimension, in canonical units (mm, kg, rad).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity {
    /// Value in canonical units.
    pub value: f64,
    /// Dimension.
    pub dim: Dim,
}

impl Quantity {
    /// A dimensionless number.
    pub fn number(value: f64) -> Self {
        Quantity {
            value,
            dim: Dim::NONE,
        }
    }

    /// A length in millimetres.
    pub fn mm(value: f64) -> Self {
        Quantity {
            value,
            dim: Dim::LENGTH,
        }
    }
}

impl fmt::Display for Quantity {
    /// Shows lengths in mm, angles in degrees, masses in kg, densities in kg/m^3.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.dim {
            Dim::NONE => write!(f, "{}", self.value),
            Dim::LENGTH => write!(f, "{} mm", self.value),
            Dim::ANGLE => write!(f, "{} deg", self.value * 180.0 / std::f64::consts::PI),
            Dim::MASS => write!(f, "{} kg", self.value),
            Dim::DENSITY => write!(f, "{} kg/m^3", self.value * 1.0e9),
            d => write!(f, "{} ({d})", self.value),
        }
    }
}

/// The result of evaluating an expression.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A number or quantity.
    Num(Quantity),
    /// A boolean.
    Bool(bool),
    /// A string.
    Str(String),
    /// `(a, b, ...)`, used for points and vectors.
    Tuple(Vec<Value>),
    /// `[a, b, ...]`
    List(Vec<Value>),
}

impl Value {
    /// A short description of the value's type for messages.
    pub fn type_name(&self) -> String {
        match self {
            Value::Num(q) => q.dim.to_string(),
            Value::Bool(_) => "a boolean".into(),
            Value::Str(_) => "a string".into(),
            Value::Tuple(_) => "a tuple".into(),
            Value::List(_) => "a list".into(),
        }
    }

    /// The quantity, if this is one.
    pub fn as_quantity(&self) -> Option<Quantity> {
        match self {
            Value::Num(q) => Some(*q),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Num(q) => write!(f, "{q}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::Tuple(v) | Value::List(v) => {
                let items: Vec<String> = v.iter().map(ToString::to_string).collect();
                let (open, close) = if matches!(self, Value::Tuple(_)) {
                    ('(', ')')
                } else {
                    ('[', ']')
                };
                write!(f, "{open}{}{close}", items.join(", "))
            }
        }
    }
}
