# Versioning and migration

Status: **normative** (format 0.2). Source requirements: 4.1. Decisions: ADR 0016.

## Header

- Line 1 of a file (after optional comment-only lines) is `tcad <major>.<minor>`, both decimal
  integers. This document describes format versions `0.1` and `0.2`; the newest is `0.2`.
- A reader rejects a different **major** version with `E0101`.
- A reader accepts a newer **minor** version of the same major version with warning `W0102`.
  Unknown constructs in it are reported as ordinary syntax errors.
- A missing or malformed header is `E1101`; its fix hint inserts the newest version.
- A file is read with the rules of the version its header declares (below). Tools never change the
  header on their own: formatting keeps it; only the author or a migration (below) changes it. Tools
  that create new files write the newest version.

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

Every breaking change is published together with the rules of its migration (below).

## Keywords added in a minor version

A new keyword would make older files that use the same word as a declared name invalid. Therefore
keywords are **gated by the header**:

- A keyword introduced in version *v* is a keyword only in files whose header declares *v* or
  later. In older files it is an ordinary identifier and may be a declared name.
- In a file that declares *v* or later, declaring the word as a name is `E1104`.
- A statement that starts with a keyword newer than the header is `E0103` ("requires format
  *v*"). Its fix hint changes the version number in the header; a migration does the same and also
  renames clashing names.
- An implementation that does not support a keyword's statement yet (for example while the chapter
  is a draft) reports it as `E0001` (not yet supported), the same as a reserved word.

| Keyword | Context | Introduced | Chapter | Status |
| --- | --- | --- | --- | --- |
| `meta` | part | 0.2 | [features.md](features.md) § Part metadata | normative; also `meta.<key> = ...` in configurations |
| `connector` | part | 0.2 | [features.md](features.md) § Connectors | normative |
| `publish` | part | 0.2 | [features.md](features.md) § Published geometry | normative |
| `uses` | part | 0.2 | [assembly.md](assembly.md) § Skeleton | draft |

A change to the format that adds a part-level keyword adds a row to this table. Keywords that only
have a meaning inside a new block (for example `ground` inside `assembly`) cannot clash with older files and need no row.

## Migration

A **migration** moves a file to a newer version of the format. Tools that offer one must:

- rewrite the header to the target version;
- rename every part-level declared name (parameter, sketch, feature, reference geometry, tag,
  import, configuration) that is a keyword of the target version but not of the file's version. The
  new name is `<name>_1` (or `_2`, ... if taken), and every reference is renamed with it, including
  configuration overrides and suppression lists. Sketch entities and argument names are not names in
  that scope and are not touched;
- leave the meaning unchanged: the result must be valid, every configuration must evaluate to the
  same parameters and suppressions under the new names, and the feature list must be the same. The
  geometry is then unchanged apart from the renamed feature prefixes of semantic IDs;
- refuse, and change nothing, for: files with errors; files without a valid header; a target older
  than the file or unknown to the tool; and a clashing name that is also the name of an entity in a
  sketch (inside that sketch a bare word may mean either, so the author renames one of them first).

A file already at the target version is unchanged by a migration.

## Bundle format

`bundle.toml`'s `format` is the version of the **bundle layout** (spec/bundle.md). It is separate from
the language version in each file's header and changes on its own schedule.
