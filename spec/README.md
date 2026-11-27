# TextCAD specification

Normative. This directory defines the TextCAD format: how a part (and, in Phase 2, an assembly) is
written as text, what every statement means, and what geometry it describes. It says nothing about any
particular program; any tool that reads, checks, regenerates or exports TextCAD files follows it.

If a tool and this specification disagree, the specification is right. Changes to the format are made
here first, recorded as ADRs in [`../docs/adr/`](../docs/adr/), and covered by conformance cases in
[`../corpus/`](../corpus/).

| Chapter | Topic | Status |
| --- | --- | --- |
| [name.md](name.md) | Name and extensions | normative |
| [grammar.ebnf](grammar.ebnf) | Grammar | normative (format 0.2) |
| [lexical.md](lexical.md) | Lexical structure | normative |
| [canonical-form.md](canonical-form.md) | Canonical text form | normative |
| [versioning.md](versioning.md) | Versions, evolution rules, migration | normative |
| [reserved.md](reserved.md) | Reserved keywords | normative |
| [units.md](units.md) | Values and units | normative |
| [expressions.md](expressions.md) | Expressions and scopes | normative |
| [parameters.md](parameters.md) | Parameters and configurations | normative |
| [sketch.md](sketch.md) | Sketches and constraints | normative |
| [features.md](features.md) | Features and part metadata | normative |
| [naming.md](naming.md) | Persistent naming | normative |
| [kernel.md](kernel.md) | Geometric semantics, conformance tiers | normative |
| [bundle.md](bundle.md) | Bundle layout, archive, source hash | normative |
| [io.md](io.md) | STEP import | normative |
| [diagnostics.md](diagnostics.md) | Validity and standard diagnostic codes | normative |
| [assembly.md](assembly.md) | Assemblies, connectors, mates, BOM | draft (Phase 2) |

Examples live in [`examples/`](examples/); they are valid and in canonical form.

## Conformance

A tool conforms to a chapter when it accepts every valid file of the conformance corpus, rejects the
invalid ones with the listed diagnostics, and (for tools that regenerate geometry) matches the expected
results at determinism tier T2 ([kernel.md](kernel.md)).
