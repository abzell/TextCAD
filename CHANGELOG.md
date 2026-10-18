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

### Changed
- Replaced the MIT `LICENSE` with Apache-2.0 (locked decision, requirements section 2).
