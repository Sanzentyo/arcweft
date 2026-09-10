//! Final semantic projection into runtime-plan facts.
//!
//! This module is the compiler-owned dependency inversion boundary between
//! semantic analysis and runtime-plan lowering. It consumes the exact accepted
//! final-HIR generation and never opens source text, rebuilds a detached HIR,
//! or consults the removed `TypeCheckReport` sidecar.

mod closure_instances;
#[path = "lower/evaluated_effects.rs"]
mod evaluated_effects;
#[path = "lower/fx.rs"]
pub(crate) mod fx;
mod project_instances;
#[path = "lower/reachability.rs"]
mod reachability;
#[path = "lower/text_proxy.rs"]
mod text_proxy;
#[path = "lower/variants.rs"]
mod variants;

use evaluated_effects::{runtime_evaluated_effect, runtime_evaluated_effect_under};
use project_instances::{
    DiscoveredProjectInstances, ProjectInstanceNode, ProjectInstanceProjection,
    ProjectInstanceSelection, ProjectInstanceTypes, ProjectInstantiationSession,
};
pub use project_instances::{
    ProjectInstantiationControl, ProjectInstantiationError, ProjectInstantiationLimitKind,
    ProjectInstantiationLimits, ProjectInstantiationOrigin,
};
use variants::{runtime_variant, runtime_variant_under};

pub(crate) use reachability::project_view_value_program_reachability;
pub use reachability::{
    RuntimeEmissionMode, RuntimeReachabilityProjectionError, project_runtime_reachability,
    validate_reachable_runtime_callables,
};

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_character::{
    id::CharacterId,
    presentation_name::{
        CharacterDisplayNameInput, CharacterDisplayNameRecordInput, CharacterDisplayNameValue,
        CharacterNameFallbackLocale, CharacterNameLocale, CharacterNameLocalePolicy,
        CharacterPresentationCatalogData, CharacterPresentationCatalogGeneration,
        CharacterPresentationCatalogInput, CharacterPresentationCatalogRevision,
        CharacterPresentationRole,
    },
};
use arcweft_core::{
    entry::RuntimeNominalTypeId,
    pattern::RuntimeOpaqueTypeProducerId,
    plan::{
        FlowRuntimeId, RuntimeBuiltinIteratorFamily, RuntimeDialogueValueRole, RuntimeLineId,
        RuntimeLocalDeclarationTableError,
    },
    runtime_id::{
        RuntimeDialogueContentTemplateId, RuntimeDialogueMarkId, RuntimeDialogueValueSlotId,
    },
    step::RuntimeHostCallMode,
    time::LogicalDuration,
    value::{
        RuntimeHandleKind, RuntimeInt, RuntimeIntrinsic, RuntimeNominalRecordLayout,
        RuntimeNominalRecordLayoutError, RuntimeOpaquePersistence, RuntimeOpaqueValueClass,
        RuntimeRecordFieldId, RuntimeSignedIntWidth, RuntimeUInt, RuntimeUnsignedIntWidth,
        RuntimeValue, runtime_sequence_from_literal_values,
    },
};
use arcweft_dialogue::{
    DialoguePresentationProfile, DialogueProfileRevision, InlineFailureSelection,
    character_presentation::{
        CharacterPresentationTargetEvidence, CheckedCharacterPresentationPlan,
    },
};
use arcweft_id::closed_enum::ClosedEnumDomainId;
use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, ItemId, LocalId, PatternId, StmtId},
    item::{HirDeclarationMemberKind, HirItemKind, HirParameterKind, HirRetainedName},
    leaf::{
        HirBigUint, HirCharacterLiteral, HirDecimal, HirDurationLiteral, HirFloatLiteral,
        HirIntegerLiteral, HirLiteral, HirStringLiteral, HirUnitNumberLiteral,
    },
    project::{
        HirAnalysisProjectView, HirProjectItemRef, HirRuntimeExecutableOwner,
        HirRuntimeReachabilityError, HirRuntimeSemanticReachability,
        HirSelectedExpressionInventoryError,
    },
    scope::HirScopeOwner,
    symbol::{
        CallableDeclarationKey, ImplMethodDeclarationId, ProjectSymbolTable,
        nominal::ProjectNominalDeclarationId,
    },
};
use arcweft_lang_sema::semantic_coordinate::{
    StableCheckedContentFragmentCoordinate, StableCheckedDialogueMarkCoordinate,
};
use arcweft_lang_sema::{
    assertion::AssertionRuntimePolicy,
    callable::{
        AgentIntrinsicSignatureId, BuiltinCallableId, CallTargetFacts, CallableCandidateId,
        CallableFamily, CallableLogLevel, CallableParameterPresence, CallableValidator,
        CheckedCallApplication, CheckedCallArgumentPassing, CheckedCallCalleeExecution,
        CheckedCallOperandDestination, CheckedCallReceiverProjection, CheckedCallRuntimeOperand,
        CheckedCallableExecution, CheckedProjectFunctionInstanceSolution,
        CheckedProjectFunctionRuntimeInput, CheckedProjectFunctionRuntimeOutcome,
        CheckedProjectFunctionRuntimeSelection, DomainMethodId, LineContextMethodId,
        LineScheduleCallableId, MathCallableId, ProbeComparisonOperator, ReductionConstructorKind,
        ResolvedCallableOrigin, ResolvedCallableState, StageMethodId, StandardMapFamily,
        StdFloatOperation, select_project_function_root_runtime, select_project_function_runtime,
    },
    checked_rich_text::{
        CheckedContentEmission, CheckedContentModifier, CheckedContentParameter,
        CheckedDialogueControl, CheckedDialogueHostEvent, CheckedDialogueToken,
        CheckedRichTextAction, CheckedRichTextReport, CheckedVoiceSource,
    },
    effects::EffectId,
    entry::{CheckedCallableRole, CheckedEntryBinding},
    env::nominal::AcceptedNominalId,
    env::nominal::AcceptedNominalSemantics,
    final_analysis::{
        CheckedAssertionDisposition, CheckedAssignment, CheckedCharacterDialogueTarget,
        CheckedCompileTimeScalar, CheckedCompileTimeValue, CheckedContentApplication,
        CheckedDialogueEffectSite, CheckedDialogueEffectTrigger, CheckedDropFade,
        CheckedDropInvocation, CheckedEffectField, CheckedEvaluatedEffect,
        CheckedEvaluatedEffectOperand, CheckedEvaluatedEffectOperation,
        CheckedExecutableRuntimeExpressionFactFamily, CheckedExecutableRuntimePatternFactFamily,
        CheckedExecutableRuntimeStatementFactFamily, CheckedExplicitDropPolicy,
        CheckedExpressionEdgeError, CheckedExpressionResolution, CheckedItemRole, CheckedIteration,
        CheckedIteratorFamily, CheckedOrdinaryFunctionEmission, CheckedPatternResolution,
        CheckedProjectItemOwner, CheckedProjectNominal, CheckedRecordPattern,
        CheckedRecordPatternOwner, CheckedRecordPatternRest, CheckedRecordPatternSourceRef,
        CheckedRecordValueSource, CheckedSelectResolution, CheckedStatementPayload,
        CheckedTraitConformance, CheckedTraitIdentity, CheckedTriggerView, CheckedTryCarrier,
        CheckedValueResolution, CheckedVariantOwner, CheckedVariantOwnerKind,
        CheckedVariantResolution, FinalAnalysisImplicitCallableBody, FinalAnalysisTryView,
        FinalSemanticAnalysis, FinalSemanticAnalysisError, NominalSchemaPath,
        NominalSchemaProjectionError,
    },
    registration::RegisteredSemanticWorld,
    types::{
        AgentBuiltinType, ArrayLength, CheckedConstraintContainerConstructor,
        CheckedConstraintSourceProjection, IteratorStateKind, MapKind, SemanticTypeDigest,
        TypeKind, VariantPayloadTypeShape,
    },
};
use arcweft_manifest_model::CharacterNameLocalePolicySpec;
use arcweft_presentation::fx::{FxDefinition, FxTarget};
use arcweft_presentation::rich_text::{
    Jlreq, LayoutDirection, PresentationContentCallableDefinitionId,
    PresentationContentCallableParameterId, RichTextLayoutProperty, RichTextLayoutSelector,
    RichTextStyleProperty, RichTextStyleSelector, RichTextTransformProperty,
    RichTextTransformSelector, TransformOrigin, TransformTarget, VerticalLatin,
};
use arcweft_runtime_plan::{
    agent::RuntimeAgentIntrinsic,
    assertion_identity::RuntimeAssertionMode,
    semantic_facts::{
        RuntimeAcceptedDeclarationSemanticId, RuntimeAgentTypeShape, RuntimeAssertionAdmission,
        RuntimeAssignmentFact, RuntimeAwaitFact, RuntimeAwaitPendingObserverFact,
        RuntimeBuiltinIteratorFact, RuntimeCallParameterCoordinate, RuntimeCallResultShape,
        RuntimeCallableAttachedContentAbi, RuntimeCallableAttachedContentDefault,
        RuntimeCheckedCapture, RuntimeCheckedTypeProjectionError, RuntimeChoiceFact,
        RuntimeChoiceGotoFact, RuntimeClosureCaptureFact, RuntimeClosureInstanceFact,
        RuntimeClosureInstanceKey, RuntimeClosureParameterFact, RuntimeContentFragmentFact,
        RuntimeDialogueApplication, RuntimeDialogueEffectCaptureFact,
        RuntimeDialogueEffectProgramFact, RuntimeDialogueEffectTrigger, RuntimeDialogueMarkFact,
        RuntimeDialogueMarkKey, RuntimeDialogueValueExpression, RuntimeDropFadeFact,
        RuntimeDropPolicyFact, RuntimeEffectFieldFact, RuntimeEvaluatedEffect,
        RuntimeEvaluatedEffectFact, RuntimeEvaluatedEffectOperandFact, RuntimeImplicitCallableFact,
        RuntimeIteratorFact, RuntimeIteratorWitnessExecutableFact, RuntimeIteratorWitnessFact,
        RuntimeLineCallable, RuntimeLogLevel, RuntimeMapKind, RuntimeNominalRecordFactError,
        RuntimeNormalizedType, RuntimeNormalizedVariantCase, RuntimePipeFact,
        RuntimePlanSemanticFactInput, RuntimePlanSemanticFacts, RuntimePositionedAttachedContent,
        RuntimeProjectAttachedDefaultCapture, RuntimeProjectAttachedDefaultFunctionFact,
        RuntimeProjectCallable, RuntimeProjectContinuationAbi, RuntimeProjectFunctionBody,
        RuntimeProjectFunctionCallInput, RuntimeProjectFunctionCallOutcome,
        RuntimeProjectFunctionCallPlan, RuntimeProjectFunctionExecution,
        RuntimeProjectFunctionExpressionPayload, RuntimeProjectFunctionExpressionSemanticFact,
        RuntimeProjectFunctionInstanceFact, RuntimeProjectFunctionInstanceKey,
        RuntimeProjectFunctionInstanceSemanticFacts, RuntimeProjectFunctionParameterAbi,
        RuntimeProjectFunctionParameterMaterialization, RuntimeProjectFunctionParameterSource,
        RuntimeProjectFunctionPatternPayload, RuntimeProjectFunctionPatternSemanticFact,
        RuntimeProjectFunctionRootFact, RuntimeProjectFunctionRootRole,
        RuntimeProjectFunctionStatementPayload, RuntimeProjectFunctionStatementSemanticFact,
        RuntimeProjectFunctionTypeOwner, RuntimeProjectFunctionTypeProjection, RuntimeProjectItem,
        RuntimePureProgramCaptureFact, RuntimePureProgramFact, RuntimeRecordExpressionFact,
        RuntimeRecordExpressionField, RuntimeRecordExpressionSource, RuntimeRecordPatternFact,
        RuntimeRecordPatternField, RuntimeRecordPatternRest, RuntimeRecordPatternSource,
        RuntimeRecordPlanError, RuntimeRecordTypeField, RuntimeReductionConstructor,
        RuntimeRegisteredValueId, RuntimeResolvedAttachedContent, RuntimeResolvedCall,
        RuntimeResolvedCallDispatch, RuntimeResolvedCallOperand, RuntimeResolvedCallOperandBinding,
        RuntimeResolvedCallOperandOrigin, RuntimeResolvedCallOperandProjection,
        RuntimeResolvedCallOperandSource, RuntimeResolvedHostCall, RuntimeResolvedNominal,
        RuntimeResolvedNominalRecord, RuntimeResolvedSelect, RuntimeResolvedSpreadContainer,
        RuntimeResolvedStaticCallTarget, RuntimeResolvedValue, RuntimeResolvedVariant,
        RuntimeSemanticFactsError, RuntimeSemanticTypeId, RuntimeSequenceKind,
        RuntimeStandardMapCall, RuntimeStandardMapFamily, RuntimeStandardMapOperandOrder,
        RuntimeTraitIdentity, RuntimeTraitMethodFact, RuntimeTriggerAdmission,
        RuntimeTryBoundaryOwner, RuntimeTryCarrierFact, RuntimeTryFact, RuntimeTypeProjectionPath,
        RuntimeTypeProjectionStep, RuntimeTypeShape,
    },
};
use arcweft_source::ProductSourceRef;
use arcweft_text_model::{
    DialogueContentFragmentTemplate, DialogueContentSpec, DialogueContentTemplateEffect,
    DialogueContentTemplateMark, DialogueContentTemplateSlot, DialogueHostEvent,
    DialoguePresentationSnapshot, DialogueVoiceSource, Milli, RichTextAngle, RichTextColor,
    RichTextControl, RichTextDocument, RichTextFontFamily, RichTextInlineDirection,
    RichTextJlreqStrictness, RichTextLayout, RichTextNode, RichTextPresentationStyle,
    RichTextRubyPosition, RichTextStyle, RichTextTransform, RichTextTransformOrigin, RichTextVec2,
    RichTextVerticalLatinMode, RichTextWritingMode,
};
use thiserror::Error;

/// Exact final-HIR owner rejected before any runtime record fact is published.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeRecordExecutableOwner {
    Expression(ExprId),
    Pattern(PatternId),
}

/// Failure to project one accepted semantic generation into the closed runtime
/// fact vocabulary.
#[derive(Debug, Error)]
pub enum RuntimeSemanticProjectionError {
    #[error(transparent)]
    TypeInstantiation(#[from] arcweft_lang_sema::types::TypeInstantiationError),
    #[error(transparent)]
    GenericScope(#[from] arcweft_lang_sema::types::GenericScopeError),
    #[error(transparent)]
    VariantOwner(#[from] arcweft_lang_sema::final_analysis::CheckedVariantOwnerError),
    #[error(transparent)]
    Generation(Box<FinalSemanticAnalysisError>),
    #[error(transparent)]
    Facts(Box<RuntimeSemanticFactsError>),
    #[error(transparent)]
    LocalDeclarations(#[from] RuntimeLocalDeclarationTableError),
    #[error(transparent)]
    ExpressionTypeInventory(#[from] HirSelectedExpressionInventoryError),
    #[error(transparent)]
    ExecutionProjection(
        #[from] arcweft_lang_sema::final_analysis::FinalAnalysisExecutionProjectionError,
    ),
    #[error(transparent)]
    RuntimeReachability(#[from] HirRuntimeReachabilityError),
    #[error(transparent)]
    ProjectInstantiation(#[from] ProjectInstantiationError),
    #[error("project-function instance projection at {origin:?} failed: {source}")]
    ProjectFunctionProjection {
        origin: ProjectInstantiationOrigin,
        #[source]
        source: Box<
            arcweft_lang_sema::callable::CheckedProjectFunctionInstanceProjectionError<
                ProjectInstantiationError,
            >,
        >,
    },
    #[error("final semantic analysis omits runtime-domain HIR local {local:?}")]
    MissingLocalSemanticFact { local: LocalId },
    #[error("final semantic owner {owner:?} belongs to no executable HIR module")]
    MissingModule { owner: ExprId },
    #[error("final semantic record owner {owner:?} has no atomic checked edge fact")]
    ExpressionEdges {
        owner: ExprId,
        #[source]
        source: CheckedExpressionEdgeError,
    },
    #[error("checked field selection for {owner:?} has no exact sealed runtime relation")]
    FieldProjection {
        owner: ExprId,
        #[source]
        source: NominalSchemaProjectionError,
    },
    #[error("checked assignment field for {owner:?} has no exact sealed runtime relation")]
    AssignmentFieldProjection {
        owner: StmtId,
        #[source]
        source: NominalSchemaProjectionError,
    },
    #[error("project nominal {declaration:?} is absent from the accepted symbol table")]
    MissingNominal {
        declaration: Box<ProjectNominalDeclarationId>,
    },
    #[error("checked nominal schema projection failed for `{nominal}`")]
    NominalSchemaProjection {
        nominal: String,
        #[source]
        source: NominalSchemaProjectionError,
    },
    #[error("project nominal contains an opaque leaf without a schema-derived layout")]
    OpaqueProjectNominalLayout {
        nominal: Box<ProjectNominalDeclarationId>,
        path: Box<NominalSchemaPath>,
        accepted_nominal: Box<AcceptedNominalId>,
        semantic_identity: SemanticTypeDigest,
    },
    #[error("runtime nominal-record layout projection failed for `{nominal}`")]
    NominalRecordLayout {
        nominal: String,
        #[source]
        source: RuntimeNominalRecordLayoutError,
    },
    #[error("runtime nominal-record fact projection failed for `{nominal}`")]
    NominalRecordFact {
        nominal: String,
        #[source]
        source: RuntimeNominalRecordFactError,
    },
    #[error(
        "environment record field {ordinal} of {semantic_owner:?} on {owner:?} has no executable runtime coordinate"
    )]
    UnrepresentableEnvironmentRecordField {
        owner: RuntimeRecordExecutableOwner,
        semantic_owner: SemanticTypeDigest,
        ordinal: u32,
    },
    #[error(
        "environment record {semantic_owner:?} on {owner:?} has no executable runtime nominal owner"
    )]
    UnrepresentableEnvironmentRecord {
        owner: RuntimeRecordExecutableOwner,
        semantic_owner: SemanticTypeDigest,
    },
    #[error("checked runtime record plan for {owner:?} is invalid")]
    RecordPlan {
        owner: RuntimeRecordExecutableOwner,
        #[source]
        source: RuntimeRecordPlanError,
    },
    #[error("flow item {owner:?} has no executable absolute or named identity")]
    InvalidFlowIdentity { owner: ItemId },
    #[error("flow item {owner:?} has an invalid closed effect set: {source}")]
    InvalidFlowEffects {
        owner: ItemId,
        source: arcweft_core::plan::RuntimeEffectSetError,
    },
    #[error("expression literal {owner:?} has no exact runtime value: {reason}")]
    ExpressionLiteral { owner: ExprId, reason: String },
    #[error("pattern literal {owner:?} has no exact runtime value: {reason}")]
    PatternLiteral { owner: PatternId, reason: String },
    #[error("semantic type cannot enter runtime lowering: {reason}")]
    Type { reason: String },
    #[error(transparent)]
    CheckedTypeProjection(#[from] RuntimeCheckedTypeProjectionError),
    #[error("value expression {owner:?} has no exact runtime projection: {reason}")]
    Value { owner: ExprId, reason: String },
    #[error("call {owner:?} is not an accepted executable call: {reason}")]
    Call { owner: ExprId, reason: String },
    #[error("project-function instance rooted at item {owner:?} is invalid: {reason}")]
    ProjectFunctionInstance { owner: ItemId, reason: String },
    #[error("iteration statement {owner:?} references an unbound runtime trait method")]
    MissingIterationMethod { owner: StmtId },
    #[error("one checked iterator conformance was assigned conflicting self types")]
    InconsistentIterationConformance,
    #[error("assertion statement {owner:?} has an invalid runtime disposition")]
    InvalidAssertionDisposition { owner: StmtId },
    #[error("evaluated-effect statement {owner:?} has no matching registered runtime call")]
    InvalidEvaluatedEffectDisposition { owner: StmtId },
    #[error("Try expression {owner:?} has no exact checked propagation boundary")]
    InvalidTryBoundary { owner: ExprId },
    #[error("dialogue projection failed for {owner:?}: {reason}")]
    Dialogue {
        owner: Option<ExprId>,
        reason: String,
    },
}

impl RuntimeSemanticProjectionError {
    /// Stable compiler diagnostic identity for this projection failure.
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::ProjectInstantiation(ProjectInstantiationError::Cancelled { .. }) => {
                "compiler.project_instantiation.cancelled"
            }
            Self::ProjectInstantiation(
                ProjectInstantiationError::LimitExceeded { .. }
                | ProjectInstantiationError::ArithmeticOverflow { .. },
            ) => "compiler.project_instantiation.limit",
            Self::OpaqueProjectNominalLayout { .. } => {
                "compiler.runtime_nominal.opaque_leaf_has_no_schema_layout"
            }
            Self::UnrepresentableEnvironmentRecordField { .. } => {
                "compiler.runtime_record.environment_field_unrepresentable"
            }
            _ => "compiler.runtime_semantic_projection",
        }
    }
}

impl From<FinalSemanticAnalysisError> for RuntimeSemanticProjectionError {
    fn from(error: FinalSemanticAnalysisError) -> Self {
        Self::Generation(Box::new(error))
    }
}

impl From<RuntimeSemanticFactsError> for RuntimeSemanticProjectionError {
    fn from(error: RuntimeSemanticFactsError) -> Self {
        Self::Facts(Box::new(error))
    }
}

/// Projects the sole accepted semantic generation into runtime-plan facts.
///
/// The resulting fact set is validated against the same executable project
/// lease before it is returned. No partially projected fact set is observable.
#[expect(
    clippy::too_many_lines,
    reason = "the final semantic fact admission matrix is intentionally exhaustive and atomic"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "one projection transaction receives the same accepted project, semantic world, reachability, presentation inputs and discovery control"
)]
pub fn project_runtime_semantic_facts(
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    dialogue_profile: Option<(&DialoguePresentationProfile, &DialogueProfileRevision)>,
    character_name_policy: Option<&CharacterNameLocalePolicySpec>,
    instantiation_control: &ProjectInstantiationControl,
) -> Result<(RuntimePlanSemanticFacts, Arc<[FxDefinition]>), RuntimeSemanticProjectionError> {
    let fx_catalog = crate::fx_catalog::CompiledFxCatalog::lower(analysis).map_err(|error| {
        RuntimeSemanticProjectionError::Type {
            reason: error.to_string(),
        }
    })?;
    let facts = project_runtime_semantic_fact_inventories(
        project,
        symbols,
        world,
        analysis,
        runtime_owners,
        None,
        &[],
        dialogue_profile,
        character_name_policy,
        &fx_catalog,
        instantiation_control,
    )?;
    Ok((
        facts,
        Arc::from(fx_catalog.definitions().definitions().to_vec()),
    ))
}

/// Projects runtime facts and the presentation-owned Fx definitions emitted
/// by accepted RichText effects in the same lowering transaction.
pub(crate) fn project_runtime_semantic_facts_with_view_value_programs_and_fx(
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    view_value_owners: &HirRuntimeSemanticReachability<'_>,
    pure_programs: &[crate::view::CheckedViewHandlerProgram],
    dialogue_profile: Option<(&DialoguePresentationProfile, &DialogueProfileRevision)>,
    character_name_policy: Option<&CharacterNameLocalePolicySpec>,
    fx_catalog: &crate::fx_catalog::CompiledFxCatalog,
    instantiation_control: &ProjectInstantiationControl,
) -> Result<RuntimePlanSemanticFacts, RuntimeSemanticProjectionError> {
    project_runtime_semantic_fact_inventories(
        project,
        symbols,
        world,
        analysis,
        runtime_owners,
        Some(view_value_owners),
        pure_programs,
        dialogue_profile,
        character_name_policy,
        fx_catalog,
        instantiation_control,
    )
}

