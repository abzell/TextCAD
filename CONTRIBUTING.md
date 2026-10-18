# Contributing

- Licensed under Apache-2.0. Contributions are accepted under the same license.
- Sign off every commit (`git commit -s`) to certify the
  [Developer Certificate of Origin](https://developercertificate.org/). CI checks this on pull requests.
- The spec in `spec/` is normative. Change spec and implementation together; if they disagree, fix one and log an ADR.
- Record every non-trivial decision as an ADR in `docs/adr/` (copy `0000-template.md`).
  Decisions locked in section 2 of `docs/TextCAD-Requirements.md` cannot be overturned by an ADR.
- Before pushing: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `cargo deny check`.
- Dependencies must satisfy `deny.toml`: no GPL/AGPL.
