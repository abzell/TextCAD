//! `tcad fmt --solve`: write the solved coordinates of every sketch entity back into the file as
//! `@hint(...)`, so later regenerations start from the same solution and pick the same branch.

use crate::ItemKind;
use crate::check::analyze;
use crate::sketch::Geometry;
use tcad_diag::Diagnostic;

/// Formats a value for a hint: rounded to nanometres (the shortest text that round-trips the
/// rounded value), without negative zero.
fn number(v: f64) -> String {
    let r = (v * 1e9).round() / 1e9;
    let r = if r == 0.0 { 0.0 } else { r };
    format!("{r}")
}

fn hint_text(values: &[(f64, bool)]) -> String {
    let items: Vec<String> = values
        .iter()
        .map(|&(v, is_angle)| {
            if is_angle {
                format!("{} deg", number(v.to_degrees()))
            } else {
                format!("{} mm", number(v))
            }
        })
        .collect();
    format!("@hint({})", items.join(", "))
}

/// Solves all sketches (under the default configuration) and returns the canonically formatted
/// source with `@hint(...)` added or updated on every entity. Fails with the diagnostics if the
/// file has errors.
pub fn solve_hints(src: &str) -> Result<String, Vec<Diagnostic>> {
    let analysis = analyze(src);
    if analysis.has_errors() {
        return Err(analysis.diagnostics);
    }
    let Some(model) = analysis.model else {
        return Err(analysis.diagnostics);
    };
    let config = model.default_config().map(|c| c.name.clone());
    let solved = model.solve_sketches(config.as_deref())?;

    // (start, end, replacement) edits, applied back to front
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for item in &model.part.items {
        let ItemKind::Sketch(sk) = &item.kind else {
            continue;
        };
        let Some(s) = solved.iter().find(|s| s.name == item.name) else {
            continue;
        };
        for ent in &sk.entities {
            let Some(solved_ent) = s.entities.iter().find(|e| e.name == ent.name) else {
                continue;
            };
            if matches!(
                solved_ent.geometry,
                Geometry::Projected | Geometry::Spline { .. }
            ) {
                continue;
            }
            let text = hint_text(&solved_ent.hint_values());
            match ent.hint_span {
                Some(sp) => edits.push((sp.start, sp.end, text)),
                None => edits.push((ent.content_end, ent.content_end, format!(" {text}"))),
            }
        }
    }
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut out = src.to_string();
    for (start, end, text) in edits {
        out.replace_range(start..end, &text);
    }
    tcad_syntax::format_source(&out).map_err(|e| e.diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_rounded_and_have_no_negative_zero() {
        assert_eq!(number(7.071_067_811_865_475), "7.071067812");
        assert_eq!(number(-1e-12), "0");
        assert_eq!(number(25.0), "25");
        assert_eq!(number(-3.5), "-3.5");
    }
}
