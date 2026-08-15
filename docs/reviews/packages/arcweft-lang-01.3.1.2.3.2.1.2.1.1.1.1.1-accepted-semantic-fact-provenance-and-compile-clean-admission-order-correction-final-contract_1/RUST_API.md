# Exact Rust-shaped API

All fields shown without `pub` are private. Error enums are `thiserror` owners in
the same modules as their operations. Every Arcweft-owned codec/schema constant
is `1`.

## `arcweft-core::plan::type_table`

```rust
impl RuntimePlanTypeTableBuilder {
    pub fn intern_batch(
        &mut self,
        declarations: impl IntoIterator<Item = RuntimePlanTypeDeclaration>,
    ) -> Result<Box<[RuntimePlanTypeId]>, RuntimePlanTypeTableError>;

    pub fn intern(
        &mut self,
        declaration: RuntimePlanTypeDeclaration,
    ) -> Result<RuntimePlanTypeId, RuntimePlanTypeTableError>;
}
```

`intern_batch` returns one ID per input row. It preflights existing and
intra-batch conflicts plus capacity, commits in input order, and is atomic on
error. It is added to the original inherent implementation; no helper trait or
second interner is introduced.

## `arcweft-core::plan::typed_sites`

```rust
pub const MAX_RUNTIME_INDEX_PATH_DEPTH: usize = 64;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeIndexPath(Box<[u32]>);

impl RuntimeIndexPath {
    pub fn try_from_indices(
        indices: impl IntoIterator<Item = u32>,
    ) -> Result<Self, RuntimeIndexPathError>;
    pub fn root() -> Self;                 // exactly [0]
    pub fn child(&self, ordinal: usize) -> Result<Self, RuntimeIndexPathError>;
    pub const fn indices(&self) -> &[u32];
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExprTypeFact {
    path: RuntimeIndexPath,
    ty: RuntimePlanTypeId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePatternTypeFact {
    path: RuntimeIndexPath,
    ty: RuntimePlanTypeId,
}

impl RuntimeExprTypeFact {
    pub const fn path(&self) -> &RuntimeIndexPath;
    pub const fn ty(&self) -> RuntimePlanTypeId;
}

impl RuntimePatternTypeFact {
    pub const fn path(&self) -> &RuntimeIndexPath;
    pub const fn ty(&self) -> RuntimePlanTypeId;
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTypedExpr {
    expr: RuntimeExpr,
    types: Box<[RuntimeExprTypeFact]>,
}

impl RuntimeTypedExpr {
    pub const fn expr(&self) -> &RuntimeExpr;
    pub const fn types(&self) -> &[RuntimeExprTypeFact];
    pub fn type_at(&self, path: &RuntimeIndexPath) -> Option<RuntimePlanTypeId>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTypedPattern {
    pattern: RuntimePattern,
    types: Box<[RuntimePatternTypeFact]>,
    bindings: Box<[RuntimePatternBindingCoordinate]>,
}

impl RuntimeTypedPattern {
    pub const fn pattern(&self) -> &RuntimePattern;
    pub const fn types(&self) -> &[RuntimePatternTypeFact];
    pub const fn bindings(&self) -> &[RuntimePatternBindingCoordinate];
    pub fn type_at(&self, path: &RuntimeIndexPath) -> Option<RuntimePlanTypeId>;
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePlanTypedSite {
    Entry { entry: u32, slot: RuntimeEntryTypedSlot },
    FlowExecutable { flow: u32, slot: RuntimeFlowExecutableTypedSlot },
    FlowExpression { flow: u32, path: RuntimeFlowOpPath, field: RuntimeFlowExpressionField },
    FlowPattern { flow: u32, path: RuntimeFlowOpPath, field: RuntimeFlowPatternField },
    PureHelper { helper: u32, slot: RuntimePureHelperTypedSlot },
    PureHelperExpression { helper: u32, node: RuntimeIndexPath },
    TraitMethod { method: u32, slot: RuntimeTraitMethodTypedSlot },
    TraitMethodExpression { method: u32, node: RuntimeIndexPath },
    Stream { stream: u32, slot: RuntimeStreamTypedSlot },
    StreamExpression { stream: u32, path: RuntimeStreamOpPath, field: RuntimeStreamExpressionField },
    StreamPattern { stream: u32, path: RuntimeStreamOpPath, field: RuntimeStreamPatternField },
    Source { source: u32, slot: RuntimeSourceTypedSlot },
    SourceExpression { source: u32, op: u32, field: RuntimeSourceExpressionField },
    SourcePattern { source: u32, handler: u32, field: RuntimeSourcePatternField },
    LineTaskGroup { group: u32, slot: RuntimeLineTaskGroupTypedSlot },
}
```

The nested slot/path enums retain the already-selected parent contract and
closed tags. No generic `u32 slot` or string slot is added.

## `arcweft-core::plan::construction`

