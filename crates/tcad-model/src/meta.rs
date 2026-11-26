//! Part metadata: the `meta` statement (format 0.2, `spec/features.md` § Part metadata).

use tcad_diag::{Diagnostic, Span, codes};
use tcad_expr::eval::{Env, EvalError, eval};
use tcad_expr::{Arg, Dim, Expr, ExprKind, Value};

/// The keys `meta` accepts, in canonical (spec) order.
pub const META_KEYS: &[&str] = &[
    "number",
    "revision",
    "kind",
    "vendor",
    "vendor_number",
    "mass",
    "com",
    "color",
];

/// The values of `kind`.
pub const KINDS: &[&str] = &["made", "purchased", "phantom"];

/// A `meta` statement as written.
#[derive(Clone, Debug)]
pub struct Meta {
    /// Its arguments in source order.
    pub args: Vec<Arg>,
    /// Span of the statement.
    pub span: Span,
}

/// `meta.<key> = <expr>` in a configuration.
#[derive(Clone, Debug)]
pub struct MetaOverride {
    /// The metadata key.
    pub key: String,
    /// The new value.
    pub expr: Expr,
    /// Span of the override.
    pub span: Span,
}

/// How a part is obtained, for the bill of materials.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MetaKind {
    /// Made in house (the default).
    #[default]
    Made,
    /// Bought.
    Purchased,
    /// Not a real item: dissolved into its parent in a BOM.
    Phantom,
}

impl MetaKind {
    /// The word used in the source.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Made => "made",
            Self::Purchased => "purchased",
            Self::Phantom => "phantom",
        }
    }
}

/// Metadata of a part under one configuration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartMeta {
    /// Part number.
    pub number: Option<String>,
    /// Revision.
    pub revision: Option<String>,
    /// Made, purchased or phantom.
    pub kind: MetaKind,
    /// Vendor of a purchased part.
    pub vendor: Option<String>,
    /// The vendor's part number.
    pub vendor_number: Option<String>,
    /// Mass that overrides the computed one (kg).
    pub mass_kg: Option<f64>,
    /// Center of mass that overrides the computed one (mm); only with `mass_kg`.
    pub com_mm: Option<[f64; 3]>,
    /// Display colour, red green blue in 0..1.
    pub color: Option<[f64; 3]>,
}

