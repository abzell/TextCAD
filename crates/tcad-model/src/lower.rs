//! Lowering of the concrete syntax tree to the IR. Expects a tree without syntax errors.

use crate::ir::{
    Config, Constraint, Entity, Item, ItemKind, Material, Override, Param, Part, Sketch,
};
use tcad_diag::{Diagnostic, Span, codes};
use tcad_expr::{
    Arg, BinOp, Dim, Direction, Expr, ExprKind, GenArg, Reference, Segment, UnOp, Unit, Value,
    base_unit,
};
use tcad_syntax::{Child, Node, NodeKind as N, Token, TokenKind as K};

/// Default modeling tolerance (requirements 4.4): 1e-6 mm.
pub const DEFAULT_TOLERANCE_MM: f64 = 1e-6;

/// The result of lowering a file.
pub struct Lowered {
    /// `(major, minor)` from the header.
    pub version: (u32, u32),
    /// All parts found (a valid file has exactly one).
    pub parts: Vec<Part>,
    /// The span of the first `tolerance` statement per part, to report duplicates.
    pub extra: Vec<Diagnostic>,
}

/// Lowers a parsed file.
pub fn lower_file(root: &Node) -> Lowered {
    let mut lw = Lowerer { diags: Vec::new() };
    let mut version = (0, 1);
    let mut parts = Vec::new();
    for n in root.nodes() {
        match n.kind {
            N::Header => {
                if let Some(t) = n.token_of(K::Number)
                    && let Some((a, b)) = t.text.split_once('.')
                {
                    version = (a.parse().unwrap_or(0), b.parse().unwrap_or(1));
                }
            }
            N::Part => parts.extend(lw.part(n)),
            _ => {}
        }
    }
    Lowered {
        version,
        parts,
        extra: lw.diags,
    }
}

struct Lowerer {
    diags: Vec<Diagnostic>,
}

fn sig(n: &Node) -> Vec<&Child> {
    n.children
        .iter()
        .filter(|c| !matches!(c, Child::Token(t) if matches!(t.kind, K::Newline | K::Semi)))
        .collect()
}

fn span_of(n: &Node) -> Span {
    n.span().unwrap_or_default()
}

fn ident_after(n: &Node, skip: usize) -> Option<&Token> {
    n.tokens().filter(|t| t.kind == K::Ident).nth(skip)
}

