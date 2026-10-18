# Values and units

Status: **normative** (M3). Source requirements: 4.4. Implemented by `tcad-expr` (`dim.rs`).

## Dimensions

A value has a dimension: integer exponents of **length**, **mass** and **angle**. Angle is a
dimension of its own, so `30 deg` and a bare `30` can never be mixed. A value with all exponents
zero is *dimensionless* (counts, ratios).

## Units

| Unit | Dimension | Canonical factor |
| --- | --- | --- |
| `mm` | length | 1 mm |
| `cm` | length | 10 mm |
| `m` | length | 1000 mm |
| `in` | length | 25.4 mm |
| `ft` | length | 304.8 mm |
| `deg` | angle | pi/180 rad |
| `rad` | angle | 1 rad |
| `kg` | mass | 1 kg |
| `g` | mass | 0.001 kg |
| `lb` | mass | 0.45359237 kg |

Composite units are built with `*`, `/` and `^` (integer exponent) with no whitespace around the
operators (see [lexical.md](lexical.md)): `kg/m^3`, `g/cm^3`. Each operator applies to the term that
follows it, left to right. The dimension of `kg/m^3` is mass per length cubed, called *density*.

## Canonical storage

All quantities are stored and computed in **millimetres, kilograms and radians**. A literal is
converted once, by multiplying by the factor above. A dimensionless number is stored as written.
Display units in messages are mm, degrees, kg and kg/m^3.

## Rules

- Every length, angle, mass and density literal carries a unit. A bare number is dimensionless.
- `+`, `-`, comparisons, `min`, `max` need operands of equal dimension; otherwise `E2201`.
- `*` adds and `/` subtracts exponents. Exponents outside the range -128..127 are `E2204`.
- A result must be finite; overflow, `NaN` and division by zero are `E2205`.
- The modeling tolerance is declared with `tolerance <length>` and defaults to `1e-6 mm`. It must be
  a positive length (`E2302`) and may be declared once (`E2303`).
- A material's density, `material "name" density=<density>`, must be a positive density (`E2305`).

## Determinism

Arithmetic uses IEEE 754 double precision with no fused operations. `sin`, `cos`, `tan`, `asin`,
`acos`, `atan`, `atan2` and non-integer powers come from the pure-Rust `libm` crate and integer powers
use repeated squaring, so results are bit-identical on every platform (determinism tier T1).
