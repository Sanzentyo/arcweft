//! Candidate-wide equations, prepared source traces, and finalization.

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

use super::super::{ArrayLength, GenericConstReference, GenericTypeReference, TypeKind};
use super::application::{
    ConstraintApplicationId, ConstraintApplicationScope, ConstraintApplicationScopes,
};
use super::context::{TypeConstraintAccounting, TypeConstraintContext};
use super::hints::SourceProbeTerm;
use super::normalization::{project_type, validate_selected_call_self};
use super::{
    CheckedConstraintSourceProjection, ClosedMaterializationSubmission, ConstraintAcceptance,
    ConstraintClosurePolicy, ConstraintDomain, ExpectedHint, InheritedSolutionInvariant,
    InheritedSolutionInvariantKind, MaterializationImmediateFailure, MaterializedSourceRequest,
    PreparedConstraintSourceProjection, PreparedSourceConstraint, ProjectedExpectedHint,
    SolvedCandidate, SourceAlternativeHint, SourceError, SourcePhase, SourceProbeResult,
    SourceProbeSelection, TypeConstraintAbort, TypeConstraintCandidateFailure, TypeConstraintError,
    TypeConstraintFailure, TypeConstraintFailureInvariant, TypeConstraintInvariant,
    TypeConstraintProjectionClosure, TypeConstraintProjectionInvariant, TypeConstraintRejection,
    TypeConstraintSolution, TypeConstraintSourceProtocolInvariant, relate_selected_call, seal_path,
    seal_type, validate_type,
};
use super::{ConstraintSourceId, RejectedConstraintSourceProjection};

/// One equation retained until candidate closure. Source ordinals connect to
/// the single source trace; selection and container evidence stay there.
#[derive(Clone)]
pub(crate) struct PendingEquation {
    pub(crate) ordinal: ConstraintEquationId,
    pub(crate) direction: ConstraintAcceptance,
    pub(crate) pattern: TypeKind,
    pub(crate) actual: TypeKind,
    pub(crate) source_ordinal: Option<PreparedSourceOrdinal>,
    pub(crate) final_expected: Option<TypeKind>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ChoiceForkRole {
    ExpectedAlternative,
    ActualAlternative,
    ExpectedActualPair,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ChoiceDerivationStep {
    pub(crate) equation: ConstraintEquationId,
    pub(crate) direction: ConstraintAcceptance,
    pub(crate) role: ChoiceForkRole,
    pub(crate) expected: Option<u32>,
    pub(crate) actual: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DeferredCycleWitness {
    pub(crate) parameters: BTreeSet<super::ConstraintGenericParameterId>,
}

mod source;
use source::{ActiveConstraintProbe, ConstraintProbe};
pub(crate) use source::{ClosedConstraintProbe, ClosedConstraintSourceTrace};

pub(crate) enum StoredSourceSelection<D: ConstraintDomain> {
    Unchecked,
    Checked {
        alternative: D::AlternativeIndex,
        evidence: Arc<D::ObservedEvidence>,
    },
}

impl<D: ConstraintDomain> Clone for StoredSourceSelection<D> {
    fn clone(&self) -> Self {
        match self {
            Self::Unchecked => Self::Unchecked,
            Self::Checked {
                alternative,
                evidence,
            } => Self::Checked {
                alternative: *alternative,
                evidence: Arc::clone(evidence),
            },
        }
    }
}

impl<D: ConstraintDomain> PartialEq for StoredSourceSelection<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unchecked, Self::Unchecked) => true,
            (
                Self::Checked {
                    alternative: left_alternative,
                    evidence: left_evidence,
                },
                Self::Checked {
                    alternative: right_alternative,
                    evidence: right_evidence,
                },
            ) => left_alternative == right_alternative && left_evidence == right_evidence,
            _ => false,
        }
    }
}

impl<D: ConstraintDomain> Eq for StoredSourceSelection<D> {}

pub(crate) struct ConstraintPath<D: ConstraintDomain> {
    pub(super) applications: Arc<ConstraintApplicationScopes<D>>,
    pub(crate) bindings: BTreeMap<GenericTypeReference, TypeKind>,
    pub(crate) const_bindings: BTreeMap<GenericConstReference, ArrayLength>,
    pub(crate) effects: crate::effect_row::EffectConstraintEnvironment,
    pub(crate) equations: Vec<PendingEquation>,
    pub(crate) choice_key: Vec<ChoiceDerivationStep>,
    pub(crate) deferred_cycles: DeferredCycleWitness,
    pub(super) probe_trace: Vec<ConstraintProbe<D>>,
    pub(super) projections: Vec<Arc<ProjectionRequest<D>>>,
}

impl<D: ConstraintDomain> ConstraintPath<D> {
    pub(super) fn empty(
        application: ConstraintApplicationScope<D>,
        effects: crate::effect_row::EffectConstraintEnvironment,
    ) -> Self {
        Self::empty_with_imported(application, effects, None)
    }

    pub(super) fn empty_with_imported(
        application: ConstraintApplicationScope<D>,
        effects: crate::effect_row::EffectConstraintEnvironment,
        imported: Option<super::ImportedGenericParameterScopeLease>,
    ) -> Self {
        Self {
            applications: Arc::new(ConstraintApplicationScopes::root(application, imported)),
            bindings: BTreeMap::new(),
            const_bindings: BTreeMap::new(),
            effects,
            equations: Vec::new(),
            choice_key: Vec::new(),
            deferred_cycles: DeferredCycleWitness::default(),
            probe_trace: Vec::new(),
            projections: Vec::new(),
        }
    }
}

impl<D: ConstraintDomain> Clone for ConstraintPath<D> {
    fn clone(&self) -> Self {
        Self {
            applications: Arc::clone(&self.applications),
            bindings: self.bindings.clone(),
            const_bindings: self.const_bindings.clone(),
            effects: self.effects.clone(),
            equations: self.equations.clone(),
            choice_key: self.choice_key.clone(),
            deferred_cycles: self.deferred_cycles.clone(),
            probe_trace: self.probe_trace.clone(),
            projections: self.projections.clone(),
        }
    }
}

/// One affine source branch. Observation consumes its path; forks share the
/// immutable input receipt and charge the surrounding context before cloning.
pub(crate) struct ProbeTicket<D: ConstraintDomain> {
    input: Arc<ProbeInput<D>>,
    path: Option<ConstraintPath<D>>,
}

/// Exact parent source receipt retained while a nested callable contributes
/// constraints to the same lower path.  The input identity binds the source,
/// prepared schema alternatives, and exact path branch together.
pub(crate) struct ConstraintSourceReceipt<D: ConstraintDomain> {
    input: Arc<ProbeInput<D>>,
}

impl<D: ConstraintDomain> Clone for ConstraintSourceReceipt<D> {
    fn clone(&self) -> Self {
        Self {
            input: Arc::clone(&self.input),
        }
    }
}

impl<D: ConstraintDomain> ConstraintSourceReceipt<D> {
    pub(crate) fn source(&self) -> ConstraintSourceId<D::Source> {
        self.input.source
    }

    pub(crate) fn matches(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.input, &other.input)
    }
}

/// A fork of one exact source path that may admit a child callable
/// application.  Its receipt prevents a child candidate from attaching to a
/// sibling source ticket.
pub(crate) struct NestedConstraintPath<D: ConstraintDomain> {
    receipt: ConstraintSourceReceipt<D>,
    path: ConstraintPath<D>,
}

/// Owner-bound handle to a result projection registered on one admitted child
/// application.  It carries no open `TypeKind`; the lower transaction opens
/// the schema term on the matching path when it submits the parent source.
pub(crate) struct ConstraintResultProjection<D: ConstraintDomain> {
    application: ConstraintApplicationId,
    key: Arc<D::Projection>,
}

impl<D: ConstraintDomain> Clone for ConstraintResultProjection<D> {
    fn clone(&self) -> Self {
        Self {
            application: self.application,
            key: Arc::clone(&self.key),
        }
    }
}

impl<D: ConstraintDomain> PartialEq for ConstraintResultProjection<D> {
    fn eq(&self, other: &Self) -> bool {
        self.application == other.application && self.key == other.key
    }
}

impl<D: ConstraintDomain> Eq for ConstraintResultProjection<D> {}

impl<D: ConstraintDomain> ConstraintResultProjection<D> {
    pub(crate) const fn application(&self) -> ConstraintApplicationId {
        self.application
    }

    pub(crate) fn key(&self) -> &D::Projection {
        &self.key
    }
}

/// Open child application alternatives waiting for their result to be related
/// to the exact parent source.  Completion remains owned by the parent path.
pub(crate) struct PendingChildConstraint<D: ConstraintDomain> {
    receipt: ConstraintSourceReceipt<D>,
    alternatives: Vec<PendingChildAlternative<D>>,
}

struct PendingChildAlternative<D: ConstraintDomain> {
    path: ConstraintPath<D>,
    result: ConstraintResultProjection<D>,
    branch: Option<Arc<D::ProbeSemanticBranch>>,
}

