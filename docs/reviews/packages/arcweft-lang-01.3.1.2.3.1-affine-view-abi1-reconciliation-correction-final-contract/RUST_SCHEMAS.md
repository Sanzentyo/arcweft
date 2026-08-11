# Exact corrected Rust-shaped schemas

This file is normative for corrected names, fields, visibility, and trait surfaces. Parent schemas not named here remain unchanged.

## ABI and operand facts

```rust
pub const AWBC_ABI_VERSION: u32 = 1;
pub const AWBC_CODEC_VERSION: u32 = 8;

pub enum AwbcInstruction {
    // existing variants
    CopyValue {
        dst: AwbcRegisterId,
        src: AwbcRegisterId,
    },
}
```

`CopyValue` has opcode `0x2a`. No `AWBC_ABI_VERSION_V2`, `Abi2`, compatibility enum, or alternate decoder exists.

## Activation authority

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeExecutionHolderId(u64);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeExecutionActivationGeneration(u64);

pub struct RuntimeExecutionDomain {
    activation: RuntimeExecutionActivationAuthority,
}

pub(crate) struct RuntimeExecutionActivationAuthority {
    active: BTreeMap<ExecutionInstanceId, RuntimeExecutionActivationEntry>,
    next_holder: RuntimeExecutionHolderId,
}

pub struct RuntimeExecutionActivationLease {
    execution: ExecutionInstanceId,
    holder: RuntimeExecutionHolderId,
    generation: RuntimeExecutionActivationGeneration,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeExecutionActivationError {
    #[error("execution is already active in this runtime domain")]
    ExecutionAlreadyActive { execution: ExecutionInstanceId },
    #[error("the driver does not own the active execution holder")]
    HolderMismatch { execution: ExecutionInstanceId },
    #[error("the target driver is not empty")]
    DriverNotEmpty,
    #[error("runtime execution activation limit exceeded")]
    LimitExceeded,
    #[error("runtime execution holder allocator is exhausted")]
    HolderExhausted,
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

The domain constructor is owned by the existing runtime host/session context; lower consumers cannot mint a second authority for one host. `RuntimeExecutionHolderId` and activation generation are domain-local evidence and are not serialized.

The lease implements neither `Clone`, `Copy`, Serde, equality, hash, nor ordering. It has no public constructor.

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeValueSnapshotV2 {
    pub ownership: RuntimeValueOwnership,
    pub value: RuntimeValueSnapshotKindV2,
}
```

Neither type derives `Eq` when its transitive value graph may contain floats.

## Prepared drop

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

## View transfer

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewValueTransferMode {
    Copy,
    Move,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewValueInputBinding {
    pub register: u16,
    pub source: ViewValueInputSource,
    pub value_type: RuntimeCheckedType,
    pub ownership: RuntimeValueOwnership,
    pub transfer: ViewValueTransferMode,
}
```

`ownership` is recomputed from the exact runtime type/layout table during product decode.

## Static requirement

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ViewStaticRequirementDigest(BundleDigest);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewStaticRequirementResource {
    pub subject: ViewStaticSubjectResource,
    pub requirement: ViewStaticRequirementDigest,
    pub attribute_source: Option<SourceRangeRef>,
}
```

`ViewStaticCertificateResource` retains `proof_origin` and adds:

```rust
pub requirement: Option<ViewStaticRequirementDigest>,
```

Validation requires `None` for `Automatic` and `Some(exact)` for `AuthoredRequired`.
