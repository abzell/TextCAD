# 0005. A shared `tcad-diag` crate

- Status: accepted
- Date: 2026-10-18

## Context

Every crate from the parser to the kernel produces diagnostics with the same shape (stable code,
span, message, fix hint, text and JSON output; requirements 4.7). Section 3 does not list a crate for
that.

## Options

1. Define diagnostics in `tcad-syntax` and make every crate depend on it.
2. Add a small `tcad-diag` crate that all others depend on.

## Decision

Option 2. `tcad-solver` and `tcad-kernel` have no reason to depend on the parser. `tcad-diag` is pure
Rust, WASM-capable, and depends only on `serde` and `serde_json` (MIT/Apache-2.0).

## Consequences

The code registry lives in one place (`crates/tcad-diag/src/codes.rs`). The WASM CI job builds it.
