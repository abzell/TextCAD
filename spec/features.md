# Features

Status: **normative for the M5 features** (reference geometry, extrude, revolve, sweep, loft, boolean
operations); detail features and patterns follow in M6. Source requirements: 7. Implemented by
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

## Not yet

`hole`, `fillet`, `chamfer`, `shell`, `draft`, `rib`, patterns and `mirror` are `E5003` until M6;
`import` is `E5003` until M7; guided lofts need a kernel feature OpenCASCADE lacks and are tracked in
`docs/progress.md`.
