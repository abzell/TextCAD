# TextCAD Format — Requirements

Oct 18, 2026 · Adam (revised Nov 26, 2026: the format is separated from its implementations)

## 0. How this document is used

This document states what the **TextCAD format** must achieve. The normative definition is the
specification in `spec/`; this document is its source of requirements, and spec chapters cite its
section numbers. It says nothing about how any program implements the format: tools that read, check,
regenerate, view or export TextCAD files are separate products with their own requirements.

- **Spec first.** Every construct is defined in `spec/` (prose plus grammar) and covered by
  conformance cases in `corpus/` before tools are expected to support it.
- **Decision records.** Every non-trivial decision about the format is an ADR in `docs/adr/`.
  Decisions in section 2 are locked.
- **Ambiguity.** Where this document is silent, choose the most reasonable option, record an ADR and
  keep going.
- **Phases may overlap.** Work on a later phase may begin while an earlier one is open, as long as files
  valid in the earlier phase stay valid; keywords of later phases are reserved (section 4.8). A phase
  is complete when its gate (section 13) holds.

## 1. Vision, goals and non-goals

TextCAD is an open, human- and LLM-readable text format that carries full parametric design intent and
can, by Phase 5, replace proprietary native CAD formats (.prt, .sldprt, .ipt, Parasolid-native files)
for mechanical design.

**Goals**

1. Store the complete design recipe: parameters, sketches, constraints, feature tree, references,
   configurations.
2. Regenerate the same geometry in any conforming implementation (section 8).
3. Be diffable and mergeable in git with meaningful, line-oriented diffs.
4. Be easy for LLM agents to read, write, query and repair, with structured errors.
5. Interoperate through STEP AP242 so users never get stranded.
6. Be governed as an open standard: open specification, conformance suite, any number of
   implementations.

**Non-goals**

- Describing how a tool works (its user interface, caches, file exports, performance). Tools are
  separate products.
- Importing native proprietary files; revisit after Phase 5.
- Simulation (FEA/CFD), CAM toolpaths, ECAD.
- Bit-identical results across different implementations (see the determinism tiers, section 8).

## 2. Locked decisions

| Topic | Decision |
| --- | --- |
| Kernel independence | The format never encodes the behaviour of a particular geometry kernel |
| Syntax | Custom declarative DSL; no general-purpose code, guaranteed termination |
| Persistent naming | Hybrid: semantic IDs from history + user tags + geometric queries as fallback |
| On-disk layout | Directory bundle: one text file per part/assembly |
| Units | Explicit unit on every dimensional value; file declares modeling tolerance |
| Determinism | Tiered: identical within one implementation; property-equivalent across implementations |
| Verification | Conformance corpus with known properties, cross-checked against an independent modeler (build123d) |
| Separation | This repository holds the format only; implementations live elsewhere |
| Licensing | Open specification under Apache-2.0 |
| Ambiguity policy | Decide and record an ADR |

## 3. Scope of this repository

| Path | Content |
| --- | --- |
| `spec/` | The normative specification (Markdown and EBNF grammar) and examples |
| `corpus/` | The conformance corpus: source bundles with expected results and independent reference models |
| `docs/` | These requirements, the roadmap and the ADRs |

## 4. Language requirements

The language is a declarative description of a feature graph; it must always terminate, be statically analyzable, and have exactly one canonical text form.

**4.1 File header and versioning**

- Line 1 is `tcad <major>.<minor>`. Parsers reject unknown major versions and warn on newer minors.
- Schema evolution rules live in `spec/versioning.md`; every breaking change is published with the rules of its migration.

**4.2 Lexical rules**

- UTF-8, LF line endings, `#` line comments, `/* */` block comments. Comments are preserved by the formatter.
- Identifiers: `[a-z_][a-z0-9_]*`, unique within their scope. Display names are quoted strings.

**4.3 Declarative core**

- Top-level blocks: `part`, `param`, `sketch`, feature statements, `tag`, `config`. Order of declaration is the feature order.
- No loops, recursion, user functions or I/O. Repetition is expressed by pattern features. Conditionals are limited to `suppress if <bool expr>`.
- References may only point backward in the feature order (acyclic by construction).

**4.4 Values, units and expressions**

