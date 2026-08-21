# Focused and adversarial test matrix

`machine/tests.json` contains 94 rows. Implementation tests use the exact IDs below; package-only validator self-tests are marked `kind=package`.

## Focused

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `VAL-001` | `arcweft-core canonical RuntimeValue visitor` | Plain+SnapshotOnly opaque canonical bytes succeed and preserve the existing opaque transcript | 4 / implementation |
| `VAL-003` | `Need producer admission` | the same Plain+SnapshotOnly value passes argument admission and producer-instance construction | 5 / implementation |
| `EXE-005` | `AwaitMany base` | aggregate TaskSpec accepts Runtime::AwaitManyAggregate and rejects Host | 5 / implementation |
| `EXE-006` | `Timeout family` | Timeout accepts Runtime::Timeout+Join only and never reaches adapter | 5 / implementation |
| `EXE-007` | `LineTask` | LineTask accepts Host+AlwaysStart only | 5 / implementation |
| `EXE-008` | `ViewMatchSubscription` | View Match subscription accepts Host+Join only | 5 / implementation |
| `EXE-009` | `AwaitMany child` | host and both runtime child execution variants are accepted when their complete child TaskSpec validates | 5 / implementation |
| `SCH-004` | `TaskLaunchAdapter` | commit_launch and rollback_launch return unit and are infallible by trait contract | 5 / implementation |
| `HND-001` | `RuntimeNeedHandle` | same NeedId with different debug/origin/spec labels compares, hashes and orders equal | 5 / implementation |
| `HND-002` | `RuntimeValue::NeedHandle` | canonical bytes are exactly tag 20 plus NeedId | 5 / implementation |
| `AM-001` | `RuntimeAwaitManyAggregateTask` | source order and output order are identical even when children complete out of order | 5 / implementation |
| `AM-004` | `AwaitMany terminal precedence` | aggregate cancellation beats child terminal; infrastructure failure beats all-ready; otherwise all-ready publishes once | 5 / implementation |
| `TO-001` | `RuntimeTimeoutNeed` | first demand starts remaining=requested_limit and registers one source observer | 5 / implementation |
| `TO-003` | `RuntimeTimeoutNeed` | same-step precedence is cancellation then normalized source terminal then expiration then Pending | 5 / implementation |
| `TO-004` | `RuntimeTimeoutNeed` | zero duration snapshots source first and only then expires | 5 / implementation |
| `TO-005` | `RuntimeTimeoutNeed` | cancelling wrapper does not cancel source Need | 5 / implementation |
| `EVT-004` | `failure categories` | domain Result/Option error remains payload; infrastructure failure remains typed RuntimeTaskFailure; cancellation is Need::Cancelled | 5 / implementation |
| `MAT-010` | `guard classification` | only exact Boolean literals become ConstantTrue/ConstantFalse; all other checked Boolean expressions are Dynamic | 1 / implementation |
| `MAT-012` | `coverage` | bounded constructor matrix publishes exhaustive witness and sorted unreachable evidence | 1 / implementation |
| `BND-002` | `bundle validation` | joins compiler/AWBC/current revision products without minting a new semantic identity | 5 / implementation |
| `OWN-002` | `TypeKind::Predicate` | classifier has no child recursion edge | 2 / implementation |

## Property

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `VAL-006` | `canonical sink pair` | byte sink and hash sink have identical recursion/node/byte limit and first-error precedence | 4 / implementation |
| `SCH-005` | `AlwaysStart counter` | successful launches use distinct positive ordinals beginning at one; failures create no gaps | 5 / implementation |
| `SCH-006` | `Join` | same generation+instance returns one NeedId/TaskKey/TaskId and never invokes adapter twice | 5 / implementation |
| `AM-002` | `AwaitMany launch cursor` | at most limit children are active and launch_cursor never exceeds source_count | 5 / implementation |
| `MAT-011` | `stable coordinates` | every checked child/binding resolves to one declaration-rooted role path; duplicate coordinate construction rejects | 1 / implementation |
| `OWN-004` | `SnapshotClone evidence` | every successful row names projection, live carrier, canonical identity and snapshot codec | 2 / implementation |

## Differential

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `VAL-002` | `arcweft-core canonical RuntimeValue visitor` | direct BLAKE3 sink equals BLAKE3(bytes sink output) for every RuntimeValue variant and limit edge | 4 / implementation |
| `EXE-004` | `family execution validation` | debug label and host operation spelling changes cannot change family/execution selection | 5 / implementation |
| `TO-002` | `RuntimeTimeoutNeed` | only RuntimeStepInput.dt changes remaining; wall/monotonic clocks are never read | 5 / implementation |
| `MAT-007` | `CheckedMatchSemanticDigest` | renumber ExprId/PatternId/LocalId and change SourceSpan while preserving semantics leaves digest unchanged | 1 / implementation |
| `MAT-008` | `CheckedMatchSemanticDigest` | semantic field/case/callable/layout changes alter digest | 1 / implementation |

