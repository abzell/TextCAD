# 0003. Workspace layout and M1 stubs

- Status: accepted
- Date: 2026-10-18

## Context

Section 3 lists the crates. Section 14 puts the OCCT backend and the CLI/LSP/MCP tooling in M5 and M8.

## Options

1. Create only the crates needed by the current milestone.
2. Create every crate from section 3 as an empty stub in M1.

## Decision

Option 2, under `crates/`. CI, the WASM job and `cargo-deny` then cover the full layout from day one.
Stubs carry only the name constants (`FORMAT_NAME`, `FILE_EXTENSION`, `BUNDLE_EXTENSION`), one set per
crate as section 0 requires. `tcad-occt` has no `cxx` dependency or C++ code until M5. `viewer/` is
created in M8. `unsafe_code` is forbidden workspace-wide; `tcad-occt` will relax this for its FFI
module in M5 via a recorded ADR.

## Consequences

Inter-crate dependencies are added as each milestone needs them. The WASM job builds
`tcad-syntax`, `-model`, `-expr`, `-solver`, `-naming`, `-kernel` and `-cache` for `wasm32-unknown-unknown`.