```rust
pub struct RuntimePlanBuilder {
    locals: RuntimeLocalDeclarationTable,
    types: RuntimePlanTypeTableBuilder,
    entries: Vec<RuntimeEntrySpec>,
    callable_executables: Vec<RuntimeCallableExecutable>,
    flow_executables: Vec<RuntimeFlowExecutable>,
    flows: Vec<RuntimeFlow>,
    pure_helpers: Vec<RuntimePureHelper>,
    trait_methods: Vec<RuntimeTraitMethod>,
    line_task_groups: Vec<LineTaskGroup>,
    stream_plans: Vec<StreamPlan>,
    source_plans: Vec<SourcePlan>,
    typed_sites: BTreeMap<RuntimePlanTypedSite, RuntimePlanTypeId>,
}

impl RuntimePlanBuilder {
    pub fn new(locals: RuntimeLocalDeclarationTable) -> Self;

    pub fn try_build_expr(
        &mut self,
        expr: RuntimeExpr,
        declarations: impl IntoIterator<Item = (RuntimeIndexPath, RuntimePlanTypeDeclaration)>,
    ) -> Result<RuntimeTypedExpr, RuntimePlanBuildError>;

    pub fn try_build_pattern(
        &mut self,
        pattern: RuntimePattern,
        declarations: impl IntoIterator<Item = (RuntimeIndexPath, RuntimePlanTypeDeclaration)>,
        bindings: impl IntoIterator<Item = RuntimePatternBindingCoordinate>,
    ) -> Result<RuntimeTypedPattern, RuntimePlanBuildError>;

    pub fn push_entry(&mut self, entry: RuntimeEntrySpec) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_callable_executable(&mut self, value: RuntimeCallableExecutable) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_flow_executable(&mut self, value: RuntimeFlowExecutable) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_flow(&mut self, value: RuntimeFlow) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_pure_helper(&mut self, value: RuntimePureHelper) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_trait_method(&mut self, value: RuntimeTraitMethod) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_line_task_group(&mut self, value: LineTaskGroup) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_stream_plan(&mut self, value: StreamPlan) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_source_plan(&mut self, value: SourcePlan) -> Result<u32, RuntimePlanBuildError>;

    pub fn bind_declaration_site(
        &mut self,
        site: RuntimePlanTypedSite,
        declaration: RuntimePlanTypeDeclaration,
    ) -> Result<(), RuntimePlanBuildError>;

    pub fn bind_expression_site(
        &mut self,
        site: RuntimePlanTypedSite,
        expression: &RuntimeTypedExpr,
        node: &RuntimeIndexPath,
    ) -> Result<(), RuntimePlanBuildError>;

    pub fn bind_pattern_site(
        &mut self,
        site: RuntimePlanTypedSite,
        pattern: &RuntimeTypedPattern,
        node: &RuntimeIndexPath,
    ) -> Result<(), RuntimePlanBuildError>;

    pub fn finish(self) -> Result<RuntimePlan, RuntimePlanBuildError>;
}

impl RuntimePlan {
    pub const fn local_declarations(&self) -> &RuntimeLocalDeclarationTable;
    pub const fn types(&self) -> &RuntimePlanTypeTable;
    pub fn typed_sites(&self) -> impl ExactSizeIterator<Item = (&RuntimePlanTypedSite, RuntimePlanTypeId)>;
    pub fn resolve_typed_site(&self, site: &RuntimePlanTypedSite) -> Option<&RuntimePlanTypeDeclaration>;
    pub const fn entries(&self) -> &[RuntimeEntrySpec];
    pub const fn callable_executables(&self) -> &[RuntimeCallableExecutable];
    pub const fn flow_executables(&self) -> &[RuntimeFlowExecutable];
    pub const fn flows(&self) -> &[RuntimeFlow];
    pub const fn pure_helpers(&self) -> &[RuntimePureHelper];
    pub const fn trait_methods(&self) -> &[RuntimeTraitMethod];
    pub const fn line_task_groups(&self) -> &[LineTaskGroup];
    pub const fn stream_plans(&self) -> &[StreamPlan];
    pub const fn source_plans(&self) -> &[SourcePlan];
}
```

The custom v1 decoder owns a `RuntimePlanBuilder`; there is no raw field DTO
that can bypass `try_build_expr`, `try_build_pattern`, the three typed-site binders, or
`finish`.

## HIR/runtime-plan synthetic provenance

