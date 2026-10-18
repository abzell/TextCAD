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

## M5: kernel, OCCT, core solids, cache

Done:
- `Kernel` trait with mock and OpenCASCADE backends; canonical B-rep text; T1 (bit-identical across
  runs) and T2 (analytic/build123d, 1e-6 relative) verified on eight primitive parts.
- Features: reference geometry, extrude (distance, symmetric, through_all, to, draft, add/subtract/
  intersect), revolve, sweep (with guide), ruled and smooth loft.
- Bundles and `.cache/` with manifest and invalidation; `tcad build`, `tcad props`.

Deferred: guided loft (ThruSections has no guides), sketches on faces and topology maps (M6),
STEP/GLB export (M7), multi-body handling beyond union.

## M6: naming, detail features, patterns

Done:
- Kernel lineage and topology listing; fillet, chamfer, shell, draft and transforms in the OpenCASCADE
  backend (tests: `crates/tcad-occt/tests/detail.rs`).
- `tcad-naming`: IDs, aliases, ordinals, edge names, tags, queries, resolver errors with candidates.
- `tcad-build`: names threaded through extrude, revolve, sweep, loft and booleans; holes, fillets,
  chamfers, shell, draft, rib, patterns, mirror; sketches/planes/`to=` on faces; manifest topology map.
- `tcad refs`.
- Corpus: `feat_*` (11 parts) and `nam_*` (2 parts, 54 swept parameter sets), all at T2 against
  closed-form volumes and build123d, T1-deterministic.

Findings: OpenCASCADE reports no `Generated` face for a profile segment perpendicular to the axis of a
revolution (hence the geometric fallback); `ShapeFix_Face` may rebuild profile edges, so segments are
matched back to edges by midpoint; sweeping profile edges map cleanly through `Generated`.

Deferred: variable-radius fillet direction follows the edge orientation (documented, tested for
volume only); `rib` along arcs; vertex queries; loft guides.

## M7: I/O, properties, verify, diff

Done:
- Kernel: `export_step` (XDE, AP242, name/material/configuration), `import_step`, `mesh`; all on copies.
- `tcad-io`: binary STL, 3MF (stored ZIP, fixed date), GLB (one primitive per face with semantic IDs).
- `import`/`combine` in the build crate; the import bytes are part of the source hash.
- CLI: `export`, `verify`, `diff`; `props` prints mass and the inertia tensor in kg mm^2.
- STEP round trip (own importer) and independent reader (build123d) over all 21 corpus parts.

Finding: exporting or meshing the shape itself changed its canonical B-rep (tolerances, triangulation)
and broke the T1 comparison; both now work on a copy.

Deferred: the manual spot check in commercial CAD (docs/interop.md), STEP assemblies, GLB materials per
face. The hydraulic parts and FreeCAD opening in CI belong to M8.

## M8: tooling and the full corpus

Done:
- Corpus of 43 parts: guided loft, every extrude end condition, variable fillet, ruled loft, sweep,
  booleans, four more naming-robustness parts (up to 81 swept parameter sets), six hydraulic parts.
  `spec/examples/rod_clevis.tcad` is the clevis part and builds.
- `tcad-mcp`: 12 tools over the same crates as the CLI, structured validated edits, probe-tag
  reference resolution, software renderer; a scripted agent builds the rod clevis from an empty bundle
  and matches `expected.json` (spec/mcp.md, ADR 0012).
- `tcad-lsp`: diagnostics, completion, hover, definition, rename (verified by re-analysis), formatting,
  quick fixes, symbols; WASM-capable core with a provider for the persistent names (ADR 0013).
- `tcad serve` and `viewer/` (three.js, TypeScript): feature tree, source and model linked both ways,
  re-render on save; Chromium test (Playwright) in CI. `editors/vscode`: language client and viewer
  webview for VS Code and Theia; compile job in CI.
- Cross-platform determinism: `corpus/determinism.txt` digests the canonical text and solved sketches
  of every corpus part; checked on Linux, Windows and under WASI.
- CI: FreeCAD opens the six hydraulic STEP exports; release workflow (Windows zip and MSI, on a tag or
  by hand); one-hour parser fuzz workflow (weekly and by hand).

Findings: `LocOpe_DPrism` (drafted extrusion) is not deterministic across runs, so a drafted extrusion
is now a straight prism plus `DraftAngle`; OpenCASCADE's default volume quadrature differs by up to
4e-6 between a shape and its STEP round trip, so properties are integrated to 1e-12; holes on datum
planes no longer start 0.5 mm behind the plane; rename must treat `config` keys as references and
argument names as not.

Not done / needs a person:
- A tagged Windows release (acceptance criterion "published from a tagged build"): the workflow exists
  but no tag has been created; that is the maintainer's decision.
- The spot check of the hydraulic STEP files in a commercial CAD system (docs/interop.md).
- The Windows MSI definition (`packaging/windows/tcad.wxs`) has not run on a Windows runner yet; it is
  exercised by the first release workflow run.
- The viewer is not exercised inside a real VS Code or Theia window (the extension only compiles in CI).
