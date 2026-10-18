//! Semantic analysis: scopes, evaluation of parameters under every configuration, ranges,
//! configurations, enumerated arguments.

use crate::ir::{Config, Item, ItemKind, Model, Param, Part, Resolved, Sketch};
use crate::lower::lower_file;
use crate::sketch::{Report, SolvedSketch, solve_sketch};
use std::collections::{BTreeSet, HashMap, HashSet};
use tcad_diag::{Diagnostic, Edit, FixHint, Span, codes};
use tcad_expr::{Arg, Env, EvalError, Expr, ExprKind, Reference, Value, eval};

/// The result of analyzing a source text.
#[derive(Debug)]
pub struct Analysis {
    /// The checked model; `None` if the file has syntax errors or no part.
    pub model: Option<Model>,
    /// All diagnostics, sorted by position and de-duplicated.
    pub diagnostics: Vec<Diagnostic>,
}

impl Analysis {
    /// True if there is at least one error.
    pub fn has_errors(&self) -> bool {
        tcad_diag::has_errors(&self.diagnostics)
    }
}

/// Parses and checks a source text. Semantic checks run only if the syntax is valid.
pub fn analyze(src: &str) -> Analysis {
    let parsed = tcad_syntax::parse(src);
    let mut diags = parsed.diagnostics.clone();
    if parsed.has_errors() {
        return Analysis {
            model: None,
            diagnostics: finish(diags),
        };
    }
    let lowered = lower_file(&parsed.root);
    diags.extend(lowered.extra);
    let mut parts = lowered.parts.into_iter();
    let Some(mut part) = parts.next() else {
        diags.push(Diagnostic::error(
            codes::E2701,
            Span::new(0, 0),
            "the file contains no `part`",
        ));
        return Analysis {
            model: None,
            diagnostics: finish(diags),
        };
    };
    if let Some(second) = parts.next() {
        diags.push(Diagnostic::error(
            codes::E2701,
            second.name_span,
            "a file holds exactly one part; move this one to its own file",
        ));
    }
    check_part(&mut part, src, &mut diags);
    let model = Model {
        version: lowered.version,
        part,
        source: src.to_string(),
    };
    Analysis {
        model: Some(model),
        diagnostics: finish(diags),
    }
}

fn finish(mut diags: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diags.sort_by_key(|d| (d.span.start, d.span.end));
    let mut seen = HashSet::new();
    diags.retain(|d| seen.insert((d.code, d.span, d.message.clone())));
    diags
}

impl Model {
    /// Evaluates the part under a configuration (`None` for the base part).
    pub fn resolve(&self, config: Option<&str>) -> Result<Resolved, Vec<Diagnostic>> {
        let part = &self.part;
        let idx = match config {
            None => None,
            Some(name) => match part.configs.iter().position(|c| c.name == name) {
                Some(i) => Some(i),
                None => {
                    return Err(vec![Diagnostic::error(
                        codes::E2401,
                        Span::new(0, 0),
                        format!("no configuration named `{name}`"),
                    )]);
                }
            },
        };
        let mut diags = Vec::new();
        let resolved = resolve_part(part, idx, &mut diags);
        if tcad_diag::has_errors(&diags) {
            Err(finish(diags))
        } else {
            Ok(resolved)
        }
    }

    /// Solves every sketch under a configuration (`None` for the base part).
    pub fn solve_sketches(
        &self,
        config: Option<&str>,
    ) -> Result<Vec<SolvedSketch>, Vec<Diagnostic>> {
        let resolved = self.resolve(config)?;
        let env: HashMap<String, Value> = resolved.params.iter().cloned().collect();
        let mut diags = Vec::new();
        let mut out = Vec::new();
        for item in &self.part.items {
            if let ItemKind::Sketch(sk) = &item.kind {
                out.push(solve_sketch(
                    &item.name,
                    item.name_span,
                    sk,
                    &env,
                    &self.source,
                    Report::Problems,
                    &mut diags,
                ));
            }
        }
        if tcad_diag::has_errors(&diags) {
            Err(finish(diags))
        } else {
            Ok(out)
        }
    }

    /// The configuration used when none is requested: the one marked `default`, else the base.
    pub fn default_config(&self) -> Option<&Config> {
        self.part.configs.iter().find(|c| c.is_default)
    }
}

struct ParamEnv<'a>(&'a HashMap<String, Value>);

impl Env for ParamEnv<'_> {
    fn get(&self, name: &str) -> Option<Value> {
        self.0.get(name).cloned()
    }
}

