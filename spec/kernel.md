# Geometric semantics

Status: **normative** (format 0.1). Source requirements: 8.

This chapter defines what the geometric operations behind the features (spec/features.md) must
produce. It is independent of any geometry kernel: an implementation may use any B-rep modeler, and the
format never encodes behaviour specific to one.

## Conventions

- Lengths in millimetres, angles in radians.
- A **frame** is a right-handed orthonormal triple (x, y, z = x cross y) with an origin. A sketch lies
  in the x/y plane of its frame. The base planes: `plane.XY` has x = X, y = Y, normal +Z; `plane.YZ`
  has x = Y, y = Z, normal +X; `plane.XZ` has x = X, y = Z, normal **-Y**.
- A **profile** is a planar face: an outer loop and optional holes in a frame. **Every loop runs
  counterclockwise** seen from the frame's normal, holes included; the operation turns holes around.
  A loop is a chain of segments (line, arc given by start/middle/end, full circle, full ellipse,
  spline through or controlled by points) that share end points.
- A **path** is an open or closed chain of segments in a frame.

## Operations

| Operation | Inputs | Result | Failure |
| --- | --- | --- | --- |
| `extrude` | profile, unit direction, signed distance, draft | The profile swept along the direction by the distance (a negative distance sweeps the other way). A positive draft (radians) tapers the sides inward along the sweep; the distance is measured **along the direction**, not along the slanted sides. A draft needs the direction to be normal to the profile. | degenerate or open loops |
| `revolve` | profile, axis point and direction, angle | The profile turned about the axis; an angle of 2 pi or more is a full turn. | profile crosses the axis |
| `sweep` | profile, path, optional guide | The profile moved along the path. With a guide, the guide controls the orientation of the profile (auxiliary spine); a guided profile has no holes. | path not a chain; result not a solid |
| `loft` | two or more profiles (no holes), ruled flag, optional guides | A solid through the sections in order: smooth, or ruled with straight sides. | incompatible sections |
| `boolean` | union / subtract / intersect, two shapes, modeling tolerance | The combined shape. Faces on the same surface are merged afterwards, so a union of two blocks with a common top has one top face. Disjoint results stay separate solids. | the operation fails |
| `fillet` | shape, edges with start and end radius | The edges rounded; faces next to them are trimmed and each edge gets a new face (corner patches where several meet). | radius too large for the geometry |
| `chamfer` | shape, edges with a reference face and two distances | The edges beveled: `d1` on the reference face, `d2` on the other. | distances too large |
| `shell` | shape, faces to open, thickness, outward flag | The solid hollowed with constant walls, inward unless `outward`. | thickness too large |
| `draft` | shape, faces, pull direction, angle, neutral plane | The faces tilted about their intersection with the neutral plane; a positive angle narrows the body along the pull direction. | a face cannot be drafted |
| `transform` | shape, translate / rotate / mirror | A copy moved or reflected. | |

Every operation must also report, for naming (spec/naming.md), which faces of its result came from
which faces of its inputs and which faces it created, with their role (cap, side of a profile segment,
from an input edge, from an input face). How an implementation obtains that history is its own
business; the names it produces are not.

## Mass properties

Volume, area, center of mass and the inertia tensor about the center of mass (axes parallel to the
world axes) are properties of the exact result. Implementations integrate them to a relative precision
of at least 1e-9, so that differences between implementations stay well inside the T2 tolerances
below.

## Modeling tolerance

The modeling tolerance of the part (default 1e-6 mm, spec/units.md) is the fuzzy value of booleans and
the gap within which sketch end points join into profiles.

## Conformance and determinism

Two results of the same source and configuration are compared in tiers:

| Tier | Requirement |
| --- | --- |
| T1 Identical | the exact results are identical. **Required of one implementation**: the same implementation, version and platform must produce the identical result every time (no parallel reductions, fixed iteration orders, no hash-map order in outputs). Implementations should also reproduce it across platforms. |
| T2 Equivalent | same topology counts per feature role; volume and area within 1e-6 relative; center of mass and bounding box within 10 times the modeling tolerance; Hausdorff distance below 10 times the modeling tolerance. **This is the conformance criterion between different implementations**, and the one the conformance corpus (`corpus/`) uses. |
| T3 Failed | neither holds |

Kernel indices (the order in which a modeler enumerates faces and edges) are never part of the format:
persistent names (spec/naming.md) are built from history and from kernel-independent ordering rules.
