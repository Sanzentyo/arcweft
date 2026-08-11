# Snapshot, save, and restore ownership contract

This file is normative for whole-execution snapshot/save/restore. Snapshot is a typed projection into dormant evidence. It is not `RuntimeValue::clone`, does not create a runnable second token/lease/handle, and does not add a parallel executable runtime-value model.

## 1. Terms

- **active owner**: one `RuntimeAffineOwnerToken` reachable from the runnable execution and reciprocally represented by its owning domain (currently the sole Stream table).
- **snapshot evidence**: serializable identity/layout/domain facts describing an owner; it has no method or field that can execute, drop, transfer, rotate a lease, or mutate a table.
- **snapshot image**: canonical schema-2 data for a whole execution at one global checkpoint.
- **restore candidate**: one validated image plus private activation plan; still non-runnable and non-Clone.
- **activation**: the private non-fallible final step that replaces an empty/frozen driver state with a complete execution and creates active tokens only after conflicting old owners are retired.

## 2. Whole-execution freeze state

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeExecutionRunState {
    Runnable,
    SnapshotFrozen { epoch: RuntimeExecutionEpoch },
    RestoreFrozen { epoch: RuntimeExecutionEpoch },
    Terminal,
}

pub struct RuntimeSnapshotGuard<'a> {
    execution: &'a mut RuntimeExecutionState,
    epoch: RuntimeExecutionEpoch,
    completed: bool,
}
```

`try_begin_snapshot(&mut self)` is accepted only at a global checkpoint and transitions `Runnable -> SnapshotFrozen`. While the guard exists, the scheduler/host event reducer/VM/structured engine cannot step the execution. The guard may build a snapshot image or cancel. Guard cleanup that merely restores run state is control-state RAII; it does not perform language value drop or owner mutation.

```rust
impl RuntimeExecutionState {
    pub fn try_begin_snapshot(
        &mut self,
    ) -> Result<RuntimeSnapshotGuard<'_>, RuntimeSnapshotBeginError>;
}

impl RuntimeSnapshotGuard<'_> {
    pub fn try_build_image(
        &self,
        limits: &RuntimeSnapshotLimits,
    ) -> Result<RuntimeSnapshotImageV2, RuntimeSnapshotError>;

    pub fn resume(self);
}
```

The original is frozen only while traversal observes it. After `resume`, the image remains dormant evidence and the original is runnable again. Any number of evidence copies can exist without adding runnable owners.

## 3. Snapshot image owner

The outer save version remains the accepted parent value:

```rust
pub const BUNDLE_SESSION_SAVE_SCHEMA_VERSION: u32 = 2;
```

The value snapshot owner is:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeValueSnapshotV2 {
    pub ownership: RuntimeValueOwnership,
    pub value: RuntimeValueSnapshotKindV2,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeAffineOwnerEvidenceSnapshotV2 {
    StreamConsumer {
        instance: StreamInstanceKey,
        lease: StreamConsumerLease,
        item_layout: TypeLayoutHash,
        error_layout: TypeLayoutHash,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeAffineOwnerSnapshotV2 {
    pub owner: RuntimeAffineOwnerId,
    pub evidence: RuntimeAffineOwnerEvidenceSnapshotV2,
}
```

A live handle projects to one `RuntimeAffineOwnerSnapshotV2`. The image contains no `RuntimeAffineOwnerToken`, provider object, open socket/task, Rust reference, execution borrow, mutable table pointer, or callable that can activate the evidence.

Every snapshot/value enum is closed, strict, and recursively bounded. Unknown fields/tags and duplicate map keys are errors. Canonical order is retained for all maps/sets/vectors whose semantic order is canonical.

## 4. Snapshot traversal

The whole execution traversal is deterministic and complete. It visits in this order:

1. durable/root/runtime-driver roots;
2. structured engine environments in stable fiber/scope/frame order;
3. each closure capture in `RuntimeCaptureSlot` order;
4. each external partial's canonical argument product coordinate order, including positional/named rest members;
5. tuple/record/variant/sequence children in canonical value-path order;
6. iterator remaining values in next-delivery order;
7. AWBC registers, frames, suspended application slots, and cleanup slots;
8. mailboxes, child-fiber transfer packets, join results, and scheduler-owned value slots;
9. compiled-region exchange candidates, if any accepted safe-point representation remains;
10. every live Stream handle and the sole Stream instance table/tombstone/replay state.

