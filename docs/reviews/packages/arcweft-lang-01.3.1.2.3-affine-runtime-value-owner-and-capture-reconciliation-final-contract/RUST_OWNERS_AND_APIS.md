# Exact Rust-shaped owners and APIs

This file fixes names, owning modules, visibility, traits, and error-preserving ownership. `FINAL_CONTRACT.md` remains normative when prose and a snippet are read together. Every Arcweft-owned enum or struct named here is changed in its original owner and receives inherent behavior there. The snippets do not authorize an extension trait, Stream-only value model, copied registry, second environment, or renamed sequence wrapper.

## 1. Owner map

| Existing or final owner | Responsibility |
|---|---|
| `arcweft_core::value::ownership` | generic ownership class, canonical paths, checked duplication, opaque affine leaf token, slot/drop/transfer primitives |
| `arcweft_core::value::RuntimeValue` | sole executable value graph and inherent traversal/conversion/equality behavior |
| `arcweft_core::value::RuntimeBinding` / `RuntimeEnv` (`value.rs` plus existing `value/env.rs`) | local slots and exact typed closure-capture transaction; `bindings_snapshot()` is deleted |
| `arcweft_core::value::RuntimeSeq` (`value.rs` plus existing sequence implementation modules) | sequence ownership, consuming materialization, repeat/get/slice/push/take behavior; **no `RuntimeSequenceValue` type exists** |
| `arcweft_core::value::RuntimeIterator` (existing `value/range.rs`) | consuming sequence/range/witness iteration; no clone-backed cursor |
| `arcweft_core::value::RuntimePayload` | sole closed, cloneable, non-runnable host/replay payload algebra |
| `arcweft_core::pattern::RuntimePattern` | existing typed pattern enum; literal constants become IDs and matching/binding uses one typed plan |
| `arcweft_core::plan` | immutable checked constant table, `RuntimeExpr::Constant`, pattern literal IDs, and plan-owned transfer facts |
| `arcweft_core::awbc` | one ownership-aware register/frame state machine and codec-8 `CopyValue` |
| `arcweft_core::stream` | sole Stream table, lease authority, private affine-token mint/drop/restore hooks, accepted handle/partial/product owners |
| `arcweft_runtime_driver` | whole-execution snapshot/save/restore exclusivity and atomic swap |
| `arcweft_runtime_plan` | projection of accepted HIR/sema ownership, capture, pattern, and operand facts; no runtime token authority |

Physical responsibility files may be split under the existing modules to satisfy structure audit, but the public owner names above do not change and no forwarding compatibility type is added.

## 2. Generic ownership class, evidence ID, and path

```rust
// Owner: arcweft-core::value::ownership

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeValueOwnership {
    Unrestricted,
    Affine,
}

impl RuntimeValueOwnership {
    pub const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unrestricted, Self::Unrestricted) => Self::Unrestricted,
            _ => Self::Affine,
        }
    }

    pub const fn permits_copy(self) -> bool {
        matches!(self, Self::Unrestricted)
    }
}

#[repr(transparent)]
#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct RuntimeAffineOwnerOrdinal(u64);

impl RuntimeAffineOwnerOrdinal {
    pub const fn get(self) -> u64;
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct RuntimeAffineOwnerId {
    pub execution: ExecutionInstanceId,
    pub ordinal: RuntimeAffineOwnerOrdinal,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeValuePath(Box<[RuntimeValuePathSegment]>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeValuePathSegment {
    Tuple(u32),
    Record(RuntimeRecordFieldId),
    Sequence(u32),
    VariantPayload,
    ClosureCapture(RuntimeCaptureSlot),
    ExternalStreamArgument(RuntimeCallableParameterCoordinate),
    RestPositional(u32),
    RestNamed(RuntimeParameterName),
    IteratorRemaining(u32),
    Environment(RuntimeLocalSlotId),
    Register(AwbcRegisterId),
    FrameLocal(u32),
    Mailbox(u32),
    ChildTransfer(u32),
    Cleanup(u32),
}
```

`RuntimeValueOwnership`, IDs, and paths are evidence, not authority. Paths are constructed inside the owner so they are canonical: tuple/sequence indices ascend, records use declaration/authored order, captures use capture ordinal, external arguments use accepted coordinate order, and keyed rest entries use canonical key order.

## 3. Opaque affine leaf authority and minting

```rust
// Owner: arcweft-core::value::ownership
// No Clone/Copy/Serde/PartialEq/Hash/Ord and no public constructor.
pub(crate) struct RuntimeAffineOwnerToken {
    id: RuntimeAffineOwnerId,
}

impl RuntimeAffineOwnerToken {
    pub(crate) fn id(&self) -> RuntimeAffineOwnerId;
}

pub(crate) struct RuntimeAffineOwnerAllocator {
    execution: ExecutionInstanceId,
    next: RuntimeAffineOwnerOrdinal,
}

pub(crate) struct RuntimeAffineOwnerMint<'a> {
    allocator: &'a mut RuntimeAffineOwnerAllocator,
    id: RuntimeAffineOwnerId,
}

impl RuntimeAffineOwnerAllocator {
    pub(crate) fn try_begin_mint(
        &mut self,
    ) -> Result<RuntimeAffineOwnerMint<'_>, RuntimeAffineOwnerAllocationError>;
}

impl RuntimeAffineOwnerMint<'_> {
    pub(crate) fn id(&self) -> RuntimeAffineOwnerId;

    // Non-fallible: ordinal exhaustion was checked by try_begin_mint and the
    // exclusive mutable borrow prevents a competing mint.
    pub(crate) fn commit(self) -> RuntimeAffineOwnerToken;
}
```

Beginning a mint does not increment the allocator or publish authority. Dropping an uncommitted `RuntimeAffineOwnerMint` leaves the allocator byte-identical. All other validation, allocation, table/request reservation, and destination checks happen before `try_begin_mint`; the final Stream Open commit immediately commits the mint together with the lease/table/handle/request batch. This is the only fresh-token production caller in Lang-01.3.

A move needs no token-state rotation: the sole token object moves with the owning `RuntimeValue`. Prepared transfer records carry source-slot revisions and a canonical owner-ID set while the execution transaction has exclusive mutation authority. Thus there is no second `ReservedForTransfer` authority state and no transaction-ID mismatch surface.

