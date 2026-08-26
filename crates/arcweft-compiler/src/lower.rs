//! Final semantic projection into runtime-plan facts.
//!
//! This module is the compiler-owned dependency inversion boundary between
//! semantic analysis and runtime-plan lowering. It consumes the exact accepted
//! final-HIR generation and never opens source text, rebuilds a detached HIR,
//! or consults the removed `TypeCheckReport` sidecar.

#[path = "lower/reachability.rs"]
mod reachability;

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
    runtime_id::RuntimeDialogueValueSlotId,
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
    DialoguePresentationProfile, DialogueProfileRevision, InlineFailurePolicy,
    character_presentation::{
        CharacterPresentationTargetEvidence, CheckedCharacterPresentationPlan,
    },
};
use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, ItemId, LocalId, PatternId, StmtId},
    item::{HirCharacterSurfaceAlias, HirDeclarationMemberKind, HirItemKind, HirRetainedName},
    leaf::{
        HirBigUint, HirCharacterLiteral, HirDecimal, HirDurationLiteral, HirFloatLiteral,
        HirIntegerLiteral, HirLiteral, HirStringLiteral, HirUnitNumberLiteral,
    },
    project::{
        HirExecutableProjectView, HirProjectItemRef, HirRuntimeReachabilityError,
        HirRuntimeSemanticReachability, HirSelectedExpressionInventoryError,
    },
    scope::HirScopeOwner,
    stmt::HirStmtKind,
    symbol::{
        CallableDeclarationKey, ImplMethodDeclarationId, ProjectSymbolTable,
        nominal::ProjectNominalDeclarationId,
    },
};
use arcweft_lang_sema::{
    assertion::AssertionRuntimePolicy,
    callable::{
        AgentIntrinsicSignatureId, BuiltinCallableId, CallTargetFacts, CallableCandidateId,
        CallableFamily, CallableLogLevel, CheckedCallApplication, CheckedCallArgumentPassing,
        CheckedCallCalleeExecution, CheckedCallOperandDestination, CheckedCallRuntimeOperand,
        CheckedCallRuntimeOperandOrder, CheckedCallableExecution, DomainMethodId, MathCallableId,
        ProbeComparisonOperator, ReductionConstructorKind, ResolvedCallableBaseInstantiation,
        ResolvedCallableOrigin, StdFloatOperation,
    },
    checked_rich_text::{
        CheckedDialogueControl, CheckedDialogueHostEvent, CheckedDialogueToken,
        CheckedDirectStyleSpan, CheckedRichTextAction, CheckedRichTextReport, CheckedVoiceSource,
        LengthUnit,
    },
    effects::EffectId,
    env::nominal::AcceptedNominalId,
    env::nominal::AcceptedNominalSemantics,
    final_analysis::{
        CheckedAssertionDisposition, CheckedAssignment, CheckedCharacterDialogueTarget,
        CheckedEvaluatedEffect, CheckedExpressionEdgeError, CheckedExpressionResolution,
        CheckedItemRole, CheckedIteration, CheckedIteratorFamily, CheckedPatternResolution,
        CheckedProjectItemOwner, CheckedProjectNominal, CheckedRecordPattern,
        CheckedRecordPatternRest, CheckedRecordPatternSourceRef, CheckedRecordValueSource,
        CheckedSelectResolution, CheckedStatementRole, CheckedTraitConformance,
        CheckedTraitIdentity, CheckedTry, CheckedTryBoundary, CheckedTryCarrier,
        CheckedValueResolution, CheckedVariantOwner, CheckedVariantResolution,
        FinalSemanticAnalysis, FinalSemanticAnalysisError, NominalSchemaPath,
        NominalSchemaProjectionError,
    },
    registration::RegisteredSemanticWorld,
    types::{
        AgentBuiltinType, ArrayLength, CheckedConstraintContainerConstructor,
        CheckedConstraintSourceProjection, IteratorStateKind, MapKind, SemanticTypeDigest,
        TypeKind,
    },
};
use arcweft_manifest_model::CharacterNameLocalePolicySpec;
use arcweft_runtime_plan::{
    agent::RuntimeAgentIntrinsic,
    assertion_identity::RuntimeAssertionMode,
    semantic_facts::{
        RuntimeAgentTypeShape, RuntimeAssertionAdmission, RuntimeAssignmentFact, RuntimeAwaitFact,
        RuntimeAwaitPendingObserverFact, RuntimeBuiltinIteratorFact, RuntimeCallResultShape,
        RuntimeCheckedCapture, RuntimeCheckedTypeProjectionError, RuntimeChoiceFact,
        RuntimeChoiceGotoFact, RuntimeDialogueApplication, RuntimeDialogueEffectExpression,
        RuntimeDialogueEffectTrigger, RuntimeDialogueValueExpression, RuntimeEffectFieldFact,
        RuntimeEvaluatedEffect, RuntimeEvaluatedEffectFact, RuntimeImplicitCallableFact,
        RuntimeIteratorFact, RuntimeIteratorWitnessExecutableFact, RuntimeIteratorWitnessFact,
        RuntimeLogLevel, RuntimeMapKind, RuntimeNominalRecordFactError, RuntimeNormalizedType,
        RuntimeNormalizedVariantCase, RuntimePipeFact, RuntimePlanSemanticFactInput,
        RuntimePlanSemanticFacts, RuntimePositionedCallOperand, RuntimeProjectCallable,
        RuntimeProjectItem, RuntimePureProgramCaptureFact, RuntimePureProgramFact,
        RuntimeRecordExpressionFact, RuntimeRecordExpressionField, RuntimeRecordExpressionSource,
        RuntimeRecordPatternFact, RuntimeRecordPatternField, RuntimeRecordPatternRest,
        RuntimeRecordPatternSource, RuntimeRecordPlanError, RuntimeReductionConstructor,
        RuntimeRegisteredValueId, RuntimeResolvedCall, RuntimeResolvedCallDispatch,
        RuntimeResolvedCallOperand, RuntimeResolvedCallOperandBinding,
        RuntimeResolvedCallOperandOrigin, RuntimeResolvedCallOperandProjection,
        RuntimeResolvedCallOperandSource, RuntimeResolvedHostCall, RuntimeResolvedNominal,
        RuntimeResolvedNominalRecord, RuntimeResolvedSelect, RuntimeResolvedSpreadContainer,
        RuntimeResolvedStaticCallTarget, RuntimeResolvedValue, RuntimeResolvedVariant,
        RuntimeSemanticFactsError, RuntimeSemanticTypeId, RuntimeSequenceKind,
        RuntimeTraitIdentity, RuntimeTraitMethodFact, RuntimeTryBoundaryOwner,
        RuntimeTryCarrierFact, RuntimeTryFact, RuntimeTypeProjectionPath,
        RuntimeTypeProjectionStep, RuntimeTypeShape,
    },
};
use arcweft_source::ProductSourceRef;
use arcweft_text_model::{
    DialogueContentSpec, DialogueHostEvent, DialoguePresentationSnapshot, DialogueVoiceSource,
    Milli, RichTextAngle, RichTextColor, RichTextControl, RichTextDocument, RichTextFontFamily,
    RichTextNode, RichTextSpanKind, RichTextStyle,
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
    Generation(Box<FinalSemanticAnalysisError>),
    #[error(transparent)]
    Facts(Box<RuntimeSemanticFactsError>),
    #[error(transparent)]
    LocalDeclarations(#[from] RuntimeLocalDeclarationTableError),
    #[error(transparent)]
    ExpressionTypeInventory(#[from] HirSelectedExpressionInventoryError),
    #[error(transparent)]
    RuntimeReachability(#[from] HirRuntimeReachabilityError),
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
pub fn project_runtime_semantic_facts(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    dialogue_profile: Option<(&DialoguePresentationProfile, &DialogueProfileRevision)>,
    character_name_policy: Option<&CharacterNameLocalePolicySpec>,
) -> Result<RuntimePlanSemanticFacts, RuntimeSemanticProjectionError> {
    project_runtime_semantic_fact_inventories(
        project,
        symbols,
        world,
        analysis,
        runtime_owners,
        None,
        &[],
        dialogue_profile,
        character_name_policy,
    )
}

pub(crate) fn project_runtime_semantic_facts_with_view_value_programs(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    view_value_owners: &HirRuntimeSemanticReachability<'_>,
    pure_programs: &[crate::view::CheckedViewHandlerProgram],
    dialogue_profile: Option<(&DialoguePresentationProfile, &DialogueProfileRevision)>,
    character_name_policy: Option<&CharacterNameLocalePolicySpec>,
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
    )
}

fn project_runtime_semantic_fact_inventories(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    view_value_owners: Option<&HirRuntimeSemanticReachability<'_>>,
    pure_programs: &[crate::view::CheckedViewHandlerProgram],
    dialogue_profile: Option<(&DialoguePresentationProfile, &DialogueProfileRevision)>,
    character_name_policy: Option<&CharacterNameLocalePolicySpec>,
) -> Result<RuntimePlanSemanticFacts, RuntimeSemanticProjectionError> {
    analysis.validate_generation(project, symbols)?;
    validate_executable_record_projections(world, analysis, runtime_owners)?;
    if let Some(view_value_owners) = view_value_owners {
        validate_executable_record_projections(world, analysis, view_value_owners)?;
    }
    let dialogue_application_calls =
        dialogue_application_owned_calls(project, analysis, runtime_owners)?;
    let mut evaluated_effect_calls = BTreeSet::new();
    for (statement, checked) in analysis.statements() {
        if !matches!(checked.role(), CheckedStatementRole::EvaluatedEffect(_)) {
            continue;
        }
        let module = project
            .modules()
            .find_map(|(_, module)| {
                (module.module_id() == statement.module()).then_some(module.as_ref())
            })
            .ok_or(
                RuntimeSemanticProjectionError::InvalidEvaluatedEffectDisposition {
                    owner: statement,
                },
            )?;
        let HirStmtKind::Expression { expression } = module
            .resolve_stmt(statement)
            .map_err(
                |_| RuntimeSemanticProjectionError::InvalidEvaluatedEffectDisposition {
                    owner: statement,
                },
            )?
            .kind()
        else {
            return Err(
                RuntimeSemanticProjectionError::InvalidEvaluatedEffectDisposition {
                    owner: statement,
                },
            );
        };
        evaluated_effect_calls.insert(*expression);
    }
    let runtime_calls = analysis
        .calls()
        .filter(|(owner, _)| {
            (runtime_owners.contains_expression(*owner)
                || view_value_owners.is_some_and(|owners| owners.contains_expression(*owner)))
                && !dialogue_application_calls.contains(owner)
                && !evaluated_effect_calls.contains(owner)
        })
        .map(|(owner, call)| {
            runtime_call(owner, call, symbols, world, analysis).map(|call| (owner, call))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
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
            input.push_flow(owner, identity);
        }
    }

    for (owner, ty) in analysis.types() {
        if runtime_owners.contains_type(owner)
            || view_value_owners.is_some_and(|owners| owners.contains_type(owner))
        {
            input.push_type(owner, runtime_type(ty, symbols, world, analysis)?);
        }
    }

    for (owner, expression) in analysis.expressions() {
        if !runtime_owners.contains_expression(owner)
            && !view_value_owners.is_some_and(|owners| owners.contains_expression(owner))
        {
            continue;
        }
        if runtime_expression_type_owners.contains(&owner) {
            input.push_expression_type(
                owner,
                runtime_type(expression.ty(), symbols, world, analysis)?,
            );
        }
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
                    let TypeKind::Vec(item) = expression.ty() else {
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
                    runtime_literal(literal, expression.ty()).map_err(|reason| {
                        RuntimeSemanticProjectionError::ExpressionLiteral { owner, reason }
                    })?,
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
                if let Some(value) =
                    runtime_value_resolution(value, expression.ty(), project_item_is_runtime_entity)
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
            CheckedExpressionResolution::PostfixBracket(resolution) => {
                input.push_postfix_candidate(owner, resolution.candidate());
            }
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
            CheckedExpressionResolution::Try(tried) => {
                push_runtime_try_fact(&mut input, owner, tried, project, symbols, world, analysis)?;
            }
            CheckedExpressionResolution::ImplicitCallable(callable) => {
                input.push_implicit_callable(
                    owner,
                    RuntimeImplicitCallableFact::new(
                        runtime_type(callable.parameter(), symbols, world, analysis)?,
                        runtime_type(callable.result(), symbols, world, analysis)?,
                        callable.placeholders().into(),
                        callable
                            .captures()
                            .iter()
                            .map(arcweft_lang_sema::final_analysis::CheckedCapture::local)
                            .collect::<Box<[_]>>(),
                    ),
                );
                if let CheckedExpressionResolution::Try(tried) = callable.body_resolution() {
                    push_runtime_try_fact(
                        &mut input, owner, tried, project, symbols, world, analysis,
                    )?;
                }
            }
            CheckedExpressionResolution::Pipe(pipe) => {
                input.push_pipe(
                    owner,
                    RuntimePipeFact::new(pipe.left(), pipe.right(), pipe.placeholders().into()),
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
            CheckedExpressionResolution::ImplicitParameter { .. }
            | CheckedExpressionResolution::PipeLeft { .. }
            | CheckedExpressionResolution::DialogueLineCoordinate(_)
            | CheckedExpressionResolution::DialogueTextKeyCoordinate(_)
            | CheckedExpressionResolution::CharacterDialogueFactory(_)
            | CheckedExpressionResolution::CharacterDialogueReconfigure(_)
            | CheckedExpressionResolution::Call
            | CheckedExpressionResolution::ViewCall(_)
            | CheckedExpressionResolution::ViewCallee(_)
            | CheckedExpressionResolution::StyleValue(_)
            | CheckedExpressionResolution::StyleCallee(_)
            | CheckedExpressionResolution::StageLook(_)
            | CheckedExpressionResolution::DialogueApplication { .. }
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
        match statement.role() {
            CheckedStatementRole::Assignment(assignment) => {
                input.push_assignment(
                    owner,
                    runtime_assignment(owner, assignment, symbols, world, analysis)?,
                );
            }
            CheckedStatementRole::Assertion(disposition) => {
                input.push_assertion(owner, runtime_assertion(owner, *disposition)?);
            }
            CheckedStatementRole::EvaluatedEffect(effect) => {
                input.push_evaluated_effect(
                    owner,
                    runtime_evaluated_effect(owner, effect, project)?,
                );
            }
            CheckedStatementRole::Iteration(iteration) => {
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
            CheckedStatementRole::Suspension(_)
            | CheckedStatementRole::Ordinary
            | CheckedStatementRole::Yield
            | CheckedStatementRole::UnsafeAudit => {}
        }
    }

    for (owner, capture) in analysis.captures() {
        if !runtime_owners.contains_capture(owner)
            && !view_value_owners.is_some_and(|owners| owners.contains_capture(owner))
        {
            continue;
        }
        input.push_capture(RuntimeCheckedCapture::new(
            owner,
            runtime_type(capture.ty(), symbols, world, analysis)?,
        ));
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

    let facts = match view_value_owners {
        Some(view_value_owners) => RuntimePlanSemanticFacts::try_new_with_view_value_programs(
            project,
            runtime_owners,
            view_value_owners,
            input,
        )?,
        None => RuntimePlanSemanticFacts::try_new(project, runtime_owners, input)?,
    };
    project_dialogue_semantic_facts(
        project,
        analysis,
        dialogue_profile,
        character_name_policy,
        runtime_owners,
        facts,
    )
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
    tried: &CheckedTry,
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), RuntimeSemanticProjectionError> {
    let module = project
        .modules()
        .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module.as_ref()))
        .ok_or(RuntimeSemanticProjectionError::MissingModule { owner })?;
    let (boundary, boundary_ty) = match tried.boundary() {
        CheckedTryBoundary::Infallible => {
            let checked = analysis
                .expression(tried.operand())
                .ok_or(RuntimeSemanticProjectionError::InvalidTryBoundary { owner })?;
            (RuntimeTryBoundaryOwner::Infallible, checked.ty())
        }
        CheckedTryBoundary::CarrierBlock(boundary) => {
            let checked = analysis
                .expression(boundary)
                .ok_or(RuntimeSemanticProjectionError::InvalidTryBoundary { owner })?;
            (
                RuntimeTryBoundaryOwner::CarrierBlock(boundary),
                checked.ty(),
            )
        }
        CheckedTryBoundary::FunctionSite(boundary) => {
            let checked = analysis
                .expression(boundary)
                .ok_or(RuntimeSemanticProjectionError::InvalidTryBoundary { owner })?;
            let TypeKind::Function { return_type, .. } = checked.ty() else {
                return Err(RuntimeSemanticProjectionError::InvalidTryBoundary { owner });
            };
            (
                RuntimeTryBoundaryOwner::FunctionSite(boundary),
                return_type.as_ref(),
            )
        }
        CheckedTryBoundary::Callable(boundary) => {
            let item = module
                .resolve_item(boundary)
                .map_err(|_| RuntimeSemanticProjectionError::InvalidTryBoundary { owner })?;
            let return_type = match item.kind() {
                HirItemKind::Function(function) => function.return_type(),
                HirItemKind::Flow(flow) => flow.result().authored_type(),
                _ => None,
            }
            .ok_or(RuntimeSemanticProjectionError::InvalidTryBoundary { owner })?;
            let checked = analysis
                .ty(return_type)
                .ok_or(RuntimeSemanticProjectionError::InvalidTryBoundary { owner })?;
            (RuntimeTryBoundaryOwner::Callable(boundary), checked)
        }
    };
    let carrier = match tried.carrier() {
        CheckedTryCarrier::Result { success, residual } => RuntimeTryCarrierFact::Result {
            success: runtime_type(success, symbols, world, analysis)?,
            residual: Box::new(runtime_type(residual, symbols, world, analysis)?),
        },
        CheckedTryCarrier::Option { success } => RuntimeTryCarrierFact::Option {
            success: runtime_type(success, symbols, world, analysis)?,
        },
    };
    input.push_try(
        owner,
        RuntimeTryFact::new(
            tried.operand(),
            runtime_type(
                analysis
                    .expression(tried.operand())
                    .ok_or(RuntimeSemanticProjectionError::InvalidTryBoundary { owner })?
                    .ty(),
                symbols,
                world,
                analysis,
            )?,
            carrier,
            boundary,
            runtime_type(boundary_ty, symbols, world, analysis)?,
        ),
    );
    Ok(())
}

fn runtime_assignment(
    owner: StmtId,
    assignment: &CheckedAssignment,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
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
        runtime_nominal(place.nominal(), analysis)?,
        field.field().runtime_field(),
        runtime_type(place.field_type(), symbols, world, analysis)?,
        runtime_type(assignment.value_type(), symbols, world, analysis)?,
    ))
}

fn dialogue_application_owned_calls(
    project: HirExecutableProjectView<'_>,
    analysis: &FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
) -> Result<BTreeSet<ExprId>, RuntimeSemanticProjectionError> {
    analysis
        .expressions()
        .filter_map(|(owner, expression)| {
            (runtime_owners.contains_expression(owner)
                && matches!(
                    expression.resolution(),
                    CheckedExpressionResolution::DialogueApplication { .. }
                ))
            .then_some(owner)
        })
        .try_fold(BTreeSet::new(), |mut calls, owner| {
            let module = project
                .modules()
                .find_map(|(_, module)| {
                    (module.module_id() == owner.module()).then_some(module.as_ref())
                })
                .ok_or(RuntimeSemanticProjectionError::MissingModule { owner })?;
            let expression = module.resolve_expr(owner).map_err(|error| {
                RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(owner),
                    reason: error.to_string(),
                }
            })?;
            let HirExprKind::DialogueContentApplication(application) = expression.kind() else {
                return Err(RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(owner),
                    reason: "checked dialogue application has a different final-HIR family"
                        .to_owned(),
                });
            };
            calls.insert(owner);
            if analysis.call(application.target()).is_some() {
                calls.insert(application.target());
            }
            Ok(calls)
        })
}

fn project_dialogue_semantic_facts(
    project: HirExecutableProjectView<'_>,
    analysis: &FinalSemanticAnalysis,
    dialogue_profile: Option<(&DialoguePresentationProfile, &DialogueProfileRevision)>,
    policy: Option<&CharacterNameLocalePolicySpec>,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
    facts: RuntimePlanSemanticFacts,
) -> Result<RuntimePlanSemanticFacts, RuntimeSemanticProjectionError> {
    let applications = executable_dialogue_applications(project, analysis, runtime_owners)?;
    if applications.is_empty() {
        return facts
            .with_dialogue_projection(project, runtime_owners, None, [])
            .map_err(Into::into);
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
    let catalog = Arc::new(build_character_presentation_catalog(
        project, analysis, policy,
    )?);
    let generation = CharacterPresentationCatalogGeneration::new(
        CharacterPresentationCatalogRevision::INITIAL,
        catalog.semantic_digest(),
        catalog.locale_policy_digest(),
    );
    let presentation = DialoguePresentationSnapshot::new(
        dialogue_profile.clone(),
        dialogue_profile_revision.clone(),
    );
    let mut projected = Vec::with_capacity(applications.len());
    for (owner, target, rich_text) in applications {
        projected.push(project_dialogue_application(
            project,
            owner,
            target,
            rich_text,
            generation,
            presentation.clone(),
        )?);
    }
    facts
        .with_dialogue_projection(project, runtime_owners, Some(catalog), projected)
        .map_err(Into::into)
}

type CheckedDialogueApplication<'analysis> = (
    ExprId,
    &'analysis CheckedCharacterDialogueTarget,
    &'analysis CheckedRichTextReport,
);

fn executable_dialogue_applications<'analysis>(
    project: HirExecutableProjectView<'_>,
    analysis: &'analysis FinalSemanticAnalysis,
    runtime_owners: &HirRuntimeSemanticReachability<'_>,
) -> Result<Vec<CheckedDialogueApplication<'analysis>>, RuntimeSemanticProjectionError> {
    analysis
        .expressions()
        .filter_map(|(owner, expression)| match expression.resolution() {
            CheckedExpressionResolution::DialogueApplication {
                target, rich_text, ..
            } if runtime_owners.contains_expression(owner) => {
                Some((owner, target, rich_text.as_ref()))
            }
            _ => None,
        })
        .try_fold(Vec::new(), |mut applications, application| {
            let (owner, _, _) = application;
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
    project: HirExecutableProjectView<'_>,
    owner: ExprId,
    target: &CheckedCharacterDialogueTarget,
    rich_text: &CheckedRichTextReport,
    generation: CharacterPresentationCatalogGeneration,
    presentation: DialoguePresentationSnapshot,
) -> Result<(ExprId, RuntimeDialogueApplication), RuntimeSemanticProjectionError> {
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
    let line = project.dialogue_lines().for_expr(owner).ok_or_else(|| {
        RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: "dialogue application has no accepted line identity".to_owned(),
        }
    })?;
    let runtime_line =
        RuntimeLineId::from_source_entity_body(line.id().as_str()).map_err(|error| {
            RuntimeSemanticProjectionError::Dialogue {
                owner: Some(owner),
                reason: error.to_string(),
            }
        })?;
    let (content, values, effects) = lower_checked_rich_text(owner, rich_text)?;
    let source = ProductSourceRef::try_for_identity(line.source().application_span().source())
        .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
            owner: Some(owner),
            reason: error.to_string(),
        })?;
    Ok((
        owner,
        RuntimeDialogueApplication::new(
            DialogueContentSpec::new(
                runtime_line,
                line.text_key().as_text_key().clone(),
                content,
                plan,
                presentation,
                Vec::new(),
                source,
            ),
            values,
            effects,
        ),
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
    project: HirExecutableProjectView<'_>,
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
        .display_name()
        .map(|member| {
            let member = item
                .module()
                .declaration_members()
                .resolve(member)
                .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
                    owner: None,
                    reason: error.to_string(),
                })?;
            let HirDeclarationMemberKind::CharacterDisplayName(member) = member.kind() else {
                return Err(RuntimeSemanticProjectionError::Dialogue {
                    owner: None,
                    reason: "Character display_name member has the wrong typed family".to_owned(),
                });
            };
            let initializer =
                member
                    .initializer()
                    .ok_or_else(|| RuntimeSemanticProjectionError::Dialogue {
                        owner: None,
                        reason: "Character display_name has no checked initializer".to_owned(),
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
                    reason: "Character display_name must be a checked constant String".to_owned(),
                })?;
            CharacterDisplayNameValue::try_new(value)
                .map(CharacterDisplayNameInput::Visible)
                .map_err(|error| RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(initializer),
                    reason: error.to_string(),
                })
        })
        .transpose()?;
    let fallback = match character.surface_alias() {
        HirCharacterSurfaceAlias::Resolved(alias) => Some(alias.as_str()),
        HirCharacterSurfaceAlias::Absent | HirCharacterSurfaceAlias::Missing => {
            match character.header().name() {
                HirRetainedName::Resolved(name) => Some(name.as_str()),
                HirRetainedName::Missing | HirRetainedName::Invalid => None,
            }
        }
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
    report: &CheckedRichTextReport,
) -> Result<
    (
        RichTextDocument,
        Vec<RuntimeDialogueValueExpression>,
        Vec<RuntimeDialogueEffectExpression>,
    ),
    RuntimeSemanticProjectionError,
