# Bundle layout

Status: **normative** (format 0.1). Source requirements: 5.

A design is a directory, the **bundle**. Its `.tcad` text files are the source of truth.

```
rod_clevis/                 bundle directory
  bundle.toml               bundle layout version, entry part, units, license, metadata
  parts/
    rod_clevis.tcad         one part per file
  imports/                  STEP files brought in with `import` (spec/io.md)
```

Phase 2 adds `assemblies/`, `skeletons/`, `library/` and `bundle.lock` (spec/assembly.md).

## `bundle.toml`

| Key | Meaning |
| --- | --- |
| `format` | bundle layout version, `"0.1"` (separate from the language version in each file's header, spec/versioning.md) |
| `entry` | the entry part, relative to the bundle directory |
| `name`, `units`, `license` | optional metadata |

A lone `.tcad` file without a `bundle.toml` is a bundle whose root is the file's directory and whose
entry is that file.

## Archive

A bundle may be archived as a single `.tcadz` file: a ZIP of the bundle directory with `bundle.toml` at
the root of the archive. Paths in the archive are relative, use `/`, and never contain `..`.

## Source hash

A **source hash** identifies the exact source of a part under one configuration. It is the SHA-256 over
length-prefixed pieces: a version label, the **canonical text** of the part (spec/canonical-form.md,
so changes in formatting or comments that do not change the canonical form do not change the hash), the
resolved configuration name, and the bytes of each imported file. It is used by `bundle.lock`
(spec/assembly.md) and by tools that cache derived results.

## Derived data

Tools may keep derived data (regenerated geometry, meshes, reports) next to the sources, for example in
a `.cache/` directory. Such data is **not part of the format**: it can always be deleted and recreated
from the sources, and a tool must never read it as a source. Exchange files such as STEP are produced
by tools on request; the format does not require them (spec/io.md).