Restore does not call the fresh allocator. `RuntimeActivationPlan` owns one crate-private restore authority that validates dormant evidence and creates one token with the recorded ID only during the atomic activation step described in `SNAPSHOT_SAVE_RESTORE_CONTRACT.md`. Copying or deserializing an ID cannot reach that authority.

The token has a manually redacted `Debug` implementation. Its Rust destructor may only assert/log an already-terminal internal invariant; language release is the explicit table-aware drop transaction.

## 4. `StreamHandle` correction

The accepted parent type remains in `arcweft-core::stream` and gains the private generic token. Invariant-bearing fields are read-only outside the owner.

```rust
#[derive(Debug)]
pub struct StreamHandle {
    key: StreamInstanceKey,
    item_layout: TypeLayoutHash,
    error_layout: TypeLayoutHash,
    lease: StreamConsumerLease,
    owner: RuntimeAffineOwnerToken,
}

impl StreamHandle {
    pub fn key(&self) -> StreamInstanceKey;
    pub fn item_layout(&self) -> TypeLayoutHash;
    pub fn error_layout(&self) -> TypeLayoutHash;
    pub fn lease(&self) -> StreamConsumerLease;

    pub(crate) fn owner_id(&self) -> RuntimeAffineOwnerId;

    pub(crate) fn try_prepare_drop(
        &self,
        table: &StreamInstanceTable,
    ) -> Result<PreparedStreamConsumerDrop, StreamConsumerDropError>;
}
```

`StreamHandle` implements neither `Clone`, `Copy`, `Serialize`, `Deserialize`, `PartialEq`, `Eq`, `Hash`, nor `Ord`. Typed identity comparison uses `key()` only where identity comparison is semantically intended; it is not language value equality. Save uses `StreamHandleSnapshotV2`, never Serde on the live handle.

## 5. Existing executable value, binding, and environment owners

The existing `RuntimeValue` enum is extended in place and remains the sole executable value representation. It does not implement `Clone`, `Copy`, Serde, or blanket `PartialEq`/`Eq`/`Hash`/`Ord` after G3.

```rust
impl RuntimeValue {
    pub fn ownership(&self) -> RuntimeValueOwnership;

    pub fn try_duplicate_unrestricted(
        &self,
    ) -> Result<Self, RuntimeDuplicateError>;

    pub fn payload_eligibility(
        &self,
    ) -> Result<(), RuntimePayloadError>;

    pub fn try_into_payload(
        self,
    ) -> Result<RuntimePayload, RuntimeOwnedPayloadError>;

    pub fn plan_constant_eligibility(
        &self,
    ) -> Result<(), RuntimePlanConstantError>;

    pub fn snapshot_eligibility(
        &self,
        context: &RuntimeSnapshotEligibilityContext<'_>,
    ) -> Result<(), RuntimeSnapshotEligibilityError>;

    pub fn try_borrowed_eq(
        &self,
        other: &Self,
        evidence: &RuntimeEqualityEvidence,
    ) -> Result<bool, RuntimeEqualityError>;
}

#[derive(Debug)]
pub struct RuntimeBinding {
    name: String,
    value: RuntimeValueSlot,
    value_type: RuntimeTypeId,
    mutable: bool,
}

impl RuntimeBinding {
    pub fn name(&self) -> &str;
    pub fn value_type(&self) -> RuntimeTypeId;
    pub fn is_mutable(&self) -> bool;
    pub fn slot(&self) -> &RuntimeValueSlot;
    pub(crate) fn slot_mut(&mut self) -> &mut RuntimeValueSlot;
}
```

`RuntimeBinding`, `RuntimeScope`, and `RuntimeEnv` do not implement `Clone` or Serde. The current `bindings_snapshot()` API is deleted rather than retained as a checked-looking fallback.

```rust
impl RuntimeEnv {
    pub fn borrow_local(
        &self,
        slot: RuntimeLocalSlotId,
    ) -> Result<RuntimeValueRef<'_>, RuntimeSlotError>;

    pub fn try_copy_local(
        &self,
        slot: RuntimeLocalSlotId,
    ) -> Result<RuntimeValue, RuntimeTransferError>;

    pub fn try_move_local(
        &mut self,
        slot: RuntimeLocalSlotId,
        transaction: &mut RuntimeOwnershipTransaction<'_>,
    ) -> Result<RuntimeValue, RuntimeTransferError>;

    pub fn try_install_local(
        &mut self,
        spec: &RuntimeLocalInstallSpec,
        value: RuntimeValue,
    ) -> Result<(), RuntimeOwnedLocalInstallError>;

    pub fn try_capture_closure(
        &mut self,
        plan: &RuntimeCapturePlan,
        function: RuntimeFunctionBody,
        transaction: &mut RuntimeOwnershipTransaction<'_>,
    ) -> Result<RuntimeClosureValue, RuntimeCaptureError>;
}
```

`RuntimeValueRef<'a>` is an opaque borrow tied to the environment/register/value borrow. It cannot be stored in `RuntimeValue`, captured, serialized, sent to a child, or cross a suspension/safe point.

`ownership()` recursively joins child ownership. `ExternalStreamPartial` recomputes its captured product rather than trusting its private cache. `try_duplicate_unrestricted()` performs a complete deterministic preflight before allocating the result; it creates no partial duplicate on error.

```rust
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeDuplicateError {
    #[error("runtime value contains an affine owner")]
    AffineLeaf {
        path: RuntimeValuePath,
        owner: RuntimeAffineOwnerId,
        kind: RuntimeAffineOwnerKind,
    },
    #[error("runtime ownership cache disagrees with the value graph")]
    OwnershipInvariant {
        path: RuntimeValuePath,
        cached: RuntimeValueOwnership,
        computed: RuntimeValueOwnership,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAffineOwnerKind {
    StreamConsumer,
}
```

The errors carry evidence only and may be cloned. On error the source value, Stream table, lease, allocator, and execution revision are unchanged.

## 6. Exact closed `RuntimePayload`