Traversal records a `RuntimeAffineOwnerId -> RuntimeValuePath` occurrence map and a Stream `(StreamInstanceKey, StreamConsumerLease)` occurrence map. A second occurrence is an error even when bytes/layout are equal.

The table is not treated as a second handle occurrence. It is reciprocal domain evidence. Exact invariants are:

```text
one live handle token occurrence
<-> one matching live/tombstone consumer owner row as allowed by parent lifecycle
<-> same StreamInstanceKey
<-> same current StreamConsumerLease
<-> same item/error layouts
```

The parent table's producer authority remains separate and follows its accepted producer lease/reference rules.

## 5. Ownership and eligibility projection

For each `RuntimeValue`:

1. recompute recursive ownership;
2. compare any private cache;
3. check snapshot eligibility of every variant;
4. project unrestricted data directly into strict snapshot values;
5. project an affine leaf only through its typed domain evidence;
6. store computed ownership in the snapshot row.

A closure/partial/aggregate is snapshotable exactly when every nested value is snapshotable. A partial additionally requires its exact generation artifact and accepted external signature/definition identity. An active group-application transaction is not snapshotable. A Stream handle/table state follows parent safe-point/lifecycle rules.

General payload eligibility is irrelevant to save: schema 2 is the explicit owning boundary for handle/partial evidence. Conversely, save eligibility does not make the value a general payload or replay value.

## 6. Generation pin traversal

The required-generation set is recomputed from the complete candidate graph. It includes:

- every `RuntimeExternalStreamPartialFunction.generation`;
- every `StreamHandle.key().generation`;
- every corresponding live/tombstone Stream table entry and producer child/frame reference;
- every suspended group application/canonical product that names a generation;
- every mailbox/transfer/cleanup/iterator/aggregate/closure path containing one of those values;
- every parent-required generator/derived Stream artifact already defined by .1/.2/.2.1.

A nested partial contributes its own generation even when it is inside another closure/record/sequence/rest/iterator. A handle contributes its instance generation and table relation. The encoded `required_generations` vector must be strictly sorted/unique and exactly equal to the recomputed set. Missing pins and extra pins are both tampering; there is no “at least” acceptance or rebinding to current generation.

## 7. Candidate construction

```rust
pub struct RuntimeRestoreCandidate {
    image: RuntimeSnapshotImageV2,
    activation: RuntimeActivationPlan,
}

// Private snapshot-specific typestate; not an executable RuntimeValue model.
pub(crate) struct RuntimeActivationPlan {
    allocations: RuntimeActivationAllocationPlan,
    owner_sites: Box<[RuntimeAffineOwnerActivationSite]>,
    stream_relations: Box<[RuntimeStreamActivationRelation]>,
    final_digest: RuntimeExecutionSnapshotDigest,
}
```

`RuntimeActivationPlan` stores validated offsets/indices/preallocated capacities and owner activation sites into the existing snapshot tree. It does not duplicate runtime semantic variants and is never consumed by evaluator/VM/host. It exists only so final installation has no validation/allocation error branch. This is not a parallel runtime value model.

Candidate construction runs with no live-state mutation and no token activation. It may allocate ordinary memory, validate every value/table/frame relation, and reserve capacity for the final execution. It cannot call Stream Open, providers, host adapters, default/argument expressions, replay injection, scheduler progression, or lease rotation.

## 8. Restore entrypoints and exclusivity

```rust
impl RuntimeDriver {
    pub fn try_prepare_restore(
        &self,
        bytes: &[u8],
        artifacts: &RuntimeArtifactSet,
        limits: &RuntimeRestoreLimits,
    ) -> Result<RuntimeRestoreCandidate, RuntimeRestoreError>;

    pub fn try_restore_empty(
        &mut self,
        candidate: RuntimeRestoreCandidate,
    ) -> Result<(), RuntimeRestoreCommitError>;

    pub fn try_restore_replace(
        &mut self,
        candidate: RuntimeRestoreCandidate,
    ) -> Result<(), RuntimeRestoreCommitError>;
}
```