> {
    let mut nodes = Vec::new();
    let mut values = Vec::new();
    let mut effects = Vec::new();
    let mut spans = BTreeMap::new();
    for token in report.content().tokens() {
        match token {
            CheckedDialogueToken::Text(text) => nodes.push(RichTextNode::Text {
                text: text.to_string(),
            }),
            CheckedDialogueToken::RawText(text) => nodes.push(RichTextNode::Control {
                control: RichTextControl::Raw {
                    text: text.to_string(),
                },
            }),
            CheckedDialogueToken::Escape(value) => nodes.push(RichTextNode::Text {
                text: value.to_string(),
            }),
            CheckedDialogueToken::Ruby { base, ruby } => nodes.push(RichTextNode::Ruby {
                base: base.to_string(),
                ruby: ruby.to_string(),
            }),
            CheckedDialogueToken::Interpolation(expression) => {
                let slot = next_dialogue_slot(owner, values.len())?;
                values.push(RuntimeDialogueValueExpression {
                    slot,
                    role: RuntimeDialogueValueRole::Interpolation,
                    expression: *expression,
                });
                nodes.push(RichTextNode::Interpolation {
                    slot,
                    label: format!("{expression:?}"),
                    on_error: InlineFailurePolicy::FailLine,
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
            CheckedDialogueToken::Open(tag) => {
                if let Some(span) = lower_rich_text_action(
                    owner,
                    tag.action(),
                    &mut nodes,
                    &mut values,
                    &mut effects,
                )? {
                    spans.insert(tag.id(), span);
                }
            }
            CheckedDialogueToken::Close(close) => {
                if let Some(span) = spans.get(&close.open()).copied() {
                    nodes.push(RichTextNode::StyleEnd { span });
                }
            }
            CheckedDialogueToken::InvalidTag { .. } => {
                return Err(RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(owner),
                    reason: "invalid RichText tag reached executable projection".to_owned(),
                });
            }
        }
    }
    Ok((RichTextDocument::new(nodes), values, effects))
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

fn lower_rich_text_action(
    owner: ExprId,
    action: &CheckedRichTextAction,
    nodes: &mut Vec<RichTextNode>,
    values: &mut Vec<RuntimeDialogueValueExpression>,
    effects: &mut Vec<RuntimeDialogueEffectExpression>,
) -> Result<Option<RichTextSpanKind>, RuntimeSemanticProjectionError> {
    let style = match action {
        CheckedRichTextAction::DirectStyle { action, .. } => Some(match action {
            CheckedDirectStyleSpan::Emphasis => RichTextStyle::Em,
            CheckedDirectStyleSpan::Strong => RichTextStyle::Strong,
            CheckedDirectStyleSpan::Italic => RichTextStyle::Italic,
            CheckedDirectStyleSpan::Oblique { angle } => RichTextStyle::Oblique {
                angle: RichTextAngle {
                    degrees: Milli(angle.milli_degrees),
                },
            },
            CheckedDirectStyleSpan::Color { value } => RichTextStyle::Color {
                value: match value {
                    arcweft_lang_sema::checked_rich_text::CheckedColor::Rgba8(value) => {
                        RichTextColor::Rgba8 { value: *value }
                    }
                    arcweft_lang_sema::checked_rich_text::CheckedColor::Resource(id) => {
                        RichTextColor::Resource {
                            id: id.as_str().to_owned(),
                        }
                    }
                },
            },
            CheckedDirectStyleSpan::Font { family } => RichTextStyle::Font {
                family: RichTextFontFamily::Named {
                    name: family.clone(),
                },
            },
            CheckedDirectStyleSpan::Size { value } if value.unit == LengthUnit::Pt => {
                RichTextStyle::Size {
                    milli_points: Milli(value.milli),
                }
            }
            CheckedDirectStyleSpan::Size { .. } => {
                return Err(RuntimeSemanticProjectionError::Dialogue {
                    owner: Some(owner),
                    reason: "RichText size execution currently requires a point length".to_owned(),
                });
            }
            CheckedDirectStyleSpan::Ruby { annotation } => RichTextStyle::Ruby {
                annotation: annotation.clone(),
            },
        }),
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
                CheckedDialogueControl::RevealRate { milli_cps } => {
                    let style = RichTextStyle::Speed {
                        milli_cps: Milli(milli_cps.0),
                    };
                    let span = style.span_kind();
                    nodes.push(RichTextNode::StyleStart {
                        style: Box::new(style),
                    });
                    return Ok(Some(span));
                }
            };
            nodes.push(RichTextNode::Control { control });
            None
        }
        CheckedRichTextAction::Host { action, .. } => {
            lower_dialogue_host_action(owner, action, nodes, values, effects)?;
            None
        }
        CheckedRichTextAction::Marker(marker) => {
            nodes.push(RichTextNode::Control {
                control: RichTextControl::Mark {
                    name: marker.as_str().to_owned(),
                },
            });
            None
        }
        CheckedRichTextAction::Style { .. }
        | CheckedRichTextAction::Layout { .. }
        | CheckedRichTextAction::Transform { .. }
        | CheckedRichTextAction::Object { .. }
        | CheckedRichTextAction::BuiltinFx { .. } => {
            return Err(RuntimeSemanticProjectionError::Dialogue {
                owner: Some(owner),
                reason: "checked RichText action has no final runtime projection".to_owned(),
            });
        }
    };
    if let Some(style) = style {
        let span = style.span_kind();
        nodes.push(RichTextNode::StyleStart {
            style: Box::new(style),
        });
        Ok(Some(span))
    } else {
        Ok(None)
    }
}

fn lower_dialogue_host_action(
    owner: ExprId,
    event: &CheckedDialogueHostEvent,
    nodes: &mut Vec<RichTextNode>,
    values: &mut Vec<RuntimeDialogueValueExpression>,
    effects: &mut Vec<RuntimeDialogueEffectExpression>,
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
        CheckedDialogueHostEvent::TimedCue { at, call } => {
            effects.push(RuntimeDialogueEffectExpression {
                trigger: RuntimeDialogueEffectTrigger::DelayMillis(at.millis),
                expression: *call,
            });
            return Ok(());
        }
        CheckedDialogueHostEvent::Call { call } => {
            let mark = format!("__arcweft_inline_call_{}", effects.len());
            nodes.push(RichTextNode::Control {
                control: RichTextControl::Mark { name: mark.clone() },
            });
            effects.push(RuntimeDialogueEffectExpression {
                trigger: RuntimeDialogueEffectTrigger::Mark(mark),
                expression: *call,
            });
            return Ok(());
        }
        CheckedDialogueHostEvent::Signal { signal } => DialogueHostEvent::Signal {
            signal: signal.as_str().to_owned(),
        },
        CheckedDialogueHostEvent::ConditionalStart { condition } => {
            let slot = next_dialogue_slot(owner, values.len())?;
            values.push(RuntimeDialogueValueExpression {
                slot,
                role: RuntimeDialogueValueRole::Condition,
                expression: *condition,
            });
            nodes.push(RichTextNode::ConditionalStart { condition: slot });
            return Ok(());
        }
        CheckedDialogueHostEvent::ConditionalElse => {
            nodes.push(RichTextNode::ConditionalElse);
            return Ok(());
        }
        CheckedDialogueHostEvent::ConditionalEnd => {
            nodes.push(RichTextNode::ConditionalEnd);
            return Ok(());
        }
    };
    nodes.push(RichTextNode::HostEvent {
        event: static_event,
    });
    Ok(())
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
    let identity = RuntimeSemanticTypeId::from_bytes(*ty.semantic_identity_digest().as_bytes());
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
        TypeKind::AgentValue => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::AgentValue),
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
        TypeKind::AgentResourceBody => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::ResourceBody),
        TypeKind::RagContextPack => RuntimeTypeShape::Agent(RuntimeAgentTypeShape::RagContextPack),
        TypeKind::AgentBuiltin(builtin) => {
            RuntimeTypeShape::Agent(runtime_agent_builtin_type(*builtin))
        }
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
        TypeKind::Result { ok, error } => RuntimeTypeShape::Result {
            value: nested_at(ok, RuntimeTypeProjectionStep::ResultOk)?,
            error: nested_at(error, RuntimeTypeProjectionStep::ResultError)?,
        },
        TypeKind::Option(item) => {
            RuntimeTypeShape::Option(nested_at(item, RuntimeTypeProjectionStep::OptionItem)?)
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
            let semantic_type = ty.semantic_identity_digest();
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
                .environment_record_for_semantic_type(ty.semantic_identity_digest())
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

const fn runtime_agent_builtin_type(builtin: AgentBuiltinType) -> RuntimeAgentTypeShape {
    match builtin {
        AgentBuiltinType::ObservedObjectId => RuntimeAgentTypeShape::ObservedObjectId,
        AgentBuiltinType::CaptureFormat => RuntimeAgentTypeShape::CaptureFormat,
        AgentBuiltinType::CaptureKind => RuntimeAgentTypeShape::CaptureKind,
        AgentBuiltinType::Diagnostics => RuntimeAgentTypeShape::Diagnostics,
        AgentBuiltinType::WaitError => RuntimeAgentTypeShape::WaitError,
        AgentBuiltinType::ViewportPoint => RuntimeAgentTypeShape::ViewportPoint,
        AgentBuiltinType::PointerButton => RuntimeAgentTypeShape::PointerButton,
        AgentBuiltinType::RagError => RuntimeAgentTypeShape::RagError,
    }
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

fn runtime_nominal_record(
    nominal: &CheckedProjectNominal,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeResolvedNominalRecord, RuntimeSemanticProjectionError> {
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
    let Some(nominal) = record.owner().project_nominal() else {
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
    };
    let fields = record
        .fields()
        .iter()
        .map(|field| {
            let runtime_field = field.runtime_field().ok_or(
                RuntimeSemanticProjectionError::UnrepresentableEnvironmentRecordField {
                    owner: RuntimeRecordExecutableOwner::Pattern(owner),
                    semantic_owner: record.owner().semantic_type(),
                    ordinal: field.declaration_ordinal(),
                },
            )?;
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
    RuntimeRecordPatternFact::try_new(
        runtime_nominal_record(nominal, symbols, world, analysis)?,
        fields,
        rest,
    )
    .map_err(|source| RuntimeSemanticProjectionError::RecordPlan {
        owner: RuntimeRecordExecutableOwner::Pattern(owner),
        source,
    })
}

fn runtime_variant(
    variant: &CheckedVariantResolution,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<
    arcweft_runtime_plan::semantic_facts::RuntimeResolvedVariant,
    RuntimeSemanticProjectionError,
> {
    let projected = match variant.owner() {
        CheckedVariantOwner::Project {
            nominal,
            semantic_type,
            layout,
            cases,
        } => {
            if nominal.identity() != *semantic_type {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: "checked project variant semantic identity is inconsistent".to_owned(),
                });
            }
            let projection = analysis
                .runtime_nominal_projection(*semantic_type)
                .filter(|projection| {
                    projection.kind()
                        == arcweft_lang_sema::final_analysis::RuntimeProjectNominalKind::Variant
                })
                .ok_or_else(|| RuntimeSemanticProjectionError::NominalSchemaProjection {
                    nominal: nominal.declaration().qualified_name(),
                    source: NominalSchemaProjectionError::MissingCachedProjection {
                        semantic_type: *semantic_type,
                    },
                })?;
            if projection.variant_cases().len() != cases.len() {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: "checked project variant case inventory is incomplete".to_owned(),
                });
            }
            let runtime_nominal = runtime_nominal(nominal, analysis)?;
            if runtime_nominal.layout() != *layout {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason:
                        "checked project variant layout differs from its runtime nominal layout"
                            .to_owned(),
                });
            }
            RuntimeResolvedVariant::project(
                runtime_nominal,
                nominal
                    .arguments()
                    .iter()
                    .map(|argument| runtime_type(argument, symbols, world, analysis))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
                variant.ordinal(),
                checked_variant_selected_name(variant)?,
                runtime_checked_variant_cases(cases, symbols, world, analysis)?,
            )
            .map_err(|error| runtime_variant_projection_error(&error))?
        }
        CheckedVariantOwner::CharacterNominal {
            nominal,
            semantic_type,
            cases,
        } => {
            if TypeKind::CharacterNominal(nominal.clone()).semantic_identity_digest()
                != *semantic_type
            {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason: "checked character variant semantic identity is inconsistent"
                        .to_owned(),
                });
            }
            RuntimeResolvedVariant::character(
                RuntimeSemanticTypeId::from_bytes(*semantic_type.as_bytes()),
                RuntimeNominalTypeId::from_checked_digest(*semantic_type.as_bytes()),
                runtime_checked_variant_cases(cases, symbols, world, analysis)?,
                variant.ordinal(),
                checked_variant_selected_name(variant)?,
            )
            .map_err(|error| runtime_variant_projection_error(&error))?
        }
        CheckedVariantOwner::BuiltinClosed {
            nominal,
            semantic_type,
            cases,
        } => RuntimeResolvedVariant::builtin_closed(
            RuntimeSemanticTypeId::from_bytes(*semantic_type.as_bytes()),
            RuntimeNominalTypeId::try_new(nominal.as_str().to_owned()).map_err(|error| {
                RuntimeSemanticProjectionError::Type {
                    reason: format!("checked base-environment enum identity is invalid: {error}"),
                }
            })?,
            runtime_checked_variant_cases(cases, symbols, world, analysis)?,
            variant.ordinal(),
            checked_variant_selected_name(variant)?,
        )
        .map_err(|error| runtime_variant_projection_error(&error))?,
        CheckedVariantOwner::Option { item, cases } => {
            let item = runtime_type(item, symbols, world, analysis)?;
            let normalized_cases = runtime_checked_variant_cases(cases, symbols, world, analysis)?;
            validate_runtime_closed_variant_cases(
                &normalized_cases,
                &[("Some", Some(&item)), ("None", None)],
            )?;
            RuntimeResolvedVariant::option(
                item,
                variant.ordinal(),
                checked_variant_selected_name(variant)?,
            )
            .map_err(|error| runtime_variant_projection_error(&error))?
        }
        CheckedVariantOwner::Result { ok, error, cases } => {
            let ok = runtime_type(ok, symbols, world, analysis)?;
            let error = runtime_type(error, symbols, world, analysis)?;
            let normalized_cases = runtime_checked_variant_cases(cases, symbols, world, analysis)?;
            validate_runtime_closed_variant_cases(
                &normalized_cases,
                &[("Ok", Some(&ok)), ("Err", Some(&error))],
            )?;
            RuntimeResolvedVariant::result(
                ok,
                error,
                variant.ordinal(),
                checked_variant_selected_name(variant)?,
            )
            .map_err(|error| runtime_variant_projection_error(&error))?
        }
    };
    Ok(projected)
}

