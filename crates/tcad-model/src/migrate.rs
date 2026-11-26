//! Moving a file to a newer format version (`tcad migrate`, `spec/versioning.md`).
//!
//! A migration rewrites the version in the header and renames every declared name that would
//! clash with a keyword of the new version, together with all references to it. It never changes
//! what the file means: the result is checked to analyze without errors and to evaluate every
//! configuration to the same parameter values, otherwise nothing is returned.

use crate::{Model, analyze};
use std::collections::BTreeSet;
use tcad_diag::{Diagnostic, Span};
use tcad_syntax::keywords::{self as kw, Version};
use tcad_syntax::lexer::lex;
use tcad_syntax::token::{Token, TokenKind};

/// The result of a migration.
#[derive(Clone, Debug, PartialEq)]
pub struct Migration {
    /// The migrated source text (equal to the input if nothing had to change).
    pub text: String,
    /// The version the file declared.
    pub from: Version,
    /// The version it declares now.
    pub to: Version,
    /// Declared names that were renamed, `(old, new)`, in declaration order.
    pub renames: Vec<(String, String)>,
}

impl Migration {
    /// True if the text changed.
    pub fn changed(&self) -> bool {
        self.from != self.to || !self.renames.is_empty()
    }
}

/// Why a file cannot be migrated.
#[derive(Clone, Debug, PartialEq)]
pub enum MigrateError {
    /// The file has errors; they must be fixed first (the diagnostics are attached).
    Invalid(Vec<Diagnostic>),
    /// The file has no valid header, so its version is unknown.
    NoVersion,
    /// The target version is not known to this implementation.
    UnknownTarget(Version),
    /// The target is older than the file (migrations only move forward).
    Downgrade {
        /// The file's version.
        from: Version,
        /// The requested version.
        to: Version,
    },
    /// A clashing name is also the name of a sketch entity, so its uses inside that sketch cannot
    /// be told apart by text alone; rename one of them by hand (`tcad` LSP rename) first.
    Ambiguous {
        /// The clashing name.
        name: String,
        /// The sketch that declares an entity with the same name.
        sketch: String,
    },
    /// The migrated text would mean something else (a bug in the migration; nothing is written).
    Changed(String),
}

impl std::fmt::Display for MigrateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(d) => write!(
                f,
                "the file has {} error(s); fix them before migrating",
                d.len()
            ),
            Self::NoVersion => f.write_str("the file has no valid `tcad <major>.<minor>` header"),
            Self::UnknownTarget((a, b)) => write!(
                f,
                "format {a}.{b} is not known; this tool knows up to {}.{}",
                kw::LATEST.0,
                kw::LATEST.1
            ),
            Self::Downgrade { from, to } => write!(
                f,
                "the file is format {}.{}; migrating back to {}.{} is not supported",
                from.0, from.1, to.0, to.1
            ),
            Self::Ambiguous { name, sketch } => write!(
                f,
                "`{name}` must be renamed, but sketch `{sketch}` also has an entity named `{name}`; \
                 rename one of them by hand first"
            ),
            Self::Changed(why) => write!(f, "the migration would change the file's meaning: {why}"),
        }
    }
}

impl std::error::Error for MigrateError {}

/// Migrates `src` to format version `to`.
pub fn migrate(src: &str, to: Version) -> Result<Migration, MigrateError> {
    if to.0 != kw::LATEST.0 || to > kw::LATEST {
        return Err(MigrateError::UnknownTarget(to));
    }
    let analysis = analyze(src);
    if analysis.has_errors() {
        return Err(MigrateError::Invalid(
            analysis
                .diagnostics
                .into_iter()
                .filter(Diagnostic::is_error)
                .collect(),
        ));
    }
    let parsed = tcad_syntax::parse(src);
    let (Some(from), Some(version_span)) = (parsed.version, parsed.version_span) else {
        return Err(MigrateError::NoVersion);
    };
    if from > to {
        return Err(MigrateError::Downgrade { from, to });
    }
    if from == to {
        return Ok(Migration {
            text: src.to_string(),
            from,
            to,
            renames: Vec::new(),
        });
    }
    let model = analysis.model.ok_or(MigrateError::NoVersion)?;

    // Every name declared anywhere in the part; new names must not collide with any of them.
    let declared = declared_names(&model);
    let mut taken: BTreeSet<String> = declared.iter().map(|(n, _)| n.clone()).collect();
    for item in &model.part.items {
        if let crate::ItemKind::Sketch(sk) = &item.kind {
            taken.extend(sk.entities.iter().map(|e| e.name.clone()));
        }
    }

    let (tokens, _) = lex(src);
    let mut edits: Vec<(Span, String)> = vec![(version_span, format!("{}.{}", to.0, to.1))];
    let mut renames = Vec::new();
    for (old, decl) in &declared {
        if !kw::is_keyword_in(old, false, to) || kw::is_keyword_in(old, false, from) {
            continue;
        }
        // Inside a sketch with an entity of the same name a bare word may mean either.
        if let Some(sketch) = model.part.items.iter().find(|i| {
            matches!(&i.kind, crate::ItemKind::Sketch(sk) if sk.entities.iter().any(|e| &e.name == old))
        }) {
            return Err(MigrateError::Ambiguous {
                name: old.clone(),
                sketch: sketch.name.clone(),
            });
        }
        let new = fresh_name(old, &taken, to);
        taken.insert(new.clone());
        for span in top_level_references(&tokens, old, *decl) {
            edits.push((span, new.clone()));
        }
        renames.push((old.clone(), new));
    }

    edits.sort_by_key(|(s, _)| std::cmp::Reverse(s.start));
    let mut text = src.to_string();
    for (span, replacement) in &edits {
        text.replace_range(span.start..span.end, replacement);
    }

    verify_same_meaning(&model, &text, &renames)?;
    Ok(Migration {
        text,
        from,
        to,
        renames,
    })
}