impl<D: ConstraintDomain> PendingChildConstraint<D> {
    pub(crate) fn with_probe_branch(mut self, branch: D::ProbeSemanticBranch) -> Self {
        let branch = Arc::new(branch);
        for alternative in &mut self.alternatives {
            alternative.branch = Some(Arc::clone(&branch));
        }
        self
    }

    pub(crate) fn append(&mut self, other: Self) -> Result<(), TypeConstraintError> {
        if !self.receipt.matches(&other.receipt) {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::WrongSource,
            ));
        }
        self.alternatives.extend(other.alternatives);
        Ok(())
    }
}

/// One source's alternatives retain their own path, term and semantic evidence.
/// Construction consumes affine probe branches; no observation is broadcast.
pub(crate) struct SourceProbeContribution<D: ConstraintDomain> {
    input: Arc<ProbeInput<D>>,
    alternatives: Vec<ObservedProbeAlternative<D>>,
}

struct ObservedProbeAlternative<D: ConstraintDomain> {
    path: ConstraintPath<D>,
    result: SourceProbeResult<D>,
}

impl<D: ConstraintDomain> SourceProbeContribution<D> {
    pub(crate) fn append(&mut self, other: Self) -> Result<(), TypeConstraintError> {
        if !Arc::ptr_eq(&self.input, &other.input) {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        self.alternatives.extend(other.alternatives);
        Ok(())
    }
}

/// Immutable source/schema observation shared while a probe updates its own
/// constraint frontier. It owns no mutable bindings or application membership.
pub(crate) struct ProbeInput<D: ConstraintDomain> {
    source: ConstraintSourceId<D::Source>,
    prepared: Arc<PreparedSourceConstraint<D>>,
    hints: Vec<OwnedAlternativeHint<D>>,
    acceptance: ConstraintAcceptance,
    equation_ordinal: Option<ConstraintEquationId>,
}

struct OwnedAlternativeHint<D: ConstraintDomain> {
    alternative: D::AlternativeIndex,
    expected: TypeKind,
    unbound: Box<[super::ConstraintGenericParameterId]>,
    scope_lease: Option<super::ImportedGenericParameterScopeLease>,
}

impl<D: ConstraintDomain> ProbeTicket<D> {
    #[cfg(test)]
    pub(super) fn test_path(&self) -> &ConstraintPath<D> {
        self.path.as_ref().expect("unobserved probe branch")
    }

    pub(crate) fn source(&self) -> ConstraintSourceId<D::Source> {
        self.input.source
    }

    pub(crate) fn input(&self) -> Arc<ProbeInput<D>> {
        Arc::clone(&self.input)
    }

    pub(crate) fn receipt(&self) -> ConstraintSourceReceipt<D> {
        ConstraintSourceReceipt {
            input: Arc::clone(&self.input),
        }
    }

    pub(crate) fn fork_for_child<A: TypeConstraintAccounting>(
        &self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<NestedConstraintPath<D>, TypeConstraintError> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::Ticket))?;
        Ok(NestedConstraintPath {
            receipt: self.receipt(),
            path: context.fork_path(path)?,
        })
    }

    pub(crate) fn fork<A: TypeConstraintAccounting>(
        &self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::Ticket))?;
        Ok(Self {
            input: Arc::clone(&self.input),
            path: Some(context.fork_path(path)?),
        })
    }

    pub(crate) fn observe(
        &mut self,
        result: SourceProbeResult<D>,
    ) -> Result<SourceProbeContribution<D>, TypeConstraintError> {
        let path = self
            .path
            .take()
            .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::Ticket))?;
        Ok(SourceProbeContribution {
            input: Arc::clone(&self.input),
            alternatives: vec![ObservedProbeAlternative { path, result }],
        })
    }

    pub(crate) fn observe_child(
        &mut self,
        pending: PendingChildConstraint<D>,
        branch: D::ProbeSemanticBranch,
        selection: SourceProbeSelection<D::AlternativeIndex, Arc<D::ObservedEvidence>>,
    ) -> Result<SourceProbeContribution<D>, TypeConstraintError> {
        if !self.receipt().matches(&pending.receipt) {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::WrongSource,
            ));
        }
        if self.path.is_none() || pending.alternatives.is_empty() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        let PendingChildConstraint {
            receipt: _,
            alternatives,
        } = pending;
        let branch = Arc::new(branch);
        let alternatives = alternatives
            .into_iter()
            .map(|alternative| ObservedProbeAlternative {
                path: alternative.path,
                result: SourceProbeResult::projection(
                    alternative.result,
                    alternative.branch.unwrap_or_else(|| Arc::clone(&branch)),
                    selection.clone(),
                ),
            })
            .collect();
        self.path.take();
        Ok(SourceProbeContribution {
            input: Arc::clone(&self.input),
            alternatives,
        })
    }
}

impl<D: ConstraintDomain> ProbeInput<D> {
    /// Build borrowed per-alternative hints for the callback lifetime.  The
    /// temporary view cannot escape this call, so the callback never owns or
    /// rewrites a lower expected type.
    pub(crate) fn with_hint<R>(
        &self,
        callback: impl for<'h> FnOnce(ExpectedHint<'h, D>) -> R,
    ) -> R {
        if self.prepared.is_unchecked() {
            return callback(ExpectedHint::Unchecked);
        }
        let hints =
            self.hints
                .iter()
                .map(|hint| {
                    let alternative = self
                        .prepared
                        .alternative(hint.alternative)
                        .expect("prepared alternative is retained by its ticket");
                    let value_expected = if hint.unbound.is_empty() {
                        ProjectedExpectedHint::Complete(&hint.expected)
                    } else {
                        ProjectedExpectedHint::Parametric {
                            expected: &hint.expected,
                            unbound: &hint.unbound,
                            scope_lease: hint.scope_lease.as_ref().expect(
                                "parametric expected hints retain their source scope lease",
                            ),
                        }
                    };
                    SourceAlternativeHint::new(
                        hint.alternative,
                        alternative.evidence(),
                        value_expected,
                        self.prepared.source_projection(),
                    )
                })
                .collect::<Vec<_>>();
        callback(ExpectedHint::Alternatives(&hints))
    }
}

pub(crate) enum ProbeSubmission<D: ConstraintDomain> {
    Accepted(SourceProbeContribution<D>),
    Rejected(D::SourceErrorCause),
}

pub(crate) struct MaterializationTicket<D: ConstraintDomain> {
    identity: MaterializationTicketIdentity,
    correlation: MaterializationCorrelationOrdinal,
    component: super::CompletedConstraintComponent<D>,
    phase: MaterializationTicketPhase,
}

enum MaterializationTicketPhase {
    Ready,
    CallbackBound,
    Closed,
}

