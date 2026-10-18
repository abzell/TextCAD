# 0004. Syntax decisions for the parser

- Status: accepted
- Date: 2026-10-18

## Context

The M1 grammar draft listed six open questions (end of `spec/grammar.ebnf`). Parsing the section 15
example also showed two conflicts: `#` is both the comment marker and the ordinal suffix
(`boss.side(profile.c1)#2`), and the example uses `slot` as a feature name although it is a sketch
entity word.

## Options and decisions

1. **Statement termination.** Newline-terminated with `;` as an optional separator, versus explicit
   `;` everywhere. Chosen: newline, because it keeps diffs line-oriented (goal 3). Line breaks are
   ignored inside `( )` and `[ ]` only. Continuation after a trailing operator or comma was rejected:
   it makes recovery after a half-typed line ambiguous.
2. **Units versus identifiers.** A unit is recognized only directly after a number. Composite units
   need no whitespace around `*`, `/`, `^` (`kg/m^3`). A parameter named `m` is therefore still legal
   but `5 m` is always metres.
3. **Comma lists.** `through=a, b` is accepted in statement arguments to match the requirements
   example, but not inside the parentheses of calls and queries, where the comma separates
   arguments. Bracketed lists work everywhere.
4. **Contextual keywords.** All words lex as identifiers and the parser recognizes keywords by
   position, so `plane.XZ` and `axis.Y` parse as references. A declared name may not be a keyword
   that is meaningful in the same scope; sketch entity words are reserved only inside sketches.
5. **Boolean operators.** `and`, `or`, `not` are added to the expression language (needed for
   `suppress if <bool expr>`).
6. **`#`.** It starts a comment unless it directly follows a token and directly precedes a digit,
   when it is the ordinal suffix. The formatter writes trailing comments after two spaces, so it
   never produces the ambiguous form.
7. **Axis names.** `X Y Z XY YZ XZ` are the only uppercase words; they lex as a separate token kind.
8. **Hint syntax.** `@hint(v, ...)` is the last item of a sketch entity.
9. **Argument order table.** Provisional and kept in `spec/canonical-form.md`; the per-feature
   chapters take it over when written (M5, M6).

## Consequences

The parser stays a hand-written recursive descent with statement-level error recovery, and the
formatter needs no source-layout information other than blank lines and comments.