fn checked_variant_selected_name(
    variant: &CheckedVariantResolution,
) -> Result<&str, RuntimeSemanticProjectionError> {
    variant
        .selected()
        .diagnostic_name()
        .ok_or_else(|| RuntimeSemanticProjectionError::Type {
            reason: "checked variant case has no diagnostic name authority".to_owned(),
        })
}

fn runtime_checked_variant_cases(
    cases: &[arcweft_lang_sema::final_analysis::CheckedVariantCase],
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<Box<[RuntimeNormalizedVariantCase]>, RuntimeSemanticProjectionError> {
    cases
        .iter()
        .map(|case| {
            let name =
                case.diagnostic_name()
                    .ok_or_else(|| RuntimeSemanticProjectionError::Type {
                        reason: "checked variant case has no diagnostic name authority".to_owned(),
                    })?;
            let payload = case
                .payload()
                .map(|payload| {
                    runtime_type(payload, symbols, world, analysis)
                        .and_then(retain_checked_variant_payload)
                })
                .transpose()?;
            Ok(RuntimeNormalizedVariantCase::new(name.to_owned(), payload))
        })
        .collect()
}

fn validate_runtime_closed_variant_cases(
    cases: &[RuntimeNormalizedVariantCase],
    expected: &[(&str, Option<&RuntimeNormalizedType>)],
) -> Result<(), RuntimeSemanticProjectionError> {
    if cases.len() != expected.len() {
        return Err(RuntimeSemanticProjectionError::Type {
            reason: format!(
                "checked closed variant case count {} differs from runtime shape {}",
                cases.len(),
                expected.len()
            ),
        });
    }
    for (ordinal, (case, (expected_name, expected_payload))) in
        cases.iter().zip(expected.iter()).enumerate()
    {
        if case.name() != *expected_name || case.payload() != *expected_payload {
            return Err(RuntimeSemanticProjectionError::Type {
                reason: format!(
                    "checked closed variant case {ordinal} differs from its runtime shape"
                ),
            });
        }
    }
    Ok(())
}

fn runtime_evaluated_effect(
    owner: StmtId,
    effect: &CheckedEvaluatedEffect,
    project: HirExecutableProjectView<'_>,
) -> Result<RuntimeEvaluatedEffectFact, RuntimeSemanticProjectionError> {
    let module = project
        .modules()
        .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module.as_ref()))
        .ok_or(RuntimeSemanticProjectionError::InvalidEvaluatedEffectDisposition { owner })?;
    let statement = module
        .resolve_stmt(owner)
        .map_err(|_| RuntimeSemanticProjectionError::InvalidEvaluatedEffectDisposition { owner })?;
    let HirStmtKind::Expression { expression: _ } = statement.kind() else {
        return Err(RuntimeSemanticProjectionError::InvalidEvaluatedEffectDisposition { owner });
    };
    Ok(RuntimeEvaluatedEffectFact::new(match effect {
        CheckedEvaluatedEffect::Log {
            level,
            message,
            fields,
        } => RuntimeEvaluatedEffect::Log {
            level: runtime_log_level(*level),
            message: *message,
            fields: runtime_effect_fields(fields),
        },
        CheckedEvaluatedEffect::SignalWrite { target, value } => {
            RuntimeEvaluatedEffect::SignalWrite {
                target: *target,
                value: *value,
            }
        }
        CheckedEvaluatedEffect::MetricWrite { target, value } => {
            RuntimeEvaluatedEffect::MetricWrite {
                target: *target,
                value: *value,
            }
        }
        CheckedEvaluatedEffect::EmitEvent { event, fields } => RuntimeEvaluatedEffect::EmitEvent {
            event: *event,
            fields: runtime_effect_fields(fields),
        },
        CheckedEvaluatedEffect::Panic { message } => {
            RuntimeEvaluatedEffect::Panic { message: *message }
        }
        CheckedEvaluatedEffect::Fail { message } => {
            RuntimeEvaluatedEffect::Fail { message: *message }
        }
        CheckedEvaluatedEffect::Bail { message } => {
            RuntimeEvaluatedEffect::Bail { message: *message }
        }
        CheckedEvaluatedEffect::Ensure { condition, message } => RuntimeEvaluatedEffect::Ensure {
            condition: *condition,
            message: *message,
        },
    }))
}