fn project_runtime_semantic_fact_inventories(
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    view_value_owners: Option<&HirRuntimeSemanticReachability<'_>>,
    pure_programs: &[crate::view::CheckedViewHandlerProgram],
    dialogue_profile: Option<(&DialoguePresentationProfile, &DialogueProfileRevision)>,
    character_name_policy: Option<&CharacterNameLocalePolicySpec>,
    fx_catalog: &crate::fx_catalog::CompiledFxCatalog,
    instantiation_control: &ProjectInstantiationControl,
) -> Result<RuntimePlanSemanticFacts, RuntimeSemanticProjectionError> {
    analysis.validate_generation(project, symbols)?;
    validate_executable_record_projections(world, analysis, runtime_owners)?;
    if let Some(view_value_owners) = view_value_owners {
        validate_executable_record_projections(world, analysis, view_value_owners)?;
    }
    let execution_projection = analysis.execution_projection();
    let instance_owned_call_owners =
        ordinary_function_runtime_call_owners(project, runtime_owners, &execution_projection)?;
    let mut instance_discovery = ProjectInstantiationSession::new(instantiation_control.clone());
    let mut instance_projection = ProjectInstanceProjection::Discover(&mut instance_discovery);
    let project_function_roots = runtime_project_function_roots(
        symbols,
        world,
        analysis,
        runtime_owners,
        &mut instance_projection,
    )?;
    let mut runtime_calls = BTreeMap::new();
    for (owner, call) in analysis.calls() {
        if (!runtime_owners.contains_expression(owner)
            && !view_value_owners.is_some_and(|owners| owners.contains_expression(owner)))
            || instance_owned_call_owners.contains(&owner)
        {
            continue;
        }
        let expression = analysis.expression(owner).ok_or_else(|| {
            RuntimeSemanticProjectionError::ExecutionProjection(
                arcweft_lang_sema::final_analysis::FinalAnalysisExecutionProjectionError::MissingExpression {
                    owner,
                },
            )
        })?;
        if !expression.execution_plan().executes_as_runtime_call() {
            continue;
        }
        let projected = runtime_call(
            owner,
            call,
            project,
            symbols,
            world,
            analysis,
            None,
            &mut instance_projection,
        )?;
        if runtime_calls.insert(owner, projected).is_some() {
            return Err(RuntimeSemanticProjectionError::Call {
                owner,
                reason: "runtime call projection repeats one expression owner".to_owned(),
            });
        }
    }
    discover_runtime_project_function_instances(
        symbols,
        world,
        analysis,
        runtime_owners,
        &mut instance_discovery,
    )?;
    let discovered_instances = instance_discovery.seal()?;
    let dialogue_projection = project_runtime_dialogue_projection_catalog(
        project,
        symbols,
        world,
        analysis,
        dialogue_profile,
        character_name_policy,
        runtime_owners,
        &discovered_instances,
        fx_catalog,
    )?;
    let project_function_instances = materialize_runtime_project_function_instances(
        project,
        symbols,
        world,
        analysis,
        runtime_owners,
        &dialogue_projection,
        &discovered_instances,
    )?;
    let root_closures = closure_instances::materialize_root_closures(
        project,
        symbols,
        world,
        analysis,
        runtime_owners,
        &dialogue_projection,
        &discovered_instances,
    )?;
    let mut closed_instance_type_owners = BTreeSet::new();
    let mut closed_instance_capture_owners = BTreeSet::new();
    let mut closed_instance_statement_owners = BTreeSet::new();
    for instance in &project_function_instances {
        instance.visit_type_projections(&mut |projection| {
            closed_instance_type_owners.insert(projection.owner());
        });
        instance.visit_captures(&mut |capture| {
            closed_instance_capture_owners.insert(capture.capture());
        });
        instance.visit_statement_owners(&mut |statement| {
            closed_instance_statement_owners.insert(statement);
        });
    }
    for closure in &root_closures {
        closure
            .semantics()
            .visit_type_projections(&mut |projection| {
                closed_instance_type_owners.insert(projection.owner());
            });
        closure.semantics().visit_captures(&mut |capture| {
            closed_instance_capture_owners.insert(capture.capture());
        });
        closure
            .semantics()
            .visit_statement_owners(&mut |statement| {
                closed_instance_statement_owners.insert(statement);
            });
    }
    runtime_calls.retain(|owner, _| {
        !closed_instance_type_owners.contains(&RuntimeProjectFunctionTypeOwner::Expression(*owner))
    });
    let mut runtime_expression_type_owners = runtime_owners.selected_expression_type_owners()?;
    if let Some(view_value_owners) = view_value_owners {
        runtime_expression_type_owners.extend(view_value_owners.selected_expression_type_owners()?);
    }
    let mut input = RuntimePlanSemanticFactInput::new();

    let runtime_locals = runtime_owners
        .locals()
        .chain(
            view_value_owners
                .into_iter()
                .flat_map(|owners| owners.locals()),
        )
        .collect::<BTreeSet<_>>();
    for owner in runtime_locals {
        if closed_instance_type_owners.contains(&RuntimeProjectFunctionTypeOwner::Local(owner)) {
            continue;
        }
        let local = analysis
            .local(owner)
            .ok_or(RuntimeSemanticProjectionError::MissingLocalSemanticFact { local: owner })?;
        input.push_local_declaration(owner, runtime_type(local.ty(), symbols, world, analysis)?);
    }

    let iteration_methods = runtime_iteration_methods(analysis, runtime_owners, view_value_owners)?;
    let mut method_declarations = BTreeMap::new();
    for (conformance, self_type) in &iteration_methods {
        let declaration = conformance.declaration().clone();
        method_declarations.insert(conformance.clone(), declaration.clone());
        input.push_trait_method(RuntimeTraitMethodFact::new(
            declaration,
            conformance.implementation(),
            conformance.method(),
            runtime_trait_identity(conformance.trait_identity()),
            runtime_type(self_type, symbols, world, analysis)?,
        ));
    }

    for (owner, item) in analysis.items() {
        if matches!(item.role(), CheckedItemRole::Flow { .. })
            && runtime_owners.contains_runtime_owner(
                &arcweft_lang_hir::project::HirRuntimeExecutableOwner::Item(owner),
            )
        {
            let symbol = symbols
                .flow_symbol_for_item(owner)
                .ok_or(RuntimeSemanticProjectionError::InvalidFlowIdentity { owner })?;
            let CallableDeclarationKey::Flow(declaration) = symbol.declaration() else {
                return Err(RuntimeSemanticProjectionError::InvalidFlowIdentity { owner });
            };
            let identity = runtime_flow_identity(declaration)
                .map_err(|_| RuntimeSemanticProjectionError::InvalidFlowIdentity { owner })?;
            let effects = arcweft_core::plan::RuntimeEffectSet::try_from_effects(
                item.effects().iter().cloned(),
            )
            .map_err(
                |source| RuntimeSemanticProjectionError::InvalidFlowEffects { owner, source },
            )?;
            input.push_flow(
                owner,
                arcweft_runtime_plan::semantic_facts::RuntimeFlowFact::new(identity, effects),
            );
        }
    }

    for (owner, ty) in analysis.types() {
        if runtime_owners.contains_type(owner)
            || view_value_owners.is_some_and(|owners| owners.contains_type(owner))
        {
            if closed_instance_type_owners.contains(&RuntimeProjectFunctionTypeOwner::Type(owner)) {
                continue;
            }
            input.push_type(owner, runtime_type(ty, symbols, world, analysis)?);
        }
    }

    for (owner, expression) in analysis.expressions() {
        if !runtime_owners.contains_expression(owner)
            && !view_value_owners.is_some_and(|owners| owners.contains_expression(owner))
        {
            continue;
        }
        if closed_instance_type_owners.contains(&RuntimeProjectFunctionTypeOwner::Expression(owner))
        {
            continue;
        }
        // Interpretation is structural execution evidence. The selector can
        // forward its selected child's value without owning a runtime type.
        if let CheckedExpressionResolution::PostfixBracket(resolution) = expression.resolution() {
            input.push_postfix_candidate(owner, resolution.candidate());
        }
        if !runtime_expression_type_owners.contains(&owner) {
            continue;
        }
        input.push_expression_type(
            owner,
            runtime_type(
                checked_expression_type(expression, owner)?,
                symbols,
                world,
                analysis,
            )?,
        );
        match expression.resolution() {
            CheckedExpressionResolution::Structural => {
                let module = project
                    .modules()
                    .find_map(|(_, module)| {
                        (module.module_id() == owner.module()).then_some(module.as_ref())
                    })
                    .ok_or(RuntimeSemanticProjectionError::MissingModule { owner })?;
                let hir = module.resolve_expr(owner).map_err(|error| {
                    RuntimeSemanticProjectionError::ExpressionLiteral {
                        owner,
                        reason: error.to_string(),
                    }
                })?;
                if let HirExprKind::NumericBracketSequence(sequence) = hir.kind() {
                    let TypeKind::Vec(item) = checked_expression_type(expression, owner)? else {
                        return Err(RuntimeSemanticProjectionError::ExpressionLiteral {
                            owner,
                            reason: "compact numeric sequence did not retain its checked item type"
                                .to_owned(),
                        });
                    };
                    let values = sequence
                        .elements()
                        .iter()
                        .map(|element| runtime_integer_magnitude(element.magnitude(), item))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|reason| RuntimeSemanticProjectionError::ExpressionLiteral {
                            owner,
                            reason,
                        })?;
                    input.push_expression_literal(
                        owner,
                        runtime_sequence_from_literal_values(values),
                    );
                }
            }
            CheckedExpressionResolution::Literal(literal) => {
                input.push_expression_literal(
                    owner,
                    runtime_literal(literal, checked_expression_type(expression, owner)?).map_err(
                        |reason| RuntimeSemanticProjectionError::ExpressionLiteral {
                            owner,
                            reason,
                        },
                    )?,
                );
            }
            CheckedExpressionResolution::Value(value) => {
                let project_item_is_runtime_entity =
                    if matches!(value, CheckedValueResolution::ProjectItem(_)) {
                        let module = project
                            .modules()
                            .find_map(|(_, module)| {
                                (module.module_id() == owner.module()).then_some(module.as_ref())
                            })
                            .ok_or(RuntimeSemanticProjectionError::MissingModule { owner })?;
                        matches!(
                            module
                                .resolve_expr(owner)
                                .map_err(|error| RuntimeSemanticProjectionError::Value {
                                    owner,
                                    reason: error.to_string(),
                                })?
                                .kind(),
                            HirExprKind::EntityReference(_)
                        )
                    } else {
                        false
                    };
                if let Some(value) = runtime_value_resolution(
                    value,
                    checked_expression_type(expression, owner)?,
                    project_item_is_runtime_entity,
                )
                .map_err(|reason| RuntimeSemanticProjectionError::Value { owner, reason })?
                {
                    input.push_value(owner, value);
                }
            }
            CheckedExpressionResolution::Select(select) => {
                if let Some(select) = runtime_select(owner, select, world, analysis)? {
                    input.push_select(owner, select);
                }
            }
            CheckedExpressionResolution::Nominal(nominal) => {
                let fields = analysis
                    .checked_expression_edge_fact(owner)
                    .map_err(|source| RuntimeSemanticProjectionError::ExpressionEdges {
                        owner,
                        source,
                    })?
                    .record_fields()
                    .iter()
                    .map(|field| {
                        let source = match field.source() {
                            CheckedRecordValueSource::Expression(source) => {
                                RuntimeRecordExpressionSource::Expression(source.raw())
                            }
                            CheckedRecordValueSource::Binding(source) => {
                                RuntimeRecordExpressionSource::Binding(source.raw())
                            }
                        };
                        RuntimeRecordExpressionField::new(field.runtime_field(), source)
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                let record = RuntimeRecordExpressionFact::try_new(
                    runtime_nominal_record(nominal, symbols, world, analysis)?,
                    fields,
                )
                .map_err(|source| RuntimeSemanticProjectionError::RecordPlan {
                    owner: RuntimeRecordExecutableOwner::Expression(owner),
                    source,
                })?;
                input.push_nominal_record(owner, record);
            }
            CheckedExpressionResolution::Variant(variant) => {
                input.push_expression_variant(
                    owner,
                    runtime_variant(variant, symbols, world, analysis)?,
                );
            }
            CheckedExpressionResolution::PostfixBracket(_) => {}
            CheckedExpressionResolution::Await(awaited) => {
                input.push_await(
                    owner,
                    RuntimeAwaitFact::new(
                        awaited.operand(),
                        awaited
                            .observers()
                            .iter()
                            .map(|observer| {
                                RuntimeAwaitPendingObserverFact::new(observer.pattern())
                            })
                            .collect::<Vec<_>>(),
                    ),
                );
            }
            CheckedExpressionResolution::Choice(choice) => {
                input.push_choice(
                    owner,
                    RuntimeChoiceFact::new(
                        choice.public_id().cloned(),
                        choice.option_ids().to_vec(),
                        choice
                            .gotos()
                            .iter()
                            .map(|goto| {
                                runtime_project_item(goto.target())
                                    .map(|target| RuntimeChoiceGotoFact::new(goto.arm(), target))
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                );
            }
            CheckedExpressionResolution::Try(_) => {
                let tried = execution_projection.try_expression(owner)?;
                push_runtime_try_fact(&mut input, owner, tried, symbols, world, analysis)?;
            }
            CheckedExpressionResolution::ImplicitCallable(_) => {
                let callable_view = execution_projection.implicit_callable(owner)?;
                let placeholders = callable_view.placeholders().collect::<Box<[_]>>();
                let captures = callable_view.captures().collect::<Box<[_]>>();
                input.push_implicit_callable(
                    owner,
                    RuntimeImplicitCallableFact::new(
                        runtime_type(callable_view.parameter(), symbols, world, analysis)?,
                        runtime_type(callable_view.result(), symbols, world, analysis)?,
                        placeholders,
                        captures,
                    ),
                );
                match callable_view.body() {
                    FinalAnalysisImplicitCallableBody::Plain(_) => {}
                    FinalAnalysisImplicitCallableBody::Try(tried) => {
                        push_runtime_try_fact(&mut input, owner, tried, symbols, world, analysis)?;
                    }
                    FinalAnalysisImplicitCallableBody::Pipe(pipe) => {
                        input.push_pipe(
                            owner,
                            RuntimePipeFact::new(
                                pipe.left(),
                                pipe.right(),
                                pipe.placeholders().collect(),
                            ),
                        );
                    }
                }
            }
            CheckedExpressionResolution::Pipe(_) => {
                let pipe_view = execution_projection.pipe(owner)?;
                input.push_pipe(
                    owner,
                    RuntimePipeFact::new(
                        pipe_view.left(),
                        pipe_view.right(),
                        pipe_view.placeholders().collect(),
                    ),
                );
            }
            CheckedExpressionResolution::DialogueLineReference(target) => {
                let line =
                    RuntimeLineId::from_source_entity_body(target.as_str()).map_err(|error| {
                        RuntimeSemanticProjectionError::Value {
                            owner,
                            reason: error.to_string(),
                        }
                    })?;
                input.push_value(owner, RuntimeResolvedValue::DialogueLine(line));
            }
            CheckedExpressionResolution::StageLook(look) => {
                input.push_value(
                    owner,
                    RuntimeResolvedValue::CharacterLook {
                        character: look.character().clone(),
                        look: look.look_id().clone(),
                    },
                );
            }
            CheckedExpressionResolution::ImplicitParameter { .. }
            | CheckedExpressionResolution::PipeLeft(_)
            | CheckedExpressionResolution::DialogueLineCoordinate(_)
            | CheckedExpressionResolution::DialogueTextKeyCoordinate(_)
            | CheckedExpressionResolution::CharacterDialogueFactory(_)
            | CheckedExpressionResolution::CharacterDialogueReconfigure(_)
            | CheckedExpressionResolution::Call
            | CheckedExpressionResolution::ViewCall(_)
            | CheckedExpressionResolution::ViewFxApplication(_)
            | CheckedExpressionResolution::StyleValue(_)
            | CheckedExpressionResolution::CompileTimeCallee(_)
            | CheckedExpressionResolution::CompileTimeScalar(_)
            | CheckedExpressionResolution::TypeValue(_)
            | CheckedExpressionResolution::CompileTimeEnum(_)
            | CheckedExpressionResolution::DialogueApplication { .. }
            | CheckedExpressionResolution::ContentApplication(_)
            | CheckedExpressionResolution::Effect(_)
            | CheckedExpressionResolution::Closure(_) => {}
        }
    }

    for (owner, pattern) in analysis.patterns() {
        if !runtime_owners.contains_pattern(owner)
            && !view_value_owners.is_some_and(|owners| owners.contains_pattern(owner))
        {
            continue;
        }
        if closed_instance_type_owners.contains(&RuntimeProjectFunctionTypeOwner::Pattern(owner)) {
            continue;
        }
        input.push_pattern_type(owner, runtime_type(pattern.ty(), symbols, world, analysis)?);
        match pattern.resolution() {
            CheckedPatternResolution::Literal(literal) => {
                input.push_pattern_literal(
                    owner,
                    runtime_literal(literal, pattern.ty()).map_err(|reason| {
                        RuntimeSemanticProjectionError::PatternLiteral { owner, reason }
                    })?,
                );
            }
            CheckedPatternResolution::Record(record) => {
                input.push_pattern_nominal_record(
                    owner,
                    runtime_record_pattern(owner, record, symbols, world, analysis)?,
                );
            }
            CheckedPatternResolution::Variant(variant) => {
                input.push_pattern_variant(
                    owner,
                    runtime_variant(variant, symbols, world, analysis)?,
                );
            }
            CheckedPatternResolution::Entity(item) => {
                input.push_pattern_item(owner, runtime_project_item(item)?);
            }
            CheckedPatternResolution::Structural | CheckedPatternResolution::TypedBinding(_) => {}
        }
    }

    for (owner, statement) in analysis.statements() {
        if !runtime_owners.contains_statement(owner)
            && !view_value_owners.is_some_and(|owners| owners.contains_statement(owner))
        {
            continue;
        }
        if closed_instance_statement_owners.contains(&owner) {
            continue;
        }
        match statement.payload() {
            CheckedStatementPayload::Assignment(assignment) => {
                input.push_assignment(
                    owner,
                    runtime_assignment(owner, assignment, symbols, world, analysis)?,
                );
            }
            CheckedStatementPayload::Assertion(disposition) => {
                input.push_assertion(owner, runtime_assertion(owner, *disposition)?);
            }
            CheckedStatementPayload::EvaluatedEffect(effect) => {
                input.push_evaluated_effect(
                    owner,
                    runtime_evaluated_effect(effect, symbols, world, analysis)?,
                );
            }
            CheckedStatementPayload::Iteration(iteration) => {
                input.push_iteration(
                    owner,
                    runtime_iteration(
                        owner,
                        iteration,
                        &method_declarations,
                        symbols,
                        world,
                        analysis,
                    )?,
                );
            }
            CheckedStatementPayload::Structural
            | CheckedStatementPayload::Defer(_)
            | CheckedStatementPayload::ControlTransfer(_)
            | CheckedStatementPayload::Trigger(_)
            | CheckedStatementPayload::UnsafeAudit(_)
            | CheckedStatementPayload::Select(_)
            | CheckedStatementPayload::SourceLocale(_)
            | CheckedStatementPayload::Scope(_)
            | CheckedStatementPayload::Include(_)
            | CheckedStatementPayload::Suspension(_)
            | CheckedStatementPayload::Yield => {}
        }
    }

    for (owner, capture) in analysis.captures() {
        if !runtime_owners.contains_capture(owner)
            && !view_value_owners.is_some_and(|owners| owners.contains_capture(owner))
        {
            continue;
        }
        if closed_instance_capture_owners.contains(&owner) {
            continue;
        }
        input.push_capture(RuntimeCheckedCapture::new(
            *analysis.selected_capture(owner).ok_or_else(|| {
                RuntimeSemanticProjectionError::Facts(Box::new(
                    RuntimeSemanticFactsError::InvalidCaptureProjection { capture: owner },
                ))
            })?,
            runtime_type(capture.ty(), symbols, world, analysis)?,
        ));
    }

    for instance in project_function_instances {
        input.push_project_function_instance(instance);
    }
    for closure in root_closures {
        input.push_root_closure(closure);
    }
    for root in project_function_roots {
        input.push_project_function_root(root);
    }
    for (owner, call) in runtime_calls {
        input.push_call(owner, call);
    }

    for program in pure_programs {
        input.push_pure_program(RuntimePureProgramFact::new(
            program.id(),
            program.closure(),
            program.body(),
            program
                .captures()
                .iter()
                .copied()
                .map(|capture| {
                    RuntimePureProgramCaptureFact::new(
                        capture.capture(),
                        capture.local(),
                        capture.schema().parameter().value(),
                        capture.schema().value_type(),
                    )
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            program.result().value_type(),
        ));
    }

    attach_dialogue_and_trigger_semantic_facts(
        analysis,
        runtime_owners,
        view_value_owners,
        &closed_instance_statement_owners,
        &closed_instance_type_owners,
        &mut input,
        &dialogue_projection,
    )?;
    Ok(match view_value_owners {
        Some(view_value_owners) => RuntimePlanSemanticFacts::try_new_with_view_value_programs(
            project,
            runtime_owners,
            view_value_owners,
            input,
        )?,
        None => RuntimePlanSemanticFacts::try_new(project, runtime_owners, input)?,
    })
}

fn validate_executable_record_projections(
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
) -> Result<(), RuntimeSemanticProjectionError> {
    for (owner, expression) in analysis.expressions() {
        if !runtime_owners.contains_expression(owner) {
            continue;
        }
        let CheckedExpressionResolution::Select(select) = expression.resolution() else {
            continue;
        };
        let selection = match select {
            CheckedSelectResolution::DialogueView { field, .. }
            | CheckedSelectResolution::Field(field) => field,
            CheckedSelectResolution::Method(_)
            | CheckedSelectResolution::AgentField { .. }
            | CheckedSelectResolution::ProgressField { .. } => continue,
        };
        if selection
            .project_runtime_field(analysis)
            .map_err(|source| RuntimeSemanticProjectionError::FieldProjection { owner, source })?
            .is_some()
            || accepted_runtime_environment_field(world, selection).is_some()
        {
            continue;
        }
        return Err(
            RuntimeSemanticProjectionError::UnrepresentableEnvironmentRecordField {
                owner: RuntimeRecordExecutableOwner::Expression(owner),
                semantic_owner: selection.owner_type(),
                ordinal: selection.declaration_ordinal(),
            },
        );
    }
    for (owner, pattern) in analysis.patterns() {
        if !runtime_owners.contains_pattern(owner) {
            continue;
        }
        let CheckedPatternResolution::Record(record) = pattern.resolution() else {
            continue;
        };
        if record.owner().project_nominal().is_some() {
            continue;
        }
        return Err(record.fields().first().map_or(
            RuntimeSemanticProjectionError::UnrepresentableEnvironmentRecord {
                owner: RuntimeRecordExecutableOwner::Pattern(owner),
                semantic_owner: record.owner().semantic_type(),
            },
            |field| RuntimeSemanticProjectionError::UnrepresentableEnvironmentRecordField {
                owner: RuntimeRecordExecutableOwner::Pattern(owner),
                semantic_owner: record.owner().semantic_type(),
                ordinal: field.declaration_ordinal(),
            },
        ));
    }
    Ok(())
}

fn push_runtime_try_fact(
    input: &mut RuntimePlanSemanticFactInput,
    owner: ExprId,
    tried: FinalAnalysisTryView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), RuntimeSemanticProjectionError> {
    input.push_try(
        owner,
        runtime_try_fact(owner, tried, symbols, world, analysis, None)?,
    );
    Ok(())
}

fn runtime_try_fact(
    _owner: ExprId,
    tried: FinalAnalysisTryView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instance: Option<ProjectInstanceTypes<'_>>,
) -> Result<RuntimeTryFact, RuntimeSemanticProjectionError> {
    let operand = tried.operand();
    let boundary = match tried.boundary().owner() {
        arcweft_lang_sema::final_analysis::CheckedTryBoundaryOwner::Infallible => {
            RuntimeTryBoundaryOwner::Infallible
        }
        arcweft_lang_sema::final_analysis::CheckedTryBoundaryOwner::CarrierBlock(boundary) => {
            RuntimeTryBoundaryOwner::CarrierBlock(boundary.lookup_owner())
        }
        arcweft_lang_sema::final_analysis::CheckedTryBoundaryOwner::FunctionSite(site) => {
            match site {
                arcweft_lang_sema::final_analysis::CheckedTryFunctionSite::Explicit(boundary) => {
                    RuntimeTryBoundaryOwner::ExplicitFunctionSite(boundary.lookup_owner())
                }
                arcweft_lang_sema::final_analysis::CheckedTryFunctionSite::Implicit {
                    site,
                    ..
                } => RuntimeTryBoundaryOwner::ImplicitFunctionSite(site.lookup_owner()),
            }
        }
        arcweft_lang_sema::final_analysis::CheckedTryBoundaryOwner::Callable(boundary) => {
            RuntimeTryBoundaryOwner::Callable(RuntimeAcceptedDeclarationSemanticId::from_bytes(
                *boundary.accepted().as_bytes(),
            ))
        }
    };
    let close =
        |ty: &TypeKind| instance.map_or_else(|| Ok(ty.clone()), |row| row.instantiate_type(ty));
    let carrier = match tried.carrier() {
        CheckedTryCarrier::Result { success, residual } => RuntimeTryCarrierFact::Result {
            success: runtime_type(&close(success)?, symbols, world, analysis)?,
            residual: Box::new(runtime_type(&close(residual)?, symbols, world, analysis)?),
        },
        CheckedTryCarrier::Option { success } => RuntimeTryCarrierFact::Option {
            success: runtime_type(&close(success)?, symbols, world, analysis)?,
        },
    };
    Ok(RuntimeTryFact::new(
        operand,
        runtime_type(&close(tried.operand_type())?, symbols, world, analysis)?,
        carrier,
        boundary,
        runtime_type(
            &close(tried.boundary().boundary_type())?,
            symbols,
            world,
            analysis,
        )?,
    ))
}

fn runtime_assignment(
    owner: StmtId,
    assignment: &CheckedAssignment,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeAssignmentFact, RuntimeSemanticProjectionError> {
    runtime_assignment_under(owner, assignment, symbols, world, analysis, None)
}

fn runtime_assignment_under(
    owner: StmtId,
    assignment: &CheckedAssignment,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instance: Option<ProjectInstanceTypes<'_>>,
) -> Result<RuntimeAssignmentFact, RuntimeSemanticProjectionError> {
    let place = assignment.place();
    let field = place
        .field()
        .project_runtime_field(analysis)
        .map_err(
            |source| RuntimeSemanticProjectionError::AssignmentFieldProjection { owner, source },
        )?
        .ok_or_else(
            || RuntimeSemanticProjectionError::AssignmentFieldProjection {
                owner,
                source: NominalSchemaProjectionError::InvalidProjectFieldRelation {
                    owner: place.field().owner_type(),
                    ordinal: place.field().declaration_ordinal(),
                },
            },
        )?;
    Ok(RuntimeAssignmentFact::new(
        place.local(),
        runtime_nominal_under(place.nominal(), analysis, instance)?,
        field.field().runtime_field(),
        runtime_type_under(place.field_type(), instance, symbols, world, analysis)?,
        runtime_type_under(assignment.value_type(), instance, symbols, world, analysis)?,
    ))
}

/// Compiler-local occurrence scope for a closed dialogue/content projection.
/// The same stable source fragment may be instantiated more than once, while
/// its dense runtime template identity must remain unique within one plan.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum RuntimeDialogueProjectionScope {
    Global,
    ProjectInstance(RuntimeProjectFunctionInstanceKey),
}

/// One lexical type/effect environment for a complete executable partition.
/// A project instance's identity and solution travel together; global source
/// owns already-closed semantic types rather than a fabricated instance.
#[derive(Clone, Copy)]
enum RuntimeExecutableInstantiation<'a> {
    Global,
    Project {
        key: &'a RuntimeProjectFunctionInstanceKey,
        solution: ProjectInstanceTypes<'a>,
    },
}

impl<'a> RuntimeExecutableInstantiation<'a> {
    const fn types(self) -> Option<ProjectInstanceTypes<'a>> {
        match self {
            Self::Global => None,
            Self::Project { solution, .. } => Some(solution),
        }
    }

    const fn project_key(self) -> Option<&'a RuntimeProjectFunctionInstanceKey> {
        match self {
            Self::Global => None,
            Self::Project { key, .. } => Some(key),
        }
    }

    fn instantiate_type(
        self,
        ty: &TypeKind,
    ) -> Result<TypeKind, arcweft_lang_sema::types::TypeProjectionError<ProjectInstantiationError>>
    {
        match self {
            Self::Global => Ok(ty.clone()),
            Self::Project { solution, .. } => solution.instantiate_type(ty),
        }
    }

    fn instantiate_effect_row(
        self,
        row: &arcweft_lang_sema::effect_row::EffectRow,
    ) -> Result<
        arcweft_lang_sema::effects::EffectSet,
        arcweft_lang_sema::types::TypeProjectionError<ProjectInstantiationError>,
    > {
        match self {
            Self::Global => Ok(row
                .resolve(&arcweft_lang_sema::effect_row::EffectSubstitution::default())
                .map_err(arcweft_lang_sema::types::TypeInstantiationError::from)?),
            Self::Project { solution, .. } => solution.instantiate_effect_row(row),
        }
    }

    fn dialogue_scope(self) -> RuntimeDialogueProjectionScope {
        self.project_key()
            .map_or(RuntimeDialogueProjectionScope::Global, |key| {
                RuntimeDialogueProjectionScope::ProjectInstance(key.clone())
            })
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RuntimeDialogueTemplateOccurrenceKey {
    scope: RuntimeDialogueProjectionScope,
    fragment: StableCheckedContentFragmentCoordinate,
}

struct RuntimeDialogueTemplateIdCatalog {
    ids: BTreeMap<RuntimeDialogueTemplateOccurrenceKey, RuntimeDialogueContentTemplateId>,
}

struct RuntimeDialogueProjectionCatalog {
    character_catalog: Option<Arc<CharacterPresentationCatalogData>>,
    applications: BTreeMap<(RuntimeDialogueProjectionScope, ExprId), RuntimeDialogueApplication>,
    fragments: BTreeMap<(RuntimeDialogueProjectionScope, ExprId), RuntimeContentFragmentFact>,
}

impl RuntimeDialogueProjectionCatalog {
    fn application(
        &self,
        scope: &RuntimeDialogueProjectionScope,
        owner: ExprId,
    ) -> Option<&RuntimeDialogueApplication> {
        self.applications.get(&(scope.clone(), owner))
    }

    fn fragment(
        &self,
        scope: &RuntimeDialogueProjectionScope,
        owner: ExprId,
    ) -> Option<&RuntimeContentFragmentFact> {
        self.fragments.get(&(scope.clone(), owner))
    }

    fn mark(
        &self,
        scope: &RuntimeDialogueProjectionScope,
        coordinate: &StableCheckedDialogueMarkCoordinate,
    ) -> Option<&RuntimeDialogueMarkFact> {
        self.fragments
            .iter()
            .filter(|((candidate, _), _)| candidate == scope)
            .flat_map(|(_, fragment)| fragment.marks())
            .find(|mark| mark.coordinate() == coordinate)
    }

    fn global_applications(&self) -> BTreeMap<ExprId, RuntimeDialogueApplication> {
        self.applications
            .iter()
            .filter_map(|((scope, owner), application)| {
                matches!(scope, RuntimeDialogueProjectionScope::Global)
                    .then(|| (*owner, application.clone()))
            })
            .collect()
    }

    fn global_fragments(&self) -> Vec<RuntimeContentFragmentFact> {
        self.fragments
            .iter()
            .filter_map(|((scope, _), fragment)| {
                matches!(scope, RuntimeDialogueProjectionScope::Global).then(|| fragment.clone())
            })
            .collect()
    }
}

impl RuntimeDialogueTemplateIdCatalog {
    fn try_new<'report>(
        roots: impl IntoIterator<
            Item = (
                RuntimeDialogueProjectionScope,
                &'report CheckedRichTextReport,
            ),
        >,
    ) -> Result<Self, RuntimeSemanticProjectionError> {
        let mut keys = BTreeSet::new();
        for (scope, report) in roots {
            collect_runtime_dialogue_template_keys(&scope, report, true, &mut keys)?;
        }
        let ids = keys
            .into_iter()
            .enumerate()
            .map(|(index, key)| {
                RuntimeDialogueContentTemplateId::from_zero_based(index)
                    .map(|id| (key, id))
                    .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
                        owner: None,
                        reason: "dialogue content template identity exceeds u32".to_owned(),
                    })
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(Self { ids })
    }

    fn id(
        &self,
        scope: &RuntimeDialogueProjectionScope,
        report: &CheckedRichTextReport,
    ) -> Result<RuntimeDialogueContentTemplateId, RuntimeSemanticProjectionError> {
        self.ids
            .get(&RuntimeDialogueTemplateOccurrenceKey {
                scope: scope.clone(),
                fragment: report.fragment_coordinate().clone(),
            })
            .copied()
            .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
                owner: Some(report.content().id().owner()),
                reason: "checked content report has no preallocated runtime template identity"
                    .to_owned(),
            })
    }
}

fn collect_runtime_dialogue_template_keys(
    scope: &RuntimeDialogueProjectionScope,
    report: &CheckedRichTextReport,
    owns_template: bool,
    keys: &mut BTreeSet<RuntimeDialogueTemplateOccurrenceKey>,
) -> Result<(), RuntimeSemanticProjectionError> {
    if owns_template
        && !keys.insert(RuntimeDialogueTemplateOccurrenceKey {
            scope: scope.clone(),
            fragment: report.fragment_coordinate().clone(),
        })
    {
        return Err(RuntimeSemanticProjectionError::Dialogue {
            owner: Some(report.content().id().owner()),
            reason: "checked content report occurs more than once in one runtime instance"
                .to_owned(),
        });
    }
    for token in report.content().tokens() {
        let CheckedDialogueToken::ContentInsert(insertion) = token else {
            continue;
        };
        let Some(body) = insertion.argument().checked_content() else {
            continue;
        };
        collect_runtime_dialogue_template_keys(
            scope,
            body,
            matches!(insertion.emission(), CheckedContentEmission::ContentResult),
            keys,
        )?;
    }
    Ok(())
}

struct RuntimeDialogueApplicationProjection<'analysis> {
    scope: RuntimeDialogueProjectionScope,
    owner: ExprId,
    target: &'analysis CheckedCharacterDialogueTarget,
    report: &'analysis CheckedRichTextReport,
    line_result: &'analysis TypeKind,
    solution: Option<ProjectInstanceTypes<'analysis>>,
}

#[allow(
    clippy::too_many_arguments,
    reason = "one compiler-owned catalog projects global and closed-instance dialogue occurrences under one template allocator"
)]
fn project_runtime_dialogue_projection_catalog<'analysis>(
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &'analysis FinalSemanticAnalysis,
    dialogue_profile: Option<(&DialoguePresentationProfile, &DialogueProfileRevision)>,
    policy: Option<&CharacterNameLocalePolicySpec>,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    instances: &'analysis DiscoveredProjectInstances,
    fx_catalog: &crate::fx_catalog::CompiledFxCatalog,
) -> Result<RuntimeDialogueProjectionCatalog, RuntimeSemanticProjectionError> {
    let mut projections = Vec::new();
    let mut instance_expression_owners = BTreeSet::new();
    for (key, node) in instances.nodes() {
        instances.check_cancelled(node.origin)?;
        let executable = HirRuntimeExecutableOwner::Item(node.callable.owner());
        let mut expressions = BTreeSet::new();
        let mut visited = BTreeSet::new();
        collect_runtime_project_executable_expression_owners(
            &executable,
            analysis,
            runtime_owners,
            &mut visited,
            &mut expressions,
        )?;
        for owner in expressions {
            instance_expression_owners.insert(owner);
            let Some(checked) = analysis.expression(owner) else {
                continue;
            };
            let CheckedExpressionResolution::DialogueApplication {
                target,
                rich_text,
                line_result,
                ..
            } = checked.resolution()
            else {
                continue;
            };
            let module = project
                .modules()
                .find_map(|(_, module)| {
                    (module.module_id() == owner.module()).then_some(module.as_ref())
                })
                .ok_or(RuntimeSemanticProjectionError::MissingModule { owner })?;
            if expression_belongs_to_non_product_plan(module, owner)? {
                continue;
            }
            projections.push(RuntimeDialogueApplicationProjection {
                scope: RuntimeDialogueProjectionScope::ProjectInstance(key.clone()),
                owner,
                target,
                report: rich_text,
                line_result,
                solution: Some(instances.types(node)),
            });
        }
    }
    for (owner, target, report, line_result) in
        executable_dialogue_applications(project, analysis, runtime_owners)?
    {
        if instance_expression_owners.contains(&owner) {
            continue;
        }
        projections.push(RuntimeDialogueApplicationProjection {
            scope: RuntimeDialogueProjectionScope::Global,
            owner,
            target,
            report,
            line_result,
            solution: None,
        });
    }
    projections.sort_by(|left, right| (&left.scope, left.owner).cmp(&(&right.scope, right.owner)));
    if projections.is_empty() {
        return Ok(RuntimeDialogueProjectionCatalog {
            character_catalog: None,
            applications: BTreeMap::new(),
            fragments: BTreeMap::new(),
        });
    }

    let (dialogue_profile, dialogue_profile_revision) =
        dialogue_profile.ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason:
                "an executable dialogue product requires one compiler-admitted dialogue profile"
                    .to_owned(),
        })?;
    let policy = policy
        .map(character_name_locale_policy)
        .transpose()?
        .unwrap_or_else(CharacterNameLocalePolicy::engine_default);
    let character_catalog = Arc::new(build_character_presentation_catalog(
        project, analysis, policy,
    )?);
    let generation = CharacterPresentationCatalogGeneration::new(
        CharacterPresentationCatalogRevision::INITIAL,
        character_catalog.semantic_digest(),
        character_catalog.locale_policy_digest(),
    );
    let presentation = DialoguePresentationSnapshot::new(
        dialogue_profile.clone(),
        dialogue_profile_revision.clone(),
    );
    let template_ids = RuntimeDialogueTemplateIdCatalog::try_new(
        projections
            .iter()
            .map(|projection| (projection.scope.clone(), projection.report)),
    )?;
    let mut applications = BTreeMap::new();
    let mut fragments = BTreeMap::new();
    let mut mark_coordinates = BTreeSet::new();
    for projection in projections {
        let (_, application, projected_fragments) = project_dialogue_application(
            project,
            projection.owner,
            projection.target,
            projection.report,
            projection.line_result,
            symbols,
            world,
            analysis,
            projection.solution,
            generation,
            presentation.clone(),
            &projection.scope,
            &template_ids,
            fx_catalog,
        )?;
        if applications
            .insert((projection.scope.clone(), projection.owner), application)
            .is_some()
        {
            return Err(RuntimeSemanticProjectionError::Dialogue {
                owner: Some(projection.owner),
                reason: "checked dialogue application occurrence was projected more than once"
                    .to_owned(),
            });
        }
        for fragment in projected_fragments {
            let source = fragment.source();
            for mark in fragment.marks() {
                if !mark_coordinates.insert((projection.scope.clone(), mark.coordinate().clone())) {
                    return Err(RuntimeSemanticProjectionError::Dialogue {
                        owner: Some(source),
                        reason: "checked dialogue marker occurrence was projected more than once"
                            .to_owned(),
                    });
                }
            }
            if fragments
                .insert((projection.scope.clone(), source), fragment)
                .is_some()
            {
                return Err(RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(source),
                    reason: "checked content fragment occurrence was projected more than once"
                        .to_owned(),
                });
            }
        }
    }
    Ok(RuntimeDialogueProjectionCatalog {
        character_catalog: Some(character_catalog),
        applications,
        fragments,
    })
}

fn collect_runtime_project_executable_expression_owners(
    executable: &HirRuntimeExecutableOwner,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    visited: &mut BTreeSet<HirRuntimeExecutableOwner>,
    expressions: &mut BTreeSet<ExprId>,
) -> Result<(), RuntimeSemanticProjectionError> {
    if !visited.insert(executable.clone()) {
        return Ok(());
    }
    let partition = analysis
        .execution_projection()
        .runtime_fact_partition(runtime_owners, executable)?;
    for row in partition.expressions() {
        expressions.insert(row.owner());
        if row.family() == CheckedExecutableRuntimeExpressionFactFamily::Closure {
            collect_runtime_project_executable_expression_owners(
                &HirRuntimeExecutableOwner::Closure(row.owner()),
                analysis,
                runtime_owners,
                visited,
                expressions,
            )?;
        }
    }
    Ok(())
}

fn attach_dialogue_and_trigger_semantic_facts(
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    view_value_owners: Option<&HirRuntimeSemanticReachability<'_>>,
    instance_statement_owners: &BTreeSet<StmtId>,
    instance_type_owners: &BTreeSet<RuntimeProjectFunctionTypeOwner>,
    input: &mut RuntimePlanSemanticFactInput,
    catalog: &RuntimeDialogueProjectionCatalog,
) -> Result<(), RuntimeSemanticProjectionError> {
    let mut applications = catalog.global_applications();
    applications.retain(|owner, _| {
        !instance_type_owners.contains(&RuntimeProjectFunctionTypeOwner::Expression(*owner))
    });
    let fragments = catalog
        .global_fragments()
        .into_iter()
        .filter(|fragment| {
            !instance_type_owners.contains(&RuntimeProjectFunctionTypeOwner::Expression(
                fragment.source(),
            ))
        })
        .collect();
    input.attach_dialogue_projection(
        catalog.character_catalog.clone(),
        applications,
        fragments,
        Arc::new(analysis.dialogue_lines().clone()),
    )?;
    for (owner, statement) in analysis.statements() {
        if !runtime_owners.contains_statement(owner)
            && !view_value_owners.is_some_and(|owners| owners.contains_statement(owner))
        {
            continue;
        }
        if instance_statement_owners.contains(&owner) {
            continue;
        }
        let CheckedStatementPayload::Trigger(trigger) = statement.payload() else {
            continue;
        };
        match trigger.view() {
            CheckedTriggerView::Input => input.push_input_trigger(owner)?,
            CheckedTriggerView::Event => input.push_event_trigger(owner)?,
            CheckedTriggerView::Signal => input.push_signal_trigger(owner)?,
            CheckedTriggerView::Timeout => input.push_timeout_trigger(owner)?,
            CheckedTriggerView::Mark(coordinate) => {
                let mark = catalog
                    .mark(&RuntimeDialogueProjectionScope::Global, coordinate)
                    .ok_or_else(|| {
                    RuntimeSemanticProjectionError::Dialogue {
                        owner: None,
                        reason: format!(
                            "reachable mark Trigger {owner:?} has no owning checked content projection"
                        ),
                    }
                })?;
                input.push_mark_trigger(owner, mark.clone())?;
            }
            CheckedTriggerView::Select => input.push_select_trigger(owner)?,
            CheckedTriggerView::Task => input.push_task_trigger(owner)?,
            CheckedTriggerView::Scope => input.push_scope_trigger(owner)?,
            CheckedTriggerView::Expression => input.push_expression_trigger(owner)?,
        }
    }
    Ok(())
}

type CheckedDialogueApplication<'analysis> = (
    ExprId,
    &'analysis CheckedCharacterDialogueTarget,
    &'analysis CheckedRichTextReport,
    &'analysis TypeKind,
);

fn executable_dialogue_applications<'analysis>(
    project: HirAnalysisProjectView<'_>,
    analysis: &'analysis FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
) -> Result<Vec<CheckedDialogueApplication<'analysis>>, RuntimeSemanticProjectionError> {
    analysis
        .expressions()
        .filter_map(|(owner, expression)| match expression.resolution() {
            CheckedExpressionResolution::DialogueApplication {
                target,
                rich_text,
                line_result,
                ..
            } => {
                analysis.dialogue_lines().for_semantic_expr(owner)?;
                runtime_owners.contains_expression(owner).then_some((
                    owner,
                    target,
                    rich_text.as_ref(),
                    line_result,
                ))
            }
            _ => None,
        })
        .try_fold(Vec::new(), |mut applications, application| {
            let (owner, _, _, _) = application;
            let module = project
                .modules()
                .find_map(|(_, module)| {
                    (module.module_id() == owner.module()).then_some(module.as_ref())
                })
                .ok_or(RuntimeSemanticProjectionError::MissingModule { owner })?;
            if !expression_belongs_to_non_product_plan(module, owner)? {
                applications.push(application);
            }
            Ok::<_, RuntimeSemanticProjectionError>(applications)
        })
}

