# Versioning and migration

Status: **partial** (M2 covers the header). Source requirements: 4.1.

## Header

- Line 1 of a file (after optional comment-only lines) is `tcad <major>.<minor>`, both decimal
  integers. This document describes format version `0.1`.
- A reader rejects a different **major** version with `E0101`.
- A reader accepts a newer **minor** version of the same major version with warning `W0102`.
  Unknown constructs in it are reported as ordinary syntax errors.
- A missing or malformed header is `E1101`.

## To be specified

- Schema evolution rules: what may change in a minor version (new optional arguments, new keywords
  taken from the reserved list) and what needs a major version.
- Migrations for every breaking change, run by `tcad migrate`.