- Every length, angle, mass or density literal carries a unit: `25 mm`, `1.5 in`, `30 deg`, `0.5 rad`. Bare numbers are allowed only for dimensionless quantities (counts, ratios).
- Supported units Phase 1: mm, cm, m, in, ft, deg, rad, kg, g, lb, kg/m^3, g/cm^3.
- Expressions: + − × ÷ ^, parentheses, comparisons, `min max abs sqrt sin cos tan asin acos atan atan2 floor ceil round`. Unit mismatches are compile errors.
- The file declares `tolerance <value> <unit>` (default 1e-6 mm); this is the modeling tolerance of the geometry.

**4.5 Parameters and configurations**

- `param name = expr` with optional `range(lo, hi)` and `doc "…"`.
- `config name { param overrides; suppress list }` defines variants (family tables). One config is marked `default`.

**4.6 Canonical form**

- Every valid file has one canonical layout: fixed indentation (2 spaces), sorted key=value arguments in spec order, normalized numbers and units. Formatting twice is a no-op.
- Canonical form is what makes git diffs and merges meaningful; the examples and the corpus are in canonical form.

**4.7 Diagnostics**

- Every error has a stable code (`E1203`), a span, a message, and a machine-readable fix hint where possible, so that every tool, person and agent names a problem the same way.

**4.8 Reserved for later phases**

The grammar parses and rejects with a message (`feature not yet supported`) these keywords: `assembly`, `instance`, `mate`, `skeleton`, `bom`, `pmi`, `datum_feature`, `gdt`, `drawing`, `view`, `section`, `surface`, `boundary`, `fill`, `sheet_metal`, `flange`, `bend`, `flat_pattern`, `weldment`, `direct_edit`.

## 5. Bundle layout

A design is a directory: the `.tcad` text files are the source of truth.

```
rod_clevis/                 # bundle directory
  bundle.toml               # bundle layout version, default units, entry part, license, metadata
  parts/
    rod_clevis.tcad         # one part per file
  imports/
    bushing_vendor.step     # imported non-parametric solids (section 9)
```

- **Archive form.** A bundle may be zipped into a single `.tcadz` file.
- **Source hash.** SHA-256 of the canonical text plus imported file bytes plus resolved configuration;
  it identifies a source exactly (lock files, caches).
- **Derived data.** Tools may keep regenerated geometry, meshes and exports next to the sources; such
  data is never part of the format and never read as source. STEP is produced on request.

## 6. Persistent naming

Every reference to a face, edge or vertex must survive upstream parameter edits; this is the highest-risk requirement and gets its own spec chapter (`spec/naming.md`) and conformance parts.

**6.1 Semantic IDs (primary)**

- Every topological entity a feature creates gets an ID derived from history, not from kernel indices: `<feature>.<role>[(<generator>)]`.
- Roles are defined per feature in the spec. Examples: `boss.cap_end`, `boss.cap_start`, `boss.side(profile.r1.top)`, `fillet_1.face(boss.side(profile.r1.top) & boss.cap_end)`, `pin_hole.wall`.
- Entities split by later features get an ordinal suffix ordered by a kernel-independent rule (sorted by centroid along a declared axis, then by area): `boss.side(profile.c1)#2`.
- Entities merged by booleans keep the ID of the earliest contributing feature; others become aliases.

**6.2 User tags**

- `tag pin_bore = pin_hole.wall` binds a stable, human name. Tags are the recommended way for humans and agents to reference geometry used by several downstream features.

**6.3 Geometric queries (fallback)**

- `query(faces, created_by=boss, normal=+Z, nearest=(0mm,0mm,30mm))`, filters: `type` (plane/cylinder/cone/torus/bspline), `normal`, `parallel_to`, `axis`, `radius`, `created_by`, `adjacent_to`, `nearest`, `largest`, `smallest`.
- A query must resolve to exactly the expected count; zero or extra matches are errors (`E3xxx`) with the candidates listed.

**6.4 Resolution and failure**

- Resolution order: tag → semantic ID → query.
- If a reference no longer resolves, regeneration stops at that feature with a diagnostic that lists the nearest candidates and a suggested rewrite. It never silently picks one.

## 7. Phase 1 modeling features

Phase 1 must model typical machined and cast single parts (brackets, clevises, rod eyes, manifold blocks) entirely in text.