pub(crate) struct MaterializationCallbackBinding<D: ConstraintDomain> {
    identity: MaterializationTicketIdentity,
    sources: Box<[ConstraintSourceId<D::Source>]>,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PreparedSourceOrdinal {
    application: ConstraintApplicationId,
    ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ConstraintEquationId {
    application: ConstraintApplicationId,
    ordinal: u32,
}

/// Completion precedence follows the one ordered trace, not opening issuance
/// or an application's independently numbered prepared sources.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct MaterializationSourceOrdinal(u32);

#[derive(Clone)]
struct MaterializationTicketIdentity {
    issuer: Arc<MaterializationTicketIssuer>,
    ordinal: u64,
}

struct MaterializationTicketIssuer;

struct MaterializationCorrelationOrdinal(u32);

pub(crate) struct ClosedMaterialization<D: ConstraintDomain> {
    identity: MaterializationTicketIdentity,
    submission: ClosedMaterializationSubmission<D>,
}

impl<D: ConstraintDomain> MaterializationTicket<D> {
    pub(crate) fn requests(
        &self,
    ) -> impl ExactSizeIterator<Item = MaterializedSourceRequest<'_, D>> + '_ {
        (0..self.component.sources().all().len()).map(|ordinal| {
            MaterializedSourceRequest::from_component(&self.component, ordinal)
                .expect("component source ordinal")
        })
    }

    pub(crate) fn bind_callback(
        &mut self,
    ) -> Result<MaterializationCallbackBinding<D>, TypeConstraintSourceProtocolInvariant> {
        if !matches!(self.phase, MaterializationTicketPhase::Ready) {
            return Err(TypeConstraintSourceProtocolInvariant::Ticket);
        }
        self.phase = MaterializationTicketPhase::CallbackBound;
        Ok(MaterializationCallbackBinding {
            identity: self.identity.clone(),
            sources: self
                .component
                .sources()
                .all()
                .iter()
                .map(ClosedConstraintProbe::source)
                .collect(),
        })
    }

    pub(crate) fn validate_callback_binding(
        &self,
        binding: &MaterializationCallbackBinding<D>,
    ) -> Result<(), TypeConstraintSourceProtocolInvariant> {
        if !matches!(self.phase, MaterializationTicketPhase::CallbackBound) {
            return Err(TypeConstraintSourceProtocolInvariant::Ticket);
        }
        if !materialization_identity_matches(&self.identity, &binding.identity) {
            return Err(TypeConstraintSourceProtocolInvariant::Ticket);
        }
        let expected = self
            .component
            .sources()
            .all()
            .iter()
            .map(ClosedConstraintProbe::source);
        if expected.eq(binding.sources.iter().copied()) {
            Ok(())
        } else {
            Err(TypeConstraintSourceProtocolInvariant::WrongSource)
        }
    }

    pub(crate) fn bind_closed_submission(
        &mut self,
        submission: ClosedMaterializationSubmission<D>,
    ) -> Result<ClosedMaterialization<D>, TypeConstraintSourceProtocolInvariant> {
        if !matches!(self.phase, MaterializationTicketPhase::CallbackBound) {
            return Err(TypeConstraintSourceProtocolInvariant::Ticket);
        }
        self.phase = MaterializationTicketPhase::Closed;
        Ok(ClosedMaterialization {
            identity: self.identity.clone(),
            submission,
        })
    }
}

impl<D: ConstraintDomain> MaterializationCallbackBinding<D> {
    pub(crate) fn sources(&self) -> &[ConstraintSourceId<D::Source>] {
        &self.sources
    }

    pub(crate) fn authorizes(&self, source: &ConstraintSourceId<D::Source>) -> bool {
        self.sources.iter().any(|candidate| candidate == source)
    }
}

fn materialization_identity_matches(
    left: &MaterializationTicketIdentity,
    right: &MaterializationTicketIdentity,
) -> bool {
    Arc::ptr_eq(&left.issuer, &right.issuer) && left.ordinal == right.ordinal
}

pub(super) struct ProjectionRequest<D: ConstraintDomain> {
    application: ConstraintApplicationId,
    key: Arc<D::Projection>,
    value: TypeKind,
    closure: TypeConstraintProjectionClosure,
    source: Option<ConstraintResultProjection<D>>,
    input: Option<TypeKind>,
}

struct ProbeOperation<D: ConstraintDomain> {
    source: ConstraintSourceId<D::Source>,
    source_ordinal: PreparedSourceOrdinal,
    prepared: Arc<PreparedSourceConstraint<D>>,
    acceptance: ConstraintAcceptance,
    equation_ordinal: Option<ConstraintEquationId>,
    rows: VecDeque<ConstraintPath<D>>,
    active_input: Option<Arc<ProbeInput<D>>>,
    advanced: Vec<ConstraintPath<D>>,
    rejections: Vec<D::SourceErrorCause>,
    relation_rejections: Vec<RejectedConstraintSourceProjection<D>>,
    deferred_rejection_tail: bool,
}

impl<D: ConstraintDomain> ProbeOperation<D> {
    fn into_ordinary_rejection(self) -> TypeConstraintFailure<D> {
        if !self.rejections.is_empty() {
            return TypeConstraintFailure::Rejected(TypeConstraintCandidateFailure::Source(
                Box::new(SourceError::new(
                    self.source,
                    SourcePhase::Probe,
                    self.rejections.into_boxed_slice(),
                )),
            ));
        }
        if let Some(rejection) = self.relation_rejections.into_iter().next() {
            return TypeConstraintFailure::Rejected(
                TypeConstraintCandidateFailure::SourceProjection(Box::new(rejection)),
            );
        }
        TypeConstraintError::Rejected(TypeConstraintRejection::Mismatch).into()
    }
}

/// One mapper-issued authored-source group. If an earlier slot eliminates
/// the semantic frontier, later slots still execute against the last live
/// correlated rows, but their results cannot resurrect the failed group.
struct ProbeGroup<D: ConstraintDomain> {
    remaining_sources: usize,
    last_live_frontier: Vec<ConstraintPath<D>>,
    deferred_failure: Option<TypeConstraintFailure<D>>,
    ordinary_rejection_pending: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProbeStart {
    Started,
    Skipped,
}

enum MaterializedRecord<D: ConstraintDomain> {
    Sealed {
        component: super::CompletedConstraintComponent<D>,
        value: D::SealedBranchValue,
    },
    Rejected {
        source_ordinal: MaterializationSourceOrdinal,
        correlation: MaterializationCorrelationOrdinal,
        source: ConstraintSourceId<D::Source>,
        cause: D::SourceErrorCause,
    },
    Fatal {
        source_ordinal: MaterializationSourceOrdinal,
        correlation: MaterializationCorrelationOrdinal,
        error: SourceError<ConstraintSourceId<D::Source>, D::SourceErrorCause>,
    },
}

enum NormalizedPath<D: ConstraintDomain> {
    Acyclic(ConstraintPath<D>),
    Cyclic(super::ConstraintGenericParameterId),
}

pub(crate) struct TypeConstraintTransaction<D: ConstraintDomain> {
    application: ConstraintApplicationId,
    parent_receipt: Option<ConstraintSourceReceipt<D>>,
    frontier: Vec<ConstraintPath<D>>,
    first_failure: Option<TypeConstraintFailure<D>>,
    next_equation: u32,
    next_source_ordinal: u32,
    first_cycle: Option<super::ConstraintGenericParameterId>,
    next_correlation_ordinal: u32,
    next_materialization_ticket_ordinal: u64,
    materialization_issuer: Arc<MaterializationTicketIssuer>,
    active_materialization: Option<MaterializationTicketIdentity>,
    probe: Option<ProbeOperation<D>>,
    probe_group: Option<ProbeGroup<D>>,
    prepared_sources: BTreeSet<ConstraintSourceId<D::Source>>,
    materialization: VecDeque<MaterializationTicket<D>>,
    materialized: Vec<MaterializedRecord<D>>,
    closed: bool,
}

impl<D: ConstraintDomain> TypeConstraintTransaction<D> {
    #[cfg(test)]
    pub(super) fn test_single_path(&self) -> &ConstraintPath<D> {
        assert_eq!(
            self.frontier.len(),
            1,
            "fixture has one current alternative"
        );
        &self.frontier[0]
    }

    /// A transaction always names an application already admitted on its path.
    /// Template opening must use that application, which may be a descendant
    /// of the path's root.
    pub(super) fn from_path<A: TypeConstraintAccounting>(
        context: &mut TypeConstraintContext<'_, A, D>,
        application: ConstraintApplicationId,
        path: ConstraintPath<D>,
        inherited: Option<&TypeConstraintSolution>,
    ) -> Result<Self, TypeConstraintError> {
        Self::from_path_with_parent(context, application, path, inherited, None)
    }

    fn from_path_with_parent<A: TypeConstraintAccounting>(
        context: &mut TypeConstraintContext<'_, A, D>,
        application: ConstraintApplicationId,
        path: ConstraintPath<D>,
        inherited: Option<&TypeConstraintSolution>,
        parent_receipt: Option<ConstraintSourceReceipt<D>>,
    ) -> Result<Self, TypeConstraintError> {
        let path = Self::prepare_application(context, application, path, inherited)?;
        Ok(Self {
            application,
            parent_receipt,
            frontier: vec![path],
            first_failure: None,
            next_equation: 0,
            next_source_ordinal: 0,
            first_cycle: None,
            next_correlation_ordinal: 0,
            next_materialization_ticket_ordinal: 0,
            materialization_issuer: Arc::new(MaterializationTicketIssuer),
            active_materialization: None,
            probe: None,
            probe_group: None,
            prepared_sources: BTreeSet::new(),
            materialization: VecDeque::new(),
            materialized: Vec::new(),
            closed: false,
        })
    }

    pub(crate) fn initialize_from_nested_path<A>(
        context: &mut TypeConstraintContext<'_, A, D>,
        application: D::Application,
        parameters: super::TypeConstraintParameterScope,
        inherited: Option<Arc<TypeConstraintSolution>>,
        nested: NestedConstraintPath<D>,
    ) -> Result<Self, super::TypeConstraintInitializationFailure>
    where
        A: TypeConstraintAccounting,
    {
        let scope = ConstraintApplicationScope::new(application, parameters);
        let application = scope.id();
        let result = context
            .admit_application(nested.path, scope)
            .and_then(|path| {
                Self::from_path_with_parent(
                    context,
                    application,
                    path,
                    inherited.as_deref(),
                    Some(nested.receipt),
                )
            });
        result.map_err(|error| match error {
            TypeConstraintError::Abort(error) => {
                super::TypeConstraintInitializationFailure::Abort(error)
            }
            TypeConstraintError::Invariant(error) => {
                super::TypeConstraintInitializationFailure::Invariant(error)
            }
            TypeConstraintError::Rejected(_) => {
                super::TypeConstraintInitializationFailure::Invariant(
                    super::TypeConstraintInvariant::InheritedSolution(
                        super::InheritedSolutionInvariant {
                            kind: super::InheritedSolutionInvariantKind::Forbidden,
                            parameter: None,
                        },
                    ),
                )
            }
        })
    }

    pub(crate) fn initialize<A>(
        context: &mut TypeConstraintContext<'_, A, D>,
        application: D::Application,
        parameters: super::TypeConstraintParameterScope,
        inherited: Option<Arc<TypeConstraintSolution>>,
    ) -> Result<Self, super::TypeConstraintInitializationFailure>
    where
        A: TypeConstraintAccounting,
    {
        Self::initialize_with_imported(context, application, parameters, inherited, None)
    }

    pub(crate) fn initialize_with_imported<A>(
        context: &mut TypeConstraintContext<'_, A, D>,
        application: D::Application,
        parameters: super::TypeConstraintParameterScope,
        inherited: Option<Arc<TypeConstraintSolution>>,
        imported: Option<super::ImportedGenericParameterScopeLease>,
    ) -> Result<Self, super::TypeConstraintInitializationFailure>
    where
        A: TypeConstraintAccounting,
    {
        let scope = ConstraintApplicationScope::new(application, parameters);
        let application = scope.id();
        match context
            .start_path_with_imported(scope, imported)
            .and_then(|path| Self::from_path(context, application, path, inherited.as_deref()))
        {
            Ok(transaction) => Ok(transaction),
            Err(error) => Err(match error {
                TypeConstraintError::Abort(error) => {
                    super::TypeConstraintInitializationFailure::Abort(error)
                }
                TypeConstraintError::Invariant(error) => {
                    super::TypeConstraintInitializationFailure::Invariant(error)
                }
                TypeConstraintError::Rejected(_) => {
                    super::TypeConstraintInitializationFailure::Invariant(
                        super::TypeConstraintInvariant::InheritedSolution(
                            super::InheritedSolutionInvariant {
                                kind: super::InheritedSolutionInvariantKind::Forbidden,
                                parameter: None,
                            },
                        ),
                    )
                }
            }),
        }
    }

    pub(crate) fn constrain<A>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        pattern: &TypeKind,
        actual: &TypeKind,
        acceptance: ConstraintAcceptance,
    ) where
        A: TypeConstraintAccounting,
    {
        if self.first_failure.is_some() || self.closed {
            return;
        }
        let ordinal = match self.next_equation.checked_add(1) {
            Some(next) => {
                self.next_equation = next;
                next - 1
            }
            None => {
                self.first_failure = Some(
                    TypeConstraintError::Abort(TypeConstraintAbort::ArithmeticOverflow).into(),
                );
                return;
            }
        };
        let frontier = core::mem::take(&mut self.frontier);
        let mut advanced = Vec::new();
        for mut path in frontier {
            let pattern = match context.open_template_type(pattern, &path, self.application) {
                Ok(pattern) => pattern,
                Err(error) => {
                    self.first_failure = Some(error.into());
                    return;
                }
            };
            path.equations.push(PendingEquation {
                ordinal: ConstraintEquationId {
                    application: self.application,
                    ordinal,
                },
                direction: acceptance,
                pattern: pattern.clone(),
                actual: actual.clone(),
                source_ordinal: None,
                final_expected: None,
            });
            match relate_selected_call(&pattern, actual, path, context, acceptance) {
                Ok(paths) => advanced.extend(paths),
                Err(error) => {
                    self.first_failure = Some(error.into());
                    return;
                }
            }
        }
        if advanced.is_empty() {
            self.first_failure =
                Some(TypeConstraintError::Rejected(TypeConstraintRejection::Mismatch).into());
        } else {
            self.frontier = advanced;
        }
    }

    /// Adds fixed row evidence to the same path-local effect environment used
    /// by function-type relations. Both directions are needed: a known row
    /// cannot grow to satisfy another use or collapse to the empty solution.
    pub(crate) fn constrain_effect_equality<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        left: &crate::effect_row::EffectRow,
        right: &crate::effect_row::EffectRow,
    ) {
        if self.first_failure.is_some() || self.closed {
            return;
        }
        let frontier = core::mem::take(&mut self.frontier);
        let mut advanced = Vec::with_capacity(frontier.len());
        let mut rejection = None;
        for mut path in frontier {
            if let Err(error) = context
                .validate_effect_row(left, path.projection_view())
                .and_then(|()| context.validate_effect_row(right, path.projection_view()))
            {
                self.first_failure = Some(error.into());
                return;
            }
            let constrained = (|| {
                context.enter_node()?;
                path.effects.constrain_subset(left, right, context)?;
                context.enter_node()?;
                path.effects.constrain_subset(right, left, context)
            })();
            match constrained {
                Ok(()) => advanced.push(path),
                Err(TypeConstraintError::Rejected(error)) => {
                    rejection.get_or_insert(error);
                }
                Err(error) => {
                    self.first_failure = Some(error.into());
                    return;
                }
            }
        }
        if advanced.is_empty() {
            self.first_failure = Some(
                TypeConstraintError::Rejected(
                    rejection.unwrap_or(TypeConstraintRejection::Mismatch),
                )
                .into(),
            );
        } else {
            self.frontier = advanced;
        }
    }

    pub(crate) fn request_projection<A: TypeConstraintAccounting>(
        &mut self,
        _context: &mut TypeConstraintContext<'_, A, D>,
        key: D::Projection,
        value: &TypeKind,
        closure: TypeConstraintProjectionClosure,
    ) {
        self.request_projection_inner(key, value, closure, None);
    }

    pub(crate) fn request_value_use_projection<A: TypeConstraintAccounting>(
        &mut self,
        _context: &mut TypeConstraintContext<'_, A, D>,
        key: D::Projection,
        input: &TypeKind,
        value: &TypeKind,
    ) {
        self.request_projection_inner(
            key,
            value,
            TypeConstraintProjectionClosure::Closed,
            Some(input.clone()),
        );
    }

    fn request_projection_inner(
        &mut self,
        key: D::Projection,
        value: &TypeKind,
        closure: TypeConstraintProjectionClosure,
        input: Option<TypeKind>,
    ) {
        if self.first_failure.is_none() && !self.closed {
            if self.probe.is_some() || self.probe_group.is_some() {
                self.record_failure(
                    protocol_error(TypeConstraintSourceProtocolInvariant::Outcome).into(),
                );
                return;
            }
            let request = Arc::new(ProjectionRequest {
                application: self.application,
                key: Arc::new(key),
                value: value.clone(),
                closure,
                source: None,
                input,
            });
            for path in &mut self.frontier {
                path.projections.push(Arc::clone(&request));
            }
        }
    }

    pub(crate) fn record_failure(&mut self, failure: TypeConstraintFailure<D>) {
        if self.first_failure.is_none() {
            self.first_failure = Some(failure);
        }
        if self.probe.is_some() {
            self.probe = None;
            self.probe_group = None;
            self.frontier.clear();
        }
    }

    /// The only source entry point: a callable mapper supplies the complete
    /// prepared source constraint before any callback can run.
    pub(crate) fn begin_prepared_probe<A>(
        &mut self,
        _context: &mut TypeConstraintContext<'_, A, D>,
        prepared: PreparedSourceConstraint<D>,
        acceptance: ConstraintAcceptance,
    ) -> Result<ProbeStart, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        self.begin_probe_inner(prepared, acceptance)
    }

