# Diagnostics

Status: **normative** for the codes listed (M2); later milestones add codes. Source requirements: 4.7.

## Shape

Every diagnostic has:

- a **stable code**: `E` (error) or `W` (warning) and four digits. A code is never renumbered or
  reused; retired codes stay in the registry.
- a **severity**: `error` makes a command fail, `warning` does not.
- a **message** for humans.
- a **span**: a byte range in the source, reported with 1-based line and column of both ends
  (columns count Unicode scalar values).
- an optional **fix hint**: a message and a list of text edits (byte range plus replacement) that
  can be applied mechanically.

The registry lives in `crates/tcad-diag/src/codes.rs`; a test keeps it unique and ordered.

## Ranges

| Range | Area | Milestone |
| --- | --- | --- |
| E0xxx, W0xxx | General: unsupported features, versioning | M2 |
| E1xxx | Lexical and syntax | M2 |
| E2xxx | Types, units, parameters, expressions | M3 |
| E3xxx | Naming and references | M6 |
| E4xxx | Sketch solver | M4 |
| E5xxx | Kernel and geometry | M5 |

## Codes (M2)

| Code | Meaning |
| --- | --- |
| E0001 | feature not yet supported (a keyword reserved for a later phase) |
| E0101 | unsupported format major version |
| W0102 | format minor version is newer than supported |
| E1001 | unexpected character |
| E1002 | unterminated string |
| E1003 | unterminated block comment |
| E1004 | invalid escape sequence in string |
| E1005 | carriage return or byte order mark |
| E1006 | identifiers must be lowercase |
| E1101 | missing or malformed file header |
| E1102 | expected a specific token (hint: insert it, where that is unambiguous) |
| E1103 | unexpected token |
| E1104 | reserved word used as a name |
| E1201 | unknown statement |
| E1202 | unknown axis or plane name |
| E1203 | expected end of statement |
| E1204 | expected an expression |
| E1205 | unclosed block |

## Output

- **Text**: `file:line:col: error[E1102]: message`, the source line, a caret line and `= help: ...`.
- **JSON** (`--json`): `{"file": ..., "diagnostics": [{"code", "severity", "message", "span": {"start",
  "end", "start_line", "start_col", "end_line", "end_col"}, "hint"?: {"message", "edits": [{"span",
  "replacement"}]}}]}`. CLI commands wrap the per-file objects in `{"command", "ok", "files": [...]}`
  with a `status` per file.

## Recovery

The parser reports several errors per file. After a syntax error it skips to the end of the
statement (a line break outside nested braces) and continues, so later statements are still
checked. A file with any error is never formatted.
