# Canonical form

Status: **normative** (M2). Source requirements: 4.6. Implemented by `tcad fmt`
(`tcad_syntax::format_source`).

A syntactically valid file has exactly one canonical text form up to the author's choice of blank
lines and comments. `tcad fmt` produces it, and `tcad fmt --check` (run by CI on the examples and
the corpus) fails when a file differs. Formatting a file with syntax errors is refused.

## Rules

1. **Header.** `tcad <major>.<minor>`, then one blank line before anything else.
2. **Indentation.** 2 spaces per block level, no tabs. One statement per line.
3. **Spacing.** One space between the words of a statement. `=` has no spaces in `key=value`
   arguments and one space on each side in declarations (`param a = 1 mm`, `tag t = x.y`). No space
   inside brackets or before `,`; one after `,`. Binary operators have one space on each side,
   except `^`, which has none (`2^-1`). Unary `-` has none; `not` has one. `&` in a generator list
   has one on each side. A unit follows its number after one space (`25 mm`); composite units have
   none (`g/cm^3`).
4. **Numbers.** Normalized without floating point: leading zeros of the integer part and trailing
   zeros of the fraction are dropped (`040`, `25.50`, `25.0` become `40`, `25.5`, `25`), the
   exponent is lowercase `e` without `+` or leading zeros (`1E-06` becomes `1e-6`), and a zero
   exponent is removed.
5. **Arguments.** After the name of a reference-geometry, feature or sketch-entity statement,
   arguments are ordered: the `from` clause; known `key=value` arguments in the order of the
   per-keyword table below; unknown `key=value` arguments alphabetically; flags alphabetically; the
   flag `construction`; the clause `suppress if ...`; the solver `@hint(...)`. The same table orders
   the arguments inside `rect(...)` profile calls and `query(...)` (after its entity kind).
   Equal keys keep their relative order. Parentheses and brackets written by the author are kept.
6. **Config blocks.** A `config` block is written on one line, `config heavy { a = 1 mm; b = 2 mm }`,
   if it contains no comments and the line is at most 100 columns wide; otherwise one override per
   line. An empty block is `{ }`. All other blocks (`part`, `sketch`) are multi-line.
7. **Blank lines and comments.** Runs of blank lines collapse to one; blank lines at the start and end
   of a block are removed. Comments keep their text. A trailing comment is preceded by two spaces. A
   comment on its own line is indented like the statement that follows it. A comment inside brackets
   stays at its token; a line comment there moves the rest of the statement to a continuation line.
8. **End of file.** Exactly one trailing line break.

`format(format(x)) == format(x)` for every input that formats at all; this is tested on the examples,
by mutation tests and by the fuzz target.

## Argument order table (provisional)

The table is fixed per keyword in `crates/tcad-syntax/src/format.rs` and will move into the
chapters [sketch.md](sketch.md) and [features.md](features.md) as those are written.

| Keyword | Argument order |
| --- | --- |
| material | density |
| point | at |
| line | start, end |
| arc | center, start, end, through, tangent_to, r |
| circle | center, d, r |
| ellipse | center, a, b, angle |
| spline | points, through, closed |
| slot | start, end, center, w, length |
| rect | on, corner, center, w, h |
| polygon | center, sides, d, r |
| project | edges |
| plane | from, offset, angle, about, through |
| axis | through, from, direction |
| csys | origin, x, y |
| extrude | distance, depth, to, draft, op |
| revolve | axis, angle, op |
| sweep | path, guide, op |
| loft | sections, guides, op |
| hole | on, at, type, d, thread, depth, end |
| fillet | edges, r |
| chamfer | edges, d, angle |
| shell | faces, thickness |
| draft | faces, angle, pull, neutral |
| rib | profile, thickness, direction |
| linear_pattern | of, dir, count, spacing, skip |
| circular_pattern | of, axis, count, angle, skip |
| mirror | of, plane |
| query | type, normal, parallel_to, axis, radius, created_by, adjacent_to, nearest |
