# MCP server

Status: **normative** (M8). Source requirements: 10.4. Implemented by `tcad-mcp`.

`tcad-mcp [--root DIR]` is a [Model Context Protocol](https://modelcontextprotocol.io) server on
standard input and output (JSON-RPC 2.0, one message per line). It serves the bundles below the root;
every path argument is relative to it and may not be absolute or contain `..`. It is a thin layer over
the same crates as the CLI, so a tool behaves exactly like the matching `tcad` command.

Every tool returns `structuredContent` (the data below) plus the same data as text, and `isError`.
Errors carry `error` and, where there are any, `diagnostics` in the format of `tcad --json`.

| Tool | Arguments | Result |
| --- | --- | --- |
| `create_part` | `name`, `title?`, `source?` | creates bundle `<name>/`; `source` is validated before it is written |
| `read_part` | `path` | source, canonical text, parameters, statements with lines, configurations, diagnostics |
| `check_part` | `path` | diagnostics; an error result if there are errors |
| `apply_edit` | `path`, `edits`, `validate_geometry?` | applies the edits and writes the canonical result, or rejects it |
| `set_param` | `path`, `name`, `value` | changes a parameter's expression (an `apply_edit` with `set_param`) |
| `build` | `path`, `config?`, `force?` | what `tcad build --json` returns |
| `get_properties` | `path`, `config?` | what `tcad props --json` returns: mass, volume, area, center, inertia, box |
| `query_topology` | `path`, `config?`, `kind?`, `feature?`, `contains?`, `geometry?` | persistent names, types, sizes and positions of faces and edges |
| `resolve_reference` | `path`, `reference`, `after?`, `config?` | what a face/edge reference denotes at that point, or why it does not resolve |
| `export` | `path`, `to`, `out?`, `config?` | `tcad export`; default `<bundle>/exports/<part>.<ext>` |
| `render_view` | `path`, `view?`, `size?`, `highlight?`, `config?` | a PNG as an MCP `image` plus its description |
| `explain_error` | `code` | the code's title, meaning and how to fix it |

## Edits

`apply_edit` edits are structured and located through the analyzed model, never by searching text:

| `op` | Fields |
| --- | --- |
| `add_statement` | `text`, `after?` or `before?` (a statement name; default: the end of the part) |
| `replace_statement` | `name`, `text` |
| `remove_statement` | `name` |
| `set_arg` | `name`, `entity?` (a sketch entity), `arg`, `value` |
| `remove_arg` | `name`, `entity?`, `arg` |
| `add_param` | `name`, `value`, `range?`, `doc?`, `after?` |
| `set_param` | `name`, `value` |
| `add_to_sketch` | `sketch`, `text` |

The edits apply in order. The result must analyze without errors and, unless `validate_geometry` is
false, must regenerate (so references still resolve and features still build). If any step fails, the
call is an error with `rejected: true`, the diagnostics, and **the files are untouched**. A valid result is
written in canonical form.

## Rendering

`render_view` draws the result from one of the views `iso`, `front`, `back`, `left`, `right`, `top`,
`bottom` (orthographic, flat shaded, face boundaries as lines). Faces whose persistent name (or alias)
contains one of the `highlight` strings are orange.

## Test

`crates/tcad-mcp/tests/clevis.rs` is a scripted agent that builds the rod clevis of
`corpus/hyd_rod_clevis` from an empty bundle using only these tools, including a rejected edit, and
checks the result against the corpus properties at tier T2.
