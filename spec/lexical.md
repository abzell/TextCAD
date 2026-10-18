# Lexical structure

Status: **normative** (M2). Source requirements: 4.2. Grammar: [grammar.ebnf](grammar.ebnf).

## Source text

- A source file is UTF-8 text with LF line endings. A byte order mark (U+FEFF) or a carriage return
  (U+000D) anywhere outside a string is error `E1005`.
- A source file is a sequence of tokens separated by trivia. Trivia are spaces, tabs and comments.
  A line break is a token (`NEWLINE`), not trivia; see "Statement termination".

## Comments

- `#` starts a line comment that runs to the end of the line.
- `/* ... */` is a block comment. Block comments do not nest. An unclosed one is `E1003`.
- Comments are kept in the syntax tree and preserved by the formatter. A comment on the same line
  as a statement, after it, is that statement's trailing comment.
- Exception: a `#` that directly follows a token with no whitespace and is directly followed by a
  digit is the ordinal suffix of a reference (`boss.side(profile.c1)#2`), not a comment. To start a
  comment right after code, put a space before the `#`.

## Identifiers and keywords

- An identifier matches `[a-z_][a-z0-9_]*`. Uppercase letters inside a lowercase-initial word are
  `E1006`.
- A word that starts with an uppercase letter is an *upper name*. The only valid upper names are the
  axis and plane names `X Y Z XY YZ XZ`, written after a `.` (`plane.XZ`, `axis.Y`) or with a sign
  as a direction (`+Y`, `-Z`). Any other upper name is `E1202`.
- Keywords are **contextual**. The lexer produces identifiers; the parser gives a word its keyword
  meaning by position (the first word of a statement, or a clause word such as `from` or `on`).
  Hence `plane.XZ` is a reference although `plane` also starts a statement.
- A *declared name* (of a part, parameter, sketch, entity, feature, tag, config or import) may not
  be a keyword that is meaningful where it is declared: statement keywords, clause words
  (`from on suppress if range doc default construction true false and or not query origin`),
  built-in function names and all words reserved for later phases (see [reserved.md](reserved.md)).
  Sketch entity words (`line arc circle ...`) are reserved only inside sketches, so `slot` is a fine
  feature name. Violations are `E1104`. Unit names are not keywords.

## Numbers

- A number is decimal digits, an optional fraction (`.` and digits) and an optional exponent
  (`e` or `E`, optional sign, digits). There is no sign, no leading `.` and no trailing `.`.
- The exponent is only part of the number if digits follow, so `2e` is the number `2` and the word `e`.
- A number is written exactly as in the source in the tree; the canonical form normalizes it
  (see [canonical-form.md](canonical-form.md)).

## Strings

- A string is delimited by `"` and cannot contain a line break. Escapes: `\"`, `\\`, `\n`, `\t`.
  Anything else after a backslash is `E1004`. A missing closing quote is `E1002`.

## Operators and punctuation

`{ } ( ) [ ] , ; = == != < <= > >= . # + - * / ^ & @`. Any other character is `E1001`.

## Statement termination

- A statement ends at a line break, at `;`, or implicitly before the `}` that closes its block and at
  the end of the file.
- Line breaks are insignificant inside `( )` and `[ ]`, at any nesting depth, and nowhere else. A
  long argument list therefore continues on the next line only inside brackets.
- A line break directly after `;` belongs to the `;`. Blank lines and comment-only lines between
  statements are preserved by the formatter (at most one blank line in a row).

## Units

A unit name (`mm cm m in ft deg rad kg g lb`) is recognized only directly after a number. Composite
units are built with `*`, `/` and `^` with no whitespace on either side: `7.85 g/cm^3`. With
whitespace the operator is ordinary arithmetic: `10 mm / 2`. A unit name directly followed by `=`
is an argument name instead (`depth=45 m=subtract` is the number 45 and an argument `m`). The unit algebra itself is in
[units.md](units.md).
