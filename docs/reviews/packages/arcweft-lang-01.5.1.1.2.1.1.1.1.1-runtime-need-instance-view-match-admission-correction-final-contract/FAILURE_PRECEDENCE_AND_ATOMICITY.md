# Failure precedence and atomicity

## 1. Global rule

At every boundary, validation proceeds from structural identity to semantic
content to work limits to publication. A later failure never masks an earlier
typed failure and never publishes a partial product.

Diagnostics and debug labels are not identity and cannot change precedence.

## 2. Generic Match

```text
stale/malformed HIR generation or poisoned owner
< missing checked scrutinee/arm/pattern/binding/body
< scrutinee/pattern/binding type mismatch
< non-Boolean guard
< unsupported/missing constructor-domain owner
< coverage work limit
< non-exhaustive witness
< retained unreachable diagnostics
< Match semantic digest construction
```

Only after success is `CheckedMatch` inserted. Warnings are emitted from the
retained successful evidence and never before the hard-error boundary.

## 3. Producer and View admission

```text
stale CheckedMatchRef / retained coordinate mismatch
< duplicate/missing output or capture
< unresolved type owner
< missing opaque value_class/persistence
< exact runtime carrier/snapshot mismatch
< affine/Stream/borrow/frame-local/ViewValue rejection
< recursive ownership cycle
< ownership work limit
< Need producer argument/capture admission
< ownership/producers digest construction
< View admission digest construction
< stable View site/coordinate construction
```

Failure publishes no View row but does not remove the valid generic Match.

## 4. Task construction

```text
TaskSpec structural/request/outcome validation
< producer instance field/key recomputation
< policy restriction (including MakeNeedHandle)
< journal capacity check
< Join existing-spec equality
< AlwaysStart ordinal overflow
< NeedId/TaskKey/TaskId zero-hash/derivation
< adapter prepare
< staged journal cross-reference validation
< atomic commit
```

The AlwaysStart counter is read before prepare and changed only at commit.
Every precommit failure consumes no ordinal.

## 5. Event ingestion

```text
generation lookup
< task lookup
< complete TaskCorrelation equality
< producer instance/contract equality
< Need row correlation equality
< TaskEventKind/outcome validation
< event digest validation (replay)
< cursor duplicate/stale/gap/conflict decision
< terminal transition validation
< observer fanout work limit
< atomic task/Need/observer update
```

A correlation-tampered event cannot be accepted as a stale duplicate.

## 6. Duplicate/stale/conflict behavior

| Case | Result | State mutation |
|---|---|---|
| exact cursor + exact event digest | duplicate success | no semantic mutation |
| lower cursor | stale success/audit | no semantic mutation |
| exact cursor + different event | conflict error | none |
| cursor gap/epoch regression | cursor error | none |
| publication after terminal | terminal error | none |
| same TaskKey but wrong correlation | correlation error | none |
| different AlwaysStart NeedId | independent stream | normal |

Audit counters are nonidentity and updated only in a separate bounded metrics
channel.

## 7. Cancellation

Cancellation remains the existing `Need::Cancelled` state.

Same-step task/timeout ordering retains parent authority:

```text
scope cancellation
< source terminal publication
< timeout expiration
< pending/progress
```

Cancellation does not fabricate `InfrastructureFailure` or a domain Result
error. A post-cancellation task publication is rejected as post-terminal after
correlation validation.

Observer detachment alone does not cancel producer work unless the retained
parent producer/cancel-scope lifecycle authorizes it.

## 8. Save restore

```text
envelope/version
< byte/count/depth limits
< sorted uniqueness and duplicate keys
< fixed identity nonzero decode
< TaskSpec/producer/plan/type/value digest validation
< all correlation rederivation
< group/task/Need/observer cross-reference validation
< cursor/terminal invariants
< embedded handle/AwaitMany validation
< replacement mapping validation
< one atomic runtime swap
```

The old live runtime remains untouched on any failure.

## 9. Replay

```text
envelope/version/generation
< event digest
< normal event correlation
< normal cursor/state transition
< observer invalidation
```

Replay does not have a compatibility or relaxed branch.

## 10. View bundle and replacement

Bundle admission:

```text
strict version 1 decode
< current ViewProgramId parse
< AcceptedViewProgramRevision validation
< checked coordinate/digest cross-section
< task-plan recomputation
< producer/payload/argument/evidence join
< bounded catalog publication
```

Replacement:

```text
explicit site mapping
< old/new accepted revision validity
< generic Match equality
< View admission equality
< producer admission equality
< producer family/contract equality
< payload/plan equality
< ownership/resource/argument equality
< quiescent barrier
< new generation correlation derivation
< adapter rebind prepare
< staged runtime cross-reference validation
< atomic catalog/runtime/adapter commit
```

Revision equality is not in the equality list.

## 11. Atomicity table

| Operation | Staged owners | Commit result | Rollback result |
|---|---|---|---|
| generic Match construction | private Match/coverage/digest | one CheckedMatch | no Match/warnings/digest |
| ownership certificate | private traversal/evidence | one certificate | no certificate |
| View admission | private outputs/captures/evidence | one View row | generic Match retained; no View row |
| Join new launch | group/task/Need + adapter token | one launch | no group/task/Need |
| Join existing | observer/metric only | existing handle | no change on spec conflict |
| AlwaysStart launch | counter/group/task/Need + adapter token | ordinal and launch visible together | counter unchanged |
| event apply | task/Need/observer invalidations | all new state | all old state |
| AwaitMany child batch | bounded child rows/fiber state | complete selected batch | no child/ordinal leak |
| aggregate completion | outputs/base Need/event | complete terminal | old in-flight state |
| timeout construction | derived spec/Need/observer | separate Join Need | source untouched |
| save restore | private decoded maps | one runtime swap | previous runtime |
| replay event | normal event transaction | same as live | no state |
| View replacement | catalog/tasks/Needs/observers/adapters | complete new generation | old generation or prescribed cancellation transaction |

## 12. Infrastructure versus domain failure

`RuntimeTaskFailure` is accepted only from trusted host/runtime boundaries and
contains a closed failure kind plus bounded diagnostic text. It is stored as:

```text
Need::Ready(RuntimeNeedOutcome::InfrastructureFailure(...))
```

The awaited function's declared domain error remains inside its typed
RuntimePayload. Conversion between these categories is forbidden.

## 13. Integer and allocation failure

Checked arithmetic precedes allocation. A counter overflow, length overflow,
or `u32` source-index overflow is a typed work/identity error. It cannot wrap,
truncate, consume an AlwaysStart ordinal, or produce a partial digest.

A BLAKE3 all-zero result for producer-instance/Need/task fixed identity is a
typed failure. The implementation does not rehash with another domain, append a
byte, or map it to a sentinel.
