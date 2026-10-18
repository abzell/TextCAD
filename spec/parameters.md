# Parameters and configurations

Status: **normative** (M3). Source requirements: 4.5. Implemented by `tcad-model`.

## Parameters

```
param pin_dia = 20 mm range(10 mm, 60 mm) doc "Pin diameter"
```

- A parameter has a name, a defining expression (see [expressions.md](expressions.md)) and optionally
  `range(lo, hi)` and `doc "..."` in that order. Values may be quantities, numbers, strings and
  booleans.
- `range` needs a numeric parameter and bounds of the same dimension; `lo` must not exceed `hi`
  (`E2304`). A value outside it is `E2301`, in the base part and in every configuration.
- Parameters are evaluated in declaration order, so a parameter can use only earlier ones.

## Configurations

```
config heavy { pin_dia = 25 mm; jaw_thk = 20 mm; suppress rod_thread }
config default standard { }
```

- A configuration is a variant of the part: parameter overrides and a list of suppressed features.
  It never duplicates features.
- An override names an existing parameter (`E2401`) and must have the same type and dimension as the
  parameter's own expression (`E2402`). It may use parameters declared before the overridden one
  (`E2103` otherwise). Parameters that depend on an overridden one are recomputed.
- `suppress` lists features, sketches or reference geometry that exist in the part (`E2403`).
- At most one configuration is `default` (`E2404`). If configurations exist but none is `default` the
  base part is the default and `W2405` is reported. Configuration names are unique (`E2101`).
- A feature statement can end with `suppress if <boolean expression>`; the expression is evaluated
  under each configuration and must be boolean (`E2601`).
- Every configuration is fully evaluated by `tcad check`, so an invalid variant is found without
  building it. A problem that exists in the base part is reported once, not once per configuration.

## Resolution

`Model::resolve(config)` returns the parameter values (in canonical units) and the set of suppressed
names for the base part (`None`) or a named configuration. A build with no `--config` uses the
`default` configuration if there is one, else the base part.

## Single-use statements

`tolerance` and `material` may each appear once per part (`E2303`). A file holds exactly one `part`
(`E2701`).
