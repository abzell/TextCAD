# Name

The format is called **TextCAD**. Source files use the extension `.tcad`; archived bundles use `.tcadz`.
The name is a working name and may change.

To keep a rename cheap:

- Each Rust crate defines `FORMAT_NAME`, `FILE_EXTENSION` and `BUNDLE_EXTENSION` once (in `src/lib.rs`).
- The file header keyword (`tcad`) and the crate/binary prefix are the only other places the short name appears.
- This file is the single place the spec states the name.
