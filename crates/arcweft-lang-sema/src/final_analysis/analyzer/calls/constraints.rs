//! Analyzer-owned callable constraint preparation.
//!
//! This module is the narrow boundary between final call analysis and the
//! callable/type constraint owners.  Mapping and graph initialization happen
//! here before a lower work session is opened; callback execution is kept in
//! the affine client below so it cannot mint an expected type or projection.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_lang_hir::{
    expr::HirCallArgumentOrdinal, identity::ExprId, project::HirSelectedCallExpressionInventory,
};

use crate::{
    callable::{
        CallConstraintInvariant, CallableArgumentSemanticAction, CallableArgumentSlotIndex,
        CallableCandidateId, CallableGroupIndex, CallableInstantiation, CallableParameterAdmission,
        CallableParameterConsumer, CallableParameterCoordinate, CallableRestContainerPolicy,
        CallableResultSchema, CallableSemanticValueGuard, CallableValidator,
        CheckedCallArgumentSlotSource, CheckedCallSite, CheckedSemanticValueEvidence,
        DetachedPreparedResolvedCallable, EnclosingGenericParameterScope,
        ObservedSemanticValueEvidence, ParameterExpectedTypeProjection,
        PreparedArgumentSourceProjection, PreparedCallCalleeConstraintInputs, PreparedCallGraph,
        PreparedCallInputs, PreparedCallPrefixPayload, PreparedCallSemanticOperandOwner,
        PreparedCallSemanticOperandRole, PreparedCallableApplication,
        PreparedConstraintInitialization, PreparedDialogueApplicationMetadataArgument,
        PreparedFunctionValueOriginEvidence, PreparedResolvedCallable,
        PreparedResolvedCallableDetachArena, PreparedSourceConstraintGroup,
        VariantPayloadRequirement,
    },
    types::{
        GenericTypeReference, TypeKind,
        constraints::{
            ClosedConstraintProbe, ConstraintAcceptance, ConstraintDomain, ExpectedHint,
            KeyedConstraintProjection, MaterializationOutcome, MaterializedSourceRequest,
            PreparedConstraintSourceProjection, PreparedSourceAlternative,
            PreparedSourceConstraint, ProjectedExpectedHint, SourceError, SourcePhase,
            SourceProbeOutcome, SourceProbeResult, TypeConstraintAbort, TypeConstraintFailure,
            TypeConstraintFailureInvariant, TypeConstraintInitializationFailure,
            TypeConstraintInvariant,
        },
    },
};

use crate::final_analysis::{
    CandidateEvaluationPass, CandidateExpectedType, PhysicalArgumentEvaluationKind,
    PhysicalCandidateArgument, PhysicalCandidateArgumentEvaluation,
};

use super::super::{
    expression_error::{AnalyzerExpressionContext, PhysicalCallAttemptId},
    expressions::AnalyzerExpressionExpectation,
    state::{
        ActiveCallbackFactScope, CandidateSemanticProjection, CandidateSemanticReplayMismatch,
        MaterializationFactCheckpoint, ProbeFactCheckpoint,
    },
};

fn static_content_callee_matches(
    inputs: &PreparedCallCalleeConstraintInputs,
    selected: &CallableCandidateId,
    schema: crate::callable::CallableSignatureSchemaDigest,
) -> bool {
    match inputs {
        PreparedCallCalleeConstraintInputs::StaticContentCallee(static_callee) => {
            selected == &CallableCandidateId::Content(static_callee.identity())
                && schema == static_callee.schema()
        }
        PreparedCallCalleeConstraintInputs::Free
        | PreparedCallCalleeConstraintInputs::EnumConstructor
        | PreparedCallCalleeConstraintInputs::ValueReceiver { .. }
        | PreparedCallCalleeConstraintInputs::AssociatedType { .. }
        | PreparedCallCalleeConstraintInputs::DialogueCallee
        | PreparedCallCalleeConstraintInputs::DialogueApplication
        | PreparedCallCalleeConstraintInputs::FunctionValue { .. }
        | PreparedCallCalleeConstraintInputs::NonCallable => true,
    }
}

/// Exact source identity used by analyzer callback work.  The HIR expression
/// identity is already generation-owned and ordered; no source spelling is
/// reconstructed here.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum AnalyzerCallConstraintSource {
    BaseInstantiation,
    Receiver {
        source: ExprId,
    },
    Argument {
        argument: HirCallArgumentOrdinal,
        slot: CallableArgumentSlotIndex,
        source: CheckedCallArgumentSlotSource,
        physical_kind: PhysicalArgumentEvaluationKind,
    },
    DialoguePatch {
        argument: HirCallArgumentOrdinal,
        slot: CallableArgumentSlotIndex,
        source: CheckedCallArgumentSlotSource,
        coordinate: CallableParameterCoordinate,
        physical_kind: PhysicalArgumentEvaluationKind,
    },
    DialogueApplicationMetadata {
        argument: HirCallArgumentOrdinal,
        slot: CallableArgumentSlotIndex,
        source: ExprId,
        coordinate: crate::callable::DialogueApplicationMetadataCoordinate,
    },
    DialogueApplicationOperand {
        owner: PreparedCallSemanticOperandOwner,
        source: ExprId,
        role: PreparedCallSemanticOperandRole,
        coordinate: CallableParameterCoordinate,
    },
    TextProxyObjectOperand {
        argument: HirCallArgumentOrdinal,
        source: ExprId,
        coordinate: CallableParameterCoordinate,
    },
    Result {
        source: ExprId,
    },
}

impl AnalyzerCallConstraintSource {
    fn value_coordinate(self) -> Option<AnalyzerCallValueCoordinate> {
        match self {
            Self::Receiver { .. } => Some(AnalyzerCallValueCoordinate::Receiver),
            Self::Argument { argument, slot, .. }
            | Self::DialoguePatch { argument, slot, .. }
            | Self::DialogueApplicationMetadata { argument, slot, .. } => {
                Some(AnalyzerCallValueCoordinate::Argument { argument, slot })
            }
            Self::BaseInstantiation
            | Self::DialogueApplicationOperand { .. }
            | Self::TextProxyObjectOperand { .. }
            | Self::Result { .. } => None,
        }
    }

    fn physical_argument(self) -> Option<PhysicalCandidateArgument> {
        let (argument, slot, source, physical_kind) = match self {
            Self::Argument {
                argument,
                slot,
                source,
                physical_kind,
            }
            | Self::DialoguePatch {
                argument,
                slot,
                source,
                physical_kind,
                ..
            } => (argument, slot, source, physical_kind),
            _ => return None,
        };
        Some(PhysicalCandidateArgument::new(
            argument,
            slot,
            source,
            physical_kind,
            CandidateExpectedType::Unchecked,
        ))
    }

    pub(crate) fn same_argument_identity(self, other: Self) -> bool {
        match (self, other) {
            (
                Self::Argument {
                    argument: left_argument,
                    slot: left_slot,
                    source: left_source,
                    ..
                },
                Self::Argument {
                    argument: right_argument,
                    slot: right_slot,
                    source: right_source,
                    ..
                },
            ) => {
                left_argument == right_argument
                    && left_slot == right_slot
                    && left_source == right_source
            }
            (
                Self::DialoguePatch {
                    argument: left_argument,
                    slot: left_slot,
                    source: left_source,
                    coordinate: left_coordinate,
                    ..
                },
                Self::DialoguePatch {
                    argument: right_argument,
                    slot: right_slot,
                    source: right_source,
                    coordinate: right_coordinate,
                    ..
                },
            ) => {
                left_argument == right_argument
                    && left_slot == right_slot
                    && left_source == right_source
                    && left_coordinate == right_coordinate
            }
            _ => false,
        }
    }
}

