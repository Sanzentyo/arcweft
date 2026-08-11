# Structure and dependency plan

## 1. Required measurement set

At each reviewable public AST/HIR cut, measure the current checkout for:

- every changed Rust file;
- the largest production Rust files in each affected crate;
- every changed `lib.rs`/`main.rs`;
- integration tests receiving retained-declaration coverage;
- modules that contain orchestration plus parsing/lowering/registry/bundle/presentation responsibilities;
- any repeated boundary type, identifier, payload, or conversion across crates.

Record exact bytes, physical LOC, embedded `#[cfg(test)]` LOC, role, owning crate, and dependency fan-in/fan-out. Do not use diff additions as file size.

## 2. Expected responsibility modules

| Crate/area | Target responsibility |
|---|---|
| `arcweft-id` | retained family behavior, declaration names, PublicId validation/derivation, AssetId/virtual-path identity |
| `arcweft-lang-syntax::parser` | one lossless grammar and recovery only |
| `arcweft-lang-syntax::attachment` | exact snapshot/node identity and typed navigation |
| public syntax AST modules | thin attached declaration wrappers/accessors, not parsing or sema |
| `arcweft-lang-hir::identity` | typed session IDs, kind/limit behavior, stale errors |
| HIR retained declaration module | immutable payload records and arena resolution |
| HIR lowering context | source-bound allocation/transaction, not project resolution |
| sema/project index | resolution, collisions, accessibility, family/schema checks, one symbol table |
| compiler domain modules | Character/View/Action/Activity/Signal/Metric/Layer product projections |
| project/bundle asset catalog | path/catalog/digest/media/inclusion ownership; filesystem I/O stays adapter-side |
| presentation | LayerTree materialization and runtime policy behavior |
| LSP/CLI/Agent | borrow syntax/HIR/project products; no duplicate parse/index |

## 3. Decomposition triggers

Apply current AGENTS thresholds:

- production Rust file warning above 1,200 physical LOC, error above 2,500;
- `lib.rs`/`main.rs` warning above 1,000, with post-split facade target at most 250;
- integration-test warning above 2,500, error above 8,000;
- ordinary responsibility module preferred 300–800 LOC;
- public contract/dependency change always triggers audit regardless of size.

A split follows responsibility, not arbitrary line counts. Cohesive generated tables/algorithms require explicit rationale.

## 4. Specific hotspot checks

1. `ast/items.rs` must shrink when generic retained declarations are removed; new family wrappers should live in responsibility modules rather than recreating a large enum/string owner.
2. `parser/document.rs` remains orchestration and dispatch; family grammar stays in seven modules.
3. `parser/declaration.rs` remains shared header utilities only. Asset catalog logic cannot enter syntax.
4. HIR `model.rs`/`lower.rs` must not accumulate seven large inline branches; use a retained declaration payload module and named lowering context while preserving a single transaction.
5. `project_index/entities.rs` must not keep syntax values or family-specific string conversion. Split registration by typed facet only if the one symbol authority remains explicit.
6. `cli/app/bundle.rs` must lose pure asset-ID normalization after that behavior moves to its owner; file enumeration and command orchestration may remain.
7. Layer closed enum default/reference behavior belongs on the original enum/newtype, not another `match` helper in parser/sema/presentation.
8. View and Action callable facets must not introduce a second catalog/table or cyclic crate dependency.

## 5. Dependency proof

Use structured Cargo metadata to assert:

```text
arcweft-id / arcweft-source
        -> arcweft-lang-syntax
        -> arcweft-lang-hir
        -> arcweft-lang-sema / project
        -> compiler / runtime-plan / verify
        -> CLI / LSP / Agent / players
```

Data-format/bundle crates remain Sans I/O; adapters perform path reads. `arcweft-core` gains no syntax/HIR/project dependency. Presentation consumes admitted typed products and does not depend on parser source.

## 6. Checked-in audit deliverables

The implementation audit includes:

- baseline and final Git/Jujutsu identities;
- exact measured table;
- dependency graph deltas;
- responsibilities before/after;
- warnings/errors and their resolution or explicit rationale;
- confirmation that no second reader, HIR, symbol table, asset registry, or callable owner survived;
- confirmation that no source gate was added.