| Group | Required features |
| --- | --- |
| Reference geometry | Origin, base planes XY/YZ/XZ, offset/angled/through-3-points planes, axes, points, local coordinate systems |
| Sketch entities | Point, line, arc (3 forms), circle, ellipse, spline (interpolated + control-point), slot, rectangle, polygon, construction flag, projected/converted edges |
| Sketch constraints | Coincident, horizontal, vertical, parallel, perpendicular, tangent, equal, symmetric, concentric, midpoint, fix, point-on-curve; dimensions: distance, horizontal/vertical distance, radius, diameter, angle |
| Core solids | Extrude (blind, symmetric, to-face, through-all, two-sided, draft), revolve, sweep (with guide), loft (with guides), boolean add/subtract/intersect, multi-body |
| Detail features | Hole (simple, counterbore, countersink, tapped with thread spec), fillet (constant, variable), chamfer (distance, distance-angle), shell, draft, rib |
| Patterns | Linear, circular, mirror, pattern-of-pattern; instance skip lists |
| Parameters | Expressions, units, ranges, configurations, suppression |
| Properties | Material (name, density), mass, volume, area, center of mass, inertia tensor, bounding box |

Thread specs (e.g. `M20x2.5`, `1/2-13 UNC`, SAE O-ring ports) are stored as metadata on the hole; cosmetic threads only in Phase 1.

**7.1 Sketch constraints**

- Degrees of freedom are defined per entity; over- and under-constrained sketches are detectable, and a
  conflict is reported by naming the conflicting constraints.
- Solution stability: solving starts from the stored coordinates in the file, so small parameter
  changes produce the nearest solution, never a flipped one.
- Solved coordinates may be written back as hint values (`@hint`), making branch selection explicit in
  the text.

## 8. Geometric semantics and determinism

The spec defines what each operation must produce; no geometry kernel's behaviour is part of the format.

**8.1 Operations**

- The format's geometric operations (extrude, revolve, sweep, loft, boolean, fillet, chamfer, shell,
  draft, pattern transform, mass properties) are specified by inputs, result, output topology roles
  (for naming), tolerance handling and failure modes (`spec/kernel.md`).

**8.2 Determinism tiers**

| Tier | Applies when | Requirement |
| --- | --- | --- |
| T1 Identical | Same implementation, version and platform | Identical results every time |
| T2 Equivalent | Different implementations or versions | Same topology counts per feature role; volume, area, center of mass within tolerance; Hausdorff distance below 10× modeling tolerance |
| T3 Failed | Neither holds | Non-conforming |

- Property tolerances for T2: volume and area relative 1e-6, center of mass absolute 10× modeling
  tolerance.

## 9. Import and interchange

- **Import STEP:** `import body = step("imports/x.step")` brings in a dead solid; its faces get IDs
  `body.face#n` ordered by the kernel-independent rule in 6.1; usable as a boolean tool or reference.
- **Export:** the format recommends how part data maps to STEP AP242 (`spec/io.md`) so exports from
  different tools agree. Producing exports is a tool's job.

## 10. Requirements on tools (informative)

The format is designed so that tools can offer, without guessing: precise diagnostics with stable codes
and fix hints; a canonical formatter; completion and navigation by names; structured edits for agents;
and regeneration that never silently picks a different entity. Whether and how a tool does so is up to
the tool.

## 11. Conformance corpus

Geometry is accepted only when it matches independently computed properties.

**11.1 Corpus contents** (`corpus/`, each part = `.tcad` bundle + `expected.json`)

1. Primitives (8): block, cylinder, cone, sphere-by-revolve, torus, wedge, tube, hex prism. Expected
   properties computed analytically.
2. Sketch stress (6): fully constrained, under-constrained, over-constrained (must fail with named
   conflict), tangent chains, splines, 200-entity sketch.
3. Features (14+): each feature at least once, including every extrude end condition, holes of all four
   types, variable fillet, shell with removed faces, loft with guides, patterns with skips.
4. Naming robustness (6): parts whose upstream parameters are swept across a range while downstream
   fillets, holes and tags must keep resolving to the same semantic entities.
5. Hydraulic parts (6): cylinder rod clevis, rod eye with spherical-bearing bore, cylinder end cap with
   SAE O-ring port, tie-rod cylinder head, 4-port manifold block with cross-drilled galleries, pin with
   grease groove and cross-hole.

**11.2 Expected results per part**

- Volume, surface area, center of mass, bounding box and topology counts in `expected.json`; for
  sketches, solved coordinates, degrees of freedom and diagnostics.
- Cross-check: an independent build123d model in `corpus/<part>/reference.py` builds the same part, and
  the expected values are checked against it at T2.
- All sources are valid and in canonical form.

## 12. Licensing and contributions

- The specification, examples and corpus are licensed under Apache-2.0, with a `NOTICE` file and DCO
  sign-off on commits. Anyone may implement the format, in open or closed tools.