/// Uniqueness coordinate of an evaluated receiver or argument slot. Source
/// evidence owns its type; this coordinate does not index a second projection.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum AnalyzerCallValueCoordinate {
    Receiver,
    Argument {
        argument: HirCallArgumentOrdinal,
        slot: CallableArgumentSlotIndex,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AnalyzerCallScopeCoordinate {
    Probe {
        source: AnalyzerCallConstraintSource,
    },
    Materialization {
        owner: AnalyzerCallConstraintSource,
        sources: Box<[AnalyzerCallConstraintSource]>,
    },
}

impl AnalyzerCallScopeCoordinate {
    fn owner_source(&self) -> AnalyzerCallConstraintSource {
        match self {
            Self::Probe { source } => *source,
            Self::Materialization { owner, .. } => *owner,
        }
    }

    fn accepts_probe(&self, source: AnalyzerCallConstraintSource) -> bool {
        matches!(self, Self::Probe { source: expected } if *expected == source)
    }

    fn accepts_materialization(&self, source: AnalyzerCallConstraintSource) -> bool {
        matches!(self, Self::Materialization { sources, .. } if sources.iter().any(|expected| *expected == source))
    }
}

/// Analyzer-owned probe checkpoint proof.  The lower fact checkpoint proves
/// the fact transaction identity; this source coordinate proves which callback
/// source the checkpoint was opened for.  Keeping the two in one affine value
/// prevents a raw fact checkpoint from being reattached to another source.
struct AnalyzerProbeCheckpoint {
    checkpoint: ProbeFactCheckpoint,
    source: AnalyzerCallConstraintSource,
}

/// Analyzer-owned materialization checkpoint proof.  The ordered source list
/// is part of the callback authority, rather than an independently supplied
/// close argument.  `next_source` records the validated request prefix.
struct AnalyzerMaterializationCheckpoint {
    checkpoint: MaterializationFactCheckpoint,
    sources: Box<[AnalyzerCallConstraintSource]>,
    next_source: usize,
}

#[derive(Debug)]
pub(in crate::final_analysis::analyzer) struct AnalyzerCallConstraintDomain;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum AnalyzerCallSourceFailureCause {
    Mismatch,
    FinalSemantic(Box<crate::final_analysis::FinalSemanticAnalysisError>),
    NestedCallFatal {
        owner: ExprId,
        error: Box<SourceError<AnalyzerCallConstraintSource, AnalyzerCallSourceFailureCause>>,
    },
}

impl AnalyzerCallSourceFailureCause {
    /// Returns only an authored terminal diagnostic raised by this exact
    /// source callback. Nested call failures retain their call-site
    /// provenance and must not be flattened into the outer source.
    pub(crate) fn direct_final_semantic(
        &self,
    ) -> Option<&crate::final_analysis::FinalSemanticAnalysisError> {
        match self {
            Self::FinalSemantic(error) => Some(error.as_ref()),
            Self::Mismatch | Self::NestedCallFatal { .. } => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AnalyzerCallProbeSemanticBranch {
    pub(crate) source: AnalyzerCallConstraintSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AnalyzerPreparedDialoguePatchAdmission {
    argument: HirCallArgumentOrdinal,
    source: ExprId,
    coordinate: CallableParameterCoordinate,
    field: arcweft_interaction_model::dialogue::CharacterDialogueCustomFieldId,
    declared: TypeKind,
    clearable: bool,
    supply_alternative: u32,
    field_span: arcweft_source::SourceSpan,
    value_span: arcweft_source::SourceSpan,
    declaration_span: arcweft_source::SourceSpan,
}

/// Definition-owned coordinate row for one semantic compile-time scalar
/// parameter. The callable schema retains the closed kind and exact value
/// type; this analyzer row additionally carries the checked reduction grammar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AnalyzerPreparedCompileTimeScalarAdmission {
    coordinate: CallableParameterCoordinate,
    admission: Arc<crate::checked_compile_time::PreparedCompileTimeScalarAdmission>,
}

impl AnalyzerPreparedCompileTimeScalarAdmission {
    pub(crate) fn new(
        coordinate: CallableParameterCoordinate,
        admission: crate::checked_compile_time::PreparedCompileTimeScalarAdmission,
    ) -> Self {
        Self {
            coordinate,
            admission: Arc::new(admission),
        }
    }

    pub(crate) const fn coordinate(&self) -> CallableParameterCoordinate {
        self.coordinate
    }

    pub(crate) fn admission(
        &self,
    ) -> &Arc<crate::checked_compile_time::PreparedCompileTimeScalarAdmission> {
        &self.admission
    }
}

impl AnalyzerPreparedDialoguePatchAdmission {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        argument: HirCallArgumentOrdinal,
        source: ExprId,
        coordinate: CallableParameterCoordinate,
        field: arcweft_interaction_model::dialogue::CharacterDialogueCustomFieldId,
        declared: TypeKind,
        clearable: bool,
        supply_alternative: u32,
        field_span: arcweft_source::SourceSpan,
        value_span: arcweft_source::SourceSpan,
        declaration_span: arcweft_source::SourceSpan,
    ) -> Self {
        Self {
            argument,
            source,
            coordinate,
            field,
            declared,
            clearable,
            supply_alternative,
            field_span,
            value_span,
            declaration_span,
        }
    }

    pub(crate) const fn argument(&self) -> HirCallArgumentOrdinal {
        self.argument
    }
    pub(crate) const fn source(&self) -> ExprId {
        self.source
    }
    pub(crate) const fn coordinate(&self) -> CallableParameterCoordinate {
        self.coordinate
    }
    pub(crate) const fn declared(&self) -> &TypeKind {
        &self.declared
    }
    pub(crate) const fn clearable(&self) -> bool {
        self.clearable
    }

    fn validates_parameter(
        &self,
        coordinate: CallableParameterCoordinate,
        parameter: &crate::callable::CallableParameter,
    ) -> bool {
        if coordinate != self.coordinate
            || parameter.declared_type() != Some(&self.declared)
            || !matches!(
                parameter.consumer(),
                CallableParameterConsumer::DialoguePatch(
                    crate::final_analysis::CharacterDialogueFieldCoordinate::Custom(field)
                ) if field == &self.field
            )
        {
            return false;
        }
        let Some(rule) = parameter.value_rule() else {
            return false;
        };
        let Ok(supply_alternative) = u32::try_from(rule.guarded().len()) else {
            return false;
        };
        if supply_alternative != self.supply_alternative
            || rule.otherwise().expected() != &ParameterExpectedTypeProjection::Identity
            || rule.otherwise().action() != CallableArgumentSemanticAction::Supply
        {
            return false;
        }
        let clearable = match rule.guarded() {
            [] => false,
            [guarded]
                if guarded.expected()
                    == &ParameterExpectedTypeProjection::ApplyUnary(
                        crate::callable::CallableUnaryTypeConstructor::Option,
                    )
                    && guarded.action() == CallableArgumentSemanticAction::Clear
                    && matches!(
                        guarded.guard(),
                        CallableSemanticValueGuard::VariantCase {
                            owner: ParameterExpectedTypeProjection::ApplyUnary(
                                crate::callable::CallableUnaryTypeConstructor::Option
                            ),
                            ordinal: 1,
                            payload: VariantPayloadRequirement::Unit,
                        }
                    ) =>
            {
                true
            }
            _ => return false,
        };
        clearable == self.clearable
    }
    fn clear_failure(&self) -> crate::final_analysis::FinalSemanticAnalysisError {
        crate::final_analysis::FinalSemanticAnalysisError::CharacterDialogueFieldNotClearable {
            field: self.field.clone(),
            field_span: self.field_span.clone(),
            declaration_span: self.declaration_span.clone(),
        }
    }
    pub(super) fn mismatch_failure(
        &self,
        actual: TypeKind,
    ) -> crate::final_analysis::FinalSemanticAnalysisError {
        crate::final_analysis::FinalSemanticAnalysisError::CharacterDialogueCustomFieldTypeMismatch {
            field: self.field.clone(),
            declared: Box::new(self.declared.clone()),
            actual: Box::new(actual),
            value_span: self.value_span.clone(),
            declaration_span: self.declaration_span.clone(),
        }
    }

    pub(super) fn accepts_rejected_source_projection(
        &self,
        rejected: &crate::types::constraints::RejectedConstraintSourceProjection<
            AnalyzerCallConstraintDomain,
        >,
    ) -> bool {
        matches!(
            rejected.source(),
            AnalyzerCallConstraintSource::DialoguePatch {
                argument,
                source: CheckedCallArgumentSlotSource::Expression(source),
                coordinate,
                ..
            } if argument == self.argument
                && source == self.source
                && coordinate == self.coordinate
        ) && rejected.alternative() == Some(self.supply_alternative)
            && matches!(
                rejected.source_projection(),
                crate::types::constraints::CheckedConstraintSourceProjection::Scalar
            )
            && rejected.acceptance() == ConstraintAcceptance::PatternAcceptsActual
            && rejected.expected() == &self.declared
    }
}

/// The callback branch carried by a completed lower candidate.
///
/// A zero-source path is a real, sealed empty branch.  A materialized path
/// owns only the fact projection extracted at checkpoint close.  There is no
/// optional projection state: callers must handle the two sealed outcomes.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::final_analysis::analyzer) enum AnalyzerCallSealedBranch {
    Empty,
    Materialized {
        projection: CandidateSemanticProjection,
    },
}

impl AnalyzerCallSealedBranch {
    fn semantic_replay_mismatch(&self, other: &Self) -> Option<CallConstraintInvariant> {
        let mismatch = match (self, other) {
            (Self::Empty, Self::Empty) => return None,
            (Self::Materialized { projection: left }, Self::Materialized { projection: right }) => {
                left.semantic_replay_mismatch(right)?
            }
            (Self::Empty, Self::Materialized { .. }) | (Self::Materialized { .. }, Self::Empty) => {
                return Some(CallConstraintInvariant::ReplaySealedBranchShapeMismatch);
            }
        };
        Some(match mismatch {
            CandidateSemanticReplayMismatch::Authority => {
                CallConstraintInvariant::ReplayBranchProjectionAuthorityMismatch
            }
            CandidateSemanticReplayMismatch::PreparedGraph(mismatch) => {
                CallConstraintInvariant::ReplayBranchPreparedGraphMismatch(mismatch)
            }
            CandidateSemanticReplayMismatch::Locals => {
                CallConstraintInvariant::ReplayBranchLocalFactsMismatch
            }
            CandidateSemanticReplayMismatch::Patterns => {
                CallConstraintInvariant::ReplayBranchPatternFactsMismatch
            }
            CandidateSemanticReplayMismatch::Expressions => {
                CallConstraintInvariant::ReplayBranchExpressionFactsMismatch
            }
            CandidateSemanticReplayMismatch::CheckedContent => {
                CallConstraintInvariant::ReplayBranchDialogueMarkCatalogMismatch
            }
            CandidateSemanticReplayMismatch::Iterations => {
                CallConstraintInvariant::ReplayBranchIterationFactsMismatch
            }
            CandidateSemanticReplayMismatch::ImplicitCaptureUses => {
                CallConstraintInvariant::ReplayBranchImplicitCaptureMismatch
            }
            CandidateSemanticReplayMismatch::PhysicalCandidateEvaluations => {
                CallConstraintInvariant::ReplayBranchPhysicalTranscriptMismatch
            }
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AnalyzerCallPreparedSealedBranch;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum AnalyzerCallProjection {
    BaseInstantiation,
    Result,
    Future(GenericTypeReference),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AnalyzerCallClientInvariant {
    pub(crate) source: AnalyzerCallConstraintSource,
    pub(crate) cause: AnalyzerCallClientInvariantCause,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum AnalyzerCallClientInvariantCause {
    Constraint(CallConstraintInvariant),
    NestedCall {
        owner: ExprId,
        invariant: Box<CallAnalysisInvariant>,
    },
    FinalSemantic(Box<crate::final_analysis::FinalSemanticAnalysisError>),
    FactTransaction(crate::final_analysis::CandidateFactTransactionViolation),
    ActiveFactScope(AnalyzerCallScopeCoordinate),
    CallFrame {
        owner: ExprId,
        violation: crate::final_analysis::analyzer::expression_error::CallFrameInvariant,
    },
    ActiveFactScopeConflict {
        existing: AnalyzerCallScopeCoordinate,
        requested: AnalyzerCallScopeCoordinate,
    },
}

impl AnalyzerCallClientInvariant {
    fn constraint(
        source: AnalyzerCallConstraintSource,
        invariant: CallConstraintInvariant,
    ) -> Self {
        Self {
            source,
            cause: AnalyzerCallClientInvariantCause::Constraint(invariant),
        }
    }

    pub(crate) fn nested_call(
        source: AnalyzerCallConstraintSource,
        owner: ExprId,
        invariant: CallAnalysisInvariant,
    ) -> Self {
        Self {
            source,
            cause: AnalyzerCallClientInvariantCause::NestedCall {
                owner,
                invariant: Box::new(invariant),
            },
        }
    }

    pub(crate) fn final_semantic(
        source: AnalyzerCallConstraintSource,
        error: crate::final_analysis::FinalSemanticAnalysisError,
    ) -> Self {
        Self {
            source,
            cause: AnalyzerCallClientInvariantCause::FinalSemantic(Box::new(error)),
        }
    }

    pub(crate) fn fact_transaction(
        source: AnalyzerCallConstraintSource,
        violation: crate::final_analysis::CandidateFactTransactionViolation,
    ) -> Self {
        Self {
            source,
            cause: AnalyzerCallClientInvariantCause::FactTransaction(violation),
        }
    }

    pub(crate) fn active_fact_scope(coordinate: AnalyzerCallScopeCoordinate) -> Self {
        Self {
            source: coordinate.owner_source(),
            cause: AnalyzerCallClientInvariantCause::ActiveFactScope(coordinate),
        }
    }

    pub(crate) fn active_fact_scope_mismatch(
        source: AnalyzerCallConstraintSource,
        coordinate: AnalyzerCallScopeCoordinate,
    ) -> Self {
        Self {
            source,
            cause: AnalyzerCallClientInvariantCause::ActiveFactScope(coordinate),
        }
    }

    pub(crate) fn active_fact_scope_conflict(
        existing: AnalyzerCallScopeCoordinate,
        requested: AnalyzerCallScopeCoordinate,
    ) -> Self {
        Self {
            source: requested.owner_source(),
            cause: AnalyzerCallClientInvariantCause::ActiveFactScopeConflict {
                existing,
                requested,
            },
        }
    }

    pub(crate) fn call_frame(
        source: AnalyzerCallConstraintSource,
        owner: ExprId,
        violation: crate::final_analysis::analyzer::expression_error::CallFrameInvariant,
    ) -> Self {
        Self {
            source,
            cause: AnalyzerCallClientInvariantCause::CallFrame { owner, violation },
        }
    }
}

impl ConstraintDomain for AnalyzerCallConstraintDomain {
    type Source = AnalyzerCallConstraintSource;
    type AlternativeIndex = u32;
    type EvidenceRule = AnalyzerCallEvidenceRule;
    type ObservedEvidence = ObservedSemanticValueEvidence;
    type CheckedEvidence = CheckedSemanticValueEvidence;
    type ProbeSemanticBranch = AnalyzerCallProbeSemanticBranch;
    type SealedBranchValue = AnalyzerCallSealedBranch;
    type Projection = AnalyzerCallProjection;
    type SourceErrorCause = AnalyzerCallSourceFailureCause;
    type ClientInvariant = AnalyzerCallClientInvariant;

    fn evidence_accepts(rule: &Self::EvidenceRule, checked: &Self::ObservedEvidence) -> bool {
        rule.accepts(checked)
    }

    fn project_checked_evidence(
        checked: &Self::ObservedEvidence,
        actual: &TypeKind,
    ) -> Option<Self::CheckedEvidence> {
        checked
            .try_project_owner(|_| actual.semantic_identity_digest())
            .ok()
    }

    fn alternative_ordinal(index: &Self::AlternativeIndex) -> u32 {
        *index
    }

    fn client_invariant_source(invariant: &Self::ClientInvariant) -> Self::Source {
        invariant.source
    }

    fn empty_sealed_branch() -> Self::SealedBranchValue {
        AnalyzerCallSealedBranch::Empty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AnalyzerCallEvidenceRule {
    kind: AnalyzerCallEvidenceRuleKind,
    effect_projection: Option<AnalyzerCallEffectProjectionRequest>,
    compile_time_scalar:
        Option<Arc<crate::checked_compile_time::PreparedCompileTimeScalarAdmission>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AnalyzerCallEvidenceRuleKind {
    Guarded {
        guard: CallableSemanticValueGuard,
        declared: TypeKind,
    },
    Otherwise,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AnalyzerCallEffectProjectionRequest {
    coordinate: CallableParameterCoordinate,
    expected: ParameterExpectedTypeProjection,
    source_projection: PreparedConstraintSourceProjection,
}

impl AnalyzerCallEvidenceRule {
    fn guarded(
        guard: CallableSemanticValueGuard,
        declared: TypeKind,
        effect_projection: Option<AnalyzerCallEffectProjectionRequest>,
    ) -> Self {
        Self {
            kind: AnalyzerCallEvidenceRuleKind::Guarded { guard, declared },
            effect_projection,
            compile_time_scalar: None,
        }
    }

    fn otherwise(effect_projection: Option<AnalyzerCallEffectProjectionRequest>) -> Self {
        Self {
            kind: AnalyzerCallEvidenceRuleKind::Otherwise,
            effect_projection,
            compile_time_scalar: None,
        }
    }

    fn compile_time_scalar(
        admission: Arc<crate::checked_compile_time::PreparedCompileTimeScalarAdmission>,
    ) -> Self {
        Self {
            kind: AnalyzerCallEvidenceRuleKind::Otherwise,
            effect_projection: None,
            compile_time_scalar: Some(admission),
        }
    }

    fn accepts(&self, checked: &ObservedSemanticValueEvidence) -> bool {
        match &self.kind {
            AnalyzerCallEvidenceRuleKind::Guarded { guard, declared } => {
                guard.accepts_observation(declared, checked)
            }
            AnalyzerCallEvidenceRuleKind::Otherwise => true,
        }
    }
}

struct AnalyzerCallObservedSource {
    actual: TypeKind,
    evidence: ObservedSemanticValueEvidence,
}

fn validate_materialized_source_request(
    request: &MaterializedSourceRequest<'_, AnalyzerCallConstraintDomain>,
    checked: &AnalyzerCallObservedSource,
) -> Result<(), TypeConstraintInvariant> {
    let source = *request.source();
    if request.canonical_branch().source != source {
        return Err(TypeConstraintInvariant::SourceProtocol(
            crate::types::constraints::TypeConstraintSourceProtocolInvariant::Outcome,
        ));
    }
    if &checked.actual != request.actual()
        || !request.source_projection().matches_actual(&checked.actual)
    {
        return Err(TypeConstraintInvariant::Projection(
            crate::types::constraints::TypeConstraintProjectionInvariant::Mismatch(
                crate::types::constraints::TypeConstraintRejection::Mismatch,
            ),
        ));
    }
    if request.evidence().is_some_and(|evidence| {
        <AnalyzerCallConstraintDomain as ConstraintDomain>::project_checked_evidence(
            &checked.evidence,
            request.actual(),
        )
        .as_ref()
            != Some(evidence)
    }) {
        return Err(TypeConstraintInvariant::SourceProtocol(
            crate::types::constraints::TypeConstraintSourceProtocolInvariant::InvalidEvidence,
        ));
    }
    Ok(())
}

/// A source value that was already checked while the callee or a
/// schema-owned semantic application was staged.  These rows are the only
/// callback inputs that may bypass expression re-evaluation: the variant
/// records which owner issued the actual, and the map key records its exact
/// lower source coordinate.
enum AnalyzerPreparedSourceActual {
    ValueReceiver(TypeKind),
    DialogueApplicationMetadata(PreparedDialogueApplicationMetadataArgument),
    DialogueApplicationOperand(TypeKind),
    TextProxyObjectOperand(TypeKind),
}

impl AnalyzerPreparedSourceActual {
    fn validates(&self, source: AnalyzerCallConstraintSource) -> bool {
        match (self, source) {
            (Self::ValueReceiver(_), AnalyzerCallConstraintSource::Receiver { .. })
            | (
                Self::DialogueApplicationOperand(_),
                AnalyzerCallConstraintSource::DialogueApplicationOperand { .. },
            ) => true,
            (
                Self::TextProxyObjectOperand(_),
                AnalyzerCallConstraintSource::TextProxyObjectOperand { .. },
            ) => true,
            (
                Self::DialogueApplicationMetadata(prepared),
                AnalyzerCallConstraintSource::DialogueApplicationMetadata {
                    argument,
                    source,
                    coordinate,
                    ..
                },
            ) => {
                prepared.argument() == argument
                    && prepared.source() == source
                    && prepared.coordinate() == coordinate
            }
            _ => false,
        }
    }

    const fn actual(&self) -> &TypeKind {
        match self {
            Self::ValueReceiver(actual)
            | Self::DialogueApplicationOperand(actual)
            | Self::TextProxyObjectOperand(actual) => actual,
            Self::DialogueApplicationMetadata(prepared) => prepared.actual(),
        }
    }

    fn evidence(&self) -> ObservedSemanticValueEvidence {
        match self {
            Self::ValueReceiver(_)
            | Self::DialogueApplicationMetadata(_)
            | Self::DialogueApplicationOperand(_)
            | Self::TextProxyObjectOperand(_) => ObservedSemanticValueEvidence::NoVariantCase,
        }
    }
}

enum AnalyzerCallCheckFailure {
    Mismatch,
    Fatal(SourceError<AnalyzerCallConstraintSource, AnalyzerCallSourceFailureCause>),
    Abort(TypeConstraintAbort),
    Invariant(AnalyzerCallClientInvariant),
}

struct AnalyzerCallActiveFactScope {
    scope: ActiveCallbackFactScope,
    coordinate: AnalyzerCallScopeCoordinate,
}

/// The analyzer-side callback implementation used by the candidate driver.
/// It owns no lower authority: all expected types and source projections come
/// from the borrowed lower hint/request. Expression checking is the sole way
/// to obtain an actual except for the exact callee receiver or schema-owned
/// semantic operand already staged by its typed source owner; those prepared
/// rows are admitted only at their issuer-defined source coordinate.
pub(crate) struct AnalyzerCallExpressionClient<'a, 'project, 'catalog, 'control> {
    analyzer: &'a mut super::super::Analyzer<'project, 'catalog, 'control>,
    context: &'a AnalyzerExpressionContext<'a>,
    candidate: Option<Arc<PreparedResolvedCallable>>,
    effect_projections:
        BTreeMap<(AnalyzerCallConstraintSource, u32), AnalyzerCallEffectProjectionRequest>,
    compile_time_scalar_admissions: BTreeMap<
        (AnalyzerCallConstraintSource, u32),
        Arc<crate::checked_compile_time::PreparedCompileTimeScalarAdmission>,
    >,
    prepared_source_actuals: BTreeMap<AnalyzerCallConstraintSource, AnalyzerPreparedSourceActual>,
    dialogue_patch_admissions:
        BTreeMap<AnalyzerCallConstraintSource, AnalyzerPreparedDialoguePatchAdmission>,
    pass: CandidateEvaluationPass,
    attempt: Option<PhysicalCallAttemptId>,
    active_fact_scope: Option<AnalyzerCallActiveFactScope>,
}

impl<'a, 'project, 'catalog, 'control>
    AnalyzerCallExpressionClient<'a, 'project, 'catalog, 'control>
{
    fn new(
        analyzer: &'a mut super::super::Analyzer<'project, 'catalog, 'control>,
        context: &'a AnalyzerExpressionContext<'a>,
        candidate: Option<Arc<PreparedResolvedCallable>>,
        effect_projections: BTreeMap<
            (AnalyzerCallConstraintSource, u32),
            AnalyzerCallEffectProjectionRequest,
        >,
        compile_time_scalar_admissions: BTreeMap<
            (AnalyzerCallConstraintSource, u32),
            Arc<crate::checked_compile_time::PreparedCompileTimeScalarAdmission>,
        >,
        prepared_source_actuals: BTreeMap<
            AnalyzerCallConstraintSource,
            AnalyzerPreparedSourceActual,
        >,
        dialogue_patch_admissions: BTreeMap<
            AnalyzerCallConstraintSource,
            AnalyzerPreparedDialoguePatchAdmission,
        >,
        pass: CandidateEvaluationPass,
        attempt: Option<PhysicalCallAttemptId>,
    ) -> Self {
        Self {
            analyzer,
            context,
            candidate,
            effect_projections,
            compile_time_scalar_admissions,
            prepared_source_actuals,
            dialogue_patch_admissions,
            pass,
            attempt,
            active_fact_scope: None,
        }
    }

    fn physical_expected(
        source: AnalyzerCallConstraintSource,
        hint: &ExpectedHint<'_, AnalyzerCallConstraintDomain>,
    ) -> CandidateExpectedType {
        let Some(physical) = source.physical_argument() else {
            return CandidateExpectedType::Unchecked;
        };
        match physical.kind() {
            PhysicalArgumentEvaluationKind::TypedRestSpread => CandidateExpectedType::Unchecked,
            PhysicalArgumentEvaluationKind::Unmapped => CandidateExpectedType::Unmapped,
            PhysicalArgumentEvaluationKind::Authored
            | PhysicalArgumentEvaluationKind::Recovered
            | PhysicalArgumentEvaluationKind::FixedLiteralSpread => match hint {
                ExpectedHint::Unchecked => CandidateExpectedType::Unchecked,
                ExpectedHint::Alternatives(alternatives) => {
                    alternatives
                        .first()
                        .map_or(CandidateExpectedType::Unchecked, |alternative| {
                            let expected = match alternative.value_expected() {
                                ProjectedExpectedHint::Complete(expected)
                                | ProjectedExpectedHint::Parametric { expected, .. } => expected,
                            };
                            CandidateExpectedType::Exact((*expected).clone())
                        })
                }
            },
        }
    }

    fn record_physical_source(
        &mut self,
        source: AnalyzerCallConstraintSource,
        expected: CandidateExpectedType,
    ) -> Result<(), crate::final_analysis::FinalSemanticAnalysisError> {
        let Some(physical) = source.physical_argument() else {
            return Ok(());
        };
        let Some(candidate) = self.candidate.as_ref() else {
            return Ok(());
        };
        let physical = PhysicalCandidateArgument::new(
            physical.argument(),
            physical.slot(),
            physical.source(),
            physical.kind(),
            expected,
        );
        let attempt = self.attempt.clone().ok_or_else(|| {
            crate::final_analysis::FinalSemanticAnalysisError::CandidateFactTransaction {
                violation: crate::final_analysis::CandidateFactTransactionViolation::PhysicalCallAttemptRootMismatch,
            }
        })?;
        self.analyzer.record_physical_candidate_argument_evaluation(
            PhysicalCandidateArgumentEvaluation::new(
                attempt,
                candidate.id().clone(),
                self.pass,
                physical,
            ),
        )
    }

    fn admit_physical_source(
        &mut self,
        source: AnalyzerCallConstraintSource,
        phase: SourcePhase,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
    ) -> Result<(), crate::callable::SourceCallbackFailure<AnalyzerCallConstraintDomain>> {
        if source.physical_argument().is_some() {
            self.analyzer
                .control
                .check_physical_slot_boundary()
                .map_err(|error| match error {
                    crate::final_analysis::FinalSemanticAnalysisError::Cancelled => {
                        crate::callable::SourceCallbackFailure::Abort(
                            TypeConstraintAbort::Cancelled,
                        )
                    }
                    error => crate::callable::SourceCallbackFailure::fatal(SourceError::new(
                        source,
                        phase,
                        AnalyzerCallSourceFailureCause::FinalSemantic(Box::new(error)),
                    )),
                })?;
        }
        work.charge_callback_expression(1)
            .map_err(crate::callable::SourceCallbackFailure::Abort)?;
        work.check_cancelled()
            .map_err(crate::callable::SourceCallbackFailure::Abort)?;
        Ok(())
    }

    fn active_source_check(
        &self,
        source: AnalyzerCallConstraintSource,
        phase: SourcePhase,
    ) -> Result<(), AnalyzerCallClientInvariant> {
        let Some(active) = self.active_fact_scope.as_ref() else {
            return Err(AnalyzerCallClientInvariant::fact_transaction(
                source,
                crate::final_analysis::CandidateFactTransactionViolation::StaleCheckpoint,
            ));
        };
        let accepted = match phase {
            SourcePhase::Probe => active.coordinate.accepts_probe(source),
            SourcePhase::Materialize => active.coordinate.accepts_materialization(source),
        };
        accepted.then_some(()).ok_or_else(|| {
            AnalyzerCallClientInvariant::active_fact_scope_mismatch(
                source,
                active.coordinate.clone(),
            )
        })
    }

    fn probe_checkpoint_check(
        &self,
        source: AnalyzerCallConstraintSource,
        checkpoint: &AnalyzerProbeCheckpoint,
    ) -> Result<(), crate::callable::SourceCallbackFailure<AnalyzerCallConstraintDomain>> {
        let Some(active) = self.active_fact_scope.as_ref() else {
            return Err(crate::callable::SourceCallbackFailure::invariant(
                AnalyzerCallClientInvariant::fact_transaction(
                    source,
                    crate::final_analysis::CandidateFactTransactionViolation::StaleCheckpoint,
                ),
            ));
        };
        if active.coordinate.accepts_probe(source)
            && checkpoint.source == source
            && active
                .scope
                .matches_probe_checkpoint(&checkpoint.checkpoint)
        {
            return Ok(());
        }
        Err(crate::callable::SourceCallbackFailure::invariant(
            AnalyzerCallClientInvariant::active_fact_scope_mismatch(
                source,
                active.coordinate.clone(),
            ),
        ))
    }

    fn materialization_checkpoint_check(
        &mut self,
        source: AnalyzerCallConstraintSource,
        checkpoint: &mut AnalyzerMaterializationCheckpoint,
    ) -> Result<(), crate::callable::SourceCallbackFailure<AnalyzerCallConstraintDomain>> {
        let Some(active) = self.active_fact_scope.as_ref() else {
            return Err(crate::callable::SourceCallbackFailure::invariant(
                AnalyzerCallClientInvariant::fact_transaction(
                    source,
                    crate::final_analysis::CandidateFactTransactionViolation::StaleCheckpoint,
                ),
            ));
        };
        if matches!(
            &active.coordinate,
            AnalyzerCallScopeCoordinate::Materialization { sources, .. }
                if sources.as_ref() == checkpoint.sources.as_ref()
        ) && active
            .scope
            .matches_materialization_checkpoint(&checkpoint.checkpoint)
        {
            if checkpoint.sources.get(checkpoint.next_source) == Some(&source) {
                checkpoint.next_source += 1;
                return Ok(());
            }
        }
        Err(crate::callable::SourceCallbackFailure::invariant(
            AnalyzerCallClientInvariant::active_fact_scope_mismatch(
                source,
                active.coordinate.clone(),
            ),
        ))
    }

    fn check_source(
        &mut self,
        source: AnalyzerCallConstraintSource,
        expectation: AnalyzerExpressionExpectation<'_>,
        effect_projection: Option<&AnalyzerCallEffectProjectionRequest>,
        compile_time_scalar: Option<
            &crate::checked_compile_time::PreparedCompileTimeScalarAdmission,
        >,
        phase: SourcePhase,
    ) -> Result<AnalyzerCallObservedSource, AnalyzerCallCheckFailure> {
        self.active_source_check(source, phase)
            .map_err(AnalyzerCallCheckFailure::Invariant)?;
        if let Some(prepared) = self.prepared_source_actuals.get(&source) {
            if compile_time_scalar.is_some() || !prepared.validates(source) {
                return Err(AnalyzerCallCheckFailure::Invariant(
                    AnalyzerCallClientInvariant::constraint(
                        source,
                        CallConstraintInvariant::MalformedMapperSeal,
                    ),
                ));
            }
            let actual = match effect_projection {
                None => prepared.actual().clone(),
                Some(request) => {
                    let candidate = self.candidate.as_ref().ok_or_else(|| {
                        AnalyzerCallCheckFailure::Invariant(
                            AnalyzerCallClientInvariant::constraint(
                                source,
                                CallConstraintInvariant::PreparedEffectInstantiationMismatch,
                            ),
                        )
                    })?;
                    let token = candidate
                        .issue_parameter_effect_projection(
                            request.coordinate,
                            &request.expected,
                            request.source_projection,
                        )
                        .map_err(|invariant| {
                            AnalyzerCallCheckFailure::Invariant(
                                AnalyzerCallClientInvariant::constraint(source, invariant),
                            )
                        })?;
                    token.seal_actual(prepared.actual()).map_err(|invariant| {
                        AnalyzerCallCheckFailure::Invariant(
                            AnalyzerCallClientInvariant::constraint(source, invariant),
                        )
                    })?
                }
            };
            return Ok(AnalyzerCallObservedSource {
                actual,
                evidence: prepared.evidence(),
            });
        }
        if matches!(
            source,
            AnalyzerCallConstraintSource::Receiver { .. }
                | AnalyzerCallConstraintSource::DialogueApplicationMetadata { .. }
                | AnalyzerCallConstraintSource::DialogueApplicationOperand { .. }
                | AnalyzerCallConstraintSource::TextProxyObjectOperand { .. }
        ) {
            return Err(AnalyzerCallCheckFailure::Invariant(
                AnalyzerCallClientInvariant::constraint(
                    source,
                    CallConstraintInvariant::MalformedMapperSeal,
                ),
            ));
        }
        let scope = self.active_fact_scope.as_ref().ok_or_else(|| {
            AnalyzerCallCheckFailure::Invariant(AnalyzerCallClientInvariant::fact_transaction(
                source,
                crate::final_analysis::CandidateFactTransactionViolation::StaleCheckpoint,
            ))
        })?;
        let authority = self
            .analyzer
            .facts
            .callback_fact_authority(&scope.scope)
            .map_err(|violation| {
                AnalyzerCallCheckFailure::Invariant(AnalyzerCallClientInvariant::fact_transaction(
                    source, violation,
                ))
            })?;
        let child_context = self.context.child_candidate(authority);
        let child_context = if matches!(source, AnalyzerCallConstraintSource::Argument { .. })
            && self.candidate.as_ref().is_some_and(|candidate| {
                candidate.schema().validator()
                    == &crate::callable::CallableValidator::ViewModifier(
                        crate::callable::ViewModifierId::Fx,
                    )
            }) {
            child_context.with_consumer(
                super::super::expression_error::AnalyzerExpressionConsumer::ViewFxProducer,
            )
        } else {
            child_context
        };
        let result = self.analyzer.evaluate_call_constraint_source(
            &child_context,
            source,
            expectation,
            compile_time_scalar,
        );
        drop(child_context);
        match result {
            Ok(checked) => {
                let invalid_variant_evidence = || {
                    AnalyzerCallCheckFailure::Invariant(
                        AnalyzerCallClientInvariant::constraint(
                            source,
                            CallConstraintInvariant::Lower(
                                TypeConstraintInvariant::SourceProtocol(
                                    crate::types::constraints::TypeConstraintSourceProtocolInvariant::InvalidEvidence,
                                ),
                            ),
                        ),
                    )
                };
                let Some(checked_type) = checked.value_type() else {
                    return Err(AnalyzerCallCheckFailure::Mismatch);
                };
                let variant = match &checked {
                    crate::final_analysis::PreparedExpressionFact::Variant(prepared) => {
                        if &prepared.owner().ty() != checked_type
                        {
                            return Err(invalid_variant_evidence());
                        }
                        let index = usize::try_from(prepared.selected_ordinal())
                            .map_err(|_| invalid_variant_evidence())?;
                        let selected = prepared
                            .owner()
                            .cases()
                            .get(index)
                            .filter(|case| case.ordinal() == prepared.selected_ordinal())
                            .ok_or_else(invalid_variant_evidence)?;
                        Some((
                            selected.ordinal(),
                            if selected.payload().is_some() {
                                VariantPayloadRequirement::Present
                            } else {
                                VariantPayloadRequirement::Unit
                            },
                        ))
                    }
                    crate::final_analysis::PreparedExpressionFact::Complete(complete) => match
                        complete.resolution()
                    {
                        crate::final_analysis::CheckedExpressionResolution::Variant(variant) => {
                        if &variant.owner().ty() != checked_type
                        {
                            return Err(invalid_variant_evidence());
                        }
                        Some((
                            variant.ordinal(),
                            if !variant.selected().payload().is_unit() {
                                VariantPayloadRequirement::Present
                            } else {
                                VariantPayloadRequirement::Unit
                            },
                        ))
                        }
                        _ => None,
                    },
                    crate::final_analysis::PreparedExpressionFact::OwnerBound(_)
                    | crate::final_analysis::PreparedExpressionFact::DialogueApplication(_)
                    | crate::final_analysis::PreparedExpressionFact::ContentApplication(_)
                    | crate::final_analysis::PreparedExpressionFact::CompileTimeScalar(_)
                    | crate::final_analysis::PreparedExpressionFact::Method(_)
                    | crate::final_analysis::PreparedExpressionFact::Entry(_)
                    | crate::final_analysis::PreparedExpressionFact::ProjectField(_)
                    | crate::final_analysis::PreparedExpressionFact::ProjectRecord(_)
                    | crate::final_analysis::PreparedExpressionFact::ProjectNominalTypeValue(_) => {
                        None
                    }
                };
                let actual = match effect_projection {
                    None => checked_type.clone(),
                    Some(request) => {
                        let candidate = self.candidate.as_ref().ok_or_else(|| {
                            AnalyzerCallCheckFailure::Invariant(
                                AnalyzerCallClientInvariant::constraint(
                                    source,
                                    CallConstraintInvariant::PreparedEffectInstantiationMismatch,
                                ),
                            )
                        })?;
                        let token = candidate
                            .issue_parameter_effect_projection(
                                request.coordinate,
                                &request.expected,
                                request.source_projection,
                            )
                            .map_err(|invariant| {
                                AnalyzerCallCheckFailure::Invariant(
                                    AnalyzerCallClientInvariant::constraint(source, invariant),
                                )
                            })?;
                        token.seal_actual(checked_type).map_err(|invariant| {
                            AnalyzerCallCheckFailure::Invariant(
                                AnalyzerCallClientInvariant::constraint(source, invariant),
                            )
                        })?
                    }
                };
                let evidence = variant.map_or(
                    ObservedSemanticValueEvidence::NoVariantCase,
                    |(ordinal, payload)| ObservedSemanticValueEvidence::VariantCase {
                        owner: actual.clone(),
                        ordinal,
                        payload,
                    },
                );
                Ok(AnalyzerCallObservedSource { actual, evidence })
            }
            Err(crate::final_analysis::analyzer::expression_error::AnalyzerExpressionError::Rejected(_)) => {
                Err(AnalyzerCallCheckFailure::Mismatch)
            }
            Err(crate::final_analysis::analyzer::expression_error::AnalyzerExpressionError::Fatal(error)) => {
                Err(AnalyzerCallCheckFailure::Fatal(SourceError::new(
                    source,
                    phase,
                    AnalyzerCallSourceFailureCause::FinalSemantic(error),
                )))
            }
            Err(crate::final_analysis::analyzer::expression_error::AnalyzerExpressionError::Abort(error)) => {
                Err(AnalyzerCallCheckFailure::Abort(error))
            }
            Err(crate::final_analysis::analyzer::expression_error::AnalyzerExpressionError::Invariant(
                crate::final_analysis::analyzer::expression_error::AnalyzerExpressionInvariant::Fact(
                    violation,
                ),
            )) => Err(AnalyzerCallCheckFailure::Invariant(
                AnalyzerCallClientInvariant::fact_transaction(source, *violation),
            )),
            Err(crate::final_analysis::analyzer::expression_error::AnalyzerExpressionError::Invariant(
                crate::final_analysis::analyzer::expression_error::AnalyzerExpressionInvariant::Semantic(
                    error,
                ),
            )) => Err(AnalyzerCallCheckFailure::Invariant(
                AnalyzerCallClientInvariant::final_semantic(source, *error),
            )),
            Err(crate::final_analysis::analyzer::expression_error::AnalyzerExpressionError::Invariant(
                crate::final_analysis::analyzer::expression_error::AnalyzerExpressionInvariant::Cycle {
                    owner,
                },
            )) => Err(AnalyzerCallCheckFailure::Invariant(
                AnalyzerCallClientInvariant::final_semantic(
                    source,
                    crate::final_analysis::FinalSemanticAnalysisError::ExpressionCycle { owner },
                ),
            )),
            Err(crate::final_analysis::analyzer::expression_error::AnalyzerExpressionError::Invariant(
                crate::final_analysis::analyzer::expression_error::AnalyzerExpressionInvariant::CallFrame {
                    owner,
                    violation,
                },
            )) => Err(AnalyzerCallCheckFailure::Invariant(
                AnalyzerCallClientInvariant::call_frame(source, owner, *violation),
            )),
            Err(crate::final_analysis::analyzer::expression_error::AnalyzerExpressionError::Call {
                owner: inner_owner,
                failure,
            }) => match failure {
                CallAnalysisFailure::FatalSource(error) => {
                    Err(AnalyzerCallCheckFailure::Fatal(SourceError::new(
                        source,
                        phase,
                        AnalyzerCallSourceFailureCause::NestedCallFatal {
                            owner: inner_owner,
                            error: Box::new(error),
                        },
                    )))
                }
                CallAnalysisFailure::Abort(error) => Err(AnalyzerCallCheckFailure::Abort(error)),
                CallAnalysisFailure::Invariant(CallAnalysisInvariant::Client(error)) => {
                    Err(AnalyzerCallCheckFailure::Invariant(
                        AnalyzerCallClientInvariant::nested_call(
                            source,
                            inner_owner,
                            CallAnalysisInvariant::Client(error),
                        ),
                    ))
                }
                CallAnalysisFailure::Invariant(invariant) => Err(
                    AnalyzerCallCheckFailure::Invariant(
                        AnalyzerCallClientInvariant::nested_call(source, inner_owner, invariant),
                    ),
                ),
            },
        }
    }

    fn close_fact_failure(
        &mut self,
        failure: super::super::state::CandidateFactCloseFailure,
        coordinate: AnalyzerCallScopeCoordinate,
    ) -> crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain> {
        let cause = self
            .analyzer
            .facts
            .abort_callback_scope_close_failure(failure);
        crate::callable::SourceCheckpointFailure::client(
            AnalyzerCallClientInvariant::fact_transaction(coordinate.owner_source(), cause),
        )
    }
}

/// Client-side source operation capability.  It is intentionally narrower
/// than `Analyzer`; nested analysis can return a move-only client invariant
/// without publishing a side fact or borrowing resolver work recursively.
trait AnalyzerCallConstraintOperations {
    type ProbeCheckpoint;
    type MaterializationCheckpoint;
    type PreparedSealedBranchValue;

    fn probe_source<'h>(
        &mut self,
        source: AnalyzerCallConstraintSource,
        hint: ExpectedHint<'h, AnalyzerCallConstraintDomain>,
        checkpoint: &mut Self::ProbeCheckpoint,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
    ) -> Result<
        SourceProbeOutcome<AnalyzerCallConstraintDomain>,
        crate::callable::SourceCallbackFailure<AnalyzerCallConstraintDomain>,
    >;

    fn open_probe_checkpoint(
        &mut self,
        source: AnalyzerCallConstraintSource,
    ) -> Result<
        Self::ProbeCheckpoint,
        crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>,
    >;

    fn close_probe_checkpoint(
        &mut self,
        checkpoint: Self::ProbeCheckpoint,
    ) -> Result<(), crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>>;

    fn open_materialization_checkpoint(
        &mut self,
        sources: &[AnalyzerCallConstraintSource],
    ) -> Result<
        Self::MaterializationCheckpoint,
        crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>,
    >;

    fn materialize_sources<'h, I>(
        &mut self,
        sources: I,
        checkpoint: &mut Self::MaterializationCheckpoint,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
    ) -> Result<
        MaterializationOutcome<
            AnalyzerCallConstraintSource,
            Self::PreparedSealedBranchValue,
            AnalyzerCallSourceFailureCause,
        >,
        crate::callable::SourceCallbackFailure<AnalyzerCallConstraintDomain>,
    >
    where
        I: IntoIterator<Item = MaterializedSourceRequest<'h, AnalyzerCallConstraintDomain>>,
        CheckedSemanticValueEvidence: 'h,
        AnalyzerCallProbeSemanticBranch: 'h;

    fn close_materialization_checkpoint(
        &mut self,
        checkpoint: Self::MaterializationCheckpoint,
        sealed: Option<Self::PreparedSealedBranchValue>,
    ) -> Result<
        Option<AnalyzerCallSealedBranch>,
        crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>,
    >;

    fn finish(
        self,
    ) -> Result<(), crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>>;
}

impl<'a, 'project, 'catalog, 'control> AnalyzerCallConstraintOperations
    for AnalyzerCallExpressionClient<'a, 'project, 'catalog, 'control>
{
    type ProbeCheckpoint = AnalyzerProbeCheckpoint;
    type MaterializationCheckpoint = AnalyzerMaterializationCheckpoint;
    type PreparedSealedBranchValue = AnalyzerCallPreparedSealedBranch;

    fn probe_source<'h>(
        &mut self,
        source: AnalyzerCallConstraintSource,
        hint: ExpectedHint<'h, AnalyzerCallConstraintDomain>,
        checkpoint: &mut Self::ProbeCheckpoint,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
    ) -> Result<
        SourceProbeOutcome<AnalyzerCallConstraintDomain>,
        crate::callable::SourceCallbackFailure<AnalyzerCallConstraintDomain>,
    > {
        self.probe_checkpoint_check(source, checkpoint)?;
        self.admit_physical_source(source, SourcePhase::Probe, work)?;
        self.record_physical_source(source, Self::physical_expected(source, &hint))
            .map_err(|error| {
                crate::callable::SourceCallbackFailure::fatal(SourceError::new(
                    source,
                    SourcePhase::Probe,
                    AnalyzerCallSourceFailureCause::FinalSemantic(Box::new(error)),
                ))
            })?;
        let map_failure = |failure: AnalyzerCallCheckFailure| match failure {
            AnalyzerCallCheckFailure::Mismatch => {
                crate::callable::SourceCallbackFailure::fatal(SourceError::new(
                    source,
                    SourcePhase::Probe,
                    AnalyzerCallSourceFailureCause::Mismatch,
                ))
            }
            AnalyzerCallCheckFailure::Fatal(error) => {
                crate::callable::SourceCallbackFailure::fatal(error)
            }
            AnalyzerCallCheckFailure::Abort(error) => {
                crate::callable::SourceCallbackFailure::Abort(error)
            }
            AnalyzerCallCheckFailure::Invariant(invariant) => {
                crate::callable::SourceCallbackFailure::invariant(invariant)
            }
        };
        let admission = self.dialogue_patch_admissions.get(&source).cloned();
        let mut observed = None;
        if let Some(admission) = admission
            .as_ref()
            .filter(|admission| !admission.clearable())
        {
            let expected_projection = ParameterExpectedTypeProjection::ApplyUnary(
                crate::callable::CallableUnaryTypeConstructor::Option,
            );
            let effect_projection = AnalyzerCallEffectProjectionRequest {
                coordinate: admission.coordinate(),
                expected: expected_projection.clone(),
                source_projection: PreparedConstraintSourceProjection::Scalar,
            };
            let clear_expected = expected_projection.apply_to(admission.declared());
            match self.check_source(
                source,
                AnalyzerExpressionExpectation::from_complete(Some(&clear_expected)),
                Some(&effect_projection),
                None,
                SourcePhase::Probe,
            ) {
                Ok(checked) => {
                    let guard = CallableSemanticValueGuard::VariantCase {
                        owner: expected_projection,
                        ordinal: 1,
                        payload: VariantPayloadRequirement::Unit,
                    };
                    if guard.accepts_observation(admission.declared(), &checked.evidence) {
                        return Err(crate::callable::SourceCallbackFailure::fatal(
                            SourceError::new(
                                source,
                                SourcePhase::Probe,
                                AnalyzerCallSourceFailureCause::FinalSemantic(Box::new(
                                    admission.clear_failure(),
                                )),
                            ),
                        ));
                    }
                    observed = Some((checked.actual, checked.evidence));
                }
                Err(AnalyzerCallCheckFailure::Mismatch) => {}
                Err(failure) => return Err(map_failure(failure)),
            }
        }
        match hint {
            ExpectedHint::Unchecked => {
                match self.check_source(
                    source,
                    AnalyzerExpressionExpectation::Unconstrained,
                    None,
                    None,
                    SourcePhase::Probe,
                ) {
                    Ok(checked) => Ok(SourceProbeOutcome::Accepted(SourceProbeResult::unchecked(
                        checked.actual,
                        AnalyzerCallProbeSemanticBranch { source },
                    ))),
                    Err(AnalyzerCallCheckFailure::Mismatch) => Ok(SourceProbeOutcome::Rejected(
                        AnalyzerCallSourceFailureCause::Mismatch,
                    )),
                    Err(failure) => Err(map_failure(failure)),
                }
            }
            ExpectedHint::Alternatives(alternatives) => {
                for alternative in alternatives {
                    let expectation = match *alternative.value_expected() {
                        ProjectedExpectedHint::Complete(expected)
                            if alternative.source_projection().is_scalar() =>
                        {
                            AnalyzerExpressionExpectation::from_complete(Some(expected))
                        }
                        ProjectedExpectedHint::Parametric { expected, unbound }
                            if alternative.source_projection().is_scalar() =>
                        {
                            AnalyzerExpressionExpectation::parametric(expected, unbound)
                                .ok_or_else(|| {
                                    crate::callable::SourceCallbackFailure::invariant(
                                        AnalyzerCallClientInvariant::constraint(
                                            source,
                                            CallConstraintInvariant::Lower(
                                                TypeConstraintInvariant::SourceProtocol(
                                                    crate::types::constraints::TypeConstraintSourceProtocolInvariant::InvalidEvidence,
                                                ),
                                            ),
                                        ),
                                    )
                                })?
                        }
                        ProjectedExpectedHint::Complete(_)
                        | ProjectedExpectedHint::Parametric { .. } => {
                            AnalyzerExpressionExpectation::Unconstrained
                        }
                    };
                    match self.check_source(
                        source,
                        expectation,
                        alternative.evidence().effect_projection.as_ref(),
                        alternative.evidence().compile_time_scalar.as_deref(),
                        SourcePhase::Probe,
                    ) {
                        Ok(checked) => {
                            if alternative.evidence().accepts(&checked.evidence) {
                                return Ok(SourceProbeOutcome::Accepted(
                                    SourceProbeResult::checked(
                                        checked.actual,
                                        AnalyzerCallProbeSemanticBranch { source },
                                        alternative.alternative(),
                                        checked.evidence,
                                    ),
                                ));
                            }
                            if observed.is_none() {
                                observed = Some((checked.actual, checked.evidence));
                            }
                            continue;
                        }
                        Err(AnalyzerCallCheckFailure::Mismatch) => continue,
                        Err(failure) => return Err(map_failure(failure)),
                    }
                }
                if let Some(admission) = admission {
                    let failure = observed.map_or_else(
                        || crate::final_analysis::FinalSemanticAnalysisError::CharacterDialogueFieldType {
                            owner: admission.source(),
                        },
                        |(actual, _)| admission.mismatch_failure(actual),
                    );
                    return Err(crate::callable::SourceCallbackFailure::fatal(
                        SourceError::new(
                            source,
                            SourcePhase::Probe,
                            AnalyzerCallSourceFailureCause::FinalSemantic(Box::new(failure)),
                        ),
                    ));
                }
                Ok(SourceProbeOutcome::Rejected(
                    AnalyzerCallSourceFailureCause::Mismatch,
                ))
            }
        }
    }

    fn open_probe_checkpoint(
        &mut self,
        source: AnalyzerCallConstraintSource,
    ) -> Result<
        Self::ProbeCheckpoint,
        crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>,
    > {
        let requested = AnalyzerCallScopeCoordinate::Probe { source };
        if let Some(active) = self.active_fact_scope.as_ref() {
            return Err(crate::callable::SourceCheckpointFailure::client(
                AnalyzerCallClientInvariant::active_fact_scope_conflict(
                    active.coordinate.clone(),
                    requested,
                ),
            ));
        }
        let scope = self
            .analyzer
            .facts
            .open_callback_fact_scope()
            .map_err(|violation| {
                crate::callable::SourceCheckpointFailure::client(
                    AnalyzerCallClientInvariant::fact_transaction(source, violation),
                )
            })?;
        let checkpoint = AnalyzerProbeCheckpoint {
            checkpoint: scope.probe_checkpoint(),
            source,
        };
        self.active_fact_scope = Some(AnalyzerCallActiveFactScope {
            scope,
            coordinate: requested,
        });
        Ok(checkpoint)
    }

    fn close_probe_checkpoint(
        &mut self,
        checkpoint: Self::ProbeCheckpoint,
    ) -> Result<(), crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>> {
        let Some(active) = self.active_fact_scope.take() else {
            return Err(crate::callable::SourceCheckpointFailure::Protocol(
                crate::types::constraints::TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        };
        let coordinate = active.coordinate;
        let identity_ok = active
            .scope
            .matches_probe_checkpoint(&checkpoint.checkpoint);
        let coordinate_ok = matches!(
            &coordinate,
            AnalyzerCallScopeCoordinate::Probe { source }
                if *source == checkpoint.source
        );
        let close = self
            .analyzer
            .facts
            .rollback_callback_fact_scope(active.scope);
        if let Err(failure) = close {
            return Err(self.close_fact_failure(failure, coordinate));
        }
        if !identity_ok {
            return Err(crate::callable::SourceCheckpointFailure::Protocol(
                crate::types::constraints::TypeConstraintSourceProtocolInvariant::Checkpoint,
            ));
        }
        if !coordinate_ok {
            return Err(crate::callable::SourceCheckpointFailure::Protocol(
                crate::types::constraints::TypeConstraintSourceProtocolInvariant::WrongSource,
            ));
        }
        Ok(())
    }

    fn open_materialization_checkpoint(
        &mut self,
        sources: &[AnalyzerCallConstraintSource],
    ) -> Result<
        Self::MaterializationCheckpoint,
        crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>,
    > {
        let Some(owner) = sources.first().copied() else {
            return Err(crate::callable::SourceCheckpointFailure::Protocol(
                crate::types::constraints::TypeConstraintSourceProtocolInvariant::WrongSource,
            ));
        };
        let ordered_sources = sources.to_vec().into_boxed_slice();
        let coordinate = AnalyzerCallScopeCoordinate::Materialization {
            owner,
            sources: ordered_sources.clone(),
        };
        if let Some(active) = self.active_fact_scope.as_ref() {
            return Err(crate::callable::SourceCheckpointFailure::client(
                AnalyzerCallClientInvariant::active_fact_scope_conflict(
                    active.coordinate.clone(),
                    coordinate,
                ),
            ));
        }
        let scope = self
            .analyzer
            .facts
            .open_callback_fact_scope()
            .map_err(|violation| {
                crate::callable::SourceCheckpointFailure::client(
                    AnalyzerCallClientInvariant::fact_transaction(owner, violation),
                )
            })?;
        let checkpoint = AnalyzerMaterializationCheckpoint {
            checkpoint: scope.materialization_checkpoint(),
            sources: ordered_sources,
            next_source: 0,
        };
        self.active_fact_scope = Some(AnalyzerCallActiveFactScope { scope, coordinate });
        Ok(checkpoint)
    }

    fn materialize_sources<'h, I>(
        &mut self,
        sources: I,
        checkpoint: &mut Self::MaterializationCheckpoint,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
    ) -> Result<
        MaterializationOutcome<
            AnalyzerCallConstraintSource,
            Self::PreparedSealedBranchValue,
            AnalyzerCallSourceFailureCause,
        >,
        crate::callable::SourceCallbackFailure<AnalyzerCallConstraintDomain>,
    >
    where
        I: IntoIterator<Item = MaterializedSourceRequest<'h, AnalyzerCallConstraintDomain>>,
        CheckedSemanticValueEvidence: 'h,
        AnalyzerCallProbeSemanticBranch: 'h,
    {
        for request in sources {
            let source = *request.source();
            self.materialization_checkpoint_check(source, checkpoint)?;
            self.active_source_check(source, SourcePhase::Materialize)
                .map_err(crate::callable::SourceCallbackFailure::invariant)?;
            self.admit_physical_source(source, SourcePhase::Materialize, work)?;
            let expected =
                source
                    .physical_argument()
                    .map_or(
                        CandidateExpectedType::Unchecked,
                        |physical| match physical.kind() {
                            PhysicalArgumentEvaluationKind::TypedRestSpread => {
                                CandidateExpectedType::Unchecked
                            }
                            PhysicalArgumentEvaluationKind::Unmapped => {
                                CandidateExpectedType::Unmapped
                            }
                            PhysicalArgumentEvaluationKind::Authored
                            | PhysicalArgumentEvaluationKind::Recovered
                            | PhysicalArgumentEvaluationKind::FixedLiteralSpread => request
                                .expected()
                                .map_or(CandidateExpectedType::Unchecked, |expected| {
                                    CandidateExpectedType::Exact(expected.clone())
                                }),
                        },
                    );
            self.record_physical_source(source, expected)
                .map_err(|error| {
                    crate::callable::SourceCallbackFailure::fatal(SourceError::new(
                        source,
                        SourcePhase::Materialize,
                        AnalyzerCallSourceFailureCause::FinalSemantic(Box::new(error)),
                    ))
                })?;
            let expected = request.expected();
            let effect_projection = request
                .alternative()
                .and_then(|alternative| self.effect_projections.get(&(source, alternative)))
                .cloned();
            let compile_time_scalar = request.alternative().and_then(|alternative| {
                self.compile_time_scalar_admissions
                    .get(&(source, alternative))
                    .cloned()
            });
            match self.check_source(
                source,
                AnalyzerExpressionExpectation::from_complete(expected),
                effect_projection.as_ref(),
                compile_time_scalar.as_deref(),
                SourcePhase::Materialize,
            ) {
                Ok(checked) => {
                    if let Err(invariant) = validate_materialized_source_request(&request, &checked)
                    {
                        return Err(crate::callable::SourceCallbackFailure::invariant(
                            AnalyzerCallClientInvariant::constraint(
                                source,
                                CallConstraintInvariant::Lower(invariant),
                            ),
                        ));
                    }
                }
                Err(AnalyzerCallCheckFailure::Mismatch) => {
                    return Ok(MaterializationOutcome::Rejected {
                        source,
                        cause: AnalyzerCallSourceFailureCause::Mismatch,
                    });
                }
                Err(AnalyzerCallCheckFailure::Fatal(error)) => {
                    return Err(crate::callable::SourceCallbackFailure::fatal(error));
                }
                Err(AnalyzerCallCheckFailure::Abort(error)) => {
                    return Err(crate::callable::SourceCallbackFailure::Abort(error));
                }
                Err(AnalyzerCallCheckFailure::Invariant(invariant)) => {
                    return Err(crate::callable::SourceCallbackFailure::invariant(invariant));
                }
            }
        }
        Ok(MaterializationOutcome::Sealed(
            AnalyzerCallPreparedSealedBranch,
        ))
    }

    fn close_materialization_checkpoint(
        &mut self,
        checkpoint: Self::MaterializationCheckpoint,
        sealed: Option<Self::PreparedSealedBranchValue>,
    ) -> Result<
        Option<AnalyzerCallSealedBranch>,
        crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>,
    > {
        let Some(active) = self.active_fact_scope.take() else {
            return Err(crate::callable::SourceCheckpointFailure::Protocol(
                crate::types::constraints::TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        };
        let coordinate = active.coordinate;
        let identity_ok = active
            .scope
            .matches_materialization_checkpoint(&checkpoint.checkpoint);
        let coordinate_ok = matches!(
            &coordinate,
            AnalyzerCallScopeCoordinate::Materialization { sources, .. }
                if sources.as_ref() == checkpoint.sources.as_ref()
        );
        let ordered_sources_complete =
            sealed.is_none() || checkpoint.next_source == checkpoint.sources.len();
        if sealed.is_none() || !identity_ok || !coordinate_ok || !ordered_sources_complete {
            if let Err(failure) = self
                .analyzer
                .facts
                .rollback_callback_fact_scope(active.scope)
            {
                return Err(self.close_fact_failure(failure, coordinate));
            }
            if !identity_ok {
                return Err(crate::callable::SourceCheckpointFailure::Protocol(
                    crate::types::constraints::TypeConstraintSourceProtocolInvariant::Checkpoint,
                ));
            }
            if !coordinate_ok || !ordered_sources_complete {
                return Err(crate::callable::SourceCheckpointFailure::Protocol(
                    crate::types::constraints::TypeConstraintSourceProtocolInvariant::WrongSource,
                ));
            }
            return Ok(None);
        }
        let projection = match self
            .analyzer
            .facts
            .extract_callback_fact_scope(active.scope)
        {
            Ok(projection) => projection,
            Err(failure) => {
                return Err(self.close_fact_failure(failure, coordinate));
            }
        };
        let Some(_sealed) = sealed else {
            return Ok(None);
        };
        Ok(Some(AnalyzerCallSealedBranch::Materialized { projection }))
    }

    fn finish(
        mut self,
    ) -> Result<(), crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>> {
        let Some(active) = self.active_fact_scope.take() else {
            return Ok(());
        };
        let coordinate = active.coordinate;
        match self
            .analyzer
            .facts
            .rollback_callback_fact_scope(active.scope)
        {
            Ok(()) => Err(crate::callable::SourceCheckpointFailure::client(
                AnalyzerCallClientInvariant::active_fact_scope(coordinate),
            )),
            Err(failure) => Err(self.close_fact_failure(failure, coordinate)),
        }
    }
}

/// Affine callback client used by one candidate-wide driver.  The operation
/// capability is borrowed only for the callback phase and cannot reach the
/// graph or manufacture a lower expected type.
struct AnalyzerCallConstraintClient<O: AnalyzerCallConstraintOperations> {
    operations: O,
}

impl<O: AnalyzerCallConstraintOperations> AnalyzerCallConstraintClient<O> {
    fn new(operations: O) -> Self {
        Self { operations }
    }
}

impl<O: AnalyzerCallConstraintOperations>
    crate::callable::TypeConstraintClient<AnalyzerCallConstraintDomain>
    for AnalyzerCallConstraintClient<O>
{
    type ProbeCheckpoint = O::ProbeCheckpoint;
    type MaterializationCheckpoint = O::MaterializationCheckpoint;
    type PreparedSealedBranchValue = O::PreparedSealedBranchValue;

    fn probe_source<'h>(
        &mut self,
        source: AnalyzerCallConstraintSource,
        hint: ExpectedHint<'h, AnalyzerCallConstraintDomain>,
        checkpoint: &mut Self::ProbeCheckpoint,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
    ) -> Result<
        SourceProbeOutcome<AnalyzerCallConstraintDomain>,
        crate::callable::SourceCallbackFailure<AnalyzerCallConstraintDomain>,
    > {
        self.operations.probe_source(source, hint, checkpoint, work)
    }

    fn open_probe_checkpoint(
        &mut self,
        source: AnalyzerCallConstraintSource,
    ) -> Result<
        Self::ProbeCheckpoint,
        crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>,
    > {
        self.operations.open_probe_checkpoint(source)
    }

    fn close_probe_checkpoint(
        &mut self,
        checkpoint: Self::ProbeCheckpoint,
    ) -> Result<(), crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>> {
        self.operations.close_probe_checkpoint(checkpoint)
    }

    fn open_materialization_checkpoint(
        &mut self,
        sources: &[AnalyzerCallConstraintSource],
    ) -> Result<
        Self::MaterializationCheckpoint,
        crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>,
    > {
        self.operations.open_materialization_checkpoint(sources)
    }

    fn materialize_sources<'h, I>(
        &mut self,
        sources: I,
        checkpoint: &mut Self::MaterializationCheckpoint,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
    ) -> Result<
        MaterializationOutcome<
            AnalyzerCallConstraintSource,
            Self::PreparedSealedBranchValue,
            AnalyzerCallSourceFailureCause,
        >,
        crate::callable::SourceCallbackFailure<AnalyzerCallConstraintDomain>,
    >
    where
        I: IntoIterator<Item = MaterializedSourceRequest<'h, AnalyzerCallConstraintDomain>>,
        CheckedSemanticValueEvidence: 'h,
        AnalyzerCallProbeSemanticBranch: 'h,
    {
        self.operations
            .materialize_sources(sources, checkpoint, work)
    }

    fn close_materialization_checkpoint(
        &mut self,
        checkpoint: Self::MaterializationCheckpoint,
        sealed: Option<Self::PreparedSealedBranchValue>,
    ) -> Result<
        Option<AnalyzerCallSealedBranch>,
        crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>,
    > {
        self.operations
            .close_materialization_checkpoint(checkpoint, sealed)
    }

    fn finish(
        self,
    ) -> Result<(), crate::callable::SourceCheckpointFailure<AnalyzerCallConstraintDomain>> {
        self.operations.finish()
    }
}

struct PreparedCallTypeConstraint {
    source: AnalyzerCallConstraintSource,
    pattern: TypeKind,
    actual: TypeKind,
    acceptance: ConstraintAcceptance,
}

struct PreparedCallProjectionRequest {
    key: AnalyzerCallProjection,
    value: TypeKind,
    closure: crate::types::constraints::TypeConstraintProjectionClosure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AnalyzerCallConsumerAdmission {
    Ordinary,
    ViewFxProducer {
        candidate: CallableCandidateId,
        schema: crate::callable::CallableSignatureSchemaDigest,
        validator: CallableValidator,
        runtime_parameters: BTreeMap<CallableParameterCoordinate, TypeKind>,
    },
}

impl AnalyzerCallConsumerAdmission {
    pub(crate) const fn ordinary() -> Self {
        Self::Ordinary
    }

    pub(in crate::final_analysis::analyzer) fn view_fx_producer(
        candidate: &PreparedResolvedCallable,
        runtime_parameters: BTreeMap<CallableParameterCoordinate, TypeKind>,
    ) -> Self {
        Self::ViewFxProducer {
            candidate: candidate.id().clone(),
            schema: candidate.schema().semantic_digest(),
            validator: candidate.schema().validator().clone(),
            runtime_parameters,
        }
    }

    pub(crate) fn runtime_parameter(
        &self,
        coordinate: CallableParameterCoordinate,
    ) -> Option<&TypeKind> {
        match self {
            Self::Ordinary => None,
            Self::ViewFxProducer {
                runtime_parameters, ..
            } => runtime_parameters.get(&coordinate),
        }
    }

    fn runtime_parameters(&self) -> Option<&BTreeMap<CallableParameterCoordinate, TypeKind>> {
        match self {
            Self::Ordinary => None,
            Self::ViewFxProducer {
                runtime_parameters, ..
            } => Some(runtime_parameters),
        }
    }

    fn validates_candidate(&self, candidate: &PreparedResolvedCallable) -> bool {
        match self {
            Self::Ordinary => true,
            Self::ViewFxProducer {
                candidate: expected,
                schema,
                validator,
                runtime_parameters,
            } => {
                expected == candidate.id()
                    && *schema == candidate.schema().semantic_digest()
                    && validator == candidate.schema().validator()
                    && runtime_parameters.keys().all(|coordinate| {
                        coordinate.group() == candidate.call_group()
                            && candidate
                                .schema()
                                .group(coordinate.group())
                                .and_then(|group| group.parameter(coordinate.parameter()))
                                .is_some()
                    })
            }
        }
    }

    pub(crate) fn validates_resolved_candidate(
        &self,
        candidate: &crate::callable::ResolvedCallable,
    ) -> bool {
        match self {
            Self::Ordinary => true,
            Self::ViewFxProducer {
                candidate: expected,
                schema,
                validator,
                runtime_parameters,
            } => {
                expected == candidate.id()
                    && *schema == candidate.schema().semantic_digest()
                    && validator == candidate.schema().validator()
                    && runtime_parameters.keys().all(|coordinate| {
                        coordinate.group() == candidate.call_group()
                            && candidate
                                .schema()
                                .group(coordinate.group())
                                .and_then(|group| group.parameter(coordinate.parameter()))
                                .is_some()
                    })
            }
        }
    }

    pub(crate) fn checked_seal(
        &self,
        candidate: &crate::callable::ResolvedCallable,
    ) -> Result<crate::callable::CheckedCallConsumerAdmissionSeal, CallConstraintInvariant> {
        if !self.validates_resolved_candidate(candidate) {
            return Err(CallConstraintInvariant::MalformedMapperSeal);
        }
        match self {
            Self::Ordinary => Ok(crate::callable::CheckedCallConsumerAdmissionSeal::Ordinary),
            Self::ViewFxProducer {
                runtime_parameters, ..
            } => Ok(
                crate::callable::CheckedCallConsumerAdmissionSeal::ViewFxProducer {
                    runtime_parameters: runtime_parameters
                        .iter()
                        .map(|(coordinate, ty)| (*coordinate, ty.clone()))
                        .collect(),
                },
            ),
        }
    }
}

/// The only analyzer-owned pairing of a candidate, mapper seal, source plans,
/// direct equations, and lower initialization.  The runner consumes this
/// carrier as one affine value, so a caller cannot provide a token from one
/// candidate with the mapping or source plans from another.
pub(crate) struct PreparedCallConstraintSet {
    candidate: Arc<PreparedResolvedCallable>,
    consumer: AnalyzerCallConsumerAdmission,
    callee_inputs: PreparedCallCalleeConstraintInputs,
    inputs: PreparedCallInputs,
    source_groups: Box<[PreparedSourceConstraintGroup<AnalyzerCallConstraintDomain>]>,
    prepared_source_actuals: BTreeMap<AnalyzerCallConstraintSource, AnalyzerPreparedSourceActual>,
    dialogue_patch_admissions:
        BTreeMap<AnalyzerCallConstraintSource, AnalyzerPreparedDialoguePatchAdmission>,
    receiver_sources: Box<[PreparedSourceConstraint<AnalyzerCallConstraintDomain>]>,
    effect_projections:
        BTreeMap<(AnalyzerCallConstraintSource, u32), AnalyzerCallEffectProjectionRequest>,
    compile_time_scalar_admissions: BTreeMap<
        (AnalyzerCallConstraintSource, u32),
        Arc<crate::checked_compile_time::PreparedCompileTimeScalarAdmission>,
    >,
    base_constraints: Box<[PreparedCallTypeConstraint]>,
    definition_effects: Option<PreparedCallableEffectConstraint>,
    receiver_constraints: Box<[PreparedCallTypeConstraint]>,
    result_constraint: Option<PreparedCallTypeConstraint>,
    result_schema: CallableResultSchema,
    projection_requests: Box<[PreparedCallProjectionRequest]>,
    initialization: PreparedConstraintInitialization,
}

struct PreparedCallableEffectConstraint {
    projected: crate::effect_row::EffectRow,
    known: crate::effect_row::EffectRow,
}

/// One fully solved call transaction before the enclosing fact projection is
/// attached.  The runner owns this value until the candidate role is sealed.
/// Its payload stays heap-owned across nested source callbacks and selection.
pub(crate) struct RanCandidateTransaction {
    data: Box<RanCandidateTransactionData>,
}

struct RanCandidateTransactionData {
    candidate: Arc<PreparedResolvedCallable>,
    consumer: AnalyzerCallConsumerAdmission,
    callee_inputs: PreparedCallCalleeConstraintInputs,
    inputs: PreparedCallInputs,
    current_group: CallableGroupIndex,
    result: CallableResultSchema,
    solved: crate::types::constraints::SolvedCandidate<AnalyzerCallConstraintDomain>,
}

/// The selected candidate after the lower solution has been sealed into the
/// callable-owned application.  The application is the only authority for
/// selected callable, completed group, and projected result; the remaining
/// fields are evidence needed for replay and publication.
pub(crate) struct PreparedCallApplicationTransaction {
    data: Box<PreparedCallApplicationTransactionData>,
}

struct PreparedCallApplicationTransactionData {
    application: PreparedCallableApplication,
    consumer: AnalyzerCallConsumerAdmission,
    callee_inputs: PreparedCallCalleeConstraintInputs,
    inputs: PreparedCallInputs,
    sealed_branch: AnalyzerCallSealedBranch,
    closed_sources: Box<[ClosedConstraintProbe<AnalyzerCallConstraintDomain>]>,
    projections: Box<[KeyedConstraintProjection<AnalyzerCallProjection>]>,
}

pub(crate) struct PreparedCallArgumentSemanticProjection {
    action: CallableArgumentSemanticAction,
    inferred: TypeKind,
}

impl PreparedCallArgumentSemanticProjection {
    pub(crate) const fn action(&self) -> CallableArgumentSemanticAction {
        self.action
    }

    pub(crate) const fn inferred(&self) -> &TypeKind {
        &self.inferred
    }
}

impl RanCandidateTransaction {
    pub(crate) fn declared_exact_argument_matches(&self) -> usize {
        let mapping = self.data.inputs.mapping();
        self.data
            .solved
            .closed_sources
            .iter()
            .filter(|source| {
                let (AnalyzerCallConstraintSource::Argument { slot, .. }
                | AnalyzerCallConstraintSource::DialoguePatch { slot, .. }) = source.source()
                else {
                    return false;
                };
                mapping
                    .arguments()
                    .iter()
                    .flat_map(|argument| argument.slots().iter())
                    .find(|mapped| mapped.slot() == slot)
                    .and_then(|mapped| mapped.declared_expected())
                    == Some(source.actual())
            })
            .count()
    }

    pub(crate) fn exact_argument_matches(&self) -> usize {
        self.data
            .solved
            .closed_sources
            .iter()
            .filter(|source| {
                matches!(
                    source.source(),
                    AnalyzerCallConstraintSource::Argument { .. }
                        | AnalyzerCallConstraintSource::DialoguePatch { .. }
                ) && source.final_expected() == Some(source.actual())
            })
            .count()
    }

    /// Consume the complete lower transaction exactly once.  This is the
    /// sole analyzer-to-callable application sealing seam.
    pub(crate) fn into_prepared_application(
        self,
    ) -> Result<PreparedCallApplicationTransaction, CallConstraintInvariant> {
        let RanCandidateTransactionData {
            candidate,
            consumer,
            callee_inputs,
            inputs,
            current_group,
            result,
            solved,
        } = *self.data;
        let crate::types::constraints::SolvedCandidate {
            solution,
            sealed_branch,
            projections,
            closed_sources,
        } = solved;
        let application =
            PreparedCallableApplication::seal_from_selected_transaction(candidate, solution)?;
        let projected_result = application.result_schema()?;
        if application.completed_group() != current_group || projected_result != result {
            return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch);
        }
        Ok(PreparedCallApplicationTransaction {
            data: Box::new(PreparedCallApplicationTransactionData {
                application,
                consumer,
                callee_inputs,
                inputs,
                sealed_branch,
                closed_sources,
                projections,
            }),
        })
    }
}

impl PreparedCallApplicationTransaction {
    pub(crate) fn candidate(&self) -> &PreparedResolvedCallable {
        self.data.application.selected()
    }

    pub(crate) fn selected_shared(&self) -> &Arc<PreparedResolvedCallable> {
        self.data.application.selected_shared()
    }

    pub(crate) fn current_group(&self) -> CallableGroupIndex {
        self.data.application.completed_group()
    }

    pub(crate) fn result(&self) -> Result<CallableResultSchema, CallConstraintInvariant> {
        self.data.application.result_schema()
    }

    /// Project one authored scalar argument from the selected mapper/lower
    /// transaction. This is the sole pre-publication authority for semantic
    /// actions such as Dialogue patch clear/supply; callers never read a
    /// not-yet-applied candidate expression fact.
    pub(crate) fn argument_semantics(
        &self,
        argument: HirCallArgumentOrdinal,
        expression: ExprId,
    ) -> Result<PreparedCallArgumentSemanticProjection, CallConstraintInvariant> {
        let mapping = self.data.inputs.mapping();
        let mapped = mapping
            .arguments()
            .get(usize::from(argument.get()))
            .ok_or(CallConstraintInvariant::MalformedMapperSeal)?;
        let [slot] = mapped.slots() else {
            return Err(CallConstraintInvariant::MalformedMapperSeal);
        };
        if slot.source() != CheckedCallArgumentSlotSource::Expression(expression) {
            return Err(CallConstraintInvariant::MalformedMapperSeal);
        }
        let dialogue_patch_coordinate = slot.coordinate().filter(|coordinate| {
            self.data
                .application
                .selected()
                .schema()
                .group(coordinate.group())
                .and_then(|group| group.parameter(coordinate.parameter()))
                .is_some_and(|parameter| {
                    matches!(
                        parameter.consumer(),
                        CallableParameterConsumer::DialoguePatch(_)
                    )
                })
        });
        let source = dialogue_patch_coordinate.map_or(
            AnalyzerCallConstraintSource::Argument {
                argument,
                slot: slot.slot(),
                source: slot.source(),
                physical_kind: PhysicalArgumentEvaluationKind::Authored,
            },
            |coordinate| AnalyzerCallConstraintSource::DialoguePatch {
                argument,
                slot: slot.slot(),
                source: slot.source(),
                coordinate,
                physical_kind: PhysicalArgumentEvaluationKind::Authored,
            },
        );
        let closed = self
            .data
            .closed_sources
            .iter()
            .find(|closed| closed.source().same_argument_identity(source))
            .ok_or(CallConstraintInvariant::MalformedMapperSeal)?;
        let action = match slot.coordinate() {
            None => {
                if slot.open_argument().is_none() || !closed.selection().is_unchecked() {
                    return Err(CallConstraintInvariant::MalformedMapperSeal);
                }
                CallableArgumentSemanticAction::Supply
            }
            Some(coordinate) => {
                let parameter = self
                    .data
                    .application
                    .selected()
                    .schema()
                    .group(coordinate.group())
                    .and_then(|group| group.parameter(coordinate.parameter()))
                    .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
                match parameter.admission() {
                    CallableParameterAdmission::UncheckedSupply => {
                        if !closed.selection().is_unchecked() {
                            return Err(CallConstraintInvariant::MalformedMapperSeal);
                        }
                        CallableArgumentSemanticAction::Supply
                    }
                    CallableParameterAdmission::Checked { rule, .. } => {
                        let alternative = closed
                            .selection()
                            .alternative()
                            .ok_or(CallConstraintInvariant::MalformedMapperSeal)?;
                        let alternative = usize::try_from(alternative)
                            .map_err(|_| CallConstraintInvariant::MalformedMapperSeal)?;
                        rule.alternative(alternative)
                            .map(crate::callable::CallableParameterValueAlternative::action)
                            .ok_or(CallConstraintInvariant::MalformedMapperSeal)?
                    }
                    CallableParameterAdmission::Semantic(_) => {
                        return Err(CallConstraintInvariant::MalformedMapperSeal);
                    }
                }
            }
        };
        Ok(PreparedCallArgumentSemanticProjection {
            action,
            inferred: closed.actual().clone(),
        })
    }

    pub(crate) fn replay_mismatch(&self, other: &Self) -> Option<CallConstraintInvariant> {
        if !self.data.application.replay_eq(&other.data.application) {
            return Some(CallConstraintInvariant::ReplayApplicationMismatch);
        }
        if self.data.consumer != other.data.consumer {
            return Some(CallConstraintInvariant::ReplayArgumentMappingMismatch);
        }
        if self.data.callee_inputs != other.data.callee_inputs {
            return Some(CallConstraintInvariant::ReplayCalleeInputsMismatch);
        }
        if self.data.inputs != other.data.inputs {
            return Some(CallConstraintInvariant::ReplayArgumentMappingMismatch);
        }
        if let Some(mismatch) = self
            .data
            .sealed_branch
            .semantic_replay_mismatch(&other.data.sealed_branch)
        {
            return Some(mismatch);
        }
        if self.data.closed_sources != other.data.closed_sources {
            return Some(CallConstraintInvariant::ReplayClosedSourcesMismatch);
        }
        if self.data.projections != other.data.projections {
            return Some(CallConstraintInvariant::ReplayProjectionMismatch);
        }
        None
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedCallableApplication,
        AnalyzerCallConsumerAdmission,
        PreparedCallCalleeConstraintInputs,
        PreparedCallInputs,
        AnalyzerCallSealedBranch,
        Box<[ClosedConstraintProbe<AnalyzerCallConstraintDomain>]>,
        Box<[KeyedConstraintProjection<AnalyzerCallProjection>]>,
    ) {
        (
            self.data.application,
            self.data.consumer,
            self.data.callee_inputs,
            self.data.inputs,
            self.data.sealed_branch,
            self.data.closed_sources,
            self.data.projections,
        )
    }
}

/// A runner result becomes selectable only after its outer fact projection has
/// been extracted from the candidate checkpoint.  Keeping the projection in
/// this separate carrier prevents an accepted candidate from existing in an
/// unsealed state or from silently losing its projection.
pub(crate) struct SealedAcceptedCandidate {
    ran: RanCandidateTransaction,
    outer_projection: CandidateSemanticProjection,
}

impl SealedAcceptedCandidate {
    pub(in crate::final_analysis::analyzer) fn seal(
        ran: RanCandidateTransaction,
        outer_projection: CandidateSemanticProjection,
    ) -> Self {
        Self {
            ran,
            outer_projection,
        }
    }

    pub(crate) fn into_parts(self) -> (RanCandidateTransaction, CandidateSemanticProjection) {
        (self.ran, self.outer_projection)
    }
}

impl RanCandidateTransaction {
    pub(crate) fn result(&self) -> &CallableResultSchema {
        &self.data.result
    }

    /// Consume a completed lower transaction for deterministic unselected
    /// recovery. The callback branch remains affine and travels with the
    /// exact candidate/result that produced it; recovery cannot reconstruct
    /// contextual facts by evaluating HIR again.
    pub(crate) fn into_contextual_parts(
        self,
    ) -> (
        Arc<PreparedResolvedCallable>,
        CallableGroupIndex,
        CallableResultSchema,
        AnalyzerCallSealedBranch,
    ) {
        let RanCandidateTransactionData {
            candidate,
            current_group,
            result,
            solved,
            ..
        } = *self.data;
        (candidate, current_group, result, solved.sealed_branch)
    }
}

pub(crate) struct AnalyzerPreparedCallPrefix {
    site: CheckedCallSite,
    application: PreparedCallableApplication,
    record: AnalyzerPreparedCandidateRecord,
}

impl AnalyzerPreparedCallPrefix {
    pub(crate) fn new(
        site: CheckedCallSite,
        application: PreparedCallableApplication,
        record: AnalyzerPreparedCandidateRecord,
    ) -> Result<Self, CallConstraintInvariant> {
        let owner = match site {
            CheckedCallSite::HirCall(owner)
            | CheckedCallSite::AttachedContentApplication {
                expression: owner, ..
            } => owner,
        };
        if record.expression() != owner {
            return Err(CallConstraintInvariant::PreparedCallSiteMismatch);
        }
        Ok(Self {
            site,
            application,
            record,
        })
    }

    pub(crate) fn application(&self) -> &PreparedCallableApplication {
        &self.application
    }

    pub(crate) fn record(&self) -> &AnalyzerPreparedCandidateRecord {
        &self.record
    }

    /// Projects the complete HIR Call child inventory owned by this selected
    /// mapper/callee transaction.  Authored expressions come from mapper rows
    /// (including zero-slot spreads); the optional callee comes only from the
    /// typed callee inputs and the exact staged callee fact.
    pub(crate) fn selected_expression_inventory(
        &self,
    ) -> Result<HirSelectedCallExpressionInventory, CallConstraintInvariant> {
        if !matches!(self.site, CheckedCallSite::HirCall(_)) {
            return Err(CallConstraintInvariant::PreparedCallSiteMismatch);
        }
        let arguments = self
            .record
            .inputs()
            .mapping()
            .selected_expression_arguments()
            .ok_or(CallConstraintInvariant::MalformedMapperSeal)?;
        let requires_value_callee = self.application.selected().requires_value_callee();
        let callee = match &self.record.callee_inputs {
            PreparedCallCalleeConstraintInputs::ValueReceiver { source, .. } => {
                if requires_value_callee {
                    return Err(CallConstraintInvariant::PreparedBaseMismatch);
                }
                self.record
                    .metadata
                    .callee_expression
                    .semantic_expression()
                    .or(Some(*source))
            }
            PreparedCallCalleeConstraintInputs::FunctionValue { .. } => {
                if !requires_value_callee {
                    return Err(CallConstraintInvariant::PreparedBaseMismatch);
                }
                Some(
                    self.record
                        .metadata
                        .callee_expression
                        .semantic_expression()
                        .ok_or(CallConstraintInvariant::PreparedCallSiteMismatch)?,
                )
            }
            PreparedCallCalleeConstraintInputs::Free
            | PreparedCallCalleeConstraintInputs::EnumConstructor
            | PreparedCallCalleeConstraintInputs::AssociatedType { .. }
            | PreparedCallCalleeConstraintInputs::DialogueCallee => {
                if requires_value_callee {
                    return Err(CallConstraintInvariant::PreparedBaseMismatch);
                }
                self.record.metadata.callee_expression.semantic_expression()
            }
            PreparedCallCalleeConstraintInputs::DialogueApplication
            | PreparedCallCalleeConstraintInputs::StaticContentCallee(_)
            | PreparedCallCalleeConstraintInputs::NonCallable => {
                return Err(CallConstraintInvariant::PreparedCallSiteMismatch);
            }
        };
        Ok(HirSelectedCallExpressionInventory::with_argument_semantics(
            arguments, callee,
        ))
    }

    pub(crate) fn into_parts(
        self,
    ) -> (PreparedCallableApplication, AnalyzerPreparedCandidateRecord) {
        (self.application, self.record)
    }
}

impl PreparedCallPrefixPayload for AnalyzerPreparedCallPrefix {
    type Unselected = AnalyzerPreparedUnselectedCall;

    fn application(&self) -> &PreparedCallableApplication {
        &self.application
    }

    fn dependencies(&self) -> Box<[crate::callable::PreparedCallContinuationRef]> {
        self.record
            .inventory()
            .considered
            .iter()
            .filter_map(|candidate| match candidate {
                AnalyzerPreparedConsideredCandidate::Selected => {
                    self.application.selected().prepared_continuation()
                }
                AnalyzerPreparedConsideredCandidate::Other(candidate) => {
                    candidate.prepared_continuation()
                }
            })
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    fn validate_site(&self, site: CheckedCallSite) -> Result<(), CallConstraintInvariant> {
        (self.site == site
            && matches!(
                (site, self.record.expression()),
                (CheckedCallSite::HirCall(owner), expression)
                    | (
                        CheckedCallSite::AttachedContentApplication {
                            expression: owner,
                            ..
                        },
                        expression,
                    )
                    if owner == expression
            ))
        .then_some(())
        .ok_or(CallConstraintInvariant::PreparedCallSiteMismatch)
    }

    fn replay_mismatch(
        &self,
        other: &Self,
    ) -> Option<crate::callable::PreparedCallPrefixReplayMismatch> {
        if self.site != other.site {
            return Some(crate::callable::PreparedCallPrefixReplayMismatch::Site);
        }
        if let Some(mismatch) = self.application.replay_mismatch(&other.application) {
            return Some(crate::callable::PreparedCallPrefixReplayMismatch::Application(mismatch));
        }
        if self.record != other.record {
            return Some(crate::callable::PreparedCallPrefixReplayMismatch::Payload);
        }
        None
    }
}

/// Projection-free analyzer evidence retained by an unselected prepared graph
/// node until the final C sealer can consume every prepared callable.  Tied
/// ambiguous rows are IDs into the one owned `considered` inventory, so the
/// same prepared candidate is never duplicated across two vectors.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AnalyzerPreparedUnselectedCall {
    pub(crate) enclosing_callable: Option<arcweft_lang_hir::symbol::CallableDeclarationKey>,
    pub(crate) outcome: AnalyzerPreparedUnselectedOutcome,
    pub(crate) accounting: crate::callable::CallResolverAccountingReport,
    pub(crate) selected_expression_inventory: HirSelectedCallExpressionInventory,
}

pub(crate) type AnalyzerPreparedCallGraph =
    PreparedCallGraph<AnalyzerPreparedCallPrefix, AnalyzerPreparedUnselectedCall>;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum AnalyzerPreparedUnselectedOutcome {
    Ambiguous {
        callee: Option<crate::callable::CallCalleeClassificationFact>,
        considered: Vec<Arc<PreparedResolvedCallable>>,
        tied: Box<[CallableCandidateId]>,
    },
    Rejected {
        callee: Option<crate::callable::CallCalleeClassificationFact>,
        candidates: Vec<Arc<PreparedResolvedCallable>>,
    },
    NonCallable {
        callee: Option<crate::callable::CallCalleeClassificationFact>,
        source: crate::callable::NonCallableSource,
        ty: TypeKind,
    },
    Missing {
        callee: Option<crate::callable::CallCalleeClassificationFact>,
        kind: crate::callable::UnknownCallKind,
    },
}

impl AnalyzerPreparedUnselectedCall {
    pub(crate) fn selected_expression_inventory(&self) -> HirSelectedCallExpressionInventory {
        self.selected_expression_inventory.clone()
    }

    pub(crate) fn dependencies(&self) -> Box<[crate::callable::PreparedCallContinuationRef]> {
        let candidates: &[Arc<PreparedResolvedCallable>] = match &self.outcome {
            AnalyzerPreparedUnselectedOutcome::Ambiguous { considered, .. } => considered,
            AnalyzerPreparedUnselectedOutcome::Rejected { candidates, .. } => candidates,
            AnalyzerPreparedUnselectedOutcome::NonCallable { .. }
            | AnalyzerPreparedUnselectedOutcome::Missing { .. } => &[],
        };
        candidates
            .iter()
            .filter_map(|candidate| candidate.prepared_continuation().cloned())
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    pub(crate) fn detach(
        self,
        arena: &mut PreparedResolvedCallableDetachArena,
    ) -> Result<AnalyzerDetachedUnselectedCall, CallConstraintInvariant> {
        let outcome = match self.outcome {
            AnalyzerPreparedUnselectedOutcome::Ambiguous {
                callee,
                considered,
                tied,
            } => AnalyzerDetachedUnselectedOutcome::Ambiguous {
                callee,
                considered: considered
                    .into_iter()
                    .map(|candidate| arena.detach(candidate))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
                tied,
            },
            AnalyzerPreparedUnselectedOutcome::Rejected { callee, candidates } => {
                AnalyzerDetachedUnselectedOutcome::Rejected {
                    callee,
                    candidates: candidates
                        .into_iter()
                        .map(|candidate| arena.detach(candidate))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                }
            }
            AnalyzerPreparedUnselectedOutcome::NonCallable { callee, source, ty } => {
                AnalyzerDetachedUnselectedOutcome::NonCallable { callee, source, ty }
            }
            AnalyzerPreparedUnselectedOutcome::Missing { callee, kind } => {
                AnalyzerDetachedUnselectedOutcome::Missing { callee, kind }
            }
        };
        Ok(AnalyzerDetachedUnselectedCall {
            enclosing_callable: self.enclosing_callable,
            outcome,
            accounting: self.accounting,
            selected_expression_inventory: self.selected_expression_inventory,
        })
    }
}

pub(crate) struct AnalyzerDetachedUnselectedCall {
    pub(crate) enclosing_callable: Option<arcweft_lang_hir::symbol::CallableDeclarationKey>,
    pub(crate) outcome: AnalyzerDetachedUnselectedOutcome,
    pub(crate) accounting: crate::callable::CallResolverAccountingReport,
    pub(crate) selected_expression_inventory: HirSelectedCallExpressionInventory,
}

pub(crate) enum AnalyzerDetachedUnselectedOutcome {
    Ambiguous {
        callee: Option<crate::callable::CallCalleeClassificationFact>,
        considered: Box<[DetachedPreparedResolvedCallable]>,
        tied: Box<[CallableCandidateId]>,
    },
    Rejected {
        callee: Option<crate::callable::CallCalleeClassificationFact>,
        candidates: Box<[DetachedPreparedResolvedCallable]>,
    },
    NonCallable {
        callee: Option<crate::callable::CallCalleeClassificationFact>,
        source: crate::callable::NonCallableSource,
        ty: TypeKind,
    },
    Missing {
        callee: Option<crate::callable::CallCalleeClassificationFact>,
        kind: crate::callable::UnknownCallKind,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CallAnalysisInvariant {
    Constraint(CallConstraintInvariant),
    Client(Box<AnalyzerCallClientInvariant>),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CallAnalysisFailure {
    FatalSource(SourceError<AnalyzerCallConstraintSource, AnalyzerCallSourceFailureCause>),
    Abort(TypeConstraintAbort),
    Invariant(CallAnalysisInvariant),
}

pub(crate) type CallAnalysisResult<T> = Result<T, CallAnalysisFailure>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AnalyzerPreparedConsideredCandidate {
    Selected,
    Other(Arc<PreparedResolvedCallable>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AnalyzerPreparedCandidateInventory {
    considered: Vec<AnalyzerPreparedConsideredCandidate>,
}

impl AnalyzerPreparedCandidateInventory {
    pub(crate) fn from_considered(
        selected: &PreparedResolvedCallable,
        considered: Vec<Arc<PreparedResolvedCallable>>,
    ) -> Result<Self, CallConstraintInvariant> {
        let mut selected_count = 0;
        let considered = considered
            .into_iter()
            .map(|candidate| {
                if candidate.id() == selected.id() {
                    selected_count += 1;
                    AnalyzerPreparedConsideredCandidate::Selected
                } else {
                    AnalyzerPreparedConsideredCandidate::Other(candidate)
                }
            })
            .collect();
        (selected_count == 1)
            .then_some(Self { considered })
            .ok_or(CallConstraintInvariant::PreparedBaseMismatch)
    }

    pub(crate) fn detach(
        self,
        arena: &mut PreparedResolvedCallableDetachArena,
    ) -> Result<Box<[AnalyzerDetachedConsideredCandidate]>, CallConstraintInvariant> {
        self.considered
            .into_iter()
            .map(|candidate| match candidate {
                AnalyzerPreparedConsideredCandidate::Selected => {
                    Ok(AnalyzerDetachedConsideredCandidate::Selected)
                }
                AnalyzerPreparedConsideredCandidate::Other(candidate) => arena
                    .detach(candidate)
                    .map(AnalyzerDetachedConsideredCandidate::Other),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    }
}

pub(crate) enum AnalyzerDetachedConsideredCandidate {
    Selected,
    Other(DetachedPreparedResolvedCallable),
}

/// Exact prepared disposition of one source callee expression. Semantic graph
/// retention and post-C callable-type publication are distinct roles: a
/// Character/variant/type-receiver fact may be retained without ever being
/// rewritten as a callable value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AnalyzerPreparedCalleeExpression {
    None,
    RetainExisting { expression: ExprId },
    SealCallable { expression: ExprId },
}

impl AnalyzerPreparedCalleeExpression {
    pub(crate) const fn none() -> Self {
        Self::None
    }

    pub(crate) const fn semantic(expression: ExprId) -> Self {
        Self::RetainExisting { expression }
    }

    pub(crate) const fn callable(expression: ExprId) -> Self {
        Self::SealCallable { expression }
    }

    pub(crate) const fn semantic_expression(self) -> Option<ExprId> {
        match self {
            Self::None => None,
            Self::RetainExisting { expression } | Self::SealCallable { expression } => {
                Some(expression)
            }
        }
    }

    pub(crate) const fn callable_type_projection(self) -> Option<ExprId> {
        match self {
            Self::SealCallable { expression } => Some(expression),
            Self::None | Self::RetainExisting { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AnalyzerPreparedExpressionResolution {
    Complete(crate::final_analysis::CheckedExpressionResolution),
    DialogueApplication,
    ContentApplication,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AnalyzerPreparedCandidateMetadata {
    expression: ExprId,
    expression_resolution: AnalyzerPreparedExpressionResolution,
    callee_expression: AnalyzerPreparedCalleeExpression,
    enclosing_callable: Option<arcweft_lang_hir::symbol::CallableDeclarationKey>,
    inventory: AnalyzerPreparedCandidateInventory,
    function_value_origin: Option<PreparedFunctionValueOriginEvidence>,
    accounting: crate::callable::CallResolverAccountingReport,
}

impl AnalyzerPreparedCandidateMetadata {
    pub(crate) fn new(
        expression: ExprId,
        expression_resolution: AnalyzerPreparedExpressionResolution,
        callee_expression: AnalyzerPreparedCalleeExpression,
        enclosing_callable: Option<arcweft_lang_hir::symbol::CallableDeclarationKey>,
        inventory: AnalyzerPreparedCandidateInventory,
        function_value_origin: Option<PreparedFunctionValueOriginEvidence>,
        accounting: crate::callable::CallResolverAccountingReport,
    ) -> Self {
        Self {
            expression,
            expression_resolution,
            callee_expression,
            enclosing_callable,
            inventory,
            function_value_origin,
            accounting,
        }
    }
}

/// Projection-free selected transaction evidence.  Semantic projections have
/// already been consumed by atomic graph publication; every other typed lower
/// product remains owned here until the final C sealer validates and consumes
/// it.
#[derive(Eq, PartialEq)]
pub(crate) struct AnalyzerPreparedCandidateRecord {
    metadata: AnalyzerPreparedCandidateMetadata,
    consumer: AnalyzerCallConsumerAdmission,
    callee_inputs: PreparedCallCalleeConstraintInputs,
    inputs: PreparedCallInputs,
    closed_sources: Box<[ClosedConstraintProbe<AnalyzerCallConstraintDomain>]>,
}

impl AnalyzerPreparedCandidateRecord {
    pub(in crate::final_analysis::analyzer) fn seal(
        metadata: AnalyzerPreparedCandidateMetadata,
        selected: &PreparedResolvedCallable,
        consumer: AnalyzerCallConsumerAdmission,
        callee_inputs: PreparedCallCalleeConstraintInputs,
        inputs: PreparedCallInputs,
        closed_sources: Box<[ClosedConstraintProbe<AnalyzerCallConstraintDomain>]>,
    ) -> Result<Self, CallConstraintInvariant> {
        let semantic_operands = inputs.semantic_operands();
        let is_dialogue_application = matches!(
            callee_inputs,
            PreparedCallCalleeConstraintInputs::DialogueApplication
        );
        let is_static_content_callee = matches!(
            callee_inputs,
            PreparedCallCalleeConstraintInputs::StaticContentCallee(_)
        );
        if !consumer.validates_candidate(selected)
            || inputs.candidate() != Some(selected.id())
            || !static_content_callee_matches(&callee_inputs, selected.id(), inputs.schema())
            || (is_dialogue_application && semantic_operands.is_empty())
            || (!is_dialogue_application
                && !is_static_content_callee
                && !semantic_operands.is_empty())
            || (is_dialogue_application
                && semantic_operands.iter().any(|operand| {
                    operand.owner() != PreparedCallSemanticOperandOwner::DialogueApplication
                }))
            || (is_static_content_callee
                && (semantic_operands.len() != 1
                    || semantic_operands.iter().any(|operand| {
                        operand.owner() != PreparedCallSemanticOperandOwner::TextProxyObject
                            || operand.role()
                                != PreparedCallSemanticOperandRole::TextProxyNominalDiscriminator
                            || operand.argument().is_none()
                    })))
        {
            return Err(CallConstraintInvariant::MalformedMapperSeal);
        }
        Ok(Self {
            metadata,
            consumer,
            callee_inputs,
            inputs,
            closed_sources,
        })
    }

    pub(crate) const fn expression(&self) -> ExprId {
        self.metadata.expression
    }

    pub(crate) fn inventory(&self) -> &AnalyzerPreparedCandidateInventory {
        &self.metadata.inventory
    }

    pub(crate) const fn inputs(&self) -> &PreparedCallInputs {
        &self.inputs
    }

    pub(crate) fn function_value_origin(&self) -> Option<&PreparedFunctionValueOriginEvidence> {
        self.metadata.function_value_origin.as_ref()
    }

    pub(crate) fn into_parts(self) -> AnalyzerPreparedCandidateRecordParts {
        let Self {
            metadata,
            consumer,
            callee_inputs,
            inputs,
            closed_sources,
        } = self;
        AnalyzerPreparedCandidateRecordParts {
            expression: metadata.expression,
            expression_resolution: metadata.expression_resolution,
            callee_expression: metadata.callee_expression,
            enclosing_callable: metadata.enclosing_callable,
            inventory: metadata.inventory,
            accounting: metadata.accounting,
            consumer,
            callee_inputs,
            inputs,
            closed_sources,
        }
    }
}

pub(crate) struct AnalyzerPreparedCandidateRecordParts {
    pub(crate) expression: ExprId,
    pub(crate) expression_resolution: AnalyzerPreparedExpressionResolution,
    pub(crate) callee_expression: AnalyzerPreparedCalleeExpression,
    pub(crate) enclosing_callable: Option<arcweft_lang_hir::symbol::CallableDeclarationKey>,
    pub(crate) inventory: AnalyzerPreparedCandidateInventory,
    pub(crate) accounting: crate::callable::CallResolverAccountingReport,
    pub(crate) consumer: AnalyzerCallConsumerAdmission,
    pub(crate) callee_inputs: PreparedCallCalleeConstraintInputs,
    pub(crate) inputs: PreparedCallInputs,
    pub(in crate::final_analysis::analyzer) closed_sources:
        Box<[ClosedConstraintProbe<AnalyzerCallConstraintDomain>]>,
}

impl AnalyzerPreparedCandidateRecordParts {
    pub(crate) fn detach(
        self,
        arena: &mut PreparedResolvedCallableDetachArena,
    ) -> Result<AnalyzerDetachedCandidateRecord, CallConstraintInvariant> {
        Ok(AnalyzerDetachedCandidateRecord {
            expression: self.expression,
            expression_resolution: self.expression_resolution,
            callee_expression: self.callee_expression,
            enclosing_callable: self.enclosing_callable,
            inventory: self.inventory.detach(arena)?,
            accounting: self.accounting,
            consumer: self.consumer,
            callee_inputs: self.callee_inputs,
            inputs: self.inputs,
            closed_sources: self.closed_sources,
        })
    }
}

/// Fully detached projection-free analyzer record.  Every prepared candidate
/// is now either the unique selected marker or an arena-owned opaque
/// definition reference; no `Arc<PreparedResolvedCallable>` survives.
pub(crate) struct AnalyzerDetachedCandidateRecord {
    pub(crate) expression: ExprId,
    pub(crate) expression_resolution: AnalyzerPreparedExpressionResolution,
    pub(crate) callee_expression: AnalyzerPreparedCalleeExpression,
    pub(crate) enclosing_callable: Option<arcweft_lang_hir::symbol::CallableDeclarationKey>,
    pub(crate) inventory: Box<[AnalyzerDetachedConsideredCandidate]>,
    pub(crate) accounting: crate::callable::CallResolverAccountingReport,
    pub(crate) consumer: AnalyzerCallConsumerAdmission,
    pub(crate) callee_inputs: PreparedCallCalleeConstraintInputs,
    pub(crate) inputs: PreparedCallInputs,
    pub(in crate::final_analysis::analyzer) closed_sources:
        Box<[ClosedConstraintProbe<AnalyzerCallConstraintDomain>]>,
}

/// The single analyzer preparation gate. It seals one composite prepared-input
/// inventory containing the ordinary mapper and any schema-owned semantic
/// operands, then asks the callable graph issuer for the exact lower scope/seed
/// token before any driver callback or accounting charge is possible.
pub(crate) fn validate_and_prepare_call_constraints(
    graph: &AnalyzerPreparedCallGraph,
    candidate: Arc<PreparedResolvedCallable>,
    inputs: PreparedCallInputs,
    authored_arguments: &[arcweft_lang_hir::expr::HirCallArgument],
    expected_result: Option<&TypeKind>,
    result_source: ExprId,
    callee_inputs: PreparedCallCalleeConstraintInputs,
    dialogue_patch_admissions: &[AnalyzerPreparedDialoguePatchAdmission],
    compile_time_scalar_admissions: &[AnalyzerPreparedCompileTimeScalarAdmission],
    scalar_types: &crate::registration::RegisteredCompileTimeScalarTypes,
    consumer: AnalyzerCallConsumerAdmission,
    enclosing: &EnclosingGenericParameterScope,
    known_callable_effects: Option<&crate::effects::EffectSet>,
) -> CallAnalysisResult<PreparedCallConstraintSet> {
    let group = candidate.call_group();
    if inputs.candidate() != Some(candidate.id())
        || inputs.group() != group
        || inputs.schema() != candidate.schema().semantic_digest()
    {
        return Err(CallAnalysisFailure::Invariant(
            CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
        ));
    }
    let initialization = graph
        .validate_and_issue_constraint_initialization(&candidate, enclosing)
        .map_err(|error| {
            CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error))
        })?;
    let future_parameters = initialization.future_parameters().to_vec();
    let definition_effects = known_callable_effects
        .filter(|_| candidate.schema().effects().fixed_row().is_none())
        .map(|known| {
            candidate
                .constraint_callable_effects()
                .map(|projected| PreparedCallableEffectConstraint {
                    projected,
                    known: crate::effect_row::EffectRow::closed(known.clone()),
                })
                .map_err(|error| {
                    CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error))
                })
        })
        .transpose()?;
    if !consumer.validates_candidate(&candidate) {
        return Err(CallAnalysisFailure::Invariant(
            CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
        ));
    }
    let view_fx_runtime_parameters = consumer.runtime_parameters();
    let mut compile_time_scalar_admission_map = BTreeMap::new();
    for row in compile_time_scalar_admissions {
        if compile_time_scalar_admission_map
            .insert(row.coordinate(), Arc::clone(row.admission()))
            .is_some()
        {
            return Err(CallAnalysisFailure::Invariant(
                CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
            ));
        }
    }
    let mut dialogue_patch_admission_map = BTreeMap::new();
    let (source_groups, mut prepared_source_actuals) = if inputs.semantic_operands().is_empty() {
        if !compile_time_scalar_admission_map.is_empty() {
            return Err(CallAnalysisFailure::Invariant(
                CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
            ));
        }
        let mapping = inputs.mapping();
        if matches!(
            callee_inputs,
            PreparedCallCalleeConstraintInputs::DialogueApplication
        ) || mapping.arguments().len() != authored_arguments.len()
        {
            return Err(CallAnalysisFailure::Invariant(
                CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
            ));
        }
        let mut actuals = BTreeMap::new();
        let mut groups = Vec::with_capacity(mapping.arguments().len());
        for (argument_index, argument) in mapping.arguments().iter().enumerate() {
            let ordinal = HirCallArgumentOrdinal::try_from_usize(argument_index).map_err(|_| {
                CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                    CallConstraintInvariant::MalformedMapperSeal,
                ))
            })?;
            let authored =
                authored_arguments
                    .get(argument_index)
                    .ok_or(CallAnalysisFailure::Invariant(
                        CallAnalysisInvariant::Constraint(
                            CallConstraintInvariant::MalformedMapperSeal,
                        ),
                    ))?;
            let mut sources = Vec::with_capacity(argument.slots().len());
            for slot in argument.slots() {
                let consumer = slot.coordinate().and_then(|coordinate| {
                    candidate
                        .schema()
                        .group(coordinate.group())
                        .and_then(|group| group.parameter(coordinate.parameter()))
                        .map(|parameter| (coordinate, parameter.consumer()))
                });
                let metadata_coordinate = match consumer {
                    Some((_, crate::callable::CallableParameterConsumer::Content(_))) => {
                        return Err(CallAnalysisFailure::Invariant(
                            CallAnalysisInvariant::Constraint(
                                CallConstraintInvariant::MalformedMapperSeal,
                            ),
                        ));
                    }
                    Some((
                        _,
                        crate::callable::CallableParameterConsumer::DialogueApplicationMetadata(
                            coordinate,
                        ),
                    )) => Some(*coordinate),
                    Some((_, crate::callable::CallableParameterConsumer::Value))
                    | Some((_, crate::callable::CallableParameterConsumer::DialoguePatch(_)))
                    | None => None,
                };
                let dialogue_patch_coordinate = match consumer {
                    Some((_, crate::callable::CallableParameterConsumer::DialoguePatch(_))) => {
                        slot.coordinate()
                    }
                    Some((_, crate::callable::CallableParameterConsumer::Content(_))) => {
                        return Err(CallAnalysisFailure::Invariant(
                            CallAnalysisInvariant::Constraint(
                                CallConstraintInvariant::MalformedMapperSeal,
                            ),
                        ));
                    }
                    Some((
                        _,
                        crate::callable::CallableParameterConsumer::Value
                        | crate::callable::CallableParameterConsumer::DialogueApplicationMetadata(_),
                    ))
                    | None => None,
                };
                let source = if let Some(coordinate) = metadata_coordinate {
                    let CheckedCallArgumentSlotSource::Expression(source) = slot.source() else {
                        return Err(CallAnalysisFailure::Invariant(
                            CallAnalysisInvariant::Constraint(
                                CallConstraintInvariant::MalformedMapperSeal,
                            ),
                        ));
                    };
                    let mut rows = mapping
                        .dialogue_application_metadata()
                        .iter()
                        .filter(|row| {
                            row.argument() == ordinal
                                && row.source() == source
                                && row.coordinate() == coordinate
                        });
                    let row = rows.next().cloned().ok_or_else(|| {
                        CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            CallConstraintInvariant::MalformedMapperSeal,
                        ))
                    })?;
                    if rows.next().is_some() {
                        return Err(CallAnalysisFailure::Invariant(
                            CallAnalysisInvariant::Constraint(
                                CallConstraintInvariant::MalformedMapperSeal,
                            ),
                        ));
                    }
                    let source = AnalyzerCallConstraintSource::DialogueApplicationMetadata {
                        argument: ordinal,
                        slot: slot.slot(),
                        source,
                        coordinate,
                    };
                    if actuals
                        .insert(
                            source,
                            AnalyzerPreparedSourceActual::DialogueApplicationMetadata(row),
                        )
                        .is_some()
                    {
                        return Err(CallAnalysisFailure::Invariant(
                            CallAnalysisInvariant::Constraint(
                                CallConstraintInvariant::MalformedMapperSeal,
                            ),
                        ));
                    }
                    source
                } else if let Some(coordinate) = dialogue_patch_coordinate {
                    let source = AnalyzerCallConstraintSource::DialoguePatch {
                        argument: ordinal,
                        slot: slot.slot(),
                        source: slot.source(),
                        coordinate,
                        physical_kind: super::semantics::physical_evaluation_kind(
                            authored,
                            slot,
                            false,
                            candidate.schema().argument_policy().spread(),
                        ),
                    };
                    let mut rows = dialogue_patch_admissions.iter().filter(|row| {
                        row.argument() == ordinal
                            && slot.source()
                                == CheckedCallArgumentSlotSource::Expression(row.source())
                            && row.coordinate() == coordinate
                    });
                    if let Some(row) = rows.next().cloned() {
                        let parameter = candidate
                            .schema()
                            .group(coordinate.group())
                            .and_then(|group| group.parameter(coordinate.parameter()))
                            .ok_or(CallAnalysisFailure::Invariant(
                                CallAnalysisInvariant::Constraint(
                                    CallConstraintInvariant::MalformedSchemaInventory,
                                ),
                            ))?;
                        if rows.next().is_some()
                            || !row.validates_parameter(coordinate, parameter)
                            || dialogue_patch_admission_map.insert(source, row).is_some()
                        {
                            return Err(CallAnalysisFailure::Invariant(
                                CallAnalysisInvariant::Constraint(
                                    CallConstraintInvariant::MalformedMapperSeal,
                                ),
                            ));
                        }
                    }
                    source
                } else {
                    AnalyzerCallConstraintSource::Argument {
                        argument: ordinal,
                        slot: slot.slot(),
                        source: slot.source(),
                        physical_kind: super::semantics::physical_evaluation_kind(
                            authored,
                            slot,
                            false,
                            candidate.schema().argument_policy().spread(),
                        ),
                    }
                };
                sources.push(prepare_source_constraint(
                    &candidate,
                    source,
                    slot,
                    scalar_types,
                    view_fx_runtime_parameters,
                )?);
            }
            groups.push(
                PreparedSourceConstraintGroup::seal(sources).map_err(|error| {
                    CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        CallConstraintInvariant::Lower(error),
                    ))
                })?,
            );
        }
        if dialogue_patch_admission_map.len() != dialogue_patch_admissions.len() {
            return Err(CallAnalysisFailure::Invariant(
                CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
            ));
        }
        (groups.into_boxed_slice(), actuals)
    } else {
        let semantic_operands = inputs.semantic_operands();
        let is_dialogue_application = matches!(
            callee_inputs,
            PreparedCallCalleeConstraintInputs::DialogueApplication
        );
        let is_static_content_callee = matches!(
            callee_inputs,
            PreparedCallCalleeConstraintInputs::StaticContentCallee(_)
        );
        if (!is_dialogue_application && !is_static_content_callee)
            || !inputs.validates(&candidate)
            || !callee_inputs.validates_candidate(&candidate)
            || (is_dialogue_application
                && (!authored_arguments.is_empty()
                    || !inputs.mapping().arguments().is_empty()
                    || !inputs.mapping().dialogue_application_metadata().is_empty()
                    || semantic_operands.iter().any(|operand| {
                        operand.owner() != PreparedCallSemanticOperandOwner::DialogueApplication
                    })))
            || (is_static_content_callee
                && (inputs.mapping().arguments().len() != authored_arguments.len()
                    || inputs
                        .mapping()
                        .dialogue_application_metadata()
                        .iter()
                        .next()
                        .is_some()
                    || semantic_operands.len() != 1
                    || !matches!(
                        semantic_operands.first(),
                        Some(operand)
                            if operand.owner() == PreparedCallSemanticOperandOwner::TextProxyObject
                                && operand.role()
                                    == PreparedCallSemanticOperandRole::TextProxyNominalDiscriminator
                                && operand.argument().is_some()
                    )))
        {
            return Err(CallAnalysisFailure::Invariant(
                CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
            ));
        }
        let mut actuals = BTreeMap::new();
        let mut groups = Vec::new();
        if is_dialogue_application {
            if !compile_time_scalar_admission_map.is_empty() {
                return Err(CallAnalysisFailure::Invariant(
                    CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
                ));
            }
            groups.reserve(semantic_operands.len());
            for operand in semantic_operands {
                let source = AnalyzerCallConstraintSource::DialogueApplicationOperand {
                    owner: operand.owner(),
                    source: operand.source(),
                    role: operand.role(),
                    coordinate: operand.coordinate(),
                };
                if actuals
                    .insert(
                        source,
                        AnalyzerPreparedSourceActual::DialogueApplicationOperand(
                            operand.actual().clone(),
                        ),
                    )
                    .is_some()
                {
                    return Err(CallAnalysisFailure::Invariant(
                        CallAnalysisInvariant::Constraint(
                            CallConstraintInvariant::MalformedMapperSeal,
                        ),
                    ));
                }
                let prepared = prepare_parameter_source_constraint(
                    &candidate,
                    source,
                    operand.coordinate(),
                    PreparedConstraintSourceProjection::Scalar,
                    scalar_types,
                    view_fx_runtime_parameters,
                )?;
                groups.push(
                    PreparedSourceConstraintGroup::seal([prepared]).map_err(|error| {
                        CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            CallConstraintInvariant::Lower(error),
                        ))
                    })?,
                );
            }
        } else {
            let discriminator = semantic_operands
                .first()
                .ok_or(CallAnalysisFailure::Invariant(
                    CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
                ))?;
            let crate::callable::CallableCandidateId::Content(
                crate::callable::ContentCallableIdentity::TextProxyObject { owner, .. },
            ) = candidate.id()
            else {
                return Err(CallAnalysisFailure::Invariant(
                    CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
                ));
            };
            let actual_owner =
                discriminator
                    .actual()
                    .semantic_identity_digest()
                    .map_err(|error| {
                        CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            error.into(),
                        ))
                    })?;
            if actual_owner != *owner {
                return Err(CallAnalysisFailure::Invariant(
                    CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
                ));
            }
            let discriminator_argument =
                discriminator
                    .argument()
                    .ok_or(CallAnalysisFailure::Invariant(
                        CallAnalysisInvariant::Constraint(
                            CallConstraintInvariant::MalformedMapperSeal,
                        ),
                    ))?;
            let discriminator_source = AnalyzerCallConstraintSource::TextProxyObjectOperand {
                argument: discriminator_argument,
                source: discriminator.source(),
                coordinate: discriminator.coordinate(),
            };
            let Some(parameter) = candidate
                .schema()
                .group(discriminator.coordinate().group())
                .and_then(|group| group.parameter(discriminator.coordinate().parameter()))
            else {
                return Err(CallAnalysisFailure::Invariant(
                    CallAnalysisInvariant::Constraint(
                        CallConstraintInvariant::MalformedSchemaInventory,
                    ),
                ));
            };
            if !matches!(
                parameter.consumer(),
                crate::callable::CallableParameterConsumer::Content(
                    crate::callable::CallableContentParameterConsumer::ObjectType
                )
            ) || !matches!(
                parameter.admission(),
                CallableParameterAdmission::Semantic(
                    crate::callable::CallableSemanticAdmission::TextProxyNominal
                )
            ) {
                return Err(CallAnalysisFailure::Invariant(
                    CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
                ));
            }
            if actuals
                .insert(
                    discriminator_source,
                    AnalyzerPreparedSourceActual::TextProxyObjectOperand(
                        discriminator.actual().clone(),
                    ),
                )
                .is_some()
            {
                return Err(CallAnalysisFailure::Invariant(
                    CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
                ));
            }
            for (argument_index, argument) in inputs.mapping().arguments().iter().enumerate() {
                let ordinal =
                    HirCallArgumentOrdinal::try_from_usize(argument_index).map_err(|_| {
                        CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                            CallConstraintInvariant::MalformedMapperSeal,
                        ))
                    })?;
                let authored = authored_arguments.get(argument_index).ok_or(
                    CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        CallConstraintInvariant::MalformedMapperSeal,
                    )),
                )?;
                for slot in argument.slots() {
                    let coordinate = slot.coordinate().ok_or(CallAnalysisFailure::Invariant(
                        CallAnalysisInvariant::Constraint(
                            CallConstraintInvariant::MalformedMapperSeal,
                        ),
                    ))?;
                    let parameter = candidate
                        .schema()
                        .group(coordinate.group())
                        .and_then(|group| group.parameter(coordinate.parameter()))
                        .ok_or(CallAnalysisFailure::Invariant(
                            CallAnalysisInvariant::Constraint(
                                CallConstraintInvariant::MalformedSchemaInventory,
                            ),
                        ))?;
                    if !matches!(
                        parameter.consumer(),
                        crate::callable::CallableParameterConsumer::Content(_)
                    ) {
                        return Err(CallAnalysisFailure::Invariant(
                            CallAnalysisInvariant::Constraint(
                                CallConstraintInvariant::MalformedMapperSeal,
                            ),
                        ));
                    }
                    let source = if matches!(
                        parameter.consumer(),
                        crate::callable::CallableParameterConsumer::Content(
                            crate::callable::CallableContentParameterConsumer::ObjectType
                        )
                    ) {
                        if ordinal != discriminator_argument
                            || slot.source()
                                != CheckedCallArgumentSlotSource::Expression(discriminator.source())
                            || coordinate != discriminator.coordinate()
                        {
                            return Err(CallAnalysisFailure::Invariant(
                                CallAnalysisInvariant::Constraint(
                                    CallConstraintInvariant::MalformedMapperSeal,
                                ),
                            ));
                        }
                        discriminator_source
                    } else {
                        AnalyzerCallConstraintSource::Argument {
                            argument: ordinal,
                            slot: slot.slot(),
                            source: slot.source(),
                            physical_kind: super::semantics::physical_evaluation_kind(
                                authored,
                                slot,
                                false,
                                candidate.schema().argument_policy().spread(),
                            ),
                        }
                    };
                    let prepared = if source == discriminator_source {
                        typed_source_constraint(source, discriminator.actual())?
                    } else {
                        let admission = compile_time_scalar_admission_map
                            .get(&coordinate)
                            .cloned()
                            .ok_or(CallAnalysisFailure::Invariant(
                                CallAnalysisInvariant::Constraint(
                                    CallConstraintInvariant::MalformedMapperSeal,
                                ),
                            ))?;
                        let CallableParameterAdmission::Semantic(
                            crate::callable::CallableSemanticAdmission::CompileTimeScalar(
                                schema_admission,
                            ),
                        ) = parameter.admission()
                        else {
                            return Err(CallAnalysisFailure::Invariant(
                                CallAnalysisInvariant::Constraint(
                                    CallConstraintInvariant::MalformedMapperSeal,
                                ),
                            ));
                        };
                        if schema_admission.kind() != admission.kind().callable_kind()
                            || schema_admission.value_type() != admission.value_type()
                        {
                            return Err(CallAnalysisFailure::Invariant(
                                CallAnalysisInvariant::Constraint(
                                    CallConstraintInvariant::MalformedMapperSeal,
                                ),
                            ));
                        }
                        compile_time_scalar_source_constraint(source, admission)?
                    };
                    groups.push(PreparedSourceConstraintGroup::seal([prepared]).map_err(
                        |error| {
                            CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                                CallConstraintInvariant::Lower(error),
                            ))
                        },
                    )?);
                }
            }
        }
        (groups.into_boxed_slice(), actuals)
    };

    let mut effect_projections = BTreeMap::new();
    let mut compile_time_scalar_admissions = BTreeMap::new();
    for group in &source_groups {
        for prepared in group.sources() {
            if prepared.is_unchecked() {
                continue;
            }
            let source = prepared.source();
            for alternative in prepared.alternatives() {
                if let Some(admission) = alternative.evidence().compile_time_scalar.as_ref()
                    && compile_time_scalar_admissions
                        .insert((source, alternative.alternative()), Arc::clone(admission))
                        .is_some()
                {
                    return Err(CallAnalysisFailure::Invariant(
                        CallAnalysisInvariant::Constraint(
                            CallConstraintInvariant::MalformedMapperSeal,
                        ),
                    ));
                }
                let Some(request) = alternative.evidence().effect_projection.as_ref() else {
                    continue;
                };
                if effect_projections
                    .insert((source, alternative.alternative()), request.clone())
                    .is_some()
                {
                    return Err(CallAnalysisFailure::Invariant(
                        CallAnalysisInvariant::Constraint(
                            CallConstraintInvariant::PreparedEffectInstantiationMismatch,
                        ),
                    ));
                }
            }
        }
    }

    let mut base_constraints = Vec::new();
    let mut receiver_sources = Vec::new();
    let mut receiver_constraints = Vec::new();
    match (&callee_inputs, candidate.instantiation()) {
        (
            PreparedCallCalleeConstraintInputs::EnumConstructor,
            CallableInstantiation::EnumConstructor,
        ) => {}
        (PreparedCallCalleeConstraintInputs::Free, instantiation)
            if matches!(
                instantiation,
                CallableInstantiation::None
                    | CallableInstantiation::Result { .. }
                    | CallableInstantiation::Option
                    | CallableInstantiation::Character { .. }
            ) => {}
        (
            PreparedCallCalleeConstraintInputs::ValueReceiver { source, actual },
            CallableInstantiation::Receiver { receiver },
        ) => {
            let receiver_source = AnalyzerCallConstraintSource::Receiver { source: *source };
            if receiver != actual
                || prepared_source_actuals
                    .insert(
                        receiver_source,
                        AnalyzerPreparedSourceActual::ValueReceiver(actual.clone()),
                    )
                    .is_some()
            {
                return Err(CallAnalysisFailure::Invariant(
                    CallAnalysisInvariant::Constraint(
                        CallConstraintInvariant::MalformedSchemaInventory,
                    ),
                ));
            }
            receiver_sources.push(typed_source_constraint(receiver_source, receiver)?);
            receiver_constraints.push(PreparedCallTypeConstraint {
                source: receiver_source,
                pattern: receiver.clone(),
                actual: actual.clone(),
                acceptance: ConstraintAcceptance::PatternAcceptsActual,
            });
        }
        (
            PreparedCallCalleeConstraintInputs::ValueReceiver { source, actual },
            CallableInstantiation::Extension {
                receiver,
                group,
                parameter,
            },
        ) => {
            let receiver_source = AnalyzerCallConstraintSource::Receiver { source: *source };
            if receiver != actual
                || prepared_source_actuals
                    .insert(
                        receiver_source,
                        AnalyzerPreparedSourceActual::ValueReceiver(actual.clone()),
                    )
                    .is_some()
            {
                return Err(CallAnalysisFailure::Invariant(
                    CallAnalysisInvariant::Constraint(
                        CallConstraintInvariant::MalformedSchemaInventory,
                    ),
                ));
            }
            let declared = candidate
                .constraint_parameter_type(CallableParameterCoordinate::new(*group, *parameter))
                .map_err(|error| {
                    CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error))
                })?
                .ok_or_else(|| {
                    CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                        CallConstraintInvariant::MalformedSchemaInventory,
                    ))
                })?;
            receiver_sources.push(typed_source_constraint(receiver_source, &declared)?);
            receiver_constraints.push(PreparedCallTypeConstraint {
                source: receiver_source,
                pattern: declared.clone(),
                actual: actual.clone(),
                acceptance: ConstraintAcceptance::PatternAcceptsActual,
            });
        }
        (
            PreparedCallCalleeConstraintInputs::AssociatedType { actual },
            CallableInstantiation::TypeReceiver { receiver },
        ) => {
            receiver_constraints.push(PreparedCallTypeConstraint {
                source: AnalyzerCallConstraintSource::BaseInstantiation,
                pattern: receiver.receiver().clone(),
                actual: actual.clone(),
                acceptance: ConstraintAcceptance::PatternAcceptsActual,
            });
        }
        (
            PreparedCallCalleeConstraintInputs::FunctionValue { actual },
            CallableInstantiation::None,
        ) => {
            let constraint = candidate
                .prepare_function_value_constraint(group, actual)
                .map_err(|error| {
                    CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error))
                })?;
            if let Some((pattern, actual)) = constraint {
                base_constraints.push(PreparedCallTypeConstraint {
                    source: AnalyzerCallConstraintSource::BaseInstantiation,
                    pattern,
                    actual,
                    acceptance: ConstraintAcceptance::PatternAcceptsActual,
                });
            }
        }
        (
            PreparedCallCalleeConstraintInputs::DialogueCallee
            | PreparedCallCalleeConstraintInputs::DialogueApplication,
            instantiation,
        ) if matches!(
            instantiation,
            CallableInstantiation::None | CallableInstantiation::Character { .. }
        ) => {}
        (
            PreparedCallCalleeConstraintInputs::StaticContentCallee(_),
            CallableInstantiation::None,
        ) => {}
        (PreparedCallCalleeConstraintInputs::NonCallable, _) => {
            return Err(CallAnalysisFailure::Invariant(
                CallAnalysisInvariant::Constraint(
                    CallConstraintInvariant::MalformedSchemaInventory,
                ),
            ));
        }
        _ => {
            return Err(CallAnalysisFailure::Invariant(
                CallAnalysisInvariant::Constraint(
                    CallConstraintInvariant::MalformedSchemaInventory,
                ),
            ));
        }
    }

    let result_constraint = if let Some(expected) = expected_result {
        let token = candidate
            .issue_group_result_effect_projection(group)
            .map_err(|error| {
                CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error))
            })?;
        let pattern = token.projected_type().map_err(|error| {
            CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error))
        })?;
        let projected_expected = token.seal_actual(expected).map_err(|error| {
            CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error))
        })?;
        Some(PreparedCallTypeConstraint {
            source: AnalyzerCallConstraintSource::Result {
                source: result_source,
            },
            pattern,
            actual: projected_expected,
            acceptance: ConstraintAcceptance::ActualAcceptsPattern,
        })
    } else {
        None
    };
    let result_closure = if candidate.next_group_for(group).is_some() {
        crate::types::constraints::TypeConstraintProjectionClosure::AllowFutureEligible
    } else {
        crate::types::constraints::TypeConstraintProjectionClosure::Closed
    };
    let mut projection_requests = base_constraints
        .iter()
        .map(|constraint| PreparedCallProjectionRequest {
            key: AnalyzerCallProjection::BaseInstantiation,
            value: constraint.pattern.clone(),
            closure: result_closure,
        })
        .collect::<Vec<_>>();
    let result_schema = candidate.result_schema_for_group(group).map_err(|error| {
        CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error))
    })?;
    if let CallableResultSchema::Value(result_projection) = &result_schema {
        projection_requests.push(PreparedCallProjectionRequest {
            key: AnalyzerCallProjection::Result,
            value: result_projection.clone(),
            closure: result_closure,
        });
    } else if expected_result.is_some() {
        return Err(CallAnalysisFailure::Invariant(
            CallAnalysisInvariant::Constraint(
                CallConstraintInvariant::PreparedFunctionTypeMismatch,
            ),
        ));
    }
    projection_requests.extend(future_parameters.into_iter().map(|parameter| {
        PreparedCallProjectionRequest {
            key: AnalyzerCallProjection::Future(parameter.clone()),
            value: TypeKind::GenericParam(parameter),
            closure:
                crate::types::constraints::TypeConstraintProjectionClosure::AllowFutureEligible,
        }
    }));
    Ok(PreparedCallConstraintSet {
        candidate,
        consumer,
        callee_inputs,
        inputs,
        source_groups,
        prepared_source_actuals,
        dialogue_patch_admissions: dialogue_patch_admission_map,
        receiver_sources: receiver_sources.into_boxed_slice(),
        effect_projections,
        compile_time_scalar_admissions,
        base_constraints: base_constraints.into_boxed_slice(),
        definition_effects,
        receiver_constraints: receiver_constraints.into_boxed_slice(),
        result_constraint,
        result_schema,
        projection_requests: projection_requests.into_boxed_slice(),
        initialization,
    })
}