```rust
// arcweft-lang-hir; identity only, no semantic/core type dependency.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeSyntheticExprSite {
    SpreadArgument { call: ExprId, authored_ordinal: u32 },
    PostfixAlias { postfix: ExprId },
    AssignmentTarget { statement: StmtId },
    AssignmentValue { statement: StmtId },
    AssignmentContinuation { statement: StmtId },
    VariantPayloadTuple { call: ExprId },
    ReductionCommands { call: ExprId },
    RecordShorthandLocal { record: ExprId, field_ordinal: u32, local: LocalId },
    NominalRecordShorthandLocal { record: ExprId, field_ordinal: u32, local: LocalId },
    PureBlockLetValue { block: ExprId, statement: StmtId },
    PureBlockLetBody { block: ExprId, statement: StmtId },
    PureBlockAssignmentTarget { block: ExprId, statement: StmtId },
    PureBlockAssignmentValue { block: ExprId, statement: StmtId },
    PureBlockAssignmentBody { block: ExprId, statement: StmtId },
    FunctionBodyLetValue { function: ItemId, statement: StmtId },
    FunctionBodyLetBody { function: ItemId, statement: StmtId },
    FunctionBodyAssignmentTarget { function: ItemId, statement: StmtId },
    FunctionBodyAssignmentValue { function: ItemId, statement: StmtId },
    FunctionBodyAssignmentBody { function: ItemId, statement: StmtId },
    FlowAssignmentOuter { statement: StmtId },
    FlowAssignmentContinuationUnit { statement: StmtId },
    AssertionFailureMessage { statement: StmtId, condition: u32 },
    ChoiceActionRecord { call: ExprId },
    ChoiceActionId { call: ExprId },
    ChoiceActionTarget { call: ExprId },
    ChoiceActionAction { call: ExprId },
    ChoiceActionKind { call: ExprId },
    ChoiceActionEnabled { call: ExprId },
    AgentTargetViewportRecord { call: ExprId },
    AgentTargetViewportKind { call: ExprId },
    AgentTargetLayerRecord { call: ExprId },
    AgentTargetLayerKind { call: ExprId },
    AgentTargetObjectRecord { call: ExprId },
    AgentTargetObjectKind { call: ExprId },
    AgentTargetViewportPointRecord { call: ExprId },
    AgentProbeSignalRecord { call: ExprId },
    AgentProbeSignalKind { call: ExprId },
    AgentProbeMetricRecord { call: ExprId },
    AgentProbeMetricKind { call: ExprId },
    AgentProbeStateRecord { call: ExprId },
    AgentProbeStateKind { call: ExprId },
    AgentProbeObservationRecord { call: ExprId },
    AgentProbeObservationKind { call: ExprId },
    AgentProbeDiagnosticsRecord { call: ExprId },
    AgentProbeDiagnosticsKind { call: ExprId },
    AgentPredicateExistsRecord { call: ExprId },
    AgentPredicateExistsKind { call: ExprId },
    AgentPredicateNotRecord { call: ExprId },
    AgentPredicateNotKind { call: ExprId },
    AgentPredicateActionEnabledRecord { call: ExprId },
    AgentPredicateActionEnabledKind { call: ExprId },
    AgentPredicateActionEnabledTarget { call: ExprId },
    AgentPredicateAllRecord { call: ExprId },
    AgentPredicateAllKind { call: ExprId },
    AgentPredicateAllTuple { call: ExprId },
    AgentPredicateAnyRecord { call: ExprId },
    AgentPredicateAnyKind { call: ExprId },
    AgentPredicateAnyTuple { call: ExprId },
    AgentComparisonRecord { call: ExprId },
    AgentComparisonKind { call: ExprId },
    AgentComparisonOperator { call: ExprId },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeSyntheticPatternSite {
    RecordShorthandBinding { pattern: PatternId, field_ordinal: u32, local: LocalId },
    RecordRestBinding { pattern: PatternId, local: LocalId },
    SequenceRestBinding { pattern: PatternId, local: LocalId },
    WholeBinding { pattern: PatternId, local: LocalId },
    FlowAssignmentDiscard { statement: StmtId },
}

// arcweft-runtime-plan::semantic_facts
impl RuntimePlanSemanticFactStagingInput {
    pub fn push_synthetic_expression_type(
        &mut self,
        site: HirRuntimeSyntheticExprSite,
        ty: RuntimeNormalizedType,
    );
    pub fn push_synthetic_pattern_type(
        &mut self,
        site: HirRuntimeSyntheticPatternSite,
        ty: RuntimeNormalizedType,
    );
}

impl RuntimePlanSemanticFacts {
    pub fn synthetic_expression_type(
        &self,
        site: HirRuntimeSyntheticExprSite,
    ) -> Option<&RuntimeNormalizedType>;
    pub fn synthetic_pattern_type(
        &self,
        site: HirRuntimeSyntheticPatternSite,
    ) -> Option<&RuntimeNormalizedType>;
    pub fn into_lowering_parts(self) -> RuntimePlanLoweringParts;
}

pub struct RuntimePlanLoweringParts {
    builder: RuntimePlanBuilder,
    facts: RuntimePlanLoweringFacts,
}

impl RuntimePlanLoweringParts {
    pub fn into_parts(self) -> (RuntimePlanBuilder, RuntimePlanLoweringFacts);
}
```

`RuntimePlanLoweringFacts` has no `Clone` and owns the moved accepted maps. It
has no fallback resolver.

## Exact lowerers

```rust
pub(crate) struct FinalExprLowerer<'hir, 'facts, 'plan> {
    module: &'hir HirModule,
    facts: &'facts RuntimePlanLoweringFacts,
    plan: &'plan mut RuntimePlanBuilder,
}

impl FinalExprLowerer<'_, '_, '_> {
    pub(crate) fn lower(&mut self, id: ExprId) -> Result<RuntimeTypedExpr, RuntimePlanLowerError>;
}

pub(crate) struct FinalPatternLowerer<'hir, 'facts, 'plan> {
    module: &'hir HirModule,
    facts: &'facts RuntimePlanLoweringFacts,
    plan: &'plan mut RuntimePlanBuilder,
}

impl FinalPatternLowerer<'_, '_, '_> {
    pub(crate) fn lower(&mut self, id: PatternId) -> Result<RuntimeTypedPattern, RuntimePlanLowerError>;
}

pub(crate) struct FinalFlowLowerer<'hir, 'facts> {
    module: &'hir HirModule,
    facts: &'facts RuntimePlanLoweringFacts,
    plan: RuntimePlanBuilder,
}

impl FinalFlowLowerer<'_, '_> {
    pub(crate) fn lower(mut self) -> Result<RuntimePlan, RuntimePlanLowerError>;
}
```

