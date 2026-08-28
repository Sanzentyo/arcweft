//! Ordinary-call probing, accounting, and final semantic selection.

use std::{collections::BTreeSet, rc::Rc, sync::Arc};

#[path = "calls/constraints.rs"]
mod constraints;
#[path = "calls/semantics.rs"]
mod semantics;

pub(in crate::final_analysis::analyzer) use constraints::AnalyzerCallConstraintDomain;
pub(crate) use constraints::CallAnalysisFailure;
pub(crate) use constraints::{
    AnalyzerCallConstraintSource, AnalyzerDetachedCandidateRecord,
    AnalyzerDetachedConsideredCandidate, AnalyzerDetachedUnselectedCall,
    AnalyzerDetachedUnselectedOutcome, AnalyzerPreparedCallGraph, AnalyzerPreparedCallPrefix,
    AnalyzerPreparedCalleeExpression, AnalyzerPreparedCandidateInventory,
    AnalyzerPreparedCandidateMetadata, AnalyzerPreparedCandidateRecord,
    AnalyzerPreparedDialoguePatchAdmission, AnalyzerPreparedUnselectedCall,
    AnalyzerPreparedUnselectedOutcome, CallAnalysisInvariant, PreparedCallApplicationTransaction,
    RanCandidateTransaction, SealedAcceptedCandidate, run_prepared_candidate,
    validate_and_prepare_call_constraints,
};

pub(super) use semantics::{
    checked_project_nominal, final_call_effects, final_callable_effects, nominal_substitutions,
    source_callable_schema_type,
};
use semantics::{
    provisional_call_effects, provisional_callable_effects, select_prepared_candidates,
};

use super::expression_types::value_resolution_type;
use super::preparation::AssociatedReceiverTypeResolution;
use super::statements::{expression_span, scope_is_within, source_span};
use super::{
    AcceptedCandidateRank, Analyzer, AnalyzerExpressionContext, BTreeMap,
    CallCalleeClassificationFact, CallResolverAuthority, CallResolverRequest,
    CallableDeclarationKey, CallableDeclarationOwner, CallableGroupIndex, CallableInstantiation,
    CandidateSelection, CharacterDialogueCharacterType, CharacterDialogueFieldCoordinate,
    CharacterDialoguePatchContext, CharacterOwnerSource, CheckedCallArgumentSlotSource,
    CheckedCallableDeclaration, CheckedCharacterDialogueFactory, CheckedCharacterDialoguePatch,
    CheckedCharacterDialoguePatchField, CheckedCharacterDialogueReconfigure,
    CheckedCharacterDialogueTarget, CheckedExpression, CheckedExpressionResolution,
    CheckedPatchOperation, CheckedTypeSelection, CheckedValueResolution, EffectRow, EffectSet,
    ExprId, FinalCallCalleeFacts, FinalSemanticAnalysisError, HirAssociatedSeparator,
    HirCallArgument, HirCallArgumentSourcePart, HirCallCallee, HirCallExpr, HirExprKind,
    HirExprSourceRole, HirModule, HirSelectedMember, HirSourcePresence, HirSourceQuery,
    HirSourceSite, PreparedResolvedCallable, RegisteredSemanticValueId, ResolveCallOutcome,
    ResolvedCallTarget, ResolvedCharacterOwner, ResolverWork, ScopeId, TypeKind,
    map_call_arguments, prepare_final_call_callee, prepare_language_free_dot_path,
    resolve_call_target,
};
use crate::callable::{
    CallableCandidateId, DialogueCallableId, EnclosingGenericParameterScope,
    PreparedCallGraphIngress, PreparedFunctionValueOriginEvidence,
    PreparedFunctionValueOriginProducer, PreparedFunctionValueOriginProgress,
    PreparedFunctionValueOriginQueryError, prepare_function_value_origin_query,
    prepare_presentation_callee_id,
};
use crate::final_analysis::type_rules::compact_numeric_element_type as infer_compact_numeric_element_type;
use crate::final_analysis::{CandidateEvaluationPass, CandidateFactTransactionViolation};

#[derive(Clone, Copy)]
struct CallSource<'a> {
    module: &'a HirModule,
    owner: ExprId,
    call: &'a HirCallExpr,
    expected: Option<&'a TypeKind>,
    dialogue_application_metadata:
        Option<&'a crate::callable::PreparedDialogueApplicationMetadataInventory>,
    attempt: &'a PhysicalCallAttemptId,
}

use super::expression_error::{
    ActiveCallFrame, AnalyzerExpressionError, AnalyzerExpressionInvariant, CallFrameEnterFailure,
    PhysicalCallAttemptId,
};
use super::state::{
    CandidateFactOperationFailure, CandidateFactTransactionAction,
    CandidateFactTransactionAuthority, CandidateFactTransactionOutcome, PhysicalCallAttemptClose,
};

struct ResolvedCallQuery {
    callee: CallCalleeClassificationFact,
    considered: Vec<Arc<PreparedResolvedCallable>>,
    callee_inputs: crate::callable::PreparedCallCalleeConstraintInputs,
    function_value_origin: Option<PreparedFunctionValueOriginEvidence>,
    current_group: CallableGroupIndex,
    work: ResolverWork,
    argument_count: u64,
    dialogue_context: CharacterDialoguePatchContext,
    dialogue_patch_admissions: Box<[AnalyzerPreparedDialoguePatchAdmission]>,
}

struct AssociatedReceiverRecovery {
    receiver: arcweft_lang_hir::identity::TypeId,
    separator: HirAssociatedSeparator,
    result: TypeKind,
}

struct StagedCallCalleeChildren {
    recovery: Option<AssociatedReceiverRecovery>,
    function_value_origin: Option<PreparedFunctionValueOriginEvidence>,
}

fn terminal_call_constraint_failure(
    owner: ExprId,
    failure: CallAnalysisFailure,
) -> AnalyzerExpressionError {
    AnalyzerExpressionError::Call { owner, failure }
}

fn terminal_lower_constraint_failure(
    owner: ExprId,
    failure: crate::types::constraints::TypeConstraintFailure<
        constraints::AnalyzerCallConstraintDomain,
    >,
) -> AnalyzerExpressionError {
    let failure = match failure {
        crate::types::constraints::TypeConstraintFailure::Rejected(error) => {
            let rejection = match error {
                crate::types::constraints::TypeConstraintCandidateFailure::Constraint(error) => {
                    error
                }
                crate::types::constraints::TypeConstraintCandidateFailure::Source(_)
                | crate::types::constraints::TypeConstraintCandidateFailure::SourceProjection(_) => {
                    crate::types::constraints::TypeConstraintRejection::Mismatch
                }
            };
            CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                crate::callable::CallConstraintInvariant::UnexpectedLowerRejection(rejection),
            ))
        }
        crate::types::constraints::TypeConstraintFailure::FatalSource(error) => {
            CallAnalysisFailure::FatalSource(*error)
        }
        crate::types::constraints::TypeConstraintFailure::Abort(error) => {
            CallAnalysisFailure::Abort(error)
        }
        crate::types::constraints::TypeConstraintFailure::Invariant(
            crate::types::constraints::TypeConstraintFailureInvariant::Constraint(error),
        ) => CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
            crate::callable::CallConstraintInvariant::Lower(error),
        )),
        crate::types::constraints::TypeConstraintFailure::Invariant(
            crate::types::constraints::TypeConstraintFailureInvariant::Client(error),
        ) => CallAnalysisFailure::Invariant(CallAnalysisInvariant::Client(error)),
    };
    AnalyzerExpressionError::Call { owner, failure }
}

fn close_call_frame<T>(
    owner: ExprId,
    frame: ActiveCallFrame,
    result: Result<T, AnalyzerExpressionError>,
) -> Result<T, AnalyzerExpressionError> {
    match frame.close() {
        Ok(()) => result,
        Err(violation) => Err(AnalyzerExpressionError::Invariant(
            AnalyzerExpressionInvariant::CallFrame {
                owner,
                violation: Box::new(violation),
            },
        )),
    }
}

struct PreparedCandidateBatch {
    probes: Vec<PreparedCandidateOutcome>,
}

struct PreparedRecoveryCandidateBatch {
    candidates: Vec<Arc<PreparedResolvedCallable>>,
    primary_result: TypeKind,
    primary_argument_count: Option<usize>,
    primary_projection: PreparedCandidateSemanticProjection,
    discarded_projections: Vec<PreparedCandidateSemanticProjection>,
}

/// Complete move-only semantic state produced by one candidate attempt.
///
/// The outer candidate transaction owns staging performed around the lower
/// driver, while the sealed branch owns facts produced by source callbacks.
/// Selection and deterministic recovery must consume both layers in this
/// order; exposing either layer independently would permit a recovery call to
/// publish a graph whose contextual expression facts were silently dropped.
struct PreparedCandidateSemanticProjection {
    outer: super::state::CandidateSemanticProjection,
    branch: constraints::AnalyzerCallSealedBranch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreparedCallMappingRejection {
    authored_arguments: usize,
}

impl PreparedCallMappingRejection {
    fn from_authored(arguments: &[HirCallArgument]) -> Self {
        Self {
            authored_arguments: arguments.len(),
        }
    }
}

enum PreparedCandidateRejection {
    Mapping(PreparedCallMappingRejection),
    Constraint,
}

enum PreparedCandidateRunOutcome {
    Accepted {
        transaction: RanCandidateTransaction,
        rank: AcceptedCandidateRank,
    },
    Rejected {
        candidate: Arc<PreparedResolvedCallable>,
        result: TypeKind,
        evidence: PreparedCandidateRejection,
        branch: constraints::AnalyzerCallSealedBranch,
    },
}

enum PreparedCandidateOutcome {
    Accepted {
        transaction: SealedAcceptedCandidate,
        rank: AcceptedCandidateRank,
    },
    Rejected {
        candidate: Arc<PreparedResolvedCallable>,
        result: TypeKind,
        evidence: PreparedCandidateRejection,
        projection: super::state::CandidateSemanticProjection,
        branch: constraints::AnalyzerCallSealedBranch,
    },
}

impl PreparedCandidateOutcome {
    fn into_accepted(
        self,
        owner: ExprId,
    ) -> Result<(SealedAcceptedCandidate, AcceptedCandidateRank), AnalyzerExpressionError> {
        match self {
            Self::Accepted { transaction, rank } => Ok((transaction, rank)),
            Self::Rejected { .. } => Err(AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                    crate::callable::CallConstraintInvariant::UnexpectedRejectedSelection,
                )),
            }),
        }
    }

    fn recovery_argument_count(&self) -> Option<usize> {
        match self {
            Self::Rejected { evidence, .. } => match evidence {
                PreparedCandidateRejection::Mapping(rejection) => {
                    Some(rejection.authored_arguments)
                }
                PreparedCandidateRejection::Constraint => None,
            },
            Self::Accepted { .. } => None,
        }
    }

    fn into_contextual_parts(
        self,
    ) -> (
        Arc<PreparedResolvedCallable>,
        TypeKind,
        PreparedCandidateSemanticProjection,
    ) {
        match self {
            Self::Accepted { transaction, .. } => {
                let (ran, outer) = transaction.into_parts();
                let (candidate, _current_group, result, branch) = ran.into_contextual_parts();
                (
                    candidate,
                    result,
                    PreparedCandidateSemanticProjection { outer, branch },
                )
            }
            Self::Rejected {
                candidate,
                result,
                projection,
                branch,
                ..
            } => (
                candidate,
                result,
                PreparedCandidateSemanticProjection {
                    outer: projection,
                    branch,
                },
            ),
        }
    }
}

impl PreparedCandidateBatch {
    fn into_recovery(
        self,
        owner: ExprId,
        primary: usize,
        retained: impl IntoIterator<Item = usize>,
    ) -> Result<PreparedRecoveryCandidateBatch, AnalyzerExpressionError> {
        let retained = retained.into_iter().collect::<Vec<_>>();
        let retained_set = retained.iter().copied().collect::<BTreeSet<_>>();
        if retained_set.len() != retained.len()
            || !retained_set.contains(&primary)
            || retained_set.iter().any(|index| *index >= self.probes.len())
        {
            return Err(AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                    crate::callable::CallConstraintInvariant::UnexpectedRejectedSelection,
                )),
            });
        }
        let mut by_index = Vec::with_capacity(self.probes.len());
        let mut primary_result = None;
        let mut primary_argument_count = None;
        let mut primary_projection = None;
        let mut discarded_projections = Vec::new();
        for (index, outcome) in self.probes.into_iter().enumerate() {
            let argument_count = outcome.recovery_argument_count();
            let (candidate, result, projection) = outcome.into_contextual_parts();
            by_index.push(candidate);
            if index == primary {
                primary_result = Some(result);
                primary_argument_count = argument_count;
                primary_projection = Some(projection);
            } else {
                discarded_projections.push(projection);
            }
        }
        let mut candidates = Vec::with_capacity(retained.len());
        candidates.push(Arc::clone(by_index.get(primary).ok_or_else(|| {
            AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                    crate::callable::CallConstraintInvariant::UnexpectedRejectedSelection,
                )),
            }
        })?));
        for index in retained {
            if index != primary {
                candidates.push(Arc::clone(&by_index[index]));
            }
        }
        Ok(PreparedRecoveryCandidateBatch {
            candidates,
            primary_result: primary_result.ok_or_else(|| AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                    crate::callable::CallConstraintInvariant::UnexpectedRejectedSelection,
                )),
            })?,
            primary_argument_count,
            primary_projection: primary_projection.ok_or_else(|| {
                AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::UnexpectedRejectedSelection,
                    )),
                }
            })?,
            discarded_projections,
        })
    }
}

