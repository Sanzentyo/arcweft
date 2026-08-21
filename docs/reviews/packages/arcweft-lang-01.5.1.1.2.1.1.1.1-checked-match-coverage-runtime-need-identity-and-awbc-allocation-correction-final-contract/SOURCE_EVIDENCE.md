# Current source and maintained-contract evidence

All rows were inspected at full Git SHA `c49099fb154d9e3dbb587e1bcd7ee243214da0c4`. Ranges are numeric and
claim-specific; no `1-end` labels or search-only claims are used.

| ID | Exact current range | Result-changing evidence |
|---|---|---|
| `S001` | `crates/arcweft-core/src/awbc/schema.rs:430-795` | current AWBC IDs, runtime types, functions, flags, instruction and opcode owners |
| `S002` | `crates/arcweft-core/src/awbc/schema.rs:796-1035` | current AwbcFunctionKind, naked AwbcFunctionFlags, duplicate opcode numeric mapping |
| `S003` | `crates/arcweft-core/src/awbc/schema.rs:1036-1375` | instruction opcode mapping and terminator family |
| `S004` | `crates/arcweft-core/src/awbc/schema.rs:1376-1715` | match/pattern/task plan tables including string need_id |
| `S005` | `crates/arcweft-core/src/awbc/codec.rs:1-248` | payload-first encoder copy and direct borrowed decoder |
| `S006` | `crates/arcweft-core/src/awbc/codec/wire.rs:1-360` | canonical u32 varint, collection lengths, usize wire exposure |
| `S007` | `crates/arcweft-core/src/awbc/codec/types.rs:1-390` | runtime type and ID wire rows |
| `S008` | `crates/arcweft-core/src/awbc/codec/types.rs:440-690` | tensor fixed-LE shape writer versus varint reader and raw flag decode |
| `S009` | `crates/arcweft-core/src/awbc/codec/code.rs:1-355` | function-kind numeric wire table and instruction writer |
| `S010` | `crates/arcweft-core/src/awbc/codec/code.rs:356-760` | instruction decoder and opcode-class rejection |
| `S011` | `crates/arcweft-core/src/awbc/vm.rs:1-245` | functional VM entry points step/step_with_host and observations |
| `S012` | `crates/arcweft-core/src/task.rs:1-310` | String TaskId, TaskKey and NeedId owners |
| `S013` | `crates/arcweft-runtime-driver/src/task.rs:1-420` | task start, keying and lifecycle publication |
| `S014` | `crates/arcweft-core/src/awbc/product_step/suspension.rs:1-520` | Await/AwaitMany child string suffix identity and snapshot state |
| `S015` | `crates/arcweft-core/src/awbc/product_step/lifecycle.rs:1-420` | task lifecycle fallback NeedId construction |
| `S016` | `crates/arcweft-core/src/awbc/product_step/mapping.rs:1-440` | task-plan need_id and TaskKey mapping |
| `S017` | `crates/arcweft-core/src/awbc/fiber.rs:1-500` | FiberAwaitManyInFlight String fields and snapshots |
| `S018` | `crates/arcweft-runtime-plan/src/awbc_lower/inventory.rs:1-500` | NamedTaskSpec string identity interning |
| `S019` | `crates/arcweft-runtime-plan/src/semantic_facts.rs:1-520` | constructible RuntimePlanSemanticFactInput boundary and normalized type projection |
| `S020` | `crates/arcweft-lang-sema/src/types.rs:430-585` | complete current TypeKind enum |
| `S021` | `crates/arcweft-lang-sema/src/final_analysis/analyzer.rs:1-360` | analyze_final_project and FinalSemanticCatalogs construction boundary |
| `S022` | `crates/arcweft-lang-sema/src/final_analysis/analyzer/patterns.rs:1-360` | current checked pattern construction and local seeding |
| `S023` | `crates/arcweft-lang-sema/src/final_analysis/analyzer/patterns.rs:360-820` | record, sequence, Or, Result/Option and closed variant resolution |
| `S024` | `crates/arcweft-lang-sema/src/final_analysis/model.rs:1-380` | checked project nominal and stable registered semantic value identities |
| `S025` | `crates/arcweft-lang-sema/src/final_analysis/model.rs:380-760` | checked variant owner and binding semantic rows |
| `S026` | `crates/arcweft-lang-sema/src/registration/model.rs:560-980` | RegisteredSemanticWorld and RegisteredTypeCheckEnv owners |
| `S027` | `crates/arcweft-lang-sema/src/registration/model.rs:980-1320` | AcceptedNominalWorld exact catalog access |
| `S028` | `crates/arcweft-lang-sema/src/env/nominal.rs:1-520` | AcceptedNominalRecord, opaque producer and open nominal owners |
| `S029` | `crates/arcweft-core/src/value/opaque.rs:1-360` | RuntimeOpaqueValueClass and RuntimeOpaquePersistence |
| `S030` | `crates/arcweft-core/src/value/ownership.rs:1-360` | runtime value ownership authority |
| `S031` | `crates/arcweft-resource-model/src/registry.rs:1-680` | ResourceTypeRegistry, digest and integrity authority |
| `D001` | `docs/02-runtime/executable-runtime-core.md:140-285` | maintained AWBC opcode and wire contract including NeedTimeout |
| `D002` | `docs/02-runtime/need-timeout.md:1-285` | maintained timeout lifecycle and identity requirements |
| `D003` | `docs/02-runtime/control-flow-runtime.md:1-300` | maintained pattern and Match runtime behavior |
| `D004` | `docs/reviews/packages/arcweft-aw-ah-009.4.4.1-line-plan-runtime-handle-result-authority-reconciliation-final-contract/AWBC_SCHEMA_CODEC_VM.md:1-355` | accepted line-operation fields and stale allocation reconciled here |
| `D005` | `docs/reviews/packages/arcweft-lang-01.3.1.2.2.1-curried-stream-wire-allocation-reconciliation-final-contract/RUST_SHAPED_OWNERS.md:120-230` | accepted Stream instruction field shapes retained while bytes move |
| `D006` | `docs/reviews/packages/arcweft-lang-01.3.1.2.2.1-curried-stream-wire-allocation-reconciliation-final-contract/NORMATIVE_DELTA.md:1-110` | accepted Stream allocation superseded by this global table |

The request itself is copied byte-for-byte at `inputs/CURRENT_REQUEST.md` and is
validated against SHA-256 `8bf22dbee57a94ee178e25d0004be7a18694a8b801ef79189da3f9e1a3741299` and Git blob
`a1411adcf7f2c9651f250d9db3302d3ab61ddfa7`. The immediate predecessor request object and
its retained package MANIFEST SHA-256/length are recorded in
`machine/request-chain.json`.
