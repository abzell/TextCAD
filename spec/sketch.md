# Sketches

Status: **normative** (M4). Source requirements: 7, 7.1. Implemented by `tcad-solver` (the
constraint solver) and `tcad-model` (`sketch.rs`, the bridge from the language to the solver).

```
sketch side on plane.XZ {
  rect  outline corner=(0 mm, -29 mm) w=70 mm h=58 mm
  circle pin    center=(50 mm, 0 mm) d=20 mm
  arc   nose    start=outline.bottom_right end=outline.top_right through=(75 mm, 0 mm)
  constrain concentric(nose, pin)
}
```

A sketch is a set of 2D entities and constraints in the sketch plane (coordinates in mm). The plane
reference (`plane.XZ`, a face, a datum plane) is scope-checked here and placed in 3D by the kernel
in M5.

## Soft starting values and hard dimensions

- **Coordinates are starting values.** `center=(50 mm, 0 mm)`, `corner=...`, `start=...` say where
  an entity is drawn. The solver keeps them exactly if the constraints allow it; otherwise it moves
  them as little as possible. They are *authored* values: they resist movement far more than values
  the tool had to invent (such as the starting center of a three-point arc).
- **Dimensions are constraints.** `r`, `d`, `w`, `h`, the ellipse `a`, `b`, `angle` and the
  `width` of a slot are hard: they are satisfied exactly.
- **References share points.** `line l start=a.end end=(5 mm, 0 mm)` makes `l.start` the same point
  as `a.end`. A reference is never a starting value.
- **Missing values** are taken from the entity's `@hint(...)` (below), then from a deterministic
  default placement. Values given in the arguments always win over a hint.

Everything else about the geometry is stated with `constrain`.

## Entities

Arguments not listed for a keyword are `E4001`. Lengths are in any length unit; angles in any angle
unit.

| Entity | Arguments | Variables (`@hint` order) |
| --- | --- | --- |
| `point p` | `at=(x, y)` | x, y |
| `line l` | `start`, `end` (coordinates or point references) | x1, y1, x2, y2 |
| `circle c` | `center`, `r` or `d` | cx, cy, r |
| `arc a` | form 1: `center`, `start`, `end`; form 2: `start`, `end`, `through`; form 3: `start`, `end`, `tangent_to`; any form may add `r` | cx, cy, sx, sy, ex, ey |
| `ellipse e` | `center`, `a`, `b`, `angle` | cx, cy, a, b, angle |
| `spline s` | `points=[...]` (control points) or `through=[...]` (interpolated), flag `closed` | x0, y0, x1, y1, ... |
| `slot s` | `start`, `end`, `w` | x1, y1, x2, y2, w |
| `rect r` | `corner` (bottom left) or `center`, `w`, `h` | x, y of bottom left, bottom right, top right, top left |
| `polygon g` | `sides` (3 to 256), `center`, `r` or `d` (circumscribed), `angle` | cx, cy, r, angle of the first vertex |
| `project p` | `edges` | none (resolved by the kernel; `W4601`) |

Any entity may carry the flag `construction` (reference geometry that is not part of a profile).

**Arcs** run counterclockwise from `start` to `end`. In form 2 a `through` that is a coordinate pair
only picks the starting circle (soft); a `through` that is a point reference makes the arc pass
through that point (hard). In form 3 the arc starts on the curve named by `tangent_to` and is
tangent to it there (hard). Writing `through=P, Q` with two values means `start=P end=Q`.

**Rectangles** are axis-aligned in the sketch plane. **Polygons** are regular: equal distance from
the center and equal sides. **Slots** are a centerline with a width.

## Points and curves of an entity

A reference `name.role` names a point or a curve of an entity; the bare name is the entity itself.

| Entity | Point roles | Curve roles |
| --- | --- | --- |
| `point` | (the entity), `at` | |
| `line` | `start`, `end` | (the entity) |
| `circle` | `center` | (the entity) |
| `arc` | `center`, `start`, `end` | (the entity, as its full circle) |
| `ellipse` | `center` | (the entity) |
| `rect` | `bottom_left`, `bottom_right`, `top_right`, `top_left` | `bottom`, `right`, `top`, `left` (counterclockwise) |
| `polygon` | `center`, `v0` ... | `e0` ... (edge `i` runs from `v_i` to `v_i+1`) |
| `slot` | `start`, `end` | (the entity, as its centerline); its four outline segments are `right`, `end_cap`, `left`, `start_cap` (counterclockwise from the right side) |
| `spline` | `p0`, `p1`, ... | |

## Constraints

