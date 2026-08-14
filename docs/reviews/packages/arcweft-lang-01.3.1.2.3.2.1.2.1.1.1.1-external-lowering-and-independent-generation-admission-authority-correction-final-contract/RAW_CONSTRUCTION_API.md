# Exact public checked raw-construction API

## Plan types and wrappers

Owner: `crates/arcweft-core/src/plan/typed_sites.rs`, re-exported narrowly from
`arcweft_core::plan`.

```rust
pub const MAX_RUNTIME_INDEX_PATH_SEGMENTS: u32 = 64;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeIndexPath(Box<[u32]>);

impl RuntimeIndexPath {
    pub fn root() -> Self;
    pub fn try_from_indices(
        indices: impl IntoIterator<Item = u32>,
    ) -> Result<Self, RuntimeIndexPathError>;
    pub fn child(&self, ordinal: u32) -> Result<Self, RuntimeIndexPathError>;
    pub fn segments(&self) -> &[u32];
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct RuntimePlanTypeId(u32);

impl RuntimePlanTypeId {
    pub const fn index(self) -> u32;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum RuntimeTypeAuthorityDeclaration {
    Project { root: RuntimeProjectRootId },
    Producer {
        producer: RuntimeOpaqueTypeProducerId,
        root: RuntimeProducerRootId,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
pub enum RuntimeOperationalType {
    MatrixF32 = 0,
    MatrixF64 = 1,
    TensorF32 = 2,
    TensorF64 = 3,
    Range = 4,
    Iterator = 5,
    StructuralRecord = 6,
    Map = 7,
    Need = 8,
    Stream = 9,
    Source = 10,
    ThreadHandle = 11,
    Shared = 12,
    Reference = 13,
    Function = 14,
    Sequence = 15,
    Tuple = 16,
    Choice = 17,
    Result = 18,
    Option = 19,
    Variant = 20,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum RuntimePlanTypeKind {
    Checked {
        checked_type: RuntimeCheckedType,
        authority: RuntimeTypeAuthorityDeclaration,
    },
    Operational(RuntimeOperationalType),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimePlanTypeDeclaration {
    semantic_identity: RuntimeSemanticTypeId,
    kind: RuntimePlanTypeKind,
}

impl RuntimePlanTypeDeclaration {
    pub fn try_checked(
        semantic_identity: RuntimeSemanticTypeId,
        checked_type: RuntimeCheckedType,
        authority: RuntimeTypeAuthorityDeclaration,
    ) -> Result<Self, RuntimePlanTypeDeclarationError>;

    pub fn try_operational(
        semantic_identity: RuntimeSemanticTypeId,
        shape: RuntimeOperationalType,
    ) -> Result<Self, RuntimePlanTypeDeclarationError>;

    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;
    pub const fn kind(&self) -> &RuntimePlanTypeKind;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeExprTypeFact {
    path: RuntimeIndexPath,
    ty: RuntimePlanTypeId,
}

impl RuntimeExprTypeFact {
    pub fn new(path: RuntimeIndexPath, ty: RuntimePlanTypeId) -> Self;
    pub const fn path(&self) -> &RuntimeIndexPath;
    pub const fn ty(&self) -> RuntimePlanTypeId;
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RuntimeTypedExpr {
    expr: RuntimeExpr,
    nodes: Box<[RuntimeExprTypeFact]>,
}

impl RuntimeTypedExpr {
    pub fn try_new(
        expr: RuntimeExpr,
        nodes: impl IntoIterator<Item = RuntimeExprTypeFact>,
    ) -> Result<Self, RuntimeTypedExprConstructionError>;
    pub const fn expr(&self) -> &RuntimeExpr;
    pub const fn nodes(&self) -> &[RuntimeExprTypeFact];
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimePatternTypeFact {
    path: RuntimeIndexPath,
    expected: RuntimePlanTypeId,
}

impl RuntimePatternTypeFact {
    pub fn new(path: RuntimeIndexPath, expected: RuntimePlanTypeId) -> Self;
    pub const fn path(&self) -> &RuntimeIndexPath;
    pub const fn expected(&self) -> RuntimePlanTypeId;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimePatternBindingFact {
    path: RuntimeIndexPath,
    binding: RuntimePatternBindingCoordinate,
    ty: RuntimePlanTypeId,
}

impl RuntimePatternBindingFact {
    pub fn new(
        path: RuntimeIndexPath,
        binding: RuntimePatternBindingCoordinate,
        ty: RuntimePlanTypeId,
    ) -> Self;
    pub const fn path(&self) -> &RuntimeIndexPath;
    pub const fn binding(&self) -> RuntimePatternBindingCoordinate;
    pub const fn ty(&self) -> RuntimePlanTypeId;
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RuntimeTypedPattern {
    pattern: RuntimePattern,
    nodes: Box<[RuntimePatternTypeFact]>,
    bindings: Box<[RuntimePatternBindingFact]>,
}

impl RuntimeTypedPattern {
    pub fn try_new(
        pattern: RuntimePattern,
        nodes: impl IntoIterator<Item = RuntimePatternTypeFact>,
        bindings: impl IntoIterator<Item = RuntimePatternBindingFact>,
    ) -> Result<Self, RuntimeTypedPatternConstructionError>;
    pub const fn pattern(&self) -> &RuntimePattern;
    pub const fn nodes(&self) -> &[RuntimePatternTypeFact];
    pub const fn bindings(&self) -> &[RuntimePatternBindingFact];
}
```

