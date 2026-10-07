//! Sole mutable construction authority for a runtime plan.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;
use std::sync::Arc;

use thiserror::Error;

mod callable_specialization;
mod callable_states;
mod control_effect;
mod lower;
mod nominal_schema;
mod seed;
pub(crate) mod task_coordinates;

pub use callable_specialization::{
    RuntimeCallableSpecializationSeed, RuntimeCallableSpecializationSeedId,
};
pub use callable_states::{RuntimeCallableStateSeed, RuntimeCallableStateSeedId};
pub use control_effect::{RuntimeControlEffectContractSeed, RuntimeControlEffectContractSeedId};
pub use nominal_schema::{RuntimePlanNominalSchemaError, RuntimePlanSchemaComponent};

#[cfg(test)]
mod nominal_domains_tests;

use lower::{function_input_scope, require_same};
use seed::RuntimePlanConstructionIssuer;
pub use seed::{
    RuntimeAgentExprSeed, RuntimeAssignmentSeed, RuntimeAudioCommandSeed,
    RuntimeAwaitManyTargetSeed, RuntimeAwaitPendingObserverSeed, RuntimeAwaitTargetSeed,
    RuntimeBorrowedLocalSeed, RuntimeBuiltinIteratorEvidenceSeed, RuntimeCallArgumentSeed,
    RuntimeCallableExecutableSeed, RuntimeCallableExecutableSeedCode, RuntimeCallableParameterSeed,
    RuntimeChoiceOptionSeed, RuntimeDialogueContentEffectBindingSeed,
    RuntimeDialogueContentEffectSlotSeed, RuntimeDialogueContentPlanSeed,
    RuntimeDialogueContentPlanSeedId, RuntimeDialogueContentSlotSeed,
    RuntimeDialogueContentTemplateManifestSeed, RuntimeDialogueEffectSiteSeed,
    RuntimeDialogueMarkSeedId, RuntimeDialogueResultTargetSeed,
    RuntimeDialogueResultTargetSeedError, RuntimeDialogueValueSiteSeed, RuntimeDropPolicySeed,
    RuntimeEffectFieldSeed, RuntimeEvaluatedEffectSeed, RuntimeExecutableBodySeed,
    RuntimeExprMatchArmSeed, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFieldProjectionSeed,
    RuntimeFlowMatchArmSeed, RuntimeFlowMatchGuardSeed, RuntimeFlowOpSeed, RuntimeFlowSeed,
    RuntimeFormatAttemptDeclarationSeed, RuntimeFormatAttemptOperandSeed,
    RuntimeFormatAttemptSeedId, RuntimeFormatContentOperandSeed, RuntimeFunctionInputBindingSeed,
    RuntimeFunctionSiteBodySeed, RuntimeFunctionSiteDeclarationSeed, RuntimeFunctionSiteSeedId,
    RuntimeHostArgumentSeed, RuntimeHostCallTargetSeed, RuntimeHostTaskRequestTemplateSeed,
    RuntimeIteratorEvidenceSeed, RuntimeIteratorWitnessEvidenceSeed,
    RuntimeIteratorWitnessExecutableSeed, RuntimeLineEffectSeed, RuntimeLineHandleSiteSeed,
    RuntimeLineOperationSeed, RuntimeLineTaskCancelRuleSeed, RuntimeLineTaskGroupSeed,
    RuntimeLineTaskGroupSeedId, RuntimeLineTaskNodeSeed, RuntimeLineTaskNodeSeedId,
    RuntimeLineTaskTriggerSeed, RuntimeLocalDeclarationSeed, RuntimeLocalReadSeed,
    RuntimeLocalSeedId, RuntimeMutablePlaceSeed, RuntimeNeedProducerStartTargetSeed,
    RuntimeNeedProducerTemplateSeed, RuntimeNominalRecordFieldSeed, RuntimePatternRestSeed,
    RuntimePatternSeed, RuntimePatternSeedKind, RuntimePureHelperDeclarationSeed,
    RuntimePureHelperSeed, RuntimePureHelperSeedId, RuntimePureProgramBindingSeed,
    RuntimeRecordFieldSeedId, RuntimeRecordPatternFieldSeed, RuntimeScheduledCaptureSeed,
    RuntimeStreamMatchArmSeed, RuntimeStreamOpSeed, RuntimeStreamPlanSeed,
    RuntimeTraitMethodDeclarationSeed, RuntimeTraitMethodSeed, RuntimeTraitMethodSeedId,
};

use crate::entry::{
    RuntimeCallableExecutable, RuntimeCallableExecutableCode, RuntimeFlowExecutable,
    RuntimeFlowSchema, RuntimeNominalTypeId,
};
use crate::line_task::{
    LineCancelRule, LineTaskCleanup, LineTaskGroup, LineTaskNode, LineTaskTrigger,
    MAX_LINE_HANDLE_SITES, RuntimeLineHandleSite,
};
use crate::pattern::{RuntimePatternBindingPathError, RuntimeSemanticTypeId};
use crate::runtime_id::{
    RuntimeDeferSiteId, RuntimeDialogueContentPlanId, RuntimeDialogueEffectSiteCount,
    RuntimeDialogueMarkId, RuntimeLineHandleSiteId, RuntimeLineTaskGroupId, RuntimeLineTaskNodeId,
    RuntimeLocalDeclarationId, RuntimePlanTypeId,
};
use crate::stream::StreamPlan;
use crate::value::{
    RuntimeAgentConstructor, RuntimeDialogueOpaqueRole, RuntimeDialoguePlainTextContextTemplateRef,
    RuntimeRecordFieldIdError,
};

use super::dialogue_content::RuntimeDialogueContentPlanTableBuilder;
use super::executable_body::{RuntimeEffectSet, RuntimeExecutableBody};
use super::function_sites::{
    RuntimeFunctionInputBinding, RuntimeFunctionInputSource, RuntimeFunctionSiteBody,
    RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteError, RuntimeFunctionSiteTableBuilder,
};
use super::local_declarations::{
    RuntimeLocalDeclarationTableBuilder, RuntimeLocalDeclarationTableError,
};
use super::nominal_record_domains::{
    RuntimeNominalRecordDomain, RuntimeNominalRecordDomainError, RuntimeNominalRecordDomainSeed,
    RuntimeNominalRecordDomainTableBuilder,
};
use super::type_table::{
    PreparedRuntimePlanTypeBatch, RuntimePlanTypeSeed, RuntimePlanTypeTableBuilder,
    RuntimePlanTypeTableError,
};
use super::variant_domains::{
    RuntimeVariantDomain, RuntimeVariantDomainError, RuntimeVariantDomainSeed,
    RuntimeVariantDomainTableBuilder,
};
use super::{
    FlowOp, FlowRuntimeId, RuntimeDialogueContentEffectSlot, RuntimeDialogueContentPlan,
    RuntimeDialogueContentSlot, RuntimeDialogueContentTemplateManifest, RuntimeDialogueEffectSite,
    RuntimeDialogueMark, RuntimeDialogueValueRole, RuntimeDialogueValueSite, RuntimeEntrySpec,
    RuntimeFlow, RuntimeLineOperation, RuntimePlan, RuntimePlanTypeProjection, RuntimePureHelper,
    RuntimePureProgramBinding, RuntimeTraitMethod,
};

/// Result identities issued by one atomic semantic graph transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePlanSemanticAdmission {
    type_ids: Box<[RuntimePlanTypeId]>,
    local_ids: Box<[RuntimeLocalSeedId]>,
}

impl RuntimePlanSemanticAdmission {
    /// Type IDs in input-seed order, issued only after the complete transaction.
    #[must_use]
    pub const fn type_ids(&self) -> &[RuntimePlanTypeId] {
        &self.type_ids
    }