## Nominal field projection

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeNominalRecordFieldProjection {
    field: RuntimeRecordFieldId,
    ty: RuntimeSemanticTypeId,
}

impl RuntimeNominalRecordFieldProjection {
    pub fn try_from_accepted_ordinal(
        zero_based_ordinal: usize,
        ty: RuntimeSemanticTypeId,
    ) -> Result<Self, RuntimeNominalRecordFieldProjectionError>;
    pub const fn field(&self) -> RuntimeRecordFieldId;
    pub const fn ty(&self) -> RuntimeSemanticTypeId;
}
```

## Core generation facts

```rust
pub struct RuntimeProjectRootFact {
    semantic_identity: RuntimeSemanticTypeId,
    root: RuntimeProjectRootId,
    checked_type: RuntimeCheckedType,
}

impl RuntimeProjectRootFact {
    pub fn try_new(
        semantic_identity: RuntimeSemanticTypeId,
        checked_type: RuntimeCheckedType,
    ) -> Result<Self, RuntimeGenerationFactError>;
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;
    pub const fn root(&self) -> RuntimeProjectRootId;
    pub const fn checked_type(&self) -> &RuntimeCheckedType;
}

pub struct RuntimeProducerFact {
    producer: RuntimeOpaqueTypeProducerId,
    owner: RuntimeOpaqueTypeOwner,
    payload_semantic_identity: RuntimeSemanticTypeId,
    root: RuntimeProducerRootId,
}

impl RuntimeProducerFact {
    pub fn try_new(
        producer: RuntimeOpaqueTypeProducerId,
        owner: RuntimeOpaqueTypeOwner,
        payload_semantic_identity: RuntimeSemanticTypeId,
    ) -> Result<Self, RuntimeGenerationFactError>;
    pub const fn producer(&self) -> &RuntimeOpaqueTypeProducerId;
    pub const fn owner(&self) -> &RuntimeOpaqueTypeOwner;
    pub const fn payload_semantic_identity(&self) -> RuntimeSemanticTypeId;
    pub const fn root(&self) -> RuntimeProducerRootId;
}

pub enum RuntimeNominalRecordConstructionOwner {
    Project(RuntimeProjectRootId),
    Producer {
        producer: RuntimeOpaqueTypeProducerId,
        root: RuntimeProducerRootId,
    },
}

pub struct RuntimeGenerationFactAggregate {
    core: CoreRuntimeGenerationFacts,
    characters: RuntimeCharacterCatalogFact,
    views: RuntimeViewCatalogFact,
    dialogue: CharacterDialogueRuntimeCustomFieldFact,
}

impl RuntimeGenerationFactAggregate {
    pub fn try_new(
        core: CoreRuntimeGenerationFacts,
        characters: RuntimeCharacterCatalogFact,
        views: RuntimeViewCatalogFact,
        dialogue: CharacterDialogueRuntimeCustomFieldFact,
    ) -> Result<Self, RuntimeGenerationFactError>;
    pub const fn core(&self) -> &CoreRuntimeGenerationFacts;
    pub const fn characters(&self) -> &RuntimeCharacterCatalogFact;
    pub const fn views(&self) -> &RuntimeViewCatalogFact;
    pub const fn dialogue(&self) -> &CharacterDialogueRuntimeCustomFieldFact;
}

pub struct AdmittedRuntimeGeneration { inner: Arc<AdmittedRuntimeGenerationInner> }

impl AdmittedRuntimeGeneration {
    pub fn try_issue(
        facts: RuntimeGenerationFactAggregate,
    ) -> Result<Arc<Self>, RuntimeGenerationAdmissionError>;
    pub const fn identity(&self) -> RuntimeGenerationIdentity;
    pub fn require_same_parent(&self, other: &Self) -> Result<(), RuntimeGenerationParentMismatch>;
}
```

The four aggregate fields are core-facing fact wrappers returned by their
existing final owners. `RuntimeGenerationFactAggregate::try_new` accepts no bare
root map or digest scalar. `AdmittedRuntimeGeneration::try_issue` is the explicit
public trusted-integrator boundary: safe Rust privacy prevents accidental field
mutation, but it is not a proof that a malicious public caller could not invoke
the structural issuer with self-authored accepted catalog inputs.

## Admission and product

```rust
pub struct AdmittedRuntimePlan {
    parent: Arc<AdmittedRuntimeGeneration>,
    key: Arc<RuntimePlanAdmissionKey>,
    plan: RuntimePlan,
}

pub struct AdmittedAwbcProgram {
    parent: Arc<AdmittedRuntimeGeneration>,
    plan_key: Arc<RuntimePlanAdmissionKey>,
    program: AwbcProgram,
}

pub struct AdmittedRuntimeProduct {
    plan: AdmittedRuntimePlan,
    awbc: AdmittedAwbcProgram,
}

pub fn try_admit_runtime_plan(
    parent: Arc<AdmittedRuntimeGeneration>,
    plan: RuntimePlan,
) -> Result<AdmittedRuntimePlan, RuntimePlanAdmissionError>;

pub fn try_admit_awbc_program(
    parent: Arc<AdmittedRuntimeGeneration>,
    plan: &AdmittedRuntimePlan,
    program: AwbcProgram,
) -> Result<AdmittedAwbcProgram, AwbcAdmissionError>;

