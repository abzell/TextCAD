# Import

Status: **normative** (format 0.1). Source requirements: 9.

A part is defined by its text; its geometry is always regenerated from that text. Exporting to STEP or
any other exchange format is up to tools and is not part of the format. Exchange files enter a design
only through the `import` statement below.

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
