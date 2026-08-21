# Rust-shaped owners and APIs

The declarations below are normative API shapes, not a production overlay.
Names are assigned to existing owning modules; implementation may split a
large module into ordinary child files while keeping these ownership
boundaries.

## 1. Final semantic type surface

Owner: `arcweft-lang-sema::types`.

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum StageActorHandleType {
    Exact(CharacterId),
    Any,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TypeKind {
    // existing variants ...

    /// Compile-time capability projected from one exact Character.
    /// It has no RuntimeValue projection.
    StageApi(CharacterId),

    /// Compile-time capability available only in a line activation/action.
    /// It has no RuntimeValue projection.
    LineContext,

    /// Affine exact opaque runtime handle.
    StageActorHandle(StageActorHandleType),
    CueHandle,
    VoiceHandle,
}
```

`CharacterLook<Character>` remains the existing Character-owned exact type and
projects to the existing entity-reference runtime family.  It does not become
an opaque handle.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LineContextMethodId {
    VoiceHandle,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LineScheduleCallableId {
    At,
}

impl LineContextMethodId {
    pub(crate) fn resolve(
        receiver: &TypeKind,
        method: &CallableName,
        arity: usize,
    ) -> Option<Self> {
        matches!((receiver, method.as_str(), arity),
            (TypeKind::LineContext, "voice_handle", 0)
        ).then_some(Self::VoiceHandle)
    }

    pub(crate) fn signature_schema(self) -> CallableSignatureSchema {
        empty(
            TypeKind::VoiceHandle,
            &["dialogue.voice"],
            CallableValidator::LineContext(self),
        )
    }
}
```

The `LineContext` branch is deleted from `CapacityMethodId::resolve`,
`CapacityMethodId::result_type`, and `CapacityMethodId::signature_schema`.
`StageMethodId::{Acquire, Look}` remains the original stage enum and gains its
runtime mapping on that enum's implementation rather than via a helper trait.

```rust
impl StageMethodId {
    pub(crate) fn runtime_line_operation(
        self,
        checked: &CheckedCall,
        context: &LineLoweringContext<'_>,
    ) -> Result<RuntimeLineOperation, RuntimePlanLowerError>;
}

impl LineContextMethodId {
    pub(crate) fn runtime_line_operation(
        self,
        checked: &CheckedCall,
        context: &LineLoweringContext<'_>,
    ) -> Result<RuntimeLineOperation, RuntimePlanLowerError>;
}

impl LineScheduleCallableId {
    pub(crate) fn runtime_line_operation(
        self,
        checked: &CheckedCall,
        context: &LineLoweringContext<'_>,
    ) -> Result<RuntimeLineOperation, RuntimePlanLowerError>;
}
```

## 2. Existing opaque value owner extended in place

Owner: `arcweft-core::value::opaque` and
`arcweft-core::pattern::RuntimeOpaqueTypeOwner`.

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq,
         PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeHandleKind {
    StageActor,
    Cue,
    Voice,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq,
         PartialOrd, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RuntimeOpaqueValueClass {
    Plain,
    AffineHandle(RuntimeHandleKind),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq,
         PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOpaquePersistence {
    ConstantAndSnapshot,
    SnapshotOnly,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RuntimeOpaqueTypeOwner {
    producer: RuntimeOpaqueTypeProducerId,
    semantic_identity: RuntimeSemanticTypeId,
    admission: RuntimeOpaqueTypeAdmission,
    value_class: RuntimeOpaqueValueClass,
    persistence: RuntimeOpaquePersistence,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeOpaqueValue {
    producer: RuntimeOpaqueTypeProducerId,
    semantic_identity: RuntimeSemanticTypeId,
    value_class: RuntimeOpaqueValueClass,
    persistence: RuntimeOpaquePersistence,
    payload: Box<RuntimeValue>,
}
```

Construction remains owner-only:

```rust
impl RuntimeOpaqueTypeOwner {
    pub fn try_wrap(
        &self,
        payload: RuntimeValue,
    ) -> Result<RuntimeOpaqueValue, RuntimeOpaqueValueError> {
        if self.admission != RuntimeOpaqueTypeAdmission::ExactIdentity {
            return Err(RuntimeOpaqueValueError::NonExactOwner);
        }
        Ok(RuntimeOpaqueValue {
            producer: self.producer.clone(),
            semantic_identity: self.semantic_identity,
            value_class: self.value_class,
            persistence: self.persistence,
            payload: Box::new(payload),
        })
    }

    pub fn accepts(&self, value: &RuntimeOpaqueValue) -> bool {
        self.producer == value.producer
            && self.value_class == value.value_class
            && self.persistence == value.persistence
            && match self.admission {
                RuntimeOpaqueTypeAdmission::ExactIdentity =>
                    self.semantic_identity == value.semantic_identity,
                RuntimeOpaqueTypeAdmission::ProducerWide => true,
            }
    }
}
```

The existing exhaustive ownership implementation is changed directly:

```rust
impl RuntimeValue {
    pub fn ownership(&self) -> RuntimeValueOwnership {
        match self {
            // existing exhaustive cases ...
            Self::Opaque(value) => match value.value_class() {
                RuntimeOpaqueValueClass::Plain => value.payload().ownership(),
                RuntimeOpaqueValueClass::AffineHandle(_) =>
                    RuntimeValueOwnership::Affine
                        .join(value.payload().ownership()),
            },
        }
    }
}
```

`RuntimeOpaquePersistence::SnapshotOnly` is rejected by constant-pool,
constant-expression, bundle-literal, and AWBC constant admission.  It is
accepted only inside a validated live runtime snapshot.

### Exact producers

```text
std.line.stage_actor_handle
std.line.cue_handle
std.line.voice_handle
```

- `StageActorHandle<Exact(C)>` has one exact semantic identity derived from
  producer id, exact `CharacterId`, value class, and persistence.
- `StageActorHandle<Any>` is a producer-wide checked owner usable for storage,
  move, drop, and equality only.  `look` requires an exact Character owner.
- `CueHandle` and `VoiceHandle` each have one exact semantic identity; cue kind
  and voice session are token-ledger state, not alternate source types.

No source name or debug string is in the producer identity or payload grammar.

## 3. Stable identity newtypes

Owner: `arcweft-core::runtime_id`.

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq,
         PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimePersistentFiberId(u64);

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq,
         PartialOrd, Serialize)]
pub struct DialogueActivationId {
    artifact: RuntimeArtifactFingerprint,
    owner_fiber: RuntimePersistentFiberId,
    content: RuntimeDialogueContentPlanId,
    occurrence: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq,
         PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeLineHandleSiteId(u32);

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq,
         PartialOrd, Serialize)]
pub struct RuntimeLineHandleToken {
    activation: DialogueActivationId,
    site: RuntimeLineHandleSiteId,
    issuance: u32,
}
```

The opaque payload is the exact existing `RuntimeValue::NominalRecord` for the
internal nominal `std.runtime.LineHandleTokenV1`, in field order:

```text
artifact_fingerprint : Bytes[32]
owner_fiber           : U64
content_plan          : U32
occurrence            : U64
handle_site           : U32
issuance              : U32
```

`RuntimeLineHandleToken::encode_payload` and `try_decode_payload` are methods on
that newtype.  There is no second runtime value algebra and no alternate
string form.

## 4. Handle-site and ledger authority

Owner: `arcweft-core::line_task`.

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeLineHandleSite {
    id: RuntimeLineHandleSiteId,
    source_ordinal: u32,
    kind: RuntimeHandleKind,
    result_type: RuntimePlanTypeId,
    character: Option<CharacterId>,
    scheduled_child: Option<RuntimeLineTaskNodeId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeHandleOwnerSlot {
    LineScope,
    ActivationLocal(RuntimeOwnedSlotId),
    ChildScope(LineTaskWorkTag),
    DialogueResult(RuntimeValuePath),
    ParentFiber(RuntimeOwnedSlotId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHandleLeaseState {
    Allocating,
    Active,
    Pending,
    Running,
    Completed,
    Cancelling,
    Cancelled,
    Failed,
    Released,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeHandleResource {
    StageActor(RuntimeStageActorLease),
    Cue(RuntimeCueLease),
    Voice(RuntimeVoiceLease),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeHandleLease {
    token: RuntimeLineHandleToken,
    owner: RuntimeHandleOwnerSlot,
    state: RuntimeHandleLeaseState,
    resource: RuntimeHandleResource,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeLineHandleLedger {
    issuance_by_site: BTreeMap<RuntimeLineHandleSiteId, u32>,
    leases: BTreeMap<RuntimeLineHandleToken, RuntimeHandleLease>,
}
```

Required methods are on the owner, not a helper trait:

```rust
impl RuntimeLineHandleLedger {
    pub(crate) fn issue(
        &mut self,
        activation: &DialogueActivationId,
        site: &RuntimeLineHandleSite,
        resource: RuntimeHandleResource,
        owner: RuntimeHandleOwnerSlot,
    ) -> Result<RuntimeOpaqueValue, LineRuntimeError>;

    pub(crate) fn transfer(
        &mut self,
        token: &RuntimeLineHandleToken,
        expected: &RuntimeHandleOwnerSlot,
        destination: RuntimeHandleOwnerSlot,
    ) -> Result<(), LineRuntimeError>;

    pub(crate) fn drop_owned(
        &mut self,
        token: &RuntimeLineHandleToken,
        expected: &RuntimeHandleOwnerSlot,
        commands: &mut RuntimeCommandQueue,
    ) -> Result<(), LineRuntimeError>;

    pub(crate) fn validate_value(
        &self,
        value: &RuntimeOpaqueValue,
        expected_kind: RuntimeHandleKind,
        activation: &DialogueActivationId,
    ) -> Result<&RuntimeHandleLease, LineRuntimeError>;
}
```

## 5. Final RuntimePlan operation surface

Owner: the existing `arcweft-core::plan::FlowOp` enum and the existing
`arcweft-core::line_task::LineTaskGroup`.

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeLineHandleScope {
    Line,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeLineOperation {
    AcquireActor {
        site: RuntimeLineHandleSiteId,
        character: CharacterId,
        scope: RuntimeLineHandleScope,
    },
    Schedule {
        site: RuntimeLineHandleSiteId,
        delay: RuntimeExpr,
        child: RuntimeLineTaskNodeId,
        captures: Box<[RuntimeExpr]>,
    },
    ActorLook {
        site: RuntimeLineHandleSiteId,
        actor: RuntimeExpr,
        look: RuntimeExpr,
        crossfade: RuntimeExpr,
    },
    VoiceHandle {
        site: RuntimeLineHandleSiteId,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDialogueResultTarget {
    ty: RuntimePlanTypeId,
    pattern: RuntimePattern,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FlowOp {
    // existing variants ...
    LineOperation {
        /// `None` means the handle remains owned by the current implicit scope.
        /// `Some(Discard)` is explicit `_` and drops after successful binding.
        binding: Option<RuntimePattern>,
        operation: RuntimeLineOperation,
    },
    CommitDialogueResult {
        value: RuntimeExpr,
    },
    Dialogue {
        content: RuntimeDialogueContentPlanId,
        result: RuntimeDialogueResultTarget,
    },
}
```

`LineTaskGroup` is extended directly:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct LineTaskGroup {
    captures: Box<[RuntimeLocalDeclarationId]>,
    activation_ops: Box<[FlowOp]>,
    result_type: RuntimePlanTypeId,
    handle_sites: Box<[RuntimeLineHandleSite]>,
    root: RuntimeLineTaskNodeId,
    nodes: Box<[LineTaskNode]>,
    cancel_rules: Box<[LineCancelRule]>,
    cleanup: LineTaskCleanup,
}
```

Scheduled children retain captured runtime values per issued cue:

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum LineTaskTrigger {
    Immediate,
    Mark(RuntimeDialogueMarkId),
    Scheduled(RuntimeLineHandleSiteId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeScheduledLineTask {
    token: RuntimeLineHandleToken,
    node: RuntimeLineTaskNodeId,
    deadline: LogicalDuration,
    captures: Box<[RuntimeValue]>,
    state: RuntimeScheduledState,
}
```

The old constant-only `Delay(LogicalDuration)` trigger is replaced for authored
`at`; there is one schema and no old reader.  Other maintained delayed-runtime
features must lower through the same scheduling operation rather than revive a
synthetic delay node.

## 6. Dialogue state and result cell

Owner: `arcweft-core::engine::DialogueState`.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogueRuntimePhase {
    HostPreparing,
    Activating,
    Ready,
    Closing,
    Publishing,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DialogueResultState {
    Uncommitted,
    Committed {
        ty: RuntimePlanTypeId,
        value: RuntimeValue,
    },
    Published,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DialogueState {
    // existing line/content/task/resume/capture/elapsed fields ...
    activation: DialogueActivationId,
    generation: RuntimeArtifactFingerprint,
    phase: DialogueRuntimePhase,
    result_target: RuntimeDialogueResultTarget,
    result: DialogueResultState,
    handle_ledger: RuntimeLineHandleLedger,
    command_sequence: u64,
}
```

```rust
impl DialogueState {
    pub(crate) fn commit_result(
        &mut self,
        value: RuntimeValue,
        types: &RuntimeTypeRegistry,
    ) -> Result<(), LineRuntimeError>;

    pub(crate) fn publish_result(
        &mut self,
        parent: &mut FlowFiber,
        commands: &mut RuntimeCommandQueue,
    ) -> Result<(), LineRuntimeError>;

    pub(crate) fn abandon_result(
        &mut self,
        commands: &mut RuntimeCommandQueue,
    ) -> Result<(), LineRuntimeError>;
}
```

`commit_result` validates `R`, transfers all affine leaves to
`DialogueResult(path)`, and changes `Uncommitted -> Committed` atomically.
`publish_result` first simulates the full pattern and all affine transfers;
only then does it mutate parent locals, drop explicit discards, and set
`Published`.

## 7. Typed Sans-I/O command owner

Owner: `arcweft-core::presentation`.

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq,
         PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeStageCommandId(u64);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeStageCommand {
    AcquireActor {
        command: RuntimeStageCommandId,
        activation: DialogueActivationId,
        actor: RuntimeLineHandleToken,
        character: CharacterId,
        scope: RuntimeLineHandleScope,
    },
    SetCharacterLook {
        command: RuntimeStageCommandId,
        activation: DialogueActivationId,
        cue: RuntimeLineHandleToken,
        actor: RuntimeLineHandleToken,
        character: CharacterId,
        look: CharacterLookId,
        crossfade: LogicalDuration,
    },
    ReleaseActor {
        command: RuntimeStageCommandId,
        activation: DialogueActivationId,
        actor: RuntimeLineHandleToken,
    },
    CancelCue {
        command: RuntimeStageCommandId,
        activation: DialogueActivationId,
        cue: RuntimeLineHandleToken,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RuntimeStageCommandOutcome {
    Acquired {
        command: RuntimeStageCommandId,
        actor: RuntimeLineHandleToken,
    },
    Accepted {
        command: RuntimeStageCommandId,
        cue: RuntimeLineHandleToken,
    },
    Completed {
        command: RuntimeStageCommandId,
        cue: RuntimeLineHandleToken,
    },
    Cancelled {
        command: RuntimeStageCommandId,
        cue: RuntimeLineHandleToken,
    },
    Rejected {
        command: RuntimeStageCommandId,
        code: RuntimeStageRejectCode,
    },
}
```

The host boundary accepts and returns these exact types.  Renderer-local object
ids remain host-private and are mapped by the host against the echoed token.

## 8. Voice lifecycle owner

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeDialogueVoiceState {
    Absent,
    Lazy(RuntimeVoiceStartTicket),
    Ready(RuntimeVoiceSessionId),
    Failed(RuntimeVoiceFailure),
    Completed(RuntimeVoiceSessionId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeVoiceLease {
    session: RuntimeVoiceSessionId,
    lease_ordinal: u32,
    stop_on_last_release: bool,
}
```

`RuntimeLineOperation::VoiceHandle` is interpreted only while a matching
`DialogueState` is `Activating` or `Ready`.  A lazy start is a typed suspending
host request owned by the dialogue activation; it is not an ordinary pure
call.

## 9. Structured error surface

```rust
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimePlanLowerError {
    #[error("line capability has no runtime value projection: {capability:?}")]
    NonValueLineCapability { capability: LineCapabilityKind },
    #[error("missing exact opaque producer for {kind:?}")]
    MissingLineHandleProducer { kind: RuntimeHandleKind },
    #[error("line handle site {site:?} is duplicated")]
    DuplicateLineHandleSite { site: RuntimeLineHandleSiteId },
    #[error("scheduled site {site:?} does not own child {child:?}")]
    InvalidScheduledChild {
        site: RuntimeLineHandleSiteId,
        child: RuntimeLineTaskNodeId,
    },
    #[error("dialogue result path may complete without one commit")]
    MissingDialogueResult,
    #[error("dialogue result path commits more than once")]
    DuplicateDialogueResult,
    #[error("dialogue result type does not match the dialogue target")]
    DialogueResultTypeMismatch,
    #[error("line-plan limit exceeded: {kind:?}, {actual} > {limit}")]
    LineLimitExceeded {
        kind: LineLimitKind,
        actual: usize,
        limit: usize,
    },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LineRuntimeError {
    #[error("line handle has the wrong opaque producer")]
    WrongOpaqueProducer,
    #[error("line handle generation is stale")]
    StaleGeneration,
    #[error("line handle belongs to a different activation")]
    WrongActivation,
    #[error("stage actor belongs to a different Character")]
    WrongActorCharacter,
    #[error("look belongs to a different Character")]
    WrongLookOwner,
    #[error("active dialogue has no voice")]
    MissingActiveVoice,
    #[error("dialogue voice start was rejected")]
    VoiceStartRejected,
    #[error("cue delay is negative")]
    NegativeCueDelay,
    #[error("cue delay or deadline overflowed")]
    CueDeadlineOverflow,
    #[error("line handle issuance ordinal overflowed")]
    HandleIssuanceOverflow,
    #[error("dialogue result was not committed")]
    ResultNotCommitted,
    #[error("dialogue result was committed twice")]
    ResultAlreadyCommitted,
    #[error("dialogue result failed its exact type or pattern")]
    ResultPatternOrTypeMismatch,
    #[error("host rejected a stage command: {code:?}")]
    StageCommandRejected { code: RuntimeStageRejectCode },
}
```