pub fn try_pair_runtime_product(
    plan: AdmittedRuntimePlan,
    awbc: AdmittedAwbcProgram,
) -> Result<AdmittedRuntimeProduct, RuntimeProductAdmissionError>;

impl AdmittedRuntimePlan {
    pub const fn generation(&self) -> &Arc<AdmittedRuntimeGeneration>;
    pub const fn plan(&self) -> &RuntimePlan;
}

impl AdmittedAwbcProgram {
    pub const fn generation(&self) -> &Arc<AdmittedRuntimeGeneration>;
    pub const fn program(&self) -> &AwbcProgram;
}

impl AdmittedRuntimeProduct {
    pub const fn generation(&self) -> &Arc<AdmittedRuntimeGeneration>;
    pub const fn plan(&self) -> &AdmittedRuntimePlan;
    pub const fn awbc(&self) -> &AdmittedAwbcProgram;

    pub fn checked_value_context(
        &self,
        site: &RuntimePlanTypedSite,
    ) -> Result<RuntimeCheckedValueContext<'_>, RuntimeCheckedValueContextError>;

    pub fn nominal_record_domain(
        &self,
        id: AwbcNominalRecordDomainId,
    ) -> Result<RuntimeNominalRecordAdmissionDomain<'_>, RuntimeNominalRecordDomainError>;
}

pub struct RuntimeCheckedValueContext<'product> {
    product: &'product AdmittedRuntimeProduct,
    site: &'product RuntimePlanTypedSite,
    ty: RuntimePlanTypeId,
    checked: &'product RuntimeCheckedType,
}

impl RuntimeCheckedValueContext<'_> {
    pub const fn site(&self) -> &RuntimePlanTypedSite;
    pub const fn type_id(&self) -> RuntimePlanTypeId;
    pub const fn checked_type(&self) -> &RuntimeCheckedType;
    pub fn validate(
        &self,
        value: &RuntimeValue,
        budget: &mut RuntimeCheckedTypeWorkBudget,
    ) -> Result<(), RuntimeCheckedTypeError>;
}

pub struct RuntimeNominalRecordAdmissionDomain<'product> {
    product: &'product AdmittedRuntimeProduct,
    owner: RuntimeNominalRecordConstructionOwner,
    layout: &'product RuntimeNominalRecordLayout,
}

