# Exact Rust owners and APIs

This document is normative. A declaration headed **exact target declaration**
is complete for the symbol shown. Existing current/parent owners listed under
“imported owner” are consumed directly and are not redefined.

## 1. Existing imported owners

The correction imports these owners without aliases:

```rust
use arcweft_core::awbc::schema::AwbcRegisterId;
use arcweft_core::pattern::RuntimeCheckedType;
use arcweft_core::value::{RuntimeValue, RuntimeValueOwnership};
use arcweft_runtime_driver::BundleSession;
```

It also consumes the parent-owned closed `RuntimeExecutionSnapshotV2` and
checked unrestricted duplication API. The parent snapshot payload enum is not
duplicated here; this correction changes only its floating representation and
trait row as specified in §15.

## 2. Scalar runtime IDs

Owner: existing `arcweft_core::runtime_id`.

All fields are private. The wrappers implement exactly the listed traits. Every
`NonZeroU64` identity implements:

```text
Clone, Copy, Debug, Eq, Hash, PartialEq
Display, Ord, PartialOrd
Serialize, Deserialize (manual strict codec)
```

Every `NonZeroU32` identity implements the same traits. None implements
`Default`, `From<u64/u32>`, `TryFrom<u64/u32>`, `FromStr`, arithmetic operator
traits, random generation, or a public raw constructor.

### 2.1 Execution and cursors

**Exact target declarations:**

```rust
#[repr(transparent)]
pub struct ExecutionInstanceId(NonZeroU64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeIdCursor {
    Next(NonZeroU64),
    Exhausted,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeIdNamespace {
    Execution,
    ExecutionReservation,
    Occurrence,
    LocalSlot,
    OwnershipTransaction,
    AffineOwner,
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
#[error("{namespace:?} runtime identity space is exhausted")]
pub struct RuntimeIdExhausted {
    namespace: RuntimeIdNamespace,
}
```

**Exact inherent API:**

```rust
impl ExecutionInstanceId {
    #[must_use]
    pub const fn get(self) -> NonZeroU64;

    pub(crate) const fn from_allocated(raw: NonZeroU64) -> Self;
}

impl RuntimeIdCursor {
    #[must_use]
    pub const fn initial() -> Self;

    #[must_use]
    pub const fn next(self) -> Option<NonZeroU64>;

    #[must_use]
    pub const fn last_issued(self) -> Option<NonZeroU64>;

    pub(crate) fn take_next(
        &mut self,
        namespace: RuntimeIdNamespace,
    ) -> Result<NonZeroU64, RuntimeIdExhausted>;
}

impl RuntimeIdExhausted {
    #[must_use]
    pub const fn namespace(self) -> RuntimeIdNamespace;
}
```

`RuntimeIdCursor::initial()` is `Next(NonZeroU64::MIN)`. `take_next` returns the
current value and advances. When the current value is `u64::MAX`, it returns
that value and stores `Exhausted`. `last_issued()` is `None` for `Next(1)`,
`Some(n - 1)` for `Next(n)` where `n > 1`, and `Some(u64::MAX)` for
`Exhausted`. An exhausted cursor therefore remains the complete high-water
authority even after the storage occurrence that consumed `u64::MAX` has been
retired; it is never reconstructed from currently live IDs.

### 2.2 Dynamic occurrence identities

**Exact target declarations:**

```rust
#[repr(transparent)]
pub struct RuntimeScopeInstanceId(NonZeroU64);

#[repr(transparent)]
pub struct RuntimeClosureInstanceId(NonZeroU64);

#[repr(transparent)]
pub struct RuntimeFiberInstanceId(NonZeroU64);

#[repr(transparent)]
pub struct RuntimeFrameInstanceId(NonZeroU64);

#[repr(transparent)]
pub struct RuntimeMailboxInstanceId(NonZeroU64);

#[repr(transparent)]
pub struct RuntimeChildInstanceId(NonZeroU64);

#[repr(transparent)]
pub struct RuntimeTransferInstanceId(NonZeroU64);

#[repr(transparent)]
pub struct RuntimeCleanupScopeId(NonZeroU64);

#[repr(transparent)]
pub struct RuntimeLocalSlotId(NonZeroU64);
```

Each has exactly:

```rust
impl RuntimeLocalSlotId {
    #[must_use]
    pub const fn get(self) -> NonZeroU64;

    pub(crate) const fn from_allocated(raw: NonZeroU64) -> Self;
}
```

with the type name substituted for every wrapper. All occurrence wrappers are
allocated from the one execution-wide occurrence cursor. Local slots are
allocated from the separate local-slot cursor.

### 2.3 Static/owner-local identities

**Exact target declarations:**

```rust
#[repr(transparent)]
pub struct RuntimeLocalDeclarationId(NonZeroU32);

#[repr(transparent)]
pub struct RuntimeCaptureSlotId(NonZeroU32);

#[repr(transparent)]
pub struct RuntimeFrameLocalId(NonZeroU32);

#[repr(transparent)]
pub struct RuntimeMailboxLaneId(NonZeroU32);

#[repr(transparent)]
pub struct RuntimeChildPacketId(NonZeroU32);

#[repr(transparent)]
pub struct RuntimeTransferPacketId(NonZeroU32);

#[repr(transparent)]
pub struct RuntimeCleanupSlotId(NonZeroU32);
```

Each has exactly:

```rust
impl RuntimeCaptureSlotId {
    #[must_use]
    pub const fn get(self) -> NonZeroU32;

    pub(crate) const fn from_accepted_ordinal(raw: NonZeroU32) -> Self;
}
```

with the type name substituted. `from_accepted_ordinal` remains crate-private;
the owning plan/aggregate validator is the only caller.

### 2.4 Revisions, record fields, affine owners, and transaction IDs

Owner of `RuntimeRecordFieldId`: existing `arcweft_core::value`.  
Owners of the remaining declarations:
`arcweft_core::runtime_id` and `arcweft_core::value::ownership`.

**Exact target declarations:**

```rust
#[repr(transparent)]
pub struct RuntimeRecordFieldId(NonZeroU32);

#[repr(transparent)]
pub struct RuntimeSlotRevision(NonZeroU64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeAffineOwnerId {
    execution: ExecutionInstanceId,
    ordinal: NonZeroU64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeOwnershipTransactionId {
    execution: ExecutionInstanceId,
    ordinal: NonZeroU64,
}
```

**Exact inherent API:**

```rust
impl RuntimeRecordFieldId {
    pub(crate) fn from_accepted_zero_based(
        ordinal: usize,
    ) -> Result<Self, RuntimeRecordFieldIdError>;

    #[must_use]
    pub const fn get(self) -> NonZeroU32;

    #[must_use]
    pub const fn zero_based(self) -> u32;
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeRecordFieldIdError {
    #[error("runtime record field count exceeds u32 identity space")]
    OrdinalOverflow,
}

impl RuntimeSlotRevision {
    #[must_use]
    pub const fn initial() -> Self;

    #[must_use]
    pub const fn get(self) -> NonZeroU64;

    pub(crate) fn checked_next(self) -> Result<Self, RuntimeRevisionExhausted>;
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
#[error("runtime slot revision is exhausted")]
pub struct RuntimeRevisionExhausted;

impl RuntimeAffineOwnerId {
    #[must_use]
    pub const fn execution(self) -> ExecutionInstanceId;

    #[must_use]
    pub const fn ordinal(self) -> NonZeroU64;

    pub(crate) const fn from_allocated(
        execution: ExecutionInstanceId,
        ordinal: NonZeroU64,
    ) -> Self;
}

impl RuntimeOwnershipTransactionId {
    #[must_use]
    pub const fn execution(self) -> ExecutionInstanceId;

    #[must_use]
    pub const fn ordinal(self) -> NonZeroU64;

    pub(crate) const fn from_allocated(
        execution: ExecutionInstanceId,
        ordinal: NonZeroU64,
    ) -> Self;
}
```

## 3. Record carriers

Owner: existing `arcweft_core::value`.

### 3.1 Anonymous record field

**Exact target declaration:**

```rust
#[derive(Debug, PartialEq)]
pub struct RuntimeFieldValue {
    field: RuntimeRecordFieldId,
    name: String,
    value: RuntimeValue,
}
```

`Clone`, `Serialize`, and `Deserialize` are intentionally absent from the live
carrier after the parent affine cut.

**Exact inherent API:**

```rust
impl RuntimeFieldValue {
    pub(crate) fn new_accepted(
        field: RuntimeRecordFieldId,
        name: String,
        value: RuntimeValue,
    ) -> Self;

    #[must_use]
    pub const fn field(&self) -> RuntimeRecordFieldId;

    #[must_use]
    pub fn name(&self) -> &str;

    #[must_use]
    pub fn value(&self) -> &RuntimeValue;

    pub(crate) fn value_mut(&mut self) -> &mut RuntimeValue;

    pub(crate) fn into_value(self) -> RuntimeValue;
}
```

### 3.2 Record-column field

The existing `RecordSeqField` is changed in place:

```rust
#[derive(Debug, PartialEq)]
pub struct RecordSeqField {
    field: RuntimeRecordFieldId,
    name: String,
    values: RuntimeSeq,
}

impl RecordSeqField {
    pub(crate) fn new_accepted(
        field: RuntimeRecordFieldId,
        name: String,
        values: RuntimeSeq,
    ) -> Self;

    #[must_use]
    pub const fn field(&self) -> RuntimeRecordFieldId;

    #[must_use]
    pub fn name(&self) -> &str;

    #[must_use]
    pub fn values(&self) -> &RuntimeSeq;
}
```

