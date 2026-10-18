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

### Changed
- Replaced the MIT `LICENSE` with Apache-2.0 (locked decision, requirements section 2).
