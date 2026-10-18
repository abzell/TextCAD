# 0007. Sketch solver design and sketch semantics

- Status: accepted
- Date: 2026-10-18

## Context

Section 7.1 requires a pure-Rust solver (Newton-Raphson with a Levenberg-Marquardt fallback on a
sparse system, independent clusters, DOF per entity, named conflicts, nearest-solution stability,
hints, 200 entities in under 50 ms). It does not say what a sketch's coordinate arguments mean, how
conflicts are minimized, or how tangency at a shared end point is handled.

## Decisions

1. **Exact Jacobians by forward-mode automatic differentiation** over at most eight local variables
   per equation. Every constraint is a small residual function; there are no hand-written
   derivatives to get wrong. Residuals are lengths or radians, so a single tolerance works.
2. **Step solver.** The linearized system is solved for the minimum-weighted-norm step with an
   envelope Cholesky factorization after reverse Cuthill-McKee ordering. For fewer equations than
   variables the step is `A^T (A A^T + mu I)^-1 b`, which lies in the row space by construction, so
   round-off cannot drift along free directions (a first version using the primal normal equations
   did, by about 1e-7). Conjugate gradients (CGLS) was the first implementation but needed hundreds of
   iterations on chains and missed the 50 ms target by 10x; it remains as a fallback.
3. **Globalization.** Newton step, then backtracking, then LM damping. Stop at 1e-11; accept at 1e-8.
4. **Clusters** by union-find on shared variables. A conflict is isolated by a deletion filter over
   the constraints of a failing cluster, giving a minimal set; feasibility never depends on weights.
5. **Authored coordinates are soft, dimensions are hard, references share points.** Starting values
   that the author wrote are *pinned*: held exactly in a first attempt, which both preserves them and
   decouples clusters; only if that fails are they released with large weights. Values the tool has
   to invent (a three-point arc's center) are very soft, so authored geometry does not give way to a
   guess. Stored hints sit between the two. Without this, `concentric(nose, pin)` moved the authored
   pin to meet a guessed arc center.
6. **Tangency at a known contact point** uses perpendicular-radius / collinear-centers equations. The
   general distance form is first-order dependent with "end point on circle" and "end point on line"
   and made fully constrained tangent chains report phantom degrees of freedom.
7. **Rank and redundancy** come from the same envelope machinery applied to the Gram matrix of the
   Jacobian rows in source order (the pivot is the squared distance of a row from the earlier rows),
   so analysis of a connected 200-entity sketch costs milliseconds instead of the cubic dense
   elimination used at first (40 s in a debug build).
8. **Per-entity degrees of freedom** are `|V| - rank(J) + rank(J without the columns of V)`.
9. **Information severity.** Under-constrained sketches are normal; they are reported as `info`
   (code `I4201`), which never fails a command. Diagnostics gained related locations to name the
   other halves of a conflict.
10. **`fix(p)`** holds a point where it is written, not at a stated coordinate.

## Consequences

The solver crate has no language dependency (the bridge lives in `tcad-model`), so the kernel and
other tools can reuse it. Splines participate through their points only. The pinned-first strategy
means a file with an inconsistent authored coordinate still solves (softly) and no diagnostic is
raised for it; only constraint conflicts are errors.
