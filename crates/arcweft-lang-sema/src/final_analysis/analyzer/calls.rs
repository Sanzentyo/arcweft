//! Ordinary-call probing, accounting, and final semantic selection.

use std::{collections::BTreeSet, rc::Rc, sync::Arc};

#[path = "calls/constraints.rs"]
mod constraints;
#[path = "calls/semantics.rs"]
mod semantics;

pub(crate) use constraints::CallAnalysisFailure;
pub(in crate::final_analysis::analyzer) use constraints::{
    AnalyzerCallConstraintDomain, AnalyzerCallProjection,
};
pub(crate) use constraints::{
    AnalyzerCallConstraintSource, AnalyzerDetachedCandidateRecord,
    AnalyzerDetachedConsideredCandidate, AnalyzerDetachedUnselectedCall,
    AnalyzerDetachedUnselectedOutcome, AnalyzerPreparedCallGraph, AnalyzerPreparedCallPrefix,
    AnalyzerPreparedCalleeExpression, AnalyzerPreparedCandidateInventory,
    AnalyzerPreparedCandidateMetadata, AnalyzerPreparedCandidateRecord,
    AnalyzerPreparedDialoguePatchAdmission, AnalyzerPreparedExpressionResolution,
    AnalyzerPreparedUnselectedCall, AnalyzerPreparedUnselectedOutcome, CallAnalysisInvariant,
    PreparedCallApplicationTransaction, RanCandidateTransaction, SealedAcceptedCandidate,
    run_prepared_candidate, validate_and_prepare_call_constraints,
};

use semantics::select_prepared_candidates;
pub(super) use semantics::{
    checked_project_nominal, final_call_effects, final_callable_effect_row, final_callable_effects,
    nominal_substitutions,
};

use super::expression_types::value_resolution_type;
use super::expressions::AnalyzerExpressionExpectation;
use super::preparation::AssociatedReceiverTypeResolution;
use super::statements::{expression_span, source_span};
use super::{
    AcceptedCandidateRank, Analyzer, AnalyzerExpressionContext, BTreeMap,
    CallCalleeClassificationFact, CallResolverAuthority, CallResolverRequest,
    CallableDeclarationKey, CallableDeclarationOwner, CallableGroupIndex, CallableInstantiation,
    CandidateSelection, CharacterDialogueCharacterType, CharacterDialogueFieldCoordinate,
    CharacterDialoguePatchContext, CharacterOwnerSource, CheckedCallArgumentSlotSource,
    CheckedCharacterDialogueFactory, CheckedCharacterDialoguePatch,
    CheckedCharacterDialoguePatchField, CheckedCharacterDialogueReconfigure,
    CheckedCharacterDialogueTarget, CheckedExpression, CheckedExpressionResolution,
    CheckedPatchOperation, CheckedTypeSelection, CheckedValueResolution, EffectRow, EffectSet,
    ExprId, FinalCallCalleeFacts, FinalSemanticAnalysisError, HirAssociatedSeparator,
    HirCallArgument, HirCallArgumentSourcePart, HirCallCallee, HirCallInvocation, HirExprKind,
    HirExprSourceRole, HirModule, HirSelectedMember, HirSourcePresence, HirSourceQuery,
    HirSourceSite, PreparedResolvedCallable, RegisteredSemanticValueId, ResolveCallOutcome,
    ResolvedCallTarget, ResolvedCharacterOwner, ResolverWork, ScopeId, TypeKind,
    map_call_arguments, prepare_final_call_callee, prepare_language_free_dot_path,
    resolve_call_target,
};
use crate::callable::{
    CallableCandidateId, CallableResultSchema, DialogueCallableId, EnclosingGenericParameterScope,
    PreparedCallGraphIngress, PreparedCaptureIdentityRow, PreparedFunctionValueOriginEvidence,
    PreparedFunctionValueOriginProducer, PreparedFunctionValueOriginProgress,
    PreparedFunctionValueOriginQueryError,
    prepare_function_value_origin_query_with_pending_captures, prepare_presentation_callee_id,
};
use crate::final_analysis::PreparedProjectNominalTypeValueExpression;
use crate::final_analysis::type_rules::compact_numeric_element_type as infer_compact_numeric_element_type;
use crate::final_analysis::{CandidateEvaluationPass, CandidateFactTransactionViolation};

#[derive(Clone, Copy)]
struct CallSource<'a> {
    module: &'a HirModule,
    owner: ExprId,
    call: &'a HirCallInvocation,
    site: crate::callable::CheckedCallSite,
    expectation: &'a AnalyzerExpressionExpectation<'a>,
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

enum CallQueryResolution {
    Callable(Box<ResolvedCallQuery>),
    NonCallable,
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

pub(super) fn terminal_lower_constraint_failure(
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
    primary_result: CallableResultSchema,
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
        result: CallableResultSchema,
        evidence: PreparedCandidateRejection,
        branch: constraints::AnalyzerCallSealedBranch,
    },
}

enum PreparedChildCandidateRunOutcome {
    Deferred {
        candidate: Arc<PreparedResolvedCallable>,
        pending: crate::types::constraints::PendingChildConstraint<
            constraints::AnalyzerCallConstraintDomain,
        >,
        rank_seed: AcceptedCandidateRank,
        recipe: PreparedCallCandidateRecipe,
        descendants: Vec<PreparedCorrelatedCallRecipe>,
    },
    Rejected {
        candidate: Arc<PreparedResolvedCallable>,
        result: CallableResultSchema,
        evidence: PreparedCandidateRejection,
    },
}

enum PreparedCandidatePreparationOutcome {
    Accepted {
        transaction: RanCandidateTransaction,
        rank: AcceptedCandidateRank,
    },
    Deferred {
        candidate: Arc<PreparedResolvedCallable>,
        pending: crate::types::constraints::PendingChildConstraint<
            constraints::AnalyzerCallConstraintDomain,
        >,
        rank_seed: AcceptedCandidateRank,
        recipe: PreparedCallCandidateRecipe,
        descendants: Vec<PreparedCorrelatedCallRecipe>,
    },
    Rejected {
        candidate: Arc<PreparedResolvedCallable>,
        result: CallableResultSchema,
        evidence: PreparedCandidateRejection,
        branch: constraints::AnalyzerCallSealedBranch,
    },
}

