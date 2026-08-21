# Corrected owner and API map

Each Arcweft-owned enum receives its behavior in the original owner/module or
a same-module inherent implementation. The map does not use extension traits
to avoid changing an owned enum.

| Owner | Current/final path | Normative API | Borrow/ownership flow | Cut | Dependencies/notes |
|---|---|---|---|---:|---|
| `arcweft_core::entry::RuntimeValueDigest` | `crates/arcweft-core/src/entry/identity.rs + entry/schema.rs` | RuntimeValue::try_digest / try_canonical_bytes share write_canonical(&mut impl CanonicalRuntimeValueSink) | &RuntimeValue + &mut sink; no allocation required for Blake3Sink | 4 | RuntimeValue, CanonicalRuntimeValueSink. sole identity grammar; no producer-only digest |
| `arcweft_core::value::RuntimeValue` | `crates/arcweft-core/src/value.rs` | validate_constant_admission; final NeedHandle enum arm in original enum | recursive immutable visitor; bounded work counter | 5 | RuntimeNeedHandle, RuntimeOpaqueValue. all exhaustive visitors update in one protected commit |
| `arcweft_core::task::RuntimeNeedHandle` | `crates/arcweft-core/src/task.rs` | try_new; validate_structure; validate_use; rebind_for_replacement; semantic_key | owned descriptor; validation borrows scheduler generation/spec | 5 | TaskCorrelation, NeedProducerSpec, TaskOutcomeContract. manual Eq/Hash/Ord by NeedId only |
| `arcweft_core::task::TaskSpec` | `crates/arcweft-core/src/task.rs` | try_new; validate; producer_instance_key; structurally_eq_for_join | owned spec enters scheduler transaction; no IDs supplied by caller | 5 | NeedProducerSpec, TaskExecution, TaskPolicy, TaskOutcomeContract. one execution field |
| `arcweft_runtime_scheduler::RuntimeTaskScheduler<A>` | `crates/arcweft-runtime-scheduler/src/lib.rs` | ensure_task; ingest_host_events; step_runtime_tasks; register_observer; cancel_scope; snapshot; restore; replay; prepare_replacement; commit_replacement | all mutating methods require &mut self; private deltas avoid overlapping borrows; no unsafe/global state | 5 | RuntimeTaskJournal, RuntimeTaskState, TaskLaunchAdapter, RuntimeSchedulerConfig. sole journal/counter/adapter/runtime-task owner |
| `arcweft_runtime_scheduler::RuntimeTaskJournal` | `crates/arcweft-runtime-scheduler/src/journal.rs` | inspect_ensure; validate_delta; apply_delta; validate_event; apply_event; snapshot_projection; restore_from_validated | private to scheduler; BTreeMap staging in temporary values | 5 | TaskGroup, TaskLaunch, RuntimeNeedCell, TaskObserver. not separately owned by driver |
| `arcweft_runtime_scheduler::RuntimeTaskState` | `crates/arcweft-runtime-scheduler/src/runtime_task.rs` | stage; select_step_batch; apply_step_result; snapshot_projection | select keys first, remove or clone bounded state, call scheduler internal ensure, then reinsert/update | 5 | RuntimeAwaitManyAggregateTask, RuntimeTimeoutNeed. runtime tasks never reach adapter |
| `arcweft_host_adapter::TaskLaunchAdapter` | `crates/arcweft-host-adapter/src/lib.rs` | prepare_launch/commit_launch/rollback_launch; prepare_restore/commit_restore/rollback_restore; prepare_rebind/commit_rebind/rollback_rebind | owned prepared tokens; prepare fallible; commit/rollback infallible | 5 | HostTaskLaunchRequest, HostTaskRestoreBatch, HostTaskRebindBatch. accepts Host execution rows only |
| `arcweft_lang_sema::FinalSemanticAnalysis` | `crates/arcweft-lang-sema/src/final_analysis/report.rs` | checked_match_ref; checked_match; write_expression_semantic_v1; write_pattern_semantic_v1 | exact report/module/symbol generation borrowed during construction | 1 | HirSnapshotId, ExprId, CheckedExpressionResolution, CheckedPatternResolution. current snapshot owner; no AcceptedSemanticGeneration |
| `arcweft_lang_sema::CheckedExpressionSemanticEncoderV1` | `crates/arcweft-lang-sema/src/final_analysis/semantic_transcript.rs` | write_expression; write_literal; stable_coordinate; declaration_semantic_id | raw arena IDs only for lookup; emitted transcript uses stable roles/accepted identities | 1 | FinalSemanticAnalysis, HirExecutableProjectView, ProjectSymbolTable. purpose-built, no generic Serde |
| `arcweft_lang_sema::OwnershipClassifier` | `crates/arcweft-lang-sema/src/ownership.rs` | classify(TypeId); evidence_digest; need_producer_admission | memoized bounded traversal over accepted TypeKind graph | 2 | TypeKind, AcceptedOpaqueRuntimeEvidence, RuntimeCarrierProjection. 85 exhaustive variants; Predicate leaf; Shared reject |
| `arcweft_compiler::CompilerLocalViewMatchCatalogRow` | `crates/arcweft-compiler/src/view.rs` | publish after exact CheckedMatchRef + View admission validation | compiler-generation-local; never serialized | 3 | CheckedMatchRef, ViewProgramId, AcceptedViewProgramRevision, ViewMatchSiteId. contains no task types |
| `arcweft_bundle::AcceptedViewMatchBundleRowV1` | `crates/arcweft-bundle/src/product.rs` | project_from_joined_products; validate_against | owned strict projection; joins compiler/AWBC/current revision products | 5 | View identity projections, checked/admission/ownership/task digests. no compiler-local IDs |
| `arcweft_runtime_scheduler::RuntimeTaskSnapshotCodecV1` | `crates/arcweft-runtime-scheduler/src/snapshot.rs` | encode; decode_private; validate_private; publish_restore | direct borrowed reader; private temporary maps; final mem::replace | 5 | all persistence rows, TaskLaunchAdapter restore transaction. strict v1 only |

## Dependency direction rules

- `arcweft-core` owns RuntimeValue, canonical value identity and core task/Need identity types.
- `arcweft-lang-sema` owns checked semantic facts, stable coordinates and ownership/admission certificates. It does not depend on runtime scheduler.
- `arcweft-view`/compiler own compiler-local View rows and current View identities.
- `arcweft-runtime-plan` computes task-plan/type projections and consumes sema products without creating a second semantic map.
- `arcweft-runtime-scheduler` consumes core task/value types and owns execution/persistence.
- `arcweft-host-adapter` defines the prepared-token boundary; concrete desktop/web/runtime-host adapters implement it.
- `arcweft-bundle` stores strict projections and does not depend on compiler-local IDs.
- `arcweft-runtime-driver` owns the scheduler as a field and does not own journal state.