### 3.3 Record admission

**Exact target API on existing owners:**

```rust
impl RuntimeValue {
    pub(crate) fn try_record(
        fields_in_authored_order: Vec<(String, RuntimeValue)>,
    ) -> Result<Self, RuntimeRecordAdmissionError>;
}

impl RuntimeNominalRecordValue {
    pub(crate) fn try_from_accepted_layout(
        schema: Arc<RuntimeNominalRecordSchema>,
        fields_in_layout_order: Vec<RuntimeValue>,
    ) -> Result<Self, RuntimeNominalRecordError>;

    #[must_use]
    pub fn field_id(&self, zero_based_ordinal: usize)
        -> Option<RuntimeRecordFieldId>;
}

impl RecordSeq {
    pub(crate) fn try_from_accepted_fields(
        rows: usize,
        fields_in_accepted_order: Vec<(String, RuntimeSeq)>,
    ) -> Result<Self, RecordSeqError>;
}
```

`RuntimeNominalRecordSchema`, `RuntimeNominalRecordError`, `RecordSeq`, and
`RecordSeqError` are existing owners. `try_from_accepted_layout` replaces any
unchecked public constructor.

**Exact new error:**

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeRecordAdmissionError {
    #[error("runtime record has duplicate field name `{name}`")]
    DuplicateName { name: String },

    #[error("runtime record has too many fields")]
    TooManyFields,

    #[error("runtime record field `{name}` has invalid identity")]
    InvalidFieldIdentity {
        name: String,
        source: RuntimeRecordFieldIdError,
    },
}
```

## 4. Local/capture storage

Owner: existing `arcweft_core::value`.

### 4.1 Mutability, state, and integrated reservation

**Exact target declarations:**

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBindingMutability {
    Immutable,
    Mutable,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeSlotStateKind {
    Vacant,
    Live,
    Moved,
    Dropped,
}

#[derive(Debug, PartialEq)]
pub enum RuntimeSlotState {
    Vacant,
    Live(RuntimeValue),
    Moved(RuntimeMovedValueEvidence),
    Dropped(RuntimeDroppedValueEvidence),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeSlotReservationRole {
    CopySource,
    CopyDestination,
    MoveSource,
    MoveDestination,
    DropSource,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RuntimeSlotReservation {
    transaction: RuntimeOwnershipTransactionId,
    expected_revision: RuntimeSlotRevision,
    role: RuntimeSlotReservationRole,
}

#[derive(Debug, PartialEq)]
pub(crate) struct RuntimeSlotCell {
    declared_type: RuntimeCheckedType,
    revision: RuntimeSlotRevision,
    state: RuntimeSlotState,
    reservation: Option<RuntimeSlotReservation>,
}
```

**Exact inherent API:**

```rust
impl RuntimeSlotState {
    #[must_use]
    pub const fn kind(&self) -> RuntimeSlotStateKind;

    #[must_use]
    pub fn live_value(&self) -> Option<&RuntimeValue>;

    pub(crate) fn live_value_mut(&mut self) -> Option<&mut RuntimeValue>;
}

impl RuntimeSlotCell {
    pub(crate) fn new_live(
        declared_type: RuntimeCheckedType,
        value: RuntimeValue,
    ) -> Self;

    pub(crate) fn new_vacant(
        declared_type: RuntimeCheckedType,
    ) -> Self;

    #[must_use]
    pub fn declared_type(&self) -> &RuntimeCheckedType;

    #[must_use]
    pub const fn revision(&self) -> RuntimeSlotRevision;

    #[must_use]
    pub const fn state_kind(&self) -> RuntimeSlotStateKind;

    #[must_use]
    pub fn state(&self) -> &RuntimeSlotState;

    #[must_use]
    pub(crate) const fn reservation(&self) -> Option<RuntimeSlotReservation>;
}
```

There is no public setter. Every state/revision/reservation mutation is
crate-private and called only by the transaction engine.

### 4.2 Environment binding

**Exact target declaration:**

```rust
#[derive(Debug, PartialEq)]
pub struct RuntimeBinding {
    slot: RuntimeLocalSlotId,
    declaration: RuntimeLocalDeclarationId,
    mutability: RuntimeBindingMutability,
    diagnostic_name: String,
    cell: RuntimeSlotCell,
}
```

**Exact inherent API:**

```rust
impl RuntimeBinding {
    pub(crate) fn new_live(
        slot: RuntimeLocalSlotId,
        declaration: RuntimeLocalDeclarationId,
        mutability: RuntimeBindingMutability,
        diagnostic_name: String,
        declared_type: RuntimeCheckedType,
        value: RuntimeValue,
    ) -> Self;

    #[must_use]
    pub const fn slot(&self) -> RuntimeLocalSlotId;

    #[must_use]
    pub const fn declaration(&self) -> RuntimeLocalDeclarationId;

    #[must_use]
    pub const fn mutability(&self) -> RuntimeBindingMutability;

    #[must_use]
    pub fn diagnostic_name(&self) -> &str;

    #[must_use]
    pub fn declared_type(&self) -> &RuntimeCheckedType;

    #[must_use]
    pub const fn revision(&self) -> RuntimeSlotRevision;

    #[must_use]
    pub const fn state_kind(&self) -> RuntimeSlotStateKind;

    #[must_use]
    pub fn value(&self) -> Result<&RuntimeValue, RuntimeBindingAccessError<'_>>;
}

#[derive(Debug, Error, PartialEq)]
pub enum RuntimeBindingAccessError<'a> {
    #[error("runtime local {slot} was moved")]
    Moved {
        slot: RuntimeLocalSlotId,
        evidence: &'a RuntimeMovedValueEvidence,
    },

    #[error("runtime local {slot} was dropped")]
    Dropped {
        slot: RuntimeLocalSlotId,
        evidence: &'a RuntimeDroppedValueEvidence,
    },

    #[error("runtime local {slot} is vacant")]
    Vacant {
        slot: RuntimeLocalSlotId,
    },
}
```

### 4.3 Capture binding

**Exact target declaration:**

```rust
#[derive(Debug, PartialEq)]
pub struct RuntimeCaptureBinding {
    capture: RuntimeCaptureSlotId,
    source_declaration: RuntimeLocalDeclarationId,
    mutability: RuntimeBindingMutability,
    diagnostic_name: String,
    cell: RuntimeSlotCell,
}
```

**Exact inherent API:**

```rust
impl RuntimeCaptureBinding {
    pub(crate) fn new_live(
        capture: RuntimeCaptureSlotId,
        source_declaration: RuntimeLocalDeclarationId,
        mutability: RuntimeBindingMutability,
        diagnostic_name: String,
        declared_type: RuntimeCheckedType,
        value: RuntimeValue,
    ) -> Self;

    #[must_use]
    pub const fn capture(&self) -> RuntimeCaptureSlotId;

    #[must_use]
    pub const fn source_declaration(&self) -> RuntimeLocalDeclarationId;

    #[must_use]
    pub const fn revision(&self) -> RuntimeSlotRevision;

    #[must_use]
    pub const fn state_kind(&self) -> RuntimeSlotStateKind;

    #[must_use]
    pub fn value(&self) -> Result<&RuntimeValue, RuntimeCaptureAccessError<'_>>;
}

#[derive(Debug, Error, PartialEq)]
pub enum RuntimeCaptureAccessError<'a> {
    #[error("runtime capture {capture:?} was moved")]
    Moved {
        capture: RuntimeCaptureSlotId,
        evidence: &'a RuntimeMovedValueEvidence,
    },

    #[error("runtime capture {capture:?} was dropped")]
    Dropped {
        capture: RuntimeCaptureSlotId,
        evidence: &'a RuntimeDroppedValueEvidence,
    },

    #[error("runtime capture {capture:?} is vacant")]
    Vacant {
        capture: RuntimeCaptureSlotId,
    },
}
```

The parent `RuntimeFunctionValue` is changed in place to carry exactly one
`RuntimeClosureInstanceId` and `Vec<RuntimeCaptureBinding>`. Its accepted
parameter/body plan remains parent-owned. Its `captures` field is private and
returned in capture-ID order; it is not `Vec<RuntimeBinding>`.

### 4.4 Environment API

The existing `RuntimeEnv` remains the sole environment. Its exact successful
identity-bearing API becomes:

```rust
impl RuntimeEnv {
    pub(crate) fn push_scope(
        &mut self,
        scope: RuntimeScopeInstanceId,
        binding_capacity: usize,
    );

    pub(crate) fn scope_exit_view(
        &self,
    ) -> Result<RuntimeScopeExitView<'_>, RuntimeScopeExitError>;

    pub(crate) fn recycle_committed_scope(
        &mut self,
        scope: RuntimeScopeInstanceId,
    ) -> Result<(), RuntimeScopeExitError>;

    pub(crate) fn bind(
        &mut self,
        binding: RuntimeBinding,
    ) -> Result<(), RuntimeEnvBindError>;

    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&RuntimeBinding>;

    #[must_use]
    pub fn resolve_slot(
        &self,
        slot: RuntimeLocalSlotId,
    ) -> Option<&RuntimeBinding>;

    pub(crate) fn resolve_slot_mut(
        &mut self,
        slot: RuntimeLocalSlotId,
    ) -> Option<&mut RuntimeBinding>;

    pub fn visible_bindings(
        &self,
    ) -> impl DoubleEndedIterator<Item = &RuntimeBinding>;

    pub fn scopes(
        &self,
    ) -> impl ExactSizeIterator<Item = RuntimeScopeView<'_>>;
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeScopeView<'a> {
    scope: RuntimeScopeInstanceId,
    bindings: &'a [RuntimeBinding],
}

impl RuntimeScopeView<'_> {
    #[must_use]
    pub const fn scope(&self) -> RuntimeScopeInstanceId;

    #[must_use]
    pub fn bindings(&self) -> &[RuntimeBinding];
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeEnvBindError {
    #[error("runtime local slot {slot} already exists")]
    DuplicateSlot {
        slot: RuntimeLocalSlotId,
    },

    #[error("runtime declaration {declaration:?} is bound twice in scope {scope:?}")]
    DuplicateDeclarationInScope {
        scope: RuntimeScopeInstanceId,
        declaration: RuntimeLocalDeclarationId,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeScopeExitView<'a> {
    scope: RuntimeScopeInstanceId,
    bindings: &'a [RuntimeBinding],
}

impl RuntimeScopeExitView<'_> {
    #[must_use]
    pub const fn scope(&self) -> RuntimeScopeInstanceId;

    #[must_use]
    pub fn bindings(&self) -> &[RuntimeBinding];
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeScopeExitError {
    #[error("the root runtime scope cannot exit")]
    RootScope,

    #[error("runtime scope exit expected {expected:?}, found {actual:?}")]
    WrongScope {
        expected: RuntimeScopeInstanceId,
        actual: RuntimeScopeInstanceId,
    },

    #[error("runtime local slot {slot} is not dropped during scope recycling")]
    BindingNotDropped {
        slot: RuntimeLocalSlotId,
        state: RuntimeSlotStateKind,
    },
}
```

The structured engine obtains `RuntimeScopeExitView`, builds one canonical Drop
transaction for every still-live binding, commits it, and only then calls
`recycle_committed_scope`. Recycling verifies the same top scope and that every
binding is `Dropped`; it may return the backing vector capacity to the existing
spare-scope pool but never reuses any slot identity. A prepare or commit error
therefore owns the transaction at the transaction layer and leaves the scope in
place; `RuntimeEnv` does not accept a half-prepared transaction.

`get_cloned`, `bindings_snapshot`, `set_ref`, `bind_all_ref`, root-ref
replacement, and name-only mutation are not successful APIs after the switch.

## 5. Canonical value path

Owner: existing `arcweft_core::value::ownership`.

