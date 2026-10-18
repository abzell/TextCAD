# Features

Status: **normative** (M5, M6): reference geometry, extrude, revolve, sweep, loft, holes, fillets,
chamfers, shell, draft, rib, patterns. Source requirements: 7. Implemented by
`tcad-build`.

Features are evaluated in declaration order. Regeneration **stops at the first feature that fails**,
with a diagnostic naming it; it never skips or substitutes. A feature suppressed by `suppress if` or by
the active configuration is skipped. Unknown or misplaced arguments are `E5001`; a reference that is
not the right kind of geometry is `E5002`.

## Reference geometry

| Statement | Arguments | Result |
| --- | --- | --- |
| `plane p` | `from=<plane>` with `offset=<length>` and/or `angle=<angle> about=<axis>`; or `through=[p1, p2, p3]` | A plane. `offset` moves it along its normal; `angle` rotates it about the axis (after the offset). Three points give x along `p1 p2`, normal by the right-hand rule. |
| `point p` | `at=(x, y, z)` | A 3D point. |
| `axis a` | `through=[p1, p2]`; or `from=<point> direction=<(x, y, z) or axis>` | An axis. |
| `csys c` | `origin`, `x`, `y` (a point, and two direction tuples) | A coordinate system, usable as a plane. |

Built in: `plane.XY`, `plane.YZ`, `plane.XZ`, `axis.X`, `axis.Y`, `axis.Z`, `origin`. Points are
given as `(x, y, z)` lengths or the name of a `point`. A sketch can lie on any plane or `csys`; sketches
on faces need persistent naming (M6).

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
| `loft l` | `sections=[a, b, ...]` (two or more single profiles); optional flag `ruled` |

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
surface. A hole tool sticks 0.5 mm out of the surface so the cut is clean.

**Tapped holes** are drilled at the tap drill diameter (`d` if given, otherwise from the thread:
`M8` / `M8x1.25` give `nominal - pitch`, `1/2-13 UNC` gives `nominal - 1/tpi`) and the thread
designation is stored as metadata in the manifest. Threads are cosmetic in Phase 1.

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

Guided lofts need a kernel feature OpenCASCADE lacks and are tracked in
`docs/progress.md`. Queries on vertices, ribs along arcs and circular patterns of several axes are
not part of Phase 1.