fn project_dialogue_application(
    project: HirAnalysisProjectView<'_>,
    owner: ExprId,
    target: &CheckedCharacterDialogueTarget,
    rich_text: &CheckedRichTextReport,
    line_result: &TypeKind,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instance: Option<ProjectInstanceTypes<'_>>,
    generation: CharacterPresentationCatalogGeneration,
    presentation: DialoguePresentationSnapshot,
    scope: &RuntimeDialogueProjectionScope,
    template_ids: &RuntimeDialogueTemplateIdCatalog,
    fx_catalog: &crate::fx_catalog::CompiledFxCatalog,
) -> Result<
    (
        ExprId,
        RuntimeDialogueApplication,
        Vec<RuntimeContentFragmentFact>,
    ),
    RuntimeSemanticProjectionError,
> {
    let character = target.character().exact().cloned().ok_or_else(|| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "dynamic CharacterDialogue target requires typed runtime-plan lowering"
                .to_owned(),
        }
    })?;
    let plan = CheckedCharacterPresentationPlan::try_new(
        CharacterPresentationTargetEvidence::Exact(character),
        generation,
    )
    .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
        owner: Some(owner),
        reason: error.to_string(),
    })?;
    let line = analysis
        .dialogue_lines()
        .for_semantic_expr(owner)
        .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "dialogue application has no accepted line identity".to_owned(),
        })?;
    let runtime_line =
        RuntimeLineId::from_source_entity_body(line.id().as_str()).map_err(|error| {
            RuntimeSemanticProjectionError::Dialogue {
                owner: Some(owner),
                reason: error.to_string(),
            }
        })?;
    let cue_handle_type =
        runtime_type_under(&TypeKind::CueHandle, instance, symbols, world, analysis)?;
    let template_id = template_ids.id(scope, rich_text)?;
    let (content, values, effects, marks, slots, nested_fragments) = lower_checked_rich_text(
        owner,
        project,
        rich_text,
        &cue_handle_type,
        symbols,
        world,
        analysis,
        instance,
        scope,
        template_ids,
        fx_catalog,
    )?;
    let mut mark_labels = Vec::new();
    collect_mark_labels(&content.nodes, &mut mark_labels);
    let template = DialogueContentFragmentTemplate::try_new_canonical(
        template_id,
        slots,
        mark_labels
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                DialogueContentTemplateMark::new(
                    RuntimeDialogueMarkId::from_zero_based(index)
                        .expect("checked dialogue mark ordinals fit runtime IDs"),
                    label,
                )
            })
            .collect(),
        effects
            .iter()
            .enumerate()
            .map(|(index, _)| {
                DialogueContentTemplateEffect::new(
                    arcweft_core::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index)
                        .expect("checked dialogue effect ordinals fit runtime IDs"),
                )
            })
            .collect(),
        content,
    )
    .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
        owner: Some(owner),
        reason: error.to_string(),
    })?;
    let source = ProductSourceRef::try_for_identity(line.source().application_span().source())
        .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: error.to_string(),
        })?;
    let mut fragments = Vec::with_capacity(nested_fragments.len() + 1);
    let spec = DialogueContentSpec::try_new(
        runtime_line,
        line.text_key().as_text_key().clone(),
        &template,
        plan,
        presentation,
        Vec::new(),
        source,
    )
    .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
        owner: Some(owner),
        reason: error.to_string(),
    })?;
    let fragment = RuntimeContentFragmentFact::try_new(
        owner,
        rich_text.fragment_coordinate().clone(),
        template,
        values,
        effects,
        runtime_dialogue_mark_facts(template_id, &marks),
    )
    .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
        owner: Some(owner),
        reason: error.to_string(),
    })?;
    fragments.push(fragment);
    fragments.extend(nested_fragments);
    Ok((
        owner,
        RuntimeDialogueApplication::new(
            spec,
            runtime_type_under(line_result, instance, symbols, world, analysis)?,
        ),
        fragments,
    ))
}

fn expression_belongs_to_non_product_plan(
    module: &arcweft_lang_hir::module::HirModule,
    owner: ExprId,
) -> Result<bool, RuntimeSemanticProjectionError> {
    let mut scope = Some(
        module
            .resolve_expr(owner)
            .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
                owner: Some(owner),
                reason: error.to_string(),
            })?
            .scope(),
    );
    while let Some(current) = scope {
        let resolved = module.resolve_scope(current).map_err(|error| {
            RuntimeSemanticProjectionError::Dialogue {
                owner: Some(owner),
                reason: error.to_string(),
            }
        })?;
        if let HirScopeOwner::Item(item) = *resolved.owner() {
            let item = module.resolve_item(item).map_err(|error| {
                RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(owner),
                    reason: error.to_string(),
                }
            })?;
            return Ok(matches!(
                item.kind(),
                HirItemKind::Test(_) | HirItemKind::Bench(_)
            ));
        }
        scope = resolved.parent();
    }
    Ok(false)
}

fn build_character_presentation_catalog(
    project: HirAnalysisProjectView<'_>,
    analysis: &FinalSemanticAnalysis,
    policy: CharacterNameLocalePolicy,
) -> Result<CharacterPresentationCatalogData, RuntimeSemanticProjectionError> {
    let records = project
        .items()
        .filter(|item| matches!(item.item().kind(), HirItemKind::Character(_)))
        .map(|item| character_presentation_record(item, analysis))
        .collect::<Result<Vec<_>, _>>()?;
    let input = CharacterPresentationCatalogInput::try_new(policy, records).map_err(|error| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: error.to_string(),
        }
    })?;
    CharacterPresentationCatalogData::try_from_inputs(input).map_err(|error| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: error.to_string(),
        }
    })
}

fn character_name_locale_policy(
    policy: &CharacterNameLocalePolicySpec,
) -> Result<CharacterNameLocalePolicy, RuntimeSemanticProjectionError> {
    let active = CharacterNameLocale::new(policy.active().clone());
    let fallbacks = policy
        .fallbacks()
        .iter()
        .cloned()
        .map(CharacterNameLocale::new)
        .map(CharacterNameFallbackLocale::new)
        .collect();
    CharacterNameLocalePolicy::try_new(active, fallbacks).map_err(|error| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: error.to_string(),
        }
    })
}

fn character_presentation_record(
    item: HirProjectItemRef<'_>,
    analysis: &FinalSemanticAnalysis,
) -> Result<CharacterDisplayNameRecordInput, RuntimeSemanticProjectionError> {
    let HirItemKind::Character(character) = item.item().kind() else {
        return Err(RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "non-Character item entered Character presentation projection".to_owned(),
        });
    };
    let public_id = character.header().public_id().resolved().ok_or_else(|| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "a recovered Character identity cannot enter the presentation catalog"
                .to_owned(),
        }
    })?;
    let character_id = CharacterId::try_new(public_id.as_str()).map_err(|error| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: error.to_string(),
        }
    })?;
    let base = character
        .display()
        .map(|member| {
            let member = item
                .module()
                .declaration_members()
                .resolve(member)
                .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
                    owner: None,
                    reason: error.to_string(),
                })?;
            let HirDeclarationMemberKind::CharacterDisplay(member) = member.kind() else {
                return Err(RuntimeSemanticProjectionError::Dialogue {
                    owner: None,
                    reason: "Character display member has the wrong typed family".to_owned(),
                });
            };
            let initializer =
                member
                    .initializer()
                    .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
                        owner: None,
                        reason: "Character display has no checked initializer".to_owned(),
                    })?;
            let value = analysis
                .expression(initializer)
                .and_then(|expression| match expression.resolution() {
                    CheckedExpressionResolution::Literal(HirLiteral::String(
                        HirStringLiteral::Value(value),
                    )) => Some(value.to_string()),
                    _ => None,
                })
                .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(initializer),
                    reason: "Character display must be a checked constant String".to_owned(),
                })?;
            CharacterDisplayNameValue::try_new(value)
                .map(CharacterDisplayNameInput::Visible)
                .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(initializer),
                    reason: error.to_string(),
                })
        })
        .transpose()?;
    let fallback = match character.header().name() {
        HirRetainedName::Resolved(name) => Some(name.as_str()),
        HirRetainedName::Missing | HirRetainedName::Invalid => None,
    }
    .map(CharacterDisplayNameValue::try_new)
    .transpose()
    .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
        owner: None,
        reason: error.to_string(),
    })?;
    CharacterDisplayNameRecordInput::try_new(
        character_id,
        CharacterPresentationRole::Character,
        None,
        base,
        Vec::new(),
        fallback,
    )
    .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
        owner: None,
        reason: error.to_string(),
    })
}

fn lower_checked_rich_text(
    owner: ExprId,
    project: HirAnalysisProjectView<'_>,
    report: &CheckedRichTextReport,
    cue_handle_type: &RuntimeNormalizedType,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instance: Option<ProjectInstanceTypes<'_>>,
    scope: &RuntimeDialogueProjectionScope,
    template_ids: &RuntimeDialogueTemplateIdCatalog,
    fx_catalog: &crate::fx_catalog::CompiledFxCatalog,
) -> Result<
    (
        RichTextDocument,
        Vec<RuntimeDialogueValueExpression>,
        Vec<RuntimeDialogueEffectProgramFact>,
        BTreeMap<StableCheckedDialogueMarkCoordinate, RuntimeDialogueMarkId>,
        Vec<DialogueContentTemplateSlot>,
        Vec<RuntimeContentFragmentFact>,
    ),
    RuntimeSemanticProjectionError,
> {
    let effect_sites = report.effect_plan().effect_sites();
    let direct_mark_count = report
        .content()
        .tokens()
        .iter()
        .filter(|token| {
            matches!(
                token,
                CheckedDialogueToken::PointAction(CheckedRichTextAction::Marker(_))
            )
        })
        .count();
    let mut nodes = Vec::new();
    let mut values = Vec::new();
    let mut effects = Vec::new();
    let mut next_effect = 0_usize;
    let mut next_mark = 0_usize;
    let mut marks = BTreeMap::new();
    let mut nested_fragments = Vec::new();
    for token in report.content().tokens() {
        match token {
            CheckedDialogueToken::Text(text) => nodes.push(RichTextNode::Text {
                text: text.to_string(),
            }),
            CheckedDialogueToken::RawLiteral(text) => nodes.push(RichTextNode::Raw {
                text: text.to_string(),
            }),
            CheckedDialogueToken::Escape(value) => nodes.push(RichTextNode::Text {
                text: value.to_string(),
            }),
            CheckedDialogueToken::Interpolation(expression) => {
                let slot = next_dialogue_slot(owner, values.len())?;
                values.push(runtime_dialogue_value_expression(
                    owner,
                    slot,
                    RuntimeDialogueValueRole::Interpolation,
                    *expression,
                    symbols,
                    world,
                    analysis,
                    instance,
                )?);
                nodes.push(RichTextNode::Interpolation {
                    slot,
                    label: format!("{expression:?}"),
                    on_error: InlineFailureSelection::InheritCharacterDialogue,
                });
            }
            CheckedDialogueToken::LineBreak(kind) => match kind {
                arcweft_lang_hir::dialogue_application::HirLineBreakKind::Line => {
                    nodes.push(RichTextNode::Control {
                        control: RichTextControl::HardBreak,
                    });
                }
                arcweft_lang_hir::dialogue_application::HirLineBreakKind::Paragraph => {
                    nodes.push(RichTextNode::Text {
                        text: "\n\n".to_owned(),
                    });
                }
                arcweft_lang_hir::dialogue_application::HirLineBreakKind::Page => {
                    nodes.push(RichTextNode::Control {
                        control: RichTextControl::Page,
                    });
                }
            },
            CheckedDialogueToken::PointAction(action) => {
                let output_effect_index = effects.len();
                let output_mark_index = marks.len();
                lower_rich_text_action(
                    owner,
                    action,
                    &mut nodes,
                    &mut effects,
                    effect_sites,
                    output_effect_index,
                    output_mark_index,
                    &mut next_effect,
                    &mut next_mark,
                    &mut marks,
                    cue_handle_type,
                    symbols,
                    world,
                    analysis,
                    instance,
                )?;
            }
            CheckedDialogueToken::ContentInsert(insertion) => {
                let body = insertion.argument().checked_content();
                match insertion.emission() {
                    CheckedContentEmission::ContentResult => {
                        let expression = match analysis.expression(insertion.site().raw()) {
                            Some(checked) => match checked.resolution() {
                                CheckedExpressionResolution::ContentApplication(application) => {
                                    match application.as_ref() {
                                        CheckedContentApplication::Value { source, .. } => {
                                            source.raw()
                                        }
                                        CheckedContentApplication::ContentResultCall { .. } => {
                                            insertion.site().raw()
                                        }
                                        CheckedContentApplication::EmissionCall { .. } => {
                                            return Err(RuntimeSemanticProjectionError::Dialogue {
                                                owner: Some(owner),
                                                reason: "content-result insertion has an emission execution authority".to_owned(),
                                            });
                                        }
                                    }
                                }
                                _ => {
                                    return Err(RuntimeSemanticProjectionError::Dialogue {
                                        owner: Some(owner),
                                        reason: "content insertion site has no content execution authority".to_owned(),
                                    });
                                }
                            },
                            None => {
                                return Err(RuntimeSemanticProjectionError::Dialogue {
                                    owner: Some(owner),
                                    reason: format!(
                                        "content insertion site {:?} has no checked expression fact",
                                        insertion.site().raw()
                                    ),
                                });
                            }
                        };
                        let checked = analysis.expression(expression).ok_or_else(|| {
                            RuntimeSemanticProjectionError::Dialogue {
                                owner: Some(owner),
                                reason: format!(
                                    "content insertion producer {expression:?} has no checked expression fact"
                                ),
                            }
                        })?;
                        let ty = checked.value_type().ok_or_else(|| {
                            RuntimeSemanticProjectionError::Dialogue {
                                owner: Some(owner),
                                reason: format!(
                                    "content insertion producer {expression:?} has no runtime value type"
                                ),
                            }
                        })?;
                        let runtime_ty =
                            runtime_type_under(ty, instance, symbols, world, analysis)?;
                        let semantic_type = runtime_ty.identity();
                        if semantic_type
                            != arcweft_core::value::RuntimeDialogueOpaqueRole::Content
                                .semantic_identity()
                        {
                            return Err(RuntimeSemanticProjectionError::Dialogue {
                                owner: Some(owner),
                                reason: format!(
                                    "content insertion producer {expression:?} is not the exact Content opaque type"
                                ),
                            });
                        }
                        let slot = next_dialogue_slot(owner, values.len())?;
                        values.push(RuntimeDialogueValueExpression::new(
                            slot,
                            RuntimeDialogueValueRole::Content,
                            expression,
                            runtime_ty,
                        ));
                        nodes.push(RichTextNode::ContentInsert {
                            slot,
                            on_error: insertion.failure_selection().clone(),
                        });

                        // A ContentResult body is the immutable template used by
                        // the produced Content value. It remains a separate
                        // template; structural modifier bodies are flattened
                        // below into their owning node.
                        if let Some(child) = body {
                            let child_owner = child.content().id().owner();
                            let child_template_id = template_ids.id(scope, child)?;
                            let (
                                child_document,
                                child_values,
                                child_effects,
                                child_marks,
                                child_slots,
                                child_nested_fragments,
                            ) = lower_checked_rich_text(
                                child_owner,
                                project,
                                child,
                                cue_handle_type,
                                symbols,
                                world,
                                analysis,
                                instance,
                                scope,
                                template_ids,
                                fx_catalog,
                            )?;
                            let child_template = make_dialogue_content_template(
                                child_owner,
                                child_template_id,
                                child_document,
                                &child_effects,
                                &child_marks,
                                child_slots,
                            )?;
                            nested_fragments.push(
                                RuntimeContentFragmentFact::try_new(
                                    child_owner,
                                    child.fragment_coordinate().clone(),
                                    child_template,
                                    child_values,
                                    child_effects,
                                    runtime_dialogue_mark_facts(child_template_id, &child_marks),
                                )
                                .map_err(|error| {
                                    RuntimeSemanticProjectionError::Dialogue {
                                        owner: Some(child_owner),
                                        reason: error.to_string(),
                                    }
                                })?,
                            );
                            nested_fragments.extend(child_nested_fragments);
                        }
                    }
                    CheckedContentEmission::Modifier(modifier) => {
                        let (body, child_values, child_effects, child_marks, child_nested) =
                            lower_attached_content_body(
                                owner,
                                project,
                                body,
                                cue_handle_type,
                                symbols,
                                world,
                                analysis,
                                instance,
                                scope,
                                template_ids,
                                fx_catalog,
                            )?;
                        let (body, child_values, child_effects, child_marks) = rebase_child_body(
                            owner,
                            body,
                            child_values,
                            child_effects,
                            child_marks,
                            values.len(),
                            marks.len(),
                            effects.len(),
                        )?;
                        values.extend(child_values);
                        effects.extend(child_effects);
                        merge_child_marks(owner, &mut marks, child_marks)?;
                        nodes.push(RichTextNode::Scope {
                            style: Box::new(lower_content_modifier(owner, modifier)?),
                            body,
                        });
                        nested_fragments.extend(child_nested);
                    }
                    CheckedContentEmission::Fx(application) => {
                        let (body, child_values, child_effects, child_marks, child_nested) =
                            lower_attached_content_body(
                                owner,
                                project,
                                body,
                                cue_handle_type,
                                symbols,
                                world,
                                analysis,
                                instance,
                                scope,
                                template_ids,
                                fx_catalog,
                            )?;
                        let (body, child_values, child_effects, child_marks) = rebase_child_body(
                            owner,
                            body,
                            child_values,
                            child_effects,
                            child_marks,
                            values.len(),
                            marks.len(),
                            effects.len(),
                        )?;
                        values.extend(child_values);
                        effects.extend(child_effects);
                        merge_child_marks(owner, &mut marks, child_marks)?;
                        nodes.push(RichTextNode::Scope {
                            style: Box::new(lower_content_fx_application(
                                owner,
                                application,
                                fx_catalog,
                                analysis.checked_fx_definitions(),
                            )?),
                            body,
                        });
                        nested_fragments.extend(child_nested);
                    }
                    CheckedContentEmission::Ruby(ruby) => {
                        let (body, child_values, child_effects, child_marks, child_nested) =
                            lower_attached_content_body(
                                owner,
                                project,
                                body,
                                cue_handle_type,
                                symbols,
                                world,
                                analysis,
                                instance,
                                scope,
                                template_ids,
                                fx_catalog,
                            )?;
                        let mark_offset = marks.len();
                        let effect_offset = effects.len();
                        let (body, child_values, child_effects, child_marks) = rebase_child_body(
                            owner,
                            body,
                            child_values,
                            child_effects,
                            child_marks,
                            values.len(),
                            mark_offset,
                            effect_offset,
                        )?;
                        values.extend(child_values);
                        effects.extend(child_effects);
                        merge_child_marks(owner, &mut marks, child_marks)?;
                        nodes.push(RichTextNode::Ruby {
                            body,
                            ruby: ruby.reading().to_owned(),
                        });
                        nested_fragments.extend(child_nested);
                    }
                    CheckedContentEmission::Raw(raw) => {
                        // Raw is an opaque typed literal.  Its checked body is
                        // deliberately not a dialogue report: reparsing it
                        // (or lowering it as ordinary content) would restore
                        // the removed source-delimiter authority.  A generic
                        // attached body is therefore an invalid sealed shape,
                        // rather than a candidate for best-effort lowering.
                        if body.is_some() {
                            return Err(RuntimeSemanticProjectionError::Dialogue {
                                owner: Some(owner),
                                reason: "raw Content emission cannot carry a checked attached body"
                                    .to_owned(),
                            });
                        }
                        nodes.push(RichTextNode::Raw {
                            text: raw.body().to_owned(),
                        });
                    }
                    CheckedContentEmission::ObjectSpan(_) => {
                        let (body, child_values, child_effects, child_marks, child_nested) =
                            lower_attached_content_body(
                                owner,
                                project,
                                body,
                                cue_handle_type,
                                symbols,
                                world,
                                analysis,
                                instance,
                                scope,
                                template_ids,
                                fx_catalog,
                            )?;
                        let mark_offset = marks.len();
                        let effect_offset = effects.len();
                        let (body, child_values, child_effects, child_marks) = rebase_child_body(
                            owner,
                            body,
                            child_values,
                            child_effects,
                            child_marks,
                            values.len(),
                            mark_offset,
                            effect_offset,
                        )?;
                        let proxy = text_proxy::lower_checked_object_emission(
                            owner,
                            insertion.emission(),
                            analysis,
                        )?;
                        values.extend(child_values);
                        effects.extend(child_effects);
                        merge_child_marks(owner, &mut marks, child_marks)?;
                        nodes.push(RichTextNode::Scope {
                            style: Box::new(RichTextStyle::Object { proxy }),
                            body,
                        });
                        nested_fragments.extend(child_nested);
                    }
                }
            }
        }
    }
    if next_effect != effect_sites.len() {
        return Err(RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "checked RichText effect-site inventory was not consumed exactly".to_owned(),
        });
    }
    if next_mark != direct_mark_count {
        return Err(RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "checked RichText marker coordinates were not projected bijectively".to_owned(),
        });
    }
    let slots = values
        .iter()
        .map(|value| {
            Ok(DialogueContentTemplateSlot::new(
                value.slot(),
                value.role(),
                value.ty().identity(),
            ))
        })
        .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?;
    Ok((
        RichTextDocument::new(nodes),
        values,
        effects,
        marks,
        slots,
        nested_fragments,
    ))
}

type LoweredContentParts = (
    Vec<RichTextNode>,
    Vec<RuntimeDialogueValueExpression>,
    Vec<RuntimeDialogueEffectProgramFact>,
    BTreeMap<StableCheckedDialogueMarkCoordinate, RuntimeDialogueMarkId>,
    Vec<RuntimeContentFragmentFact>,
);

fn lower_attached_content_body(
    owner: ExprId,
    project: HirAnalysisProjectView<'_>,
    body: Option<&CheckedRichTextReport>,
    cue_handle_type: &RuntimeNormalizedType,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instance: Option<ProjectInstanceTypes<'_>>,
    scope: &RuntimeDialogueProjectionScope,
    template_ids: &RuntimeDialogueTemplateIdCatalog,
    fx_catalog: &crate::fx_catalog::CompiledFxCatalog,
) -> Result<LoweredContentParts, RuntimeSemanticProjectionError> {
    let body = body.ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
        owner: Some(owner),
        reason: "content emission has no checked attached body".to_owned(),
    })?;
    let body_owner = body.content().id().owner();
    let (document, values, effects, marks, _slots, nested_fragments) = lower_checked_rich_text(
        body_owner,
        project,
        body,
        cue_handle_type,
        symbols,
        world,
        analysis,
        instance,
        scope,
        template_ids,
        fx_catalog,
    )?;
    Ok((document.nodes, values, effects, marks, nested_fragments))
}

fn rebase_child_body(
    owner: ExprId,
    body: Vec<RichTextNode>,
    values: Vec<RuntimeDialogueValueExpression>,
    effects: Vec<RuntimeDialogueEffectProgramFact>,
    marks: BTreeMap<StableCheckedDialogueMarkCoordinate, RuntimeDialogueMarkId>,
    slot_offset: usize,
    mark_offset: usize,
    effect_offset: usize,
) -> Result<
    (
        Vec<RichTextNode>,
        Vec<RuntimeDialogueValueExpression>,
        Vec<RuntimeDialogueEffectProgramFact>,
        BTreeMap<StableCheckedDialogueMarkCoordinate, RuntimeDialogueMarkId>,
    ),
    RuntimeSemanticProjectionError,
> {
    let body = body
        .into_iter()
        .map(|node| rebase_structured_node(owner, node, slot_offset, mark_offset, effect_offset))
        .collect::<Result<Vec<_>, _>>()?;
    let values = values
        .into_iter()
        .map(|value| {
            Ok(RuntimeDialogueValueExpression::new(
                rebase_dialogue_slot(owner, value.slot(), slot_offset)?,
                value.role(),
                value.expression(),
                value.ty().clone(),
            ))
        })
        .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?;
    let effects = effects
        .into_iter()
        .map(|effect| {
            Ok(RuntimeDialogueEffectProgramFact::new(
                rebase_effect_site(owner, effect.site(), effect_offset)?,
                effect.trigger().clone(),
                effect.effects().clone(),
                effect.operation().clone(),
                effect.captures().to_vec(),
            ))
        })
        .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?;
    let marks = marks
        .into_iter()
        .map(|(coordinate, mark)| Ok((coordinate, rebase_mark_id(owner, mark, mark_offset)?)))
        .collect::<Result<BTreeMap<_, _>, RuntimeSemanticProjectionError>>()?;
    Ok((body, values, effects, marks))
}

fn merge_child_marks(
    owner: ExprId,
    target: &mut BTreeMap<StableCheckedDialogueMarkCoordinate, RuntimeDialogueMarkId>,
    child: BTreeMap<StableCheckedDialogueMarkCoordinate, RuntimeDialogueMarkId>,
) -> Result<(), RuntimeSemanticProjectionError> {
    for (coordinate, mark) in child {
        if target.insert(coordinate, mark).is_some() {
            return Err(RuntimeSemanticProjectionError::Dialogue {
                owner: Some(owner),
                reason: "attached content body repeats a dialogue marker coordinate".to_owned(),
            });
        }
    }
    Ok(())
}

fn next_dialogue_slot(
    owner: ExprId,
    index: usize,
) -> Result<RuntimeDialogueValueSlotId, RuntimeSemanticProjectionError> {
    RuntimeDialogueValueSlotId::from_zero_based(index).ok_or_else(|| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "dialogue value slot count exceeds u32".to_owned(),
        }
    })
}

fn runtime_dialogue_value_expression(
    owner: ExprId,
    slot: RuntimeDialogueValueSlotId,
    role: RuntimeDialogueValueRole,
    expression: ExprId,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instance: Option<ProjectInstanceTypes<'_>>,
) -> Result<RuntimeDialogueValueExpression, RuntimeSemanticProjectionError> {
    let checked = analysis.expression(expression).ok_or_else(|| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: format!("dialogue value {expression:?} has no checked expression"),
        }
    })?;
    let ty = checked
        .value_type()
        .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: format!("dialogue value {expression:?} has no runtime value type"),
        })?;
    Ok(RuntimeDialogueValueExpression::new(
        slot,
        role,
        expression,
        runtime_type_under(ty, instance, symbols, world, analysis)?,
    ))
}

fn make_dialogue_content_template(
    owner: ExprId,
    id: RuntimeDialogueContentTemplateId,
    document: RichTextDocument,
    effects: &[RuntimeDialogueEffectProgramFact],
    marks: &BTreeMap<StableCheckedDialogueMarkCoordinate, RuntimeDialogueMarkId>,
    slots: Vec<DialogueContentTemplateSlot>,
) -> Result<DialogueContentFragmentTemplate, RuntimeSemanticProjectionError> {
    let mut mark_labels = Vec::new();
    collect_mark_labels(&document.nodes, &mut mark_labels);
    if mark_labels.len() != marks.len() {
        return Err(RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "nested dialogue content mark inventory is not bijective".to_owned(),
        });
    }
    let template_marks = mark_labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            RuntimeDialogueMarkId::from_zero_based(index)
                .map(|mark| DialogueContentTemplateMark::new(mark, label))
                .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(owner),
                    reason: "nested dialogue mark identity exceeds the runtime domain".to_owned(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let template_effects = effects
        .iter()
        .enumerate()
        .map(|(index, effect)| {
            let expected =
                arcweft_core::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index)
                    .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
                        owner: Some(owner),
                        reason: "nested dialogue effect identity exceeds the runtime domain"
                            .to_owned(),
                    })?;
            if effect.site() != expected {
                return Err(RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(owner),
                    reason: "nested dialogue effect inventory is not canonical".to_owned(),
                });
            }
            Ok(DialogueContentTemplateEffect::new(expected))
        })
        .collect::<Result<Vec<_>, _>>()?;
    DialogueContentFragmentTemplate::try_new_canonical(
        id,
        slots,
        template_marks,
        template_effects,
        document,
    )
    .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
        owner: Some(owner),
        reason: error.to_string(),
    })
}

fn runtime_dialogue_mark_facts(
    template: RuntimeDialogueContentTemplateId,
    marks: &BTreeMap<StableCheckedDialogueMarkCoordinate, RuntimeDialogueMarkId>,
) -> Box<[RuntimeDialogueMarkFact]> {
    let mut facts = marks
        .iter()
        .map(|(coordinate, mark)| {
            RuntimeDialogueMarkFact::new(
                coordinate.clone(),
                RuntimeDialogueMarkKey::new(template, *mark),
            )
        })
        .collect::<Vec<_>>();
    facts.sort_by_key(|fact| fact.key().mark());
    facts.into_boxed_slice()
}

fn collect_mark_labels(nodes: &[RichTextNode], labels: &mut Vec<String>) {
    for node in nodes {
        match node {
            RichTextNode::Control {
                control:
                    RichTextControl::Mark {
                        diagnostic_name, ..
                    },
            } => labels.push(diagnostic_name.clone()),
            RichTextNode::Scope { body, .. } | RichTextNode::Ruby { body, .. } => {
                collect_mark_labels(body, labels);
            }
            _ => {}
        }
    }
}

fn rebase_structured_node(
    owner: ExprId,
    node: RichTextNode,
    slot_offset: usize,
    mark_offset: usize,
    effect_offset: usize,
) -> Result<RichTextNode, RuntimeSemanticProjectionError> {
    let rebase_slot = |slot| rebase_dialogue_slot(owner, slot, slot_offset);
    Ok(match node {
        RichTextNode::Interpolation {
            slot,
            label,
            on_error,
        } => RichTextNode::Interpolation {
            slot: rebase_slot(slot)?,
            label,
            on_error,
        },
        RichTextNode::ContentInsert { slot, on_error } => RichTextNode::ContentInsert {
            slot: rebase_slot(slot)?,
            on_error,
        },
        RichTextNode::Scope { style, body } => RichTextNode::Scope {
            style,
            body: body
                .into_iter()
                .map(|node| {
                    rebase_structured_node(owner, node, slot_offset, mark_offset, effect_offset)
                })
                .collect::<Result<Vec<_>, _>>()?,
        },
        RichTextNode::Ruby { body, ruby } => RichTextNode::Ruby {
            body: body
                .into_iter()
                .map(|node| {
                    rebase_structured_node(owner, node, slot_offset, mark_offset, effect_offset)
                })
                .collect::<Result<Vec<_>, _>>()?,
            ruby,
        },
        RichTextNode::Control { control } => RichTextNode::Control {
            control: match control {
                RichTextControl::Mark {
                    mark,
                    diagnostic_name,
                } => RichTextControl::Mark {
                    mark: rebase_mark_id(owner, mark, mark_offset)?,
                    diagnostic_name,
                },
                RichTextControl::Effect { site } => RichTextControl::Effect {
                    site: rebase_effect_site(owner, site, effect_offset)?,
                },
                other => other,
            },
        },
        other => other,
    })
}

fn rebase_dialogue_slot(
    owner: ExprId,
    slot: RuntimeDialogueValueSlotId,
    offset: usize,
) -> Result<RuntimeDialogueValueSlotId, RuntimeSemanticProjectionError> {
    let index = slot.index().checked_add(offset).ok_or_else(|| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "flattened object slot identity overflows usize".to_owned(),
        }
    })?;
    RuntimeDialogueValueSlotId::from_zero_based(index).ok_or_else(|| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "flattened object slot identity exceeds the runtime domain".to_owned(),
        }
    })
}

fn rebase_mark_id(
    owner: ExprId,
    mark: RuntimeDialogueMarkId,
    offset: usize,
) -> Result<RuntimeDialogueMarkId, RuntimeSemanticProjectionError> {
    let index = mark.index().checked_add(offset).ok_or_else(|| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "flattened object mark identity overflows usize".to_owned(),
        }
    })?;
    RuntimeDialogueMarkId::from_zero_based(index).ok_or_else(|| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "flattened object mark identity exceeds the runtime domain".to_owned(),
        }
    })
}

fn rebase_effect_site(
    owner: ExprId,
    site: arcweft_core::runtime_id::RuntimeDialogueEffectSiteId,
    offset: usize,
) -> Result<arcweft_core::runtime_id::RuntimeDialogueEffectSiteId, RuntimeSemanticProjectionError> {
    let index = site.index().checked_add(offset).ok_or_else(|| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "flattened object effect identity overflows usize".to_owned(),
        }
    })?;
    arcweft_core::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index).ok_or_else(|| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "flattened object effect identity exceeds the runtime domain".to_owned(),
        }
    })
}

fn lower_content_modifier(
    owner: ExprId,
    modifier: &CheckedContentModifier,
) -> Result<RichTextStyle, RuntimeSemanticProjectionError> {
    let value = |id| required_modifier_value(owner, modifier, id);
    match modifier.definition() {
        PresentationContentCallableDefinitionId::Strong => Ok(RichTextStyle::Strong),
        PresentationContentCallableDefinitionId::Em => Ok(RichTextStyle::Em),
        PresentationContentCallableDefinitionId::Color => Ok(RichTextStyle::Color {
            value: lower_checked_color(value(PresentationContentCallableParameterId::Value)?)?,
        }),
        PresentationContentCallableDefinitionId::Font => Ok(RichTextStyle::Font {
            family: RichTextFontFamily::Named {
                name: lower_checked_text(value(PresentationContentCallableParameterId::Value)?)?,
            },
        }),
        PresentationContentCallableDefinitionId::Size => {
            let length =
                lower_checked_length(value(PresentationContentCallableParameterId::Value)?)?;
            if length.1 != arcweft_lang_sema::checked_rich_text::LengthUnit::Pt {
                return Err(content_modifier_error(
                    owner,
                    "RichText size emission requires a point length",
                ));
            }
            Ok(RichTextStyle::Size {
                milli_points: Milli(length.0),
            })
        }
        PresentationContentCallableDefinitionId::Style(selector) => {
            lower_style_modifier(owner, selector, modifier)
        }
        PresentationContentCallableDefinitionId::Layout(selector) => {
            lower_layout_modifier(owner, selector, modifier)
        }
        PresentationContentCallableDefinitionId::Transform(selector) => {
            lower_transform_modifier(owner, selector, modifier)
        }
        PresentationContentCallableDefinitionId::Fx => Err(content_modifier_error(
            owner,
            "Fx adapter reached scalar Content modifier lowering",
        )),
        PresentationContentCallableDefinitionId::Ruby
        | PresentationContentCallableDefinitionId::Raw => Err(content_modifier_error(
            owner,
            "non-modifier Content emission reached modifier lowering",
        )),
    }
}