/// Provisional enumerated arguments (fixed per feature in `spec/features.md` from M5).
fn enum_values(keyword: &str, key: &str) -> Option<&'static [&'static str]> {
    const BOOL_OPS: &[&str] = &["add", "subtract", "intersect"];
    match (keyword, key) {
        ("extrude" | "revolve" | "sweep" | "loft" | "combine", "op") => Some(BOOL_OPS),
        ("hole", "type") => Some(&["simple", "counterbore", "countersink", "tapped"]),
        ("hole", "end") => Some(&["through_all", "blind"]),
        ("query", "type") => Some(&["plane", "cylinder", "cone", "torus", "bspline"]),
        _ => None,
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn closest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    candidates
        .map(|c| (levenshtein(name, c), c))
        .filter(|(d, _)| *d <= 2.max(name.len() / 4))
        .min_by_key(|(d, c)| (*d, *c))
        .map(|(_, c)| c)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Param,
    Item,
}

struct Scope {
    /// Part-level names: declaration position and kind.
    names: HashMap<String, (usize, Kind)>,
}

struct Checker<'a> {
    part: &'a Part,
    scope: Scope,
    diags: Vec<Diagnostic>,
    flagged: HashSet<(usize, usize)>,
}

fn check_part(part: &mut Part, src: &str, diags: &mut Vec<Diagnostic>) {
    // Names must be unique across parameters and items.
    let mut names: HashMap<String, (usize, Kind)> = HashMap::new();
    let mut declare =
        |name: &str, pos: usize, span: Span, kind: Kind, diags: &mut Vec<Diagnostic>| {
            if names.contains_key(name) {
                diags.push(Diagnostic::error(
                    codes::E2101,
                    span,
                    format!("`{name}` is already declared"),
                ));
            } else {
                names.insert(name.to_string(), (pos, kind));
            }
        };
    for p in &part.params {
        declare(&p.name, p.name_span.start, p.name_span, Kind::Param, diags);
    }
    for i in &part.items {
        declare(&i.name, i.name_span.start, i.name_span, Kind::Item, diags);
    }
    let mut cnames = HashSet::new();
    for c in &part.configs {
        if !cnames.insert(c.name.as_str()) {
            diags.push(Diagnostic::error(
                codes::E2101,
                c.name_span,
                format!("configuration `{}` is already declared", c.name),
            ));
        }
    }
    let defaults: Vec<&Config> = part.configs.iter().filter(|c| c.is_default).collect();
    if defaults.len() > 1 {
        diags.push(Diagnostic::error(
            codes::E2404,
            defaults[1].name_span,
            "only one configuration can be marked `default`",
        ));
    } else if defaults.is_empty() && !part.configs.is_empty() {
        diags.push(Diagnostic::warning(
            codes::W2405,
            part.configs[0].name_span,
            "configurations exist but none is marked `default`; the base part is used by default",
        ));
    }

    let mut ck = Checker {
        part,
        scope: Scope { names },
        diags: Vec::new(),
        flagged: HashSet::new(),
    };
    ck.scopes();
    let flagged = ck.flagged.clone();
    diags.append(&mut ck.diags);

    // Evaluate the base part and every configuration.
    let mut all: Vec<Option<usize>> = vec![None];
    all.extend((0..part.configs.len()).map(Some));
    let mut base_values: Vec<(String, Value)> = Vec::new();
    let mut base_keys: HashSet<(tcad_diag::Code, Span)> = HashSet::new();
    let solvable = flagged.is_empty() && !tcad_diag::has_errors(diags);
    for cfg in all {
        let mut ds = Vec::new();
        let resolved = resolve_part_with(part, cfg, &mut ds, &flagged);
        if solvable && !tcad_diag::has_errors(&ds) {
            let env: HashMap<String, Value> = resolved.params.iter().cloned().collect();
            let report = if cfg.is_none() {
                Report::WithDegreesOfFreedom
            } else {
                Report::Problems
            };
            let mut sketch_diags = Vec::new();
            for item in &part.items {
                if let ItemKind::Sketch(sk) = &item.kind {
                    solve_sketch(
                        &item.name,
                        item.name_span,
                        sk,
                        &env,
                        src,
                        report,
                        &mut sketch_diags,
                    );
                }
            }
            if let Some(c) = cfg.map(|i| &part.configs[i]) {
                for d in &mut sketch_diags {
                    d.message = format!("{} (in configuration `{}`)", d.message, c.name);
                }
            }
            ds.append(&mut sketch_diags);
        }
        if cfg.is_none() {
            base_keys = ds.iter().map(|d| (d.code, d.span)).collect();
            base_values = resolved.params;
        } else {
            // A problem in the base part shows up again in every configuration; report it once.
            ds.retain(|d| !base_keys.contains(&(d.code, d.span)));
        }
        diags.append(&mut ds);
    }
    for (p, (_, v)) in part.params.iter_mut().zip(base_values) {
        p.value = v;
    }
}

