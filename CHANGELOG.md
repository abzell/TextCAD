# Changelog

All notable changes are recorded here. Format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- M1: Cargo workspace with stub crates, GitHub Actions CI, `cargo-deny` policy, Apache-2.0 licensing
  (`LICENSE`, `NOTICE`, DCO), ADR template and log, normative spec skeleton in `spec/` with a draft EBNF grammar.
- Requirements specification in `docs/TextCAD-Requirements.md`.

- M2: `tcad-diag` (diagnostics with stable codes, spans, fix hints, text and JSON output) and
  `tcad-syntax` (lexer, lossless concrete syntax tree, error-recovering parser, canonical formatter);
  `tcad check` and `tcad fmt [--check]` with `--json`; normative spec chapters for lexical structure,
  canonical form, diagnostics, reserved keywords and the header; parser fuzz target and 5-minute CI
  fuzz smoke run; ADRs 0004 and 0005; the section 15 example as `spec/examples/rod_clevis.tcad`.

- M3: `tcad-expr` (expression AST, dimension and unit algebra, deterministic evaluation) and
  `tcad-model` (typed IR, lowering from the syntax tree, scope/unit/range/configuration checks,
  `Model::resolve`); `tcad check` now performs the full semantic check; diagnostics E2xxx; spec
  chapters for units, expressions and parameters; ADR 0006; randomized property and mutation tests.

- M5: `tcad-kernel` (Kernel trait, mock backend), `tcad-occt` (OpenCASCADE via cxx, dynamic linking),
  `tcad-build` (reference geometry, extrude, revolve, sweep, loft, booleans), `tcad-cache` (bundles,
  `.cache/`, source hashing), `tcad build` and `tcad props`, eight primitive corpus parts with analytic
  expectations and a build123d cross-check, spec chapters kernel/bundle/features, ADRs 0008-0009,
  `docs/building.md`, OCCT and cross-check CI jobs.
- M6: persistent naming (`tcad-naming`: semantic IDs from operation history, aliases for merged faces,
  ordinals for split faces, `edge(a & b)`, tags, geometric queries, `E3xxx` diagnostics with nearest
  names and fix hints), kernel topology listing and lineage, `hole` (simple, counterbore, countersink,
  tapped), `fillet` (constant and variable), `chamfer`, `shell`, `draft`, `rib`, `linear_pattern`,
  `circular_pattern`, `mirror` (including patterns of patterns and skip lists), sketches and `to=` on
  planar faces, the topology map in the manifest, `tcad refs`, thirteen corpus parts (two with
  naming-robustness sweeps over 54 parameter sets), `spec/naming.md`, ADR 0010.
- M7: STEP AP242 export (name, material, configuration) and import as a dead solid with
  `import`/`combine`, binary STL, 3MF and GLB (per-face semantic IDs), `tcad export`, `tcad verify`
  (T1/T2/T3), `tcad diff`, mass and inertia tensor in `tcad props`, `result.step` and `mesh.glb` in
  the cache, `tcad-io`, STEP round trip and independent-reader checks over the corpus,
  `spec/io.md`, `docs/interop.md`, ADR 0011.
- M8: 43 corpus parts including six hydraulic components and guided lofts (`loft ... guides=`),
  a deterministic drafted extrusion, properties integrated to 1e-12, a Windows OpenCASCADE CI job;
  `tcad-mcp` (MCP server, 12 tools, software renderer); `tcad-lsp` (language server); `tcad serve` and
  the three.js `viewer/`; the VS Code / Theia extension in `editors/vscode`; bit-exact determinism
  digests checked on Linux, Windows and WASI; FreeCAD import check; release (Windows zip, MSI) and
  one-hour fuzz workflows; `spec/viewer.md`, `spec/mcp.md`, ADRs 0012 and 0013.

- Phase 1b: in-browser geometry. `crates/tcad-web` (the build as a WebAssembly module with
  OpenCASCADE 8.0.1 as a replaceable side module), the viewer's in-browser mode (`viewer/web`), the
  T1 gate against Windows (`corpus/brep-t1.txt`, `tools/wasm/gate.mjs`), `third_party/musl-math` (one
  implementation of the transcendental functions on Windows and in the browser), cross-platform
  B-rep comparison (`tools/brep-diff.py`), ADR 0014.
- CI: the language server inside VS Code and Eclipse Theia (job `editors`).
- Format 0.2 (#13): schema-evolution rules in `spec/versioning.md`; keywords gated by the file
  header (`meta`, `connector`, `publish`, `uses` are names in 0.1 files and keywords from 0.2 on,
  not implemented yet: `E0001`); `E0103` for a newer construct under an older header, with a fix
  hint; `tcad migrate [--to] [--check] [--json]` (`tcad_model::migrate`) rewrites the header and
  renames clashing names, verified to keep every configuration's meaning; ADR 0016.
- Part metadata (#9, format 0.2): `meta number= revision= kind=made|purchased|phantom vendor=
  vendor_number= mass= com= color=` and `meta.<key> = ...` in configurations
  (`spec/features.md` § Part metadata); stored in the manifest (`meta`); STEP product id, version,
  description and colour; 3MF `PartNumber`/`Revision` and display colour; GLB mesh extras and base
  colour; `tcad props` uses `meta mass`/`com` and reports the computed values alongside; LSP
  completion; MCP `read_part` returns it and `apply_edit` has `set_meta`. The six hydraulic parts
  and `spec/examples/rod_clevis.tcad` are format 0.2 with part numbers.
- Draft Phase 2 spec (`spec/assembly.md`, ADR 0015) and the plan of the remaining work
  (`docs/phase2-plan.md`).

### Changed
- STEP is generated only on request (`tcad export --to step`, MCP `export`): `tcad build` no longer
  writes `.cache/<part>/result.step` and deletes one left by an older version (`spec/io.md`,
  `spec/bundle.md`, requirements §5).
- Phases may start before the previous phase's gate passes (`docs/TextCAD-Requirements.md` §0, §13);
  items that need a person are tracked in issue #90.
- The newest format version is 0.2: a missing header is fixed with `tcad 0.2` and the MCP server's
  `create_part` writes 0.2. Files keep the version they declare; `tcad fmt` never changes it.
- Replaced the MIT `LICENSE` with Apache-2.0 (locked decision, requirements section 2).