fn lower_source_projection(
    projection: PreparedArgumentSourceProjection,
) -> PreparedConstraintSourceProjection {
    match projection {
        PreparedArgumentSourceProjection::Scalar => PreparedConstraintSourceProjection::Scalar,
        PreparedArgumentSourceProjection::InferSpreadContainer { policy } => {
            PreparedConstraintSourceProjection::InferSpreadContainer {
                policy: match policy {
                    CallableRestContainerPolicy::Positional => {
                        crate::types::constraints::ConstraintSourceContainerPolicy::Positional
                    }
                    CallableRestContainerPolicy::Named => {
                        crate::types::constraints::ConstraintSourceContainerPolicy::Named
                    }
                },
            }
        }
    }
}

fn typed_source_constraint(
    source: AnalyzerCallConstraintSource,
    expected: &TypeKind,
) -> CallAnalysisResult<PreparedSourceConstraint<AnalyzerCallConstraintDomain>> {
    PreparedSourceConstraint::checked(
        source,
        PreparedConstraintSourceProjection::Scalar,
        [],
        PreparedSourceAlternative::new(
            0,
            AnalyzerCallEvidenceRule::otherwise(None),
            expected.clone(),
        ),
    )
    .map_err(|error| {
        CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
            CallConstraintInvariant::Lower(match error {
                crate::types::constraints::TypeConstraintError::Invariant(error) => error,
                crate::types::constraints::TypeConstraintError::Rejected(_) => {
                    TypeConstraintInvariant::SourceProtocol(
                        crate::types::constraints::TypeConstraintSourceProtocolInvariant::Outcome,
                    )
                }
                crate::types::constraints::TypeConstraintError::Abort(_) => {
                    TypeConstraintInvariant::SourceProtocol(
                        crate::types::constraints::TypeConstraintSourceProtocolInvariant::Outcome,
                    )
                }
            }),
        ))
    })
}

