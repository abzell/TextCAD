# 0006. Canonical units, angle dimension and pure-Rust math

- Status: accepted
- Date: 2026-10-18

## Context

Requirements 4.4 demands explicit units and compile-time unit mismatches; 8.2 demands bit-identical
results (tier T1) across Windows and WASM builds. Section 4.4 does not say how angles behave or how
trigonometry is computed.

## Options

1. Treat angles as dimensionless (radians), so `sin(0.5)` is legal.
2. Make angle a dimension, so `sin(30)` is an error and `sin(30 deg)` is required.
3. Use the platform's `f64::sin` and friends, or the pure-Rust `libm` crate.

## Decision

- Angle is its own dimension (option 2): it catches the commonest unit slip in CAD, a bare number
  where degrees were meant.
- Values are stored in canonical units (mm, kg, rad). Conversion happens once when a literal is read.
- Trigonometric functions and non-integer powers use `libm` (MIT/Apache-2.0, pure Rust) and integer
  powers use repeated squaring, so T1 does not depend on the platform's math library.
- Parameter expressions are constant, so evaluating them is also type-checking; there is no separate
  type checker. Every configuration is evaluated by `check`.
- Reference-valued parts of an expression (`outline.right`) are not values until the kernel runs;
  the evaluator reports them as "not a value" and the model ignores that outside parameters.

## Consequences

`tcad check` finds unit and range errors in all variants without any geometry. Specs for angle
arguments must say whether a dimensionless number is allowed; the default is that it is not.
