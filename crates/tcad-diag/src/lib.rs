//! `tcad-diag`: diagnostics shared by all TextCAD crates.
//!
//! Every diagnostic has a stable code (see [`codes`] and `spec/diagnostics.md`), a byte span, a
//! message and, where possible, a machine-readable fix hint. Diagnostics render as text for humans
//! and as JSON for agents.

pub mod codes;
mod render;

pub use codes::Code;
pub use render::{LineIndex, render_json, render_text};

use serde::Serialize;

/// Working name of the format (see `spec/name.md`).
pub const FORMAT_NAME: &str = "TextCAD";

/// File extension of a part/assembly source file.
pub const FILE_EXTENSION: &str = "tcad";

/// File extension of an archived bundle.
pub const BUNDLE_EXTENSION: &str = "tcadz";

/// A half-open byte range `start..end` in a source text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
pub struct Span {
    /// Byte offset of the first byte.
    pub start: usize,
    /// Byte offset one past the last byte.
    pub end: usize,
}

impl Span {
    /// Creates a span; `end` is clamped to be at least `start`.
    pub fn new(start: usize, end: usize) -> Self {
        Span {
            start,
            end: end.max(start),
        }
    }

    /// Smallest span covering both.
    pub fn cover(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

/// How serious a diagnostic is. Errors make a command fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The input is invalid.
    Error,
    /// The input is accepted but suspicious or newer than supported.
    Warning,
}

/// One text replacement of a fix hint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Edit {
    /// Range to replace (empty for a pure insertion).
    pub span: Span,
    /// Replacement text.
    pub replacement: String,
}

/// A suggested fix that tools can apply mechanically.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FixHint {
    /// What applying the edits does.
    pub message: String,
    /// The edits, in source order, non-overlapping.
    pub edits: Vec<Edit>,
}

/// A single diagnostic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    /// Stable code, e.g. `E1102`.
    pub code: Code,
    /// Severity.
    pub severity: Severity,
    /// Human-readable message.
    pub message: String,
    /// Where it applies.
    pub span: Span,
    /// Optional mechanical fix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<FixHint>,
}

impl Diagnostic {
    /// An error diagnostic.
    pub fn error(code: Code, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            code,
            severity: Severity::Error,
            message: message.into(),
            span,
            hint: None,
        }
    }

    /// A warning diagnostic.
    pub fn warning(code: Code, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            ..Diagnostic::error(code, span, message)
        }
    }

    /// Attaches a fix hint.
    pub fn with_hint(mut self, hint: FixHint) -> Self {
        self.hint = Some(hint);
        self
    }

    /// True for errors.
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// True if any diagnostic is an error.
pub fn has_errors(diags: &[Diagnostic]) -> bool {
    diags.iter().any(Diagnostic::is_error)
}
