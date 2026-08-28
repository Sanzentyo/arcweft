//! Candidate-wide equations, prepared source traces, and finalization.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

use super::super::{ArrayLength, GenericConstParameterId, GenericTypeParameterId, TypeKind};
use super::RejectedConstraintSourceProjection;
use super::context::{TypeConstraintAccounting, TypeConstraintContext};
use super::normalization::{project_type, validate_selected_call_self};
use super::{
    CheckedConstraintSourceProjection, ClosedMaterializationSubmission, ConstraintAcceptance,
    ConstraintClosurePolicy, ConstraintDomain, ExpectedHint, InheritedSolutionInvariant,
    InheritedSolutionInvariantKind, KeyedConstraintProjection, MaterializationImmediateFailure,
    MaterializedSourceRequest, PreparedConstraintSourceProjection, PreparedSourceConstraint,
    ProjectedExpectedHint, SolvedCandidate, SourceAlternativeHint, SourceError, SourcePhase,
    SourceProbeResult, SourceProbeSelection, TypeConstraintAbort, TypeConstraintCandidateFailure,
    TypeConstraintError, TypeConstraintFailure, TypeConstraintInvariant,
    TypeConstraintProjectionClosure, TypeConstraintProjectionInvariant, TypeConstraintRejection,
    TypeConstraintSolution, TypeConstraintSourceProtocolInvariant, bindings_equal,
    relate_selected_call, seal_path, seal_type, validate_type,
};

/// One complete equation retained until candidate closure.  Source equations
/// retain their selected schema row and lower-derived projection; ordinary
/// equations leave those fields empty.
pub(crate) struct PendingEquation<D: ConstraintDomain> {
    pub(crate) ordinal: u32,
    pub(crate) direction: ConstraintAcceptance,
    pub(crate) pattern: TypeKind,
    pub(crate) actual: TypeKind,
    pub(crate) source_ordinal: Option<u32>,
    pub(crate) alternative: Option<D::AlternativeIndex>,
    pub(crate) evidence: Option<Arc<D::CheckedEvidence>>,
    pub(crate) source_projection: Option<CheckedConstraintSourceProjection>,
    pub(crate) final_expected: Option<TypeKind>,
}

