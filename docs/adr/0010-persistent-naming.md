# 0010. Persistent naming from operation history

Status: accepted (M6)

## Context
References to faces and edges must survive parameter edits (requirements 6). Kernel indices change
whenever a parameter changes, and "pick the face at this position" breaks when geometry moves.

## Decision
- **Names come from history.** Every kernel operation reports a *lineage*: which input faces each
  result face came from (`Modified`/`Generated`/unchanged in OpenCASCADE, and the face merge of
  `UnifySameDomain`) and which faces the operation created, with a role (cap, side of profile segment
  `(loop, seg)`, from an input edge, from an input face). `tcad-build` turns that into IDs
  `<feature>.<role>`; `tcad-naming` carries IDs through later features.
- **Primitives that report no history for a profile segment** (a segment perpendicular to the axis of a
  revolution) are located geometrically: the lowest-numbered face that contains three sample points
  of the segment.
- **Merged faces keep the earliest feature's ID**, the others become aliases (all resolve). **Split
  faces share an ID and get an ordinal** ordered by centroid (x, y, z) and area, rounded to 1e-6 mm,
  so the order never depends on the kernel.
- **Edges are named by their faces**: `edge(a & b)`. Edges need no history of their own, they follow
  from the face names, and the form matches the generator syntax of fillet faces
  (`round.face(a & b)`).
- **Tags re-resolve** from the reference they were bound to, so they follow merges and report splits
  instead of pointing at stale indices.
- **Failure is explicit**: unresolved is `E3001` with the nearest names and a fix hint, ambiguous is
  `E3002` with the ordinals, queries must match the expected count (`E3003`).
- **Patterns copy tools**: the tool solid of the source feature is transformed and combined again.
  Copies are named `<pattern>.i<k>.<role>` from the source's role text, so a pattern of a pattern
  nests prefixes.
- The mock kernel reports no topology; naming-dependent features then fail with `E5003`.

## Consequences
Every operation added to the kernel must report a lineage. A face that no rule names becomes
`<feature>.extra`; the corpus sweeps in `corpus/nam_*` catch regressions where names appear or
vanish. IDs are only as stable as the topology: a parameter range that changes the topology (a hole
that breaks through an edge) can legitimately change names, and the sweeps are limited to ranges
that do not.