fn compile_time_scalar_source_constraint(
    source: AnalyzerCallConstraintSource,
    admission: Arc<crate::checked_compile_time::PreparedCompileTimeScalarAdmission>,
) -> CallAnalysisResult<PreparedSourceConstraint<AnalyzerCallConstraintDomain>> {
    PreparedSourceConstraint::checked(
        source,
        PreparedConstraintSourceProjection::Scalar,
        [],
        PreparedSourceAlternative::new(
            0,
            AnalyzerCallEvidenceRule::compile_time_scalar(Arc::clone(&admission)),
            admission.value_type().clone(),
        ),
    )
    .map_err(|error| {
        CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
            CallConstraintInvariant::Lower(match error {
                crate::types::constraints::TypeConstraintError::Invariant(error) => error,
                crate::types::constraints::TypeConstraintError::Rejected(_)
                | crate::types::constraints::TypeConstraintError::Abort(_) => {
                    TypeConstraintInvariant::SourceProtocol(
                        crate::types::constraints::TypeConstraintSourceProtocolInvariant::Outcome,
                    )
                }
            }),
        ))
    })
}

fn prepare_compile_time_scalar_type_admission(
    value_type: &TypeKind,
    scalar_types: &crate::registration::RegisteredCompileTimeScalarTypes,
) -> Option<crate::checked_compile_time::PreparedCompileTimeScalarAdmission> {
    use crate::{
        checked_compile_time::{
            CheckedCompileTimeScalarKind, CompileTimeScalarSourceMode,
            PreparedCompileTimeScalarAdmission,
        },
        registration::CompileTimeScalarTypeRoleId,
    };

    let TypeKind::CompileTimeScalar(scalar) = value_type else {
        return None;
    };
    let (kind, role, source_mode) = match scalar.kind() {
        crate::types::CompileTimeScalarKind::Milli => (
            CheckedCompileTimeScalarKind::Milli,
            CompileTimeScalarTypeRoleId::Milli,
            CompileTimeScalarSourceMode::Literal,
        ),
        crate::types::CompileTimeScalarKind::Ratio => (
            CheckedCompileTimeScalarKind::Ratio,
            CompileTimeScalarTypeRoleId::Ratio,
            CompileTimeScalarSourceMode::Literal,
        ),
        crate::types::CompileTimeScalarKind::Length => (
            CheckedCompileTimeScalarKind::Length,
            CompileTimeScalarTypeRoleId::Length,
            CompileTimeScalarSourceMode::Literal,
        ),
        crate::types::CompileTimeScalarKind::Angle => (
            CheckedCompileTimeScalarKind::Angle,
            CompileTimeScalarTypeRoleId::Angle,
            CompileTimeScalarSourceMode::Literal,
        ),
        crate::types::CompileTimeScalarKind::PublicId => (
            CheckedCompileTimeScalarKind::PublicId,
            CompileTimeScalarTypeRoleId::PublicId,
            CompileTimeScalarSourceMode::PublicId,
        ),
        crate::types::CompileTimeScalarKind::Color => (
            CheckedCompileTimeScalarKind::Color,
            CompileTimeScalarTypeRoleId::Color,
            CompileTimeScalarSourceMode::Typed(
                scalar_types
                    .type_for(CompileTimeScalarTypeRoleId::Color)
                    .clone(),
            ),
        ),
    };
    if scalar_types.type_for(role) != value_type {
        return None;
    }
    PreparedCompileTimeScalarAdmission::try_new(kind, value_type.clone(), source_mode)
}