fn runtime_effect_fields(
    fields: &[arcweft_lang_sema::final_analysis::CheckedEffectField],
) -> Box<[RuntimeEffectFieldFact]> {
    fields
        .iter()
        .map(|field| RuntimeEffectFieldFact::new(field.name(), field.value()))
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

const fn runtime_log_level(level: CallableLogLevel) -> RuntimeLogLevel {
    match level {
        CallableLogLevel::Trace => RuntimeLogLevel::Trace,
        CallableLogLevel::Debug => RuntimeLogLevel::Debug,
        CallableLogLevel::Info => RuntimeLogLevel::Info,
        CallableLogLevel::Warn => RuntimeLogLevel::Warn,
        CallableLogLevel::Error => RuntimeLogLevel::Error,
    }
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
                    runtime_type(item, symbols, world, analysis)?,
                    runtime_type(&iterator, symbols, world, analysis)?,
                    runtime_type(&next_value, symbols, world, analysis)?,
                    runtime_type(&step, symbols, world, analysis)?,
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
                    runtime_type(item, symbols, world, analysis)?,
                    runtime_type(into_iter, symbols, world, analysis)?,
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
                runtime_type(item, symbols, world, analysis)?,
                runtime_type(source, symbols, world, analysis)?,
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
        let CheckedStatementRole::Iteration(iteration) = statement.role() else {
            continue;
        };
        match iteration.as_ref() {
            CheckedIteration::Builtin { .. } => {}
            CheckedIteration::Witness {
                source, into_iter, ..
            } => {
                let [Some(into_iterator), Some(iterator)] = iteration.trait_dispatches() else {
                    unreachable!("checked witness owns both exact conformances")
                };
                insert(into_iterator, source)?;
                insert(iterator, into_iter)?;
            }
            CheckedIteration::IteratorWitness { source, .. } => {
                let [Some(iterator), None] = iteration.trait_dispatches() else {
                    unreachable!("checked iterator witness owns one exact conformance")
                };
                insert(iterator, source)?;
            }
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

fn runtime_call(
    owner: ExprId,
    facts: &CallTargetFacts,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
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
    let dispatch = match application.core().callee() {
        CheckedCallCalleeExecution::Direct => RuntimeResolvedCallDispatch::Static(
            runtime_call_target(owner, application, symbols, world, analysis)?,
        ),
        CheckedCallCalleeExecution::Value { source } => {
            let arcweft_lang_sema::callable::CheckedCallArgumentSlotSource::Expression(callee) =
                source.raw()
            else {
                return Err(RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "checked value dispatch has a non-expression callee source".to_owned(),
                });
            };
            RuntimeResolvedCallDispatch::Value { callee }
        }
    };
    let selected = application.core().candidates().selected();
    let mut positioned = Vec::new();
    for operand in application
        .core()
        .execution()
        .ordered_runtime_operands(CheckedCallRuntimeOperandOrder::Abi)
    {
        match operand {
            CheckedCallRuntimeOperand::Receiver {
                ty,
                source,
                abi_position,
                ..
            } => positioned.push(RuntimePositionedCallOperand::new(
                abi_position,
                RuntimeResolvedCallOperand::new(
                    RuntimeResolvedCallOperandOrigin::Receiver,
                    runtime_call_operand_source(source.raw()),
                    runtime_type(ty, symbols, world, analysis)?,
                    RuntimeResolvedCallOperandBinding::Positional,
                    RuntimeResolvedCallOperandProjection::Scalar,
                ),
            )),
            CheckedCallRuntimeOperand::Argument {
                argument,
                passing,
                slot,
            } => positioned.push(RuntimePositionedCallOperand::new(
                slot.abi_position(),
                RuntimeResolvedCallOperand::new(
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
                    runtime_type(slot.inferred(), symbols, world, analysis)?,
                    runtime_call_operand_binding(owner, selected, passing, slot)?,
                    runtime_call_operand_projection(owner, slot, symbols, world, analysis)?,
                ),
            )),
        }
    }
    let result = match application.result() {
        arcweft_lang_sema::callable::CheckedCallResult::Value(_) => RuntimeCallResultShape::Value,
        arcweft_lang_sema::callable::CheckedCallResult::Continuation(_) => {
            RuntimeCallResultShape::PartialFunction
        }
    };
    RuntimeResolvedCall::try_new(dispatch, positioned, result).map_err(|error| {
        RuntimeSemanticProjectionError::Call {
            owner,
            reason: error.to_string(),
        }
    })
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
                    let ArrayLength::Const(len) = len else {
                        return Err(RuntimeSemanticProjectionError::Call {
                            owner,
                            reason: "array spread operand does not have a constant length"
                                .to_owned(),
                        });
                    };
                    RuntimeResolvedSpreadContainer::Array { len: *len }
                }
                CheckedConstraintContainerConstructor::MapValue { kind, key } => {
                    RuntimeResolvedSpreadContainer::MapValue {
                        kind: match kind {
                            MapKind::Ordered => RuntimeMapKind::Ordered,
                            MapKind::Sorted => RuntimeMapKind::Sorted,
                            MapKind::BTree => RuntimeMapKind::BTree,
                        },
                        key: runtime_type(key, symbols, world, analysis)?,
                    }
                }
            })
        }
    };
    Ok(projection)
}

