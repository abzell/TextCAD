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

## M3: Model, expressions, units (2026-10-18)

CI for M2 was green at `938da73` (8 jobs, including Windows and the 5-minute fuzz run) after two
fixes to my CI config: wildcard path dependencies (`cargo-deny`) and the nightly toolchain for
`cargo fuzz` (the repo pins stable in `rust-toolchain.toml`).

Done:
- `tcad-expr`: dimensions (length, mass, angle), unit table, canonical storage, evaluator with
  dimension checking, `libm` for platform-independent trigonometry (ADR 0006).
- `tcad-model`: IR, CST lowering, scope analysis ("references only point backward", with
  did-you-mean hints), parameter evaluation with ranges, configurations (overrides, suppression,
  default), `suppress if`, enumerated arguments, one part per file; `Model::resolve`.
- `tcad check` runs the semantic checks and evaluates every configuration.
- Tests: 16 model tests, 7 randomized property tests for units/expressions, a 60k-iteration mutation
  run (2k in the normal run), fuzz target extended to semantic analysis.
- Spec: units, expressions, parameters; diagnostics table extended.

Deferred:
- Per-feature argument schemas (required arguments, expected dimensions, topology roles) arrive with
  the features in M5/M6; until then only names, units, ranges and the provisional enum words are checked.
- The golden corpus starts in M5, when geometry exists to compare against `expected.json`.

## M4: Sketch solver (2026-10-18)

CI for M3 was green at `484e961` (8 jobs).

Done:
- `tcad-solver`: exact Jacobians by forward-mode autodiff; Newton with backtracking and LM damping;
  min-weighted-norm steps through banded Cholesky (RCM ordering); clusters; pinned and weighted
  variables; conflict isolation (minimal sets); redundancy and per-entity DOF by envelope rank.
  A connected 200-entity sketch (1000 variables) solves in about 16 ms (release).
- Sketch bridge in `tcad-model`: entities (point, line, circle, arc in three forms, ellipse, spline,
  slot, rect, polygon, project placeholder), 20 constraint forms, hints, configurations; diagnostics
  name conflicting constraints with related locations.
- `tcad fmt --solve`; `info` severity; spec chapter `sketch.md`; ADR 0007.
- Corpus: six sketch-stress parts and a harness that checks diagnostics, DOF, coordinates and the
  200-entity timing.

Findings worth remembering (details in ADR 0007): CGLS was too slow on chains; the primal normal
equations drift along free directions, hence the dual form; tangency at a contact point is first-order
degenerate in its general form; authored coordinates must be stiffer than guessed ones.

Deferred:
- Splines in tangency/on-curve constraints; `project` (needs the kernel).
- The remaining ~34 corpus parts (primitives, features, naming robustness, hydraulics) arrive with
  the kernel (M5-M7).
