//! The canonical formatter (`spec/canonical-form.md`).
//!
//! One layout per tree: 2-space indentation, one statement per line, arguments in spec order,
//! normalized numbers and single spaces. Comments are preserved. Blank lines are kept (at most one
//! in a row). Formatting is idempotent: `format(format(x)) == format(x)`.

use crate::cst::{Child, Node, NodeKind};
use crate::number::normalize_number;
use crate::parser::parse;
use crate::token::{Token, TokenKind as K, TriviaKind};
use tcad_diag::Diagnostic;

/// Maximum width of a config block kept on a single line.
const ONE_LINE_WIDTH: usize = 100;

/// Formatting failed because the source has syntax errors.
#[derive(Debug)]
pub struct FormatError {
    /// The errors (and warnings) found while parsing.
    pub diagnostics: Vec<Diagnostic>,
}

/// Formats a source text. Fails if the source has syntax errors.
pub fn format_source(src: &str) -> Result<String, FormatError> {
    let parsed = parse(src);
    if parsed.has_errors() {
        return Err(FormatError {
            diagnostics: parsed.diagnostics,
        });
    }
    Ok(format_tree(&parsed.root))
}

/// Formats an error-free tree.
pub fn format_tree(root: &Node) -> String {
    let mut p = Printer { out: String::new() };
    p.file(root);
    p.out
}

/// Order of known named arguments per statement keyword (provisional until the per-feature
/// chapters of the spec are written). Unknown keys follow alphabetically.
fn arg_order(head: &str, key: &str) -> Option<usize> {
    let order: &[&str] = match head {
        "material" => &["density"],
        "point" => &["at"],
        "line" => &["start", "end"],
        "arc" => &["center", "start", "end", "through", "tangent_to", "r"],
        "circle" => &["center", "d", "r"],
        "ellipse" => &["center", "a", "b", "angle"],
        "spline" => &["points", "through", "closed"],
        "slot" => &["start", "end", "center", "w", "length"],
        "rect" => &["on", "corner", "center", "w", "h"],
        "polygon" => &["center", "sides", "d", "r"],
        "project" => &["edges"],
        "plane" => &["from", "offset", "angle", "about", "through"],
        "axis" => &["through", "from", "direction"],
        "csys" => &["origin", "x", "y"],
        "extrude" => &["distance", "depth", "to", "draft", "op"],
        "revolve" => &["axis", "angle", "op"],
        "sweep" => &["path", "guide", "op"],
        "loft" => &["sections", "guides", "op"],
        "hole" => &["on", "at", "type", "d", "thread", "depth", "end"],
        "fillet" => &["edges", "r"],
        "chamfer" => &["edges", "d", "angle"],
        "shell" => &["faces", "thickness"],
        "draft" => &["faces", "angle", "pull", "neutral"],
        "rib" => &["profile", "thickness", "direction"],
        "linear_pattern" => &["of", "dir", "count", "spacing", "skip"],
        "circular_pattern" => &["of", "axis", "count", "angle", "skip"],
        "mirror" => &["of", "plane"],
        "query" => &[
            "type",
            "normal",
            "parallel_to",
            "axis",
            "radius",
            "created_by",
            "adjacent_to",
            "nearest",
        ],
        _ => &[],
    };
    order.iter().position(|k| *k == key)
}

/// Text under construction. Comments on insignificant line breaks wait in `pending` and are
/// written before the next token.
struct Cx {
    buf: String,
    /// Indentation (in spaces) of continuation lines after a line comment inside brackets.
    cont: usize,
    pending: Vec<(bool, String)>,
}

impl Cx {
    fn new(indent: usize) -> Self {
        Cx {
            buf: String::new(),
            cont: (indent + 2) * 2,
            pending: Vec::new(),
        }
    }

    fn absorb(&mut self, t: &Token) {
        for c in t.comments() {
            let is_line = c.kind == TriviaKind::LineComment;
            let text = if is_line {
                c.text.trim_end().to_string()
            } else {
                c.text.clone()
            };
            self.pending.push((is_line, text));
        }
    }

    fn flush(&mut self) {
        for (is_line, text) in std::mem::take(&mut self.pending) {
            self.buf.push_str(&text);
            if is_line {
                self.buf.push('\n');
                self.buf.push_str(&" ".repeat(self.cont));
            } else {
                self.buf.push(' ');
            }
        }
    }

