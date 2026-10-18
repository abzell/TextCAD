//! The lexer. Total: every input produces a token stream whose texts and trivia concatenate back to
//! the input, plus diagnostics for anything invalid.

use crate::token::{Token, TokenKind, Trivia, TriviaKind};
use tcad_diag::{Diagnostic, Span, codes};

/// Lexes `src` into tokens (ending with an [`TokenKind::Eof`] token) and diagnostics.
pub fn lex(src: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut lx = Lexer {
        src,
        bytes: src.as_bytes(),
        pos: 0,
        tokens: Vec::new(),
        diags: Vec::new(),
    };
    lx.run();
    (lx.tokens, lx.diags)
}

struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
    tokens: Vec<Token>,
    diags: Vec<Diagnostic>,
}

impl Lexer<'_> {
    fn run(&mut self) {
        loop {
            let leading = self.trivia();
            let start = self.pos;
            let Some(c) = self.src[self.pos..].chars().next() else {
                self.push(TokenKind::Eof, start, leading);
                return;
            };
            let kind = match c {
                '\n' => self.single(TokenKind::Newline),
                '{' => self.single(TokenKind::LBrace),
                '}' => self.single(TokenKind::RBrace),
                '(' => self.single(TokenKind::LParen),
                ')' => self.single(TokenKind::RParen),
                '[' => self.single(TokenKind::LBracket),
                ']' => self.single(TokenKind::RBracket),
                ',' => self.single(TokenKind::Comma),
                ';' => self.single(TokenKind::Semi),
                '.' => self.single(TokenKind::Dot),
                '+' => self.single(TokenKind::Plus),
                '-' => self.single(TokenKind::Minus),
                '*' => self.single(TokenKind::Star),
                '/' => self.single(TokenKind::Slash),
                '^' => self.single(TokenKind::Caret),
                '&' => self.single(TokenKind::Amp),
                '@' => self.single(TokenKind::At),
                '#' => self.single(TokenKind::Hash),
                '=' => self.one_or_two(b'=', TokenKind::Eq, TokenKind::EqEq),
                '<' => self.one_or_two(b'=', TokenKind::Lt, TokenKind::Le),
                '>' => self.one_or_two(b'=', TokenKind::Gt, TokenKind::Ge),
                '!' => {
                    if self.bytes.get(self.pos + 1) == Some(&b'=') {
                        self.pos += 2;
                        TokenKind::NotEq
                    } else {
                        self.error_char(c)
                    }
                }
                '"' => self.string(),
                '0'..='9' => self.number(),
                'a'..='z' | 'A'..='Z' | '_' => self.ident(),
                other => self.error_char(other),
            };
            self.push(kind, start, leading);
        }
    }

    fn push(&mut self, kind: TokenKind, start: usize, leading: Vec<Trivia>) {
        self.tokens.push(Token {
            kind,
            text: self.src[start..self.pos].to_string(),
            span: Span::new(start, self.pos),
            leading,
        });
    }

    fn single(&mut self, kind: TokenKind) -> TokenKind {
        self.pos += 1;
        kind
    }

    fn one_or_two(&mut self, second: u8, one: TokenKind, two: TokenKind) -> TokenKind {
        if self.bytes.get(self.pos + 1) == Some(&second) {
            self.pos += 2;
            two
        } else {
            self.pos += 1;
            one
        }
    }

    fn error_char(&mut self, c: char) -> TokenKind {
        let start = self.pos;
        self.pos += c.len_utf8();
        self.diags.push(Diagnostic::error(
            codes::E1001,
            Span::new(start, self.pos),
            format!("unexpected character `{c}`"),
        ));
        TokenKind::Error
    }

    fn ident(&mut self) -> TokenKind {
        let start = self.pos;
        while self
            .bytes
            .get(self.pos)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            self.pos += 1;
        }
        let text = &self.src[start..self.pos];
        if self.bytes[start].is_ascii_uppercase() {
            TokenKind::Upper
        } else {
            if text.bytes().any(|b| b.is_ascii_uppercase()) {
                self.diags.push(Diagnostic::error(
                    codes::E1006,
                    Span::new(start, self.pos),
                    format!("identifier `{text}` must be lowercase"),
                ));
            }
            TokenKind::Ident
        }
    }

    fn digits(&mut self) {
        while self.bytes.get(self.pos).is_some_and(u8::is_ascii_digit) {
            self.pos += 1;
        }
    }

    fn number(&mut self) -> TokenKind {
        self.digits();
        if self.bytes.get(self.pos) == Some(&b'.')
            && self.bytes.get(self.pos + 1).is_some_and(u8::is_ascii_digit)
        {
            self.pos += 1;
            self.digits();
        }
        if matches!(self.bytes.get(self.pos), Some(b'e' | b'E')) {
            let mut p = self.pos + 1;
            if matches!(self.bytes.get(p), Some(b'+' | b'-')) {
                p += 1;
            }
            if self.bytes.get(p).is_some_and(u8::is_ascii_digit) {
                self.pos = p;
                self.digits();
            }
        }
        TokenKind::Number
    }

    fn string(&mut self) -> TokenKind {
        let start = self.pos;
        self.pos += 1;
        loop {
            match self.bytes.get(self.pos) {
                None | Some(b'\n') => {
                    self.diags.push(Diagnostic::error(
                        codes::E1002,
                        Span::new(start, self.pos),
                        "unterminated string",
                    ));
                    break;
                }
                Some(b'"') => {
                    self.pos += 1;
                    break;
                }
                Some(b'\\') => {
                    let esc_start = self.pos;
                    self.pos += 1;
                    match self.src[self.pos..].chars().next() {
                        Some(c @ ('"' | '\\' | 'n' | 't')) => self.pos += c.len_utf8(),
                        Some('\n') | None => {}
                        Some(c) => {
                            self.pos += c.len_utf8();
                            self.diags.push(Diagnostic::error(
                                codes::E1004,
                                Span::new(esc_start, self.pos),
                                format!("invalid escape `\\{c}` (use \\\", \\\\, \\n or \\t)"),
                            ));
                        }
                    }
                }
                Some(_) => {
                    let c = self.src[self.pos..].chars().next().unwrap_or('\0');
                    self.pos += c.len_utf8().max(1);
                }
            }
        }
        TokenKind::String
    }

    fn trivia(&mut self) -> Vec<Trivia> {
        let mut out = Vec::new();
        loop {
            let start = self.pos;
            match self.bytes.get(self.pos) {
                Some(b' ' | b'\t') => {
                    while matches!(self.bytes.get(self.pos), Some(b' ' | b'\t')) {
                        self.pos += 1;
                    }
                    out.push(self.trivia_item(TriviaKind::Whitespace, start));
                }
                Some(b'\r') => {
                    self.pos += 1;
                    self.diags.push(Diagnostic::error(
                        codes::E1005,
                        Span::new(start, self.pos),
                        "carriage return: files must use LF line endings",
                    ));
                    out.push(self.trivia_item(TriviaKind::Whitespace, start));
                }
                Some(0xEF) if self.src[self.pos..].starts_with('\u{feff}') => {
                    self.pos += '\u{feff}'.len_utf8();
                    self.diags.push(Diagnostic::error(
                        codes::E1005,
                        Span::new(start, self.pos),
                        "byte order mark: files must be UTF-8 without BOM",
                    ));
                    out.push(self.trivia_item(TriviaKind::Whitespace, start));
                }
                Some(b'#') if !self.is_ordinal_hash() => {
                    while self.bytes.get(self.pos).is_some_and(|b| *b != b'\n') {
                        self.pos += 1;
                    }
                    out.push(self.trivia_item(TriviaKind::LineComment, start));
                }
                Some(b'/') if self.bytes.get(self.pos + 1) == Some(&b'*') => {
                    match self.src[self.pos + 2..].find("*/") {
                        Some(i) => self.pos += 2 + i + 2,
                        None => {
                            self.pos = self.src.len();
                            self.diags.push(Diagnostic::error(
                                codes::E1003,
                                Span::new(start, self.pos),
                                "unterminated block comment",
                            ));
                        }
                    }
                    out.push(self.trivia_item(TriviaKind::BlockComment, start));
                }
                _ => return out,
            }
        }
    }

    /// `#` is an ordinal suffix (`side#2`) when attached to the previous token and followed by a
    /// digit; otherwise it starts a comment.
    fn is_ordinal_hash(&self) -> bool {
        let attached = self.pos > 0
            && self.tokens.last().is_some_and(|t| t.span.end == self.pos)
            && !matches!(self.tokens.last().map(|t| t.kind), Some(TokenKind::Newline));
        attached && self.bytes.get(self.pos + 1).is_some_and(u8::is_ascii_digit)
    }

    fn trivia_item(&self, kind: TriviaKind, start: usize) -> Trivia {
        Trivia {
            kind,
            text: self.src[start..self.pos].to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src).0.iter().map(|t| t.kind).collect()
    }

    #[test]
    fn lossless_roundtrip() {
        let src = "tcad 0.1 # c\n/* b */ x=1e-6 mm\r\n\"s\\n\" $ #2";
        let (toks, _) = lex(src);
        let mut out = String::new();
        for t in &toks {
            for tr in &t.leading {
                out.push_str(&tr.text);
            }
            out.push_str(&t.text);
        }
        assert_eq!(out, src);
    }

    #[test]
    fn numbers_and_units() {
        use TokenKind::*;
        assert_eq!(kinds("25 mm"), [Number, Ident, Eof]);
        assert_eq!(kinds("1.5e-3"), [Number, Eof]);
        assert_eq!(kinds("2e"), [Number, Ident, Eof]);
        assert_eq!(kinds("1."), [Number, Dot, Eof]);
    }

    #[test]
    fn ordinal_hash_vs_comment() {
        use TokenKind::*;
        assert_eq!(kinds("a.b#2"), [Ident, Dot, Ident, Hash, Number, Eof]);
        assert_eq!(kinds("a #2"), [Ident, Eof]);
        assert_eq!(kinds("a#x"), [Ident, Eof]);
    }

    #[test]
    fn diagnostics() {
        assert_eq!(lex("$").1[0].code, codes::E1001);
        assert_eq!(lex("\"abc").1[0].code, codes::E1002);
        assert_eq!(lex("/* x").1[0].code, codes::E1003);
        assert_eq!(lex("\"\\q\"").1[0].code, codes::E1004);
        assert_eq!(lex("a\r\n").1[0].code, codes::E1005);
        assert_eq!(lex("aB").1[0].code, codes::E1006);
    }
}
