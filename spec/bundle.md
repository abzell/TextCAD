# Bundle layout and cache

Status: **normative** (M5). Source requirements: 5. Implemented by `tcad-cache`.

A design is a directory. The `.tcad` text files are the source of truth; everything under `.cache/`
is derived and may be deleted.

```
rod_clevis/                 bundle directory
  bundle.toml               format version, entry part, units, license
  parts/
    rod_clevis.tcad         one part per file
  imports/                  imported STEP files (M7)
  .cache/                   derived; git-ignored by default, committable for consumers
    rod_clevis/
      brep.brep             the result in the kernel's canonical serialization
      manifest.json         hashes, kernel id and version, per-feature topology
      result.step           the exact result as STEP AP242 (spec/io.md)
      mesh.glb              the viewer mesh, one primitive per face with its semantic ID
```

## `bundle.toml`

| Key | Meaning |
| --- | --- |
| `format` | bundle layout version, `"0.1"` |
| `entry` | the entry part, relative to the bundle directory |
| `name`, `units`, `license` | optional metadata |

`tcad build` accepts a bundle directory, a part file inside a bundle (it finds `bundle.toml` one or two
directories up) or a lone `.tcad` file (its directory is then the bundle root).

## Source hash

SHA-256 over length-prefixed pieces: a version label, the **canonical text** of the part (`tcad fmt`
output, so formatting and comments that do not change the canonical form still change the hash only if
the canonical text changes), the resolved configuration name, and the bytes of each imported file.

## `manifest.json`

```
{ "format": "0.1", "part": "...", "config": null | "name", "source_hash": "...", "kernel": "occt-7.6.3",
  "tolerance_mm": 1e-6,
  "features": [ { "name", "keyword", "solids", "faces", "edges", "vertices", "volume_mm3",
                  "thread"?, "topology"? } ],
  "result": { "solids", "faces", "edges", "vertices", "volume_mm3", "area_mm2",
              "center_of_mass_mm", "bbox_min_mm", "bbox_max_mm" },
  "brep_sha256": "..." }
```

## Invalidation

A cache entry is valid only if the manifest's source hash and kernel id/version both match **and**
`brep.brep` hashes to `brep_sha256`; otherwise it is regenerated. Consumers without a kernel (viewers,
PLM systems, agents) read the cached results directly: recipe plus result.

`topology` is the **topology map**: after each solid-changing feature, every face and edge of the
result as `{ "kind": "face" | "edge", "id", "aliases"?, "geometry", "size", "at" }` with the primary
semantic ID (ordinal included), the other IDs, the surface or curve type, the area (mm^2) or length
(mm) and the centroid or midpoint (mm). `thread` is the designation of a tapped hole.