`constrain name(args)` or, for dimensions, `constrain name(args) = value`. Dimensions need a value of
the right dimension and other constraints take none (`E4003`); an unknown name is `E4002`.

| Constraint | Arguments | Meaning |
| --- | --- | --- |
| `coincident(p, q)` | two points | same position |
| `horizontal(l)`, `horizontal(p, q)` | a line or two points | same y |
| `vertical(l)`, `vertical(p, q)` | a line or two points | same x |
| `parallel(l, m)`, `perpendicular(l, m)` | two lines | |
| `tangent(a, b)` | line and circle/arc, or two circles/arcs | touch tangentially (see below) |
| `equal(a, b)` | two lines, or two circles/arcs | equal length, or equal radius |
| `symmetric(p, q, l)` | two points and a line | mirror images in the line |
| `concentric(a, b)` | two curves with centers | same center |
| `midpoint(p, l)` | a point and a line | p is the midpoint of the segment |
| `fix(p)` | a point | stays where it is written |
| `on_curve(p, c)` (also `point_on_curve`) | a point and a curve | p lies on the curve (an arc counts as its full circle) |
| `distance(p, q) = d` | two points | distance |
| `distance(p, l) = d` | a point and a line | perpendicular distance from the line through the segment |
| `distance(l) = d` | a line | its length |
| `hdistance(p, q) = d`, `vdistance(p, q) = d` | two points | `q.x - p.x`, `q.y - p.y` |
| `radius(c) = r`, `diameter(c) = d` | circle or arc | |
| `angle(l, m) = a` | two lines | angle from `l` to `m`, counterclockwise |

**Tangent at a shared end point.** If a line end and an arc end (or two arc ends) are known to
coincide (the same point, or related by `coincident`), `tangent` is expressed through that contact
point: the radius at the contact is perpendicular to the line, or the two centers are collinear with
the contact. Otherwise the general form is used: distance from the center to the line equals the
radius, or distance between centers equals the sum or difference of radii, whichever the starting
position is closest to. The contact form keeps tangent chains from reporting phantom degrees of
freedom.

## Solving

1. Entities become variables and primitives; constraints become scalar equations. Residuals are
   lengths (mm) or dimensionless/radian quantities, so one tolerance of 1e-8 applies to all.
2. Equations that share variables form a **cluster**; clusters are solved independently, in order of
   their smallest variable. Authored coordinates are *pinned* (constants) in a first attempt, which
   also decouples clusters; if that attempt fails they are released with large weights.
3. Each cluster is solved by Newton's method. The step is the minimum-weighted-norm solution of the
   linearized system, so an under-constrained sketch moves the *least*, and a solution never jumps
   to a mirror branch; backtracking and Levenberg-Marquardt damping are the fallback. Jacobians are
   exact (forward-mode automatic differentiation). All arithmetic is deterministic; trigonometry
   uses the pure-Rust `libm`.
4. Target: a connected sketch of 200 entities solves in under 50 ms (release build; the corpus test
   `sketch_200_entities` checks it).

### Degrees of freedom and diagnostics

After solving, the rank of the constraint Jacobian gives the degrees of freedom of the sketch and,
per entity, how many independent motions remain.

| Code | Severity | Meaning |
| --- | --- | --- |
| `I4201` | info | The sketch is under-constrained: total degrees of freedom and the entities with some left. This is normal and not a failure. |
| `W4102` | warning | A constraint adds nothing to the constraints before it (redundant but consistent). |
| `E4101` | error | Constraints that cannot all be satisfied. The report is a *minimal* set: removing any one of the named constraints makes the rest satisfiable. The later constraint is the primary location and the others are listed as related locations. |
| `E4301` | error | The geometry itself cannot be solved near its starting position. |
| `E4001`, `E4002`, `E4003` | error | Invalid entity argument, unknown constraint, invalid constraint arguments. |
| `W4601` | warning | Projected geometry is resolved by the kernel and ignored by the solver. |

Sketches are solved for the base part and again for every configuration; a problem that exists in the
base part is reported once.

### Hints and `tcad fmt --solve`

`tcad fmt --solve` solves all sketches (under the `default` configuration) and writes the result into
the file as `@hint(v1, v2, ...)` on every entity, in the variable order of the table above (lengths
in mm, angles in degrees, rounded to a nanometre). Regeneration then starts from the same solution:
the branch of a two-solution constraint is the one the file stores, and the solver needs few
iterations. A hint is a starting value of medium strength: an explicit coordinate in the arguments
overrides it. The command is idempotent.

## Not yet specified

Splines take part in constraints only through their points (no tangency or on-curve for the curve
itself); the kernel (M5) decides how a sketch becomes a profile and how `project` and the sketch
plane resolve.
