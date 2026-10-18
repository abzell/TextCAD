# Kernel contract and determinism

Status: **normative** (M5). Source requirements: 8. Implemented by `tcad-kernel` (the trait and the
analytic mock) and `tcad-occt` (OpenCASCADE).

The spec defines what each operation must produce. A backend is one implementation of that contract;
the format never encodes behavior specific to OpenCASCADE.

## Conventions

- Lengths in millimetres, angles in radians, unit density.
- A **frame** is a right-handed orthonormal triple (x, y, z = x cross y) with an origin. A sketch lies
  in the x/y plane of its frame. The base planes: `plane.XY` has x = X, y = Y, normal +Z; `plane.YZ`
  has x = Y, y = Z, normal +X; `plane.XZ` has x = X, y = Z, normal **-Y**.
- A **profile** is a planar face: an outer loop and optional holes in a frame. **Every loop runs
  counterclockwise** seen from the frame's normal, holes included; the kernel turns holes around.
  A loop is a chain of segments (line, arc given by start/middle/end, full circle, full ellipse,
  spline through or controlled by points) that share end points.
- A **path** is an open or closed chain of segments in a frame.

## Operations

| Operation | Inputs | Result | Failure |
| --- | --- | --- | --- |
| `extrude` | profile, unit direction, signed distance, draft | The profile swept along the direction by the distance (a negative distance sweeps the other way). A positive draft (radians) tapers the sides inward along the sweep; the distance is measured **along the direction**, not along the slanted sides. A draft needs the direction to be normal to the profile. | degenerate or open loops |
| `revolve` | profile, axis point and direction, angle | The profile turned about the axis; an angle of 2 pi or more is a full turn. | profile crosses the axis |
| `sweep` | profile, path, optional guide | The profile moved along the path. With a guide, the guide controls the orientation of the profile (auxiliary spine); a guided profile has no holes. | path not a chain; result not a solid |
| `loft` | two or more profiles (no holes), ruled flag | A solid through the sections in order: smooth, or ruled with straight sides. | incompatible sections |
| `boolean` | union / subtract / intersect, two shapes, modeling tolerance | The combined shape. Faces on the same surface are merged afterwards, so a union of two blocks with a common top has one top face. Disjoint results stay separate solids. | the operation fails |
| `properties` | shape | volume, area, center of mass, inertia tensor **about the center of mass** (axes parallel to the world axes, unit density), bounding box, counts of solids, faces, edges, vertices | |
| `canonical_brep` / `from_brep` | shape / bytes | A serialization that is identical for identical shapes built by the same backend on the same platform. It is also the cache format. | malformed input |

The modeling tolerance of the part (default 1e-6 mm) is the fuzzy value of booleans and the gap
within which sketch end points join into profiles.

## Backend identity

A backend reports an id and version, stored in the cache manifest: `occt-7.6.3`, `mock-0.1`. A
cache entry is valid only for the backend that wrote it.

## Determinism tiers

| Tier | Applies when | Requirement |
| --- | --- | --- |
| T1 Identical | same backend id and version, same platform build | the canonical serialization is bit-identical across runs |
| T2 Equivalent | different backend version or a different kernel | same topology counts per feature role; volume and area within 1e-6 relative; center of mass and bounding box within 10 times the modeling tolerance; Hausdorff distance below 10 times the modeling tolerance |
| T3 Failed | neither holds | `tcad verify` fails with a per-feature report |

No parallel reductions in geometry paths, fixed iteration orders, no hash-map iteration order in
outputs. The OpenCASCADE backend runs its boolean operations single-threaded.

How the tiers are tested: `crates/tcad-build/tests/corpus.rs` builds every corpus part twice and compares
the canonical B-rep byte for byte (T1), and compares properties with the analytic `expected.json` (T2).
`corpus/tools/compare_reference.py` cross-checks the expected values with an independent build123d model.

## The mock backend

`mock-0.1` handles only closed-form cases: extruding a profile that is one circle or a closed chain
of straight lines (no holes, no draft). Everything else is an error. It exists for fast unit tests and
for WASM builds before the in-browser kernel (Phase 1b).