impl<D: ConstraintDomain> Clone for PendingEquation<D> {
    fn clone(&self) -> Self {
        Self {
            ordinal: self.ordinal,
            direction: self.direction,
            pattern: self.pattern.clone(),
            actual: self.actual.clone(),
            source_ordinal: self.source_ordinal,
            alternative: self.alternative,
            evidence: self.evidence.as_ref().map(Arc::clone),
            source_projection: self.source_projection.clone(),
            final_expected: self.final_expected.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ChoiceForkRole {
    ExpectedAlternative,
    ActualAlternative,
    ExpectedActualPair,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ChoiceDerivationStep {
    pub(crate) equation: u32,
    pub(crate) direction: ConstraintAcceptance,
    pub(crate) role: ChoiceForkRole,
    pub(crate) expected: Option<u32>,
    pub(crate) actual: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DeferredCycleWitness {
    pub(crate) parameters: BTreeSet<super::ConstraintGenericParameterId>,
}

/// Semantic evidence retained by one closed source row.  The value/evidence
/// cells are private `Arc`s: path forks share them without adding `Clone`,
/// `Copy`, or `Ord` requirements to the domain values.
pub(crate) struct ConstraintProbe<D: ConstraintDomain> {
    pub(crate) source: D::Source,
    pub(crate) source_ordinal: u32,
    pub(crate) branch: Arc<D::ProbeSemanticBranch>,
    pub(crate) selection: StoredSourceSelection<D>,
    pub(crate) prepared_source_projection: PreparedConstraintSourceProjection,
    pub(crate) value_expected: Option<TypeKind>,
    pub(crate) actual: TypeKind,
    pub(crate) source_projection: CheckedConstraintSourceProjection,
    pub(crate) final_expected: Option<TypeKind>,
}

/// Name the closed row explicitly for higher sealing without introducing a
/// second representation.
pub(crate) type ClosedConstraintProbe<D> = ConstraintProbe<D>;

pub(crate) enum StoredSourceSelection<D: ConstraintDomain> {
    Unchecked,
    Checked {
        alternative: D::AlternativeIndex,
        evidence: Arc<D::CheckedEvidence>,
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

impl<D: ConstraintDomain> StoredSourceSelection<D> {
    pub(crate) const fn is_unchecked(&self) -> bool {
        matches!(self, Self::Unchecked)
    }

    pub(crate) const fn alternative(&self) -> Option<D::AlternativeIndex> {
        match self {
            Self::Unchecked => None,
            Self::Checked { alternative, .. } => Some(*alternative),
        }
    }

    pub(crate) fn evidence(&self) -> Option<&D::CheckedEvidence> {
        match self {
            Self::Unchecked => None,
            Self::Checked { evidence, .. } => Some(evidence.as_ref()),
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

impl<D: ConstraintDomain> Clone for ConstraintProbe<D> {
    fn clone(&self) -> Self {
        Self {
            source: self.source,
            source_ordinal: self.source_ordinal,
            branch: Arc::clone(&self.branch),
            selection: self.selection.clone(),
            prepared_source_projection: self.prepared_source_projection,
            value_expected: self.value_expected.clone(),
            actual: self.actual.clone(),
            source_projection: self.source_projection.clone(),
            final_expected: self.final_expected.clone(),
        }
    }
}

impl<D: ConstraintDomain> PartialEq for ConstraintProbe<D> {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.source_ordinal == other.source_ordinal
            && self.branch == other.branch
            && self.selection == other.selection
            && self.prepared_source_projection == other.prepared_source_projection
            && self.value_expected == other.value_expected
            && self.actual == other.actual
            && self.source_projection == other.source_projection
            && self.final_expected == other.final_expected
    }
}

impl<D: ConstraintDomain> Eq for ConstraintProbe<D> {}

impl<D: ConstraintDomain> ConstraintProbe<D> {
    pub(crate) const fn source(&self) -> D::Source {
        self.source
    }

    pub(crate) const fn actual(&self) -> &TypeKind {
        &self.actual
    }

    pub(crate) const fn final_expected(&self) -> Option<&TypeKind> {
        self.final_expected.as_ref()
    }

    pub(crate) const fn selection(&self) -> &StoredSourceSelection<D> {
        &self.selection
    }

    pub(crate) const fn prepared_source_projection(&self) -> PreparedConstraintSourceProjection {
        self.prepared_source_projection
    }

    pub(crate) const fn source_projection(&self) -> &CheckedConstraintSourceProjection {
        &self.source_projection
    }
}

pub(crate) struct ConstraintPath<D: ConstraintDomain> {
    pub(crate) bindings: BTreeMap<GenericTypeParameterId, TypeKind>,
    pub(crate) const_bindings: BTreeMap<GenericConstParameterId, ArrayLength>,
    pub(crate) effects: crate::effect_row::EffectConstraintEnvironment,
    pub(crate) equations: Vec<PendingEquation<D>>,
    pub(crate) choice_key: Vec<ChoiceDerivationStep>,
    pub(crate) deferred_cycles: DeferredCycleWitness,
    pub(crate) probe_trace: Vec<ConstraintProbe<D>>,
}

impl<D: ConstraintDomain> ConstraintPath<D> {
    pub(crate) fn empty(effects: crate::effect_row::EffectConstraintEnvironment) -> Self {
        Self {
            bindings: BTreeMap::new(),
            const_bindings: BTreeMap::new(),
            effects,
            equations: Vec::new(),
            choice_key: Vec::new(),
            deferred_cycles: DeferredCycleWitness::default(),
            probe_trace: Vec::new(),
        }
    }
}

impl<D: ConstraintDomain> Clone for ConstraintPath<D> {
    fn clone(&self) -> Self {
        Self {
            bindings: self.bindings.clone(),
            const_bindings: self.const_bindings.clone(),
            effects: self.effects.clone(),
            equations: self.equations.clone(),
            choice_key: self.choice_key.clone(),
            deferred_cycles: self.deferred_cycles.clone(),
            probe_trace: self.probe_trace.clone(),
        }
    }
}

/// An affine lower ticket.  It can be submitted exactly once by the driver.
pub(crate) struct ProbeTicket<D: ConstraintDomain> {
    source: D::Source,
    path: ConstraintPath<D>,
    prepared: Arc<PreparedSourceConstraint<D>>,
    hints: Vec<OwnedAlternativeHint<D>>,
    acceptance: ConstraintAcceptance,
    equation_ordinal: Option<u32>,
}

struct OwnedAlternativeHint<D: ConstraintDomain> {
    alternative: D::AlternativeIndex,
    expected: TypeKind,
    unbound: Box<[super::ConstraintGenericParameterId]>,
}

impl<D: ConstraintDomain> ProbeTicket<D> {
    pub(crate) const fn source(&self) -> D::Source {
        self.source
    }

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
        let hints = self
            .hints
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
    Accepted(SourceProbeResult<D>),
    Rejected(D::SourceErrorCause),
}

pub(crate) struct MaterializationTicket<D: ConstraintDomain> {
    identity: MaterializationTicketIdentity,
    correlation: MaterializationCorrelationOrdinal,
    path: ConstraintPath<D>,
    requests: Box<[ClosedMaterializationRequest<D>]>,
    phase: MaterializationTicketPhase,
}

enum MaterializationTicketPhase {
    Ready,
    CallbackBound,
    Closed,
}

pub(crate) struct MaterializationCallbackBinding<D: ConstraintDomain> {
    identity: MaterializationTicketIdentity,
    sources: Box<[D::Source]>,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct PreparedSourceOrdinal(u32);

struct ClosedMaterializationRequest<D: ConstraintDomain> {
    source: D::Source,
    source_ordinal: PreparedSourceOrdinal,
    row: ClosedMaterializationRequestRow<D>,
}

enum ClosedMaterializationRequestRow<D: ConstraintDomain> {
    Unchecked {
        source_projection: CheckedConstraintSourceProjection,
        actual: TypeKind,
        canonical_branch: Arc<D::ProbeSemanticBranch>,
    },
    Checked {
        alternative: D::AlternativeIndex,
        evidence: Arc<D::CheckedEvidence>,
        source_projection: CheckedConstraintSourceProjection,
        actual: TypeKind,
        expected: TypeKind,
        canonical_branch: Arc<D::ProbeSemanticBranch>,
    },
}

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
        self.requests.iter().map(|request| match &request.row {
            ClosedMaterializationRequestRow::Unchecked {
                source_projection,
                actual,
                canonical_branch,
            } => MaterializedSourceRequest::Unchecked {
                source: request.source,
                source_projection,
                actual,
                canonical_branch: canonical_branch.as_ref(),
            },
            ClosedMaterializationRequestRow::Checked {
                alternative,
                evidence,
                source_projection,
                actual,
                expected,
                canonical_branch,
            } => MaterializedSourceRequest::Checked {
                source: request.source,
                alternative: *alternative,
                evidence: evidence.as_ref(),
                source_projection,
                actual,
                expected,
                canonical_branch: canonical_branch.as_ref(),
            },
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
            sources: self.requests.iter().map(|request| request.source).collect(),
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
        let expected = self.requests.iter().map(|request| request.source);
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
    pub(crate) fn sources(&self) -> &[D::Source] {
        &self.sources
    }

    pub(crate) fn authorizes(&self, source: &D::Source) -> bool {
        self.sources.iter().any(|candidate| candidate == source)
    }
}

fn materialization_identity_matches(
    left: &MaterializationTicketIdentity,
    right: &MaterializationTicketIdentity,
) -> bool {
    Arc::ptr_eq(&left.issuer, &right.issuer) && left.ordinal == right.ordinal
}

struct ProjectionRequest<D: ConstraintDomain> {
    key: D::Projection,
    value: TypeKind,
    closure: TypeConstraintProjectionClosure,
}

/// A completed run owns the accounting reservation until it is consumed (or
/// dropped).  There is no caller-side report merge path.
pub(crate) struct TypeConstraintRun<'c, A: TypeConstraintAccounting, D: ConstraintDomain> {
    outcome: Option<Result<SolvedCandidate<D>, TypeConstraintFailure<D>>>,
    context: TypeConstraintContext<'c, A, D>,
}

impl<'c, A, D> TypeConstraintRun<'c, A, D>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    pub(crate) fn complete(mut self) -> Result<SolvedCandidate<D>, TypeConstraintFailure<D>> {
        self.context.commit_accounting();
        self.outcome
            .take()
            .expect("type constraint outcome is completed exactly once")
    }
}

impl<A, D> Drop for TypeConstraintRun<'_, A, D>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    fn drop(&mut self) {
        self.context.commit_accounting();
    }
}

struct ProbeOperation<D: ConstraintDomain> {
    source: D::Source,
    source_ordinal: u32,
    prepared: Arc<PreparedSourceConstraint<D>>,
    acceptance: ConstraintAcceptance,
    equation_ordinal: Option<u32>,
    rows: VecDeque<ConstraintPath<D>>,
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
        path: ConstraintPath<D>,
        value: D::SealedBranchValue,
    },
    Rejected {
        source_ordinal: u32,
        correlation: MaterializationCorrelationOrdinal,
        source: D::Source,
        cause: D::SourceErrorCause,
    },
    Fatal {
        source_ordinal: u32,
        correlation: MaterializationCorrelationOrdinal,
        error: SourceError<D::Source, D::SourceErrorCause>,
    },
}

enum NormalizedPath<D: ConstraintDomain> {
    Acyclic(ConstraintPath<D>),
    Cyclic(super::ConstraintGenericParameterId),
}

pub(crate) struct TypeConstraintTransaction<D: ConstraintDomain> {
    frontier: Vec<ConstraintPath<D>>,
    first_failure: Option<TypeConstraintFailure<D>>,
    projections: Vec<ProjectionRequest<D>>,
    next_equation: u32,
    next_source_ordinal: u32,
    first_cycle: Option<super::ConstraintGenericParameterId>,
    next_correlation_ordinal: u32,
    next_materialization_ticket_ordinal: u64,
    materialization_issuer: Arc<MaterializationTicketIssuer>,
    active_materialization: Option<MaterializationTicketIdentity>,
    probe: Option<ProbeOperation<D>>,
    probe_group: Option<ProbeGroup<D>>,
    prepared_sources: BTreeSet<D::Source>,
    materialization: VecDeque<MaterializationTicket<D>>,
    materialized: Vec<MaterializedRecord<D>>,
    closed: bool,
}

impl<D: ConstraintDomain> TypeConstraintTransaction<D> {
    pub(crate) fn new() -> Self {
        Self {
            frontier: Vec::new(),
            first_failure: None,
            projections: Vec::new(),
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
        }
    }

    pub(crate) fn initialize<A>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        inherited: Option<Arc<TypeConstraintSolution>>,
    ) -> Result<(), super::TypeConstraintInitializationFailure>
    where
        A: TypeConstraintAccounting,
    {
        match Self::seed(context, inherited.as_deref()) {
            Ok(path) => {
                self.frontier.push(path);
                Ok(())
            }
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
            path.equations.push(PendingEquation {
                ordinal,
                direction: acceptance,
                pattern: pattern.clone(),
                actual: actual.clone(),
                source_ordinal: None,
                alternative: None,
                evidence: None,
                source_projection: None,
                final_expected: None,
            });
            match relate_selected_call(pattern, actual, path, context, acceptance) {
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

    pub(crate) fn request_projection(
        &mut self,
        key: D::Projection,
        value: &TypeKind,
        closure: TypeConstraintProjectionClosure,
    ) {
        if self.first_failure.is_none() && !self.closed {
            self.projections.push(ProjectionRequest {
                key,
                value: value.clone(),
                closure,
            });
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
        let source = prepared.source();
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
            Some(ordinal)
        } else {
            None
        };
        let source_ordinal = self.next_source_ordinal;
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
            let projected = project_type(
                alternative.value_expected(),
                &path.bindings,
                &path.const_bindings,
                ConstraintClosurePolicy::Hint,
                context,
            )?;
            hints.push(OwnedAlternativeHint {
                alternative: alternative.alternative(),
                expected: projected.value,
                unbound: projected
                    .remaining
                    .iter()
                    .map(|parameter| parameter.parameter().clone())
                    .collect(),
            });
        }
        Ok(Some(ProbeTicket {
            source: operation.source,
            path,
            prepared: Arc::clone(&operation.prepared),
            hints,
            acceptance: operation.acceptance,
            equation_ordinal: operation.equation_ordinal,
        }))
    }

    pub(crate) fn submit_probe<A>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        mut ticket: ProbeTicket<D>,
        submission: ProbeSubmission<D>,
    ) -> Result<(), TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        let operation = self.probe.as_mut().expect("probe ticket without operation");
        match submission {
            ProbeSubmission::Rejected(cause) => operation.rejections.push(cause),
            ProbeSubmission::Accepted(result) => {
                let (actual, branch, callback_selection) = result.into_parts();
                if ticket
                    .path
                    .probe_trace
                    .iter()
                    .any(|probe| probe.source == ticket.source)
                {
                    return Err(protocol_error(
                        TypeConstraintSourceProtocolInvariant::Outcome,
                    ));
                }

                let selected = if ticket.prepared.is_unchecked() {
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
                    } = callback_selection
                    else {
                        return Err(protocol_error(
                            TypeConstraintSourceProtocolInvariant::UnknownAlternative,
                        ));
                    };
                    match validate_checked_selection(
                        &ticket.prepared,
                        alternative,
                        evidence,
                        &actual,
                    )? {
                        Some(selected) => Some(selected),
                        None => return Ok(()),
                    }
                };

                let (pattern, stored_selection, source_projection, value_expected) = match selected
                {
                    None => {
                        let Some(source_projection) = CheckedConstraintSourceProjection::derive(
                            ticket.prepared.source_projection(),
                            &actual,
                        ) else {
                            return Ok(());
                        };
                        (
                            None,
                            StoredSourceSelection::Unchecked,
                            source_projection,
                            None,
                        )
                    }
                    Some((alternative, evidence, value_expected, source_projection)) => {
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
                    ticket.path.equations.push(PendingEquation {
                        ordinal: ticket.equation_ordinal.unwrap_or(self.next_equation),
                        direction: ticket.acceptance,
                        pattern: expected.clone(),
                        actual: actual.clone(),
                        source_ordinal: Some(operation.source_ordinal),
                        alternative: match &stored_selection {
                            StoredSourceSelection::Checked { alternative, .. } => {
                                Some(*alternative)
                            }
                            StoredSourceSelection::Unchecked => None,
                        },
                        evidence: match &stored_selection {
                            StoredSourceSelection::Checked { evidence, .. } => {
                                Some(Arc::clone(evidence))
                            }
                            StoredSourceSelection::Unchecked => None,
                        },
                        source_projection: Some(source_projection.clone()),
                        final_expected: None,
                    });
                }
                let rejected_alternative = match &stored_selection {
                    StoredSourceSelection::Checked { alternative, .. } => Some(*alternative),
                    StoredSourceSelection::Unchecked => None,
                };
                let relation_rejection = pattern.as_ref().map(|expected| {
                    RejectedConstraintSourceProjection::new(
                        ticket.source,
                        rejected_alternative,
                        source_projection.clone(),
                        ticket.acceptance,
                        expected.clone(),
                        actual.clone(),
                    )
                });
                let probe = ConstraintProbe {
                    source: ticket.source,
                    source_ordinal: operation.source_ordinal,
                    branch: Arc::new(branch),
                    selection: stored_selection,
                    prepared_source_projection: ticket.prepared.source_projection(),
                    value_expected,
                    actual: actual.clone(),
                    source_projection,
                    final_expected: None,
                };
                ticket.path.probe_trace.push(probe);

                let related = if let Some(expected) = pattern.as_ref() {
                    relate_selected_call(expected, &actual, ticket.path, context, ticket.acceptance)
                } else {
                    validate_type(&actual, context).map(|()| vec![ticket.path])
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
            self.close(context).map_err(materialization_immediate)?;
        }
        let Some(ticket) = self.materialization.pop_front() else {
            return Ok(None);
        };
        if self.active_materialization.is_some() {
            return Err(materialization_immediate(TypeConstraintError::Invariant(
                TypeConstraintInvariant::SourceProtocol(
                    TypeConstraintSourceProtocolInvariant::Ticket,
                ),
            )));
        }
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
            path,
            requests,
            ..
        } = ticket;
        match closed.submission {
            ClosedMaterializationSubmission::Sealed(value) => {
                ensure_unique_request_sources(&requests).map_err(materialization_immediate)?;
                self.materialized
                    .push(MaterializedRecord::Sealed { path, value });
            }
            ClosedMaterializationSubmission::Rejected { source, cause } => {
                ensure_unique_request_sources(&requests).map_err(materialization_immediate)?;
                let source_ordinal = unique_request_ordinal(&requests, source)
                    .map_err(materialization_immediate)?
                    .0;
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
                ensure_unique_request_sources(&requests).map_err(materialization_immediate)?;
                let source_ordinal = unique_request_ordinal(&requests, *error.source())
                    .map_err(materialization_immediate)?
                    .0;
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
        mut self,
        mut context: TypeConstraintContext<'_, A, D>,
    ) -> TypeConstraintRun<'_, A, D>
    where
        A: TypeConstraintAccounting,
    {
        let outcome = if self.materialization.is_empty() && !self.closed {
            match self.close(&mut context) {
                Ok(()) => self.finish_candidate(&mut context),
                Err(error) => Err(error.into()),
            }
        } else if !self.materialization.is_empty() && self.first_failure.is_none() {
            Err(TypeConstraintError::Rejected(TypeConstraintRejection::UnresolvedType).into())
        } else {
            self.finish_candidate(&mut context)
        };
        TypeConstraintRun {
            outcome: Some(outcome),
            context,
        }
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
        let frontier = core::mem::take(&mut self.frontier);
        let mut acyclic = Vec::new();
        for path in frontier {
            match self.normalize_path(path, context)? {
                NormalizedPath::Acyclic(path) => acyclic.push(path),
                NormalizedPath::Cyclic(parameter) => {
                    if self.first_cycle.is_none() {
                        self.first_cycle = Some(parameter);
                    }
                }
            }
        }
        if acyclic.is_empty() {
            return Ok(());
        }
        self.first_cycle = None;

        let mut finalized = Vec::new();
        for mut path in acyclic {
            close_source_rows(&mut path, context)?;
            let effect_substitution = path
                .effects
                .substitution()
                .map_err(super::map_effect_environment_error)?;
            let mut valid = true;
            for equation in &path.equations {
                let pattern = equation
                    .final_expected
                    .as_ref()
                    .unwrap_or(&equation.pattern);
                let pattern = seal_type(
                    pattern,
                    &path.bindings,
                    &path.const_bindings,
                    &mut BTreeSet::new(),
                    context,
                )?
                .substitute_effect_rows(&effect_substitution)
                .map_err(|_| {
                    super::effect_invariant(
                        super::TypeConstraintEffectInvariantKind::NonCanonicalInherited,
                        None,
                    )
                })?;
                let actual = seal_type(
                    &equation.actual,
                    &path.bindings,
                    &path.const_bindings,
                    &mut BTreeSet::new(),
                    context,
                )?
                .substitute_effect_rows(&effect_substitution)
                .map_err(|_| {
                    super::effect_invariant(
                        super::TypeConstraintEffectInvariantKind::NonCanonicalInherited,
                        None,
                    )
                })?;
                let accepted = match equation.direction {
                    ConstraintAcceptance::PatternAcceptsActual => pattern
                        .accepts_with(
                            &actual,
                            super::super::compatibility::TypeCompatibilityPolicy::SelectedCall,
                            context,
                        )
                        .map_err(
                            super::super::compatibility::binding_plan::map_compatibility_error,
                        )?,
                    ConstraintAcceptance::ActualAcceptsPattern => actual
                        .accepts_with(
                            &pattern,
                            super::super::compatibility::TypeCompatibilityPolicy::SelectedCall,
                            context,
                        )
                        .map_err(
                            super::super::compatibility::binding_plan::map_compatibility_error,
                        )?,
                };
                if !accepted {
                    valid = false;
                    break;
                }
            }
            if valid {
                finalized.push(path);
            }
        }

        let mut groups: Vec<Vec<ConstraintPath<D>>> = Vec::new();
        for path in finalized {
            let group_index = groups.iter().position(|group| {
                group
                    .first()
                    .map(|first| bindings_equal(first, &path, context).unwrap_or(false))
                    .unwrap_or(false)
            });
            if let Some(index) = group_index {
                groups[index].push(path);
            } else {
                groups.push(vec![path]);
            }
        }

        for mut group in groups {
            group.sort_by(path_correlation_cmp::<D>);
            for path in group {
                if path.probe_trace.is_empty() {
                    self.materialized.push(MaterializedRecord::Sealed {
                        path,
                        value: D::empty_sealed_branch(),
                    });
                    continue;
                }
                ensure_unique_sources(&path)?;
                let requests = closed_materialization_requests(&path)?;
                let correlation = MaterializationCorrelationOrdinal(self.next_correlation_ordinal);
                self.next_correlation_ordinal =
                    self.next_correlation_ordinal.checked_add(1).ok_or(
                        TypeConstraintError::Abort(TypeConstraintAbort::ArithmeticOverflow),
                    )?;
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
                    path,
                    requests,
                    phase: MaterializationTicketPhase::Ready,
                });
            }
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

    fn finish_candidate<A>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<SolvedCandidate<D>, TypeConstraintFailure<D>>
    where
        A: TypeConstraintAccounting,
    {
        if let Some(error) = self.first_failure.take() {
            return Err(error);
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
        let mut candidates: Vec<(ConstraintPath<D>, D::SealedBranchValue)> = Vec::new();
        let mut rejected = Vec::new();
        for record in core::mem::take(&mut self.materialized) {
            match record {
                MaterializedRecord::Sealed { path, value } => {
                    let mut duplicate = false;
                    for (existing, existing_value) in &candidates {
                        if bindings_equal(existing, &path, context)? && existing_value == &value {
                            duplicate = true;
                            break;
                        }
                    }
                    if !duplicate {
                        candidates.push((path, value));
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
        if candidates.len() != 1 {
            return Err(TypeConstraintError::Rejected(
                TypeConstraintRejection::AmbiguousSolution {
                    actual: candidates.len(),
                },
            )
            .into());
        }
        let (path, sealed_branch) = candidates.pop().expect("candidate length checked");
        context.check_cancelled()?;
        let projections = self.finish_projections(&path, context)?;
        let effect_bindings = path
            .effects
            .bindings()
            .map_err(super::map_effect_environment_error)?
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let solution = Arc::new(TypeConstraintSolution::complete_path(
            path.bindings,
            path.const_bindings,
            effect_bindings,
            context,
        )?);
        Ok(SolvedCandidate {
            solution,
            sealed_branch,
            projections,
            closed_sources: path.probe_trace.into_boxed_slice(),
        })
    }

    fn finish_projections<A>(
        &mut self,
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Box<[KeyedConstraintProjection<D::Projection>]>, TypeConstraintFailure<D>>
    where
        A: TypeConstraintAccounting,
    {
        let mut requests = core::mem::take(&mut self.projections);
        requests.extend(path.probe_trace.iter().filter_map(|probe| {
            D::projection_for_source(&probe.source).map(|key| ProjectionRequest {
                key,
                value: probe.actual.clone(),
                closure: TypeConstraintProjectionClosure::Closed,
            })
        }));
        let effect_substitution = path
            .effects
            .substitution()
            .map_err(super::map_effect_environment_error)?;
        let mut projections = Vec::with_capacity(requests.len());
        for request in requests {
            let policy = match request.closure {
                TypeConstraintProjectionClosure::Closed => {
                    ConstraintClosurePolicy::ProjectionClosed
                }
                TypeConstraintProjectionClosure::AllowFutureEligible => {
                    ConstraintClosurePolicy::ProjectionFuture
                }
            };
            let value = project_type(
                &request.value,
                &path.bindings,
                &path.const_bindings,
                policy,
                context,
            )
            .map_err(projection_error::<D>)?
            .value
            .substitute_effect_rows(&effect_substitution)
            .map_err(|_| {
                super::effect_invariant(
                    super::TypeConstraintEffectInvariantKind::NonCanonicalInherited,
                    None,
                )
            })?;
            validate_selected_call_self(&value, context).map_err(projection_error::<D>)?;
            projections.push(KeyedConstraintProjection::new(request.key, value));
        }
        projections.sort_by(|left, right| left.key().cmp(right.key()));
        if projections
            .windows(2)
            .any(|pair| pair[0].key() == pair[1].key())
        {
            return Err(
                TypeConstraintError::Invariant(TypeConstraintInvariant::Projection(
                    TypeConstraintProjectionInvariant::DuplicateKey,
                ))
                .into(),
            );
        }
        Ok(projections.into_boxed_slice())
    }

    fn seed<A>(
        context: &mut TypeConstraintContext<'_, A, D>,
        inherited: Option<&TypeConstraintSolution>,
    ) -> Result<ConstraintPath<D>, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        context.check_cancelled()?;
        let Some(inherited) = inherited else {
            if let Some(parameter) = context.required_inherited_keys().first() {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                        kind: InheritedSolutionInvariantKind::Unclosed,
                        parameter: Some(parameter.clone().into()),
                    }),
                ));
            }
            if let Some(parameter) = context.required_inherited_const_keys().first() {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                        kind: InheritedSolutionInvariantKind::Unclosed,
                        parameter: Some(parameter.clone().into()),
                    }),
                ));
            }
            if let Some(variable) = context.required_inherited_effects().first() {
                return Err(super::effect_invariant(
                    super::TypeConstraintEffectInvariantKind::MissingInherited,
                    Some(*variable),
                ));
            }
            return context.start_path();
        };
        inherited.restore_inherited_path(context)
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

fn projection_error<D: ConstraintDomain>(error: TypeConstraintError) -> TypeConstraintFailure<D> {
    match error {
        TypeConstraintError::Rejected(_) => {
            TypeConstraintFailure::Invariant(super::TypeConstraintFailureInvariant::Constraint(
                TypeConstraintInvariant::Projection(TypeConstraintProjectionInvariant::Mismatch),
            ))
        }
        TypeConstraintError::Abort(error) => TypeConstraintFailure::Abort(error),
        TypeConstraintError::Invariant(error) => TypeConstraintFailure::Invariant(
            super::TypeConstraintFailureInvariant::Constraint(error),
        ),
    }
}

fn validate_checked_selection<D: ConstraintDomain>(
    prepared: &PreparedSourceConstraint<D>,
    selected: D::AlternativeIndex,
    evidence: D::CheckedEvidence,
    actual: &TypeKind,
) -> Result<
    Option<(
        D::AlternativeIndex,
        Arc<D::CheckedEvidence>,
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
        Arc::new(evidence),
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
        .any(|probe| !sources.insert(probe.source))
    {
        Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::Ticket,
        ))
    } else {
        Ok(())
    }
}

fn closed_materialization_requests<D: ConstraintDomain>(
    path: &ConstraintPath<D>,
) -> Result<Box<[ClosedMaterializationRequest<D>]>, TypeConstraintError> {
    path.probe_trace
        .iter()
        .map(|probe| {
            let row = match (
                &probe.selection,
                probe.final_expected.as_ref(),
                probe.value_expected.as_ref(),
            ) {
                (
                    StoredSourceSelection::Checked {
                        alternative,
                        evidence,
                    },
                    Some(expected),
                    Some(_),
                ) => ClosedMaterializationRequestRow::Checked {
                    alternative: *alternative,
                    evidence: Arc::clone(evidence),
                    source_projection: probe.source_projection.clone(),
                    actual: probe.actual.clone(),
                    expected: expected.clone(),
                    canonical_branch: Arc::clone(&probe.branch),
                },
                (StoredSourceSelection::Unchecked, None, None) => {
                    ClosedMaterializationRequestRow::Unchecked {
                        source_projection: probe.source_projection.clone(),
                        actual: probe.actual.clone(),
                        canonical_branch: Arc::clone(&probe.branch),
                    }
                }
                _ => {
                    return Err(protocol_error(
                        TypeConstraintSourceProtocolInvariant::Outcome,
                    ));
                }
            };
            Ok(ClosedMaterializationRequest {
                source: probe.source,
                source_ordinal: PreparedSourceOrdinal(probe.source_ordinal),
                row,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn ensure_unique_request_sources<D: ConstraintDomain>(
    requests: &[ClosedMaterializationRequest<D>],
) -> Result<(), TypeConstraintError> {
    let mut sources = BTreeSet::new();
    if requests
        .iter()
        .any(|request| !sources.insert(request.source))
    {
        Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::Ticket,
        ))
    } else {
        Ok(())
    }
}

fn unique_request_ordinal<D: ConstraintDomain>(
    requests: &[ClosedMaterializationRequest<D>],
    source: D::Source,
) -> Result<PreparedSourceOrdinal, TypeConstraintError> {
    let mut matches = requests
        .iter()
        .filter(|request| request.source == source)
        .map(|request| request.source_ordinal);
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
    Ok(ordinal)
}

fn close_source_rows<A, D>(
    path: &mut ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<(), TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let effect_substitution = path
        .effects
        .substitution()
        .map_err(super::map_effect_environment_error)?;
    for probe in &mut path.probe_trace {
        let actual = project_type(
            &probe.actual,
            &path.bindings,
            &path.const_bindings,
            ConstraintClosurePolicy::SolutionCompletion,
            context,
        )?
        .value
        .substitute_effect_rows(&effect_substitution)
        .map_err(|_| {
            super::effect_invariant(
                super::TypeConstraintEffectInvariantKind::NonCanonicalInherited,
                None,
            )
        })?;
        probe.actual = actual.clone();
        if let StoredSourceSelection::Checked { evidence, .. } = &mut probe.selection {
            let projected =
                D::project_checked_evidence(evidence.as_ref(), &actual).ok_or_else(|| {
                    protocol_error(TypeConstraintSourceProtocolInvariant::InvalidEvidence)
                })?;
            *evidence = Arc::new(projected);
        }

        let Some(source_projection) =
            CheckedConstraintSourceProjection::derive(probe.prepared_source_projection, &actual)
        else {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            ));
        };
        probe.source_projection = source_projection.clone();

        let Some(value_expected) = probe.value_expected.as_ref() else {
            for equation in &mut path.equations {
                if equation.source_ordinal == Some(probe.source_ordinal) {
                    equation.actual = actual.clone();
                }
            }
            continue;
        };
        let value_expected = project_type(
            value_expected,
            &path.bindings,
            &path.const_bindings,
            ConstraintClosurePolicy::SolutionCompletion,
            context,
        )?
        .value
        .substitute_effect_rows(&effect_substitution)
        .map_err(|_| {
            super::effect_invariant(
                super::TypeConstraintEffectInvariantKind::NonCanonicalInherited,
                None,
            )
        })?;
        probe.value_expected = Some(value_expected.clone());
        probe.final_expected = Some(source_projection.compose_expected(&value_expected));
        for equation in &mut path.equations {
            if equation.source_ordinal == Some(probe.source_ordinal) {
                equation.actual = actual.clone();
                equation.source_projection = Some(source_projection.clone());
                equation.final_expected = probe.final_expected.clone();
                equation.pattern = probe
                    .final_expected
                    .as_ref()
                    .expect("just assigned final expected")
                    .clone();
            }
        }
    }
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
                .map(|probe| (probe.source_ordinal, probe.source))
                .cmp(
                    right
                        .probe_trace
                        .iter()
                        .map(|probe| (probe.source_ordinal, probe.source)),
                )
        })
        .then_with(|| left.probe_trace.len().cmp(&right.probe_trace.len()))
}

fn protocol_error(kind: TypeConstraintSourceProtocolInvariant) -> TypeConstraintError {
    TypeConstraintError::Invariant(TypeConstraintInvariant::SourceProtocol(kind))
}
