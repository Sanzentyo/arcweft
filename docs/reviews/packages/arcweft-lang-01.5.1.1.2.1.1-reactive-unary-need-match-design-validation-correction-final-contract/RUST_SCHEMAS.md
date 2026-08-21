# Exact Rust-shaped schemas

Names and placement are normative unless current visibility requires narrowing.
Fields, equality domains, canonical order, dependency direction, and behavior are
not optional. Public wire DTOs use `Serialize`, `Deserialize`,
`#[serde(deny_unknown_fields)]`, and explicit snake-case tags. Session HIR IDs
never derive Serde.

## Checked semantic facts

```rust
// arcweft-lang-sema::final_analysis::view
#[derive(Clone, Debug)]
pub struct CheckedViewCatalog {
    generation: CheckedViewGeneration,
    definitions: BTreeMap<ItemId, CheckedViewDefinition>,
    nodes: BTreeMap<CheckedViewNodeKey, CheckedViewNode>,
    need_matches: BTreeMap<CheckedViewNodeKey, CheckedViewNeedMatch>,
    need_subscriptions:
        BTreeMap<CheckedViewNeedSubscriptionKey, CheckedViewNeedSubscription>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedViewNeedSubscriptionKey {
    owner: ItemId,
    scrutinee: ExprId,
}

#[derive(Clone, Debug)]
pub struct CheckedViewNeedMatch {
    key: CheckedViewNodeKey,
    owner: ItemId,
    match_expression: ExprId,
    scrutinee: ExprId,
    subscription: CheckedViewNeedSubscriptionKey,
    need_type: TypeId,
    payload_type: TypeId,
    arms: Box<[CheckedViewNeedMatchArm]>,
    exhaustiveness: CheckedMatchExhaustiveness,
    effects: EffectSet,
    source: CheckedViewSourceRole,
    ownership: CheckedViewOwnershipDisposition,
}

#[derive(Clone, Debug)]
pub struct CheckedViewNeedMatchArm {
    ordinal: u16,
    arm_expression: ExprId,
    pattern: PatternId,
    guard: Option<ExprId>,
    bindings: Box<[CheckedViewMatchBinding]>,
    body_root: ExprId,
    source: CheckedViewSourceRole,
}

#[derive(Clone, Debug)]
pub struct CheckedViewMatchBinding {
    local: LocalId,
    pattern: PatternId,
    ty: TypeId,
    ownership: CheckedBindingOwnership,
    source: CheckedViewSourceRole,
}

#[derive(Clone, Debug)]
pub struct CheckedViewNeedSubscription {
    key: CheckedViewNeedSubscriptionKey,
    producer_expression: ExprId,
    need_type: TypeId,
    payload_type: TypeId,
    producer: CheckedNeedProducerBinding,
    start: CheckedNeedStartPolicy,
    cancellation: CheckedNeedCancellationPolicy,
    persistence: CheckedNeedPersistenceAdmission,
    source: CheckedViewSourceRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedNeedStartPolicy {
    ObserveStartsNotStarted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedNeedCancellationPolicy {
    ProducerOwned,
}
```

`FinalSemanticAnalysis` publishes `checked_views: Arc<CheckedViewCatalog>` only
after exact generation, type, pattern, binding, effect, call, and ownership
validation.

## Product identities and wire record

```rust
// arcweft-view: lightweight coordinates
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewNeedSubscriptionId(NonZeroU32);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewNeedObserverRevision(u64);

// arcweft-bundle: persisted v1 records
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq,
         PartialOrd, Serialize)]
pub struct ViewNeedSubscriptionSemanticId([u8; 32]);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq,
         PartialOrd, Serialize)]
pub struct ViewNeedSubscriptionContractDigest([u8; 32]);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewNeedSubscriptionResourceV1 {
    pub version: u16, // exactly 1
    pub id: ViewNeedSubscriptionId,
    pub semantic_id: ViewNeedSubscriptionSemanticId,
    pub contract_digest: ViewNeedSubscriptionContractDigest,
    pub owner_node: ViewProgramNodeId,
    pub producer: ViewNeedProducerRefV1,
    pub need_type: RuntimeTypeRef,
    pub payload_type: RuntimeTypeRef,
    pub state_projection_type: RuntimeTypeRef,
    pub start_policy: ViewNeedStartPolicyV1,
    pub cancellation_policy: ViewNeedCancellationPolicyV1,
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewNeedProducerRefV1 {
    pub function: ViewValueFunctionRef,
    pub task_plan: AwbcTaskPlanId,
    pub function_binding: CrossSectionRef,
    pub task_binding: CrossSectionRef,
    pub awbc_program_digest: BundleDigest,
    pub producer_contract: ViewNeedProducerContractDigest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewNeedStartPolicyV1 {
    ObserveStartsNotStarted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewNeedCancellationPolicyV1 {
    ProducerOwned,
}
```

`version != 1`, zero/non-dense IDs, unknown fields/tags, noncanonical order,
missing cross-section refs, type disagreement, or digest mismatch reject before
catalog publication.

