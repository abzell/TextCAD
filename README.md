# TextCAD

An open, human- and LLM-readable text format that carries full parametric design intent
(parameters, sketches, constraints, feature tree, references, configurations).

Status: Phase 1 (M1-M8 implemented). The `tcad` command line (check, fmt, build, props, refs, export, verify,
diff, serve), the language server `tcad-lsp`, the MCP server `tcad-mcp`, the browser viewer and the
VS Code / Theia extension exist; see [`docs/progress.md`](docs/progress.md) for what is still open and
[`docs/building.md`](docs/building.md) to build with the OpenCASCADE kernel.

- Requirements: [`docs/TextCAD-Requirements.md`](docs/TextCAD-Requirements.md)
- Specification (normative): [`spec/`](spec/README.md)
- Decisions: [`docs/adr/`](docs/adr/)
- Progress: [`docs/progress.md`](docs/progress.md)

Licensed under Apache-2.0; see [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE).
