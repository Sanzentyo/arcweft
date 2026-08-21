# Exact Rust-shaped schemas

These signatures are normative shapes. Existing Arcweft-owned enums receive inherent variants and methods; no extension trait, compatibility wrapper, or ad hoc parallel type map is authorized.

## 1. `arcweft-lang-sema::final_analysis::model`

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedMatchArmOrdinal(u32);

impl CheckedMatchArmOrdinal {
    pub const fn new(value: u32) -> Self { Self(value) }
    pub const fn get(self) -> u32 { self.0 }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedMatchBindingOrdinal(u32);

impl CheckedMatchBindingOrdinal {
    pub const fn new(value: u32) -> Self { Self(value) }
    pub const fn get(self) -> u32 { self.0 }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedMatchArmId {
    owner: ExprId,
    ordinal: CheckedMatchArmOrdinal,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedMatchSemanticDigest([u8; 32]);

impl CheckedMatchSemanticDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self { Self(bytes) }
    pub const fn as_bytes(&self) -> &[u8; 32] { &self.0 }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatch {
    scrutinee: ExprId,
    arms: Box<[CheckedMatchArm]>,
    coverage: CheckedMatchCoverage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchArm {
    id: CheckedMatchArmId,
    scope: ScopeId,
    pattern: PatternId,
    guard: Option<ExprId>,
    value: ExprId,
    locals: Box<[CheckedMatchLocal]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchLocal {
    ordinal: CheckedMatchBindingOrdinal,
    local: LocalId,
    ownership: CheckedOwnershipDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedOwnershipDisposition {
    Copy,
    SnapshotClone,
    Rejected(CheckedOwnershipRejection),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedOwnershipRejection {
    Borrowed,
    Unique,
    Affine,
    MustDrop,
    FrameLocal,
    NonCloneable,
    NonSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchCoverage {
    exhaustive: bool,
    unreachable_arms: Box<[CheckedMatchArmOrdinal]>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedMatchRef {
    expression: ExprId,
    semantic_digest: CheckedMatchSemanticDigest,
}

// Existing enum: add this variant directly.
pub enum CheckedExpressionResolution {
    // existing variants ...
    Match(Box<CheckedMatch>),
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CheckedMatchConstructionError {
    #[error("expression {owner:?} is absent from the accepted HIR generation")]
    OwnerMissing { owner: ExprId },
    #[error("expression {owner:?} is not a Match expression")]
    OwnerNotMatch { owner: ExprId },
    #[error("Match {owner:?} references missing expression {expression:?}")]
    MissingExpression { owner: ExprId, expression: ExprId },
    #[error("Match {owner:?} references missing pattern {pattern:?}")]
    MissingPattern { owner: ExprId, pattern: PatternId },
    #[error("Match {owner:?} references missing binding {local:?}")]
    MissingBinding { owner: ExprId, local: LocalId },
    #[error("Match {owner:?} crosses final-HIR or semantic generations")]
    StaleGeneration { owner: ExprId },
    #[error("Match {owner:?} has a non-Bool guard {guard:?}")]
    GuardNotBool { owner: ExprId, guard: ExprId },
    #[error("Match {owner:?} arm/local cardinality exceeds u32")]
    CardinalityOverflow { owner: ExprId },
    #[error("Match {owner:?} coverage is inconsistent with its accepted arms")]
    CoverageMismatch { owner: ExprId },
}

impl CheckedMatch {
    pub(crate) fn try_from_hir(
        module: &HirModule,
        owner: ExprId,
        expressions: &BTreeMap<ExprId, CheckedExpression>,
        patterns: &BTreeMap<PatternId, CheckedPattern>,
        bindings: &BTreeMap<LocalId, CheckedBinding>,
        world: &RegisteredSemanticWorld,
        coverage: CheckedMatchCoverage,
    ) -> Result<Self, CheckedMatchConstructionError>;
}
```

`try_from_hir` reads `HirMatchExpr::scrutinee()` and each current `HirMatchArm { scope, pattern, guard, value, locals }` directly. It does not accept a detached arm array. Result type comes from the enclosing `CheckedExpression`; scrutinee/guard/value types and effects come from their referenced `CheckedExpression`s; pattern type comes from `CheckedPattern`; local type comes from `CheckedBinding`. `CheckedMatch` stores none of those facts again.

`FinalSemanticAnalysis` adds inherent accessors:

```rust
pub fn checked_match(&self, expression: ExprId) -> Option<&CheckedMatch>;
pub fn checked_match_digest(
    &self,
    expression: ExprId,
) -> Result<CheckedMatchSemanticDigest, CheckedMatchDigestError>;
```

The digest method reads the same expression/pattern/binding maps and fails on an absent or stale child fact.

The complete checked View catalog replaces its copied Match row with:

```rust
pub struct CheckedViewNeedMatch {
    key: CheckedViewNodeKey,
    owner: ItemId,
    match_fact: CheckedMatchRef,
    subscription: CheckedViewNeedSubscriptionKey,
    source: CheckedViewSourceRole,
}
```

It has no arms, bindings, coverage, inferred types, effects, or AWBC coordinates.

## 2. `arcweft-lang-sema::final_analysis::analyzer`

```rust
#[derive(Clone, Copy)]
pub struct FinalSemanticCatalogs<'a> {
    world: &'a RegisteredSemanticWorld,
    resource_types: &'a ResourceTypeRegistry,
    resource_type_digest: ResourceTypeRegistryDigest,
    callable_limits: CallableLimits,
}

impl<'a> FinalSemanticCatalogs<'a> {
    pub fn production(
        world: &'a RegisteredSemanticWorld,
        resource_types: &'a ResourceTypeRegistry,
    ) -> Result<Self, FinalSemanticCatalogInputError>;

    pub const fn world(self) -> &'a RegisteredSemanticWorld;
    pub const fn resource_types(self) -> &'a ResourceTypeRegistry;
    pub const fn resource_type_digest(self) -> ResourceTypeRegistryDigest;
    pub const fn callable_limits(self) -> CallableLimits;
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum FinalSemanticCatalogInputError {
    #[error("resource type registry failed canonical integrity validation: {source}")]
    ResourceRegistryIntegrity { source: ResourceRegistryIntegrityError },
}
```

`FinalSemanticAnalysis` retains the exact `ResourceTypeRegistryDigest`. No second resource digest type or constructor is introduced.

## 3. Compiler projection and `arcweft-runtime-plan::semantic_facts`

The compiler is the only layer that sees both sema facts and runtime-plan staging APIs. It projects a checked View Need Match into this runtime-plan-owned, generation-bound codegen seed:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeViewMatchSelectorSeed {
    checked_match_digest: [u8; 32],
    match_expression: ExprId,
    scrutinee_expression: ExprId,
    input_state_type: RuntimeSemanticTypeId,
    arms: Box<[RuntimeViewMatchSelectorArmSeed]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeViewMatchSelectorArmSeed {
    ordinal: u32,
    scope: ScopeId,
    pattern: PatternId,
    guard: Option<ExprId>,
    bindings: Box<[RuntimeViewMatchSelectorBindingSeed]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeViewMatchSelectorBindingSeed {
    ordinal: u32,
    local: LocalId,
    semantic_type: RuntimeSemanticTypeId,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeViewMatchSelectorSeedError {
    #[error("selector arms or bindings are not dense and source ordered")]
    NonDenseOrder,
    #[error("selector references a semantic type absent from the same type-seed batch")]
    MissingTypeSeed { identity: RuntimeSemanticTypeId },
    #[error("selector references a missing runtime expression/pattern/local fact")]
    MissingRuntimeFact,
    #[error("selector checked-Match digest does not match the compiler input")]
    CheckedMatchDigest,
}

impl RuntimeSemanticFactInput {
    pub fn insert_view_match_selector(
        &mut self,
        selector: RuntimeViewMatchSelectorSeed,
    ) -> Result<(), RuntimeViewMatchSelectorSeedError>;
}
```

`input_state_type` identifies the synthetic four-case `NeedState<T>` `RuntimePlanTypeSeed` created in the same type-seed batch. Binding identities refer to the one existing normalized semantic type graph. Runtime-plan atomic finalization rewrites those identities into its sole plan-local type table:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeViewMatchSelector {
    checked_match_digest: [u8; 32],
    match_expression: ExprId,
    scrutinee_expression: ExprId,
    input_state_type: RuntimePlanTypeId,
    arms: Box<[RuntimeViewMatchSelectorArm]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeViewMatchSelectorArm {
    ordinal: u32,
    scope: ScopeId,
    pattern: RuntimePattern,
    guard: Option<RuntimeExpr>,
    bindings: Box<[RuntimeViewMatchSelectorBinding]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeViewMatchSelectorBinding {
    ordinal: u32,
    local: RuntimeLocalDeclarationId,
    value_type: RuntimePlanTypeId,
}

impl RuntimePlan {
    pub fn view_match_selector(
        &self,
        expression: ExprId,
    ) -> Option<&RuntimeViewMatchSelector>;
}

pub struct LoweredViewMatchSelector {
    pub match_expression: ExprId,
    pub checked_match_digest: [u8; 32],
    pub function: AwbcFunctionId,
    pub input_state_type: AwbcTypeId,
    pub input_state_type_digest: AwbcRuntimeTypeDigest,
    pub result_type: AwbcTypeId,
    pub result_type_digest: AwbcRuntimeTypeDigest,
    pub cases: Box<[LoweredViewMatchSelectorCase]>,
}

pub struct ViewMatchSelectorBuilder<'a> {
    inventory: &'a mut AwbcInventory,
    plan: &'a RuntimePlan,
    selector: &'a RuntimeViewMatchSelector,
}

impl<'a> ViewMatchSelectorBuilder<'a> {
    pub fn lower(self) -> Result<LoweredViewMatchSelector, ViewMatchSelectorLowerError>;
}
```

The seed/final row is codegen input, not a checked semantic authority: it contains no coverage, effects, source spans, ownership decision, View local/body coordinate, or copied type graph. Its digest must match `CheckedMatchRef`, and runtime-plan has no dependency on sema/View/bundle.

## 4. `arcweft-view::program::reactive_match`

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ViewMatchSiteId(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ViewMatchArmOrdinal(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ViewMatchBindingOutputOrdinal(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ViewLocalRef(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ViewInstructionOffset(u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewInstructionRange {
    start: ViewInstructionOffset,
    len: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewMatchSite {
    site: ViewMatchSiteId,
    arms: Box<[ViewMatchArmCoordinate]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewMatchArmCoordinate {
    arm: ViewMatchArmOrdinal,
    body: ViewInstructionRange,
    bindings: Box<[ViewMatchBindingCoordinate]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewMatchBindingCoordinate {
    output: ViewMatchBindingOutputOrdinal,
    local: ViewLocalRef,
}
```

These are all Match-related View-owned rows. They contain no core value/register/type coordinate or copied type table.

## 5. `arcweft-core::pattern`, `arcweft-core::task`, and `arcweft-core::value`

```rust
// Existing enum: add directly.
pub enum RuntimeCheckedType {
    // existing variants ...
    Need(Box<RuntimeCheckedType>),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct NeedId([u8; 32]);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct NeedProducerContractDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AwbcRuntimeTypeDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeValueDigest([u8; 32]);

impl NeedId {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self;
    pub const fn as_bytes(&self) -> &[u8; 32];
    pub fn to_lower_hex(self) -> String;

    pub(crate) fn derive(
        producer: NeedProducerContractDigest,
        arguments: RuntimeValueDigest,
    ) -> Self;
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedHandle {
    need: NeedId,
    producer_contract: NeedProducerContractDigest,
    payload_type: AwbcRuntimeTypeDigest,
    arguments: Box<[RuntimeValue]>,
}

impl RuntimeNeedHandle {
    pub const fn need(&self) -> NeedId;
    pub const fn producer_contract(&self) -> NeedProducerContractDigest;
    pub const fn payload_type(&self) -> AwbcRuntimeTypeDigest;
    pub const fn arguments(&self) -> &[RuntimeValue];

    pub(crate) fn from_verified_producer(
        need: NeedId,
        producer_contract: NeedProducerContractDigest,
        payload_type: AwbcRuntimeTypeDigest,
        arguments: Box<[RuntimeValue]>,
    ) -> Self;
}

// Existing enum: add directly.
pub enum RuntimeValue {
    // existing variants ...
    NeedHandle(RuntimeNeedHandle),
}
```

`NeedId` no longer exposes a String field or parser from arbitrary text. `TaskId`, `TaskKey`, and `TaskHandle` remain outside this correction.

## 6. `arcweft-core::awbc`

```rust
pub enum AwbcRuntimeType {
    // existing variants ...
    NeedHandle { payload: AwbcTypeId },
}

pub enum AwbcInstruction {
    // existing variants ...
    MakeNeedHandle {
        dst: AwbcRegisterId,
        plan: AwbcTaskPlanId,
        site: u32,
        args: Vec<AwbcRegisterId>,
    },
}

impl AwbcFunctionFlags {
    pub const NEED_PRODUCER: u32 = 1 << 4;
}

pub struct AwbcTaskPlan {
    pub public_id: AwbcStringId,
    // old `need_id: AwbcStringId` is deleted.
    pub capability: AwbcStringId,
    pub operation: AwbcStringId,
    pub signature: AwbcSignatureId,
    pub class: AwbcTaskClass,
    pub priority: i32,
    pub cancel_scope: AwbcStringId,
    pub policy: AwbcTaskPolicy,
    pub payload_type: AwbcTypeId,
    pub arguments: Vec<AwbcHostArgument>,
    pub many: Option<AwbcAwaitManyPolicy>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedAwbcViewMatchSelector {
    function: AwbcFunctionId,
    input_state_type: AwbcTypeId,
    input_state_type_digest: AwbcRuntimeTypeDigest,
    result_type: AwbcTypeId,
    result_type_digest: AwbcRuntimeTypeDigest,
    cases: Box<[VerifiedAwbcViewMatchSelectorCase]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedAwbcViewMatchSelectorCase {
    case_ordinal: u32,
    payload_tuple: AwbcTypeId,
    binding_types: Box<[AwbcTypeId]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedAwbcNeedProducer {
    function: AwbcFunctionId,
    result_type: AwbcTypeId,
    payload_type: AwbcTypeId,
    task_plan: AwbcTaskPlanId,
    site: u32,
    argument_types: Box<[AwbcTypeId]>,
    producer_contract: NeedProducerContractDigest,
}

impl AwbcProgram {
    pub fn verify_view_match_selector(
        &self,
        function: AwbcFunctionId,
    ) -> Result<VerifiedAwbcViewMatchSelector, AwbcViewMatchSelectorVerifyError>;

    pub fn verify_need_producer(
        &self,
        function: AwbcFunctionId,
    ) -> Result<VerifiedAwbcNeedProducer, AwbcNeedProducerVerifyError>;

    pub fn canonical_type_digest(
        &self,
        ty: AwbcTypeId,
    ) -> Result<AwbcRuntimeTypeDigest, AwbcTypeDigestError>;
}

impl AwbcVm {
    pub fn invoke_need_producer(
        &mut self,
        producer: &VerifiedAwbcNeedProducer,
        args: &[RuntimeValue],
    ) -> Result<RuntimeNeedHandle, AwbcNeedProducerExecutionError>;
}
```

`AwbcOpcode::MakeNeedHandle.encoded() == 0x1e`; existing `Drop` remains `0x1f`. `RuntimeNormalizedType::checked_type_at`, `AwbcProgram::checked_type`, and `awbc_lower::pattern::intern_runtime_type` add exhaustive inherent `Need` branches. No fallback to `Dynamic` is admitted for a checked Need producer/result.

## 7. `arcweft-bundle::resource_codec::view_reactive`

```rust
pub const VIEW_REACTIVE_BINDING_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewReactiveBindingSectionV1 {
    schema_version: u32,
    resource_types: ResourceTypeRegistryDigest,
    match_selectors: Box<[ViewMatchSelectorBindingV1]>,
    need_producers: Box<[ViewNeedProducerBindingV1]>,
    source_maps: Box<[ViewReactiveSourceMapEntryV1]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewMatchSelectorBindingV1 {
    site: ViewMatchSiteId,
    checked_match_digest: [u8; 32],
    function: AwbcFunctionId,
    input_state_type: AwbcTypeId,
    input_state_type_digest: AwbcRuntimeTypeDigest,
    result_type: AwbcTypeId,
    result_type_digest: AwbcRuntimeTypeDigest,
    cases: Box<[ViewMatchSelectorCaseBindingV1]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewMatchSelectorCaseBindingV1 {
    arm: ViewMatchArmOrdinal,
    case_ordinal: u32,
    payload_tuple: AwbcTypeId,
    outputs: Box<[ViewMatchSelectorOutputBindingV1]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewMatchSelectorOutputBindingV1 {
    output: ViewMatchBindingOutputOrdinal,
    local: ViewLocalRef,
    value_type: AwbcTypeId,
    disposition: ViewMatchBindingDispositionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMatchBindingDispositionV1 {
    SnapshotClone,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewNeedProducerBindingV1 {
    producer_contract: NeedProducerContractDigest,
    function: AwbcFunctionId,
    result_type: AwbcTypeId,
    payload_type: AwbcTypeId,
    payload_type_digest: AwbcRuntimeTypeDigest,
    task_plan: AwbcTaskPlanId,
    argument_types: Box<[AwbcTypeId]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewReactiveSourceMapEntryV1 {
    role: ViewReactiveSourceRoleV1,
    source_map: AwbcSourceMapId,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewReactiveSourceRoleV1 {
    MatchSite { site: ViewMatchSiteId },
    MatchArm { site: ViewMatchSiteId, arm: ViewMatchArmOrdinal },
    MatchBinding {
        site: ViewMatchSiteId,
        arm: ViewMatchArmOrdinal,
        output: ViewMatchBindingOutputOrdinal,
    },
    NeedProducer { producer_contract: NeedProducerContractDigest },
}
```

Canonical order is selector site; arm; output; producer-contract bytes; then source-map role discriminant and coordinates. Duplicate keys require byte-for-byte equality at merge and are otherwise rejected.

## 8. `arcweft-runtime-driver::view::reactive`

```rust
#[derive(Clone, Debug)]
pub(crate) struct VerifiedViewReactiveBindings {
    section: Arc<ViewReactiveBindingSectionV1>,
    selector_index: BTreeMap<ViewMatchSiteId, u32>,
    producer_index: BTreeMap<NeedProducerContractDigest, u32>,
}

#[derive(Clone, Debug)]
pub(crate) struct DecodedViewMatchSelection {
    arm: ViewMatchArmOrdinal,
    bindings: Box<[RuntimeValue]>,
}

#[derive(Clone, Debug)]
pub(crate) struct ViewLocalStore {
    revision: u64,
    values: BTreeMap<ViewLocalRef, RuntimeValue>,
}

#[derive(Clone, Debug)]
pub(crate) struct LocalInstallTransaction {
    expected_revision: u64,
    staged: BTreeMap<ViewLocalRef, RuntimeValue>,
}

impl LocalInstallTransaction {
    pub(crate) fn begin(locals: &ViewLocalStore) -> Self;

    pub(crate) fn stage(
        &mut self,
        local: ViewLocalRef,
        expected: AwbcTypeId,
        value: RuntimeValue,
        program: &AwbcProgram,
    ) -> Result<(), ViewLocalInstallError>;

    pub(crate) fn commit(
        self,
        locals: &mut ViewLocalStore,
    ) -> Result<(), ViewLocalInstallError>;
}

#[derive(Clone, Debug)]
pub(crate) struct VerifiedNeedHandle<'a> {
    generation: GenerationId,
    handle: &'a RuntimeNeedHandle,
    binding: &'a ViewNeedProducerBindingV1,
    plan: &'a AwbcTaskPlan,
}

pub(crate) fn decode_match_selection(
    program: &AwbcProgram,
    binding: &ViewMatchSelectorBindingV1,
    value: RuntimeValue,
) -> Result<DecodedViewMatchSelection, ViewMatchDecodeError>;

pub(crate) fn extract_need_handle<'a>(
    active: &'a ProgramGeneration,
    program: &'a AwbcProgram,
    bindings: &'a VerifiedViewReactiveBindings,
    value: &'a RuntimeValue,
) -> Result<VerifiedNeedHandle<'a>, NeedHandleExtractionError>;
```

`DecodedViewMatchSelection`, `LocalInstallTransaction`, and `VerifiedNeedHandle` are private scratch/verified owners. They are never exported by `arcweft-view` or serialized as public product rows.