impl Checker<'_> {
    fn flag(&mut self, d: Diagnostic) {
        self.flagged.insert((d.span.start, d.span.end));
        self.diags.push(d);
    }

    /// Resolves the head of a reference. `before` is the position the name must precede.
    fn check_ref(
        &mut self,
        r: &Reference,
        before: usize,
        locals: &HashMap<String, usize>,
        local_before: usize,
    ) {
        let head = r.head();
        if matches!(head, "plane" | "axis" | "origin" | "edge") {
            return;
        }
        if let Some(&pos) = locals.get(head) {
            if pos < local_before {
                return;
            }
            // A sketch entity declared later does not hide an earlier part-level name.
            if !matches!(self.scope.names.get(head), Some(&(p, _)) if p < before) {
                self.flag(Diagnostic::error(
                    codes::E2103,
                    r.span,
                    format!(
                        "`{head}` is used before its declaration; references only point backward"
                    ),
                ));
            }
            return;
        }
        match self.scope.names.get(head) {
            Some(&(pos, _)) if pos < before => {}
            Some(_) => self.flag(Diagnostic::error(
                codes::E2103,
                r.span,
                format!("`{head}` is used before its declaration; references only point backward"),
            )),
            None => {
                let candidates = self
                    .scope
                    .names
                    .keys()
                    .map(String::as_str)
                    .chain(locals.keys().map(String::as_str));
                let mut d =
                    Diagnostic::error(codes::E2102, r.span, format!("`{head}` is not defined"));
                if let Some(c) = closest(head, candidates) {
                    d.message = format!("`{head}` is not defined; did you mean `{c}`?");
                    let head_span = r.segments.first().map_or(r.span, |s| s.span);
                    d = d.with_hint(FixHint {
                        message: format!("replace with `{c}`"),
                        edits: vec![Edit {
                            span: head_span,
                            replacement: c.to_string(),
                        }],
                    });
                }
                self.flag(d);
            }
        }
    }

    /// Checks every reference in an expression. `enum_ok` lists the words allowed where the
    /// expression is the value of an enumerated argument.
    fn check_expr(
        &mut self,
        e: &Expr,
        before: usize,
        locals: &HashMap<String, usize>,
        local_before: usize,
        enum_ok: Option<&[&str]>,
    ) {
        match &e.kind {
            ExprKind::Ref(r) => match (enum_ok, r.simple_name()) {
                (Some(allowed), Some(name)) => {
                    if !allowed.contains(&name) && !self.scope.names.contains_key(name) {
                        self.flag(Diagnostic::error(
                            codes::E2501,
                            r.span,
                            format!(
                                "unknown value `{name}`; expected one of: {}",
                                allowed.join(", ")
                            ),
                        ));
                    }
                }
                _ => self.check_ref(r, before, locals, local_before),
            },
            ExprKind::Unary(_, x) => self.check_expr(x, before, locals, local_before, None),
            ExprKind::Binary(_, a, b) => {
                self.check_expr(a, before, locals, local_before, None);
                self.check_expr(b, before, locals, local_before, None);
            }
            ExprKind::Call(_, es) | ExprKind::Tuple(es) | ExprKind::List(es) => {
                for x in es {
                    self.check_expr(x, before, locals, local_before, None);
                }
            }
            ExprKind::Query(kind, args) | ExprKind::Profile(kind, args) => {
                let kind = if matches!(e.kind, ExprKind::Query(..)) {
                    "query"
                } else {
                    kind.as_str()
                };
                self.check_args(kind, args, before, locals, local_before);
            }
            _ => {}
        }
    }

    /// Checks the argument values of a statement, query or profile.
    fn check_args(
        &mut self,
        keyword: &str,
        args: &[Arg],
        before: usize,
        locals: &HashMap<String, usize>,
        local_before: usize,
    ) {
        for a in args {
            let allowed = enum_values(keyword, &a.name);
            for v in &a.values {
                self.check_expr(v, before, locals, local_before, allowed);
            }
        }
    }

    fn scopes(&mut self) {
        let part = self.part;
        for p in &part.params {
            let none = HashMap::new();
            self.check_expr(&p.expr, p.span.start, &none, 0, None);
            if let Some((lo, hi)) = &p.range {
                self.check_expr(lo, p.span.start, &none, 0, None);
                self.check_expr(hi, p.span.start, &none, 0, None);
            }
        }
        for item in &part.items {
            self.check_item(item);
        }
        for c in &part.configs {
            self.check_config(c);
        }
    }

    fn check_item(&mut self, item: &Item) {
        let pos = item.span.start;
        let none = HashMap::new();
        if let Some(s) = &item.suppress_if {
            self.check_expr(s, pos, &none, 0, None);
        }
        match &item.kind {
            ItemKind::RefGeom | ItemKind::Feature => {
                self.check_args(&item.keyword, &item.args, pos, &none, 0);
            }
            ItemKind::Tag(target) => self.check_ref(target, pos, &none, 0),
            ItemKind::Import(_) => {}
            ItemKind::Sketch(sk) => self.check_sketch(item, sk),
        }
    }

    fn check_sketch(&mut self, item: &Item, sk: &Sketch) {
        let pos = item.span.start;
        let none = HashMap::new();
        self.check_ref(&sk.plane, pos, &none, 0);
        let mut locals: HashMap<String, usize> = HashMap::new();
        for e in &sk.entities {
            if locals.contains_key(&e.name) {
                self.flag(Diagnostic::error(
                    codes::E2101,
                    e.name_span,
                    format!("`{}` is already declared in this sketch", e.name),
                ));
            } else {
                locals.insert(e.name.clone(), e.name_span.start);
            }
        }
        for e in &sk.entities {
            self.check_args(&e.kind, &e.args, pos, &locals, e.span.start);
            for h in e.hint.iter().flatten() {
                self.check_expr(h, pos, &locals, e.span.start, None);
            }
        }
        for c in &sk.constraints {
            for a in c.args.iter().chain(c.value.iter()) {
                self.check_expr(a, pos, &locals, usize::MAX, None);
            }
        }
    }

    fn check_config(&mut self, c: &Config) {
        for o in &c.overrides {
            match self.part.params.iter().find(|p| p.name == o.param) {
                Some(target) => {
                    let none = HashMap::new();
                    self.check_expr(&o.expr, target.span.start, &none, 0, None);
                }
                None => {
                    let mut d = Diagnostic::error(
                        codes::E2401,
                        o.span,
                        format!(
                            "configuration `{}` overrides `{}`, which is not a parameter",
                            c.name, o.param
                        ),
                    );
                    if let Some(s) =
                        closest(&o.param, self.part.params.iter().map(|p| p.name.as_str()))
                    {
                        d.message.push_str(&format!("; did you mean `{s}`?"));
                    }
                    self.flag(d);
                }
            }
        }
        for r in &c.suppress {
            let ok = r.simple_name().is_some_and(|n| {
                self.part.items.iter().any(|i| {
                    i.name == n && !matches!(i.kind, ItemKind::Tag(_) | ItemKind::Import(_))
                })
            });
            if !ok {
                self.flag(Diagnostic::error(
                    codes::E2403,
                    r.span,
                    format!(
                        "configuration `{}` suppresses `{}`, which is not a feature",
                        c.name,
                        r.dotted()
                    ),
                ));
            }
        }
    }
}

