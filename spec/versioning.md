# Versioning and migration

Status: **normative** (format 0.2). Source requirements: 4.1. Implemented by `tcad-syntax`
(header, keyword gating), `tcad-model` (`migrate`) and `tcad migrate`. Decisions: ADR 0016.

## Header

- Line 1 of a file (after optional comment-only lines) is `tcad <major>.<minor>`, both decimal
  integers. This document describes format versions `0.1` and `0.2`; the newest is `0.2`.
- A reader rejects a different **major** version with `E0101`.
- A reader accepts a newer **minor** version of the same major version with warning `W0102`.
  Unknown constructs in it are reported as ordinary syntax errors.
- A missing or malformed header is `E1101`; its fix hint inserts the newest version.
- A file is read with the rules of the version its header declares (below). Tools never change the
  header on their own: `tcad fmt` keeps it, only the author or `tcad migrate` changes it. Tools that
  create new files (the MCP server's `create_part`) write the newest version.

## What a minor version may change

A minor version (0.1 → 0.2) may only **add**:

- statements and keywords (contextual, ADR 0004), including words taken off the reserved list
  ([reserved.md](reserved.md)),
- optional arguments and enum values,
- diagnostics.

Every file that is valid under version 0.*m* is valid under 0.*m+1* **with its own header** and
means exactly the same: the same canonical text, the same evaluated parameters and the same geometry
(the same canonical B-rep, tier T1, spec/kernel.md).

## What needs a major version

- Removing or renaming a statement, argument or enum value.
- Changing a default, a semantic-ID role (spec/naming.md), a sign or frame convention, or the
  geometry that the same text produces.
- Making a previously valid file invalid, other than by the keyword rule below.

Every breaking change ships a migration in `tcad migrate`.

## Keywords added in a minor version

A new keyword would make older files that use the same word as a declared name invalid. Therefore
keywords are **gated by the header**:

- A keyword introduced in version *v* is a keyword only in files whose header declares *v* or
  later. In older files it is an ordinary identifier and may be a declared name.
- In a file that declares *v* or later, declaring the word as a name is `E1104`.
- A statement that starts with a keyword newer than the header is `E0103` ("requires format
  *v*"). Its fix hint changes the version number in the header; `tcad migrate` does the same and
  also renames clashing names.
- A keyword that the format has introduced but this implementation does not support yet is
  `E0001` (not yet supported), the same as a reserved word.

| Keyword | Context | Introduced | Chapter | Implemented |
| --- | --- | --- | --- | --- |
| `meta` | part | 0.2 | [assembly.md](assembly.md) § `meta` | not yet (#9) |
| `connector` | part | 0.2 | [assembly.md](assembly.md) § `connector` | not yet (#28) |
| `publish` | part | 0.2 | [assembly.md](assembly.md) § `publish` | not yet (#28) |
| `uses` | part | 0.2 | [assembly.md](assembly.md) § Skeleton | not yet (#41) |

The table lives in `crates/tcad-syntax/src/keywords.rs` (`VERSIONED_PART_KW`); a feature that adds a
keyword adds a row here and there in the same change. Keywords that only have a meaning inside a new
block (for example `ground` inside `assembly`) cannot clash with older files and need no row.

## `tcad migrate`

```
tcad migrate [--to 0.2] [--check] [--json] <path>...
```

- Rewrites each file's header to the target version (default: the newest) and renames every
  part-level declared name (parameter, sketch, feature, reference geometry, tag, import,
  configuration) that is a keyword of the target version but not of the file's version. The new
  name is `<name>_1` (or `_2`, ... if taken); every reference is renamed with it, including
  configuration overrides and suppression lists. Sketch entities and argument names are not names
  in that scope and are not touched.
- The result is checked before anything is written: it must analyze without errors and every
  configuration must evaluate to the same parameters and suppressions under the new names, with the
  same feature list. Geometry is therefore unchanged apart from the renamed feature prefixes of
  semantic IDs.
- It refuses (status `refused`, exit code 1, nothing written):
  - files with errors (they are reported),
  - files without a valid header,
  - a target older than the file (no downgrades) or unknown to the tool,
  - a clashing name that is also the name of an entity in a sketch, because inside that sketch a
    bare word may mean either; the user renames one of them first.
- `--check` writes nothing and exits 1 if any file would change. `--json` reports per file
  `status` (`unchanged`, `would_change`, `migrated`, `refused`, `error`), `renames`
  (`[{ "from", "to" }]`) and, when refused, `message`.
- A file that is already at the target version is `unchanged`.

## Bundle and cache formats

`bundle.toml`'s `format` and `manifest.json`'s `format` are the versions of the **bundle layout** and
the **cache manifest** (spec/bundle.md). They are separate from the language version in the header
and change on their own schedule.
