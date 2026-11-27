# Features

Status: **normative** (format 0.1; part metadata, connectors and published geometry format 0.2). Source requirements: 7.

Features are evaluated in declaration order. Regeneration **stops at the first feature that fails**,
with a diagnostic naming it; it never skips or substitutes. A feature suppressed by `suppress if` or by
the active configuration is skipped. Unknown or misplaced arguments are `E5001`; a reference that is
not the right kind of geometry is `E5002`.

## Part metadata

Format 0.2 (`spec/versioning.md`).

```
part rod "Piston rod, 50 mm" {
  material "AISI 1045" density=7.85 g/cm^3
  meta number="HC-110-014" revision="B" kind=made mass=0.84 kg com=(0 mm, 0 mm, 12 mm) color=(0.7, 0.7, 0.72)
  config heavy { meta.number = "HC-110-015" }
}
```

The description of a part is the string after its name. `meta` adds:

| Key | Value | Use |
| --- | --- | --- |
| `number` | string | part number: STEP product id, BOM (Phase 2), title blocks (Phase 4) |
| `revision` | string | revision: STEP product version, BOM, `rev=` pins (spec/assembly.md) |
| `kind` | `made` (default), `purchased`, `phantom` | BOM classification |
| `vendor`, `vendor_number` | string | for purchased parts |
| `mass` | mass, positive | overrides the computed mass (purchased parts modeled simplified) |
| `com` | `(x, y, z)` lengths | overrides the center of mass; only together with `mass` |
| `color` | `(r, g, b)`, numbers from 0 to 1 | display colour, **sRGB** |

- Values are literals or parameters of the right type (`number=num` with `param num = "HC-1"`);
  there is no string concatenation. Parameters must be declared before the `meta` statement.
- `meta` may appear once (`E2303`), anywhere in the part body; the canonical form does not move
  statements.
  The keys are written in the order of the table.
- A configuration overrides keys with `meta.<key> = <value>` (`E2401` for an unknown key). Keys
  it does not override keep their base values.
- Errors: unknown key, a flag, a repeated key or a key with several values `E5001`; `com` without
  `mass` `E5001`; a value of the wrong type `E2202`; a non-positive mass or a colour component
  outside 0..1 `E2301`; an unknown `kind` `E2501`.
- With `mass`, the part's mass properties are that mass (and `com` if given, else the computed
  center), and its inertia is the unit-density inertia scaled by `mass / volume`, i.e. that of a
  uniform body of that mass about the computed center of mass. Without `mass` the material density
  is used. Tools should report the computed values alongside an override.

## Connectors

Format 0.2 (`spec/versioning.md`). Connectors and published geometry are a part's interface to
assemblies (spec/assembly.md): assemblies refer to them, never to raw faces or edges.

A connector is a named right-handed frame (origin, x, y, z) owned by the part. It is evaluated after the
last feature, from references resolved by persistent naming (spec/naming.md), once per configuration.

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
follows the rules for suppressed references (spec/assembly.md, `I6202`). A connector can also be
suppressed on its own (`suppress if`).

The conformance cases `corpus/con_*` cover these rules.

Every part has the implicit connector `origin` (the part frame) and its datums are visible to
assemblies under their own names (`plate.plane.XY`, `plate.mid_plane`, `plate.pin_axis_a`).

## Published geometry

Format 0.2.

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

## Reference geometry

| Statement | Arguments | Result |
| --- | --- | --- |
| `plane p` | `from=<plane>` with `offset=<length>` and/or `angle=<angle> about=<axis>`; or `through=[p1, p2, p3]` | A plane. `offset` moves it along its normal; `angle` rotates it about the axis (after the offset). Three points give x along `p1 p2`, normal by the right-hand rule. |
| `point p` | `at=(x, y, z)` | A 3D point. |
| `axis a` | `through=[p1, p2]`; or `from=<point> direction=<(x, y, z) or axis>` | An axis. |
| `csys c` | `origin`, `x`, `y` (a point, and two direction tuples) | A coordinate system, usable as a plane. |

Built in: `plane.XY`, `plane.YZ`, `plane.XZ`, `axis.X`, `axis.Y`, `axis.Z`, `origin`. Points are
given as `(x, y, z)` lengths or the name of a `point`. A sketch can lie on any plane or `csys`; sketches
on faces use persistent naming ([naming.md](naming.md)).

## Profiles

`from=` (and `sections=`) name a profile: a sketch (`from side`: every non-construction closed
loop), a sketch entity (`from side.outline`), or an inline `rect(on=<plane>, corner=|center=, w=, h=)` or
`circle(on=<plane>, center=, r=|d=)`. Closed entities (rectangles, circles, ellipses, polygons, slots,
closed splines) are loops; lines, arcs and open splines chain end to end within the modeling tolerance.
A chain that does not close is `E5005` and names the point where it breaks. Loops nest by containment:
an even depth is an outer boundary, an odd depth a hole in the loop around it.

## Solid features

All solid features take `op=add|subtract|intersect` (default `add`). The first solid feature must add
(`E5006` otherwise). A feature made of several profiles unites their solids before combining.

