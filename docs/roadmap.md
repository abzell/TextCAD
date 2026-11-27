# Roadmap

The format grows in phases (requirements section 13). Each phase extends the specification, then the
conformance corpus. Phases may overlap; a phase is complete when its gate holds. Open work is tracked as
GitHub issues in this repository, one epic per phase. Items that need the maintainer are in #90.

| Phase | Scope | Status | Epic |
| --- | --- | --- | --- |
| 1 — Parts | Parameters, sketches, features, naming, metadata, STEP import | specified; open items below | #4, #68 |
| 2 — Assemblies and BOM | `spec/assembly.md` | draft; connectors and `publish` (#28), assembly files and placement (#29) normative | #5 |
| 3 — PMI, GD&T, MBD | `spec/pmi.md` (to be written) | planned | #6 |
| 4 — 2D drawings | `spec/drawing.md` (to be written) | planned | #69 |
| 5 — Surfacing, sheet metal, weldments | new chapters | planned | #7 |
| Governance and evaluations | conformance levels, governance, direct editing | planned | #70 |

## Phase 1 (parts)

| Issue | Construct |
| --- | --- |
| #8 | Guided loft rules |
| #9 | Part metadata (specified; conformance cases left) |
| #13 | Format 0.2 and migration rules (specified; conformance cases left) |
| #10 | Material properties and library |
| #11, #12 | Multi-body parts; move/copy body |
| #14, #15 | 3D curves; helix and modeled threads |
| #16 | Extrude up to any face, offset, up to next |
| #17 | Hole point angle, clearance fits, hole tables |
| #18 | External cosmetic threads |
| #19, #20 | Grooves; keyways |
| #21 | Point and curve patterns |
| #22 | Variable-fillet direction, face fillet, full round |
| #23 | Vertex references |
| #24 | Ribs along arcs |
| #72, #73 | Sketch `project`; spline constraints |
| #76 | Consistency sweep |

## Phase 2 (assemblies)

#27 spec review → #28 connectors → #29 files, references, placement → #30 lock file → #33 classic mates
→ #34 joints → #35 drives and poses → #36 couplings → #37 surface mates → #38 sub-assemblies → #39
configurations → #40 patterns and mirroring → #41 skeleton → #42 BOM → #43 mass properties → #45 exploded
views → #46 standard-part library → #47 fastener stacks → #48 assembly features → #49 routes → #50 welds
→ #51 checks → #54 assembly conformance cases → **#55 excavator reference model (gate)**.

## Phase 3 (PMI)

#56 chapter and core model → #57 size tolerances and fits → #58 datums → #59 feature control frames →
#60 finish and notes → **#61 PMI conformance corpus (gate)**.

## Phase 4 (drawings)

#77 drawing files and sheets → #78 views → #79 derived views → #80 dimensions and annotations → #81 BOM
tables and balloons → **#84 conformance drawings (gate)**.

## Phase 5 (surfacing, sheet metal, weldments)

#62 sheet-metal model → #63 sheet-metal features → **#64 flat patterns (gate)** · #65 weldments · **#66
surfacing (gate)** → #67 G2 blends.

## Governance

#85 conformance levels, runner, governance, spec releases · #88 direct-editing evaluation.
