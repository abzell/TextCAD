//! The parser: tokens to a lossless CST with error recovery.
//!
//! The parser never panics and always makes progress. On a syntax error it records a diagnostic,
//! wraps the unparsable rest of the statement in an [`NodeKind::Error`] node and continues with the
//! next statement.

use crate::cst::{Child, Node, NodeKind};
use crate::keywords as kw;
use crate::lexer::lex;
use crate::token::{Token, TokenKind as K};
use tcad_diag::{Diagnostic, Edit, FixHint, Span, codes};

/// The format version this implementation reads and writes.
pub const SUPPORTED_VERSION: (u32, u32) = (0, 1);

/// Result of parsing.
#[derive(Debug)]
pub struct Parse {
    /// The tree (always present, even with errors).
    pub root: Node,
    /// Lexical and syntax diagnostics in source order of discovery.
    pub diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// True if there is at least one error.
    pub fn has_errors(&self) -> bool {
        tcad_diag::has_errors(&self.diagnostics)
    }
}

/// Parses a source text.
pub fn parse(src: &str) -> Parse {
    let (tokens, mut diags) = lex(src);
    let mut p = Parser {
        ctx: Ctx::Part,
        toks: tokens,
        pos: 0,
        depth: 0,
        prev_end: 0,
        stack: vec![Frame {
            kind: NodeKind::File,
            children: Vec::new(),
        }],
        diags: Vec::new(),
    };
    p.file();
    diags.append(&mut p.diags);
    diags.sort_by_key(|d| (d.span.start, d.span.end));
    let frame = p.stack.pop().expect("root frame");
    Parse {
        root: Node {
            kind: frame.kind,
            children: frame.children,
        },
        diagnostics: diags,
    }
}

struct Frame {
    kind: NodeKind,
    children: Vec<Child>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Part,
    Sketch,
    Config,
}

type R = Result<(), ()>;

struct Parser {
    /// Context of the block being parsed; decides which words may not be declared as names.
    ctx: Ctx,
    toks: Vec<Token>,
    pos: usize,
    /// Nesting depth of `(` and `[`; line breaks are insignificant when positive.
    depth: u32,
    prev_end: usize,
    stack: Vec<Frame>,
    diags: Vec<Diagnostic>,
}

impl Parser {
    // ---- tree building ----

    fn start(&mut self, kind: NodeKind) {
        self.stack.push(Frame {
            kind,
            children: Vec::new(),
        });
    }

    fn finish(&mut self) {
        let f = self.stack.pop().expect("frame");
        let node = Node {
            kind: f.kind,
            children: f.children,
        };
        self.stack
            .last_mut()
            .expect("parent frame")
            .children
            .push(Child::Node(node));
    }

    fn cp(&self) -> usize {
        self.stack.last().expect("frame").children.len()
    }

    /// Starts a node that adopts the children added since `cp`.
    fn start_at(&mut self, cp: usize, kind: NodeKind) {
        let top = self.stack.last_mut().expect("frame");
        let adopted = top.children.split_off(cp);
        self.stack.push(Frame {
            kind,
            children: adopted,
        });
    }

    // ---- token access ----

    fn look_idx(&self, n: usize) -> usize {
        let mut i = self.pos;
        let mut left = n;
        loop {
            if self.depth > 0 {
                while self.toks[i].kind == K::Newline {
                    i += 1;
                }
            }
            if left == 0 || self.toks[i].kind == K::Eof {
                return i;
            }
            i += 1;
            left -= 1;
        }
    }

    fn look(&self, n: usize) -> &Token {
        &self.toks[self.look_idx(n)]
    }

    fn peek(&self) -> &Token {
        self.look(0)
    }

    fn kind(&self) -> K {
        self.peek().kind
    }

    fn is_word(&self, w: &str) -> bool {
        let t = self.peek();
        t.kind == K::Ident && t.text == w
    }

    /// Is the token `n` ahead directly attached (no trivia) to the one before it?
    fn attached(&self, n: usize) -> bool {
        let i = self.look_idx(n);
        if n == 0 {
            self.toks[i].span.start == self.prev_end && self.toks[i].leading.is_empty()
        } else {
            let prev = self.look_idx(n - 1);
            self.toks[i].span.start == self.toks[prev].span.end
        }
    }