impl PartMeta {
    /// True if nothing is set (the part has no `meta` or it sets nothing).
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Structural checks that do not depend on values: unknown keys, flags, repeated keys, `com`
/// without `mass`, unknown override keys.
pub(crate) fn check_structure(
    meta: Option<&Meta>,
    overrides: &[(&str, &MetaOverride)],
    diags: &mut Vec<Diagnostic>,
) {
    let mut keys: Vec<&str> = Vec::new();
    if let Some(m) = meta {
        for a in &m.args {
            if a.flag || a.name == "from" {
                diags.push(Diagnostic::error(
                    codes::E5001,
                    a.span,
                    format!(
                        "`meta` takes `key=value` arguments, not `{}`; keys: {}",
                        a.name,
                        META_KEYS.join(", ")
                    ),
                ));
            } else if !META_KEYS.contains(&a.name.as_str()) {
                diags.push(Diagnostic::error(
                    codes::E5001,
                    a.span,
                    format!(
                        "unknown `meta` key `{}`; keys: {}",
                        a.name,
                        META_KEYS.join(", ")
                    ),
                ));
            } else if keys.contains(&a.name.as_str()) {
                diags.push(Diagnostic::error(
                    codes::E5001,
                    a.span,
                    format!("`meta` key `{}` is given twice", a.name),
                ));
            } else if a.values.len() != 1 {
                diags.push(Diagnostic::error(
                    codes::E5001,
                    a.span,
                    format!("`meta` key `{}` takes exactly one value", a.name),
                ));
            } else {
                keys.push(&a.name);
            }
        }
        if keys.contains(&"com")
            && !keys.contains(&"mass")
            && let Some(a) = m.args.iter().find(|a| a.name == "com")
        {
            diags.push(Diagnostic::error(
                codes::E5001,
                a.span,
                "`com` overrides the center of mass only together with `mass`",
            ));
        }
    }
    for (config, o) in overrides {
        if !META_KEYS.contains(&o.key.as_str()) {
            diags.push(Diagnostic::error(
                codes::E2401,
                o.span,
                format!(
                    "configuration `{config}` overrides `meta.{}`, which is not a `meta` key; keys: {}",
                    o.key,
                    META_KEYS.join(", ")
                ),
            ));
        } else if o.key == "com" && !keys.contains(&"mass") {
            diags.push(Diagnostic::error(
                codes::E5001,
                o.span,
                "`meta.com` can be overridden only when `meta` sets `mass`",
            ));
        }
    }
}

/// Evaluates the metadata with the configuration's overrides applied. Problems are pushed to
/// `diags`; keys with problems stay unset.
pub(crate) fn evaluate(
    meta: Option<&Meta>,
    overrides: &[MetaOverride],
    env: &dyn Env,
    diags: &mut Vec<Diagnostic>,
) -> PartMeta {
    let mut out = PartMeta::default();
    let mut entries: Vec<(&str, &Expr, Span)> = Vec::new();
    if let Some(m) = meta {
        for a in &m.args {
            if !a.flag && a.values.len() == 1 && META_KEYS.contains(&a.name.as_str()) {
                if entries.iter().any(|(k, _, _)| *k == a.name) {
                    continue;
                }
                entries.push((&a.name, &a.values[0], a.span));
            }
        }
    }
    for o in overrides {
        if let Some(e) = entries.iter_mut().find(|(k, _, _)| *k == o.key) {
            *e = (&o.key, &o.expr, o.span);
        } else if META_KEYS.contains(&o.key.as_str()) {
            entries.push((&o.key, &o.expr, o.span));
        }
    }
    let value = |e: &Expr, diags: &mut Vec<Diagnostic>| match eval(e, env) {
        Ok(v) => Some(v),
        Err(EvalError::Diag(d)) => {
            diags.push(d);
            None
        }
        Err(EvalError::NotAValue(..)) => None, // reported by the scope check
    };
    let type_error = |key: &str, want: &str, got: &Value, span: Span| {
        Diagnostic::error(
            codes::E2202,
            span,
            format!("`meta` `{key}` must be {want}, found {}", got.type_name()),
        )
    };
    for (key, e, span) in entries {
        match key {
            "kind" => {
                // an enumerated word, not an expression
                if let ExprKind::Ref(r) = &e.kind
                    && let Some(w) = r.simple_name()
                {
                    match w {
                        "made" => out.kind = MetaKind::Made,
                        "purchased" => out.kind = MetaKind::Purchased,
                        "phantom" => out.kind = MetaKind::Phantom,
                        _ => {} // E2501 from the scope check
                    }
                } else {
                    diags.push(Diagnostic::error(
                        codes::E2501,
                        e.span,
                        format!("`kind` is one of: {}", KINDS.join(", ")),
                    ));
                }
            }
            "number" | "revision" | "vendor" | "vendor_number" => match value(e, diags) {
                Some(Value::Str(s)) => {
                    let slot = match key {
                        "number" => &mut out.number,
                        "revision" => &mut out.revision,
                        "vendor" => &mut out.vendor,
                        _ => &mut out.vendor_number,
                    };
                    *slot = Some(s);
                }
                Some(other) => diags.push(type_error(key, "a string", &other, span)),
                None => {}
            },
            "mass" => match value(e, diags) {
                Some(Value::Num(q)) if q.dim == Dim::MASS && q.value > 0.0 => {
                    out.mass_kg = Some(q.value);
                }
                Some(Value::Num(q)) if q.dim == Dim::MASS => diags.push(Diagnostic::error(
                    codes::E2301,
                    span,
                    "`meta` `mass` must be positive",
                )),
                Some(other) => {
                    diags.push(type_error(key, "a mass such as `0.84 kg`", &other, span))
                }
                None => {}
            },
            "com" | "color" => {
                let Some(v) = value(e, diags) else { continue };
                let parts: Option<Vec<_>> = match &v {
                    Value::Tuple(xs) if xs.len() == 3 => {
                        xs.iter().map(Value::as_quantity).collect()
                    }
                    _ => None,
                };
                let want = if key == "com" {
                    "a point `(x, y, z)` of lengths"
                } else {
                    "a colour `(r, g, b)` of numbers from 0 to 1"
                };
                let dim = if key == "com" { Dim::LENGTH } else { Dim::NONE };
                match parts {
                    Some(qs) if qs.iter().all(|q| q.dim == dim) => {
                        let xyz = [qs[0].value, qs[1].value, qs[2].value];
                        if key == "com" {
                            out.com_mm = Some(xyz);
                        } else if xyz.iter().all(|c| (0.0..=1.0).contains(c)) {
                            out.color = Some(xyz);
                        } else {
                            diags.push(Diagnostic::error(
                                codes::E2301,
                                span,
                                "`meta` `color` components must lie between 0 and 1",
                            ));
                        }
                    }
                    _ => diags.push(type_error(key, want, &v, span)),
                }
            }
            _ => {}
        }
    }
    if out.com_mm.is_some() && out.mass_kg.is_none() {
        out.com_mm = None; // reported by `check_structure`
    }
    out
}
