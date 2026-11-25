# Plan: remaining work after Phase 1b

Created 2026-11-25 from a review of what it takes to fully define parts and assemblies. Each item is a
GitHub issue written so that a coding agent can complete it alone: spec text to write, syntax, rules,
crates to change, tests and corpus parts, acceptance criteria and dependencies. The common rules
(spec first, ADRs, T1 determinism, corpus with independent checks, diagnostics registry, LSP/MCP/fmt
parity, CHANGELOG and progress entries, signed-off commits) are stated once in each epic.

The Phase 2 design is drafted in [`spec/assembly.md`](../spec/assembly.md) and
[ADR 0015](adr/0015-assembly-model.md).

## Epics

| Epic | Scope |
| --- | --- |
| [#4](https://github.com/abzell/TextCAD/issues/4) | Part-modeling gaps (Phase 1.x) |
| [#5](https://github.com/abzell/TextCAD/issues/5) | Phase 2: assemblies and BOM |
| [#6](https://github.com/abzell/TextCAD/issues/6) | Phase 3: PMI, tolerances, GD&T |
| [#7](https://github.com/abzell/TextCAD/issues/7) | Phase 5: sheet metal, weldments, surfacing |

Phase 4 (2D drawings) is not broken down yet.

## Part-modeling gaps (#4)

| Issue | Item | Needed by |
| --- | --- | --- |
| #8 | Document guided lofts; remove the stale "Not yet" note | – |
| #9 | Part `meta` (number, revision, kind, mass override, colour) | Phase 2 (BOM, STEP) |
| #13 | Format 0.2: evolution rules, keyword gating, `tcad migrate` | every new keyword |
| #10 | Materials beyond density; material library | BOM, PMI |
| #11 | Multi-body parts (named bodies, booleans, split) | #12, weldments, surfaces |
| #12 | Move / copy body | – |
| #14 | 3D curves (polyline with bends, spline) for sweeps | Phase 2 routes (#49) |
| #15 | Helix, springs, modeled threads | – |
| #16 | Extrude up to / offset from any face, up to next | surfaces |
| #17 | Hole point angle, clearance fits, hole tables | fastener stacks |
| #18 | External cosmetic threads (`thread`) | library parts (#46) |
| #19 | Grooves: O-ring, snap ring, undercut | hydraulic parts |
| #20 | Keyways (DIN 6885, ASME B17.1) | – |
| #21 | Point and curve patterns; several axes | component patterns (#40) |
| #22 | Kernel-independent variable fillet direction; face fillet, full round | – |
| #23 | Vertex references and queries | connectors, datum targets |
| #24 | Ribs along arcs; normal-mode webs | – |
| #25 | Linux T1 parity (OpenCASCADE with musl-math) | – |
| #26 | Needs a person: tagged release, commercial CAD spot check | Phase 1 acceptance |

## Phase 2 (#5)

Order of work (each issue lists its own dependencies):

1. **Foundations**: #27 spec review → #13 format 0.2 → #9 meta → #28 connectors and `publish`
2. **Files and placement**: #29 assembly parser/model → #30 `bundle.lock` → #31 build, cache, GLB
3. **Solving**: #32 solver core (`tcad-asm`) → #33 classic mates → #34 joints and limits → #35 drives
   and poses → #36 gear/rack/screw couplings → #37 tangent, path, cam
4. **Structure**: #38 sub-assemblies → #39 configurations and suppression → #40 component patterns and
   mirroring → #41 skeleton and `uses`
5. **Outputs**: #42 BOM → #43 mass properties → #44 STEP/GLB/3MF assembly export → #45 exploded views
6. **Productivity**: #46 standard-part library → #47 fastener stacks → #48 assembly features → #49
   routes → #50 welds → #51 interference and clearance checks
7. **Tooling and proof**: #52 LSP and MCP → #53 viewer → #54 assembly corpus → **#55 excavator gate**

## Phase 3 (#6)

#56 spec and core PMI model → #57 size tolerances and fits → #58 datum features and targets → #59
feature control frames → #60 finish, notes, thread and weld callouts → #61 semantic AP242 export,
validation (gate) and display.

## Phase 5 (#7)

#62 sheet-metal model and bends → #63 flanges, reliefs, hems, forming tools → #64 flat pattern and
DXF (gate) · #65 weldment members and cut lists · #66 surfacing (gate) → #67 G2 blends.