fn lower_content_fx_application(
    owner: ExprId,
    application: &arcweft_lang_sema::final_analysis::CheckedContentFxApplication,
    catalog: &crate::fx_catalog::CompiledFxCatalog,
    checked_catalog: &arcweft_lang_sema::final_analysis::CheckedFxDefinitionCatalog,
) -> Result<RichTextStyle, RuntimeSemanticProjectionError> {
    use arcweft_lang_sema::final_analysis::{CheckedContentFxBinding, CheckedFxBindingDecision};
    use arcweft_presentation::fx::{FxApplication, FxApplicationDraft};

    let definition_id = application.definition().definition();
    let definition = catalog.get(definition_id).ok_or_else(|| {
        content_modifier_error(
            owner,
            "checked Fx definition is absent from the compiled catalog",
        )
    })?;
    if application.definition().layout() != definition.parameter_layout().digest() {
        return Err(content_modifier_error(
            owner,
            "checked Fx application layout does not match the compiled definition",
        ));
    }

    let mut arguments = vec![None; definition.parameters().len()];
    for argument in application.arguments() {
        let parameter = checked_catalog
            .abi_parameter(application.definition(), argument.parameter())
            .map_err(|error| content_modifier_error_owned(owner, error.to_string()))?;
        match argument.decision() {
            CheckedFxBindingDecision::Explicit(CheckedContentFxBinding::Abi(value)) => {
                let parameter = parameter.ok_or_else(|| {
                    content_modifier_error(owner, "checked Fx ABI value has no ABI parameter")
                })?;
                let definition_parameter = definition
                    .parameters()
                    .get(usize::from(parameter.get()))
                    .filter(|row| row.index() == parameter)
                    .ok_or_else(|| {
                        content_modifier_error(
                            owner,
                            "checked Fx parameter index exceeds its compiled definition",
                        )
                    })?;
                if definition_parameter.parameter_type() != value.parameter_type() {
                    return Err(content_modifier_error(
                        owner,
                        "checked Fx argument type differs from its compiled definition",
                    ));
                }
                let slot = arguments
                    .get_mut(usize::from(parameter.get()))
                    .ok_or_else(|| {
                        content_modifier_error(
                            owner,
                            "checked Fx parameter index exceeds its compiled definition",
                        )
                    })?;
                if slot.replace(value.clone()).is_some() {
                    return Err(content_modifier_error(
                        owner,
                        "checked Fx application repeats an ABI parameter",
                    ));
                }
            }
            CheckedFxBindingDecision::Explicit(
                CheckedContentFxBinding::Phase(_)
                | CheckedContentFxBinding::Target(_)
                | CheckedContentFxBinding::MotionFunction(_),
            ) => {
                if parameter.is_some() {
                    return Err(content_modifier_error(
                        owner,
                        "checked structural Fx selector aliases an ABI parameter",
                    ));
                }
            }
            CheckedFxBindingDecision::Defaulted | CheckedFxBindingDecision::Omitted => {}
        }
    }
    let draft = FxApplicationDraft::try_new(
        definition_id.clone(),
        arguments,
        application.ordinal().get(),
        None,
    )
    .map_err(|error| content_modifier_error_owned(owner, error.to_string()))?;
    let bound = FxApplication::bind(definition, draft)
        .map_err(|error| content_modifier_error_owned(owner, error.to_string()))?;
    bound
        .validate_for_definition(definition)
        .map_err(|error| content_modifier_error_owned(owner, error.to_string()))?;
    Ok(RichTextStyle::Fx { application: bound })
}

fn lower_style_modifier(
    owner: ExprId,
    selector: RichTextStyleSelector,
    modifier: &CheckedContentModifier,
) -> Result<RichTextStyle, RuntimeSemanticProjectionError> {
    let value = |id| required_modifier_value(owner, modifier, id);
    match selector {
        RichTextStyleSelector::Italic => Ok(RichTextStyle::Italic),
        RichTextStyleSelector::Oblique => Ok(RichTextStyle::Oblique {
            angle: RichTextAngle {
                degrees: Milli(lower_checked_angle(value(
                    PresentationContentCallableParameterId::Style(RichTextStyleProperty::Angle),
                )?)?),
            },
        }),
        RichTextStyleSelector::Opacity => Ok(RichTextStyle::Presentation {
            presentation: RichTextPresentationStyle {
                opacity: Some(Milli(i32::from(lower_checked_ratio(value(
                    PresentationContentCallableParameterId::Style(RichTextStyleProperty::Opacity),
                )?)?))),
                ..RichTextPresentationStyle::default()
            },
        }),
        RichTextStyleSelector::Layer => Ok(RichTextStyle::Presentation {
            presentation: RichTextPresentationStyle {
                layer: Some(lower_checked_public_id(value(
                    PresentationContentCallableParameterId::Style(RichTextStyleProperty::Layer),
                )?)?),
                ..RichTextPresentationStyle::default()
            },
        }),
        RichTextStyleSelector::ZIndex => {
            let value = lower_checked_int(value(PresentationContentCallableParameterId::Style(
                RichTextStyleProperty::ZIndex,
            ))?)?;
            let z_index = i16::try_from(value).map_err(|_| {
                content_modifier_error(owner, "RichText z-index is outside the runtime range")
            })?;
            Ok(RichTextStyle::Presentation {
                presentation: RichTextPresentationStyle {
                    z_index: Some(z_index),
                    ..RichTextPresentationStyle::default()
                },
            })
        }
    }
}

fn lower_layout_modifier(
    owner: ExprId,
    selector: RichTextLayoutSelector,
    modifier: &CheckedContentModifier,
) -> Result<RichTextStyle, RuntimeSemanticProjectionError> {
    let value = |id| required_modifier_value(owner, modifier, id);
    let mut layout = RichTextLayout::default();
    match selector {
        RichTextLayoutSelector::HorizontalTb => {
            layout.writing_mode = RichTextWritingMode::HorizontalTb;
        }
        RichTextLayoutSelector::VerticalRl => {
            layout.writing_mode = RichTextWritingMode::VerticalRl;
        }
        RichTextLayoutSelector::VerticalLr => {
            layout.writing_mode = RichTextWritingMode::VerticalLr;
        }
        RichTextLayoutSelector::Direction => {
            layout.direction = lower_layout_direction(value(
                PresentationContentCallableParameterId::Layout(RichTextLayoutProperty::Direction),
            )?)?;
        }
        RichTextLayoutSelector::RubyOver => {
            layout.ruby_position = RichTextRubyPosition::Over;
        }
        RichTextLayoutSelector::RubyUnder => {
            layout.ruby_position = RichTextRubyPosition::Under;
        }
        RichTextLayoutSelector::RubyInterCharacter => {
            layout.ruby_position = RichTextRubyPosition::InterCharacter;
        }
    }
    layout.vertical_latin = lower_vertical_latin(value(
        PresentationContentCallableParameterId::Layout(RichTextLayoutProperty::Latin),
    )?)?;
    layout.jlreq_strictness = lower_jlreq(value(PresentationContentCallableParameterId::Layout(
        RichTextLayoutProperty::Jlreq,
    ))?)?;
    layout.column_gap = Milli(
        lower_checked_length(value(PresentationContentCallableParameterId::Layout(
            RichTextLayoutProperty::ColumnGap,
        ))?)?
        .0,
    );
    layout.ruby_font_size = lower_optional_length(modifier, RichTextLayoutProperty::RubySize)?;
    layout.ruby_gap = lower_optional_length(modifier, RichTextLayoutProperty::RubyGap)?;
    layout.ruby_overhang = lower_optional_length(modifier, RichTextLayoutProperty::RubyOverhang)?;
    layout.ruby_collision_gap =
        lower_optional_length(modifier, RichTextLayoutProperty::RubyCollisionGap)?;
    Ok(RichTextStyle::Layout { layout })
}

fn lower_transform_modifier(
    owner: ExprId,
    selector: RichTextTransformSelector,
    modifier: &CheckedContentModifier,
) -> Result<RichTextStyle, RuntimeSemanticProjectionError> {
    let value = |id| required_modifier_value(owner, modifier, id);
    let mut transform = RichTextTransform::default();
    match selector {
        RichTextTransformSelector::Offset => {
            transform.translate = RichTextVec2::new(
                Milli(
                    lower_checked_length(value(
                        PresentationContentCallableParameterId::Transform(
                            RichTextTransformProperty::X,
                        ),
                    )?)?
                    .0,
                ),
                Milli(
                    lower_checked_length(value(
                        PresentationContentCallableParameterId::Transform(
                            RichTextTransformProperty::Y,
                        ),
                    )?)?
                    .0,
                ),
            );
        }
        RichTextTransformSelector::Rotate => {
            transform.rotate = RichTextAngle {
                degrees: Milli(lower_checked_angle(value(
                    PresentationContentCallableParameterId::Transform(
                        RichTextTransformProperty::Angle,
                    ),
                )?)?),
            };
        }
        RichTextTransformSelector::Scale => {
            transform.scale = RichTextVec2::new(
                Milli(lower_checked_milli(value(
                    PresentationContentCallableParameterId::Transform(RichTextTransformProperty::X),
                )?)?),
                Milli(lower_checked_milli(value(
                    PresentationContentCallableParameterId::Transform(RichTextTransformProperty::Y),
                )?)?),
            );
        }
        RichTextTransformSelector::Skew => {
            transform.skew = RichTextVec2::new(
                Milli(lower_checked_angle(value(
                    PresentationContentCallableParameterId::Transform(RichTextTransformProperty::X),
                )?)?),
                Milli(lower_checked_angle(value(
                    PresentationContentCallableParameterId::Transform(RichTextTransformProperty::Y),
                )?)?),
            );
        }
    }
    transform.target = lower_transform_target(value(
        PresentationContentCallableParameterId::Transform(RichTextTransformProperty::Target),
    )?)?;
    transform.origin = lower_transform_origin(value(
        PresentationContentCallableParameterId::Transform(RichTextTransformProperty::Origin),
    )?)?;
    Ok(RichTextStyle::Transform { transform })
}

fn lower_optional_length(
    modifier: &CheckedContentModifier,
    property: RichTextLayoutProperty,
) -> Result<Option<Milli>, RuntimeSemanticProjectionError> {
    modifier
        .parameters()
        .iter()
        .find(|parameter| {
            parameter.id() == PresentationContentCallableParameterId::Layout(property)
        })
        .map(|parameter| lower_checked_length(parameter.value()).map(|(value, _)| Milli(value)))
        .transpose()
}

fn required_modifier_value<'a>(
    owner: ExprId,
    modifier: &'a CheckedContentModifier,
    id: PresentationContentCallableParameterId,
) -> Result<&'a CheckedCompileTimeValue, RuntimeSemanticProjectionError> {
    modifier
        .parameters()
        .iter()
        .find(|parameter| parameter.id() == id)
        .map(CheckedContentParameter::value)
        .ok_or_else(|| {
            content_modifier_error(owner, "checked Content modifier omits a schema parameter")
        })
}

fn lower_checked_color(
    value: &CheckedCompileTimeValue,
) -> Result<RichTextColor, RuntimeSemanticProjectionError> {
    match value {
        CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Color(value)) => {
            Ok(match value {
                arcweft_lang_sema::checked_rich_text::CheckedColor::Rgba8(value) => {
                    RichTextColor::Rgba8 { value: *value }
                }
                arcweft_lang_sema::checked_rich_text::CheckedColor::Resource(id) => {
                    RichTextColor::Resource {
                        id: id.as_str().to_owned(),
                    }
                }
            })
        }
        _ => Err(RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked Content modifier color parameter has the wrong type".to_owned(),
        }),
    }
}

fn lower_checked_text(
    value: &CheckedCompileTimeValue,
) -> Result<String, RuntimeSemanticProjectionError> {
    match value {
        CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Text(value)) => {
            Ok(value.to_string())
        }
        _ => Err(RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked Content modifier text parameter has the wrong type".to_owned(),
        }),
    }
}

fn lower_checked_public_id(
    value: &CheckedCompileTimeValue,
) -> Result<String, RuntimeSemanticProjectionError> {
    match value {
        CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::PublicId(value)) => {
            Ok(value.as_str().to_owned())
        }
        _ => Err(RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked Content modifier public-id parameter has the wrong type".to_owned(),
        }),
    }
}

fn lower_checked_int(
    value: &CheckedCompileTimeValue,
) -> Result<i64, RuntimeSemanticProjectionError> {
    match value {
        CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Int(value)) => Ok(*value),
        _ => Err(RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked Content modifier integer parameter has the wrong type".to_owned(),
        }),
    }
}

fn lower_checked_milli(
    value: &CheckedCompileTimeValue,
) -> Result<i32, RuntimeSemanticProjectionError> {
    match value {
        CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Milli(value)) => Ok(value.0),
        _ => Err(RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked Content modifier fixed parameter has the wrong type".to_owned(),
        }),
    }
}

fn lower_checked_ratio(
    value: &CheckedCompileTimeValue,
) -> Result<u16, RuntimeSemanticProjectionError> {
    match value {
        CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Ratio(value)) => Ok(value.0),
        _ => Err(RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked Content modifier ratio parameter has the wrong type".to_owned(),
        }),
    }
}

fn lower_checked_length(
    value: &CheckedCompileTimeValue,
) -> Result<(i32, arcweft_lang_sema::checked_rich_text::LengthUnit), RuntimeSemanticProjectionError>
{
    match value {
        CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Length(value)) => {
            Ok((value.milli, value.unit))
        }
        _ => Err(RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked Content modifier length parameter has the wrong type".to_owned(),
        }),
    }
}

fn lower_checked_angle(
    value: &CheckedCompileTimeValue,
) -> Result<i32, RuntimeSemanticProjectionError> {
    match value {
        CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Angle(value)) => {
            Ok(value.milli_degrees)
        }
        _ => Err(RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked Content modifier angle parameter has the wrong type".to_owned(),
        }),
    }
}

fn lower_enum_variant(
    value: &CheckedCompileTimeValue,
    domain: ClosedEnumDomainId,
) -> Result<arcweft_id::closed_enum::ClosedEnumValueId, RuntimeSemanticProjectionError> {
    match value {
        CheckedCompileTimeValue::Enum(value) if value.domain() == domain => Ok(*value),
        _ => Err(RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked Content modifier enum parameter has the wrong domain".to_owned(),
        }),
    }
}

fn lower_layout_direction(
    value: &CheckedCompileTimeValue,
) -> Result<RichTextInlineDirection, RuntimeSemanticProjectionError> {
    let value = lower_enum_variant(
        value,
        arcweft_rich_text_schema::RichTextEnumDomain::LayoutDirection.domain_id(),
    )?;
    let value = LayoutDirection::ALL
        .get(usize::from(value.variant()))
        .copied()
        .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked layout direction enum ordinal is outside its owner domain".to_owned(),
        })?;
    match value {
        LayoutDirection::Auto => Ok(RichTextInlineDirection::Auto),
        LayoutDirection::Ltr => Ok(RichTextInlineDirection::Ltr),
        LayoutDirection::Rtl => Ok(RichTextInlineDirection::Rtl),
    }
}

fn lower_vertical_latin(
    value: &CheckedCompileTimeValue,
) -> Result<RichTextVerticalLatinMode, RuntimeSemanticProjectionError> {
    let value = lower_enum_variant(
        value,
        arcweft_rich_text_schema::RichTextEnumDomain::VerticalLatin.domain_id(),
    )?;
    let value = VerticalLatin::ALL
        .get(usize::from(value.variant()))
        .copied()
        .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked vertical-latin enum ordinal is outside its owner domain".to_owned(),
        })?;
    match value {
        VerticalLatin::Mixed => Ok(RichTextVerticalLatinMode::Mixed),
        VerticalLatin::Upright => Ok(RichTextVerticalLatinMode::Upright),
        VerticalLatin::Sideways => Ok(RichTextVerticalLatinMode::Sideways),
    }
}

fn lower_jlreq(
    value: &CheckedCompileTimeValue,
) -> Result<RichTextJlreqStrictness, RuntimeSemanticProjectionError> {
    let value = lower_enum_variant(
        value,
        arcweft_rich_text_schema::RichTextEnumDomain::Jlreq.domain_id(),
    )?;
    let value = Jlreq::ALL
        .get(usize::from(value.variant()))
        .copied()
        .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked JLREQ enum ordinal is outside its owner domain".to_owned(),
        })?;
    match value {
        Jlreq::Auto => Ok(RichTextJlreqStrictness::Auto),
        Jlreq::Loose => Ok(RichTextJlreqStrictness::Loose),
        Jlreq::Normal => Ok(RichTextJlreqStrictness::Normal),
        Jlreq::Strict => Ok(RichTextJlreqStrictness::Strict),
    }
}

fn lower_transform_target(
    value: &CheckedCompileTimeValue,
) -> Result<arcweft_presentation::fx::FxTarget, RuntimeSemanticProjectionError> {
    let value = lower_enum_variant(
        value,
        arcweft_rich_text_schema::RichTextEnumDomain::TransformTarget.domain_id(),
    )?;
    let value = TransformTarget::ALL
        .get(usize::from(value.variant()))
        .copied()
        .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked transform-target enum ordinal is outside its owner domain".to_owned(),
        })?;
    Ok(match value {
        TransformTarget::Node => FxTarget::Node,
        TransformTarget::Content => FxTarget::Content,
        TransformTarget::Background => FxTarget::Background,
        TransformTarget::Line => FxTarget::Line,
        TransformTarget::Glyph => FxTarget::Glyph,
        TransformTarget::Viewport => FxTarget::Viewport,
    })
}

fn lower_transform_origin(
    value: &CheckedCompileTimeValue,
) -> Result<RichTextTransformOrigin, RuntimeSemanticProjectionError> {
    let value = lower_enum_variant(
        value,
        arcweft_rich_text_schema::RichTextEnumDomain::TransformOrigin.domain_id(),
    )?;
    let value = TransformOrigin::ALL
        .get(usize::from(value.variant()))
        .copied()
        .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
            owner: None,
            reason: "checked transform-origin enum ordinal is outside its owner domain".to_owned(),
        })?;
    Ok(match value {
        TransformOrigin::BaselineStart => RichTextTransformOrigin::BaselineStart,
        TransformOrigin::BaselineCenter => RichTextTransformOrigin::BaselineCenter,
        TransformOrigin::Center => RichTextTransformOrigin::Center,
        TransformOrigin::GlyphCenter => RichTextTransformOrigin::GlyphCenter,
    })
}

fn content_modifier_error(owner: ExprId, reason: &'static str) -> RuntimeSemanticProjectionError {
    RuntimeSemanticProjectionError::Dialogue {
        owner: Some(owner),
        reason: reason.to_owned(),
    }
}

fn content_modifier_error_owned(owner: ExprId, reason: String) -> RuntimeSemanticProjectionError {
    RuntimeSemanticProjectionError::Dialogue {
        owner: Some(owner),
        reason,
    }
}

fn lower_rich_text_action(
    owner: ExprId,
    action: &CheckedRichTextAction,
    nodes: &mut Vec<RichTextNode>,
    effects: &mut Vec<RuntimeDialogueEffectProgramFact>,
    effect_sites: &[CheckedDialogueEffectSite],
    output_effect_index: usize,
    output_mark_index: usize,
    next_effect: &mut usize,
    next_mark: &mut usize,
    marks: &mut BTreeMap<StableCheckedDialogueMarkCoordinate, RuntimeDialogueMarkId>,
    cue_handle_type: &RuntimeNormalizedType,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instance: Option<ProjectInstanceTypes<'_>>,
) -> Result<(), RuntimeSemanticProjectionError> {
    match action {
        CheckedRichTextAction::Control { action, .. } => {
            let control = match action {
                CheckedDialogueControl::Page => RichTextControl::Page,
                CheckedDialogueControl::LineWait => RichTextControl::LineWait,
                CheckedDialogueControl::HardBreak => RichTextControl::HardBreak,
                CheckedDialogueControl::TimedWait { duration } => RichTextControl::TimedWait {
                    duration_millis: duration.millis,
                },
                CheckedDialogueControl::Clear => RichTextControl::Clear,
                CheckedDialogueControl::Reset => RichTextControl::Reset,
                CheckedDialogueControl::RevealRate { milli_cps } => RichTextControl::RevealRate {
                    milli_cps: Milli(milli_cps.0),
                },
            };
            nodes.push(RichTextNode::Control { control });
        }
        CheckedRichTextAction::Host { action, .. } => {
            lower_dialogue_host_action(
                owner,
                action,
                nodes,
                effects,
                effect_sites,
                output_effect_index,
                next_effect,
                cue_handle_type,
                symbols,
                world,
                analysis,
                instance,
            )?;
        }
        CheckedRichTextAction::Marker(marker) => {
            let expected = u32::try_from(*next_mark).map_err(|_| {
                RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(owner),
                    reason: "checked RichText mark ordinal exceeds u32".to_owned(),
                }
            })?;
            if marker.coordinate().ordinal().get() != expected {
                return Err(RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(owner),
                    reason: "checked RichText marker coordinate is not in content order".to_owned(),
                });
            }
            let mark =
                RuntimeDialogueMarkId::from_zero_based(output_mark_index).ok_or_else(|| {
                    RuntimeSemanticProjectionError::Dialogue {
                        owner: Some(owner),
                        reason: "checked RichText mark ordinal exceeds the runtime domain"
                            .to_owned(),
                    }
                })?;
            if marks.insert(marker.coordinate().clone(), mark).is_some() {
                return Err(RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(owner),
                    reason: "checked RichText marker coordinate occurs more than once".to_owned(),
                });
            }
            *next_mark = next_mark.checked_add(1).ok_or_else(|| {
                RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(owner),
                    reason: "checked RichText mark cursor overflow".to_owned(),
                }
            })?;
            nodes.push(RichTextNode::Control {
                control: RichTextControl::Mark {
                    mark,
                    diagnostic_name: marker.diagnostic_name().as_str().to_owned(),
                },
            });
        }
    }
    Ok(())
}

fn lower_dialogue_host_action(
    owner: ExprId,
    event: &CheckedDialogueHostEvent,
    nodes: &mut Vec<RichTextNode>,
    effects: &mut Vec<RuntimeDialogueEffectProgramFact>,
    effect_sites: &[CheckedDialogueEffectSite],
    output_effect_index: usize,
    next_effect: &mut usize,
    cue_handle_type: &RuntimeNormalizedType,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instance: Option<ProjectInstanceTypes<'_>>,
) -> Result<(), RuntimeSemanticProjectionError> {
    let static_event = match event {
        CheckedDialogueHostEvent::Voice { source } => DialogueHostEvent::Voice {
            source: match source {
                CheckedVoiceSource::Auto => DialogueVoiceSource::Auto,
                CheckedVoiceSource::Identity(id) => DialogueVoiceSource::Identity {
                    id: id.as_str().to_owned(),
                },
            },
        },
        CheckedDialogueHostEvent::Face { expression } => DialogueHostEvent::Face {
            expression: expression.as_str().to_owned(),
        },
        CheckedDialogueHostEvent::Pose { pose } => DialogueHostEvent::Pose {
            pose: pose.as_str().to_owned(),
        },
        CheckedDialogueHostEvent::Show { entity } => DialogueHostEvent::Show {
            entity: entity.as_str().to_owned(),
        },
        CheckedDialogueHostEvent::Hide { entity } => DialogueHostEvent::Hide {
            entity: entity.as_str().to_owned(),
        },
        CheckedDialogueHostEvent::Move { x, y } => DialogueHostEvent::Move {
            x: Milli(x.milli),
            y: Milli(y.milli),
        },
        CheckedDialogueHostEvent::Scale { x, y } => DialogueHostEvent::Scale {
            x: Milli(x.0),
            y: Milli(y.0),
        },
        CheckedDialogueHostEvent::Rotate { angle } => DialogueHostEvent::Rotate {
            angle: RichTextAngle {
                degrees: Milli(angle.milli_degrees),
            },
        },
        CheckedDialogueHostEvent::Animation { animation } => DialogueHostEvent::Anim {
            animation: animation.as_str().to_owned(),
        },
        CheckedDialogueHostEvent::Shake { amplitude } => DialogueHostEvent::Shake {
            amplitude: Milli(amplitude.milli),
        },
        CheckedDialogueHostEvent::TimedCue { .. } => {
            let site = checked_dialogue_effect_site(
                owner,
                effect_sites,
                next_effect,
                CheckedDialogueEffectTriggerKind::Delay,
            )?;
            let runtime_site = runtime_dialogue_effect_site_id(owner, output_effect_index)?;
            effects.push(runtime_dialogue_effect(
                owner,
                site,
                runtime_site,
                cue_handle_type,
                symbols,
                world,
                analysis,
                instance,
            )?);
            return Ok(());
        }
        CheckedDialogueHostEvent::Call { .. } => {
            let site = checked_dialogue_effect_site(
                owner,
                effect_sites,
                next_effect,
                CheckedDialogueEffectTriggerKind::Content,
            )?;
            let runtime_site = runtime_dialogue_effect_site_id(owner, output_effect_index)?;
            nodes.push(RichTextNode::Control {
                control: RichTextControl::Effect { site: runtime_site },
            });
            effects.push(runtime_dialogue_effect(
                owner,
                site,
                runtime_site,
                cue_handle_type,
                symbols,
                world,
                analysis,
                instance,
            )?);
            return Ok(());
        }
        CheckedDialogueHostEvent::Signal { signal } => DialogueHostEvent::Signal {
            signal: signal.as_str().to_owned(),
        },
    };
    nodes.push(RichTextNode::HostEvent {
        event: static_event,
    });
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CheckedDialogueEffectTriggerKind {
    Content,
    Delay,
}

fn checked_dialogue_effect_site<'a>(
    owner: ExprId,
    sites: &'a [CheckedDialogueEffectSite],
    next: &mut usize,
    expected: CheckedDialogueEffectTriggerKind,
) -> Result<&'a CheckedDialogueEffectSite, RuntimeSemanticProjectionError> {
    let site = sites
        .get(*next)
        .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "RichText effect has no checked source-ordered site".to_owned(),
        })?;
    *next = next
        .checked_add(1)
        .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "RichText effect-site cursor overflow".to_owned(),
        })?;
    let trigger_matches = matches!(
        (site.trigger(), expected),
        (
            CheckedDialogueEffectTrigger::Content,
            CheckedDialogueEffectTriggerKind::Content
        ) | (
            CheckedDialogueEffectTrigger::Delay(_),
            CheckedDialogueEffectTriggerKind::Delay
        )
    );
    if !trigger_matches {
        return Err(RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "RichText effect differs from its checked source-ordered site".to_owned(),
        });
    }
    Ok(site)
}

fn runtime_dialogue_effect_site_id(
    owner: ExprId,
    index: usize,
) -> Result<arcweft_core::runtime_id::RuntimeDialogueEffectSiteId, RuntimeSemanticProjectionError> {
    arcweft_core::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index).ok_or_else(|| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "checked RichText effect-site ordinal exceeds the runtime domain".to_owned(),
        }
    })
}

fn runtime_dialogue_effect(
    owner: ExprId,
    site: &CheckedDialogueEffectSite,
    runtime_site: arcweft_core::runtime_id::RuntimeDialogueEffectSiteId,
    cue_handle_type: &RuntimeNormalizedType,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instance: Option<ProjectInstanceTypes<'_>>,
) -> Result<RuntimeDialogueEffectProgramFact, RuntimeSemanticProjectionError> {
    let operation =
        runtime_evaluated_effect_under(site.effect(), symbols, world, analysis, instance)?;
    let trigger = match site.trigger() {
        CheckedDialogueEffectTrigger::Content => RuntimeDialogueEffectTrigger::Content,
        CheckedDialogueEffectTrigger::Delay(at) => {
            let nanos = at.millis.checked_mul(1_000_000).ok_or_else(|| {
                RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(owner),
                    reason: "checked RichText cue duration exceeds logical time".to_owned(),
                }
            })?;
            RuntimeDialogueEffectTrigger::Delay {
                duration: LogicalDuration::from_nanos(nanos),
                duration_type: runtime_type_under(
                    &TypeKind::Duration,
                    instance,
                    symbols,
                    world,
                    analysis,
                )?,
                schedule_handle_type: cue_handle_type.clone(),
            }
        }
    };
    let captures = site
        .captures()
        .iter()
        .map(|capture| {
            Ok(RuntimeDialogueEffectCaptureFact::new(
                capture.local(),
                capture.origin().clone(),
                runtime_type_under(capture.ty(), instance, symbols, world, analysis)?,
            ))
        })
        .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?;
    Ok(RuntimeDialogueEffectProgramFact::new(
        runtime_site,
        trigger,
        site.effects().clone(),
        operation,
        captures,
    ))
}

fn runtime_type(
    ty: &TypeKind,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeNormalizedType, RuntimeSemanticProjectionError> {
    runtime_type_at(
        ty,
        symbols,
        world,
        analysis,
        &RuntimeTypeProjectionPath::root(),
    )
}

fn runtime_type_under(
    ty: &TypeKind,
    enclosing: Option<ProjectInstanceTypes<'_>>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeNormalizedType, RuntimeSemanticProjectionError> {
    match enclosing {
        Some(solution) => runtime_type(&solution.instantiate_type(ty)?, symbols, world, analysis),
        None => runtime_type(ty, symbols, world, analysis),
    }
}

fn instantiate_array_length_under(
    length: &ArrayLength,
    enclosing: Option<ProjectInstanceTypes<'_>>,
) -> Result<ArrayLength, RuntimeSemanticProjectionError> {
    let Some(solution) = enclosing else {
        return Ok(length.clone());
    };
    Ok(solution.instantiate_array_length(length)?)
}

fn checked_expression_type<'a>(
    expression: &'a arcweft_lang_sema::final_analysis::CheckedExpression,
    owner: ExprId,
) -> Result<&'a TypeKind, RuntimeSemanticProjectionError> {
    expression
        .value_type()
        .ok_or_else(|| RuntimeSemanticProjectionError::Type {
            reason: format!("expression {owner:?} has a non-value semantic result"),
        })
}

