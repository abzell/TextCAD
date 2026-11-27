# Assemblies

Status: **draft** (Phase 2 of the format). Source requirements: 13 (Phase 2). Decisions: ADR 0015.
Sections become *normative* one by one as they are reviewed and covered by conformance cases; changes
to this text are recorded in ADRs. The roadmap is in [`../docs/roadmap.md`](../docs/roadmap.md).

An assembly places instances of parts and other assemblies relative to each other and records the design
intent behind the placement: which frames meet, what may still move, which values drive the pose. Like
a part, it is a recipe: the resolved transforms are derived from it and are not part of the source
(apart from optional solver hints).

## Principles

1. **Parts publish, assemblies consume.** A part declares named **connectors** (coordinate frames) and
   **published** faces or edges. An assembly may refer only to those, to the part's datums
   (`plane`, `axis`, `point`, `csys`) and to its implicit `origin` connector. It never refers to a raw
   face, edge or kernel index of a part. A part edit that keeps its connector names never breaks an
   assembly.
2. **Everything resolves to a transform.** Every instance ends with one rigid transform relative to the
   assembly origin. Mates, joints, patterns and fastener stacks are intent that the assembly solver turns
   into those transforms.
3. **Determinism.** The same text gives the same transforms. Where a mate allows several solutions, the
   text chooses one with an explicit orientation flag; a solver never picks "the closest" by itself.
   Tools may write solved poses back as `@hint(...)`, as for sketches.
4. **Interfaces are enough to solve.** Mates use only the connectors and published geometry of the
   parts. An implementation can therefore check, solve, pose and list an assembly (BOM, mass) from each
   part's interface data and properties, without regenerating geometry in the assembly. Only geometry
   that the assembly creates itself (assembly features, routed parts) and geometric checks
   (interference, clearance) need the parts' regenerated shapes.
5. **References point backward.** An assembly may reference parts, sub-assemblies and its skeleton; a
   part may reference a skeleton; nothing may reference the assembly that contains it (`E6004`).

## Files and bundles

```
excavator_boom/
  bundle.toml                 entry = "assemblies/boom.tcad"
  bundle.lock                 source hashes of every referenced file (written by tools)
  parts/        *.tcad         one part per file
  assemblies/   *.tcad         one assembly per file
  skeletons/    *.tcad         layout parts (optional)
  library/                    standard parts, or a path to a shared library bundle (see "Libraries")
```

- A `.tcad` file holds exactly one `part` **or** one `assembly` (`E2701` is extended to say so).
- Files that use any construct of this chapter declare `tcad 0.2` (spec/versioning.md).
- `bundle.toml` gains `[libraries]`: a table of name to path (a directory or a `.tcadz` archive),
  so `part("lib:iso4762.tcad")` resolves without relative paths.

### Version lock

The requirement "reference part files by path and version hash" is met by **`bundle.lock`**, not by
hashes in the source text, so that editing a part does not rewrite every assembly that uses it:

```toml
format = "0.1"
[files]
"parts/rod.tcad"       = "sha256:2f1c..."   # source hash (spec/bundle.md) of the canonical text
"lib:iso4762.tcad"     = "sha256:9ab0..."
```

- Tools write it on request. A checker reports `W6003` for every referenced file whose current source
  hash differs from the lock (or that the lock does not list), and `E6003` in a strict mode meant for
  release builds.
- An instance may also pin a revision from the part's `meta` (`rev="B"`); a different revision in the
  file is `E6005`.

## Part side: metadata, connectors and published geometry

These statements are added to **parts** (spec/features.md links here).

### `meta`