enum PreparedCallCandidateExecution {
    Root(RanCandidateTransaction),
    Deferred {
        pending: crate::types::constraints::PendingChildConstraint<
            constraints::AnalyzerCallConstraintDomain,
        >,
        descendants: Vec<PreparedCorrelatedCallRecipe>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PreparedCallCandidateRecipe {
    candidate: Arc<PreparedResolvedCallable>,
    group: CallableGroupIndex,
    consumer: constraints::AnalyzerCallConsumerAdmission,
    callee_inputs: crate::callable::PreparedCallCalleeConstraintInputs,
    inputs: crate::callable::PreparedCallInputs,
    source_preparation: constraints::AnalyzerCallApplicationSources,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PreparedCorrelatedCallRecipe {
    owner: ExprId,
    site: crate::callable::CheckedCallSite,
    parent_application: Option<ExprId>,
    parent_source: constraints::AnalyzerCallConstraintSource,
    candidate: Arc<PreparedResolvedCallable>,
    group: CallableGroupIndex,
    rank_seed: AcceptedCandidateRank,
    consumer: constraints::AnalyzerCallConsumerAdmission,
    callee_inputs: crate::callable::PreparedCallCalleeConstraintInputs,
    inputs: crate::callable::PreparedCallInputs,
    source_preparation: constraints::AnalyzerCallApplicationSources,
    callee_prerequisites: Option<Arc<super::state::CandidateSemanticProjection>>,
    considered: Box<[Arc<PreparedResolvedCallable>]>,
    function_value_origin: Option<Arc<PreparedFunctionValueOriginEvidence>>,
    dialogue_context: CharacterDialoguePatchContext,
    accounting: crate::callable::CallResolverAccountingReport,
    attempt: PhysicalCallAttemptId,
    descendants: Vec<PreparedCorrelatedCallRecipe>,
}

impl PreparedCorrelatedCallRecipe {
    /// Replay compares the selected source and preparation, not the nonce of
    /// the physical evaluation that produced that preparation.
    fn semantic_replay_eq(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.site == other.site
            && self.parent_application == other.parent_application
            && self.parent_source == other.parent_source
            && self.candidate == other.candidate
            && self.group == other.group
            && self.rank_seed == other.rank_seed
            && self.consumer == other.consumer
            && self.callee_inputs == other.callee_inputs
            && self.inputs == other.inputs
            && self.source_preparation == other.source_preparation
            && match (&self.callee_prerequisites, &other.callee_prerequisites) {
                (Some(left), Some(right)) => left.semantic_replay_mismatch(right).is_none(),
                (None, None) => true,
                _ => false,
            }
            && self.considered == other.considered
            && self.function_value_origin == other.function_value_origin
            && self.dialogue_context == other.dialogue_context
            && self.accounting == other.accounting
            && self.descendants.len() == other.descendants.len()
            && self
                .descendants
                .iter()
                .zip(other.descendants.iter())
                .all(|(left, right)| left.semantic_replay_eq(right))
    }

    fn matches_choice(&self, choice: &constraints::AnalyzerNestedCallChoice) -> bool {
        self.owner == choice.application()
            && self.site == choice.site()
            && self.candidate.id() == choice.candidate()
            && self.candidate.schema().semantic_digest() == choice.schema()
            && self.group == choice.group()
            && self.rank_seed == choice.rank_seed()
    }

    fn find_choice<'a>(
        recipes: &'a [Self],
        parent_source: constraints::AnalyzerCallConstraintSource,
        choice: &constraints::AnalyzerNestedCallChoice,
    ) -> Option<&'a Self> {
        let mut matches = recipes.iter().filter(|recipe| {
            recipe.parent_source == parent_source && recipe.matches_choice(choice)
        });
        let first = matches.next()?;
        if matches.any(|recipe| !recipe.semantic_replay_eq(first)) {
            return None;
        }
        Some(first)
    }

    fn declared_exact_source(
        &self,
        source: constraints::AnalyzerCallConstraintSource,
        actual: &TypeKind,
    ) -> bool {
        let slot = match source {
            constraints::AnalyzerCallConstraintSource::Argument { slot, .. }
            | constraints::AnalyzerCallConstraintSource::DialoguePatch { slot, .. } => slot,
            _ => return false,
        };
        self.inputs
            .mapping()
            .arguments()
            .iter()
            .flat_map(|argument| argument.slots().iter())
            .find(|mapped| mapped.slot() == slot)
            .and_then(|mapped| mapped.declared_expected())
            == Some(actual)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PreparedSelectedNestedCall {
    recipe: PreparedCorrelatedCallRecipe,
    rank: AcceptedCandidateRank,
    selection: CheckedTypeSelection,
    specialization: Option<Arc<crate::callable::CheckedFunctionSpecialization>>,
}

impl PreparedSelectedNestedCall {
    fn semantic_replay_eq(&self, other: &Self) -> bool {
        self.recipe.semantic_replay_eq(&other.recipe)
            && self.rank == other.rank
            && self.selection == other.selection
            && self.specialization == other.specialization
    }
}

enum PreparedCandidateOutcome {
    Accepted {
        transaction: SealedAcceptedCandidate,
        rank: AcceptedCandidateRank,
    },
    Rejected {
        candidate: Arc<PreparedResolvedCallable>,
        result: CallableResultSchema,
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
        CallableResultSchema,
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
    site: crate::callable::CheckedCallSite,
    authored_arguments: &'a [HirCallArgument],
    explicit_type_application: Option<&'a arcweft_lang_hir::expr::HirCallTypeApplication>,
    dialogue_application_metadata:
        Option<&'a crate::callable::PreparedDialogueApplicationMetadataInventory>,
    semantic_operands: Box<[crate::callable::PreparedCallSemanticOperand]>,
    candidate: Arc<PreparedResolvedCallable>,
    current_group: CallableGroupIndex,
    expected_result: Option<&'a TypeKind>,
    expected_result_scope:
        Option<&'a crate::types::constraints::ImportedGenericParameterScopeLease>,
    callee_inputs: crate::callable::PreparedCallCalleeConstraintInputs,
    pass: CandidateEvaluationPass,
    attempt: Option<&'a PhysicalCallAttemptId>,
    context: &'ctx AnalyzerExpressionContext<'ctx>,
    dialogue_patch_admissions: &'a [AnalyzerPreparedDialoguePatchAdmission],
    compile_time_scalar_admissions: Box<[constraints::AnalyzerPreparedCompileTimeScalarAdmission]>,
}

enum PreparedAttachedContentAdmission {
    Rejected,
    Accepted(Option<crate::callable::PreparedCallAttachedContentOperand>),
}

fn prepare_attached_content_operand(
    module: &HirModule,
    owner: ExprId,
    site: crate::callable::CheckedCallSite,
    candidate: &PreparedResolvedCallable,
    current_group: CallableGroupIndex,
) -> Result<PreparedAttachedContentAdmission, crate::callable::CallConstraintInvariant> {
    let parameter = candidate
        .schema()
        .attached_content()
        .filter(|parameter| parameter.group() == current_group);
    let body = match site {
        crate::callable::CheckedCallSite::HirCall(expression) if expression == owner => None,
        crate::callable::CheckedCallSite::AttachedContentApplication {
            expression,
            family: crate::callable::CheckedAttachedContentApplicationFamily::DialogueLine,
        } if expression == owner => {
            if parameter.is_some() {
                return Err(crate::callable::CallConstraintInvariant::MalformedMapperSeal);
            }
            let expression = module
                .resolve_expr(owner)
                .map_err(|_| crate::callable::CallConstraintInvariant::MalformedMapperSeal)?;
            let HirExprKind::AttachedContentApplication(application) = expression.kind() else {
                return Err(crate::callable::CallConstraintInvariant::MalformedMapperSeal);
            };
            if !matches!(
                application.family(),
                arcweft_lang_hir::dialogue_application::HirAttachedContentApplicationFamily::DialogueLine { .. }
            ) || application.content().id().owner() != owner
            {
                return Err(crate::callable::CallConstraintInvariant::MalformedMapperSeal);
            }
            return Ok(PreparedAttachedContentAdmission::Accepted(None));
        }
        crate::callable::CheckedCallSite::AttachedContentApplication {
            expression,
            family: crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
        } if expression == owner => {
            let expression = module
                .resolve_expr(owner)
                .map_err(|_| crate::callable::CallConstraintInvariant::MalformedMapperSeal)?;
            let HirExprKind::AttachedContentApplication(application) = expression.kind() else {
                return Err(crate::callable::CallConstraintInvariant::MalformedMapperSeal);
            };
            if !matches!(
                application.family(),
                arcweft_lang_hir::dialogue_application::HirAttachedContentApplicationFamily::ContentCall { .. }
            ) || application.content().id().owner() != owner
            {
                return Err(crate::callable::CallConstraintInvariant::MalformedMapperSeal);
            }
            match application.body_presence() {
                arcweft_lang_hir::dialogue_application::HirAttachedContentBodyPresence::Absent => {
                    None
                }
                arcweft_lang_hir::dialogue_application::HirAttachedContentBodyPresence::Present => {
                    Some(application.content().id())
                }
            }
        }
        crate::callable::CheckedCallSite::HirCall(_)
        | crate::callable::CheckedCallSite::AttachedContentApplication { .. } => {
            return Err(crate::callable::CallConstraintInvariant::MalformedMapperSeal);
        }
    };
    match (parameter, body) {
        (None, None) => Ok(PreparedAttachedContentAdmission::Accepted(None)),
        (None, Some(_)) => Ok(PreparedAttachedContentAdmission::Rejected),
        (Some(_), Some(source)) => Ok(PreparedAttachedContentAdmission::Accepted(Some(
            crate::callable::PreparedCallAttachedContentOperand::present(source),
        ))),
        (Some(parameter), None)
            if parameter.presence() == crate::callable::CallableParameterPresence::Required =>
        {
            Ok(PreparedAttachedContentAdmission::Rejected)
        }
        (Some(_), None) => Ok(PreparedAttachedContentAdmission::Accepted(Some(
            crate::callable::PreparedCallAttachedContentOperand::Omitted,
        ))),
    }
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
    result: CallableResultSchema,
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
    let Some(checked_type) = checked.value_type() else {
        return Ok(None);
    };
    Ok(match checked_type {
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
        Ok(self
            .enclosing_callable(module, expression)?
            .filter(|declaration| declaration.owner() == CallableDeclarationOwner::Function))
    }

    fn enclosing_callable(
        &self,
        module: &HirModule,
        expression: ExprId,
    ) -> Result<Option<CallableDeclarationKey>, FinalSemanticAnalysisError> {
        if module.module_id() != expression.module() || module.resolve_expr(expression).is_err() {
            return Err(FinalSemanticAnalysisError::InvalidOwner);
        }
        let location = self
            .topology
            .semantic_path(
                arcweft_lang_hir::project::HirSemanticPathOwnerId::Expression(expression),
            )
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        match location.root() {
            arcweft_lang_hir::project::HirSemanticPathRoot::Declaration(declaration) => {
                Ok(Some(declaration.clone()))
            }
            arcweft_lang_hir::project::HirSemanticPathRoot::Item { .. } => Ok(None),
        }
    }

    pub(super) fn check_call_expression_in_context(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallInvocation,
        expectation: &AnalyzerExpressionExpectation<'_>,
        dialogue_application_metadata: Option<
            &crate::callable::PreparedDialogueApplicationMetadataInventory,
        >,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        self.check_call_expression_in_context_at_site(
            context,
            module,
            owner,
            call,
            expectation,
            dialogue_application_metadata,
            crate::callable::CheckedCallSite::HirCall(owner),
        )
    }

    pub(super) fn check_content_call_expression_in_context(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallInvocation,
        expectation: &AnalyzerExpressionExpectation<'_>,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        self.check_call_expression_in_context_at_site(
            context,
            module,
            owner,
            call,
            expectation,
            None,
            crate::callable::CheckedCallSite::AttachedContentApplication {
                expression: owner,
                family: crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
            },
        )
    }

    fn check_call_expression_in_context_at_site(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallInvocation,
        expectation: &AnalyzerExpressionExpectation<'_>,
        dialogue_application_metadata: Option<
            &crate::callable::PreparedDialogueApplicationMetadataInventory,
        >,
        site: crate::callable::CheckedCallSite,
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
            expectation,
            dialogue_application_metadata,
            site,
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
        call: &HirCallInvocation,
        expectation: &AnalyzerExpressionExpectation<'_>,
        dialogue_application_metadata: Option<
            &crate::callable::PreparedDialogueApplicationMetadataInventory,
        >,
        site: crate::callable::CheckedCallSite,
        attempt: &PhysicalCallAttemptId,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        let source = CallSource {
            module,
            owner,
            call,
            site,
            expectation,
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
        let staged_callee = match self.stage_call_callee_children(
            context,
            source.module,
            source.call,
            source.expectation.contextual_shape(),
            source.site,
        ) {
            Ok(recovery) => recovery,
            Err(error) => return Err(error),
        };
        if let Some(recovery) = staged_callee.recovery {
            return self.publish_associated_receiver_recovery(context, source, recovery, work);
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
            Ok(CallQueryResolution::Callable(resolution)) => *resolution,
            Ok(CallQueryResolution::NonCallable) => {
                return Ok(CheckedExpression::unavailable_call());
            }
            Err(error) => return Err(error),
        };
        resolution.dialogue_patch_admissions =
            self.prepare_character_dialogue_source_admission(source, &resolution)?;
        let probes = match self.prepare_resolved_candidates(context, source, &mut resolution) {
            Ok(probes) => probes,
            Err(error) => {
                return Err(error);
            }
        };
        match select_prepared_candidates(&probes.probes) {
            CandidateSelection::Selected(selected) => {
                self.publish_selected_call(context, source, resolution, probes, selected)
            }
            CandidateSelection::Ambiguous { primary, tied } => {
                self.publish_ambiguous_call(source, resolution, probes, primary, tied)
            }
            CandidateSelection::Rejected { primary } => {
                self.publish_rejected_call(source, resolution, probes, primary)
            }
        }
    }

    pub(super) fn probe_correlated_call_constraint_source(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        parent_application: Option<ExprId>,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallInvocation,
        expectation: &AnalyzerExpressionExpectation<'_>,
        parent_source: &mut crate::callable::CandidateConstraintSourceContext<
            '_,
            '_,
            constraints::AnalyzerCallConstraintDomain,
        >,
    ) -> Result<
        (
            Option<
                crate::types::constraints::PendingChildConstraint<
                    constraints::AnalyzerCallConstraintDomain,
                >,
            >,
            Vec<PreparedCorrelatedCallRecipe>,
        ),
        AnalyzerExpressionError,
    > {
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
        let result = self.probe_correlated_call_constraint_source_inner(
            context,
            parent_application,
            module,
            owner,
            call,
            expectation,
            parent_source,
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

    fn probe_correlated_call_constraint_source_inner(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        parent_application: Option<ExprId>,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallInvocation,
        expectation: &AnalyzerExpressionExpectation<'_>,
        parent_source: &mut crate::callable::CandidateConstraintSourceContext<
            '_,
            '_,
            constraints::AnalyzerCallConstraintDomain,
        >,
        attempt: &PhysicalCallAttemptId,
    ) -> Result<
        (
            Option<
                crate::types::constraints::PendingChildConstraint<
                    constraints::AnalyzerCallConstraintDomain,
                >,
            >,
            Vec<PreparedCorrelatedCallRecipe>,
        ),
        AnalyzerExpressionError,
    > {
        let outcome = self.run_candidate_fact_transaction::<_, CandidateFactOperationFailure>(
            |this, expression_authority, _transaction_authority| {
                let candidate_context = AnalyzerExpressionContext::candidate(
                    expression_authority,
                    Rc::clone(&this.call_frames),
                )
                .with_consumer(context.consumer());
                let prepared = this
                    .probe_correlated_call_constraint_source_prepared(
                        &candidate_context,
                        parent_application,
                        module,
                        owner,
                        call,
                        expectation,
                        parent_source,
                        attempt,
                    )
                    .map_err(CandidateFactOperationFailure::from)?;
                drop(candidate_context);
                if prepared.0.is_some() != !prepared.1.is_empty() {
                    return Err(CandidateFactOperationFailure::Expression(
                        AnalyzerExpressionError::Call {
                            owner,
                            failure: CallAnalysisFailure::Invariant(
                                CallAnalysisInvariant::Constraint(
                                    crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                                ),
                            ),
                        },
                    ));
                }
                if prepared.1.is_empty() {
                    Ok(CandidateFactTransactionAction::Rollback(prepared))
                } else {
                    Ok(CandidateFactTransactionAction::Extract(prepared))
                }
            },
        )?;
        match outcome {
            CandidateFactTransactionOutcome::Extracted {
                value: (pending, mut recipes),
                projection,
            } => {
                if recipes.is_empty() {
                    return Err(AnalyzerExpressionError::fact(
                        CandidateFactTransactionViolation::UnrecoverableLedger,
                    ));
                }
                let projection = Arc::new(projection);
                for recipe in &mut recipes {
                    recipe.callee_prerequisites = Some(Arc::clone(&projection));
                }
                Ok((pending, recipes))
            }
            CandidateFactTransactionOutcome::RolledBack(prepared) => Ok(prepared),
            CandidateFactTransactionOutcome::Committed(_) => Err(AnalyzerExpressionError::fact(
                CandidateFactTransactionViolation::UnrecoverableLedger,
            )),
        }
    }

    fn probe_correlated_call_constraint_source_prepared(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        parent_application: Option<ExprId>,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallInvocation,
        expectation: &AnalyzerExpressionExpectation<'_>,
        parent_source: &mut crate::callable::CandidateConstraintSourceContext<
            '_,
            '_,
            constraints::AnalyzerCallConstraintDomain,
        >,
        attempt: &PhysicalCallAttemptId,
    ) -> Result<
        (
            Option<
                crate::types::constraints::PendingChildConstraint<
                    constraints::AnalyzerCallConstraintDomain,
                >,
            >,
            Vec<PreparedCorrelatedCallRecipe>,
        ),
        AnalyzerExpressionError,
    > {
        let source = CallSource {
            module,
            owner,
            call,
            site: crate::callable::CheckedCallSite::HirCall(owner),
            expectation,
            dialogue_application_metadata: None,
            attempt,
        };
        let argument_count = u64::try_from(call.arguments().len()).map_err(|_| {
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
        let staged_callee = self.stage_call_callee_children(
            context,
            module,
            call,
            expectation.contextual_shape(),
            source.site,
        )?;
        if staged_callee.recovery.is_some() {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::CallResolutionFailed { owner },
            ));
        }
        let dialogue_context = CharacterDialoguePatchContext::ReusableValue;
        let mut resolution = match self.resolve_call_query(
            context,
            source,
            work,
            argument_count,
            dialogue_context,
            staged_callee.function_value_origin,
        )? {
            CallQueryResolution::Callable(resolution) => *resolution,
            CallQueryResolution::NonCallable => {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::CallResolutionFailed { owner },
                ));
            }
        };
        resolution.dialogue_patch_admissions =
            self.prepare_character_dialogue_source_admission(source, &resolution)?;
        self.prepare_correlated_child_candidates(
            context,
            parent_application,
            source,
            &mut resolution,
            parent_source,
        )
    }

    fn prepare_correlated_child_candidates(
        &mut self,
        source_context: &AnalyzerExpressionContext<'_>,
        parent_application: Option<ExprId>,
        source: CallSource<'_>,
        resolution: &mut ResolvedCallQuery,
        parent_source: &mut crate::callable::CandidateConstraintSourceContext<
            '_,
            '_,
            constraints::AnalyzerCallConstraintDomain,
        >,
    ) -> Result<
        (
            Option<
                crate::types::constraints::PendingChildConstraint<
                    constraints::AnalyzerCallConstraintDomain,
                >,
            >,
            Vec<PreparedCorrelatedCallRecipe>,
        ),
        AnalyzerExpressionError,
    > {
        let mut candidates = Vec::new();
        for candidate in &resolution.considered {
            self.control
                .check()
                .map_err(AnalyzerExpressionError::fatal)?;
            resolution
                .work
                .record_candidate_argument_probes(resolution.argument_count)
                .and_then(|_| {
                    resolution
                        .work
                        .charge_argument_mapping(resolution.argument_count)
                })
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
            // Declaration body prerequisites are independent of candidate
            // argument solving. Retain them with the callee transaction;
            // candidate probes deliberately discard their source facts.
            self.prepare_pending_result_projection(source.site, candidate)
                .map_err(AnalyzerExpressionError::fatal)?;
            let prepared = self
                .run_candidate_fact_transaction::<_, CandidateFactOperationFailure>(
                    |this, authority, _transaction_authority| {
                        let candidate_context = AnalyzerExpressionContext::candidate(
                            authority,
                            Rc::clone(&this.call_frames),
                        )
                        .with_consumer(source_context.consumer());
                        let result = this.prepare_child_candidate(
                            PreparedCandidateRequest {
                                module: source.module,
                                owner: source.owner,
                                site: source.site,
                                authored_arguments: source.call.arguments(),
                                explicit_type_application: Some(
                                    source.call.explicit_type_application(),
                                ),
                                dialogue_application_metadata: source.dialogue_application_metadata,
                                semantic_operands: Box::new([]),
                                candidate: Arc::clone(candidate),
                                current_group: candidate_group,
                                expected_result: source.expectation.nested_call_type(),
                                expected_result_scope: source.expectation.nested_call_scope_lease(),
                                callee_inputs: resolution.callee_inputs.clone(),
                                pass: CandidateEvaluationPass::Probe,
                                attempt: Some(source.attempt),
                                context: &candidate_context,
                                dialogue_patch_admissions: &resolution.dialogue_patch_admissions,
                                compile_time_scalar_admissions: Box::new([]),
                            },
                            &mut resolution.work,
                            parent_source,
                        );
                        drop(candidate_context);
                        result
                            .map(CandidateFactTransactionAction::Extract)
                            .map_err(CandidateFactOperationFailure::from)
                    },
                )?;
            let outcome = match prepared {
                CandidateFactTransactionOutcome::Extracted { value, projection } => {
                    self.facts
                        .discard_candidate_projection(projection)
                        .map_err(AnalyzerExpressionError::fact)?;
                    value
                }
                CandidateFactTransactionOutcome::Committed(_)
                | CandidateFactTransactionOutcome::RolledBack { .. } => {
                    return Err(AnalyzerExpressionError::fact(
                        crate::final_analysis::CandidateFactTransactionViolation::UnrecoverableLedger,
                    ));
                }
            };
            if let PreparedChildCandidateRunOutcome::Deferred {
                candidate: selected,
                pending: candidate_pending,
                rank_seed,
                recipe,
                descendants,
            } = outcome
            {
                if !Arc::ptr_eq(&recipe.candidate, &selected)
                    || recipe.candidate.id() != selected.id()
                    || recipe.candidate.schema().semantic_digest()
                        != selected.schema().semantic_digest()
                    || recipe.group != selected.call_group()
                    || recipe.inputs.candidate() != Some(selected.id())
                {
                    return Err(AnalyzerExpressionError::Call {
                        owner: source.owner,
                        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                        )),
                    });
                }
                candidates.push((selected, candidate_pending, rank_seed, recipe, descendants));
            }
        }
        let function_value_origin = resolution.function_value_origin.take().map(Arc::new);
        let accounting = resolution.work.call_accounting();
        let mut pending: Option<
            crate::types::constraints::PendingChildConstraint<
                constraints::AnalyzerCallConstraintDomain,
            >,
        > = None;
        let mut recipes = Vec::with_capacity(candidates.len());
        for (candidate, candidate_pending, rank_seed, recipe, descendants) in candidates {
            let choice = constraints::AnalyzerNestedCallChoice::new(
                source.owner,
                source.site,
                candidate.id().clone(),
                candidate.schema().semantic_digest(),
                recipe.group,
                rank_seed,
            );
            let candidate_pending =
                candidate_pending.with_probe_branch(constraints::AnalyzerCallProbeSemanticBranch {
                    source: parent_source.source().local(),
                    child_choice: Some(choice),
                });
            let recipe = PreparedCorrelatedCallRecipe {
                owner: source.owner,
                site: source.site,
                parent_application,
                parent_source: parent_source.source().local(),
                candidate,
                group: recipe.group,
                rank_seed,
                consumer: recipe.consumer,
                callee_inputs: recipe.callee_inputs,
                inputs: recipe.inputs,
                source_preparation: recipe.source_preparation,
                callee_prerequisites: None,
                considered: resolution.considered.clone().into_boxed_slice(),
                function_value_origin: function_value_origin.clone(),
                dialogue_context: resolution.dialogue_context,
                accounting: accounting.clone(),
                attempt: source.attempt.clone(),
                descendants,
            };
            if let Some(existing) = pending.as_mut() {
                existing.append(candidate_pending).map_err(|error| {
                    terminal_lower_constraint_failure(source.owner, error.into())
                })?;
            } else {
                pending = Some(candidate_pending);
            }
            recipes.push(recipe);
        }
        Ok((pending, recipes))
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
        if !matches!(
            selected.schema().value_type(),
            Some(value) if value == &TypeKind::CharacterDialogue(result)
        ) {
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

    fn is_environment_namespace(&self, resolution: &CheckedValueResolution) -> bool {
        let CheckedValueResolution::Registered(value) = resolution else {
            return false;
        };
        value.environment_binding().is_some_and(|binding| {
            self.catalogs
                .world
                .environment()
                .typecheck_env()
                .is_namespace_binding(binding)
        })
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
    ) -> Result<CallQueryResolution, AnalyzerExpressionError> {
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
        let implicit_extension_receiver = if let Some(pipe) = self
            .pipe_stack
            .last()
            .filter(|pipe| pipe.right == source.owner)
        {
            let topology = self.topology.module(pipe.owner.module()).ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
            })?;
            let region = topology
                .expression_uses()
                .pipe_left_region(pipe.owner)
                .map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                })?;
            let has_placeholders = region.has_placeholders().map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
            })?;
            (!has_placeholders).then(|| {
                crate::callable::PreparedImplicitExtensionReceiver::new(
                    pipe.left,
                    pipe.value.clone(),
                )
            })
        } else {
            None
        };
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
                let call_source = match source.module.source_site(
                    source.module.provenance().source_identity(),
                    HirSourceQuery::Expr {
                        owner: source.owner,
                        role: HirExprSourceRole::CallCallee,
                    },
                ) {
                    Ok(lookup) => {
                        let HirSourcePresence::Present(HirSourceSite::Span(span)) =
                            lookup.presence()
                        else {
                            return Err(AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::RecoveredOwner,
                            ));
                        };
                        span.clone()
                    }
                    Err(_) => expression_span(source.module, source.owner).map_err(|_| {
                        AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                    })?,
                };
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::UnknownCallTarget {
                        owner: source.owner,
                        kind: target.kind(),
                        name,
                        call_source,
                    },
                ));
            }
            ResolveCallOutcome::Resolved(ResolvedCallTarget::NonCallable(target)) => {
                self.publish_non_callable_call(context, source, callee, target, work)?;
                return Ok(CallQueryResolution::NonCallable);
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
        Ok(CallQueryResolution::Callable(Box::new(ResolvedCallQuery {
            callee,
            considered,
            callee_inputs,
            function_value_origin,
            current_group,
            work,
            argument_count,
            dialogue_context,
            dialogue_patch_admissions: Box::new([]),
        })))
    }

    fn publish_associated_receiver_recovery(
        &mut self,
        source_context: &AnalyzerExpressionContext<'_>,
        source: CallSource<'_>,
        recovery: AssociatedReceiverRecovery,
        mut work: ResolverWork,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        self.run_candidate_fact_transaction::<_, AnalyzerExpressionError>(
            |this, expression_authority, transaction_authority| {
                let context = source_context.child_candidate(expression_authority);
                let arguments =
                    this.stage_unselected_call_arguments(&context, source, &mut work)?;
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
                        arguments,
                        callee_expression,
                    );
                let enclosing_callable = this
                    .enclosing_ordinary_callable(source.module, source.owner)
                    .map_err(AnalyzerExpressionError::fatal)?;
                this.facts
                    .insert_unselected_call(
                        &transaction_authority,
                        source.site,
                        AnalyzerPreparedUnselectedCall {
                            enclosing_callable,
                            outcome: AnalyzerPreparedUnselectedOutcome::Missing {
                                callee: Some(callee),
                                kind: crate::callable::UnknownCallKind::AssociatedType,
                            },
                            accounting: work.call_accounting(),
                            selected_expression_inventory,
                        },
                    )
                    .map_err(AnalyzerExpressionError::fact)?;
                Ok(CandidateFactTransactionAction::Commit(
                    CheckedExpression::unavailable_call(),
                ))
            },
        )?
        .into_committed()
        .map_err(AnalyzerExpressionError::fact)
    }

    fn publish_non_callable_call(
        &mut self,
        source_context: &AnalyzerExpressionContext<'_>,
        source: CallSource<'_>,
        callee: CallCalleeClassificationFact,
        target: crate::callable::ResolvedNonCallableTarget,
        mut work: ResolverWork,
    ) -> Result<(), AnalyzerExpressionError> {
        let callee_expression = match callee {
            CallCalleeClassificationFact::Value { expression } => Some(expression),
            CallCalleeClassificationFact::AssociatedType { .. } => None,
        };
        let non_callable_source = target.source().clone();
        let non_callable_type = target.ty().clone();
        let outcome = self.run_candidate_fact_transaction::<_, AnalyzerExpressionError>(
            |this, expression_authority, transaction_authority| {
                let context = source_context.child_candidate(expression_authority);
                let arguments =
                    this.stage_unselected_call_arguments(&context, source, &mut work)?;
                let enclosing_callable = this
                    .enclosing_ordinary_callable(source.module, source.owner)
                    .map_err(AnalyzerExpressionError::fatal)?;
                this.facts
                    .insert_unselected_call(
                        &transaction_authority,
                        source.site,
                        AnalyzerPreparedUnselectedCall {
                            enclosing_callable,
                            outcome: AnalyzerPreparedUnselectedOutcome::NonCallable {
                                callee: Some(callee),
                                source: non_callable_source,
                                ty: non_callable_type,
                            },
                            accounting: work.call_accounting(),
                            selected_expression_inventory:
                                arcweft_lang_hir::project::HirSelectedCallExpressionInventory::new(
                                    arguments,
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

    /// A target with no candidates still owns its authored argument sources.
    /// Check them once inside the same fact transaction, without inventing a
    /// callable schema or suppressing an argument's fatal source failure.
    fn stage_unselected_call_arguments(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        source: CallSource<'_>,
        work: &mut ResolverWork,
    ) -> Result<Box<[ExprId]>, AnalyzerExpressionError> {
        let mut arguments = Vec::with_capacity(source.call.arguments().len());
        for argument in source.call.arguments() {
            let owner = argument.value();
            self.evaluate_expression(context, owner, None)?;
            arguments.push(owner);
        }
        let count = u64::try_from(arguments.len()).map_err(|_| {
            AnalyzerExpressionError::Abort(
                crate::types::constraints::TypeConstraintAbort::ArithmeticOverflow,
            )
        })?;
        work.record_retained_argument_fact_publications(count)
            .map_err(|_| {
                AnalyzerExpressionError::Abort(
                    crate::types::constraints::TypeConstraintAbort::ArithmeticOverflow,
                )
            })?;
        Ok(arguments.into_boxed_slice())
    }

    fn stage_associated_receiver_recovery_expression(
        &mut self,
        module: &HirModule,
        call: &HirCallInvocation,
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
                    CheckedExpression::value(
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
        source_context: &AnalyzerExpressionContext<'_>,
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
                    )
                    .with_consumer(source_context.consumer());
                    let probe = this.prepare_candidate(
                        PreparedCandidateRequest {
                            module: source.module,
                            owner: source.owner,
                            site: source.site,
                            authored_arguments: source.call.arguments(),
                            explicit_type_application: Some(
                                source.call.explicit_type_application(),
                            ),
                            dialogue_application_metadata: source.dialogue_application_metadata,
                            semantic_operands: Box::new([]),
                            candidate: Arc::clone(candidate),
                            current_group: candidate_group,
                            expected_result: source.expectation.nested_call_type(),
                            expected_result_scope: source.expectation.nested_call_scope_lease(),
                            callee_inputs: resolution.callee_inputs.clone(),
                            pass: CandidateEvaluationPass::Probe,
                            attempt: Some(source.attempt),
                            context: &context,
                            dialogue_patch_admissions: &resolution.dialogue_patch_admissions,
                            compile_time_scalar_admissions: Box::new([]),
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
                    transaction: SealedAcceptedCandidate::seal(transaction, projection),
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
                    projection,
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
        context: &AnalyzerExpressionContext<'_>,
        source: CallSource<'_>,
        resolution: ResolvedCallQuery,
        batch: PreparedCandidateBatch,
        selected_index: usize,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        let singleton = resolution.considered.len() == 1;
        let consumer = context.consumer();
        let outcome = self.run_candidate_fact_transaction::<_, CandidateFactOperationFailure>(
            |this, _expression_authority, transaction_authority| {
                this.publish_selected_call_in_transaction(
                    source,
                    resolution,
                    batch,
                    selected_index,
                    singleton,
                    consumer,
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
        let structural_inputs = crate::callable::PreparedCallInputs::dialogue_application(
            &candidate,
            owner,
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
                        site: crate::callable::CheckedCallSite::AttachedContentApplication {
                            expression: owner,
                            family: crate::callable::CheckedAttachedContentApplicationFamily::DialogueLine,
                        },
                        authored_arguments: &[],
                        explicit_type_application: None,
                        dialogue_application_metadata: None,
                        semantic_operands: structural_inputs.semantic_operands().to_vec().into_boxed_slice(),
                        candidate: Arc::clone(&candidate),
                        current_group: candidate.call_group(),
                        expected_result: expected,
                        expected_result_scope: None,
                        callee_inputs:
                            crate::callable::PreparedCallCalleeConstraintInputs::DialogueApplication,
                        pass: CandidateEvaluationPass::Probe,
                        attempt: None,
                        context: &candidate_context,
                        dialogue_patch_admissions: &[],
                        compile_time_scalar_admissions: Box::new([]),
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
            } => (ran, rank, projection),
            CandidateFactTransactionOutcome::Committed(_)
            | CandidateFactTransactionOutcome::RolledBack(_) => {
                return Err(AnalyzerExpressionError::fact(
                    CandidateFactTransactionViolation::UnrecoverableLedger,
                ));
            }
        };
        let transaction = ran
            .into_prepared_application(
                self.checked_callable_effect_authority_with_projection(&outer_projection)
                    .map_err(AnalyzerExpressionError::fatal)?,
            )
            .map_err(|error| AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
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
            AnalyzerPreparedExpressionResolution::DialogueApplication,
            AnalyzerPreparedCalleeExpression::none(),
            self.enclosing_ordinary_callable(module, owner)
                .map_err(AnalyzerExpressionError::fatal)?,
            inventory,
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
                    crate::callable::CheckedCallSite::AttachedContentApplication {
                        expression: owner,
                        family:
                            crate::callable::CheckedAttachedContentApplicationFamily::DialogueLine,
                    },
                )?;
                let CallableResultSchema::Value(result) = result else {
                    return Err(CandidateFactOperationFailure::Expression(
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        ),
                    ));
                };
                Ok(CandidateFactTransactionAction::Commit(result))
            },
        )?
        .into_committed()
        .map_err(AnalyzerExpressionError::fact)
    }

    /// Publishes one exact text-proxy Object invocation through the ordinary
    /// candidate transaction. The caller has already resolved the unique
    /// nominal `type` discriminator; the remaining authored arguments stay in
    /// the normal mapper and are evaluated by its source callback exactly
    /// once.
    pub(super) fn publish_text_proxy_object_application(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        invocation: &HirCallInvocation,
        definition: &crate::checked_text_proxy::PreparedCheckedTextProxyDefinition,
        type_argument: (
            arcweft_lang_hir::expr::HirCallArgumentOrdinal,
            ExprId,
            TypeKind,
            PreparedProjectNominalTypeValueExpression,
            crate::callable::CallableParameterCoordinate,
        ),
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
        let result = self.publish_text_proxy_object_application_inner(
            context,
            module,
            owner,
            invocation,
            definition,
            type_argument,
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

    fn prepare_object_compile_time_scalar_admissions(
        &self,
        definition: &crate::checked_text_proxy::PreparedCheckedTextProxyDefinition,
        schema: &crate::callable::CallableSignatureSchema,
    ) -> Result<
        Box<[constraints::AnalyzerPreparedCompileTimeScalarAdmission]>,
        AnalyzerExpressionError,
    > {
        let mut rows = Vec::new();
        for group in schema.groups() {
            for parameter in group.parameters() {
                let kind = match parameter.consumer() {
                    crate::callable::CallableParameterConsumer::Content(
                        crate::callable::CallableContentParameterConsumer::ObjectType,
                    ) => continue,
                    crate::callable::CallableParameterConsumer::Content(
                        crate::callable::CallableContentParameterConsumer::ObjectId
                        | crate::callable::CallableContentParameterConsumer::ObjectRole
                        | crate::callable::CallableContentParameterConsumer::ObjectLayer,
                    ) => crate::checked_compile_time::CheckedCompileTimeScalarKind::PublicId,
                    crate::callable::CallableParameterConsumer::Content(
                        crate::callable::CallableContentParameterConsumer::ObjectDepth,
                    ) => crate::checked_compile_time::CheckedCompileTimeScalarKind::Length,
                    crate::callable::CallableParameterConsumer::Content(
                        crate::callable::CallableContentParameterConsumer::ObjectHitTest,
                    ) => crate::checked_compile_time::CheckedCompileTimeScalarKind::Bool,
                    crate::callable::CallableParameterConsumer::Content(
                        crate::callable::CallableContentParameterConsumer::ObjectCustomField(field),
                    ) => definition
                        .checked()
                        .fields()
                        .iter()
                        .find(|candidate| candidate.semantic_id() == *field)
                        .map(|field| field.kind().clone())
                        .ok_or_else(|| {
                            AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::WrongPayloadFamily,
                            )
                        })?,
                    _ => {
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        ));
                    }
                };
                let crate::callable::CallableParameterAdmission::Semantic(
                    crate::callable::CallableSemanticAdmission::CompileTimeScalar(admission),
                ) = parameter.admission()
                else {
                    return Err(AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::WrongPayloadFamily,
                    ));
                };
                if admission.kind() != kind.callable_kind()
                    || admission.value_type() != &self.compile_time_scalar_type(&kind)
                {
                    return Err(AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::WrongPayloadFamily,
                    ));
                }
                let prepared =
                    crate::checked_compile_time::PreparedCompileTimeScalarAdmission::try_new(
                        kind.clone(),
                        admission.value_type().clone(),
                        self.compile_time_scalar_source_mode(&kind).ok_or_else(|| {
                            AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::WrongPayloadFamily,
                            )
                        })?,
                    )
                    .ok_or_else(|| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        )
                    })?;
                rows.push(
                    constraints::AnalyzerPreparedCompileTimeScalarAdmission::new(
                        crate::callable::CallableParameterCoordinate::new(
                            group.index(),
                            parameter.index(),
                        ),
                        prepared,
                    ),
                );
            }
        }
        Ok(rows.into_boxed_slice())
    }

    fn publish_text_proxy_object_application_inner(
        &mut self,
        _context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        invocation: &HirCallInvocation,
        definition: &crate::checked_text_proxy::PreparedCheckedTextProxyDefinition,
        (type_argument, type_expression, type_actual, type_value, type_coordinate): (
            arcweft_lang_hir::expr::HirCallArgumentOrdinal,
            ExprId,
            TypeKind,
            PreparedProjectNominalTypeValueExpression,
            crate::callable::CallableParameterCoordinate,
        ),
        attempt: &PhysicalCallAttemptId,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        let schema = definition
            .callable_schema(self.catalogs.world().environment().compile_time_scalars())
            .map_err(|_| AnalyzerExpressionError::rejected(owner))?;
        let identity = definition
            .callable_identity()
            .map_err(|_| AnalyzerExpressionError::rejected(owner))?;
        let compile_time_scalar_admissions =
            self.prepare_object_compile_time_scalar_admissions(definition, &schema)?;
        let candidate = Arc::new(
            PreparedResolvedCallable::try_from_intrinsic(
                CallableCandidateId::Content(identity),
                crate::callable::SignatureOrigin::Language {
                    family: crate::callable::LanguageCallableFamily::Content,
                },
                Arc::new(schema),
                CallableInstantiation::None,
                Vec::new(),
                &self.catalogs.callable_limits,
            )
            .map_err(|_| AnalyzerExpressionError::rejected(owner))?,
        );
        if invocation.arguments().is_empty() {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        let callee_expression = invocation.callee().value_expression().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
        })?;
        let static_content_callee = crate::callable::PreparedStaticContentCallee::new(
            callee_expression,
            identity,
            candidate.schema().semantic_digest(),
        );
        let semantic_operand = crate::callable::PreparedCallSemanticOperand::text_proxy_object(
            type_expression,
            type_argument,
            type_coordinate,
            type_actual,
        );
        let mut work = ResolverWork::new(self.catalogs.callable_limits.max_query_work());
        let argument_count = u64::try_from(invocation.arguments().len()).map_err(|_| {
            AnalyzerExpressionError::Abort(
                crate::types::constraints::TypeConstraintAbort::ArithmeticOverflow,
            )
        })?;
        work.record_candidate_argument_probes(argument_count)
            .and_then(|_| work.charge_argument_mapping(argument_count))
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                    owner,
                })
            })?;
        let candidate_probe = self
            .run_candidate_fact_transaction::<_, CandidateFactOperationFailure>(
                |this, authority, _transaction_authority| {
                    let candidate_context = AnalyzerExpressionContext::candidate(
                        authority,
                        Rc::clone(&this.call_frames),
                    );
                    this.facts
                        .publish_new_expression(
                            type_expression,
                            crate::final_analysis::PreparedExpressionFact::from(
                                type_value.clone(),
                            ),
                        )
                        .map_err(|_| {
                            AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::WrongPayloadFamily,
                            )
                        })?;
                    let outcome = this.prepare_candidate(
                    PreparedCandidateRequest {
                        module,
                        owner,
                        site: crate::callable::CheckedCallSite::AttachedContentApplication {
                            expression: owner,
                            family: crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
                        },
                        authored_arguments: invocation.arguments(),
                        explicit_type_application:
                            Some(invocation.explicit_type_application()),
                        dialogue_application_metadata: None,
                        semantic_operands: Box::new([semantic_operand.clone()]),
                        candidate: Arc::clone(&candidate),
                        current_group: candidate.call_group(),
                        expected_result: None,
                        expected_result_scope: None,
                        callee_inputs: crate::callable::PreparedCallCalleeConstraintInputs::
                            StaticContentCallee(static_content_callee),
                        pass: CandidateEvaluationPass::Probe,
                        attempt: Some(attempt),
                        context: &candidate_context,
                        dialogue_patch_admissions: &[],
                        compile_time_scalar_admissions: compile_time_scalar_admissions.clone(),
                    },
                    &mut work,
                );
                    drop(candidate_context);
                    match outcome? {
                        PreparedCandidateRunOutcome::Accepted { transaction, rank } => {
                            Ok(CandidateFactTransactionAction::Extract((transaction, rank)))
                        }
                        PreparedCandidateRunOutcome::Rejected { .. } => {
                            Err(AnalyzerExpressionError::rejected(owner).into())
                        }
                    }
                },
            )?;
        let (ran, _rank, outer_projection) = match candidate_probe {
            CandidateFactTransactionOutcome::Extracted {
                value: (ran, rank),
                projection,
            } => (ran, rank, projection),
            CandidateFactTransactionOutcome::RolledBack { .. }
            | CandidateFactTransactionOutcome::Committed(_) => {
                return Err(AnalyzerExpressionError::rejected(owner));
            }
        };
        let transaction = ran
            .into_prepared_application(
                self.checked_callable_effect_authority_with_projection(&outer_projection)
                    .map_err(AnalyzerExpressionError::fatal)?,
            )
            .map_err(|error| AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
            })?;
        let inventory = AnalyzerPreparedCandidateInventory::from_considered(
            transaction.candidate(),
            vec![Arc::clone(&candidate)],
        )
        .map_err(|error| AnalyzerExpressionError::Call {
            owner,
            failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
        })?;
        let metadata = AnalyzerPreparedCandidateMetadata::new(
            owner,
            AnalyzerPreparedExpressionResolution::ContentApplication,
            AnalyzerPreparedCalleeExpression::none(),
            self.enclosing_ordinary_callable(module, owner)
                .map_err(AnalyzerExpressionError::fatal)?,
            inventory,
            None,
            work.call_accounting(),
        );
        let result = self
            .run_candidate_fact_transaction::<_, CandidateFactOperationFailure>(
                |this, _authority, transaction_authority| {
                    let result = this.apply_sealed_candidate(
                        transaction,
                        outer_projection,
                        metadata,
                        &transaction_authority,
                        crate::callable::CheckedCallSite::AttachedContentApplication {
                            expression: owner,
                            family:
                                crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
                        },
                    )?;
                    if !matches!(
                        result,
                        CallableResultSchema::ContentEmission(
                            crate::callable::ContentCallableIdentity::TextProxyObject { .. },
                        )
                    )
                    {
                        return Err(CandidateFactOperationFailure::Expression(
                            AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::WrongPayloadFamily,
                            ),
                        ));
                    }
                    Ok(CandidateFactTransactionAction::Commit(result))
                },
            )?
            .into_committed()
            .map_err(AnalyzerExpressionError::fact)?;
        let CallableResultSchema::ContentEmission(callable) = result else {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::WrongPayloadFamily,
            ));
        };
        Ok(CheckedExpression::content_emission(
            callable,
            EffectSet::new(),
            CheckedExpressionResolution::Call,
        ))
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
        consumer: super::expression_error::AnalyzerExpressionConsumer,
        transaction_authority: &CandidateFactTransactionAuthority<'_>,
    ) -> Result<CheckedExpression, CandidateFactOperationFailure> {
        let (selected_transaction, selected_rank) = batch
            .probes
            .remove(selected_index)
            .into_accepted(source.owner)?;
        let (selected_ran, selected_outer_projection) = selected_transaction.into_parts();
        let selected_transaction = selected_ran
            .into_prepared_application(
                self.checked_callable_effect_authority_with_projection(&selected_outer_projection)
                    .map_err(AnalyzerExpressionError::fatal)
                    .map_err(CandidateFactOperationFailure::Expression)?,
            )
            .map_err(|error| {
                CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                    owner: source.owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        error,
                    )),
                })
            })?;
        let selected = selected_transaction.selected_shared().clone();
        let current_group = selected_transaction.current_group();
        let (selected_transaction, outer_projection) = if singleton {
            (selected_transaction, selected_outer_projection)
        } else {
            if resolution
                .work
                .record_selected_replay_argument_visits(resolution.argument_count)
                .is_err()
            {
                self.discard_prepared_call_transaction_projection(
                    source.owner,
                    selected_transaction,
                    selected_outer_projection,
                )
                .map_err(CandidateFactOperationFailure::Expression)?;
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::CallResolutionFailed {
                        owner: source.owner,
                    },
                )
                .into());
            }
            let replay = self.run_candidate_fact_transaction::<_, CandidateFactOperationFailure>(
                |this, authority, _transaction_authority| {
                    let context = AnalyzerExpressionContext::candidate(
                        authority,
                        Rc::clone(&this.call_frames),
                    )
                    .with_consumer(consumer);
                    let replay = this.prepare_candidate(
                        PreparedCandidateRequest {
                            module: source.module,
                            owner: source.owner,
                            site: source.site,
                            authored_arguments: source.call.arguments(),
                            explicit_type_application: Some(
                                source.call.explicit_type_application(),
                            ),
                            dialogue_application_metadata: source.dialogue_application_metadata,
                            semantic_operands: Box::new([]),
                            candidate: Arc::clone(&selected),
                            current_group,
                            expected_result: source.expectation.nested_call_type(),
                            expected_result_scope: source.expectation.nested_call_scope_lease(),
                            callee_inputs: resolution.callee_inputs.clone(),
                            pass: CandidateEvaluationPass::SelectedReplay,
                            attempt: Some(source.attempt),
                            context: &context,
                            dialogue_patch_admissions: &resolution.dialogue_patch_admissions,
                            compile_time_scalar_admissions: Box::new([]),
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
            );
            let replay = match replay {
                Ok(replay) => replay,
                Err(failure) => {
                    self.discard_prepared_call_transaction_projection(
                        source.owner,
                        selected_transaction,
                        selected_outer_projection,
                    )
                    .map_err(CandidateFactOperationFailure::Expression)?;
                    return Err(failure.into());
                }
            };
            let (replay_ran, replay_outer_projection, replay_rank) = match replay {
                CandidateFactTransactionOutcome::Extracted {
                    value: PreparedCandidateRunOutcome::Accepted { transaction, rank },
                    projection,
                } => (transaction, projection, rank),
                CandidateFactTransactionOutcome::RolledBack(
                    PreparedCandidateRunOutcome::Rejected { .. },
                ) => {
                    self.discard_prepared_call_transaction_projection(
                        source.owner,
                        selected_transaction,
                        selected_outer_projection,
                    )
                    .map_err(CandidateFactOperationFailure::Expression)?;
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
                    self.discard_prepared_call_transaction_projection(
                        source.owner,
                        selected_transaction,
                        selected_outer_projection,
                    )
                    .map_err(CandidateFactOperationFailure::Expression)?;
                    return Err(AnalyzerExpressionError::Call {
                        owner: source.owner,
                        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            crate::callable::CallConstraintInvariant::ReplayTransactionShapeMismatch,
                        )),
                    }
                    .into());
                }
            };
            let replay_transaction = replay_ran
                .into_prepared_application(
                    self.checked_callable_effect_authority_with_projection(
                        &replay_outer_projection,
                    )
                    .map_err(AnalyzerExpressionError::fatal)
                    .map_err(CandidateFactOperationFailure::Expression)?,
                )
                .map_err(|error| {
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
                self.discard_prepared_call_transaction_projection(
                    source.owner,
                    selected_transaction,
                    selected_outer_projection,
                )
                .map_err(CandidateFactOperationFailure::Expression)?;
                self.discard_prepared_call_transaction_projection(
                    source.owner,
                    replay_transaction,
                    replay_outer_projection,
                )
                .map_err(CandidateFactOperationFailure::Expression)?;
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
                _selected_consumer,
                _selected_callee_inputs,
                _selected_inputs,
                selected_branch,
                _selected_component,
            ) = selected_transaction.into_parts();
            self.facts
                .discard_candidate_projection(selected_outer_projection)
                .map_err(|violation| {
                    CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fact(
                        violation,
                    ))
                })?;
            self.discard_nested_call_callee_prerequisites_from_branch(
                source.owner,
                selected_branch,
            )
            .map_err(CandidateFactOperationFailure::Expression)?;
            (replay_transaction, replay_outer_projection)
        };
        let specialization = selected_transaction
            .specialization_result()
            .map(|result| {
                self.seal_function_result_use(source.owner, &result, &mut resolution.work)
            })
            .transpose()?;
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

        let direct_effects = match self.source_call_intrinsic_effects(&selected, current_group) {
            Ok(effects) => effects,
            Err(error) => {
                return Err(AnalyzerExpressionError::fatal(error).into());
            }
        };
        let callee_expression = match &result {
            CallableResultSchema::Value(value) => {
                let callable_effects = self
                    .source_callable_effects(&selected, Some(&outer_projection))
                    .map_err(AnalyzerExpressionError::fatal)?;
                self.stage_resolved_callee_expression(
                    source.owner,
                    source.site,
                    source.module,
                    source.call,
                    &selected,
                    &resolution.callee_inputs,
                    value,
                    callable_effects.as_ref(),
                )
                .map_err(AnalyzerExpressionError::fatal)?
            }
            CallableResultSchema::ContentEmission(_) => {
                if source.expectation.complete_type().is_some() {
                    return Err(CandidateFactOperationFailure::Expression(
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        ),
                    ));
                }
                AnalyzerPreparedCalleeExpression::none()
            }
        };
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
        let prepared_expression_resolution = match source.site.attached_content_family() {
            Some(crate::callable::CheckedAttachedContentApplicationFamily::ContentCall) => {
                AnalyzerPreparedExpressionResolution::ContentApplication
            }
            Some(crate::callable::CheckedAttachedContentApplicationFamily::DialogueLine) | None => {
                AnalyzerPreparedExpressionResolution::Complete(expression_resolution.clone())
            }
        };
        let metadata = AnalyzerPreparedCandidateMetadata::new(
            source.owner,
            prepared_expression_resolution,
            callee_expression,
            self.enclosing_ordinary_callable(source.module, source.owner)
                .map_err(AnalyzerExpressionError::fatal)?,
            inventory,
            resolution.function_value_origin.take(),
            resolution.work.call_accounting(),
        );
        let result = self.apply_sealed_candidate(
            selected_transaction,
            outer_projection,
            metadata,
            transaction_authority,
            source.site,
        )?;
        // The provisional expression stores intrinsic effects. Selected calls
        // retain their declared rows or project-body edges until the existing
        // callable closure and final execution-effect publication complete them.
        match result {
            CallableResultSchema::Value(result) => {
                let checked = CheckedExpression::value(
                    result,
                    if source.expectation.complete_type().is_some() {
                        CheckedTypeSelection::Expected
                    } else {
                        CheckedTypeSelection::Inferred
                    },
                    direct_effects,
                    expression_resolution,
                );
                match specialization {
                    Some(witness) => checked
                        .with_function_specialization(source.owner, witness)
                        .map_err(|error| {
                            super::function_value_use::specialization_invariant(source.owner, error)
                                .into()
                        }),
                    None => Ok(checked),
                }
            }
            CallableResultSchema::ContentEmission(callable) => {
                if source.expectation.complete_type().is_some() {
                    return Err(CandidateFactOperationFailure::Expression(
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        ),
                    ));
                }
                Ok(CheckedExpression::content_emission(
                    callable,
                    direct_effects,
                    expression_resolution,
                ))
            }
        }
    }

    fn apply_sealed_candidate(
        &mut self,
        transaction: PreparedCallApplicationTransaction,
        outer_projection: super::state::CandidateSemanticProjection,
        metadata: AnalyzerPreparedCandidateMetadata,
        transaction_authority: &CandidateFactTransactionAuthority<'_>,
        site: crate::callable::CheckedCallSite,
    ) -> Result<CallableResultSchema, CandidateFactOperationFailure> {
        let owner = match site {
            crate::callable::CheckedCallSite::HirCall(owner)
            | crate::callable::CheckedCallSite::AttachedContentApplication {
                expression: owner,
                ..
            } => owner,
        };
        let result = match transaction.result() {
            Ok(result) => result,
            Err(error) => {
                let (_, _, _, _, branch, _) = transaction.into_parts();
                self.discard_nested_call_callee_prerequisites_from_branch(owner, branch)
                    .map_err(CandidateFactOperationFailure::Expression)?;
                self.facts
                    .discard_candidate_projection(outer_projection)
                    .map_err(|violation| {
                        CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fact(
                            violation,
                        ))
                    })?;
                return Err(CandidateFactOperationFailure::Expression(
                    AnalyzerExpressionError::Call {
                        owner,
                        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            error,
                        )),
                    },
                ));
            }
        };
        let (application, consumer, callee_inputs, inputs, sealed_branch, component) =
            transaction.into_parts();
        let (materialized_projection, mut nested_calls) = match sealed_branch {
            constraints::AnalyzerCallSealedBranch::Empty => (None, Vec::new()),
            constraints::AnalyzerCallSealedBranch::Materialized {
                projection,
                nested_calls,
            } => (Some(projection), nested_calls.into_vec()),
        };
        let shared_component = component.shared_component();
        if let Err(failure) = self
            .facts
            .apply_candidate_projection(transaction_authority, outer_projection)
        {
            self.discard_nested_call_callee_prerequisites(
                owner,
                std::mem::take(&mut nested_calls).into_boxed_slice(),
            )
            .map_err(CandidateFactOperationFailure::Expression)?;
            return Err(CandidateFactOperationFailure::Projection(Box::new(failure)));
        }
        if let Some(projection) = materialized_projection {
            if let Err(failure) = self
                .facts
                .apply_candidate_projection(transaction_authority, projection)
            {
                self.discard_nested_call_callee_prerequisites(
                    owner,
                    std::mem::take(&mut nested_calls).into_boxed_slice(),
                )
                .map_err(CandidateFactOperationFailure::Expression)?;
                return Err(CandidateFactOperationFailure::Projection(Box::new(failure)));
            }
        }
        let mut nested_calls = nested_calls.into_iter();
        while let Some(nested) = nested_calls.next() {
            if let Err(failure) = self.stage_completed_nested_call(
                nested,
                Arc::clone(&shared_component),
                transaction_authority,
            ) {
                self.discard_nested_call_callee_prerequisites(
                    owner,
                    nested_calls.collect::<Vec<_>>().into_boxed_slice(),
                )
                .map_err(CandidateFactOperationFailure::Expression)?;
                return Err(failure);
            }
        }
        let record = AnalyzerPreparedCandidateRecord::seal(
            metadata,
            application.selected(),
            consumer,
            callee_inputs,
            inputs,
            component,
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

    fn stage_completed_nested_call(
        &mut self,
        selected: PreparedSelectedNestedCall,
        component: Arc<
            crate::types::constraints::CompletedConstraintComponent<
                constraints::AnalyzerCallConstraintDomain,
            >,
        >,
        transaction_authority: &CandidateFactTransactionAuthority<'_>,
    ) -> Result<(), CandidateFactOperationFailure> {
        let PreparedSelectedNestedCall {
            mut recipe,
            selection,
            specialization,
            ..
        } = selected;
        let owner = recipe.owner;
        let callee_prerequisites = recipe.callee_prerequisites.take().ok_or_else(|| {
            CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                    crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                )),
            })
        })?;
        let callee_prerequisites = Arc::try_unwrap(callee_prerequisites).map_err(|_| {
            CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                    crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                )),
            })
        })?;
        self.facts
            .apply_candidate_projection(transaction_authority, callee_prerequisites)
            .map_err(|failure| CandidateFactOperationFailure::Projection(Box::new(failure)))?;
        let module = self.module(owner.module()).map_err(|error| {
            CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fatal(error))
        })?;
        let expression = module.resolve_expr(owner).map_err(|_| {
            CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::InvalidOwner,
            ))
        })?;
        let HirExprKind::Call(call) = expression.kind() else {
            return Err(CandidateFactOperationFailure::Expression(
                AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::PreparedCallSiteMismatch,
                    )),
                },
            ));
        };
        if recipe.site != crate::callable::CheckedCallSite::HirCall(owner) {
            return Err(CandidateFactOperationFailure::Expression(
                AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::PreparedCallSiteMismatch,
                    )),
                },
            ));
        }
        let transaction = PreparedCallApplicationTransaction::from_completed_nested_call(
            &recipe,
            Arc::clone(&component),
            self.checked_callable_effect_authority()
                .map_err(AnalyzerExpressionError::fatal)
                .map_err(CandidateFactOperationFailure::Expression)?,
        )
        .map_err(|error| {
            CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
            })
        })?;
        let result = match transaction.result() {
            Ok(result) => result,
            Err(error) => {
                let (_, _, _, _, branch, _) = transaction.into_parts();
                self.discard_nested_call_callee_prerequisites_from_branch(owner, branch)
                    .map_err(CandidateFactOperationFailure::Expression)?;
                return Err(CandidateFactOperationFailure::Expression(
                    AnalyzerExpressionError::Call {
                        owner,
                        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            error,
                        )),
                    },
                ));
            }
        };
        let selected_callable = transaction.selected_shared().clone();
        let current_group = transaction.current_group();
        let direct_effects = self
            .source_call_intrinsic_effects(&selected_callable, current_group)
            .map_err(|error| {
                CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fatal(error))
            })?;
        if let crate::callable::PreparedCallCalleeConstraintInputs::ValueReceiver { source, actual } =
            &recipe.callee_inputs
            && !self
                .facts
                .expressions()
                .get(source)
                .is_some_and(|checked| checked.value_type() == Some(actual))
        {
            return Err(CandidateFactOperationFailure::Expression(
                AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::PreparedBaseMismatch,
                    )),
                },
            ));
        }
        let callee_expression = match &result {
            CallableResultSchema::Value(value) => {
                let callable_effects = self
                    .source_callable_effects(&selected_callable, None)
                    .map_err(|error| {
                        CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fatal(
                            error,
                        ))
                    })?;
                self.stage_resolved_callee_expression(
                    owner,
                    recipe.site,
                    module,
                    call,
                    &selected_callable,
                    &recipe.callee_inputs,
                    value,
                    callable_effects.as_ref(),
                )
                .map_err(|error| {
                    CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fatal(error))
                })?
            }
            CallableResultSchema::ContentEmission(_) => {
                return Err(CandidateFactOperationFailure::Expression(
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily),
                ));
            }
        };
        let expectation = AnalyzerExpressionExpectation::Unconstrained;
        let source = CallSource {
            module,
            owner,
            call,
            site: recipe.site,
            expectation: &expectation,
            dialogue_application_metadata: None,
            attempt: &recipe.attempt,
        };
        let expression_resolution = self
            .checked_character_dialogue_resolution(
                source,
                &selected_callable,
                recipe.dialogue_context,
                &transaction,
            )
            .map_err(|failure| match failure {
                CharacterDialogueResolutionFailure::Semantic(error) => {
                    CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fatal(error))
                }
                CharacterDialogueResolutionFailure::Constraint(error) => {
                    CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                        owner,
                        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            error,
                        )),
                    })
                }
            })?
            .unwrap_or(CheckedExpressionResolution::Call);
        let inventory = AnalyzerPreparedCandidateInventory::from_considered(
            transaction.candidate(),
            recipe.considered.to_vec(),
        )
        .map_err(|error| {
            CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
            })
        })?;
        let function_value_origin = recipe
            .function_value_origin
            .map(Arc::try_unwrap)
            .transpose()
            .map_err(|_| {
                CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                    )),
                })
            })?;
        let metadata = AnalyzerPreparedCandidateMetadata::new(
            owner,
            AnalyzerPreparedExpressionResolution::Complete(expression_resolution.clone()),
            callee_expression,
            self.enclosing_ordinary_callable(module, owner)
                .map_err(|error| {
                    CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fatal(error))
                })?,
            inventory,
            function_value_origin,
            recipe.accounting,
        );
        let (application, consumer, callee_inputs, inputs, sealed_branch, component_evidence) =
            transaction.into_parts();
        if !matches!(sealed_branch, constraints::AnalyzerCallSealedBranch::Empty)
            || component_evidence.application() != owner
        {
            return Err(CandidateFactOperationFailure::Expression(
                AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::PreparedCallSiteMismatch,
                    )),
                },
            ));
        }
        let record = AnalyzerPreparedCandidateRecord::seal(
            metadata,
            application.selected(),
            consumer,
            callee_inputs,
            inputs,
            component_evidence,
        )
        .map_err(|error| {
            CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                owner,
                failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
            })
        })?;
        let prefix = constraints::AnalyzerPreparedCallPrefix::new(recipe.site, application, record)
            .map_err(|error| {
                CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        error,
                    )),
                })
            })?;
        let (sealed_result, _) = self
            .facts
            .seal_selected_application(transaction_authority, recipe.site, prefix)
            .map_err(|violation| {
                CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fact(violation))
            })?;
        if sealed_result != result {
            return Err(CandidateFactOperationFailure::Expression(
                AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::PreparedFunctionTypeMismatch,
                    )),
                },
            ));
        }
        let CallableResultSchema::Value(value) = result else {
            return Err(CandidateFactOperationFailure::Expression(
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily),
            ));
        };
        let checked =
            CheckedExpression::value(value, selection, direct_effects, expression_resolution);
        let checked = match specialization {
            Some(specialization) => checked
                .with_function_specialization(owner, specialization)
                .map_err(|error| {
                    CandidateFactOperationFailure::Expression(AnalyzerExpressionError::Call {
                        owner,
                        failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            error,
                        )),
                    })
                })?,
            None => checked,
        };
        let checked = self
            .attach_nested_path_evidence(owner, checked.into())
            .map_err(CandidateFactOperationFailure::Expression)?;
        self.record_implicit_capture_fact(owner, &checked)
            .map_err(|error| {
                CandidateFactOperationFailure::Expression(AnalyzerExpressionError::fatal(error))
            })?;
        let write = if self.facts.expressions().contains_key(&owner) {
            self.facts.replace_existing_expression(owner, checked)
        } else {
            self.facts.publish_new_expression(owner, checked)
        };
        write.map_err(|error| {
            CandidateFactOperationFailure::Expression(match error {
                super::state::ExpressionFactWriteViolation::AlreadyPublished => {
                    AnalyzerExpressionError::invariant(FinalSemanticAnalysisError::DuplicateFact {
                        family: super::SemanticFactFamily::Expression,
                    })
                }
                super::state::ExpressionFactWriteViolation::MissingPublishedFact => {
                    AnalyzerExpressionError::invariant(
                        FinalSemanticAnalysisError::WrongPayloadFamily,
                    )
                }
                super::state::ExpressionFactWriteViolation::Candidate(violation) => {
                    AnalyzerExpressionError::fact(violation)
                }
            })
        })?;
        Ok(())
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
                    source.owner,
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
                    source.owner,
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
        owner: ExprId,
        primary: PreparedCandidateSemanticProjection,
        discarded: Vec<PreparedCandidateSemanticProjection>,
    ) -> Result<(), AnalyzerExpressionError> {
        for projection in discarded {
            self.discard_recovery_projection(owner, projection)?;
        }
        let PreparedCandidateSemanticProjection { outer, branch } = primary;
        let branch_projection = match branch {
            constraints::AnalyzerCallSealedBranch::Empty => None,
            constraints::AnalyzerCallSealedBranch::Materialized {
                projection,
                nested_calls,
            } => {
                self.discard_nested_call_callee_prerequisites(owner, nested_calls)?;
                Some(projection)
            }
        };
        self.facts
            .apply_candidate_projection(transaction_authority, outer)
            .map_err(|failure| {
                let (violation, _projection) = failure.into_parts();
                AnalyzerExpressionError::fact(violation)
            })?;
        if let Some(projection) = branch_projection {
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
        owner: ExprId,
        projection: PreparedCandidateSemanticProjection,
    ) -> Result<(), AnalyzerExpressionError> {
        let PreparedCandidateSemanticProjection { outer, branch } = projection;
        self.discard_nested_call_callee_prerequisites_from_branch(owner, branch)?;
        self.facts
            .discard_candidate_projection(outer)
            .map_err(AnalyzerExpressionError::fact)?;
        Ok(())
    }

    fn discard_nested_call_callee_prerequisites(
        &self,
        owner: ExprId,
        nested_calls: Box<[PreparedSelectedNestedCall]>,
    ) -> Result<(), AnalyzerExpressionError> {
        fn collect(
            owner: ExprId,
            recipe: &mut PreparedCorrelatedCallRecipe,
            projections: &mut Vec<Arc<super::state::CandidateSemanticProjection>>,
        ) -> Result<(), AnalyzerExpressionError> {
            let projection = recipe.callee_prerequisites.take().ok_or_else(|| {
                AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                    )),
                }
            })?;
            if !projections
                .iter()
                .any(|existing| Arc::ptr_eq(existing, &projection))
            {
                projections.push(projection);
            }
            for descendant in &mut recipe.descendants {
                collect(owner, descendant, projections)?;
            }
            recipe.descendants.clear();
            Ok(())
        }

        let mut projections = Vec::new();
        for mut selected in nested_calls.into_vec() {
            collect(owner, &mut selected.recipe, &mut projections)?;
        }
        for shared in projections {
            let projection =
                Arc::try_unwrap(shared).map_err(|_| AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                    )),
                })?;
            self.facts
                .discard_candidate_projection(projection)
                .map_err(AnalyzerExpressionError::fact)?;
        }
        Ok(())
    }

    fn discard_nested_call_callee_prerequisites_from_branch(
        &self,
        owner: ExprId,
        branch: constraints::AnalyzerCallSealedBranch,
    ) -> Result<(), AnalyzerExpressionError> {
        if let constraints::AnalyzerCallSealedBranch::Materialized {
            projection,
            nested_calls,
        } = branch
        {
            self.discard_nested_call_callee_prerequisites(owner, nested_calls)?;
            self.facts
                .discard_candidate_projection(projection)
                .map_err(AnalyzerExpressionError::fact)?;
        }
        Ok(())
    }

    fn discard_prepared_call_transaction_projection(
        &self,
        owner: ExprId,
        transaction: PreparedCallApplicationTransaction,
        outer: super::state::CandidateSemanticProjection,
    ) -> Result<(), AnalyzerExpressionError> {
        let (_, _, _, _, branch, _) = transaction.into_parts();
        self.facts
            .discard_candidate_projection(outer)
            .map_err(AnalyzerExpressionError::fact)?;
        self.discard_nested_call_callee_prerequisites_from_branch(owner, branch)
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
        let callee_expression = match &result {
            CallableResultSchema::Value(value) => {
                let callable_effects = self.source_callable_effects(primary, None)?;
                self.stage_resolved_callee_expression(
                    source.owner,
                    source.site,
                    source.module,
                    source.call,
                    primary,
                    &callee_inputs,
                    value,
                    callable_effects.as_ref(),
                )?
            }
            CallableResultSchema::ContentEmission(_) => AnalyzerPreparedCalleeExpression::none(),
        };
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
                source.site,
                AnalyzerPreparedUnselectedCall {
                    enclosing_callable,
                    outcome,
                    accounting: work.call_accounting(),
                    selected_expression_inventory,
                },
            )
            .map_err(FinalSemanticAnalysisError::from)?;
        Ok(CheckedExpression::unavailable_call())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "callee child staging exhaustively follows the final-HIR callee variants and their typed owners"
    )]
    fn stage_call_callee_children(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        call: &HirCallInvocation,
        expected: Option<&TypeKind>,
        site: crate::callable::CheckedCallSite,
    ) -> Result<StagedCallCalleeChildren, AnalyzerExpressionError> {
        let owns_callee_expression_fact = site.owns_callee_expression_fact();
        let mut function_value_origin = None;
        match call.callee() {
            HirCallCallee::Value { value } => {
                let expression = module.resolve_expr(*value).map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                })?;
                if let HirExprKind::Select(select) = expression.kind() {
                    self.evaluate_expression(context, select.target(), None)?;
                } else if !matches!(expression.kind(), HirExprKind::Path(_)) {
                    if matches!(expression.kind(), HirExprKind::ShortVariant(_))
                        && let Some(expected) = expected
                    {
                        let (_, template) = crate::callable::CallableGenericParameterIssuer::for_enum_constructor_type(expected, self.symbols)
                            .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed { owner: *value }))?;
                        self.evaluate_expression_with_expectation(
                            context,
                            *value,
                            super::expressions::AnalyzerExpressionExpectation::enum_constructor_head(
                                &template,
                            ),
                        )?;
                    } else {
                        self.evaluate_expression(context, *value, None)?;
                    }
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
                        if owns_callee_expression_fact
                            && let Some(ty) = ty
                            && !self.facts.expressions().contains_key(value)
                        {
                            self.facts
                                .publish_new_expression(
                                    *value,
                                    CheckedExpression::value(
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
                    && matches!(checked.value_type(), Some(TypeKind::Function { .. }))
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
                            if !self.is_environment_namespace(&resolution)
                                && owns_callee_expression_fact
                                && let Some(ty) = self
                                    .staged_value_resolution_type(&resolution, *value_receiver)
                                    .map_err(AnalyzerExpressionError::fatal)?
                                && !self.facts.expressions().contains_key(value_receiver)
                            {
                                self.facts
                                    .publish_new_expression(
                                        *value_receiver,
                                        CheckedExpression::value(
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
                                    if !self.is_environment_namespace(&resolution)
                                        && owns_callee_expression_fact
                                        && let Some(ty) = self
                                            .staged_value_resolution_type(
                                                &resolution,
                                                *value_receiver,
                                            )
                                            .map_err(AnalyzerExpressionError::fatal)?
                                        && !self.facts.expressions().contains_key(value_receiver)
                                    {
                                        self.facts
                                            .publish_new_expression(
                                                *value_receiver,
                                                CheckedExpression::value(
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
                                    if let Some(receiver_fact) = self
                                        .prepare_direct_project_field_path_receiver(
                                            module,
                                            *value_receiver,
                                            expression.scope(),
                                            path,
                                        )?
                                    {
                                        if owns_callee_expression_fact
                                            && !self
                                                .facts
                                                .expressions()
                                                .contains_key(value_receiver)
                                        {
                                            self.facts
                                                .publish_new_expression(
                                                    *value_receiver,
                                                    receiver_fact,
                                                )
                                                .map_err(|_| {
                                                    AnalyzerExpressionError::fatal(
                                                        FinalSemanticAnalysisError::WrongPayloadFamily,
                                                    )
                                                })?;
                                        }
                                    } else {
                                        if self.path_has_local_prefix(
                                            module,
                                            *value_receiver,
                                            expression.scope(),
                                            path,
                                        )? {
                                            let call_owner = site.expression();
                                            let call_source =
                                                expression_span(module, call_owner)
                                                    .map_err(AnalyzerExpressionError::fatal)?;
                                            return Err(AnalyzerExpressionError::fatal(
                                                FinalSemanticAnalysisError::UnknownCallTarget {
                                                    owner: call_owner,
                                                    kind: crate::callable::UnknownCallKind::Method,
                                                    name: member.as_str().to_owned(),
                                                    call_source,
                                                },
                                            ));
                                        }
                                        let line_context = path.lexical_name() == Some("line")
                                            && scope_is_dialogue_line_plan(
                                                module,
                                                expression.scope(),
                                            );
                                        if line_context && owns_callee_expression_fact {
                                            self.facts
                                                .publish_new_expression(
                                                    *value_receiver,
                                                    CheckedExpression::value(
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
                                        .map_err(|_| {
                                            FinalSemanticAnalysisError::CallResolutionFailed {
                                                owner: *value_receiver,
                                            }
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
                                            AssociatedReceiverTypeResolution::UnresolvedNominal => {
                                            }
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
                    AssociatedReceiverTypeResolution::UnresolvedNominal => {
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::TypeResolutionFailed { owner: receiver },
                        ));
                    }
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
        let pending_capture_rows = self.pending_capture_identity_rows()?;
        let mut progress = prepare_function_value_origin_query_with_pending_captures(
            Arc::clone(&self.topology),
            module,
            expression,
            self.facts.expressions(),
            pending_capture_rows,
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

    pub(super) fn pending_capture_identity_rows(
        &self,
    ) -> Result<Arc<BTreeMap<ExprId, Box<[PreparedCaptureIdentityRow]>>>, AnalyzerExpressionError>
    {
        let mut by_owner = BTreeMap::<
            ExprId,
            Vec<(
                u32,
                arcweft_lang_hir::identity::LocalId,
                arcweft_lang_hir::scope::CaptureAccess,
            )>,
        >::new();
        for (owner, fact) in self.facts.expressions() {
            if matches!(
                fact,
                super::PreparedExpressionFact::OwnerBound(prepared)
                    if matches!(
                        prepared.resolution(),
                        super::PreparedOwnerBoundResolution::ImplicitCallable(_)
                    )
            ) {
                by_owner.entry(*owner).or_default();
            }
        }
        for ((owner, expression), local) in self.facts.pending_implicit_capture_uses() {
            let module = self.topology.module(owner.module()).ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
            })?;
            let row = module.expression_uses().row(*expression).ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
            })?;
            by_owner.entry(*owner).or_default().push((
                row.source_ordinal(),
                *local,
                row.capture_access(),
            ));
        }
        let mut rows = BTreeMap::new();
        for (owner, mut uses) in by_owner {
            uses.sort_by_key(|(source_ordinal, _, _)| *source_ordinal);
            let mut captures = Vec::<(
                arcweft_lang_hir::identity::LocalId,
                arcweft_lang_hir::scope::CaptureAccess,
            )>::new();
            let mut indexes = BTreeMap::<arcweft_lang_hir::identity::LocalId, usize>::new();
            for (_, local, access) in uses {
                if let Some(index) = indexes.get(&local).copied() {
                    if access == arcweft_lang_hir::scope::CaptureAccess::Reassign {
                        captures[index].1 = access;
                    }
                } else {
                    indexes.insert(local, captures.len());
                    captures.push((local, access));
                }
            }
            rows.insert(
                owner,
                captures
                    .into_iter()
                    .map(|(local, access)| PreparedCaptureIdentityRow::new(local, access))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
        }
        Ok(Arc::new(rows))
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
        let owner = request.owner;
        match self.prepare_candidate_impl(request, work, None)? {
            PreparedCandidatePreparationOutcome::Accepted { transaction, rank } => {
                Ok(PreparedCandidateRunOutcome::Accepted { transaction, rank })
            }
            PreparedCandidatePreparationOutcome::Rejected {
                candidate,
                result,
                evidence,
                branch,
            } => Ok(PreparedCandidateRunOutcome::Rejected {
                candidate,
                result,
                evidence,
                branch,
            }),
            PreparedCandidatePreparationOutcome::Deferred { .. } => {
                Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::CallResolutionFailed { owner },
                ))
            }
        }
    }

    pub(super) fn enclosing_constraint_scope(
        &self,
        module: &HirModule,
        owner: ExprId,
        imported: Option<&crate::types::constraints::ImportedGenericParameterScopeLease>,
    ) -> Result<EnclosingGenericParameterScope, AnalyzerExpressionError> {
        let enclosing_declaration = self
            .enclosing_callable(module, owner)
            .map_err(AnalyzerExpressionError::fatal)?;
        let enclosing_inventory = enclosing_declaration
            .as_ref()
            .map(|declaration| {
                self.catalogs
                    .world
                    .environment()
                    .callable_catalog()
                    .project_record(declaration)
                    .map(|record| record.schema().generic_inventory())
                    .ok_or_else(|| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::CheckedCallableCatalog,
                        )
                    })
            })
            .transpose()?;
        let enclosing_types = enclosing_inventory
            .into_iter()
            .flat_map(|inventory| inventory.types().iter())
            .map(|entry| {
                entry
                    .parameter()
                    .free_parameter()
                    .cloned()
                    .map(crate::types::GenericTypeReference::Free)
                    .ok_or_else(|| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::CallResolutionFailed { owner },
                        )
                    })
            })
            .collect::<Result<BTreeSet<crate::types::GenericTypeReference>, _>>()?;
        let enclosing_consts = enclosing_inventory
            .into_iter()
            .flat_map(|inventory| inventory.consts().iter())
            .map(|entry| {
                entry
                    .parameter()
                    .free_parameter()
                    .cloned()
                    .map(crate::types::GenericConstReference::Free)
                    .ok_or_else(|| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::CallResolutionFailed { owner },
                        )
                    })
            })
            .collect::<Result<BTreeSet<crate::types::GenericConstReference>, _>>()?;
        let enclosing_effects = enclosing_inventory
            .into_iter()
            .flat_map(|inventory| inventory.effects().iter())
            .map(|entry| {
                entry
                    .parameter()
                    .free_parameter()
                    .cloned()
                    .map(crate::types::GenericEffectReference::Free)
                    .ok_or_else(|| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::CallResolutionFailed { owner },
                        )
                    })
            })
            .collect::<Result<BTreeSet<crate::types::GenericEffectReference>, _>>()?;
        EnclosingGenericParameterScope::sealed_with_imported_scope(
            enclosing_types,
            enclosing_consts,
            enclosing_effects,
            imported,
        )
        .map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                owner,
            })
        })
    }

    fn prepare_child_candidate(
        &mut self,
        request: PreparedCandidateRequest<'_, '_>,
        work: &mut ResolverWork,
        parent_source: &mut crate::callable::CandidateConstraintSourceContext<
            '_,
            '_,
            constraints::AnalyzerCallConstraintDomain,
        >,
    ) -> Result<PreparedChildCandidateRunOutcome, AnalyzerExpressionError> {
        let owner = request.owner;
        match self.prepare_candidate_impl(request, work, Some(parent_source))? {
            PreparedCandidatePreparationOutcome::Deferred {
                candidate,
                pending,
                rank_seed,
                recipe,
                descendants,
            } => Ok(PreparedChildCandidateRunOutcome::Deferred {
                candidate,
                pending,
                rank_seed,
                recipe,
                descendants,
            }),
            PreparedCandidatePreparationOutcome::Rejected {
                candidate,
                result,
                evidence,
                ..
            } => Ok(PreparedChildCandidateRunOutcome::Rejected {
                candidate,
                result,
                evidence,
            }),
            PreparedCandidatePreparationOutcome::Accepted { .. } => {
                Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::CallResolutionFailed { owner },
                ))
            }
        }
    }

    fn prepare_candidate_impl(
        &mut self,
        request: PreparedCandidateRequest<'_, '_>,
        work: &mut ResolverWork,
        mut parent_source: Option<
            &mut crate::callable::CandidateConstraintSourceContext<
                '_,
                '_,
                constraints::AnalyzerCallConstraintDomain,
            >,
        >,
    ) -> Result<PreparedCandidatePreparationOutcome, AnalyzerExpressionError> {
        let PreparedCandidateRequest {
            module,
            owner,
            site,
            authored_arguments,
            explicit_type_application,
            dialogue_application_metadata,
            semantic_operands,
            candidate,
            current_group,
            expected_result,
            expected_result_scope,
            callee_inputs,
            pass,
            attempt,
            context,
            dialogue_patch_admissions,
            compile_time_scalar_admissions,
        } = request;
        self.prepare_pending_result_projection(site, &candidate)
            .map_err(AnalyzerExpressionError::fatal)?;
        let implicit = match candidate.instantiation() {
            CallableInstantiation::Extension {
                group, parameter, ..
            } if *group == current_group => Some(*parameter),
            _ => None,
        };
        let attached_content = match prepare_attached_content_operand(
            module,
            owner,
            site,
            candidate.as_ref(),
            current_group,
        )
        .map_err(|failure| AnalyzerExpressionError::Call {
            owner,
            failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(failure)),
        })? {
            PreparedAttachedContentAdmission::Accepted(operand) => operand,
            PreparedAttachedContentAdmission::Rejected => {
                let result = self
                    .source_result_schema_for_group(owner, &candidate, current_group)
                    .map_err(AnalyzerExpressionError::fatal)?;
                return Ok(PreparedCandidatePreparationOutcome::Rejected {
                    candidate: Arc::clone(&candidate),
                    result,
                    evidence: PreparedCandidateRejection::Mapping(
                        PreparedCallMappingRejection::from_authored(authored_arguments),
                    ),
                    branch: constraints::AnalyzerCallSealedBranch::Empty,
                });
            }
        };
        let mapping = if semantic_operands.is_empty() {
            let Some(mapping) = map_call_arguments(
                module,
                candidate.schema(),
                candidate.id(),
                current_group,
                authored_arguments,
                implicit,
            ) else {
                let result = self
                    .source_result_schema_for_group(owner, &candidate, current_group)
                    .map_err(AnalyzerExpressionError::fatal)?;
                return Ok(PreparedCandidatePreparationOutcome::Rejected {
                    candidate: Arc::clone(&candidate),
                    result,
                    evidence: PreparedCandidateRejection::Mapping(
                        PreparedCallMappingRejection::from_authored(authored_arguments),
                    ),
                    branch: constraints::AnalyzerCallSealedBranch::Empty,
                });
            };
            mapping
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
                })?
        } else if matches!(
            callee_inputs,
            crate::callable::PreparedCallCalleeConstraintInputs::StaticContentCallee(_)
        ) {
            if dialogue_application_metadata.is_some() || implicit.is_some() {
                return Err(AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                    )),
                });
            }
            let Some(mapping) = map_call_arguments(
                module,
                candidate.schema(),
                candidate.id(),
                current_group,
                authored_arguments,
                None,
            ) else {
                let result = self
                    .source_result_schema_for_group(owner, &candidate, current_group)
                    .map_err(AnalyzerExpressionError::fatal)?;
                return Ok(PreparedCandidatePreparationOutcome::Rejected {
                    candidate: Arc::clone(&candidate),
                    result,
                    evidence: PreparedCandidateRejection::Mapping(
                        PreparedCallMappingRejection::from_authored(authored_arguments),
                    ),
                    branch: constraints::AnalyzerCallSealedBranch::Empty,
                });
            };
            mapping
        } else {
            if !authored_arguments.is_empty()
                || dialogue_application_metadata.is_some()
                || implicit.is_some()
            {
                return Err(AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                    )),
                });
            }
            let omitted_parameters = usize::from(!semantic_operands.iter().any(|operand| {
                operand.role() == crate::callable::PreparedCallSemanticOperandRole::DialogueLinePlan
            }));
            crate::callable::PreparedCallArgumentMapping::empty(
                candidate.id().clone(),
                candidate.schema().semantic_digest(),
                current_group,
                omitted_parameters,
            )
        };
        let type_application = match explicit_type_application
            .filter(|application| application.spelling().is_some())
        {
            Some(application) => crate::callable::PreparedCallTypeApplication::Present(
                application
                    .arguments()
                    .iter()
                    .map(|argument| match argument {
                        arcweft_lang_hir::expr::HirCallTypeArgument::Resolved { ty } => {
                            self.types.get(ty).cloned()
                        }
                        arcweft_lang_hir::expr::HirCallTypeArgument::InvalidPresent { .. }
                        | arcweft_lang_hir::expr::HirCallTypeArgument::Missing => None,
                    })
                    .collect(),
            ),
            None => crate::callable::PreparedCallTypeApplication::Absent,
        };
        let inputs =
            crate::callable::PreparedCallInputs::new(mapping, semantic_operands, attached_content)
                .with_type_application(type_application);
        if !inputs.validates_type_application(&candidate) {
            let result = self
                .source_result_schema_for_group(owner, &candidate, current_group)
                .map_err(AnalyzerExpressionError::fatal)?;
            return Ok(PreparedCandidatePreparationOutcome::Rejected {
                candidate: Arc::clone(&candidate),
                result,
                evidence: PreparedCandidateRejection::Constraint,
                branch: constraints::AnalyzerCallSealedBranch::Empty,
            });
        }
        let mut rank = AcceptedCandidateRank {
            exact_matches: 0,
            declared_exact_matches: 0,
            unchecked_or_open: inputs.unchecked_or_open_slots(),
            omitted_parameters: inputs.omitted_parameters(),
            authority: candidate.authority(),
        };
        let default_result = self
            .source_result_schema_for_group(owner, &candidate, current_group)
            .map_err(AnalyzerExpressionError::fatal)?;
        let prepared_candidate = Arc::clone(&candidate);
        let view_fx_runtime_parameters = self.view_fx_runtime_parameter_overrides(
            context,
            owner,
            candidate.as_ref(),
            current_group,
        )?;
        let consumer = match context.consumer() {
            super::expression_error::AnalyzerExpressionConsumer::Ordinary => {
                constraints::AnalyzerCallConsumerAdmission::ordinary()
            }
            super::expression_error::AnalyzerExpressionConsumer::ViewFxProducer => {
                constraints::AnalyzerCallConsumerAdmission::view_fx_producer(
                    candidate.as_ref(),
                    view_fx_runtime_parameters,
                )
            }
        };
        let enclosing = self.enclosing_constraint_scope(
            module,
            owner,
            parent_source
                .is_none()
                .then_some(expected_result_scope)
                .flatten(),
        )?;
        let constraint_set = validate_and_prepare_call_constraints(
            self.facts
                .prepared_calls()
                .map_err(AnalyzerExpressionError::fact)?,
            prepared_candidate,
            self.checked_callable_effect_authority()
                .map_err(AnalyzerExpressionError::fatal)?,
            inputs.clone(),
            authored_arguments,
            expected_result,
            owner,
            callee_inputs.clone(),
            dialogue_patch_admissions,
            &compile_time_scalar_admissions,
            self.catalogs.world().environment().compile_time_scalars(),
            consumer.clone(),
            &enclosing,
            parent_source.as_deref(),
            site,
        )
        .map_err(|failure| terminal_call_constraint_failure(owner, failure))?;
        let child_recipe = parent_source.as_ref().map(|_| PreparedCallCandidateRecipe {
            candidate: Arc::clone(&candidate),
            group: current_group,
            consumer,
            callee_inputs,
            inputs,
            source_preparation: constraint_set.application_sources(owner),
        });
        let execution = match parent_source.as_deref_mut() {
            Some(parent_source) => constraints::run_prepared_child_candidate(
                self,
                owner,
                site,
                context,
                pass,
                attempt.cloned(),
                parent_source,
                constraint_set,
            )
            .map(|contribution| PreparedCallCandidateExecution::Deferred {
                pending: contribution.pending,
                descendants: contribution.descendants,
            }),
            None => run_prepared_candidate(
                self,
                owner,
                work,
                context,
                pass,
                attempt.cloned(),
                constraint_set,
            )
            .map(PreparedCallCandidateExecution::Root),
        };
        let execution = match execution {
            Ok(execution) => execution,
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
                return Ok(PreparedCandidatePreparationOutcome::Rejected {
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
        match execution {
            PreparedCallCandidateExecution::Root(transaction) => {
                let result = transaction.result().clone();
                rank.exact_matches = rank
                    .exact_matches
                    .checked_add(transaction.exact_argument_matches())
                    .ok_or(FinalSemanticAnalysisError::AccountingOverflow)
                    .map_err(AnalyzerExpressionError::fatal)?;
                rank.declared_exact_matches = transaction.declared_exact_argument_matches();
                if matches!(&result, CallableResultSchema::Value(value) if expected_result == Some(value))
                {
                    rank.exact_matches = rank
                        .exact_matches
                        .checked_add(1)
                        .ok_or(FinalSemanticAnalysisError::AccountingOverflow)
                        .map_err(AnalyzerExpressionError::fatal)?;
                }
                Ok(PreparedCandidatePreparationOutcome::Accepted { transaction, rank })
            }
            PreparedCallCandidateExecution::Deferred {
                pending,
                descendants,
            } => Ok(PreparedCandidatePreparationOutcome::Deferred {
                candidate: Arc::clone(&candidate),
                pending,
                rank_seed: rank,
                recipe: child_recipe.ok_or_else(|| AnalyzerExpressionError::Call {
                    owner,
                    failure: CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                    )),
                })?,
                descendants,
            }),
        }
    }

    pub(super) fn evaluate_call_constraint_source(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        source: AnalyzerCallConstraintSource,
        expectation: super::expressions::AnalyzerExpressionExpectation<'_>,
        compile_time_scalar: Option<
            &crate::checked_compile_time::PreparedCompileTimeScalarAdmission,
        >,
    ) -> Result<super::PreparedExpressionFact, AnalyzerExpressionError> {
        match source {
            AnalyzerCallConstraintSource::BaseInstantiation
            | AnalyzerCallConstraintSource::DialogueApplicationMetadata { .. }
            | AnalyzerCallConstraintSource::DialogueApplicationOperand { .. }
            | AnalyzerCallConstraintSource::TextProxyObjectOperand { .. } => Err(
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
                if compile_time_scalar.is_some() {
                    return Err(AnalyzerExpressionError::invariant(
                        FinalSemanticAnalysisError::WrongPayloadFamily,
                    ));
                }
                let actual = self
                    .compact_numeric_element_type(sequence, ordinal, expectation.contextual_shape())
                    .map_err(AnalyzerExpressionError::fatal)?;
                Ok(CheckedExpression::value(
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
            } => self.evaluate_call_constraint_expression(
                context,
                expression,
                expectation,
                compile_time_scalar,
            ),
            AnalyzerCallConstraintSource::Receiver { source }
            | AnalyzerCallConstraintSource::Result { source } => {
                if compile_time_scalar.is_some() {
                    return Err(AnalyzerExpressionError::invariant(
                        FinalSemanticAnalysisError::WrongPayloadFamily,
                    ));
                }
                self.evaluate_expression_with_expectation(context, source, expectation)
            }
        }
    }

    /// Evaluates one mapped call expression through the exact compile-time
    /// scalar owner when the selected parameter carries that semantic type.
    /// The ordinary source fact and its reduced scalar stay inside the same
    /// candidate transaction; no post-call source reconstruction is needed.
    fn evaluate_call_constraint_expression(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        expression: ExprId,
        expectation: super::expressions::AnalyzerExpressionExpectation<'_>,
        admission: Option<&crate::checked_compile_time::PreparedCompileTimeScalarAdmission>,
    ) -> Result<super::PreparedExpressionFact, AnalyzerExpressionError> {
        let Some(admission) = admission else {
            return self.evaluate_expression_with_expectation(context, expression, expectation);
        };
        if expectation.contextual_shape() != Some(admission.value_type()) {
            return Err(AnalyzerExpressionError::invariant(
                FinalSemanticAnalysisError::WrongPayloadFamily,
            ));
        }
        let fact =
            self.evaluate_compile_time_scalar_source(context, expression, admission.source_mode())?;
        let module = self
            .module(expression.module())
            .map_err(AnalyzerExpressionError::fatal)?;
        self.materialize_compile_time_scalar_fact(
            module,
            expression,
            admission.kind(),
            admission.value_type().clone(),
            fact,
        )
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
        site: crate::callable::CheckedCallSite,
        module: &HirModule,
        call: &HirCallInvocation,
        selected: &PreparedResolvedCallable,
        callee_inputs: &crate::callable::PreparedCallCalleeConstraintInputs,
        result: &TypeKind,
        callable_effects: Option<&EffectRow>,
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
            if checked.value_type() != Some(actual) {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
            }
            return Ok(AnalyzerPreparedCalleeExpression::callable(*value));
        }
        if callee_inputs.is_function_value() {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
        }
        if let crate::callable::PreparedCallCalleeConstraintInputs::StaticContentCallee(
            static_callee,
        ) = callee_inputs
        {
            let HirCallCallee::Value { value } = call.callee() else {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
            };
            if value != &static_callee.expression() {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
            }
            let CallableCandidateId::Content(identity) = selected.id() else {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
            };
            if identity != &static_callee.identity()
                || selected.schema().semantic_digest() != static_callee.schema()
            {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
            }
            self.facts
                .discard_provisional_expression(*value)
                .map_err(FinalSemanticAnalysisError::from)?;
            return Ok(AnalyzerPreparedCalleeExpression::none());
        }
        if matches!(
            site,
            crate::callable::CheckedCallSite::AttachedContentApplication {
                family: crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
                ..
            }
        ) {
            let HirCallCallee::Value { value } = call.callee() else {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
            };
            self.facts
                .discard_provisional_expression(*value)
                .map_err(FinalSemanticAnalysisError::from)?;
            return Ok(AnalyzerPreparedCalleeExpression::none());
        }
        let (value, nominal_receiver) = match call.callee() {
            HirCallCallee::Value { value } => (*value, false),
            HirCallCallee::UnresolvedDot { value_receiver, .. } => match callee_inputs {
                crate::callable::PreparedCallCalleeConstraintInputs::ValueReceiver {
                    source,
                    actual,
                } if source == value_receiver => {
                    let checked = self.facts.expressions().get(value_receiver).ok_or(
                        FinalSemanticAnalysisError::CallResolutionFailed {
                            owner: *value_receiver,
                        },
                    )?;
                    if checked.value_type() != Some(actual) {
                        return Err(FinalSemanticAnalysisError::CallResolutionFailed {
                            owner: *value_receiver,
                        });
                    }
                    return Ok(AnalyzerPreparedCalleeExpression::semantic(*value_receiver));
                }
                crate::callable::PreparedCallCalleeConstraintInputs::ValueReceiver { .. } => {
                    return Err(FinalSemanticAnalysisError::CallResolutionFailed {
                        owner: *value_receiver,
                    });
                }
                crate::callable::PreparedCallCalleeConstraintInputs::DialogueCallee => {
                    if !self.facts.expressions().contains_key(value_receiver) {
                        return Err(FinalSemanticAnalysisError::CallResolutionFailed {
                            owner: *value_receiver,
                        });
                    }
                    return Ok(AnalyzerPreparedCalleeExpression::semantic(*value_receiver));
                }
                crate::callable::PreparedCallCalleeConstraintInputs::Free
                | crate::callable::PreparedCallCalleeConstraintInputs::EnumConstructor
                | crate::callable::PreparedCallCalleeConstraintInputs::AssociatedType { .. }
                | crate::callable::PreparedCallCalleeConstraintInputs::StaticContentCallee(_) => {
                    (*value_receiver, true)
                }
                crate::callable::PreparedCallCalleeConstraintInputs::DialogueApplication
                | crate::callable::PreparedCallCalleeConstraintInputs::FunctionValue { .. }
                | crate::callable::PreparedCallCalleeConstraintInputs::NonCallable => {
                    return Ok(AnalyzerPreparedCalleeExpression::none());
                }
            },
            HirCallCallee::Associated { .. } => {
                return Ok(AnalyzerPreparedCalleeExpression::none());
            }
        };
        if nominal_receiver {
            let ty = callee_inputs
                .nominal_callee_expression_type(selected.instantiation())
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner: value })?;
            let Some(ty) = ty else {
                return Ok(AnalyzerPreparedCalleeExpression::none());
            };
            if let Some(checked) = self.facts.expressions().get(&value) {
                if checked.value_type() != Some(ty) {
                    return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner: value });
                }
                return Ok(AnalyzerPreparedCalleeExpression::semantic(value));
            }
            self.facts
                .publish_new_expression(
                    value,
                    CheckedExpression::value(
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
        if matches!(expression.kind(), HirExprKind::ShortVariant(_))
            && matches!(
                callee_inputs,
                crate::callable::PreparedCallCalleeConstraintInputs::EnumConstructor
            )
            && !matches!(
                self.facts.expressions().get(&value),
                Some(super::PreparedExpressionFact::Variant(_))
            )
        {
            let template = selected
                .schema()
                .value_type()
                .ok_or(FinalSemanticAnalysisError::CallResolutionFailed { owner: value })?;
            let prepared = self
                .prepare_variant_expression_kind(value, expression, Some(template), true)
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner: value })?
                .ok_or(FinalSemanticAnalysisError::CallResolutionFailed { owner: value })?;
            if self.facts.expressions().contains_key(&value) {
                self.facts
                    .replace_existing_expression(value, prepared)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            } else {
                self.facts
                    .publish_new_expression(value, prepared)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            }
            return Ok(AnalyzerPreparedCalleeExpression::semantic(value));
        }
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
                None if matches!(existing, super::PreparedExpressionFact::Variant(_)) => {
                    return Ok(AnalyzerPreparedCalleeExpression::semantic(value));
                }
                _ if matches!(
                    callee_inputs,
                    crate::callable::PreparedCallCalleeConstraintInputs::Free
                        | crate::callable::PreparedCallCalleeConstraintInputs::EnumConstructor
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
        let ty = match callable_effects
            .and_then(|effects| instantiated_callee_type(selected, result, effects))
        {
            Some(ty) => ty,
            None if method_callee.is_none()
                && matches!(
                    callee_inputs,
                    crate::callable::PreparedCallCalleeConstraintInputs::Free
                        | crate::callable::PreparedCallCalleeConstraintInputs::EnumConstructor
                        | crate::callable::PreparedCallCalleeConstraintInputs::DialogueCallee
                ) =>
            {
                // A direct static call may intentionally admit a supply that
                // has no first-class function-type projection (for example
                // `panic` accepts an unchecked diagnostic payload). The
                // selected call fact already owns its exact callable/schema
                // identity; fabricating `Any` for a callee value would create
                // a second, weaker authority. Such a head is therefore not a
                // value expression in the checked graph.
                self.facts
                    .discard_provisional_expression(value)
                    .map_err(FinalSemanticAnalysisError::from)?;
                return Ok(AnalyzerPreparedCalleeExpression::none());
            }
            None => {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner: value });
            }
        };
        if let Some((receiver, _)) = &method_callee {
            match selected.instantiation() {
                CallableInstantiation::Receiver {
                    receiver: selected_receiver,
                } if self
                    .facts
                    .expressions()
                    .get(receiver)
                    .is_some_and(|checked| checked.value_type() == Some(selected_receiver)) => {}
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
            super::PreparedExpressionFact::from(
                crate::final_analysis::PreparedMethodExpression::new(
                    super::PreparedExpressionShell::value(
                        ty,
                        CheckedTypeSelection::Inferred,
                        effects,
                    ),
                    name,
                ),
            )
        } else {
            CheckedExpression::value(
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
        .constraint_callable_type_with_terminal_effects(effects)
        .ok()
}

fn scope_is_dialogue_line_plan(module: &HirModule, mut scope: ScopeId) -> bool {
    loop {
        if module.expressions().any(|(_, expression)| {
            matches!(
                expression.kind(),
                HirExprKind::AttachedContentApplication(application)
                    if matches!(
                        application.family(),
                        arcweft_lang_hir::dialogue_application::HirAttachedContentApplicationFamily::DialogueLine {
                            target: _,
                            plan: Some(plan),
                            coordinates: _,
                        } if plan.root_scope() == scope
                    )
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
            .analysis_view()
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
    fn checked_character_items_are_exact_while_structural_character_refs_remain_any() {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
        let module = fixture
            .project
            .analysis_view()
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
            crate::final_analysis::PreparedExpressionFact::from(CheckedExpression::value(
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
            crate::final_analysis::PreparedExpressionFact::from(CheckedExpression::value(
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
