# Final contract

## 1. Scope and precedence

This design is the single complete nonnumeric correction required by
Lang-01.5.1.1.2.1.1.1.1.1. It preserves the parent unary-Need lifecycle,
selector Variant/Tuple ABI, explicit guard Branch lowering, View/core
independence, timeout race ordering, line-plan result authority, Stream
boundaries, and the maintained AWBC version-1 numeric allocation.

The authority order is:

1. production and maintained documentation at `17b384a36e1412cc7e7d9f13073d8dd33dcb5cbc`;
2. later accepted contracts and the 2026-08-22 repository intake;
3. this request and its required exact decisions;
4. retained parent contracts where not contradicted here;
5. predecessor-package observations only as frozen evidence.

No numeric AWBC allocation is restated. A mismatch with the frozen predecessor
numeric tables is not an alternative and does not reopen this design.

## 2. Identity separation

The runtime has four distinct owners:

| Owner | Meaning | Generation input | Launch ordinal input |
|---|---|---:|---:|
| `NeedProducerInstanceKey` | one fully bound producer request before policy | no | no |
| `NeedId` | one concrete terminal cell within its generation | no | yes |
| `TaskKey` | one generation-bound coalescing group | yes | no |
| `TaskId` | one actual launch | through `TaskKey` | yes |

The exact transcripts are normative in `IDENTITY_AND_DIGESTS.md`. Fixed
producer/Need/task identities reject all-zero bytes without rehash. Semantic
digest types admit the full hash output; optionality is represented only by
`Option`.

`JoinSameKey` uses ordinal `0`. `AlwaysStart` uses a generation-and-instance
scoped monotonic journal counter beginning at `1`. The counter advance,
correlation derivation, journal insertion, adapter prepare/accept, and task
visibility form one transaction. An error consumes no ordinal and exposes no
task, event stream, Need cell, or observer-visible state.

## 3. Runtime value and argument authority

`arcweft_core::entry::RuntimeValueDigest` remains the sole digest type for
runtime values. `RuntimeValue::try_canonical_bytes` and `try_digest` share one
sink-parametric visitor. Byte encoding and BLAKE3 hashing therefore have the
same grammar, recursion, ordering, byte budget, and first-error behavior;
hashing allocates no intermediate byte buffer.

Producer arguments and AwaitMany item lists are canonical
`RuntimeValue::Tuple` values in source order. Empty arguments are the digest of
`RuntimeValue::Tuple(Vec::new())`; `RuntimeValueDigest::ZERO` never denotes an
empty list.

`RuntimeValue::NeedHandle` is added to the existing owner. Its canonical
runtime-value identity is the exact fixed `NeedId`. Its private snapshot codec
also carries the full validated handle and rederives every correlation field
on restore.

## 4. Task and Need boundary

`GenerationId(u64)` moves from runtime-driver to
`arcweft_core::task::GenerationId`. Zero remains valid.

`TaskSpec` contains generation, a complete `NeedProducerInstance`, scheduling
metadata, policy, outcome, request, and debug label. It contains no `NeedId`,
`TaskKey`, `TaskId`, or ordinal. `TaskHost::ensure_task` derives or allocates a
`TaskCorrelation` and returns a `TaskHandle`.

A launch has one event stream. Join fanout is an observer-table operation,
never multiple task event streams or multiple terminal publications. Every
event carries the complete correlation and a publication cursor. Correlation
tamper fails before cursor handling. The terminal idempotence/conflict domain
is exactly `(GenerationId, NeedId, NeedProducerContractDigest, cursor)`.

Domain failure is a typed `Ready(RuntimePayload(Result::Err(...)))` value.
`InfrastructureFailure` is reserved for host/runtime failure.
Cancellation remains `Need::Cancelled`.

## 5. Handle policy

`MakeNeedHandle` and any reusable immutable `RuntimeNeedHandle` are
`JoinSameKey` only. The AWBC verifier rejects a `MakeNeedHandle` task plan whose
policy is `AlwaysStart`.

For `AlwaysStart`, a producer descriptor is not a pre-launch handle. The only
way to obtain a concrete `RuntimeNeedHandle` is a successful
`TaskHost::ensure_task` launch transaction. That handle may be awaited, saved,
or passed to timeout, but cannot be submitted as a reusable start descriptor.

Direct Await consumes the concrete handle and reads its `NeedId`; it neither
parses nor rederives identity.

## 6. AwaitMany and timeout

AwaitMany evaluates its source once. It creates:

- one source-order base producer instance representing the aggregate Need; and
- one child producer instance for each source index.

A child argument tuple contains the captured producer arguments, the exact
`u32` source index, and the item value. Equal item values at different indexes
therefore have different argument digests and instance keys. Reordering changes
the source tuple and indexed child identities. The aggregate and children apply
the ordinary policy relation; no indexed String suffix exists.

Timeout derives a new producer instance from a canonical tuple containing the
source `RuntimeNeedHandle` and limit value, plus the timeout contract, stable
site, payload type, and Timeout family. Because the handle's canonical value
identity is its exact `NeedId`, timeout commits the source cell without parsing
or mutating it. Timeout publishes a separate `JoinSameKey` Need cell.

