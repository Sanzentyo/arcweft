# Dependency graph

## Selected crate edges

```text
arcweft-lang-sema
  -> arcweft-lang-hir
  -> arcweft-resource-model      # only new manifest edge in this correction
  -> arcweft-view                # existing checked View value vocabulary

arcweft-runtime-plan
  -> arcweft-lang-hir
  -> arcweft-core

arcweft-compiler
  -> arcweft-lang-sema
  -> arcweft-runtime-plan
  -> arcweft-bundle
  -> arcweft-view
  -> arcweft-resource-model

arcweft-bundle
  -> arcweft-core
  -> arcweft-view
  -> arcweft-resource-model

arcweft-runtime-driver
  -> arcweft-bundle
  -> arcweft-core
  -> arcweft-view
  -> arcweft-resource-model
```

`arcweft-compiler` is the one join that projects `CheckedMatchRef` into runtime-plan seed and later joins lowered selector output with View coordinates during bundle construction. Runtime-plan never imports sema/View/bundle types.

## Explicitly absent edges

- no `arcweft-view -> arcweft-core`;
- no `arcweft-runtime-plan -> arcweft-lang-sema`;
- no `arcweft-runtime-plan -> arcweft-view`;
- no `arcweft-runtime-plan -> arcweft-bundle`;
- no `arcweft-core -> arcweft-runtime-driver`;
- no `arcweft-core -> arcweft-view`;
- no `arcweft-resource-model -> arcweft-lang-sema`;
- no runtime-driver dependency from sema; and
- no bundle dependency on compiler/runtime-plan.

## Join placement proof

View cannot name AWBC types/values. Core cannot name View mounts/generations. Runtime-plan already owns AWBC generation and one type table but must stay below sema/View. Compiler already consumes sema and runtime-plan and therefore performs the one-way checked-fact projection. Bundle already depends on core/View and owns persisted static joins. Runtime-driver already depends on bundle/core/View and owns active-generation validation/transactional installation. No copied authority or cycle is required.
