//! Tokens and trivia.

use tcad_diag::Span;

/// Kind of a token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// Lowercase identifier or keyword.
    Ident,
    /// Identifier starting with an uppercase letter (axis and plane names).
    Upper,
    /// Numeric literal, text as written.
    Number,
    /// String literal including quotes.
    String,
    /// A line break (significant outside brackets).
    Newline,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `,`
    Comma,
    /// `;`
    Semi,
    /// `=`
    Eq,
    /// `==`
    EqEq,
    /// `!=`
    NotEq,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `.`
    Dot,
    /// `#` directly followed by a digit and attached to the previous token (ordinal suffix).
    Hash,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `^`
    Caret,
    /// `&`
    Amp,
    /// `@`
    At,
    /// A character the lexer could not accept.
    Error,
    /// End of file; carries trailing trivia.
    Eof,
}

/// Kind of trivia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriviaKind {
    /// Spaces, tabs and other insignificant whitespace.
    Whitespace,
    /// `# ...` up to (not including) the line break.
    LineComment,
    /// `/* ... */`
    BlockComment,
}

/// Whitespace or a comment attached before a token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trivia {
    /// Kind.
    pub kind: TriviaKind,
    /// Exact source text.
    pub text: String,
}

impl Trivia {
    /// True for comments.
    pub fn is_comment(&self) -> bool {
        self.kind != TriviaKind::Whitespace
    }
}

/// A token with the trivia that precedes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// Kind.
    pub kind: TokenKind,
    /// Exact source text.
    pub text: String,
    /// Byte span of `text` (excluding trivia).
    pub span: Span,
    /// Trivia between the previous token and this one.
    pub leading: Vec<Trivia>,
}

impl Token {
    /// Comments among the leading trivia.
    pub fn comments(&self) -> impl Iterator<Item = &Trivia> {
        self.leading.iter().filter(|t| t.is_comment())
    }

    /// True if any leading trivia is a comment.
    pub fn has_comments(&self) -> bool {
        self.comments().next().is_some()
    }
}