#[expect(
    clippy::too_many_lines,
    reason = "the closed semantic type vocabulary must be projected exhaustively in one boundary"
)]
fn runtime_type_at(
    ty: &TypeKind,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    path: &RuntimeTypeProjectionPath,
) -> Result<RuntimeNormalizedType, RuntimeSemanticProjectionError> {
    let semantic_identity = ty.semantic_identity_digest()?;
    let identity = RuntimeSemanticTypeId::from(semantic_identity);
    let nested = |ty: &TypeKind| runtime_type_at(ty, symbols, world, analysis, path).map(Box::new);
    let nested_at = |ty: &TypeKind, step| {
        runtime_type_at(ty, symbols, world, analysis, &path.pushed(step)).map(Box::new)
    };
    let shape = match ty {
        TypeKind::Unit => RuntimeTypeShape::Unit,
        TypeKind::Never => RuntimeTypeShape::Never,
        TypeKind::Bool => RuntimeTypeShape::Bool,
        TypeKind::I8 => RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I8),
        TypeKind::I16 => RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I16),
        TypeKind::I32 => RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I32),
        TypeKind::I64 => RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I64),
        TypeKind::I128 => RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I128),
        TypeKind::ISize => RuntimeTypeShape::Signed(RuntimeSignedIntWidth::ISize),
        TypeKind::U8 => RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U8),
        TypeKind::U16 => RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U16),
        TypeKind::U32 => RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U32),
        TypeKind::U64 => RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U64),
        TypeKind::U128 => RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U128),
        TypeKind::USize => RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::USize),
        TypeKind::F32 => RuntimeTypeShape::F32,
        TypeKind::F64 => RuntimeTypeShape::F64,
        TypeKind::String => RuntimeTypeShape::String,
        TypeKind::Char => RuntimeTypeShape::Char,
        TypeKind::Bytes => RuntimeTypeShape::Bytes,
        TypeKind::Duration => RuntimeTypeShape::Duration,
        TypeKind::Progress => RuntimeTypeShape::Progress,
        TypeKind::StageActorHandle(handle) => RuntimeTypeShape::Opaque {
            producer: standard_line_handle_producer(RuntimeHandleKind::StageActor),
            admission: match handle {
                arcweft_lang_sema::types::StageActorHandleType::Exact(_) => {
                    arcweft_core::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity
                }
                arcweft_lang_sema::types::StageActorHandleType::Any => {
                    arcweft_core::pattern::RuntimeOpaqueTypeAdmission::ProducerWide
                }
            },
            value_class: RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::StageActor),
            persistence: RuntimeOpaquePersistence::SnapshotOnly,
            arguments: Box::new([]),
        },
        TypeKind::CueHandle => RuntimeTypeShape::Opaque {
            producer: standard_line_handle_producer(RuntimeHandleKind::Cue),
            admission: arcweft_core::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
            persistence: RuntimeOpaquePersistence::SnapshotOnly,
            arguments: Box::new([]),
        },
        TypeKind::VoiceHandle => RuntimeTypeShape::Opaque {
            producer: standard_line_handle_producer(RuntimeHandleKind::Voice),
            admission: arcweft_core::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Voice),
            persistence: RuntimeOpaquePersistence::SnapshotOnly,
            arguments: Box::new([]),
        },
        TypeKind::Ref(_) => RuntimeTypeShape::EntityReference,
        TypeKind::DebugStatePath => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::DebugStatePath),
        TypeKind::ObservationFieldPath => {
            RuntimeTypeShape::Agent(RuntimeAgentTypeShape::ObservationFieldPath)
        }
        TypeKind::Probe(value) => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::Probe(nested_at(
            value,
            RuntimeTypeProjectionStep::AgentProbeValue,
        )?)),
        TypeKind::Predicate => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::Predicate),
        TypeKind::Observation => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::Observation),
        TypeKind::ObservedObject => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::ObservedObject),
        TypeKind::AgentBBox => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::BoundingBox),
        TypeKind::ActionName => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::ActionName),
        TypeKind::ActionTarget => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::ActionTarget),
        TypeKind::ActionResult => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::ActionResult),
        TypeKind::AgentValue => RuntimeTypeShape::AgentValue,
        TypeKind::DataFormat => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::DataFormat),
        TypeKind::DataShape => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::DataShape),
        TypeKind::AgentEntityMetadata => {
            RuntimeTypeShape::Agent(RuntimeAgentTypeShape::EntityMetadata)
        }
        TypeKind::AgentSourceAnchor => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::SourceAnchor),
        TypeKind::AgentProjectGraphNeighborhood => {
            RuntimeTypeShape::Agent(RuntimeAgentTypeShape::ProjectGraphNeighborhood)
        }
        TypeKind::AgentProjectGraphSymbol => {
            RuntimeTypeShape::Agent(RuntimeAgentTypeShape::ProjectGraphSymbol)
        }
        TypeKind::AgentProjectGraphEdge => {
            RuntimeTypeShape::Agent(RuntimeAgentTypeShape::ProjectGraphEdge)
        }
        TypeKind::CaptureTarget => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::CaptureTarget),
        TypeKind::CaptureRef => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::CaptureReference),
        TypeKind::AgentResource => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::Resource),
        TypeKind::AgentResourceBody => RuntimeTypeShape::BuiltinVariant {
            owner: arcweft_core::pattern::RuntimeBuiltinVariantIdentity::AgentResourceBody,
            cases: vec![
                Some(runtime_type_at(
                    &TypeKind::AgentValue,
                    symbols,
                    world,
                    analysis,
                    path,
                )?),
                Some(runtime_type_at(
                    &TypeKind::String,
                    symbols,
                    world,
                    analysis,
                    path,
                )?),
                Some(runtime_type_at(
                    &TypeKind::AgentBuiltin(AgentBuiltinType::AgentBinaryBody),
                    symbols,
                    world,
                    analysis,
                    path,
                )?),
            ]
            .into_boxed_slice(),
        },
        TypeKind::RagContextPack => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::RagContextPack),
        TypeKind::AgentBuiltin(builtin) => match builtin.runtime_variant() {
            Some(owner) => RuntimeTypeShape::BuiltinVariant {
                owner,
                cases: owner.cases().iter().map(|_| None).collect(),
            },
            None => RuntimeTypeShape::Agent(
                runtime_agent_builtin_type(*builtin)
                    .expect("non-variant Agent builtin has one operational projection"),
            ),
        },
        TypeKind::Range(item) => RuntimeTypeShape::Range(nested(item)?),
        TypeKind::IteratorState { item, .. } => RuntimeTypeShape::Iterator(nested(item)?),
        TypeKind::Vec(item) => RuntimeTypeShape::Sequence {
            kind: RuntimeSequenceKind::Vec,
            item: nested_at(item, RuntimeTypeProjectionStep::SequenceItem)?,
        },
        TypeKind::Array {
            item,
            len: ArrayLength::Const(length),
        } => RuntimeTypeShape::Array {
            item: nested_at(item, RuntimeTypeProjectionStep::SequenceItem)?,
            length: *length,
        },
        TypeKind::Slice(item) => RuntimeTypeShape::Sequence {
            kind: RuntimeSequenceKind::Slice,
            item: nested_at(item, RuntimeTypeProjectionStep::SequenceItem)?,
        },
        TypeKind::Seq(item) => RuntimeTypeShape::Sequence {
            kind: RuntimeSequenceKind::Seq,
            item: nested_at(item, RuntimeTypeProjectionStep::SequenceItem)?,
        },
        TypeKind::Map { key, value, .. } => RuntimeTypeShape::Map {
            key: nested(key)?,
            value: nested(value)?,
        },
        TypeKind::BorrowRef { inner, .. } => RuntimeTypeShape::Reference(nested(inner)?),
        TypeKind::Need(item) => RuntimeTypeShape::Need(nested(item)?),
        TypeKind::Stream { item, error } => RuntimeTypeShape::Stream {
            item: nested(item)?,
            error: nested(error)?,
        },
        TypeKind::Parser { item, error } => RuntimeTypeShape::Parser {
            item: nested(item)?,
            error: nested(error)?,
        },
        TypeKind::Result { ok, error } => {
            let owner =
                CheckedVariantOwner::try_result(ok.as_ref().clone(), error.as_ref().clone())?;
            let value_payload = owner.case_payload_type(0).flatten().ok_or_else(|| {
                RuntimeSemanticProjectionError::Type {
                    reason: "Result::Ok has no exact accepted payload type".to_owned(),
                }
            })?;
            let error_payload = owner.case_payload_type(1).flatten().ok_or_else(|| {
                RuntimeSemanticProjectionError::Type {
                    reason: "Result::Err has no exact accepted payload type".to_owned(),
                }
            })?;
            RuntimeTypeShape::Result {
                value: nested_at(ok, RuntimeTypeProjectionStep::ResultOk)?,
                error: nested_at(error, RuntimeTypeProjectionStep::ResultError)?,
                value_payload: nested_at(
                    &value_payload,
                    RuntimeTypeProjectionStep::BuiltinVariantCase(0),
                )?,
                error_payload: nested_at(
                    &error_payload,
                    RuntimeTypeProjectionStep::BuiltinVariantCase(1),
                )?,
            }
        }
        TypeKind::Option(item) => {
            let owner = CheckedVariantOwner::try_option(item.as_ref().clone())?;
            let some_payload = owner.case_payload_type(0).flatten().ok_or_else(|| {
                RuntimeSemanticProjectionError::Type {
                    reason: "Option::Some has no exact accepted payload type".to_owned(),
                }
            })?;
            RuntimeTypeShape::Option {
                item: nested_at(item, RuntimeTypeProjectionStep::OptionItem)?,
                some_payload: nested_at(
                    &some_payload,
                    RuntimeTypeProjectionStep::BuiltinVariantCase(0),
                )?,
            }
        }
        TypeKind::ThreadHandle(item) => RuntimeTypeShape::ThreadHandle(nested(item)?),
        TypeKind::Shared(item) => RuntimeTypeShape::Shared(nested(item)?),
        TypeKind::Function {
            params,
            return_type,
            ..
        } => RuntimeTypeShape::Function {
            parameters: params
                .iter()
                .map(|parameter| runtime_type(parameter, symbols, world, analysis))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
            result: nested(return_type)?,
        },
        TypeKind::ProjectNominal(nominal) => {
            let semantic_type = semantic_identity;
            let projection = analysis
                .runtime_nominal_projection(semantic_type)
                .filter(|projection| projection.declaration() == nominal.declaration())
                .ok_or_else(|| RuntimeSemanticProjectionError::NominalSchemaProjection {
                    nominal: nominal.declaration().qualified_name(),
                    source: NominalSchemaProjectionError::MissingCachedProjection { semantic_type },
                })?;
            RuntimeTypeShape::ProjectNominal {
                nominal: RuntimeResolvedNominal::new(
                    nominal.declaration().clone(),
                    projection.owner(),
                    projection.nominal().clone(),
                    projection.semantic_identity(),
                    projection.layout(),
                ),
                arguments: nominal
                    .arguments()
                    .iter()
                    .map(|argument| runtime_type(argument, symbols, world, analysis))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            }
        }
        TypeKind::CompileTimeCallable(_) => {
            return Err(RuntimeSemanticProjectionError::Type {
                reason: format!(
                    "compile-time callable type `{}` cannot enter runtime projection",
                    ty.source_label()
                ),
            });
        }
        TypeKind::MetaType(_) => {
            return Err(RuntimeSemanticProjectionError::Type {
                reason: format!(
                    "compile-time meta-type `{}` cannot enter runtime projection",
                    ty.source_label()
                ),
            });
        }
        TypeKind::CompileTimeScalar(_) => {
            return Err(RuntimeSemanticProjectionError::Type {
                reason: format!(
                    "compile-time scalar type `{}` cannot enter runtime projection",
                    ty.source_label()
                ),
            });
        }
        TypeKind::CompileTimeEnum(_) | TypeKind::CompileTimeFx(_) | TypeKind::FixedVector(_) => {
            return Err(RuntimeSemanticProjectionError::Type {
                reason: format!(
                    "compile-time presentation type `{}` cannot enter runtime projection",
                    ty.source_label()
                ),
            });
        }
        TypeKind::DialogueLine(_) => {
            return Err(RuntimeSemanticProjectionError::Type {
                reason: "non-escaping DialogueLine operation reached runtime type projection"
                    .to_owned(),
            });
        }
        TypeKind::Tuple(items) => RuntimeTypeShape::Tuple(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    runtime_type_at(
                        item,
                        symbols,
                        world,
                        analysis,
                        &path.pushed(RuntimeTypeProjectionStep::TupleItem(projection_index(
                            index,
                        ))),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        ),
        TypeKind::VariantPayload(payload) => match payload.shape() {
            VariantPayloadTypeShape::Tuple(fields) => RuntimeTypeShape::Tuple(
                fields
                    .iter()
                    .enumerate()
                    .map(|(ordinal, field)| {
                        let ordinal = u32::try_from(ordinal).map_err(|_| {
                            RuntimeSemanticProjectionError::Type {
                                reason:
                                    "payload field ordinal exceeds the runtime coordinate domain"
                                        .to_owned(),
                            }
                        })?;
                        runtime_type_at(
                            field,
                            symbols,
                            world,
                            analysis,
                            &path.pushed(RuntimeTypeProjectionStep::TupleItem(ordinal)),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
            VariantPayloadTypeShape::Record(fields) => RuntimeTypeShape::Record(
                fields
                    .iter()
                    .map(|field| {
                        runtime_type_at(
                            field.ty(),
                            symbols,
                            world,
                            analysis,
                            &path.pushed(RuntimeTypeProjectionStep::RecordField(field.ordinal())),
                        )
                        .map(|ty| RuntimeRecordTypeField::new(field.diagnostic_name(), ty))
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
        },
        TypeKind::Choice(items) => RuntimeTypeShape::Choice(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    runtime_type_at(
                        item,
                        symbols,
                        world,
                        analysis,
                        &path.pushed(RuntimeTypeProjectionStep::ChoiceAlternative(
                            projection_index(index),
                        )),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        ),
        TypeKind::Error(poison) => {
            return Err(RuntimeSemanticProjectionError::Type {
                reason: format!(
                    "semantic poison {} reached runtime projection",
                    poison.index()
                ),
            });
        }
        TypeKind::AcceptedNominal(nominal) => {
            let record = world
                .environment()
                .nominal_catalog()
                .exact(nominal.declaration().canonical_path())
                .filter(|record| {
                    record.id() == nominal.declaration()
                        && usize::from(record.arity()) == nominal.arguments().len()
                })
                .ok_or_else(|| RuntimeSemanticProjectionError::Type {
                    reason: "accepted nominal runtime carrier is absent or stale".to_owned(),
                })?;
            let AcceptedNominalSemantics::Opaque(carrier) = record.semantics() else {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: "accepted nominal has no opaque runtime-plan carrier".to_owned(),
                });
            };
            RuntimeTypeShape::Opaque {
                producer: carrier.producer().clone(),
                admission: arcweft_core::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: carrier.value_class(),
                persistence: carrier.persistence(),
                arguments: nominal
                    .arguments()
                    .iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        runtime_type_at(
                            argument,
                            symbols,
                            world,
                            analysis,
                            &path.pushed(RuntimeTypeProjectionStep::OpaqueArgument(
                                projection_index(index),
                            )),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            }
        }
        TypeKind::CharacterDialogue(dialogue) => {
            let owner = dialogue.runtime_opaque_owner();
            debug_assert_eq!(owner.semantic_identity(), identity);
            RuntimeTypeShape::Opaque {
                producer: owner.producer().clone(),
                admission: owner.admission(),
                value_class: owner.value_class(),
                persistence: owner.persistence(),
                arguments: Box::new([]),
            }
        }
        TypeKind::Named(type_label) => {
            let carrier = world
                .environment()
                .nominal_catalog()
                .environment_record_for_semantic_type(semantic_identity)
                .and_then(arcweft_lang_sema::env::nominal::AcceptedNominalRecord::runtime_carrier)
                .ok_or_else(|| {
                    RuntimeCheckedTypeProjectionError::MissingOpaqueProducerEvidence {
                        semantic_identity: identity,
                        path: path.clone(),
                        type_label: type_label.clone(),
                    }
                })?;
            RuntimeTypeShape::Opaque {
                producer: carrier.producer().clone(),
                admission: arcweft_core::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: carrier.value_class(),
                persistence: carrier.persistence(),
                arguments: Box::new([]),
            }
        }
        TypeKind::Array { .. }
        | TypeKind::TextCluster
        | TypeKind::DisplayText
        | TypeKind::StageApi(_)
        | TypeKind::LineContext
        | TypeKind::StatementIngress(_)
        | TypeKind::Handle { .. }
        | TypeKind::GenericParam(_)
        | TypeKind::OpenNominal(_)
        | TypeKind::Projection { .. }
        | TypeKind::CharacterPatch(_)
        | TypeKind::FocusPatch
        | TypeKind::ViewValue
        | TypeKind::CharacterNominal(_) => {
            return Err(RuntimeSemanticProjectionError::Type {
                reason: format!(
                    "checked type `{}` has no closed runtime representation",
                    ty.source_label()
                ),
            });
        }
    };
    Ok(RuntimeNormalizedType::new(identity, shape))
}

fn standard_line_handle_producer(kind: RuntimeHandleKind) -> RuntimeOpaqueTypeProducerId {
    kind.try_producer()
        .expect("standard line handle producer identities are canonical")
}

const fn runtime_agent_builtin_type(builtin: AgentBuiltinType) -> Option<RuntimeAgentTypeShape> {
    Some(match builtin {
        AgentBuiltinType::ObservedObjectId => RuntimeAgentTypeShape::ObservedObjectId,
        AgentBuiltinType::Diagnostics => RuntimeAgentTypeShape::Diagnostics,
        AgentBuiltinType::WaitError => RuntimeAgentTypeShape::WaitError,
        AgentBuiltinType::ViewportPoint => RuntimeAgentTypeShape::ViewportPoint,
        AgentBuiltinType::RagError => RuntimeAgentTypeShape::RagError,
        AgentBuiltinType::AgentSourcePosition => RuntimeAgentTypeShape::SourcePosition,
        AgentBuiltinType::AgentProjectFlowControlSummary => {
            RuntimeAgentTypeShape::ProjectFlowControlSummary
        }
        AgentBuiltinType::AgentProjectGraphSummary => RuntimeAgentTypeShape::ProjectGraphSummary,
        AgentBuiltinType::AgentBinaryBody => RuntimeAgentTypeShape::BinaryResourceBody,
        AgentBuiltinType::AgentBinaryData => RuntimeAgentTypeShape::BinaryData,
        AgentBuiltinType::CaptureFormat
        | AgentBuiltinType::CaptureKind
        | AgentBuiltinType::PointerButton
        | AgentBuiltinType::AgentBinaryEncoding => return None,
    })
}

fn projection_index(index: usize) -> u32 {
    u32::try_from(index).expect("semantic type collections fit the u32 projection path contract")
}

fn runtime_literal(literal: &HirLiteral, ty: &TypeKind) -> Result<RuntimeValue, String> {
    match literal {
        HirLiteral::String(HirStringLiteral::Value(value)) => {
            Ok(RuntimeValue::String(value.to_string()))
        }
        HirLiteral::Character(HirCharacterLiteral::Value(value)) => Ok(RuntimeValue::Char(*value)),
        HirLiteral::Integer(HirIntegerLiteral::Value { magnitude, .. }) => {
            runtime_integer_magnitude(magnitude, ty)
        }
        HirLiteral::Float(HirFloatLiteral::Value { decimal, .. }) => runtime_decimal(decimal, ty),
        HirLiteral::UnitNumber(HirUnitNumberLiteral::Value { decimal, .. }) => {
            runtime_decimal(decimal, ty)
        }
        HirLiteral::Boolean(value) => Ok(RuntimeValue::Bool(*value)),
        HirLiteral::Duration(HirDurationLiteral::Value(value)) => {
            let nanos = big_uint_to_u128(value.semantic_value().nanoseconds())
                .and_then(|value| u64::try_from(value).ok())
                .ok_or_else(|| {
                    "Duration literal exceeds the runtime u64 nanosecond domain".to_owned()
                })?;
            Ok(RuntimeValue::Duration(LogicalDuration::from_nanos(nanos)))
        }
        HirLiteral::String(HirStringLiteral::Invalid(issue)) => Err(issue.to_string()),
        HirLiteral::Character(HirCharacterLiteral::Invalid(issue)) => Err(issue.to_string()),
        HirLiteral::Integer(HirIntegerLiteral::Invalid(issue)) => Err(issue.to_string()),
        HirLiteral::Float(HirFloatLiteral::Invalid(issue)) => Err(issue.to_string()),
        HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(issue)) => Err(issue.to_string()),
        HirLiteral::Duration(HirDurationLiteral::Invalid(issue)) => Err(issue.to_string()),
    }
}

fn runtime_integer_magnitude(
    magnitude: &HirBigUint,
    ty: &TypeKind,
) -> Result<RuntimeValue, String> {
    let value = big_uint_to_u128(magnitude)
        .ok_or_else(|| "integer literal exceeds the runtime u128 magnitude domain".to_owned())?;
    let signed = |width| {
        let value = i128::try_from(value)
            .map_err(|_| "positive integer literal exceeds the runtime i128 domain".to_owned())?;
        RuntimeInt::from_i128(width, value)
            .map(RuntimeValue::Int)
            .ok_or_else(|| format!("integer literal does not fit {width:?}"))
    };
    let unsigned = |width| {
        RuntimeUInt::from_u128(width, value)
            .map(RuntimeValue::UInt)
            .ok_or_else(|| format!("integer literal does not fit {width:?}"))
    };
    match ty {
        TypeKind::I8 => signed(RuntimeSignedIntWidth::I8),
        TypeKind::I16 => signed(RuntimeSignedIntWidth::I16),
        TypeKind::I32 => signed(RuntimeSignedIntWidth::I32),
        TypeKind::I64 => signed(RuntimeSignedIntWidth::I64),
        TypeKind::I128 => signed(RuntimeSignedIntWidth::I128),
        TypeKind::ISize => signed(RuntimeSignedIntWidth::ISize),
        TypeKind::U8 => unsigned(RuntimeUnsignedIntWidth::U8),
        TypeKind::U16 => unsigned(RuntimeUnsignedIntWidth::U16),
        TypeKind::U32 => unsigned(RuntimeUnsignedIntWidth::U32),
        TypeKind::U64 => unsigned(RuntimeUnsignedIntWidth::U64),
        TypeKind::U128 => unsigned(RuntimeUnsignedIntWidth::U128),
        TypeKind::USize => unsigned(RuntimeUnsignedIntWidth::USize),
        _ => Err("integer literal has a non-integer checked type".to_owned()),
    }
}

fn big_uint_to_u128(value: &HirBigUint) -> Option<u128> {
    if value.limbs_le().len() > 4 {
        return None;
    }
    Some(
        value
            .limbs_le()
            .iter()
            .rev()
            .fold(0_u128, |accumulator, limb| {
                (accumulator << 32) | u128::from(*limb)
            }),
    )
}

fn runtime_decimal(decimal: &HirDecimal, ty: &TypeKind) -> Result<RuntimeValue, String> {
    let digits = decimal
        .coefficient()
        .digits()
        .iter()
        .map(|digit| char::from(b'0' + *digit))
        .collect::<String>();
    let exponent = i64::from(decimal.exponent10()) - i64::from(decimal.scale());
    let canonical = format!("{digits}e{exponent}");
    match ty {
        TypeKind::F32 => canonical
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map(RuntimeValue::F32)
            .ok_or_else(|| "decimal literal is outside the finite f32 domain".to_owned()),
        TypeKind::F64 => canonical
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .map(RuntimeValue::F64)
            .ok_or_else(|| "decimal literal is outside the finite f64 domain".to_owned()),
        _ => Err(
            "decimal literal has no runtime scalar representation for its checked type".to_owned(),
        ),
    }
}

fn runtime_value_resolution(
    value: &CheckedValueResolution,
    ty: &TypeKind,
    project_item_is_runtime_entity: bool,
) -> Result<Option<RuntimeResolvedValue>, String> {
    Ok(Some(match value {
        CheckedValueResolution::Local(local) => RuntimeResolvedValue::Local(*local),
        CheckedValueResolution::ProjectItem(item) if project_item_is_runtime_entity => {
            RuntimeResolvedValue::ProjectItem(
                runtime_project_item(item).map_err(|error| error.to_string())?,
            )
        }
        // A retained item selected through a Path is semantic input owned by
        // its enclosing typed construct (for example Dialogue application),
        // not a standalone runtime scalar. The parent checked fact carries
        // the exact retained owner into runtime-plan lowering.
        // The selected call fact, not a callee path expression, owns the
        // generation-bound checked callable digest. A bare callable reference
        // needs a typed function-value identity before it can be executable.
        CheckedValueResolution::ProjectCallable(_)
        | CheckedValueResolution::ProjectItem(_)
        | CheckedValueResolution::LineContext
        | CheckedValueResolution::CharacterField { .. }
        // Entry references are generation-bound tooling/selection identities;
        // they are not executable scalar values in the runtime expression VM.
        | CheckedValueResolution::Entry(_) => return Ok(None),
        CheckedValueResolution::Registered(registered) => RuntimeResolvedValue::Registered(
            RuntimeRegisteredValueId::from_bytes(*registered.as_bytes()),
        ),
        CheckedValueResolution::Constant(literal) => {
            RuntimeResolvedValue::Constant(runtime_literal(literal, ty)?)
        }
    }))
}

fn runtime_project_item(
    item: &arcweft_lang_sema::final_analysis::CheckedProjectItem,
) -> Result<RuntimeProjectItem, RuntimeSemanticProjectionError> {
    match item.owner() {
        CheckedProjectItemOwner::Retained(owner) => Ok(RuntimeProjectItem::new_retained(
            item.public_id().clone(),
            item.family(),
            *owner,
        )),
        CheckedProjectItemOwner::Flow {
            declaration,
            item: owner,
        } => {
            let CallableDeclarationKey::Flow(flow) = declaration else {
                unreachable!("checked structural Flow owns a Flow declaration key")
            };
            let runtime = runtime_flow_identity(flow).map_err(|_| {
                RuntimeSemanticProjectionError::InvalidFlowIdentity { owner: *owner }
            })?;
            Ok(RuntimeProjectItem::new_structural_flow(
                item.public_id().clone(),
                *owner,
                runtime,
            ))
        }
        CheckedProjectItemOwner::External(_) => Ok(RuntimeProjectItem::new_external_character(
            item.public_id().clone(),
        )),
    }
}

fn runtime_flow_identity(
    declaration: &arcweft_lang_hir::symbol::FlowDeclarationId,
) -> Result<FlowRuntimeId, arcweft_core::runtime_id::RuntimeIdError> {
    FlowRuntimeId::from_checked_declaration_digest(
        declaration.semantic_digest().into_bytes(),
        declaration.public_id().as_str(),
    )
}

fn runtime_select(
    owner: ExprId,
    select: &CheckedSelectResolution,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<Option<RuntimeResolvedSelect>, RuntimeSemanticProjectionError> {
    runtime_select_under(owner, select, None, world, analysis)
}

fn runtime_select_under(
    owner: ExprId,
    select: &CheckedSelectResolution,
    closed_owner: Option<RuntimeSemanticTypeId>,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<Option<RuntimeResolvedSelect>, RuntimeSemanticProjectionError> {
    Ok(Some(match select {
        CheckedSelectResolution::DialogueView { field, .. } => {
            runtime_opaque_environment_select(world, field).ok_or(
                RuntimeSemanticProjectionError::UnrepresentableEnvironmentRecordField {
                    owner: RuntimeRecordExecutableOwner::Expression(owner),
                    semantic_owner: field.owner_type(),
                    ordinal: field.declaration_ordinal(),
                },
            )?
        }
        CheckedSelectResolution::Method(_) => RuntimeResolvedSelect::Method,
        CheckedSelectResolution::AgentField { field } => {
            RuntimeResolvedSelect::AgentField { field: *field }
        }
        CheckedSelectResolution::ProgressField { field } => RuntimeResolvedSelect::ProgressField {
            field: match field {
                arcweft_lang_sema::types::ProgressField::Ratio => {
                    arcweft_core::value::RuntimeProgressField::Ratio
                }
                arcweft_lang_sema::types::ProgressField::Label => {
                    arcweft_core::value::RuntimeProgressField::Label
                }
            },
        },
        CheckedSelectResolution::Field(selection) => {
            if let (Some(owner), Some(field)) = (closed_owner, selection.runtime_field()) {
                return Ok(Some(RuntimeResolvedSelect::Field { owner, field }));
            }
            if let Some(projection) =
                selection
                    .project_runtime_field(analysis)
                    .map_err(|source| RuntimeSemanticProjectionError::FieldProjection {
                        owner,
                        source,
                    })?
            {
                RuntimeResolvedSelect::Field {
                    owner: projection.owner().semantic_identity(),
                    field: projection.field().runtime_field(),
                }
            } else {
                runtime_opaque_environment_select(world, selection).ok_or(
                    RuntimeSemanticProjectionError::UnrepresentableEnvironmentRecordField {
                        owner: RuntimeRecordExecutableOwner::Expression(owner),
                        semantic_owner: selection.owner_type(),
                        ordinal: selection.declaration_ordinal(),
                    },
                )?
            }
        }
    }))
}

fn accepted_runtime_environment_field<'a>(
    world: &'a RegisteredSemanticWorld,
    selection: &arcweft_lang_sema::final_analysis::CheckedFieldSelection,
) -> Option<arcweft_lang_sema::env::nominal::AcceptedRuntimeEnvironmentFieldProjection<'a>> {
    world
        .environment()
        .nominal_catalog()
        .runtime_environment_field(
            selection.owner_type(),
            selection.declaration_ordinal(),
            selection.field_type(),
        )
}

fn runtime_opaque_environment_select(
    world: &RegisteredSemanticWorld,
    selection: &arcweft_lang_sema::final_analysis::CheckedFieldSelection,
) -> Option<RuntimeResolvedSelect> {
    let projection = accepted_runtime_environment_field(world, selection)?;
    let ordinal = usize::try_from(projection.field().ordinal()).ok()?;
    let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).ok()?;
    Some(RuntimeResolvedSelect::OpaqueRecord {
        owner: RuntimeSemanticTypeId::from_bytes(*projection.owner().semantic_type().as_bytes()),
        producer: projection.carrier().producer().clone(),
        field,
        field_type: RuntimeSemanticTypeId::from_bytes(*projection.field().type_digest().as_bytes()),
    })
}

fn runtime_nominal(
    nominal: &CheckedProjectNominal,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeResolvedNominal, RuntimeSemanticProjectionError> {
    let name = nominal.declaration().qualified_name();
    let projected = analysis
        .runtime_nominal_projection(nominal.identity())
        .ok_or_else(|| RuntimeSemanticProjectionError::NominalSchemaProjection {
            nominal: name,
            source: NominalSchemaProjectionError::MissingCachedProjection {
                semantic_type: nominal.identity(),
            },
        })?;
    if projected.declaration() != nominal.declaration() || projected.owner() != nominal.owner() {
        return Err(RuntimeSemanticProjectionError::NominalSchemaProjection {
            nominal: nominal.declaration().qualified_name(),
            source: NominalSchemaProjectionError::OwnerMismatch {
                nominal: nominal.declaration().qualified_name(),
                expected: projected.owner(),
                actual: nominal.owner(),
            },
        });
    }
    Ok(RuntimeResolvedNominal::new(
        nominal.declaration().clone(),
        nominal.owner(),
        projected.nominal().clone(),
        projected.semantic_identity(),
        projected.layout(),
    ))
}

fn runtime_nominal_under(
    nominal: &CheckedProjectNominal,
    analysis: &FinalSemanticAnalysis,
    instance: Option<ProjectInstanceTypes<'_>>,
) -> Result<RuntimeResolvedNominal, RuntimeSemanticProjectionError> {
    let closed = instance
        .map(|solution| solution.instantiate_project_nominal(nominal))
        .transpose()?;
    runtime_nominal(closed.as_ref().unwrap_or(nominal), analysis)
}

fn runtime_nominal_record(
    nominal: &CheckedProjectNominal,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeResolvedNominalRecord, RuntimeSemanticProjectionError> {
    runtime_nominal_record_under(nominal, symbols, world, analysis, None)
}

fn runtime_nominal_record_under(
    nominal: &CheckedProjectNominal,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instance: Option<ProjectInstanceTypes<'_>>,
) -> Result<RuntimeResolvedNominalRecord, RuntimeSemanticProjectionError> {
    let closed = instance
        .map(|solution| solution.instantiate_project_nominal(nominal))
        .transpose()?;
    let nominal = closed.as_ref().unwrap_or(nominal);
    let name = nominal.declaration().qualified_name();
    let projection = analysis
        .runtime_nominal_projection(nominal.identity())
        .ok_or_else(|| RuntimeSemanticProjectionError::NominalSchemaProjection {
            nominal: name.clone(),
            source: NominalSchemaProjectionError::MissingCachedProjection {
                semantic_type: nominal.identity(),
            },
        })?;
    let arcweft_data::TypeShape::Record { fields, .. } = projection.shape() else {
        return Err(RuntimeSemanticProjectionError::Type {
            reason: format!("checked nominal record `{name}` is not a struct"),
        });
    };
    if fields.len() != projection.record_fields().len() {
        return Err(RuntimeSemanticProjectionError::Type {
            reason: format!(
                "checked nominal record `{name}` has an incomplete cached field relation"
            ),
        });
    }
    let resolved = runtime_nominal(nominal, analysis)?;
    let projected_fields = fields
        .iter()
        .zip(projection.record_fields())
        .enumerate()
        .map(|(ordinal, (shape, field))| {
            if usize::try_from(field.declaration_ordinal()).ok() != Some(ordinal)
                || usize::try_from(field.runtime_field().zero_based()).ok() != Some(ordinal)
            {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: format!(
                        "checked nominal record `{name}` has a non-canonical cached field coordinate"
                    ),
                });
            }
            let normalized = runtime_type(field.ty(), symbols, world, analysis)?;
            let checked_type = normalized.checked_type().map_err(|reason| {
                RuntimeSemanticProjectionError::Type {
                    reason: reason.to_string(),
                }
            })?;
            Ok((shape.rust_name.clone(), normalized, checked_type))
        })
        .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?;
    let projected_arguments = nominal
        .arguments()
        .iter()
        .map(|argument| {
            runtime_type(argument, symbols, world, analysis)?
                .checked_type()
                .map_err(|reason| RuntimeSemanticProjectionError::Type {
                    reason: reason.to_string(),
                })
        })
        .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?;
    let layout = RuntimeNominalRecordLayout::try_from_checked_projection(
        resolved.runtime_nominal_id(),
        resolved.identity(),
        resolved.layout(),
        projected_arguments,
        projected_fields
            .iter()
            .map(|(name, _, checked)| (name.clone(), checked.clone()))
            .collect(),
    )
    .map(Arc::new)
    .map_err(
        |source| RuntimeSemanticProjectionError::NominalRecordLayout {
            nominal: name.clone(),
            source,
        },
    )?;
    RuntimeResolvedNominalRecord::try_new(
        resolved,
        layout,
        projected_fields
            .into_iter()
            .map(|(name, normalized, _)| (name, normalized)),
    )
    .map_err(|source| RuntimeSemanticProjectionError::NominalRecordFact {
        nominal: name,
        source,
    })
}

fn runtime_record_pattern(
    owner: PatternId,
    record: &CheckedRecordPattern,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeRecordPatternFact, RuntimeSemanticProjectionError> {
    runtime_record_pattern_under(owner, record, symbols, world, analysis, None)
}

fn runtime_record_pattern_under(
    owner: PatternId,
    record: &CheckedRecordPattern,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instance: Option<ProjectInstanceTypes<'_>>,
) -> Result<RuntimeRecordPatternFact, RuntimeSemanticProjectionError> {
    let fields = record
        .fields()
        .iter()
        .map(|field| {
            let runtime_field = match record.owner() {
                CheckedRecordPatternOwner::Project { .. } => field.runtime_field().ok_or(
                    RuntimeSemanticProjectionError::UnrepresentableEnvironmentRecordField {
                        owner: RuntimeRecordExecutableOwner::Pattern(owner),
                        semantic_owner: record.owner().semantic_type(),
                        ordinal: field.declaration_ordinal(),
                    },
                )?,
                CheckedRecordPatternOwner::VariantPayload { .. } => {
                    RuntimeRecordFieldId::try_from_zero_based_ordinal(
                        usize::try_from(field.declaration_ordinal()).map_err(|_| {
                            RuntimeSemanticProjectionError::UnrepresentableEnvironmentRecordField {
                                owner: RuntimeRecordExecutableOwner::Pattern(owner),
                                semantic_owner: record.owner().semantic_type(),
                                ordinal: field.declaration_ordinal(),
                            }
                        })?,
                    )
                    .map_err(|_| {
                        RuntimeSemanticProjectionError::UnrepresentableEnvironmentRecordField {
                            owner: RuntimeRecordExecutableOwner::Pattern(owner),
                            semantic_owner: record.owner().semantic_type(),
                            ordinal: field.declaration_ordinal(),
                        }
                    })?
                }
                CheckedRecordPatternOwner::Environment { .. } => {
                    return Err(
                        RuntimeSemanticProjectionError::UnrepresentableEnvironmentRecordField {
                            owner: RuntimeRecordExecutableOwner::Pattern(owner),
                            semantic_owner: record.owner().semantic_type(),
                            ordinal: field.declaration_ordinal(),
                        },
                    );
                }
            };
            let source = match field.source().value() {
                CheckedRecordPatternSourceRef::Pattern(pattern) => {
                    RuntimeRecordPatternSource::Pattern(pattern)
                }
                CheckedRecordPatternSourceRef::Binding(binding) => {
                    RuntimeRecordPatternSource::Binding(binding.raw())
                }
            };
            Ok(RuntimeRecordPatternField::new(runtime_field, source))
        })
        .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?
        .into_boxed_slice();
    let rest = match record.rest() {
        CheckedRecordPatternRest::Absent => RuntimeRecordPatternRest::Absent,
        CheckedRecordPatternRest::Ignore => RuntimeRecordPatternRest::Ignore,
        CheckedRecordPatternRest::Binding(binding) => {
            RuntimeRecordPatternRest::Binding(binding.raw())
        }
    };
    let projected = match record.owner() {
        CheckedRecordPatternOwner::Project { nominal, .. } => RuntimeRecordPatternFact::try_new(
            runtime_nominal_record_under(nominal, symbols, world, analysis, instance)?,
            fields,
            rest,
        ),
        CheckedRecordPatternOwner::VariantPayload {
            payload,
            semantic_type,
            ..
        } => {
            let open_ty = TypeKind::VariantPayload(Box::new(payload.to_type()));
            if open_ty.semantic_identity_digest()? != *semantic_type {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: "variant record payload identity is inconsistent".to_owned(),
                });
            }
            let ty = instance.map_or_else(
                || Ok(open_ty.clone()),
                |solution| solution.instantiate_type(&open_ty),
            )?;
            RuntimeRecordPatternFact::try_structural(
                runtime_type(&ty, symbols, world, analysis)?,
                fields,
                rest,
            )
        }
        CheckedRecordPatternOwner::Environment { .. } => {
            return Err(
                RuntimeSemanticProjectionError::UnrepresentableEnvironmentRecord {
                    owner: RuntimeRecordExecutableOwner::Pattern(owner),
                    semantic_owner: record.owner().semantic_type(),
                },
            );
        }
    };
    projected.map_err(|source| RuntimeSemanticProjectionError::RecordPlan {
        owner: RuntimeRecordExecutableOwner::Pattern(owner),
        source,
    })
}

fn runtime_assertion(
    owner: StmtId,
    disposition: CheckedAssertionDisposition,
) -> Result<RuntimeAssertionAdmission, RuntimeSemanticProjectionError> {
    match disposition {
        CheckedAssertionDisposition::PendingProof => {
            Err(RuntimeSemanticProjectionError::InvalidAssertionDisposition { owner })
        }
        CheckedAssertionDisposition::Discharged => Ok(RuntimeAssertionAdmission::Discharged),
        CheckedAssertionDisposition::OmittedDebug => Ok(RuntimeAssertionAdmission::OmittedDebug),
        CheckedAssertionDisposition::Runtime(AssertionRuntimePolicy::AlwaysGuard) => Ok(
            RuntimeAssertionAdmission::Runtime(RuntimeAssertionMode::Check),
        ),
        CheckedAssertionDisposition::Runtime(AssertionRuntimePolicy::DebugGuard) => Ok(
            RuntimeAssertionAdmission::Runtime(RuntimeAssertionMode::Debug),
        ),
    }
}

fn runtime_iteration(
    owner: StmtId,
    iteration: &CheckedIteration,
    methods: &BTreeMap<CheckedTraitConformance, ImplMethodDeclarationId>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeIteratorFact, RuntimeSemanticProjectionError> {
    runtime_iteration_under(owner, iteration, methods, symbols, world, analysis, None)
}

fn runtime_iteration_under(
    owner: StmtId,
    iteration: &CheckedIteration,
    methods: &BTreeMap<CheckedTraitConformance, ImplMethodDeclarationId>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    instance: Option<ProjectInstanceTypes<'_>>,
) -> Result<RuntimeIteratorFact, RuntimeSemanticProjectionError> {
    match iteration {
        CheckedIteration::Builtin { family, item } => {
            let (family, state_family) = match family {
                CheckedIteratorFamily::Range => (
                    RuntimeBuiltinIteratorFamily::Range,
                    IteratorStateKind::Range,
                ),
                CheckedIteratorFamily::Seq => {
                    (RuntimeBuiltinIteratorFamily::Seq, IteratorStateKind::Seq)
                }
                CheckedIteratorFamily::Stream => (
                    RuntimeBuiltinIteratorFamily::Stream,
                    IteratorStateKind::Stream,
                ),
                CheckedIteratorFamily::Vec => {
                    (RuntimeBuiltinIteratorFamily::Vec, IteratorStateKind::Vec)
                }
                CheckedIteratorFamily::Array => (
                    RuntimeBuiltinIteratorFamily::Array,
                    IteratorStateKind::Array,
                ),
                CheckedIteratorFamily::Slice => (
                    RuntimeBuiltinIteratorFamily::Slice,
                    IteratorStateKind::Slice,
                ),
            };
            let iterator = TypeKind::IteratorState {
                family: state_family,
                item: Box::new(item.clone()),
            };
            let next_value = TypeKind::Option(Box::new(item.clone()));
            let step = TypeKind::Tuple(vec![iterator.clone(), next_value.clone()]);
            Ok(RuntimeIteratorFact::Builtin(Box::new(
                RuntimeBuiltinIteratorFact::new(
                    family,
                    runtime_type_under(item, instance, symbols, world, analysis)?,
                    runtime_type_under(&iterator, instance, symbols, world, analysis)?,
                    runtime_type_under(&next_value, instance, symbols, world, analysis)?,
                    runtime_type_under(&step, instance, symbols, world, analysis)?,
                ),
            )))
        }
        CheckedIteration::Witness {
            item,
            into_iter,
            into_iterator,
            iterator,
            ..
        } => {
            Ok(RuntimeIteratorFact::Witness(Box::new(
                RuntimeIteratorWitnessFact::new(
                    runtime_type_under(item, instance, symbols, world, analysis)?,
                    runtime_type_under(into_iter, instance, symbols, world, analysis)?,
                    RuntimeIteratorWitnessExecutableFact::TraitCalls {
                        into_iter: methods.get(into_iterator).cloned().ok_or(
                            RuntimeSemanticProjectionError::MissingIterationMethod { owner },
                        )?,
                        next: methods.get(iterator).cloned().ok_or(
                            RuntimeSemanticProjectionError::MissingIterationMethod { owner },
                        )?,
                    },
                ),
            )))
        }
        CheckedIteration::IteratorWitness {
            source,
            item,
            iterator,
        } => Ok(RuntimeIteratorFact::Witness(Box::new(
            RuntimeIteratorWitnessFact::new(
                runtime_type_under(item, instance, symbols, world, analysis)?,
                runtime_type_under(source, instance, symbols, world, analysis)?,
                RuntimeIteratorWitnessExecutableFact::IdentityIntoIterator {
                    next: methods
                        .get(iterator)
                        .cloned()
                        .ok_or(RuntimeSemanticProjectionError::MissingIterationMethod { owner })?,
                },
            ),
        ))),
    }
}

fn runtime_iteration_methods(
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    view_value_owners: Option<&HirRuntimeSemanticReachability<'_>>,
) -> Result<BTreeMap<CheckedTraitConformance, TypeKind>, RuntimeSemanticProjectionError> {
    let mut methods = BTreeMap::new();
    let mut insert = |conformance: &CheckedTraitConformance, self_type: &TypeKind| match methods
        .insert(conformance.clone(), self_type.clone())
    {
        Some(existing) if existing != *self_type => {
            Err(RuntimeSemanticProjectionError::InconsistentIterationConformance)
        }
        _ => Ok(()),
    };
    for (owner, statement) in analysis.statements() {
        if !runtime_owners.contains_statement(owner)
            && !view_value_owners.is_some_and(|owners| owners.contains_statement(owner))
        {
            continue;
        }
        let CheckedStatementPayload::Iteration(iteration) = statement.payload() else {
            continue;
        };
        for (_, conformance, self_type) in iteration.witness_methods() {
            insert(conformance, self_type)?;
        }
    }
    Ok(methods)
}

fn runtime_trait_identity(identity: &CheckedTraitIdentity) -> RuntimeTraitIdentity {
    match identity {
        CheckedTraitIdentity::Project(owner) => RuntimeTraitIdentity::Project(*owner),
        CheckedTraitIdentity::StandardIterator => RuntimeTraitIdentity::StandardIterator,
        CheckedTraitIdentity::StandardIntoIterator => RuntimeTraitIdentity::StandardIntoIterator,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "checked non-call roots join Entry roles to the same closed ordinary Function instance graph as terminal calls"
)]
fn runtime_project_function_roots(
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    instances: &mut ProjectInstanceProjection<'_>,
) -> Result<Vec<RuntimeProjectFunctionRootFact>, RuntimeSemanticProjectionError> {
    let mut roots = Vec::new();
    for binding in analysis.checked_entries().entries() {
        let entry = binding.source_item();
        if !runtime_owners.contains_runtime_owner(&HirRuntimeExecutableOwner::Item(entry)) {
            continue;
        }
        let roles: &[(&CheckedCallableRole, RuntimeProjectFunctionRootRole)] = match binding {
            CheckedEntryBinding::Stateful(checked) => &[
                (
                    checked.initializer(),
                    RuntimeProjectFunctionRootRole::EntryInitializer,
                ),
                (
                    checked.reducer(),
                    RuntimeProjectFunctionRootRole::EntryReducer,
                ),
            ],
            CheckedEntryBinding::Agent(checked) => &[((
                checked.controller(),
                RuntimeProjectFunctionRootRole::EntryController,
            ))],
            CheckedEntryBinding::Existing(_) => &[],
        };
        for (role, root_role) in roles {
            let origin = ProjectInstantiationOrigin::Root(entry);
            let declaration = CallableDeclarationKey::Existing(role.declaration().clone());
            let selection =
                select_project_function_root_runtime(&declaration, analysis.checked_callables())
                    .map_err(|error| origin.error(error.to_string()))?;
            let callable =
                runtime_project_callable(selection.declaration(), symbols, world, analysis)
                    .map_err(|reason| origin.error(reason))?;
            if !runtime_owners
                .contains_runtime_owner(&HirRuntimeExecutableOwner::Item(callable.owner()))
            {
                return Err(
                    origin.error("checked Entry callable is absent from runtime reachability")
                );
            }
            let key = RuntimeProjectFunctionInstanceKey::new(
                callable.runtime().clone(),
                selection.solution().instantiation(),
                selection.group(),
            );
            ensure_runtime_project_function_instance(
                origin,
                key.clone(),
                callable,
                ProjectInstanceSelection::from_root(&selection),
                selection.solution().clone(),
                instances,
            )?;
            roots.push(RuntimeProjectFunctionRootFact::new(entry, *root_role, key));
        }
    }
    Ok(roots)
}

fn ordinary_function_runtime_call_owners(
    project: HirAnalysisProjectView<'_>,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    execution: &arcweft_lang_sema::final_analysis::FinalAnalysisExecutionProjection<'_>,
) -> Result<BTreeSet<ExprId>, RuntimeSemanticProjectionError> {
    let mut calls = BTreeSet::new();
    for executable in runtime_owners.reachable_executables() {
        let HirRuntimeExecutableOwner::Item(owner) = executable else {
            continue;
        };
        let module = project
            .modules()
            .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module))
            .ok_or_else(|| RuntimeSemanticProjectionError::Type {
                reason: "runtime executable owner module is absent".to_owned(),
            })?;
        let item =
            module
                .resolve_item(*owner)
                .map_err(|_| RuntimeSemanticProjectionError::Type {
                    reason: "runtime executable owner item is absent".to_owned(),
                })?;
        if !matches!(item.kind(), HirItemKind::Function(_)) {
            continue;
        }
        let partition = execution.runtime_fact_partition(runtime_owners, executable)?;
        for row in partition.expressions().iter().filter(|row| {
            row.family()
                == arcweft_lang_sema::final_analysis::CheckedExecutableRuntimeExpressionFactFamily::Call
        }) {
            if !calls.insert(row.owner()) {
                return Err(RuntimeSemanticProjectionError::Call {
                    owner: row.owner(),
                    reason: "runtime call belongs to more than one ordinary Function partition"
                        .to_owned(),
                });
            }
        }
    }
    Ok(calls)
}

fn runtime_call(
    owner: ExprId,
    facts: &CallTargetFacts,
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    enclosing: Option<ProjectInstanceTypes<'_>>,
    project_function_instances: &mut ProjectInstanceProjection<'_>,
) -> Result<RuntimeResolvedCall, RuntimeSemanticProjectionError> {
    if facts.expression() != owner {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: "call fact owner differs from the final-HIR call owner".to_owned(),
        });
    }
    let Some(application) = facts.selected_application() else {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: "unselected call evidence cannot enter runtime lowering".to_owned(),
        });
    };
    let callable_join = analysis.checked_callable_join(owner).map_err(|error| {
        RuntimeSemanticProjectionError::Call {
            owner,
            reason: format!("runtime call has no exact checked callable join: {error}"),
        }
    })?;
    let project_function_selection =
        select_project_function_runtime(application, callable_join, analysis.checked_callables())
            .map_err(|error| RuntimeSemanticProjectionError::Call {
            owner,
            reason: error.to_string(),
        })?;
    let project_function_callable = project_function_selection
        .as_ref()
        .map(|selection| {
            runtime_project_callable(selection.declaration(), symbols, world, analysis)
                .map_err(|reason| RuntimeSemanticProjectionError::Call { owner, reason })
        })
        .transpose()?;
    let dispatch = if let Some(line) =
        runtime_line_callable(owner, application, analysis, enclosing)?
    {
        RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Line(line))
    } else if let (Some(selection), Some(callable)) = (
        project_function_selection.as_ref(),
        project_function_callable.as_ref(),
    ) {
        match selection.input() {
            CheckedProjectFunctionRuntimeInput::Direct => RuntimeResolvedCallDispatch::Static(
                RuntimeResolvedStaticCallTarget::Declaration(callable.clone()),
            ),
            CheckedProjectFunctionRuntimeInput::Continuation { .. } => {
                let CheckedCallCalleeExecution::Value { source } = application.core().callee()
                else {
                    return Err(RuntimeSemanticProjectionError::Call {
                        owner,
                        reason: "checked project continuation has no value callee".to_owned(),
                    });
                };
                let arcweft_lang_sema::callable::CheckedCallArgumentSlotSource::Expression(callee) =
                    source.raw()
                else {
                    return Err(RuntimeSemanticProjectionError::Call {
                        owner,
                        reason: "checked project continuation callee is not an expression"
                            .to_owned(),
                    });
                };
                RuntimeResolvedCallDispatch::Value { callee }
            }
        }
    } else {
        match application.core().callee() {
            CheckedCallCalleeExecution::Direct => {
                RuntimeResolvedCallDispatch::Static(runtime_call_target(
                    owner,
                    application,
                    project,
                    symbols,
                    world,
                    analysis,
                    enclosing,
                )?)
            }
            CheckedCallCalleeExecution::Value { source } => {
                let arcweft_lang_sema::callable::CheckedCallArgumentSlotSource::Expression(callee) =
                    source.raw()
                else {
                    return Err(RuntimeSemanticProjectionError::Call {
                        owner,
                        reason: "checked value dispatch has a non-expression callee source"
                            .to_owned(),
                    });
                };
                RuntimeResolvedCallDispatch::Value { callee }
            }
        }
    };
    let selected = application.core().candidates().selected();
    let mut operands = Vec::new();
    let mut positioned_attached_content = None;
    for operand in application.core().runtime_operands() {
        match operand {
            CheckedCallRuntimeOperand::Receiver {
                ty,
                source,
                abi_position,
                ..
            } => operands.push(RuntimeResolvedCallOperand::new(
                abi_position,
                RuntimeResolvedCallOperandOrigin::Receiver,
                runtime_call_operand_source(source.raw()),
                runtime_type_under(ty, enclosing, symbols, world, analysis)?,
                RuntimeResolvedCallOperandBinding::Positional,
                RuntimeResolvedCallOperandProjection::Scalar,
                None,
            )),
            CheckedCallRuntimeOperand::Argument {
                argument,
                passing,
                slot,
            } => operands.push(RuntimeResolvedCallOperand::new(
                slot.abi_position(),
                RuntimeResolvedCallOperandOrigin::Argument {
                    argument: u32::from(argument.get()),
                    slot: u32::try_from(slot.slot().get()).map_err(|_| {
                        RuntimeSemanticProjectionError::Call {
                            owner,
                            reason: "call argument slot exceeds the runtime u32 coordinate"
                                .to_owned(),
                        }
                    })?,
                },
                runtime_call_operand_source(slot.source().raw()),
                runtime_type_under(slot.inferred(), enclosing, symbols, world, analysis)?,
                runtime_call_operand_binding(owner, selected, passing, slot)?,
                runtime_call_operand_projection(owner, slot, symbols, world, analysis, enclosing)?,
                match slot.destination() {
                    CheckedCallOperandDestination::Parameter(coordinate) => {
                        Some(RuntimeCallParameterCoordinate::new(
                            u32::try_from(coordinate.group().get()).map_err(|_| {
                                RuntimeSemanticProjectionError::Call {
                                    owner,
                                    reason:
                                        "call parameter group exceeds the runtime u32 coordinate"
                                            .to_owned(),
                                }
                            })?,
                            u32::try_from(coordinate.parameter().get()).map_err(|_| {
                                RuntimeSemanticProjectionError::Call {
                                    owner,
                                    reason:
                                        "call parameter index exceeds the runtime u32 coordinate"
                                            .to_owned(),
                                }
                            })?,
                        ))
                    }
                    CheckedCallOperandDestination::Open(_) => None,
                },
            )),
            CheckedCallRuntimeOperand::AttachedContent {
                source,
                presence,
                ty,
                abi_position,
            } => {
                let source = source.map(|source| source.raw().owner());
                let ty = runtime_type_under(ty, enclosing, symbols, world, analysis)?;
                let content = match (presence, source) {
                    (CallableParameterPresence::Required, Some(source)) => {
                        RuntimeResolvedAttachedContent::Required { source, ty }
                    }
                    (CallableParameterPresence::Optional, Some(source)) => {
                        RuntimeResolvedAttachedContent::OptionalPresent { source, ty }
                    }
                    (CallableParameterPresence::Optional, None) => {
                        RuntimeResolvedAttachedContent::OptionalOmitted { ty }
                    }
                    (CallableParameterPresence::Defaulted, Some(source)) => {
                        RuntimeResolvedAttachedContent::DefaultedPresent { source, ty }
                    }
                    (CallableParameterPresence::Defaulted, None) => {
                        RuntimeResolvedAttachedContent::DefaultedOmitted { ty }
                    }
                    (CallableParameterPresence::Required, None) => {
                        return Err(RuntimeSemanticProjectionError::Call {
                            owner,
                            reason: "required runtime attached content has no checked source"
                                .to_owned(),
                        });
                    }
                };
                if positioned_attached_content
                    .replace(RuntimePositionedAttachedContent::new(abi_position, content))
                    .is_some()
                {
                    return Err(RuntimeSemanticProjectionError::Call {
                        owner,
                        reason: "runtime call contains more than one attached-content ABI member"
                            .to_owned(),
                    });
                }
            }
        }
    }
    let result = match application.result() {
        arcweft_lang_sema::callable::CheckedCallResult::Value(_) => RuntimeCallResultShape::Value,
        arcweft_lang_sema::callable::CheckedCallResult::Continuation(_) => {
            RuntimeCallResultShape::PartialFunction
        }
        arcweft_lang_sema::callable::CheckedCallResult::ContentEmission(_) => {
            return Err(RuntimeSemanticProjectionError::Call {
                owner,
                reason: "content emission call reached ordinary runtime call lowering".to_owned(),
            });
        }
    };
    let project_function = match (
        project_function_selection.as_ref(),
        project_function_callable,
    ) {
        (Some(selection), Some(callable)) => {
            let plan = runtime_project_function_projection(
                owner,
                application,
                selection,
                callable,
                project,
                symbols,
                world,
                analysis,
                enclosing,
                project_function_instances,
            )?;
            Some(plan)
        }
        (None, None) => None,
        _ => {
            return Err(RuntimeSemanticProjectionError::Call {
                owner,
                reason: "project-function selection and callable descriptor disagree".to_owned(),
            });
        }
    };
    let call = RuntimeResolvedCall::try_new(
        dispatch,
        application.core().current_group(),
        operands,
        positioned_attached_content,
        project_function,
        result,
    )
    .map_err(|error| RuntimeSemanticProjectionError::Call {
        owner,
        reason: error.to_string(),
    })?;
    Ok(call)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the compiler projection atomically joins one checked call, callable descriptor, and optional terminal instance"
)]
fn runtime_project_function_projection(
    owner: ExprId,
    application: &CheckedCallApplication,
    selection: &CheckedProjectFunctionRuntimeSelection,
    callable: RuntimeProjectCallable,
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    enclosing: Option<ProjectInstanceTypes<'_>>,
    project_function_instances: &mut ProjectInstanceProjection<'_>,
) -> Result<RuntimeProjectFunctionCallPlan, RuntimeSemanticProjectionError> {
    let input = match selection.input() {
        CheckedProjectFunctionRuntimeInput::Direct => RuntimeProjectFunctionCallInput::Direct,
        CheckedProjectFunctionRuntimeInput::Continuation { abi } => {
            let CheckedCallCalleeExecution::Value { source } = application.core().callee() else {
                return Err(RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "checked project continuation has no value callee".to_owned(),
                });
            };
            let arcweft_lang_sema::callable::CheckedCallArgumentSlotSource::Expression(callee) =
                source.raw()
            else {
                return Err(RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "checked project continuation callee is not an expression".to_owned(),
                });
            };
            RuntimeProjectFunctionCallInput::Continuation {
                callee,
                abi: runtime_project_continuation_abi(
                    owner, abi, enclosing, symbols, world, analysis,
                )?,
            }
        }
    };
    let outcome = match selection.outcome() {
        CheckedProjectFunctionRuntimeOutcome::Continue { abi, next_group } => {
            RuntimeProjectFunctionCallOutcome::Continue {
                abi: runtime_project_continuation_abi(
                    owner, abi, enclosing, symbols, world, analysis,
                )?,
                next_group: *next_group,
            }
        }
        CheckedProjectFunctionRuntimeOutcome::Invoke { .. } => {
            let instance_solution = project_function_instances.close_instance(
                ProjectInstantiationOrigin::Call(owner),
                selection,
                enclosing.map(ProjectInstanceTypes::solution),
            )?;
            let key = RuntimeProjectFunctionInstanceKey::new(
                callable.runtime().clone(),
                instance_solution.instantiation(),
                selection.group(),
            );
            ensure_runtime_project_function_instance(
                ProjectInstantiationOrigin::Call(owner),
                key.clone(),
                callable.clone(),
                ProjectInstanceSelection::from_call(selection),
                instance_solution,
                project_function_instances,
            )?;
            RuntimeProjectFunctionCallOutcome::Invoke { instance: key }
        }
    };
    let current_group_materialization = runtime_project_function_materialization(
        owner, &callable, selection, project, symbols, world, analysis, enclosing,
    )?;
    let plan = RuntimeProjectFunctionCallPlan::try_new(
        callable,
        selection.group(),
        current_group_materialization,
        input,
        outcome,
    )
    .map_err(|error| RuntimeSemanticProjectionError::Call {
        owner,
        reason: error.to_string(),
    })?;
    Ok(plan)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the compiler projection joins one checked materialization row to its exact final-HIR parameter and normalized types"
)]
fn runtime_project_function_materialization(
    owner: ExprId,
    callable: &RuntimeProjectCallable,
    selection: &CheckedProjectFunctionRuntimeSelection,
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    enclosing: Option<ProjectInstanceTypes<'_>>,
) -> Result<Box<[RuntimeProjectFunctionParameterMaterialization]>, RuntimeSemanticProjectionError> {
    let error = |reason: &str| RuntimeSemanticProjectionError::Call {
        owner,
        reason: reason.to_owned(),
    };
    let module = project
        .modules()
        .find_map(|(_, module)| (module.module_id() == callable.owner().module()).then_some(module))
        .ok_or_else(|| error("project-function materialization owner module is absent"))?;
    let item = module
        .resolve_item(callable.owner())
        .map_err(|_| error("project-function materialization owner item is absent"))?;
    let HirItemKind::Function(function) = item.kind() else {
        return Err(error(
            "project-function materialization owner is not an ordinary Function",
        ));
    };
    let group = function
        .parameter_groups()
        .get(selection.group().get())
        .ok_or_else(|| error("project-function materialization group is absent"))?;
    let checked = selection.current_group_materialization();
    if group.parameters().len() != checked.len() {
        return Err(error(
            "project-function materialization row does not cover its HIR group",
        ));
    }
    let mut result = Vec::with_capacity(checked.len());
    for (parameter_index, (parameter, checked)) in
        group.parameters().iter().zip(checked).enumerate()
    {
        let kind = parameter.kind();
        if checked.coordinate().group() != selection.group()
            || checked.coordinate().parameter().get() != parameter_index
            || (parameter.kind() == HirParameterKind::RestPositional)
                != (checked.passing()
                    == arcweft_lang_sema::callable::CallableParameterPassing::RestPositional)
        {
            return Err(error(
                "checked project-function materialization disagrees with its HIR parameter",
            ));
        }
        if parameter.default().is_some() {
            return Err(error(
                "runtime project-function parameter default bypassed semantic surface admission",
            ));
        }
        let source = checked.operand_indices().to_vec().into_boxed_slice();
        let parameter = u32::try_from(parameter_index)
            .map_err(|_| error("project-function materialization parameter exceeds u32"))?;
        result.push(
            RuntimeProjectFunctionParameterMaterialization::try_new(
                selection.group(),
                parameter,
                kind,
                runtime_type_under(checked.abi_type(), enclosing, symbols, world, analysis)?,
                runtime_type_under(checked.binding_type(), enclosing, symbols, world, analysis)?,
                source,
            )
            .map_err(|reason| RuntimeSemanticProjectionError::Call {
                owner,
                reason: reason.to_string(),
            })?,
        );
    }
    Ok(result.into_boxed_slice())
}

