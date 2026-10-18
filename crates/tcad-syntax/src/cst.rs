//! The lossless concrete syntax tree.

use crate::token::{Token, TokenKind};
use tcad_diag::Span;

/// Kind of a syntax node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum NodeKind {
    File,
    Header,
    Part,
    Block,
    Tolerance,
    Material,
    Param,
    RangeClause,
    DocClause,
    Import,
    Tag,
    Config,
    ConfigOverride,
    ConfigSuppress,
    Sketch,
    Entity,
    Constraint,
    Hint,
    RefGeom,
    Feature,
    FromClause,
    ProfileCall,
    SuppressClause,
    NamedArg,
    Flag,
    ValueList,
    Reserved,
    Error,
    Binary,
    Unary,
    Paren,
    Vector,
    List,
    Quantity,
    Unit,
    Literal,
    Reference,
    PathSegment,
    Call,
    Query,
    Direction,
}

/// A child of a node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Child {
    /// A nested node.
    Node(Node),
    /// A token.
    Token(Token),
}

/// A syntax node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    /// Kind.
    pub kind: NodeKind,
    /// Children in source order.
    pub children: Vec<Child>,
}

impl Node {
    /// Child nodes only.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().filter_map(|c| match c {
            Child::Node(n) => Some(n),
            Child::Token(_) => None,
        })
    }

    /// Child tokens only (including line breaks).
    pub fn tokens(&self) -> impl Iterator<Item = &Token> {
        self.children.iter().filter_map(|c| match c {
            Child::Token(t) => Some(t),
            Child::Node(_) => None,
        })
    }

    /// First child node of a kind.
    pub fn node_of(&self, kind: NodeKind) -> Option<&Node> {
        self.nodes().find(|n| n.kind == kind)
    }

    /// First child token of a kind.
    pub fn token_of(&self, kind: TokenKind) -> Option<&Token> {
        self.tokens().find(|t| t.kind == kind)
    }

    /// All tokens in the subtree, in source order.
    pub fn descendant_tokens(&self) -> Vec<&Token> {
        let mut out = Vec::new();
        self.collect_tokens(&mut out);
        out
    }

    fn collect_tokens<'a>(&'a self, out: &mut Vec<&'a Token>) {
        for c in &self.children {
            match c {
                Child::Token(t) => out.push(t),
                Child::Node(n) => n.collect_tokens(out),
            }
        }
    }

    /// Span from the first to the last token (excluding outer trivia); `None` if empty.
    pub fn span(&self) -> Option<Span> {
        let toks = self.descendant_tokens();
        let first = toks.first()?;
        let last = toks.last()?;
        Some(Span::new(first.span.start, last.span.end))
    }

    /// Writes the exact source text of the subtree (trivia included).
    pub fn write_source(&self, out: &mut String) {
        for t in self.descendant_tokens() {
            for tr in &t.leading {
                out.push_str(&tr.text);
            }
            out.push_str(&t.text);
        }
    }

    /// The exact source text of the subtree.
    pub fn to_source(&self) -> String {
        let mut s = String::new();
        self.write_source(&mut s);
        s
    }

    /// Source text of the subtree without the first token's leading trivia.
    pub fn trimmed_source(&self) -> String {
        let toks = self.descendant_tokens();
        let mut s = String::new();
        for (i, t) in toks.iter().enumerate() {
            if i > 0 {
                for tr in &t.leading {
                    s.push_str(&tr.text);
                }
            }
            s.push_str(&t.text);
        }
        s
    }
}