/// Decodes a string literal token (quotes and escapes).
pub fn unescape(text: &str) -> String {
    let inner = text.strip_prefix('"').unwrap_or(text);
    let inner = inner.strip_suffix('"').unwrap_or(inner);
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

impl Lowerer {
    fn part(&mut self, n: &Node) -> Option<Part> {
        let name_tok = ident_after(n, 1)?;
        let display_name = n.token_of(K::String).map(|t| unescape(&t.text));
        let block = n.node_of(N::Block)?;
        let mut part = Part {
            name: name_tok.text.clone(),
            display_name,
            name_span: name_tok.span,
            tolerance: DEFAULT_TOLERANCE_MM,
            material: None,
            params: Vec::new(),
            items: Vec::new(),
            configs: Vec::new(),
        };
        let mut seen_tolerance = false;
        for s in block.nodes() {
            match s.kind {
                N::Tolerance => {
                    let Some(e) = s.nodes().next().and_then(|e| self.expr(e)) else {
                        continue;
                    };
                    match tcad_expr::eval(&e, &tcad_expr::eval::EmptyEnv) {
                        Ok(Value::Num(q)) if q.dim == Dim::LENGTH && q.value > 0.0 => {
                            if seen_tolerance {
                                self.diags.push(Diagnostic::error(
                                    codes::E2303,
                                    span_of(s),
                                    "`tolerance` is declared more than once",
                                ));
                            } else {
                                part.tolerance = q.value;
                            }
                        }
                        Ok(_) | Err(_) => self.diags.push(Diagnostic::error(
                            codes::E2302,
                            e.span,
                            "the tolerance must be a positive length such as `1e-6 mm`",
                        )),
                    }
                    seen_tolerance = true;
                }
                N::Material => {
                    if part.material.is_some() {
                        self.diags.push(Diagnostic::error(
                            codes::E2303,
                            span_of(s),
                            "`material` is declared more than once",
                        ));
                        continue;
                    }
                    let name = s
                        .token_of(K::String)
                        .map(|t| unescape(&t.text))
                        .unwrap_or_default();
                    let args = self.args(s);
                    let mut density = None;
                    for a in &args {
                        if a.name != "density" {
                            self.diags.push(Diagnostic::error(
                                codes::E2305,
                                a.span,
                                format!(
                                    "unknown material argument `{}` (expected `density`)",
                                    a.name
                                ),
                            ));
                            continue;
                        }
                        let ok = a.values.first().and_then(|e| {
                            match tcad_expr::eval(e, &tcad_expr::eval::EmptyEnv) {
                                Ok(Value::Num(q)) if q.dim == Dim::DENSITY && q.value > 0.0 => {
                                    Some(q.value)
                                }
                                _ => None,
                            }
                        });
                        match ok {
                            Some(d) => density = Some(d),
                            None => self.diags.push(Diagnostic::error(
                                codes::E2305,
                                a.span,
                                "the density must be a positive density such as `7.85 g/cm^3`",
                            )),
                        }
                    }
                    part.material = Some(Material { name, density });
                }
                N::Param => part.params.extend(self.param(s)),
                N::Config => part.configs.extend(self.config(s)),
                N::Sketch => part.items.extend(self.sketch(s)),
                N::Tag => part.items.extend(self.tag(s)),
                N::Import => part.items.extend(self.import(s)),
                N::RefGeom => part.items.extend(self.feature(s, ItemKind::RefGeom)),
                N::Feature => part.items.extend(self.feature(s, ItemKind::Feature)),
                _ => {}
            }
        }
        Some(part)
    }

    fn param(&mut self, n: &Node) -> Option<Param> {
        let name = ident_after(n, 1)?;
        let mut expr = None;
        let mut range = None;
        let mut doc = None;
        for c in n.nodes() {
            match c.kind {
                N::RangeClause => {
                    let mut es = c.nodes().filter_map(|e| self.expr(e));
                    if let (Some(lo), Some(hi)) = (es.next(), es.next()) {
                        range = Some((lo, hi));
                    }
                }
                N::DocClause => doc = c.token_of(K::String).map(|t| unescape(&t.text)),
                _ if expr.is_none() => expr = self.expr(c),
                _ => {}
            }
        }
        Some(Param {
            name: name.text.clone(),
            expr: expr?,
            range,
            doc,
            value: Value::Bool(false),
            span: span_of(n),
            name_span: name.span,
        })
    }

    fn config(&mut self, n: &Node) -> Option<Config> {
        let is_default = ident_after(n, 1).is_some_and(|t| t.text == "default")
            && n.tokens().filter(|t| t.kind == K::Ident).count() >= 3;
        let name = n
            .tokens()
            .filter(|t| t.kind == K::Ident)
            .nth(if is_default { 2 } else { 1 })?;
        let block = n.node_of(N::Block)?;
        let mut overrides = Vec::new();
        let mut suppress = Vec::new();
        for s in block.nodes() {
            match s.kind {
                N::ConfigOverride => {
                    let param = ident_after(s, 0)?.text.clone();
                    let expr = s.nodes().next().and_then(|e| self.expr(e))?;
                    overrides.push(Override {
                        param,
                        expr,
                        span: span_of(s),
                    });
                }
                N::ConfigSuppress => {
                    suppress.extend(
                        s.nodes()
                            .filter(|r| r.kind == N::Reference)
                            .map(|r| self.reference(r)),
                    );
                }
                _ => {}
            }
        }
        Some(Config {
            name: name.text.clone(),
            is_default,
            overrides,
            suppress,
            span: span_of(n),
            name_span: name.span,
        })
    }

    fn tag(&mut self, n: &Node) -> Option<Item> {
        let name = ident_after(n, 1)?;
        let target = n.node_of(N::Reference).map(|r| self.reference(r))?;
        Some(Item {
            kind: ItemKind::Tag(target),
            keyword: "tag".into(),
            name: name.text.clone(),
            args: Vec::new(),
            suppress_if: None,
            span: span_of(n),
            name_span: name.span,
        })
    }

    fn import(&mut self, n: &Node) -> Option<Item> {
        let name = ident_after(n, 1)?;
        let path = n.token_of(K::String).map(|t| unescape(&t.text))?;
        Some(Item {
            kind: ItemKind::Import(path),
            keyword: "import".into(),
            name: name.text.clone(),
            args: Vec::new(),
            suppress_if: None,
            span: span_of(n),
            name_span: name.span,
        })
    }

    fn feature(&mut self, n: &Node, kind: ItemKind) -> Option<Item> {
        let keyword = ident_after(n, 0)?.text.clone();
        let name = ident_after(n, 1)?;
        let suppress_if = n
            .node_of(N::SuppressClause)
            .and_then(|s| s.nodes().next())
            .and_then(|e| self.expr(e));
        Some(Item {
            kind,
            keyword,
            name: name.text.clone(),
            args: self.args(n),
            suppress_if,
            span: span_of(n),
            name_span: name.span,
        })
    }

    fn sketch(&mut self, n: &Node) -> Option<Item> {
        let name = ident_after(n, 1)?;
        let plane = n.node_of(N::Reference).map(|r| self.reference(r))?;
        let block = n.node_of(N::Block)?;
        let mut entities = Vec::new();
        let mut constraints = Vec::new();
        for s in block.nodes() {
            match s.kind {
                N::Entity => {
                    let kind = ident_after(s, 0)?.text.clone();
                    let ename = ident_after(s, 1)?;
                    let mut args = self.args(s);
                    let construction = args.iter().any(|a| a.flag && a.name == "construction");
                    args.retain(|a| !(a.flag && a.name == "construction"));
                    let hint = s
                        .node_of(N::Hint)
                        .map(|h| h.nodes().filter_map(|e| self.expr(e)).collect());
                    let hint_span = s.node_of(N::Hint).map(span_of);
                    let content_end = s
                        .descendant_tokens()
                        .iter()
                        .rev()
                        .find(|t| !matches!(t.kind, K::Newline | K::Semi))
                        .map_or(0, |t| t.span.end);
                    entities.push(Entity {
                        kind,
                        name: ename.text.clone(),
                        args,
                        construction,
                        hint,
                        hint_span,
                        content_end,
                        span: span_of(s),
                        name_span: ename.span,
                    });
                }
                N::Constraint => {
                    let kind = ident_after(s, 1)?.text.clone();
                    let all: Vec<Expr> = s.nodes().filter_map(|e| self.expr(e)).collect();
                    let has_value = s.token_of(K::Eq).is_some();
                    let (args, value) = if has_value && !all.is_empty() {
                        let mut a = all;
                        let v = a.pop();
                        (a, v)
                    } else {
                        (all, None)
                    };
                    constraints.push(Constraint {
                        kind,
                        args,
                        value,
                        span: span_of(s),
                    });
                }
                _ => {}
            }
        }
        Some(Item {
            kind: ItemKind::Sketch(Sketch {
                plane,
                entities,
                constraints,
            }),
            keyword: "sketch".into(),
            name: name.text.clone(),
            args: Vec::new(),
            suppress_if: None,
            span: span_of(n),
            name_span: name.span,
        })
    }

    /// Named arguments, flags and the `from` clause of a statement.
    fn args(&mut self, n: &Node) -> Vec<Arg> {
        let mut out = Vec::new();
        for c in n.nodes() {
            match c.kind {
                N::NamedArg | N::Flag => out.extend(self.arg(c)),
                N::FromClause => {
                    let value = c.nodes().next().and_then(|v| match v.kind {
                        N::ProfileCall => self.profile(v),
                        _ => self.expr(v),
                    });
                    out.extend(value.map(|v| Arg {
                        name: "from".into(),
                        values: vec![v],
                        flag: false,
                        span: span_of(c),
                    }));
                }
                _ => {}
            }
        }
        out
    }

    fn arg(&mut self, c: &Node) -> Option<Arg> {
        let name = ident_after(c, 0)?.text.clone();
        if c.kind == N::Flag {
            return Some(Arg {
                name,
                values: Vec::new(),
                flag: true,
                span: span_of(c),
            });
        }
        let list = c.node_of(N::ValueList)?;
        let values = list.nodes().filter_map(|e| self.expr(e)).collect();
        Some(Arg {
            name,
            values,
            flag: false,
            span: span_of(c),
        })
    }

    fn profile(&mut self, n: &Node) -> Option<Expr> {
        let kind = ident_after(n, 0)?.text.clone();
        let args = n.nodes().filter_map(|a| self.arg(a)).collect();
        Some(Expr {
            kind: ExprKind::Profile(kind, args),
            span: span_of(n),
        })
    }

    fn reference(&mut self, n: &Node) -> Reference {
        let mut segments = Vec::new();
        let mut ordinal = None;
        for c in sig(n) {
            match c {
                Child::Node(seg) if seg.kind == N::PathSegment => segments.push(self.segment(seg)),
                Child::Token(t) if t.kind == K::Number => ordinal = t.text.parse().ok(),
                _ => {}
            }
        }
        Reference {
            segments,
            ordinal,
            span: span_of(n),
        }
    }

    fn segment(&mut self, n: &Node) -> Segment {
        let name = n
            .tokens()
            .find(|t| matches!(t.kind, K::Ident | K::Upper))
            .map(|t| t.text.clone())
            .unwrap_or_default();
        let args = n.token_of(K::LParen).map(|_| {
            n.nodes()
                .filter_map(|a| match a.kind {
                    N::Reference => Some(GenArg::Ref(self.reference(a))),
                    N::Direction => Some(GenArg::Dir(self.direction(a))),
                    _ => None,
                })
                .collect()
        });
        Segment {
            name,
            args,
            span: span_of(n),
        }
    }

    fn direction(&mut self, n: &Node) -> Direction {
        let negative = n.token_of(K::Minus).is_some();
        let axis = n
            .token_of(K::Upper)
            .map(|t| t.text.clone())
            .unwrap_or_default();
        Direction {
            negative,
            axis,
            span: span_of(n),
        }
    }

    fn unit(&mut self, n: &Node) -> Unit {
        let mut unit = Unit::NONE;
        let mut divide = false;
        let toks: Vec<&Token> = n.tokens().collect();
        let mut i = 0;
        while i < toks.len() {
            let t = toks[i];
            match t.kind {
                K::Slash => divide = true,
                K::Star => divide = false,
                K::Ident => {
                    let mut term = base_unit(&t.text).unwrap_or(Unit::NONE);
                    if toks.get(i + 1).is_some_and(|c| c.kind == K::Caret) {
                        let exp: i32 = toks
                            .get(i + 2)
                            .and_then(|e| e.text.parse().ok())
                            .unwrap_or(1);
                        match term.pow(exp) {
                            Some(p) => term = p,
                            None => self.overflow(span_of(n)),
                        }
                        i += 2;
                    }
                    let combined = if divide {
                        unit.checked_div(term)
                    } else {
                        unit.checked_mul(term)
                    };
                    match combined {
                        Some(u) => unit = u,
                        None => self.overflow(span_of(n)),
                    }
                    divide = false;
                }
                _ => {}
            }
            i += 1;
        }
        unit
    }

    fn overflow(&mut self, span: Span) {
        self.diags.push(Diagnostic::error(
            codes::E2204,
            span,
            "unit exponent out of range",
        ));
    }

    fn expr(&mut self, n: &Node) -> Option<Expr> {
        let span = span_of(n);
        let kind = match n.kind {
            N::Literal => {
                let t = n.tokens().next()?;
                match t.kind {
                    K::Number => ExprKind::Number(t.text.parse().ok()?),
                    K::String => ExprKind::Str(unescape(&t.text)),
                    _ => ExprKind::Bool(t.text == "true"),
                }
            }
            N::Quantity => {
                let num: f64 = n.token_of(K::Number)?.text.parse().ok()?;
                let unit = self.unit(n.node_of(N::Unit)?);
                ExprKind::Quantity(num * unit.factor, unit.dim)
            }
            N::Paren => return self.expr(n.nodes().next()?),
            N::Vector => ExprKind::Tuple(n.nodes().filter_map(|e| self.expr(e)).collect()),
            N::List => ExprKind::List(n.nodes().filter_map(|e| self.expr(e)).collect()),
            N::Call => ExprKind::Call(
                ident_after(n, 0)?.text.clone(),
                n.nodes().filter_map(|e| self.expr(e)).collect(),
            ),
            N::Unary => {
                let op = if n.token_of(K::Minus).is_some() {
                    UnOp::Neg
                } else {
                    UnOp::Not
                };
                ExprKind::Unary(op, Box::new(self.expr(n.nodes().next()?)?))
            }
            N::Binary => {
                let op_tok = n.tokens().find(|t| t.kind != K::Newline)?;
                let op = match op_tok.text.as_str() {
                    "+" => BinOp::Add,
                    "-" => BinOp::Sub,
                    "*" => BinOp::Mul,
                    "/" => BinOp::Div,
                    "^" => BinOp::Pow,
                    "==" => BinOp::Eq,
                    "!=" => BinOp::Ne,
                    "<" => BinOp::Lt,
                    "<=" => BinOp::Le,
                    ">" => BinOp::Gt,
                    ">=" => BinOp::Ge,
                    "and" => BinOp::And,
                    _ => BinOp::Or,
                };
                let mut operands = n.nodes();
                let a = self.expr(operands.next()?)?;
                let b = self.expr(operands.next()?)?;
                ExprKind::Binary(op, Box::new(a), Box::new(b))
            }
            N::Reference => ExprKind::Ref(self.reference(n)),
            N::Direction => ExprKind::Direction(self.direction(n)),
            N::Query => {
                let kind = ident_after(n, 1)?.text.clone();
                ExprKind::Query(kind, n.nodes().filter_map(|a| self.arg(a)).collect())
            }
            _ => return None,
        };
        Some(Expr { kind, span })
    }
}
