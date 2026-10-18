//! Text and JSON rendering of diagnostics.

use crate::{Diagnostic, Severity, Span};
use serde_json::{Value, json};

/// Maps byte offsets to 1-based line and column numbers (columns count Unicode scalar values).
pub struct LineIndex<'a> {
    src: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    /// Indexes `src`.
    pub fn new(src: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(src.match_indices('\n').map(|(i, _)| i + 1));
        LineIndex { src, line_starts }
    }

    /// 1-based `(line, column)` of a byte offset (clamped to the text length).
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let mut offset = offset.min(self.src.len());
        while !self.src.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.line_starts.partition_point(|&s| s <= offset) - 1;
        let col = self.src[self.line_starts[line]..offset].chars().count() + 1;
        (line + 1, col)
    }

    fn line_text(&self, line: usize) -> &'a str {
        let start = self.line_starts[line - 1];
        let end = self
            .line_starts
            .get(line)
            .map_or(self.src.len(), |&e| e.saturating_sub(1));
        &self.src[start..end.max(start)]
    }
}

fn severity_name(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

/// Renders diagnostics for humans: `file:line:col: error[E1102]: message` plus the source line.
pub fn render_text(file: &str, src: &str, diags: &[Diagnostic]) -> String {
    let index = LineIndex::new(src);
    let mut out = String::new();
    for d in diags {
        let (line, col) = index.line_col(d.span.start);
        out.push_str(&format!(
            "{file}:{line}:{col}: {}[{}]: {}\n",
            severity_name(d.severity),
            d.code,
            d.message
        ));
        let text = index.line_text(line);
        let width = if d.span.end > d.span.start {
            let (end_line, end_col) = index.line_col(d.span.end);
            if end_line == line { end_col - col } else { 1 }
        } else {
            1
        };
        out.push_str(&format!(
            "  | {text}\n  | {}{}\n",
            " ".repeat(col - 1),
            "^".repeat(width.max(1))
        ));
        for rel in &d.related {
            let (l, c) = index.line_col(rel.span.start);
            out.push_str(&format!("  = note: {file}:{l}:{c}: {}\n", rel.message));
        }
        if let Some(hint) = &d.hint {
            out.push_str(&format!("  = help: {}\n", hint.message));
        }
    }
    out
}

fn span_json(index: &LineIndex<'_>, span: Span) -> Value {
    let (start_line, start_col) = index.line_col(span.start);
    let (end_line, end_col) = index.line_col(span.end);
    json!({
        "start": span.start,
        "end": span.end,
        "start_line": start_line,
        "start_col": start_col,
        "end_line": end_line,
        "end_col": end_col,
    })
}

/// Renders diagnostics as a JSON object `{"file": ..., "diagnostics": [...]}` for agents.
pub fn render_json(file: &str, src: &str, diags: &[Diagnostic]) -> Value {
    let index = LineIndex::new(src);
    let items: Vec<Value> = diags
        .iter()
        .map(|d| {
            let mut v = json!({
                "code": d.code,
                "severity": d.severity,
                "message": d.message,
                "span": span_json(&index, d.span),
            });
            if !d.related.is_empty() {
                v["related"] = json!(
                    d.related
                        .iter()
                        .map(|r| json!({"span": span_json(&index, r.span), "message": r.message}))
                        .collect::<Vec<_>>()
                );
            }
            if let Some(h) = &d.hint {
                v["hint"] = json!({
                    "message": h.message,
                    "edits": h.edits.iter().map(|e| json!({
                        "span": span_json(&index, e.span),
                        "replacement": e.replacement,
                    })).collect::<Vec<_>>(),
                });
            }
            v
        })
        .collect();
    json!({ "file": file, "diagnostics": items })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FixHint, codes};

    #[test]
    fn line_col_counts_chars() {
        let idx = LineIndex::new("ab\nçd\n");
        assert_eq!(idx.line_col(0), (1, 1));
        assert_eq!(idx.line_col(3), (2, 1));
        assert_eq!(idx.line_col(5), (2, 2));
        assert_eq!(idx.line_col(1000), (3, 1));
    }

    #[test]
    fn text_and_json() {
        let d =
            Diagnostic::error(codes::E1102, Span::new(4, 5), "expected `=`").with_hint(FixHint {
                message: "insert `=`".into(),
                edits: vec![],
            });
        let text = render_text("a.tcad", "x y z\n", std::slice::from_ref(&d));
        assert!(text.starts_with("a.tcad:1:5: error[E1102]: expected `=`\n"));
        assert!(text.contains("  |     ^\n"));
        assert!(text.contains("help: insert `=`"));
        let j = render_json("a.tcad", "x y z\n", &[d]);
        assert_eq!(j["diagnostics"][0]["code"], "E1102");
        assert_eq!(j["diagnostics"][0]["span"]["start_col"], 5);
        assert_eq!(j["diagnostics"][0]["severity"], "error");
    }
}
