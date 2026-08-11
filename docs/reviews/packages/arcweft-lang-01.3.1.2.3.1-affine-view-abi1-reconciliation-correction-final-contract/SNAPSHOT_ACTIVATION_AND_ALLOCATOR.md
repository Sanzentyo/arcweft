# Snapshot activation and affine allocator correction

## Runtime execution domain

`RuntimeExecutionDomain` is the runtime-driver Sans-I/O owner for one activation universe. It contains one `RuntimeExecutionActivationAuthority`. Every driver with an installed execution owns a non-Clone/non-Serde lease created by that authority. Direct execution installation is crate-private.

```rust
pub struct RuntimeExecutionDomain {
    activation: RuntimeExecutionActivationAuthority,
}

pub struct RuntimeExecutionActivationAuthority {
    active: BTreeMap<ExecutionInstanceId, RuntimeExecutionActivationEntry>,
    next_holder: RuntimeExecutionHolderOrdinal,
}

pub struct RuntimeExecutionActivationLease {
    execution: ExecutionInstanceId,
    holder: RuntimeExecutionHolderId,
    generation: RuntimeExecutionActivationGeneration,
}

impl RuntimeExecutionDomain {
    pub fn try_activate_fresh(
        &mut self,
        driver: &mut RuntimeDriver,
        execution: RuntimeFreshExecution,
    ) -> Result<(), RuntimeExecutionActivationError>;

    pub fn try_restore_empty(
        &mut self,
        driver: &mut RuntimeDriver,
        candidate: RuntimeRestoreCandidate,
    ) -> Result<(), RuntimeRestoreCommitError>;

    pub fn try_restore_replace(
        &mut self,
        driver: &mut RuntimeDriver,
        candidate: RuntimeRestoreCandidate,
    ) -> Result<(), RuntimeRestoreCommitError>;
}
```

`RuntimeDriver::try_restore_empty` and `RuntimeDriver::try_restore_replace` are deleted. A driver exposes its current lease only to the domain owner through crate-private access.

## Activation rules

- Empty restore succeeds only when the driver is empty and the domain has no active entry for the candidate execution.
- Replacement succeeds only when the target driver owns its exact current entry/lease. The candidate may name the same execution or a different inactive execution. A different candidate execution is rejected only when another driver already owns it.
- A copied image prepared for a second driver in the same domain fails `ExecutionAlreadyActive`.
- A stale or foreign current-holder lease fails `ActivationHolderMismatch` before old-state retirement.
- Preparation and decoding create no active entry, token, Stream row, or holder.
- Replacement retirement and new activation are one non-fallible domain transaction after all checks and reservations.
- Dropping a candidate or failed reservation does not alter the active table.

Independent domains/processes are separate execution universes. The standard host owns exactly one domain. External distributed coordination, when required, is an adapter policy and cannot mint core owner tokens.

## Allocator snapshot

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeAffineOwnerCursorSnapshotV2 {
    Next { ordinal: RuntimeAffineOwnerOrdinal },
    Exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeAffineOwnerAllocatorSnapshotV2 {
    pub execution: ExecutionInstanceId,
    pub cursor: RuntimeAffineOwnerCursorSnapshotV2,
}
```

`RuntimeExecutionSnapshotV2` contains exactly one allocator snapshot. `Next(n)` means `n` has never been issued and is the next mint candidate. `Exhausted` means no further owner can be minted. Mint commit advances `n` to `n+1`, or to `Exhausted` when `n == u64::MAX`; begin-mint does not advance.

Validation order:

1. allocator execution equals snapshot execution;
2. exactly one allocator row;
3. every recorded live/tombstone/cleanup/table owner uses the same execution;
4. with `Next(n)`, every recorded ordinal is `< n`;
5. with `Exhausted`, no mint is permitted;
6. cursor participates in the execution snapshot digest and canonical save bytes;
7. install the exact cursor before execution publication;
8. first post-restore mint proves its ID equals the persisted cursor and advances once.

The cursor is not recomputed from live owners because owners already issued in the snapshot history must not be reused. Replacing a newer active execution with an older snapshot installs the older snapshot cursor exactly after the newer execution is fully retired; deterministic replay may therefore reproduce post-snapshot owner IDs, but no two active occurrences coexist.
