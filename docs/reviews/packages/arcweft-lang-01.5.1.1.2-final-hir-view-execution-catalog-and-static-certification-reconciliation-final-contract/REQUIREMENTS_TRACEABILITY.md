# Requirements traceability

| Requirement | Closed requirement | Contract location | Test prefixes |
|---|---|---|---|
| D1 | sole checked semantic catalog, exact APIs/identity/facts | FINAL_CONTRACT C1; OWNERS_AND_APIS; RUST_SCHEMAS; SEMANTIC_CATALOG | VAL, STR, IDN, API, LIM |
| D2 | complete current View semantic variants and surface scope | FINAL_CONTRACT C2; RUST_SCHEMAS; SEMANTIC_CATALOG | STR, INP, IMG, STA |
| D3 | dynamic-value execution owner and projections | FINAL_CONTRACT C3; DYNAMIC_VALUE_EXECUTION; RUST_SCHEMAS | VAL, INP, API |
| D4 | dynamic-capable product fields, wire, validation, runtime, deletion | FINAL_CONTRACT C4; PRODUCT_WIRE_AND_SAVE; WIRE_ALLOCATIONS.json; DELETION_MATRIX | INP, TAM, LIM |
| D5 | image/resource/animation behavior | FINAL_CONTRACT C5; IMAGE_RESOURCE_ANIMATION | IMG, PAR, SAV, HOT |
| D6 | static evidence/identity/granularity/digest/invalidation/#[static] | FINAL_CONTRACT C6; STATIC_CERTIFICATION | STA, IDN, TAM, HOT, LIM |
| D7 | dynamic/static runtime parity and mandatory lifecycle | FINAL_CONTRACT C7; RUNTIME_PARITY; TIER2_MATRIX | PAR, SAV, IMG, T2 |
| D8 | parameters/defaults/exports/nested-call identity | FINAL_CONTRACT C8; PARAM_DEFAULT_EXPORT_IDENTITY; RUST_SCHEMAS | VAL, STR, IDN, SAV, HOT |
| D9 | source diagnostics and failure precedence | FINAL_CONTRACT C9; DIAGNOSTICS | NEG, VAL, IDN, IMG, STA, TAM |
| D10 | AWFB/bundle/runtime/backend/save/reload/generated migration | FINAL_CONTRACT C10; PRODUCT_WIRE_AND_SAVE; CONSUMER_MIGRATION_MATRIX; TIER2_MATRIX | PAR, SAV, HOT, T2 |
| D11 | deletion-driven compile-clean interleave | FINAL_CONTRACT C11; IMPLEMENTATION_PLAN; DELETION_MATRIX | API, T2 |
| D12 | exact bounded work accounting | FINAL_CONTRACT C12; WORK_ACCOUNTING | LIM |
| CI-syntax | syntax View/attribute/attached-body ownership | PRODUCER_CONSUMER_MATRIX; IMPLEMENTATION_PLAN C7 | STR, STA, API |
| CI-HIR | HIR item/expression/member/scope/source/generation/project view | REPOSITORY_EVIDENCE; SEMANTIC_CATALOG | VAL, STR, NEG |
| CI-sema | final analysis/call/type/effect/resource/catalog publication | OWNERS_AND_APIS; SEMANTIC_CATALOG | VAL, STR, STA, LIM |
| CI-compiler | View/image/style/Fx/dialogue/source maps/atomic CompiledProject | PRODUCT_WIRE_AND_SAVE; IMPLEMENTATION_PLAN C3-C4 | VAL, INP, IMG, TAM |
| CI-view | program/value/mount/parts/resources/identities | RUST_SCHEMAS; RUNTIME_PARITY | STR, INP, PAR, API |
| CI-bundle | all View codecs/digest/validation/merge/product compatibility | PRODUCT_WIRE_AND_SAVE; WIRE_ALLOCATIONS.json | TAM, LIM, T2 |
| CI-runtime | catalog/evaluator/replacement/save/plan/host/backends/Agent/MCP/generated | CONSUMER_MIGRATION_MATRIX; RUNTIME_PARITY; TIER2_MATRIX | PAR, SAV, HOT, T2 |
| CI-tests | compiler/runtime tests, Tier 2, Cargo metadata, structure audit | TEST_MATRIX; TIER2_MATRIX; GATE_COMMANDS | all |

Every numbered request decision D1-D12 has one selected answer. Every required consumer family has an owner, migration, test prefix, deletion or preservation rule, and final gate.