fn kinds_match(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.dim == y.dim,
        (Value::Str(_), Value::Str(_)) | (Value::Bool(_), Value::Bool(_)) => true,
        (Value::Tuple(x), Value::Tuple(y)) | (Value::List(x), Value::List(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| kinds_match(p, q))
        }
        _ => false,
    }
}

fn resolve_part(part: &Part, cfg: Option<usize>, diags: &mut Vec<Diagnostic>) -> Resolved {
    resolve_part_with(part, cfg, diags, &HashSet::new())
}

/// Evaluates all parameters under a configuration and the value-like expressions of every item.
fn resolve_part_with(
    part: &Part,
    cfg: Option<usize>,
    diags: &mut Vec<Diagnostic>,
    flagged: &HashSet<(usize, usize)>,
) -> Resolved {
    let config = cfg.map(|i| &part.configs[i]);
    let ctx = config
        .map(|c| format!(" (in configuration `{}`)", c.name))
        .unwrap_or_default();
    let mut env: HashMap<String, Value> = HashMap::new();
    let mut failed: HashSet<String> = HashSet::new();
    let mut ordered: Vec<(String, Value)> = Vec::new();

    let report = |d: Diagnostic, diags: &mut Vec<Diagnostic>| {
        diags.push(Diagnostic {
            message: format!("{}{}", d.message, ctx),
            ..d
        });
    };
    let not_value =
        |span: Span, what: &str, failed: &HashSet<String>, diags: &mut Vec<Diagnostic>| {
            let root = what.split('.').next().unwrap_or(what);
            if !flagged.contains(&(span.start, span.end)) && !failed.contains(root) {
                diags.push(Diagnostic::error(
                    codes::E2104,
                    span,
                    format!("`{what}` does not denote a value here"),
                ));
            }
        };

    for p in &part.params {
        let ov = config.and_then(|c| c.overrides.iter().find(|o| o.param == p.name));
        let expr = ov.map_or(&p.expr, |o| &o.expr);
        let value = match eval(expr, &ParamEnv(&env)) {
            Ok(v) => v,
            Err(EvalError::Diag(d)) => {
                report(d, diags);
                failed.insert(p.name.clone());
                continue;
            }
            Err(EvalError::NotAValue(span, what)) => {
                not_value(span, &what, &failed, diags);
                failed.insert(p.name.clone());
                continue;
            }
        };
        if let Some(o) = ov {
            // The override must keep the parameter's type and dimension.
            if let Ok(base) = eval(&p.expr, &ParamEnv(&env))
                && !kinds_match(&base, &value)
            {
                report(
                    Diagnostic::error(
                        codes::E2402,
                        o.span,
                        format!(
                            "`{}` is {}, but the override is {}",
                            p.name,
                            base.type_name(),
                            value.type_name()
                        ),
                    ),
                    diags,
                );
                failed.insert(p.name.clone());
                continue;
            }
        }
        if let Some(d) = check_range(p, &value, &env) {
            report(d, diags);
        }
        env.insert(p.name.clone(), value.clone());
        ordered.push((p.name.clone(), value));
    }

    // Items: evaluate everything that is a plain value, so unit errors surface.
    let penv = ParamEnv(&env);
    let mut suppressed: BTreeSet<String> = BTreeSet::new();
    if let Some(c) = config {
        suppressed.extend(
            c.suppress
                .iter()
                .filter_map(|r| r.simple_name().map(str::to_string)),
        );
    }
    let eval_value = |e: &Expr, diags: &mut Vec<Diagnostic>| match eval(e, &penv) {
        Ok(v) => Some(v),
        Err(EvalError::Diag(d)) => {
            report(d, diags);
            None
        }
        Err(EvalError::NotAValue(..)) => None,
    };
    for item in &part.items {
        for a in &item.args {
            a.values.iter().for_each(|v| {
                eval_value(v, diags);
            });
        }
        if let ItemKind::Sketch(sk) = &item.kind {
            for e in &sk.entities {
                for a in &e.args {
                    a.values.iter().for_each(|v| {
                        eval_value(v, diags);
                    });
                }
                e.hint.iter().flatten().for_each(|h| {
                    eval_value(h, diags);
                });
            }
            for c in &sk.constraints {
                c.args.iter().chain(c.value.iter()).for_each(|v| {
                    eval_value(v, diags);
                });
            }
        }
        if let Some(cond) = &item.suppress_if {
            match eval(cond, &penv) {
                Ok(Value::Bool(true)) => {
                    suppressed.insert(item.name.clone());
                }
                Ok(Value::Bool(false)) => {}
                Ok(other) => report(
                    Diagnostic::error(
                        codes::E2601,
                        cond.span,
                        format!("`suppress if` needs a boolean, found {}", other.type_name()),
                    ),
                    diags,
                ),
                Err(EvalError::Diag(d)) => report(d, diags),
                Err(EvalError::NotAValue(span, what)) => not_value(span, &what, &failed, diags),
            }
        }
    }

    Resolved {
        config: config.map(|c| c.name.clone()),
        params: ordered,
        suppressed,
    }
}

fn check_range(p: &Param, value: &Value, env: &HashMap<String, Value>) -> Option<Diagnostic> {
    let (lo, hi) = p.range.as_ref()?;
    let bad = |msg: String| Some(Diagnostic::error(codes::E2304, p.span, msg));
    let (Value::Num(v), Ok(Value::Num(l)), Ok(Value::Num(h))) =
        (value, eval(lo, &ParamEnv(env)), eval(hi, &ParamEnv(env)))
    else {
        return bad(format!(
            "`range` needs numeric bounds and a numeric parameter (`{}`)",
            p.name
        ));
    };
    if l.dim != v.dim || h.dim != v.dim {
        return bad(format!(
            "the range of `{}` must have the dimension of the parameter ({}), found {} and {}",
            p.name, v.dim, l.dim, h.dim
        ));
    }
    if l.value > h.value {
        return bad(format!(
            "the range of `{}` is empty: {} is greater than {}",
            p.name, l, h
        ));
    }
    if v.value < l.value || v.value > h.value {
        return Some(Diagnostic::error(
            codes::E2301,
            p.span,
            format!("`{}` = {} is outside its range {} to {}", p.name, v, l, h),
        ));
    }
    None
}