struct PreparedCandidateRequest<'a, 'ctx> {
    module: &'a HirModule,
    owner: ExprId,
    inputs: PreparedCandidateInputs<'a>,
    candidate: Arc<PreparedResolvedCallable>,
    current_group: CallableGroupIndex,
    expected_result: Option<&'a TypeKind>,
    callee_inputs: crate::callable::PreparedCallCalleeConstraintInputs,
    pass: CandidateEvaluationPass,
    attempt: Option<&'a PhysicalCallAttemptId>,
    context: &'ctx AnalyzerExpressionContext<'ctx>,
    dialogue_patch_admissions: &'a [AnalyzerPreparedDialoguePatchAdmission],
}

enum PreparedCandidateInputs<'a> {
    Authored {
        arguments: &'a [HirCallArgument],
        dialogue_application_metadata:
            Option<&'a crate::callable::PreparedDialogueApplicationMetadataInventory>,
    },
    SemanticOnly(crate::callable::PreparedDialogueCallConstraintInputs),
}

#[derive(Clone, Copy)]
struct CharacterDialoguePatchFieldRequest<'a> {
    source: CallSource<'a>,
    context: CharacterDialoguePatchContext,
    index: usize,
    argument: &'a HirCallArgument,
}

enum CharacterDialogueResolutionFailure {
    Semantic(FinalSemanticAnalysisError),
    Constraint(crate::callable::CallConstraintInvariant),
}

impl From<FinalSemanticAnalysisError> for CharacterDialogueResolutionFailure {
    fn from(error: FinalSemanticAnalysisError) -> Self {
        Self::Semantic(error)
    }
}

impl From<crate::callable::CallConstraintInvariant> for CharacterDialogueResolutionFailure {
    fn from(error: crate::callable::CallConstraintInvariant) -> Self {
        Self::Constraint(error)
    }
}

struct RecoveryCall<'a> {
    source: CallSource<'a>,
    callee: CallCalleeClassificationFact,
    callee_inputs: crate::callable::PreparedCallCalleeConstraintInputs,
    candidates: Vec<Arc<PreparedResolvedCallable>>,
    considered: Vec<Arc<PreparedResolvedCallable>>,
    argument_count: usize,
    result: TypeKind,
    work: ResolverWork,
    ambiguous: bool,
}

pub(super) fn checked_character_dialogue_target(
    expression: ExprId,
    checked: &super::PreparedExpressionFact,
) -> Result<Option<CheckedCharacterDialogueTarget>, crate::callable::CallConstraintInvariant> {
    if let Some(CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item))) =
        checked.checked_resolution()
        && item.family() == arcweft_id::DeclarationIdentityFamily::Character
    {
        let character = item
            .character()
            .map(CharacterDialogueCharacterType::Exact)
            .ok_or(crate::callable::CallConstraintInvariant::MissingCheckedCharacterIdentity)?;
        return Ok(Some(CheckedCharacterDialogueTarget::Character {
            expression,
            item: Some(Box::new(item.clone())),
            character,
        }));
    }
    Ok(match checked.ty() {
        TypeKind::Ref(entity) if entity.kind() == &crate::types::EntityKind::Character => {
            Some(CheckedCharacterDialogueTarget::Character {
                expression,
                item: match checked.checked_resolution() {
                    Some(CheckedExpressionResolution::Value(
                        CheckedValueResolution::ProjectItem(item),
                    )) => Some(Box::new(item.clone())),
                    _ => None,
                },
                character: CharacterDialogueCharacterType::Any,
            })
        }
        TypeKind::CharacterDialogue(ty) => Some(CheckedCharacterDialogueTarget::Dialogue {
            expression,
            ty: ty.clone(),
        }),
        _ => None,
    })
}

fn call_argument_span(
    module: &HirModule,
    owner: ExprId,
    index: usize,
) -> Result<arcweft_source::SourceSpan, FinalSemanticAnalysisError> {
    call_argument_part_span(module, owner, index, HirCallArgumentSourcePart::Whole)
}

fn call_argument_part_span(
    module: &HirModule,
    owner: ExprId,
    index: usize,
    part: HirCallArgumentSourcePart,
) -> Result<arcweft_source::SourceSpan, FinalSemanticAnalysisError> {
    let argument = arcweft_lang_hir::expr::HirCallArgumentOrdinal::try_from_usize(index)
        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
    source_span(
        module,
        HirSourceQuery::Expr {
            owner,
            role: HirExprSourceRole::CallArgument { argument, part },
        },
    )
}

