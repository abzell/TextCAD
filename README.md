# TextCAD

**An open, human- and LLM-readable text format for parametric CAD.** A TextCAD file carries the full
design intent of a mechanical part: parameters with units, sketches with constraints, the feature tree,
stable references to faces and edges, configurations and part metadata. Assemblies, PMI, drawings,
surfacing and sheet metal follow in later phases.

```
tcad 0.2

part rod_clevis "Rod clevis, 20 mm pin" {
  material "AISI 1045" density=7.85 g/cm^3
  meta number="TC-HYD-001" revision="A"

  param pin_dia = 20 mm range(10 mm, 60 mm)
  sketch side on plane.XY {
    rect outline corner=(0 mm, -29 mm) w=70 mm h=58 mm
  }
  extrude body from side.outline distance=40 mm symmetric
  hole pin_bore on=body.side(side.outline.top) at=(-50 mm, 0 mm) d=pin_dia end=through_all
  config heavy { pin_dia = 25 mm }
  config default standard { }
}
```

This repository is the **format**: what a valid file is and what geometry it describes. It contains no
program. Any tool, open or proprietary, may read, check, regenerate, edit, view or export TextCAD files
by following it.

| Path | Content |
| --- | --- |
| [`spec/`](spec/README.md) | The normative specification: grammar, statements, units, naming, geometric semantics, diagnostics |
| [`spec/examples/`](spec/examples/) | Example parts |
| [`corpus/`](corpus/README.md) | The conformance corpus: 94 cases with expected results and independent build123d reference models |
| [`docs/TextCAD-Requirements.md`](docs/TextCAD-Requirements.md) | What the format must achieve |
| [`docs/roadmap.md`](docs/roadmap.md) | Phases and open work |
| [`docs/adr/`](docs/adr/) | Decisions about the format |

Status: format 0.2. Parts (Phase 1), including their connectors and published geometry, are
specified; assemblies (Phase 2) are a draft.

Licensed under Apache-2.0 ([`LICENSE`](LICENSE), [`NOTICE`](NOTICE)). Contributions: see
[`CONTRIBUTING.md`](CONTRIBUTING.md).