## Exhaustive

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `EXE-001` | `NeedProducerFamily policy` | all nine families have at least one explicit execution route | 5 / implementation |
| `MAT-001` | `CheckedExpressionResolution transcript` | all 27 current variants have one semantic transcript arm | 1 / implementation |
| `MAT-002` | `CheckedValueResolution transcript` | all 8 current variants have one semantic transcript arm | 1 / implementation |
| `MAT-003` | `CheckedSelectResolution transcript` | all 7 current variants have one semantic transcript arm | 1 / implementation |
| `MAT-004` | `CheckedPatternResolution transcript` | all 5 current variants have one semantic transcript arm | 1 / implementation |
| `MAT-005` | `HirPatternKind transcript` | all 13 current families are covered; Error returns typed failure | 1 / implementation |
| `MAT-006` | `HirLiteral transcript` | all 7 current literal families use exact semantic payload rules | 1 / implementation |
| `OWN-001` | `TypeKind classifier` | all 85 current variants have one machine/prose/classifier row | 2 / implementation |

## Negative

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `VAL-005` | `runtime-plan constant lowering` | the same Plain+SnapshotOnly value fails at ConstantAdmissionError::SnapshotOnlyOpaque | 5 / implementation |
| `VAL-007` | `canonical RuntimeValue identity` | AffineHandle fails even in non-producer diagnostic RuntimeValueDigest; snapshot codec does not mint identity | 4 / implementation |
| `EXE-002` | `TaskSpec` | unconditional request field and parallel host/runtime Option fields do not compile | 5 / implementation |
| `EXE-003` | `TaskLaunchAdapter` | Runtime AwaitMany/Timeout rows are rejected before prepare_launch | 5 / implementation |
| `SCH-007` | `Join` | same TaskKey with structurally different spec returns JoinSpecConflict without mutation | 5 / implementation |
| `HND-003` | `Await boundary` | stale-generation handle has equal value identity but fails before observer/task mutation | 5 / implementation |
| `HND-004` | `timeout boundary` | stale-generation source handle fails before runtime timeout staging | 5 / implementation |
| `EVT-003` | `event application` | gap, epoch regression and post-terminal publication reject without mutation | 5 / implementation |
| `PER-007` | `restore` | in-flight MustBeQuiescent host row makes snapshot/restore fail rather than serialize adapter-private state | 5 / implementation |
| `PER-008` | `snapshot` | snapshot while adapter launch/rebind token is prepared is rejected | 5 / implementation |
| `MAT-009` | `semantic transcript` | raw HIR ids, spans, source spelling, debug names, hash-map order and generic Serde are absent | 1 / implementation |
| `MAT-013` | `CheckedMatchRef` | stale or foreign HirSnapshotId rejects exact lookup | 1 / implementation |
| `OWN-003` | `TypeKind::Shared` | returns MissingRuntimeSnapshotOwner and no Shared carrier/side table exists | 2 / implementation |

## Tamper

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `HND-006` | `handle construction/restore` | tampered producer/spec/correlation fails structural validation | 5 / implementation |
| `EVT-001` | `host event ingestion` | wrong generation/task/producer/Need correlation rejects before cursor duplicate logic | 5 / implementation |
| `PER-002` | `strict decoder` | unknown field/tag rejects | 5 / implementation |
| `PER-003` | `strict decoder` | duplicate map key/field rejects before publication | 5 / implementation |
| `PER-004` | `strict decoder` | trailing bytes and nonminimal u32 varints reject | 5 / implementation |
| `PER-005` | `identity restore` | stored NeedProducerInstanceKey/NeedId/TaskKey/TaskId mismatches rederived values reject | 5 / implementation |
| `BND-003` | `bundle validation` | any checked-match/view/need/ownership/contract/type/plan/arguments mismatch rejects | 5 / implementation |

## Rollback

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `SCH-001` | `RuntimeTaskScheduler::ensure_task` | adapter prepare failure leaves group/task/Need and ordinal counter unchanged | 5 / implementation |
| `SCH-002` | `RuntimeTaskScheduler::ensure_task` | post-prepare staging invariant failure calls rollback and consumes no ordinal | 5 / implementation |
| `SCH-010` | `replacement rebind` | prepare_rebind failure preserves old generation and adapter state | 5 / implementation |
| `SCH-011` | `replacement rebind` | precommit validation failure rolls back prepared rebind and publishes no new correlation | 5 / implementation |
| `AM-003` | `AwaitMany child batch` | failed child ensure publishes none of the selected batch and leaks no AlwaysStart ordinal | 5 / implementation |
| `PER-006` | `restore` | any join/invariant/adapter prepare failure leaves the old scheduler state untouched | 5 / implementation |