    #[must_use]
    pub const fn local_ids(&self) -> &[RuntimeLocalSeedId] {
        &self.local_ids
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePlanTable {
    DialogueContent,
    FormatAttempts,
    Entries,
    CallableExecutables,
    FlowSchemas,
    FlowExecutables,
    Flows,
    ProjectCallSites,
    PureHelpers,
    PurePrograms,
    TraitMethods,
    LineTaskGroups,
    StreamPlans,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimePlanBuildError {
    #[error("runtime executable inventory count overflow")]
    ExecutableInventoryArithmeticOverflow,
    #[error("runtime executable inventory has {actual} rows, exceeding maximum {maximum}")]
    ExecutableInventoryRowsLimit { actual: u32, maximum: u32 },
    #[error(transparent)]
    ControlEffectContract(#[from] super::RuntimeControlEffectContractError),
    #[error(transparent)]
    NominalSchema(#[from] RuntimePlanNominalSchemaError),
    #[error("runtime-plan construction is poisoned by an earlier post-admission failure")]
    Poisoned,
    #[error("internal flow construction invariant failed: {context}")]
    FlowLoweringInvariant { context: &'static str },
    #[error("guard Copy obligations are attached to an expression outside a pattern guard")]
    GuardCopyRequirementOutsideGuard,
    #[error("guard Copy local {local} is not bound by the selected pattern")]
    InvalidGuardCopyBinding {
        local: crate::runtime_id::RuntimeLocalDeclarationId,
    },
    #[error(transparent)]
    TypeGraph(#[from] RuntimePlanTypeTableError),
    #[error(transparent)]
    LocalDeclarations(#[from] RuntimeLocalDeclarationTableError),
    #[error(transparent)]
    NominalRecordDomain(#[from] RuntimeNominalRecordDomainError),
    #[error(transparent)]
    VariantDomain(#[from] RuntimeVariantDomainError),
    #[error(transparent)]
    FunctionSite(#[from] RuntimeFunctionSiteError),
    #[error(transparent)]
    CallableState(#[from] super::RuntimeCallableStateError),
    #[error("a callable-state handle belongs to another runtime-plan builder")]
    ForeignCallableStateSeed,
    #[error("a callable-specialization handle belongs to another runtime-plan builder")]
    ForeignCallableSpecializationSeed,
    #[error("callable-specialization table exhausted its identity space")]
    CallableSpecializationIdentityExhausted,
    #[error("callable-state reservation {state} does not exist")]
    UnknownCallableState {
        state: crate::runtime_id::RuntimeCallableStateId,
    },
    #[error("callable state {state} has already been defined")]
    DuplicateCallableState {
        state: crate::runtime_id::RuntimeCallableStateId,
    },
    #[error("{count} callable-state reservations remain undefined")]
    IncompleteCallableStates { count: usize },
    #[error(transparent)]
    DialogueContent(#[from] super::RuntimeDialogueContentPlanTableError),
    #[error(transparent)]
    Plan(#[from] super::RuntimePlanError),
    #[error(transparent)]
    ProjectCall(#[from] super::RuntimeProjectCallPlanError),
    #[error(transparent)]
    ProjectCallSites(#[from] super::RuntimeProjectCallSiteTableError),
    #[error(transparent)]
    NeedProducerStartTarget(#[from] super::RuntimeNeedProducerStartTargetError),
    #[error("project-call {context} has an invalid typed ABI")]
    InvalidProjectCallAbi { context: &'static str },
    #[error("semantic type {semantic_identity:?} is absent from the transaction type graph")]
    UnknownSemanticType {
        semantic_identity: RuntimeSemanticTypeId,
    },
    #[error("type {owner} is not a nominal owner for a record domain")]
    InvalidNominalRecordOwner { owner: RuntimePlanTypeId },
    #[error("type {owner} is not a nominal owner for a variant domain")]
    InvalidVariantOwner { owner: RuntimePlanTypeId },
    #[error("variant domain nominal `{actual:?}` does not match owner type `{expected:?}`")]
    VariantNominalMismatch {
        owner: RuntimePlanTypeId,
        expected: RuntimeNominalTypeId,
        actual: RuntimeNominalTypeId,
    },
    #[error("variant domain layout does not match owner type {owner}")]
    VariantLayoutMismatch {
        owner: RuntimePlanTypeId,
        expected: crate::entry::TypeLayoutHash,
        actual: crate::entry::TypeLayoutHash,
    },
    #[error("nominal owner type {owner} cannot have both record and variant domains")]
    ConflictingNominalDomainKinds { owner: RuntimePlanTypeId },
    #[error("function site references local {local} outside the plan")]
    UnknownFunctionLocal { local: RuntimeLocalDeclarationId },
    #[error("function body references local {local} outside its lexical scope")]
    UnreachableFunctionLocal { local: RuntimeLocalDeclarationId },
    #[error("function site declares capture {local} that is not used by its body")]
    UnusedFunctionCapture { local: RuntimeLocalDeclarationId },
    #[error("function body references unknown function site {site}")]
    UnknownFunctionSite {
        site: crate::runtime_id::RuntimeFunctionSiteId,
    },
    #[error("a construction-only local handle belongs to another runtime-plan builder")]
    ForeignLocalSeed,
    #[error("a construction-only function-site handle belongs to another runtime-plan builder")]
    ForeignFunctionSiteSeed,
    #[error("runtime defer site has no executable capture-only Unit function body")]
    InvalidDeferFunctionSite,
    #[error("runtime function site is registered as a defer more than once")]
    DuplicateDeferFunctionSite,
    #[error("runtime defer-site identity space is exhausted")]
    DeferSiteIdentityExhausted,
    #[error(
        "runtime function site {site} body family is {actual:?}, expected reserved family {expected:?}"
    )]
    FunctionSiteBodyKindMismatch {
        site: crate::runtime_id::RuntimeFunctionSiteId,
        expected: RuntimeFunctionSiteBodyKind,
        actual: RuntimeFunctionSiteBodyKind,
    },
    #[error("runtime function site {site} effect set does not match its reservation")]
    FunctionSiteEffectSetMismatch {
        site: crate::runtime_id::RuntimeFunctionSiteId,
    },
    #[error("a construction-only dialogue content handle belongs to another runtime-plan builder")]
    ForeignDialogueContentSeed,
    #[error("a construction-only format-attempt handle belongs to another runtime-plan builder")]
    ForeignFormatAttemptSeed,
    #[error("runtime format-attempt table exhausted its identity space")]
    FormatAttemptIdentityExhausted,
    #[error("format attempt {attempt} is not registered in this runtime plan")]
    UnknownFormatAttempt {
        attempt: crate::runtime_id::RuntimeFormatAttemptId,
    },
    #[error("format attempt {attempt} does not use formatter template {template}")]
    FormatAttemptTemplateMismatch {
        attempt: crate::runtime_id::RuntimeFormatAttemptId,
        template: crate::runtime_id::RuntimeDialogueContentTemplateId,
    },
    #[error("format attempt {attempt} cannot be combined with inline operands")]
    FormatAttemptHasInlineOperands {
        attempt: crate::runtime_id::RuntimeFormatAttemptId,
    },
    #[error("format attempt {attempt} has no operand for parameter {parameter:?}")]
    MissingFormatAttemptOperand {
        attempt: crate::runtime_id::RuntimeFormatAttemptId,
        parameter: crate::value::RuntimeFmtParameterId,
    },
    #[error("format attempt {attempt} operand {parameter:?} has an invalid source type")]
    InvalidFormatAttemptOperand {
        attempt: crate::runtime_id::RuntimeFormatAttemptId,
        parameter: crate::value::RuntimeFmtParameterId,
    },
    #[error("a construction-only dialogue mark handle belongs to another runtime-plan builder")]
    ForeignDialogueMarkSeed,
    #[error(
        "a construction-only dialogue effect-site handle belongs to another runtime-plan builder"
    )]
    ForeignDialogueEffectSiteSeed,
    #[error("a construction-only line-task group handle belongs to another runtime-plan builder")]
    ForeignLineTaskGroupSeed,
    #[error("line-task graph has no node at required dense ordinal {ordinal}")]
    InvalidLineTaskNodeOrdinal { ordinal: usize },
    #[error("line-task child seed {actual} is not canonical preorder node {expected}")]
    NonCanonicalLineTaskNodeSeed { expected: u32, actual: u32 },
    #[error("line-task child seed {actual} cannot be represented as a runtime node")]
    InvalidLineTaskNodeSeedId { actual: u32 },
    #[error("line-task handle site {actual} is not canonical dense site {expected}")]
    NonCanonicalLineHandleSite { expected: u32, actual: u32 },
    #[error("line-task handle site source ordinals are not strictly increasing")]
    NonCanonicalLineHandleSourceOrder,
    #[error("line-task handle site count {actual} exceeds limit {limit}")]
    LineHandleSiteLimit { actual: usize, limit: usize },
    #[error("line-task handle site {site} does not project to an exact affine opaque handle")]
    InvalidLineHandleType {
        site: crate::runtime_id::RuntimeLineHandleSiteId,
    },
    #[error("line-task scheduled handle site {site} does not match child {child}")]
    InvalidScheduledLineTaskSite {
        site: crate::runtime_id::RuntimeLineHandleSiteId,
        child: RuntimeLineTaskNodeId,
    },
    #[error(transparent)]
    LineRuntime(#[from] crate::line_task::LineRuntimeError),
    #[error("line-task Detach requires a proved ownership-transfer target and is not admitted")]
    UnsupportedLineTaskDetach,
    #[error("line-task group is attached to a dialogue content plan more than once")]
    DuplicateDialogueLineTaskGroup,
    #[error(
        "line-task content event belongs to dialogue content {actual} rather than attached content {expected}"
    )]
    LineTaskContentEventOwnerMismatch {
        expected: RuntimeDialogueContentPlanId,
        actual: RuntimeDialogueContentPlanId,
    },
    #[error("sealed line-task group {group} is not attached to any dialogue content plan")]
    OrphanLineTaskGroup { group: RuntimeLineTaskGroupId },
    #[error("dialogue content slot {actual} is not the expected canonical slot {expected}")]
    NonCanonicalDialogueValueSlot {
        expected: crate::runtime_id::RuntimeDialogueValueSlotId,
        actual: crate::runtime_id::RuntimeDialogueValueSlotId,
    },
    #[error(
        "dialogue content slot {slot} does not have the exact DialogueContent opaque type {ty}"
    )]
    InvalidDialogueContentType {
        slot: crate::runtime_id::RuntimeDialogueValueSlotId,
        ty: RuntimePlanTypeId,
    },
    #[error(
        "dialogue content template slot {actual} is not the expected canonical slot {expected}"
    )]
    NonCanonicalDialogueTemplateSlot {
        expected: crate::runtime_id::RuntimeDialogueValueSlotId,
        actual: crate::runtime_id::RuntimeDialogueValueSlotId,
    },
    #[error("dialogue content template slot {slot} does not match its evaluated value site")]
    DialogueTemplateSlotMismatch {
        slot: crate::runtime_id::RuntimeDialogueValueSlotId,
    },
    #[error(
        "dialogue content has {actual} evaluated value sites but its template declares {expected} slots"
    )]
    DialogueValueCountMismatch { expected: usize, actual: usize },
    #[error(
        "fmt Content template {template} must contain one exact Formatted/Content slot and no effects"
    )]
    InvalidFormatContentTemplate {
        template: crate::runtime_id::RuntimeDialogueContentTemplateId,
    },
    #[error("fmt Content expression is missing its primary value operand")]
    MissingFormatPrimaryValue,
    #[error("fmt parameter {parameter:?} is selected more than once")]
    DuplicateFormatParameter {
        parameter: crate::value::RuntimeFmtParameterId,
    },
    #[error("fmt parameter {parameter:?} has invalid plan type {ty}")]
    InvalidFormatParameterType {
        parameter: crate::value::RuntimeFmtParameterId,
        ty: RuntimePlanTypeId,
    },
    #[error("fmt Content expression selects more than one failure policy")]
    ConflictingFormatFailurePolicy,
    #[error("fmt project DisplayText method has an invalid {context} contract")]
    InvalidFormatDisplayMethod { context: &'static str },
    #[error(
        "dialogue content has {actual} effect bindings but its template declares {expected} sites"
    )]
    DialogueEffectCountMismatch { expected: usize, actual: usize },
    #[error("dialogue content effect site {actual} is not the expected canonical site {expected}")]
    NonCanonicalDialogueEffectSite {
        expected: crate::runtime_id::RuntimeDialogueEffectSiteId,
        actual: crate::runtime_id::RuntimeDialogueEffectSiteId,
    },
    #[error("dialogue content effect site {site} has a mismatched callback capture ABI")]
    DialogueEffectCaptureTypeMismatch {
        site: crate::runtime_id::RuntimeDialogueEffectSiteId,
    },
    #[error(
        "dialogue content effect site {site} callback has {actual} captures, expected {expected}"
    )]
    DialogueEffectCaptureCountMismatch {
        site: crate::runtime_id::RuntimeDialogueEffectSiteId,
        expected: usize,
        actual: usize,
    },
    #[error("dialogue content effect site {site} callback does not return Unit")]
    DialogueEffectResultMismatch {
        site: crate::runtime_id::RuntimeDialogueEffectSiteId,
    },
    #[error("dialogue content expression references missing template manifest {template}")]
    MissingDialogueTemplateManifest {
        template: crate::runtime_id::RuntimeDialogueContentTemplateId,
    },
    #[error(transparent)]
    DialogueTemplateManifest(
        #[from] super::dialogue_content::RuntimeDialogueContentTemplateManifestError,
    ),
    #[error("a construction-only pure-helper handle belongs to another runtime-plan builder")]
    ForeignPureHelperSeed,
    #[error("runtime pure program {program} is bound more than once")]
    DuplicatePureProgram {
        program: arcweft_id::runtime_program::RuntimePureProgramId,
    },
    #[error("a construction-only trait-method handle belongs to another runtime-plan builder")]
    ForeignTraitMethodSeed,
    #[error("invalid iterator witness signature at {context}")]
    InvalidIteratorWitness { context: &'static str },
    #[error("AwaitMany concurrency limit must be greater than zero")]
    ZeroAwaitManyLimit,
    #[error("{context} has inconsistent producer family, payload, or policy")]
    InvalidProducerTemplate { context: &'static str },
    #[error("runtime function site {site} was defined more than once")]
    DuplicateFunctionSiteDefinition {
        site: crate::runtime_id::RuntimeFunctionSiteId,
    },
    #[error("runtime pure helper {helper:?} was defined more than once")]
    DuplicatePureHelperDefinition { helper: super::RuntimePureHelperId },
    #[error("runtime trait method {method:?} was defined more than once")]
    DuplicateTraitMethodDefinition { method: super::RuntimeTraitMethodId },
    #[error(
        "runtime-plan construction has incomplete definitions: {function_sites} function site(s), {pure_helpers} pure helper(s), {trait_methods} trait method(s)"
    )]
    IncompleteDefinitions {
        function_sites: usize,
        pure_helpers: usize,
        trait_methods: usize,
    },
    #[error("function site repeats local declaration {local}")]
    DuplicateFunctionLocal { local: RuntimeLocalDeclarationId },
    #[error("function site uses local declaration {local} as both a parameter and capture")]
    FunctionParameterCaptureOverlap { local: RuntimeLocalDeclarationId },
    #[error("function site input row {index} has a non-canonical source order")]
    InvalidFunctionInputSource { index: usize },
    #[error("function site input row {index} has an invalid unrestricted binding {local}")]
    InvalidFunctionInputOwnership {
        index: usize,
        local: RuntimeLocalDeclarationId,
    },
    #[error("function site input row {index} repeats a synthetic input local {local}")]
    DuplicateFunctionInputLocal {
        index: usize,
        local: RuntimeLocalDeclarationId,
    },
    #[error("{context} has {actual} ABI rows for {expected} input locals")]
    CallableAbiArity {
        context: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{context} input {index} plan type {ty} does not match its scalar ABI")]
    CallableInputAbi {
        context: &'static str,
        index: usize,
        ty: RuntimePlanTypeId,
    },
    #[error("{context} result plan type {ty} does not match its scalar ABI")]
    CallableOutputAbi {
        context: &'static str,
        ty: RuntimePlanTypeId,
    },
    #[error("runtime trait method has no receiver input")]
    MissingTraitMethodReceiver,
    #[error(
        "{context} references semantic type {semantic_identity:?} that is absent from the admitted graph"
    )]
    UnknownSeedType {
        context: &'static str,
        semantic_identity: RuntimeSemanticTypeId,
    },
    #[error("{context} uses a program-owned nominal type in a standalone task outcome")]
    InvalidStandaloneTaskOutcome { context: &'static str },
    #[error("{context} expected plan type {expected}, found {actual}")]
    TypeMismatch {
        context: &'static str,
        expected: RuntimePlanTypeId,
        actual: RuntimePlanTypeId,
    },
    #[error("{context} cannot use plan type {ty} with its admitted projection")]
    InvalidTypeProjection {
        context: &'static str,
        ty: RuntimePlanTypeId,
    },
    #[error("runtime-plan {context} contains a live function value")]
    FunctionValueInPlan { context: &'static str },
    #[error("runtime-plan {context} contains a non-constant opaque value")]
    NonConstantOpaqueValueInPlan { context: &'static str },
    #[error("runtime-plan {context} literal must be recursively unrestricted")]
    AffineLiteralInPlan { context: &'static str },
    #[error("runtime-plan {context} contains a detached expression carrier")]
    RawExpressionCarrier { context: &'static str },
    #[error("runtime-plan contains non-canonical flow operation {operation}")]
    NonCanonicalFlowOperation { operation: &'static str },
    #[error("runtime flow operation `{operation}` has no active lexical scope")]
    FlowScopeUnderflow { operation: &'static str },
    #[error("runtime-plan {context} value does not satisfy plan type {ty}")]
    InvalidValueType {
        context: &'static str,
        ty: RuntimePlanTypeId,
    },
    #[error("runtime-plan {context} value admission failed: {source}")]
    ValueAdmission {
        context: &'static str,
        source: Box<super::RuntimePlanValueAdmissionError>,
    },
    #[error("nominal record type {owner} has no admitted field domain")]
    UnknownNominalRecordDomain { owner: RuntimePlanTypeId },
    #[error("record type {owner} has no field at zero-based ordinal {ordinal}")]
    UnknownRecordField {
        owner: RuntimePlanTypeId,
        ordinal: u32,
    },
    #[error("record type {owner} repeats field {field}")]
    DuplicateRecordField {
        owner: RuntimePlanTypeId,
        field: crate::value::RuntimeRecordFieldId,
    },
    #[error("record type {owner} is missing field {field}")]
    MissingRecordField {
        owner: RuntimePlanTypeId,
        field: crate::value::RuntimeRecordFieldId,
    },
    #[error(transparent)]
    VariantCase(#[from] super::RuntimePlanVariantCaseError),
    #[error("variant type {owner} case {ordinal} expected payload {expected:?}, found {actual:?}")]
    VariantPayloadMismatch {
        owner: RuntimePlanTypeId,
        ordinal: u32,
        expected: Option<RuntimePlanTypeId>,
        actual: Option<RuntimePlanTypeId>,
    },
    #[error("pattern binds local declaration {local} more than once")]
    DuplicatePatternBinding { local: RuntimeLocalDeclarationId },
    #[error(transparent)]
    PatternBindingPath(#[from] RuntimePatternBindingPathError),
    #[error(transparent)]
    RecordFieldIdentity(#[from] RuntimeRecordFieldIdError),
    #[error("Agent constructor {constructor:?} has an invalid typed expression shape")]
    InvalidAgentExpression {
        constructor: RuntimeAgentConstructor,
    },
    #[error("Agent constructor {constructor:?} operand {operand} has invalid plan type {actual}")]
    InvalidAgentOperandType {
        constructor: RuntimeAgentConstructor,
        operand: usize,
        actual: RuntimePlanTypeId,
    },
    #[error("Agent constructor {constructor:?} result has invalid plan type {actual}")]
    InvalidAgentResultType {
        constructor: RuntimeAgentConstructor,
        actual: RuntimePlanTypeId,
    },
    #[error("spread argument has non-expandable plan type {ty}")]
    IndeterminateSpreadArgument { ty: RuntimePlanTypeId },
    #[error("call argument {index} has a duplicate or overlapping ABI position {position}")]
    InvalidCallArgumentPosition { index: usize, position: u32 },
    #[error("call argument ABI positions are not contiguous at position {position}")]
    NonContiguousCallArgumentPosition { position: u32 },
    #[error("runtime range expression must retain at least one typed bound")]
    EmptyRangeExpression,
    #[error("runtime sequence expected {expected} item(s), found {actual}")]
    SequenceLengthMismatch { expected: u64, actual: usize },
    #[error(
        "Reduction.unchanged root type {ty} is not an admitted single-argument opaque reduction"
    )]
    InvalidReductionUnchanged { ty: RuntimePlanTypeId },
    #[error("runtime plan table {table:?} exceeds its u32 row limit")]
    TooManyRows { table: RuntimePlanTable },
    #[error("runtime flow `{flow}` contains duplicate parameter local {local}")]
    DuplicateFlowParameter {
        flow: String,
        local: RuntimeLocalDeclarationId,
    },
    #[error("runtime flow `{flow}` executable repeats parameter name `{name}`")]
    DuplicateFlowParameterName { flow: String, name: String },
    #[error("runtime flow `{flow}` executable parameter {index} has an empty name")]
    EmptyFlowParameterName { flow: String, index: usize },
    #[error("runtime flow `{flow}` references unknown parameter local {local}")]
    UnknownFlowParameter {
        flow: String,
        local: RuntimeLocalDeclarationId,
    },
    #[error("runtime flow `{flow}` is defined more than once")]
    DuplicateFlowDefinition { flow: String },
    #[error("runtime flow `{flow}` has more than one invocation schema row")]
    DuplicateFlowSchema { flow: String },
    #[error("runtime flow `{flow}` has no invocation schema row")]
    MissingFlowSchema { flow: String },
    #[error("runtime flow schema `{flow}` has no matching plan flow")]
    MissingFlowDefinition { flow: String },
    #[error("runtime flow `{flow}` has {actual} local parameters, expected {expected}")]
    FlowParameterCount {
        flow: String,
        expected: usize,
        actual: usize,
    },
    #[error(
        "runtime flow `{flow}` executable parameter {index} has non-canonical position {actual}"
    )]
    FlowParameterPosition {
        flow: String,
        index: usize,
        actual: u32,
    },
    #[error(
        "runtime flow `{flow}` parameter {index} local {local} has {actual}, expected {expected}"
    )]
    FlowParameterType {
        flow: String,
        index: usize,
        local: RuntimeLocalDeclarationId,
        expected: String,
        actual: String,
    },
    #[error("runtime stream `{stream}` is defined more than once")]
    DuplicateStreamDefinition { stream: String },
    #[error("Flow {flow} requires its own executable Flow function declaration")]
    InvalidFlowFunction { flow: String },
    #[error("Flow {flow} parameter schema differs from its function input contract")]
    FlowParameterContractMismatch { flow: String },
}

#[derive(Debug)]
struct ReservedFunctionSite {
    definition: super::RuntimeFunctionDefinitionIdentity,
    role: super::RuntimeFunctionSemanticRole,
    function_type: Option<RuntimePlanTypeId>,
    inputs: Box<[RuntimeFunctionInputBinding]>,
    result: RuntimePlanTypeId,
    body_kind: RuntimeFunctionSiteBodyKind,
    effects: RuntimeEffectSet,
    body: Option<RuntimeFunctionSiteBody>,
}

#[derive(Debug)]
struct ReservedFlowRoot {
    id: FlowRuntimeId,
    function_site: crate::runtime_id::RuntimeFunctionSiteId,
    params: Box<[RuntimeLocalDeclarationId]>,
}

#[derive(Debug)]
struct ReservedPureHelper {
    definition: super::RuntimeFunctionDefinitionIdentity,
    name: String,
    inputs: Box<[super::RuntimeCallableParameter]>,
    output_abi: super::RuntimePureOutputType,
    scalar_eval_supported: bool,
    origin: super::RuntimePureHelperOrigin,
    body: Option<crate::value::RuntimeExpr>,
}

#[derive(Debug)]
struct ReservedTraitMethod {
    definition: super::RuntimeFunctionDefinitionIdentity,
    identity: super::RuntimeTraitMethodIdentity,
    receiver: super::RuntimeReceiverMode,
    inputs: Box<[super::RuntimeCallableParameter]>,
    output_abi: super::RuntimePureOutputType,
    body: Option<crate::value::RuntimeExpr>,
}

/// Private atomic owner used while recursively lowering Flow operations. A
/// fully validated site row is appended once, and the resulting ID cannot be
/// injected by a caller or by another builder.
#[derive(Debug, Default)]
struct RuntimeProjectCallSiteTableBuilder {
    rows: Vec<super::RuntimeProjectCallSite>,
}

impl RuntimeProjectCallSiteTableBuilder {
    fn push(
        &mut self,
        row: super::RuntimeProjectCallSite,
    ) -> Result<crate::runtime_id::RuntimeProjectCallSiteId, super::RuntimeProjectCallSiteTableError>
    {
        let ordinal = self
            .rows
            .len()
            .try_into()
            .ok()
            .and_then(|length: u32| length.checked_add(1))
            .and_then(NonZeroU32::new)
            .ok_or(super::RuntimeProjectCallSiteTableError::IdentityExhausted)?;
        let site = crate::runtime_id::RuntimeProjectCallSiteId::from_accepted_ordinal(ordinal);
        self.rows.push(row);
        Ok(site)
    }

    fn get(
        &self,
        site: crate::runtime_id::RuntimeProjectCallSiteId,
    ) -> Option<&super::RuntimeProjectCallSite> {
        self.rows.get(site.index())
    }

    fn finish(self) -> super::RuntimeProjectCallSiteTable {
        super::RuntimeProjectCallSiteTable::from_admitted_rows(self.rows.into_boxed_slice())
    }
}

/// Sole mutable aggregate owner. Its internal issuers are never published or
/// cloned; successful `finish` consumes them into one immutable plan.
#[derive(Debug)]
pub struct RuntimePlanBuilder {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    poisoned: bool,
    types: RuntimePlanTypeTableBuilder,
    locals: RuntimeLocalDeclarationTableBuilder,
    nominal_record_domains: RuntimeNominalRecordDomainTableBuilder,
    variant_domains: RuntimeVariantDomainTableBuilder,
    function_sites: Vec<ReservedFunctionSite>,
    control_effect_contracts: Vec<Option<super::RuntimeControlEffectContract>>,
    defer_sites: Vec<crate::runtime_id::RuntimeFunctionSiteId>,
    callable_states: RefCell<callable_states::RuntimeCallableStateBuilder>,
    callable_specializations: Vec<
        super::RuntimeCallableSpecializationDefinition<
            RuntimePlanTypeId,
            crate::runtime_id::RuntimeCallableStateId,
        >,
    >,
    project_call_sites: RefCell<RuntimeProjectCallSiteTableBuilder>,
    dialogue_content: RuntimeDialogueContentPlanTableBuilder,
    format_attempts: Vec<super::RuntimeFormatAttempt>,
    entries: Vec<RuntimeEntrySpec>,
    callable_executables: Vec<RuntimeCallableExecutable>,
    flow_schemas: Vec<RuntimeFlowSchema>,
    flow_executables: Vec<RuntimeFlowExecutable>,
    flows: Vec<ReservedFlowRoot>,
    pure_helpers: Vec<ReservedPureHelper>,
    pure_programs: Vec<RuntimePureProgramBinding>,
    trait_methods: Vec<ReservedTraitMethod>,
    line_task_groups: Vec<LineTaskGroup>,
    line_task_group_event_owners: Vec<BTreeSet<RuntimeDialogueContentPlanId>>,
    line_task_group_attachments: Vec<bool>,
    stream_plans: Vec<StreamPlan>,
}

impl RuntimePlanBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            issuer: Arc::new(RuntimePlanConstructionIssuer),
            poisoned: false,
            types: RuntimePlanTypeTableBuilder::new(),
            locals: RuntimeLocalDeclarationTableBuilder::new(),
            nominal_record_domains: RuntimeNominalRecordDomainTableBuilder::new(),
            variant_domains: RuntimeVariantDomainTableBuilder::new(),
            function_sites: Vec::new(),
            control_effect_contracts: Vec::new(),
            defer_sites: Vec::new(),
            callable_states: RefCell::new(callable_states::RuntimeCallableStateBuilder::default()),
            callable_specializations: Vec::new(),
            project_call_sites: RefCell::new(RuntimeProjectCallSiteTableBuilder::default()),
            dialogue_content: RuntimeDialogueContentPlanTableBuilder::new(),
            format_attempts: Vec::new(),
            entries: Vec::new(),
            callable_executables: Vec::new(),
            flow_schemas: Vec::new(),
            flow_executables: Vec::new(),
            flows: Vec::new(),
            pure_helpers: Vec::new(),
            pure_programs: Vec::new(),
            trait_methods: Vec::new(),
            line_task_groups: Vec::new(),
            line_task_group_event_owners: Vec::new(),
            line_task_group_attachments: Vec::new(),
            stream_plans: Vec::new(),
        }
    }

    /// Atomically rewrites semantic type, local, record-domain, and
    /// variant-domain seeds into final plan-local tables, correlated with the
    /// complete source nominal schema graph for this batch.
    ///
    /// Every subtable is prepared before any issuer commits. Consequently a
    /// failure in the last domain leaves type and local row counts unchanged.
    pub fn admit_semantic_batch(
        &mut self,
        types: impl IntoIterator<Item = RuntimePlanTypeSeed>,
        locals: impl IntoIterator<Item = RuntimeLocalDeclarationSeed>,
        nominal_record_domains: impl IntoIterator<Item = RuntimeNominalRecordDomainSeed>,
        variant_domains: impl IntoIterator<Item = RuntimeVariantDomainSeed>,
        nominal_schema: &crate::entry::RuntimeNominalSchemaGraph,
    ) -> Result<RuntimePlanSemanticAdmission, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let mut prepared_types = self.types.prepare_batch(types)?;
        let locals = locals.into_iter().collect::<Box<[_]>>();
        let declared_local_types = locals
            .iter()
            .map(|local| {
                let ty = resolve_semantic_type(&prepared_types, local.ty())?;
                let context = local.context().map(|context| resolve_semantic_type(&prepared_types, context)).transpose()?;
                let valid = prepared_types.get(ty).is_some_and(|row| match context {
                    None => row.scope().is_root(),
                    Some(context) => prepared_types.get(context).is_some_and(|owner| {
                        owner.scope().is_root() && matches!(owner.projection(), super::RuntimePlanTypeProjection::Function { contract, .. }
                            if owner.scope().enter(contract.binder()).is_ok_and(|scope| &scope == row.scope()))
                    }),
                });
                if !valid {
                    return Err(RuntimePlanBuildError::InvalidTypeProjection {
                        context: if context.is_some() {
                            "function-owned local declaration type"
                        } else {
                            "scoped local declaration type"
                        },
                        ty,
                    });
                }
                Ok((local.origin(), ty, context))
            })
            .collect::<Result<Box<[_]>, _>>()?;
        let prepared_locals = self
            .locals
            .prepare_batch(declared_local_types.iter().copied())?;

        let record_domains = nominal_record_domains
            .into_iter()
            .map(|seed| {
                let codec = nominal_schema
                    .definition(seed.owner())
                    .map(|definition| definition.codec_uses(nominal_schema.limits()))
                    .transpose()
                    .map_err(|source| RuntimePlanNominalSchemaError::CodecUse {
                        source: Box::new(source),
                    })?
                    .flatten();
                rewrite_record_domain(&prepared_types, &seed)
                    .map(|domain| domain.with_data_codec(codec))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let variant_domains = variant_domains
            .into_iter()
            .map(|seed| {
                let codec = nominal_schema
                    .definition(seed.owner())
                    .map(|definition| definition.codec_uses(nominal_schema.limits()))
                    .transpose()
                    .map_err(|source| RuntimePlanNominalSchemaError::CodecUse {
                        source: Box::new(source),
                    })?
                    .flatten();
                rewrite_variant_domain(&prepared_types, &seed)
                    .map(|domain| domain.with_data_codec(codec))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.validate_domain_exclusivity(&record_domains, &variant_domains)?;
        let domain_owners = record_domains
            .iter()
            .map(RuntimeNominalRecordDomain::owner)
            .chain(variant_domains.iter().map(RuntimeVariantDomain::owner))
            .collect::<Vec<_>>();
        let prepared_records = self.nominal_record_domains.prepare_batch(record_domains)?;
        let prepared_variants = self.variant_domains.prepare_batch(variant_domains)?;

        nominal_schema::PreparedNominalSchema {
            existing_types: &self.types,
            types: &prepared_types,
            records: &prepared_records,
            variants: &prepared_variants,
        }
        .validate(nominal_schema, domain_owners)?;

        prepared_types.retain_nominal_declarations(nominal_schema);

        let type_ids = self.types.commit_batch(prepared_types);
        let admitted_local_ids = self.locals.commit_batch(prepared_locals);
        self.nominal_record_domains.commit_batch(prepared_records);
        self.variant_domains.commit_batch(prepared_variants);
        let local_ids = admitted_local_ids
            .into_vec()
            .into_iter()
            .zip(declared_local_types)
            .map(|(local, (_, ty, _))| RuntimeLocalSeedId::issued(&self.issuer, local, ty))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(RuntimePlanSemanticAdmission {
            type_ids,
            local_ids,
        })
    }

    /// Admits ordinary type/local rows without introducing nominal definitions.
    /// References to already admitted nominal types remain valid; a new nominal
    /// requires the schema-bearing aggregate transaction instead.
    pub fn admit_type_batch(
        &mut self,
        types: impl IntoIterator<Item = RuntimePlanTypeSeed>,
        locals: impl IntoIterator<Item = RuntimeLocalDeclarationSeed>,
    ) -> Result<RuntimePlanSemanticAdmission, RuntimePlanBuildError> {
        let empty = crate::entry::RuntimeNominalSchemaGraph::try_new(
            Vec::new(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        )
        .map_err(|source| RuntimePlanNominalSchemaError::Graph {
            source: Box::new(source),
        })?;
        self.admit_semantic_batch(types, locals, [], [], &empty)
    }

    pub fn push_function_site_seed(
        &mut self,
        definition: super::RuntimeFunctionDefinitionIdentity,
        role: super::RuntimeFunctionSemanticRole,
        inputs: impl IntoIterator<Item = RuntimeFunctionInputBindingSeed>,
        body: RuntimeExprSeed,
    ) -> Result<RuntimeFunctionSiteSeedId, RuntimePlanBuildError> {
        let result = body.ty();
        let site = self.reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition,
            role,
            function_type: None,
            inputs: inputs.into_iter().collect(),
            result,
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeEffectSet::empty(),
        })?;
        self.define_function_site_seed(&site, body)?;
        Ok(site)
    }

    pub fn reserve_function_site_seed(
        &mut self,
        seed: RuntimeFunctionSiteDeclarationSeed,
    ) -> Result<RuntimeFunctionSiteSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_reserve_function_site_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_reserve_function_site_seed(
        &mut self,
        seed: RuntimeFunctionSiteDeclarationSeed,
    ) -> Result<RuntimeFunctionSiteSeedId, RuntimePlanBuildError> {
        let function_type = seed
            .function_type
            .map(|ty| self.resolve_seed_type("function frame contract", ty))
            .transpose()?;
        let body_context = self.function_body_context(function_type)?;
        let mut inputs = Vec::with_capacity(seed.inputs.len());
        let mut input_sources = Vec::with_capacity(seed.inputs.len());
        let mut input_types = Vec::with_capacity(seed.inputs.len());
        for input in seed.inputs {
            let (input_local, input_type) = input
                .input_local
                .resolve(&self.issuer)
                .ok_or(RuntimePlanBuildError::ForeignLocalSeed)?;
            let pattern = body_context.lower_pattern_seed(input.pattern)?;
            require_same("function input pattern", input_type, pattern.ty())?;
            let mut unrestricted_bindings = input
                .unrestricted_bindings
                .into_vec()
                .into_iter()
                .map(|local| {
                    local
                        .resolve(&self.issuer)
                        .map(|(id, _)| id)
                        .ok_or(RuntimePlanBuildError::ForeignLocalSeed)
                })
                .collect::<Result<Vec<_>, _>>()?;
            unrestricted_bindings.sort_unstable();
            input_types.push(input_type);
            input_sources.push(input.source);
            inputs.push(RuntimeFunctionInputBinding::new(
                input.transfer,
                input.origin,
                input.source,
                input_local,
                pattern,
                input.ownership,
                unrestricted_bindings.into_boxed_slice(),
            ));
        }
        validate_function_input_bindings(&inputs)?;
        let input_types = input_types.into_boxed_slice();
        let result = body_context.resolve_seed_type("function result", seed.result)?;
        if let Some(function_type) = function_type {
            let RuntimePlanTypeProjection::Function {
                contract,
                parameters,
                result: expected_result,
            } = body_context.projection(function_type)?
            else {
                return Err(RuntimePlanBuildError::InvalidTypeProjection {
                    context: "function frame contract",
                    ty: function_type,
                });
            };
            if parameters.as_ref() != input_types.as_ref()
                || *expected_result != result
                || contract.invocation()
                    != &crate::effect_row::EffectFormula::literal(
                        seed.effects.iter().cloned().collect(),
                        None,
                    )
            {
                return Err(RuntimePlanBuildError::InvalidTypeProjection {
                    context: "function frame ABI",
                    ty: function_type,
                });
            }
        }
        let body_kind = seed.body_kind;
        let effects = seed.effects;
        let ordinal = self
            .function_sites
            .len()
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .ok_or(RuntimeFunctionSiteError::IdentityExhausted)?;
        let site = crate::runtime_id::RuntimeFunctionSiteId::from_accepted_ordinal(ordinal);
        self.function_sites.push(ReservedFunctionSite {
            definition: seed.definition,
            role: seed.role,
            function_type,
            inputs: inputs.into_boxed_slice(),
            result,
            body_kind,
            effects: effects.clone(),
            body: None,
        });
        Ok(RuntimeFunctionSiteSeedId::issued(
            &self.issuer,
            site,
            input_sources.into_boxed_slice(),
            input_types,
            result,
            body_kind,
            effects,
        ))
    }

    /// Reserves one source defer site against a checked executable function.
    /// The function body may be defined later, but its ABI is fixed here.
    pub fn reserve_defer_site_seed(
        &mut self,
        function: &RuntimeFunctionSiteSeedId,
    ) -> Result<RuntimeDeferSiteId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_reserve_defer_site_seed(function);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_reserve_defer_site_seed(
        &mut self,
        function: &RuntimeFunctionSiteSeedId,
    ) -> Result<RuntimeDeferSiteId, RuntimePlanBuildError> {
        let (function_id, sources, _, result, body_kind, _) = function
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
        let valid_result = self.types.get(result).is_some_and(|declaration| {
            matches!(
                declaration.projection(),
                super::RuntimePlanTypeProjection::Unit
            )
        });
        if body_kind != RuntimeFunctionSiteBodyKind::Executable
            || !valid_result
            || sources
                .iter()
                .any(|source| matches!(source, RuntimeFunctionInputSource::Parameter { .. }))
        {
            return Err(RuntimePlanBuildError::InvalidDeferFunctionSite);
        }
        if self.defer_sites.contains(&function_id) {
            return Err(RuntimePlanBuildError::DuplicateDeferFunctionSite);
        }
        let site = RuntimeDeferSiteId::from_zero_based(self.defer_sites.len())
            .ok_or(RuntimePlanBuildError::DeferSiteIdentityExhausted)?;
        self.defer_sites.push(function_id);
        Ok(site)
    }

    pub fn define_function_site_seed(
        &mut self,
        site: &RuntimeFunctionSiteSeedId,
        body: impl Into<RuntimeFunctionSiteBodySeed>,
    ) -> Result<(), RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_define_function_site_seed(site, body.into());
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_define_function_site_seed(
        &mut self,
        site: &RuntimeFunctionSiteSeedId,
        body: RuntimeFunctionSiteBodySeed,
    ) -> Result<(), RuntimePlanBuildError> {
        let (site_id, _, _, result, body_kind, effects) = site
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
        let index = usize::try_from(site_id.get().get() - 1)
            .map_err(|_| RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
        let reserved = self
            .function_sites
            .get(index)
            .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
        if reserved.body.is_some() {
            return Err(RuntimePlanBuildError::DuplicateFunctionSiteDefinition { site: site_id });
        }
        if reserved.body_kind != body_kind {
            return Err(RuntimePlanBuildError::FunctionSiteBodyKindMismatch {
                site: site_id,
                expected: reserved.body_kind,
                actual: body_kind,
            });
        }
        if reserved.effects != *effects {
            return Err(RuntimePlanBuildError::FunctionSiteEffectSetMismatch { site: site_id });
        }
        let inputs = reserved.inputs.clone();
        let body_context = self.function_body_context(reserved.function_type)?;
        let lowered = match body {
            RuntimeFunctionSiteBodySeed::Expression(body) => {
                let body = body_context.lower_expression(body)?;
                require_reserved_result("function result", result, body.ty())?;
                body_context.validate_function_body_locals(&body, &inputs)?;
                RuntimeFunctionSiteBody::Expression(body)
            }
            RuntimeFunctionSiteBodySeed::Executable(body) => {
                let body_effects = body.effects;
                if !effects.covers(&body_effects) {
                    return Err(RuntimePlanBuildError::FunctionSiteEffectSetMismatch {
                        site: site_id,
                    });
                }
                let ops = body_context.lower_flow_ops(body.ops.into_vec())?;
                let mut scope = function_input_scope(&inputs);
                body_context.validate_flow_operation_locals_with_usage(&ops, &mut scope)?;
                RuntimeFunctionSiteBody::Executable(RuntimeExecutableBody::new(
                    body_effects,
                    ops.into_boxed_slice(),
                ))
            }
        };
        self.function_sites[index].body = Some(lowered);
        Ok(())
    }

    pub fn push_dialogue_content_seed(
        &mut self,
        seed: RuntimeDialogueContentPlanSeed,
    ) -> Result<RuntimeDialogueContentPlanSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_push_dialogue_content_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    /// Interns one text-model-owned immutable template manifest before any
    /// expression site that constructs a `DialogueContent` value is lowered.
    pub fn register_dialogue_content_template_seed(
        &mut self,
        seed: RuntimeDialogueContentTemplateManifestSeed,
    ) -> Result<crate::runtime_id::RuntimeDialogueContentTemplateId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_register_dialogue_content_template_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    /// Registers the exact text-model manifest reserved for plain-text
    /// context messages and retains its validated typed identity on the plan.
    pub fn register_plain_text_context_template_seed(
        &mut self,
        seed: RuntimeDialogueContentTemplateManifestSeed,
    ) -> Result<RuntimeDialoguePlainTextContextTemplateRef, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_register_plain_text_context_template_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_register_dialogue_content_template_seed(
        &mut self,
        seed: RuntimeDialogueContentTemplateManifestSeed,
    ) -> Result<crate::runtime_id::RuntimeDialogueContentTemplateId, RuntimePlanBuildError> {
        let manifest = self.lower_dialogue_content_template_manifest_seed(seed)?;
        self.dialogue_content
            .intern_template(manifest)
            .map_err(RuntimePlanBuildError::from)
    }

    /// Reserves one plan-local formatter attempt and its source-order typed
    /// operand manifest before any flow operand operations are lowered.
    pub fn reserve_format_attempt_seed(
        &mut self,
        seed: RuntimeFormatAttemptDeclarationSeed,
    ) -> Result<RuntimeFormatAttemptSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_reserve_format_attempt_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_reserve_format_attempt_seed(
        &mut self,
        seed: RuntimeFormatAttemptDeclarationSeed,
    ) -> Result<RuntimeFormatAttemptSeedId, RuntimePlanBuildError> {
        let template = self.dialogue_content.template(seed.template).ok_or(
            RuntimePlanBuildError::MissingDialogueTemplateManifest {
                template: seed.template,
            },
        )?;
        let exact_formatted_slot = matches!(
            template.slots(),
            [slot]
                if slot.role() == super::RuntimeDialogueValueRole::Formatted
                    && slot.semantic_type()
                        == crate::value::RuntimeDialogueOpaqueRole::Content.semantic_identity()
        );
        if !exact_formatted_slot || !template.effects().is_empty() {
            return Err(RuntimePlanBuildError::InvalidFormatContentTemplate {
                template: seed.template,
            });
        }

        let ordinal = self
            .format_attempts
            .len()
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .ok_or(RuntimePlanBuildError::FormatAttemptIdentityExhausted)?;
        let id =
            crate::runtime_id::RuntimeFormatAttemptId::from_zero_based(self.format_attempts.len())
                .ok_or(RuntimePlanBuildError::FormatAttemptIdentityExhausted)?;
        debug_assert_eq!(id.get(), ordinal);

        let mut seen = BTreeSet::new();
        let mut primary_present = false;
        let mut failure_policy_count = 0;
        let mut operands = Vec::with_capacity(seed.operands.len());
        for operand in seed.operands.into_vec() {
            if !seen.insert(operand.parameter) {
                return Err(RuntimePlanBuildError::DuplicateFormatParameter {
                    parameter: operand.parameter,
                });
            }
            if operand.parameter == crate::value::RuntimeFmtParameterId::Value {
                primary_present = true;
            }
            if matches!(
                operand.parameter,
                crate::value::RuntimeFmtParameterId::OnError
                    | crate::value::RuntimeFmtParameterId::Fallback
                    | crate::value::RuntimeFmtParameterId::DiscardError
            ) {
                failure_policy_count += 1;
                if failure_policy_count > 1 {
                    return Err(RuntimePlanBuildError::ConflictingFormatFailurePolicy);
                }
            }
            let ty = self.resolve_seed_type("format-attempt operand", operand.ty)?;
            let valid_type = match operand.parameter {
                crate::value::RuntimeFmtParameterId::Style
                | crate::value::RuntimeFmtParameterId::Locale
                | crate::value::RuntimeFmtParameterId::Currency
                | crate::value::RuntimeFmtParameterId::NoneValue
                | crate::value::RuntimeFmtParameterId::Fallback => self.is_string(ty)?,
                crate::value::RuntimeFmtParameterId::Color => {
                    matches!(self.projection(ty)?, RuntimePlanTypeProjection::Color)
                }
                crate::value::RuntimeFmtParameterId::DiscardError => {
                    matches!(self.projection(ty)?, RuntimePlanTypeProjection::Bool)
                }
                crate::value::RuntimeFmtParameterId::Value
                | crate::value::RuntimeFmtParameterId::OnError => true,
            };
            if !valid_type {
                return Err(RuntimePlanBuildError::InvalidFormatAttemptOperand {
                    attempt: id,
                    parameter: operand.parameter,
                });
            }
            operands.push(super::RuntimeFormatAttemptOperand::new(
                operand.parameter,
                ty,
            ));
        }
        if !primary_present {
            return Err(RuntimePlanBuildError::MissingFormatPrimaryValue);
        }
        self.format_attempts.push(super::RuntimeFormatAttempt::new(
            id,
            seed.template,
            operands.into_boxed_slice(),
        ));
        Ok(RuntimeFormatAttemptSeedId::issued(&self.issuer, id))
    }

    fn try_register_plain_text_context_template_seed(
        &mut self,
        seed: RuntimeDialogueContentTemplateManifestSeed,
    ) -> Result<RuntimeDialoguePlainTextContextTemplateRef, RuntimePlanBuildError> {
        let manifest = self.lower_dialogue_content_template_manifest_seed(seed)?;
        self.dialogue_content
            .intern_plain_text_context_template(manifest)
            .map_err(RuntimePlanBuildError::from)
    }

    fn lower_dialogue_content_template_manifest_seed(
        &self,
        seed: RuntimeDialogueContentTemplateManifestSeed,
    ) -> Result<RuntimeDialogueContentTemplateManifest, RuntimePlanBuildError> {
        let RuntimeDialogueContentTemplateManifestSeed {
            id,
            digest,
            slots: slot_seeds,
            effects: effect_seeds,
        } = seed;
        let slots = slot_seeds
            .into_vec()
            .into_iter()
            .enumerate()
            .map(|(index, slot)| {
                let expected = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(
                    index,
                )
                .ok_or(RuntimePlanBuildError::TooManyRows {
                    table: RuntimePlanTable::DialogueContent,
                })?;
                if slot.slot != expected {
                    return Err(RuntimePlanBuildError::NonCanonicalDialogueTemplateSlot {
                        expected,
                        actual: slot.slot,
                    });
                }
                if matches!(
                    slot.role,
                    RuntimeDialogueValueRole::Content | RuntimeDialogueValueRole::Formatted
                ) && slot.semantic_type != RuntimeDialogueOpaqueRole::Content.semantic_identity()
                {
                    return Err(RuntimePlanBuildError::DialogueTemplateSlotMismatch {
                        slot: slot.slot,
                    });
                }
                Ok(RuntimeDialogueContentSlot::new(
                    slot.slot,
                    slot.role,
                    slot.semantic_type,
                ))
            })
            .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
        let effects = effect_seeds
            .into_vec()
            .into_iter()
            .enumerate()
            .map(|(index, effect)| {
                let expected = crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(
                    index,
                )
                .ok_or(RuntimePlanBuildError::TooManyRows {
                    table: RuntimePlanTable::DialogueContent,
                })?;
                if effect.site != expected {
                    return Err(RuntimePlanBuildError::NonCanonicalDialogueEffectSite {
                        expected,
                        actual: effect.site,
                    });
                }
                let capture_types = effect
                    .capture_types
                    .into_vec()
                    .into_iter()
                    .map(|semantic_identity| {
                        self.types.id_for_semantic(semantic_identity).ok_or(
                            RuntimePlanBuildError::UnknownSeedType {
                                context: "dialogue content effect capture",
                                semantic_identity,
                            },
                        )
                    })
                    .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?
                    .into_boxed_slice();
                Ok(RuntimeDialogueContentEffectSlot::new(
                    effect.site,
                    effect.trigger,
                    capture_types,
                ))
            })
            .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
        Ok(RuntimeDialogueContentTemplateManifest::new_with_effects(
            id,
            digest,
            slots.into_boxed_slice(),
            effects.into_boxed_slice(),
        ))
    }

    fn try_push_dialogue_content_seed(
        &mut self,
        seed: RuntimeDialogueContentPlanSeed,
    ) -> Result<RuntimeDialogueContentPlanSeedId, RuntimePlanBuildError> {
        let RuntimeDialogueContentPlanSeed {
            line,
            template,
            values: value_seeds,
            effect_sites: effect_site_seeds,
            marks: mark_seeds,
            effect_site_count,
        } = seed;
        let template_id = template.id;
        let manifest = self.lower_dialogue_content_template_manifest_seed(template)?;
        let slots = manifest.slots().to_vec();
        let effects = manifest.effects().to_vec();
        let effect_count = RuntimeDialogueEffectSiteCount::try_from_len(effects.len()).ok_or(
            RuntimePlanBuildError::TooManyRows {
                table: RuntimePlanTable::DialogueContent,
            },
        )?;
        if effect_site_count != effect_count {
            return Err(RuntimePlanBuildError::DialogueEffectCountMismatch {
                expected: effects.len(),
                actual: usize::try_from(effect_site_count.get()).unwrap_or(usize::MAX),
            });
        }
        if effect_site_seeds.len() != effects.len() {
            return Err(RuntimePlanBuildError::DialogueEffectCountMismatch {
                expected: effects.len(),
                actual: effect_site_seeds.len(),
            });
        }
        if value_seeds.len() != slots.len() {
            return Err(RuntimePlanBuildError::DialogueValueCountMismatch {
                expected: slots.len(),
                actual: value_seeds.len(),
            });
        }
        let mut values = Vec::with_capacity(value_seeds.len());
        for (index, value) in value_seeds.into_vec().into_iter().enumerate() {
            let expected = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(index)
                .ok_or(RuntimePlanBuildError::TooManyRows {
                    table: RuntimePlanTable::DialogueContent,
                })?;
            if value.slot != expected {
                return Err(RuntimePlanBuildError::NonCanonicalDialogueValueSlot {
                    expected,
                    actual: value.slot,
                });
            }
            let (function, input_sources, input_types, result, body_kind, _) = value
                .function
                .resolve(&self.issuer)
                .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
            if body_kind != RuntimeFunctionSiteBodyKind::Expression {
                return Err(RuntimePlanBuildError::FunctionSiteBodyKindMismatch {
                    site: function,
                    expected: RuntimeFunctionSiteBodyKind::Expression,
                    actual: body_kind,
                });
            }
            let parameter_count = input_sources
                .iter()
                .filter(|source| matches!(source, RuntimeFunctionInputSource::Parameter { .. }))
                .count();
            if parameter_count != 0 {
                return Err(RuntimePlanBuildError::CallableAbiArity {
                    context: "dialogue value site",
                    expected: 0,
                    actual: parameter_count,
                });
            }
            let capture_types = input_sources
                .iter()
                .zip(input_types)
                .filter_map(|(source, ty)| {
                    matches!(
                        source,
                        RuntimeFunctionInputSource::Capture { .. }
                            | RuntimeFunctionInputSource::CapturedParameter { .. }
                    )
                    .then_some(*ty)
                })
                .collect::<Vec<_>>();
            if value.captures.len() != capture_types.len() {
                return Err(RuntimePlanBuildError::CallableAbiArity {
                    context: "dialogue value capture expressions",
                    expected: capture_types.len(),
                    actual: value.captures.len(),
                });
            }
            let captures = value
                .captures
                .into_vec()
                .into_iter()
                .zip(capture_types)
                .map(|(capture, expected)| {
                    let capture = self.lower_expression(capture)?;
                    require_same("dialogue value capture expression", expected, capture.ty())?;
                    Ok(capture)
                })
                .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
            if matches!(
                value.role,
                RuntimeDialogueValueRole::Content | RuntimeDialogueValueRole::Formatted
            ) && !self.is_exact_dialogue_content_type(result)
            {
                return Err(RuntimePlanBuildError::InvalidDialogueContentType {
                    slot: value.slot,
                    ty: result,
                });
            }
            let Some(slot) = slots.get(index) else {
                return Err(RuntimePlanBuildError::DialogueTemplateSlotMismatch {
                    slot: value.slot,
                });
            };
            if slot.slot() != value.slot
                || slot.role() != value.role
                || self
                    .types
                    .get(result)
                    .is_none_or(|ty| slot.semantic_type() != ty.semantic_identity())
            {
                return Err(RuntimePlanBuildError::DialogueTemplateSlotMismatch {
                    slot: value.slot,
                });
            }
            values.push(RuntimeDialogueValueSite::new(
                value.slot,
                value.role,
                function,
                captures.into_boxed_slice(),
            ));
        }
        let marks = mark_seeds
            .into_vec()
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                let id = RuntimeDialogueMarkId::from_zero_based(index).ok_or(
                    RuntimePlanBuildError::TooManyRows {
                        table: RuntimePlanTable::DialogueContent,
                    },
                )?;
                Ok(RuntimeDialogueMark::new(id, label))
            })
            .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
        let mut effect_sites = Vec::with_capacity(effect_site_seeds.len());
        for (index, effect) in effect_site_seeds.into_vec().into_iter().enumerate() {
            let expected = crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index)
                .ok_or(RuntimePlanBuildError::TooManyRows {
                table: RuntimePlanTable::DialogueContent,
            })?;
            if effect.site != expected {
                return Err(RuntimePlanBuildError::NonCanonicalDialogueEffectSite {
                    expected,
                    actual: effect.site,
                });
            }
            let (function, input_sources, input_types, result, body_kind, _) = effect
                .function
                .resolve(&self.issuer)
                .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
            if body_kind != RuntimeFunctionSiteBodyKind::Executable {
                return Err(RuntimePlanBuildError::FunctionSiteBodyKindMismatch {
                    site: function,
                    expected: RuntimeFunctionSiteBodyKind::Executable,
                    actual: body_kind,
                });
            }
            let parameter_count = input_sources
                .iter()
                .filter(|source| matches!(source, RuntimeFunctionInputSource::Parameter { .. }))
                .count();
            if parameter_count != 0 {
                return Err(RuntimePlanBuildError::CallableAbiArity {
                    context: "dialogue effect site",
                    expected: 0,
                    actual: parameter_count,
                });
            }
            if !matches!(
                self.types
                    .get(result)
                    .map(|declaration| declaration.projection()),
                Some(super::RuntimePlanTypeProjection::Unit)
            ) {
                return Err(RuntimePlanBuildError::DialogueEffectResultMismatch {
                    site: effect.site,
                });
            }
            let Some(slot) = effects.get(index) else {
                return Err(RuntimePlanBuildError::DialogueEffectCountMismatch {
                    expected: effects.len(),
                    actual: index,
                });
            };
            let capture_types = input_sources
                .iter()
                .zip(input_types)
                .filter_map(|(source, ty)| {
                    matches!(
                        source,
                        RuntimeFunctionInputSource::Capture { .. }
                            | RuntimeFunctionInputSource::CapturedParameter { .. }
                    )
                    .then_some(*ty)
                })
                .collect::<Vec<_>>();
            if capture_types != slot.capture_types() {
                return Err(RuntimePlanBuildError::DialogueEffectCaptureTypeMismatch {
                    site: effect.site,
                });
            }
            if effect.captures.len() != capture_types.len() {
                return Err(RuntimePlanBuildError::DialogueEffectCaptureCountMismatch {
                    site: effect.site,
                    expected: capture_types.len(),
                    actual: effect.captures.len(),
                });
            }
            let captures = effect
                .captures
                .into_vec()
                .into_iter()
                .zip(capture_types)
                .map(|(capture, expected)| {
                    let capture = self.lower_expression(capture)?;
                    require_same("dialogue effect capture expression", expected, capture.ty())?;
                    Ok(capture)
                })
                .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
            effect_sites.push(RuntimeDialogueEffectSite::new(
                effect.site,
                self.intern_function_callable(
                    self.resolve_seed_type("content effect type", effect.callable_type)?,
                    function,
                    input_sources,
                    input_types,
                    result,
                )?,
                captures.into_boxed_slice(),
            ));
        }
        let key = super::RuntimeDialogueContentApplicationKey::new(line.clone(), template_id);
        self.dialogue_content.ensure_pushable(&key)?;
        self.dialogue_content.intern_template(manifest)?;
        let content = self.dialogue_content.push(RuntimeDialogueContentPlan::new(
            line,
            template_id,
            values.into_boxed_slice(),
            effect_sites.into_boxed_slice(),
            marks.into_boxed_slice(),
            effect_site_count,
        ))?;
        Ok(RuntimeDialogueContentPlanSeedId::issued(
            &self.issuer,
            content,
        ))
    }

    fn is_exact_dialogue_content_type(&self, ty: RuntimePlanTypeId) -> bool {
        let Some(declaration) = self.types.get(ty) else {
            return false;
        };
        let super::RuntimePlanTypeProjection::Opaque {
            producer,
            admission: crate::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: crate::value::RuntimeOpaqueValueClass::Plain,
            persistence: crate::value::RuntimeOpaquePersistence::SnapshotOnly,
            arguments,
        } = declaration.projection()
        else {
            return false;
        };
        let owner = RuntimeDialogueOpaqueRole::Content.exact_owner();
        arguments.is_empty()
            && producer == owner.producer()
            && declaration.semantic_identity() == owner.semantic_identity()
    }

    /// Lowers a recursive, construction-only line-task seed into the one
    /// dense preorder graph admitted by this plan builder.
    pub fn push_line_task_group_seed(
        &mut self,
        seed: RuntimeLineTaskGroupSeed,
    ) -> Result<RuntimeLineTaskGroupSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_push_line_task_group_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_push_line_task_group_seed(
        &mut self,
        seed: RuntimeLineTaskGroupSeed,
    ) -> Result<RuntimeLineTaskGroupSeedId, RuntimePlanBuildError> {
        if line_task_seed_requests_detach(&seed) {
            return Err(RuntimePlanBuildError::UnsupportedLineTaskDetach);
        }
        let event_owners = self.line_task_event_owners(&seed)?;
        let captures = seed
            .free_locals()
            .into_vec()
            .into_iter()
            .map(|capture| {
                capture
                    .resolve(&self.issuer)
                    .map(|(local, _)| local)
                    .ok_or(RuntimePlanBuildError::ForeignLocalSeed)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let capture_scope = captures.iter().copied().collect::<BTreeSet<_>>();
        let activation_ops = self.lower_flow_ops(seed.activation_ops)?;
        let mut activation_scope = capture_scope.clone();
        self.validate_flow_operation_locals(&activation_ops, &mut activation_scope)?;
        let result_type = self.types.id_for_semantic(seed.result_type).ok_or(
            RuntimePlanBuildError::UnknownSeedType {
                context: "line-task result",
                semantic_identity: seed.result_type,
            },
        )?;
        let mut nodes = Vec::new();
        let root = self.lower_line_task_node_seed(seed.root, &mut nodes)?;
        let nodes = nodes
            .into_iter()
            .enumerate()
            .map(|(ordinal, node)| {
                node.ok_or(RuntimePlanBuildError::InvalidLineTaskNodeOrdinal { ordinal })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let handle_sites = self.lower_line_handle_sites(seed.handle_sites, &nodes)?;
        let cancel_rules = seed
            .cancel_rules
            .into_vec()
            .into_iter()
            .map(|rule| self.lower_line_task_cancel_rule_seed(rule))
            .collect::<Result<Vec<_>, _>>()?;
        let cleanup = LineTaskCleanup::new(
            self.lower_flow_ops(seed.cleanup_completed)?
                .into_boxed_slice(),
            self.lower_flow_ops(seed.cleanup_cancelled)?
                .into_boxed_slice(),
            self.lower_flow_ops(seed.cleanup_failed)?.into_boxed_slice(),
            seed.cleanup_policy,
        );
        let mut scheduled_packets = BTreeMap::<
            RuntimeLineHandleSiteId,
            (RuntimeLineTaskNodeId, BTreeSet<RuntimeLocalDeclarationId>),
        >::new();
        for op in &activation_ops {
            let FlowOp::LineOperation {
                operation:
                    RuntimeLineOperation::Schedule {
                        site,
                        child,
                        captures,
                        ..
                    },
                ..
            } = op
            else {
                continue;
            };
            let packet = captures.iter().map(|capture| capture.local()).collect();
            if scheduled_packets.insert(*site, (*child, packet)).is_some() {
                return Err(RuntimePlanBuildError::InvalidScheduledLineTaskSite {
                    site: *site,
                    child: *child,
                });
            }
        }
        let scheduled_actions = nodes
            .iter()
            .filter_map(|node| match node {
                LineTaskNode::Child {
                    trigger: LineTaskTrigger::Scheduled(site),
                    scope,
                    ..
                } => Some((*scope, *site)),
                _ => None,
            })
            .collect::<BTreeMap<RuntimeLineTaskNodeId, RuntimeLineHandleSiteId>>();
        let mut action_sets = Vec::new();
        for (ordinal, node) in nodes.iter().enumerate() {
            let LineTaskNode::Action(actions) = node else {
                continue;
            };
            let id = RuntimeLineTaskNodeId::from_zero_based(ordinal)
                .ok_or(RuntimePlanBuildError::InvalidLineTaskNodeOrdinal { ordinal })?;
            if let Some(site) = scheduled_actions.get(&id) {
                let child = handle_sites
                    .get(site.index())
                    .and_then(RuntimeLineHandleSite::scheduled_child)
                    .ok_or(RuntimePlanBuildError::InvalidLineHandleType { site: *site })?;
                let (packet_child, packet) = scheduled_packets.get(site).ok_or(
                    RuntimePlanBuildError::InvalidScheduledLineTaskSite { site: *site, child },
                )?;
                if *packet_child != child {
                    return Err(RuntimePlanBuildError::InvalidScheduledLineTaskSite {
                        site: *site,
                        child,
                    });
                }
                self.validate_line_task_actions_locals(&[actions], packet)?;
            } else {
                action_sets.push(actions.as_ref());
            }
        }
        action_sets.extend(cancel_rules.iter().map(LineCancelRule::action));
        action_sets.push(cleanup.actions(crate::line_task::ScopeExit::Completed));
        action_sets.push(cleanup.actions(crate::line_task::ScopeExit::Cancelled));
        action_sets.push(cleanup.actions(crate::line_task::ScopeExit::Failed));
        let used = self.validate_line_task_actions_locals(&action_sets, &activation_scope)?;
        let activation_exports = activation_scope
            .difference(&capture_scope)
            .filter(|local| used.contains(local))
            .copied()
            .collect::<Vec<_>>();
        let group = RuntimeLineTaskGroupId::from_zero_based(self.line_task_groups.len()).ok_or(
            RuntimePlanBuildError::TooManyRows {
                table: RuntimePlanTable::LineTaskGroups,
            },
        )?;
        self.line_task_groups.push(LineTaskGroup::new(
            seed.definition,
            captures.into_boxed_slice(),
            activation_exports.into_boxed_slice(),
            activation_ops.into_boxed_slice(),
            result_type,
            handle_sites,
            root,
            nodes.into_boxed_slice(),
            cancel_rules.into_boxed_slice(),
            cleanup,
        ));
        self.line_task_group_event_owners.push(event_owners);
        self.line_task_group_attachments.push(false);
        Ok(RuntimeLineTaskGroupSeedId::issued(&self.issuer, group))
    }

    fn lower_line_task_node_seed(
        &self,
        seed: RuntimeLineTaskNodeSeed,
        nodes: &mut Vec<Option<LineTaskNode>>,
    ) -> Result<RuntimeLineTaskNodeId, RuntimePlanBuildError> {
        let ordinal = nodes.len();
        let id = RuntimeLineTaskNodeId::from_zero_based(ordinal).ok_or(
            RuntimePlanBuildError::TooManyRows {
                table: RuntimePlanTable::LineTaskGroups,
            },
        )?;
        let expected_seed =
            u32::try_from(ordinal).map_err(|_| RuntimePlanBuildError::TooManyRows {
                table: RuntimePlanTable::LineTaskGroups,
            })?;
        nodes.push(None);
        let node = match seed {
            RuntimeLineTaskNodeSeed::Sequence(children) => LineTaskNode::Sequence(
                children
                    .into_iter()
                    .map(|child| self.lower_line_task_node_seed(child, nodes))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
            RuntimeLineTaskNodeSeed::Start(children) => LineTaskNode::Start(
                children
                    .into_iter()
                    .map(|child| self.lower_line_task_node_seed(child, nodes))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
            RuntimeLineTaskNodeSeed::Parallel { policy, children } => LineTaskNode::Parallel {
                policy,
                children: children
                    .into_iter()
                    .map(|child| self.lower_line_task_node_seed(child, nodes))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            },
            RuntimeLineTaskNodeSeed::Child {
                node,
                trigger,
                join_policy,
                cancel_policy,
                scope,
            } => {
                if node.get() != expected_seed {
                    return Err(RuntimePlanBuildError::NonCanonicalLineTaskNodeSeed {
                        expected: expected_seed,
                        actual: node.get(),
                    });
                }
                LineTaskNode::Child {
                    trigger: self.lower_line_task_trigger_seed(trigger)?,
                    join_policy,
                    cancel_policy,
                    scope: self.lower_line_task_node_seed(*scope, nodes)?,
                }
            }
            RuntimeLineTaskNodeSeed::Action(actions) => {
                LineTaskNode::Action(self.lower_flow_ops(actions)?.into_boxed_slice())
            }
        };
        nodes[ordinal] = Some(node);
        Ok(id)
    }

    fn lower_line_task_trigger_seed(
        &self,
        seed: RuntimeLineTaskTriggerSeed,
    ) -> Result<LineTaskTrigger, RuntimePlanBuildError> {
        match seed {
            RuntimeLineTaskTriggerSeed::Immediate => Ok(LineTaskTrigger::Immediate),
            RuntimeLineTaskTriggerSeed::Scheduled(site) => Ok(LineTaskTrigger::Scheduled(site)),
            RuntimeLineTaskTriggerSeed::Mark(mark) => {
                let (content, mark) = mark
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignDialogueMarkSeed)?;
                if self
                    .dialogue_content
                    .get(content)
                    .and_then(|content| content.marks().get(mark.index()))
                    .is_none()
                {
                    return Err(RuntimePlanBuildError::ForeignDialogueMarkSeed);
                }
                Ok(LineTaskTrigger::Mark(mark))
            }
        }
    }

    fn lower_line_handle_sites(
        &self,
        seeds: Box<[RuntimeLineHandleSiteSeed]>,
        nodes: &[LineTaskNode],
    ) -> Result<Box<[RuntimeLineHandleSite]>, RuntimePlanBuildError> {
        if seeds.len() > MAX_LINE_HANDLE_SITES {
            return Err(RuntimePlanBuildError::LineHandleSiteLimit {
                actual: seeds.len(),
                limit: MAX_LINE_HANDLE_SITES,
            });
        }
        let mut previous_source = None;
        let mut sites = Vec::with_capacity(seeds.len());
        for (expected, seed) in seeds.into_vec().into_iter().enumerate() {
            let expected = u32::try_from(expected).map_err(|_| {
                RuntimePlanBuildError::LineHandleSiteLimit {
                    actual: sites.len().saturating_add(1),
                    limit: MAX_LINE_HANDLE_SITES,
                }
            })?;
            if seed.id.get() != expected {
                return Err(RuntimePlanBuildError::NonCanonicalLineHandleSite {
                    expected,
                    actual: seed.id.get(),
                });
            }
            if previous_source.is_some_and(|previous| previous >= seed.source_ordinal) {
                return Err(RuntimePlanBuildError::NonCanonicalLineHandleSourceOrder);
            }
            previous_source = Some(seed.source_ordinal);
            let result_type = self.types.id_for_semantic(seed.result_type).ok_or(
                RuntimePlanBuildError::UnknownSeedType {
                    context: "line handle result",
                    semantic_identity: seed.result_type,
                },
            )?;
            let declaration = self
                .types
                .get(result_type)
                .ok_or(RuntimePlanBuildError::InvalidLineHandleType { site: seed.id })?;
            let RuntimePlanTypeProjection::Opaque {
                producer,
                admission: crate::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class,
                persistence,
                ..
            } = declaration.projection()
            else {
                return Err(RuntimePlanBuildError::InvalidLineHandleType { site: seed.id });
            };
            let owner = crate::pattern::RuntimeOpaqueTypeOwner::exact_with(
                producer.clone(),
                declaration.semantic_identity(),
                *value_class,
                *persistence,
            );
            let scheduled_child = seed
                .scheduled_child
                .map(|child| {
                    child
                        .runtime_id()
                        .ok_or(RuntimePlanBuildError::InvalidLineTaskNodeSeedId {
                            actual: child.get(),
                        })
                })
                .transpose()?;
            let site = RuntimeLineHandleSite::new(
                seed.id,
                seed.source_ordinal,
                seed.kind,
                result_type,
                seed.character,
                scheduled_child,
                owner,
            )?;
            if let Some(child) = site.scheduled_child()
                && !matches!(
                    nodes.get(child.index()),
                    Some(LineTaskNode::Child {
                        trigger: LineTaskTrigger::Scheduled(trigger),
                        join_policy: crate::line_task::ChildJoinPolicy::Join,
                        cancel_policy: crate::line_task::ChildCancelPolicy::CancelAndJoin,
                        scope,
                    }) if *trigger == site.id()
                        && matches!(nodes.get(scope.index()), Some(LineTaskNode::Action(_)))
                )
            {
                return Err(RuntimePlanBuildError::InvalidScheduledLineTaskSite {
                    site: site.id(),
                    child,
                });
            }
            sites.push(site);
        }
        for (index, node) in nodes.iter().enumerate() {
            let LineTaskNode::Child {
                trigger: LineTaskTrigger::Scheduled(site),
                ..
            } = node
            else {
                continue;
            };
            let child = RuntimeLineTaskNodeId::from_zero_based(index)
                .ok_or(RuntimePlanBuildError::InvalidLineTaskNodeOrdinal { ordinal: index })?;
            if sites
                .get(site.index())
                .and_then(RuntimeLineHandleSite::scheduled_child)
                != Some(child)
            {
                return Err(RuntimePlanBuildError::InvalidScheduledLineTaskSite {
                    site: *site,
                    child,
                });
            }
        }
        Ok(sites.into_boxed_slice())
    }

    fn lower_line_task_cancel_rule_seed(
        &self,
        seed: RuntimeLineTaskCancelRuleSeed,
    ) -> Result<LineCancelRule, RuntimePlanBuildError> {
        Ok(LineCancelRule::new(
            seed.trigger,
            self.lower_flow_ops(seed.action)?.into_boxed_slice(),
        ))
    }

    /// Establishes the only link between one dialogue content plan and its
    /// optional line-task group. A finished group cannot be shared.
    pub fn attach_line_task_group_seed(
        &mut self,
        content: &RuntimeDialogueContentPlanSeedId,
        group: &RuntimeLineTaskGroupSeedId,
    ) -> Result<(), RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_attach_line_task_group_seed(content, group);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_attach_line_task_group_seed(
        &mut self,
        content: &RuntimeDialogueContentPlanSeedId,
        group: &RuntimeLineTaskGroupSeedId,
    ) -> Result<(), RuntimePlanBuildError> {
        let content = content
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignDialogueContentSeed)?;
        let group = group
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignLineTaskGroupSeed)?;
        if self.line_task_groups.get(group.index()).is_none() {
            return Err(RuntimePlanBuildError::ForeignLineTaskGroupSeed);
        }
        if self
            .line_task_group_attachments
            .get(group.index())
            .copied()
            .unwrap_or(false)
        {
            return Err(RuntimePlanBuildError::DuplicateDialogueLineTaskGroup);
        }
        let owners = self
            .line_task_group_event_owners
            .get(group.index())
            .ok_or(RuntimePlanBuildError::ForeignLineTaskGroupSeed)?;
        if let Some(actual) = owners.iter().copied().find(|owner| *owner != content) {
            return Err(RuntimePlanBuildError::LineTaskContentEventOwnerMismatch {
                expected: content,
                actual,
            });
        }
        let content = self
            .dialogue_content
            .get_mut(content)
            .ok_or(RuntimePlanBuildError::ForeignDialogueContentSeed)?;
        if !content.attach_line_task_group(group) {
            return Err(RuntimePlanBuildError::DuplicateDialogueLineTaskGroup);
        }
        self.line_task_group_attachments[group.index()] = true;
        Ok(())
    }

    fn line_task_event_owners(
        &self,
        seed: &RuntimeLineTaskGroupSeed,
    ) -> Result<BTreeSet<RuntimeDialogueContentPlanId>, RuntimePlanBuildError> {
        let mut owners = BTreeSet::new();
        self.collect_line_task_node_event_owners(&seed.root, &mut owners)?;
        Ok(owners)
    }

    fn collect_line_task_node_event_owners(
        &self,
        node: &RuntimeLineTaskNodeSeed,
        owners: &mut BTreeSet<RuntimeDialogueContentPlanId>,
    ) -> Result<(), RuntimePlanBuildError> {
        match node {
            RuntimeLineTaskNodeSeed::Sequence(children)
            | RuntimeLineTaskNodeSeed::Start(children)
            | RuntimeLineTaskNodeSeed::Parallel { children, .. } => {
                for child in children {
                    self.collect_line_task_node_event_owners(child, owners)?;
                }
            }
            RuntimeLineTaskNodeSeed::Child { trigger, scope, .. } => {
                match trigger {
                    RuntimeLineTaskTriggerSeed::Mark(mark) => {
                        let (content, _) = mark
                            .resolve(&self.issuer)
                            .ok_or(RuntimePlanBuildError::ForeignDialogueMarkSeed)?;
                        owners.insert(content);
                    }
                    RuntimeLineTaskTriggerSeed::Immediate
                    | RuntimeLineTaskTriggerSeed::Scheduled(_) => {}
                }
                self.collect_line_task_node_event_owners(scope, owners)?;
            }
            RuntimeLineTaskNodeSeed::Action(_) => {}
        }
        Ok(())
    }

    pub fn push_pure_helper_seed(
        &mut self,
        seed: RuntimePureHelperSeed,
    ) -> Result<RuntimePureHelperSeedId, RuntimePlanBuildError> {
        let result = seed.body.ty();
        let body = seed.body;
        let helper = self.reserve_pure_helper_seed(RuntimePureHelperDeclarationSeed {
            definition: seed.definition,
            name: seed.name,
            inputs: seed.inputs,
            result,
            output_abi: seed.output_abi,
            scalar_eval_supported: seed.scalar_eval_supported,
            origin: seed.origin,
        })?;
        self.define_pure_helper_seed(&helper, body)?;
        Ok(helper)
    }

    /// Binds one stable domain-owned pure-program identity to a function frame
    /// reserved by this same aggregate construction transaction.
    pub fn push_pure_program_binding_seed(
        &mut self,
        seed: &RuntimePureProgramBindingSeed,
    ) -> Result<u32, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let Some((site, _, parameters, result, _, effects)) = seed.site.resolve(&self.issuer)
        else {
            self.poisoned = true;
            return Err(RuntimePlanBuildError::ForeignFunctionSiteSeed);
        };
        if !effects.is_empty() {
            self.poisoned = true;
            return Err(RuntimePlanBuildError::InvalidTypeProjection {
                context: "pure program requires an effect-free function frame",
                ty: result,
            });
        }
        let input_types = parameters
            .iter()
            .map(|ty| {
                self.types
                    .get(*ty)
                    .map(super::RuntimePlanTypeDeclaration::semantic_identity)
                    .ok_or(RuntimePlanBuildError::InvalidTypeProjection {
                        context: "pure program input",
                        ty: *ty,
                    })
            })
            .collect::<Result<Box<[_]>, _>>()?;
        let result_type = self
            .types
            .get(result)
            .map(super::RuntimePlanTypeDeclaration::semantic_identity)
            .ok_or(RuntimePlanBuildError::InvalidTypeProjection {
                context: "pure program result",
                ty: result,
            })?;
        if self
            .pure_programs
            .iter()
            .any(|binding| binding.program() == seed.program)
        {
            self.poisoned = true;
            return Err(RuntimePlanBuildError::DuplicatePureProgram {
                program: seed.program,
            });
        }
        let function_type = self
            .function_sites
            .get(
                usize::try_from(site.get().get() - 1)
                    .map_err(|_| RuntimePlanBuildError::ForeignFunctionSiteSeed)?,
            )
            .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?
            .function_type
            .map(|ty| {
                self.types
                    .get(ty)
                    .map(super::RuntimePlanTypeDeclaration::semantic_identity)
                    .ok_or(RuntimePlanBuildError::InvalidTypeProjection {
                        context: "pure program frame contract",
                        ty,
                    })
            })
            .transpose()?;
        push_row(
            &mut self.pure_programs,
            RuntimePureProgramBinding::new(
                seed.program,
                site,
                function_type,
                input_types,
                result_type,
            ),
            RuntimePlanTable::PurePrograms,
        )
    }

    pub fn reserve_pure_helper_seed(
        &mut self,
        seed: RuntimePureHelperDeclarationSeed,
    ) -> Result<RuntimePureHelperSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_reserve_pure_helper_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_reserve_pure_helper_seed(
        &mut self,
        seed: RuntimePureHelperDeclarationSeed,
    ) -> Result<RuntimePureHelperSeedId, RuntimePlanBuildError> {
        let inputs =
            self.resolve_function_locals(seed.inputs.iter().map(|input| input.local.clone()))?;
        let input_abi = seed
            .inputs
            .iter()
            .map(|input| input.abi)
            .collect::<Vec<_>>();
        self.validate_callable_input_abi("pure helper", &inputs, &input_abi)?;
        let input_types = inputs
            .iter()
            .map(|(_, ty)| *ty)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let inputs = inputs
            .into_iter()
            .zip(seed.inputs)
            .map(|((local, _), input)| {
                super::RuntimeCallableParameter::new(
                    input.identity,
                    local,
                    input.passing,
                    input.abi,
                )
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let result = self.resolve_seed_type("pure helper result", seed.result)?;
        self.validate_callable_output_abi("pure helper", result, seed.output_abi)?;
        let helper = super::RuntimePureHelperId(self.pure_helpers.len());
        if u32::try_from(self.pure_helpers.len()).is_err() {
            return Err(RuntimePlanBuildError::TooManyRows {
                table: RuntimePlanTable::PureHelpers,
            });
        }
        self.pure_helpers.push(ReservedPureHelper {
            definition: seed.definition,
            name: seed.name,
            inputs,
            output_abi: seed.output_abi,
            scalar_eval_supported: seed.scalar_eval_supported,
            origin: seed.origin,
            body: None,
        });
        Ok(RuntimePureHelperSeedId::issued(
            &self.issuer,
            helper,
            input_types,
            result,
        ))
    }

    pub fn define_pure_helper_seed(
        &mut self,
        helper: &RuntimePureHelperSeedId,
        body: RuntimeExprSeed,
    ) -> Result<(), RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_define_pure_helper_seed(helper, body);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_define_pure_helper_seed(
        &mut self,
        helper: &RuntimePureHelperSeedId,
        body: RuntimeExprSeed,
    ) -> Result<(), RuntimePlanBuildError> {
        let (helper_id, _, result) = helper
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignPureHelperSeed)?;
        let reserved = self
            .pure_helpers
            .get(helper_id.0)
            .ok_or(RuntimePlanBuildError::ForeignPureHelperSeed)?;
        if reserved.body.is_some() {
            return Err(RuntimePlanBuildError::DuplicatePureHelperDefinition { helper: helper_id });
        }
        let inputs = reserved
            .inputs
            .iter()
            .map(|input| input.local())
            .collect::<Vec<_>>();
        let body = self.lower_expression(body)?;
        require_reserved_result("pure helper result", result, body.ty())?;
        self.validate_callable_body_locals(&body, &inputs, &[])?;
        self.pure_helpers[helper_id.0].body = Some(body);
        Ok(())
    }

    pub fn push_trait_method_seed(
        &mut self,
        seed: RuntimeTraitMethodSeed,
    ) -> Result<RuntimeTraitMethodSeedId, RuntimePlanBuildError> {
        let result = seed.body.ty();
        let body = seed.body;
        let method = self.reserve_trait_method_seed(RuntimeTraitMethodDeclarationSeed {
            definition: seed.definition,
            identity: seed.identity,
            receiver: seed.receiver,
            inputs: seed.inputs,
            result,
            output_abi: seed.output_abi,
        })?;
        self.define_trait_method_seed(&method, body)?;
        Ok(method)
    }

    pub fn reserve_trait_method_seed(
        &mut self,
        seed: RuntimeTraitMethodDeclarationSeed,
    ) -> Result<RuntimeTraitMethodSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_reserve_trait_method_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_reserve_trait_method_seed(
        &mut self,
        seed: RuntimeTraitMethodDeclarationSeed,
    ) -> Result<RuntimeTraitMethodSeedId, RuntimePlanBuildError> {
        let inputs =
            self.resolve_function_locals(seed.inputs.iter().map(|input| input.local.clone()))?;
        if inputs.is_empty() {
            return Err(RuntimePlanBuildError::MissingTraitMethodReceiver);
        }
        let input_abi = seed
            .inputs
            .iter()
            .map(|input| input.abi)
            .collect::<Vec<_>>();
        self.validate_callable_input_abi("trait method", &inputs, &input_abi)?;
        let input_types = inputs
            .iter()
            .map(|(_, ty)| *ty)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let inputs = inputs
            .into_iter()
            .zip(seed.inputs)
            .map(|((local, _), input)| {
                super::RuntimeCallableParameter::new(
                    input.identity,
                    local,
                    input.passing,
                    input.abi,
                )
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let result = self.resolve_seed_type("trait method result", seed.result)?;
        self.validate_callable_output_abi("trait method", result, seed.output_abi)?;
        let method = super::RuntimeTraitMethodId(self.trait_methods.len());
        if u32::try_from(self.trait_methods.len()).is_err() {
            return Err(RuntimePlanBuildError::TooManyRows {
                table: RuntimePlanTable::TraitMethods,
            });
        }
        self.trait_methods.push(ReservedTraitMethod {
            definition: seed.definition,
            identity: seed.identity,
            receiver: seed.receiver,
            inputs,
            output_abi: seed.output_abi,
            body: None,
        });
        Ok(RuntimeTraitMethodSeedId::issued(
            &self.issuer,
            method,
            seed.receiver,
            input_types,
            result,
        ))
    }

    pub fn define_trait_method_seed(
        &mut self,
        method: &RuntimeTraitMethodSeedId,
        body: RuntimeExprSeed,
    ) -> Result<(), RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_define_trait_method_seed(method, body);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_define_trait_method_seed(
        &mut self,
        method: &RuntimeTraitMethodSeedId,
        body: RuntimeExprSeed,
    ) -> Result<(), RuntimePlanBuildError> {
        let (method_id, _, _, result) = method
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignTraitMethodSeed)?;
        let reserved = self
            .trait_methods
            .get(method_id.0)
            .ok_or(RuntimePlanBuildError::ForeignTraitMethodSeed)?;
        if reserved.body.is_some() {
            return Err(RuntimePlanBuildError::DuplicateTraitMethodDefinition {
                method: method_id,
            });
        }
        let inputs = reserved
            .inputs
            .iter()
            .map(|input| input.local())
            .collect::<Vec<_>>();
        let body = self.lower_expression(body)?;
        require_reserved_result("trait method result", result, body.ty())?;
        self.validate_callable_body_locals(&body, &inputs, &[])?;
        self.trait_methods[method_id.0].body = Some(body);
        Ok(())
    }

    pub fn push_entry(&mut self, value: RuntimeEntrySpec) -> Result<u32, RuntimePlanBuildError> {
        push_row(&mut self.entries, value, RuntimePlanTable::Entries)
    }

    pub fn push_callable_executable_seed(
        &mut self,
        seed: RuntimeCallableExecutableSeed,
    ) -> Result<u32, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_push_callable_executable_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_push_callable_executable_seed(
        &mut self,
        seed: RuntimeCallableExecutableSeed,
    ) -> Result<u32, RuntimePlanBuildError> {
        let code = match seed.code {
            RuntimeCallableExecutableSeedCode::PureHelper(helper) => {
                let (helper, _, _) = helper
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignPureHelperSeed)?;
                if self.pure_helpers.get(helper.0).is_none() {
                    return Err(RuntimePlanBuildError::ForeignPureHelperSeed);
                }
                RuntimeCallableExecutableCode::PureHelper(helper)
            }
            RuntimeCallableExecutableSeedCode::FunctionSite(site) => {
                let (site, _, _, _, _, _) = site
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
                let index = usize::try_from(site.get().get() - 1)
                    .map_err(|_| RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
                if self.function_sites.get(index).is_none() {
                    return Err(RuntimePlanBuildError::ForeignFunctionSiteSeed);
                }
                RuntimeCallableExecutableCode::FunctionSite(site)
            }
            RuntimeCallableExecutableSeedCode::ControllerFlow(flow) => {
                RuntimeCallableExecutableCode::ControllerFlow(flow)
            }
        };
        push_row(
            &mut self.callable_executables,
            RuntimeCallableExecutable {
                callable: seed.callable,
                contract: seed.contract,
                code,
            },
            RuntimePlanTable::CallableExecutables,
        )
    }

    pub fn push_flow_executable(
        &mut self,
        value: RuntimeFlowExecutable,
    ) -> Result<u32, RuntimePlanBuildError> {
        push_row(
            &mut self.flow_executables,
            value,
            RuntimePlanTable::FlowExecutables,
        )
    }

    pub fn push_flow_schema(
        &mut self,
        value: RuntimeFlowSchema,
    ) -> Result<u32, RuntimePlanBuildError> {
        push_row(&mut self.flow_schemas, value, RuntimePlanTable::FlowSchemas)
    }

    pub fn push_flow_seed(&mut self, seed: RuntimeFlowSeed) -> Result<u32, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_push_flow_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    /// Admits one complete typed stream transform into the immutable plan.
    pub fn push_stream_plan_seed(
        &mut self,
        seed: RuntimeStreamPlanSeed,
    ) -> Result<u32, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_push_stream_plan_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_push_stream_plan_seed(
        &mut self,
        seed: RuntimeStreamPlanSeed,
    ) -> Result<u32, RuntimePlanBuildError> {
        if self.stream_plans.iter().any(|plan| plan.id() == &seed.id) {
            return Err(RuntimePlanBuildError::DuplicateStreamDefinition {
                stream: seed.id.canonical_label(),
            });
        }
        let plan = self.lower_stream_plan_seed(seed)?;
        push_row(&mut self.stream_plans, plan, RuntimePlanTable::StreamPlans)
    }

    pub fn finish(self) -> Result<RuntimePlan, RuntimePlanBuildError> {
        self.finish_with_seal_limits(super::RuntimeTaskPlanSealLimits::default())
    }

    /// Consumes the builder with explicit semantic-validation policy.
    /// Limits are not executable semantics and never enter a digest.
    pub fn finish_with_seal_limits(
        self,
        limits: super::RuntimeTaskPlanSealLimits,
    ) -> Result<RuntimePlan, RuntimePlanBuildError> {
        let inventory = self.prepare_inventory(limits)?;
        inventory.verify()?;
        Ok(RuntimePlan {
            artifact: None,
            inventory,
        })
    }

    /// Materializes the sole table authority before public-plan construction.
    /// Semantic sealing borrows this same storage after structural admission.
    pub(super) fn prepare_inventory(
        self,
        limits: super::RuntimeTaskPlanSealLimits,
    ) -> Result<super::RuntimePlanInventory, RuntimePlanBuildError> {
        self.preflight_executable_inventory(limits)?;
        self.callable_states.borrow_mut().seal()?;
        self.validate_finish_preconditions()?;
        let mut function_site_builder = RuntimeFunctionSiteTableBuilder::new();
        for site in self.function_sites {
            let Some(body) = site.body else {
                unreachable!("incomplete function sites returned before materialization")
            };
            function_site_builder.push(super::RuntimeFunctionSite {
                definition: site.definition,
                role: site.role,
                function_type: site.function_type,
                inputs: site.inputs,
                result: site.result,
                invocation_effects: site.effects,
                body,
            })?;
        }
        let pure_helpers = self
            .pure_helpers
            .into_iter()
            .enumerate()
            .map(|(index, helper)| {
                let Some(expr) = helper.body else {
                    unreachable!("incomplete pure helpers returned before materialization")
                };
                RuntimePureHelper {
                    definition: helper.definition,
                    id: super::RuntimePureHelperId(index),
                    name: helper.name,
                    inputs: helper.inputs,
                    output_type: helper.output_abi,
                    expr,
                    scalar_eval_supported: helper.scalar_eval_supported,
                    origin: helper.origin,
                }
            })
            .collect();
        let trait_methods = self
            .trait_methods
            .into_iter()
            .enumerate()
            .map(|(index, method)| {
                let Some(body) = method.body else {
                    unreachable!("incomplete trait methods returned before materialization")
                };
                RuntimeTraitMethod {
                    definition: method.definition,
                    id: super::RuntimeTraitMethodId(index),
                    identity: method.identity,
                    receiver: method.receiver,
                    inputs: method.inputs,
                    output_type: method.output_abi,
                    body,
                }
            })
            .collect();
        let function_sites = function_site_builder.finish();
        let flows = self
            .flows
            .into_iter()
            .map(|flow| {
                let function = function_sites.shared(flow.function_site).ok_or_else(|| {
                    RuntimePlanBuildError::InvalidFlowFunction {
                        flow: flow.id.canonical_label(),
                    }
                })?;
                Ok(RuntimeFlow {
                    id: flow.id,
                    params: flow.params,
                    function_site: flow.function_site,
                    function,
                })
            })
            .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
        let type_table = self.types.finish()?;
        let mut meter = crate::task::semantic::TaskSemanticMeter::new(
            limits.max_semantic_work,
            limits.max_transcript_bytes,
        );
        let control_effect_contracts = super::RuntimeControlEffectContractTable::seal(
            self.control_effect_contracts
                .into_iter()
                .enumerate()
                .map(|(index, row)| {
                    row.ok_or(super::RuntimeControlEffectContractError::Incomplete { index })
                })
                .collect::<Result<Vec<_>, _>>()?,
            &type_table,
            limits,
            &mut meter,
        )?;
        let project_call_sites = self.project_call_sites.into_inner().finish();
        let local_declarations = self.locals.finish();
        validate_flow_parameters(&flows, &self.flow_schemas, &local_declarations, &type_table)?;
        let inventory = super::RuntimePlanInventory {
            type_table,
            local_declarations,
            nominal_record_domains: self.nominal_record_domains.finish(),
            variant_domains: self.variant_domains.finish(),
            function_sites,
            control_effect_contracts,
            defer_sites: self.defer_sites.into_boxed_slice(),
            callable_states: self.callable_states.into_inner().finish(),
            callable_specializations: self.callable_specializations.into_boxed_slice(),
            project_call_sites,
            format_attempts: super::RuntimeFormatAttemptTable::from_admitted_rows(
                self.format_attempts.into_boxed_slice(),
            ),
            dialogue_content: self.dialogue_content.finish(),
            entries: self.entries,
            callable_executables: self.callable_executables,
            flow_schemas: self.flow_schemas,
            flow_executables: self.flow_executables,
            flows,
            pure_helpers,
            pure_programs: self.pure_programs,
            trait_methods,
            line_task_groups: self.line_task_groups,
            stream_plans: self.stream_plans,
        };
        Ok(inventory)
    }

    fn preflight_executable_inventory(
        &self,
        limits: super::RuntimeTaskPlanSealLimits,
    ) -> Result<(), RuntimePlanBuildError> {
        // The accepted table inventory 0..13; table 14 has no candidate owner yet.
        // Auxiliary implementation tables are not extra semantic image rows.
        let counts = [
            self.types.len(),
            self.locals.len(),
            self.nominal_record_domains.len(),
            self.variant_domains.len(),
            self.function_sites.len(),
            self.dialogue_content.len(),
            self.entries.len(),
            self.callable_executables.len(),
            self.flow_executables.len(),
            self.flows.len(),
            self.pure_helpers.len(),
            self.trait_methods.len(),
            self.line_task_groups.len(),
            self.stream_plans.len(),
        ];
        let actual = counts.into_iter().try_fold(0_u32, |total, count| {
            u32::try_from(count)
                .ok()
                .and_then(|count| total.checked_add(count))
                .ok_or(RuntimePlanBuildError::ExecutableInventoryArithmeticOverflow)
        })?;
        if actual > limits.max_executable_rows {
            return Err(RuntimePlanBuildError::ExecutableInventoryRowsLimit {
                actual,
                maximum: limits.max_executable_rows,
            });
        }
        Ok(())
    }

    fn validate_finish_preconditions(&self) -> Result<(), RuntimePlanBuildError> {
        if self.poisoned {
            return Err(RuntimePlanBuildError::Poisoned);
        }
        let incomplete_callable_states = self
            .callable_states
            .borrow()
            .states
            .iter()
            .filter(|state| state.is_none())
            .count();
        if incomplete_callable_states != 0 {
            return Err(RuntimePlanBuildError::IncompleteCallableStates {
                count: incomplete_callable_states,
            });
        }
        let incomplete_function_sites = self
            .function_sites
            .iter()
            .filter(|site| site.body.is_none())
            .count();
        let incomplete_pure_helpers = self
            .pure_helpers
            .iter()
            .filter(|helper| helper.body.is_none())
            .count();
        let incomplete_trait_methods = self
            .trait_methods
            .iter()
            .filter(|method| method.body.is_none())
            .count();
        if incomplete_function_sites != 0
            || incomplete_pure_helpers != 0
            || incomplete_trait_methods != 0
        {
            return Err(RuntimePlanBuildError::IncompleteDefinitions {
                function_sites: incomplete_function_sites,
                pure_helpers: incomplete_pure_helpers,
                trait_methods: incomplete_trait_methods,
            });
        }
        if let Some((index, _)) = self
            .line_task_group_attachments
            .iter()
            .enumerate()
            .find(|(_, attached)| !**attached)
        {
            let group = RuntimeLineTaskGroupId::from_zero_based(index).ok_or(
                RuntimePlanBuildError::TooManyRows {
                    table: RuntimePlanTable::LineTaskGroups,
                },
            )?;
            return Err(RuntimePlanBuildError::OrphanLineTaskGroup { group });
        }
        Ok(())
    }

    fn validate_domain_exclusivity(
        &self,
        records: &[RuntimeNominalRecordDomain],
        variants: &[RuntimeVariantDomain],
    ) -> Result<(), RuntimePlanBuildError> {
        let record_owners = records
            .iter()
            .map(RuntimeNominalRecordDomain::owner)
            .collect::<BTreeSet<_>>();
        let variant_owners = variants
            .iter()
            .map(RuntimeVariantDomain::owner)
            .collect::<BTreeSet<_>>();
        if let Some(owner) = record_owners.intersection(&variant_owners).next() {
            return Err(RuntimePlanBuildError::ConflictingNominalDomainKinds { owner: *owner });
        }
        for owner in record_owners {
            if self.variant_domains.contains_owner(owner) {
                return Err(RuntimePlanBuildError::ConflictingNominalDomainKinds { owner });
            }
        }
        for owner in variant_owners {
            if self.nominal_record_domains.contains_owner(owner) {
                return Err(RuntimePlanBuildError::ConflictingNominalDomainKinds { owner });
            }
        }
        Ok(())
    }

    fn try_push_flow_seed(&mut self, seed: RuntimeFlowSeed) -> Result<u32, RuntimePlanBuildError> {
        let (id, declaration, body) = seed.into_parts();
        if declaration.role != super::RuntimeFunctionSemanticRole::Flow
            || declaration.body_kind != RuntimeFunctionSiteBodyKind::Executable
        {
            return Err(RuntimePlanBuildError::InvalidFlowFunction {
                flow: id.canonical_label(),
            });
        }
        let site = self.reserve_function_site_seed(declaration)?;
        let (function_site, ..) = site
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
        self.define_function_site_seed(&site, seed::RuntimeFunctionSiteBodySeed::Executable(body))?;
        let row = self
            .function_sites
            .get(function_site.get().get() as usize - 1)
            .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
        let params = row
            .inputs
            .iter()
            .filter(|input| matches!(input.source(), RuntimeFunctionInputSource::Parameter { .. }))
            .map(RuntimeFunctionInputBinding::input_local)
            .collect::<Box<[_]>>();
        push_row(
            &mut self.flows,
            ReservedFlowRoot {
                id,
                function_site,
                params,
            },
            RuntimePlanTable::Flows,
        )
    }

    fn resolve_function_locals(
        &self,
        locals: impl IntoIterator<Item = RuntimeLocalSeedId>,
    ) -> Result<Vec<(RuntimeLocalDeclarationId, RuntimePlanTypeId)>, RuntimePlanBuildError> {
        let mut resolved = Vec::new();
        let mut unique = BTreeSet::new();
        for local in locals {
            let (local, ty) = local
                .resolve(&self.issuer)
                .ok_or(RuntimePlanBuildError::ForeignLocalSeed)?;
            if !self.locals.contains(local) {
                return Err(RuntimePlanBuildError::UnknownFunctionLocal { local });
            }
            if !unique.insert(local) {
                return Err(RuntimePlanBuildError::DuplicateFunctionLocal { local });
            }
            resolved.push((local, ty));
        }
        Ok(resolved)
    }

    fn ensure_usable(&self) -> Result<(), RuntimePlanBuildError> {
        if self.poisoned {
            Err(RuntimePlanBuildError::Poisoned)
        } else {
            Ok(())
        }
    }
}

fn line_task_seed_requests_detach(seed: &RuntimeLineTaskGroupSeed) -> bool {
    matches!(
        seed.cleanup_policy.child_tasks,
        crate::line_task::ChildTaskCleanup::Detach
    ) || line_task_node_requests_detach(&seed.root)
}

fn line_task_node_requests_detach(node: &RuntimeLineTaskNodeSeed) -> bool {
    match node {
        RuntimeLineTaskNodeSeed::Sequence(children)
        | RuntimeLineTaskNodeSeed::Start(children)
        | RuntimeLineTaskNodeSeed::Parallel { children, .. } => {
            children.iter().any(line_task_node_requests_detach)
        }
        RuntimeLineTaskNodeSeed::Child {
            cancel_policy,
            scope,
            ..
        } => {
            *cancel_policy == crate::line_task::ChildCancelPolicy::Detach
                || line_task_node_requests_detach(scope)
        }
        RuntimeLineTaskNodeSeed::Action(_) => false,
    }
}

fn validate_flow_parameters(
    flows: &[RuntimeFlow],
    schemas: &[RuntimeFlowSchema],
    locals: &super::RuntimeLocalDeclarationTable,
    types: &super::RuntimePlanTypeTable,
) -> Result<(), RuntimePlanBuildError> {
    let mut flow_ids = BTreeSet::new();
    for flow in flows {
        let label = flow.id.canonical_label();
        if !flow_ids.insert(flow.id.clone()) {
            return Err(RuntimePlanBuildError::DuplicateFlowDefinition { flow: label });
        }
        let mut unique = BTreeSet::new();
        for &local in &flow.params {
            if !unique.insert(local) {
                return Err(RuntimePlanBuildError::DuplicateFlowParameter { flow: label, local });
            }
            if !locals.contains(local) {
                return Err(RuntimePlanBuildError::UnknownFlowParameter { flow: label, local });
            }
        }
        let mut matching = schemas.iter().filter(|row| row.flow == flow.id);
        let Some(schema) = matching.next() else {
            return Err(RuntimePlanBuildError::MissingFlowSchema { flow: label });
        };
        if matching.next().is_some() {
            return Err(RuntimePlanBuildError::DuplicateFlowSchema { flow: label });
        }
        if schema.parameters.len() != flow.params.len() {
            return Err(RuntimePlanBuildError::FlowParameterCount {
                flow: label,
                expected: schema.parameters.len(),
                actual: flow.params.len(),
            });
        }
        let mut parameter_names = BTreeSet::new();
        for (index, (parameter, &local)) in schema.parameters.iter().zip(&flow.params).enumerate() {
            if parameter.name.is_empty() {
                return Err(RuntimePlanBuildError::EmptyFlowParameterName { flow: label, index });
            }
            if !parameter_names.insert(parameter.name.as_str()) {
                return Err(RuntimePlanBuildError::DuplicateFlowParameterName {
                    flow: label,
                    name: parameter.name.clone(),
                });
            }
            if parameter.coordinate.index().ok() != Some(index) {
                return Err(RuntimePlanBuildError::FlowParameterPosition {
                    flow: label,
                    index,
                    actual: parameter.coordinate.position(),
                });
            }
            let local_ty = locals
                .get(local)
                .ok_or(RuntimePlanBuildError::UnknownFlowParameter {
                    flow: label.clone(),
                    local,
                })?
                .ty();
            let matches = types.get(local_ty).is_some_and(|declaration| {
                declaration.semantic_identity() == parameter.semantic_identity
            });
            if !matches {
                let expected = format!("semantic type {:?}", parameter.semantic_identity);
                let actual = types.get(local_ty).map_or_else(
                    || "missing type declaration".to_owned(),
                    |declaration| format!("{:?}", declaration.projection()),
                );
                return Err(RuntimePlanBuildError::FlowParameterType {
                    flow: label,
                    index,
                    local,
                    expected,
                    actual,
                });
            }
        }
        if !flow.matches_parameter_contract(schema) {
            return Err(RuntimePlanBuildError::FlowParameterContractMismatch { flow: label });
        }
    }
    for schema in schemas {
        if !flow_ids.contains(&schema.flow) {
            return Err(RuntimePlanBuildError::MissingFlowDefinition {
                flow: schema.flow.canonical_label(),
            });
        }
    }
    Ok(())
}

impl Default for RuntimePlanBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn resolve_semantic_type(
    types: &PreparedRuntimePlanTypeBatch,
    semantic_identity: RuntimeSemanticTypeId,
) -> Result<RuntimePlanTypeId, RuntimePlanBuildError> {
    types
        .id_for_semantic(semantic_identity)
        .ok_or(RuntimePlanBuildError::UnknownSemanticType { semantic_identity })
}

fn validate_function_input_bindings(
    inputs: &[RuntimeFunctionInputBinding],
) -> Result<(), RuntimePlanBuildError> {
    let mut captures = 0_u32;
    let mut parameters = 0_u32;
    let mut parameter_phase = false;
    let mut locals = BTreeSet::new();
    for (index, input) in inputs.iter().enumerate() {
        if !input.source().accepts_transfer(input.transfer()) {
            return Err(RuntimePlanBuildError::InvalidFunctionInputSource { index });
        }
        if !input.source().accepts_origin(input.origin()) {
            return Err(RuntimePlanBuildError::InvalidFunctionInputSource { index });
        }
        if input
            .unrestricted_bindings()
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(RuntimePlanBuildError::InvalidFunctionInputSource { index });
        }
        for &local in input.unrestricted_bindings() {
            if !crate::pattern::runtime_pattern_contains_binding(input.pattern(), local) {
                return Err(RuntimePlanBuildError::InvalidFunctionInputOwnership { index, local });
            }
        }
        if !locals.insert(input.input_local()) {
            return Err(RuntimePlanBuildError::DuplicateFunctionInputLocal {
                index,
                local: input.input_local(),
            });
        }
        match input.source() {
            RuntimeFunctionInputSource::Capture { position }
            | RuntimeFunctionInputSource::CapturedParameter { position, .. }
                if !parameter_phase && position == captures =>
            {
                captures = captures
                    .checked_add(1)
                    .ok_or(RuntimePlanBuildError::InvalidFunctionInputSource { index })?;
            }
            RuntimeFunctionInputSource::Parameter { position, .. } if position == parameters => {
                parameter_phase = true;
                parameters = parameters
                    .checked_add(1)
                    .ok_or(RuntimePlanBuildError::InvalidFunctionInputSource { index })?;
            }
            RuntimeFunctionInputSource::Capture { .. }
            | RuntimeFunctionInputSource::CapturedParameter { .. }
            | RuntimeFunctionInputSource::Parameter { .. } => {
                return Err(RuntimePlanBuildError::InvalidFunctionInputSource { index });
            }
        }
    }
    Ok(())
}

fn rewrite_record_domain(
    types: &PreparedRuntimePlanTypeBatch,
    seed: &RuntimeNominalRecordDomainSeed,
) -> Result<RuntimeNominalRecordDomain, RuntimePlanBuildError> {
    let owner = resolve_semantic_type(types, seed.owner())?;
    let owner_declaration = types
        .get(owner)
        .ok_or(RuntimePlanBuildError::UnknownSemanticType {
            semantic_identity: seed.owner(),
        })?;
    if !matches!(
        owner_declaration.projection(),
        RuntimePlanTypeProjection::Nominal { .. }
    ) {
        return Err(RuntimePlanBuildError::InvalidNominalRecordOwner { owner });
    }
    let fields = seed
        .fields()
        .iter()
        .map(|field| {
            Ok((
                field.field(),
                field.name().map(str::to_owned),
                resolve_semantic_type(types, field.ty())?,
            ))
        })
        .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
    Ok(RuntimeNominalRecordDomain::from_admitted_parts(
        owner,
        seed.shape(),
        fields,
    ))
}

fn rewrite_variant_domain(
    types: &PreparedRuntimePlanTypeBatch,
    seed: &RuntimeVariantDomainSeed,
) -> Result<RuntimeVariantDomain, RuntimePlanBuildError> {
    let owner = resolve_semantic_type(types, seed.owner())?;
    let owner_declaration = types
        .get(owner)
        .ok_or(RuntimePlanBuildError::UnknownSemanticType {
            semantic_identity: seed.owner(),
        })?;
    match owner_declaration.projection() {
        RuntimePlanTypeProjection::Nominal {
            nominal, layout, ..
        } => {
            if nominal != seed.nominal() {
                return Err(RuntimePlanBuildError::VariantNominalMismatch {
                    owner,
                    expected: nominal.clone(),
                    actual: seed.nominal().clone(),
                });
            }
            if *layout != seed.layout() {
                return Err(RuntimePlanBuildError::VariantLayoutMismatch {
                    owner,
                    expected: *layout,
                    actual: seed.layout(),
                });
            }
        }
        RuntimePlanTypeProjection::Opaque { .. } => {}
        _ => return Err(RuntimePlanBuildError::InvalidVariantOwner { owner }),
    }
    let cases = seed
        .cases()
        .iter()
        .map(|case| {
            let payload = case
                .payload()
                .map(|semantic_identity| resolve_semantic_type(types, semantic_identity))
                .transpose()?;
            Ok((case.name().to_owned(), payload))
        })
        .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
    Ok(RuntimeVariantDomain::from_admitted_parts(
        owner,
        seed.nominal().clone(),
        seed.layout(),
        cases,
    ))
}

fn push_row<T>(
    rows: &mut Vec<T>,
    value: T,
    table: RuntimePlanTable,
) -> Result<u32, RuntimePlanBuildError> {
    let index =
        u32::try_from(rows.len()).map_err(|_| RuntimePlanBuildError::TooManyRows { table })?;
    rows.len()
        .checked_add(1)
        .and_then(|len| u32::try_from(len).ok())
        .ok_or(RuntimePlanBuildError::TooManyRows { table })?;
    rows.push(value);
    Ok(index)
}

fn require_reserved_result(
    context: &'static str,
    expected: RuntimePlanTypeId,
    actual: RuntimePlanTypeId,
) -> Result<(), RuntimePlanBuildError> {
    if expected == actual {
        Ok(())
    } else {
        Err(RuntimePlanBuildError::TypeMismatch {
            context,
            expected,
            actual,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{
        RuntimeNominalSchemaBody, RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition,
        RuntimeNominalSchemaField, RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity,
        RuntimeSchemaLimits, RuntimeTypeSchema as Schema, TypeLayoutHash,
    };
    use crate::pattern::RuntimeCheckedType;
    use crate::plan::{
        RuntimeNominalRecordDomainFieldSeed, RuntimePlanTypeProjection,
        RuntimePlanTypeResolutionError, RuntimeVariantCaseSeed,
    };
    use crate::value::RuntimeValue;

    fn identity(marker: u8) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes([marker; 32])
    }

    fn nominal() -> RuntimeNominalTypeId {
        RuntimeNominalTypeId::try_new("game.State").expect("nominal identity")
    }

    #[test]
    fn dialogue_result_target_seed_keeps_target_and_pattern_types_correlated() {
        let target = identity(1);
        let pattern_type = identity(2);
        assert_eq!(
            RuntimeDialogueResultTargetSeed::try_new(
                target,
                RuntimePatternSeed::new(pattern_type, RuntimePatternSeedKind::Discard),
            ),
            Err(RuntimeDialogueResultTargetSeedError::PatternTypeMismatch {
                target,
                pattern: pattern_type,
            })
        );

        let discard = RuntimeDialogueResultTargetSeed::discard(target);
        assert_eq!(discard.ty(), target);
        assert_eq!(discard.pattern().ty(), target);
    }

    fn schema(body: RuntimeNominalSchemaBody) -> RuntimeNominalSchemaGraph {
        RuntimeNominalSchemaGraph::try_new(
            vec![RuntimeNominalSchemaDefinition::new(
                crate::entry::RuntimeNominalDeclarationId::from_bytes(
                    *(RuntimeNominalSchemaIdentity::new(nominal(), identity(1)))
                        .semantic_identity()
                        .as_bytes(),
                ),
                RuntimeNominalSchemaIdentity::new(nominal(), identity(1)),
                vec![],
                body,
            )],
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap()
    }

    fn variant_schema(name: &str, payload: Option<Schema>) -> RuntimeNominalSchemaGraph {
        schema(RuntimeNominalSchemaBody::Variant {
            cases: vec![RuntimeNominalSchemaCase::new(0, name.to_owned(), payload)]
                .into_boxed_slice(),
        })
    }

    fn type_seeds(schema: &RuntimeNominalSchemaGraph) -> Vec<RuntimePlanTypeSeed> {
        vec![
            RuntimePlanTypeSeed::new(
                identity(1),
                RuntimePlanTypeProjection::Nominal {
                    nominal: nominal(),
                    layout: schema.try_layout_hash(identity(1)).unwrap(),
                    arguments: Box::new([]),
                },
            ),
            RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::Bool),
        ]
    }

    #[test]
    fn record_domain_failure_rolls_back_types_and_locals() {
        let schema = schema(RuntimeNominalSchemaBody::Record {
            shape: crate::entry::RuntimeNominalRecordShape::Record,
            fields: vec![RuntimeNominalSchemaField::new(
                crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                Some("value".to_owned()),
                Schema::Bool,
            )]
            .into_boxed_slice(),
        });
        let mut builder = RuntimePlanBuilder::new();
        let invalid = RuntimeNominalRecordDomainSeed::new(
            identity(1),
            crate::entry::RuntimeNominalRecordShape::Record,
            [
                RuntimeNominalRecordDomainFieldSeed::new(
                    crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                    Some("value".to_owned()),
                    identity(2),
                ),
                RuntimeNominalRecordDomainFieldSeed::new(
                    crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(1).unwrap(),
                    Some("value".to_owned()),
                    identity(2),
                ),
            ],
        );
        assert!(matches!(
            builder.admit_semantic_batch(
                type_seeds(&schema),
                [RuntimeLocalDeclarationSeed::new(manual_local_origin("arcweft-core.fixture.plan.construction.record_domain_failure_rolls_back_types_and_locals.binding_a"), identity(2))],
                [invalid],
                [],
                &schema,
            ),
            Err(RuntimePlanBuildError::NominalRecordDomain(
                RuntimeNominalRecordDomainError::Shape {
                    source: crate::entry::RuntimeNominalRecordShapeError::DuplicateFieldName { .. },
                    ..
                }
            ))
        ));

        let valid = RuntimeNominalRecordDomainSeed::new(
            identity(1),
            crate::entry::RuntimeNominalRecordShape::Record,
            [RuntimeNominalRecordDomainFieldSeed::new(
                crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                Some("value".to_owned()),
                identity(2),
            )],
        );
        let admitted = builder
            .admit_semantic_batch(
                type_seeds(&schema),
                [RuntimeLocalDeclarationSeed::new(manual_local_origin("arcweft-core.fixture.plan.construction.record_domain_failure_rolls_back_types_and_locals.binding_b"), identity(2))],
                [valid],
                [],
                &schema,
            )
            .expect("failed transaction committed nothing");
        assert_eq!(admitted.local_ids().len(), 1);
        let plan = builder.finish().expect("sealed plan");
        let record_ty = plan
            .type_table()
            .id_for_semantic(identity(1))
            .expect("record type");
        assert_eq!(plan.type_table().len(), 2);
        assert_eq!(plan.local_declarations().len(), 1);
        assert!(plan.nominal_record_domains().get(record_ty).is_some());
    }

    #[test]
    fn variant_domain_failure_rolls_back_the_type_batch() {
        let schema = variant_schema("Ready", Some(Schema::Bool));
        let mut builder = RuntimePlanBuilder::new();
        let empty = RuntimeVariantDomainSeed::new(
            identity(1),
            nominal(),
            schema.try_layout_hash(identity(1)).unwrap(),
            [],
        );
        assert!(matches!(
            builder.admit_semantic_batch(type_seeds(&schema), [], [], [empty], &schema),
            Err(RuntimePlanBuildError::VariantDomain(
                RuntimeVariantDomainError::EmptyDomain { .. }
            ))
        ));

        let valid = RuntimeVariantDomainSeed::new(
            identity(1),
            nominal(),
            schema.try_layout_hash(identity(1)).unwrap(),
            [RuntimeVariantCaseSeed::new("Ready", Some(identity(2)))],
        );
        builder
            .admit_semantic_batch(type_seeds(&schema), [], [], [valid], &schema)
            .expect("failed transaction committed nothing");
        let plan = builder.finish().expect("sealed plan");
        let variant_ty = plan
            .type_table()
            .id_for_semantic(identity(1))
            .expect("variant type");
        assert!(plan.variant_domains().get(variant_ty).is_some());
        assert!(matches!(
            plan.checked_type(variant_ty),
            Ok(Some(RuntimeCheckedType::Variant { .. }))
        ));
    }

    #[test]
    fn recursive_variant_domain_is_checked_without_materializing_its_predicate() {
        let schema = variant_schema(
            "Next",
            Some(Schema::NominalRef(RuntimeNominalSchemaIdentity::new(
                nominal(),
                identity(1),
            ))),
        );
        let mut builder = RuntimePlanBuilder::new();
        let recursive = RuntimeVariantDomainSeed::new(
            identity(1),
            nominal(),
            schema.try_layout_hash(identity(1)).unwrap(),
            [RuntimeVariantCaseSeed::new("Next", Some(identity(1)))],
        );
        builder
            .admit_semantic_batch(type_seeds(&schema), [], [], [recursive], &schema)
            .expect("recursive nominal domain is structurally valid");
        let plan = builder.finish().expect("sealed plan");
        let recursive_ty = plan
            .type_table()
            .id_for_semantic(identity(1))
            .expect("recursive type");

        assert_eq!(
            plan.type_class(recursive_ty),
            Ok(crate::plan::RuntimePlanTypeClass::Checked)
        );
        assert_eq!(
            plan.checked_type(recursive_ty),
            Err(RuntimePlanTypeResolutionError::CheckedProjectionCycle { ty: recursive_ty })
        );
    }

    #[test]
    fn variant_layout_is_correlated_before_admission_and_value_acceptance() {
        let schema = variant_schema("Ready", None);
        let layout = schema.try_layout_hash(identity(1)).unwrap();
        let wrong_layout = TypeLayoutHash::from_bytes([4; 32]);
        let domain = |layout| {
            RuntimeVariantDomainSeed::new(
                identity(1),
                nominal(),
                layout,
                [RuntimeVariantCaseSeed::new("Ready", None)],
            )
        };
        let mut rejected = RuntimePlanBuilder::new();
        assert!(matches!(
            rejected.admit_semantic_batch(type_seeds(&schema), [], [], [domain(wrong_layout)], &schema),
            Err(RuntimePlanBuildError::VariantLayoutMismatch { expected, actual, .. })
                if expected == layout && actual == wrong_layout
        ));
        assert_eq!(rejected.finish().unwrap().type_table().len(), 0);

        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_semantic_batch(type_seeds(&schema), [], [], [domain(layout)], &schema)
            .unwrap();
        let plan = builder.finish().unwrap();
        let ty = plan.type_table().id_for_semantic(identity(1)).unwrap();
        assert_eq!(plan.variant_domains().get(ty).unwrap().layout(), layout);
        let checked = plan.checked_type(ty).unwrap().unwrap();
        let value = |layout| RuntimeValue::Variant {
            owner: crate::pattern::RuntimeVariantIdentity::Nominal {
                nominal: nominal(),
                semantic_identity: identity(1),
                layout,
            },
            ordinal: 0,
            name: "Ready".to_owned(),
            payload: None,
            type_instantiation: None,
        };
        assert!(checked.accepts_value(&value(layout)));
        assert!(!checked.accepts_value(&value(wrong_layout)));
        let mut other_layout = checked.clone();
        let RuntimeCheckedType::Variant {
            owner: crate::pattern::RuntimeVariantIdentity::Nominal { layout, .. },
            ..
        } = &mut other_layout
        else {
            unreachable!()
        };
        *layout = TypeLayoutHash::from_bytes([4; 32]);
        // A derived layout cannot become an input to the semantic identity
        // from which the nominal graph derives that same layout.
        assert_eq!(
            checked.semantic_identity_digest(),
            other_layout.semantic_identity_digest()
        );
    }

    #[test]
    fn executable_row_limit_counts_types_and_locals_before_publication() {
        let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
        let make = || {
            let mut builder = RuntimePlanBuilder::new();
            builder
                .admit_type_batch(
                    [RuntimePlanTypeSeed::new(
                        unit,
                        RuntimePlanTypeProjection::Unit,
                    )],
                    [RuntimeLocalDeclarationSeed::new(
                        super::super::RuntimeLocalOrigin::Binding([0x41; 32]),
                        unit,
                    )],
                )
                .unwrap();
            builder
        };
        let limits = super::super::RuntimeTaskPlanSealLimits {
            max_executable_rows: 2,
            ..super::super::RuntimeTaskPlanSealLimits::default()
        };
        let plan = make().finish_with_seal_limits(limits).unwrap();
        assert_eq!(plan.type_table().len(), 1);
        assert_eq!(plan.local_declarations().len(), 1);
        assert!(matches!(
            make().finish_with_seal_limits(super::super::RuntimeTaskPlanSealLimits {
                max_executable_rows: 1,
                ..limits
            }),
            Err(RuntimePlanBuildError::ExecutableInventoryRowsLimit {
                actual: 2,
                maximum: 1
            })
        ));
    }

    #[test]
    fn finish_policy_reaches_the_shared_control_contract_meter() {
        let make = || {
            let mut builder = RuntimePlanBuilder::new();
            builder
                .push_control_effect_contract(
                    super::super::RuntimeControlEffectContractDefinition {
                        mode: super::super::RuntimeTaskControlMode::StraightLine,
                        effects: Box::new([]),
                        children: Box::new([]),
                    },
                )
                .unwrap();
            builder
        };
        assert!(make().finish().is_ok());
        assert!(matches!(
            make().finish_with_seal_limits(super::super::RuntimeTaskPlanSealLimits {
                max_semantic_work: 0,
                ..super::super::RuntimeTaskPlanSealLimits::default()
            }),
            Err(RuntimePlanBuildError::ControlEffectContract(
                super::super::RuntimeControlEffectContractError::WorkLimit
            ))
        ));
    }

    #[test]
    fn executable_row_preflight_precedes_unfinished_definition_validation() {
        let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    unit,
                    RuntimePlanTypeProjection::Unit,
                )],
                [],
            )
            .unwrap();
        builder
            .reserve_pure_helper_seed(RuntimePureHelperDeclarationSeed {
                definition: super::super::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [0x42; 32],
                ),
                name: "unfinished".to_owned(),
                inputs: Box::new([]),
                result: unit,
                output_abi: super::super::RuntimePureOutputType::Value,
                scalar_eval_supported: false,
                origin: super::super::RuntimePureHelperOrigin::Annotated,
            })
            .unwrap();
        assert!(matches!(
            builder.finish_with_seal_limits(super::super::RuntimeTaskPlanSealLimits {
                max_executable_rows: 1,
                ..super::super::RuntimeTaskPlanSealLimits::default()
            }),
            Err(RuntimePlanBuildError::ExecutableInventoryRowsLimit {
                actual: 2,
                maximum: 1
            })
        ));
    }

    #[test]
    fn function_semantic_role_survives_materialization_independently_of_body_kind() {
        use crate::plan::RuntimeFunctionSemanticRole;
        let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    unit,
                    RuntimePlanTypeProjection::Unit,
                )],
                [],
            )
            .unwrap();
        for role in [
            RuntimeFunctionSemanticRole::Ordinary,
            RuntimeFunctionSemanticRole::Closure,
        ] {
            builder
                .push_function_site_seed(
                    crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [41; 32],
                    ),
                    role,
                    [],
                    RuntimeExprSeed::new(unit, RuntimeExprSeedKind::Value(RuntimeValue::Unit)),
                )
                .unwrap();
        }
        let plan = builder.finish().unwrap();
        let sites = plan.function_sites().iter().collect::<Vec<_>>();
        assert_eq!(sites[0].role(), RuntimeFunctionSemanticRole::Ordinary);
        assert_eq!(sites[1].role(), RuntimeFunctionSemanticRole::Closure);
        assert_eq!(sites[0].definition().as_bytes(), &[41; 32]);
        assert_eq!(sites[0].definition(), sites[1].definition());
        assert_eq!(sites[0].body(), sites[1].body());
        assert_ne!(sites[0], sites[1]);
        assert_eq!(sites[0].role().semantic_tag(), 0);
        assert_eq!(sites[1].role().semantic_tag(), 1);
    }

    #[test]
    fn conflicting_batch_does_not_commit_local_rows() {
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    identity(1),
                    RuntimePlanTypeProjection::Bool,
                )],
                [],
            )
            .expect("initial bool type");
        assert!(matches!(
            builder.admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    identity(1),
                    RuntimePlanTypeProjection::String,
                )],
                [RuntimeLocalDeclarationSeed::new(manual_local_origin("arcweft-core.fixture.plan.construction.conflicting_batch_does_not_commit_local_rows.binding_a"), identity(1))],
            ),
            Err(RuntimePlanBuildError::TypeGraph(
                RuntimePlanTypeTableError::ConflictingProjection { .. }
            ))
        ));

        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    identity(2),
                    RuntimePlanTypeProjection::String,
                )],
                [RuntimeLocalDeclarationSeed::new(manual_local_origin("arcweft-core.fixture.plan.construction.conflicting_batch_does_not_commit_local_rows.binding_b"), identity(2))],
            )
            .expect("failed batch left no local row");
        let plan = builder.finish().expect("unpoisoned preflight failure");
        assert_eq!(plan.type_table().len(), 2);
        assert_eq!(plan.local_declarations().len(), 1);
    }

    #[test]
    fn function_input_origin_rejects_an_unrelated_semantic_role_atomically() {
        for (source, origin, transfer) in [
            (
                RuntimeFunctionInputSource::Capture { position: 0 },
                super::super::RuntimeFunctionInputOrigin::Parameter(
                    crate::plan::RuntimeFunctionParameterIdentity::from_accepted_identity([1; 32]),
                ),
                crate::plan::RuntimeFunctionInputTransfer::ExternalBinding,
            ),
            (
                RuntimeFunctionInputSource::CapturedParameter {
                    position: 0,
                    passing: super::super::RuntimeFunctionParameterPassing::Value,
                },
                super::super::RuntimeFunctionInputOrigin::Binding([1; 32]),
                crate::plan::RuntimeFunctionInputTransfer::Formal,
            ),
            (
                RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: super::super::RuntimeFunctionParameterPassing::Value,
                },
                super::super::RuntimeFunctionInputOrigin::EvaluatedResult(
                    super::super::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [1; 32],
                    ),
                ),
                crate::plan::RuntimeFunctionInputTransfer::Formal,
            ),
            (
                RuntimeFunctionInputSource::Capture { position: 0 },
                super::super::RuntimeFunctionInputOrigin::Binding([1; 32]),
                crate::plan::RuntimeFunctionInputTransfer::Formal,
            ),
            (
                RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: crate::plan::RuntimeFunctionParameterPassing::Value,
                },
                super::super::RuntimeFunctionInputOrigin::Parameter(
                    crate::plan::RuntimeFunctionParameterIdentity::from_accepted_identity([1; 32]),
                ),
                crate::plan::RuntimeFunctionInputTransfer::Transferred(
                    crate::plan::RuntimeFunctionCaptureMode::Copy,
                ),
            ),
        ] {
            let mut builder = RuntimePlanBuilder::new();
            let admitted = builder
                .admit_type_batch(
                    [RuntimePlanTypeSeed::new(
                        identity(1),
                        RuntimePlanTypeProjection::Bool,
                    )],
                    [RuntimeLocalDeclarationSeed::new(manual_local_origin("arcweft-core.fixture.plan.construction.function_input_origin_rejects_an_unrelated_semantic_role_atomically.binding_a"), identity(1))],
                )
                .unwrap();
            let result = builder.reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                definition: super::super::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [2; 32],
                ),
                role: super::super::RuntimeFunctionSemanticRole::Ordinary,
                function_type: None,
                inputs: Box::new([RuntimeFunctionInputBindingSeed {
                    transfer,
                    origin,
                    source,
                    input_local: admitted.local_ids()[0].clone(),
                    pattern: RuntimePatternSeed::new(identity(1), RuntimePatternSeedKind::Discard),
                    ownership: Default::default(),
                    unrestricted_bindings: Box::new([]),
                }]),
                result: identity(1),
                body_kind: RuntimeFunctionSiteBodyKind::Expression,
                effects: RuntimeEffectSet::empty(),
            });
            assert!(matches!(
                result,
                Err(RuntimePlanBuildError::InvalidFunctionInputSource { index: 0 })
            ));
            assert!(builder.function_sites.is_empty());
            assert!(builder.poisoned);
        }
    }

    #[test]
    fn cross_builder_local_injection_poisoned_the_target_builder() {
        let mut first = RuntimePlanBuilder::new();
        let foreign = first
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    identity(1),
                    RuntimePlanTypeProjection::Bool,
                )],
                [RuntimeLocalDeclarationSeed::new(manual_local_origin("arcweft-core.fixture.plan.construction.cross_builder_local_injection_poisoned_the_target_builder.binding_a"), identity(1))],
            )
            .expect("first admission")
            .local_ids()[0]
            .clone();
        let mut second = RuntimePlanBuilder::new();
        second
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    identity(1),
                    RuntimePlanTypeProjection::Bool,
                )],
                [RuntimeLocalDeclarationSeed::new(manual_local_origin("arcweft-core.fixture.plan.construction.cross_builder_local_injection_poisoned_the_target_builder.binding_b"), identity(1))],
            )
            .expect("second admission");

        assert_eq!(
            second.push_function_site_seed(
                crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([41; 32]),
                crate::plan::RuntimeFunctionSemanticRole::Ordinary,
                [RuntimeFunctionInputBindingSeed {
                    transfer: crate::plan::RuntimeFunctionInputTransfer::Formal,
                    origin: crate::plan::RuntimeFunctionInputOrigin::Parameter(
                        crate::plan::RuntimeFunctionParameterIdentity::from_accepted_identity(
                            [81; 32]
                        )
                    ),
                    ownership: Default::default(),
                    unrestricted_bindings: Box::new([]),
                    source: RuntimeFunctionInputSource::Parameter {
                        position: 0,
                        passing: crate::plan::RuntimeFunctionParameterPassing::Value
                    },
                    input_local: foreign.clone(),
                    pattern: RuntimePatternSeed::new(
                        identity(1),
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: foreign.clone(),
                        },
                    ),
                }],
                RuntimeExprSeed::new(
                    identity(1),
                    RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                ),
            ),
            Err(RuntimePlanBuildError::ForeignLocalSeed)
        );
        assert_eq!(second.finish(), Err(RuntimePlanBuildError::Poisoned));
    }
}

#[cfg(test)]
fn manual_local_origin(declaration: &str) -> crate::plan::RuntimeLocalOrigin {
    // This fixture declares a semantic binding name independent of its value,
    // type, source offset, and builder-issued local ordinal.
    let mut identity = blake3::Hasher::new();
    identity.update(b"arcweft.manual-fixture-binding.v1\0");
    identity.update(declaration.as_bytes());
    crate::plan::RuntimeLocalOrigin::Binding(*identity.finalize().as_bytes())
}