Every invariant-bearing type manually implements `Deserialize` through one
private `*WireV1` DTO and the public checked constructor above. No wire DTO is
re-exported. `RuntimePlanTypeId` is returned only by the plan builder; it has no
public constructor or `From<u32>`.

## Plan aggregate builder

Owner: `crates/arcweft-core/src/plan/construction.rs`.

```rust
pub struct RuntimePlanBuilder {
    generation: RuntimeGenerationContractDeclaration,
    // all staging tables private
}

impl RuntimePlanBuilder {
    pub fn new(generation: RuntimeGenerationContractDeclaration) -> Self;

    pub fn push_type(
        &mut self,
        declaration: RuntimePlanTypeDeclaration,
    ) -> Result<RuntimePlanTypeId, RuntimePlanBuildError>;

    pub fn push_entry(
        &mut self,
        entry: RuntimeEntrySpec,
    ) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_callable_executable(
        &mut self,
        executable: RuntimeCallableExecutable,
    ) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_flow_executable(
        &mut self,
        executable: RuntimeFlowExecutable,
    ) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_flow(
        &mut self,
        flow: RuntimeFlow,
    ) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_pure_helper(
        &mut self,
        helper: RuntimePureHelper,
    ) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_trait_method(
        &mut self,
        method: RuntimeTraitMethod,
    ) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_line_task_group(
        &mut self,
        group: LineTaskGroup,
    ) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_stream_plan(
        &mut self,
        plan: StreamPlan,
    ) -> Result<u32, RuntimePlanBuildError>;
    pub fn push_source_plan(
        &mut self,
        plan: SourcePlan,
    ) -> Result<u32, RuntimePlanBuildError>;

    pub fn finish(self) -> Result<RuntimePlan, RuntimePlanBuildError>;
}

impl RuntimePlan {
    pub const fn generation_contract(&self) -> &RuntimeGenerationContractDeclaration;
    pub const fn type_declarations(&self) -> &[RuntimePlanTypeDeclaration];
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

`RuntimePlan` fields become private; `Default` and derived `Deserialize` are
deleted. Its custom `Deserialize` decodes private `RuntimePlanWireV1`, invokes
exactly these push methods in wire order, and calls `finish`.

## AWBC typed primitives and aggregate builder

Owners: `crates/arcweft-core/src/awbc/typed_sites.rs` and
`crates/arcweft-core/src/awbc/construction.rs`.

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AwbcRuntimeTypeDeclaration {
    semantic_identity: RuntimeSemanticTypeId,
    kind: RuntimePlanTypeKind,
}

impl AwbcRuntimeTypeDeclaration {
    pub fn try_new(
        semantic_identity: RuntimeSemanticTypeId,
        kind: RuntimePlanTypeKind,
    ) -> Result<Self, AwbcRuntimeTypeDeclarationError>;
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;
    pub const fn kind(&self) -> &RuntimePlanTypeKind;
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AwbcTypedConstant {
    value: AwbcConstant,
    ty: AwbcTypeId,
}

impl AwbcTypedConstant {
    pub fn new(value: AwbcConstant, ty: AwbcTypeId) -> Self;
    pub const fn value(&self) -> &AwbcConstant;
    pub const fn ty(&self) -> AwbcTypeId;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AwbcTypedPattern {
    pattern: AwbcPattern,
    expected: AwbcTypeId,
}

impl AwbcTypedPattern {
    pub fn new(pattern: AwbcPattern, expected: AwbcTypeId) -> Self;
    pub const fn pattern(&self) -> &AwbcPattern;
    pub const fn expected(&self) -> AwbcTypeId;
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AwbcTypedOrigin {
    plan_site: RuntimePlanTypedSite,
    awbc_site: AwbcTypedSite,
}

impl AwbcTypedOrigin {
    pub fn new(plan_site: RuntimePlanTypedSite, awbc_site: AwbcTypedSite) -> Self;
    pub const fn plan_site(&self) -> &RuntimePlanTypedSite;
    pub const fn awbc_site(&self) -> &AwbcTypedSite;
}

pub struct AwbcProgramBuilder {
    header: AwbcHeader,
    generation: RuntimeGenerationContractDeclaration,
    // all staging tables private
}

impl AwbcProgramBuilder {
    pub fn new(
        header: AwbcHeader,
        generation: RuntimeGenerationContractDeclaration,
    ) -> Result<Self, AwbcProgramBuildError>;

    pub fn push_string(
        &mut self,
        value: String,
    ) -> Result<AwbcStringId, AwbcProgramBuildError>;
    pub fn push_runtime_type(
        &mut self,
        declaration: AwbcRuntimeTypeDeclaration,
    ) -> Result<AwbcTypeId, AwbcProgramBuildError>;
    pub fn push_constant(
        &mut self,
        constant: AwbcTypedConstant,
    ) -> Result<AwbcConstantId, AwbcProgramBuildError>;
    pub fn push_effect_set(
        &mut self,
        effect_set: AwbcEffectSet,
    ) -> Result<AwbcEffectSetId, AwbcProgramBuildError>;
    pub fn push_signature(
        &mut self,
        signature: AwbcSignature,
    ) -> Result<AwbcSignatureId, AwbcProgramBuildError>;
    pub fn push_frame_layout(
        &mut self,
        frame: AwbcFrameLayout,
    ) -> Result<AwbcFrameLayoutId, AwbcProgramBuildError>;
    pub fn push_function(
        &mut self,
        function: AwbcFunction,
    ) -> Result<AwbcFunctionId, AwbcProgramBuildError>;
    pub fn push_block(
        &mut self,
        block: AwbcBlock,
    ) -> Result<AwbcBlockId, AwbcProgramBuildError>;
    pub fn push_instruction(
        &mut self,
        instruction: AwbcInstruction,
    ) -> Result<AwbcInstructionId, AwbcProgramBuildError>;
    pub fn push_resume_point(
        &mut self,
        point: AwbcResumePoint,
    ) -> Result<AwbcResumePointId, AwbcProgramBuildError>;
    pub fn push_pattern(
        &mut self,
        pattern: AwbcTypedPattern,
    ) -> Result<AwbcPatternId, AwbcProgramBuildError>;
    pub fn push_match_arm(
        &mut self,
        arm: AwbcMatchArm,
    ) -> Result<AwbcMatchArmId, AwbcProgramBuildError>;
    pub fn push_intrinsic(
        &mut self,
        intrinsic: AwbcIntrinsic,
    ) -> Result<AwbcIntrinsicId, AwbcProgramBuildError>;
    pub fn push_host_call(
        &mut self,
        call: AwbcHostCall,
    ) -> Result<AwbcHostCallId, AwbcProgramBuildError>;
    pub fn push_task_plan(
        &mut self,
        plan: AwbcTaskPlan,
    ) -> Result<AwbcTaskPlanId, AwbcProgramBuildError>;
    pub fn push_audio_command(
        &mut self,
        command: AwbcAudioCommand,
    ) -> Result<AwbcAudioCommandId, AwbcProgramBuildError>;
    pub fn push_effect_plan(
        &mut self,
        plan: AwbcEffectPlan,
    ) -> Result<AwbcEffectPlanId, AwbcProgramBuildError>;
    pub fn push_choice(
        &mut self,
        choice: AwbcChoice,
    ) -> Result<AwbcChoiceId, AwbcProgramBuildError>;
    pub fn push_choice_option(
        &mut self,
        option: AwbcChoiceOption,
    ) -> Result<AwbcChoiceOptionId, AwbcProgramBuildError>;
    pub fn push_content_unit(
        &mut self,
        unit: AwbcContentUnit,
    ) -> Result<AwbcContentUnitId, AwbcProgramBuildError>;
    pub fn push_line_task_group(
        &mut self,
        group: AwbcLineTaskGroup,
    ) -> Result<AwbcLineTaskGroupId, AwbcProgramBuildError>;
    pub fn push_line_task_node(
        &mut self,
        node: AwbcLineTaskNode,
    ) -> Result<AwbcLineTaskNodeId, AwbcProgramBuildError>;
    pub fn push_stream_plan(
        &mut self,
        plan: AwbcStreamPlan,
    ) -> Result<AwbcStreamPlanId, AwbcProgramBuildError>;
    pub fn push_source_plan(
        &mut self,
        plan: AwbcSourcePlan,
    ) -> Result<AwbcSourcePlanId, AwbcProgramBuildError>;
    pub fn push_pure_helper(
        &mut self,
        helper: AwbcPureHelper,
    ) -> Result<AwbcPureHelperId, AwbcProgramBuildError>;
    pub fn push_trait_method(
        &mut self,
        method: AwbcTraitMethod,
    ) -> Result<AwbcTraitMethodId, AwbcProgramBuildError>;
    pub fn push_display_map(
        &mut self,
        entry: AwbcDisplayMapEntry,
    ) -> Result<AwbcDisplayMapId, AwbcProgramBuildError>;
    pub fn push_source_map(
        &mut self,
        entry: AwbcSourceMapEntry,
    ) -> Result<AwbcSourceMapId, AwbcProgramBuildError>;
    pub fn push_resource(
        &mut self,
        resource: AwbcResourceRef,
    ) -> Result<AwbcResourceId, AwbcProgramBuildError>;
    pub fn push_callable_executable(
        &mut self,
        executable: AwbcCallableExecutable,
    ) -> Result<u32, AwbcProgramBuildError>;
    pub fn push_flow_binding(
        &mut self,
        binding: AwbcFlowBinding,
    ) -> Result<u32, AwbcProgramBuildError>;
    pub fn push_flow_executable(
        &mut self,
        executable: AwbcFlowExecutable,
    ) -> Result<u32, AwbcProgramBuildError>;
    pub fn push_entry(
        &mut self,
        entry: AwbcEntry,
    ) -> Result<AwbcEntryId, AwbcProgramBuildError>;

    pub fn push_typed_origin(
        &mut self,
        origin: AwbcTypedOrigin,
    ) -> Result<(), AwbcProgramBuildError>;

    pub fn finish(self) -> Result<AwbcProgram, AwbcProgramBuildError>;
}

impl AwbcProgram {
    pub const fn header(&self) -> &AwbcHeader;
    pub const fn generation_contract(&self) -> &RuntimeGenerationContractDeclaration;
    pub fn strings(&self) -> &[String];
    pub fn runtime_types(&self) -> &[AwbcRuntimeTypeDeclaration];
    pub fn constants(&self) -> &[AwbcTypedConstant];
    pub fn effect_sets(&self) -> &[AwbcEffectSet];
    pub fn signatures(&self) -> &[AwbcSignature];
    pub fn frame_layouts(&self) -> &[AwbcFrameLayout];
    pub fn functions(&self) -> &[AwbcFunction];
    pub fn blocks(&self) -> &[AwbcBlock];
    pub fn instructions(&self) -> &[AwbcInstruction];
    pub fn resume_points(&self) -> &[AwbcResumePoint];
    pub fn patterns(&self) -> &[AwbcTypedPattern];
    pub fn match_arms(&self) -> &[AwbcMatchArm];
    pub fn intrinsics(&self) -> &[AwbcIntrinsic];
    pub fn host_calls(&self) -> &[AwbcHostCall];
    pub fn task_plans(&self) -> &[AwbcTaskPlan];
    pub fn audio_commands(&self) -> &[AwbcAudioCommand];
    pub fn effect_plans(&self) -> &[AwbcEffectPlan];
    pub fn choices(&self) -> &[AwbcChoice];
    pub fn choice_options(&self) -> &[AwbcChoiceOption];
    pub fn content_units(&self) -> &[AwbcContentUnit];
    pub fn line_task_groups(&self) -> &[AwbcLineTaskGroup];
    pub fn line_task_nodes(&self) -> &[AwbcLineTaskNode];
    pub fn stream_plans(&self) -> &[AwbcStreamPlan];
    pub fn source_plans(&self) -> &[AwbcSourcePlan];
    pub fn pure_helpers(&self) -> &[AwbcPureHelper];
    pub fn trait_methods(&self) -> &[AwbcTraitMethod];
    pub fn display_map(&self) -> &[AwbcDisplayMapEntry];
    pub fn source_map(&self) -> &[AwbcSourceMapEntry];
    pub fn resources(&self) -> &[AwbcResourceRef];
    pub fn callable_executables(&self) -> &[AwbcCallableExecutable];
    pub fn flow_bindings(&self) -> &[AwbcFlowBinding];
    pub fn flow_executables(&self) -> &[AwbcFlowExecutable];
    pub fn entries(&self) -> &[AwbcEntry];
    pub fn typed_origins(&self) -> &[AwbcTypedOrigin];
}
```