| Statement | Arguments |
| --- | --- |
| `extrude e` | `from=` profile; **one** end condition: `distance=<length>`, `through_all`, or `to=<plane>`; optional `symmetric`, `distance2=<length>` (a second distance the other way, used with `distance`), `draft=<angle>` |
| `revolve r` | `from=` profile; `axis=<axis>` (`axis.Z`, an `axis` feature, or a line of a sketch); `angle=<angle>` (default 360 deg) |
| `sweep s` | `from=` profile; `path=` sketch (or entity) holding a chain; optional `guide=` |
| `loft l` | `sections=[a, b, ...]` (two or more single profiles); optional flag `ruled`; optional `guides=[...]` (sketch curves from the first section to the last that steer the surface; the exact rules are being specified, see the conformance part `feat_loft_guided`) |

**End conditions.** A profile extrudes from its own plane along its normal. `distance` is signed; a
negative value goes the other way. `symmetric` centers the distance on the plane. `distance2` adds a
second distance on the other side. `through_all` reaches past the whole current body (twice its
bounding box diagonal). `to=<plane>` stops at a plane parallel to the profile. A `draft` tapers the
sides inward and cannot be combined with `symmetric` or `distance2`.

**Sweep.** The profile must lie at the start of the path, in a plane perpendicular to the path there
(the kernel does not move it). A guide controls the orientation of the profile along the path.

**Loft.** Sections must be single closed profiles. A smooth loft interpolates; `ruled` uses straight sides.

## Detail features

Detail features modify the body. They need an existing body (`E5006`) and refer to faces and edges
by tag, semantic ID or query (`spec/naming.md`). Their results are named as listed there.

| Statement | Arguments |
| --- | --- |
| `hole h` | where: `on=<face or plane>` with `at=(x, y)` or `at=[(x, y), ...]` in the face's frame, or `from=<sketch>` whose `point` entities are the positions (and whose plane is used if `on` is left out); what: `d=<diameter>`, optional `type=simple\|counterbore\|countersink\|tapped` (inferred from the other arguments), `depth=<length>` or `end=through_all\|blind` (through if neither is given), `cbore_d`, `cbore_depth`, `csk_d`, `csk_angle` (included angle, default 90 deg), `thread="M8x1.25"`, flag `flip` |
| `fillet f` | `edges=[...]` (edges, or faces for all their edges); `r=<length>` constant, or `r=(start, end)` varying linearly along each edge |
| `chamfer c` | `edges=[...]`; `d=<length>`, then optionally `d2=<length>` or `angle=<angle>` (the second distance is `d * tan(angle)`) |
| `shell s` | `faces=[...]` to open (omit for a closed cavity), `thickness=<length>`, flag `outward` |
| `draft d` | `faces=[...]`, `angle=<angle>`, `pull=<direction>`, `neutral=<plane or face>` |
| `rib r` | `profile=<sketch>` (straight lines only), `thickness=<length>`, `direction=<in-plane direction>`, and `to=<plane or face>` or `reach=<length>` |

**Hole frames.** A planar face has the frame: origin where its plane is closest to the world
origin, z along the outward normal, x horizontal (`Z cross n`; world X for horizontal faces), y
`n cross x`. A hole drills along `-z` of its frame (into the material when `on` is a face); `flip`
reverses it. Through holes reach past the whole body; a blind hole's `depth` is measured from the
surface. A hole on a face starts 0.5 mm above the surface so the cut is clean; a hole on a datum plane starts exactly at the plane.

**Tapped holes** are drilled at the tap drill diameter (`d` if given, otherwise from the thread:
`M8` / `M8x1.25` give `nominal - pitch`, `1/2-13 UNC` gives `nominal - 1/tpi`) and the thread
designation is stored as metadata in the manifest. A tapped hole may also have `cbore_d` and `cbore_depth`
(for example the seal face of an O-ring port). Threads are cosmetic in Phase 1.

**Chamfer distances.** `d` is measured on the adjacent face whose name sorts first, so the choice
never depends on kernel numbering; `d2` on the other face.

**Draft.** A positive angle makes the faces lean so that the body is narrower along `pull`. The
faces turn about their intersection with the neutral plane.

**Rib.** The rib region is the polygon of the centerline and its projection along `direction` onto the
`to` plane (or moved by `reach`); it is extruded symmetrically by `thickness` about the sketch plane
and added to the body. A centerline that already touches the base contributes no closing edge there.

## Patterns

A pattern copies the tool of an earlier feature (the solid it added, subtracted or intersected) and
applies it again with the same operation. Instance 1 is the original; `skip=[3, 5]` leaves out
instances of a pattern (instance 1 cannot be skipped).

| Statement | Arguments |
| --- | --- |
| `linear_pattern p` | `of=<feature or pattern>`, `dir=<direction>`, `count=<n>` (including the original), `spacing=<length>`, optional `skip` |
| `circular_pattern p` | `of=`, `axis=<axis>`, `count=<n>`, optional `angle=<angle>` (default 360 deg: `count` evenly spaced copies; otherwise the copies span the angle with the last at its end), optional `skip` |
| `mirror m` | `of=`, `plane=<plane or face>` (one mirrored copy) |

A pattern of a pattern repeats every instance of the first pattern. Sketches may lie on a planar
face (`sketch s on boss.cap_end { ... }`), and `to=` accepts a planar face as well as a plane.

## Import

`import name = step("imports/x.step")` and `combine c with=name [at=(x, y, z)] [op=...]` bring a STEP
solid into the part (spec/io.md).

## Not yet

Queries on vertices, ribs along arcs and circular patterns of several axes are not yet part of the
format.