`try_restore_empty` requires no installed execution. `try_restore_replace` obtains exclusive mutable driver access, freezes the current execution at a global checkpoint, and rechecks the epoch/artifact/host identities used during preparation. There is no API that installs the candidate beside a runnable execution or merges two runtime graphs.

The final commit sequence is:

1. ensure target is empty or still the exact frozen epoch;
2. transfer old active owners into a private retirement batch;
3. validate retirement batch is non-fallible under the already held domain/table transaction;
4. retire/revoke old owners and detach old execution from the driver;
5. instantiate existing `RuntimeValue`/frame/fiber owners from the prevalidated snapshot plan;
6. activate each affine token exactly once with its preserved `RuntimeAffineOwnerId` and reciprocal Stream row;
7. install the complete execution and required-generation pins;
8. publish one driver revision/restore observation;
9. explicitly drop retired terminal memory/state.

Steps 4–8 are one non-fallible swap. New active tokens do not exist before old conflicting active tokens are retired. A process may retain snapshot bytes or a consumed candidate's former bytes, but those are evidence only.

## 9. Exact tamper rejection order

Restore rejects the first error in this order:

1. outer envelope framing, checksum, declared schema, canonical length/trailing bytes;
2. artifact/content identity, project/bundle identity, ABI 2, codec 8, host ABI, bundle schema 6, save schema 2;
3. global and nested byte/count/depth/work limits, integer/length overflow, unknown tags/fields, noncanonical map/set/order;
4. runtime type/layout/schema validity and recursive ownership recomputation/cache equality;
5. value boundary eligibility for snapshot/save (not general payload eligibility);
6. duplicate `RuntimeAffineOwnerId` occurrences, then duplicate Stream `(key, lease)` handle occurrences;
7. local/environment/capture/iterator/aggregate structural validity;
8. AWBC register/frame/instruction-cursor/cleanup/safe-point ownership facts;
9. mailbox/child/scope/join/transfer/compiled-exchange ownership facts;
10. sole Stream table/tombstone/replay/cursor/lifecycle/accounting validity;
11. handle-token-table key/lease/layout reciprocity and no owner orphan;
12. explicit rejection that no affine-only value appears through `RuntimePayload`/general canonical data fields;
13. exact required-generation recomputation and artifact availability;
14. candidate allocation-plan construction and canonical digest recheck;
15. at commit, active driver freeze/epoch/artifact recheck before old-state retirement.

Within one class, traversal/path/canonical table order determines the first error. The decoder does not continue to produce an aggregate error set that could vary by hash-map order.

## 10. Failed restore cleanup

Before the final non-fallible swap, every error drops only ordinary candidate/snapshot buffers and staged unrestricted values. No active token or table row was created, so no language owner release is necessary. The installed session remains byte/value/owner/table/request/queue/revision equivalent and resumes if it was temporarily frozen for a commit recheck.

If a stale epoch/artifact mismatch is found at step 15, the candidate is destroyed and the old execution is unfreezed unchanged. There is no partial facade assignment, table replacement, generation-pin mutation, or observation.

Unexpected internal impossibility after old-owner retirement is a process-level invariant failure, not a recoverable restore branch. The implementation must arrange validation/capacity so ordinary malformed input cannot reach that point; tests inject every fallible condition before retirement.

## 11. Snapshot copy operation

The only copy operation is ordinary data duplication of `RuntimeSnapshotImageV2` or its canonical bytes. It is distinct from `try_duplicate_unrestricted`:

| Property | Runtime value duplication | Snapshot image copy |
|---|---|---|
| input | runnable `&RuntimeValue` | dormant data/evidence |
| affine value | rejected | evidence may be copied |
| output | another runnable value | non-runnable bytes/DTO |
| token/lease authority | would duplicate, therefore forbidden | absent; IDs only |
| provider/table operation | none | none |
| installation | immediately usable value | private exclusive restore required |

Two copied images can each be prepared, but only one candidate at a time can acquire exclusive driver replacement and activate. Reusing the same image later replaces/restarts according to restore policy; it does not coexist with the prior active execution.

## 12. Save blockers and safe-point rules

Retain parent blockers and the .2 blockers, interpreted through the final owner:

