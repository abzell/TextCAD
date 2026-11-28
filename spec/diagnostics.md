# Diagnostics

Status: **normative** (format 0.2). Source requirements: 4.7.

A file is **valid** if it has no diagnostic of severity `error`. The codes below are the format's
standard names for problems, so that every tool reports the same problem the same way and an agent or
a person can look a code up here. Implementations must report the listed condition as an error (or
warning) and should use the listed code; they may add tool-specific codes outside the ranges below.

## Shape

Every diagnostic has:

- a **stable code**: `E` (error) or `W` (warning) and four digits. A code is never renumbered or
  reused; retired codes stay in the registry.
- a **severity**: an `error` makes the file invalid; `warning` and `info` do not. Codes start with `E`, `W` or `I` accordingly.
- a **message** for humans.
- a **span**: a byte range in the source, reported with 1-based line and column of both ends
  (columns count Unicode scalar values).
- optional **related locations**: other spans that explain the diagnostic (the other halves of a
  conflict), each with a message.
- an optional **fix hint**: a message and a list of text edits (byte range plus replacement) that
  can be applied mechanically.



## Ranges

| Range | Area |
| --- | --- |
| E0xxx, W0xxx | General: unsupported features, versioning |
| E1xxx | Lexical and syntax |
| E2xxx | Types, units, parameters, expressions, configurations |
| E3xxx | Naming and references |
| E4xxx | Sketches and constraints |
| E5xxx | Features and geometry |
| E6xxx | Assemblies and connectors (spec/assembly.md: the codes listed here are normative, the others draft) |

## Codes

| Code | Meaning |
| --- | --- |
| E0001 | feature not yet supported (a keyword reserved for a later phase, or introduced by the declared format version but not implemented yet) |
| E0101 | unsupported format major version |
| W0102 | format minor version is newer than supported |
| E0103 | a statement starts with a keyword of a newer format version than the header declares (hint: update the header, or migrate the file, spec/versioning.md) |
| E1001 | unexpected character |
| E1002 | unterminated string |
| E1003 | unterminated block comment |
| E1004 | invalid escape sequence in string |
| E1005 | carriage return or byte order mark |
| E1006 | identifiers must be lowercase |
| E1101 | missing or malformed file header |
| E1102 | expected a specific token (hint: insert it, where that is unambiguous) |
| E1103 | unexpected token |
| E1104 | reserved word used as a name |
| E1201 | unknown statement |
| E1202 | unknown axis or plane name |
| E1203 | expected end of statement |
| E1204 | expected an expression |
| E1205 | unclosed block |
| E2101 | name declared more than once |
| E2102 | undefined name (with a "did you mean" hint when a close name exists) |
| E2103 | name used before its declaration |
| E2104 | name does not denote a value |
| E2201 | dimension mismatch |
| E2202 | type mismatch |
| E2204 | invalid exponent or root of a dimensional quantity; unit exponent out of range |
| E2205 | arithmetic error: division by zero or non-finite result |
| E2206 | wrong function name or number of arguments |
| E2301 | value outside the declared range |
| E2302 | invalid tolerance |
| E2303 | single-use statement declared twice |
| E2304 | invalid range |
| E2305 | invalid material |
| E2401 | configuration overrides an unknown parameter |
| E2402 | override does not match the parameter's type or dimension |
| E2403 | configuration suppresses something that is not a feature |
| E2404 | more than one default configuration |
| W2405 | configurations exist but none is marked default |
| E2501 | unknown value for an enumerated argument |
| E2601 | `suppress if` condition is not boolean |
| E2701 | a file must contain exactly one part (format 0.2: one part or one assembly) |
| E4001 | invalid sketch entity argument |
| E4002 | unknown sketch constraint |
| E4003 | invalid constraint arguments |
| E4101 | sketch constraints conflict (names a minimal conflicting set, with related locations) |
| W4102 | redundant sketch constraint |
| I4201 | sketch has degrees of freedom (information) |
| E4301 | sketch cannot be solved near its starting geometry |
| W4601 | projected geometry is not available before the kernel runs |
| E3001 | reference does not resolve (the message gives the nearest names) |
| E3002 | reference is ambiguous: a split face needs an ordinal |
| E3003 | query does not match the expected number of entities |
| E3004 | invalid query filter |
| E3005 | reference resolves to the wrong kind of entity |
| E5001 | invalid feature argument (unknown, missing, misplaced or conflicting) |
| E5002 | reference does not resolve to the right kind of geometry |
| E5003 | feature not yet supported |
| E5004 | the kernel failed to build the feature |
| E5005 | profile problem (open chain, self-intersection, empty sketch) |
| E5006 | the first solid feature must add material, or the feature needs an existing body |
| E6001 | reference to an unknown instance, connector, published entity or datum (with nearest names) |
| E6002 | referenced file not found, outside the bundle, or not a part / assembly as the call says |
| W6003 | a file reachable from the entry differs from `bundle.lock` or is not listed in it (the message names the file) |
| E6003 | the same as `W6003` in strict mode, and a missing or unreadable `bundle.lock` in strict mode |
| E6004 | reference cycle between files (the message lists the cycle) |
| E6005 | an instance's `rev=` differs from the referenced part's `meta revision` under the configuration it builds |
| E6006 | `config=` names a configuration the referenced file does not declare |
| E6011 | connector axes are parallel or degenerate (x parallel to z, or a zero direction), or a zero rotation axis |
| E6012 | an assembly has no grounded instance |
| I6014 | a connector or published entity is suppressed with the feature it refers to (information) |

## Recovery (recommended)

A parser should report several errors per file: after a syntax error, skip to the end of the
statement (a line break outside nested braces) and continue, so later statements are still checked.
A file with any error has no canonical form.