    pub(crate) fn begin_prepared_probe_group(
        &mut self,
        source_count: usize,
    ) -> Result<ProbeStart, TypeConstraintError> {
        if source_count == 0 || self.probe.is_some() || self.probe_group.is_some() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            ));
        }
        if self.first_failure.is_some() || self.closed {
            return Ok(ProbeStart::Skipped);
        }
        self.probe_group = Some(ProbeGroup {
            remaining_sources: source_count,
            last_live_frontier: self.frontier.clone(),
            deferred_failure: None,
            ordinary_rejection_pending: false,
        });
        Ok(ProbeStart::Started)
    }

    fn begin_probe_inner(
        &mut self,
        prepared: PreparedSourceConstraint<D>,
        acceptance: ConstraintAcceptance,
    ) -> Result<ProbeStart, TypeConstraintError> {
        if self.first_failure.is_some() || self.closed {
            if let Some(mut group) = self.probe_group.take() {
                group.remaining_sources =
                    group.remaining_sources.checked_sub(1).ok_or_else(|| {
                        protocol_error(TypeConstraintSourceProtocolInvariant::Outcome)
                    })?;
                if group.remaining_sources != 0 {
                    self.probe_group = Some(group);
                }
            }
            return Ok(ProbeStart::Skipped);
        }
        if self.probe.is_some() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            ));
        }
        prepared.validate()?;
        let source = ConstraintSourceId::new(self.application, prepared.source());
        if !self.prepared_sources.insert(source) {
            return Err(TypeConstraintError::Invariant(
                TypeConstraintInvariant::PreparedSource(
                    super::PreparedSourceConstraintInvariant::DuplicateCoordinate,
                ),
            ));
        }
        let equation_ordinal = if !prepared.is_unchecked() {
            let ordinal = self.next_equation;
            self.next_equation =
                self.next_equation
                    .checked_add(1)
                    .ok_or(TypeConstraintError::Abort(
                        TypeConstraintAbort::ArithmeticOverflow,
                    ))?;
            Some(ConstraintEquationId {
                application: self.application,
                ordinal,
            })
        } else {
            None
        };
        let source_ordinal = PreparedSourceOrdinal {
            application: self.application,
            ordinal: self.next_source_ordinal,
        };
        self.next_source_ordinal =
            self.next_source_ordinal
                .checked_add(1)
                .ok_or(TypeConstraintError::Abort(
                    TypeConstraintAbort::ArithmeticOverflow,
                ))?;
        let deferred_rejection_tail = self
            .probe_group
            .as_ref()
            .is_some_and(|group| group.ordinary_rejection_pending);
        let rows = if deferred_rejection_tail {
            self.probe_group
                .as_ref()
                .expect("a deferred rejection tail requires an active source group")
                .last_live_frontier
                .clone()
                .into()
        } else {
            if let Some(group) = self.probe_group.as_mut() {
                group.last_live_frontier = self.frontier.clone();
            }
            core::mem::take(&mut self.frontier).into()
        };
        self.probe = Some(ProbeOperation {
            source,
            source_ordinal,
            prepared: Arc::new(prepared),
            acceptance,
            equation_ordinal,
            rows,
            active_input: None,
            advanced: Vec::new(),
            rejections: Vec::new(),
            relation_rejections: Vec::new(),
            deferred_rejection_tail,
        });
        Ok(ProbeStart::Started)
    }

    pub(crate) fn next_probe<A>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Option<ProbeTicket<D>>, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        let Some(operation) = self.probe.as_mut() else {
            return Ok(None);
        };
        if operation.active_input.is_some() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        let Some(path) = operation.rows.pop_front() else {
            let operation = self.probe.take().expect("probe operation exists");
            if let Some(mut group) = self.probe_group.take() {
                group.remaining_sources =
                    group.remaining_sources.checked_sub(1).ok_or_else(|| {
                        protocol_error(TypeConstraintSourceProtocolInvariant::Outcome)
                    })?;
                // Fatal/abort/invariant callbacks set `first_failure` and
                // therefore outrank the deferred ordinary group rejection.
                // Only accepted/rejected ordinary tail results reach this
                // branch and neither may resurrect the semantic frontier.
                if self.first_failure.is_none() && !operation.deferred_rejection_tail {
                    if operation.advanced.is_empty() {
                        group.ordinary_rejection_pending = true;
                        group.deferred_failure = Some(operation.into_ordinary_rejection());
                    } else {
                        self.frontier = operation.advanced;
                        group.last_live_frontier = self.frontier.clone();
                    }
                }
                if group.remaining_sources == 0 {
                    if self.first_failure.is_none() && group.ordinary_rejection_pending {
                        self.first_failure = group.deferred_failure;
                    }
                } else {
                    self.probe_group = Some(group);
                }
            } else if operation.advanced.is_empty() && self.first_failure.is_none() {
                self.first_failure = Some(operation.into_ordinary_rejection());
            } else if self.first_failure.is_none() {
                self.frontier = operation.advanced;
            }
            return Ok(None);
        };
        let mut hints = Vec::new();
        for alternative in operation.prepared.alternatives() {
            let expected = context.open_template_type(
                alternative.value_expected(),
                &path,
                self.application,
            )?;
            let projected = project_type(
                &expected,
                path.projection_view(),
                ConstraintClosurePolicy::Hint,
                context,
            )?;
            let unbound = projected
                .remaining
                .iter()
                .map(|parameter| parameter.parameter().clone())
                .collect::<Box<[_]>>();
            let scope_lease = if unbound.is_empty() {
                None
            } else {
                Some(path.applications.import_parameters(&unbound)?)
            };
            hints.push(OwnedAlternativeHint {
                alternative: alternative.alternative(),
                expected: projected.value,
                unbound,
                scope_lease,
            });
        }
        let input = Arc::new(ProbeInput {
            source: operation.source,
            prepared: Arc::clone(&operation.prepared),
            hints,
            acceptance: operation.acceptance,
            equation_ordinal: operation.equation_ordinal,
        });
        operation.active_input = Some(Arc::clone(&input));
        Ok(Some(ProbeTicket {
            input,
            path: Some(path),
        }))
    }

    pub(crate) fn submit_probe<A>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        input: Arc<ProbeInput<D>>,
        submission: ProbeSubmission<D>,
    ) -> Result<(), TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        let operation = self
            .probe
            .as_mut()
            .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::Ticket))?;
        if !operation
            .active_input
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(active, &input))
        {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        if let ProbeSubmission::Accepted(contribution) = &submission
            && !Arc::ptr_eq(&input, &contribution.input)
        {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        operation.active_input = None;
        match submission {
            ProbeSubmission::Rejected(cause) => operation.rejections.push(cause),
            ProbeSubmission::Accepted(contribution) => {
                for ObservedProbeAlternative { mut path, result } in contribution.alternatives {
                    let (actual_term, branch, callback_selection) = result.into_parts();
                    let (actual, result_origin) = resolve_probe_term(actual_term, &path, context)?;
                    let callback_selection = match callback_selection {
                        SourceProbeSelection::Unchecked => SourceProbeSelection::Unchecked,
                        SourceProbeSelection::Checked {
                            alternative,
                            evidence,
                        } => SourceProbeSelection::Checked {
                            alternative,
                            evidence,
                        },
                    };
                    if path
                        .probe_trace
                        .iter()
                        .any(|probe| probe.source() == input.source)
                    {
                        return Err(protocol_error(
                            TypeConstraintSourceProtocolInvariant::Outcome,
                        ));
                    }

                    let selected = if input.prepared.is_unchecked() {
                        if !matches!(callback_selection, SourceProbeSelection::Unchecked) {
                            return Err(protocol_error(
                                TypeConstraintSourceProtocolInvariant::InvalidEvidence,
                            ));
                        }
                        None
                    } else {
                        let SourceProbeSelection::Checked {
                            alternative,
                            evidence,
                        } = &callback_selection
                        else {
                            return Err(protocol_error(
                                TypeConstraintSourceProtocolInvariant::UnknownAlternative,
                            ));
                        };
                        match validate_source_selection(
                            &input.prepared,
                            *alternative,
                            Arc::clone(evidence),
                            &actual,
                        )? {
                            Some(selected) => Some(selected),
                            None => continue,
                        }
                    };

                    let (pattern, stored_selection, source_projection, value_expected) =
                        match selected {
                            None => {
                                let Some(source_projection) =
                                    CheckedConstraintSourceProjection::derive(
                                        input.prepared.source_projection(),
                                        &actual,
                                    )
                                else {
                                    continue;
                                };
                                (
                                    None,
                                    StoredSourceSelection::Unchecked,
                                    source_projection,
                                    None,
                                )
                            }
                            Some((alternative, evidence, value_expected, source_projection)) => {
                                let value_expected = context.open_template_type(
                                    &value_expected,
                                    &path,
                                    self.application,
                                )?;
                                let pattern = source_projection.compose_expected(&value_expected);
                                (
                                    Some(pattern),
                                    StoredSourceSelection::Checked {
                                        alternative,
                                        evidence,
                                    },
                                    source_projection,
                                    Some(value_expected),
                                )
                            }
                        };

                    if let Some(expected) = pattern.as_ref() {
                        path.equations.push(PendingEquation {
                            ordinal: input.equation_ordinal.ok_or_else(|| {
                                protocol_error(TypeConstraintSourceProtocolInvariant::Outcome)
                            })?,
                            direction: input.acceptance,
                            pattern: expected.clone(),
                            actual: actual.clone(),
                            source_ordinal: Some(operation.source_ordinal),
                            final_expected: None,
                        });
                    }
                    let rejected_alternative = match &stored_selection {
                        StoredSourceSelection::Checked { alternative, .. } => Some(*alternative),
                        StoredSourceSelection::Unchecked => None,
                    };
                    let relation_rejection = pattern.as_ref().map(|expected| {
                        RejectedConstraintSourceProjection::new(
                            input.source,
                            rejected_alternative,
                            source_projection.clone(),
                            input.acceptance,
                            expected.clone(),
                            actual.clone(),
                        )
                    });
                    let probe = ConstraintProbe::Active(ActiveConstraintProbe {
                        source: input.source,
                        source_ordinal: operation.source_ordinal,
                        branch: Arc::clone(&branch),
                        selection: stored_selection,
                        prepared_source_projection: input.prepared.source_projection(),
                        value_expected,
                        result_origin,
                        actual: actual.clone(),
                    });
                    path.probe_trace.push(probe);

                    let related = if let Some(expected) = pattern.as_ref() {
                        relate_selected_call(expected, &actual, path, context, input.acceptance)
                    } else {
                        validate_type(&actual, path.projection_view(), context).map(|()| vec![path])
                    };
                    match related {
                        Ok(paths) if paths.is_empty() => {
                            if let Some(rejection) = relation_rejection {
                                operation.relation_rejections.push(rejection);
                            }
                        }
                        Ok(paths) => operation.advanced.extend(paths),
                        Err(TypeConstraintError::Rejected(TypeConstraintRejection::Mismatch)) => {
                            if let Some(rejection) = relation_rejection {
                                operation.relation_rejections.push(rejection);
                            }
                        }
                        Err(error) => {
                            self.first_failure = Some(error.into());
                            operation.rows.clear();
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn next_materialization_ticket<A>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Option<MaterializationTicket<D>>, MaterializationImmediateFailure<D>>
    where
        A: TypeConstraintAccounting,
    {
        if !self.closed {
            match self.close(context) {
                Ok(()) => {}
                // An incomplete or inconsistent component has no facts to
                // materialize. `close` retains the typed rejection for finish.
                Err(TypeConstraintError::Rejected(_)) => return Ok(None),
                Err(error) => return Err(materialization_immediate(error)),
            }
        }
        if self.active_materialization.is_some() {
            return Err(materialization_immediate(TypeConstraintError::Invariant(
                TypeConstraintInvariant::SourceProtocol(
                    TypeConstraintSourceProtocolInvariant::Ticket,
                ),
            )));
        }
        let Some(ticket) = self.materialization.pop_front() else {
            return Ok(None);
        };
        self.active_materialization = Some(ticket.identity.clone());
        if !matches!(ticket.phase, MaterializationTicketPhase::Ready) {
            self.active_materialization = None;
            return Err(materialization_immediate(TypeConstraintError::Invariant(
                TypeConstraintInvariant::SourceProtocol(
                    TypeConstraintSourceProtocolInvariant::Ticket,
                ),
            )));
        }
        Ok(Some(ticket))
    }

    pub(crate) fn validate_materialization_callback_begin(
        &self,
        ticket: &MaterializationTicket<D>,
    ) -> Result<(), TypeConstraintSourceProtocolInvariant> {
        if !materialization_identity_matches(
            &ticket.identity,
            &MaterializationTicketIdentity {
                issuer: Arc::clone(&self.materialization_issuer),
                ordinal: ticket.identity.ordinal,
            },
        ) {
            return Err(TypeConstraintSourceProtocolInvariant::Ticket);
        }
        if !self
            .active_materialization
            .as_ref()
            .is_some_and(|identity| materialization_identity_matches(identity, &ticket.identity))
        {
            return Err(TypeConstraintSourceProtocolInvariant::Ticket);
        }
        if !matches!(ticket.phase, MaterializationTicketPhase::Ready) {
            return Err(TypeConstraintSourceProtocolInvariant::Ticket);
        }
        Ok(())
    }

    pub(crate) fn submit_closed_materialization(
        &mut self,
        ticket: MaterializationTicket<D>,
        closed: ClosedMaterialization<D>,
    ) -> Result<(), MaterializationImmediateFailure<D>> {
        if !materialization_identity_matches(
            &ticket.identity,
            &MaterializationTicketIdentity {
                issuer: Arc::clone(&self.materialization_issuer),
                ordinal: ticket.identity.ordinal,
            },
        ) || !self
            .active_materialization
            .as_ref()
            .is_some_and(|identity| materialization_identity_matches(identity, &ticket.identity))
        {
            return Err(materialization_immediate(TypeConstraintError::Invariant(
                TypeConstraintInvariant::SourceProtocol(
                    TypeConstraintSourceProtocolInvariant::Ticket,
                ),
            )));
        }
        if !matches!(ticket.phase, MaterializationTicketPhase::Closed)
            || !materialization_identity_matches(&ticket.identity, &closed.identity)
        {
            return Err(materialization_immediate(TypeConstraintError::Invariant(
                TypeConstraintInvariant::SourceProtocol(
                    TypeConstraintSourceProtocolInvariant::Outcome,
                ),
            )));
        }
        self.active_materialization = None;
        let MaterializationTicket {
            correlation,
            component,
            ..
        } = ticket;
        match closed.submission {
            ClosedMaterializationSubmission::Sealed(value) => {
                self.materialized
                    .push(MaterializedRecord::Sealed { component, value });
            }
            ClosedMaterializationSubmission::Rejected { source, cause } => {
                let source_ordinal = unique_request_ordinal(component.sources().all(), source)
                    .map_err(materialization_immediate)?;
                self.materialized.push(MaterializedRecord::Rejected {
                    source_ordinal,
                    correlation,
                    source,
                    cause,
                });
            }
            ClosedMaterializationSubmission::Fatal(error) => {
                if error.phase() != SourcePhase::Materialize {
                    return Err(materialization_immediate(protocol_error(
                        TypeConstraintSourceProtocolInvariant::WrongPhase,
                    )));
                }
                let source_ordinal =
                    unique_request_ordinal(component.sources().all(), *error.source())
                        .map_err(materialization_immediate)?;
                self.materialized.push(MaterializedRecord::Fatal {
                    source_ordinal,
                    correlation,
                    error,
                });
            }
        }
        Ok(())
    }

    pub(crate) fn finish<A>(
        self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<SolvedCandidate<D>, TypeConstraintFailure<D>>
    where
        A: TypeConstraintAccounting,
    {
        self.finish_alternatives(context)?.into_unique()
    }

    pub(crate) fn finish_alternatives<A>(
        mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<super::CompletedCandidateAlternatives<D>, TypeConstraintFailure<D>>
    where
        A: TypeConstraintAccounting,
    {
        match self.close(context) {
            Ok(()) => self.finish_candidate_alternatives(context),
            Err(error) => Err(error.into()),
        }
    }

    pub(crate) fn defer_child_result(
        self,
        key: D::Projection,
    ) -> Result<PendingChildConstraint<D>, TypeConstraintFailure<D>> {
        let Some(receipt) = self.parent_receipt else {
            return Err(TypeConstraintFailure::Invariant(
                TypeConstraintFailureInvariant::Constraint(
                    TypeConstraintInvariant::SourceProtocol(
                        TypeConstraintSourceProtocolInvariant::WrongPhase,
                    ),
                ),
            ));
        };
        if self.probe.is_some() || self.probe_group.is_some() || self.closed {
            return Err(TypeConstraintFailure::Invariant(
                TypeConstraintFailureInvariant::Constraint(
                    TypeConstraintInvariant::SourceProtocol(
                        TypeConstraintSourceProtocolInvariant::WrongPhase,
                    ),
                ),
            ));
        }
        if let Some(failure) = self.first_failure {
            return Err(failure);
        }
        if self.frontier.is_empty() {
            return Err(TypeConstraintFailure::Rejected(
                TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::UnresolvedType),
            ));
        }
        for path in &self.frontier {
            let mut requests = path.projections.iter().filter(|request| {
                request.application == self.application && request.key.as_ref() == &key
            });
            if requests.next().is_none() || requests.next().is_some() {
                return Err(TypeConstraintFailure::Invariant(
                    TypeConstraintFailureInvariant::Constraint(
                        TypeConstraintInvariant::Projection(
                            TypeConstraintProjectionInvariant::MissingKey,
                        ),
                    ),
                ));
            }
        }
        let key = Arc::new(key);
        Ok(PendingChildConstraint {
            receipt,
            alternatives: self
                .frontier
                .into_iter()
                .map(|path| PendingChildAlternative {
                    path,
                    result: ConstraintResultProjection {
                        application: self.application,
                        key: Arc::clone(&key),
                    },
                    branch: None,
                })
                .collect(),
        })
    }

    fn close<A>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        if self.closed {
            return Ok(());
        }
        if self.probe.is_some() || self.probe_group.is_some() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            ));
        }
        self.closed = true;
        if self.first_failure.is_some() {
            return Ok(());
        }
        let result = self.close_frontier(context);
        if let Err(error) = &result {
            self.record_failure(error.clone().into());
            self.materialization.clear();
            self.materialized.clear();
        }
        result
    }

    fn close_frontier<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        let frontier = core::mem::take(&mut self.frontier);
        let mut acyclic = Vec::new();
        let mut completion_rejection = None;
        for path in frontier {
            match self.normalize_path(path, context) {
                Ok(NormalizedPath::Acyclic(path)) => acyclic.push(path),
                Ok(NormalizedPath::Cyclic(parameter)) => {
                    if self.first_cycle.is_none() {
                        self.first_cycle = Some(parameter);
                    }
                }
                Err(TypeConstraintError::Rejected(rejection)) => {
                    completion_rejection.get_or_insert(rejection);
                }
                Err(error) => return Err(error),
            }
        }
        if acyclic.is_empty() {
            return completion_rejection.map_or(Ok(()), |rejection| Err(rejection.into()));
        }
        self.first_cycle = None;
        acyclic.sort_by(path_correlation_cmp::<D>);

        let mut groups: Vec<Vec<super::CompletedConstraintComponent<D>>> = Vec::new();
        for mut path in acyclic {
            let component = match Self::complete_component(&mut path, self.application, context) {
                Ok(component) => component,
                Err(TypeConstraintError::Rejected(rejection)) => {
                    completion_rejection.get_or_insert(rejection);
                    continue;
                }
                Err(error) => return Err(error),
            };
            let mut group_index = None;
            for (index, group) in groups.iter().enumerate() {
                if let Some(first) = group.first()
                    && first.equal_with(&component, context)?
                {
                    group_index = Some(index);
                    break;
                }
            }
            if let Some(index) = group_index {
                groups[index].push(component);
            } else {
                groups.push(vec![component]);
            }
        }
        if groups.is_empty()
            && let Some(rejection) = completion_rejection
        {
            return Err(rejection.into());
        }
        for component in groups.into_iter().flatten() {
            if component.sources().all().is_empty() {
                self.materialized.push(MaterializedRecord::Sealed {
                    component,
                    value: D::empty_sealed_branch(),
                });
                continue;
            }
            let correlation = MaterializationCorrelationOrdinal(self.next_correlation_ordinal);
            self.next_correlation_ordinal =
                self.next_correlation_ordinal
                    .checked_add(1)
                    .ok_or(TypeConstraintError::Abort(
                        TypeConstraintAbort::ArithmeticOverflow,
                    ))?;
            let ticket_ordinal = self.next_materialization_ticket_ordinal;
            self.next_materialization_ticket_ordinal =
                ticket_ordinal
                    .checked_add(1)
                    .ok_or(TypeConstraintError::Abort(
                        TypeConstraintAbort::ArithmeticOverflow,
                    ))?;
            self.materialization.push_back(MaterializationTicket {
                identity: MaterializationTicketIdentity {
                    issuer: Arc::clone(&self.materialization_issuer),
                    ordinal: ticket_ordinal,
                },
                correlation,
                component,
                phase: MaterializationTicketPhase::Ready,
            });
        }
        Ok(())
    }

    fn normalize_path<A>(
        &mut self,
        path: ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<NormalizedPath<D>, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        let cyclic = path.deferred_cycles.parameters.iter().next().cloned();
        let path = seal_path(path, context)?;
        if let Some(parameter) = cyclic {
            if self.first_cycle.is_none() {
                self.first_cycle = Some(parameter.clone());
            }
            return Ok(NormalizedPath::Cyclic(parameter));
        }
        Ok(NormalizedPath::Acyclic(path))
    }

    fn finish_candidate_alternatives<A>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<super::CompletedCandidateAlternatives<D>, TypeConstraintFailure<D>>
    where
        A: TypeConstraintAccounting,
    {
        if let Some(error) = self.first_failure.take() {
            return Err(error);
        }
        if self.active_materialization.is_some() {
            return Err(protocol_error(TypeConstraintSourceProtocolInvariant::Outcome).into());
        }
        if !self.materialization.is_empty() {
            return Err(
                TypeConstraintError::Rejected(TypeConstraintRejection::UnresolvedType).into(),
            );
        }
        let fatal_index = self
            .materialized
            .iter()
            .enumerate()
            .filter_map(|(index, record)| match record {
                MaterializedRecord::Fatal {
                    source_ordinal,
                    correlation,
                    ..
                } => Some((index, *source_ordinal, correlation)),
                _ => None,
            })
            .min_by(|left, right| left.1.cmp(&right.1).then_with(|| left.2.0.cmp(&right.2.0)))
            .map(|entry| entry.0);
        if let Some(index) = fatal_index {
            let MaterializedRecord::Fatal { error, .. } = self.materialized.remove(index) else {
                unreachable!("fatal index identifies fatal record")
            };
            let (source, phase, cause) = error.into_parts();
            return Err(TypeConstraintFailure::fatal_source(SourceError::new(
                source, phase, cause,
            )));
        }
        let mut candidates: Vec<(super::CompletedConstraintComponent<D>, D::SealedBranchValue)> =
            Vec::new();
        let mut rejected = Vec::new();
        for record in core::mem::take(&mut self.materialized) {
            match record {
                MaterializedRecord::Sealed { component, value } => {
                    let mut duplicate = false;
                    for (existing_component, existing_value) in &candidates {
                        if existing_component.equal_with(&component, context)?
                            && existing_value == &value
                        {
                            duplicate = true;
                            break;
                        }
                    }
                    if !duplicate {
                        candidates.push((component, value));
                    }
                }
                MaterializedRecord::Rejected {
                    source_ordinal,
                    correlation,
                    source,
                    cause,
                } => rejected.push((source_ordinal, correlation, source, cause)),
                MaterializedRecord::Fatal { .. } => unreachable!("fatal records returned above"),
            }
        }
        if candidates.is_empty() {
            if let Some(parameter) = self.first_cycle.take() {
                return Err(TypeConstraintError::Rejected(
                    TypeConstraintRejection::CyclicInstantiation { parameter },
                )
                .into());
            }
            let Some((source_ordinal, _, source, _)) = rejected
                .iter()
                .min_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.0.cmp(&right.1.0)))
                .map(|entry| (entry.0, &entry.1, entry.2, ()))
            else {
                return Err(
                    TypeConstraintError::Rejected(TypeConstraintRejection::Mismatch).into(),
                );
            };
            let causes = rejected
                .into_iter()
                .filter(|entry| entry.0 == source_ordinal && entry.2 == source)
                .map(|entry| entry.3)
                .collect::<Vec<_>>()
                .into_boxed_slice();
            return Err(TypeConstraintFailure::Rejected(
                TypeConstraintCandidateFailure::Source(
                    SourceError::new(source, SourcePhase::Materialize, causes).into(),
                ),
            ));
        }
        let Some((component, sealed_branch)) = candidates.pop() else {
            return Err(TypeConstraintError::Rejected(TypeConstraintRejection::Mismatch).into());
        };
        let first = SolvedCandidate {
            component,
            sealed_branch,
        };
        let remaining = candidates
            .into_iter()
            .map(|(component, sealed_branch)| SolvedCandidate {
                component,
                sealed_branch,
            })
            .collect();
        context.check_cancelled()?;
        Ok(super::CompletedCandidateAlternatives::new(first, remaining))
    }

    fn complete_component<A>(
        path: &mut ConstraintPath<D>,
        selected: ConstraintApplicationId,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<super::CompletedConstraintComponent<D>, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        context.check_cancelled()?;
        ensure_unique_sources(path)?;
        close_source_rows(path, context)?;
        Self::validate_completed_equations(path, context)?;
        context.validate_type_and_const_completion(path)?;
        let selected_domain = path
            .applications
            .require_application(selected)?
            .application();
        for request in &path.projections {
            path.applications.require_application(request.application)?;
        }
        let ordered = path
            .applications
            .applications()
            .map(|scope| (scope.application(), scope.id()))
            .collect::<BTreeMap<_, _>>();
        let mut completed = BTreeMap::new();
        for (domain, application) in ordered {
            let solution = Arc::new(TypeConstraintSolution::complete_application(
                path,
                application,
                context,
            )?);
            let projections = Self::finish_projections(path, application, &solution, context)?;
            completed.insert(
                domain,
                super::CompletedConstraintApplication::new(solution, projections),
            );
        }
        path.projections.clear();
        let closed_sources = core::mem::take(&mut path.probe_trace)
            .into_iter()
            .map(ConstraintProbe::into_closed)
            .collect::<Result<Box<[_]>, _>>()?;
        Ok(super::CompletedConstraintComponent::new(
            selected_domain,
            completed,
            ClosedConstraintSourceTrace::new(
                selected,
                Arc::clone(&path.applications),
                closed_sources,
            ),
        ))
    }

    fn validate_completed_equations<A: TypeConstraintAccounting>(
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        let effect_substitution = path.effects.substitution(context)?;
        for equation in &path.equations {
            let pattern = equation
                .final_expected
                .as_ref()
                .unwrap_or(&equation.pattern);
            let mut project = |value| {
                seal_type(value, path.projection_view(), &mut BTreeSet::new(), context)?
                    .substitute_effect_rows(&effect_substitution)
                    .map_err(|_error| {
                        super::effect_invariant(
                            super::TypeConstraintEffectInvariantKind::NonCanonicalInherited,
                            None,
                        )
                    })
            };
            let pattern = project(pattern)?;
            let actual = project(&equation.actual)?;
            let (expected, actual) = match equation.direction {
                ConstraintAcceptance::PatternAcceptsActual => (&pattern, &actual),
                ConstraintAcceptance::ActualAcceptsPattern => (&actual, &pattern),
            };
            if !expected
                .accepts_with(
                    actual,
                    super::super::compatibility::TypeCompatibilityPolicy::SelectedCall,
                    context,
                )
                .map_err(super::super::compatibility::binding_plan::map_compatibility_error)?
            {
                return Err(TypeConstraintRejection::Mismatch.into());
            }
        }
        Ok(())
    }

    fn finish_projections<A>(
        path: &ConstraintPath<D>,
        application: ConstraintApplicationId,
        solution: &TypeConstraintSolution,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<
        Box<[super::KeyedConstraintProjection<D::Application, D::Projection>]>,
        TypeConstraintError,
    >
    where
        A: TypeConstraintAccounting,
    {
        let effect_substitution = path.effects.substitution(context)?;
        let mut projections = Vec::new();
        for request in path
            .projections
            .iter()
            .filter(|request| request.application == application)
        {
            let policy = match request.closure {
                TypeConstraintProjectionClosure::Closed => {
                    ConstraintClosurePolicy::ProjectionClosed
                }
                TypeConstraintProjectionClosure::AllowFutureEligible => {
                    ConstraintClosurePolicy::ProjectionFuture
                }
            };
            let opened = context
                .open_template_type(&request.value, path, application)
                .map_err(projection_error)?;
            let value = project_type(&opened, path.projection_view(), policy, context)
                .map_err(projection_error)?
                .value
                .substitute_effect_rows(&effect_substitution)
                .map_err(|_error| {
                    super::effect_invariant(
                        super::TypeConstraintEffectInvariantKind::NonCanonicalInherited,
                        None,
                    )
                })?;
            if policy != ConstraintClosurePolicy::ProjectionFuture {
                validate_selected_call_self(&value, context).map_err(projection_error)?;
            }
            let input = request
                .input
                .as_ref()
                .map(|input| {
                    project_type(
                        input,
                        path.projection_view(),
                        ConstraintClosurePolicy::ProjectionClosed,
                        context,
                    )
                    .map(|projected| projected.value)
                    .map_err(projection_error)
                })
                .transpose()?;
            projections.push(
                solution
                    .reify_projection(
                        Arc::clone(&request.key),
                        &value,
                        path,
                        application,
                        policy,
                        context,
                    )
                    .map_err(projection_error)?
                    .with_source(
                        request.source.as_ref().map(|source| {
                            super::completion::CompletedProjectionAddress {
                                application: path
                                    .applications
                                    .require_application(source.application)
                                    .expect("projection source was admitted on this path")
                                    .application(),
                                key: Arc::clone(&source.key),
                            }
                        }),
                        input,
                    ),
            );
        }
        projections.sort_by(|left, right| left.key().cmp(right.key()));
        if projections
            .windows(2)
            .any(|pair| pair[0].key() == pair[1].key())
        {
            return Err(TypeConstraintError::Invariant(
                TypeConstraintInvariant::Projection(
                    TypeConstraintProjectionInvariant::DuplicateKey,
                ),
            ));
        }
        Ok(projections.into_boxed_slice())
    }

    fn prepare_application<A>(
        context: &mut TypeConstraintContext<'_, A, D>,
        application: ConstraintApplicationId,
        path: ConstraintPath<D>,
        inherited: Option<&TypeConstraintSolution>,
    ) -> Result<ConstraintPath<D>, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        context.check_cancelled()?;
        let applications = Arc::clone(&path.applications);
        let scope = applications.require_application(application)?;
        let Some(inherited) = inherited else {
            if let Some(parameter) = scope.parameters().required_inherited_keys().first() {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                        kind: InheritedSolutionInvariantKind::Unclosed,
                        parameter: Some(parameter.clone().into()),
                    }),
                ));
            }
            if let Some(parameter) = scope.parameters().required_inherited_const_keys().first() {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                        kind: InheritedSolutionInvariantKind::Unclosed,
                        parameter: Some(parameter.clone().into()),
                    }),
                ));
            }
            if let Some(variable) = scope.effects().required_inherited().first() {
                return Err(super::effect_invariant(
                    super::TypeConstraintEffectInvariantKind::MissingInherited,
                    Some(variable.clone()),
                ));
            }
            return Ok(path);
        };
        inherited.restore_inherited_path(application, path, context)
    }
}