impl Analyzer<'_, '_, '_> {
    /// Returns the exact ordinary Function declaration that lexically owns an
    /// expression. The checked-callable staging transaction already retains
    /// each accepted body scope and checked identity, so call facts do not
    /// reconstruct ownership from source text or maintain a parallel index.
    pub(super) fn enclosing_ordinary_callable(
        &self,
        module: &HirModule,
        expression: ExprId,
    ) -> Result<Option<CallableDeclarationKey>, FinalSemanticAnalysisError> {
        let scope = module
            .resolve_expr(expression)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .scope();
        let staged = self
            .staged_callables
            .as_ref()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let mut enclosing = None;
        for body in &staged.bodies {
            if body.module != module.module_id()
                || body.owner != CallableDeclarationOwner::Function
                || !scope_is_within(module, scope, body.scope)?
            {
                continue;
            }
            let CheckedCallableDeclaration::Project(declaration) = body.id.declaration() else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            if declaration.owner() != CallableDeclarationOwner::Function
                || enclosing.replace(declaration.clone()).is_some()
            {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            }
        }
        Ok(enclosing)
    }

    pub(super) fn check_call_expression_in_context(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallExpr,
        expected: Option<&TypeKind>,
        dialogue_application_metadata: Option<
            &crate::callable::PreparedDialogueApplicationMetadataInventory,
        >,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        let frame = match context.enter_call(owner) {
            Ok(frame) => frame,
            Err(CallFrameEnterFailure::Abort(error)) => {
                return Err(AnalyzerExpressionError::Abort(error));
            }
            Err(CallFrameEnterFailure::Invariant(violation)) => {
                return Err(AnalyzerExpressionError::Invariant(
                    AnalyzerExpressionInvariant::CallFrame {
                        owner,
                        violation: Box::new(violation),
                    },
                ));
            }
        };
        let attempt = frame.physical_attempt(context);
        if let Err(violation) = self.facts.begin_physical_call_attempt(attempt.clone()) {
            return close_call_frame(owner, frame, Err(AnalyzerExpressionError::fact(violation)));
        }
        let result = self.check_call_expression_inner(
            context,
            module,
            owner,
            call,
            expected,
            dialogue_application_metadata,
            &attempt,
        );
        let close = match &result {
            Ok(_) => PhysicalCallAttemptClose::Completed,
            Err(error) if error.is_cancellation() => PhysicalCallAttemptClose::Cancelled,
            Err(_) => PhysicalCallAttemptClose::Failed,
        };
        let result = match self.facts.close_physical_call_attempt(&attempt, close) {
            Ok(()) => result,
            Err(violation) => Err(AnalyzerExpressionError::fact(violation)),
        };
        close_call_frame(owner, frame, result)
    }

    fn check_call_expression_inner(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallExpr,
        expected: Option<&TypeKind>,
        dialogue_application_metadata: Option<
            &crate::callable::PreparedDialogueApplicationMetadataInventory,
        >,
        attempt: &PhysicalCallAttemptId,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        let source = CallSource {
            module,
            owner,
            call,
            expected,
            dialogue_application_metadata,
            attempt,
        };
        let argument_count = u64::try_from(source.call.arguments().len()).map_err(|_| {
            AnalyzerExpressionError::Abort(
                crate::types::constraints::TypeConstraintAbort::ArithmeticOverflow,
            )
        })?;
        let mut work = ResolverWork::new(self.catalogs.callable_limits.max_query_work());
        if work.record_logical_argument_checks(argument_count).is_err() {
            return Err(AnalyzerExpressionError::Abort(
                crate::types::constraints::TypeConstraintAbort::WorkLimit {
                    requested: argument_count,
                    consumed: 0,
                    limit: self.catalogs.callable_limits.max_query_work(),
                },
            ));
        }
        let staged_callee =
            match self.stage_call_callee_children(context, source.module, source.call) {
                Ok(recovery) => recovery,
                Err(error) => return Err(error),
            };
        if let Some(recovery) = staged_callee.recovery {
            return self.publish_associated_receiver_recovery(source, recovery, work);
        }
        let dialogue_context = if dialogue_application_metadata.is_some() {
            CharacterDialoguePatchContext::ImmediateContentApplication
        } else {
            CharacterDialoguePatchContext::ReusableValue
        };
        let mut resolution = match self.resolve_call_query(
            context,
            source,
            work,
            argument_count,
            dialogue_context,
            staged_callee.function_value_origin,
        ) {
            Ok(resolution) => resolution,
            Err(error) => return Err(error),
        };
        resolution.dialogue_patch_admissions =
            self.prepare_character_dialogue_source_admission(source, &resolution)?;
        let probes = match self.prepare_resolved_candidates(source, &mut resolution) {
            Ok(probes) => probes,
            Err(error) => {
                return Err(error);
            }
        };
        match select_prepared_candidates(&probes.probes) {
            CandidateSelection::Selected(selected) => {
                self.publish_selected_call(source, resolution, probes, selected)
            }
            CandidateSelection::Ambiguous { primary, tied } => {
                self.publish_ambiguous_call(source, resolution, probes, primary, tied)
            }
            CandidateSelection::Rejected { primary } => {
                self.publish_rejected_call(source, resolution, probes, primary)
            }
        }
    }

    fn checked_character_dialogue_resolution(
        &mut self,
        source: CallSource<'_>,
        selected: &PreparedResolvedCallable,
        context: CharacterDialoguePatchContext,
        transaction: &PreparedCallApplicationTransaction,
    ) -> Result<Option<CheckedExpressionResolution>, CharacterDialogueResolutionFailure> {
        let crate::callable::CallableValidator::Dialogue(id) = selected.schema().validator() else {
            return Ok(None);
        };
        if !matches!(
            id,
            crate::callable::DialogueCallableId::CharacterFactory
                | crate::callable::DialogueCallableId::CharacterReconfigure
        ) {
            return Ok(None);
        }
        let HirCallCallee::Value { value } = source.call.callee() else {
            return Ok(None);
        };
        let Some(callee) = self.facts.expressions().get(value).cloned() else {
            return Ok(None);
        };
        let Some(target) = checked_character_dialogue_target(*value, &callee)? else {
            return Ok(None);
        };
        let patch = self.checked_character_dialogue_patch(source, &target, context, transaction)?;
        let result = target.result_type();
        let resolution = match (&target, id) {
            (
                CheckedCharacterDialogueTarget::Character { .. },
                crate::callable::DialogueCallableId::CharacterFactory,
            ) => CheckedExpressionResolution::CharacterDialogueFactory(
                CheckedCharacterDialogueFactory::new(target, patch),
            ),
            (
                CheckedCharacterDialogueTarget::Dialogue { .. },
                crate::callable::DialogueCallableId::CharacterReconfigure,
            ) => CheckedExpressionResolution::CharacterDialogueReconfigure(
                CheckedCharacterDialogueReconfigure::new(target, patch),
            ),
            _ => {
                return Err(FinalSemanticAnalysisError::InvalidCharacterDialoguePatch {
                    owner: source.owner,
                }
                .into());
            }
        };
        if selected.schema().result() != &TypeKind::CharacterDialogue(result) {
            return Err(FinalSemanticAnalysisError::InvalidCharacterDialoguePatch {
                owner: source.owner,
            }
            .into());
        }
        Ok(Some(resolution))
    }

    fn checked_character_dialogue_patch(
        &self,
        source: CallSource<'_>,
        _target: &CheckedCharacterDialogueTarget,
        context: CharacterDialoguePatchContext,
        transaction: &PreparedCallApplicationTransaction,
    ) -> Result<CheckedCharacterDialoguePatch, CharacterDialogueResolutionFailure> {
        let mut fields = Vec::with_capacity(source.call.arguments().len());
        let mut coordinates = BTreeMap::new();
        for (index, argument) in source.call.arguments().iter().enumerate() {
            let request = CharacterDialoguePatchFieldRequest {
                source,
                context,
                index,
                argument,
            };
            let Some((coordinate, field_span)) =
                self.character_dialogue_field_coordinate(request)?
            else {
                continue;
            };
            if let Some(first_span) = coordinates.insert(coordinate.clone(), field_span.clone()) {
                return Err(
                    FinalSemanticAnalysisError::DuplicateCharacterDialogueField {
                        coordinate,
                        first_span,
                        duplicate_span: field_span,
                    }
                    .into(),
                );
            }
            fields.push(self.checked_character_dialogue_patch_field(
                request,
                coordinate,
                transaction,
            )?);
        }
        Ok(CheckedCharacterDialoguePatch::new(
            context,
            fields,
            expression_span(source.module, source.owner)?,
        ))
    }

    /// Seals the structural and registry-owned CharacterDialogue source
    /// policy before overload probing. Expression checking remains inside the
    /// ordinary source callback transaction; this preflight issues only the
    /// exact custom-field coordinate, declared type, and terminal diagnostics.
    fn prepare_character_dialogue_source_admission(
        &self,
        source: CallSource<'_>,
        resolution: &ResolvedCallQuery,
    ) -> Result<Box<[AnalyzerPreparedDialoguePatchAdmission]>, AnalyzerExpressionError> {
        let mut dialogue_candidates = resolution.considered.iter().filter(|candidate| {
            matches!(
                candidate.schema().validator(),
                crate::callable::CallableValidator::Dialogue(
                    crate::callable::DialogueCallableId::CharacterFactory
                        | crate::callable::DialogueCallableId::CharacterReconfigure
                )
            )
        });
        let Some(candidate) = dialogue_candidates.next() else {
            return Ok(Box::new([]));
        };
        if dialogue_candidates.next().is_some() {
            return Err(AnalyzerExpressionError::Call {
                owner: source.owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                    crate::callable::CallConstraintInvariant::MalformedSchemaInventory,
                )),
            });
        }

        let mut custom = Vec::new();
        let mut coordinates = BTreeMap::new();
        for (index, argument) in source.call.arguments().iter().enumerate() {
            let request = CharacterDialoguePatchFieldRequest {
                source,
                context: resolution.dialogue_context,
                index,
                argument,
            };
            let Some((coordinate, field_span)) = self
                .character_dialogue_field_coordinate(request)
                .map_err(AnalyzerExpressionError::fatal)?
            else {
                continue;
            };
            if let Some(first_span) = coordinates.insert(coordinate.clone(), field_span.clone()) {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::DuplicateCharacterDialogueField {
                        coordinate,
                        first_span,
                        duplicate_span: field_span,
                    },
                ));
            }
            if let CharacterDialogueFieldCoordinate::Custom(field) = coordinate {
                custom.push((index, argument, field, field_span));
            }
        }
        if custom.is_empty() {
            return Ok(Box::new([]));
        }
        let mapping = map_call_arguments(
            source.module,
            candidate.schema(),
            candidate.id(),
            candidate.call_group(),
            source.call.arguments(),
            None,
        )
        .ok_or_else(|| {
            AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::InvalidCharacterDialoguePatch {
                    owner: source.owner,
                },
            )
        })?;
        let mut prepared = Vec::with_capacity(custom.len());
        for (index, argument, field, field_span) in custom {
            let descriptor = self
                .catalogs
                .world
                .environment()
                .character_dialogue_fields()
                .descriptor(&field)
                .cloned()
                .ok_or_else(|| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                })?;
            let mapped = mapping.arguments().get(index).ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })?;
            let [slot] = mapped.slots() else {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::InvalidCharacterDialoguePatch {
                        owner: source.owner,
                    },
                ));
            };
            let parameter_coordinate = slot.coordinate().ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })?;
            let parameter = candidate
                .schema()
                .group(parameter_coordinate.group())
                .and_then(|group| group.parameter(parameter_coordinate.parameter()))
                .ok_or_else(|| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                })?;
            if !matches!(
                parameter.consumer(),
                crate::callable::CallableParameterConsumer::DialoguePatch(
                    CharacterDialogueFieldCoordinate::Custom(actual)
                ) if actual == &field
            ) || slot.source() != CheckedCallArgumentSlotSource::Expression(argument.value())
            {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::WrongPayloadFamily,
                ));
            }

            let argument_ordinal = arcweft_lang_hir::expr::HirCallArgumentOrdinal::try_from_usize(
                index,
            )
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::AccountingOverflow)
            })?;
            let value_span = call_argument_part_span(
                source.module,
                source.owner,
                index,
                HirCallArgumentSourcePart::Value,
            )
            .map_err(AnalyzerExpressionError::fatal)?;
            prepared.push(AnalyzerPreparedDialoguePatchAdmission::new(
                argument_ordinal,
                argument.value(),
                parameter_coordinate,
                field,
                descriptor.value_type().clone(),
                descriptor.clearable(),
                u32::try_from(
                    parameter
                        .value_rule()
                        .ok_or_else(|| {
                            AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::WrongPayloadFamily,
                            )
                        })?
                        .guarded()
                        .len(),
                )
                .map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::AccountingOverflow)
                })?,
                field_span,
                value_span,
                descriptor.declaration().clone(),
            ));
        }
        Ok(prepared.into_boxed_slice())
    }

    fn character_dialogue_field_coordinate(
        &self,
        request: CharacterDialoguePatchFieldRequest<'_>,
    ) -> Result<
        Option<(CharacterDialogueFieldCoordinate, arcweft_source::SourceSpan)>,
        FinalSemanticAnalysisError,
    > {
        let CharacterDialoguePatchFieldRequest {
            source,
            context,
            index,
            argument,
            ..
        } = request;
        let field_span = match argument {
            HirCallArgument::Named { .. } => call_argument_part_span(
                source.module,
                source.owner,
                index,
                HirCallArgumentSourcePart::Name,
            )?,
            HirCallArgument::Positional { .. } | HirCallArgument::Spread { .. } => {
                call_argument_span(source.module, source.owner, index)?
            }
        };
        let coordinate = match argument {
            HirCallArgument::Positional { .. } if index == 0 => {
                CharacterDialogueFieldCoordinate::Look
            }
            HirCallArgument::Positional { .. } | HirCallArgument::Spread { .. } => {
                return Err(FinalSemanticAnalysisError::InvalidCharacterDialoguePatch {
                    owner: source.owner,
                });
            }
            HirCallArgument::Named { .. } => {
                let name = argument.resolved_name().ok_or(
                    FinalSemanticAnalysisError::InvalidCharacterDialoguePatch {
                        owner: source.owner,
                    },
                )?;
                match name.as_str() {
                    "id" | "text_key"
                        if context
                            == CharacterDialoguePatchContext::ImmediateContentApplication =>
                    {
                        return Ok(None);
                    }
                    "id" | "text_key" => {
                        return Err(
                            FinalSemanticAnalysisError::CharacterDialogueApplicationOnlyField {
                                field: name.as_str().to_owned(),
                                field_span,
                            },
                        );
                    }
                    "character" | "character_id" | "content" => {
                        return Err(FinalSemanticAnalysisError::InvalidCharacterDialoguePatch {
                            owner: source.owner,
                        });
                    }
                    "voice" => CharacterDialogueFieldCoordinate::Voice,
                    "look" => CharacterDialogueFieldCoordinate::Look,
                    "stage" => CharacterDialogueFieldCoordinate::Stage,
                    "portrait" => CharacterDialogueFieldCoordinate::Portrait,
                    "focus" => CharacterDialogueFieldCoordinate::Focus,
                    "cleanup" => CharacterDialogueFieldCoordinate::Cleanup,
                    "view" => CharacterDialogueFieldCoordinate::View,
                    "source_locale" => CharacterDialogueFieldCoordinate::SourceLocale,
                    "hooks" => CharacterDialogueFieldCoordinate::Hooks,
                    "style" => CharacterDialogueFieldCoordinate::Style,
                    "rich_text" => CharacterDialogueFieldCoordinate::RichText,
                    "inline_error" | "inline_error_policy" | "inline_fallback" => {
                        CharacterDialogueFieldCoordinate::InlineFailure
                    }
                    name => {
                        let descriptor = self
                            .catalogs
                            .world
                            .environment()
                            .character_dialogue_fields()
                            .resolve(source.module.key().path(), name)
                            .ok_or_else(|| {
                                FinalSemanticAnalysisError::UnknownCharacterDialogueField {
                                    name: name.to_owned(),
                                    field_span: field_span.clone(),
                                    scope: source.module.key().path().clone(),
                                }
                            })?;
                        CharacterDialogueFieldCoordinate::Custom(descriptor.id().clone())
                    }
                }
            }
        };
        Ok(Some((coordinate, field_span)))
    }

    fn checked_character_dialogue_patch_field(
        &self,
        request: CharacterDialoguePatchFieldRequest<'_>,
        coordinate: CharacterDialogueFieldCoordinate,
        transaction: &PreparedCallApplicationTransaction,
    ) -> Result<CheckedCharacterDialoguePatchField, CharacterDialogueResolutionFailure> {
        let CharacterDialoguePatchFieldRequest {
            source,
            index,
            argument,
            ..
        } = request;
        let argument_ordinal =
            arcweft_lang_hir::expr::HirCallArgumentOrdinal::try_from_usize(index)
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let semantic = transaction.argument_semantics(argument_ordinal, argument.value())?;
        let operation = match semantic.action() {
            crate::callable::CallableArgumentSemanticAction::Clear => CheckedPatchOperation::Clear,
            crate::callable::CallableArgumentSemanticAction::Supply => CheckedPatchOperation::Set {
                value: argument.value(),
                ty: semantic.inferred().clone(),
            },
        };
        Ok(CheckedCharacterDialoguePatchField::new(
            coordinate,
            operation,
            call_argument_span(source.module, source.owner, index)?,
        ))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "call-query resolution keeps preparation, charged resolver execution, and checked fact publication atomic"
    )]
    fn resolve_call_query(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        source: CallSource<'_>,
        mut work: ResolverWork,
        argument_count: u64,
        dialogue_context: CharacterDialoguePatchContext,
        function_value_origin: Option<PreparedFunctionValueOriginEvidence>,
    ) -> Result<ResolvedCallQuery, AnalyzerExpressionError> {
        let authority = CallResolverAuthority::accepted(
            self.project,
            source.module,
            self.symbols,
            self.catalogs.world,
        );
        let presentation_id = prepare_presentation_callee_id(
            source.module,
            source.call,
            &self.catalogs.callable_limits,
        )
        .map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            })
        })?;
        let presentation_character_owner =
            self.presentation_character_owner(context, source.call.arguments(), presentation_id)?;
        let prepared = prepare_final_call_callee(
            authority,
            source.owner,
            FinalCallCalleeFacts::new(
                self.facts.expressions(),
                PreparedCallGraphIngress::new(
                    self.facts
                        .prepared_calls()
                        .map_err(AnalyzerExpressionError::fact)?,
                ),
                &self.type_reports,
                function_value_origin,
            ),
            dialogue_context,
            &self.catalogs.callable_limits,
        )
        .map_err(|error| match error {
            crate::callable::PrepareFinalCallCalleeError::PreparedContinuationInvariant(
                invariant,
            ) => AnalyzerExpressionError::Call {
                owner: source.owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                    invariant,
                )),
            },
            crate::callable::PrepareFinalCallCalleeError::MissingFunctionValueOrigin { .. } => {
                AnalyzerExpressionError::Call {
                    owner: source.owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::MissingFunctionValueOrigin,
                    )),
                }
            }
            crate::callable::PrepareFinalCallCalleeError::UnexpectedFunctionValueOrigin {
                ..
            } => AnalyzerExpressionError::Call {
                owner: source.owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                    crate::callable::CallConstraintInvariant::UnexpectedFunctionValueOrigin,
                )),
            },
            crate::callable::PrepareFinalCallCalleeError::InvalidFunctionValueOrigin { .. } => {
                AnalyzerExpressionError::Call {
                    owner: source.owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::InvalidFunctionValueOrigin,
                    )),
                }
            }
            _ => AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            }),
        })?;
        let mut callee_inputs = prepared.constraint_inputs();
        let implicit_extension_receiver = self
            .pipe_stack
            .last()
            .filter(|pipe| pipe.right == source.owner && pipe.placeholders.is_empty())
            .map(|pipe| {
                crate::callable::PreparedImplicitExtensionReceiver::new(
                    pipe.left,
                    pipe.value.clone(),
                )
            });
        let staged = self.staged_callables.as_ref().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CheckedCallableCatalog)
        })?;
        let request = CallResolverRequest::try_new(
            prepared.as_borrowed(),
            &super::CallResolverContext {
                authority,
                checked: (&staged.builder).into(),
                presentation_character_owner: presentation_character_owner.as_ref(),
                expression: source.owner,
                cancellation: self.control.cancellation(),
                prepared_continuations: self
                    .facts
                    .prepared_calls()
                    .map_err(AnalyzerExpressionError::fact)?,
                limits: &self.catalogs.callable_limits,
                implicit_extension_receiver: implicit_extension_receiver.clone(),
            },
            &mut work,
        )
        .map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            })
        })?;
        let callee = request.classification();
        let outcome = resolve_call_target(request);
        let function_value_origin = prepared.into_function_value_origin();
        let (considered, current_group) = match outcome {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) => {
                let current_group = candidates.first().call_group();
                let candidates = candidates.into_shared().map_err(|_| {
                    AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::CallResolutionFailed {
                            owner: source.owner,
                        },
                    )
                })?;
                (candidates, current_group)
            }
            ResolveCallOutcome::Missing(target) => {
                let name = target
                    .path()
                    .map(crate::callable::CallablePath::dotted_name)
                    .or_else(|| target.method().map(|method| method.as_str().to_owned()))
                    .unwrap_or_else(|| "<recovered>".to_owned());
                let lookup = source
                    .module
                    .source_site(
                        source.module.provenance().source_identity(),
                        HirSourceQuery::Expr {
                            owner: source.owner,
                            role: HirExprSourceRole::CallCallee,
                        },
                    )
                    .map_err(|_| {
                        AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                    })?;
                let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence()
                else {
                    return Err(AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::RecoveredOwner,
                    ));
                };
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::UnknownCallTarget {
                        owner: source.owner,
                        kind: target.kind(),
                        name,
                        call_source: span.clone(),
                    },
                ));
            }
            ResolveCallOutcome::Resolved(ResolvedCallTarget::NonCallable(target)) => {
                self.publish_non_callable_call(source, callee, target, work)?;
                return Err(AnalyzerExpressionError::rejected(source.owner));
            }
            ResolveCallOutcome::Rejected(_) => {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::CallResolutionFailed {
                        owner: source.owner,
                    },
                ));
            }
            ResolveCallOutcome::Invariant(error) => {
                return Err(AnalyzerExpressionError::Call {
                    owner: source.owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        error,
                    )),
                });
            }
        };
        if let Some(receiver) = implicit_extension_receiver.as_ref() {
            if considered.iter().all(|candidate| {
                matches!(
                    candidate.instantiation(),
                    crate::callable::CallableInstantiation::Extension { .. }
                )
            }) {
                callee_inputs =
                    crate::callable::PreparedCallCalleeConstraintInputs::ValueReceiver {
                        source: receiver.source(),
                        actual: receiver.actual().clone(),
                    };
            } else if considered.iter().any(|candidate| {
                matches!(
                    candidate.instantiation(),
                    crate::callable::CallableInstantiation::Extension { .. }
                )
            }) {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::CallResolutionFailed {
                        owner: source.owner,
                    },
                ));
            }
        }
        Ok(ResolvedCallQuery {
            callee,
            considered,
            callee_inputs,
            function_value_origin,
            current_group,
            work,
            argument_count,
            dialogue_context,
            dialogue_patch_admissions: Box::new([]),
        })
    }

    fn publish_associated_receiver_recovery(
        &mut self,
        source: CallSource<'_>,
        recovery: AssociatedReceiverRecovery,
        mut work: ResolverWork,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        self.run_candidate_fact_transaction::<_, AnalyzerExpressionError>(
            |this, _expression_authority, transaction_authority| {
                let argument_count = source.call.arguments().len();
                work.record_retained_argument_fact_publications(
                    u64::try_from(argument_count).map_err(|_| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::AccountingOverflow,
                        )
                    })?,
                )
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: source.owner,
                })
                .map_err(AnalyzerExpressionError::fatal)?;
                let callee = CallCalleeClassificationFact::AssociatedType {
                    receiver: recovery.receiver,
                    separator: recovery.separator,
                };
                let callee_expression = this
                    .stage_associated_receiver_recovery_expression(
                        source.module,
                        source.call,
                        &recovery.result,
                    )
                    .map_err(AnalyzerExpressionError::fatal)?;
                let selected_expression_inventory =
                    arcweft_lang_hir::project::HirSelectedCallExpressionInventory::new(
                        source
                            .call
                            .arguments()
                            .iter()
                            .map(HirCallArgument::value)
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        callee_expression,
                    );
                let enclosing_callable = this
                    .enclosing_ordinary_callable(source.module, source.owner)
                    .map_err(AnalyzerExpressionError::fatal)?;
                this.facts
                    .insert_unselected_call(
                        &transaction_authority,
                        crate::callable::CheckedCallSite::HirCall(source.owner),
                        AnalyzerPreparedUnselectedCall {
                            enclosing_callable,
                            outcome: AnalyzerPreparedUnselectedOutcome::Missing {
                                callee: Some(callee),
                                kind: crate::callable::UnknownCallKind::AssociatedType,
                            },
                            diagnostics: Vec::new(),
                            accounting: work.call_accounting(),
                            selected_expression_inventory,
                        },
                    )
                    .map_err(AnalyzerExpressionError::fact)?;
                let selection = if source
                    .expected
                    .is_some_and(|expected| expected.accepts(&recovery.result))
                {
                    CheckedTypeSelection::Expected
                } else {
                    CheckedTypeSelection::Inferred
                };
                Ok(CandidateFactTransactionAction::Commit(
                    CheckedExpression::new(
                        recovery.result.clone(),
                        selection,
                        EffectSet::new(),
                        CheckedExpressionResolution::Call,
                    ),
                ))
            },
        )?
        .into_committed()
        .map_err(AnalyzerExpressionError::fact)
    }

    fn publish_non_callable_call(
        &mut self,
        source: CallSource<'_>,
        callee: CallCalleeClassificationFact,
        target: crate::callable::ResolvedNonCallableTarget,
        work: ResolverWork,
    ) -> Result<(), AnalyzerExpressionError> {
        let callee_expression = match callee {
            CallCalleeClassificationFact::Value { expression } => Some(expression),
            CallCalleeClassificationFact::AssociatedType { .. } => None,
        };
        let non_callable_source = target.source().clone();
        let non_callable_type = target.ty().clone();
        let outcome = self.run_candidate_fact_transaction::<_, AnalyzerExpressionError>(
            |this, _expression_authority, transaction_authority| {
                let enclosing_callable = this
                    .enclosing_ordinary_callable(source.module, source.owner)
                    .map_err(AnalyzerExpressionError::fatal)?;
                this.facts
                    .insert_unselected_call(
                        &transaction_authority,
                        crate::callable::CheckedCallSite::HirCall(source.owner),
                        AnalyzerPreparedUnselectedCall {
                            enclosing_callable,
                            outcome: AnalyzerPreparedUnselectedOutcome::NonCallable {
                                callee: Some(callee),
                                source: non_callable_source,
                                ty: non_callable_type,
                            },
                            diagnostics: Vec::new(),
                            accounting: work.call_accounting(),
                            selected_expression_inventory:
                                arcweft_lang_hir::project::HirSelectedCallExpressionInventory::new(
                                    Box::new([]),
                                    callee_expression,
                                ),
                        },
                    )
                    .map_err(AnalyzerExpressionError::fact)?;
                Ok(CandidateFactTransactionAction::Commit(()))
            },
        )?;
        outcome
            .into_committed()
            .map_err(AnalyzerExpressionError::fact)
    }

    fn stage_associated_receiver_recovery_expression(
        &mut self,
        module: &HirModule,
        call: &HirCallExpr,
        receiver_type: &TypeKind,
    ) -> Result<Option<ExprId>, FinalSemanticAnalysisError> {
        let HirCallCallee::UnresolvedDot { value_receiver, .. } = call.callee() else {
            return Ok(None);
        };
        module
            .resolve_expr(*value_receiver)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if !self.facts.expressions().contains_key(value_receiver) {
            self.facts
                .publish_new_expression(
                    *value_receiver,
                    CheckedExpression::new(
                        receiver_type.clone(),
                        CheckedTypeSelection::Inferred,
                        EffectSet::new(),
                        CheckedExpressionResolution::Structural,
                    ),
                )
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        }
        Ok(Some(*value_receiver))
    }

    fn prepare_resolved_candidates(
        &mut self,
        source: CallSource<'_>,
        resolution: &mut ResolvedCallQuery,
    ) -> Result<PreparedCandidateBatch, AnalyzerExpressionError> {
        let mut probes = Vec::with_capacity(resolution.considered.len());
        for candidate in &resolution.considered {
            self.control
                .check()
                .map_err(AnalyzerExpressionError::fatal)?;
            resolution
                .work
                .record_candidate_argument_probes(resolution.argument_count)
                .map_err(|_| {
                    AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::CallResolutionFailed {
                            owner: source.owner,
                        },
                    )
                })?;
            resolution
                .work
                .charge_argument_mapping(resolution.argument_count)
                .map_err(|_| {
                    AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::CallResolutionFailed {
                            owner: source.owner,
                        },
                    )
                })?;
            let candidate_group = if resolution.callee_inputs.is_function_value() {
                resolution.current_group
            } else {
                candidate.call_group()
            };
            let probe = self.run_candidate_fact_transaction::<_, CandidateFactOperationFailure>(
                |this, authority, _transaction_authority| {
                    let context = AnalyzerExpressionContext::candidate(
                        authority,
                        Rc::clone(&this.call_frames),
                    );
                    let probe = this.prepare_candidate(
                        PreparedCandidateRequest {
                            module: source.module,
                            owner: source.owner,
                            inputs: PreparedCandidateInputs::Authored {
                                arguments: source.call.arguments(),
                                dialogue_application_metadata: source.dialogue_application_metadata,
                            },
                            candidate: Arc::clone(candidate),
                            current_group: candidate_group,
                            expected_result: source.expected,
                            callee_inputs: resolution.callee_inputs.clone(),
                            pass: CandidateEvaluationPass::Probe,
                            attempt: Some(source.attempt),
                            context: &context,
                            dialogue_patch_admissions: &resolution.dialogue_patch_admissions,
                        },
                        &mut resolution.work,
                    );
                    drop(context);
                    match probe? {
                        PreparedCandidateRunOutcome::Accepted { transaction, rank } => {
                            Ok(CandidateFactTransactionAction::Extract(
                                PreparedCandidateRunOutcome::Accepted { transaction, rank },
                            ))
                        }
                        rejected @ PreparedCandidateRunOutcome::Rejected { .. } => {
                            Ok(CandidateFactTransactionAction::Extract(rejected))
                        }
                    }
                },
            )?;
            let probe = match probe {
                CandidateFactTransactionOutcome::Extracted {
                    value: PreparedCandidateRunOutcome::Accepted { transaction, rank },
                    projection,
                } => PreparedCandidateOutcome::Accepted {
                    transaction: SealedAcceptedCandidate::seal(transaction, *projection),
                    rank,
                },
                CandidateFactTransactionOutcome::Extracted {
                    value:
                        PreparedCandidateRunOutcome::Rejected {
                            candidate,
                            result,
                            evidence,
                            branch,
                        },
                    projection,
                } => PreparedCandidateOutcome::Rejected {
                    candidate,
                    result,
                    evidence,
                    projection: *projection,
                    branch,
                },
                CandidateFactTransactionOutcome::Committed(_)
                | CandidateFactTransactionOutcome::RolledBack { .. } => {
                    return Err(AnalyzerExpressionError::fact(
                        crate::final_analysis::CandidateFactTransactionViolation::UnrecoverableLedger,
                    ));
                }
            };
            probes.push(probe);
        }
        Ok(PreparedCandidateBatch { probes })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "selected-call publication validates the complete semantic candidate and argument-accounting record"
    )]
    fn publish_selected_call(
        &mut self,
        source: CallSource<'_>,
        resolution: ResolvedCallQuery,
        batch: PreparedCandidateBatch,
        selected_index: usize,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        let singleton = resolution.considered.len() == 1;
        let outcome = self.run_candidate_fact_transaction::<_, CandidateFactOperationFailure>(
            |this, _expression_authority, transaction_authority| {
                this.publish_selected_call_in_transaction(
                    source,
                    resolution,
                    batch,
                    selected_index,
                    singleton,
                    &transaction_authority,
                )
                .map(CandidateFactTransactionAction::Commit)
            },
        )?;
        outcome
            .into_committed()
            .map_err(AnalyzerExpressionError::fact)
    }

    /// Publishes the language-owned zero-argument Dialogue application through
    /// the same probe/extract/materialize/graph transaction as an ordinary
    /// selected call.  The only difference is the typed call-site family;
    /// there is no provisional public call fact or empty-solution shortcut.
    pub(super) fn publish_resolved_dialogue_application(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        expected: Option<&TypeKind>,
        target_expression: ExprId,
        target_actual: TypeKind,
        has_line_plan: bool,
        expression_resolution: CheckedExpressionResolution,
        considered: Vec<Arc<PreparedResolvedCallable>>,
        mut work: ResolverWork,
    ) -> Result<TypeKind, AnalyzerExpressionError> {
        let candidate = considered.first().cloned().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                owner,
            })
        })?;
        if considered.len() != 1
            || candidate.id()
                != &CallableCandidateId::Dialogue(DialogueCallableId::ContentApplication)
        {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::CallResolutionFailed { owner },
            ));
        }
        let structural_inputs = crate::callable::PreparedDialogueCallConstraintInputs::seal(
            &candidate,
            target_expression,
            target_actual,
            has_line_plan,
        )
        .map_err(|error| AnalyzerExpressionError::Call {
            owner,
            failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
        })?;
        work.record_candidate_argument_probes(0)
            .and_then(|_| work.charge_argument_mapping(0))
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                    owner,
                })
            })?;
        let probe = self.run_candidate_fact_transaction::<_, CandidateFactOperationFailure>(
            |this, authority, _transaction_authority| {
                let candidate_context =
                    AnalyzerExpressionContext::candidate(authority, Rc::clone(&this.call_frames));
                let outcome = this.prepare_candidate(
                    PreparedCandidateRequest {
                        module,
                        owner,
                        inputs: PreparedCandidateInputs::SemanticOnly(structural_inputs.clone()),
                        candidate: Arc::clone(&candidate),
                        current_group: candidate.call_group(),
                        expected_result: expected,
                        callee_inputs:
                            crate::callable::PreparedCallCalleeConstraintInputs::DialogueApplication,
                        pass: CandidateEvaluationPass::Probe,
                        attempt: None,
                        context: &candidate_context,
                        dialogue_patch_admissions: &[],
                    },
                    &mut work,
                );
                drop(candidate_context);
                match outcome? {
                    PreparedCandidateRunOutcome::Accepted { transaction, rank } => {
                        Ok(CandidateFactTransactionAction::Extract((transaction, rank)))
                    }
                    PreparedCandidateRunOutcome::Rejected { .. } => {
                        Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::CallResolutionFailed { owner },
                        )
                        .into())
                    }
                }
            },
        )?;
        let (ran, _rank, outer_projection) = match probe {
            CandidateFactTransactionOutcome::Extracted {
                value: (ran, rank),
                projection,
            } => (ran, rank, *projection),
            CandidateFactTransactionOutcome::Committed(_)
            | CandidateFactTransactionOutcome::RolledBack(_) => {
                return Err(AnalyzerExpressionError::fact(
                    CandidateFactTransactionViolation::UnrecoverableLedger,
                ));
            }
        };
        let transaction =
            ran.into_prepared_application()
                .map_err(|error| AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        error,
                    )),
                })?;
        let inventory = AnalyzerPreparedCandidateInventory::from_considered(
            transaction.candidate(),
            considered,
        )
        .map_err(|error| AnalyzerExpressionError::Call {
            owner,
            failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
        })?;
        let metadata = AnalyzerPreparedCandidateMetadata::new(
            owner,
            expression_resolution,
            AnalyzerPreparedCalleeExpression::none(),
            self.enclosing_ordinary_callable(module, owner)
                .map_err(AnalyzerExpressionError::fatal)?,
            inventory,
            Vec::new(),
            None,
            work.call_accounting(),
        );
        self.run_candidate_fact_transaction::<_, CandidateFactOperationFailure>(
            |this, _authority, transaction_authority| {
                let result = this.apply_sealed_candidate(
                    transaction,
                    outer_projection,
                    metadata,
                    &transaction_authority,
                    crate::callable::CheckedCallSite::DialogueApplication(owner),
                )?;
                Ok(CandidateFactTransactionAction::Commit(result))
            },
        )?
        .into_committed()
        .map_err(AnalyzerExpressionError::fact)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "selected-call publication validates the complete semantic candidate and argument-accounting record"
    )]
    fn publish_selected_call_in_transaction(
        &mut self,
        source: CallSource<'_>,
        mut resolution: ResolvedCallQuery,
        mut batch: PreparedCandidateBatch,
        selected_index: usize,
        singleton: bool,
        transaction_authority: &CandidateFactTransactionAuthority<'_>,
    ) -> Result<CheckedExpression, CandidateFactOperationFailure> {
        let (selected_transaction, selected_rank) = batch
            .probes
            .remove(selected_index)
            .into_accepted(source.owner)?;
        let (selected_ran, selected_outer_projection) = selected_transaction.into_parts();
        let selected_transaction = selected_ran.into_prepared_application().map_err(|error| {
            CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                owner: source.owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
            })
        })?;
        let selected = selected_transaction.selected_shared().clone();
        let current_group = selected_transaction.current_group();
        let (selected_transaction, outer_projection) = if singleton {
            (selected_transaction, selected_outer_projection)
        } else {
            resolution
                .work
                .record_selected_replay_argument_visits(resolution.argument_count)
                .map_err(|_| {
                    AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::CallResolutionFailed {
                            owner: source.owner,
                        },
                    )
                })?;
            let replay = self.run_candidate_fact_transaction::<_, CandidateFactOperationFailure>(
                |this, authority, _transaction_authority| {
                    let context = AnalyzerExpressionContext::candidate(
                        authority,
                        Rc::clone(&this.call_frames),
                    );
                    let replay = this.prepare_candidate(
                        PreparedCandidateRequest {
                            module: source.module,
                            owner: source.owner,
                            inputs: PreparedCandidateInputs::Authored {
                                arguments: source.call.arguments(),
                                dialogue_application_metadata: source.dialogue_application_metadata,
                            },
                            candidate: Arc::clone(&selected),
                            current_group,
                            expected_result: source.expected,
                            callee_inputs: resolution.callee_inputs.clone(),
                            pass: CandidateEvaluationPass::SelectedReplay,
                            attempt: Some(source.attempt),
                            context: &context,
                            dialogue_patch_admissions: &resolution.dialogue_patch_admissions,
                        },
                        &mut resolution.work,
                    );
                    drop(context);
                    match replay? {
                        accepted @ PreparedCandidateRunOutcome::Accepted { .. } => {
                            Ok(CandidateFactTransactionAction::Extract(accepted))
                        }
                        rejected @ PreparedCandidateRunOutcome::Rejected { .. } => {
                            Ok(CandidateFactTransactionAction::Rollback(rejected))
                        }
                    }
                },
            )?;
            let (replay_ran, replay_outer_projection, replay_rank) = match replay {
                CandidateFactTransactionOutcome::Extracted {
                    value: PreparedCandidateRunOutcome::Accepted { transaction, rank },
                    projection,
                } => (transaction, *projection, rank),
                CandidateFactTransactionOutcome::RolledBack(
                    PreparedCandidateRunOutcome::Rejected { .. },
                ) => {
                    return Err(AnalyzerExpressionError::Call {
                        owner: source.owner,
                        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            crate::callable::CallConstraintInvariant::ReplayRejected,
                        )),
                    }
                    .into());
                }
                CandidateFactTransactionOutcome::Committed(_)
                | CandidateFactTransactionOutcome::Extracted { .. }
                | CandidateFactTransactionOutcome::RolledBack { .. } => {
                    return Err(AnalyzerExpressionError::Call {
                        owner: source.owner,
                        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            crate::callable::CallConstraintInvariant::ReplayTransactionShapeMismatch,
                        )),
                    }
                    .into());
                }
            };
            let replay_transaction = replay_ran.into_prepared_application().map_err(|error| {
                CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                    owner: source.owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        error,
                    )),
                })
            })?;
            let replay_mismatch = if selected_rank != replay_rank {
                Some(crate::callable::CallConstraintInvariant::ReplayRankMismatch)
            } else {
                selected_transaction.replay_mismatch(&replay_transaction)
            };
            if let Some(replay_mismatch) = replay_mismatch {
                return Err(AnalyzerExpressionError::Call {
                    owner: source.owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        replay_mismatch,
                    )),
                }
                .into());
            }
            let (
                _selected_application,
                _selected_callee_inputs,
                _selected_input_projection,
                selected_branch,
                _selected_closed_sources,
                _selected_projections,
            ) = selected_transaction.into_parts();
            self.facts
                .discard_candidate_projection(selected_outer_projection)
                .map_err(|violation| {
                    CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fact(
                        violation,
                    ))
                })?;
            if let constraints::AnalyzerCallSealedBranch::Materialized { projection } =
                selected_branch
            {
                self.facts
                    .discard_candidate_projection(projection)
                    .map_err(|violation| {
                        CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fact(
                            violation,
                        ))
                    })?;
            }
            (replay_transaction, replay_outer_projection)
        };
        let result = selected_transaction.result().map_err(|error| {
            CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                owner: source.owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
            })
        })?;
        resolution
            .work
            .record_retained_argument_fact_publications(resolution.argument_count)
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: source.owner,
                })
            })?;

        let effects = match provisional_call_effects(&selected, current_group) {
            Ok(effects) => effects,
            Err(error) => {
                return Err(AnalyzerExpressionError::fatal(error).into());
            }
        };
        let callee_expression = self
            .stage_resolved_callee_expression(
                source.owner,
                source.module,
                source.call,
                &selected,
                &resolution.callee_inputs,
                &result,
                &provisional_callable_effects(&selected),
            )
            .map_err(AnalyzerExpressionError::fatal)?;
        let expression_resolution = self
            .checked_character_dialogue_resolution(
                source,
                &selected,
                resolution.dialogue_context,
                &selected_transaction,
            )
            .map_err(|failure| match failure {
                CharacterDialogueResolutionFailure::Semantic(error) => {
                    AnalyzerExpressionError::fatal(error)
                }
                CharacterDialogueResolutionFailure::Constraint(error) => {
                    AnalyzerExpressionError::Call {
                        owner: source.owner,
                        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            error,
                        )),
                    }
                }
            })?
            .unwrap_or(CheckedExpressionResolution::Call);
        let inventory = AnalyzerPreparedCandidateInventory::from_considered(
            selected_transaction.candidate(),
            resolution.considered.clone(),
        )
        .map_err(|error| {
            CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                owner: source.owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
            })
        })?;
        let metadata = AnalyzerPreparedCandidateMetadata::new(
            source.owner,
            expression_resolution.clone(),
            callee_expression,
            self.enclosing_ordinary_callable(source.module, source.owner)
                .map_err(AnalyzerExpressionError::fatal)?,
            inventory,
            Vec::new(),
            resolution.function_value_origin.take(),
            resolution.work.call_accounting(),
        );
        let result = self.apply_sealed_candidate(
            selected_transaction,
            outer_projection,
            metadata,
            transaction_authority,
            crate::callable::CheckedCallSite::HirCall(source.owner),
        )?;
        Ok(CheckedExpression::new(
            result,
            if source.expected.is_some() {
                CheckedTypeSelection::Expected
            } else {
                CheckedTypeSelection::Inferred
            },
            effects.concrete().clone(),
            expression_resolution,
        ))
    }

    fn apply_sealed_candidate(
        &mut self,
        transaction: PreparedCallApplicationTransaction,
        outer_projection: super::state::CandidateSemanticProjection,
        metadata: AnalyzerPreparedCandidateMetadata,
        transaction_authority: &CandidateFactTransactionAuthority<'_>,
        site: crate::callable::CheckedCallSite,
    ) -> Result<TypeKind, CandidateFactOperationFailure> {
        let owner = match site {
            crate::callable::CheckedCallSite::HirCall(owner)
            | crate::callable::CheckedCallSite::DialogueApplication(owner) => owner,
        };
        self.facts
            .apply_candidate_projection(transaction_authority, outer_projection)
            .map_err(|failure| CandidateFactOperationFailure::Projection(Box::new(failure)))?;
        let result = transaction.result().map_err(|error| {
            CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
            })
        })?;
        let (
            application,
            callee_inputs,
            input_projection,
            sealed_branch,
            closed_sources,
            _projections,
        ) = transaction.into_parts();
        match sealed_branch {
            constraints::AnalyzerCallSealedBranch::Empty => {}
            constraints::AnalyzerCallSealedBranch::Materialized { projection } => {
                self.facts
                    .apply_candidate_projection(transaction_authority, projection)
                    .map_err(|failure| {
                        CandidateFactOperationFailure::Projection(Box::new(failure))
                    })?;
            }
        }
        let record = AnalyzerPreparedCandidateRecord::seal(
            metadata,
            application.selected().id(),
            callee_inputs,
            input_projection,
            closed_sources,
        )
        .map_err(|error| {
            CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
            })
        })?;
        let prefix =
            AnalyzerPreparedCallPrefix::new(site, application, record).map_err(|error| {
                CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        error,
                    )),
                })
            })?;
        let (projected_result, _continuation) = self
            .facts
            .seal_selected_application(transaction_authority, site, prefix)
            .map_err(|violation| {
                CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fact(violation))
            })?;
        if projected_result != result {
            return Err(CandidateFactOperationFailure::Expression(
                AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::PreparedFunctionTypeMismatch,
                    )),
                },
            ));
        }
        Ok(result)
    }

    fn publish_ambiguous_call(
        &mut self,
        source: CallSource<'_>,
        resolution: ResolvedCallQuery,
        batch: PreparedCandidateBatch,
        primary: usize,
        tied: Vec<usize>,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        let ResolvedCallQuery {
            callee,
            callee_inputs,
            considered,
            work,
            ..
        } = resolution;
        let outcome = self.run_candidate_fact_transaction::<_, AnalyzerExpressionError>(
            |this, _expression_authority, transaction_authority| {
                let recovery = batch.into_recovery(source.owner, primary, tied)?;
                this.apply_primary_recovery_projection(
                    &transaction_authority,
                    recovery.primary_projection,
                    recovery.discarded_projections,
                )?;
                let argument_count = source.call.arguments().len();
                if recovery
                    .primary_argument_count
                    .is_some_and(|count| count != argument_count)
                {
                    return Err(AnalyzerExpressionError::Call {
                        owner: source.owner,
                        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                        )),
                    });
                }
                this.publish_recovery_call(
                    RecoveryCall {
                        source,
                        callee,
                        callee_inputs,
                        candidates: recovery.candidates,
                        considered,
                        argument_count,
                        result: recovery.primary_result,
                        work,
                        ambiguous: true,
                    },
                    &transaction_authority,
                )
                .map(CandidateFactTransactionAction::Commit)
                .map_err(AnalyzerExpressionError::fatal)
            },
        )?;
        outcome
            .into_committed()
            .map_err(AnalyzerExpressionError::fact)
    }

    fn publish_rejected_call(
        &mut self,
        source: CallSource<'_>,
        resolution: ResolvedCallQuery,
        batch: PreparedCandidateBatch,
        primary: usize,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        let retained = 0..batch.probes.len();
        let outcome = self.run_candidate_fact_transaction::<_, AnalyzerExpressionError>(
            |this, _expression_authority, transaction_authority| {
                let recovery = batch.into_recovery(source.owner, primary, retained)?;
                this.apply_primary_recovery_projection(
                    &transaction_authority,
                    recovery.primary_projection,
                    recovery.discarded_projections,
                )?;
                let argument_count = source.call.arguments().len();
                if recovery
                    .primary_argument_count
                    .is_some_and(|count| count != argument_count)
                {
                    return Err(AnalyzerExpressionError::Call {
                        owner: source.owner,
                        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                        )),
                    });
                }
                let considered = resolution.considered;
                this.publish_recovery_call(
                    RecoveryCall {
                        source,
                        callee: resolution.callee,
                        callee_inputs: resolution.callee_inputs.clone(),
                        candidates: recovery.candidates,
                        considered,
                        argument_count,
                        result: recovery.primary_result,
                        work: resolution.work,
                        ambiguous: false,
                    },
                    &transaction_authority,
                )
                .map(CandidateFactTransactionAction::Commit)
                .map_err(AnalyzerExpressionError::fatal)
            },
        )?;
        outcome
            .into_committed()
            .map_err(AnalyzerExpressionError::fact)
    }

    fn apply_primary_recovery_projection(
        &mut self,
        transaction_authority: &CandidateFactTransactionAuthority<'_>,
        primary: PreparedCandidateSemanticProjection,
        discarded: Vec<PreparedCandidateSemanticProjection>,
    ) -> Result<(), AnalyzerExpressionError> {
        for projection in discarded {
            self.discard_recovery_projection(projection)?;
        }
        let PreparedCandidateSemanticProjection { outer, branch } = primary;
        self.facts
            .apply_candidate_projection(transaction_authority, outer)
            .map_err(|failure| {
                let (violation, _projection) = failure.into_parts();
                AnalyzerExpressionError::fact(violation)
            })?;
        if let constraints::AnalyzerCallSealedBranch::Materialized { projection } = branch {
            self.facts
                .apply_candidate_projection(transaction_authority, projection)
                .map_err(|failure| {
                    let (violation, _projection) = failure.into_parts();
                    AnalyzerExpressionError::fact(violation)
                })?;
        }
        Ok(())
    }

    fn discard_recovery_projection(
        &self,
        projection: PreparedCandidateSemanticProjection,
    ) -> Result<(), AnalyzerExpressionError> {
        let PreparedCandidateSemanticProjection { outer, branch } = projection;
        self.facts
            .discard_candidate_projection(outer)
            .map_err(AnalyzerExpressionError::fact)?;
        if let constraints::AnalyzerCallSealedBranch::Materialized { projection } = branch {
            self.facts
                .discard_candidate_projection(projection)
                .map_err(AnalyzerExpressionError::fact)?;
        }
        Ok(())
    }

    fn publish_recovery_call(
        &mut self,
        recovery: RecoveryCall<'_>,
        transaction_authority: &CandidateFactTransactionAuthority<'_>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let RecoveryCall {
            source,
            callee,
            callee_inputs,
            candidates,
            considered,
            argument_count,
            result,
            mut work,
            ambiguous,
        } = recovery;
        let primary =
            candidates
                .first()
                .ok_or(FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: source.owner,
                })?;
        work.record_retained_argument_fact_publications(
            u64::try_from(argument_count)
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;
        let callee_expression = self.stage_resolved_callee_expression(
            source.owner,
            source.module,
            source.call,
            primary,
            &callee_inputs,
            &result,
            &provisional_callable_effects(primary),
        )?;
        let tied = candidates
            .iter()
            .map(|candidate| candidate.id().clone())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let outcome = if ambiguous {
            AnalyzerPreparedUnselectedOutcome::Ambiguous {
                callee: Some(callee),
                considered,
                tied,
            }
        } else {
            AnalyzerPreparedUnselectedOutcome::Rejected {
                callee: Some(callee),
                candidates: considered,
            }
        };
        let enclosing_callable = self.enclosing_ordinary_callable(source.module, source.owner)?;
        let selected_expression_inventory =
            arcweft_lang_hir::project::HirSelectedCallExpressionInventory::new(
                source
                    .call
                    .arguments()
                    .iter()
                    .map(HirCallArgument::value)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                callee_expression.semantic_expression(),
            );
        self.facts
            .insert_unselected_call(
                transaction_authority,
                crate::callable::CheckedCallSite::HirCall(source.owner),
                AnalyzerPreparedUnselectedCall {
                    enclosing_callable,
                    outcome,
                    diagnostics: Vec::new(),
                    accounting: work.call_accounting(),
                    selected_expression_inventory,
                },
            )
            .map_err(FinalSemanticAnalysisError::from)?;
        let selection = if source
            .expected
            .is_some_and(|expected| expected.accepts(&result))
        {
            CheckedTypeSelection::Expected
        } else {
            CheckedTypeSelection::Inferred
        };
        Ok(CheckedExpression::new(
            result,
            selection,
            EffectSet::new(),
            CheckedExpressionResolution::Call,
        ))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "callee child staging exhaustively follows the final-HIR callee variants and their typed owners"
    )]
    fn stage_call_callee_children(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        call: &HirCallExpr,
    ) -> Result<StagedCallCalleeChildren, AnalyzerExpressionError> {
        let mut function_value_origin = None;
        match call.callee() {
            HirCallCallee::Value { value } => {
                let expression = module.resolve_expr(*value).map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                })?;
                if let HirExprKind::Select(select) = expression.kind() {
                    self.evaluate_expression(context, select.target(), None)?;
                } else if !matches!(expression.kind(), HirExprKind::Path(_)) {
                    self.evaluate_expression(context, *value, None)?;
                } else if let HirExprKind::Path(path) = expression.kind() {
                    let path = path
                        .as_resolved()
                        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)
                        .map_err(AnalyzerExpressionError::fatal)?;
                    if let Some(resolution) = self
                        .resolve_path_value(module, *value, expression.scope(), path)
                        .map_err(AnalyzerExpressionError::fatal)?
                    {
                        let ty = self
                            .staged_value_resolution_type(&resolution, *value)
                            .map_err(AnalyzerExpressionError::fatal)?;
                        if let Some(ty) = ty
                            && !self.facts.expressions().contains_key(value)
                        {
                            self.facts
                                .publish_new_expression(
                                    *value,
                                    CheckedExpression::new(
                                        ty,
                                        CheckedTypeSelection::Inferred,
                                        EffectSet::new(),
                                        CheckedExpressionResolution::Value(resolution),
                                    ),
                                )
                                .map_err(|_| {
                                    AnalyzerExpressionError::fatal(
                                        FinalSemanticAnalysisError::WrongPayloadFamily,
                                    )
                                })?;
                        }
                    }
                }
                if let Some(checked) = self.facts.expressions().get(value)
                    && matches!(checked.ty(), TypeKind::Function { .. })
                    && !matches!(
                        checked.checked_resolution(),
                        Some(CheckedExpressionResolution::Value(
                            CheckedValueResolution::ProjectCallable(_)
                        ))
                    )
                    && match expression.kind() {
                        HirExprKind::Select(_) => false,
                        HirExprKind::Path(_) => matches!(
                            checked.checked_resolution(),
                            Some(CheckedExpressionResolution::Value(
                                CheckedValueResolution::Local(_)
                            ))
                        ),
                        _ => true,
                    }
                {
                    function_value_origin =
                        Some(self.stage_function_value_origin(context, module, *value)?);
                }
            }
            HirCallCallee::UnresolvedDot {
                value_receiver,
                nominal_receiver,
                separator,
                member,
            } => {
                let expression = module.resolve_expr(*value_receiver).map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                })?;
                if !matches!(expression.kind(), HirExprKind::Path(_)) {
                    self.evaluate_expression(context, *value_receiver, None)?;
                } else if let HirExprKind::Path(path) = expression.kind() {
                    let path = path
                        .as_resolved()
                        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)
                        .map_err(AnalyzerExpressionError::fatal)?;
                    let member = member
                        .resolved()
                        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)
                        .map_err(AnalyzerExpressionError::fatal)?;
                    let full_path = path.with_terminal_member(member);
                    let full_resolution = self
                        .resolve_path_value(module, *value_receiver, expression.scope(), &full_path)
                        .map_err(AnalyzerExpressionError::fatal)?;
                    match full_resolution {
                        Some(resolution) => {
                            if let Some(ty) = self
                                .staged_value_resolution_type(&resolution, *value_receiver)
                                .map_err(AnalyzerExpressionError::fatal)?
                                && !self.facts.expressions().contains_key(value_receiver)
                            {
                                self.facts
                                    .publish_new_expression(
                                        *value_receiver,
                                        CheckedExpression::new(
                                            ty,
                                            CheckedTypeSelection::Inferred,
                                            EffectSet::new(),
                                            CheckedExpressionResolution::Value(resolution),
                                        ),
                                    )
                                    .map_err(|_| {
                                        AnalyzerExpressionError::fatal(
                                            FinalSemanticAnalysisError::WrongPayloadFamily,
                                        )
                                    })?;
                            }
                        }
                        None => {
                            match self
                                .resolve_path_value(
                                    module,
                                    *value_receiver,
                                    expression.scope(),
                                    path,
                                )
                                .map_err(AnalyzerExpressionError::fatal)?
                            {
                                Some(resolution) => {
                                    if let Some(ty) = self
                                        .staged_value_resolution_type(&resolution, *value_receiver)
                                        .map_err(AnalyzerExpressionError::fatal)?
                                        && !self.facts.expressions().contains_key(value_receiver)
                                    {
                                        self.facts
                                            .publish_new_expression(
                                                *value_receiver,
                                                CheckedExpression::new(
                                                    ty,
                                                    CheckedTypeSelection::Inferred,
                                                    EffectSet::new(),
                                                    CheckedExpressionResolution::Value(resolution),
                                                ),
                                            )
                                            .map_err(|_| {
                                                AnalyzerExpressionError::fatal(
                                                    FinalSemanticAnalysisError::WrongPayloadFamily,
                                                )
                                            })?;
                                    }
                                }
                                None => {
                                    let line_context = path.lexical_name() == Some("line")
                                        && scope_is_dialogue_line_plan(module, expression.scope());
                                    if line_context {
                                        self.facts
                                            .publish_new_expression(
                                                *value_receiver,
                                                CheckedExpression::new(
                                                    TypeKind::LineContext,
                                                    CheckedTypeSelection::Inferred,
                                                    EffectSet::new(),
                                                    CheckedExpressionResolution::Value(
                                                        CheckedValueResolution::LineContext,
                                                    ),
                                                ),
                                            )
                                            .map_err(|_| {
                                                AnalyzerExpressionError::fatal(
                                                    FinalSemanticAnalysisError::WrongPayloadFamily,
                                                )
                                            })?;
                                    } else if prepare_language_free_dot_path(
                                        self.catalogs.world.environment().callable_catalog(),
                                        *value_receiver,
                                        expression,
                                        member,
                                        &self.catalogs.callable_limits,
                                    )
                                    .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
                                        owner: *value_receiver,
                                    })
                                    .map_err(AnalyzerExpressionError::fatal)?
                                    .is_none()
                                    {
                                        let receiver = nominal_receiver
                                            .type_id()
                                            .ok_or(
                                                FinalSemanticAnalysisError::CallResolutionFailed {
                                                    owner: *value_receiver,
                                                },
                                            )
                                            .map_err(AnalyzerExpressionError::fatal)?;
                                        match self
                                            .resolve_associated_receiver_type(receiver)
                                            .map_err(AnalyzerExpressionError::fatal)?
                                        {
                                            AssociatedReceiverTypeResolution::Complete(_) => {}
                                            AssociatedReceiverTypeResolution::WrongArity(
                                                result,
                                            ) => {
                                                return Ok(StagedCallCalleeChildren {
                                                    recovery: Some(AssociatedReceiverRecovery {
                                                        receiver,
                                                        separator: *separator,
                                                        result,
                                                    }),
                                                    function_value_origin: None,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            HirCallCallee::Associated {
                receiver,
                separator,
                ..
            } => {
                let receiver = receiver
                    .type_id()
                    .ok_or(FinalSemanticAnalysisError::RecoveredOwner)
                    .map_err(AnalyzerExpressionError::fatal)?;
                match self
                    .resolve_associated_receiver_type(receiver)
                    .map_err(AnalyzerExpressionError::fatal)?
                {
                    AssociatedReceiverTypeResolution::Complete(_) => {}
                    AssociatedReceiverTypeResolution::WrongArity(result) => {
                        return Ok(StagedCallCalleeChildren {
                            recovery: Some(AssociatedReceiverRecovery {
                                receiver,
                                separator: *separator,
                                result,
                            }),
                            function_value_origin: None,
                        });
                    }
                }
            }
        }
        Ok(StagedCallCalleeChildren {
            recovery: None,
            function_value_origin,
        })
    }

    fn stage_function_value_origin(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        expression: ExprId,
    ) -> Result<PreparedFunctionValueOriginEvidence, AnalyzerExpressionError> {
        let mut progress = prepare_function_value_origin_query(
            Arc::clone(&self.topology),
            module,
            expression,
            self.facts.expressions(),
        )
        .map_err(|error| self.map_function_value_origin_query_error(expression, error))?;
        loop {
            match progress {
                PreparedFunctionValueOriginProgress::Ready(evidence) => {
                    if let PreparedFunctionValueOriginProducer::Call(
                        crate::callable::CheckedCallSite::HirCall(origin),
                    ) = evidence.producer()
                    {
                        self.evaluate_expression(context, *origin, None)?;
                    }
                    return Ok(evidence);
                }
                PreparedFunctionValueOriginProgress::Need(need) => {
                    let owner = need.expression();
                    self.evaluate_expression(context, owner, None)?;
                    let checked = self.facts.expressions().get(&owner).ok_or_else(|| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                        )
                    })?;
                    progress = need.resume(owner, checked, module).map_err(|error| {
                        self.map_function_value_origin_query_error(expression, error)
                    })?;
                }
            }
        }
    }

    fn map_function_value_origin_query_error(
        &self,
        owner: ExprId,
        error: PreparedFunctionValueOriginQueryError,
    ) -> AnalyzerExpressionError {
        match error {
            PreparedFunctionValueOriginQueryError::Composite => AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                    crate::callable::CallConstraintInvariant::CompositeFunctionValue,
                )),
            },
            PreparedFunctionValueOriginQueryError::Cycle => {
                AnalyzerExpressionError::Invariant(AnalyzerExpressionInvariant::Cycle { owner })
            }
            PreparedFunctionValueOriginQueryError::Invalid => {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
            }
            PreparedFunctionValueOriginQueryError::CaptureTopologyMismatch(violation)
            | PreparedFunctionValueOriginQueryError::CaptureProducerMismatch(violation)
            | PreparedFunctionValueOriginQueryError::CaptureEvidenceMismatch(violation) => {
                AnalyzerExpressionError::invariant(FinalSemanticAnalysisError::from(violation))
            }
        }
    }

    fn staged_value_resolution_type(
        &self,
        resolution: &CheckedValueResolution,
        expression: ExprId,
    ) -> Result<Option<TypeKind>, FinalSemanticAnalysisError> {
        match resolution {
            CheckedValueResolution::Local(local) => self
                .facts
                .locals()
                .get(local)
                .cloned()
                .map(Some)
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: *local }),
            CheckedValueResolution::ProjectCallable(_) => Ok(None),
            _ => value_resolution_type(self.catalogs.world, resolution)
                .map(Some)
                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: expression }),
        }
    }

    fn presentation_character_owner(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        arguments: &[HirCallArgument],
        id: Option<crate::callable::PresentationCallableId>,
    ) -> Result<Option<ResolvedCharacterOwner>, AnalyzerExpressionError> {
        let Some(id) = id else {
            return Ok(None);
        };
        let schema = id.checker_signature_schema().map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
        })?;
        let group = schema.group(CallableGroupIndex::ZERO).ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
        })?;
        let Some(parameter) = group.parameters().iter().find(|parameter| {
            parameter
                .name()
                .is_some_and(|name| name.as_str() == "character")
        }) else {
            return Ok(None);
        };
        let Some(argument) = Self::presentation_character_argument(parameter, arguments) else {
            return Ok(None);
        };
        let checked = self.evaluate_expression(context, argument.value(), None)?;
        let Some(CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item))) =
            checked.checked_resolution()
        else {
            return Ok(None);
        };
        let Some(character) = item.character() else {
            return Ok(None);
        };
        if self
            .catalogs
            .world
            .environment()
            .character_manifest(&character)
            .is_none()
        {
            return Ok(None);
        }
        Ok(Some(ResolvedCharacterOwner::new(
            character,
            CharacterOwnerSource::EntityReference,
        )))
    }

    fn presentation_character_argument<'a>(
        parameter: &crate::callable::CallableParameter,
        arguments: &'a [HirCallArgument],
    ) -> Option<&'a HirCallArgument> {
        let name = parameter.name()?;
        let mut positional_index = 0usize;
        let mut positional_character = false;
        let mut named_character = false;
        let mut character = None;

        for argument in arguments {
            match argument {
                HirCallArgument::Positional { .. } => {
                    if positional_index == 0 && !named_character {
                        if positional_character {
                            return None;
                        }
                        positional_character = true;
                        character = Some(argument);
                    }
                    positional_index = positional_index.checked_add(1)?;
                }
                HirCallArgument::Named { .. }
                    if argument
                        .resolved_name()
                        .is_some_and(|candidate| candidate.as_str() == name.as_str()) =>
                {
                    if positional_character || named_character {
                        return None;
                    }
                    named_character = true;
                    character = Some(argument);
                }
                HirCallArgument::Named { .. } => {}
                HirCallArgument::Spread { .. } => return None,
            }
        }

        character
    }

    fn prepare_candidate(
        &mut self,
        request: PreparedCandidateRequest<'_, '_>,
        work: &mut ResolverWork,
    ) -> Result<PreparedCandidateRunOutcome, AnalyzerExpressionError> {
        let PreparedCandidateRequest {
            module,
            owner,
            inputs,
            candidate,
            current_group,
            expected_result,
            callee_inputs,
            pass,
            attempt,
            context,
            dialogue_patch_admissions,
        } = request;
        let implicit = match candidate.instantiation() {
            CallableInstantiation::Extension {
                group, parameter, ..
            } if *group == current_group => Some(*parameter),
            _ => None,
        };
        let authored_arguments;
        let input_projection = match inputs {
            PreparedCandidateInputs::Authored {
                arguments,
                dialogue_application_metadata,
            } => {
                authored_arguments = arguments;
                let Some(mapping) = map_call_arguments(
                    module,
                    candidate.schema(),
                    candidate.id(),
                    current_group,
                    arguments,
                    implicit,
                ) else {
                    let result = candidate
                        .result_type_for_group(current_group)
                        .ok_or(FinalSemanticAnalysisError::CallResolutionFailed { owner })
                        .map_err(AnalyzerExpressionError::fatal)?;
                    return Ok(PreparedCandidateRunOutcome::Rejected {
                        candidate: Arc::clone(&candidate),
                        result,
                        evidence: PreparedCandidateRejection::Mapping(
                            PreparedCallMappingRejection::from_authored(arguments),
                        ),
                        branch: constraints::AnalyzerCallSealedBranch::Empty,
                    });
                };
                let mapping = mapping
                    .seal_dialogue_application_metadata(
                        owner,
                        candidate.schema(),
                        dialogue_application_metadata,
                    )
                    .map_err(|failure| AnalyzerExpressionError::Call {
                        owner,
                        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            failure,
                        )),
                    })?;
                crate::callable::PreparedCallInputProjection::Authored(mapping)
            }
            PreparedCandidateInputs::SemanticOnly(inputs) => {
                authored_arguments = &[];
                if implicit.is_some() || !inputs.validates(&candidate) {
                    return Err(AnalyzerExpressionError::Call {
                        owner,
                        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                        )),
                    });
                }
                crate::callable::PreparedCallInputProjection::SemanticOnly(inputs)
            }
        };
        let mut rank = AcceptedCandidateRank {
            exact_matches: 0,
            declared_exact_matches: 0,
            unchecked_or_open: input_projection.unchecked_or_open_slots(),
            omitted_parameters: input_projection.omitted_parameters(),
            authority: candidate.authority(),
        };
        let default_result = candidate
            .result_type_for_group(current_group)
            .ok_or(FinalSemanticAnalysisError::CallResolutionFailed { owner })
            .map_err(AnalyzerExpressionError::fatal)?;
        let prepared_candidate = Arc::clone(&candidate);
        let enclosing = EnclosingGenericParameterScope::sealed(
            std::iter::empty::<crate::types::GenericTypeParameterId>(),
            std::iter::empty::<crate::types::GenericConstParameterId>(),
        )
        .map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                owner,
            })
        })?;
        let constraint_set = validate_and_prepare_call_constraints(
            self.facts
                .prepared_calls()
                .map_err(AnalyzerExpressionError::fact)?,
            prepared_candidate,
            input_projection,
            authored_arguments,
            expected_result,
            owner,
            callee_inputs,
            dialogue_patch_admissions,
            &enclosing,
        )
        .map_err(|failure| terminal_call_constraint_failure(owner, failure))?;
        let transaction = match run_prepared_candidate(
            self,
            work,
            context,
            pass,
            attempt.cloned(),
            constraint_set,
        ) {
            Ok(ran) => ran,
            Err(crate::types::constraints::TypeConstraintFailure::Rejected(error)) => {
                if let crate::types::constraints::TypeConstraintCandidateFailure::SourceProjection(
                    rejected,
                ) = &error
                    && let Some(admission) = dialogue_patch_admissions
                        .iter()
                        .find(|admission| admission.accepts_rejected_source_projection(rejected))
                {
                    return Err(AnalyzerExpressionError::fatal(
                        admission.mismatch_failure(rejected.actual().clone()),
                    ));
                }
                return Ok(PreparedCandidateRunOutcome::Rejected {
                    candidate: Arc::clone(&candidate),
                    result: default_result,
                    evidence: PreparedCandidateRejection::Constraint,
                    branch: constraints::AnalyzerCallSealedBranch::Empty,
                });
            }
            Err(crate::types::constraints::TypeConstraintFailure::Abort(error)) => {
                return Err(AnalyzerExpressionError::Abort(error));
            }
            Err(crate::types::constraints::TypeConstraintFailure::FatalSource(error))
                if error.cause().direct_final_semantic().is_some() =>
            {
                let diagnostic = error
                    .cause()
                    .direct_final_semantic()
                    .expect("guard proves a direct terminal source diagnostic")
                    .clone();
                return Err(AnalyzerExpressionError::fatal(diagnostic));
            }
            Err(failure) => {
                return Err(terminal_lower_constraint_failure(owner, failure));
            }
        };
        let result = transaction.result().clone();
        rank.exact_matches = rank
            .exact_matches
            .checked_add(transaction.exact_argument_matches())
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)
            .map_err(AnalyzerExpressionError::fatal)?;
        rank.declared_exact_matches = transaction.declared_exact_argument_matches();
        if expected_result == Some(&result) {
            rank.exact_matches = rank
                .exact_matches
                .checked_add(1)
                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)
                .map_err(AnalyzerExpressionError::fatal)?;
        }
        Ok(PreparedCandidateRunOutcome::Accepted { transaction, rank })
    }

    pub(super) fn evaluate_call_constraint_source(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        source: AnalyzerCallConstraintSource,
        expectation: super::expressions::AnalyzerExpressionExpectation<'_>,
    ) -> Result<super::PreparedExpressionFact, AnalyzerExpressionError> {
        match source {
            AnalyzerCallConstraintSource::BaseInstantiation
            | AnalyzerCallConstraintSource::DialogueApplicationMetadata { .. }
            | AnalyzerCallConstraintSource::DialogueApplicationOperand { .. } => Err(
                AnalyzerExpressionError::invariant(FinalSemanticAnalysisError::WrongPayloadFamily),
            ),
            AnalyzerCallConstraintSource::Argument {
                source: CheckedCallArgumentSlotSource::CompactNumericElement { sequence, ordinal },
                ..
            }
            | AnalyzerCallConstraintSource::DialoguePatch {
                source: CheckedCallArgumentSlotSource::CompactNumericElement { sequence, ordinal },
                ..
            } => {
                let actual = self
                    .compact_numeric_element_type(sequence, ordinal, expectation.contextual_shape())
                    .map_err(AnalyzerExpressionError::fatal)?;
                Ok(CheckedExpression::new(
                    actual,
                    if expectation.is_contextual() {
                        CheckedTypeSelection::Expected
                    } else {
                        CheckedTypeSelection::Inferred
                    },
                    EffectSet::new(),
                    CheckedExpressionResolution::Structural,
                )
                .into())
            }
            AnalyzerCallConstraintSource::Argument {
                source: CheckedCallArgumentSlotSource::Expression(expression),
                ..
            }
            | AnalyzerCallConstraintSource::DialoguePatch {
                source: CheckedCallArgumentSlotSource::Expression(expression),
                ..
            } => self.evaluate_expression_with_expectation(context, expression, expectation),
            AnalyzerCallConstraintSource::Receiver { source }
            | AnalyzerCallConstraintSource::Result { source } => {
                self.evaluate_expression_with_expectation(context, source, expectation)
            }
        }
    }

    fn compact_numeric_element_type(
        &self,
        owner: ExprId,
        ordinal: u32,
        expected: Option<&TypeKind>,
    ) -> Result<TypeKind, FinalSemanticAnalysisError> {
        let module = self.module(owner.module())?;
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::NumericBracketSequence(sequence) = expression.kind() else {
            return Err(FinalSemanticAnalysisError::InvalidOwner);
        };
        let ordinal =
            usize::try_from(ordinal).map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        sequence
            .elements()
            .get(ordinal)
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        Ok(infer_compact_numeric_element_type(
            sequence.common_suffix(),
            expected,
        ))
    }

    fn stage_resolved_callee_expression(
        &mut self,
        owner: ExprId,
        module: &HirModule,
        call: &HirCallExpr,
        selected: &PreparedResolvedCallable,
        callee_inputs: &crate::callable::PreparedCallCalleeConstraintInputs,
        result: &TypeKind,
        callable_effects: &EffectRow,
    ) -> Result<AnalyzerPreparedCalleeExpression, FinalSemanticAnalysisError> {
        if selected.requires_value_callee() {
            let HirCallCallee::Value { value } = call.callee() else {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
            };
            let crate::callable::PreparedCallCalleeConstraintInputs::FunctionValue { actual } =
                callee_inputs
            else {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
            };
            let checked = self
                .facts
                .expressions()
                .get(value)
                .ok_or(FinalSemanticAnalysisError::CallResolutionFailed { owner })?;
            if checked.ty() != actual {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
            }
            return Ok(AnalyzerPreparedCalleeExpression::callable(*value));
        }
        if callee_inputs.is_function_value() {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
        }
        let (value, nominal_receiver) = match call.callee() {
            HirCallCallee::Value { value } => (*value, false),
            HirCallCallee::UnresolvedDot { value_receiver, .. }
                if !self.facts.expressions().contains_key(value_receiver) =>
            {
                (*value_receiver, true)
            }
            HirCallCallee::UnresolvedDot { .. } | HirCallCallee::Associated { .. } => {
                let retained = match (call.callee(), callee_inputs) {
                    (
                        HirCallCallee::UnresolvedDot { value_receiver, .. },
                        crate::callable::PreparedCallCalleeConstraintInputs::ValueReceiver {
                            source,
                            ..
                        },
                    ) if value_receiver == source => Some(*source),
                    (
                        HirCallCallee::UnresolvedDot { value_receiver, .. },
                        crate::callable::PreparedCallCalleeConstraintInputs::DialogueCallee,
                    ) => Some(*value_receiver),
                    _ => None,
                };
                return Ok(retained.map_or_else(
                    AnalyzerPreparedCalleeExpression::none,
                    AnalyzerPreparedCalleeExpression::semantic,
                ));
            }
        };
        if nominal_receiver {
            let ty = callee_inputs
                .nominal_callee_expression_type(selected.instantiation())
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner: value })?;
            let Some(ty) = ty else {
                return Ok(AnalyzerPreparedCalleeExpression::none());
            };
            self.facts
                .publish_new_expression(
                    value,
                    CheckedExpression::new(
                        ty.clone(),
                        CheckedTypeSelection::Inferred,
                        EffectSet::new(),
                        CheckedExpressionResolution::Structural,
                    ),
                )
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            return Ok(AnalyzerPreparedCalleeExpression::semantic(value));
        }
        let expression = module
            .resolve_expr(value)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let method_callee = match expression.kind() {
            HirExprKind::Select(select) => {
                let HirSelectedMember::Name(name) = select.member() else {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                };
                Some((select.target(), name.clone()))
            }
            _ => None,
        };
        let retained_resolution = if method_callee.is_some() {
            None
        } else if let Some(existing) = self.facts.expressions().get(&value) {
            match existing.checked_resolution() {
                Some(CheckedExpressionResolution::Value(
                    CheckedValueResolution::ProjectCallable(_)
                    | CheckedValueResolution::Registered(_),
                )) => existing.checked_resolution().cloned(),
                None if matches!(existing, super::PreparedExpressionFact::ProjectVariant(_)) => {
                    return Ok(AnalyzerPreparedCalleeExpression::semantic(value));
                }
                _ if matches!(
                    callee_inputs,
                    crate::callable::PreparedCallCalleeConstraintInputs::Free { .. }
                        | crate::callable::PreparedCallCalleeConstraintInputs::DialogueCallee
                ) =>
                {
                    return Ok(AnalyzerPreparedCalleeExpression::semantic(value));
                }
                _ => return Ok(AnalyzerPreparedCalleeExpression::none()),
            }
        } else {
            None
        };
        let ty = instantiated_callee_type(selected, result, callable_effects)
            .ok_or(FinalSemanticAnalysisError::CallResolutionFailed { owner: value })?;
        if let Some((receiver, _)) = &method_callee {
            match selected.instantiation() {
                CallableInstantiation::Receiver {
                    receiver: selected_receiver,
                } if self
                    .facts
                    .expressions()
                    .get(receiver)
                    .is_some_and(|checked| checked.ty() == selected_receiver) => {}
                _ => return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner: value }),
            }
        }
        let resolution = if method_callee.is_some() {
            None
        } else if let Some(resolution) = retained_resolution {
            Some(resolution)
        } else if let crate::callable::CallableCandidateId::Project(declaration) = selected.id() {
            let symbol = self
                .symbols
                .callable(declaration)
                .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
            Some(CheckedExpressionResolution::Value(
                CheckedValueResolution::ProjectCallable(super::CheckedProjectCallable::new(
                    declaration.clone(),
                    symbol.source_item(),
                )),
            ))
        } else {
            Some(CheckedExpressionResolution::Value(
                CheckedValueResolution::Registered(RegisteredSemanticValueId::from_bytes(
                    *selected.schema().semantic_digest().as_bytes(),
                )),
            ))
        };
        let effects = method_callee
            .as_ref()
            .and_then(|(receiver, _)| self.facts.expressions().get(receiver))
            .map_or_else(EffectSet::new, |receiver| receiver.effects().clone());
        let prepared = if let Some((_, name)) = method_callee {
            super::PreparedExpressionFact::Method(
                crate::final_analysis::PreparedMethodExpression::new(
                    super::PreparedExpressionShell::new(
                        ty,
                        CheckedTypeSelection::Inferred,
                        effects,
                    ),
                    name,
                ),
            )
        } else {
            CheckedExpression::new(
                ty,
                CheckedTypeSelection::Inferred,
                effects,
                resolution.expect("non-method callees always select a final resolution"),
            )
            .into()
        };
        if self.facts.expressions().contains_key(&value) {
            self.facts
                .replace_existing_expression(value, prepared)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        } else {
            self.facts
                .publish_new_expression(value, prepared)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        }
        Ok(AnalyzerPreparedCalleeExpression::callable(value))
    }
}