impl RuntimeNominalRecordAdmissionDomain<'_> {
    pub const fn owner(&self) -> &RuntimeNominalRecordConstructionOwner;
    pub const fn layout(&self) -> &RuntimeNominalRecordLayout;
    pub fn try_construct(
        &self,
        fields: Vec<RuntimeValue>,
        budget: &mut RuntimeCheckedTypeWorkBudget,
    ) -> Result<RuntimeNominalRecordValue, RuntimeNominalRecordConstructionError>;
}
```

## AWBC domain and builder

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AwbcNominalRecordDomainId(u32);       // decoder/builder only
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AwbcNominalRecordDomainHandle(u32);   // staging only

pub enum AwbcNominalRecordDomainOrigin {
    Plan { site: RuntimePlanTypedSite },
    Project { root: RuntimeProjectRootId },
    Producer { producer: RuntimeOpaqueTypeProducerId, root: RuntimeProducerRootId },
}

pub struct AwbcNominalRecordDomain {
    origin: AwbcNominalRecordDomainOrigin,
    ty: AwbcTypeId,
}

pub enum AwbcRecordConstruction {
    Structural { ty: AwbcTypeId, field_names: Box<[AwbcStringId]> },
    Nominal { domain: AwbcNominalRecordDomainId },
}

pub enum AwbcRecordConstructionDraft {
    Structural { ty: AwbcTypeId, field_names: Box<[AwbcStringId]> },
    Nominal { domain: AwbcNominalRecordDomainHandle },
}

impl AwbcNominalRecordDomain {
    pub const fn origin(&self) -> &AwbcNominalRecordDomainOrigin;
    pub const fn ty(&self) -> AwbcTypeId;
}

pub struct AwbcProgramBuilder(AwbcProgramBuilderState);

impl AwbcProgramBuilder {
    pub fn new(header: AwbcHeader) -> Result<Self, AwbcProgramBuildError>;
    pub fn intern_string(&mut self, value: String) -> Result<AwbcStringId, AwbcProgramBuildError>;
    pub fn intern_runtime_type(&mut self, value: AwbcRuntimeType) -> Result<AwbcTypeId, AwbcProgramBuildError>;
    pub fn push_constant(&mut self, value: AwbcConstantDraft) -> Result<AwbcConstantId, AwbcProgramBuildError>;
    pub fn push_effect_set(&mut self, value: AwbcEffectSet) -> Result<AwbcEffectSetId, AwbcProgramBuildError>;
    pub fn push_signature(&mut self, value: AwbcSignature) -> Result<AwbcSignatureId, AwbcProgramBuildError>;
    pub fn push_frame_layout(&mut self, value: AwbcFrameLayout) -> Result<AwbcFrameLayoutId, AwbcProgramBuildError>;
    pub fn push_function(&mut self, value: AwbcFunction) -> Result<AwbcFunctionId, AwbcProgramBuildError>;
    pub fn push_block(&mut self, value: AwbcBlock) -> Result<AwbcBlockId, AwbcProgramBuildError>;
    pub fn push_instruction(&mut self, value: AwbcInstructionDraft) -> Result<AwbcInstructionId, AwbcProgramBuildError>;
    pub fn push_resume_point(&mut self, value: AwbcResumePoint) -> Result<AwbcResumePointId, AwbcProgramBuildError>;
    pub fn push_pattern(&mut self, value: AwbcPattern) -> Result<AwbcPatternId, AwbcProgramBuildError>;
    pub fn push_match_arm(&mut self, value: AwbcMatchArm) -> Result<AwbcMatchArmId, AwbcProgramBuildError>;
    pub fn push_intrinsic(&mut self, value: AwbcIntrinsic) -> Result<AwbcIntrinsicId, AwbcProgramBuildError>;
    pub fn push_host_call(&mut self, value: AwbcHostCall) -> Result<AwbcHostCallId, AwbcProgramBuildError>;
    pub fn push_task_plan(&mut self, value: AwbcTaskPlan) -> Result<AwbcTaskPlanId, AwbcProgramBuildError>;
    pub fn push_audio_command(&mut self, value: AwbcAudioCommand) -> Result<AwbcAudioCommandId, AwbcProgramBuildError>;
    pub fn push_effect_plan(&mut self, value: AwbcEffectPlan) -> Result<AwbcEffectPlanId, AwbcProgramBuildError>;
    pub fn push_choice(&mut self, value: AwbcChoice) -> Result<AwbcChoiceId, AwbcProgramBuildError>;
    pub fn push_choice_option(&mut self, value: AwbcChoiceOption) -> Result<AwbcChoiceOptionId, AwbcProgramBuildError>;
    pub fn push_content_unit(&mut self, value: AwbcContentUnit) -> Result<AwbcContentUnitId, AwbcProgramBuildError>;
    pub fn push_line_task_group(&mut self, value: AwbcLineTaskGroup) -> Result<AwbcLineTaskGroupId, AwbcProgramBuildError>;
    pub fn push_line_task_node(&mut self, value: AwbcLineTaskNode) -> Result<AwbcLineTaskNodeId, AwbcProgramBuildError>;
    pub fn push_stream_plan(&mut self, value: AwbcStreamPlan) -> Result<AwbcStreamPlanId, AwbcProgramBuildError>;
    pub fn push_source_plan(&mut self, value: AwbcSourcePlan) -> Result<AwbcSourcePlanId, AwbcProgramBuildError>;
    pub fn push_pure_helper(&mut self, value: AwbcPureHelper) -> Result<AwbcPureHelperId, AwbcProgramBuildError>;
    pub fn push_trait_method(&mut self, value: AwbcTraitMethod) -> Result<AwbcTraitMethodId, AwbcProgramBuildError>;
    pub fn push_display_map(&mut self, value: AwbcDisplayMapEntry) -> Result<AwbcDisplayMapId, AwbcProgramBuildError>;
    pub fn push_source_map(&mut self, value: AwbcSourceMapEntry) -> Result<AwbcSourceMapId, AwbcProgramBuildError>;
    pub fn push_resource(&mut self, value: AwbcResourceRef) -> Result<AwbcResourceId, AwbcProgramBuildError>;
    pub fn push_callable_executable(&mut self, value: AwbcCallableExecutable) -> Result<u32, AwbcProgramBuildError>;
    pub fn push_flow_binding(&mut self, value: AwbcFlowBinding) -> Result<u32, AwbcProgramBuildError>;
    pub fn push_flow_executable(&mut self, value: AwbcFlowExecutable) -> Result<u32, AwbcProgramBuildError>;
    pub fn push_entry(&mut self, value: AwbcEntry) -> Result<AwbcEntryId, AwbcProgramBuildError>;

    pub fn intern_nominal_record_domain(
        &mut self,
        origin: AwbcNominalRecordDomainOrigin,
        ty: AwbcTypeId,
    ) -> Result<AwbcNominalRecordDomainHandle, AwbcProgramBuildError>;
    pub fn push_make_record(
        &mut self,
        dst: AwbcRegisterId,
        construction: AwbcRecordConstructionDraft,
        fields: impl IntoIterator<Item = AwbcRegisterId>,
    ) -> Result<(), AwbcProgramBuildError>;
    pub fn push_record_constant(
        &mut self,
        construction: AwbcRecordConstructionDraft,
        fields: impl IntoIterator<Item = AwbcConstantId>,
    ) -> Result<AwbcConstantId, AwbcProgramBuildError>;
    pub fn finish(self) -> Result<AwbcProgram, AwbcProgramBuildError>;
}

impl AwbcProgram {
    pub const fn header(&self) -> &AwbcHeader;
    pub const fn strings(&self) -> &[String];
    pub const fn runtime_types(&self) -> &[AwbcRuntimeType];
    pub const fn nominal_record_domains(&self) -> &[AwbcNominalRecordDomain];
    pub const fn constants(&self) -> &[AwbcConstant];
    pub const fn effect_sets(&self) -> &[AwbcEffectSet];
    pub const fn signatures(&self) -> &[AwbcSignature];
    pub const fn frame_layouts(&self) -> &[AwbcFrameLayout];
    pub const fn functions(&self) -> &[AwbcFunction];
    pub const fn blocks(&self) -> &[AwbcBlock];
    pub const fn instructions(&self) -> &[AwbcInstruction];
    pub const fn resume_points(&self) -> &[AwbcResumePoint];
    pub const fn patterns(&self) -> &[AwbcPattern];
    pub const fn match_arms(&self) -> &[AwbcMatchArm];
    pub const fn intrinsics(&self) -> &[AwbcIntrinsic];
    pub const fn host_calls(&self) -> &[AwbcHostCall];
    pub const fn task_plans(&self) -> &[AwbcTaskPlan];
    pub const fn audio_commands(&self) -> &[AwbcAudioCommand];
    pub const fn effect_plans(&self) -> &[AwbcEffectPlan];
    pub const fn choices(&self) -> &[AwbcChoice];
    pub const fn choice_options(&self) -> &[AwbcChoiceOption];
    pub const fn content_units(&self) -> &[AwbcContentUnit];
    pub const fn line_task_groups(&self) -> &[AwbcLineTaskGroup];
    pub const fn line_task_nodes(&self) -> &[AwbcLineTaskNode];
    pub const fn stream_plans(&self) -> &[AwbcStreamPlan];
    pub const fn source_plans(&self) -> &[AwbcSourcePlan];
    pub const fn pure_helpers(&self) -> &[AwbcPureHelper];
    pub const fn trait_methods(&self) -> &[AwbcTraitMethod];
    pub const fn display_map(&self) -> &[AwbcDisplayMapEntry];
    pub const fn source_map(&self) -> &[AwbcSourceMapEntry];
    pub const fn resources(&self) -> &[AwbcResourceRef];
    pub const fn callable_executables(&self) -> &[AwbcCallableExecutable];
    pub const fn flow_bindings(&self) -> &[AwbcFlowBinding];
    pub const fn flow_executables(&self) -> &[AwbcFlowExecutable];
    pub const fn entries(&self) -> &[AwbcEntry];
}
```