**Normative in [features.md](features.md) § Part metadata** (implemented, #9); summary:

```
meta number="HC-110-014" revision="B" kind=made vendor="" mass=0.84 kg color=(0.7, 0.7, 0.72)
```

| Key | Value | Use |
| --- | --- | --- |
| `number` | string | part number (BOM, STEP product id) |
| `revision` | string | revision (BOM, STEP product version, `rev=` pins) |
| `kind` | `made`, `purchased`, `phantom` | BOM classification (default `made`) |
| `vendor`, `vendor_number` | string | purchased parts |
| `mass` | mass | overrides the computed mass (purchased parts with simplified geometry) |
| `com` | `(x, y, z)` | overrides the center of mass, only together with `mass` |
| `color` | `(r, g, b)` in 0..1 | display colour (viewer, GLB, STEP presentation) |

The part's description is the string after its name (`part rod "Piston rod, 50 mm"`). `meta` may
appear once (`E2303`). Configurations may override `meta` keys: `config heavy { meta.number = "HC-110-015" }`.

### `connector`

A connector is a named right-handed frame (origin, x, y, z) owned by the part. It is evaluated after the
last feature, from references resolved by persistent naming (spec/naming.md), and stored in the
manifest per configuration.

```
connector pin_axis from=pin_bore                 # hole: origin at the entry, z out of the material
connector mount    origin=base.cap_start z=base.cap_start x=+X
connector rod_end  origin=(0 mm, 0 mm, rod_len) z=+Z
connector bore     origin=barrel.side(s.c1) z=barrel.side(s.c1) origin_at=start
connector seat     csys=seat_csys rotate=90 deg offset=(0 mm, 0 mm, 2 mm)
```

| Argument | Accepts | Meaning |
| --- | --- | --- |
| `from=` | a `hole` feature, or a pattern or mirror of one (also nested) | one connector per hole position: origin at the centre of the hole's entry (top of a counterbore or countersink; for a hole on a datum plane, the point on the plane), z **opposite to the drilling direction** (out of the material for a hole on a face without `flip`; for a hole on a datum plane, the plane's normal, reversed by `flip`), x by the rule below. A hole with one position gives the connector `<name>` itself; several positions give `<name>.p1`, `<name>.p2`, ... (and no bare `<name>`). A pattern gives `<name>.i<k>` for every instance that is not skipped, numbered as the pattern numbers them; a mirror gives `<name>.m1` (its `of=`, the original) and `<name>.m2` (the mirrored copy). Nested repetitions give one suffix per level, outermost first, then the position: `<name>.m2.i3.p1`, as semantic IDs do (spec/naming.md). A mirrored copy's connector is built like any hole's from the mirrored position and drilling direction, so it stays right-handed. |
| `csys=` | a `csys` | copies its frame |
| `origin=` | `(x, y, z)`, a `point`, a circular edge (its centre), a planar face (its centroid), a cylindrical, conical, spherical or toroidal face (a point on its axis or its centre; see `origin_at`), a sketch point | the origin |
| `z=` | a direction (`+Z`, `(0, 1, 1)`), an `axis`, a planar face (its outward normal), a face of revolution or circular edge (its axis) | the z axis |
| `x=` | a direction or an `axis` | the x axis, projected into the plane normal to z |
| `origin_at=` | `start`, `middle`, `end` | for a face of revolution: where along the axis (default `start`, the face's lowest point along z) |
| `rotate=` | angle | turns x and y about z after everything else |
| `offset=` | `(x, y, z)` | moves the origin in the connector's own frame |
| flag `flip` | | turns the frame 180 deg about its x axis (z and y reverse) |

Rules, so that no frame depends on the kernel:

- An axis taken from a face of revolution or circular edge has no inherent sign. It is oriented so that
  its first component that is not zero (x, then y, then z, with tolerance 1e-9) is positive; use `flip`
  or give `z=` as a direction to choose the other sense. A direction given explicitly (`z=(0, 0, -1)`)
  keeps its sign; directions need not be unit length.
- `origin_at` is measured along that oriented axis: `start` is the face's lowest point along it.
- A face of revolution that later features split into pieces (`pin_bore.wall#1`, `#2`) may be given
  without an ordinal in `origin=` and `z=`: it then means all of its pieces, which must share one axis
  (within the modeling tolerance; otherwise `E3002` as for any ambiguous reference), and `origin_at`
  is measured over all of them. With an ordinal it means that piece alone.
- Without `x=`, x is `world Z cross z` normalized, or world X when z is parallel to world Z (the hole
  frame rule). `x=` is projected into the plane normal to z and normalized; y is `z cross x`.
- The frame is built in this order: the base frame (from `from=`, `csys=`, or `origin=`/`z=`/`x=`);
  then `flip` (180 deg about x: y and z reverse, the origin stays); then `rotate` (about the current z,
  by the right-hand rule: positive turns x towards y); then `offset`, measured along the resulting axes.
- `origin=` and `z=` are required unless `from=` or `csys=` is given; `from=` and `csys=` exclude
  `origin=`, `z=` and `x=` and each other. A missing or contradictory argument is `E5001`; a `from=`
  that is not a hole or a pattern of one is `E5002`; x parallel to z, or a zero direction, is `E6011`.
- Connector names share the part's name scope with parameters, sketches, features, datums and tags
  (`E2101` for a clash).
- A connector refers to geometry, datums, points and holes, never to another connector or to a
  published name (`E5002`). To share a frame, declare a `csys` and use it in both connectors.

**Suppression.** A connector or `publish` whose references name a feature that is suppressed in the
configuration being built (by the configuration's `suppress` list or by the feature's `suppress if`,
directly or through a tag) is suppressed with it. It is left out of that configuration's interface and
reported with the information `I6014`, not an error; an assembly that uses it in that configuration
follows the rules for suppressed references (`I6202`). A connector can also be suppressed on its own
(`suppress if`).

The conformance cases `corpus/con_*` cover these rules.

Every part has the implicit connector `origin` (the part frame) and its datums are visible to
assemblies under their own names (`plate.plane.XY`, `plate.mid_plane`, `plate.pin_axis_a`).

### `publish`

```
publish cam_lobe  = cam.side(lobe.s1)
publish roller    = roller_body.side(r.c1)
publish slot_path = edge(slot.side(g.top) & base.cap_end)
```

Makes a face, a set of faces or an edge chain of the part visible to assemblies, for mates that cannot
be written with frames (`tangent`, `cam`, `path`) and for named interference checks. Like connectors,
published entities are evaluated after the last feature, and their names share the part's name scope
(`E2101`).

- The references are faces or edges, not both (`E3005` for a mix). A face that later features split
  into pieces, given without an ordinal, means all its pieces, as for connectors.
- Edges must form one connected chain, each edge sharing an end point with the next in the order
  written (`E5002` otherwise).
- The entities are listed in the order of the references and, within one reference, in the order of
  their ordinals (`#1`, `#2`, spec/naming.md).

Each entity is described in the part frame by its type and, where it has them, analytic parameters
that do not depend on the kernel:

| Type | Parameters |
| --- | --- |
| `plane` | `normal` (outward unit normal), `point_mm` (the face's centroid), `area_mm2` |
| `cylinder` | `axis` (oriented by the sign rule of connectors), `axis_point_mm` (the point of the axis nearest the part origin), `radius_mm`, `span_mm` (`[start, end]` of the face along the axis, measured from `axis_point_mm`) |
| `cone` | `axis` (sign rule), `apex_mm`, `half_angle_deg`, `span_mm` (along the axis, from the apex) |
| `sphere` | `centre_mm`, `radius_mm` |
| `torus` | `centre_mm`, `axis` (sign rule), `major_radius_mm`, `minor_radius_mm` |
| `bspline` (any other surface) | `area_mm2` |
| `line` | `ends_mm` (the two end points) |
| `circle` (also an arc) | `centre_mm`, `axis` (sign rule), `radius_mm`, and `ends_mm` for an arc |
| `ellipse`, `bspline` (curves) | `length_mm` |

A face of revolution is described by its geometry, not by how it was made: a sphere made by revolving
an arc is a `sphere`. Tools may store more (a tessellation and a sampled polyline at the modeling
tolerance times 100 for free-form entities); conformance checks only the parameters above, at tier T2
for lengths and points and to 1e-9 for unit vectors. The conformance cases `corpus/pub_*` cover these
rules.

## Assembly file

```
tcad 0.2

assembly boom_cylinder "Boom cylinder, 110 bore, 900 stroke" {
  meta number="HC-110-900" revision="A"

  param stroke    = 900 mm
  param extension = 450 mm range(0 mm, 900 mm) doc "Rod extension, the driving value"

  instance barrel = part("parts/barrel.tcad")
  instance gland  = part("parts/gland.tcad")
  instance rod    = part("parts/rod.tcad") config=chromed
  instance eye    = part("parts/rod_eye.tcad")
  instance piston = part("parts/piston.tcad") { bore = 110 mm }

  ground barrel

  mate gland_seat fastened(gland.seat, barrel.head_end)
  mate rod_slide  slider(barrel.bore_axis, rod.axis) drive=extension min=0 mm max=stroke
  mate piston_on  fastened(piston.rod_seat, rod.piston_end)
  mate eye_thread fastened(eye.thread_seat, rod.thread_end) rotate=0 deg

  config retracted default { extension = 0 mm }
  config extended          { extension = stroke }
}
```

### Statements

| Statement | Form |
| --- | --- |
| `meta` | as for parts (`kind=phantom` makes the assembly transparent in the BOM) |
| `param` | as for parts; assembly parameters may be passed to instances and drive joints |
| `instance` | `instance <name> = part("<path>") \| assembly("<path>") \| skeleton("<path>")`, then optional `config=<name>`, `rev="<revision>"`, `at=(x, y, z)`, `rotate=[(axis, angle), ...]`, flag `flexible` (sub-assemblies), `bom=made\|purchased\|phantom\|exclude`, `suppress if <expr>`, an optional override block `{ param = expr; ... }` and an optional `@hint(...)` |
| `ground` | `ground <instance> [at=(x, y, z)] [rotate=[...]]`: fixes the instance (default: at the assembly origin). At least one instance is grounded (`E6012` otherwise); the first instance is **not** grounded implicitly. |
| `mate` | `mate <name> <kind>(<ref>, <ref>[, ...]) [= <value>] { <arg> } [suppress if <expr>]` (below) |
| `pattern` | component patterns (below) |
| `mirror_instance` | mirrored components (below) |
| `config` | as for parts, plus instance configuration overrides and instance suppression (below) |
| `bom` | BOM options (below) |
| `check` | interference and clearance checks (below) |
| `fastener` | fastener stacks (below) |
| `route` | routed tubes, hoses and cables (below) |
| `weld` | welds between components (below) |
| `explode` | exploded views (below) |
| assembly features | `hole`, `extrude ... op=subtract` with `affects=[...]` (below) |
| `tag` | `tag <name> = <reference>`: a local name for an instance path |

`instance` overrides `{ ... }` set parameters of the referenced part or assembly for this instance only.
Each distinct (file, configuration, overrides) triple is built once and shared by all instances that
use it; it is its own BOM line.

### References

`<instance>[.<instance> ...].<connector | published | datum>` and, for frames, the derived primitives:

| Suffix | Denotes |
| --- | --- |
| *(none)* | the frame |
| `.origin` | its origin point |
| `.x`, `.y`, `.z` | the axis line through the origin along that direction |
| `.xy`, `.yz`, `.xz` | the plane through the origin with normal z, x, y respectively |

A path through a sub-assembly names the nested instance: `left_cyl.rod.thread_end`. A pattern copy is
`<pattern>.i<k>` and its instance names follow: `bolts.i3.head`. Unknown segments are `E6001` with the
nearest names (did-you-mean), as in parts.

### Placement before solving

`at=` and `rotate=` on an instance are **starting values** (soft), like coordinates in a sketch: an
instance with no mate stays exactly there; a mated instance starts there. `@hint(x, y, z, qw, qx, qy,
qz)` (millimetres and a unit quaternion) is a solved pose written by a tool and is used
when `at=` is absent. A grounded instance's `at=` is hard.

## Mates

A mate relates geometric primitives of two (or more) instances. The table gives the default primitive
when a bare frame is passed, the degrees of freedom removed and the orientation choice.

| Kind | Arguments (default primitive for a frame) | Removes | Orientation and options |
| --- | --- | --- | --- |
| `fastened(a, b)` | frame, frame | 6 | the frames coincide (same x, y, z); `rotate=<angle>` about z and `offset=(x, y, z)` in `b`'s frame |
| `coincident(a, b)` | point/point 3, point/line 2, point/plane 1, line/line 4, line/plane 2, plane/plane 3 (frame: `.xy`) | as listed | planes: `opposed` (default, normals opposite: faces touching) or `aligned`; lines: `aligned` (default) or `opposed` |
| `concentric(a, b)` | line/line (frame: `.z`) | 4 | `aligned` (default) or `opposed` |
| `parallel(a, b)` | line or plane pairs (frame: `.z`) | 2 | `aligned` (default) or `opposed` |
| `perpendicular(a, b)` | line or plane pairs (frame: `.z`) | 1 | none |
| `distance(a, b) = d` | point, line or plane pairs (frame: `.xy`) | 1 (planes: 3) | signed: positive along the normal (or z) of `b`; planes also take `opposed`/`aligned` |
| `angle(a, b) = t` | line or plane pairs (frame: `.z`), `about=<line>` required | 1 | signed, right-handed about `about` |
| `tangent(a, b)` | published face and frame primitive or published face | 1 | `outside` (default) or `inside` |
| `revolute(a, b)` | frame, frame | 5 | frames' z axes coincide, origins coincide; coordinate: angle of `a.x` about `b.z` from `b.x` |
| `slider(a, b)` | frame, frame | 5 | z axes coincide, x axes parallel; coordinate: signed distance of `a.origin` from `b.origin` along `b.z` |
| `cylindrical(a, b)` | frame, frame | 4 | z axes coincide; coordinates: distance along and angle about `b.z` |
| `planar(a, b)` | frame, frame | 3 | xy planes coincide (opposed by default); coordinates: x, y, angle |
| `ball(a, b)` | frame, frame | 3 | origins coincide; coordinates: a rotation (reported as three angles, z-y-x) |
| `universal(a, b)` | frame, frame | 4 | origins coincide and the cross pins stay perpendicular (`a.y` perpendicular to `b.x`); coordinates: angle about `b.x`, angle about `a.y` |
| `pin_slot(a, b)` | frame (pin), frame (slot, x along the slot) | 4 | origins on `b`'s x axis, z axes aligned; coordinates: distance along `b.x`, angle |
| `path(a, b)` | point primitive, published edge chain | 2 | coordinate: arc length from the chain's start; `tangent` flag aligns `a.x` with the chain |
| `cam(a, b)` | published face (follower), published face (cam) | 1 | contact maintained; `outside` (default) or `inside` |
| `gear(j1, j2) = ratio` | two `revolute` or `cylindrical` mates | 1 | `coord(j1) = ratio * coord(j2)`; flag `opposed` negates |
| `rack_pinion(jr, js)` | `revolute` (pinion) and `slider` (rack), `radius=<length>` | 1 | `coord(js) = radius * coord(jr)` |
| `screw(j)` | one `cylindrical` mate, `lead=<length per turn>` | 1 | distance = lead * angle / 360 deg |
| `couple(j1, j2)` | two joint coordinates, `ratio=<number or quantity>`, `offset=` | 1 | `coord(j1) = ratio * coord(j2) + offset` (linear-to-rotary uses a length per angle ratio) |

Arguments common to all mates: `= <value>` for kinds that take one; `drive=<parameter>` sets a joint's
coordinate from a parameter (it then removes that DOF too); `min=` and `max=` limit a joint
coordinate; `suppress if <expr>`.

The kinds `revolute` ... `pin_slot` are **joints**: they leave named coordinates that a `drive=`,
`couple` or `gear` can use, that a pose can set, and that the viewer animates. Their coordinate is
reported in the manifest.

A mate whose primitives have the wrong kind is `E6101` (for example `concentric` on two planes); the
message lists the primitives each argument offers.

### Solving

Each non-grounded instance (or rigid sub-assembly) is a rigid body with six degrees of freedom, and
each mate is a set of equations on the world positions of its primitives. A solver must find the
solution nearest to the starting pose (`at=`, `rotate=`, `@hint`) in the sense of a minimum-norm change
from it, respecting the explicit orientation flags, and must report:
- **Over-constrained**: a set of mates with no solution is `E6102` naming a minimal conflicting set
  (with related locations), as `E4101` does for sketches; a mate that removes no further DOF is
  `W6103`.
- **Under-constrained** is allowed: `I6201` reports the remaining DOF per instance and names them
  (`rod: 1 rotation about barrel.bore_axis`). Tools may offer a strict mode that treats it as an
  error for release builds.
- **No solution near the start** is `E6301` and names the mates with the largest residual.
- Joint limits are inequality constraints; a `drive` outside `min`/`max` is `E6302`.
- Tolerances: positions within the assembly's modeling tolerance (default 1e-6 mm), angles within
  1e-9 rad.

## Sub-assemblies

`instance arm = assembly("assemblies/arm.tcad")` is **rigid** by default: its internal pose is the one
its own default configuration (or `config=`) solves to, and it moves as one body. With the flag
`flexible` its instances and mates join the parent's solve, so its joints keep their freedom and the
parent may mate to its nested instances (`arm.stick.bucket_pin`) and drive its parameters through the
override block. Instance overrides of a sub-assembly may set its parameters, including the ones that
drive its joints.

## Repetition and variation

```
pattern bolts of=[bolt, washer] follow=plate.bolt_seat           # one copy per connector instance
pattern rails of=[rail] linear dir=+X count=4 spacing=250 mm skip=[3]
pattern spokes of=[spoke] circular axis=hub.bore.z count=6 angle=360 deg
mirror_instance right_arm of=left_arm plane=frame.mid_plane.xy
mirror_instance right_bracket of=left_bracket plane=frame.mid_plane.xy part=part("parts/bracket_rh.tcad")
```

- `pattern` copies instances and the mates **among** them (mates to instances outside the set are
  re-bound per copy only through `follow`). `follow=<connector>` takes a connector created `from=` a
  patterned hole or hole set and places copy `k` so that the seed's relation to instance 1 of the
  connector holds for instance `k`. `linear` and `circular` work as the part patterns
  (spec/features.md). Copies are named `<pattern>.i<k>.<instance>`; `i1` is the seed and keeps its name.
- `mirror_instance` without `part=` makes a **mirror copy** (the same part, a reflected transform;
  STEP and BOM record it as the same part, mirrored, and the BOM requires `meta mirrored_number` or
  reports `W6503`). With `part=` it places a **true opposite-hand part**, whose connectors must have the
  same names (`E6013` lists the missing ones); its pose is the reflection of the seed's pose made
  right-handed again by flipping each connector's x axis.

### Configurations and suppression

```
config long default {
  stroke = 1200 mm
  rod.config = long
  suppress guard, bolts.i4
}
```

As in parts (spec/parameters.md), plus `<instance>.config = <name>` and suppression of instances,
pattern copies and mates. A suppressed instance is absent from the solve, the BOM, mass properties and
checks; mates that reference it are suppressed with it (`I6202` lists them). `suppress if <expr>` on
`instance` and `mate` works as on features. A **pose** is a configuration that only changes driving
parameters; tools use poses to show or check the assembly in a given state.

## Skeleton (top-down layout)

```
tcad 0.2
part boom_layout "Boom layout" {
  param boom_len = 5600 mm
  param pivot_h  = 900 mm
  point boom_foot at=(0 mm, 0 mm, 0 mm)
  point boom_tip  at=(boom_len, 0 mm, pivot_h)
  connector foot_pin origin=boom_foot z=+Y
  sketch side on plane.XZ { ... }                  # layout lines
}
```

A skeleton is an ordinary part file that is used with `skeleton("...")`; it may contain solids but
usually holds parameters, datums, sketches and connectors. In an assembly it is an instance, normally
grounded, and other instances mate to its connectors. A **part** may read a skeleton:

```
uses layout = skeleton("../skeletons/boom_layout.tcad")
param pin_span = layout.boom_len - 2 * 80 mm
plane tip_plane from=layout.boom_tip_plane
```

- `uses` makes the skeleton's parameters, datums, sketches (as profiles and paths) and connectors
  available read-only under the alias, in the skeleton's own frame (the part is assumed to be placed
  with its `origin` on the skeleton's `origin`; a different placement is expressed by datums).
- A skeleton may not `use` anything that uses it, directly or indirectly (`E6004`). A part may not
  reference another part's geometry except through a skeleton: in-context references between parts are
  not part of the format.
- The source hash of a part that uses a skeleton includes the skeleton's canonical text (spec/bundle.md).

## Assembly-level features

```
hole dowel on=plate.top_face at=[(40 mm, 20 mm), (160 mm, 20 mm)] d=8 mm depth=30 mm affects=[plate, bracket]
extrude relief from=asm_sketch.r op=subtract through_all affects=[weldment]
```

- Only `hole` and subtracting `extrude` (and patterns of them) are allowed; `affects=` lists the
  instances cut. `on=` accepts a frame primitive or a published planar face; sketches in an assembly
  lie on frame planes (`sketch s on frame.mid.xy { ... }`).
- The cut applies to those instances **in this assembly only**. Their part files are unchanged; the
  modified solids exist only in the assembly's regenerated result, and their new faces are named by
  spec/naming.md with the assembly feature as the creator (`dowel.p1.wall`).
- The BOM keeps the part number and marks the line "machined in assembly".

## Fastener stacks

```
fastener m8_stack at=plate.bolt_seat through=[plate, bracket] bolt="lib:iso4762.tcad" size=M8
                  washer="lib:iso7089.tcad" nut="lib:iso4032.tcad" length=auto engage=1.5 thread_pitch
```

Places, for each connector instance reached by `at=` (a single connector or a patterned one), a bolt,
the washers and the nut, mated along the connector's z axis, through the listed instances. `length=auto`
picks the shortest standard length (from the library part's configurations) that gives the requested
thread protrusion (`engage`, default 1.5 pitches past the nut) or, for tapped holes, thread engagement
of at least `engage_depth` (default 1.0 times the nominal diameter). The grip is measured along the
axis through the `through` instances. If no standard length fits, `E6401` gives the grip and the
nearest lengths. The stack expands to ordinary instances named `<fastener>.i<k>.bolt`, `.washer1`,
`.nut`, which the BOM counts as purchased.

## Routed parts

```
route p_hose from=pump.port_a to=boom_cyl.port_b profile=tube(od=12 mm, wall=1.5 mm)
             via=[frame.clip1, frame.clip2] min_bend=48 mm material="EN 853 2SN" kind=purchased
```

A route generates a part (part of the assembly's regenerated result, not a file) by sweeping a profile along a
3D path from connector `from` to connector `to` (leaving and entering along their z axes), through
`via` connectors or points, with every corner rounded to `min_bend` (spec/features.md, 3D paths).
`profile=` is `tube(od=, wall=)`, `rod(d=)` or a sketch profile. The BOM lists the cut length (the
centreline length) per route. A route that cannot keep `min_bend` between two consecutive points is
`E6402`.

## Welds

```
weld w1 between=[gusset, base_plate] type=fillet size=6 mm length=all side=both
```

A weld records a callout between two instances along their common edges (`length=all` or a length with
`pitch=` for intermittent welds); with the flag `bead` it also adds a simplified bead solid (a swept
triangle) to the assembly. `type` is `fillet`, `butt`, `plug`, `spot`. Welds appear in the BOM's weld
table and, from Phase 3, as PMI items of the assembly. The parts welded into one item are listed with
`weldment=<name>` on the instances; the weldment is one BOM line with its members as a cut list.

## BOM

```
bom numbering=first_appearance structure=indented columns=[number, revision, description, material, mass, kind]
```

- Lines are distinct (file, configuration, overrides, mirrored) combinations; quantities count
  instances including pattern and fastener copies, per configuration, excluding suppressed and
  `bom=exclude` instances. `phantom` sub-assemblies are dissolved into their parent.
- Item numbers follow `numbering=` (`first_appearance`, the default, or `sorted_by_number`) and can be
  pinned per instance with `item=<n>` so drawings keep their balloons; a pinned number used twice is
  `E6501`.
- How a BOM is presented (table, CSV, JSON) is up to tools; its lines, quantities and item numbers are
  defined here.

## Mass properties

The mass properties of an assembly are the mass, the centre of mass and the inertia tensor about the
centre of mass of the whole assembly in its current pose, combining each instance's part properties (or its
`meta mass` / `com` override) with the parallel axis theorem. Parts without a material are reported
(`W6601`) and counted with zero mass.

## Checks

```
check no_clash   interference among=all ignore=[[rod, gland], fastener]
check eye_clear  clearance(eye, barrel) min=10 mm over=extension from=0 mm to=stroke steps=50
```

- `interference` reports every pair of solids that overlap by more than the modeling tolerance; mates
  that are meant to touch (fastened, coincident planes) are not interference. `ignore=` takes pairs and
  instances.
- `clearance(a, b) min=` requires the minimum distance between the bodies to be at least `min`; with
  `over=<parameter> from= to= steps=` it is evaluated at each step of a driving parameter (the solver
  re-poses the assembly at each step).
- Checks need the parts' regenerated geometry. A failing check is `E6403`, reported with the pair, the
  configuration, the step and the measured distance or volume.

## Exploded views

```
explode service {
  step move=[cover, cover_bolts] along=+Z distance=120 mm
  step move=[rod, piston, eye] along=barrel.bore_axis.z distance=600 mm
}
```

Exploded views are named, ordered lists of displacements applied after solving. They do not change the
solved transforms, the BOM or the checks; tools use them to show the assembly.
(`view` is reserved for drawings in Phase 4.)

## Diagnostics (E6xxx)

| Code | Meaning |
| --- | --- |
| E6001 | reference to an unknown instance, connector, published entity or datum (with nearest names) |
| E6002 | referenced file not found, or not a part / assembly as the call says |
| W6003 / E6003 | referenced file differs from `bundle.lock` (error under `--locked`) |
| E6004 | reference cycle (an assembly or skeleton that refers back to itself) |
| E6005 | the referenced part's `meta revision` does not match `rev=` |
| E6011 | connector axes are parallel or degenerate |
| E6012 | no grounded instance |
| E6013 | opposite-hand part lacks connectors of the seed |
| I6014 | a connector or published entity is suppressed with the feature it refers to |
| E6101 | mate arguments have the wrong primitive kinds |
| E6102 | mates conflict (names a minimal conflicting set) |
| W6103 | redundant mate |
| I6201 | remaining degrees of freedom, per instance |
| I6202 | mates suppressed with a suppressed instance |
| E6301 | the solver found no solution near the start |
| E6302 | a driving value is outside the joint's limits |
| E6401 | no standard fastener length fits the grip |
| E6402 | a route cannot keep its minimum bend radius |
| E6403 | an interference or clearance check fails |
| E6501 | a pinned BOM item number is used twice |
| W6503 | mirrored copy without a mirrored part number |
| W6601 | instance without material (zero mass) |

## Grammar (draft, to be merged into grammar.ebnf when implemented)

```
top_item        = part_decl | assembly_decl | reserved_stmt ;
assembly_decl   = "assembly" , IDENT , [ STRING ] , "{" , { asm_item } , "}" ;
asm_item        = meta_decl | param_decl | instance_decl | ground_decl | mate_decl | pattern_decl
                | mirror_inst_decl | config_decl | bom_decl | check_decl | fastener_decl
                | route_decl | weld_decl | explode_decl | sketch_decl | feature_stmt | tag_decl
                | reserved_stmt ;
meta_decl       = "meta" , { named_arg } ;
instance_decl   = "instance" , IDENT , "=" , source_call , { arg } , [ suppress_clause ] ,
                  [ "{" , { IDENT , "=" , ( expr | STRING ) , [ ";" ] } , "}" ] , [ hint ] ;
source_call     = ( "part" | "assembly" | "skeleton" ) , "(" , STRING , ")" ;
ground_decl     = "ground" , reference , { arg } ;
mate_decl       = "mate" , IDENT , IDENT , "(" , reference , { "," , reference } , ")" ,
                  [ "=" , expr ] , { arg } , [ suppress_clause ] ;
pattern_decl    = "pattern" , IDENT , { arg } , [ suppress_clause ] ;
mirror_inst_decl= "mirror_instance" , IDENT , { arg } , [ suppress_clause ] ;
bom_decl        = "bom" , { arg } ;
check_decl      = "check" , IDENT , IDENT , [ "(" , reference , { "," , reference } , ")" ] , { arg } ;
fastener_decl   = "fastener" , IDENT , { arg } , [ suppress_clause ] ;
route_decl      = "route" , IDENT , { arg } , [ suppress_clause ] ;
weld_decl       = "weld" , IDENT , { arg } , [ suppress_clause ] ;
explode_decl    = "explode" , IDENT , "{" , { "step" , { arg } , [ ";" ] } , "}" ;
config_item     = (* as in parts, plus *) reference , "." , "config" , "=" , IDENT ;

(* additions to parts *)
part_item       = (* ... *) | meta_decl | connector_decl | publish_decl | uses_decl ;
connector_decl  = "connector" , IDENT , { arg } , [ suppress_clause ] ;
publish_decl    = "publish" , IDENT , "=" , reference , { "," , reference } ;
uses_decl       = "uses" , IDENT , "=" , "skeleton" , "(" , STRING , ")" ;
```

`assembly`, `instance`, `mate`, `skeleton` and `bom` leave the reserved list (spec/reserved.md) in format
0.2. The new contextual keywords are `meta`, `connector`, `publish`, `uses`, `ground`, `pattern`,
`mirror_instance`, `check`, `fastener`, `route`, `weld`, `explode`; they may not be used as declared names
in the scope where they are meaningful (ADR 0004).

## Conformance

Phase 2 is complete when the conformance corpus (`corpus/asm_*`) holds assemblies whose expected
transforms, joint coordinates, BOM lines and mass properties are computed independently, covering every
mate kind, sub-assemblies, patterns, mirroring, configurations, skeletons, fastener stacks, routes and
checks, and including a large reference model: an excavator boom with flexible hydraulic-cylinder
sub-assemblies (about 300 instances) with a clearance check through the full cylinder stroke.
