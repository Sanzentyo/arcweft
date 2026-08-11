# Bounded implementation change surface

This package does not modify these files. It prescribes the future production
change surface below.

| Repository area | Required intent |
|---|---|
| `crates/arcweft-lang-sema/src/nominal/model.rs` | add `BuiltinTypeConstructor::Ref`; add inherent inventories/selectors/expectations/projection; add `TypeArgumentKind`; update wrong-kind payloads and ordering |
| `crates/arcweft-lang-sema/src/types.rs` | add bidirectional fixed authored `EntityKind` inventory; retain `TypeKind::entity_ref` as projection constructor |
| `crates/arcweft-lang-sema/src/nominal/resolver/engine.rs` | add `NodeValue::argument_kind`; stop importing free builtin selector |
| `.../resolver/engine/support.rs` | delete free `builtin(path)` table; retain non-owner support functions |
| `.../resolver/engine/traversal.rs` | replace boolean special case with typed per-index expectation; include `Ref` through owner API |
| `.../resolver/engine/resolution.rs` | select via inherent owner; dispatch all three projections via inherent projection; authoritative const wrong-kind |
| `crates/arcweft-lang-sema/src/nominal/diagnostic.rs` | consume `TypeArgumentKind`, preserve code and poison model |
| `crates/arcweft-lang-hir/src/symbol/table.rs` and publication tests | reserve direct `Ref` with existing error |
| `crates/arcweft-lang-sema/src/env/nominal.rs` and catalog tests | reserve direct `Ref` for exact/open registrations |
| normal checker/callable/entry consumers | remove remaining context-free `Ref` conversion and consume checked result only |
| `crates/arcweft-lang-sema/src/project_index/*` | tests confirming valid/invalid contextual nodes do not become project rename edges; no duplicate index |
| `crates/arcweft-lsp/src/features/nominal_types.rs` and accepted-profile tests | typed hover/completion/definition/rename policy using retained typecheck node facts |
| compiler persistent/bytecode tests | digest tests and assertion of no schema change |
| entry schema tests | checked callable acceptance and persisted-data typed rejection |
| fixtures/Tier 2 tests | `Ref<Flow>` pass fixtures and required matrix |

Production implementation should not add a dependency solely to share a
reserved-name literal across HIR and sema. The existing layer-local gates remain
and are locked together by behavior tests.