/// Convert one mapper slot into the lower-owned source algebra.  In
/// particular, a typed rest slot keeps its prepared container policy; the
/// callback only supplies an actual container and lower derives the final
/// constructor and composed expected type.
pub(crate) fn prepare_source_constraint(
    candidate: &PreparedResolvedCallable,
    source: AnalyzerCallConstraintSource,
    slot: &crate::callable::MappedCallArgumentSlot,
    scalar_types: &crate::registration::RegisteredCompileTimeScalarTypes,
    view_fx_runtime_parameters: Option<&BTreeMap<CallableParameterCoordinate, TypeKind>>,
) -> Result<PreparedSourceConstraint<AnalyzerCallConstraintDomain>, CallAnalysisFailure> {
    let projection = lower_source_projection(slot.source_projection());
    let Some(coordinate) = slot.coordinate() else {
        return Ok(PreparedSourceConstraint::unchecked(source, projection));
    };
    prepare_parameter_source_constraint(
        candidate,
        source,
        coordinate,
        projection,
        scalar_types,
        view_fx_runtime_parameters,
    )
}

fn prepare_parameter_source_constraint(
    candidate: &PreparedResolvedCallable,
    source: AnalyzerCallConstraintSource,
    coordinate: CallableParameterCoordinate,
    projection: PreparedConstraintSourceProjection,
    scalar_types: &crate::registration::RegisteredCompileTimeScalarTypes,
    view_fx_runtime_parameters: Option<&BTreeMap<CallableParameterCoordinate, TypeKind>>,
) -> Result<PreparedSourceConstraint<AnalyzerCallConstraintDomain>, CallAnalysisFailure> {
    let Some(parameter) = candidate
        .schema()
        .group(coordinate.group())
        .and_then(|group| group.parameter(coordinate.parameter()))
    else {
        return Err(CallAnalysisFailure::Invariant(
            CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedSchemaInventory),
        ));
    };
    if parameter.admission().is_semantic()
        || matches!(
            parameter.consumer(),
            crate::callable::CallableParameterConsumer::Content(_)
        )
    {
        return Err(CallAnalysisFailure::Invariant(
            CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
        ));
    }
    let runtime_override = view_fx_runtime_parameters.and_then(|rows| rows.get(&coordinate));
    let declared = if let Some(runtime) = runtime_override {
        runtime.clone()
    } else {
        let Some(declared) = candidate
            .constraint_parameter_type(coordinate)
            .map_err(|error| {
                CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error))
            })?
        else {
            return Ok(PreparedSourceConstraint::unchecked(source, projection));
        };
        declared
    };
    let rule = parameter.value_rule().ok_or_else(|| {
        CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
            CallConstraintInvariant::MalformedSchemaInventory,
        ))
    })?;
    if runtime_override.is_none() && matches!(declared, TypeKind::CompileTimeScalar(_)) {
        if !projection.is_scalar() || rule != &crate::callable::CallableParameterValueRule::supply()
        {
            return Err(CallAnalysisFailure::Invariant(
                CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedMapperSeal),
            ));
        }
        let admission = prepare_compile_time_scalar_type_admission(&declared, scalar_types).ok_or(
            CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                CallConstraintInvariant::MalformedSchemaInventory,
            )),
        )?;
        return compile_time_scalar_source_constraint(source, Arc::new(admission));
    }
    let guarded = rule
        .guarded()
        .iter()
        .enumerate()
        .map(|(ordinal, guarded)| {
            let effect_projection = Some(AnalyzerCallEffectProjectionRequest {
                coordinate,
                expected: guarded.expected().clone(),
                source_projection: projection,
            });
            PreparedSourceAlternative::new(
                u32::try_from(ordinal).unwrap_or(u32::MAX),
                AnalyzerCallEvidenceRule::guarded(
                    guarded.guard().clone(),
                    declared.clone(),
                    effect_projection,
                ),
                guarded.expected().apply_to(&declared),
            )
        })
        .collect::<Vec<_>>();
    let otherwise = rule.otherwise();
    let otherwise = PreparedSourceAlternative::new(
        u32::try_from(rule.guarded().len()).unwrap_or(u32::MAX),
        AnalyzerCallEvidenceRule::otherwise(Some(AnalyzerCallEffectProjectionRequest {
            coordinate,
            expected: otherwise.expected().clone(),
            source_projection: projection,
        })),
        otherwise.expected().apply_to(&declared),
    );
    PreparedSourceConstraint::checked(source, projection, guarded, otherwise).map_err(|error| {
        CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
            CallConstraintInvariant::Lower(match error {
                crate::types::constraints::TypeConstraintError::Invariant(error) => error,
                crate::types::constraints::TypeConstraintError::Rejected(_) => {
                    TypeConstraintInvariant::SourceProtocol(
                        crate::types::constraints::TypeConstraintSourceProtocolInvariant::Outcome,
                    )
                }
                crate::types::constraints::TypeConstraintError::Abort(_) => {
                    TypeConstraintInvariant::SourceProtocol(
                        crate::types::constraints::TypeConstraintSourceProtocolInvariant::Outcome,
                    )
                }
            }),
        ))
    })
}