```rust
ExternalStreamGroupApplicationActive { count: usize }
UnsnapshotableExternalStreamPartialCaptures { count: usize }
MissingExternalStreamPartialGeneration { generations: Vec<StreamGeneration> }
```

Add generic blockers where the existing owner enum needs them:

```rust
OwnershipTransactionActive { count: usize }
EscapingRuntimeBorrow { count: usize }
AffineOwnerReciprocityInvalid { count: usize }
UnsnapshotableRuntimeValue { count: usize }
```

A global checkpoint has no active ownership transaction/borrow/compiled exchange and complete child/table closure. A committed partial is saveable only when all nested values are snapshot-safe and its exact generation is pinned. An affine partial is not itself an externally open Stream; parent open-instance blockers are based on the table, not the partial classification.

## 13. Stream-specific evidence

For a handle snapshot, exact checked fields are:

```text
owner ID
StreamInstanceKey(definition key, generation, ordinal)
StreamConsumerLease
item TypeLayoutHash
error TypeLayoutHash
matching table entry lifecycle/consumer state
matching policy/table generation/artifact identity
```

The generic token is not a second Stream table. Restore first validates the evidence against the sole table snapshot, then activates one token into the handle while activating the corresponding table row in the same swap.

A token/lease cannot be repaired by choosing the table's value, rotating a lease, renumbering ordinal, or dropping a duplicate. Every mismatch is a hard typed error.

## 14. Partial/function snapshots

Retain the accepted schema-2 function variants and canonical product, with final generic ownership rules:

```rust
pub enum RuntimeFunctionValueSnapshotV2 {
    Closure(RuntimeClosureValueSnapshotV2),
    ExternalStreamPartial(RuntimeExternalStreamPartialSnapshotV2),
}

pub struct RuntimeExternalStreamPartialSnapshotV2 {
    pub definition: RuntimeStreamDefinitionKey,
    pub declaration: RuntimeCallableDeclarationDigest,
    pub generation: StreamGeneration,
    pub signature: RuntimeExternalStreamSignatureFingerprint,
    pub next_group: RuntimeCallableGroupIndex,
    pub ownership: RuntimeValueOwnership,
    pub arguments: RuntimeExternalStreamArgumentProductSnapshotV2,
}
```

Restore recomputes `ownership` from every captured cell/rest member and rejects a mismatch. Closure snapshots retain exact capture-plan identity and capture-slot order. They contain values/evidence, not source text or a whole environment snapshot.

## 15. Canonical save/persistence restrictions

- Save schema 2 is the sole persistent owner for executable function/partial/handle state.
- Generic `RuntimePayload`, bundle metadata values, replay records, host JSON, and canonical data codecs do not embed `RuntimeValueSnapshotV2` as an opaque escape.
- Snapshot values carry no source expression/range/name/provider resume object unless already required by an accepted typed parent field.
- No schema-1 migration, dual reader, compatibility alias, or best-effort owner repair is added.
- Bundle/save hashes bind the exact accepted parent fingerprints/identities and this ownership shape.

## 16. Hot reload

Hot reload uses the same candidate/exclusive-swap model. A live handle/partial remains bound to its exact generation. Reload cannot mutate its definition/generation in place or translate the captured product. The driver either retains required old generations under the parent policy or rejects reload/restore before mutation. Ownership IDs and Stream key/lease relations survive an accepted state-preserving reload exactly; a new execution creates new owners through normal Open.

## 17. Required direct invariants

The snapshot/save/restore tests must prove:

- image copying never changes active token/table state;
- original can resume after image construction;
- no candidate can execute/read provider/advance scheduler;
- only empty/replace entrypoints exist;
- duplicate owner ID, handle occurrence, lease, key, table owner, orphan, stale lease, layout mismatch, pin mismatch, and cache mismatch reject in the specified order;
- failure at every validation/allocation/recheck point leaves the old execution identical;
- final replace has no observable interval with two active owners or no installed complete execution;
- nested closure/partial/rest/iterator/register/mailbox/child/cleanup paths all contribute owner occurrences and generation pins;
- snapshot/restore performs no authored/default evaluation, Stream Open, host dispatch, replay injection, provider work, scheduler step, or lease rotation.