## Snapshot

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `VAL-004` | `RuntimeValueSnapshotV1` | the same Plain+SnapshotOnly value round-trips save/restore exactly | 5 / implementation |
| `AM-005` | `AwaitMany` | mid-flight source/item/spec/child/output/cursor state round-trips exactly | 5 / implementation |
| `TO-006` | `RuntimeTimeoutNeed` | remaining/phase/source subscription/publication cursor round-trip exactly | 5 / implementation |
| `PER-001` | `RuntimeTaskSchedulerSnapshotV1` | complete scheduler snapshot round-trips all generation/group/launch/Need/observer/runtime-task rows | 5 / implementation |

## Replay

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `EVT-002` | `event application` | same cursor+same digest is duplicate success; same cursor+different digest is conflict | 5 / implementation |
| `PER-010` | `TaskReplayEnvelopeV1` | strict generation/event-digest/correlation/cursor validation matches live event path | 5 / implementation |

## Replacement

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `HND-005` | `replacement rebind` | valid explicit rebind preserves NeedId/ordinal and rederives TaskKey/TaskId for new generation | 5 / implementation |

## Migration

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `VAL-009` | `dialogue/config/command/runtime-plan constants` | every current constant publisher calls explicit constant admission before publication | 5 / implementation |

## Borrow Flow

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `SCH-009` | `RuntimeTaskScheduler` | ensure/event/step/observer/snapshot/restore/rebind APIs compile without unsafe or global interior state | 5 / implementation |

## Dependency

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `CUT-001` | `Cut 3` | type dependency set contains no Cut 4 task/digest types | 3 / implementation |

## Compile Clean

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `CUT-003` | `all cuts` | each cut lists exact crates/features and compiles using same/earlier cut types only | 5 / implementation |

## Structural Absence

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `VAL-008` | `workspace` | no producer-argument-only serializer or second RuntimeValueDigest grammar exists | 5 / implementation |
| `SCH-003` | `TaskEnsureError` | AdapterCommit variant is absent | 5 / implementation |
| `SCH-008` | `runtime-driver` | no second journal, AlwaysStart counter or RuntimeTaskRegistry remains | 5 / implementation |
| `PER-009` | `codec` | no V2, legacy reader, String fallback, translation table or zero sentinel exists | 5 / implementation |
| `BND-001` | `AcceptedViewMatchBundleRowV1` | no CheckedMatchRef, ExprId, HirSnapshotId, SourceSpan or compiler certificate field | 5 / implementation |
| `CUT-002` | `Cut 4` | no public RuntimeValue variant or final TaskSpec schema is claimed | 4 / implementation |
| `CUT-004` | `Cut 5` | old String/dual route and copied View row are absent in production, tests, fixtures and generated surfaces | 5 / implementation |

## Validator Negative

| ID | Owner | Assertion | Cut/kind |
|---|---|---|---|
| `SELF-001` | `tools/validate_package.py` | validator rejects reintroduced AdapterCommit | 0 / package |
| `SELF-002` | `tools/validate_package.py` | validator rejects TaskSpec.request | 0 / package |
| `SELF-003` | `tools/validate_package.py` | validator rejects missing schema owner | 0 / package |
| `SELF-004` | `tools/validate_package.py` | validator rejects ExprId/HirSnapshotId/CheckedMatchRef | 0 / package |
| `SELF-005` | `tools/validate_package.py` | validator rejects accepted Shared | 0 / package |
| `SELF-006` | `tools/validate_package.py` | validator rejects a child edge | 0 / package |
| `SELF-007` | `tools/validate_package.py` | validator rejects Cut 4/public-private variant wording | 0 / package |
| `SELF-008` | `tools/validate_package.py` | validator rejects NeedProducerContractDigest in Cut 3 | 0 / package |
| `SELF-009` | `tools/validate_package.py` | validator rejects fewer than nine family rows | 0 / package |
| `SELF-010` | `tools/validate_package.py` | validator rejects any Arcweft marker other than 1 | 0 / package |

## Required fixture strategy

- Identity fixtures construct accepted producer/type/plan/value owners and never inject expected IDs into constructors.
- Plain+SnapshotOnly paired evidence reuses one exact RuntimeValue instance across canonical, producer, snapshot and constant tests.
- Event fixtures compute digest through the real event owner, then tamper one field at a time.
- Snapshot tests encode through `RuntimeTaskSnapshotCodecV1`, decode through the direct borrowed reader and restore into a fresh scheduler with a deterministic fake adapter.
- Rollback adapters record prepared/committed/rolled-back token IDs and can fail only prepare. No fake commit error exists.
- Match differential fixtures rebuild semantically equivalent HIR with deterministic ID/span permutations.
- Generated enum coverage fixtures are derived from explicit variant constructors, not source-grep gates.

## No source-gate substitution

Structural-absence tests may inspect public schemas/machine projections or use compile-fail API fixtures. They do not replace behavioral tests with fragile source substring assertions. The package validator scans its own contract artifacts because its purpose is package consistency, not production behavior.
