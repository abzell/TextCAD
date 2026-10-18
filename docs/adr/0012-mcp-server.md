# 0012. MCP server design

Status: accepted (M8)

## Decision
- **Library plus thin binary.** `tcad-mcp` is a library (`Server<K: Kernel>`) with a stdio loop in
  `main.rs`, so tests drive it in process and hosts can embed it. The kernel is a type parameter.
- **Tools reuse the CLI.** `build`, `props`, `export` call `tcad_cli::run_with(kernel, args)` and return
  its `--json` output, so there is exactly one implementation of each command.
- **Edits are structured and validated.** `apply_edit` locates statements and arguments through the
  analyzed model (spans), applies the edits in memory, requires a clean analysis and, by default, a
  successful regeneration, and only then writes the canonical text. Nothing is written otherwise.
- **References are resolved by evaluating them.** `resolve_reference` inserts a probe `tag` at the
  requested point of an in-memory copy and reads what the build recorded for it, so tags, ordinals,
  queries and `edge(a & b)` behave exactly as in real statements.
- **Images** are rendered by a small software renderer in `tcad-io` (flat shading, depth buffer,
  face-boundary lines) and returned as PNG, so no GPU or browser is needed.
- **Roots.** All paths are confined to the server root.

## Consequences
The server cannot edit files outside the root and has no network access. New CLI commands become new
tools with little code. The explanation table in `explain.rs` must get an entry for every new
diagnostic code (a unit test lists the codes without one).
