//! `tcad-syntax`: lexer, parser, lossless concrete syntax tree and canonical formatter.

pub mod cst;
pub mod format;
pub mod keywords;
pub mod lexer;
pub mod number;
pub mod parser;
pub mod token;

pub use cst::{Child, Node, NodeKind};
pub use format::{FormatError, format_source};
pub use parser::{Parse, SUPPORTED_VERSION, parse};
pub use token::{Token, TokenKind, Trivia, TriviaKind};

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
}
