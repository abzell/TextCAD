# Progress

## M1: Repo and spec skeleton (2026-10-18)

Done:
- Cargo workspace (12 stub crates), pinned toolchain, workspace lints.
- CI: fmt, clippy (deny warnings), tests on Windows and Linux, WASM build, `cargo-deny`, docs, DCO check.
- Apache-2.0 `LICENSE`, `NOTICE`, `CONTRIBUTING.md`, `deny.toml`.
- ADR template and ADRs 0001-0003.
- Spec chapters stubbed in `spec/`; draft EBNF grammar in `spec/grammar.ebnf`.

Open items for M2 (parser): see "Open questions" in `spec/grammar.ebnf`.
Not verifiable from the agent's side: the CI run on GitHub (verify after push).