    fn tok(&mut self, t: &Token, text: &str) {
        self.absorb(t);
        self.flush();
        self.buf.push_str(text);
    }

    fn raw(&mut self, s: &str) {
        self.buf.push_str(s);
    }

    /// Children without line breaks; comments on those line breaks are kept for the next token.
    fn kids<'a>(&mut self, n: &'a Node) -> Vec<&'a Child> {
        let mut out = Vec::new();
        for c in &n.children {
            match c {
                Child::Token(t) if t.kind == K::Newline => self.absorb(t),
                _ => out.push(c),
            }
        }
        out
    }

    fn token(&mut self, t: &Token) {
        let text = token_text(t);
        self.tok(t, &text);
    }

    fn node(&mut self, n: &Node) {
        match n.kind {
            NodeKind::Binary => self.binary(n),
            NodeKind::Unary => self.unary(n),
            NodeKind::Quantity
            | NodeKind::FromClause
            | NodeKind::SuppressClause
            | NodeKind::DocClause => self.spaced(n),
            NodeKind::Query | NodeKind::ProfileCall => self.sorted_call(n),
            _ => self.delimited(n),
        }
    }

    fn child(&mut self, c: &Child) {
        match c {
            Child::Node(n) => self.node(n),
            Child::Token(t) => self.token(t),
        }
    }

    /// Children separated by single spaces.
    fn spaced(&mut self, n: &Node) {
        let kids = self.kids(n);
        for (i, c) in kids.into_iter().enumerate() {
            if i > 0 {
                self.raw(" ");
            }
            self.child(c);
        }
    }

    /// Children with no spaces except after `,` (and around `&`).
    fn delimited(&mut self, n: &Node) {
        let kids = self.kids(n);
        for c in kids {
            match c {
                Child::Token(t) if t.kind == K::Comma => {
                    self.tok(t, ",");
                    self.raw(" ");
                }
                Child::Token(t) if t.kind == K::Amp => {
                    self.raw(" ");
                    self.tok(t, "&");
                    self.raw(" ");
                }
                other => self.child(other),
            }
        }
    }

    fn binary(&mut self, n: &Node) {
        let kids = self.kids(n);
        for c in kids {
            match c {
                Child::Token(t) if t.text == "^" => self.tok(t, "^"),
                Child::Token(t) => {
                    self.raw(" ");
                    self.token(t);
                    self.raw(" ");
                }
                other => self.child(other),
            }
        }
    }

    fn unary(&mut self, n: &Node) {
        let kids = self.kids(n);
        for c in kids {
            match c {
                Child::Token(t) if t.kind == K::Ident => {
                    self.token(t);
                    self.raw(" ");
                }
                other => self.child(other),
            }
        }
    }

    /// `name(first, arg, arg...)` where the named arguments and flags are sorted by spec order.
    fn sorted_call(&mut self, n: &Node) {
        let head = match n.kind {
            NodeKind::Query => "query".to_string(),
            _ => n
                .tokens()
                .find(|t| t.kind == K::Ident)
                .map(|t| t.text.clone())
                .unwrap_or_default(),
        };
        let kids = self.kids(n);
        let mut args: Vec<&Node> = Vec::new();
        for c in kids {
            match c {
                Child::Node(a) if matches!(a.kind, NodeKind::NamedArg | NodeKind::Flag) => {
                    args.push(a)
                }
                Child::Token(t) if t.kind == K::Comma => {}
                Child::Token(t) if t.kind == K::RParen => {
                    sort_args(&head, &mut args);
                    // A query already printed its entity kind, so every argument needs a comma.
                    let mut sep = n.kind == NodeKind::Query;
                    for a in args.drain(..) {
                        if sep {
                            self.raw(", ");
                        }
                        sep = true;
                        self.node(a);
                    }
                    self.token(t);
                }
                other => self.child(other),
            }
        }
    }
}

fn token_text(t: &Token) -> String {
    match t.kind {
        K::Number => normalize_number(&t.text),
        _ => t.text.clone(),
    }
}

fn is_arg(n: &Node) -> bool {
    matches!(
        n.kind,
        NodeKind::NamedArg
            | NodeKind::Flag
            | NodeKind::FromClause
            | NodeKind::SuppressClause
            | NodeKind::Hint
    )
}

