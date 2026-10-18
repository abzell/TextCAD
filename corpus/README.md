# Golden corpus

Each part lives in `corpus/<name>/` as `parts/<name>.tcad` plus `expected.json`. The test suite runs
every entry (`cargo test -p tcad-model --test corpus`) and `tcad fmt --check corpus` keeps the
sources canonical. From milestone M5 on, parts with geometry add expected properties and a
`reference.py` build123d cross-check that runs in CI only (requirements section 11).

## `expected.json` for sketches (milestone M4)

| Key | Meaning |
| --- | --- |
| `note` | Why the part exists. |
| `codes` | Exact list of diagnostic codes other than information, in source order. |
| `info` | Exact list of information codes, if any are expected. |
| `dof` | Total degrees of freedom of the sketch after solving. |
| `entities` | `{ name: { "dof": n } }`: degrees of freedom per entity. |
| `points` | `{ "entity.role": [x, y] }`: solved coordinates in mm, checked to 1e-6. |
| `conflict` | Substrings that the conflict message (`E4101`) must contain. |
| `not_named` | Substrings it must *not* contain (innocent constraints). |
| `entity_count` | Number of solved entities. |

## Sketch stress parts (6 of the planned ~40)

`sketch_fully_constrained`, `sketch_under_constrained`, `sketch_over_constrained` (must fail with
a named conflict), `sketch_tangent_chain`, `sketch_splines`, `sketch_200_entities`.
