//! Structured, feature-level edits of a part's source. Each edit is located through the analyzed
//! model (statement and argument spans), never by searching the text, and the result is validated
//! by the caller before anything is written.

use serde_json::Value;
use tcad_model::{ItemKind, Model, analyze};

/// One edit; the fields are the arguments of the `apply_edit` tool of the same name.
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub enum Edit {
    /// Insert a statement (or several lines) after/before a named statement, default at the end.
    AddStatement {
        text: String,
        after: Option<String>,
        before: Option<String>,
    },
    /// Replace a statement (feature, sketch, tag, parameter, import) by new text.
    ReplaceStatement { name: String, text: String },
    /// Remove a statement.
    RemoveStatement { name: String },
    /// Set `arg=value` on a statement, or on an entity of a sketch if `entity` is given.
    SetArg {
        name: String,
        entity: Option<String>,
        arg: String,
        value: String,
    },
    /// Remove an argument.
    RemoveArg {
        name: String,
        entity: Option<String>,
        arg: String,
    },
    /// Declare a parameter.
    AddParam {
        name: String,
        value: String,
        range: Option<String>,
        doc: Option<String>,
        after: Option<String>,
    },
    /// Change the defining expression of a parameter.
    SetParam { name: String, value: String },
    /// Add an entity or constraint line inside a sketch.
    AddToSketch { sketch: String, text: String },
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn required(v: &Value, key: &str, op: &str) -> Result<String, String> {
    str_field(v, key).ok_or_else(|| format!("`{op}` needs `{key}`"))
}

impl Edit {
    /// Reads an edit from its JSON form: `{ "op": "set_arg", "name": "...", ... }`.
    pub fn from_json(v: &Value) -> Result<Edit, String> {
        let op = v
            .get("op")
            .and_then(Value::as_str)
            .ok_or("every edit needs an `op`")?;
        Ok(match op {
            "add_statement" => Edit::AddStatement {
                text: required(v, "text", op)?,
                after: str_field(v, "after"),
                before: str_field(v, "before"),
            },
            "replace_statement" => Edit::ReplaceStatement {
                name: required(v, "name", op)?,
                text: required(v, "text", op)?,
            },
            "remove_statement" => Edit::RemoveStatement {
                name: required(v, "name", op)?,
            },
            "set_arg" => Edit::SetArg {
                name: required(v, "name", op)?,
                entity: str_field(v, "entity"),
                arg: required(v, "arg", op)?,
                value: required(v, "value", op)?,
            },
            "remove_arg" => Edit::RemoveArg {
                name: required(v, "name", op)?,
                entity: str_field(v, "entity"),
                arg: required(v, "arg", op)?,
            },
            "add_param" => Edit::AddParam {
                name: required(v, "name", op)?,
                value: required(v, "value", op)?,
                range: str_field(v, "range"),
                doc: str_field(v, "doc"),
                after: str_field(v, "after"),
            },
            "set_param" => Edit::SetParam {
                name: required(v, "name", op)?,
                value: required(v, "value", op)?,
            },
            "add_to_sketch" => Edit::AddToSketch {
                sketch: required(v, "sketch", op)?,
                text: required(v, "text", op)?,
            },
            other => {
                return Err(format!(
                    "unknown edit `{other}`; edits are add_statement, replace_statement, remove_statement, set_arg, remove_arg, add_param, set_param, add_to_sketch"
                ));
            }
        })
    }
}

/// `(start, end)` of the statement called `name`: items, parameters and configurations.
fn statement_span(m: &Model, name: &str) -> Option<(usize, usize)> {
    if let Some(i) = m.part.items.iter().find(|i| i.name == name) {
        return Some((i.span.start, i.span.end));
    }
    if let Some(p) = m.part.params.iter().find(|p| p.name == name) {
        return Some((p.span.start, p.span.end));
    }
    m.part
        .configs
        .iter()
        .find(|c| c.name == name)
        .map(|c| (c.span.start, c.span.end))
}

fn line_start(src: &str, at: usize) -> usize {
    src[..at].rfind('\n').map_or(0, |i| i + 1)
}

/// Inserts `text` (already indented) on its own line after the statement that ends at `end`. A
/// statement's span may include its line break, so the break is only added when it is missing.
fn insert_after(src: &str, end: usize, text: &str) -> String {
    if src[..end].ends_with('\n') {
        format!("{}{}\n{}", &src[..end], text, &src[end..])
    } else {
        format!("{}\n{}{}", &src[..end], text, &src[end..])
    }
}

fn indent(text: &str) -> String {
    text.trim_matches('\n')
        .lines()
        .map(|l| {
            if l.trim().is_empty() {
                String::new()
            } else {
                format!("  {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn model_of(src: &str) -> Result<Model, String> {
    let a = analyze(src);
    match a.model {
        Some(m) if !a.has_errors() => Ok(m),
        _ => {
            let first = a
                .diagnostics
                .iter()
                .find(|d| d.is_error())
                .map_or_else(String::new, |d| format!("{}: {}", d.code, d.message));
            Err(format!(
                "the part has errors that must be fixed first ({first})"
            ))
        }
    }
}

/// Where the part's closing brace is.
fn part_end(src: &str) -> Result<usize, String> {
    src.rfind('}')
        .ok_or_else(|| "the source has no closing brace".to_string())
}

/// Applies one edit to the source text.
pub fn apply(src: &str, edit: &Edit) -> Result<String, String> {
    let m = model_of(src)?;
    let find = |name: &str| -> Result<(usize, usize), String> {
        statement_span(&m, name).ok_or_else(|| format!("there is no statement called `{name}`"))
    };
    match edit {
        Edit::AddStatement {
            text,
            after,
            before,
        } => {
            let body = indent(text);
            let (at, text) = match (after, before) {
                (Some(_), Some(_)) => return Err("give `after` or `before`, not both".into()),
                (Some(n), None) => return Ok(insert_after(src, find(n)?.1, &body)),
                (None, Some(n)) => (line_start(src, find(n)?.0), format!("{body}\n")),
                (None, None) => {
                    let end = part_end(src)?;
                    (line_start(src, end), format!("{body}\n"))
                }
            };
            Ok(format!("{}{}{}", &src[..at], text, &src[at..]))
        }
        Edit::ReplaceStatement { name, text } => {
            let (s, e) = find(name)?;
            Ok(format!("{}{}{}", &src[..s], text.trim(), &src[e..]))
        }
        Edit::RemoveStatement { name } => {
            let (s, e) = find(name)?;
            let (s, e) = (line_start(src, s), e);
            let e = src[e..].find('\n').map_or(src.len(), |i| e + i + 1);
            Ok(format!("{}{}", &src[..s], &src[e..]))
        }
        Edit::SetArg {
            name,
            entity,
            arg,
            value,
        } => {
            let (args, after) = arg_spans(&m, name, entity.as_deref())?;
            let new = format!("{arg}={value}");
            match args.iter().find(|(n, _)| n == arg) {
                Some((_, (s, e))) => Ok(format!("{}{}{}", &src[..*s], new, &src[*e..])),
                None => {
                    let at = args.last().map_or(after, |(_, (_, e))| *e);
                    Ok(format!("{} {}{}", &src[..at], new, &src[at..]))
                }
            }
        }
        Edit::RemoveArg { name, entity, arg } => {
            let (args, _) = arg_spans(&m, name, entity.as_deref())?;
            let Some((_, (s, e))) = args.iter().find(|(n, _)| n == arg) else {
                return Err(format!("`{name}` has no argument `{arg}`"));
            };
            let s = src[..*s].trim_end_matches([' ', '\t']).len();
            Ok(format!("{}{}", &src[..s], &src[*e..]))
        }
        Edit::AddParam {
            name,
            value,
            range,
            doc,
            after,
        } => {
            if m.part.params.iter().any(|p| &p.name == name) {
                return Err(format!("`{name}` is already a parameter"));
            }
            let mut line = format!("param {name} = {value}");
            if let Some(r) = range {
                line.push_str(&format!(" range({r})"));
            }
            if let Some(d) = doc {
                line.push_str(&format!(" doc \"{}\"", d.replace('"', "'")));
            }
            let at = match after {
                Some(n) => find(n)?.1,
                // after the last parameter, else after the last single-use declaration, else first
                None => m
                    .part
                    .params
                    .last()
                    .map(|p| p.span.end)
                    .or_else(|| {
                        let open = src.find('{')?;
                        Some(open + 1)
                    })
                    .ok_or("the source has no part body")?,
            };
            Ok(insert_after(src, at, &format!("  {line}")))
        }
        Edit::SetParam { name, value } => {
            let p = m
                .part
                .params
                .iter()
                .find(|p| &p.name == name)
                .ok_or_else(|| format!("there is no parameter `{name}`"))?;
            Ok(format!(
                "{}{}{}",
                &src[..p.expr.span.start],
                value,
                &src[p.expr.span.end..]
            ))
        }
        Edit::AddToSketch { sketch, text } => {
            let item = m
                .part
                .items
                .iter()
                .find(|i| &i.name == sketch && matches!(i.kind, ItemKind::Sketch(_)))
                .ok_or_else(|| format!("there is no sketch `{sketch}`"))?;
            let close = src[..item.span.end]
                .rfind('}')
                .ok_or("the sketch has no closing brace")?;
            let at = line_start(src, close);
            let body: String = text
                .trim_matches('\n')
                .lines()
                .map(|l| format!("    {l}\n"))
                .collect();
            Ok(format!("{}{}{}", &src[..at], body, &src[at..]))
        }
    }
}

type ArgSpans = Vec<(String, (usize, usize))>;

/// Spans of the arguments of a statement or of a sketch entity, and where a new argument goes if
/// there are none.
fn arg_spans(m: &Model, name: &str, entity: Option<&str>) -> Result<(ArgSpans, usize), String> {
    let item = m
        .part
        .items
        .iter()
        .find(|i| i.name == name)
        .ok_or_else(|| format!("there is no statement called `{name}`"))?;
    let spans = |args: &[tcad_expr::Arg]| -> ArgSpans {
        args.iter()
            .map(|a| (a.name.clone(), (a.span.start, a.span.end)))
            .collect()
    };
    match (entity, &item.kind) {
        (Some(e), ItemKind::Sketch(sk)) => {
            let ent = sk
                .entities
                .iter()
                .find(|x| x.name == e)
                .ok_or_else(|| format!("sketch `{name}` has no entity `{e}`"))?;
            Ok((spans(&ent.args), ent.name_span.end))
        }
        (Some(_), _) => Err(format!("`{name}` is not a sketch")),
        (None, _) => Ok((spans(&item.args), item.name_span.end)),
    }
}

/// Applies a list of edits in order.
pub fn apply_all(src: &str, edits: &[Edit]) -> Result<String, String> {
    let mut text = src.to_string();
    for (i, e) in edits.iter().enumerate() {
        text = apply(&text, e).map_err(|m| format!("edit {} of {}: {m}", i + 1, edits.len()))?;
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "tcad 0.1\n\npart clevis \"Rod clevis\" {\n  tolerance 1e-6 mm\n}\n";

    fn add_param(name: &str, value: &str, range: Option<&str>) -> Edit {
        Edit::AddParam {
            name: name.into(),
            value: value.into(),
            range: range.map(str::to_string),
            doc: None,
            after: None,
        }
    }

    #[test]
    fn parameters_are_added_in_order() {
        let text = apply_all(
            START,
            &[
                add_param("a", "20 mm", Some("10 mm, 60 mm")),
                add_param("b", "26 mm", None),
                add_param("c", "\"M24x2\"", None),
            ],
        )
        .expect("edits apply");
        let a = analyze(&text);
        assert!(!a.has_errors(), "{text}\n{:#?}", a.diagnostics);
        let names: Vec<_> = a
            .model
            .unwrap()
            .part
            .params
            .iter()
            .map(|p| p.name.clone())
            .collect();
        assert_eq!(names, ["a", "b", "c"], "{text}");
        assert!(text.contains("range(10 mm, 60 mm)"), "{text}");
    }
}