## Generic Match substrate

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewMatch {
    pub selector: ViewValueProgramId,
    pub arms: Box<[ViewMatchArm]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewMatchArm {
    pub ordinal: u16,
    pub body: ViewInstructionRange,
    pub bindings: Box<[ViewMatchArmBinding]>,
    pub contract: ViewMatchArmContractDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewMatchArmBinding {
    pub local: ViewLocalRef,
    pub output_register: AwbcRegisterId,
    pub value_type: RuntimeTypeRef,
    pub ownership: RuntimeBindingOwnership,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewValueInputSource {
    Parameter { parameter: ViewParameterRef },
    Local { local: ViewLocalRef },
    RepeatItem { repeat: ViewProgramNodeId },
    RepeatIndex { repeat: ViewProgramNodeId },
    MatchBinding {
        match_node: ViewProgramNodeId,
        local: ViewLocalRef,
    },
    NeedState {
        subscription: ViewNeedSubscriptionId,
    },
    HandlerInput { input: CheckedViewHandlerInputId },
    Environment { binding: EnvironmentBindingId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewMatchSelection {
    pub arm: u16,
    pub outputs: Box<[RuntimeValue]>,
}
```

`ViewMatchSelection` is an invocation result envelope of ordinary AWBC values,
not a new runtime value algebra.

## Core Need projection

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNeedMatchContract {
    pub need_type: RuntimeCheckedType,
    pub payload_type: RuntimeCheckedType,
    pub state_type: RuntimeCheckedType,
    pub not_started_case: RuntimeCheckedVariantCase,
    pub pending_case: RuntimeCheckedVariantCase,
    pub ready_case: RuntimeCheckedVariantCase,
    pub cancelled_case: RuntimeCheckedVariantCase,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeNeedStateDigest([u8; 32]);

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeNeedProjectionError {
    #[error("Need identity does not match the subscription contract")]
    NeedIdentity,
    #[error("Ready payload does not satisfy the exact checked type")]
    ReadyPayloadType,
    #[error("Need payload ownership is not retainable")]
    PayloadOwnership,
    #[error("Need payload exceeds the runtime value budget")]
    PayloadBudget,
    #[error("Need state variant contract is invalid")]
    StateContract,
}
```

Projected cases are fixed: NotStarted(no payload), Pending(one canonical
Progress), Ready(one exact T), Cancelled(no payload). Verified type graph case
identities, not spellings or integer magic, are authoritative.

## Runtime journal and observer state

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewNeedJournalKey {
    pub generation: GenerationId,
    pub need: NeedId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewNeedObserverKey {
    pub mount: ViewMountId,
    pub subscription: ViewNeedSubscriptionId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewNeedJournalEntry {
    pub cursor: Option<TaskPublicationCursor>,
    pub state: RuntimeNeedState,
    pub digest: RuntimeNeedStateDigest,
    pub terminal: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewNeedObserverState {
    pub key: ViewNeedObserverKey,
    pub journal: ViewNeedJournalKey,
    pub delivered_cursor: Option<TaskPublicationCursor>,
    pub active_arm: Option<u16>,
    pub retained_arms: BTreeMap<u16, ViewRetainedMatchArmState>,
    pub next_invalidation_revision: ViewNeedObserverRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewNeedStartIntent {
    pub observer: ViewNeedObserverKey,
    pub journal: ViewNeedJournalKey,
    pub producer: ViewNeedProducerRefV1,
    pub task_key: TaskKey,
    pub policy: TaskPolicy, // verified JoinSameKey
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewNeedInvalidation {
    pub observer: ViewNeedObserverKey,
    pub revision: ViewNeedObserverRevision,
    pub cursor: Option<TaskPublicationCursor>,
}
```

## Snapshot v1

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewNeedRuntimeSnapshotV1 {
    pub version: u16, // exactly 1
    pub producers: Vec<ViewNeedProducerSnapshotV1>,
    pub publications: Vec<ViewNeedPublicationSnapshotV1>,
    pub observers: Vec<ViewNeedObserverSnapshotV1>,
    pub invalidations: Vec<ViewNeedInvalidationSnapshotV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewNeedProducerSnapshotV1 {
    pub generation: u64,
    pub need: NeedId,
    pub producer_contract: ViewNeedProducerContractDigest,
    pub task_key: TaskKey,
    pub start_status: ViewNeedStartStatusV1,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewNeedPublicationSnapshotV1 {
    pub producer: u32,
    pub cursor: Option<TaskPublicationCursor>,
    pub state: Need<RuntimePayloadSnapshotV1>,
    pub state_digest: RuntimeNeedStateDigest,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewNeedObserverSnapshotV1 {
    pub mount: ViewMountId,
    pub subscription_semantic_id: ViewNeedSubscriptionSemanticId,
    pub subscription_contract: ViewNeedSubscriptionContractDigest,
    pub publication: u32,
    pub delivered_cursor: Option<TaskPublicationCursor>,
    pub active_arm: Option<u16>,
    pub retained_arms: Vec<ViewRetainedMatchArmSnapshotV1>,
}
```

Tables are sorted by typed keys with checked indices. Restore recomputes every
digest before constructing live state. Transient AWBC frames/caches are not saved.
