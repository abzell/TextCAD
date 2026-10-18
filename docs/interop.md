# Interoperability

What is checked automatically and what is checked by hand.

## Automatic (CI)

| Check | Where |
| --- | --- |
| Every corpus part exports to STEP and imports again within tier T2 | `crates/tcad-build/tests/corpus.rs` |
| Every corpus part's STEP export is read by an independent reader (build123d `import_step`) and matches `expected.json` | `corpus/tools/check_step_exports.py`, job `occt-linux` |
| STL, 3MF and GLB structure (header, counts, extras) | `crates/tcad-io/tests/writers.rs`, `crates/tcad-cli/tests/geometry.rs` |

## Manual spot checks

STEP AP242 files must open in Creo, NX, SolidWorks and FreeCAD. These commercial tools cannot run in
CI, so the check is documented here each time the exporter or OpenCASCADE changes.

Procedure: `tcad export <part> --to step`, open the file, check that the solid is a single body with
the right size (measure one dimension and the volume), that the name and material show up, and that
the units are millimetres.

| Date | Tool and version | Parts | Result |
| --- | --- | --- | --- |
| _none yet_ | | | The hydraulic parts that these checks target arrive with milestone M8. |