    fn bump_raw(&mut self) {
        let t = self.toks[self.pos].clone();
        if t.kind != K::Eof {
            self.pos += 1;
        }
        self.prev_end = t.span.end;
        self.stack
            .last_mut()
            .expect("frame")
            .children
            .push(Child::Token(t));
    }

    /// Consumes the next significant token (first absorbing insignificant line breaks).
    fn bump(&mut self) {
        if self.depth > 0 {
            while self.toks[self.pos].kind == K::Newline {
                self.bump_raw();
            }
        }
        self.bump_raw();
    }

    fn bump_open(&mut self) {
        self.bump();
        self.depth += 1;
    }

    fn bump_close(&mut self) {
        self.bump();
        self.depth = self.depth.saturating_sub(1);
    }

    // ---- diagnostics ----

    fn describe(t: &Token) -> String {
        match t.kind {
            K::Newline => "end of line".into(),
            K::Eof => "end of file".into(),
            _ => format!("`{}`", t.text),
        }
    }

    fn here(&self) -> Span {
        let t = self.peek();
        if t.kind == K::Eof || t.kind == K::Newline {
            Span::new(t.span.start, t.span.start)
        } else {
            t.span
        }
    }

    fn error_expected(&mut self, what: &str, insert: Option<&str>) -> R {
        let t = self.peek().clone();
        let span = self.here();
        let mut d = Diagnostic::error(
            codes::E1102,
            span,
            format!("expected {what}, found {}", Self::describe(&t)),
        );
        if let Some(text) = insert {
            d = d.with_hint(FixHint {
                message: format!("insert `{text}`"),
                edits: vec![Edit {
                    span: Span::new(span.start, span.start),
                    replacement: text.to_string(),
                }],
            });
        }
        self.diags.push(d);
        Err(())
    }

    fn expect(&mut self, kind: K, what: &str, insert: Option<&str>) -> R {
        if self.kind() == kind {
            match kind {
                K::LParen | K::LBracket => self.bump_open(),
                K::RParen | K::RBracket => self.bump_close(),
                _ => self.bump(),
            }
            Ok(())
        } else {
            self.error_expected(what, insert)
        }
    }

    fn expect_word(&mut self, w: &str) -> R {
        if self.is_word(w) {
            self.bump();
            Ok(())
        } else {
            self.error_expected(&format!("`{w}`"), None)
        }
    }

    /// Consumes an identifier that declares a name.
    fn declared_name(&mut self) -> R {
        if self.kind() != K::Ident {
            return self.error_expected("a name", None);
        }
        let t = self.peek().clone();
        if kw::is_keyword(&t.text, self.ctx == Ctx::Sketch) {
            self.diags.push(Diagnostic::error(
                codes::E1104,
                t.span,
                format!(
                    "`{}` is a reserved word and cannot be used as a name",
                    t.text
                ),
            ));
        }
        self.bump();
        Ok(())
    }

    // ---- recovery ----

    /// Wraps the rest of the statement in an error node. Stops before a line break, an unmatched
    /// `}` or the end of the file.
    fn recover(&mut self) {
        self.depth = 0;
        self.start(NodeKind::Error);
        let mut braces = 0u32;
        loop {
            match self.kind() {
                K::Eof => break,
                K::Newline if braces == 0 => break,
                K::RBrace if braces == 0 => break,
                K::LBrace => braces += 1,
                K::RBrace => braces -= 1,
                _ => {}
            }
            self.bump_raw();
        }
        self.finish();
    }

    /// Consumes the statement terminator: `;`, a line break, or nothing before `}` / end of file.
    fn end_stmt(&mut self) {
        self.depth = 0;
        match self.kind() {
            K::Semi => {
                self.bump();
                if self.kind() == K::Newline {
                    self.bump();
                }
            }
            K::Newline => self.bump(),
            K::RBrace | K::Eof => {}
            _ => {
                let t = self.peek().clone();
                self.diags.push(Diagnostic::error(
                    codes::E1203,
                    t.span,
                    format!("expected end of statement, found {}", Self::describe(&t)),
                ));
                self.recover();
                if self.kind() == K::Newline {
                    self.bump();
                }
            }
        }
    }

    /// Parses one statement into a node of `kind`, recovering from errors.
    fn statement(&mut self, kind: NodeKind, body: impl FnOnce(&mut Self) -> R) {
        let base = self.stack.len();
        self.start(kind);
        if body(self).is_err() {
            while self.stack.len() > base + 1 {
                self.finish();
            }
            self.recover();
        }
        self.end_stmt();
        self.finish();
    }