## 13. Roadmap

Each phase starts by extending the spec, then the conformance corpus. A phase is complete when its gate
holds.

1. **Phase 1 — Parts.** Sections 4–11. Gate: section 14.
2. **Phase 2 — Assemblies and BOM.** Assembly files referencing part files by path and version hash;
   instances with transforms; mates (coincident, concentric, distance, angle, parallel, tangent,
   gear/rack, slider, hinge, pin-slot); skeleton/top-down references; flexible sub-assemblies;
   configurations; exploded views; BOM with quantities and properties; interference check. Gate: the
   assembly chapter is normative and the conformance corpus holds independently computed results for
   every construct, including a 300-instance excavator boom-and-cylinder model.
3. **Phase 3 — PMI, GD&T, MBD.** Datums, datum targets, feature control frames per ASME Y14.5-2018 and
   ISO 1101, linear/angular tolerances, surface finish, notes, all tied to semantic IDs, with a mapping
   to semantic PMI in AP242. Gate: conformance parts for every PMI kind, and the AP242 mapping
   validated against an external checker.
4. **Phase 4 — 2D drawings.** Drawing files derived from parts/assemblies: views, sections, details,
   auxiliary views, dimensions and annotations linked to model IDs, title blocks, BOM tables, balloons.
   Gate: a conformance drawing per hydraulic part with no orphaned dimensions after a parameter sweep.
5. **Phase 5 — Surfacing and sheet metal.** Boundary, fill, ruled, offset and trim/extend surfaces,
   thicken, knit, curvature-continuous blends (G2); sheet metal base/edge/miter flanges, bends with
   K-factor and bend tables, forming tools, flat patterns; weldments with structural member profiles and
   cut lists. Gate: flat-pattern lengths in the corpus match analytic bend allowance within 0.01 mm;
   surface corpus defined at T2.

After Phase 5, evaluate: native-format import, direct editing, and conformance of a second independent
implementation.

## 14. Phase 1 completion criteria

- [ ] Spec chapters complete for every Phase 1 construct, with grammar, examples and naming roles
- [x] About 40 conformance parts with expected results, cross-checked against build123d at T2
- [x] Naming-robustness sweeps defined for the naming parts
- [x] Over-constrained and broken-reference cases with the expected named diagnostics
- [x] Examples and corpus in canonical form
- [ ] ADR log covers every non-trivial format decision

## 15. Illustrative syntax (non-normative)

This sketch shows the intended feel; the agent finalizes the grammar in `spec/` and may change details through ADRs, as long as the principles in section 4 hold.

```
tcad 0.1

part rod_clevis "Rod clevis, 20 mm pin" {
  tolerance 1e-6 mm
  material "AISI 1045" density=7.85 g/cm^3

  param pin_dia   = 20 mm   range(10 mm, 60 mm)
  param jaw_gap   = 26 mm
  param jaw_thk   = 16 mm
  param body_len  = 70 mm
  param thread    = "M24x2"

  sketch side on plane.XZ {
    rect   outline  corner=(0 mm, -(jaw_gap/2 + jaw_thk)) w=body_len h=jaw_gap + 2*jaw_thk
    circle pin      center=(body_len - 20 mm, 0 mm) d=pin_dia
    arc    nose     tangent_to=outline.right through=outline.top_right, outline.bottom_right
    constrain concentric(nose, pin)
  }

  extrude body from side.outline distance=40 mm symmetric
  extrude slot from rect(on=body.cap_end, w=jaw_gap, h=40 mm) depth=45 mm op=subtract

  tag jaw_inner_a = slot.side(+Y)
  tag jaw_inner_b = slot.side(-Y)

  hole pin_bore on=jaw_inner_a at=side.pin.center type=simple d=pin_dia end=through_all
  hole rod_thread on=body.cap_start at=origin type=tapped thread=thread depth=36 mm

  fillet jaw_round edges=query(edges, created_by=slot, parallel_to=axis.Y) r=2 mm
  chamfer bore_edges edges=[pin_bore.rim(jaw_inner_a), pin_bore.rim(jaw_inner_b)] d=1 mm

  config heavy { pin_dia = 25 mm; jaw_thk = 20 mm; thread = "M30x2" }
  config default standard { }
}
```

Points the example demonstrates: every dimension has a unit, references read as words (`slot.side(+Y)`, `pin_bore.rim(...)`), tags give stable handles, the one geometric query is a fallback, and configurations override parameters without duplicating features.
