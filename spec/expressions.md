# Expressions

Status: **normative** (M3). Source requirements: 4.3, 4.4. Grammar: [grammar.ebnf](grammar.ebnf).
Implemented by `tcad-expr` (`eval.rs`) and `tcad-model` (scope checks).

## Forms

Numbers, quantities (`25 mm`), strings, `true`/`false`, parameter names, parentheses, tuples
`(a, b)`, lists `[a, b]`, unary `-` and `not`, binary operators, and calls of the built-in functions.
There are no loops, recursion, user functions or I/O; evaluation always terminates.

## Precedence (lowest to highest)

`or`, `and`, `not`, comparisons (`== != < <= > >=`, non-associative), `+ -`, `* /`, unary `-`, `^`.
`^` is right-associative and binds tighter than unary minus on its left: `-a^2` is `-(a^2)`.

## Types

| Operation | Operands | Result |
| --- | --- | --- |
| `+ -` | same dimension | that dimension |
| `*` `/` | any | dimensions add / subtract |
| `^` | dimensionless exponent; for a dimensional base the exponent is a whole number | dimension multiplied by the exponent |
| comparisons | same dimension (or two strings / two booleans for `==` `!=`) | boolean |
| `and or not` | booleans | boolean |

Mismatches are `E2201` (dimension) or `E2202` (type). A tuple or list is a value but supports no
arithmetic.

## Functions

| Function | Arguments | Result |
| --- | --- | --- |
| `min(a, b, ...)`, `max(a, b, ...)` | one or more of one dimension | that dimension |
| `abs`, `floor`, `ceil`, `round` | one number | same dimension (`round` is half away from zero) |
| `sqrt` | one number whose exponents are all even, not negative | dimension halved |
| `sin`, `cos`, `tan` | an angle | dimensionless |
| `asin`, `acos` | dimensionless in [-1, 1] | angle |
| `atan` | dimensionless | angle |
| `atan2(y, x)` | same dimension | angle |

Wrong arity or an unknown function is `E2206`; an invalid root or exponent is `E2204`.

## Names and scopes

- Parameters, sketches, reference geometry, features, tags and imports share one namespace per part;
  a name may be declared once (`E2101`). Sketch entities live in their sketch's namespace and may reuse
  a part-level name; inside the sketch the entity wins once it is declared.
- **References only point backward.** A name may be used only after its declaration in the file
  (`E2103` otherwise, `E2102` if it never appears). Inside its own declaration a name is not yet
  declared. In a sketch, entities may reference earlier entities in their arguments; `constrain`
  statements may reference any entity of the sketch.
- The built-in names `plane`, `axis` and `origin` are always available.
- A parameter expression may only use value names: other parameters. Using a feature or sketch name
  as a value is `E2104`.
- The head of a geometric reference (`side.pin.center`, `slot.side(+Y)`) is scope-checked here; its
  roles are checked against the feature's roles in M6 ([naming.md](naming.md)).
- Close misspellings produce `E2102` with a fix hint ("did you mean `jaw_gap`?").

## Enumerated arguments

Some arguments take a fixed set of words instead of a reference (provisional until
[features.md](features.md)): `op` of `extrude revolve sweep loft` (`add subtract intersect`), `type` of
`hole` (`simple counterbore countersink tapped`), `end` of `hole` (`through_all blind`) and `type` in
`query(...)` (`plane cylinder cone torus bspline`). Another word is `E2501`.

## What `tcad check` evaluates

Parameters, ranges, configuration overrides, `suppress if` conditions, and every argument, hint and
constraint expression that is made of literals and parameters only. Expressions that mention
geometry (`outline.right`) cannot be evaluated before the kernel runs; their value parts are still
checked as far as they go.
