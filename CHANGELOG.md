# Changelog

Changes to the TextCAD format, its specification and its conformance corpus. Format follows
[Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- Rules for assembly files (#29): resolving `part()`/`assembly()` paths (relative to the file,
  inside the bundle, or `lib:`), instances (`config=`, override blocks, name scope), reference paths
  into rigid and flexible sub-assemblies, and placement (`rotate=` in order about the fixed axes, then
  `at=`; nested placements composed); `ground` takes no arguments; `E6006`; canonical argument order
  for `instance`. Conformance cases `asm_placement` (cross-checked against build123d) and 15 invalid
  `asm_err_*` bundles (`tools/gen_assemblies.py`).
- Conformance cases for connectors (#28): `con_hole_frames`, `con_explicit`, `con_revolution`,
  `con_split_axis`, `con_mirror_holes` and `con_datum_hole` with expected frames cross-checked against
  build123d, and eight invalid parts (`con_err_*`).
- Conformance cases for `publish` (#28): `pub_surfaces`, `pub_edges`, `pub_split`, `pub_sphere`
  and `pub_torus` with entities cross-checked against build123d, and five invalid parts (`pub_err_*`).
- Connectors and published faces on the six hydraulic parts (pin joints, rod threads, ports, tie rods,
  mounting faces, bearing surfaces), cross-checked against their build123d models; the clevis also for
  its `heavy` configuration. `con_suppressed` and `con_err_connector_ref`.
  `compare_reference.py` also compares connector frames.
- `con_sweep_flange`, `con_sweep_pocket`, `con_sweep_shell`, `con_sweep_mirror`: the naming-robustness
  parts with connectors (on pattern instances, tagged faces, mirrored holes, chamfered bore walls),
  whose frames are listed for every swept parameter set (#28).

### Clarified
- `spec/features.md` § Published geometry: a tool may store a tessellation or sampled polyline of
  free-form entities at a resolution of its choosing (the fixed "modeling tolerance times 100" is
  removed); the accuracy that mates need is defined with the mates (#37).
- `spec/assembly.md` § `connector`: naming of single- and multi-position holes and skipped pattern
  instances; the order flip → rotate → offset and the sense of `rotate`; `origin_at` along the oriented
  axis; explicit directions keep their sign; diagnostics for wrong `from=`, zero directions and name
  clashes. Canonical argument order for `connector`.
- Connectors on a face of revolution split into pieces take all pieces (one axis) unless an ordinal
  picks one; `from=` accepts mirrors of holes (`.m1` original, `.m2` copy, nested outermost first);
  a hole on a datum plane gives a connector on the plane with z opposite to the drilling direction.
- `spec/assembly.md` § `publish`: faces or edges (not both), edge chains, split faces, the order of
  entities, and the kernel-independent parameters of each surface and curve type.
- Connectors and published entities that refer to a suppressed feature are suppressed with it
  (`I6014`); connectors may not refer to other connectors (`E5002`; share a frame through a `csys`).

### Changed
- Assembly files are normative (#29): `spec/assembly.md` § Files and bundles (without the version lock
  and libraries), § Assembly file for `meta`, `param`, `instance`, `ground`, `config` and `tag`,
  § Instances, § References and § Placement before solving. `assembly_decl`, `instance_decl` and
  `ground_decl` are in `grammar.ebnf`; `E6001`, `E6002`, `E6004`, `E6006` and `E6012` are in
  `spec/diagnostics.md`. The other assembly statements stay drafts.
- Connectors and published geometry are normative: their rules moved from `spec/assembly.md` (draft)
  to `spec/features.md` § Connectors and § Published geometry, unchanged. `connector_decl` and
  `publish_decl` are in `grammar.ebnf`; `E6011` and `I6014` are in `spec/diagnostics.md`; the
  versioning table lists both keywords as normative. `spec/assembly.md` keeps summaries (#28).
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
