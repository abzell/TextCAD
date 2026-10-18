# Building TextCAD

```
cargo build --workspace            # no C++ needed: mock kernel only
cargo test --workspace
```

## With OpenCASCADE (real geometry)

Install OCCT 7.6 or newer, then build with the `occt` feature (`cargo build -p tcad-cli --features occt`).

- **Linux (Debian/Ubuntu):** `sudo apt install libocct-foundation-dev libocct-modeling-data-dev
  libocct-modeling-algorithms-dev libocct-data-exchange-dev libocct-ocaf-dev g++`
- **Windows:** `vcpkg install opencascade:x64-windows`, set `VCPKG_INSTALLATION_ROOT`.
- **Elsewhere:** set `OCCT_INCLUDE_DIR` and `OCCT_LIB_DIR`.

OCCT is linked dynamically (docs/adr/0008-occt-linking-and-license.md).

## Cross-check against build123d (optional)

`pip install build123d && python3 corpus/tools/compare_reference.py corpus`

To read the STEP exports of the corpus with an independent reader:
`python3 corpus/tools/check_step_exports.py target/debug/tcad corpus`.