/// Part-level declared names with the spans of their declarations, in declaration order.
fn declared_names(model: &Model) -> Vec<(String, Span)> {
    let part = &model.part;
    let mut names: Vec<(Span, String)> = part
        .params
        .iter()
        .map(|p| (p.name_span, p.name.clone()))
        .chain(part.items.iter().map(|i| (i.name_span, i.name.clone())))
        .chain(part.configs.iter().map(|c| (c.name_span, c.name.clone())))
        .collect();
    names.sort_by_key(|(s, _)| s.start);
    names.into_iter().map(|(s, n)| (n, s)).collect()
}

/// `old_1`, `old_2`, ...: the first that is free and not a keyword of `version`.
fn fresh_name(old: &str, taken: &BTreeSet<String>, version: Version) -> String {
    (1..)
        .map(|n| format!("{old}_{n}"))
        .find(|c| !taken.contains(c) && !kw::is_keyword_in(c, false, version))
        .expect("an unused name exists")
}

/// The spans of the words in `tokens` that refer to the part-level name `old` declared at
/// `decl`: the declaration itself and every bare (not `x.old`) use, except argument names
/// (`old=` outside configurations). Configuration overrides `old = ...` are references.
pub fn top_level_references(tokens: &[Token], old: &str, decl: Span) -> Vec<Span> {
    let mut out = Vec::new();
    for (n, t) in tokens.iter().enumerate() {
        if t.kind != TokenKind::Ident || t.text != old {
            continue;
        }
        let prev_dot = n > 0 && tokens[n - 1].kind == TokenKind::Dot;
        let key = tokens.get(n + 1).is_some_and(|x| x.kind == TokenKind::Eq)
            && !prev_dot
            && t.span != decl
            && !in_config(tokens, n);
        if !prev_dot && !key {
            out.push(t.span);
        }
    }
    out
}

/// True if token `n` is inside the `{ }` of a `config` statement.
pub fn in_config(tokens: &[Token], n: usize) -> bool {
    let mut depth = 0usize;
    for i in (0..n).rev() {
        match tokens[i].kind {
            TokenKind::RBrace => depth += 1,
            TokenKind::LBrace if depth > 0 => depth -= 1,
            TokenKind::LBrace => {
                return i >= 2 && tokens[i - 2].text == "config";
            }
            _ => {}
        }
    }
    false
}

/// The migrated text must analyze cleanly and evaluate every configuration to the same values.
fn verify_same_meaning(
    before: &Model,
    text: &str,
    renames: &[(String, String)],
) -> Result<(), MigrateError> {
    let after = analyze(text);
    if let Some(d) = after.diagnostics.iter().find(|d| d.is_error()) {
        return Err(MigrateError::Changed(format!(
            "the result has an error: {}",
            d.message
        )));
    }
    let after = after
        .model
        .ok_or_else(|| MigrateError::Changed("the result has no part".into()))?;
    let rename = |name: &str| -> String {
        renames
            .iter()
            .find(|(o, _)| o == name)
            .map_or_else(|| name.to_string(), |(_, n)| n.clone())
    };
    let configs: Vec<Option<String>> = std::iter::once(None)
        .chain(before.part.configs.iter().map(|c| Some(c.name.clone())))
        .collect();
    for config in configs {
        let a = before.resolve(config.as_deref());
        let b = after.resolve(config.as_deref().map(rename).as_deref());
        match (a, b) {
            (Ok(a), Ok(b)) => {
                let a_params: Vec<(String, _)> =
                    a.params.into_iter().map(|(n, v)| (rename(&n), v)).collect();
                let a_suppressed: BTreeSet<String> =
                    a.suppressed.iter().map(|n| rename(n)).collect();
                if a_params != b.params || a_suppressed != b.suppressed {
                    return Err(MigrateError::Changed(format!(
                        "configuration {} evaluates differently",
                        config.as_deref().unwrap_or("(base)")
                    )));
                }
            }
            (Err(_), Err(_)) => {}
            _ => {
                return Err(MigrateError::Changed(format!(
                    "configuration {} no longer resolves the same way",
                    config.as_deref().unwrap_or("(base)")
                )));
            }
        }
    }
    let names = |m: &Model| -> Vec<(String, String)> {
        m.part
            .items
            .iter()
            .map(|i| (i.keyword.clone(), i.name.clone()))
            .collect()
    };
    let expected: Vec<(String, String)> = names(before)
        .into_iter()
        .map(|(k, n)| (k, rename(&n)))
        .collect();
    if expected != names(&after) {
        return Err(MigrateError::Changed("the feature list differs".into()));
    }
    Ok(())
}
