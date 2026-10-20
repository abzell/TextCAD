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
| `fillet` | shape, edges with start and end radius | The edges rounded; faces next to them are trimmed and each edge gets a new face (corner patches where several meet). | radius too large for the geometry |
| `chamfer` | shape, edges with a reference face and two distances | The edges beveled: `d1` on the reference face, `d2` on the other. | distances too large |
| `shell` | shape, faces to open, thickness, outward flag | The solid hollowed with constant walls, inward unless `outward`. | thickness too large |
| `draft` | shape, faces, pull direction, angle, neutral plane | The faces tilted about their intersection with the neutral plane; a positive angle narrows the body along the pull direction. | a face cannot be drafted |
| `transform` | shape, translate / rotate / mirror | A copy moved or reflected. | |
| `topology` | shape | Every face (surface type, outward normal of planar faces, axis, radius, centroid, area, edges) and edge (curve type, ends, midpoint, length, radius, direction, adjacent faces), by index | |
| `lineage` | shape | Where the faces of the result came from, as recorded when the operation ran: `inherited` (result face, operand, input face; a face may come from several inputs when merged and an input may produce several faces when split) and `created` (result face, role: cap start/end, side of profile loop `lp` segment `seg`, from an input edge, from an input face). Empty for shapes read back from the cache. | |
| `export_step` | shape, part name, configuration, material (name, density) | STEP AP242 in millimetres (spec/io.md). Works on a copy: the shape is unchanged. | |
| `import_step` | bytes | The solids of the file as one shape without history. | not a STEP file, or no solids |
| `mesh` | shape, chordal and angular deflection | Triangles per face, tagged with the face index; counterclockwise from outside; normals per vertex. Works on a copy. | |
| `properties` | shape | volume, area, center of mass, inertia tensor **about the center of mass** (axes parallel to the world axes, unit density), bounding box, counts of solids, faces, edges, vertices | |
| `canonical_brep` / `from_brep` | shape / bytes | A serialization that is identical for identical shapes built by the same backend on the same platform. It is also the cache format. | malformed input |

Volume, area, center of mass and inertia are integrated to a relative precision of 1e-12 (the
OpenCASCADE default quadrature is only good to about 1e-6 on curved faces and differs between a shape and
its STEP round trip). The independent models in the corpus are measured the same way.

The modeling tolerance of the part (default 1e-6 mm) is the fuzzy value of booleans and the gap
within which sketch end points join into profiles.

Entities are identified by their **index** in the backend's enumeration of the result. The numbering
is deterministic (tier T1) and survives a cache round trip, but it is not part of the format:
persistent names (`spec/naming.md`) are built from the lineage, never from indices.

## Backend identity

A backend reports an id and version, stored in the cache manifest: `occt-7.6.3`, `mock-0.1`. A
cache entry is valid only for the backend that wrote it.

## Determinism tiers

| Tier | Applies when | Requirement |
| --- | --- | --- |
| T1 Identical | same backend id and version | the canonical serialization is bit-identical across runs on one platform, and the browser module reproduces the Windows build bit for bit (`corpus/brep-t1.txt`) |
| T2 Equivalent | different backend version or a different kernel | same topology counts per feature role; volume and area within 1e-6 relative; center of mass and bounding box within 10 times the modeling tolerance; Hausdorff distance below 10 times the modeling tolerance |
| T3 Failed | neither holds | `tcad verify` fails with a per-feature report |

No parallel reductions in geometry paths, fixed iteration orders, no hash-map iteration order in
outputs. The OpenCASCADE backend runs its boolean operations single-threaded.

How the tiers are tested: `crates/tcad-build/tests/corpus.rs` builds every corpus part three times and
compares the canonical B-rep byte for byte (T1), checks the SHA-256 of each against
`corpus/brep-t1.txt` (written by the Windows build, which must match it), and compares properties with
the analytic `expected.json` (T2). `tools/wasm/gate.mjs` checks the same hashes for the WebAssembly
build (docs/adr/0014). Other native platforms report their differences without failing: our own code
uses only portable arithmetic (the `libm` crate for every transcendental function, rotations passed to
OpenCASCADE as quaternions), but OpenCASCADE calls the platform's C library, and glibc rounds some
results differently from the Microsoft C runtime; the CI job `brep-compare` (tools/brep-diff.py) shows
the differing lines, which are rounding-level.
`corpus/tools/compare_reference.py` cross-checks the expected values with an independent build123d model.

## The mock backend

`mock-0.1` handles only closed-form cases: extruding a profile that is one circle or a closed chain
of straight lines (no holes, no draft). Everything else is an error. It exists for fast unit tests and
for builds without OpenCASCADE.

## In the browser

`crates/tcad-web` compiles the build for `wasm32-unknown-emscripten`. OpenCASCADE (the same version
as the native builds) is a separate WebAssembly side module, `occt.wasm`, that the main module loads
at start-up and that can be replaced by another build of the library (docs/adr/0014). The C interface
(`tcad_build`, `tcad_free`, `tcad_glb_ptr`, `tcad_glb_len`) returns JSON with the manifest, the
properties and the SHA-256 of the canonical B-rep, and the GLB.
