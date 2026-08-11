# Drop typestate and snapshot schema correction

## Exact source-bound prepared drop

Preparation is moved to the existing ownership transaction so the prepared object owns the exact source value and reservation.

```rust
pub(crate) struct RuntimePreparedDrop {
    source: RuntimeOwnedSlotId,
    source_revision: RuntimeValueSlotRevision,
    value: RuntimeValue,
    owners_descending: Box<[PreparedAffineLeafDrop]>,
    pure_paths_reverse: Box<[RuntimeValuePath]>,
}

impl RuntimeOwnershipTransaction<'_> {
    pub(crate) fn try_prepare_drop(
        &mut self,
        source: RuntimeOwnedSlotId,
    ) -> Result<RuntimePreparedDrop, RuntimeDropError>;

    pub(crate) fn commit_drop(
        &mut self,
        prepared: RuntimePreparedDrop,
    );
}
```

The transaction resolves `RuntimeOwnedSlotId` to the exact slot, validates the complete value/domain graph without mutation, reserves that slot revision, then removes the exact value into `RuntimePreparedDrop`. No API accepts a caller-supplied value at commit. A transaction abandoned before commit restores the reserved source value and revision without observable release. After successful preparation under the exclusive transaction, `commit_drop` is non-fallible.

The old APIs are deleted:

```rust
RuntimeValue::try_prepare_drop(&self, ...)
RuntimePreparedDrop::commit(self, value: RuntimeValue, ...)
```

## Snapshot DTO equality

The exact declaration is:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeValueSnapshotV2 {
    pub ownership: RuntimeValueOwnership,
    pub value: RuntimeValueSnapshotKindV2,
}
```

It does not implement `Eq`. Float-bearing snapshot variants retain exact codec representation; canonical byte re-encode and semantic digest are the strict validation authorities. Evidence types without floats may derive `Eq` independently.

## Whole-execution image

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSnapshotImageV2 {
    pub execution: ExecutionInstanceId,
    pub epoch: RuntimeExecutionEpoch,
    pub allocator: RuntimeAffineOwnerAllocatorSnapshotV2,
    pub values: RuntimeExecutionSnapshotV2,
    pub required_generations: Vec<StreamGeneration>,
}
```

Candidate bytes/DTO remain cloneable dormant evidence. `RuntimeRestoreCandidate` remains non-Clone and cannot step or expose runnable values.
