# 0009. Kernel trait, profile model and the build crate

Status: accepted (M5)

## Decision
- `Kernel` (tcad-kernel) is the only interface the rest of the system uses to make geometry. Inputs
  are plain data (frames, profiles, paths); outputs are opaque shapes with `properties` and a
  canonical serialization. The contract is in spec/kernel.md.
- Profiles are built in `tcad-build`, not in the kernel: sketch entities become chained segments, loops
  nest by containment, and all loops are counterclockwise so backends agree.
- Features run in declaration order and stop at the first failure (E5xxx); nothing is skipped silently.
- Determinism tiers: T1 bit-identical canonical B-rep for the same backend; T2 tolerance-equivalent
  across backends. T2 is tested against analytic values and a build123d cross-check in CI only.
- The cache (tcad-cache) stores the canonical B-rep plus a manifest; the key is a SHA-256 of canonical
  text, configuration, and imports, and the backend id.

## Consequences
Adding a feature means adding the schema to spec/features.md, a handler in tcad-build, and, only if
a new primitive operation is needed, a trait method plus its mock/OCCT implementations.