fn materialization_immediate<D: ConstraintDomain>(
    error: TypeConstraintError,
) -> MaterializationImmediateFailure<D> {
    match error {
        TypeConstraintError::Abort(error) => MaterializationImmediateFailure::Abort(error),
        TypeConstraintError::Invariant(error) => MaterializationImmediateFailure::Invariant(
            super::TypeConstraintFailureInvariant::Constraint(error),
        ),
        TypeConstraintError::Rejected(_) => MaterializationImmediateFailure::Invariant(
            super::TypeConstraintFailureInvariant::Constraint(
                TypeConstraintInvariant::SourceProtocol(
                    TypeConstraintSourceProtocolInvariant::Outcome,
                ),
            ),
        ),
    }
}

fn projection_error(error: TypeConstraintError) -> TypeConstraintError {
    match error {
        TypeConstraintError::Rejected(rejection) => {
            TypeConstraintError::Invariant(TypeConstraintInvariant::Projection(
                TypeConstraintProjectionInvariant::Mismatch(rejection),
            ))
        }
        TypeConstraintError::Abort(_) | TypeConstraintError::Invariant(_) => error,
    }
}

fn resolve_probe_term<A: TypeConstraintAccounting, D: ConstraintDomain>(
    term: SourceProbeTerm<D>,
    path: &ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<(TypeKind, Option<ConstraintResultProjection<D>>), TypeConstraintError> {
    match term {
        SourceProbeTerm::Type(value) => Ok((value, None)),
        SourceProbeTerm::Result(result) => {
            path.applications.require_application(result.application)?;
            let mut requests = path.projections.iter().filter(|request| {
                request.application == result.application && request.key == result.key
            });
            let Some(request) = requests.next() else {
                return Err(protocol_error(
                    TypeConstraintSourceProtocolInvariant::WrongSource,
                ));
            };
            if requests.next().is_some() {
                return Err(protocol_error(
                    TypeConstraintSourceProtocolInvariant::Outcome,
                ));
            }
            let actual = context.open_template_type(&request.value, path, result.application)?;
            Ok((actual, Some(result)))
        }
    }
}

mod result_port;

fn validate_source_selection<D: ConstraintDomain>(
    prepared: &PreparedSourceConstraint<D>,
    selected: D::AlternativeIndex,
    evidence: Arc<D::ObservedEvidence>,
    actual: &TypeKind,
) -> Result<
    Option<(
        D::AlternativeIndex,
        Arc<D::ObservedEvidence>,
        TypeKind,
        CheckedConstraintSourceProjection,
    )>,
    TypeConstraintError,
> {
    let selected_row = prepared.alternative(selected).ok_or(protocol_error(
        TypeConstraintSourceProtocolInvariant::UnknownAlternative,
    ))?;
    if !D::evidence_accepts(selected_row.evidence(), &evidence) {
        return Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::InvalidEvidence,
        ));
    }
    let otherwise = prepared
        .otherwise()
        .expect("checked source retains mandatory otherwise");
    let mut first_guarded = None;
    let mut guarded_matches = 0_u32;
    for row in prepared.alternatives() {
        if !D::evidence_accepts(row.evidence(), &evidence) {
            continue;
        }
        if row.alternative() == otherwise.alternative() {
            continue;
        }
        guarded_matches = guarded_matches.saturating_add(1);
        if first_guarded.is_none() {
            first_guarded = Some(row.alternative());
        }
    }
    let selected_is_otherwise = selected == otherwise.alternative();
    if guarded_matches > 1 {
        return Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::InvalidEvidence,
        ));
    }
    if let Some(first) = first_guarded {
        if selected_is_otherwise || first != selected {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::InvalidEvidence,
            ));
        }
    } else if !selected_is_otherwise {
        return Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::InvalidEvidence,
        ));
    }
    let Some(source_projection) =
        CheckedConstraintSourceProjection::derive(prepared.source_projection(), actual)
    else {
        return Ok(None);
    };
    Ok(Some((
        selected,
        evidence,
        selected_row.value_expected().clone(),
        source_projection,
    )))
}

