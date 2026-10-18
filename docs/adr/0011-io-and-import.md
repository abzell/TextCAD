# 0011. Export, import and verification

Status: accepted (M7)

## Decision
- **STEP is the kernel's job** (`export_step`, `import_step`), written through OpenCASCADE's XDE so
  that names and materials are real STEP entities, as AP242. Meshes are the kernel's job too
  (`mesh`), but their **file formats are pure Rust** in `tcad-io`, so they also build for WebAssembly.
- **Exports run on a copy** of the shape: the STEP writer adjusts tolerances and meshing attaches
  triangulations, and a shape that changed would no longer serialize to the same canonical B-rep
  (found by the determinism test).
- **Faces keep their indices through meshing**, so each mesh primitive can carry the semantic ID of
  its face in glTF `extras`.
- **No timestamps in our own files** (STL, 3MF, GLB); the STEP header's time stamp is the one
  exception, set by OpenCASCADE.
- **Imports are dead solids.** Their faces are named `<import>.face#n` ordered by position (the
  generic ordinal rule of ADR 0010), and the file's bytes are part of the source hash. An import only
  becomes part of the body through the explicit `combine` feature, so it can be a first body, an
  added body or a tool.
- **`verify`** regenerates without touching the cache and reports the tier: T1 if the canonical B-rep
  is byte-identical (same kernel), T2 if topology per feature matches and properties are within
  tolerance (volume and area 1e-6 relative, center of mass and box 10 times the modeling tolerance),
  T3 otherwise with a per-feature report. A cache built from different source is "stale", not a
  failure of determinism.
- **`diff`** compares the design (parameters, canonical statements, references and what they resolve to)
  and the geometry (properties and per-feature volumes), and exits 1 when anything differs.

## Consequences
STEP files cannot be compared byte for byte; compare geometry. CI needs the OCAF/XDE libraries of
OpenCASCADE (`libocct-ocaf-dev`, in vcpkg the default package).
