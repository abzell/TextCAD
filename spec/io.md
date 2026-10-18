# Import and export

Status: **normative** (M7). Source requirements: 9. Implemented by the kernel (`export_step`,
`import_step`, `mesh`: spec/kernel.md), `tcad-io` (STL, 3MF, GLB) and `tcad export`.

All exports are in **millimetres** and describe the result of the part under one configuration.

## STEP AP242

`tcad export <part> --to step` writes the exact B-rep as AP242 (schema `AP242DIS`), one body per solid.
It records:

| Content | Where |
| --- | --- |
| units | millimetres |
| part name | the product name of the shape |
| material | a material with name and density (g/cm^3), if the part declares one |
| configuration | the file description of the header (`TextCAD part <name>, configuration <config>`) |

The header carries a time stamp, so two exports differ in that line only; compare geometry, not bytes.
The cache holds the same export as `.cache/<part>/result.step`.

## STL

Binary STL (80-byte header `TextCAD <part>`, 32-bit little-endian floats, zero attribute bytes), one
triangle list for the whole result, normals from the triangle winding (counterclockwise seen from
outside). Tolerances: `--linear <mm>` chordal deflection (default 0.05) and `--angular <deg>`
(default 10).

## 3MF

A 3MF package (`3D/3dmodel.model`, unit `millimeter`) with one object, the part name as `Title`, the
configuration as metadata and the material name as a base material. The ZIP is written without
compression and with a fixed date, so equal meshes give equal bytes.

## GLB

Binary glTF 2.0 for the viewer: **one primitive per face**, each with `extras` `{ "face", "id",
"aliases"? }` where `id` is the face's primary semantic ID (spec/naming.md) and `face` its index. The
node and mesh carry the part name and configuration. `.cache/<part>/mesh.glb` is written by `tcad build`
with the default tolerances.

## STEP import

```
import body = step("imports/body.step")
combine fit with=body at=(10 mm, 0 mm, 0 mm) op=subtract
```

`import` reads a STEP file from the bundle (the path is relative to the bundle root and may not leave
it; the build is refused otherwise). The solids come in as one **dead** shape without history. Its faces are named
`body.face#n`, numbered by the kernel-independent rule of spec/naming.md (centroid x, y, z, then area).
The file's bytes are part of the source hash, so editing the file invalidates the cache.

Nothing is added to the part until a `combine` feature uses the import:

| Argument | Meaning |
| --- | --- |
| `with=` | the import (`E5002` if it is not an earlier, unsuppressed `import`) |
| `at=(x, y, z)` | optional translation applied to a copy of the solid |
| `op=` | `add` (default; the first solid feature may be a `combine`), `subtract`, `intersect` |

After the `combine` the imported faces are part of the body and can be used as references
(`body.face#3`) in sketches, holes, fillets and so on. A `combine` can be the `of=` of a pattern.

## Round trip

Exporting a part to STEP and importing it again gives the same volume and area (1e-6 relative) and
center of mass and bounding box (1e-5 mm): determinism tier T2. CI checks this for every corpus part,
with our own importer and with an independent reader (`corpus/tools/check_step_exports.py`).