`AwbcProgram` fields become private; `Default` and derived `Deserialize` are
deleted. `AwbcProgramWireV1` is private and decodes through the same builder.
A valid decoded program remains raw data until an admitted generation accepts
it.

## Error owners

```rust
pub enum RuntimePlanBuildError {
    InvalidGenerationVersion { actual: u16 },
    TooManyRows { table: RuntimePlanTable, maximum: u32, actual: usize },
    DuplicateTypeDeclaration { semantic_identity: RuntimeSemanticTypeId },
    TypeIdOutOfBounds { owner: RuntimePlanBuildOwner, ty: RuntimePlanTypeId },
    InvalidTypedExpr { owner: RuntimePlanBuildOwner, source: RuntimeTypedExprConstructionError },
    InvalidTypedPattern { owner: RuntimePlanBuildOwner, source: RuntimeTypedPatternConstructionError },
    InvalidTableReference { owner: RuntimePlanBuildOwner, target: RuntimePlanTableReference },
    NonCanonicalOwnerOrder { table: RuntimePlanTable, previous: u32, actual: u32 },
}

pub enum AwbcProgramBuildError {
    InvalidHeaderVersion { abi: u32, codec: u16 },
    TooManyRows { table: AwbcTable, maximum: u32, actual: usize },
    RangeOverflow { owner: AwbcBuildOwner, start: u32, len: u32 },
    ReferenceOutOfBounds { owner: AwbcBuildOwner, target: AwbcTableReference },
    DuplicateOrigin { plan_site: RuntimePlanTypedSite, awbc_site: AwbcTypedSite },
    NonCanonicalOriginOrder { previous: AwbcTypedOrigin, actual: AwbcTypedOrigin },
    InvalidTypedSite { source: AwbcTypedSiteConstructionError },
    InvalidGraphCycle { graph: AwbcTypedGraph, at: u32 },
}
```

Both errors are `#[non_exhaustive]` only if current repository policy requires
future external matching flexibility; no error is represented by a string or
sentinel.