## 7. Generic Match and View admission

`CheckedMatch::try_from_hir` validates HIR structure, checked children, types,
patterns, Boolean guards, bounded usefulness, exhaustiveness, reachability, and
the generic Match semantic digest. It never performs retained-value ownership,
snapshot, resource, or Need-producer admission.

Only exact checked Boolean literals are constant guards:

- `Literal(Boolean(true))` is `ConstantTrue`;
- `Literal(Boolean(false))` is `ConstantFalse`;
- every other checked expression is `Dynamic`.

Source evaluation, source-string folding, and speculative constant evaluation
are forbidden. `ConstantFalse` owns `FalseGuard` precedence independently of
prior pattern coverage.

`CheckedViewMatchAdmission` consumes a `CheckedMatchRef`, exact retained outputs
and captures, `CheckedOwnershipContext`, and a separate
`CheckedNeedProducerAdmission`. Its failure blocks only the View catalog/product
row. The generic Match fact remains valid and usable by ordinary language
execution.

## 8. Current View identity

The only stable program owner is current `ViewProgramId`.
`AcceptedViewProgramRevision([u8; 32])` remains the semantic catalog revision
for bundle validation, registry publication, and replacement transactions.

`CheckedViewMatchCoordinate` is exactly:

```rust
pub struct CheckedViewMatchCoordinate {
    pub program: ViewProgramId,
    pub site: ViewMatchSiteId,
    pub admission: CheckedViewMatchAdmissionDigest,
}
```

`ViewMatchSiteId` is derived from `ViewProgramId`, the enclosing accepted
declaration identity, and a closed checked-expression child-role path. HIR IDs,
source spans, debug spelling, and accepted revision are excluded.

The View task-plan digest commits program, site, and admission, but not revision.
No `ViewProgramSemanticDigest` or canonical-u32 View revision is introduced.

## 9. Replacement

Bundle rows carry the accepted revision. An old/new explicit site mapping may
rebind live state only when all of the following agree:

1. generic `CheckedMatchSemanticDigest`;
2. `CheckedViewMatchAdmissionDigest`;
3. `CheckedNeedProducerAdmissionDigest`;
4. producer family and `NeedProducerContractDigest`;
5. payload type digest;
6. task-plan semantic digest;
7. exact consulted ownership evidence digest;
8. optional exact resource-dependency digest; and
9. canonical runtime argument digest.

The revisions may differ. Revision is not hashed into the producer instance or
translated into another NeedId.

At a quiescent replacement barrier, the transaction preserves the NeedId and
launch ordinal, changes the active `GenerationId`, and rederives `TaskKey` and
`TaskId` for the new generation. The host adapter must prepare the same rebind
transaction. On any mismatch or adapter refusal, the affected state is
cancelled according to the parent lifecycle; there is no fallback alias or
persistent translation table.

## 10. Ownership evidence

The current classifier context is exactly:

```rust
pub struct CheckedOwnershipContext<'a> {
    pub symbols: &'a ProjectSymbolTable,
    pub world: &'a RegisteredSemanticWorld,
}
```

`ResourceTypeRegistry` is not accepted because current `AgentResource` and
`AgentResourceBody` types carry no exact resource-type key. Both use their
current core Agent DTO snapshot owner and classify `SnapshotClone`.

Opaque evidence is mandatory end to end:

```text
AcceptedNominalInventoryInput
  { runtime_producer, value_class, persistence }
    -> registrar, with no defaults
    -> AcceptedNominalSemantics::Opaque
       { producer, value_class, persistence }
    -> AcceptedNominalCatalogDigest
    -> CheckedOwnershipCertificate / runtime-plan projection
```

The complete type matrix is normative in `OWNERSHIP_EVIDENCE.md`.
In particular: `Need<T>`, `Ref`, and admitted `Shared<T>` are
`SnapshotClone`; `ViewValue`, Stream, borrow/frame-local values, affine handles,
and type-level Function are rejected at retained View/producer admission, not
generic Match construction.

## 11. Compile-clean publication

The five cuts are fixed:

1. generic Match only;
2. opaque ownership evidence and total classifier;
3. View admission and all View product/runtime/replacement consumers;
4. private fixed identity preparation and core `GenerationId`;
5. one atomic public task/Need carrier, persistence, host, AWBC, Await,
   AwaitMany, timeout, journal, save/replay/replacement switch plus deletion of
   every old String/suffix/fallback path.

Cut 5 is indivisible. No public intermediate cut may publish a partial typed
schema, delayed persistence migration, dummy catalog, dual carrier, or
compatibility reader.

## 12. Final disposition

All mandatory alternatives are selected. `OPEN_QUESTIONS=0`.
This contract authorizes implementation against `17b384a36e1412cc7e7d9f13073d8dd33dcb5cbc` subject to the
compile/test/Clippy/documentation/parity gates in `TEST_MATRIX.md`. It does not
claim those production gates have already passed.
