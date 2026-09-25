# Native dialogue Init owner review — 2026-09-25

Inspected base: `37b9ea86ff821cc44c37c9f8b81b79bfedf2892e` on `main`. The working tree was dirty with this native cut partly staged and independent AWBC/language work unstaged. This review covers only `crates/arcweft-core/src/engine/dialogue.rs`.

| Owner | Classification | Base physical LOC | Cut physical LOC | Growth | Cut bytes | Embedded test LOC |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `arcweft-core::engine::dialogue` | production Rust module with embedded unit tests | 1,897 | 2,802 | 905 | 121,017 | 529 |

The module owns native dialogue activation as one revisioned transaction: pre-reveal operations and lexical cleanup, line-task progression, host-command outcomes, and result publication all advance the same `DialogueActivationFrame` and `RuntimeDialogueActivationState`. The Init cut adds scoped defer, evaluated-effect, and host-call continuations inside that state machine. The registry/revision owner remains in `engine/dialogue/store.rs`; shared lease custody and defer identity remain in `line_task/handle.rs`. Neither is duplicated here. The module emits Sans-I/O requests and has no host adapter or external I/O dependency. Its direct in-crate dependencies are the plan, line-task, value, presentation, pure-call, step, effect, and runtime-ID owners; its callers are the native engine flow/suspension/child orchestration. No crate dependency edge or public API was widened to split a file.

**Disposition:** cohesive state-machine owner despite the size trigger. The 529 embedded test lines exercise the same transaction, scope, effect, and host-result boundary. A physical split now would move methods that jointly stage one activation candidate without creating an independent state/API owner. The module-level ownership comment records that boundary in source. Revisit decomposition if a new independent execution state or dependency direction appears; LOC alone is not a blocking finding.

Validation: `cargo check -q -p arcweft-core` passed; `cargo test -q -p arcweft-core --lib init_` passed 5/5; `cargo test -q -p arcweft-core --lib defer` passed 9/9; `cargo test -q -p arcweft-core --lib dialogue_drop_source_take_is_committed_with_registry_revision` passed 1/1. `just structure-audit-gate` passed on the dirty checkout: 2,619 files scanned, 96 workspace packages, 328 review triggers, and **0 blocking violations**.
