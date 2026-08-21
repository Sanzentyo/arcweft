# Decision register

Every row is normative and closed. “Rejected” means the implementation must
not preserve that alternative even temporarily in a public cut.

| ID | Owner | Selected decision | Rejected |
|---|---|---|---|
| `A1` | `arcweft_core::task::NeedProducerInstanceKey` | One fully bound producer request is hashed from the exact version-1 producer-instance transcript. | No second producer identity, source key, or generation/ordinal input. |
| `A2` | `arcweft_core::task::{NeedId,TaskKey,TaskId}` | NeedId owns one terminal cell, TaskKey one generation-bound coalescing group, TaskId one actual launch. | No role sharing or String/suffix identity. |
| `A3` | `TaskPolicy` | JoinSameKey tag is 0 and ordinal is exactly 0; AlwaysStart tag is 1 and journal ordinals begin at 1. | No caller-supplied ordinal or random ID. |
| `A4` | `RuntimeTaskJournal` | AlwaysStart allocation, insertion, adapter prepare/accept, and visibility are one rollback-capable transaction. | No consumed ordinal on failure. |
| `A5` | `RuntimeNeedHandle` | Reusable handles and MakeNeedHandle are JoinSameKey only; AlwaysStart handle is launch output only. | No reusable AlwaysStart descriptor. |
| `A6` | `AwaitManyTarget` | Base and children use canonical source-order tuples; child tuple includes exact source index. | No indexed string suffix or map iteration. |
| `A7` | `Timeout producer` | Timeout hashes exact source NeedId via canonical NeedHandle plus limit and publishes a distinct Join cell. | No source mutation, parsing, or cancellation coupling. |
| `A8` | `RuntimeNeedJournal` | Terminal idempotence/conflict is keyed by generation, NeedId, producer contract, and cursor. | No false conflict across AlwaysStart NeedIds. |
| `B1` | `arcweft_core::entry::RuntimeValueDigest` | The existing digest type is reused everywhere. | No duplicate RuntimeValueDigest. |
| `B2` | `RuntimeValue::Tuple` | Argument and AwaitMany lists are canonical Tuple values; empty is Tuple([]) digest. | No ZERO sentinel or ordered_source_digest grammar. |
| `B3` | `RuntimeValue canonical visitor` | One sink-parametric visitor writes bytes or BLAKE3 with identical budgets and grammar. | No duplicate encoder or intermediate digest buffer. |
| `B4` | `RuntimeValue::NeedHandle` | NeedHandle is a canonical RuntimeValue variant whose value identity is exact NeedId. | No String or Dynamic carrier. |
| `B5` | `Fixed identity constructors` | Producer-instance/Need/task IDs reject zero hash with typed error and no rehash. | No ZERO default for fixed identity types. |
| `B6` | `Semantic digests` | Semantic digest wrappers accept all hash outputs; absence is Option. | No reserved-zero absence. |
| `B7` | `NeedProducerContractDigest` | Contract transcript contains only accepted callable/host/builtin contract evidence. | No plan, site, payload, arguments, generation, policy, or debug fields. |
| `B8` | `TaskPlanSemanticDigest` | Plan transcript contains executable and static task-plan semantics; View rows include program/site/admission and exclude revision. | No plan self-digest field. |
| `C1` | `arcweft_core::task::GenerationId` | GenerationId moves to core as the one shared typed owner; zero is valid. | No runtime-driver duplicate or conversion DTO. |
| `C2` | `TaskSpec` | TaskSpec carries generation, producer instance, scheduling, policy, outcome, request, and debug label only. | No caller NeedId/TaskKey/TaskId/ordinal. |
| `C3` | `TaskHost::ensure_task` | ensure_task derives or allocates correlation and returns Result<TaskHandle, TaskEnsureError>. | No identity injection by caller/adapter. |
| `C4` | `TaskCorrelation` | Every handle, event, Need state, journal row, host envelope, save row, and replay row carries the same complete correlation. | No partial event schema. |
| `C5` | `TaskEvent stream` | One launch has one cursor-ordered event stream; Join fanout is observer-table state. | No observer-specific terminal publications. |
| `C6` | `RuntimeNeedOutcome` | Ready payload values and infrastructure failure are distinct; cancellation remains Need::Cancelled. | No domain-error fabrication from host failure. |
| `C7` | `AwbcTaskPlan` | need_id becomes a mandatory typed producer row; semantic_digest(&AwbcProgram) recomputes and is not stored in the plan. | No self-digest or delayed consumer switch. |
| `D1` | `CheckedMatchSemanticDigest` | Digest commits checked semantic Match meaning, bindings, guard class, bodies, and coverage only. | No View, ownership, HIR/session, source/debug, or counters. |
| `D2` | `CheckedViewMatchAdmissionDigest` | Digest commits Match digest, retained outputs/captures, exact dispositions/evidence, and Need-producer admission. | No program, revision, site, or whole unrelated catalog digest. |
| `D3` | `ViewMatchSiteId` | Site hashes ViewProgramId, accepted enclosing declaration identity, and closed child-role path. | No HIR ID, SourceSpan, spelling, or revision. |
| `D4` | `CheckedViewMatchCoordinate` | Coordinate is exactly program + site + admission. | No invented ViewProgramSemanticDigest. |
| `D5` | `AcceptedViewProgramRevision` | Current [u8;32] accepted revision is retained only for catalog/bundle/registry/replacement. | No u32 revision or identity input. |
| `D6` | `View replacement` | Explicit mapping may rebind only after all semantic/producer/argument/evidence checks; revision-only change is allowed. | No NeedId translation table or implicit mapping. |
| `E1` | `CheckedMatch::try_from_hir` | Generic construction performs structure/type/pattern/guard/coverage/reachability/digest checks only. | No retained View or producer admission. |
| `E2` | `MatchCoverageAnalyzer` | Only exact checked Boolean literals are constant; every other guard is Dynamic. | No source evaluation or string folding. |
| `E3` | `FalseGuard precedence` | ConstantFalse owns FalseGuard independent of earlier pattern coverage. | No coverage-first masking of FalseGuard. |
| `E4` | `CheckedViewMatchAdmission` | Separate admission blocks only View publication. | No loss of legal ordinary affine Match. |
| `E5` | `CheckedNeedProducerAdmission` | Producer argument/capture admission is a separate certificate and digest. | No construction of producer contract identity. |
| `F1` | `AcceptedNominalInventoryInput` | value_class and persistence become mandatory constructor fields. | No defaults or name inference. |
| `F2` | `AcceptedNominalSemantics::Opaque` | Original enum variant gains producer, value_class, and persistence and owns inherent behavior. | No extension trait/helper side table. |
| `F3` | `AcceptedNominalCatalogDigest` | Opaque evidence is included in the accepted catalog transcript. | No uncommitted evidence. |
| `F4` | `CheckedOwnershipContext` | Context is exactly ProjectSymbolTable plus RegisteredSemanticWorld. | No ResourceTypeRegistry without typed key. |
| `F5` | `AgentResource/AgentResourceBody` | Current core Agent DTO owner classifies SnapshotClone. | No unkeyed registry query. |
| `F6` | `TypeKind::Need` | Need itself is SnapshotClone after exact producer-argument certificate; payload bindings are classified independently. | No generic Match rejection. |
| `F7` | `TypeKind::Ref` | Ref is SnapshotClone because runtime carrier is EntityRef(String). | No affine classification. |
| `F8` | `TypeKind::ViewValue` | ViewValue rejects with MissingViewPersistenceEvidence. | No inferred snapshot layout. |
| `F9` | `TypeKind::Function` | Type-level classification rejects; only an exact value-level stable callable certificate may admit a value. | No closure inference from type. |
| `F10` | `TypeKind::Shared` | Shared<T> is SnapshotClone after child admission. | No blind unconditional admission. |
| `F11` | `Opaque Plain/AffineHandle` | Plain is SnapshotClone under either admitted persistence mode; AffineHandle rejects. | No producer-name inference. |
| `F12` | `CheckedOwnershipClassifier` | All current TypeKind rows have exact recursion, limits, cycle, and first-error behavior. | No default wildcard branch. |
| `CUT1` | `Generic Match cut` | Land semantic encoders, bounded coverage, CheckedMatch, and digest without View/ownership/runtime changes. | No mixed cut. |
| `CUT2` | `Ownership cut` | Change input-through-catalog evidence and every constructor/fixture in one cut, then add total classifier/certificates. | No partial default-bearing chain. |
| `CUT3` | `View admission cut` | Land admission/site/catalog/compiler/bundle/runtime/replacement together. | No copied Match authority. |
| `CUT4` | `Private identity preparation cut` | Land private fixed types, transcripts, sink encoder, and core GenerationId without public switch. | No public dual schema. |
| `CUT5` | `Atomic carrier cut` | Switch all public task/Need/Await/persistence/host consumers and delete every old path in one protected commit. | No delayed snapshot or fallback. |
| `NUMERIC` | `Maintained AWBC authority` | Numeric allocation is a frozen external prerequisite and is not restated or changed. | No second numeric table. |

`OPEN_QUESTIONS=0`.
