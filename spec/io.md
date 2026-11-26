# Import and interchange

Status: **normative** for import (format 0.1); the export mapping is informative. Source
requirements: 9.

A part is defined by its text; its geometry is always regenerated from that text. STEP and other
exchange files are **outputs** that tools produce on request, and **inputs** only through the
`import` statement below.

## STEP import

```
import body = step("imports/body.step")
combine fit with=body at=(10 mm, 0 mm, 0 mm) op=subtract
```

`import` reads a STEP file from the bundle (the path is relative to the bundle root and may not leave
it). The solids come in as one **dead** shape without history. Its faces are named `body.face#n`,
numbered by the kernel-independent rule of spec/naming.md (centroid x, y, z, then area). The file's
bytes are part of the source hash (spec/bundle.md).

Nothing is added to the part until a `combine` feature uses the import:

| Argument | Meaning |
| --- | --- |
| `with=` | the import (`E5002` if it is not an earlier, unsuppressed `import`) |
| `at=(x, y, z)` | optional translation applied to a copy of the solid |
| `op=` | `add` (default; the first solid feature may be a `combine`), `subtract`, `intersect` |

After the `combine` the imported faces are part of the body and can be used as references
(`body.face#3`) in sketches, holes, fillets and so on. A `combine` can be the `of=` of a pattern.

## Recommended mapping to STEP AP242 (informative)

Tools that export a part to STEP AP242 are encouraged to record the part's data as follows, so that
exports from different tools agree:

| Part data | STEP |
| --- | --- |
| units | millimetres |
| part name | the product name |
| `meta number` | the product id (the part name if absent) |
| description (the string after the part's name) | the product description |
| `meta revision` | the id of the product definition formation (the product version) |
| material name and density | a material with name and density (g/cm^3) |
| configuration | the file description of the header |
| `meta color` | a colour of the shape (`COLOUR_RGB`), components as written (sRGB) |
| solids | one body per solid |

An exported and re-imported part should match the original at determinism tier T2 (spec/kernel.md).
