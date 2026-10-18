# TextCAD specification

Normative. Every language construct is defined here (prose plus grammar) before it is implemented.
If the implementation and this spec disagree, fix one of them and record an ADR in `docs/adr/`.

Chapters are stubs until their milestone (M2 onward). The requirements they derive from are in
[`../docs/TextCAD-Requirements.md`](../docs/TextCAD-Requirements.md).

| Chapter | Topic | Milestone |
| --- | --- | --- |
| [name.md](name.md) | Name and extensions | M1 |
| [grammar.ebnf](grammar.ebnf) | Grammar (draft) | M1 draft, M2 final |
| [lexical.md](lexical.md) | Lexical structure | M2 |
| [canonical-form.md](canonical-form.md) | Canonical formatting | M2 |
| [diagnostics.md](diagnostics.md) | Error codes and output | M2 |
| [versioning.md](versioning.md) | Versioning and migration | M2 |
| [reserved.md](reserved.md) | Reserved keywords | M2 |
| [units.md](units.md) | Values and units | M3 |
| [expressions.md](expressions.md) | Expressions | M3 |
| [parameters.md](parameters.md) | Parameters and configurations | M3 |
| [sketch.md](sketch.md) | Sketches and constraints | M4 |
| [kernel.md](kernel.md) | Kernel contract, determinism | M5 |
| [bundle.md](bundle.md) | Bundle layout and cache | M5 |
| [features.md](features.md) | Features | M5, M6 |
| [naming.md](naming.md) | Persistent naming | M6 |
| [io.md](io.md) | Import and export | M7 |