pub(super) fn instantiated_callee_type(
    selected: &PreparedResolvedCallable,
    _result: &TypeKind,
    effects: &EffectRow,
) -> Option<TypeKind> {
    selected
        .constraint_callable_type_with_invocation_effects(effects)
        .ok()
}

fn scope_is_dialogue_line_plan(module: &HirModule, mut scope: ScopeId) -> bool {
    loop {
        if module.expressions().any(|(_, expression)| {
            matches!(
                expression.kind(),
                HirExprKind::DialogueContentApplication(application)
                    if application.plan().is_some_and(|plan| plan.root_scope() == scope)
            )
        }) {
            return true;
        }
        let Ok(current) = module.resolve_scope(scope) else {
            return false;
        };
        let Some(parent) = current.parent() else {
            return false;
        };
        scope = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_owner() -> ExprId {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
        fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root module")
            .expressions()
            .next()
            .map(|(owner, _)| owner)
            .expect("expression owner")
    }

    #[test]
    fn frame_close_failure_overrides_rejected_result() {
        let owner = test_owner();
        let stack = super::super::expression_error::CallFrameStack::new(2).expect("frame stack");
        let outer = stack.enter(owner).expect("outer frame");
        let _inner = stack.enter(owner).expect("inner frame");
        let result: Result<(), AnalyzerExpressionError> =
            close_call_frame(owner, outer, Err(AnalyzerExpressionError::rejected(owner)));

        assert!(matches!(
            result,
            Err(AnalyzerExpressionError::Invariant(
                AnalyzerExpressionInvariant::CallFrame {
                    violation,
                    ..
                }
            )) if matches!(
                violation.as_ref(),
                super::super::expression_error::CallFrameInvariant::OutOfOrderClose { .. }
            )
        ));
    }

    #[test]
    fn inherited_noncanonical_failure_remains_a_terminal_invariant() {
        let owner = test_owner();
        let parameter = crate::types::GenericTypeParameterId::new(
            crate::types::GenericParameterOwnerId::Detached(
                crate::types::DetachedGenericOwnerId::new(0),
            ),
            0,
        );
        let error = terminal_lower_constraint_failure(
            owner,
            crate::types::constraints::TypeConstraintFailure::Invariant(
                crate::types::constraints::TypeConstraintFailureInvariant::Constraint(
                    crate::types::constraints::TypeConstraintInvariant::InheritedSolution(
                        crate::types::constraints::InheritedSolutionInvariant {
                            kind: crate::types::constraints::InheritedSolutionInvariantKind::NonCanonical,
                            parameter: Some(parameter.into()),
                        },
                    ),
                ),
            ),
        );

        assert!(matches!(
            error,
            AnalyzerExpressionError::Call {
                owner: found,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                    crate::callable::CallConstraintInvariant::Lower(
                        crate::types::constraints::TypeConstraintInvariant::InheritedSolution(
                            crate::types::constraints::InheritedSolutionInvariant {
                                kind:
                                    crate::types::constraints::InheritedSolutionInvariantKind::NonCanonical,
                                ..
                            },
                        ),
                    ),
                )),
            } if found == owner
        ));
    }

    #[test]
    fn non_callable_project_binding_never_executes_a_candidate_or_arguments() {
        let fixture = crate::final_analysis::tests::fixture(
            concat!(
                "pub signal @signal.payload Payload: Watch<i64>\n",
                "fn caller() { @signal.payload(1i64); }\n",
            ),
            None,
        );
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let (result, physical) = super::super::analyze_final_project_with_physical_trace_for_test(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            crate::final_analysis::FinalSemanticCatalogs::production(&fixture.registered),
            crate::final_analysis::FinalSemanticAnalysisControl::new(&cancellation),
        );

        assert!(
            matches!(
                &result,
                Err(
                    crate::final_analysis::FinalSemanticAnalysisError::ExpressionTypeUnavailable { .. }
                )
            ),
            "unexpected NonCallable analysis result: {result:?}"
        );
        assert!(physical.is_empty());
    }

    #[test]
    fn checked_character_items_are_exact_while_structural_character_refs_remain_any() {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root module");
        let item_owner = module
            .items()
            .next()
            .map(|(owner, _)| owner)
            .expect("item owner");
        let expression = module
            .expressions()
            .next()
            .map(|(owner, _)| owner)
            .expect("expression owner");

        assert!(
            crate::final_analysis::CheckedProjectItem::try_new_retained(
                arcweft_id::PublicId::try_new("view.not_a_character")
                    .expect("valid non-Character public ID"),
                arcweft_id::DeclarationIdentityFamily::Character,
                item_owner,
                None,
            )
            .is_none(),
            "a checked Character item without CharacterId must be unrepresentable"
        );

        let exact_character = arcweft_character::id::CharacterId::try_new("character.alice")
            .expect("Character identity");
        let exact_item = crate::final_analysis::CheckedProjectItem::try_new_retained(
            exact_character.as_public_id(),
            arcweft_id::DeclarationIdentityFamily::Character,
            item_owner,
            None,
        )
        .expect("checked Character item");
        let exact_checked =
            crate::final_analysis::PreparedExpressionFact::from(CheckedExpression::new(
                exact_item.ty(),
                CheckedTypeSelection::Inferred,
                EffectSet::new(),
                CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(exact_item)),
            ));
        assert!(matches!(
            checked_character_dialogue_target(expression, &exact_checked),
            Ok(Some(CheckedCharacterDialogueTarget::Character {
                character: CharacterDialogueCharacterType::Exact(found),
                ..
            })) if found == exact_character
        ));

        let structural_checked =
            crate::final_analysis::PreparedExpressionFact::from(CheckedExpression::new(
                TypeKind::entity_ref(crate::types::EntityKind::Character),
                CheckedTypeSelection::Inferred,
                EffectSet::new(),
                CheckedExpressionResolution::Structural,
            ));
        assert!(matches!(
            checked_character_dialogue_target(expression, &structural_checked),
            Ok(Some(CheckedCharacterDialogueTarget::Character {
                item: None,
                character: CharacterDialogueCharacterType::Any,
                ..
            }))
        ));
    }
}