fn first_ident(n: &Node) -> &str {
    n.tokens()
        .find(|t| t.kind == K::Ident)
        .map_or("", |t| t.text.as_str())
}

/// Sort key: `from`, known named args in spec order, unknown named args alphabetically, flags
/// alphabetically, `construction`, `suppress if`, `@hint`. The sort is stable.
fn sort_args(head: &str, args: &mut [&Node]) {
    fn key<'a>(head: &str, a: &'a Node) -> (usize, usize, &'a str) {
        match a.kind {
            NodeKind::FromClause => (0, 0, ""),
            NodeKind::NamedArg => {
                let name = first_ident(a);
                match arg_order(head, name) {
                    Some(i) => (1, i, ""),
                    None => (2, 0, name),
                }
            }
            NodeKind::Flag => {
                let name = first_ident(a);
                if name == "construction" {
                    (4, 0, "")
                } else {
                    (3, 0, name)
                }
            }
            NodeKind::SuppressClause => (5, 0, ""),
            _ => (6, 0, ""),
        }
    }
    args.sort_by(|x, y| key(head, x).cmp(&key(head, y)));
}

struct Printer {
    out: String,
}

impl Printer {
    fn line(&mut self, indent: usize, text: &str) {
        self.out.push_str(&"  ".repeat(indent));
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn blank(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with("\n\n") {
            self.out.push('\n');
        }
    }

    fn file(&mut self, root: &Node) {
        let mut kids = root.children.iter();
        if let Some(Child::Node(h)) = kids.next() {
            self.header(h);
        }
        let rest: Vec<&Child> = kids.collect();
        self.items(&rest, 0, false, true, None);
    }

    fn header(&mut self, h: &Node) {
        let mut text = String::new();
        let mut trailing = String::new();
        for t in h.tokens() {
            match t.kind {
                K::Newline if text.is_empty() => {
                    let c = comments_joined(t);
                    if !c.is_empty() {
                        self.line(0, &c);
                    }
                }
                K::Newline => trailing = trailing_comments(std::slice::from_ref(t)),
                _ => {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    let c = comments_joined(t);
                    if !c.is_empty() {
                        text.push_str(&c);
                        text.push(' ');
                    }
                    text.push_str(&t.text);
                }
            }
        }
        self.line(0, &format!("{text}{trailing}"));
    }

    /// Prints a sequence of statements and standalone line breaks.
    fn items(
        &mut self,
        kids: &[&Child],
        indent: usize,
        mut first: bool,
        mut pending_blank: bool,
        closing: Option<&Token>,
    ) {
        for c in kids {
            match c {
                Child::Token(t) if matches!(t.kind, K::Newline | K::Semi | K::Eof) => {
                    let text = comments_joined(t);
                    if !text.is_empty() {
                        if pending_blank && !first {
                            self.blank();
                        }
                        self.line(indent, &text);
                        pending_blank = false;
                        first = false;
                    } else if t.kind == K::Newline {
                        pending_blank = true;
                    }
                }
                Child::Token(_) => {}
                Child::Node(n) => {
                    if pending_blank && !first {
                        self.blank();
                    }
                    self.item(n, indent);
                    first = false;
                    pending_blank = false;
                }
            }
        }
        if let Some(t) = closing {
            let text = comments_joined(t);
            if !text.is_empty() {
                if pending_blank && !first {
                    self.blank();
                }
                self.line(indent, &text);
            }
        }
    }

    fn item(&mut self, n: &Node, indent: usize) {
        if matches!(n.kind, NodeKind::Error | NodeKind::Reserved) {
            let src = n.trimmed_source();
            self.line(indent, src.trim_end());
            return;
        }
        let (body, term) = split_terminator(n);
        let trailing = trailing_comments(&term);
        let mut cx = Cx::new(indent);
        let mut block: Option<&Node> = None;
        head(n, &body, &mut cx, &mut block);
        match block {
            Some(b) => self.block(b, indent, cx.buf, &trailing, n.kind == NodeKind::Config),
            None => self.line(indent, &format!("{}{}", cx.buf, trailing)),
        }
    }

    fn block(
        &mut self,
        b: &Node,
        indent: usize,
        head_text: String,
        trailing: &str,
        is_config: bool,
    ) {
        let all: Vec<&Child> = b.children.iter().collect();
        let rest = match all.split_first() {
            Some((Child::Token(t), rest)) if t.kind == K::LBrace => rest,
            _ => &all[..],
        };
        let (close, inner) = match rest.split_last() {
            Some((Child::Token(t), inner)) if t.kind == K::RBrace => (Some(t), inner),
            _ => (None, rest),
        };
        let has_comments = b.descendant_tokens().iter().any(|t| t.has_comments());

        if is_config && !has_comments && trailing.is_empty() {
            let mut parts: Vec<String> = Vec::new();
            for c in inner {
                if let Child::Node(n) = c {
                    let mut cx = Cx::new(indent);
                    let (body, _) = split_terminator(n);
                    head(n, &body, &mut cx, &mut None);
                    parts.push(cx.buf);
                }
            }
            let line = if parts.is_empty() {
                format!("{head_text} {{ }}")
            } else {
                format!("{head_text} {{ {} }}", parts.join("; "))
            };
            if indent * 2 + line.len() <= ONE_LINE_WIDTH {
                self.line(indent, &line);
                return;
            }
        }

        let mut items: Vec<&Child> = inner.to_vec();
        let mut open_trailing = String::new();
        if let Some(Child::Token(t)) = items.first()
            && t.kind == K::Newline
        {
            open_trailing = trailing_comments(std::slice::from_ref(t));
            items.remove(0);
        }
        let empty = items.iter().all(|c| match c {
            Child::Token(t) => !t.has_comments(),
            Child::Node(_) => false,
        }) && close.is_none_or(|t| !t.has_comments());
        if empty {
            self.line(
                indent,
                &format!("{head_text} {{ }}{open_trailing}{trailing}"),
            );
            return;
        }
        self.line(indent, &format!("{head_text} {{{open_trailing}"));
        self.items(&items, indent + 1, true, false, close);
        self.line(indent, &format!("}}{trailing}"));
    }
}

/// Prints a statement up to (excluding) its block, if any.
fn head<'a>(n: &Node, body: &[&'a Child], cx: &mut Cx, block: &mut Option<&'a Node>) {
    let head_kw = first_ident(n);
    let mut args: Vec<&Node> = Vec::new();
    let mut first = true;
    let mut tight = false;
    for c in body {
        match c {
            Child::Node(b) if b.kind == NodeKind::Block => *block = Some(b),
            Child::Node(a) if is_arg(a) => args.push(a),
            Child::Token(t) if t.kind == K::Newline => cx.absorb(t),
            Child::Token(t) if t.kind == K::Eq => {
                cx.raw(" ");
                cx.tok(t, "=");
                cx.raw(" ");
                tight = true;
            }
            Child::Token(t) if t.kind == K::LParen => {
                cx.token(t);
                tight = true;
            }
            Child::Token(t) if t.kind == K::RParen => cx.token(t),
            Child::Token(t) if t.kind == K::Comma => {
                cx.tok(t, ",");
                cx.raw(" ");
                tight = true;
            }
            other => {
                if !first && !tight {
                    cx.raw(" ");
                }
                cx.child(other);
                tight = false;
            }
        }
        first = false;
    }
    sort_args(head_kw, &mut args);
    for a in args {
        cx.raw(" ");
        cx.node(a);
    }
}

fn split_terminator(n: &Node) -> (Vec<&Child>, Vec<Token>) {
    let mut end = n.children.len();
    while end > 0 {
        match &n.children[end - 1] {
            Child::Token(t) if matches!(t.kind, K::Semi | K::Newline) => end -= 1,
            _ => break,
        }
    }
    let body = n.children[..end].iter().collect();
    let term = n.children[end..]
        .iter()
        .filter_map(|c| match c {
            Child::Token(t) => Some(t.clone()),
            Child::Node(_) => None,
        })
        .collect();
    (body, term)
}

fn comment_texts(t: &Token) -> Vec<String> {
    t.comments()
        .map(|c| {
            if c.kind == TriviaKind::LineComment {
                c.text.trim_end().to_string()
            } else {
                c.text.clone()
            }
        })
        .collect()
}

fn comments_joined(t: &Token) -> String {
    comment_texts(t).join(" ")
}

/// Comments on a statement's terminator tokens, as a trailing `  # ...` suffix.
fn trailing_comments(term: &[Token]) -> String {
    let all: Vec<String> = term.iter().flat_map(comment_texts).collect();
    if all.is_empty() {
        String::new()
    } else {
        format!("  {}", all.join(" "))
    }
}