fn runtime_project_continuation_abi(
    owner: ExprId,
    checked: &arcweft_lang_sema::callable::CheckedProjectContinuationRuntimeAbi,
    enclosing: Option<ProjectInstanceTypes<'_>>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeProjectContinuationAbi, RuntimeSemanticProjectionError> {
    let function_type =
        runtime_type_under(checked.function_type(), enclosing, symbols, world, analysis)?;
    let prefix_types = checked
        .prefix_types()
        .iter()
        .map(|ty| runtime_type_under(ty, enclosing, symbols, world, analysis))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    RuntimeProjectContinuationAbi::try_new(
        checked.lineage().runtime_lineage_id(),
        function_type,
        prefix_types,
    )
    .map_err(|error| RuntimeSemanticProjectionError::Call {
        owner,
        reason: error.to_string(),
    })
}

fn ensure_runtime_project_function_instance(
    origin: ProjectInstantiationOrigin,
    key: RuntimeProjectFunctionInstanceKey,
    callable: RuntimeProjectCallable,
    selection: ProjectInstanceSelection,
    instance_solution: CheckedProjectFunctionInstanceSolution,
    instances: &mut ProjectInstanceProjection<'_>,
) -> Result<(), RuntimeSemanticProjectionError> {
    let node = ProjectInstanceNode {
        origin,
        callable,
        selection,
        solution: instance_solution,
    };
    instances.request(key, node)?;
    Ok(())
}