    // ---- file structure ----

    fn file(&mut self) {
        self.header();
        loop {
            match self.kind() {
                K::Eof => break,
                K::Newline | K::Semi => self.bump_raw(),
                K::Ident if self.peek().text == "part" => {
                    self.statement(NodeKind::Part, |p| p.part_body())
                }
                K::Ident if kw::is_reserved(&self.peek().text) => self.reserved(),
                K::RBrace => {
                    let span = self.peek().span;
                    self.diags
                        .push(Diagnostic::error(codes::E1103, span, "unmatched `}`"));
                    self.start(NodeKind::Error);
                    self.bump_raw();
                    self.finish();
                }
                _ => {
                    let t = self.peek().clone();
                    self.diags.push(Diagnostic::error(
                        codes::E1201,
                        t.span,
                        format!(
                            "expected `part` at the top level, found {}",
                            Self::describe(&t)
                        ),
                    ));
                    self.start(NodeKind::Error);
                    self.recover();
                    self.finish();
                    if self.kind() == K::Newline {
                        self.bump_raw();
                    }
                }
            }
        }
        self.bump_raw(); // Eof
    }

    fn header(&mut self) {
        self.start(NodeKind::Header);
        // Comment-only lines may precede the header.
        while self.kind() == K::Newline {
            self.bump_raw();
        }
        let ok = self.is_word("tcad");
        if ok {
            self.bump_raw();
        }
        let version = self.peek().clone();
        let parsed = (version.kind == K::Number)
            .then(|| version.text.split_once('.'))
            .flatten()
            .and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?)));
        if let (true, Some((major, minor))) = (ok, parsed) {
            self.bump_raw();
            if major != SUPPORTED_VERSION.0 {
                self.diags.push(Diagnostic::error(
                    codes::E0101,
                    version.span,
                    format!(
                        "unsupported format version {major}.{minor}; this tool reads {}.x",
                        SUPPORTED_VERSION.0
                    ),
                ));
            } else if minor > SUPPORTED_VERSION.1 {
                self.diags.push(Diagnostic::warning(
                    codes::W0102,
                    version.span,
                    format!(
                        "format version {major}.{minor} is newer than the supported {}.{}",
                        SUPPORTED_VERSION.0, SUPPORTED_VERSION.1
                    ),
                ));
            }
        } else {
            let span = if ok { version.span } else { self.here() };
            let mut d = Diagnostic::error(
                codes::E1101,
                span,
                "the file must start with a header like `tcad 0.1`",
            );
            if !ok {
                d = d.with_hint(FixHint {
                    message: "insert the header".into(),
                    edits: vec![Edit {
                        span: Span::new(span.start, span.start),
                        replacement: format!(
                            "tcad {}.{}\n\n",
                            SUPPORTED_VERSION.0, SUPPORTED_VERSION.1
                        ),
                    }],
                });
            }
            self.diags.push(d);
            if ok && self.kind() != K::Newline && self.kind() != K::Eof {
                self.recover();
            }
        }
        if self.kind() == K::Newline {
            self.bump_raw();
        } else if self.kind() != K::Eof {
            self.recover();
            if self.kind() == K::Newline {
                self.bump_raw();
            }
        }
        self.finish();
    }

    fn reserved(&mut self) {
        let t = self.peek().clone();
        self.diags.push(Diagnostic::error(
            codes::E0001,
            t.span,
            format!(
                "feature not yet supported: `{}` is reserved for a later phase",
                t.text
            ),
        ));
        self.statement(NodeKind::Reserved, |p| {
            let mut braces = 0u32;
            loop {
                match p.kind() {
                    K::Eof => break,
                    K::Newline if braces == 0 => break,
                    K::RBrace if braces == 0 => break,
                    K::LBrace => braces += 1,
                    K::RBrace => braces -= 1,
                    _ => {}
                }
                p.bump_raw();
            }
            Ok(())
        });
    }

    fn part_body(&mut self) -> R {
        self.bump(); // part
        self.declared_name()?;
        if self.kind() == K::String {
            self.bump();
        }
        self.block(Ctx::Part)
    }

    fn block(&mut self, ctx: Ctx) -> R {
        if self.kind() != K::LBrace {
            return self.error_expected("`{`", Some("{"));
        }
        let open = self.peek().span;
        let outer = std::mem::replace(&mut self.ctx, ctx);
        self.start(NodeKind::Block);
        self.bump_raw();
        loop {
            match self.kind() {
                K::Newline | K::Semi => self.bump_raw(),
                K::RBrace => break,
                K::Eof => {
                    self.diags.push(Diagnostic::error(
                        codes::E1205,
                        open,
                        "unclosed block: missing `}`",
                    ));
                    self.finish();
                    self.ctx = outer;
                    return Ok(());
                }
                _ => self.block_item(ctx),
            }
        }
        self.bump_raw(); // }
        self.finish();
        self.ctx = outer;
        Ok(())
    }

    fn block_item(&mut self, ctx: Ctx) {
        let t = self.peek().clone();
        if t.kind == K::Ident && kw::is_reserved(&t.text) && ctx != Ctx::Config {
            self.reserved();
            return;
        }
        let word = if t.kind == K::Ident {
            t.text.as_str()
        } else {
            ""
        };
        match ctx {
            Ctx::Part => match word {
                "tolerance" => self.statement(NodeKind::Tolerance, |p| {
                    p.bump();
                    p.expr()
                }),
                "material" => self.statement(NodeKind::Material, |p| p.material()),
                "param" => self.statement(NodeKind::Param, |p| p.param()),
                "import" => self.statement(NodeKind::Import, |p| p.import()),
                "tag" => self.statement(NodeKind::Tag, |p| p.tag()),
                "config" => self.statement(NodeKind::Config, |p| p.config()),
                "sketch" => self.statement(NodeKind::Sketch, |p| p.sketch()),
                w if kw::REF_GEOM_KW.contains(&w) => {
                    self.statement(NodeKind::RefGeom, |p| p.args_stmt(true))
                }
                w if kw::FEATURE_KW.contains(&w) => {
                    self.statement(NodeKind::Feature, |p| p.args_stmt(true))
                }
                _ => self.unknown_statement("a part statement"),
            },
            Ctx::Sketch => match word {
                "constrain" => self.statement(NodeKind::Constraint, |p| p.constraint()),
                w if kw::ENTITY_KW.contains(&w) => {
                    self.statement(NodeKind::Entity, |p| p.args_stmt(false))
                }
                _ => self.unknown_statement("a sketch entity or `constrain`"),
            },
            Ctx::Config => match word {
                "suppress" => self.statement(NodeKind::ConfigSuppress, |p| {
                    p.bump();
                    p.reference()?;
                    while p.kind() == K::Comma {
                        p.bump();
                        p.reference()?;
                    }
                    Ok(())
                }),
                _ if t.kind == K::Ident => self.statement(NodeKind::ConfigOverride, |p| {
                    p.bump();
                    p.expect(K::Eq, "`=`", Some(" = "))?;
                    p.expr()
                }),
                _ => self.unknown_statement("a parameter override or `suppress`"),
            },
        }
    }

    fn unknown_statement(&mut self, expected: &str) {
        let t = self.peek().clone();
        self.diags.push(Diagnostic::error(
            codes::E1201,
            t.span,
            format!("expected {expected}, found {}", Self::describe(&t)),
        ));
        self.recover();
        if self.kind() == K::Newline {
            self.bump_raw();
        }
    }

    // ---- statements ----

    fn material(&mut self) -> R {
        self.bump();
        if self.kind() != K::String {
            return self.error_expected("a material name string", None);
        }
        self.bump();
        self.args(false)
    }

    fn param(&mut self) -> R {
        self.bump();
        self.declared_name()?;
        self.expect(K::Eq, "`=`", Some(" = "))?;
        self.expr()?;
        if self.is_word("range") {
            self.start(NodeKind::RangeClause);
            self.bump();
            self.expect(K::LParen, "`(`", Some("("))?;
            self.expr()?;
            self.expect(K::Comma, "`,`", Some(", "))?;
            self.expr()?;
            self.expect(K::RParen, "`)`", Some(")"))?;
            self.finish();
        }
        if self.is_word("doc") {
            self.start(NodeKind::DocClause);
            self.bump();
            if self.kind() != K::String {
                return self.error_expected("a doc string", None);
            }
            self.bump();
            self.finish();
        }
        Ok(())
    }

    fn import(&mut self) -> R {
        self.bump();
        self.declared_name()?;
        self.expect(K::Eq, "`=`", Some(" = "))?;
        self.expect_word("step")?;
        self.expect(K::LParen, "`(`", Some("("))?;
        if self.kind() != K::String {
            return self.error_expected("a file path string", None);
        }
        self.bump();
        self.expect(K::RParen, "`)`", Some(")"))
    }

    fn tag(&mut self) -> R {
        self.bump();
        self.declared_name()?;
        self.expect(K::Eq, "`=`", Some(" = "))?;
        self.reference()
    }

    fn config(&mut self) -> R {
        self.bump();
        if self.is_word("default") && self.look(1).kind == K::Ident {
            self.bump();
        }
        self.declared_name()?;
        self.block(Ctx::Config)
    }

    fn sketch(&mut self) -> R {
        self.bump();
        self.declared_name()?;
        self.expect_word("on")?;
        self.reference()?;
        self.block(Ctx::Sketch)
    }

    fn constraint(&mut self) -> R {
        self.bump();
        if self.kind() != K::Ident {
            return self.error_expected("a constraint name", None);
        }
        self.bump();
        self.expect(K::LParen, "`(`", Some("("))?;
        if self.kind() != K::RParen {
            self.expr()?;
            while self.kind() == K::Comma {
                self.bump();
                self.expr()?;
            }
        }
        self.expect(K::RParen, "`)`", Some(")"))?;
        if self.kind() == K::Eq {
            self.bump();
            self.expr()?;
        }
        Ok(())
    }

    /// `keyword name args...` for reference geometry, features and sketch entities.
    fn args_stmt(&mut self, features: bool) -> R {
        self.bump();
        self.declared_name()?;
        self.args(features)
    }

    fn args(&mut self, features: bool) -> R {
        loop {
            match self.kind() {
                K::Newline | K::Semi | K::RBrace | K::Eof => return Ok(()),
                K::At if !features => self.hint()?,
                K::Ident => {
                    let w = self.peek().text.clone();
                    if w == "from" && features {
                        self.parse_from()?;
                    } else if w == "suppress" && features {
                        self.start(NodeKind::SuppressClause);
                        self.bump();
                        self.expect_word("if")?;
                        self.expr()?;
                        self.finish();
                    } else if self.look(1).kind == K::Eq {
                        self.named_arg(true)?;
                    } else {
                        self.flag();
                    }
                }
                _ => {
                    let t = self.peek().clone();
                    self.diags.push(Diagnostic::error(
                        codes::E1103,
                        t.span,
                        format!("unexpected {} in argument list", Self::describe(&t)),
                    ));
                    return Err(());
                }
            }
        }
    }

    fn flag(&mut self) {
        self.start(NodeKind::Flag);
        self.bump();
        self.finish();
    }

    /// `key=value`. In a statement's argument list `value` may be a comma-separated list; inside
    /// the parentheses of a call the comma separates arguments, so only one value is read.
    fn named_arg(&mut self, multi: bool) -> R {
        self.start(NodeKind::NamedArg);
        self.bump(); // key
        self.bump(); // =
        self.start(NodeKind::ValueList);
        self.expr()?;
        while multi && self.kind() == K::Comma {
            self.bump();
            self.expr()?;
        }
        self.finish();
        self.finish();
        Ok(())
    }

    fn hint(&mut self) -> R {
        self.start(NodeKind::Hint);
        self.bump(); // @
        if !(self.is_word("hint") && self.attached(0)) {
            return self.error_expected("`hint` after `@`", None);
        }
        self.bump();
        self.expect(K::LParen, "`(`", Some("("))?;
        if self.kind() != K::RParen {
            self.expr()?;
            while self.kind() == K::Comma {
                self.bump();
                self.expr()?;
            }
        }
        self.expect(K::RParen, "`)`", Some(")"))?;
        self.finish();
        Ok(())
    }

    fn parse_from(&mut self) -> R {
        self.start(NodeKind::FromClause);
        self.bump(); // from
        let t = self.peek().clone();
        if t.kind == K::Ident
            && kw::ENTITY_KW.contains(&t.text.as_str())
            && self.look(1).kind == K::LParen
        {
            self.start(NodeKind::ProfileCall);
            self.bump();
            self.bump_open();
            if self.kind() != K::RParen {
                self.profile_arg()?;
                while self.kind() == K::Comma {
                    self.bump();
                    self.profile_arg()?;
                }
            }
            self.expect(K::RParen, "`)`", Some(")"))?;
            self.finish();
        } else {
            self.reference()?;
        }
        self.finish();
        Ok(())
    }

    fn profile_arg(&mut self) -> R {
        if self.kind() != K::Ident {
            return self.error_expected("an argument", None);
        }
        if self.look(1).kind == K::Eq {
            self.named_arg(false)
        } else {
            self.flag();
            Ok(())
        }
    }

    // ---- references and expressions ----

    fn reference(&mut self) -> R {
        self.start(NodeKind::Reference);
        self.path_segment(true)?;
        while self.kind() == K::Dot {
            self.bump();
            self.path_segment(false)?;
        }
        if self.kind() == K::Hash && self.attached(0) {
            self.bump();
            if self.kind() != K::Number {
                return self.error_expected("an ordinal number after `#`", None);
            }
            self.bump();
        }
        self.finish();
        Ok(())
    }

    fn path_segment(&mut self, head: bool) -> R {
        let t = self.peek().clone();
        match t.kind {
            K::Ident => {}
            K::Upper if !head => {
                if !kw::is_axis_name(&t.text) {
                    self.diags.push(Diagnostic::error(
                        codes::E1202,
                        t.span,
                        format!(
                            "unknown axis or plane name `{}` (expected X, Y, Z, XY, YZ or XZ)",
                            t.text
                        ),
                    ));
                }
            }
            _ => return self.error_expected("a name", None),
        }
        self.start(NodeKind::PathSegment);
        self.bump();
        if self.kind() == K::LParen && self.attached(0) {
            self.bump_open();
            if self.kind() != K::RParen {
                self.gen_arg()?;
                while matches!(self.kind(), K::Comma | K::Amp) {
                    self.bump();
                    self.gen_arg()?;
                }
            }
            self.expect(K::RParen, "`)`", Some(")"))?;
        }
        self.finish();
        Ok(())
    }

    fn at_direction(&self) -> bool {
        matches!(self.kind(), K::Plus | K::Minus)
            && self.look(1).kind == K::Upper
            && self.attached(1)
    }

    fn direction(&mut self) -> R {
        let axis = self.look(1).clone();
        if !kw::is_axis_name(&axis.text) {
            self.diags.push(Diagnostic::error(
                codes::E1202,
                axis.span,
                format!("unknown axis name `{}` (expected X, Y or Z)", axis.text),
            ));
        }
        self.start(NodeKind::Direction);
        self.bump();
        self.bump();
        self.finish();
        Ok(())
    }

    fn gen_arg(&mut self) -> R {
        if self.at_direction() {
            self.direction()
        } else {
            self.reference()
        }
    }

    fn expr(&mut self) -> R {
        self.or_expr()
    }

    fn binary_loop(&mut self, next: fn(&mut Self) -> R, is_op: fn(&Self) -> bool) -> R {
        let cp = self.cp();
        next(self)?;
        while is_op(self) {
            self.start_at(cp, NodeKind::Binary);
            self.bump();
            next(self)?;
            self.finish();
        }
        Ok(())
    }

    fn or_expr(&mut self) -> R {
        self.binary_loop(Self::and_expr, |p| p.is_word("or"))
    }

    fn and_expr(&mut self) -> R {
        self.binary_loop(Self::not_expr, |p| p.is_word("and"))
    }

    fn not_expr(&mut self) -> R {
        if self.is_word("not") {
            self.start(NodeKind::Unary);
            self.bump();
            self.not_expr()?;
            self.finish();
            Ok(())
        } else {
            self.cmp_expr()
        }
    }

    fn cmp_expr(&mut self) -> R {
        let cp = self.cp();
        self.add_expr()?;
        if matches!(
            self.kind(),
            K::EqEq | K::NotEq | K::Lt | K::Le | K::Gt | K::Ge
        ) {
            self.start_at(cp, NodeKind::Binary);
            self.bump();
            self.add_expr()?;
            self.finish();
        }
        Ok(())
    }

    fn add_expr(&mut self) -> R {
        self.binary_loop(Self::mul_expr, |p| matches!(p.kind(), K::Plus | K::Minus))
    }

    fn mul_expr(&mut self) -> R {
        self.binary_loop(Self::unary_expr, |p| matches!(p.kind(), K::Star | K::Slash))
    }

    fn unary_expr(&mut self) -> R {
        if self.at_direction() {
            return self.direction();
        }
        if self.kind() == K::Minus {
            self.start(NodeKind::Unary);
            self.bump();
            self.unary_expr()?;
            self.finish();
            return Ok(());
        }
        self.pow_expr()
    }

    fn pow_expr(&mut self) -> R {
        let cp = self.cp();
        self.primary()?;
        if self.kind() == K::Caret {
            self.start_at(cp, NodeKind::Binary);
            self.bump();
            self.unary_expr()?;
            self.finish();
        }
        Ok(())
    }

    fn primary(&mut self) -> R {
        let t = self.peek().clone();
        match t.kind {
            K::Number => {
                let unit_follows = self.look(1).kind == K::Ident && kw::is_unit(&self.look(1).text);
                if unit_follows {
                    self.start(NodeKind::Quantity);
                    self.bump();
                    self.unit();
                } else {
                    self.start(NodeKind::Literal);
                    self.bump();
                }
                self.finish();
                Ok(())
            }
            K::String => {
                self.start(NodeKind::Literal);
                self.bump();
                self.finish();
                Ok(())
            }
            K::Ident => {
                let next_is_paren = self.look(1).kind == K::LParen && self.attached(1);
                if t.text == "true" || t.text == "false" {
                    self.start(NodeKind::Literal);
                    self.bump();
                    self.finish();
                    Ok(())
                } else if t.text == "query" && next_is_paren {
                    self.query()
                } else if kw::is_func(&t.text) && next_is_paren {
                    self.start(NodeKind::Call);
                    self.bump();
                    self.bump_open();
                    if self.kind() != K::RParen {
                        self.expr()?;
                        while self.kind() == K::Comma {
                            self.bump();
                            self.expr()?;
                        }
                    }
                    self.expect(K::RParen, "`)`", Some(")"))?;
                    self.finish();
                    Ok(())
                } else if matches!(
                    t.text.as_str(),
                    "from" | "suppress" | "if" | "on" | "and" | "or" | "not" | "range" | "doc"
                ) {
                    self.expr_expected()
                } else {
                    self.reference()
                }
            }
            K::LParen => {
                let cp = self.cp();
                self.bump_open();
                self.expr()?;
                if self.kind() == K::Comma {
                    self.start_at(cp, NodeKind::Vector);
                    while self.kind() == K::Comma {
                        self.bump();
                        self.expr()?;
                    }
                } else {
                    self.start_at(cp, NodeKind::Paren);
                }
                self.expect(K::RParen, "`)`", Some(")"))?;
                self.finish();
                Ok(())
            }
            K::LBracket => {
                self.start(NodeKind::List);
                self.bump_open();
                if self.kind() != K::RBracket {
                    self.expr()?;
                    while self.kind() == K::Comma {
                        self.bump();
                        self.expr()?;
                    }
                }
                self.expect(K::RBracket, "`]`", Some("]"))?;
                self.finish();
                Ok(())
            }
            _ => self.expr_expected(),
        }
    }

    fn expr_expected(&mut self) -> R {
        let t = self.peek().clone();
        self.diags.push(Diagnostic::error(
            codes::E1204,
            self.here(),
            format!("expected an expression, found {}", Self::describe(&t)),
        ));
        Err(())
    }

    fn unit(&mut self) {
        self.start(NodeKind::Unit);
        self.unit_term();
        loop {
            let op = self.look(0);
            let next = self.look(1);
            let continues = matches!(op.kind, K::Slash | K::Star)
                && self.attached(0)
                && next.kind == K::Ident
                && kw::is_unit(&next.text)
                && self.attached(1);
            if !continues {
                break;
            }
            self.bump();
            self.unit_term();
        }
        self.finish();
    }

    fn unit_term(&mut self) {
        self.bump(); // unit name
        if self.kind() == K::Caret
            && self.attached(0)
            && self.look(1).kind == K::Number
            && self.attached(1)
            && self.look(1).text.bytes().all(|b| b.is_ascii_digit())
        {
            self.bump();
            self.bump();
        }
    }

    fn query(&mut self) -> R {
        self.start(NodeKind::Query);
        self.bump(); // query
        self.bump_open();
        if self.kind() == K::Ident
            && matches!(self.peek().text.as_str(), "faces" | "edges" | "vertices")
        {
            self.bump();
        } else {
            return self.error_expected("`faces`, `edges` or `vertices`", None);
        }
        while self.kind() == K::Comma {
            self.bump();
            self.profile_arg()?;
        }
        self.expect(K::RParen, "`)`", Some(")"))?;
        self.finish();
        Ok(())
    }
}