#[expect(
    clippy::too_many_lines,
    reason = "checked callable families are exhaustively selected at this runtime boundary"
)]
fn runtime_call_target(
    owner: ExprId,
    application: &CheckedCallApplication,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeResolvedStaticCallTarget, RuntimeSemanticProjectionError> {
    if let Some(variant) =
        runtime_variant_constructor(owner, application, symbols, world, analysis)?
    {
        return Ok(RuntimeResolvedStaticCallTarget::Variant(variant));
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
        let symbol =
            symbols
                .callable(declaration)
                .ok_or_else(|| RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: "selected project callable is absent from the accepted symbol table"
                        .to_owned(),
                })?;
        let runtime = RuntimeProjectCallable::new(
            declaration.clone(),
            symbol.source_item(),
            runtime_selected_callable_id(owner, selected)?,
        );
        if declaration.owner()
            == arcweft_lang_hir::symbol::CallableDeclarationOwner::ExternCapability
        {
            let checked = analysis
                .checked_callables()
                .project_callable(declaration)
                .map_err(|error| RuntimeSemanticProjectionError::Call {
                    owner,
                    reason: format!(
                        "extern capability call has no accepted checked callable facts: {error:?}"
                    ),
                })?;
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
    reason = "constructor instantiations form a closed semantic validation matrix"
)]
fn runtime_variant_constructor(
    owner: ExprId,
    application: &CheckedCallApplication,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<Option<RuntimeResolvedVariant>, RuntimeSemanticProjectionError> {
    let invalid = |reason: &str| RuntimeSemanticProjectionError::Call {
        owner,
        reason: reason.to_owned(),
    };
    let selected = application.core().candidates().selected();
    match selected.instantiation() {
        ResolvedCallableBaseInstantiation::Result { kind } => {
            let TypeKind::Result { ok, error } = application.result().ty() else {
                return Err(invalid(
                    "Result constructor did not retain its exact checked Result type",
                ));
            };
            let ok = runtime_type(ok, symbols, world, analysis)?;
            let error = runtime_type(error, symbols, world, analysis)?;
            let (ordinal, name) = match kind {
                arcweft_lang_sema::callable::ResultConstructorKind::Ok => (0, "Ok"),
                arcweft_lang_sema::callable::ResultConstructorKind::Err => (1, "Err"),
            };
            RuntimeResolvedVariant::result(ok, error, ordinal, name)
                .map(Some)
                .map_err(|error| invalid(&error.to_string()))
        }
        ResolvedCallableBaseInstantiation::Option => {
            let TypeKind::Option(item) = application.result().ty() else {
                return Err(invalid(
                    "Option constructor did not retain its exact checked Option type",
                ));
            };
            if !matches!(
                selected.id(),
                arcweft_lang_sema::callable::CallableCandidateId::Option(
                    arcweft_lang_sema::callable::OptionConstructorKind::Some
                )
            ) {
                return Err(invalid(
                    "Option constructor instantiation has a non-Option candidate identity",
                ));
            }
            RuntimeResolvedVariant::option(runtime_type(item, symbols, world, analysis)?, 0, "Some")
                .map(Some)
                .map_err(|error| invalid(&error.to_string()))
        }
        ResolvedCallableBaseInstantiation::ExpectedEnum { expected } => {
            let TypeKind::ProjectNominal(nominal) = expected else {
                return Err(invalid(
                    "enum constructor expected type is not a project nominal enum",
                ));
            };
            if application.result().ty() != expected {
                return Err(invalid(
                    "enum constructor result differs from its checked project nominal type",
                ));
            }
            let arcweft_lang_sema::callable::CallableCandidateId::EnumVariant(candidate) =
                selected.id()
            else {
                return Err(invalid(
                    "enum constructor instantiation has a non-enum candidate identity",
                ));
            };
            let semantic_type = expected.semantic_identity_digest();
            let projection = analysis
                .runtime_nominal_projection(semantic_type)
                .filter(|projection| {
                    projection.kind()
                        == arcweft_lang_sema::final_analysis::RuntimeProjectNominalKind::Variant
                        && projection.declaration() == nominal.declaration()
                })
                .ok_or_else(|| RuntimeSemanticProjectionError::NominalSchemaProjection {
                    nominal: nominal.declaration().qualified_name(),
                    source: NominalSchemaProjectionError::MissingCachedProjection { semantic_type },
                })?;
            if candidate.owner().world() != nominal.declaration().world()
                || candidate.owner().module() != nominal.declaration().module()
                || candidate.owner().name().as_str() != nominal.declaration().name().as_str()
            {
                return Err(invalid(
                    "enum constructor candidate does not own the checked project nominal type",
                ));
            }
            let case = projection
                .variant_cases()
                .iter()
                .find(|case| case.diagnostic_name().as_str() == candidate.variant().as_str())
                .ok_or_else(|| {
                    invalid("enum constructor variant is absent from its sealed projection")
                })?;
            let runtime_nominal = RuntimeResolvedNominal::new(
                nominal.declaration().clone(),
                projection.owner(),
                projection.nominal().clone(),
                projection.semantic_identity(),
                projection.layout(),
            );
            Ok(Some(
                RuntimeResolvedVariant::project(
                    runtime_nominal,
                    nominal
                        .arguments()
                        .iter()
                        .map(|argument| runtime_type(argument, symbols, world, analysis))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                    case.ordinal(),
                    case.diagnostic_name().as_str(),
                    runtime_project_variant_cases(projection, symbols, world, analysis)?,
                )
                .map_err(|error| invalid(&error.to_string()))?,
            ))
        }
        ResolvedCallableBaseInstantiation::None
        | ResolvedCallableBaseInstantiation::Character { .. }
        | ResolvedCallableBaseInstantiation::Receiver { .. }
        | ResolvedCallableBaseInstantiation::TypeReceiver { .. }
        | ResolvedCallableBaseInstantiation::Extension { .. } => Ok(None),
    }
}

fn runtime_project_variant_cases(
    projection: &arcweft_lang_sema::final_analysis::RuntimeProjectNominalProjection,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<Box<[RuntimeNormalizedVariantCase]>, RuntimeSemanticProjectionError> {
    if projection.kind() != arcweft_lang_sema::final_analysis::RuntimeProjectNominalKind::Variant {
        return Err(RuntimeSemanticProjectionError::Type {
            reason: "checked project variant owner is not an enum".to_owned(),
        });
    }
    projection
        .variant_cases()
        .iter()
        .map(|case| {
            let payload = case
                .payload()
                .map(|ty| {
                    let payload = runtime_type(ty, symbols, world, analysis)?;
                    retain_checked_variant_payload(payload)
                })
                .transpose()?;
            Ok(RuntimeNormalizedVariantCase::new(
                case.diagnostic_name().as_str(),
                payload,
            ))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn retain_checked_variant_payload(
    payload: RuntimeNormalizedType,
) -> Result<RuntimeNormalizedType, RuntimeSemanticProjectionError> {
    payload
        .checked_type()
        .map_err(|reason| RuntimeSemanticProjectionError::Type {
            reason: reason.to_string(),
        })?;
    Ok(payload)
}

fn runtime_variant_projection_error(
    error: &arcweft_runtime_plan::semantic_facts::RuntimeResolvedVariantError,
) -> RuntimeSemanticProjectionError {
    RuntimeSemanticProjectionError::Type {
        reason: error.to_string(),
    }
}

fn runtime_selected_callable_id(
    owner: ExprId,
    selected: &arcweft_lang_sema::callable::ResolvedCallable,
) -> Result<arcweft_core::entry::RuntimeCallableId, RuntimeSemanticProjectionError> {
    let checked = selected
        .checked()
        .ok_or_else(|| RuntimeSemanticProjectionError::Call {
            owner,
            reason: "selected record-backed callable has no checked semantic identity".to_owned(),
        })?;
    Ok(arcweft_core::entry::RuntimeCallableId::from_checked_digest(
        checked.semantic_digest().into_bytes(),
    ))
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