fn discover_runtime_project_function_instances(
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    instances: &mut ProjectInstantiationSession,
) -> Result<(), RuntimeSemanticProjectionError> {
    while let Some(work) = instances.next()? {
        discover_runtime_project_function_instance_dependencies(
            work.node(),
            symbols,
            world,
            analysis,
            runtime_owners,
            &mut ProjectInstanceProjection::Discover(instances),
        )?;
        instances.complete(work)?;
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "closed instance discovery enqueues exact checked project-call selections from the executable partitions"
)]
fn discover_runtime_project_function_instance_dependencies(
    node: &ProjectInstanceNode,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    instances: &mut ProjectInstanceProjection<'_>,
) -> Result<(), RuntimeSemanticProjectionError> {
    let executable = HirRuntimeExecutableOwner::Item(node.callable.owner());
    let mut visited = BTreeSet::new();
    discover_runtime_project_executable_dependencies(
        &executable,
        &node.solution,
        symbols,
        world,
        analysis,
        runtime_owners,
        instances,
        &mut visited,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "dependency discovery owns one exact executable partition walk and its nested closure partitions"
)]
fn discover_runtime_project_executable_dependencies(
    executable: &HirRuntimeExecutableOwner,
    enclosing: &CheckedProjectFunctionInstanceSolution,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    instances: &mut ProjectInstanceProjection<'_>,
    visited: &mut BTreeSet<HirRuntimeExecutableOwner>,
) -> Result<(), RuntimeSemanticProjectionError> {
    let mut pending = BTreeSet::from([executable.clone()]);
    while let Some(executable) = pending.pop_first() {
        if !visited.insert(executable.clone()) {
            continue;
        }
        let partition = analysis
            .execution_projection()
            .runtime_fact_partition(runtime_owners, &executable)?;
        for row in partition.expressions() {
            match row.family() {
                CheckedExecutableRuntimeExpressionFactFamily::Call => {
                    let owner = row.owner();
                    let facts = analysis.call(owner).ok_or_else(|| {
                        RuntimeSemanticProjectionError::Call {
                            owner,
                            reason: "discovered instance call has no checked call fact".to_owned(),
                        }
                    })?;
                    let application = facts.selected_application().ok_or_else(|| {
                        RuntimeSemanticProjectionError::Call {
                            owner,
                            reason: "discovered instance call has no selected application"
                                .to_owned(),
                        }
                    })?;
                    let join = analysis.checked_callable_join(owner).map_err(|error| {
                    RuntimeSemanticProjectionError::Call {
                        owner,
                        reason: format!(
                            "discovered instance call has no exact checked callable join: {error}"
                        ),
                    }
                })?;
                    let Some(selection) = select_project_function_runtime(
                        application,
                        join,
                        analysis.checked_callables(),
                    )
                    .map_err(|error| RuntimeSemanticProjectionError::Call {
                        owner,
                        reason: error.to_string(),
                    })?
                    else {
                        continue;
                    };
                    if !matches!(
                        selection.outcome(),
                        CheckedProjectFunctionRuntimeOutcome::Invoke { .. }
                    ) {
                        continue;
                    }
                    let solution = instances.close_instance(
                        ProjectInstantiationOrigin::Call(owner),
                        &selection,
                        Some(enclosing),
                    )?;
                    let callable =
                        runtime_project_callable(selection.declaration(), symbols, world, analysis)
                            .map_err(|reason| RuntimeSemanticProjectionError::Call {
                                owner,
                                reason,
                            })?;
                    let key = RuntimeProjectFunctionInstanceKey::new(
                        callable.runtime().clone(),
                        solution.instantiation(),
                        selection.group(),
                    );
                    ensure_runtime_project_function_instance(
                        ProjectInstantiationOrigin::Call(owner),
                        key,
                        callable,
                        ProjectInstanceSelection::from_call(&selection),
                        solution,
                        instances,
                    )?;
                }
                CheckedExecutableRuntimeExpressionFactFamily::Closure => {
                    pending.insert(HirRuntimeExecutableOwner::Closure(row.owner()));
                }
                CheckedExecutableRuntimeExpressionFactFamily::Structural
                | CheckedExecutableRuntimeExpressionFactFamily::Consumed
                | CheckedExecutableRuntimeExpressionFactFamily::Literal
                | CheckedExecutableRuntimeExpressionFactFamily::Value
                | CheckedExecutableRuntimeExpressionFactFamily::Select
                | CheckedExecutableRuntimeExpressionFactFamily::NominalRecord
                | CheckedExecutableRuntimeExpressionFactFamily::Variant
                | CheckedExecutableRuntimeExpressionFactFamily::PostfixCandidate
                | CheckedExecutableRuntimeExpressionFactFamily::Await
                | CheckedExecutableRuntimeExpressionFactFamily::Choice
                | CheckedExecutableRuntimeExpressionFactFamily::Try
                | CheckedExecutableRuntimeExpressionFactFamily::ImplicitCallable
                | CheckedExecutableRuntimeExpressionFactFamily::Pipe
                | CheckedExecutableRuntimeExpressionFactFamily::DialogueApplication
                | CheckedExecutableRuntimeExpressionFactFamily::ContentApplication => {}
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "one post-discovery transaction materializes every closed instance from the same sealed graph"
)]
fn materialize_runtime_project_function_instances(
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    dialogue: &RuntimeDialogueProjectionCatalog,
    instances: &DiscoveredProjectInstances,
) -> Result<Vec<RuntimeProjectFunctionInstanceFact>, RuntimeSemanticProjectionError> {
    let mut complete = Vec::new();
    for (key, node) in instances.nodes() {
        let instance = build_runtime_project_function_instance(
            node.origin,
            key.clone(),
            node.callable.clone(),
            &node.selection,
            instances.types(node),
            project,
            symbols,
            world,
            analysis,
            runtime_owners,
            dialogue,
            &mut ProjectInstanceProjection::Materialize {
                graph: instances,
                caller: Some(key),
            },
        )?;
        complete.push(instance);
    }
    Ok(complete)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one closed instance projection needs the accepted HIR, symbols, semantic world, and frozen callable selection"
)]
fn build_runtime_project_function_instance(
    origin: ProjectInstantiationOrigin,
    key: RuntimeProjectFunctionInstanceKey,
    callable: RuntimeProjectCallable,
    selection: &ProjectInstanceSelection,
    instance_solution: ProjectInstanceTypes<'_>,
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    dialogue: &RuntimeDialogueProjectionCatalog,
    instances: &mut ProjectInstanceProjection<'_>,
) -> Result<RuntimeProjectFunctionInstanceFact, RuntimeSemanticProjectionError> {
    let error = |reason: &str| origin.error(reason);
    let module = project
        .modules()
        .find_map(|(_, module)| (module.module_id() == callable.owner().module()).then_some(module))
        .ok_or_else(|| error("project-function instance owner module is absent"))?;
    let item = module
        .resolve_item(callable.owner())
        .map_err(|_| error("project-function instance owner item is absent"))?;
    let HirItemKind::Function(function) = item.kind() else {
        return Err(error(
            "project-function instance owner is not an ordinary Function",
        ));
    };
    function
        .parameter_groups()
        .get(selection.group.get())
        .ok_or_else(|| error("project-function instance group is absent"))?;
    let mut parameters = Vec::new();
    let mut prefix_position = 0_u32;
    for (group_index, group) in function
        .parameter_groups()
        .iter()
        .enumerate()
        .take(selection.group.get() + 1)
    {
        let group_coordinate =
            arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(group_index)
                .map_err(|_| error("project-function group exceeds checked limits"))?;
        for (position, parameter) in group.parameters().iter().enumerate() {
            let parameter_coordinate = u32::try_from(position)
                .map_err(|_| error("project-function parameter ABI exceeds u32"))?;
            let source = if group_coordinate == selection.group {
                RuntimeProjectFunctionParameterSource::CurrentGroup {
                    position: parameter_coordinate,
                }
            } else {
                let source = RuntimeProjectFunctionParameterSource::ContinuationPrefix {
                    position: prefix_position,
                };
                prefix_position = prefix_position
                    .checked_add(1)
                    .ok_or_else(|| error("project-function continuation prefix exceeds u32"))?;
                source
            };
            let abi_ty = analysis
                .ty(parameter.ty())
                .ok_or_else(|| error("project-function parameter has no checked source type"))?;
            let abi_ty = instance_solution.instantiate_type(abi_ty)?;
            let binding_ty = analysis
                .pattern(parameter.pattern())
                .ok_or_else(|| error("project-function parameter has no checked binding type"))?;
            let binding_ty = instance_solution.instantiate_type(binding_ty.ty())?;
            let expected_binding_ty = if parameter.kind() == HirParameterKind::RestPositional {
                TypeKind::Vec(Box::new(abi_ty.clone()))
            } else {
                abi_ty.clone()
            };
            if binding_ty != expected_binding_ty {
                return Err(error(
                    "project-function parameter binding type disagrees with its ABI kind",
                ));
            }
            parameters.push(RuntimeProjectFunctionParameterAbi::new(
                group_coordinate,
                parameter_coordinate,
                source,
                parameter.pattern(),
                parameter.ty(),
                parameter.kind(),
                parameter.locals().to_vec().into_boxed_slice(),
                runtime_type(&abi_ty, symbols, world, analysis)?,
                runtime_type(&binding_ty, symbols, world, analysis)?,
            ));
        }
    }
    let arcweft_lang_hir::item::HirFunctionBody::Block {
        scope,
        statements,
        tail,
    } = function.body()
    else {
        return Err(error("project-function instance body is poisoned"));
    };
    let body = RuntimeProjectFunctionBody::new(*scope, statements.clone(), *tail);

    let semantic_owners = runtime_owners
        .executable_owners(&HirRuntimeExecutableOwner::Item(callable.owner()))
        .ok_or_else(|| error("project-function has no exact runtime semantic owner partition"))?;
    let executable = HirRuntimeExecutableOwner::Item(callable.owner());
    let fact_partition = analysis
        .execution_projection()
        .runtime_fact_partition(runtime_owners, &executable)?;
    let mut type_projection = Vec::new();
    for expected in fact_partition.expressions() {
        let owner = expected.owner();
        if !expected.has_runtime_type() {
            type_projection
                .push(RuntimeProjectFunctionTypeProjection::semantic_only_expression(owner));
            continue;
        }
        let checked = analysis
            .expression(owner)
            .ok_or_else(|| error("project-function expression has no checked semantic fact"))?;
        let ty = checked
            .value_type()
            .ok_or_else(|| error("runtime expression has no checked value type"))?;
        let ty = instance_solution.instantiate_type(ty)?;
        type_projection.push(RuntimeProjectFunctionTypeProjection::value(
            RuntimeProjectFunctionTypeOwner::Expression(owner),
            runtime_type(&ty, symbols, world, analysis)?,
        ));
    }
    for owner in semantic_owners.patterns() {
        let checked = analysis
            .pattern(owner)
            .ok_or_else(|| error("project-function pattern has no checked semantic fact"))?;
        let ty = instance_solution.instantiate_type(checked.ty())?;
        type_projection.push(RuntimeProjectFunctionTypeProjection::value(
            RuntimeProjectFunctionTypeOwner::Pattern(owner),
            runtime_type(&ty, symbols, world, analysis)?,
        ));
    }
    for owner in semantic_owners.locals() {
        let checked = analysis
            .local(owner)
            .ok_or_else(|| error("project-function local has no checked semantic fact"))?;
        let ty = instance_solution.instantiate_type(checked.ty())?;
        type_projection.push(RuntimeProjectFunctionTypeProjection::value(
            RuntimeProjectFunctionTypeOwner::Local(owner),
            runtime_type(&ty, symbols, world, analysis)?,
        ));
    }
    for owner in semantic_owners.types() {
        let checked = analysis.ty(owner).ok_or_else(|| {
            error(&format!(
                "project-function type root {owner:?} has no checked semantic fact"
            ))
        })?;
        let ty = instance_solution.instantiate_type(checked)?;
        type_projection.push(RuntimeProjectFunctionTypeProjection::value(
            RuntimeProjectFunctionTypeOwner::Type(owner),
            runtime_type(&ty, symbols, world, analysis)?,
        ));
    }
    type_projection.sort_by_key(RuntimeProjectFunctionTypeProjection::owner);

    let checked_callable = analysis
        .checked_callables()
        .project_callable(&selection.declaration)
        .map_err(|_| error("project-function owner has no checked callable role"))?;
    let execution = match checked_callable
        .ordinary_function_emission_with_effects(&selection.effects)
        .ok_or_else(|| error("project-function owner has no ordinary emission role"))?
    {
        CheckedOrdinaryFunctionEmission::ExpressionFunctionSite => {
            RuntimeProjectFunctionExecution::ExpressionFunctionSite
        }
        CheckedOrdinaryFunctionEmission::ExecutableFunctionSite => {
            RuntimeProjectFunctionExecution::ExecutableFunctionSite
        }
        CheckedOrdinaryFunctionEmission::StreamFactoryUnsupported => {
            return Err(error(
                "stream factory cannot publish an ordinary function instance",
            ));
        }
    };
    let function_type = runtime_type(instance_solution.function_type(), symbols, world, analysis)?;
    let attached_default = runtime_project_attached_default(
        origin,
        &callable,
        selection,
        instance_solution,
        &parameters,
        symbols,
        world,
        analysis,
    )?;
    let semantics = runtime_project_function_instance_semantic_facts(
        origin,
        RuntimeExecutableInstantiation::Project {
            key: &key,
            solution: instance_solution,
        },
        fact_partition,
        type_projection.into_boxed_slice(),
        project,
        symbols,
        world,
        analysis,
        runtime_owners,
        dialogue,
        instances,
    )?;
    RuntimeProjectFunctionInstanceFact::try_new(
        key,
        callable,
        checked_callable.suspension(),
        checked_callable.control(),
        execution,
        function_type,
        parameters.into_boxed_slice(),
        selection.effects.iter().cloned().collect(),
        attached_default,
        body,
        semantics,
    )
    .map_err(|reason| origin.error(reason.to_string()))
}

#[allow(
    clippy::too_many_arguments,
    reason = "one closed semantic subcatalog projects every family under the same frozen instance and SCC authority"
)]
fn runtime_project_function_instance_semantic_facts(
    origin: ProjectInstantiationOrigin,
    lexical: RuntimeExecutableInstantiation<'_>,
    partition: arcweft_lang_sema::final_analysis::CheckedExecutableRuntimeFactPartition,
    type_projection: Box<[RuntimeProjectFunctionTypeProjection]>,
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    dialogue: &RuntimeDialogueProjectionCatalog,
    instances: &mut ProjectInstanceProjection<'_>,
) -> Result<RuntimeProjectFunctionInstanceSemanticFacts, RuntimeSemanticProjectionError> {
    let error = |owner: ExprId, reason: &str| RuntimeSemanticProjectionError::Call {
        owner,
        reason: reason.to_owned(),
    };
    let module = project
        .modules()
        .find_map(|(_, module)| {
            partition
                .expressions()
                .first()
                .map_or_else(
                    || partition.patterns().first().map(|row| row.owner().module()),
                    |row| Some(row.owner().module()),
                )
                .is_some_and(|owner| module.module_id() == owner)
                .then_some(module)
        })
        .ok_or_else(|| origin.error("instance semantic partition module is absent"))?;
    let projected_types = type_projection
        .iter()
        .filter_map(|row| row.ty().map(|ty| (row.owner(), ty)))
        .collect::<BTreeMap<_, _>>();
    let execution = analysis.execution_projection();
    let mut expressions = Vec::with_capacity(partition.expressions().len());
    for expected in partition.expressions() {
        let owner = expected.owner();
        let checked = analysis
            .expression(owner)
            .ok_or_else(|| error(owner, "instance expression has no checked semantic fact"))?;
        let hir = module
            .resolve_expr(owner)
            .map_err(|_| error(owner, "instance expression is absent from its HIR module"))?;
        let payload = match expected.family() {
            CheckedExecutableRuntimeExpressionFactFamily::Structural => {
                RuntimeProjectFunctionExpressionPayload::Structural
            }
            CheckedExecutableRuntimeExpressionFactFamily::Consumed => {
                RuntimeProjectFunctionExpressionPayload::Consumed
            }
            CheckedExecutableRuntimeExpressionFactFamily::Literal => {
                let checked_ty = checked.value_type().ok_or_else(|| {
                    error(owner, "instance literal has no checked runtime value type")
                })?;
                let closed_ty = lexical.instantiate_type(checked_ty)?;
                let value = match (checked.resolution(), hir.kind()) {
                    (CheckedExpressionResolution::Literal(literal), _) => {
                        runtime_literal(literal, &closed_ty).map_err(|reason| {
                            RuntimeSemanticProjectionError::ExpressionLiteral { owner, reason }
                        })?
                    }
                    (
                        CheckedExpressionResolution::Structural,
                        HirExprKind::NumericBracketSequence(sequence),
                    ) => {
                        let TypeKind::Vec(item) = &closed_ty else {
                            return Err(error(
                                owner,
                                "instance numeric sequence is not a closed Vec value",
                            ));
                        };
                        runtime_sequence_from_literal_values(
                            sequence
                                .elements()
                                .iter()
                                .map(|element| {
                                    runtime_integer_magnitude(element.magnitude(), item.as_ref())
                                })
                                .collect::<Result<Vec<_>, _>>()
                                .map_err(|reason| {
                                    RuntimeSemanticProjectionError::ExpressionLiteral {
                                        owner,
                                        reason,
                                    }
                                })?,
                        )
                    }
                    _ => {
                        return Err(error(
                            owner,
                            "instance literal family disagrees with checked expression",
                        ));
                    }
                };
                RuntimeProjectFunctionExpressionPayload::Literal(value)
            }
            CheckedExecutableRuntimeExpressionFactFamily::Value => {
                let ty = checked.value_type().ok_or_else(|| {
                    error(owner, "instance value has no checked runtime value type")
                })?;
                let CheckedExpressionResolution::Value(value) = checked.resolution() else {
                    match checked.resolution() {
                        CheckedExpressionResolution::DialogueLineReference(target) => {
                            let line = RuntimeLineId::from_source_entity_body(target.as_str())
                                .map_err(|source| RuntimeSemanticProjectionError::Value {
                                    owner,
                                    reason: source.to_string(),
                                })?;
                            expressions.push(RuntimeProjectFunctionExpressionSemanticFact::new(
                                owner,
                                expected.children().into(),
                                RuntimeProjectFunctionExpressionPayload::Value(
                                    RuntimeResolvedValue::DialogueLine(line),
                                ),
                            ));
                            continue;
                        }
                        CheckedExpressionResolution::StageLook(look) => {
                            expressions.push(RuntimeProjectFunctionExpressionSemanticFact::new(
                                owner,
                                expected.children().into(),
                                RuntimeProjectFunctionExpressionPayload::Value(
                                    RuntimeResolvedValue::CharacterLook {
                                        character: look.character().clone(),
                                        look: look.look_id().clone(),
                                    },
                                ),
                            ));
                            continue;
                        }
                        _ => {
                            return Err(error(
                                owner,
                                "instance value family disagrees with checked expression",
                            ));
                        }
                    }
                };
                let is_entity = matches!(hir.kind(), HirExprKind::EntityReference(_));
                let value =
                    runtime_value_resolution(value, &lexical.instantiate_type(ty)?, is_entity)
                        .map_err(|reason| RuntimeSemanticProjectionError::Value { owner, reason })?
                        .ok_or_else(|| {
                            error(owner, "instance value has no runtime scalar projection")
                        })?;
                RuntimeProjectFunctionExpressionPayload::Value(value)
            }
            CheckedExecutableRuntimeExpressionFactFamily::Select => {
                let CheckedExpressionResolution::Select(select) = checked.resolution() else {
                    return Err(error(
                        owner,
                        "instance Select family disagrees with checked expression",
                    ));
                };
                let closed_owner = expected.children().first().and_then(|target| {
                    projected_types
                        .get(&RuntimeProjectFunctionTypeOwner::Expression(*target))
                        .map(|ty| ty.identity())
                });
                RuntimeProjectFunctionExpressionPayload::Select(
                    runtime_select_under(owner, select, closed_owner, world, analysis)?
                        .ok_or_else(|| error(owner, "instance Select has no runtime projection"))?,
                )
            }
            CheckedExecutableRuntimeExpressionFactFamily::NominalRecord => {
                let CheckedExpressionResolution::Nominal(nominal) = checked.resolution() else {
                    return Err(error(
                        owner,
                        "instance nominal-record family disagrees with checked expression",
                    ));
                };
                let fields = analysis
                    .checked_expression_edge_fact(owner)
                    .map_err(|source| RuntimeSemanticProjectionError::ExpressionEdges {
                        owner,
                        source,
                    })?
                    .record_fields()
                    .iter()
                    .map(|field| {
                        RuntimeRecordExpressionField::new(
                            field.runtime_field(),
                            match field.source() {
                                CheckedRecordValueSource::Expression(source) => {
                                    RuntimeRecordExpressionSource::Expression(source.raw())
                                }
                                CheckedRecordValueSource::Binding(source) => {
                                    RuntimeRecordExpressionSource::Binding(source.raw())
                                }
                            },
                        )
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                RuntimeProjectFunctionExpressionPayload::NominalRecord(
                    RuntimeRecordExpressionFact::try_new(
                        runtime_nominal_record_under(
                            nominal,
                            symbols,
                            world,
                            analysis,
                            lexical.types(),
                        )?,
                        fields,
                    )
                    .map_err(|source| {
                        RuntimeSemanticProjectionError::RecordPlan {
                            owner: RuntimeRecordExecutableOwner::Expression(owner),
                            source,
                        }
                    })?,
                )
            }
            CheckedExecutableRuntimeExpressionFactFamily::Variant => {
                let CheckedExpressionResolution::Variant(variant) = checked.resolution() else {
                    return Err(error(
                        owner,
                        "instance variant family disagrees with checked expression",
                    ));
                };
                RuntimeProjectFunctionExpressionPayload::Variant(runtime_variant_under(
                    variant,
                    symbols,
                    world,
                    analysis,
                    lexical.types(),
                )?)
            }
            CheckedExecutableRuntimeExpressionFactFamily::Call => {
                let facts = analysis.call(owner).ok_or_else(|| {
                    error(owner, "instance runtime call has no checked call fact")
                })?;
                RuntimeProjectFunctionExpressionPayload::Call(runtime_call(
                    owner,
                    facts,
                    project,
                    symbols,
                    world,
                    analysis,
                    lexical.types(),
                    instances,
                )?)
            }
            CheckedExecutableRuntimeExpressionFactFamily::PostfixCandidate => {
                let CheckedExpressionResolution::PostfixBracket(resolution) = checked.resolution()
                else {
                    return Err(error(
                        owner,
                        "instance postfix family disagrees with checked expression",
                    ));
                };
                RuntimeProjectFunctionExpressionPayload::PostfixCandidate(resolution.candidate())
            }
            CheckedExecutableRuntimeExpressionFactFamily::Await => {
                let CheckedExpressionResolution::Await(awaited) = checked.resolution() else {
                    return Err(error(
                        owner,
                        "instance Await family disagrees with checked expression",
                    ));
                };
                RuntimeProjectFunctionExpressionPayload::Await(RuntimeAwaitFact::new(
                    awaited.operand(),
                    awaited
                        .observers()
                        .iter()
                        .map(|observer| RuntimeAwaitPendingObserverFact::new(observer.pattern()))
                        .collect::<Vec<_>>(),
                ))
            }
            CheckedExecutableRuntimeExpressionFactFamily::Choice => {
                let CheckedExpressionResolution::Choice(choice) = checked.resolution() else {
                    return Err(error(
                        owner,
                        "instance Choice family disagrees with checked expression",
                    ));
                };
                RuntimeProjectFunctionExpressionPayload::Choice(RuntimeChoiceFact::new(
                    choice.public_id().cloned(),
                    choice.option_ids().to_vec(),
                    choice
                        .gotos()
                        .iter()
                        .map(|goto| {
                            runtime_project_item(goto.target())
                                .map(|target| RuntimeChoiceGotoFact::new(goto.arm(), target))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                ))
            }
            CheckedExecutableRuntimeExpressionFactFamily::Try => {
                RuntimeProjectFunctionExpressionPayload::Try(runtime_try_fact(
                    owner,
                    execution.try_expression(owner)?,
                    symbols,
                    world,
                    analysis,
                    lexical.types(),
                )?)
            }
            CheckedExecutableRuntimeExpressionFactFamily::ImplicitCallable => {
                let view = execution.implicit_callable(owner)?;
                let callable = RuntimeImplicitCallableFact::new(
                    runtime_type_under(
                        view.parameter(),
                        lexical.types(),
                        symbols,
                        world,
                        analysis,
                    )?,
                    runtime_type_under(view.result(), lexical.types(), symbols, world, analysis)?,
                    view.placeholders().collect(),
                    view.captures().collect(),
                );
                let (tried, pipe) = match view.body() {
                    FinalAnalysisImplicitCallableBody::Plain(_) => (None, None),
                    FinalAnalysisImplicitCallableBody::Try(tried) => (
                        Some(runtime_try_fact(
                            owner,
                            tried,
                            symbols,
                            world,
                            analysis,
                            lexical.types(),
                        )?),
                        None,
                    ),
                    FinalAnalysisImplicitCallableBody::Pipe(pipe) => (
                        None,
                        Some(RuntimePipeFact::new(
                            pipe.left(),
                            pipe.right(),
                            pipe.placeholders().collect(),
                        )),
                    ),
                };
                RuntimeProjectFunctionExpressionPayload::ImplicitCallable {
                    callable,
                    tried,
                    pipe,
                }
            }
            CheckedExecutableRuntimeExpressionFactFamily::Pipe => {
                let pipe = execution.pipe(owner)?;
                RuntimeProjectFunctionExpressionPayload::Pipe(RuntimePipeFact::new(
                    pipe.left(),
                    pipe.right(),
                    pipe.placeholders().collect(),
                ))
            }
            CheckedExecutableRuntimeExpressionFactFamily::DialogueApplication => {
                let scope = lexical.dialogue_scope();
                let application = dialogue
                    .application(&scope, owner)
                    .cloned()
                    .ok_or_else(|| {
                        error(
                            owner,
                            "instance dialogue application has no preprojected closed content occurrence",
                        )
                    })?;
                let fragment = dialogue.fragment(&scope, owner).cloned().ok_or_else(|| {
                    error(
                        owner,
                        "instance dialogue application has no preprojected root fragment",
                    )
                })?;
                RuntimeProjectFunctionExpressionPayload::DialogueApplication {
                    application,
                    fragments: vec![fragment].into_boxed_slice(),
                }
            }
            CheckedExecutableRuntimeExpressionFactFamily::ContentApplication => {
                let scope = lexical.dialogue_scope();
                let fragments = dialogue
                    .fragment(&scope, owner)
                    .cloned()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                RuntimeProjectFunctionExpressionPayload::ContentApplication { fragments }
            }
            CheckedExecutableRuntimeExpressionFactFamily::Closure => {
                RuntimeProjectFunctionExpressionPayload::Closure(Box::new(
                    runtime_closure_instance_fact(
                        origin,
                        lexical,
                        owner,
                        project,
                        symbols,
                        world,
                        analysis,
                        runtime_owners,
                        dialogue,
                        instances,
                    )?,
                ))
            }
        };
        expressions.push(RuntimeProjectFunctionExpressionSemanticFact::new(
            owner,
            expected.children().into(),
            payload,
        ));
    }

    let mut patterns = Vec::with_capacity(partition.patterns().len());
    for expected in partition.patterns() {
        let owner = expected.owner();
        let checked = analysis
            .pattern(owner)
            .ok_or_else(|| origin.error("instance pattern has no checked semantic fact"))?;
        let payload = match (expected.family(), checked.resolution()) {
            (
                CheckedExecutableRuntimePatternFactFamily::Structural,
                CheckedPatternResolution::Structural,
            ) => RuntimeProjectFunctionPatternPayload::Structural,
            (
                CheckedExecutableRuntimePatternFactFamily::Literal,
                CheckedPatternResolution::Literal(literal),
            ) => RuntimeProjectFunctionPatternPayload::Literal(
                runtime_literal(literal, &lexical.instantiate_type(checked.ty())?).map_err(
                    |reason| RuntimeSemanticProjectionError::PatternLiteral { owner, reason },
                )?,
            ),
            (
                CheckedExecutableRuntimePatternFactFamily::Entity,
                CheckedPatternResolution::Entity(item),
            ) => RuntimeProjectFunctionPatternPayload::Entity(runtime_project_item(item)?),
            (
                CheckedExecutableRuntimePatternFactFamily::NominalRecord,
                CheckedPatternResolution::Record(record),
            ) => RuntimeProjectFunctionPatternPayload::NominalRecord(runtime_record_pattern_under(
                owner,
                record,
                symbols,
                world,
                analysis,
                lexical.types(),
            )?),
            (
                CheckedExecutableRuntimePatternFactFamily::Variant,
                CheckedPatternResolution::Variant(variant),
            ) => RuntimeProjectFunctionPatternPayload::Variant(runtime_variant_under(
                variant,
                symbols,
                world,
                analysis,
                lexical.types(),
            )?),
            (
                CheckedExecutableRuntimePatternFactFamily::TypedBinding,
                CheckedPatternResolution::TypedBinding(_),
            ) => RuntimeProjectFunctionPatternPayload::TypedBinding,
            _ => {
                return Err(origin
                    .error("instance pattern family disagrees with checked semantic partition"));
            }
        };
        patterns.push(RuntimeProjectFunctionPatternSemanticFact::new(
            owner, payload,
        ));
    }

    let mut statements = Vec::with_capacity(partition.statements().len());
    for expected in partition.statements() {
        let owner = expected.owner();
        let checked = analysis
            .statement(owner)
            .ok_or_else(|| origin.error("instance statement has no checked semantic fact"))?;
        let payload = match (expected.family(), checked.payload()) {
            (
                CheckedExecutableRuntimeStatementFactFamily::Structural,
                CheckedStatementPayload::Structural,
            ) => RuntimeProjectFunctionStatementPayload::Structural,
            (
                CheckedExecutableRuntimeStatementFactFamily::Assignment,
                CheckedStatementPayload::Assignment(assignment),
            ) => RuntimeProjectFunctionStatementPayload::Assignment(runtime_assignment_under(
                owner,
                assignment,
                symbols,
                world,
                analysis,
                lexical.types(),
            )?),
            (
                CheckedExecutableRuntimeStatementFactFamily::Assertion,
                CheckedStatementPayload::Assertion(disposition),
            ) => RuntimeProjectFunctionStatementPayload::Assertion(runtime_assertion(
                owner,
                *disposition,
            )?),
            (
                CheckedExecutableRuntimeStatementFactFamily::Defer,
                CheckedStatementPayload::Defer(_),
            ) => RuntimeProjectFunctionStatementPayload::Defer,
            (
                CheckedExecutableRuntimeStatementFactFamily::EvaluatedEffect,
                CheckedStatementPayload::EvaluatedEffect(effect),
            ) => RuntimeProjectFunctionStatementPayload::EvaluatedEffect(
                runtime_evaluated_effect_under(effect, symbols, world, analysis, lexical.types())?,
            ),
            (
                CheckedExecutableRuntimeStatementFactFamily::Iteration,
                CheckedStatementPayload::Iteration(iteration),
            ) => {
                let methods = iteration
                    .witness_methods()
                    .map(|(_, conformance, _)| {
                        (conformance.clone(), conformance.declaration().clone())
                    })
                    .collect();
                RuntimeProjectFunctionStatementPayload::Iteration(runtime_iteration_under(
                    owner,
                    iteration,
                    &methods,
                    symbols,
                    world,
                    analysis,
                    lexical.types(),
                )?)
            }
            (
                CheckedExecutableRuntimeStatementFactFamily::ControlTransfer,
                CheckedStatementPayload::ControlTransfer(_),
            ) => RuntimeProjectFunctionStatementPayload::ControlTransfer,
            (
                CheckedExecutableRuntimeStatementFactFamily::Trigger,
                CheckedStatementPayload::Trigger(trigger),
            ) => RuntimeProjectFunctionStatementPayload::Trigger(match trigger.view() {
                CheckedTriggerView::Input => RuntimeTriggerAdmission::input(),
                CheckedTriggerView::Event => RuntimeTriggerAdmission::event(),
                CheckedTriggerView::Signal => RuntimeTriggerAdmission::signal(),
                CheckedTriggerView::Timeout => RuntimeTriggerAdmission::timeout(),
                CheckedTriggerView::Select => RuntimeTriggerAdmission::select(),
                CheckedTriggerView::Task => RuntimeTriggerAdmission::task(),
                CheckedTriggerView::Scope => RuntimeTriggerAdmission::scope(),
                CheckedTriggerView::Expression => RuntimeTriggerAdmission::expression(),
                CheckedTriggerView::Mark(coordinate) => RuntimeTriggerAdmission::mark(
                    dialogue
                        .mark(&lexical.dialogue_scope(), coordinate)
                        .cloned()
                        .ok_or_else(|| {
                            origin.error(
                                "instance Mark trigger has no preprojected content occurrence",
                            )
                        })?,
                ),
            }),
            (
                CheckedExecutableRuntimeStatementFactFamily::UnsafeAudit,
                CheckedStatementPayload::UnsafeAudit(_),
            ) => RuntimeProjectFunctionStatementPayload::UnsafeAudit,
            (
                CheckedExecutableRuntimeStatementFactFamily::Select,
                CheckedStatementPayload::Select(_),
            ) => RuntimeProjectFunctionStatementPayload::Select,
            (
                CheckedExecutableRuntimeStatementFactFamily::SourceLocale,
                CheckedStatementPayload::SourceLocale(_),
            ) => RuntimeProjectFunctionStatementPayload::SourceLocale,
            (
                CheckedExecutableRuntimeStatementFactFamily::Scope,
                CheckedStatementPayload::Scope(_),
            ) => RuntimeProjectFunctionStatementPayload::Scope,
            (
                CheckedExecutableRuntimeStatementFactFamily::Include,
                CheckedStatementPayload::Include(_),
            ) => RuntimeProjectFunctionStatementPayload::Include,
            (
                CheckedExecutableRuntimeStatementFactFamily::Suspension,
                CheckedStatementPayload::Suspension(_),
            ) => RuntimeProjectFunctionStatementPayload::Suspension,
            (
                CheckedExecutableRuntimeStatementFactFamily::Yield,
                CheckedStatementPayload::Yield,
            ) => RuntimeProjectFunctionStatementPayload::Yield,
            _ => {
                return Err(origin
                    .error("instance statement family disagrees with checked semantic partition"));
            }
        };
        statements.push(RuntimeProjectFunctionStatementSemanticFact::new(
            owner, payload,
        ));
    }

    let captures = partition
        .captures()
        .iter()
        .map(|owner| {
            let checked = analysis
                .capture(*owner)
                .ok_or_else(|| origin.error("instance capture has no checked semantic fact"))?;
            Ok(RuntimeCheckedCapture::new(
                *analysis.selected_capture(*owner).ok_or_else(|| {
                    origin.error("instance capture has no selected lexical projection")
                })?,
                runtime_type_under(checked.ty(), lexical.types(), symbols, world, analysis)?,
            ))
        })
        .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?;

    RuntimeProjectFunctionInstanceSemanticFacts::try_new(
        partition,
        type_projection,
        expressions.into_boxed_slice(),
        patterns.into_boxed_slice(),
        statements.into_boxed_slice(),
        captures.into_boxed_slice(),
    )
    .map_err(|reason| origin.error(reason.to_string()))
}

#[allow(
    clippy::too_many_arguments,
    reason = "one nested closure instance joins its checked callable row, outer substitution, exact executable partition, and recursive semantic facts"
)]
fn runtime_closure_instance_fact(
    origin: ProjectInstantiationOrigin,
    lexical: RuntimeExecutableInstantiation<'_>,
    owner: ExprId,
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    dialogue: &RuntimeDialogueProjectionCatalog,
    instances: &mut ProjectInstanceProjection<'_>,
) -> Result<RuntimeClosureInstanceFact, RuntimeSemanticProjectionError> {
    let error = |reason: &str| RuntimeSemanticProjectionError::Call {
        owner,
        reason: reason.to_owned(),
    };
    let module = project
        .modules()
        .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module))
        .ok_or_else(|| error("closure-instance module is absent"))?;
    let expression = module
        .resolve_expr(owner)
        .map_err(|_| error("closure-instance expression is absent"))?;
    let HirExprKind::Closure(closure) = expression.kind() else {
        return Err(error("closure-instance owner is not a Closure expression"));
    };
    let checked = analysis
        .expression(owner)
        .ok_or_else(|| error("closure-instance has no checked expression fact"))?;
    let CheckedExpressionResolution::Closure(checked_closure) = checked.resolution() else {
        return Err(error(
            "closure-instance owner has no checked closure authority",
        ));
    };
    let checked_execution = analysis
        .execution_projection()
        .closure_execution(runtime_owners, owner)?;
    let effects = lexical.instantiate_effect_row(checked_execution.effects())?;
    let execution = match (
        checked_execution.suspension(),
        effects.is_empty(),
        checked_execution.control(),
    ) {
        (
            arcweft_lang_sema::final_analysis::CheckedSuspensionRole::NonSuspending,
            true,
            arcweft_lang_sema::final_analysis::CheckedExecutableControlRole::ExpressionCompatible,
        ) => RuntimeProjectFunctionExecution::ExpressionFunctionSite,
        _ => RuntimeProjectFunctionExecution::ExecutableFunctionSite,
    };
    let function_type = runtime_type_under(
        checked
            .value_type()
            .ok_or_else(|| error("closure-instance has no checked function type"))?,
        lexical.types(),
        symbols,
        world,
        analysis,
    )?;
    let parameters = closure
        .parameters()
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            let position = u32::try_from(position)
                .map_err(|_| error("closure parameter position exceeds u32"))?;
            let ty = analysis
                .pattern(parameter.pattern())
                .ok_or_else(|| error("closure parameter has no checked pattern type"))?;
            Ok(RuntimeClosureParameterFact::new(
                position,
                parameter.pattern(),
                runtime_type_under(ty.ty(), lexical.types(), symbols, world, analysis)?,
            ))
        })
        .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?;
    let captures = checked_closure
        .captures()
        .iter()
        .enumerate()
        .map(|(position, capture)| {
            let position = u32::try_from(position)
                .map_err(|_| error("closure capture position exceeds u32"))?;
            let checked = analysis
                .capture(capture.capture())
                .ok_or_else(|| error("closure capture has no checked type"))?;
            Ok(RuntimeClosureCaptureFact::new(
                position,
                capture.capture(),
                capture.local(),
                runtime_type_under(checked.ty(), lexical.types(), symbols, world, analysis)?,
            ))
        })
        .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?;

    let executable = HirRuntimeExecutableOwner::Closure(owner);
    let semantic_owners = runtime_owners
        .executable_owners(&executable)
        .ok_or_else(|| error("closure has no exact executable semantic partition"))?;
    let partition = analysis
        .execution_projection()
        .runtime_fact_partition(runtime_owners, &executable)?;
    let mut type_projection = Vec::new();
    for expected in partition.expressions() {
        let expression = expected.owner();
        if !expected.has_runtime_type() {
            type_projection
                .push(RuntimeProjectFunctionTypeProjection::semantic_only_expression(expression));
            continue;
        }
        let checked = analysis
            .expression(expression)
            .ok_or_else(|| error("closure expression has no checked semantic fact"))?;
        let ty = checked
            .value_type()
            .ok_or_else(|| error("runtime closure expression has no checked value type"))?;
        type_projection.push(RuntimeProjectFunctionTypeProjection::value(
            RuntimeProjectFunctionTypeOwner::Expression(expression),
            runtime_type_under(ty, lexical.types(), symbols, world, analysis)?,
        ));
    }
    for pattern in semantic_owners.patterns() {
        let checked = analysis
            .pattern(pattern)
            .ok_or_else(|| error("closure pattern has no checked semantic fact"))?;
        type_projection.push(RuntimeProjectFunctionTypeProjection::value(
            RuntimeProjectFunctionTypeOwner::Pattern(pattern),
            runtime_type_under(checked.ty(), lexical.types(), symbols, world, analysis)?,
        ));
    }
    for local in semantic_owners.locals() {
        let checked = analysis
            .local(local)
            .ok_or_else(|| error("closure local has no checked semantic fact"))?;
        type_projection.push(RuntimeProjectFunctionTypeProjection::value(
            RuntimeProjectFunctionTypeOwner::Local(local),
            runtime_type_under(checked.ty(), lexical.types(), symbols, world, analysis)?,
        ));
    }
    for ty in semantic_owners.types() {
        let checked = analysis
            .ty(ty)
            .ok_or_else(|| error("closure source type has no checked semantic fact"))?;
        type_projection.push(RuntimeProjectFunctionTypeProjection::value(
            RuntimeProjectFunctionTypeOwner::Type(ty),
            runtime_type_under(checked, lexical.types(), symbols, world, analysis)?,
        ));
    }
    type_projection.sort_by_key(RuntimeProjectFunctionTypeProjection::owner);
    let semantics = runtime_project_function_instance_semantic_facts(
        origin,
        lexical,
        partition,
        type_projection.into_boxed_slice(),
        project,
        symbols,
        world,
        analysis,
        runtime_owners,
        dialogue,
        instances,
    )?;
    RuntimeClosureInstanceFact::try_new(
        RuntimeClosureInstanceKey::new(
            lexical.project_key().cloned(),
            checked_execution.id().clone(),
        ),
        owner,
        function_type,
        checked_execution.suspension(),
        checked_execution.control(),
        execution,
        effects.iter().cloned().collect(),
        closure.scope(),
        closure.body(),
        parameters.into_boxed_slice(),
        captures.into_boxed_slice(),
        semantics,
    )
    .map_err(|reason| error(&reason.to_string()))
}

