# Reserved keywords

Status: **normative** (M2). Source requirements: 4.8.

These words are reserved for later phases so that Phase 1 files stay valid:

`assembly instance mate skeleton bom pmi datum_feature gdt drawing view section surface boundary fill
sheet_metal flange bend flat_pattern weldment direct_edit`

- A statement that starts with one of them, at the top level or inside a `part` or `sketch`, is
  parsed up to the end of its statement (or its balanced `{ }` block) and rejected with `E0001`
  ("feature not yet supported"). The rest of the file is still checked.
- They may not be used as declared names (`E1104`).
- The list is extended only by a minor version bump; removing a word needs a major version
  (see [versioning.md](versioning.md)).