**Exact target declarations:**

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RuntimeValuePath(Box<[RuntimeValuePathSegment]>);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RuntimeValuePathSegment {
    TupleElement(u32),
    SequenceElement(u64),
    TupleColumn(u32),
    RecordField(RuntimeRecordFieldId),
    RecordColumn(RuntimeRecordFieldId),
    NominalRecordField(RuntimeRecordFieldId),
    FunctionCapture(RuntimeCaptureSlotId),
    VariantPayload,
    IteratorRemainder(u64),
    IteratorWitnessState,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeValuePathError {
    #[error("runtime value path has {actual} segments; maximum is {maximum}")]
    TooDeep {
        maximum: u32,
        actual: usize,
    },

    #[error("runtime value path does not exist in the selected value graph")]
    Missing {
        path: RuntimeValuePath,
    },

    #[error("runtime value path segment {segment} has the wrong aggregate kind")]
    WrongAggregateKind {
        segment: usize,
    },

    #[error("runtime record field identities are not contiguous and unique")]
    InvalidRecordFieldIdentity,
}
```

**Exact inherent/ordering API:**

```rust
impl RuntimeValuePath {
    #[must_use]
    pub fn root() -> Self;

    pub fn try_from_segments(
        segments: impl IntoIterator<Item = RuntimeValuePathSegment>,
    ) -> Result<Self, RuntimeValuePathError>;

    #[must_use]
    pub fn segments(&self) -> &[RuntimeValuePathSegment];

    #[must_use]
    pub const fn is_root(&self) -> bool;

    #[must_use]
    pub fn child(
        &self,
        segment: RuntimeValuePathSegment,
    ) -> Result<Self, RuntimeValuePathError>;
}

impl RuntimeValuePathSegment {
    #[must_use]
    pub const fn canonical_tag(self) -> u8;
}

impl Ord for RuntimeValuePath;
impl PartialOrd for RuntimeValuePath;
impl Ord for RuntimeValuePathSegment;
impl PartialOrd for RuntimeValuePathSegment;
```

The manual `Ord` implementations are exactly those specified in
`VALUE_PATH_AND_PRECEDENCE.md`; declaration-order-derived discriminant ordering
is not used.

## 6. Diagnostic slot union

Owner: existing `arcweft_core::value::ownership`.

**Exact target declaration:**

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RuntimeOwnedSlotId {
    EnvironmentLocal {
        execution: ExecutionInstanceId,
        local: RuntimeLocalSlotId,
    },
    ClosureCapture {
        execution: ExecutionInstanceId,
        closure: RuntimeClosureInstanceId,
        capture: RuntimeCaptureSlotId,
    },
    AwbcRegister {
        execution: ExecutionInstanceId,
        fiber: RuntimeFiberInstanceId,
        frame: RuntimeFrameInstanceId,
        register: AwbcRegisterId,
    },
    AwbcFrameLocal {
        execution: ExecutionInstanceId,
        fiber: RuntimeFiberInstanceId,
        frame: RuntimeFrameInstanceId,
        local: RuntimeFrameLocalId,
    },
    MailboxLane {
        execution: ExecutionInstanceId,
        mailbox: RuntimeMailboxInstanceId,
        lane: RuntimeMailboxLaneId,
    },
    ChildPacket {
        execution: ExecutionInstanceId,
        child: RuntimeChildInstanceId,
        packet: RuntimeChildPacketId,
    },
    TransferPacket {
        execution: ExecutionInstanceId,
        transfer: RuntimeTransferInstanceId,
        packet: RuntimeTransferPacketId,
    },
    CleanupSlot {
        execution: ExecutionInstanceId,
        scope: RuntimeCleanupScopeId,
        slot: RuntimeCleanupSlotId,
    },
}
```

**Exact inherent/trait API:**

```rust
impl RuntimeOwnedSlotId {
    #[must_use]
    pub const fn canonical_tag(self) -> u8;

    #[must_use]
    pub const fn execution(self) -> ExecutionInstanceId;

    #[must_use]
    pub fn render_canonical(self) -> String;
}

impl fmt::Display for RuntimeOwnedSlotId;
impl Ord for RuntimeOwnedSlotId;
impl PartialOrd for RuntimeOwnedSlotId;
impl Serialize for RuntimeOwnedSlotId;
impl<'de> Deserialize<'de> for RuntimeOwnedSlotId;
```

`render_canonical` is an inherent method on the owning enum. There is no
`RuntimeOwnedSlotExt` trait and no free rendering helper.

## 7. Transfer plan and transaction limits

Owner: existing `arcweft_core::value::ownership`.

Storage transfer endpoints are whole runtime slots. `RuntimeValuePath` is the
canonical path *inside* the source/destination value graph for owner evidence
and diagnostics; it is not a storage key and does not create partially live
slot storage.

### 7.1 Plan

**Exact target declarations:**

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeTransferStepIndex(NonZeroU32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTransferEndpoint {
    slot: RuntimeOwnedSlotId,
    expected_revision: RuntimeSlotRevision,
    expected_type: RuntimeCheckedType,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeDropReason {
    Explicit,
    ScopeExit,
    Unwind,
    Cancellation,
    ChildCompletion,
    TransferAbort,
    HotReplacement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeTransferStep {
    Copy {
        source: RuntimeTransferEndpoint,
        destination: RuntimeTransferEndpoint,
    },
    Move {
        source: RuntimeTransferEndpoint,
        destination: RuntimeTransferEndpoint,
    },
    Drop {
        source: RuntimeTransferEndpoint,
        reason: RuntimeDropReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTransferPlan {
    steps: Vec<RuntimeTransferStep>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeTransferPlanError {
    #[error("runtime ownership transaction has no steps")]
    Empty,

    #[error("runtime ownership transaction has {actual} steps; maximum is {maximum}")]
    TooManySteps {
        maximum: u32,
        actual: usize,
    },

    #[error("runtime ownership step {step:?} uses the same source and destination slot")]
    SameSourceAndDestination {
        step: RuntimeTransferStepIndex,
        slot: RuntimeOwnedSlotId,
    },

    #[error("runtime ownership step index exceeds u32 identity space")]
    StepIndexOverflow,
}
```

**Exact inherent API:**

```rust
impl RuntimeTransferStepIndex {
    #[must_use]
    pub const fn get(self) -> NonZeroU32;

    pub(crate) const fn from_accepted(raw: NonZeroU32) -> Self;
}

impl RuntimeTransferEndpoint {
    #[must_use]
    pub const fn slot(&self) -> RuntimeOwnedSlotId;

    #[must_use]
    pub const fn expected_revision(&self) -> RuntimeSlotRevision;

    #[must_use]
    pub fn expected_type(&self) -> &RuntimeCheckedType;
}

impl RuntimeTransferPlan {
    pub fn try_new(
        steps: Vec<RuntimeTransferStep>,
        limits: RuntimeOwnershipLimits,
    ) -> Result<Self, RuntimeTransferPlanError>;

    #[must_use]
    pub fn steps(&self) -> &[RuntimeTransferStep];

    #[must_use]
    pub fn into_steps(self) -> Vec<RuntimeTransferStep>;
}
```

`RuntimeTransferEndpoint` has no public constructor. Runtime-plan/engine owners
construct it through crate-private inherent constructors after typed projection.

### 7.2 Limits

**Exact target declarations:**

```rust
pub const MAX_OWNERSHIP_TRANSACTION_PARTICIPANTS: u32 = 4_096;
pub const MAX_OWNERSHIP_TRANSACTION_STEPS: u32 = 4_096;
pub const MAX_OWNERSHIP_VALUE_NODES: u64 = 1_048_576;
pub const MAX_RUNTIME_VALUE_PATH_SEGMENTS: u32 = 64;
pub const MAX_OWNERSHIP_AFFINE_OWNERS: u32 = 262_144;
pub const MAX_OWNERSHIP_STAGED_BYTES: u64 = 67_108_864;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RuntimeOwnershipLimits {
    participants: NonZeroU32,
    steps: NonZeroU32,
    value_nodes: NonZeroU64,
    path_segments: NonZeroU32,
    affine_owners: NonZeroU32,
    staged_bytes: NonZeroU64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeOwnershipLimitKind {
    Participants,
    Steps,
    ValueNodes,
    PathSegments,
    AffineOwners,
    StagedBytes,
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, PartialEq)]
pub enum RuntimeOwnershipLimitError {
    #[error("{kind:?} limit {supplied} exceeds hard maximum {maximum}")]
    AboveHardMaximum {
        kind: RuntimeOwnershipLimitKind,
        supplied: u64,
        maximum: u64,
    },
}
```

**Exact inherent API:**

```rust
impl RuntimeOwnershipLimits {
    #[must_use]
    pub const fn hard_maximum() -> Self;

    pub fn try_new(
        participants: NonZeroU32,
        steps: NonZeroU32,
        value_nodes: NonZeroU64,
        path_segments: NonZeroU32,
        affine_owners: NonZeroU32,
        staged_bytes: NonZeroU64,
    ) -> Result<Self, RuntimeOwnershipLimitError>;

    #[must_use]
    pub const fn participants(self) -> NonZeroU32;

    #[must_use]
    pub const fn steps(self) -> NonZeroU32;

    #[must_use]
    pub const fn value_nodes(self) -> NonZeroU64;

    #[must_use]
    pub const fn path_segments(self) -> NonZeroU32;

    #[must_use]
    pub const fn affine_owners(self) -> NonZeroU32;

    #[must_use]
    pub const fn staged_bytes(self) -> NonZeroU64;
}
```

## 8. Owner occurrences and committed evidence

**Exact target declarations:**

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeAffineOwnerOccurrence {
    owner: RuntimeAffineOwnerId,
    path: RuntimeValuePath,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCopiedValueEvidence {
    transaction: RuntimeOwnershipTransactionId,
    source: RuntimeOwnedSlotId,
    destination: RuntimeOwnedSlotId,
    source_revision: RuntimeSlotRevision,
    destination_revision: RuntimeSlotRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeMovedValueEvidence {
    transaction: RuntimeOwnershipTransactionId,
    source: RuntimeOwnedSlotId,
    destination: RuntimeOwnedSlotId,
    source_revision: RuntimeSlotRevision,
    destination_revision: RuntimeSlotRevision,
    ownership: RuntimeValueOwnership,
    owners: Vec<RuntimeAffineOwnerOccurrence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDroppedValueEvidence {
    transaction: RuntimeOwnershipTransactionId,
    source: RuntimeOwnedSlotId,
    source_revision: RuntimeSlotRevision,
    reason: RuntimeDropReason,
    ownership: RuntimeValueOwnership,
    owners: Vec<RuntimeAffineOwnerOccurrence>,
}
```

The three evidence types implement manual `Serialize`/`Deserialize` with the
field order shown. Every `source_revision`/`destination_revision` is the exact
*pre-commit observed revision*. After a successful Copy, the destination cell
revision is `evidence.destination_revision().checked_next()`. After Move, both
cell revisions are the checked successors of the evidence revisions. After
Drop, the source cell revision is the checked successor. This relation is
validated on restore. `RuntimeMovedValueEvidence` and
`RuntimeDroppedValueEvidence` are the exact tombstone payloads.

**Exact accessors:**

```rust
impl RuntimeAffineOwnerOccurrence {
    #[must_use]
    pub const fn owner(&self) -> RuntimeAffineOwnerId;

    #[must_use]
    pub fn path(&self) -> &RuntimeValuePath;
}

impl RuntimeMovedValueEvidence {
    #[must_use]
    pub const fn transaction(&self) -> RuntimeOwnershipTransactionId;

    #[must_use]
    pub const fn source(&self) -> RuntimeOwnedSlotId;

    #[must_use]
    pub const fn destination(&self) -> RuntimeOwnedSlotId;

    #[must_use]
    pub const fn source_revision(&self) -> RuntimeSlotRevision;

    #[must_use]
    pub const fn destination_revision(&self) -> RuntimeSlotRevision;

    #[must_use]
    pub const fn ownership(&self) -> RuntimeValueOwnership;

    #[must_use]
    pub fn owners(&self) -> &[RuntimeAffineOwnerOccurrence];
}

impl RuntimeDroppedValueEvidence {
    #[must_use]
    pub const fn transaction(&self) -> RuntimeOwnershipTransactionId;

    #[must_use]
    pub const fn source(&self) -> RuntimeOwnedSlotId;

    #[must_use]
    pub const fn source_revision(&self) -> RuntimeSlotRevision;

    #[must_use]
    pub const fn reason(&self) -> RuntimeDropReason;

    #[must_use]
    pub const fn ownership(&self) -> RuntimeValueOwnership;

    #[must_use]
    pub fn owners(&self) -> &[RuntimeAffineOwnerOccurrence];
}
```

## 9. Prepared owners

Prepared values are linear and implement neither `Clone`, `Serialize`, nor
`Deserialize`.

**Exact target declarations:**

```rust
#[derive(Debug)]
pub struct RuntimePreparedCopy {
    step: RuntimeTransferStepIndex,
    source: RuntimeTransferEndpoint,
    destination: RuntimeTransferEndpoint,
    next_destination_revision: RuntimeSlotRevision,
    duplicate: RuntimeValue,
    evidence: RuntimeCopiedValueEvidence,
}

#[derive(Debug)]
pub struct RuntimePreparedMove {
    step: RuntimeTransferStepIndex,
    source: RuntimeTransferEndpoint,
    destination: RuntimeTransferEndpoint,
    next_source_revision: RuntimeSlotRevision,
    next_destination_revision: RuntimeSlotRevision,
    source_tombstone: RuntimeMovedValueEvidence,
    committed_evidence: RuntimeMovedValueEvidence,
}

#[derive(Debug)]
pub struct RuntimePreparedDrop {
    step: RuntimeTransferStepIndex,
    source: RuntimeTransferEndpoint,
    next_source_revision: RuntimeSlotRevision,
    source_tombstone: RuntimeDroppedValueEvidence,
    committed_evidence: RuntimeDroppedValueEvidence,
}

#[derive(Debug)]
pub(crate) enum RuntimePreparedTransfer {
    Copy(RuntimePreparedCopy),
    Move(RuntimePreparedMove),
    Drop(RuntimePreparedDrop),
}
```

**Exact inherent API:**

```rust
impl RuntimePreparedCopy {
    #[must_use]
    pub const fn step(&self) -> RuntimeTransferStepIndex;

    #[must_use]
    pub const fn source(&self) -> &RuntimeTransferEndpoint;

    #[must_use]
    pub const fn destination(&self) -> &RuntimeTransferEndpoint;
}

impl RuntimePreparedMove {
    #[must_use]
    pub const fn step(&self) -> RuntimeTransferStepIndex;

    #[must_use]
    pub const fn source(&self) -> &RuntimeTransferEndpoint;

    #[must_use]
    pub const fn destination(&self) -> &RuntimeTransferEndpoint;
}

impl RuntimePreparedDrop {
    #[must_use]
    pub const fn step(&self) -> RuntimeTransferStepIndex;

    #[must_use]
    pub const fn source(&self) -> &RuntimeTransferEndpoint;
}
```

There is no `commit(value: RuntimeValue)` method on Move or Drop.

## 10. Slot observations and sealed storage protocol

The protocol is crate-private and sealed. The one core executor implements it
by delegating to the existing environment, closure, AWBC, mailbox, child,
transfer, and cleanup owners. External crates cannot implement it.

**Exact target declarations:**

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RuntimePreparedRootHandle {
    Environment {
        scope_index: u32,
        binding_index: u32,
    },
    ClosureCapture {
        closure_index: u32,
        capture_index: u32,
    },
    AwbcRegister {
        fiber_index: u32,
        frame_index: u32,
        register: AwbcRegisterId,
    },
    AwbcFrameLocal {
        fiber_index: u32,
        frame_index: u32,
        local_index: u32,
    },
    MailboxLane {
        mailbox_index: u32,
        lane_index: u32,
    },
    ChildPacket {
        child_index: u32,
        packet_index: u32,
    },
    TransferPacket {
        transfer_index: u32,
        packet_index: u32,
    },
    CleanupSlot {
        cleanup_scope_index: u32,
        slot_index: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RuntimePreparedSlotHandle {
    root: RuntimePreparedRootHandle,
}

#[derive(Debug)]
pub(crate) struct RuntimeSlotObservation<'a> {
    slot: RuntimeOwnedSlotId,
    handle: RuntimePreparedSlotHandle,
    revision: RuntimeSlotRevision,
    declared_type: &'a RuntimeCheckedType,
    state_kind: RuntimeSlotStateKind,
    live_value: Option<&'a RuntimeValue>,
    reservation: Option<RuntimeSlotReservation>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum RuntimeSlotAccessError {
    #[error("runtime slot {slot} does not exist")]
    Missing {
        slot: RuntimeOwnedSlotId,
    },

    #[error("runtime slot storage identity does not match {slot}")]
    IdentityMismatch {
        slot: RuntimeOwnedSlotId,
    },

    #[error("runtime slot {slot} is reserved by {actual}, expected {expected}")]
    ReservationMismatch {
        slot: RuntimeOwnedSlotId,
        expected: RuntimeOwnershipTransactionId,
        actual: RuntimeOwnershipTransactionId,
    },

    #[error("runtime slot {slot} revision changed from {expected:?} to {actual:?}")]
    RevisionMismatch {
        slot: RuntimeOwnedSlotId,
        expected: RuntimeSlotRevision,
        actual: RuntimeSlotRevision,
    },

    #[error("runtime slot {slot} occupancy changed from {expected:?} to {actual:?}")]
    OccupancyMismatch {
        slot: RuntimeOwnedSlotId,
        expected: RuntimeSlotStateKind,
        actual: RuntimeSlotStateKind,
    },

    #[error("runtime slot {slot} type changed")]
    TypeMismatch {
        slot: RuntimeOwnedSlotId,
    },
}
```

**Exact sealed trait:**

```rust
pub(crate) trait RuntimeOwnershipSlotStore: private::Sealed {
    fn execution(&self) -> ExecutionInstanceId;

    fn observe(
        &self,
        slot: RuntimeOwnedSlotId,
    ) -> Result<RuntimeSlotObservation<'_>, RuntimeSlotAccessError>;

    fn install_reservation(
        &mut self,
        handle: RuntimePreparedSlotHandle,
        reservation: RuntimeSlotReservation,
    ) -> Result<(), RuntimeSlotAccessError>;

    fn clear_reservation(
        &mut self,
        handle: RuntimePreparedSlotHandle,
        transaction: RuntimeOwnershipTransactionId,
    );

    fn acquire_commit_permit(
        &mut self,
        prepared: RuntimePreparedOwnershipTransaction,
    ) -> Result<RuntimeCommitPermit, RuntimeTransferCommitError>;

    fn commit_permit(
        &mut self,
        permit: RuntimeCommitPermit,
    ) -> RuntimeCommittedOwnershipTransaction;
}

mod private {
    pub trait Sealed {}
}
```

`acquire_commit_permit` performs every commit-time check before constructing the
permit. `commit_permit` has no error return.

## 11. Commit permit

**Exact target declarations:**

```rust
#[derive(Debug)]
pub(crate) enum RuntimeCommitMutation {
    Copy {
        destination: RuntimePreparedSlotHandle,
        duplicate: RuntimeValue,
        next_destination_revision: RuntimeSlotRevision,
    },
    Move {
        source: RuntimePreparedSlotHandle,
        destination: RuntimePreparedSlotHandle,
        next_source_revision: RuntimeSlotRevision,
        next_destination_revision: RuntimeSlotRevision,
        source_tombstone: RuntimeMovedValueEvidence,
    },
    Drop {
        source: RuntimePreparedSlotHandle,
        next_source_revision: RuntimeSlotRevision,
        source_tombstone: RuntimeDroppedValueEvidence,
    },
}

#[derive(Debug)]
pub(crate) struct RuntimeCommitPermit {
    transaction: RuntimeOwnershipTransactionId,
    mutations: Vec<RuntimeCommitMutation>,
    reservations: Vec<RuntimePreparedSlotHandle>,
    copied: Vec<RuntimeCopiedValueEvidence>,
    moved: Vec<RuntimeMovedValueEvidence>,
    dropped: Vec<RuntimeDroppedValueEvidence>,
}
```

The vectors have `len == capacity` before the permit is returned. `reservations`
contains each unique participant handle exactly once in canonical slot order;
this includes a Copy source referenced by more than one Copy step. The slot
store has revalidated every handle, revision, reservation, state, type, and
owner set. `commit_permit` only performs `mem::replace`/`mem::take`, writes
precomputed revisions/tombstones, clears the prevalidated reservation handles
after all mutations, and moves already owned vectors.

## 12. Transaction owner, prepare errors, and commit errors

### 12.1 Allocation and budget kinds

**Exact target declarations:**

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeOwnershipAllocationKind {
    ParticipantTable,
    PreparedTransfers,
    ValueTraversalStack,
    ValuePath,
    AffineOwnerEvidence,
    CheckedDuplicate,
    CommitMutations,
    CommitEvidence,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeOwnershipBudgetKind {
    Participants,
    Steps,
    ValueNodes,
    PathSegments,
    AffineOwners,
    StagedBytes,
}
```

### 12.2 Prepare error

**Exact target declaration:**

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeOwnershipPrepareErrorKind {
    #[error(
        "transaction {transaction} belongs to execution {transaction_execution}, \
         but participant {slot} belongs to {participant_execution}"
    )]
    ExecutionMismatch {
        transaction: RuntimeOwnershipTransactionId,
        transaction_execution: ExecutionInstanceId,
        slot: RuntimeOwnedSlotId,
        participant_execution: ExecutionInstanceId,
    },

    #[error(
        "runtime slot {slot} has conflicting participation at steps {first:?} and {second:?}"
    )]
    ConflictingParticipant {
        slot: RuntimeOwnedSlotId,
        first: RuntimeTransferStepIndex,
        second: RuntimeTransferStepIndex,
    },

    #[error("runtime slot {slot} revision is stale")]
    StaleRevision {
        slot: RuntimeOwnedSlotId,
        expected: RuntimeSlotRevision,
        actual: RuntimeSlotRevision,
    },

    #[error("runtime slot {slot} is reserved by transaction {owner}")]
    SlotReserved {
        slot: RuntimeOwnedSlotId,
        owner: RuntimeOwnershipTransactionId,
    },

    #[error("runtime source slot {slot} is not live")]
    SourceNotLive {
        slot: RuntimeOwnedSlotId,
        state: RuntimeSlotStateKind,
    },

    #[error("runtime destination slot {slot} is not vacant")]
    DestinationNotEmpty {
        slot: RuntimeOwnedSlotId,
        state: RuntimeSlotStateKind,
    },

    #[error("runtime slot {slot} has the wrong accepted type")]
    TypeMismatch {
        slot: RuntimeOwnedSlotId,
        expected: RuntimeCheckedType,
        actual: RuntimeCheckedType,
    },

    #[error("runtime value path is invalid in slot {slot}")]
    InvalidValuePath {
        slot: RuntimeOwnedSlotId,
        path: RuntimeValuePath,
        source: RuntimeValuePathError,
    },

    #[error("runtime record field identity is invalid in slot {slot}")]
    InvalidRecordLayout {
        slot: RuntimeOwnedSlotId,
        path: RuntimeValuePath,
    },

    #[error("affine owner {owner:?} occurs more than once")]
    DuplicateOwner {
        owner: RuntimeAffineOwnerId,
        first_slot: RuntimeOwnedSlotId,
        first_path: RuntimeValuePath,
        second_slot: RuntimeOwnedSlotId,
        second_path: RuntimeValuePath,
    },

    #[error("copy would duplicate affine owner {owner:?}")]
    AffineCopy {
        slot: RuntimeOwnedSlotId,
        owner: RuntimeAffineOwnerId,
        path: RuntimeValuePath,
    },

    #[error("runtime slot {slot} cannot advance its revision")]
    RevisionExhausted {
        slot: RuntimeOwnedSlotId,
    },

    #[error("{namespace:?} identity allocation is exhausted")]
    IdentityExhausted {
        namespace: RuntimeIdNamespace,
    },

    #[error("{kind:?} ownership budget exceeded: {actual} > {limit}")]
    BudgetExceeded {
        kind: RuntimeOwnershipBudgetKind,
        limit: u64,
        actual: u64,
    },

    #[error("{kind:?} allocation for {requested} element(s) failed")]
    AllocationFailed {
        kind: RuntimeOwnershipAllocationKind,
        requested: usize,
    },
}

#[derive(Debug, Error)]
#[error("{kind}")]
pub struct RuntimeOwnershipPrepareError {
    kind: RuntimeOwnershipPrepareErrorKind,
    transaction: RuntimeOwnershipTransaction,
}
```

**Exact inherent API:**

```rust
impl RuntimeOwnershipPrepareErrorKind {
    #[must_use]
    pub const fn precedence_rank(&self) -> u8;
}

impl RuntimeOwnershipPrepareError {
    #[must_use]
    pub const fn kind(&self) -> &RuntimeOwnershipPrepareErrorKind;

    #[must_use]
    pub const fn transaction(&self) -> &RuntimeOwnershipTransaction;

    pub fn into_parts(
        self,
    ) -> (
        RuntimeOwnershipPrepareErrorKind,
        RuntimeOwnershipTransaction,
    );
}
```

### 12.3 Transaction and prepared transaction

**Exact target declarations:**

```rust
#[derive(Debug)]
pub struct RuntimeOwnershipTransaction {
    id: RuntimeOwnershipTransactionId,
    limits: RuntimeOwnershipLimits,
    plan: RuntimeTransferPlan,
}

#[derive(Debug)]
pub struct RuntimePreparedOwnershipTransaction {
    id: RuntimeOwnershipTransactionId,
    limits: RuntimeOwnershipLimits,
    plan: RuntimeTransferPlan,
    prepared: Vec<RuntimePreparedTransfer>,
    participants: Vec<(RuntimeOwnedSlotId, RuntimePreparedSlotHandle)>,
    visited_value_nodes: u64,
    staged_bytes: u64,
}
```

**Exact inherent API:**

```rust
impl RuntimeOwnershipTransaction {
    pub(crate) fn new_allocated(
        id: RuntimeOwnershipTransactionId,
        limits: RuntimeOwnershipLimits,
        plan: RuntimeTransferPlan,
    ) -> Self;

    #[must_use]
    pub const fn id(&self) -> RuntimeOwnershipTransactionId;

    #[must_use]
    pub const fn limits(&self) -> RuntimeOwnershipLimits;

    #[must_use]
    pub fn plan(&self) -> &RuntimeTransferPlan;

    pub(crate) fn prepare(
        self,
        store: &mut impl RuntimeOwnershipSlotStore,
    ) -> Result<
        RuntimePreparedOwnershipTransaction,
        RuntimeOwnershipPrepareError,
    >;
}

impl RuntimePreparedOwnershipTransaction {
    #[must_use]
    pub const fn id(&self) -> RuntimeOwnershipTransactionId;

    #[must_use]
    pub fn plan(&self) -> &RuntimeTransferPlan;

    #[must_use]
    pub const fn visited_value_nodes(&self) -> u64;

    #[must_use]
    pub const fn staged_bytes(&self) -> u64;

    pub(crate) fn try_commit(
        self,
        store: &mut impl RuntimeOwnershipSlotStore,
    ) -> Result<
        RuntimeCommittedOwnershipTransaction,
        RuntimeTransferCommitError,
    >;

    pub(crate) fn abort(
        self,
        store: &mut impl RuntimeOwnershipSlotStore,
        reason: RuntimeTransferAbortReason,
    ) -> RuntimeAbortedOwnershipTransaction;
}
```

### 12.4 Commit mismatch and aborted owner

**Exact target declarations:**

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTransferAbortReason {
    CallerCancelled,
    PrepareConsumerFailed,
    CommitMismatch,
    ExecutionReplacement,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeTransferCommitErrorKind {
    #[error("transaction execution is no longer active")]
    WrongExecution {
        expected: ExecutionInstanceId,
        actual: ExecutionInstanceId,
    },

    #[error("runtime slot {slot} disappeared before commit")]
    SlotMissing {
        slot: RuntimeOwnedSlotId,
    },

    #[error("runtime slot storage identity no longer matches {slot}")]
    SlotIdentityMismatch {
        slot: RuntimeOwnedSlotId,
    },

    #[error("runtime slot {slot} reservation changed")]
    ReservationMismatch {
        slot: RuntimeOwnedSlotId,
        expected: RuntimeOwnershipTransactionId,
        actual: Option<RuntimeOwnershipTransactionId>,
    },

    #[error("runtime slot {slot} revision changed")]
    RevisionMismatch {
        slot: RuntimeOwnedSlotId,
        expected: RuntimeSlotRevision,
        actual: RuntimeSlotRevision,
    },

    #[error("runtime slot {slot} occupancy changed")]
    OccupancyMismatch {
        slot: RuntimeOwnedSlotId,
        expected: RuntimeSlotStateKind,
        actual: RuntimeSlotStateKind,
    },

    #[error("runtime slot {slot} accepted type changed")]
    TypeMismatch {
        slot: RuntimeOwnedSlotId,
    },

    #[error("affine owner {owner:?} no longer matches the prepared value graph")]
    CommitOwnerMismatch {
        slot: RuntimeOwnedSlotId,
        owner: RuntimeAffineOwnerId,
        path: RuntimeValuePath,
    },
}

#[derive(Debug)]
pub struct RuntimeAbortedOwnershipTransaction {
    id: RuntimeOwnershipTransactionId,
    plan: RuntimeTransferPlan,
    prepared: Vec<RuntimePreparedTransfer>,
    reason: RuntimeTransferAbortReason,
}

#[derive(Debug, Error)]
#[error("{kind}")]
pub struct RuntimeTransferCommitError {
    kind: RuntimeTransferCommitErrorKind,
    aborted: RuntimeAbortedOwnershipTransaction,
}
```

**Exact inherent API:**

```rust
impl RuntimeTransferCommitErrorKind {
    #[must_use]
    pub const fn precedence_rank(&self) -> u8;
}

impl RuntimeAbortedOwnershipTransaction {
    #[must_use]
    pub const fn id(&self) -> RuntimeOwnershipTransactionId;

    #[must_use]
    pub fn plan(&self) -> &RuntimeTransferPlan;

    #[must_use]
    pub const fn reason(&self) -> RuntimeTransferAbortReason;

    pub fn into_plan(self) -> RuntimeTransferPlan;
}

impl RuntimeTransferCommitError {
    #[must_use]
    pub const fn kind(&self) -> &RuntimeTransferCommitErrorKind;

    #[must_use]
    pub const fn aborted(&self) -> &RuntimeAbortedOwnershipTransaction;

    pub fn into_parts(
        self,
    ) -> (
        RuntimeTransferCommitErrorKind,
        RuntimeAbortedOwnershipTransaction,
    );
}
```

No API exists on `RuntimeAbortedOwnershipTransaction` that can prepare or
commit it again.

### 12.5 Committed result

**Exact target declaration:**

```rust
#[derive(Debug)]
pub struct RuntimeCommittedOwnershipTransaction {
    id: RuntimeOwnershipTransactionId,
    copied: Vec<RuntimeCopiedValueEvidence>,
    moved: Vec<RuntimeMovedValueEvidence>,
    dropped: Vec<RuntimeDroppedValueEvidence>,
}
```

**Exact inherent API:**

```rust
impl RuntimeCommittedOwnershipTransaction {
    #[must_use]
    pub const fn id(&self) -> RuntimeOwnershipTransactionId;

    #[must_use]
    pub fn copied(&self) -> &[RuntimeCopiedValueEvidence];

    #[must_use]
    pub fn moved(&self) -> &[RuntimeMovedValueEvidence];

    #[must_use]
    pub fn dropped(&self) -> &[RuntimeDroppedValueEvidence];

    pub fn into_evidence(
        self,
    ) -> (
        Vec<RuntimeCopiedValueEvidence>,
        Vec<RuntimeMovedValueEvidence>,
        Vec<RuntimeDroppedValueEvidence>,
    );
}
```

## 13. Execution identity state and affine allocator

Owner: existing `arcweft_core::value::ownership` plus scalar IDs from
`arcweft_core::runtime_id`.

**Exact target declarations:**

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAffineOwnerAllocator {
    execution: ExecutionInstanceId,
    next: RuntimeIdCursor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExecutionIdentityState {
    execution: ExecutionInstanceId,
    next_occurrence: RuntimeIdCursor,
    next_local_slot: RuntimeIdCursor,
    next_ownership_transaction: RuntimeIdCursor,
    affine_owners: RuntimeAffineOwnerAllocator,
}

#[derive(Debug, Error)]
#[error("{source}")]
pub struct RuntimeOwnershipTransactionStartError {
    source: RuntimeIdExhausted,
    plan: RuntimeTransferPlan,
}
```

**Exact inherent API:**

```rust
impl RuntimeAffineOwnerAllocator {
    pub(crate) fn new(execution: ExecutionInstanceId) -> Self;

    pub(crate) fn from_restored(
        execution: ExecutionInstanceId,
        next: RuntimeIdCursor,
    ) -> Self;

    #[must_use]
    pub const fn execution(&self) -> ExecutionInstanceId;

    #[must_use]
    pub const fn cursor(&self) -> RuntimeIdCursor;

    pub(crate) fn allocate(
        &mut self,
    ) -> Result<RuntimeAffineOwnerId, RuntimeIdExhausted>;
}

impl RuntimeExecutionIdentityState {
    pub(crate) fn fresh(execution: ExecutionInstanceId) -> Self;

    pub(crate) fn from_restored(
        execution: ExecutionInstanceId,
        next_occurrence: RuntimeIdCursor,
        next_local_slot: RuntimeIdCursor,
        next_ownership_transaction: RuntimeIdCursor,
        next_affine_owner: RuntimeIdCursor,
    ) -> Self;

    #[must_use]
    pub const fn execution(&self) -> ExecutionInstanceId;

    #[must_use]
    pub const fn next_occurrence(&self) -> RuntimeIdCursor;

    #[must_use]
    pub const fn next_local_slot(&self) -> RuntimeIdCursor;

    #[must_use]
    pub const fn next_ownership_transaction(&self) -> RuntimeIdCursor;

    #[must_use]
    pub const fn next_affine_owner(&self) -> RuntimeIdCursor;

    pub(crate) fn allocate_scope(
        &mut self,
    ) -> Result<RuntimeScopeInstanceId, RuntimeIdExhausted>;

    pub(crate) fn allocate_closure(
        &mut self,
    ) -> Result<RuntimeClosureInstanceId, RuntimeIdExhausted>;

    pub(crate) fn allocate_fiber(
        &mut self,
    ) -> Result<RuntimeFiberInstanceId, RuntimeIdExhausted>;

    pub(crate) fn allocate_frame(
        &mut self,
    ) -> Result<RuntimeFrameInstanceId, RuntimeIdExhausted>;

    pub(crate) fn allocate_mailbox(
        &mut self,
    ) -> Result<RuntimeMailboxInstanceId, RuntimeIdExhausted>;

    pub(crate) fn allocate_child(
        &mut self,
    ) -> Result<RuntimeChildInstanceId, RuntimeIdExhausted>;

    pub(crate) fn allocate_transfer(
        &mut self,
    ) -> Result<RuntimeTransferInstanceId, RuntimeIdExhausted>;

    pub(crate) fn allocate_cleanup_scope(
        &mut self,
    ) -> Result<RuntimeCleanupScopeId, RuntimeIdExhausted>;

    pub(crate) fn allocate_local_slot(
        &mut self,
    ) -> Result<RuntimeLocalSlotId, RuntimeIdExhausted>;

    pub(crate) fn begin_transaction(
        &mut self,
        limits: RuntimeOwnershipLimits,
        plan: RuntimeTransferPlan,
    ) -> Result<
        RuntimeOwnershipTransaction,
        RuntimeOwnershipTransactionStartError,
    >;

    pub(crate) fn affine_owner_allocator(
        &mut self,
    ) -> &mut RuntimeAffineOwnerAllocator;
}

impl RuntimeOwnershipTransactionStartError {
    #[must_use]
    pub const fn source(&self) -> RuntimeIdExhausted;

    pub fn into_plan(self) -> RuntimeTransferPlan;
}
```

The shared occurrence cursor is consumed by all eight occurrence allocators in
the exact call order made by the accepted executor. It is not split into
per-domain counters at implementation time.

## 14. Runtime-driver execution domain

Owner: new `arcweft_runtime_driver::execution`, wrapping the existing
`BundleSession`. No second runtime executor/session model is introduced.

### 14.1 Epoch and ephemeral reservation identity

**Exact target declarations:**

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeActivationEpoch(NonZeroU64);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RuntimeExecutionReservationId(NonZeroU64);

impl RuntimeActivationEpoch {
    #[must_use]
    pub const fn initial() -> Self;

    #[must_use]
    pub const fn get(self) -> NonZeroU64;

    pub(crate) fn checked_next(
        self,
    ) -> Result<Self, RuntimeActivationEpochExhausted>;
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
#[error("runtime activation epoch is exhausted")]
pub struct RuntimeActivationEpochExhausted;
```

`RuntimeActivationEpoch` implements manual strict `Serialize`/`Deserialize`
using the same nonzero canonical decimal-string JSON and little-endian scalar
rules as the other persisted `NonZeroU64` identities. It has no public raw
constructor.

`RuntimeExecutionReservationId` is process-local and is not serialized or
rendered as language/runtime identity. It is allocated monotonically only to
bind the linear reservation object to the shared domain record.

### 14.2 Source, mode, and domain state

**Exact target declarations:**

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeExecutionSource {
    New,
    Restore,
    Replay,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeActivationMode {
    Empty,
    Replace {
        expected_epoch: RuntimeActivationEpoch,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RuntimeExecutionReservationRecord {
    reservation: RuntimeExecutionReservationId,
    execution: ExecutionInstanceId,
    mode: RuntimeActivationMode,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RuntimeActiveExecutionRecord {
    execution: ExecutionInstanceId,
    epoch: RuntimeActivationEpoch,
}

#[derive(Debug)]
pub(crate) struct RuntimeExecutionDomainState {
    next_execution: RuntimeIdCursor,
    next_reservation: RuntimeIdCursor,
    reservation: Option<RuntimeExecutionReservationRecord>,
    active: Option<RuntimeActiveExecutionRecord>,
}

#[derive(Debug)]
pub struct RuntimeExecutionDomain {
    state: Mutex<RuntimeExecutionDomainState>,
}
```

`RuntimeExecutionDomain::new_for_runtime_host` is crate-private. The runtime
host creates exactly one `Arc<RuntimeExecutionDomain>` and injects it into every
driver it creates.

### 14.3 Dormant input and reservation

**Exact target declarations:**

```rust
#[derive(Debug)]
pub(crate) struct RuntimeDormantExecution {
    identity: RuntimeExecutionIdentityState,
    activation_epoch: RuntimeActivationEpoch,
    source: RuntimeExecutionSource,
    mode: RuntimeActivationMode,
    session: BundleSession,
}

#[derive(Debug)]
pub(crate) enum RuntimeFreshExecutionInput {
    New(BundleSession),
    Preserved(RuntimeDormantExecution),
}

#[derive(Debug)]
pub struct RuntimeExecutionReservation {
    domain: Arc<RuntimeExecutionDomain>,
    reservation: RuntimeExecutionReservationId,
    execution: ExecutionInstanceId,
    mode: RuntimeActivationMode,
    armed: bool,
}

#[derive(Debug)]
pub struct RuntimeFreshExecution {
    execution: ExecutionInstanceId,
    identity: RuntimeExecutionIdentityState,
    activation_epoch: RuntimeActivationEpoch,
    source: RuntimeExecutionSource,
    mode: RuntimeActivationMode,
    session: BundleSession,
    reservation: RuntimeExecutionReservation,
}
```

`RuntimeExecutionReservation` and `RuntimeFreshExecution` implement neither
`Clone` nor serialization. `RuntimeExecutionReservation::drop` clears only the
matching reservation record when `armed` remains true.

### 14.4 Fresh-construction errors

**Exact target declarations:**

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeFreshExecutionErrorKind {
    #[error("another execution reservation is active")]
    ReservationBusy,

    #[error("empty activation was requested while execution {active} is active")]
    ActiveExecutionExists {
        active: ExecutionInstanceId,
    },

    #[error("replacement activation requires an active execution")]
    ReplacementTargetMissing,

    #[error("replacement expected execution {expected}, found {actual}")]
    ReplacementTargetMismatch {
        expected: ExecutionInstanceId,
        actual: ExecutionInstanceId,
    },

    #[error("replacement expected activation epoch {expected:?}, found {actual:?}")]
    StaleActivationEpoch {
        expected: RuntimeActivationEpoch,
        actual: RuntimeActivationEpoch,
    },

    #[error("execution identity {execution} was already issued by this domain")]
    ExecutionIdentityCollision {
        execution: ExecutionInstanceId,
    },

    #[error("restored next-execution cursor regresses the runtime domain")]
    NextExecutionRegression,

    #[error("restored identity state belongs to a different execution")]
    IdentityExecutionMismatch {
        expected: ExecutionInstanceId,
        actual: ExecutionInstanceId,
    },

    #[error("{namespace:?} identity allocation is exhausted")]
    IdentityExhausted {
        namespace: RuntimeIdNamespace,
    },

    #[error("runtime execution domain mutex is poisoned")]
    DomainPoisoned,
}

#[derive(Debug, Error)]
#[error("{kind}")]
pub struct RuntimeFreshExecutionError {
    kind: RuntimeFreshExecutionErrorKind,
    input: RuntimeFreshExecutionInput,
}
```

**Exact inherent API:**

```rust
impl RuntimeFreshExecutionError {
    #[must_use]
    pub const fn kind(&self) -> &RuntimeFreshExecutionErrorKind;

    pub(crate) fn into_parts(
        self,
    ) -> (
        RuntimeFreshExecutionErrorKind,
        RuntimeFreshExecutionInput,
    );
}
```

### 14.5 Domain preparation API

**Exact target API:**

```rust
impl RuntimeExecutionDomain {
    pub(crate) fn new_for_runtime_host() -> Arc<Self>;

    pub(crate) fn prepare_new(
        self: &Arc<Self>,
        session: BundleSession,
    ) -> Result<RuntimeFreshExecution, RuntimeFreshExecutionError>;

    pub(crate) fn prepare_restored_empty(
        self: &Arc<Self>,
        snapshot: RuntimeExecutionDomainSnapshotV2,
        session: BundleSession,
    ) -> Result<RuntimeFreshExecution, RuntimeFreshExecutionError>;

    pub(crate) fn prepare_replay_empty(
        self: &Arc<Self>,
        snapshot: RuntimeExecutionDomainSnapshotV2,
        session: BundleSession,
    ) -> Result<RuntimeFreshExecution, RuntimeFreshExecutionError>;

    pub(crate) fn prepare_restored_replacement(
        self: &Arc<Self>,
        snapshot: RuntimeExecutionDomainSnapshotV2,
        expected_epoch: RuntimeActivationEpoch,
        session: BundleSession,
    ) -> Result<RuntimeFreshExecution, RuntimeFreshExecutionError>;

    pub(crate) fn prepare_replay_replacement(
        self: &Arc<Self>,
        snapshot: RuntimeExecutionDomainSnapshotV2,
        expected_epoch: RuntimeActivationEpoch,
        session: BundleSession,
    ) -> Result<RuntimeFreshExecution, RuntimeFreshExecutionError>;
}

impl RuntimeFreshExecution {
    #[must_use]
    pub const fn execution(&self) -> ExecutionInstanceId;

    #[must_use]
    pub const fn identity(&self) -> &RuntimeExecutionIdentityState;

    #[must_use]
    pub const fn activation_epoch(&self) -> RuntimeActivationEpoch;

    #[must_use]
    pub const fn source(&self) -> RuntimeExecutionSource;

    #[must_use]
    pub const fn mode(&self) -> RuntimeActivationMode;

    #[must_use]
    pub(crate) const fn session(&self) -> &BundleSession;

    pub(crate) fn session_mut(&mut self) -> &mut BundleSession;
}
```

`prepare_new` checks that the domain can reserve an empty activation before
minting the ID and sets `activation_epoch = RuntimeActivationEpoch::initial()`.
A successful reservation consumes the execution ID even if later candidate
construction is cancelled. Failed reservation/lock/exhaustion returns the
original session.

Preserved preparation accepts only a previously validated domain snapshot.
The driver consumes its core-owned active identity snapshot into
`RuntimeExecutionIdentityState`, carries the envelope epoch separately, and
never makes `arcweft-core` depend on the driver.
For empty restore/replay, the domain's current next-execution cursor must not
have passed the preserved execution ID. Gaps may be skipped; after reservation
the domain adopts the greater validated next-execution cursor and the fresh
candidate retains the envelope epoch. Replacement requires the exact active
execution ID and expected epoch, and the envelope epoch must equal that active
epoch; only `RuntimeActiveExecution::replace` computes and publishes the checked
successor epoch.

### 14.6 Active owner and activation errors

**Exact target declarations:**

```rust
#[derive(Debug)]
pub struct RuntimeActiveExecution {
    domain: Arc<RuntimeExecutionDomain>,
    execution: ExecutionInstanceId,
    epoch: RuntimeActivationEpoch,
    identity: RuntimeExecutionIdentityState,
    session: BundleSession,
    armed: bool,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeActivationErrorKind {
    #[error("runtime execution reservation is missing")]
    ReservationMissing,

    #[error("runtime execution reservation does not match the candidate")]
    ReservationMismatch,

    #[error("empty activation requires an empty runtime domain")]
    EmptyDomainRequired,

    #[error("candidate identity state belongs to the wrong execution")]
    IdentityExecutionMismatch,

    #[error("candidate identity cursor regresses the domain")]
    CursorRegression,

    #[error("runtime execution domain mutex is poisoned")]
    DomainPoisoned,
}

#[derive(Debug, Error)]
#[error("{kind}")]
pub struct RuntimeActivationError {
    kind: RuntimeActivationErrorKind,
    fresh: RuntimeFreshExecution,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeReplacementErrorKind {
    #[error("active and replacement candidates belong to different domains")]
    WrongDomain,

    #[error("replacement candidate execution does not match the active execution")]
    WrongExecution,

    #[error("replacement activation epoch is stale")]
    StaleEpoch,

    #[error("replacement reservation does not match the candidate")]
    ReservationMismatch,

    #[error("replacement identity cursor regresses active state")]
    CursorRegression,

    #[error("runtime activation epoch is exhausted")]
    EpochExhausted,

    #[error("runtime execution domain mutex is poisoned")]
    DomainPoisoned,
}

#[derive(Debug, Error)]
#[error("{kind}")]
pub struct RuntimeReplacementError {
    kind: RuntimeReplacementErrorKind,
    active: RuntimeActiveExecution,
    fresh: RuntimeFreshExecution,
}
```

**Exact inherent API:**

```rust
impl RuntimeFreshExecution {
    pub(crate) fn activate_empty(
        self,
    ) -> Result<RuntimeActiveExecution, RuntimeActivationError>;
}

impl RuntimeActiveExecution {
    #[must_use]
    pub const fn execution(&self) -> ExecutionInstanceId;

    #[must_use]
    pub const fn epoch(&self) -> RuntimeActivationEpoch;

    #[must_use]
    pub const fn identity(&self) -> &RuntimeExecutionIdentityState;

    #[must_use]
    pub(crate) const fn session(&self) -> &BundleSession;

    pub(crate) fn session_mut(&mut self) -> &mut BundleSession;

    pub(crate) fn replace(
        self,
        fresh: RuntimeFreshExecution,
    ) -> Result<RuntimeActiveExecution, RuntimeReplacementError>;

    pub(crate) fn deactivate(self) -> BundleSession;
}

impl RuntimeActivationError {
    #[must_use]
    pub const fn kind(&self) -> &RuntimeActivationErrorKind;

    pub fn into_parts(
        self,
    ) -> (RuntimeActivationErrorKind, RuntimeFreshExecution);
}

impl RuntimeReplacementError {
    #[must_use]
    pub const fn kind(&self) -> &RuntimeReplacementErrorKind;

    pub fn into_parts(
        self,
    ) -> (
        RuntimeReplacementErrorKind,
        RuntimeActiveExecution,
        RuntimeFreshExecution,
    );
}
```

`RuntimeActiveExecution` implements `Drop`; while armed, it releases only its
matching active record. Existing driver methods that execute, save, restore,
replay, or replace a session delegate through this owner. A raw active
`BundleSession` is no longer publicly constructible or independently runnable.

## 15. Snapshot identity owners and floating snapshot correction

Owner of `RuntimeExecutionIdentitySnapshotV2` and floating bit wrappers:
`arcweft_core::value::ownership`.  
Owner of `RuntimeActivationEpoch` and `RuntimeExecutionDomainSnapshotV2`:
`arcweft_runtime_driver::execution`. The core snapshot contains no driver type.

### 15.1 Exact identity snapshots

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExecutionIdentitySnapshotV2 {
    execution: ExecutionInstanceId,
    next_occurrence: RuntimeIdCursor,
    next_local_slot: RuntimeIdCursor,
    next_ownership_transaction: RuntimeIdCursor,
    next_affine_owner: RuntimeIdCursor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExecutionDomainSnapshotV2 {
    next_execution: RuntimeIdCursor,
    activation_epoch: RuntimeActivationEpoch,
    active: RuntimeExecutionIdentitySnapshotV2,
}
```

Both implement manual `Serialize`/`Deserialize` and the canonical binary codec.
Fields encode in declaration order. `RuntimeExecutionIdentitySnapshotV2` is
core-owned and has no `RuntimeActivationEpoch` field; the driver-owned domain
envelope carries that epoch. `into_identity_state` exposes no allocator or
activation ability to external callers: all mutating allocator methods and all
domain activation methods remain crate-private in their owning crates.

**Exact inherent API:**

```rust
impl RuntimeExecutionIdentitySnapshotV2 {
    #[must_use]
    pub const fn execution(&self) -> ExecutionInstanceId;

    #[must_use]
    pub const fn next_occurrence(&self) -> RuntimeIdCursor;

    #[must_use]
    pub const fn next_local_slot(&self) -> RuntimeIdCursor;

    #[must_use]
    pub const fn next_ownership_transaction(&self) -> RuntimeIdCursor;

    #[must_use]
    pub const fn next_affine_owner(&self) -> RuntimeIdCursor;

    pub fn into_identity_state(self) -> RuntimeExecutionIdentityState;
}

impl RuntimeExecutionDomainSnapshotV2 {
    #[must_use]
    pub const fn next_execution(&self) -> RuntimeIdCursor;

    #[must_use]
    pub const fn activation_epoch(&self) -> RuntimeActivationEpoch;

    #[must_use]
    pub const fn active(&self) -> &RuntimeExecutionIdentitySnapshotV2;
}
```

### 15.2 Floating bits

**Exact new wrappers:**

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeSnapshotF32Bits(u32);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeSnapshotF64Bits(u64);

impl RuntimeSnapshotF32Bits {
    #[must_use]
    pub const fn from_value(value: f32) -> Self;

    #[must_use]
    pub const fn bits(self) -> u32;

    #[must_use]
    pub const fn into_value(self) -> f32;
}

impl RuntimeSnapshotF64Bits {
    #[must_use]
    pub const fn from_value(value: f64) -> Self;

    #[must_use]
    pub const fn bits(self) -> u64;

    #[must_use]
    pub const fn into_value(self) -> f64;
}
```

The parent-owned `RuntimeValueSnapshotV2` uses these wrappers in its f32/f64
variants. Its exact trait row is:

```text
Clone, Debug, PartialEq, Serialize, Deserialize
NOT Eq
NOT Hash
```

No other parent variant, tag, or field order changes in this correction.

### 15.3 Save blocker delta on the existing enum

The existing save-blocker enum gains this variant in its owning declaration:

```rust
OwnershipTransactionActive {
    count: NonZeroU32,
}
```

The owning enum's existing inherent diagnostic rendering gains the corresponding
arm. No second blocker enum or free helper is added.

## 16. Visibility and construction summary

| Symbol family | Raw constructor visibility | Serialized | Publicly mintable in G1.2 |
|---|---|---:|---:|
| `ExecutionInstanceId` | domain-private | yes | no |
| occurrence IDs | core-private allocator | yes | no |
| `RuntimeLocalSlotId` | core-private allocator | yes | no |
| static local/capture IDs | plan-validator private | plan/save where carried | no raw |
| owner-local lane/packet IDs | owning aggregate private | yes | no raw |
| `RuntimeRecordFieldId` | record-validator private | yes | no raw |
| `RuntimeSlotRevision` | slot-cell private | yes | no raw |
| `RuntimeAffineOwnerId` | affine allocator private | yes | no token |
| transaction ID | execution identity state private | evidence/save | no raw |
| reservation ID | driver-domain private | no | no |
| activation epoch | driver-domain private | yes | no raw |

No type above has a public “for test”, “unchecked”, “from raw”, random, pointer,
name, span, or debug-string constructor.
