# 0002. Toolchain pin, MSRV and edition

- Status: accepted
- Date: 2026-10-18

## Context

Section 12.1 requires Rust stable with an MSRV pinned in `rust-toolchain.toml`. It does not give a version.

## Options

1. Pin `rust-toolchain.toml` to the current stable (1.97.0) and set `rust-version` to the same.
2. Pin the toolchain to current stable but declare a lower MSRV and test it in a separate CI job.

## Decision

Option 1, edition 2024, resolver 3. There are no downstream consumers yet, so a lower MSRV buys nothing
and costs a CI job. Bump deliberately, together with `rust-version` and the CI workflow.

## Consequences

Contributors get the exact CI toolchain via rustup. Revisit when the crates are published or the OCCT
and WASM toolchains constrain the version.