#[allow(
    clippy::too_many_arguments,
    reason = "one attached default instance joins the checked interface, closed callable solution, logical parameter ABI, and runtime types"
)]
fn runtime_project_attached_default(
    origin: ProjectInstantiationOrigin,
    callable: &RuntimeProjectCallable,
    selection: &ProjectInstanceSelection,
    instance_solution: ProjectInstanceTypes<'_>,
    parameters: &[RuntimeProjectFunctionParameterAbi],
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<Option<RuntimeProjectAttachedDefaultFunctionFact>, RuntimeSemanticProjectionError> {
    let checked = analysis
        .checked_callables()
        .project_callable(&selection.declaration)
        .map_err(|error| {
            origin.error(format!(
                "project attached default has no checked callable: {error:?}"
            ))
        })?;
    let checked_default = checked.attached_content().and_then(|row| row.default());
    let runtime_default = callable
        .attached_content_abi()
        .and_then(|row| row.default());
    let (Some(checked_default), Some(runtime_default)) = (checked_default, runtime_default) else {
        if checked_default.is_none() && runtime_default.is_none() {
            return Ok(None);
        }
        return Err(origin.error("checked and runtime attached default rows disagree"));
    };
    if checked_default.source() != runtime_default.source()
        || checked_default.coordinate() != runtime_default.coordinate()
        || checked_default.expression() != runtime_default.digest()
    {
        return Err(origin
            .error("attached default executable identity disagrees with its callable descriptor"));
    }
    let mut captures = Vec::with_capacity(checked_default.captures().len());
    for checked_capture in checked_default.captures() {
        let parameter_index = u32::try_from(checked_capture.parameter().parameter().get())
            .map_err(|_| origin.error("attached default parameter coordinate exceeds u32"))?;
        let parameter = parameters
            .iter()
            .find(|parameter| {
                parameter.group() == checked_capture.parameter().group()
                    && parameter.parameter() == parameter_index
            })
            .ok_or_else(|| {
                origin.error("attached default capture has no logical parameter source")
            })?;
        let binding_type = instance_solution.instantiate_type(checked_capture.binding_type())?;
        let binding_type = runtime_type(&binding_type, symbols, world, analysis)?;
        let binding_evidence_matches = checked_capture.bindings().len()
            == checked_capture.binding_evidence().len()
            && checked_capture
                .bindings()
                .iter()
                .zip(checked_capture.binding_evidence())
                .all(|(local, evidence)| {
                    local == &evidence.local()
                        && analysis
                            .local(*local)
                            .is_some_and(|checked| checked.ty() == evidence.ty())
                });
        if parameter.pattern() != checked_capture.pattern()
            || parameter.bindings() != checked_capture.bindings()
            || parameter.binding_ty() != &binding_type
            || !binding_evidence_matches
        {
            return Err(
                origin.error("attached default capture disagrees with its logical parameter ABI")
            );
        }
        captures.push(RuntimeProjectAttachedDefaultCapture::new(
            checked_capture.parameter().group(),
            parameter_index,
            parameter.source(),
            checked_capture.pattern(),
            checked_capture.pattern_digest(),
            checked_capture.bindings().to_vec().into_boxed_slice(),
            checked_capture
                .used_locals()
                .iter()
                .map(|local| local.local())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            binding_type,
        ));
    }
    let result = callable
        .attached_content_abi()
        .expect("runtime default requires attached ABI")
        .binding_ty()
        .clone();
    let execution = match (
        checked_default.suspension(),
        checked_default.effects().concrete().is_empty(),
        checked_default.control(),
    ) {
        (
            arcweft_lang_sema::final_analysis::CheckedSuspensionRole::NonSuspending,
            true,
            arcweft_lang_sema::final_analysis::CheckedExecutableControlRole::ExpressionCompatible,
        ) => RuntimeProjectFunctionExecution::ExpressionFunctionSite,
        (arcweft_lang_sema::final_analysis::CheckedSuspensionRole::NonSuspending, false, _)
        | (arcweft_lang_sema::final_analysis::CheckedSuspensionRole::MaySuspend, _, _)
        | (
            arcweft_lang_sema::final_analysis::CheckedSuspensionRole::NonSuspending,
            true,
            arcweft_lang_sema::final_analysis::CheckedExecutableControlRole::FlowRequired,
        ) => RuntimeProjectFunctionExecution::ExecutableFunctionSite,
    };
    RuntimeProjectAttachedDefaultFunctionFact::try_new(
        checked_default.source(),
        checked_default.coordinate().clone(),
        checked_default.expression(),
        result,
        checked_default.suspension(),
        checked_default.control(),
        execution,
        checked_default
            .effects()
            .concrete()
            .iter()
            .cloned()
            .collect(),
        captures.into_boxed_slice(),
    )
    .map(Some)
    .map_err(|reason| origin.error(reason.to_string()))
}

const fn runtime_call_operand_source(
    source: arcweft_lang_sema::callable::CheckedCallArgumentSlotSource,
) -> arcweft_runtime_plan::semantic_facts::RuntimeResolvedCallOperandSource {
    match source {
        arcweft_lang_sema::callable::CheckedCallArgumentSlotSource::Expression(expression) => {
            RuntimeResolvedCallOperandSource::Expression(expression)
        }
        arcweft_lang_sema::callable::CheckedCallArgumentSlotSource::CompactNumericElement {
            sequence,
            ordinal,
        } => RuntimeResolvedCallOperandSource::CompactNumericElement { sequence, ordinal },
    }
}

fn runtime_call_operand_binding(
    owner: ExprId,
    selected: &arcweft_lang_sema::callable::ResolvedCallable,
    passing: CheckedCallArgumentPassing,
    slot: &arcweft_lang_sema::callable::CheckedCallExecutionSlot,
) -> Result<RuntimeResolvedCallOperandBinding, RuntimeSemanticProjectionError> {
    match passing {
        CheckedCallArgumentPassing::Positional | CheckedCallArgumentPassing::Spread => {
            Ok(RuntimeResolvedCallOperandBinding::Positional)
        }
        CheckedCallArgumentPassing::Named => match slot.destination() {
            CheckedCallOperandDestination::Parameter(coordinate) => {
                let parameter = selected
                    .schema()
                    .group(coordinate.group())
                    .and_then(|group| group.parameter(coordinate.parameter()))
                    .ok_or_else(|| RuntimeSemanticProjectionError::Call {
                        owner,
                        reason: "named operand maps outside the selected callable schema"
                            .to_owned(),
                    })?;
                let name =
                    parameter
                        .name()
                        .ok_or_else(|| RuntimeSemanticProjectionError::Call {
                            owner,
                            reason: "named operand maps to an unnamed callable parameter"
                                .to_owned(),
                        })?;
                Ok(RuntimeResolvedCallOperandBinding::Named(
                    name.as_str().to_owned(),
                ))
            }
            CheckedCallOperandDestination::Open(open) => Ok(
                RuntimeResolvedCallOperandBinding::Named(open.binding().as_str().to_owned()),
            ),
        },
    }
}

fn runtime_call_operand_projection(
    owner: ExprId,
    slot: &arcweft_lang_sema::callable::CheckedCallExecutionSlot,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    enclosing: Option<ProjectInstanceTypes<'_>>,
) -> Result<RuntimeResolvedCallOperandProjection, RuntimeSemanticProjectionError> {
    let projection = match slot.source_projection() {
        CheckedConstraintSourceProjection::Scalar => RuntimeResolvedCallOperandProjection::Scalar,
        CheckedConstraintSourceProjection::SpreadContainer(container) => {
            RuntimeResolvedCallOperandProjection::SpreadContainer(match container {
                CheckedConstraintContainerConstructor::Vec => RuntimeResolvedSpreadContainer::Vec,
                CheckedConstraintContainerConstructor::Seq => RuntimeResolvedSpreadContainer::Seq,
                CheckedConstraintContainerConstructor::Slice => {
                    RuntimeResolvedSpreadContainer::Slice
                }
                CheckedConstraintContainerConstructor::Array { len } => {
                    let len = instantiate_array_length_under(len, enclosing)?;
                    let ArrayLength::Const(len) = len else {
                        return Err(RuntimeSemanticProjectionError::Call {
                            owner,
                            reason: "array spread operand does not have a constant length"
                                .to_owned(),
                        });
                    };
                    RuntimeResolvedSpreadContainer::Array { len }
                }
                CheckedConstraintContainerConstructor::MapValue { kind, key } => {
                    RuntimeResolvedSpreadContainer::MapValue {
                        kind: match kind {
                            MapKind::Ordered => RuntimeMapKind::Ordered,
                            MapKind::Sorted => RuntimeMapKind::Sorted,
                            MapKind::BTree => RuntimeMapKind::BTree,
                        },
                        key: runtime_type_under(key, enclosing, symbols, world, analysis)?,
                    }
                }
            })
        }
    };
    Ok(projection)
}

fn checked_call_parameter_expression(
    owner: ExprId,
    application: &CheckedCallApplication,
    group: usize,
    parameter: usize,
) -> Result<ExprId, RuntimeSemanticProjectionError> {
    let mut sources = application
        .core()
        .execution()
        .arguments()
        .iter()
        .flat_map(|argument| argument.slots())
        .filter_map(|slot| match slot.destination() {
            CheckedCallOperandDestination::Parameter(coordinate)
                if coordinate.group().get() == group
                    && coordinate.parameter().get() == parameter =>
            {
                Some(slot)
            }
            CheckedCallOperandDestination::Parameter(_)
            | CheckedCallOperandDestination::Open(_) => None,
        });
    let slot = sources
        .next()
        .ok_or_else(|| RuntimeSemanticProjectionError::Call {
            owner,
            reason: format!(
                "checked call has no operand for parameter coordinate ({group}, {parameter})"
            ),
        })?;
    if sources.next().is_some()
        || !matches!(
            slot.source_projection(),
            CheckedConstraintSourceProjection::Scalar
        )
    {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: format!(
                "checked call parameter coordinate ({group}, {parameter}) is not one scalar operand"
            ),
        });
    }
    let arcweft_lang_sema::callable::CheckedCallArgumentSlotSource::Expression(source) =
        slot.source().raw()
    else {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: format!(
                "checked call parameter coordinate ({group}, {parameter}) has no expression source"
            ),
        });
    };
    Ok(source)
}

fn checked_call_receiver_expression(
    owner: ExprId,
    application: &CheckedCallApplication,
) -> Result<ExprId, RuntimeSemanticProjectionError> {
    let CheckedCallReceiverProjection::Operand { source, .. } =
        application.core().execution().receiver()
    else {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: "checked call has no runtime receiver operand".to_owned(),
        });
    };
    let arcweft_lang_sema::callable::CheckedCallArgumentSlotSource::Expression(source) =
        source.raw()
    else {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: "checked call receiver has no expression source".to_owned(),
        });
    };
    Ok(source)
}

fn runtime_standard_map_call(
    owner: ExprId,
    application: &CheckedCallApplication,
    family: StandardMapFamily,
) -> Result<RuntimeStandardMapCall, RuntimeSemanticProjectionError> {
    if !matches!(
        application.result(),
        arcweft_lang_sema::callable::CheckedCallResult::Value(_)
    ) {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: "a standard map runtime target must be fully applied".to_owned(),
        });
    }
    let mapping = checked_call_parameter_expression(owner, application, 0, 0)?;
    let (receiver, order) = match application.core().execution().receiver() {
        CheckedCallReceiverProjection::Operand { .. } => (
            checked_call_receiver_expression(owner, application)?,
            RuntimeStandardMapOperandOrder::ReceiverThenMapping,
        ),
        CheckedCallReceiverProjection::None => (
            checked_call_parameter_expression(owner, application, 1, 0)?,
            RuntimeStandardMapOperandOrder::MappingThenReceiver,
        ),
        CheckedCallReceiverProjection::SemanticOnly { .. } => {
            return Err(RuntimeSemanticProjectionError::Call {
                owner,
                reason: "standard map requires one value receiver operand".to_owned(),
            });
        }
    };
    let family = match family {
        StandardMapFamily::Vec => RuntimeStandardMapFamily::Vec,
        StandardMapFamily::Seq => RuntimeStandardMapFamily::Seq,
        StandardMapFamily::Array => RuntimeStandardMapFamily::Array,
        StandardMapFamily::Slice => RuntimeStandardMapFamily::Slice,
        StandardMapFamily::Option => RuntimeStandardMapFamily::Option,
        StandardMapFamily::Result => RuntimeStandardMapFamily::Result,
        StandardMapFamily::Need | StandardMapFamily::Parser | StandardMapFamily::Stream => {
            return Err(RuntimeSemanticProjectionError::Call {
                owner,
                reason: format!(
                    "standard map family {family:?} has no accepted producer/parser runtime transform"
                ),
            });
        }
    };
    Ok(RuntimeStandardMapCall::new(
        family, mapping, receiver, order,
    ))
}

/// Projects one checked project callable and its declaration-side attached
/// ABI from the final callable interface. Call sites and Entry lowering share
/// this boundary; neither may derive a callee ABI from an observed call.
pub(crate) fn runtime_project_callable(
    declaration: &arcweft_lang_hir::symbol::CallableDeclarationKey,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeProjectCallable, String> {
    let symbol = symbols
        .callable(declaration)
        .ok_or_else(|| "project callable is absent from the accepted symbol table".to_owned())?;
    let checked = analysis
        .checked_callables()
        .project_callable(declaration)
        .map_err(|error| format!("project callable has no accepted checked facts: {error:?}"))?;
    let runtime = arcweft_core::entry::RuntimeCallableId::from_checked_digest(
        checked.id().semantic_digest().into_bytes(),
    );
    let attached_content_abi = checked
        .attached_content()
        .map(|attached| {
            let default = attached.default().map(|default| {
                RuntimeCallableAttachedContentDefault::new(
                    default.source(),
                    default.coordinate().clone(),
                    default.expression(),
                )
            });
            RuntimeCallableAttachedContentAbi::try_new(
                attached.group(),
                attached.abi_position(),
                attached.presence(),
                attached.binding(),
                runtime_type(attached.binding_type(), symbols, world, analysis)
                    .map_err(|error| error.to_string())?,
                runtime_type(attached.abi_type(), symbols, world, analysis)
                    .map_err(|error| error.to_string())?,
                default,
            )
            .map_err(|error| error.to_string())
        })
        .transpose()?;
    RuntimeProjectCallable::try_new(
        declaration.clone(),
        symbol.source_item(),
        symbol.source_owner(),
        runtime,
        attached_content_abi,
    )
    .map_err(|error| error.to_string())
}

#[expect(
    clippy::too_many_lines,
    reason = "checked callable families are exhaustively selected at this runtime boundary"
)]
fn runtime_call_target(
    owner: ExprId,
    application: &CheckedCallApplication,
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    enclosing: Option<ProjectInstanceTypes<'_>>,
) -> Result<RuntimeResolvedStaticCallTarget, RuntimeSemanticProjectionError> {
    if let Some(variant) = analysis
        .execution_projection()
        .variant_constructor(project, application)?
    {
        return runtime_variant_under(&variant, symbols, world, analysis, enclosing)
            .map(RuntimeResolvedStaticCallTarget::Variant);
    }
    let selected = application.core().candidates().selected();
    let selected_id = selected.id();
    let selected_family = selected.family();
    if let arcweft_lang_sema::callable::CallableCandidateId::Presentation(presentation) =
        selected_id
    {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: format!(
                "presentation callable {presentation:?} requires the pending typed Presentation command ABI"
            ),
        });
    }
    if selected_family == CallableFamily::TraitMethod {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: "trait-call witness has no accepted runtime method-ID inventory".to_owned(),
        });
    }
    if matches!(
        selected_family,
        CallableFamily::Lexical | CallableFamily::FunctionValue
    ) {
        return Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: "direct dispatch cannot lower a value-callee callable".to_owned(),
        });
    }
    if matches!(
        selected_id,
        arcweft_lang_sema::callable::CallableCandidateId::Builtin(BuiltinCallableId::Reduction(
            ReductionConstructorKind::Unchanged
        ))
    ) {
        return Ok(RuntimeResolvedStaticCallTarget::Reduction(
            RuntimeReductionConstructor::Unchanged,
        ));
    }
    if let CallableValidator::StandardMap(family) = selected.schema().validator() {
        return runtime_standard_map_call(owner, application, *family)
            .map(RuntimeResolvedStaticCallTarget::StandardMap);
    }
    if let Some(intrinsic) = runtime_intrinsic(selected_id) {
        return Ok(RuntimeResolvedStaticCallTarget::Intrinsic(intrinsic));
    }
    if let arcweft_lang_sema::callable::CallableCandidateId::Agent(intrinsic) = selected_id {
        let intrinsic = runtime_agent_intrinsic(*intrinsic);
        return Ok(
            if let Some(host) = RuntimeResolvedHostCall::agent(intrinsic) {
                RuntimeResolvedStaticCallTarget::Host(host)
            } else {
                RuntimeResolvedStaticCallTarget::Agent(intrinsic)
            },
        );
    }
    if let CallableCandidateId::DomainMethod(DomainMethodId::ProbeCompare { operation, .. }) =
        selected_id
    {
        return Ok(RuntimeResolvedStaticCallTarget::AgentProbeComparison(
            runtime_agent_probe_comparison(operation.operator()),
        ));
    }
    if matches!(
        selected_id,
        CallableCandidateId::DomainMethod(DomainMethodId::DiagnosticsHasError)
    ) {
        return Ok(RuntimeResolvedStaticCallTarget::AgentDiagnosticsHasError);
    }
    if let ResolvedCallableOrigin::Project { declaration, .. } = selected.origin() {
        let checked = analysis
            .checked_callables()
            .project_callable(declaration)
            .map_err(|error| RuntimeSemanticProjectionError::Call {
                owner,
                reason: format!("project call has no accepted checked callable facts: {error:?}"),
            })?;
        let runtime = runtime_project_callable(declaration, symbols, world, analysis)
            .map_err(|reason| RuntimeSemanticProjectionError::Call { owner, reason })?;
        if declaration.owner()
            == arcweft_lang_hir::symbol::CallableDeclarationOwner::ExternCapability
        {
            if !matches!(checked.execution(), CheckedCallableExecution::Runtime(_)) {
                return Err(RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "extern capability call was classified as a dispatch-only contract"
                        .to_owned(),
                });
            }
            let mode = if checked
                .exposed_row()
                .concrete()
                .iter()
                .any(EffectId::is_control_suspend)
            {
                RuntimeHostCallMode::Suspend
            } else {
                RuntimeHostCallMode::Immediate
            };
            let contract = checked.host_call_contract().ok_or_else(|| {
                RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "extern capability call has no manifest-owned host-call contract"
                        .to_owned(),
                }
            })?;
            let host = RuntimeResolvedHostCall::extern_capability(runtime, contract, mode)
                .map_err(|error| RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: error.to_string(),
                })?;
            if mode == RuntimeHostCallMode::Immediate
                && checked.exposed_row().concrete().is_empty()
                && let Some(intrinsic) = RuntimeIntrinsic::from_label(host.public_id())
            {
                return Ok(RuntimeResolvedStaticCallTarget::Intrinsic(intrinsic));
            }
            return Ok(RuntimeResolvedStaticCallTarget::Host(host));
        }
        return Ok(RuntimeResolvedStaticCallTarget::Declaration(runtime));
    }
    let checked = selected
        .checked()
        .ok_or_else(|| RuntimeSemanticProjectionError::Call {
            owner,
            reason: format!(
                "language callable family {:?} has no typed runtime intrinsic",
                selected_family
            ),
        })?;
    Ok(RuntimeResolvedStaticCallTarget::Registered(
        arcweft_core::entry::RuntimeCallableId::from_checked_digest(
            checked.semantic_digest().into_bytes(),
        ),
    ))
}

fn runtime_line_callable(
    owner: ExprId,
    application: &CheckedCallApplication,
    analysis: &FinalSemanticAnalysis,
    enclosing: Option<ProjectInstanceTypes<'_>>,
) -> Result<Option<RuntimeLineCallable>, RuntimeSemanticProjectionError> {
    let selected = application.core().candidates().selected();
    let receiver = || match application.core().execution().receiver() {
        CheckedCallReceiverProjection::SemanticOnly { ty, .. }
        | CheckedCallReceiverProjection::Operand { ty, .. } => enclosing
            .map_or_else(|| Ok(ty.clone()), |solution| solution.instantiate_type(ty))
            .map_err(RuntimeSemanticProjectionError::from),
        CheckedCallReceiverProjection::None => Err(RuntimeSemanticProjectionError::Call {
            owner,
            reason: "checked line method has no exact receiver projection".to_owned(),
        }),
    };
    let line = match selected.id() {
        CallableCandidateId::StageMethod(StageMethodId::Acquire) => {
            let TypeKind::StageApi(character) = receiver()? else {
                return Err(RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "stage acquire lost its exact Character receiver".to_owned(),
                });
            };
            RuntimeLineCallable::AcquireActor { character }
        }
        CallableCandidateId::StageMethod(StageMethodId::Look) => {
            let TypeKind::StageActorHandle(arcweft_lang_sema::types::StageActorHandleType::Exact(
                character,
            )) = receiver()?
            else {
                return Err(RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "stage look lost its exact Character handle receiver".to_owned(),
                });
            };
            RuntimeLineCallable::ActorLook {
                character,
                actor: checked_call_receiver_expression(owner, application)?,
                look: checked_call_parameter_expression(owner, application, 0, 0)?,
                crossfade: checked_call_parameter_expression(owner, application, 0, 1)?,
            }
        }
        CallableCandidateId::LineContextMethod(LineContextMethodId::VoiceHandle) => {
            if !matches!(receiver()?, TypeKind::LineContext) {
                return Err(RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "voice handle lost its LineContext receiver".to_owned(),
                });
            }
            RuntimeLineCallable::VoiceHandle
        }
        CallableCandidateId::LineSchedule(LineScheduleCallableId::At) => {
            let ResolvedCallableState::Continuation(continuation) = selected.state() else {
                return Err(RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "line schedule completion lost its checked prefix continuation"
                        .to_owned(),
                });
            };
            let prefix_owner = continuation.prefix_call_site().expression();
            let prefix = analysis
                .call(prefix_owner)
                .and_then(CallTargetFacts::selected_application)
                .filter(|prefix| {
                    prefix.core().digest() == continuation.prefix_application_core()
                        && prefix.core().stable_site() == continuation.prefix_application_site()
                        && prefix.core().site() == continuation.prefix_call_site()
                })
                .ok_or_else(|| RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "line schedule completion has no exact checked prefix application"
                        .to_owned(),
                })?;
            let anchor = checked_call_parameter_expression(owner, prefix, 0, 0)?;
            let callback = checked_call_parameter_expression(owner, application, 1, 0)?;
            RuntimeLineCallable::Schedule { anchor, callback }
        }
        _ => return Ok(None),
    };
    Ok(Some(line))
}

const fn runtime_agent_intrinsic(intrinsic: AgentIntrinsicSignatureId) -> RuntimeAgentIntrinsic {
    match intrinsic {
        AgentIntrinsicSignatureId::Observe => RuntimeAgentIntrinsic::Observe,
        AgentIntrinsicSignatureId::Expect => RuntimeAgentIntrinsic::Expect,
        AgentIntrinsicSignatureId::Deny => RuntimeAgentIntrinsic::Deny,
        AgentIntrinsicSignatureId::Checkpoint => RuntimeAgentIntrinsic::Checkpoint,
        AgentIntrinsicSignatureId::Note => RuntimeAgentIntrinsic::Note,
        AgentIntrinsicSignatureId::Attach => RuntimeAgentIntrinsic::Attach,
        AgentIntrinsicSignatureId::ChoiceAction => RuntimeAgentIntrinsic::ChoiceAction,
        AgentIntrinsicSignatureId::Viewport => RuntimeAgentIntrinsic::Viewport,
        AgentIntrinsicSignatureId::Layer => RuntimeAgentIntrinsic::Layer,
        AgentIntrinsicSignatureId::Object => RuntimeAgentIntrinsic::Object,
        AgentIntrinsicSignatureId::Capture => RuntimeAgentIntrinsic::Capture,
        AgentIntrinsicSignatureId::ReadResource => RuntimeAgentIntrinsic::ReadResource,
        AgentIntrinsicSignatureId::EntityMeta => RuntimeAgentIntrinsic::EntityMeta,
        AgentIntrinsicSignatureId::ProjectNeighbors => RuntimeAgentIntrinsic::ProjectNeighbors,
        AgentIntrinsicSignatureId::Signal => RuntimeAgentIntrinsic::Signal,
        AgentIntrinsicSignatureId::Metric => RuntimeAgentIntrinsic::Metric,
        AgentIntrinsicSignatureId::StatePath => RuntimeAgentIntrinsic::StatePath,
        AgentIntrinsicSignatureId::ObservationPath => RuntimeAgentIntrinsic::ObservationPath,
        AgentIntrinsicSignatureId::State => RuntimeAgentIntrinsic::State,
        AgentIntrinsicSignatureId::Observation => RuntimeAgentIntrinsic::Observation,
        AgentIntrinsicSignatureId::Diagnostics => RuntimeAgentIntrinsic::Diagnostics,
        AgentIntrinsicSignatureId::Exists => RuntimeAgentIntrinsic::Exists,
        AgentIntrinsicSignatureId::ActionEnabled => RuntimeAgentIntrinsic::ActionEnabled,
        AgentIntrinsicSignatureId::All => RuntimeAgentIntrinsic::All,
        AgentIntrinsicSignatureId::Any => RuntimeAgentIntrinsic::Any,
        AgentIntrinsicSignatureId::Not => RuntimeAgentIntrinsic::Not,
        AgentIntrinsicSignatureId::Wait => RuntimeAgentIntrinsic::Wait,
        AgentIntrinsicSignatureId::AdvanceText => RuntimeAgentIntrinsic::AdvanceText,
        AgentIntrinsicSignatureId::ViewportPoint => RuntimeAgentIntrinsic::ViewportPoint,
        AgentIntrinsicSignatureId::PointerClick => RuntimeAgentIntrinsic::PointerClick,
        AgentIntrinsicSignatureId::Invoke => RuntimeAgentIntrinsic::Invoke,
        AgentIntrinsicSignatureId::RagQuery => RuntimeAgentIntrinsic::RagQuery,
    }
}

const fn runtime_agent_probe_comparison(
    operation: ProbeComparisonOperator,
) -> arcweft_core::value::RuntimeAgentCompareOp {
    use arcweft_core::value::RuntimeAgentCompareOp;
    match operation {
        ProbeComparisonOperator::Eq => RuntimeAgentCompareOp::Eq,
        ProbeComparisonOperator::NotEq => RuntimeAgentCompareOp::NotEq,
        ProbeComparisonOperator::Greater => RuntimeAgentCompareOp::Greater,
        ProbeComparisonOperator::GreaterOrEqual => RuntimeAgentCompareOp::GreaterOrEqual,
        ProbeComparisonOperator::Less => RuntimeAgentCompareOp::Less,
        ProbeComparisonOperator::LessOrEqual => RuntimeAgentCompareOp::LessOrEqual,
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the builtin-to-runtime intrinsic table is intentionally exhaustive and declarative"
)]
fn runtime_intrinsic(
    candidate: &arcweft_lang_sema::callable::CallableCandidateId,
) -> Option<RuntimeIntrinsic> {
    if let arcweft_lang_sema::callable::CallableCandidateId::CapacityMethod(method) = candidate {
        return match (method.receiver(), method.method().as_str()) {
            (TypeKind::String, "trim") => Some(RuntimeIntrinsic::StringTrim),
            (TypeKind::String, "to_string") => Some(RuntimeIntrinsic::StringToString),
            _ => None,
        };
    }
    let builtin = match candidate {
        arcweft_lang_sema::callable::CallableCandidateId::Builtin(builtin) => builtin,
        _ => return None,
    };
    Some(match builtin {
        BuiltinCallableId::Math(MathCallableId::MatMulF32) => RuntimeIntrinsic::MathMatmulF32,
        BuiltinCallableId::Math(MathCallableId::MatrixAddF32) => RuntimeIntrinsic::MathMatrixAddF32,
        BuiltinCallableId::Math(MathCallableId::TensorAddF32) => RuntimeIntrinsic::MathTensorAddF32,
        BuiltinCallableId::Math(MathCallableId::MatMulF64) => RuntimeIntrinsic::MathMatmulF64,
        BuiltinCallableId::Math(MathCallableId::MatrixAddF64) => RuntimeIntrinsic::MathMatrixAddF64,
        BuiltinCallableId::Math(MathCallableId::TensorAddF64) => RuntimeIntrinsic::MathTensorAddF64,
        BuiltinCallableId::StdFloat(float) => match (float.width(), float.operation()) {
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Abs) => {
                RuntimeIntrinsic::StdF32Abs
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Floor) => {
                RuntimeIntrinsic::StdF32Floor
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Ceil) => {
                RuntimeIntrinsic::StdF32Ceil
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Round) => {
                RuntimeIntrinsic::StdF32Round
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Trunc) => {
                RuntimeIntrinsic::StdF32Trunc
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Fract) => {
                RuntimeIntrinsic::StdF32Fract
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Sqrt) => {
                RuntimeIntrinsic::StdF32Sqrt
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Sin) => {
                RuntimeIntrinsic::StdF32Sin
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Cos) => {
                RuntimeIntrinsic::StdF32Cos
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Tan) => {
                RuntimeIntrinsic::StdF32Tan
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Exp) => {
                RuntimeIntrinsic::StdF32Exp
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Exp2) => {
                RuntimeIntrinsic::StdF32Exp2
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Ln) => {
                RuntimeIntrinsic::StdF32Ln
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Log2) => {
                RuntimeIntrinsic::StdF32Log2
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Log10) => {
                RuntimeIntrinsic::StdF32Log10
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Powf) => {
                RuntimeIntrinsic::StdF32Powf
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::Atan2) => {
                RuntimeIntrinsic::StdF32Atan2
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::MulAdd) => {
                RuntimeIntrinsic::StdF32MulAdd
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::IsNan) => {
                RuntimeIntrinsic::StdF32IsNan
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::IsInfinite) => {
                RuntimeIntrinsic::StdF32IsInfinite
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::IsFinite) => {
                RuntimeIntrinsic::StdF32IsFinite
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::IsSignPositive) => {
                RuntimeIntrinsic::StdF32IsSignPositive
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::IsSignNegative) => {
                RuntimeIntrinsic::StdF32IsSignNegative
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::ToBits) => {
                RuntimeIntrinsic::StdF32ToBits
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::FromBits) => {
                RuntimeIntrinsic::StdF32FromBits
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::ToF64) => {
                RuntimeIntrinsic::StdF32ToF64
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Abs) => {
                RuntimeIntrinsic::StdF64Abs
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Floor) => {
                RuntimeIntrinsic::StdF64Floor
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Ceil) => {
                RuntimeIntrinsic::StdF64Ceil
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Round) => {
                RuntimeIntrinsic::StdF64Round
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Trunc) => {
                RuntimeIntrinsic::StdF64Trunc
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Fract) => {
                RuntimeIntrinsic::StdF64Fract
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Sqrt) => {
                RuntimeIntrinsic::StdF64Sqrt
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Sin) => {
                RuntimeIntrinsic::StdF64Sin
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Cos) => {
                RuntimeIntrinsic::StdF64Cos
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Tan) => {
                RuntimeIntrinsic::StdF64Tan
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Exp) => {
                RuntimeIntrinsic::StdF64Exp
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Exp2) => {
                RuntimeIntrinsic::StdF64Exp2
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Ln) => {
                RuntimeIntrinsic::StdF64Ln
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Log2) => {
                RuntimeIntrinsic::StdF64Log2
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Log10) => {
                RuntimeIntrinsic::StdF64Log10
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Powf) => {
                RuntimeIntrinsic::StdF64Powf
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::Atan2) => {
                RuntimeIntrinsic::StdF64Atan2
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::MulAdd) => {
                RuntimeIntrinsic::StdF64MulAdd
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::IsNan) => {
                RuntimeIntrinsic::StdF64IsNan
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::IsInfinite) => {
                RuntimeIntrinsic::StdF64IsInfinite
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::IsFinite) => {
                RuntimeIntrinsic::StdF64IsFinite
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::IsSignPositive) => {
                RuntimeIntrinsic::StdF64IsSignPositive
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::IsSignNegative) => {
                RuntimeIntrinsic::StdF64IsSignNegative
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::ToBits) => {
                RuntimeIntrinsic::StdF64ToBits
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::FromBits) => {
                RuntimeIntrinsic::StdF64FromBits
            }
            (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::ToF32) => {
                RuntimeIntrinsic::StdF64ToF32
            }
            (arcweft_lang_sema::callable::FloatWidth::F32, StdFloatOperation::ToF32)
            | (arcweft_lang_sema::callable::FloatWidth::F64, StdFloatOperation::ToF64) => {
                return None;
            }
        },
        BuiltinCallableId::InlineFailureFallback
        | BuiltinCallableId::Panic
        | BuiltinCallableId::Fail
        | BuiltinCallableId::Bail
        | BuiltinCallableId::Ensure
        | BuiltinCallableId::Rgb
        | BuiltinCallableId::Sin
        | BuiltinCallableId::Cos
        | BuiltinCallableId::Vector { .. }
        | BuiltinCallableId::Capability(_)
        | BuiltinCallableId::Reduction(_) => return None,
    })
}