The draft construction enum uses the opaque handle for nominal rows; only
`finish` creates `AwbcNominalRecordDomainId` and final instructions/constants.

## Compiler, bundle, driver, and backend evidence

```rust
// arcweft-bundle: byte/container verifier only; it never depends on compiler.
pub enum RuntimeBundleTrustPolicy<'a> {
    TrustedIntegrator,
    RequireTrustedEd25519(&'a RuntimeBundleTrustedKeys),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeBundleTrustedKeyId([u8; 32]);

impl RuntimeBundleTrustedKeyId {
    pub const fn as_bytes(&self) -> &[u8; 32];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBundleAuthentication {
    TrustedIntegrator,
    TrustedEd25519 { key: RuntimeBundleTrustedKeyId },
}

pub struct RuntimeBundleEvidence {
    generation_identity: RuntimeGenerationIdentity,
    plan_digest: RuntimeDigest,
    awbc_digest: RuntimeDigest,
    container_digest: RuntimeDigest,
    authentication: RuntimeBundleAuthentication,
}

impl RuntimeBundleEvidence {
    pub const fn generation_identity(&self) -> RuntimeGenerationIdentity;
    pub const fn plan_digest(&self) -> RuntimeDigest;
    pub const fn awbc_digest(&self) -> RuntimeDigest;
    pub const fn container_digest(&self) -> RuntimeDigest;
    pub const fn authentication(&self) -> RuntimeBundleAuthentication;
}

pub struct VerifiedRuntimeBundleProduct {
    bytes: Arc<[u8]>,
    product: AdmittedRuntimeProduct,
    evidence: RuntimeBundleEvidence,
}

impl VerifiedRuntimeBundleProduct {
    pub const fn bytes(&self) -> &[u8];
    pub const fn product(&self) -> &AdmittedRuntimeProduct;
    pub const fn evidence(&self) -> &RuntimeBundleEvidence;
}

pub fn verify_runtime_bundle_product(
    bytes: &[u8],
    external: RuntimeBundleExternalSections<'_>,
    policy: RuntimeBundleTrustPolicy<'_>,
    limits: RuntimeBundleLimits,
) -> Result<VerifiedRuntimeBundleProduct, RuntimeBundleVerifyError>;

pub fn verify_runtime_bundle_product_for_parent(
    bytes: &[u8],
    parent: Arc<AdmittedRuntimeGeneration>,
    external: RuntimeBundleExternalSections<'_>,
    policy: RuntimeBundleTrustPolicy<'_>,
    limits: RuntimeBundleLimits,
) -> Result<VerifiedRuntimeBundleProduct, RuntimeBundleVerifyError>;

// arcweft-compiler: official in-process provenance and bridge into the same
// bundle transcript/verifier. Compiler already depends on bundle.
pub struct CompilerRuntimeEvidence {
    generation_identity: RuntimeGenerationIdentity,
    plan_digest: RuntimeDigest,
    awbc_digest: RuntimeDigest,
    final_hir_snapshot_digest: RuntimeDigest,
    semantic_owner_transcript_digest: RuntimeDigest,
}

impl CompilerRuntimeEvidence {
    pub const fn generation_identity(&self) -> RuntimeGenerationIdentity;
    pub const fn plan_digest(&self) -> RuntimeDigest;
    pub const fn awbc_digest(&self) -> RuntimeDigest;
    pub const fn final_hir_snapshot_digest(&self) -> RuntimeDigest;
    pub const fn semantic_owner_transcript_digest(&self) -> RuntimeDigest;
}

pub struct CompilerRuntimeProduct {
    product: AdmittedRuntimeProduct,
    evidence: CompilerRuntimeEvidence,
}

impl CompilerRuntimeProduct {
    pub const fn product(&self) -> &AdmittedRuntimeProduct;
    pub const fn evidence(&self) -> &CompilerRuntimeEvidence;
}

pub fn compile_runtime_product(
    project: HirExecutableProjectView<'_>,
    analysis: &FinalSemanticAnalysis,
    catalogs: RuntimeCompilationCatalogs<'_>,
    options: RuntimeCompilationOptions,
) -> Result<CompilerRuntimeProduct, ProjectCompileError>;

pub fn verify_compiler_runtime_product(
    compiled: CompilerRuntimeProduct,
    external: RuntimeBundleExternalSections<'_>,
    policy: RuntimeBundleTrustPolicy<'_>,
    limits: RuntimeBundleLimits,
) -> Result<VerifiedRuntimeBundleProduct, CompilerRuntimeBundleBridgeError>;

// arcweft-runtime-driver: publication and the direct core VM only.
pub struct PublishedRuntimeGeneration {
    verified: VerifiedRuntimeBundleProduct,
    publication: Arc<RuntimePublicationState>,
}

impl PublishedRuntimeGeneration {
    pub const fn product(&self) -> &AdmittedRuntimeProduct;
    pub const fn evidence(&self) -> &RuntimeBundleEvidence;
    pub const fn publication(&self) -> &RuntimePublicationState;

    pub fn verify_hot_swap_candidate(
        &self,
        bytes: &[u8],
        external: RuntimeBundleExternalSections<'_>,
        policy: RuntimeBundleTrustPolicy<'_>,
        limits: RuntimeBundleLimits,
    ) -> Result<VerifiedRuntimeBundleProduct, RuntimeBundleVerifyError>;

    pub fn prepare_hot_swap(
        &self,
        next: VerifiedRuntimeBundleProduct,
        host: RuntimeHostBindings<'_>,
        policy: RuntimeHotSwapPolicy<'_>,
    ) -> Result<PreparedRuntimeGenerationSwap, RuntimeHotSwapError>;
}

pub struct PreparedRuntimeGenerationSwap {
    current_parent: Arc<AdmittedRuntimeGeneration>,
    next: PublishedRuntimeGeneration,
    migrated_state: RuntimeMigratedState,
}

impl PreparedRuntimeGenerationSwap {
    pub fn commit(
        self,
        current: &mut PublishedRuntimeGeneration,
    ) -> Result<RuntimeHotSwapReceipt, RuntimeHotSwapCommitError>;
}

pub fn publish_runtime_generation(
    verified: VerifiedRuntimeBundleProduct,
    host: RuntimeHostBindings<'_>,
    policy: RuntimePublicationPolicy<'_>,
) -> Result<PublishedRuntimeGeneration, RuntimePublicationError>;

pub fn publish_and_start_vm(
    verified: VerifiedRuntimeBundleProduct,
    host: RuntimeHostBindings<'_>,
    policy: RuntimePublicationPolicy<'_>,
    vm: RuntimeVmOptions,
) -> Result<PublishedVm, RuntimePublicationError>;

pub fn publish_and_restore(
    verified: VerifiedRuntimeBundleProduct,
    host: RuntimeHostBindings<'_>,
    publication: RuntimePublicationPolicy<'_>,
    snapshot_bytes: &[u8],
    restore: RuntimeRestorePolicy,
    limits: RuntimeRestoreLimits,
) -> Result<RestoredRuntimeGeneration, RuntimeRestoreError>;

pub fn replay_published_runtime(
    published: &mut PublishedRuntimeGeneration,
    replay_bytes: &[u8],
    policy: RuntimeReplayPolicy,
    limits: RuntimeReplayLimits,
) -> Result<RuntimeReplayReceipt, RuntimeReplayError>;

// arcweft-lang-jit-cranelift: adds a dependency on runtime-driver.
pub fn prepare_published_jit(
    published: &PublishedRuntimeGeneration,
    options: RuntimeJitOptions,
) -> Result<PublishedJit, RuntimeJitPrepareError>;

// arcweft-runtime-codegen: adds a dependency on runtime-driver.
pub fn prepare_published_aot(
    published: &PublishedRuntimeGeneration,
    target: RuntimeCodegenTarget,
) -> Result<PublishedAotArtifact, RuntimeCodegenError>;

// arcweft-runtime-accelerator: same publication token, no authority issuance.
pub fn prepare_published_accelerator(
    published: &PublishedRuntimeGeneration,
    options: RuntimeAcceleratorOptions,
) -> Result<PublishedAccelerator, RuntimeAcceleratorError>;
```

The compiler bridge owns serialization of the compiler product to the same
in-memory version-1 bundle transcript and calls the public bundle verifier.
`arcweft-bundle` never imports a compiler type. JIT, AOT, and accelerator crates
accept only `PublishedRuntimeGeneration`; their previous raw-plan/raw-AWBC entry
surfaces are deleted in the same dependency cut. `arcweft-save` remains a lower
Sans-I/O envelope codec; runtime-driver performs product-context semantic value
admission above it and save never depends on runtime-driver.