The current `RuntimePayload(pub RuntimeValue)` wrapper cannot remain because its derived `Clone` would require executable `RuntimeValue: Clone`. It is replaced in the **same owner and same public name** by a closed non-runnable algebra. It deliberately mirrors the existing Serde variant/field spelling for every retained safe value, so accepted payload bytes do not acquire an endpoint DTO or adapter-local representation.

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimePayload {
    Unit,
    Bool(bool),
    Int(RuntimeInt),
    UInt(RuntimeUInt),
    F32(f32),
    F64(f64),
    MatrixF32(DenseMatrixF32),
    MatrixF64(DenseMatrixF64),
    TensorF32(DenseTensorF32),
    TensorF64(DenseTensorF64),
    String(String),
    Char(char),
    Duration(LogicalDuration),
    Range(RuntimeRange),
    EntityRef(String),
    Tuple(Vec<RuntimePayload>),
    Seq(RuntimePayloadSeq),
    Record(Vec<RuntimePayloadFieldValue>),
    NominalRecord(RuntimePayloadNominalRecordValue),
    Variant {
        owner: RuntimeVariantIdentity,
        ordinal: u32,
        name: String,
        payload: Option<Box<RuntimePayload>>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimePayloadSeq {
    Values(Vec<RuntimePayload>),
    Dense(DenseSeq),
    TupleColumns(RuntimePayloadTupleSeq),
    RecordColumns(RuntimePayloadRecordSeq),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimePayloadTupleSeq {
    len: usize,
    columns: Vec<RuntimePayloadSeq>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimePayloadRecordSeq {
    len: usize,
    fields: Vec<RuntimePayloadRecordSeqField>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimePayloadRecordSeqField {
    name: String,
    values: RuntimePayloadSeq,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimePayloadFieldValue {
    pub name: String,
    pub value: RuntimePayload,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimePayloadNominalRecordValue {
    type_id: RuntimeNominalTypeId,
    layout: TypeLayoutHash,
    fields: Vec<RuntimePayload>,
}

impl RuntimePayload {
    pub fn into_runtime_value(self) -> RuntimeValue;
    pub fn canonical_label(&self) -> String;
}
```

`DenseSeq` and its scalar backing stores may remain cloneable because their closed variants contain only unrestricted scalar/data values. `RuntimeSeq`, `TupleSeq`, `RecordSeq`, and `RecordSeqField` do not remain cloneable because they can recursively own affine values.

Conversion is two-phase. `RuntimeValue::payload_eligibility()` borrows and checks the complete graph, canonical type/layout, nesting, count, and byte limits. Only after that succeeds does a private infallible consuming conversion construct `RuntimePayload`. Therefore an eligibility error returns the original executable value intact.

```rust
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimePayloadError {
    #[error("runtime value is not eligible for the general payload boundary")]
    IneligibleKind {
        path: RuntimeValuePath,
        kind: RuntimePayloadIneligibleKind,
    },
    #[error("runtime payload candidate contains an affine owner")]
    AffineOwner {
        path: RuntimeValuePath,
        owner: RuntimeAffineOwnerId,
    },
    #[error("runtime payload exceeds a declared limit")]
    LimitExceeded(RuntimePayloadLimit),
    #[error("runtime payload type/layout evidence is invalid")]
    TypeOrLayout { path: RuntimeValuePath },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePayloadIneligibleKind {
    Function,
    StreamHandle,
    Iterator,
    Reference,
    Continuation,
    RuntimeTable,
    OpaqueRuntimeValue,
}

#[derive(Debug)]
pub struct RuntimeOwnedPayloadError {
    error: RuntimePayloadError,
    value: RuntimeValue,
}

impl RuntimeOwnedPayloadError {
    pub fn error(&self) -> &RuntimePayloadError;
    pub fn into_parts(self) -> (RuntimePayloadError, RuntimeValue);
}
```

There is no `RuntimePayload::Opaque(RuntimeValue)`, `From<RuntimeValue>`, unchecked public constructor, or generic escape hatch. Adapters consume the shared core type and do not define endpoint DTOs.

## 7. Value slots, transfer, and table-aware drop

```rust
// Owner: arcweft-core::value::ownership; embedded by env/register/frame owners.

#[derive(Debug, Default)]
pub enum RuntimeValueSlot {
    #[default]
    Empty,
    Live(RuntimeValue),
    Moved(RuntimeMovedValueEvidence),
    Dropped(RuntimeDroppedValueEvidence),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeTransferMode {
    Copy,
    Move,
}

impl RuntimeValueSlot {
    pub fn live(&self) -> Result<&RuntimeValue, RuntimeSlotError>;

    pub(crate) fn try_prepare_copy(
        &self,
    ) -> Result<RuntimePreparedCopy, RuntimeTransferError>;

    pub(crate) fn try_prepare_move(
        &self,
        transaction: RuntimeOwnershipTransactionId,
    ) -> Result<RuntimePreparedMove, RuntimeTransferError>;

    pub(crate) fn install_copy(
        &mut self,
        prepared: RuntimePreparedCopy,
    ) -> Result<(), RuntimeTransferCommitError>;

    pub(crate) fn take_prepared_move(
        &mut self,
        prepared: RuntimePreparedMove,
    ) -> Result<RuntimeValue, RuntimeTransferCommitError>;
}
```

A prepared copy owns a fully staged unrestricted value. A prepared move owns only the transaction ID, source-slot revision, expected type/ownership, and canonical owner-ID set. It does not clone, borrow beyond the transaction, or mutate a token. The transaction preallocates destination storage, proves destination emptiness, and records/rechecks every source revision and canonical owner set before any take. Commit follows typed-plan source order; after the first take, all remaining steps are infallible under the exclusive execution transaction.

```rust
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeTransferError {
    #[error("source slot is not live")]
    SourceNotLive { slot: RuntimeOwnedSlotId },
    #[error("destination slot is not empty")]
    DestinationNotEmpty { slot: RuntimeOwnedSlotId },
    #[error("copy requires an unrestricted source")]
    CopyOfAffine { slot: RuntimeOwnedSlotId, path: RuntimeValuePath },
    #[error("the same affine owner occurs more than once in a transfer")]
    DuplicateOwner { owner: RuntimeAffineOwnerId },
    #[error("source value type differs from typed transfer evidence")]
    TypeMismatch { slot: RuntimeOwnedSlotId },
    #[error("source slot changed after preparation")]
    StaleRevision { slot: RuntimeOwnedSlotId },
    #[error("transfer budget exceeded")]
    LimitExceeded(RuntimeTransferLimit),
}
```

`RuntimeOwnedSlotId` is a diagnostic enum over an environment local, closure capture, AWBC register/frame slot, mailbox lane, transfer-packet slot, or cleanup slot. It is not storage.

Language drop is explicit:

```rust
pub(crate) struct RuntimePreparedDrop {
    owners_descending: Box<[PreparedAffineLeafDrop]>,
    pure_paths_reverse: Box<[RuntimeValuePath]>,
}

impl RuntimeValue {
    pub(crate) fn try_prepare_drop(
        &self,
        domains: &RuntimeOwnershipDomainView<'_>,
    ) -> Result<RuntimePreparedDrop, RuntimeDropError>;
}

impl RuntimePreparedDrop {
    pub(crate) fn commit(
        self,
        value: RuntimeValue,
        domains: &mut RuntimeOwnershipDomains<'_>,
    );
}
```

Preparation validates every owner occurrence, lease/table relation, generation, and domain capacity without mutation. Observable releases commit by descending `RuntimeAffineOwnerId`, then pure aggregate memory is consumed in reverse structural path. Duplicate IDs, stale leases, or table mismatch reject with the value still owned. Unwind lowering constructs the same prepared cleanup plan; Rust `Drop` is never the language operation.

## 8. Exact capture and pattern plans

HIR retains the accepted capture identity `(closure_expr_id, outer_local_id)` and first-use ordinal. Sema/runtime-plan add ownership intent to that owner; there is no copied capture registry.

```rust
// Immutable RuntimePlan data.

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeCapturePlanId(pub u32);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeCaptureSlot(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeCaptureMode {
    Copy,
    Move,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCaptureSpec {
    pub capture: CaptureId,
    pub source_local: LocalId,
    pub source_slot: RuntimeLocalSlotId,
    pub destination: RuntimeCaptureSlot,
    pub value_type: RuntimeTypeId,
    pub mode: RuntimeCaptureMode,
    pub mutable_capture_slot: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCapturePlan {
    pub id: RuntimeCapturePlanId,
    pub closure: ExprId,
    pub captures: Box<[RuntimeCaptureSpec]>,
}
```

Membership is exactly the accepted HIR free-local set; order is first source use; parameters are excluded; nearest visible local wins. `Affine` sources are `Move`; `Unrestricted` sources are `Copy`. `CaptureAccess::Reassign` makes the closure-owned slot mutable but never aliases or writes the outer slot. Borrow capture is functionalized or rejected before RuntimePlan.

`RuntimeEnv::try_capture_closure` stages all copies, prepares every move, preallocates capture storage, then atomically takes moves and publishes the closure. Any `Err` disposes staged unrestricted copies and leaves source slots and environment revision unchanged.

```rust
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeCaptureError {
    #[error("capture plan does not match the closure")]
    PlanIdentity,
    #[error("capture source is missing or not live")]
    SourceNotLive { capture: CaptureId, source: LocalId },
    #[error("capture source type does not match typed evidence")]
    TypeMismatch { capture: CaptureId },
    #[error("capture source appears more than once")]
    DuplicateSource { source: LocalId },
    #[error("capture destination is duplicate or noncanonical")]
    DuplicateOrOutOfOrderDestination { destination: RuntimeCaptureSlot },
    #[error("copy capture contains an affine owner")]
    CopyOfAffine { capture: CaptureId, path: RuntimeValuePath },
    #[error("capture ownership transaction is invalid")]
    Ownership(RuntimeTransferError),
    #[error("capture limit exceeded")]
    LimitExceeded(RuntimeCaptureLimit),
}
```

The current plan-owned `RuntimePattern` is also changed in place so cloneable plans contain no live runtime value:

```rust
pub enum RuntimePattern {
    // existing structural variants remain
    Literal(RuntimeConstantId), // replaces Literal(RuntimeValue)
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePatternBindingMode {
    Copy,
    Move,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePatternBindingSpec {
    pub source_path: RuntimeValuePath,
    pub destination: RuntimeLocalSlotId,
    pub value_type: RuntimeTypeId,
    pub mode: RuntimePatternBindingMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePatternBindingPlan {
    pub pattern_digest: RuntimePatternDigest,
    pub bindings: Box<[RuntimePatternBindingSpec]>,
}

impl RuntimePattern {
    pub(crate) fn try_matches_borrowed(
        &self,
        value: &RuntimeValue,
        constants: &RuntimeConstantTable,
        evidence: &RuntimePatternTypeEvidence,
    ) -> Result<bool, RuntimePatternMatchError>;
}

impl RuntimePatternBindingPlan {
    pub(crate) fn try_bind_owned(
        &self,
        pattern: &RuntimePattern,
        source: &mut RuntimeValueSlot,
        destinations: &mut RuntimeEnv,
        transaction: &mut RuntimeOwnershipTransaction<'_>,
    ) -> Result<(), RuntimePatternBindingError>;
}
```

`IfLet`, each match arm, `let` destructure, parameters, and loop patterns carry their binding plan directly at the existing owning expression/op; there is no global side table. Matching first borrows the tag/shape/literal constant. Only the selected plan prepares copies/moves. Overlapping whole/subpattern bindings require unrestricted copies; no affine leaf may occur in two destinations. Owned rest moves remaining members; borrowed rest is Copy and therefore unrestricted. Preparation failure leaves scrutinee and destinations unchanged.

## 9. Sole function-value enum, closure partial application, and owned failures

The parent type is changed in place:

```rust
#[derive(Debug)]
pub enum RuntimeFunctionValue {
    Closure(RuntimeClosureValue),
    ExternalStreamPartial(RuntimeExternalStreamPartialFunction),
}

#[derive(Debug)]
pub struct RuntimeClosureValue {
    params: Box<[RuntimeBindingSpec]>,
    body: RuntimeFunctionBody,
    captures: Box<[RuntimeBinding]>,
    capture_plan: RuntimeCapturePlanId,
}

#[derive(Debug)]
pub struct RuntimeExternalStreamPartialFunction {
    definition: RuntimeStreamDefinitionKey,
    declaration: RuntimeCallableDeclarationDigest,
    generation: StreamGeneration,
    signature: RuntimeExternalStreamSignatureFingerprint,
    next_group: RuntimeCallableGroupIndex,
    captured: RuntimeExternalStreamArgumentProduct,
    ownership: RuntimeValueOwnership,
}
```

`RuntimeFunctionBody` is immutable plan data (`Expr` contains only plan IDs after §12, or `AwbcFunctionId`) and may remain `Clone`/Serde. `RuntimeClosureValue`, captures, and `RuntimeFunctionValue` are executable owners and may not.

```rust
pub enum RuntimeClosureApplication {
    Partial(RuntimeClosureValue),
    Invoke(RuntimePreparedClosureCall),
}

impl RuntimeFunctionValue {
    pub fn ownership(&self) -> RuntimeValueOwnership;

    pub fn try_apply_closure_arguments(
        self,
        plan: &RuntimeClosureApplicationPlan,
        evaluated: RuntimeEvaluatedClosureArguments,
    ) -> Result<RuntimeClosureApplication, RuntimeOwnedClosureApplicationFailure>;

    pub fn try_apply_external_stream_group(
        self,
        definition: &RuntimeStreamDefinition,
        plan: &RuntimeExternalStreamGroupApplicationPlan,
        evaluated: RuntimeExternalStreamEvaluatedGroup,
        runtime: &mut RuntimeStreamOpenTransaction<'_>,
    ) -> Result<RuntimeExternalStreamApplication, RuntimeOwnedFunctionApplicationFailure>;
}
```

Ordinary partial application consumes the callee and evaluated argument owners. It moves arguments into the corresponding parameter-capture slots and returns a closure with the remaining parameters; it does not clone existing captures, arguments, or body-owned runtime values. A full application returns a prepared invocation environment. External non-final application consumes the old partial and publishes one new partial; final application atomically publishes the Open result.

```rust
#[derive(Debug)]
pub struct RuntimeOwnedClosureApplicationFailure {
    error: RuntimeFunctionApplicationError,
    callee: RuntimeFunctionValue,
    evaluated: RuntimeEvaluatedClosureArguments,
}

#[derive(Debug)]
pub struct RuntimeOwnedFunctionApplicationFailure {
    error: RuntimeFunctionApplicationError,
    callee: RuntimeFunctionValue,
    evaluated: RuntimeExternalStreamEvaluatedGroup,
}

impl RuntimeOwnedClosureApplicationFailure {
    pub fn error(&self) -> &RuntimeFunctionApplicationError;
    pub fn into_parts(
        self,
    ) -> (
        RuntimeFunctionApplicationError,
        RuntimeFunctionValue,
        RuntimeEvaluatedClosureArguments,
    );
}

impl RuntimeOwnedFunctionApplicationFailure {
    pub fn error(&self) -> &RuntimeFunctionApplicationError;
    pub fn into_parts(
        self,
    ) -> (
        RuntimeFunctionApplicationError,
        RuntimeFunctionValue,
        RuntimeExternalStreamEvaluatedGroup,
    );
}
```

Metadata/type/generation/group/coordinate/payload/limit checks happen before mutable table access. Already evaluated language effects are not rolled back. On preparation failure all executable owners are returned. After a transaction enters its non-fallible commit, no `Err` branch remains.

## 10. Existing `RuntimeSeq` and `RuntimeIterator` APIs

No `RuntimeSequenceValue` wrapper is introduced. The existing enum and column carriers are modified directly:

```rust
#[derive(Debug)]
pub enum RuntimeSeq {
    Values(Vec<RuntimeValue>),
    Dense(DenseSeq),
    TupleColumns(TupleSeq),
    RecordColumns(RecordSeq),
}

#[derive(Debug)]
pub struct TupleSeq {
    len: usize,
    columns: Vec<RuntimeSeq>,
}

#[derive(Debug)]
pub struct RecordSeq {
    len: usize,
    fields: Vec<RecordSeqField>,
}

#[derive(Debug)]
pub struct RecordSeqField {
    name: String,
    values: RuntimeSeq,
}

#[derive(Debug)]
pub enum RuntimeIterator {
    Values(std::vec::IntoIter<RuntimeValue>),
    Range(RuntimeRangeIterator),
    Witness {
        state: Box<RuntimeValue>,
        next: RuntimeTraitMethodId,
    },
}
```

`RuntimeSeq`, `TupleSeq`, `RecordSeq`, `RecordSeqField`, and `RuntimeIterator` implement no `Clone`, Serde, or blanket `PartialEq`. `DenseSeq` may retain data-only traits as stated in §6.

```rust
impl RuntimeSeq {
    pub fn len(&self) -> usize;
    pub fn ownership(&self) -> RuntimeValueOwnership;

    pub fn try_get_copy(
        &self,
        index: RuntimeSequenceIndex,
    ) -> Result<RuntimeValue, RuntimeSequenceAccessError>;

    pub fn try_slice_copy(
        &self,
        range: RuntimeSequenceSlice,
    ) -> Result<RuntimeSeq, RuntimeSequenceAccessError>;

    pub fn into_runtime_iterator(self) -> RuntimeIterator;

    pub fn try_push_owned(
        self,
        value: RuntimeValue,
    ) -> Result<Self, RuntimeOwnedSequencePushError>;

    pub fn try_repeat_owned(
        value: RuntimeValue,
        count: RuntimeSequenceCount,
        permission: RuntimeRepeatPermission,
        domains: &mut RuntimeOwnershipDomains<'_>,
    ) -> Result<Self, RuntimeOwnedRepeatError>;

    pub(crate) fn try_take_owned(
        self,
        index: RuntimeSequenceIndex,
        domains: &mut RuntimeOwnershipDomains<'_>,
    ) -> Result<RuntimeValue, RuntimeOwnedSequenceTakeError>;

    // Existing method changed in place: every logical cell is moved exactly once.
    pub(crate) fn into_values(self) -> Vec<RuntimeValue>;
}

impl Iterator for RuntimeIterator {
    type Item = RuntimeValue;
    fn next(&mut self) -> Option<RuntimeValue>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeRepeatPermission {
    ExactZero,
    ExactOne,
    Unrestricted,
}
```

Exact storage rules:

- `ownership()` joins `Values` recursively; `Dense` is always unrestricted; `TupleColumns` joins columns in ordinal order; `RecordColumns` joins fields in stored declaration order.
- `into_values(self)` consumes storage. `Dense` moves scalar/data cells; tuple/record columns are transposed into logical rows by consuming each column cell once. Constructor/restore invariants guarantee equal column lengths, so no failure occurs after consumption.
- `into_runtime_iterator()` is `RuntimeIterator::Values(self.into_values().into_iter())`; no indexed clone remains. Witness state is itself owned and moved by its accepted next protocol.
- `try_get_copy` and `try_slice_copy` first require the **entire sequence** to be unrestricted, then validate range/bounds, then duplicate. An affine empty slice is still rejected. Slice preserves the current storage class: `Values` duplicates selected cells, `Dense` slices closed storage, and columnar variants slice every column.
- `try_push_owned` consumes both inputs. It preflights shape and all needed capacities. Matching dense/columnar shapes extend in place; otherwise it consumes the old storage through `into_values`, appends the new value, and publishes `RuntimeSeq::Values`. No owner is copied. Error returns both owners.
- `try_take_owned` validates bounds and prepares cleanup of every non-selected logical cell before consuming storage. Commit moves the selected cell and drops the remainder. Error returns the original sequence.
- repeat 0 prepares/commits drop and returns empty; repeat 1 moves; repeat ≥2 requires unrestricted, stages `n-1` copies, then places the original in the final slot.

```rust
#[derive(Debug)]
pub struct RuntimeOwnedSequencePushError {
    error: RuntimeSequenceMutationError,
    sequence: RuntimeSeq,
    value: RuntimeValue,
}

impl RuntimeOwnedSequencePushError {
    pub fn error(&self) -> &RuntimeSequenceMutationError;
    pub fn into_parts(self) -> (RuntimeSequenceMutationError, RuntimeSeq, RuntimeValue);
}

#[derive(Debug)]
pub struct RuntimeOwnedRepeatError {
    error: RuntimeRepeatError,
    value: RuntimeValue,
}

#[derive(Debug)]
pub struct RuntimeOwnedSequenceTakeError {
    error: RuntimeSequenceAccessError,
    sequence: RuntimeSeq,
}
```

## 11. Typed borrowed equality

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEqualityEvidence {
    pub type_id: RuntimeTypeId,
    pub layout: TypeLayoutHash,
    pub schema: RuntimeEqualitySchema,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeEqualityError {
    #[error("equality evidence does not match the runtime value types")]
    EvidenceMismatch,
    #[error("runtime type is not equality-comparable")]
    NotComparable {
        path: RuntimeValuePath,
        kind: RuntimeEqualityIneligibleKind,
    },
    #[error("runtime value violates its declared type layout")]
    RuntimeTypeMismatch { path: RuntimeValuePath },
}
```

Equality borrows and changes no slot/token/table state. Function values, Stream handles, iterators, references, continuations, and other non-Eq leaves are absent from `RuntimeEqualitySchema`. Affinity by itself neither grants nor denies equality; type evidence does.

## 12. RuntimePlan constants, expression literals, and pattern literals

The plan holds immutable non-runnable constant data, not `RuntimeValue`. One wrapper reuses the sole closed payload algebra; it is not another executable value model.

```rust
// Owner: arcweft-core::plan

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeConstantId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePlanConstant(RuntimePayload);

#[derive(Debug)]
struct RuntimeConstantValue {
    constant: RuntimePlanConstant,
    layout: TypeLayoutHash,
    digest: RuntimeConstantDigest,
}

#[derive(Debug)]
pub struct RuntimeConstantTable {
    values: Box<[RuntimeConstantValue]>,
    digest: RuntimeConstantTableDigest,
}

#[derive(Debug, Default)]
pub struct RuntimeConstantTableBuilder {
    values: Vec<RuntimeConstantValue>,
}

impl RuntimeConstantTableBuilder {
    pub fn try_push(
        &mut self,
        value: RuntimeValue,
        layout: TypeLayoutHash,
    ) -> Result<RuntimeConstantId, RuntimeOwnedPlanConstantError>;

    pub fn finish(self) -> RuntimeConstantTable;
}

impl RuntimeConstantTable {
    pub fn get(
        &self,
        id: RuntimeConstantId,
    ) -> Result<&RuntimePlanConstant, RuntimeConstantIdError>;

    pub fn instantiate(
        &self,
        id: RuntimeConstantId,
    ) -> Result<RuntimeValue, RuntimeConstantInstantiationError>;

    pub fn digest(&self) -> RuntimeConstantTableDigest;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeExpr {
    // all existing non-literal variants remain, with their pattern binding plans
    Constant(RuntimeConstantId),
}
```

`RuntimeExpr::Value(RuntimeValue)` is deleted. The existing `RuntimePattern::Literal(RuntimeValue)` is simultaneously replaced by `RuntimePattern::Literal(RuntimeConstantId)`. Pattern comparison borrows the constant through `get`; expression evaluation instantiates by cloning the closed `RuntimePlanConstant` payload and consuming it into a fresh `RuntimeValue`. No live value is cloned.

`RuntimeConstantTableBuilder::try_push` first checks recursive `Unrestricted`, then closed plan-constant eligibility/type/layout/budgets, then consumes the value through the same two-phase safe-data conversion used by `RuntimePayload`. Functions, partials, handles, iterators, references, continuations, active runtime IDs, and runtime tables are rejected even if currently unrestricted. An error returns the original `RuntimeValue`.

`RuntimeConstantTable` itself implements neither `Clone` nor general Serde. `RuntimePlan` also implements neither `Clone` nor direct Serde. The accepted bundle decoder constructs one checked `RuntimePlan` containing one owned `RuntimeConstantTable`, wraps the plan in `Arc<RuntimePlan>`, and all engines/AOT/JIT caches share that plan `Arc`; they do not clone a plan or a constant table. `RuntimePlan::constants()` returns `&RuntimeConstantTable`. Cache entries contain the plan `Arc`, IDs, digests, and immutable compiled artifacts, never a live value, env, frame, iterator, token, partial, or handle.

### 12.1 Existing `RuntimeFlow` / `FlowOp` become a purely immutable program

The current `FlowOp` mixes static program operations with live execution continuations (`Bind`, `LoopNext`, `WhileNext`, `WhileLetNext`, and `ForNext`). This is corrected **on the original plan and engine owners**, not by adding a second op enum. Runtime-plan normalizes each flow into one block arena; body-bearing operations reference block IDs and no plan node owns a `RuntimeBinding` or `RuntimeIterator`.

```rust
// Owner: arcweft-core::plan; direct replacement of the existing RuntimeFlow/FlowOp shape.

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeFlowBlockId(u32);

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeFlowBlock {
    ops: Box<[FlowOp]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeFlow {
    pub id: FlowRuntimeId,
    entry: RuntimeFlowBlockId,
    blocks: Box<[RuntimeFlowBlock]>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FlowCursor {
    pub flow_index: u32,
    pub block: RuntimeFlowBlockId,
    pub op_index: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeMatchArm {
    pub pattern: RuntimePattern,
    pub binding_plan: RuntimePatternBindingPlan,
    pub guard: Option<RuntimeExpr>,
    pub block: RuntimeFlowBlockId,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FlowOp {
    // Existing non-control variants remain, but every pattern-bearing variant
    // carries its adjacent RuntimePatternBindingPlan.
    Let {
        pattern: RuntimePattern,
        binding_plan: RuntimePatternBindingPlan,
        expr: RuntimeExpr,
    },
    LetElse {
        pattern: RuntimePattern,
        binding_plan: RuntimePatternBindingPlan,
        expr: RuntimeExpr,
        else_block: RuntimeFlowBlockId,
    },
    Await {
        binding: Option<RuntimePattern>,
        binding_plan: Option<RuntimePatternBindingPlan>,
        target: AwaitTarget,
        pending: Box<[LineEffectRequest]>,
    },
    AwaitMany {
        binding: Option<RuntimePattern>,
        binding_plan: Option<RuntimePatternBindingPlan>,
        target: AwaitManyTarget,
        pending: Box<[LineEffectRequest]>,
    },
    HostCall {
        binding: Option<RuntimePattern>,
        binding_plan: Option<RuntimePatternBindingPlan>,
        target: RuntimeHostCallTarget,
    },
    If {
        condition: RuntimeExpr,
        then_block: RuntimeFlowBlockId,
        else_block: RuntimeFlowBlockId,
    },
    IfLet {
        pattern: RuntimePattern,
        binding_plan: RuntimePatternBindingPlan,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        then_block: RuntimeFlowBlockId,
        else_block: RuntimeFlowBlockId,
    },
    Match {
        scrutinee: RuntimeExpr,
        arms: Box<[RuntimeMatchArm]>,
    },
    Loop {
        body: RuntimeFlowBlockId,
    },
    LetLoop {
        result_pattern: RuntimePattern,
        result_binding_plan: RuntimePatternBindingPlan,
        body: RuntimeFlowBlockId,
    },
    While {
        condition: RuntimeExpr,
        body: RuntimeFlowBlockId,
    },
    WhileLet {
        pattern: RuntimePattern,
        binding_plan: RuntimePatternBindingPlan,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: RuntimeFlowBlockId,
    },
    For {
        pattern: RuntimePattern,
        binding_plan: RuntimePatternBindingPlan,
        source: RuntimeExpr,
        evidence: RuntimeIteratorEvidence,
        body: RuntimeFlowBlockId,
    },
    Thread {
        name: Option<String>,
        capture_plan: RuntimeCapturePlanId,
        body: RuntimeFlowBlockId,
    },
    Scope {
        body: RuntimeFlowBlockId,
    },
    LetScope {
        pattern: RuntimePattern,
        binding_plan: RuntimePatternBindingPlan,
        body: RuntimeFlowBlockId,
        value: RuntimeExpr,
    },
    ExitScopeBind {
        pattern: RuntimePattern,
        binding_plan: RuntimePatternBindingPlan,
        expr: RuntimeExpr,
    },
    // All other existing immutable variants remain.
}
```

The following original variants are deleted, not deprecated or decoded:

```text
FlowOp::Bind(Vec<RuntimeBinding>)
FlowOp::LoopNext { .. }
FlowOp::WhileNext { .. }
FlowOp::WhileLetNext { .. }
FlowOp::ForNext { iterator: RuntimeIterator, .. }
```

`RuntimeFlowBlockId` is local to one `RuntimeFlow`. Blocks are stored in ascending ID order; every referenced ID is in range; the entry block is present; and the block graph, expression IDs, pattern digests, capture plans, and binding plans are validated before publication. The bundle codec writes the normalized graph directly and has no reader for recursive-body/runtime-continuation predecessor shapes.

### 12.2 Live loop/iterator/binding state remains on the existing engine owners

The current `FlowFiber.pending_ops: VecDeque<FlowOp>` is deleted. A fiber executes only borrowed operations from its shared `Arc<RuntimePlan>` through `FlowCursor`. Dynamic continuation state is added to the original `FlowControlStackEntryKind`; no parallel flow-op model or copied body exists.

```rust
// Owner: arcweft-core::engine; direct replacement of existing fields/variants.

#[derive(Debug)]
pub struct Engine {
    plan: Arc<RuntimePlan>,
    // existing live fields; no Clone/Serde
}

#[derive(Debug)]
pub struct FlowFiber {
    pub cursor: Option<FlowCursor>,
    pub control_stack: Vec<FlowControlStackEntry>,
    pub env: RuntimeEnv,
    // existing live fields; pending_ops is absent; no Clone/Serde
}

#[derive(Debug)]
pub struct FlowControlStackEntry {
    pub kind: FlowControlStackEntryKind,
}

#[derive(Debug)]
pub enum FlowControlStackEntryKind {
    Scope {
        resume: Option<FlowCursor>,
        cleanups: Vec<FlowScopeCleanup>,
    },
    Loop {
        owner: FlowCursor,
        resume: Option<FlowCursor>,
    },
    While {
        owner: FlowCursor,
        resume: Option<FlowCursor>,
    },
    WhileLet {
        owner: FlowCursor,
        resume: Option<FlowCursor>,
    },
    For {
        owner: FlowCursor,
        resume: Option<FlowCursor>,
        iterator: RuntimeIterator,
    },
}

impl Engine {
    pub fn new(plan: Arc<RuntimePlan>) -> Self;
}

#[derive(Debug)]
pub struct AwaitState {
    owner: FlowCursor,
    resume: Option<FlowCursor>,
}

#[derive(Debug)]
pub struct AwaitManyState {
    owner: FlowCursor,
    resume: Option<FlowCursor>,
    items: Vec<RuntimeValue>,
    next_index: usize,
    in_flight: Vec<AwaitManyInFlight>,
    results: Vec<Option<RuntimePayload>>,
}

#[derive(Debug)]
pub struct HostCallState {
    owner: FlowCursor,
    id: RuntimeHostCallId,
    resume: Option<FlowCursor>,
}

#[derive(Debug)]
pub enum FlowFiberStatus {
    // existing status variants use their non-Clone state owners
    JoiningChildren { owner: FlowCursor },
}
```

For optional bindings, `binding.is_some() == binding_plan.is_some()` is a checked plan invariant. Suspended await/host states retain the owner cursor rather than cloned pattern/target plan data; completion resolves that immutable op, validates its identity, and applies the adjacent binding plan exactly once. `JoiningChildren` re-dispatches the immutable return op by cursor after all children terminate, replacing the current practice of pushing a cloned `Return`/`ReturnExpr` op.

`owner` always resolves to the corresponding immutable `Loop`/`LetLoop`/`While`/`WhileLet`/`For` op in the same plan and flow. On branch/match/while-let success, the engine opens the destination scope and commits the adjacent binding plan directly into `RuntimeEnv`; there is no `FlowOp::Bind`. On `For`, the source value is consumed into one `RuntimeIterator`, that iterator moves into the `For` control frame, and each iteration calls the existing iterator owner's consuming `next` operation exactly once. Body completion consults the frame and immutable owner op to continue or resume; it never manufactures a `*Next` op or clones a body. `continue` unwinds scopes to the same frame; `break` consumes/drops the frame-owned iterator and applies any `LetLoop` result plan transactionally. Thread children share the plan `Arc` and receive only the exact capture-plan transfer, never a cloned body or environment.

Suspension and child-join statuses retain only owned live state plus `FlowCursor` resume coordinates. `Engine`, `FlowFiber`, `FlowFiberStatus`, control frames, await/host-call/child-join states, and compiled-region exchanges implement no `Clone`. A failed plan-coordinate lookup is a checked runtime integrity error before any value transfer.

`RuntimeFunctionBody::Expr(Box<RuntimeExpr>)` and `RuntimePattern` may remain cloneable immutable leaf plan data because every literal is now an ID. The whole `RuntimePlan`, engine, and execution frames do not clone.

## 13. Snapshot evidence owners

```rust
// Owner: arcweft-core value snapshot and arcweft-runtime-driver save owner.

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeAffineOwnerSnapshotV2 {
    pub owner: RuntimeAffineOwnerId,
    pub evidence: RuntimeAffineOwnerEvidenceSnapshotV2,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeValueSnapshotV2 {
    pub ownership: RuntimeValueOwnership,
    pub value: RuntimeValueSnapshotKindV2,
}
```

`RuntimeValueSnapshotKindV2` is the existing save-value DTO changed in place. Every non-affine variant remains data-only. A live `StreamHandle` is represented only by `StreamHandleSnapshotV2` containing the owner evidence above; it contains no token and cannot execute. Function snapshots use the sole `Closure | ExternalStreamPartial` tags and recursively snapshot captures/product cells.

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSnapshotImageV2 {
    pub execution: ExecutionInstanceId,
    pub epoch: RuntimeExecutionEpoch,
    pub values: RuntimeExecutionSnapshotV2,
    pub required_generations: Vec<StreamGeneration>,
}

#[derive(Debug)]
pub struct RuntimeRestoreCandidate {
    image: RuntimeSnapshotImageV2,
    activation: RuntimeActivationPlan,
}
```

`RuntimeSnapshotImageV2` may be cloned because it is dormant evidence. `RuntimeRestoreCandidate`, `RuntimeActivationPlan`, activation arenas, prepared replacement sessions, and any live execution carrier implement no `Clone`/`Copy` or Serde. No candidate contains a runnable token. Activation performs the fixed tamper checks, reserves all tables/arenas, retires the replaced frozen execution, and only then creates and installs exactly one token per evidence row.

## 14. Trait surface summary

| Type family | `Clone`/`Copy` | Serde | equality traits |
|---|---|---|---|
| `RuntimeValue`, `RuntimeBinding`, `RuntimeSeq`, tuple/record columns, executable functions/aggregates | no | no general codec | no blanket language authority |
| `StreamHandle`, affine token | no | no | no |
| `RuntimeIterator`, env/register/frame/fiber/execution/transfer/restore candidate | no | only explicit dormant DTO | no blanket value equality |
| `DenseSeq` and closed scalar backing stores | safe data traits allowed | only through actual closed data boundary | structural traits already valid |
| `RuntimePayload` closed data algebra | `Clone`; `Copy` only existing scalar newtypes | strict existing-compatible payload shape | `PartialEq` as valid for floats/data |
| typed IDs, paths, immutable leaf plan records/evidence | yes as individually listed | only at an actual wire boundary | yes as listed |
| `RuntimePlan` owner | no `Clone`; shared as `Arc<RuntimePlan>` | bundle-owned strict codec only | digest/ID comparison |
| `RuntimeFlow` / `FlowOp` / block arena | `Clone` is safe immutable plan data, but execution only borrows through the plan `Arc` | bundle/plan codec only | structural plan equality |
| `RuntimePattern` / `RuntimeExpr` after literal-ID migration | `Clone` leaf plan data | bundle/plan codec only | structural plan equality |
| `RuntimeConstantTable` | no `Clone`; owned exactly once inside `RuntimePlan` | bundle-owned canonical codec only | digest/ID comparison, not live-value equality |
| snapshot/save DTOs | `Clone` allowed | strict schema 2 | structural validation equality |

No public API can manufacture, clone, serialize, compare as a generic value, or install a live affine authority.
