# Changelog

Changes to the TextCAD format, its specification and its conformance corpus. Format follows
[Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- Conformance cases for connectors (draft, #28): `con_hole_frames`, `con_explicit` and `con_revolution`
  with expected frames cross-checked against build123d, and eight invalid parts (`con_err_*`).
  `compare_reference.py` also compares connector frames.

### Clarified (draft)
- `spec/assembly.md` § `connector`: naming of single- and multi-position holes and skipped pattern
  instances; the order flip → rotate → offset and the sense of `rotate`; `origin_at` along the oriented
  axis; explicit directions keep their sign; diagnostics for wrong `from=`, zero directions and name
  clashes. Canonical argument order for `connector`.

### Changed
- The repository holds the format only. The specification no longer describes a particular program:
  command lines, caches, exports, the language server, the agent interface and the viewer were removed
  from `spec/` (the chapters `mcp.md` and `viewer.md` entirely), and the implementation and its tests
  were removed. `kernel.md` is now "Geometric semantics", with T2 as the conformance criterion between
  implementations; `bundle.md` keeps the layout, archive and source hash; `io.md` keeps only `import` (exports
  are a tool's concern); `diagnostics.md` defines validity and the standard codes;
  the requirements document is now about the format (section numbers unchanged).
- STEP is never required by the format; tools produce it on request.
- Phases may overlap; a phase is complete when its gate holds.

### Added
- Format 0.2: schema-evolution rules, keyword gating by the file header, `E0103`, migration rules
  (`spec/versioning.md`, ADR 0016).
- Part metadata (`meta`, `meta.<key>` overrides in configurations; `spec/features.md`); the hydraulic
  corpus parts and the rod clevis example carry part numbers.
- Draft Phase 2 specification: assemblies, connectors, mates, BOM (`spec/assembly.md`, ADR 0015).
- Roadmap (`docs/roadmap.md`).

## Format 0.1 (2026-10-19)

- Specification of single parametric parts: lexical structure, grammar, canonical form, diagnostics,
  reserved keywords, units, expressions, parameters and configurations, sketches and constraints,
  features (reference geometry, extrude, revolve, sweep, loft, holes, fillets, chamfers, shell, draft,
  rib, patterns, mirror, STEP import), persistent naming, geometric semantics, bundle layout.
- Conformance corpus of 43 parts (sketch stress, primitives, features, naming robustness with parameter
  sweeps, hydraulic components) with build123d reference models.
- ADRs 0001, 0004, 0006 and 0010.
