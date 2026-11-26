# Reserved keywords

Status: **normative** (format 0.1). Source requirements: 4.8.

These words are reserved for later phases so that Phase 1 files stay valid:

`assembly instance mate skeleton bom pmi datum_feature gdt drawing view section surface boundary fill
sheet_metal flange bend flat_pattern weldment direct_edit`

- A statement that starts with one of them, at the top level or inside a `part` or `sketch`, is
  parsed up to the end of its statement (or its balanced `{ }` block) and rejected with `E0001`
  ("feature not yet supported"). The rest of the file is still checked.
- They may not be used as declared names (`E1104`).
- The list is extended only by a minor version bump; removing a word needs a major version
  (see [versioning.md](versioning.md)).
- A word leaves this list when the statement it starts is implemented, in a minor version; files with
  an older header keep reading it under the rules of their version.
- Keywords added in later minor versions that are not reserved words (`meta`, `connector`, ...)
  are gated by the header instead: see [versioning.md](versioning.md) § Keywords added in a minor
  version.
