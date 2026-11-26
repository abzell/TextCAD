# 0016. Format versions: keyword gating by header, and migrations

- Status: accepted
- Date: 2026-11-26

## Context

Requirements 4.1: readers reject unknown major versions and warn on newer minors; schema evolution
rules live in `spec/versioning.md`; every breaking change comes with a migration. The Phase 2 work
(`spec/assembly.md`) and later features add part-level keywords (`meta`, `connector`, `publish`,
`uses`, ...). ADR 0004 makes keywords contextual, but a declared name may not be a keyword in its scope,
so a new keyword turns an older file that uses the word as a name into an error. That would break the
promise that minor versions only add.

## Options

1. **Reserve every future word now** in 0.1. Impossible to know them all; it also blocks good names.
2. **New keywords everywhere, a migration for old files.** Simple, but a minor version would break
   existing files until they are migrated.
3. **Gate keywords by the header version.** A reader applies the keyword set of the version the file
   declares; old files stay valid unchanged; a migration moves them forward and renames clashes.

## Decision

- Option 3. Each versioned keyword is listed in `spec/versioning.md` with the version that introduced
  it. In older files it is an ordinary name.
- A statement that starts with a keyword newer than the header is `E0103`, with a fix hint that changes
  the version number. A keyword whose statement a tool does not support yet is `E0001`, as reserved words
  are.
- Formatting never changes the header; tools that create new files write the newest version.
- A migration renames clashing part-level names to `<name>_<n>` with every reference, rewrites the
  header, and must leave every configuration's meaning unchanged; it changes nothing otherwise.
- A clashing name that is also a sketch entity name is refused by a migration: inside that sketch a
  bare word is the entity in a point position (`center=meta`) and the parameter in an expression
  (`r=meta`), which the text alone cannot tell apart.

## Consequences

- Every format change that adds a part-level keyword adds a row to the table in `spec/versioning.md`.
- Migration is conservative: it may refuse a file that a person could migrate; it never produces a
  file that means something else.
- Renamed features change the prefix of their semantic IDs; references in the same file follow.