fn ensure_unique_sources<D: ConstraintDomain>(
    path: &ConstraintPath<D>,
) -> Result<(), TypeConstraintError> {
    let mut sources = BTreeSet::new();
    if path
        .probe_trace
        .iter()
        .any(|probe| !sources.insert(probe.source()))
    {
        Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::Ticket,
        ))
    } else {
        Ok(())
    }
}

fn unique_request_ordinal<D: ConstraintDomain>(
    requests: &[ClosedConstraintProbe<D>],
    source: ConstraintSourceId<D::Source>,
) -> Result<MaterializationSourceOrdinal, TypeConstraintError> {
    let mut matches = requests
        .iter()
        .enumerate()
        .filter(|(_, request)| request.source() == source)
        .map(|(ordinal, _)| ordinal);
    let Some(ordinal) = matches.next() else {
        return Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::Ticket,
        ));
    };
    if matches.next().is_some() {
        return Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::Ticket,
        ));
    }
    Ok(MaterializationSourceOrdinal(
        u32::try_from(ordinal)
            .map_err(|_| TypeConstraintError::Abort(TypeConstraintAbort::ArithmeticOverflow))?,
    ))
}
fn close_source_rows<A, D>(
    path: &mut ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<(), TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let effect_substitution = path.effects.substitution(context)?;
    let probes = core::mem::take(&mut path.probe_trace);
    let mut closed = Vec::with_capacity(probes.len());
    for probe in probes {
        let probe = probe.close(path.projection_view(), &effect_substitution, context)?;
        for equation in &mut path.equations {
            if equation.source_ordinal == Some(probe.ordinal()) {
                equation.actual = probe.actual().clone();
                if let Some(expected) = probe.final_expected() {
                    equation.final_expected = Some(expected.clone());
                    equation.pattern = expected.clone();
                }
            }
        }
        closed.push(ConstraintProbe::Closed(probe));
    }
    path.probe_trace = closed;
    Ok(())
}

fn path_correlation_cmp<D: ConstraintDomain>(
    left: &ConstraintPath<D>,
    right: &ConstraintPath<D>,
) -> std::cmp::Ordering {
    left.choice_key
        .cmp(&right.choice_key)
        .then_with(|| {
            left.probe_trace
                .iter()
                .map(|probe| (probe.ordinal(), probe.source()))
                .cmp(
                    right
                        .probe_trace
                        .iter()
                        .map(|probe| (probe.ordinal(), probe.source())),
                )
        })
        .then_with(|| left.probe_trace.len().cmp(&right.probe_trace.len()))
}

fn protocol_error(kind: TypeConstraintSourceProtocolInvariant) -> TypeConstraintError {
    TypeConstraintError::Invariant(TypeConstraintInvariant::SourceProtocol(kind))
}
