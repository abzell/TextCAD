# 0015. Assembly model: connectors, explicit orientation, lock file

- Status: proposed (Phase 2 draft; becomes accepted with the first Phase 2 implementation issue)
- Date: 2026-11-25

## Context

Phase 2 (requirements section 13) adds assembly files that reference parts "by path and version hash",
mates solved by a 3D assembly solver, skeletons, flexible sub-assemblies, configurations, exploded views,
BOM and interference checks, with a 300-part excavator gate. The format's goals (section 1) require that
the text be diffable, regenerate deterministically, and survive upstream edits. Commercial systems mate
raw faces and edges of parts and pick "the closest" solution; both break those goals.

## Options

1. **Mate raw faces and edges of parts** through persistent names (`rod.body.side(s.c1)`). Familiar, no
   extra part-side declarations. But every assembly depends on the internal feature history of every
   part; renaming a sketch entity in a part breaks the assembly; solving needs the kernel.
2. **Connectors owned by the part** (named frames, plus explicitly published faces and edges for the
   mates that need surfaces). One extra declaration per mating location; assemblies depend only on a
   part's public interface; frames are stored in the manifest so solving needs no kernel.
3. **Hashes in the assembly text** (`part("rod.tcad", hash="sha256:...")`) versus **a lock file**
   (`bundle.lock`). Inline hashes make every part edit a diff in every assembly that uses it; a lock file
   keeps that churn in one generated file, like `Cargo.lock`.
4. **Solution choice**: "closest to the current pose" (as interactive CAD does) versus explicit
   `aligned`/`opposed` flags with fixed defaults plus solver hints written by `tcad fmt --solve`.

## Decision

- Option 2: assemblies reference only connectors, published entities, datums and the implicit `origin`
  connector of a part. In-context references between parts are not supported; top-down design goes
  through a skeleton part that parts `use` read-only.
- Version pinning by `bundle.lock` (option 3, lock file), with `W6003`, and `E6003` under `--locked`. An
  optional `rev=` pins the human revision from the part's `meta`.
- Mate orientation is explicit with documented defaults; solved poses are written back as `@hint`, as
  for sketches (ADR 0007). The solver is a new crate `tcad-asm` that reuses the numerical approach of
  `tcad-solver` (Newton, LM damping, minimum-norm steps, conflict isolation, DOF by rank) on rigid
  bodies with quaternion orientation, using `libm` only, so poses are T1 across platforms.
- Assemblies get the diagnostics range `E6xxx`. The format version becomes `0.2`; `assembly`,
  `instance`, `mate`, `skeleton` and `bom` leave the reserved list.

## Consequences

- Every part that is used in an assembly needs connectors. `connector from=<hole>` and the implicit
  `origin` keep this cheap; the MCP server and LSP offer to create connectors from a face.
- Assemblies can be checked, solved, posed and BOM-listed in the browser and by agents from cached part
  manifests alone.
- Tangent and cam mates need published surfaces, which store an analytic description or a tessellation
  in the manifest; their accuracy is limited to the tessellation for free-form surfaces.
- Associative in-context modeling (a part's hole following a mating part's hole) is only possible via a
  skeleton; this is deliberate (references stay acyclic, requirements 4.3).