/// Execute one complete lower candidate transaction through the affine
/// callable driver. This helper is intentionally independent of semantic
/// publication; its caller chooses singleton move or multi-candidate replay.
pub(crate) fn run_prepared_candidate(
    analyzer: &mut super::super::Analyzer<'_, '_, '_>,
    work: &mut crate::callable::ResolverWork,
    context: &AnalyzerExpressionContext<'_>,
    pass: CandidateEvaluationPass,
    attempt: Option<PhysicalCallAttemptId>,
    set: PreparedCallConstraintSet,
) -> Result<RanCandidateTransaction, TypeConstraintFailure<AnalyzerCallConstraintDomain>> {
    let PreparedCallConstraintSet {
        candidate,
        consumer,
        callee_inputs,
        inputs,
        source_groups,
        prepared_source_actuals,
        dialogue_patch_admissions,
        receiver_sources,
        effect_projections,
        compile_time_scalar_admissions,
        base_constraints,
        definition_effects,
        receiver_constraints,
        result_constraint,
        result_schema,
        projection_requests,
        initialization,
    } = set;
    let session = work
        .begin_candidate_constraint_session(
            analyzer.catalogs.callable_limits,
            analyzer.control.cancellation(),
        )
        .map_err(|error| match error {
            crate::callable::CandidateConstraintSessionStartFailure::ArithmeticOverflow => {
                TypeConstraintFailure::Abort(TypeConstraintAbort::ArithmeticOverflow)
            }
        })?;
    let operations = AnalyzerCallExpressionClient::new(
        analyzer,
        context,
        Some(Arc::clone(&candidate)),
        effect_projections,
        compile_time_scalar_admissions,
        prepared_source_actuals,
        dialogue_patch_admissions,
        pass,
        attempt,
    );
    let mut driver = session
        .start(
            initialization,
            AnalyzerCallConstraintClient::new(operations),
        )
        .map_err(|failure| match failure {
            crate::callable::CandidateConstraintDriverStartFailure::Prepared(error) => {
                TypeConstraintFailure::client_invariant(AnalyzerCallClientInvariant::constraint(
                    AnalyzerCallConstraintSource::BaseInstantiation,
                    error,
                ))
            }
            crate::callable::CandidateConstraintDriverStartFailure::Lower(
                TypeConstraintInitializationFailure::Abort(error),
            ) => TypeConstraintFailure::Abort(error),
            crate::callable::CandidateConstraintDriverStartFailure::Lower(
                TypeConstraintInitializationFailure::Invariant(error),
            ) => {
                TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Constraint(error))
            }
        })?;
    if let Some(effects) = definition_effects {
        driver.constrain_effect_equality(&effects.projected, &effects.known);
    }
    for constraint in base_constraints.iter().chain(receiver_constraints.iter()) {
        let _ = constraint.source;
        if constraint.pattern != constraint.actual {
            driver.constrain(
                &constraint.pattern,
                &constraint.actual,
                constraint.acceptance,
            );
        }
    }
    for prepared in receiver_sources {
        driver.probe_source(prepared, ConstraintAcceptance::PatternAcceptsActual)?;
    }
    for group in source_groups {
        driver.probe_source_group(group, ConstraintAcceptance::PatternAcceptsActual)?;
    }
    if let Some(constraint) = result_constraint {
        let _ = constraint.source;
        if constraint.pattern != constraint.actual {
            driver.constrain(
                &constraint.pattern,
                &constraint.actual,
                constraint.acceptance,
            );
        }
    }
    let mut expected_projection_keys = projection_requests
        .iter()
        .map(|request| request.key.clone())
        .collect::<Vec<_>>();
    for request in projection_requests {
        driver.request_projection(request.key, &request.value, request.closure);
    }
    let solved_result = driver.finish().complete();
    let solved = solved_result?;
    let mut source_coordinates = BTreeSet::new();
    for source in &solved.closed_sources {
        if let Some(coordinate) = source.source().value_coordinate()
            && !source_coordinates.insert(coordinate)
        {
            return Err(TypeConstraintFailure::Invariant(
                TypeConstraintFailureInvariant::Constraint(
                    TypeConstraintInvariant::SourceProtocol(
                        crate::types::constraints::TypeConstraintSourceProtocolInvariant::Ticket,
                    ),
                ),
            ));
        }
    }
    expected_projection_keys.sort();
    let actual_projection_keys = solved
        .projections
        .iter()
        .map(|projection| projection.key().clone())
        .collect::<Vec<_>>();
    if expected_projection_keys != actual_projection_keys {
        return Err(TypeConstraintFailure::Invariant(
            TypeConstraintFailureInvariant::Constraint(TypeConstraintInvariant::Projection(
                crate::types::constraints::TypeConstraintProjectionInvariant::MissingKey,
            )),
        ));
    }
    let current_group = candidate.call_group();
    let result = match result_schema {
        CallableResultSchema::ContentEmission(operation) => {
            CallableResultSchema::ContentEmission(operation)
        }
        CallableResultSchema::Value(_) => solved
            .projections
            .iter()
            .find(|projection| projection.key() == &AnalyzerCallProjection::Result)
            .map(|projection| {
                projection
                    .value()
                    .to_quantified_type()
                    .map(CallableResultSchema::Value)
            })
            .ok_or_else(|| {
                TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Constraint(
                    TypeConstraintInvariant::Projection(
                        crate::types::constraints::TypeConstraintProjectionInvariant::MissingKey,
                    ),
                ))
            })?
            .map_err(|error| {
                TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Constraint(
                    TypeConstraintInvariant::Instantiation(error),
                ))
            })?,
    };
    Ok(RanCandidateTransaction {
        data: Box::new(RanCandidateTransactionData {
            candidate,
            consumer,
            callee_inputs,
            inputs,
            current_group,
            result,
            solved,
        }),
    })
}

