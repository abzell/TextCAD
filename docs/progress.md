# Progress

## M1: Repo and spec skeleton (2026-10-18)

Done:
- Cargo workspace (12 stub crates), pinned toolchain, workspace lints.
- CI: fmt, clippy (deny warnings), tests on Windows and Linux, WASM build, `cargo-deny`, docs, DCO check.
- Apache-2.0 `LICENSE`, `NOTICE`, `CONTRIBUTING.md`, `deny.toml`.
- ADR template and ADRs 0001-0003.
- Spec chapters stubbed in `spec/`; draft EBNF grammar in `spec/grammar.ebnf`.

CI: green on PR #1 (all 7 jobs) at `7bf55cc`. The six grammar open questions were settled in M2 (ADR 0004).

## M2: Parser and formatter (2026-10-18)

Done:
- `tcad-diag`: spans, severities, stable code registry, fix hints, text and JSON rendering (ADR 0005).
- `tcad-syntax`: total lexer, lossless CST, recursive-descent parser with statement-level recovery,
  canonical formatter (idempotent, comment-preserving), number normalization.
- CLI: `tcad check` and `tcad fmt [--check]`, `--json`, exit codes 0/1/2.
- Tests: lossless round trip, formatter idempotence, diagnostics per code, all 20 reserved keywords,
  200k-iteration mutation run (3k in the normal test run), libFuzzer target `fuzz/parse` (60 s locally,
  5 min in CI).
- Spec: lexical, canonical-form, diagnostics, reserved, versioning (header), grammar updated.
- ADRs 0004 (syntax decisions) and 0005 (diagnostics crate).

Not done / deferred:
- The 1-hour fuzz run required for the final Phase 1 acceptance (section 14) is scheduled for M8.
- The argument order table is provisional until M5/M6.
- `tcad check` is syntax-only until M3.
