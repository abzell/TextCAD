# 0016. Format versions: keyword gating by header and `tcad migrate`

- Status: accepted
- Date: 2026-11-26

## Context

Requirements 4.1: "Parsers reject unknown major versions and warn on newer minors. Schema evolution
rules live in `spec/versioning.md`; every breaking change ships a migration in `tcad migrate`." The
Phase 2 work (`spec/assembly.md`) and the part-gap issues add part-level keywords (`meta`,
`connector`, `publish`, `uses`, ...). ADR 0004 makes keywords contextual, but a declared name may
not be a keyword in its scope, so a new keyword turns an older file that uses the word as a name
into an error. That would break the promise that minor versions only add.

## Options

1. **Reserve every future word now** in 0.1. Impossible to know them all; it also blocks good names.
2. **New keywords everywhere, a migration for old files.** Simple, but a minor version would break
   existing files until they are migrated.
3. **Gate keywords by the header version.** The reader applies the keyword set of the version the
   file declares; old files stay valid unchanged; `tcad migrate` moves them forward and renames
   clashes.

Questions for option 3: what `tcad fmt` does with the header; what version new files get; how a
migration proves it changed nothing; what to do when a rename cannot be decided from the text.

## Decision

- Option 3. `crates/tcad-syntax/src/keywords.rs` holds one table of versioned part-level keywords
  with the version that introduced each and whether it is implemented. `is_keyword_in(word, scope,
  version)` is the single test; the parser uses the header's version (the newest when the header is
  broken), the language server's rename and hover use the newest (so renames never create future
  clashes).
- A statement that starts with a keyword newer than the header is `E0103` with a fix hint that
  changes the version number. A known but unimplemented keyword is `E0001`, as reserved words are.
- `tcad fmt` never changes the header. New files (MCP `create_part`, the missing-header fix hint)
  get the newest version.
- `tcad migrate` (in `tcad-model`, shared by the CLI) renames clashing part-level names to
  `<name>_<n>` using the same reference rule as the language server's rename (now one function,
  `tcad_model::migrate::top_level_references`), rewrites the header, then re-analyzes the result
  and requires every configuration to evaluate to the same parameters and suppressions and the
  feature list to match. Any difference aborts without writing.
- A clashing name that is also a sketch entity name is refused: inside that sketch a bare word is the
  entity in a point position (`center=meta`) and the parameter in an expression (`r=meta`), which
  the token stream alone cannot tell apart. The first implementation tried to skip such tokens and
  the verification step caught the resulting error, which is how this case was found.

## Consequences

- Every feature that adds a part-level keyword adds a row to the table and to
  `spec/versioning.md`, and flips `implemented` when it lands.
- Migration is conservative: it may refuse a file that a human could migrate; it never writes a file
  that means something else.
- Feature names that are renamed change the prefix of their semantic IDs; references in the same
  file follow, but external consumers of cached names (viewers, PMI later) see new names. This is
  inherent in renaming.