#[cfg(test)]
mod tests {
    use std::{rc::Rc, sync::atomic::AtomicBool};

    use super::*;
    use crate::callable::SourceCheckpointFailure;

    fn callback_test_owner(fixture: &crate::final_analysis::tests::Fixture) -> ExprId {
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
    fn materialization_request_rejects_projection_branch_actual_and_evidence_tamper() {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
        let owner = callback_test_owner(&fixture);
        let source = AnalyzerCallConstraintSource::Result { source: owner };
        let actual = TypeKind::I32;
        let expected = TypeKind::I32;
        let scalar = crate::types::constraints::CheckedConstraintSourceProjection::Scalar;
        let branch = AnalyzerCallProbeSemanticBranch { source };
        let checked = AnalyzerCallObservedSource {
            actual: actual.clone(),
            evidence: ObservedSemanticValueEvidence::NoVariantCase,
        };
        let evidence = CheckedSemanticValueEvidence::NoVariantCase;
        let request = MaterializedSourceRequest::Checked {
            source,
            alternative: 0,
            evidence: &evidence,
            source_projection: &scalar,
            actual: &actual,
            expected: &expected,
            canonical_branch: &branch,
        };
        assert_eq!(request.expected(), Some(&expected));
        assert_eq!(
            validate_materialized_source_request(&request, &checked),
            Ok(())
        );

        let wrong_actual = TypeKind::I64;
        let request = MaterializedSourceRequest::Unchecked {
            source,
            source_projection: &scalar,
            actual: &wrong_actual,
            canonical_branch: &branch,
        };
        assert!(matches!(
            validate_materialized_source_request(&request, &checked),
            Err(TypeConstraintInvariant::Projection(
                crate::types::constraints::TypeConstraintProjectionInvariant::Mismatch(
                    crate::types::constraints::TypeConstraintRejection::Mismatch,
                )
            ))
        ));

        let spread = crate::types::constraints::CheckedConstraintSourceProjection::SpreadContainer(
            crate::types::constraints::CheckedConstraintContainerConstructor::Vec,
        );
        let request = MaterializedSourceRequest::Unchecked {
            source,
            source_projection: &spread,
            actual: &actual,
            canonical_branch: &branch,
        };
        assert!(matches!(
            validate_materialized_source_request(&request, &checked),
            Err(TypeConstraintInvariant::Projection(
                crate::types::constraints::TypeConstraintProjectionInvariant::Mismatch(
                    crate::types::constraints::TypeConstraintRejection::Mismatch,
                )
            ))
        ));

        let wrong_branch = AnalyzerCallProbeSemanticBranch {
            source: AnalyzerCallConstraintSource::BaseInstantiation,
        };
        let request = MaterializedSourceRequest::Unchecked {
            source,
            source_projection: &scalar,
            actual: &actual,
            canonical_branch: &wrong_branch,
        };
        assert!(matches!(
            validate_materialized_source_request(&request, &checked),
            Err(TypeConstraintInvariant::SourceProtocol(
                crate::types::constraints::TypeConstraintSourceProtocolInvariant::Outcome
            ))
        ));

        let wrong_evidence = CheckedSemanticValueEvidence::VariantCase {
            owner: actual
                .semantic_identity_digest()
                .expect("stable fixture type"),
            ordinal: 0,
            payload: VariantPayloadRequirement::Unit,
        };
        let request = MaterializedSourceRequest::Checked {
            source,
            alternative: 0,
            evidence: &wrong_evidence,
            source_projection: &scalar,
            actual: &actual,
            expected: &expected,
            canonical_branch: &branch,
        };
        assert!(matches!(
            validate_materialized_source_request(&request, &checked),
            Err(TypeConstraintInvariant::SourceProtocol(
                crate::types::constraints::TypeConstraintSourceProtocolInvariant::InvalidEvidence
            ))
        ));
    }

    #[test]
    fn nested_failure_payload_keeps_inner_owner_and_outer_source() {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
        let module = fixture
            .project
            .analysis_view()
            .expect("executable HIR")
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root module");
        let inner_owner = module
            .expressions()
            .next()
            .map(|(owner, _)| owner)
            .expect("nested expression owner");
        let outer_source = AnalyzerCallConstraintSource::Result {
            source: inner_owner,
        };
        let invariant = AnalyzerCallClientInvariant::nested_call(
            outer_source,
            inner_owner,
            CallAnalysisInvariant::Constraint(CallConstraintInvariant::MalformedSchemaInventory),
        );
        assert!(matches!(
            invariant.cause,
            AnalyzerCallClientInvariantCause::NestedCall { owner, .. } if owner == inner_owner
        ));

        let inner_error = SourceError::new(
            outer_source,
            SourcePhase::Probe,
            AnalyzerCallSourceFailureCause::Mismatch,
        );
        let fatal = AnalyzerCallSourceFailureCause::NestedCallFatal {
            owner: inner_owner,
            error: Box::new(inner_error),
        };
        assert!(matches!(
            fatal,
            AnalyzerCallSourceFailureCause::NestedCallFatal { owner, .. }
                if owner == inner_owner
        ));

        let public = crate::final_analysis::FinalCallConstraintFailure::new(
            inner_owner,
            CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                CallConstraintInvariant::MalformedSchemaInventory,
            )),
        );
        assert_eq!(public, public.clone());
    }

    #[test]
    fn active_callback_scope_conflict_retains_both_coordinates() {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
        let owner = callback_test_owner(&fixture);
        let cancellation = AtomicBool::new(false);
        let mut analyzer = super::super::Analyzer::new(
            fixture.project.analysis_view().expect("executable HIR"),
            &fixture.symbols,
            crate::final_analysis::FinalSemanticCatalogs::production(&fixture.registered),
            crate::final_analysis::FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        let context = AnalyzerExpressionContext::published(Rc::clone(&analyzer.call_frames));
        let mut client = AnalyzerCallExpressionClient::new(
            &mut analyzer,
            &context,
            None,
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            CandidateEvaluationPass::Probe,
            None,
        );
        let existing = AnalyzerCallConstraintSource::Receiver { source: owner };
        let requested = AnalyzerCallConstraintSource::Result { source: owner };
        let checkpoint =
            AnalyzerCallConstraintOperations::open_probe_checkpoint(&mut client, existing)
                .unwrap_or_else(|_| panic!("first callback scope"));

        let conflict = AnalyzerCallConstraintOperations::open_materialization_checkpoint(
            &mut client,
            &[requested, existing],
        );
        let conflict = match conflict {
            Err(conflict) => conflict,
            Ok(_) => panic!("a second callback scope must be rejected"),
        };
        let SourceCheckpointFailure::Client(invariant) = conflict else {
            panic!("active callback conflict must retain a client invariant");
        };
        assert_eq!(invariant.source, requested);
        let AnalyzerCallClientInvariantCause::ActiveFactScopeConflict {
            existing: actual_existing,
            requested: actual_requested,
        } = invariant.cause
        else {
            panic!("active callback conflict lost its coordinate payload");
        };
        assert_eq!(
            actual_existing,
            AnalyzerCallScopeCoordinate::Probe { source: existing }
        );
        assert_eq!(
            actual_requested,
            AnalyzerCallScopeCoordinate::Materialization {
                owner: requested,
                sources: vec![requested, existing].into_boxed_slice(),
            }
        );

        AnalyzerCallConstraintOperations::close_probe_checkpoint(&mut client, checkpoint)
            .unwrap_or_else(|_| panic!("the original scope remains closable after the conflict"));
        let next = AnalyzerCallConstraintOperations::open_probe_checkpoint(&mut client, requested)
            .unwrap_or_else(|_| panic!("conflict must not leave an active scope behind"));
        AnalyzerCallConstraintOperations::close_probe_checkpoint(&mut client, next)
            .unwrap_or_else(|_| panic!("replacement scope closes"));

        let materialization = AnalyzerCallConstraintOperations::open_materialization_checkpoint(
            &mut client,
            &[existing, requested],
        )
        .unwrap_or_else(|_| panic!("materialization callback scope"));
        let conflict =
            AnalyzerCallConstraintOperations::open_probe_checkpoint(&mut client, existing);
        let conflict = match conflict {
            Err(conflict) => conflict,
            Ok(_) => panic!("a second probe callback scope must be rejected"),
        };
        let SourceCheckpointFailure::Client(invariant) = conflict else {
            panic!("active callback conflict must retain a client invariant");
        };
        assert_eq!(invariant.source, existing);
        let AnalyzerCallClientInvariantCause::ActiveFactScopeConflict {
            existing: actual_existing,
            requested: actual_requested,
        } = invariant.cause
        else {
            panic!("active callback conflict lost its coordinate payload");
        };
        assert_eq!(
            actual_existing,
            AnalyzerCallScopeCoordinate::Materialization {
                owner: existing,
                sources: vec![existing, requested].into_boxed_slice(),
            }
        );
        assert_eq!(
            actual_requested,
            AnalyzerCallScopeCoordinate::Probe { source: existing }
        );
        let close = AnalyzerCallConstraintOperations::close_materialization_checkpoint(
            &mut client,
            materialization,
            None,
        );
        assert!(matches!(close, Ok(None)));
    }

    #[test]
    fn wrong_probe_source_close_rolls_back_and_clears_scope() {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
        let owner = callback_test_owner(&fixture);
        let cancellation = AtomicBool::new(false);
        let mut analyzer = super::super::Analyzer::new(
            fixture.project.analysis_view().expect("executable HIR"),
            &fixture.symbols,
            crate::final_analysis::FinalSemanticCatalogs::production(&fixture.registered),
            crate::final_analysis::FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        let context = AnalyzerExpressionContext::published(Rc::clone(&analyzer.call_frames));
        let mut client = AnalyzerCallExpressionClient::new(
            &mut analyzer,
            &context,
            None,
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            CandidateEvaluationPass::Probe,
            None,
        );
        let source = AnalyzerCallConstraintSource::Receiver { source: owner };
        let wrong_source = AnalyzerCallConstraintSource::Result { source: owner };
        let mut checkpoint =
            AnalyzerCallConstraintOperations::open_probe_checkpoint(&mut client, source)
                .unwrap_or_else(|_| panic!("callback scope"));
        checkpoint.source = wrong_source;
        let close =
            AnalyzerCallConstraintOperations::close_probe_checkpoint(&mut client, checkpoint);
        assert!(matches!(
            close,
            Err(SourceCheckpointFailure::Protocol(
                crate::types::constraints::TypeConstraintSourceProtocolInvariant::WrongSource
            ))
        ));

        let next =
            AnalyzerCallConstraintOperations::open_probe_checkpoint(&mut client, wrong_source)
                .unwrap_or_else(|_| panic!("wrong-source close must clear the active scope"));
        AnalyzerCallConstraintOperations::close_probe_checkpoint(&mut client, next)
            .unwrap_or_else(|_| panic!("replacement scope closes"));
    }

    #[test]
    fn wrong_materialization_order_close_rolls_back_and_clears_scope() {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
        let owner = callback_test_owner(&fixture);
        let cancellation = AtomicBool::new(false);
        let mut analyzer = super::super::Analyzer::new(
            fixture.project.analysis_view().expect("executable HIR"),
            &fixture.symbols,
            crate::final_analysis::FinalSemanticCatalogs::production(&fixture.registered),
            crate::final_analysis::FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        let context = AnalyzerExpressionContext::published(Rc::clone(&analyzer.call_frames));
        let mut client = AnalyzerCallExpressionClient::new(
            &mut analyzer,
            &context,
            None,
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            CandidateEvaluationPass::Probe,
            None,
        );
        let first = AnalyzerCallConstraintSource::Receiver { source: owner };
        let second = AnalyzerCallConstraintSource::Result { source: owner };
        let mut checkpoint = AnalyzerCallConstraintOperations::open_materialization_checkpoint(
            &mut client,
            &[first, second],
        )
        .unwrap_or_else(|_| panic!("materialization scope"));
        checkpoint.sources.reverse();
        let close = AnalyzerCallConstraintOperations::close_materialization_checkpoint(
            &mut client,
            checkpoint,
            None,
        );
        assert!(matches!(
            close,
            Err(SourceCheckpointFailure::Protocol(
                crate::types::constraints::TypeConstraintSourceProtocolInvariant::WrongSource
            ))
        ));

        let next = AnalyzerCallConstraintOperations::open_probe_checkpoint(&mut client, first)
            .unwrap_or_else(|_| panic!("wrong-order close must clear the active scope"));
        AnalyzerCallConstraintOperations::close_probe_checkpoint(&mut client, next)
            .unwrap_or_else(|_| panic!("replacement scope closes"));
    }

    #[test]
    fn dialogue_terminal_mapping_requires_the_exact_lower_source_projection() {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
        let owner = callback_test_owner(&fixture);
        let view = fixture.project.analysis_view().expect("executable HIR");
        let module = view
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root module");
        let span = crate::final_analysis::analyzer::statements::expression_span(module, owner)
            .expect("test expression span");
        let argument = HirCallArgumentOrdinal::try_from_usize(0).expect("argument ordinal");
        let slot = CallableArgumentSlotIndex::try_from_usize(0).expect("slot index");
        let parameter =
            crate::callable::CallableParameterIndex::try_from_usize(0).expect("parameter index");
        let coordinate = CallableParameterCoordinate::new(CallableGroupIndex::ZERO, parameter);
        let field = arcweft_interaction_model::dialogue::CharacterDialogueCustomFieldId::try_new(
            "character_dialogue_field.test",
        )
        .expect("custom field ID");
        let admission = AnalyzerPreparedDialoguePatchAdmission::new(
            argument,
            owner,
            coordinate,
            field,
            TypeKind::String,
            true,
            1,
            span.clone(),
            span.clone(),
            span,
        );
        let exact_source = AnalyzerCallConstraintSource::DialoguePatch {
            argument,
            slot,
            source: CheckedCallArgumentSlotSource::Expression(owner),
            coordinate,
            physical_kind: PhysicalArgumentEvaluationKind::Authored,
        };
        let rejected = |source, alternative, projection, acceptance, expected| {
            crate::types::constraints::RejectedConstraintSourceProjection::<
                AnalyzerCallConstraintDomain,
            >::test_new(
                source,
                alternative,
                projection,
                acceptance,
                expected,
                TypeKind::I32,
            )
        };
        assert!(admission.accepts_rejected_source_projection(&rejected(
            exact_source,
            Some(1),
            crate::types::constraints::CheckedConstraintSourceProjection::Scalar,
            ConstraintAcceptance::PatternAcceptsActual,
            TypeKind::String,
        )));
        assert!(!admission.accepts_rejected_source_projection(&rejected(
            AnalyzerCallConstraintSource::Argument {
                argument,
                slot,
                source: CheckedCallArgumentSlotSource::Expression(owner),
                physical_kind: PhysicalArgumentEvaluationKind::Authored,
            },
            Some(1),
            crate::types::constraints::CheckedConstraintSourceProjection::Scalar,
            ConstraintAcceptance::PatternAcceptsActual,
            TypeKind::String,
        )));
        assert!(!admission.accepts_rejected_source_projection(&rejected(
            exact_source,
            Some(0),
            crate::types::constraints::CheckedConstraintSourceProjection::Scalar,
            ConstraintAcceptance::PatternAcceptsActual,
            TypeKind::String,
        )));
        assert!(!admission.accepts_rejected_source_projection(&rejected(
            exact_source,
            Some(1),
            crate::types::constraints::CheckedConstraintSourceProjection::SpreadContainer(
                crate::types::constraints::CheckedConstraintContainerConstructor::Vec,
            ),
            ConstraintAcceptance::PatternAcceptsActual,
            TypeKind::String,
        )));
        assert!(!admission.accepts_rejected_source_projection(&rejected(
            exact_source,
            Some(1),
            crate::types::constraints::CheckedConstraintSourceProjection::Scalar,
            ConstraintAcceptance::ActualAcceptsPattern,
            TypeKind::String,
        )));
        assert!(!admission.accepts_rejected_source_projection(&rejected(
            exact_source,
            Some(1),
            crate::types::constraints::CheckedConstraintSourceProjection::Scalar,
            ConstraintAcceptance::PatternAcceptsActual,
            TypeKind::Bool,
        )));
    }
}
