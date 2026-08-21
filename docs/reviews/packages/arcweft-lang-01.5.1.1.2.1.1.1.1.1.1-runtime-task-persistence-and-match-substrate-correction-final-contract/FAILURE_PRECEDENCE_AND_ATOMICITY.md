# Failure precedence and atomicity

## Global rule

Every boundary validates structural identity before semantic joins, then work
limits, then publication. A later error never masks an earlier error and no
operation publishes partial state.

Debug labels and source diagnostics are never consulted for identity,
execution selection or error precedence.

## Canonical RuntimeValue identity

```text
recursion/depth
< node count
< current variant structural invariants
< opaque producer/type/class/persistence/payload invariants
< affine value-identity rejection
< byte limit during the sole transcript
```

The byte sink and BLAKE3 sink return the same first error.

## Constant publication

```text
constant-admission recursion/node limit
< first forbidden value in canonical child order
< canonical identity validation
< constant catalog/type validation
< publication
```

`SnapshotOnlyOpaque` is raised at the explicit fence. It is not an identity
error.

## Generic Match

```text
foreign/stale HirSnapshotId
< missing checked expression/pattern/value facts
< stable declaration-root construction
< stable child/pattern/binding coordinate uniqueness
< type/guard/constructor-domain validity
< coverage work limit
< nonexhaustive witness
< unreachable evidence normalization
< expression/pattern/Match digest construction
< publication
```

Warnings are derived from the successful private Match result.

## Ownership and View admission

```text
stale CheckedMatchRef
< unresolved TypeKind/accepted owner
< opaque evidence completeness
< exact carrier projection/live value/canonical/snapshot mapping
< affine/borrow/Stream/frame-local/View rejection
< recursive type work/cycle limit
< producer admission digest
< View admission/site construction
< compiler-local catalog publication
```

Generic Match remains published if View admission fails.

## `ensure_task`

```text
TaskSpec structural validation
< family/execution/policy truth table
< producer instance recomputation
< journal/capacity limit
< Join existing spec equality
< AlwaysStart ordinal availability/overflow
< NeedId/TaskKey/TaskId derivation and all-zero rejection
< runtime request invariants
< Host adapter prepare
< complete staged delta validation
< infallible journal/runtime apply
< infallible Host adapter commit
```

There is no error after staged validation. `AdapterCommit` does not exist.

## Observer registration/use

```text
handle structural validation
< active-generation equality
< Need lookup and producer/outcome equality
< observer work/capacity limit
< observer ID allocation
< forward/reverse staged join
< atomic publication
```

A stale handle allocates nothing and mutates nothing.

## Event ingestion

```text
generation lookup
< TaskId lookup
< complete TaskCorrelation equality
< producer/spec/Need joins
< TaskEventKind payload/failure validation
< replay event digest (when replaying)
< cursor relation
< lifecycle/Need transition
< observer fanout limit
< atomic launch/Need/observer/runtime-task delta
```

A correlation-tampered event cannot be classified as a stale duplicate.

## AwaitMany

```text
aggregate cancellation
< child event correlation/cursor
< child terminal normalization
< aggregate terminal precedence
< selected launch batch work limit
< every child spec/family/argument revalidation
< Host adapter preparations for the batch
< complete batch delta validation
< atomic child/observer/ordinal publication
< Pending progress
```

A failed batch leaks neither a child row nor an AlwaysStart ordinal.

## Timeout

```text
output/source handle structure
< source active-generation use
< wrapper cancellation
< normalized source terminal
< checked dt/remaining update
< expiration
< Pending
```

No wall-clock read can introduce another branch.

## Snapshot decode/restore

```text
envelope/version exactly one
< byte/count/depth limits and canonical varints
< enum tag/field validity
< canonical key order and duplicates
< fixed identity all-zero rejection
< RuntimeValue/TaskSpec/producer/type/value digest validation
< correlation rederivation
< group/launch/Need/observer/runtime-state joins
< AwaitMany/Timeout phase invariants
< replay and replacement mapping invariants
< Host restore-policy/quiescence
< adapter restore prepare
< final temporary-state validation
< one state swap
< infallible adapter commit
```

The prior live scheduler remains untouched through every error branch.

## Replacement

```text
old/new compiler and bundle product validity
< site mapping uniqueness
< accepted revision validity
< checked Match/View/Need/ownership equality
< producer family/contract/payload/plan/arguments equality
< quiescent barrier
< new GenerationId validity
< complete correlation rederivation
< runtime/observer mapping
< Host adapter rebind prepare
< complete staged state validation
< one scheduler swap
< infallible adapter commit
```

NeedId and ordinal mismatches are rejected before prepare.

## Atomicity table

| Operation | Private staged values | Prepared external token | Commit | Failure result |
|---|---|---|---|---|
| canonical bytes/digest | work counter and sink | none | returned bytes/digest | no partial result |
| Match | semantic rows, coverage, digests | none | one `CheckedMatch` | no Match/warnings |
| ownership | traversal and evidence rows | none | one certificate | no certificate |
| View admission | site/output/capture/evidence rows | none | one compiler-local row | generic Match retained |
| Join new | group/launch/Need/runtime delta | Host only | all rows + adapter | no rows |
| Join existing | validated handle projection | none | return handle | no change |
| AlwaysStart | counter/group/launch/Need/runtime delta | Host only | ordinal and launch together | counter unchanged |
| observer register | observer + reverse Need membership | none | both links | neither link |
| event apply | launch/Need/observer/runtime delta | none | all transitions | old state |
| AwaitMany batch | child launches/observers/statuses | per Host child | complete selected batch | no child/ordinal leak |
| timeout stage | runtime row/source observer | none | both rows | no wrapper |
| cancellation | all affected terminal/link deltas | optional Host cancellation batch | complete transaction | old state |
| restore | complete temporary scheduler | Host restore batch | state swap + adapter | old live scheduler |
| replacement | complete new-generation state | Host rebind batch | generation swap + adapter | old generation |

## Reachable error enums

`TaskEnsureError` contains only:

```text
InvalidSpec
InvalidProducer
FamilyExecutionMismatch
PolicyMismatch
JournalLimit
JoinSpecConflict
OrdinalOverflow
IdentityDerivation
AdapterPrepare
StagingInvariant
```

Restore and replacement have corresponding `AdapterPrepareRestore` and
`AdapterPrepareRebind` errors. There is no commit error for any operation.
