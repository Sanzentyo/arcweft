//! Callable-owned driver for the lower candidate constraint transaction.
//!
//! The driver owns callback checkpoints.  A callback attempt is always handed
//! to the client's affine close operation; the driver never unconditionally
//! rolls a checkpoint back and therefore cannot double-close a materialization.

use super::continuation::{
    CallConstraintInvariant, PreparedChildConstraintInitialization, PreparedConstraintAuthority,
    PreparedConstraintInitialization,
};
pub(crate) use super::limits::{CandidateConstraintWorkLoan, CandidateConstraintWorkSession};
use crate::types::constraints::context::TypeConstraintContext;
use crate::types::constraints::transaction::{
    ClosedMaterialization, ConstraintMaterialization, MaterializationCallbackBinding,
    MaterializationTicket, ProbeStart, ProbeSubmission, ProbeTicket, SuspendedConstraintFrontier,
    TypeConstraintTransaction,
};
use crate::types::constraints::{
    ClosedMaterializationSubmission, CompletedCandidateAlternatives, ConstraintDomain,
    ConstraintSourceId, ExpectedHint, MaterializationImmediateFailure, MaterializationOutcome,
    MaterializedSourceRequest, NestedConstraintPath, PendingChildConstraint,
    PreparedSourceConstraint, SolvedCandidate, SourceError, SourcePhase, SourceProbeOutcome,
    SourceProbeSelection, TypeConstraintAbort, TypeConstraintFailure,
    TypeConstraintFailureInvariant, TypeConstraintInitializationFailure, TypeConstraintInvariant,
    TypeConstraintSourceProtocolInvariant,
};
use crate::types::{ConstraintAcceptance, PreparedEffects, TypeKind};
use std::sync::Arc;

mod callbacks;
mod specialization;
pub(crate) use specialization::FunctionSpecializationFailure;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProbeCheckpointDisposition {
    Retain,
    Discard,
}

/// The callback vocabulary is deliberately owned by callable.  Source facts
/// cross the boundary only as typed lower results, while checkpoint closure
/// remains client-owned because it moves analyzer semantic state.
pub(crate) trait TypeConstraintClient<D: ConstraintDomain> {
    type ProbeCheckpoint;
    type MaterializationCheckpoint;
    type PreparedSealedBranchValue;

    /// Lend the running component its actual graph only for this lower
    /// operation. Source expression callbacks run after the loan ends.
    fn with_effect_relation<R>(
        &mut self,
        access: CandidateConstraintEffectAccess<'_>,
        operation: impl FnOnce(&mut crate::effect_row::prepared::PreparedEffectGraph) -> R,
    ) -> Result<R, SourceCheckpointFailure<D>>;

    fn probe_source(
        &mut self,
        checkpoint: &mut Self::ProbeCheckpoint,
        source: &mut CandidateConstraintSourceContext<'_, '_, D>,
    ) -> Result<SourceProbeOutcome<D>, SourceCallbackFailure<D>>;

    fn open_probe_checkpoint(
        &mut self,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
        source: ConstraintSourceId<D::Source>,
    ) -> Result<Self::ProbeCheckpoint, SourceCheckpointFailure<D>>;

    fn close_probe_checkpoint(
        &mut self,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
        checkpoint: Self::ProbeCheckpoint,
        disposition: ProbeCheckpointDisposition,
    ) -> Result<Option<D::ProbePreparation>, SourceCheckpointFailure<D>>;

    fn open_materialization_checkpoint(
        &mut self,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
        sources: &[ConstraintSourceId<D::Source>],
    ) -> Result<Self::MaterializationCheckpoint, SourceCheckpointFailure<D>>;

    fn materialize_sources<'h, I>(
        &mut self,
        sources: I,
        checkpoint: &mut Self::MaterializationCheckpoint,
        work: &mut CandidateConstraintWorkSession<'_>,
    ) -> Result<
        MaterializationOutcome<
            ConstraintSourceId<D::Source>,
            Self::PreparedSealedBranchValue,
            D::SourceErrorCause,
        >,
        SourceCallbackFailure<D>,
    >
    where
        I: IntoIterator<Item = MaterializedSourceRequest<'h, D>>,
        D::CheckedEvidence: 'h,
        D::ProbeSemanticBranch: 'h,
        D: 'h;

    fn close_materialization_checkpoint(
        &mut self,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
        checkpoint: Self::MaterializationCheckpoint,
        sealed: Option<Self::PreparedSealedBranchValue>,
    ) -> Result<Option<D::SealedBranchValue>, SourceCheckpointFailure<D>>;

    /// The live callback owner compares affine branch evidence after closing
    /// its callbacks, borrowing the component's original work reservation.
    /// Comparison aborts retain their typed cancellation or budget cause.
    fn sealed_branches_equal(
        &mut self,
        left: &D::SealedBranchValue,
        right: &D::SealedBranchValue,
        work: &mut CandidateConstraintWorkSession<'_>,
    ) -> Result<bool, crate::callable::SealedBranchComparisonFailure<D>>;

    fn finish(&mut self) -> Result<(), SourceCheckpointFailure<D>>
    where
        Self: Sized;
}

/// A running driver's permission to borrow its exact prepared relation. The
/// client cannot pair a foreign graph with the component's work or Γ.
pub(crate) struct CandidateConstraintEffectAccess<'a> {
    authority: &'a PreparedConstraintAuthority,
}

impl CandidateConstraintEffectAccess<'_> {
    pub(crate) fn with_graph<P, U, R>(
        self,
        graph: &mut super::PreparedCallGraph<P, U>,
        operation: impl FnOnce(&mut crate::effect_row::prepared::PreparedEffectGraph) -> R,
    ) -> Result<R, CallConstraintInvariant> {
        graph.validate_constraint_authority(self.authority)?;
        let (effects, _) = graph.effect_declarations_mut()?.relation();
        Ok(operation(effects))
    }
}

/// A probe owns its exact lower ticket and borrows the running component's
/// context. The driver alone constructs this capability; a client cannot pair
/// another source's hint with a fresh or unrelated accounting context.
pub(crate) struct CandidateConstraintSourceContext<'probe, 'control, D: ConstraintDomain> {
    ticket: ProbeTicket<D>,
    authority: &'probe PreparedConstraintAuthority,
    context:
        &'probe mut TypeConstraintContext<'control, CandidateConstraintWorkSession<'control>, D>,
}

impl<'probe, 'control, D: ConstraintDomain> CandidateConstraintSourceContext<'probe, 'control, D> {
    pub(crate) fn original_context(
        &self,
    ) -> Result<
        &crate::types::constraints::OriginalProducerContext,
        crate::types::constraints::TypeConstraintError,
    > {
        self.ticket.original_context()
    }
    pub(crate) fn validate_graph<P, U>(
        &self,
        graph: &super::PreparedCallGraph<P, U>,
    ) -> Result<(), CallConstraintInvariant> {
        graph.validate_constraint_authority(self.authority)
    }

    pub(crate) fn issue_child_initialization<P, U>(
        &self,
        graph: &super::PreparedCallGraph<P, U>,
        application: D::Application,
        site: crate::callable::CheckedCallSite,
        candidate: &crate::callable::PreparedResolvedCallable,
        transition: super::schema::CallableGroupTransition,
        enclosing: &crate::callable::EnclosingGenericParameterScope,
    ) -> Result<PreparedChildConstraintInitialization<D>, CallConstraintInvariant>
    where
        P: super::PreparedCallPrefixPayload<Unselected = U>,
    {
        graph.validate_and_issue_child_constraint_initialization(
            self.authority,
            self.ticket.receipt(),
            application,
            site,
            candidate,
            transition,
            enclosing,
        )
    }

    pub(crate) fn source(&self) -> ConstraintSourceId<D::Source> {
        self.ticket.source()
    }

    pub(crate) fn receipt(&self) -> crate::types::constraints::ConstraintSourceReceipt<D> {
        self.ticket.receipt()
    }

    pub(crate) fn work(&mut self) -> &mut CandidateConstraintWorkSession<'control> {
        self.context.accounting_mut()
    }

    pub(crate) fn work_loan(&mut self) -> CandidateConstraintWorkLoan<'control> {
        self.context.accounting_mut().loan()
    }

    pub(crate) fn application(
        &self,
    ) -> Result<D::Application, crate::types::constraints::TypeConstraintError> {
        self.ticket.application()
    }

    pub(crate) fn with_hint<R>(
        &mut self,
        operation: impl for<'hint> FnOnce(ExpectedHint<'hint, D>, &mut Self) -> R,
    ) -> R {
        let input = self.ticket.input();
        input.with_hint(|hint| operation(hint, self))
    }

    /// Attach the source's observed term to this branch exactly once.
    pub(crate) fn observe(
        &mut self,
        declarations: &mut super::PreparedCallableEffectDeclarations,
        result: crate::types::constraints::SourceProbeResult<D>,
    ) -> Result<SourceProbeOutcome<D>, SourceCallbackFailure<D>> {
        let (effects, _) = declarations.relation();
        self.ticket
            .observe_prepared(result, effects, self.context)
            .map(SourceProbeOutcome::Accepted)
            .map_err(SourceCallbackFailure::Constraint)
    }

    pub(crate) fn observe_child(
        &mut self,
        pending: PendingChildConstraint<D>,
        branch: D::ProbeSemanticBranch,
        selection: SourceProbeSelection<D::AlternativeIndex, Arc<D::ObservedEvidence>>,
    ) -> Result<SourceProbeOutcome<D>, SourceCallbackFailure<D>> {
        self.ticket
            .observe_child(pending, branch, selection)
            .map(SourceProbeOutcome::Accepted)
            .map_err(SourceCallbackFailure::Constraint)
    }

    /// Run a child candidate on a fork of this exact source's path. Its
    /// application is admitted into the same scope/equation inventory, so the
    /// parent can relate the child result before either application seals.
    pub(crate) fn with_child_driver<C, R>(
        &mut self,
        initialization: PreparedChildConstraintInitialization<D>,
        application: D::Application,
        site: crate::callable::CheckedCallSite,
        candidate: &crate::callable::PreparedResolvedCallable,
        mut client: C,
        drive: impl for<'driver> FnOnce(CandidateConstraintDriver<'driver, 'control, D, C>) -> R,
    ) -> Result<R, CandidateConstraintDriverStartFailure<D>>
    where
        C: TypeConstraintClient<D>,
    {
        let receipt = self.ticket.receipt();
        if !initialization.validates(self.authority, &receipt, application, site, candidate) {
            return Err(CandidateConstraintDriverStartFailure::Prepared(
                CallConstraintInvariant::PreparedCallSiteMismatch,
            ));
        }
        let nested: NestedConstraintPath<D> =
            self.ticket.fork_for_child(self.context).map_err(|error| {
                CandidateConstraintDriverStartFailure::Lower(match error {
                    crate::types::constraints::TypeConstraintError::Abort(error) => {
                        TypeConstraintInitializationFailure::Abort(error)
                    }
                    crate::types::constraints::TypeConstraintError::Invariant(error) => {
                        TypeConstraintInitializationFailure::Invariant(error)
                    }
                    crate::types::constraints::TypeConstraintError::Rejected(_) => {
                        TypeConstraintInitializationFailure::Invariant(
                            crate::types::constraints::TypeConstraintInvariant::SourceProtocol(
                                TypeConstraintSourceProtocolInvariant::Outcome,
                            ),
                        )
                    }
                })
            })?;
        let (authority, parameters, inherited) = initialization
            .into_lower_parts()
            .map_err(CandidateConstraintDriverStartFailure::Prepared)?;
        let original = authority.original_context(self.context).map_err(|error| {
            CandidateConstraintDriverStartFailure::Lower(
                TypeConstraintInitializationFailure::original_context(error),
            )
        })?;
        let lower = client
            .with_effect_relation(
                CandidateConstraintEffectAccess {
                    authority: &authority,
                },
                |graph| {
                    TypeConstraintTransaction::initialize_from_nested_path(
                        self.context,
                        graph,
                        original,
                        application,
                        parameters,
                        inherited,
                        nested,
                    )
                },
            )
            .map_err(|failure| {
                CandidateConstraintDriverStartFailure::EffectRelation(
                    CandidateConstraintCallbacks::<D, C>::checkpoint_failure(failure),
                )
            })?
            .map_err(CandidateConstraintDriverStartFailure::Lower)?;
        let callbacks = CandidateConstraintCallbacks::new(self.context, authority, client)
            .map_err(|error| {
                CandidateConstraintDriverStartFailure::Lower(
                    TypeConstraintInitializationFailure::original_context(error),
                )
            })?;
        let driver = CandidateConstraintDriver { lower, callbacks };
        let result = drive(driver);
        Ok(result)
    }

    /// Branch constraints while retaining the same lexical scopes, work
    /// reservation and physical-source callback checkpoint.
    pub(crate) fn with_alternative<R>(
        &mut self,
        operation: impl FnOnce(
            &mut CandidateConstraintSourceContext<'_, 'control, D>,
        ) -> Result<R, SourceCallbackFailure<D>>,
    ) -> Result<R, SourceCallbackFailure<D>> {
        let ticket = self
            .ticket
            .fork(self.context)
            .map_err(SourceCallbackFailure::Constraint)?;
        let mut alternative = CandidateConstraintSourceContext {
            ticket,
            authority: self.authority,
            context: self.context,
        };
        operation(&mut alternative)
    }
}

/// Callback failures are deliberately separate from ordinary semantic
/// rejections.  Only the driver may validate and promote a fatal source error.
pub(crate) enum SourceCallbackFailure<D: ConstraintDomain> {
    Fatal(Box<SourceError<ConstraintSourceId<D::Source>, D::SourceErrorCause>>),
    Abort(TypeConstraintAbort),
    Invariant(Box<D::ClientInvariant>),
    Constraint(crate::types::constraints::TypeConstraintError),
}

pub(crate) enum SourceCheckpointFailure<D: ConstraintDomain> {
    Constraint(crate::types::constraints::TypeConstraintError),
    Protocol(TypeConstraintSourceProtocolInvariant),
    Client(Box<D::ClientInvariant>),
}

/// Comparison has no source ticket and cannot mint or promote a fatal source
/// error. It may report a controlled constraint failure or its owner's invariant.
pub(crate) enum SealedBranchComparisonFailure<D: ConstraintDomain> {
    Constraint(crate::types::constraints::TypeConstraintError),
    Client(Box<D::ClientInvariant>),
}

impl<D: ConstraintDomain> From<crate::types::constraints::TypeConstraintError>
    for SealedBranchComparisonFailure<D>
{
    fn from(error: crate::types::constraints::TypeConstraintError) -> Self {
        Self::Constraint(error)
    }
}

impl<D: ConstraintDomain> From<TypeConstraintAbort> for SealedBranchComparisonFailure<D> {
    fn from(error: TypeConstraintAbort) -> Self {
        crate::types::constraints::TypeConstraintError::Abort(error).into()
    }
}

impl<D: ConstraintDomain> std::fmt::Debug for SealedBranchComparisonFailure<D> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Constraint(error) => formatter.debug_tuple("Constraint").field(error).finish(),
            Self::Client(_) => formatter.write_str("Client(..)"),
        }
    }
}

impl<D: ConstraintDomain> SourceCallbackFailure<D> {
    pub(crate) fn fatal(
        error: SourceError<ConstraintSourceId<D::Source>, D::SourceErrorCause>,
    ) -> Self {
        Self::Fatal(Box::new(error))
    }

    pub(crate) fn invariant(invariant: D::ClientInvariant) -> Self {
        Self::Invariant(Box::new(invariant))
    }
}

impl<D: ConstraintDomain> SourceCheckpointFailure<D> {
    pub(crate) fn client(invariant: D::ClientInvariant) -> Self {
        Self::Client(Box::new(invariant))
    }
}

#[derive(Clone)]
struct SourceCallbackTicketIdentity {
    issuer: Arc<SourceCallbackTicketIssuer>,
    ordinal: u64,
}

struct SourceCallbackTicketIssuer;

enum SourceCallbackAuthority<D: ConstraintDomain> {
    Probe {
        source: ConstraintSourceId<D::Source>,
    },
    Materialize {
        binding: MaterializationCallbackBinding<D>,
    },
}

/// Driver-minted affine callback authority.  Deliberately no Clone or public
/// identity/source accessors are provided.
pub(crate) struct SourceCallbackTicket<D: ConstraintDomain> {
    identity: SourceCallbackTicketIdentity,
    authority: SourceCallbackAuthority<D>,
}

pub(crate) struct BoundSourceCheckpoint<C> {
    identity: SourceCallbackTicketIdentity,
    checkpoint: C,
}

/// Mapper-issued atomic source unit. All slots from one authored argument are
/// retained together so lower can finish their real callbacks before it
/// commits a conjunctive group rejection.
pub(crate) struct PreparedSourceConstraintGroup<D: ConstraintDomain> {
    sources: Box<[PreparedSourceConstraint<D>]>,
}

impl<D: ConstraintDomain> PreparedSourceConstraintGroup<D> {
    pub(crate) fn seal(
        sources: impl IntoIterator<Item = PreparedSourceConstraint<D>>,
    ) -> Result<Self, TypeConstraintInvariant> {
        let sources = sources.into_iter().collect::<Vec<_>>().into_boxed_slice();
        if sources.is_empty() {
            return Err(TypeConstraintInvariant::SourceProtocol(
                TypeConstraintSourceProtocolInvariant::Outcome,
            ));
        }
        Ok(Self { sources })
    }

    pub(crate) fn sources(&self) -> &[PreparedSourceConstraint<D>] {
        &self.sources
    }

    fn into_sources(self) -> Box<[PreparedSourceConstraint<D>]> {
        self.sources
    }
}

pub(crate) struct CandidateConstraintDriver<
    'driver,
    'control,
    D: ConstraintDomain,
    C: TypeConstraintClient<D>,
> {
    lower: TypeConstraintTransaction<D>,
    callbacks: CandidateConstraintCallbacks<'driver, 'control, D, C>,
}

/// The callback owner has no inference frontier. Its context, graph authority,
/// client and affine checkpoint identities remain together in either phase.
struct CandidateConstraintCallbacks<
    'driver,
    'control,
    D: ConstraintDomain,
    C: TypeConstraintClient<D>,
> {
    context:
        &'driver mut TypeConstraintContext<'control, CandidateConstraintWorkSession<'control>, D>,
    authority: PreparedConstraintAuthority,
    client: C,
    ticket_issuer: Arc<SourceCallbackTicketIssuer>,
    next_ticket_ordinal: u64,
    active_ticket: Option<SourceCallbackTicketIdentity>,
}

/// Callback completion keeps the original lower frontier beside its borrowed
/// work context until it is exported as a root or submitted to its parent.
struct SuspendedCandidateConstraint<'driver, 'control, D: ConstraintDomain> {
    frontier: SuspendedConstraintFrontier<D>,
    context:
        &'driver mut TypeConstraintContext<'control, CandidateConstraintWorkSession<'control>, D>,
}

pub(crate) enum CandidateConstraintDriverStartFailure<D: ConstraintDomain> {
    Prepared(CallConstraintInvariant),
    Lower(TypeConstraintInitializationFailure),
    EffectRelation(TypeConstraintFailure<D>),
}

impl<D: ConstraintDomain> std::fmt::Debug for CandidateConstraintDriverStartFailure<D> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Prepared(error) => formatter.debug_tuple("Prepared").field(error).finish(),
            Self::Lower(error) => formatter.debug_tuple("Lower").field(error).finish(),
            Self::EffectRelation(error) => formatter
                .debug_tuple("EffectRelation")
                .field(error)
                .finish(),
        }
    }
}

impl<'a> CandidateConstraintWorkSession<'a> {
    /// Own the component's accounting context while lending its driver. The
    /// driver cannot return or replace the context; finishing its transaction
    /// and releasing the surrounding accounting reservation are separate acts.
    pub(crate) fn with_driver<D, C, R>(
        self,
        original: crate::types::constraints::OriginalProducerContext,
        application: D::Application,
        initialization: PreparedConstraintInitialization,
        client: C,
        drive: impl for<'driver> FnOnce(CandidateConstraintDriver<'driver, 'a, D, C>) -> R,
    ) -> Result<R, CandidateConstraintDriverStartFailure<D>>
    where
        D: ConstraintDomain,
        C: TypeConstraintClient<D>,
    {
        let mut context = TypeConstraintContext::with_accounting(self);
        Self::drive_with_context(
            &mut context,
            original,
            application,
            initialization,
            client,
            drive,
        )
    }

    #[cfg(test)]
    pub(crate) fn with_fixture_driver<D, C, R>(
        self,
        application: D::Application,
        initialization: PreparedConstraintInitialization,
        client: C,
        drive: impl for<'driver> FnOnce(CandidateConstraintDriver<'driver, 'a, D, C>) -> R,
    ) -> Result<R, CandidateConstraintDriverStartFailure<D>>
    where
        D: ConstraintDomain,
        C: TypeConstraintClient<D>,
    {
        let mut context = TypeConstraintContext::with_accounting(self);
        let original = initialization
            .fixture_original_context(&mut context)
            .map_err(|error| {
                CandidateConstraintDriverStartFailure::Lower(
                    TypeConstraintInitializationFailure::original_context(error),
                )
            })?;
        Self::drive_with_context(
            &mut context,
            original,
            application,
            initialization,
            client,
            drive,
        )
    }

    fn drive_with_context<D, C, R>(
        context: &mut TypeConstraintContext<'a, CandidateConstraintWorkSession<'a>, D>,
        original: crate::types::constraints::OriginalProducerContext,
        application: D::Application,
        initialization: PreparedConstraintInitialization,
        mut client: C,
        drive: impl for<'driver> FnOnce(CandidateConstraintDriver<'driver, 'a, D, C>) -> R,
    ) -> Result<R, CandidateConstraintDriverStartFailure<D>>
    where
        D: ConstraintDomain,
        C: TypeConstraintClient<D>,
    {
        let (authority, parameter_scope, inherited, imported_scope) = initialization
            .into_lower_parts()
            .map_err(CandidateConstraintDriverStartFailure::Prepared)?;
        if !authority.admits_original(&original) {
            return Err(CandidateConstraintDriverStartFailure::Lower(
                TypeConstraintInitializationFailure::Invariant(
                    TypeConstraintInvariant::OriginalProducer,
                ),
            ));
        }
        let lower = client
            .with_effect_relation(
                CandidateConstraintEffectAccess {
                    authority: &authority,
                },
                |graph| {
                    TypeConstraintTransaction::initialize_with_imported(
                        context,
                        graph,
                        original,
                        application,
                        parameter_scope,
                        inherited,
                        imported_scope,
                    )
                },
            )
            .map_err(|failure| {
                CandidateConstraintDriverStartFailure::EffectRelation(
                    CandidateConstraintCallbacks::<D, C>::checkpoint_failure(failure),
                )
            })?
            .map_err(CandidateConstraintDriverStartFailure::Lower)?;
        let callbacks =
            CandidateConstraintCallbacks::new(context, authority, client).map_err(|error| {
                CandidateConstraintDriverStartFailure::Lower(
                    TypeConstraintInitializationFailure::original_context(error),
                )
            })?;
        let driver = CandidateConstraintDriver { lower, callbacks };
        Ok(drive(driver))
    }
}

impl<'driver, 'control, D, C> CandidateConstraintDriver<'driver, 'control, D, C>
where
    D: ConstraintDomain,
    C: TypeConstraintClient<D>,
{
    pub(crate) fn request_projection(
        &mut self,
        key: D::Projection,
        value: &TypeKind<PreparedEffects>,
        closure: crate::types::constraints::TypeConstraintProjectionClosure,
        input: Option<&TypeKind<PreparedEffects>>,
    ) {
        if let Err(error) = self.with_effect_relation(|lower, context, graph| {
            lower.request_projection_with_input(context, graph, key, value, closure, input);
        }) {
            self.lower.record_failure(error);
        }
    }

    fn with_effect_relation<R>(
        &mut self,
        operation: impl FnOnce(
            &mut TypeConstraintTransaction<D>,
            &mut TypeConstraintContext<'control, CandidateConstraintWorkSession<'control>, D>,
            &mut crate::effect_row::prepared::PreparedEffectGraph,
        ) -> R,
    ) -> Result<R, TypeConstraintFailure<D>> {
        let access = CandidateConstraintEffectAccess {
            authority: &self.callbacks.authority,
        };
        self.callbacks
            .client
            .with_effect_relation(access, |effects| {
                operation(&mut self.lower, self.callbacks.context, effects)
            })
            .map_err(CandidateConstraintCallbacks::<D, C>::checkpoint_failure)
    }

    pub(crate) fn constrain(
        &mut self,
        pattern: &TypeKind<PreparedEffects>,
        actual: &TypeKind<PreparedEffects>,
        acceptance: ConstraintAcceptance,
    ) {
        if let Err(error) = self.with_effect_relation(|lower, context, effects| {
            lower.constrain(context, effects, pattern, actual, acceptance);
        }) {
            self.lower.record_failure(error);
        }
    }

    pub(crate) fn constrain_result(
        &mut self,
        pattern: &TypeKind<PreparedEffects>,
        actual: &TypeKind<PreparedEffects>,
        acceptance: ConstraintAcceptance,
    ) {
        if let Err(error) = self.with_effect_relation(|lower, context, effects| {
            lower.constrain_result(context, effects, pattern, actual, acceptance);
        }) {
            self.lower.record_failure(error);
        }
    }

    pub(crate) fn constrain_effect_equality(
        &mut self,
        left: &crate::effect_row::prepared::PreparedEffectRow,
        right: &crate::effect_row::prepared::PreparedEffectRow,
    ) {
        if let Err(error) = self.with_effect_relation(|lower, context, effects| {
            lower.constrain_effect_equality(context, effects, left, right);
        }) {
            self.lower.record_failure(error);
        }
    }

    /// Run one prepared source through every correlated frontier row.
    pub(crate) fn probe_prepared_source(
        &mut self,
        prepared: PreparedSourceConstraint<D>,
        acceptance: ConstraintAcceptance,
    ) -> Result<(), TypeConstraintFailure<D>> {
        let started = self
            .lower
            .begin_prepared_probe(self.callbacks.context, prepared, acceptance)
            .map_err(TypeConstraintFailure::from)?;
        if matches!(started, ProbeStart::Skipped) {
            return Ok(());
        }
        loop {
            let ticket = self
                .with_effect_relation(|lower, context, effects| lower.next_probe(context, effects))?
                .map_err(TypeConstraintFailure::from)?;
            let Some(lower_ticket) = ticket else {
                break;
            };
            let source = lower_ticket.source();
            let (callback_ticket, mut checkpoint) =
                match self.callbacks.begin_probe_callback(source) {
                    Ok(callback) => callback,
                    Err(error) => {
                        self.lower.record_failure(error);
                        break;
                    }
                };
            let mut source_context = CandidateConstraintSourceContext {
                ticket: lower_ticket,
                authority: &self.callbacks.authority,
                context: self.callbacks.context,
            };
            let attempt = self
                .callbacks
                .client
                .probe_source(&mut checkpoint.checkpoint, &mut source_context);
            let lower_ticket = source_context.ticket;
            let input = lower_ticket.input();
            let submission =
                match self
                    .callbacks
                    .close_probe_callback(callback_ticket, checkpoint, attempt)
                {
                    Ok(submission) => submission,
                    Err(error) => {
                        self.lower.record_failure(error);
                        break;
                    }
                };
            if let Err(error) = self.with_effect_relation(|lower, context, effects| {
                lower.submit_probe(context, effects, input, submission)
            })? {
                self.lower.record_failure(error.into());
                break;
            }
        }
        Ok(())
    }

    /// Source-order name used by the callable mapper; the argument is always
    /// the complete prepared constraint, never a raw expected type.
    pub(crate) fn probe_source(
        &mut self,
        prepared: PreparedSourceConstraint<D>,
        acceptance: ConstraintAcceptance,
    ) -> Result<(), TypeConstraintFailure<D>> {
        self.probe_prepared_source(prepared, acceptance)
    }

    pub(crate) fn probe_source_group(
        &mut self,
        group: PreparedSourceConstraintGroup<D>,
        acceptance: ConstraintAcceptance,
    ) -> Result<(), TypeConstraintFailure<D>> {
        let sources = group.into_sources();
        let started = self
            .lower
            .begin_prepared_probe_group(sources.len())
            .map_err(TypeConstraintFailure::from)?;
        if matches!(started, ProbeStart::Skipped) {
            return Ok(());
        }
        for source in sources {
            self.probe_prepared_source(source, acceptance)?;
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<SolvedCandidate<D>, TypeConstraintFailure<D>> {
        self.finish_alternatives()?.into_unique()
    }

    pub(crate) fn finish_alternatives(
        mut self,
    ) -> Result<CompletedCandidateAlternatives<D>, TypeConstraintFailure<D>> {
        match self.with_effect_relation(|lower, context, graph| lower.close(context, graph)) {
            Ok(Ok(())) => {}
            Ok(Err(failure)) => self.lower.record_failure(failure.into()),
            Err(failure) => self.lower.record_failure(failure),
        }
        match self.lower.into_materialization() {
            Ok(owner) => self.callbacks.finish_materialization(owner),
            Err(failure) => {
                // Finish the real client even when preparation could not close.
                let _ = self.callbacks.finish_callbacks();
                Err(failure)
            }
        }
    }

    pub(crate) fn defer_child_result(
        self,
        key: D::Projection,
    ) -> Result<PendingChildConstraint<D>, TypeConstraintFailure<D>> {
        let suspended = self.pause_result(key)?;
        suspended.frontier.into_child(suspended.context)
    }

    /// Export an unresolved root after the source client closes its callbacks.
    /// The component receives the real frontier, not a completed call or type.
    pub(crate) fn suspend_result(
        self,
        key: D::Projection,
    ) -> Result<SuspendedConstraintFrontier<D>, TypeConstraintFailure<D>> {
        self.pause_result(key).map(|suspended| suspended.frontier)
    }

    fn pause_result(
        mut self,
        key: D::Projection,
    ) -> Result<SuspendedCandidateConstraint<'driver, 'control, D>, TypeConstraintFailure<D>> {
        if let Err(failure) = self.callbacks.finish_callbacks() {
            self.lower.record_failure(failure);
        }
        let frontier = self.lower.suspend_result(self.callbacks.context, key)?;
        Ok(SuspendedCandidateConstraint {
            frontier,
            context: self.callbacks.context,
        })
    }
}

impl PreparedConstraintAuthority {
    /// Consume an already canonical component without constructing an inference
    /// transaction or preparing its sources again. The actual client graph is
    /// checked against both this callable authority and the component lease.
    pub(super) fn materialize_component<'control, D, C>(
        self,
        owner: ConstraintMaterialization<D>,
        context: &mut TypeConstraintContext<'control, CandidateConstraintWorkSession<'control>, D>,
        client: C,
    ) -> Result<CompletedCandidateAlternatives<D>, TypeConstraintFailure<D>>
    where
        D: ConstraintDomain,
        C: TypeConstraintClient<D>,
    {
        CandidateConstraintCallbacks::new(context, self, client)?.finish_materialization(owner)
    }
}

fn materialization_immediate_from_failure<D: ConstraintDomain>(
    failure: TypeConstraintFailure<D>,
) -> MaterializationImmediateFailure<D> {
    match failure {
        TypeConstraintFailure::Abort(error) => MaterializationImmediateFailure::Abort(error),
        TypeConstraintFailure::Invariant(error) => {
            MaterializationImmediateFailure::Invariant(error)
        }
        TypeConstraintFailure::Rejected(_) | TypeConstraintFailure::FatalSource(_) => {
            MaterializationImmediateFailure::Invariant(TypeConstraintFailureInvariant::Constraint(
                TypeConstraintInvariant::SourceProtocol(
                    TypeConstraintSourceProtocolInvariant::Outcome,
                ),
            ))
        }
    }
}

#[cfg(test)]
impl TypeConstraintClient<crate::types::NoConstraintClient> for crate::types::NoConstraintClient {
    type ProbeCheckpoint = ();
    type MaterializationCheckpoint = ();
    type PreparedSealedBranchValue = ();

    fn probe_source(
        &mut self,
        _checkpoint: &mut Self::ProbeCheckpoint,
        source: &mut CandidateConstraintSourceContext<'_, '_, crate::types::NoConstraintClient>,
    ) -> Result<
        SourceProbeOutcome<crate::types::NoConstraintClient>,
        SourceCallbackFailure<crate::types::NoConstraintClient>,
    > {
        Err(SourceCallbackFailure::fatal(SourceError::new(
            source.source(),
            crate::types::constraints::SourcePhase::Probe,
            (),
        )))
    }

    fn open_probe_checkpoint(
        &mut self,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
        _source: ConstraintSourceId<()>,
    ) -> Result<Self::ProbeCheckpoint, SourceCheckpointFailure<crate::types::NoConstraintClient>>
    {
        Ok(())
    }

    fn close_probe_checkpoint(
        &mut self,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
        _checkpoint: Self::ProbeCheckpoint,
        disposition: ProbeCheckpointDisposition,
    ) -> Result<Option<()>, SourceCheckpointFailure<crate::types::NoConstraintClient>> {
        Ok((disposition == ProbeCheckpointDisposition::Retain).then_some(()))
    }

    fn open_materialization_checkpoint(
        &mut self,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
        _sources: &[ConstraintSourceId<()>],
    ) -> Result<
        Self::MaterializationCheckpoint,
        SourceCheckpointFailure<crate::types::NoConstraintClient>,
    > {
        Ok(())
    }

    fn materialize_sources<'h, I>(
        &mut self,
        sources: I,
        _checkpoint: &mut Self::MaterializationCheckpoint,
        _work: &mut CandidateConstraintWorkSession<'_>,
    ) -> Result<
        MaterializationOutcome<ConstraintSourceId<()>, (), ()>,
        SourceCallbackFailure<crate::types::NoConstraintClient>,
    >
    where
        I: IntoIterator<Item = MaterializedSourceRequest<'h, crate::types::NoConstraintClient>>,
        (): 'h,
    {
        Err(SourceCallbackFailure::fatal(SourceError::new(
            *sources
                .into_iter()
                .next()
                .expect("materialization request")
                .source(),
            crate::types::constraints::SourcePhase::Materialize,
            (),
        )))
    }

    fn close_materialization_checkpoint(
        &mut self,
        work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
        _checkpoint: Self::MaterializationCheckpoint,
        _sealed: Option<Self::PreparedSealedBranchValue>,
    ) -> Result<Option<()>, SourceCheckpointFailure<crate::types::NoConstraintClient>> {
        Ok(None)
    }

    fn sealed_branches_equal(
        &mut self,
        _: &(),
        _: &(),
        _: &mut CandidateConstraintWorkSession<'_>,
    ) -> Result<
        bool,
        crate::callable::SealedBranchComparisonFailure<crate::types::NoConstraintClient>,
    > {
        Ok(true)
    }

    fn finish(&mut self) -> Result<(), SourceCheckpointFailure<crate::types::NoConstraintClient>> {
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::callable::limits::{PRODUCTION_CALLABLE_LIMITS, ResolverWork};
    use crate::effect_row::prepared::{
        PreparedEffectDelta, PreparedEffectFailure, PreparedEffectGraph, PreparedEffectReplayRoot,
        PreparedEffectSchemeRef,
    };
    use crate::env::nominal::{AcceptedNominalId, AcceptedNominalOwnerId};
    use crate::types::constraints::context::TypeConstraintWorkReport;
    use crate::types::constraints::{
        PreparedSourceAlternative, PreparedSourceConstraint, SourcePhase, SourceProbeResult,
        TypeConstraintParameterScope,
    };
    use crate::types::{GenericParameterOwnerId, GenericTypeParameterId, TypeKind};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    fn accepted_owner(owner: u64) -> AcceptedNominalId {
        let path = arcweft_lang_syntax::types::TypePath::from(
            arcweft_lang_syntax::ast::symbol_path::ProjectSymbolPath::new(
                arcweft_lang_syntax::ast::module_path::ModulePathRoot::ImplicitCrate,
                [
                    arcweft_lang_syntax::ast::symbol_path::ProjectSymbolSegment::try_new(format!(
                        "ConstraintOwner{owner}"
                    ))
                    .expect("constraint owner path segment"),
                ],
            )
            .expect("constraint owner path"),
        );
        AcceptedNominalId::new(AcceptedNominalOwnerId::Standard, path)
    }

    pub(super) fn accepted_type(owner: u64, ordinal: u16) -> GenericTypeParameterId {
        GenericTypeParameterId::new(
            GenericParameterOwnerId::AcceptedNominal(accepted_owner(owner)),
            ordinal,
        )
    }

    fn initialization(
        parameter_scope: TypeConstraintParameterScope,
    ) -> PreparedConstraintInitialization {
        base_initialization(parameter_scope)
    }

    pub(crate) fn no_constraint_initialization() -> PreparedConstraintInitialization {
        base_initialization(TypeConstraintParameterScope::empty())
    }

    fn base_initialization(
        scope: TypeConstraintParameterScope,
    ) -> PreparedConstraintInitialization {
        initialization_from_graph(&crate::callable::PreparedCallGraph::<()>::new(), scope)
    }

    pub(super) fn initialization_from_graph(
        graph: &crate::callable::PreparedCallGraph<()>,
        scope: TypeConstraintParameterScope,
    ) -> PreparedConstraintInitialization {
        let parameters = scope
            .iter()
            .map(|(parameter, _)| parameter.clone())
            .collect::<Vec<_>>();
        let issuer = match parameters.first().map(|parameter| {
            parameter
                .free_parameter()
                .expect("declaration-owned fixture")
                .owner()
                .clone()
        }) {
            None => crate::callable::CallableGenericParameterIssuer::empty(),
            Some(crate::types::GenericParameterOwnerId::AcceptedNominal(owner)) => {
                crate::callable::CallableGenericParameterIssuer::accepted_nominal(
                    owner,
                    u16::try_from(parameters.len()).expect("test generic count"),
                    0,
                )
                .expect("test generic issuer")
            }
            Some(_) => panic!("test graph fixture requires accepted nominal owners"),
        };
        let groups = (0..1)
            .map(|group| {
                let group_index =
                    crate::callable::CallableGroupIndex::try_from_usize(group).expect("test group");
                let group_parameters = parameters
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| {
                        crate::callable::CallableParameter::try_new(
                            crate::callable::CallableParameterIndex::try_from_usize(index)
                                .expect("test parameter"),
                            Some(
                                arcweft_lang_syntax::name::CallArgumentName::try_new(&format!(
                                    "arg{index}"
                                ))
                                .expect("test name"),
                            ),
                            crate::callable::CallableParameterAdmission::checked(
                                TypeKind::GenericParam(parameter.clone()),
                            ),
                            crate::callable::CallableParameterPassing::PositionalOnly,
                            crate::callable::CallableParameterPresence::Required,
                            None,
                            None,
                        )
                        .expect("test parameter row")
                    })
                    .collect::<Vec<_>>();
                crate::callable::CallableParameterGroup::try_new(
                    group_index,
                    if group == 0 {
                        crate::callable::CallableGroupKind::Initial
                    } else {
                        crate::callable::CallableGroupKind::Curried
                    },
                    group_parameters,
                    &crate::callable::PRODUCTION_CALLABLE_LIMITS,
                )
                .expect("test group")
            })
            .collect::<Vec<_>>();
        let schema = crate::callable::CallableSignatureSchema::try_new(
            groups,
            TypeKind::Unit,
            crate::callable::CallableEffectSchema::fixed(crate::effect_row::EffectRow::closed(
                crate::effects::EffectSet::new(),
            )),
            crate::callable::CallableArgumentPolicy::new(
                crate::callable::UnknownNamedArgumentPolicy::Reject,
                crate::callable::SpreadArgumentPolicy::Reject,
            ),
            crate::callable::CallableValidator::Ordinary,
            issuer,
            &crate::callable::PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("test schema");
        let candidate = crate::callable::PreparedResolvedCallable::try_from_intrinsic(
            crate::callable::CallableCandidateId::FxConstructor(
                crate::callable::FxSourceConstructor::Style,
            ),
            crate::callable::SignatureOrigin::Language {
                family: crate::callable::LanguageCallableFamily::FxConstructor,
            },
            Arc::new(schema),
            crate::callable::CallableInstantiation::None,
            Vec::new(),
            &crate::callable::PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("test candidate");
        let enclosing = crate::callable::EnclosingGenericParameterScope::sealed(
            std::iter::empty::<crate::types::GenericTypeParameterId>(),
            std::iter::empty::<crate::types::GenericConstParameterId>(),
            std::iter::empty::<crate::types::GenericEffectParameterId>(),
        )
        .expect("test enclosing scope");
        graph
            .validate_and_issue_base_constraint_initialization(&candidate, &enclosing)
            .expect("test initialization gate")
    }

    #[derive(Eq, PartialEq)]
    struct Branch;

    #[derive(Eq, PartialEq)]
    struct Sealed;

    #[derive(Debug)]
    struct Domain;

    impl ConstraintDomain for Domain {
        type Application = u32;
        type Source = u8;
        type AlternativeIndex = u8;
        type EvidenceRule = ();
        type ObservedEvidence = ();
        type CheckedEvidence = ();
        type ProbeSemanticBranch = Branch;
        type ProbePreparation = ();
        type SealedBranchValue = Sealed;
        type Projection = u8;
        type SourceErrorCause = &'static str;
        type ClientInvariant = ();

        fn evidence_accepts(_: &Self::EvidenceRule, _: &Self::ObservedEvidence) -> bool {
            true
        }

        fn project_checked_evidence(
            _: &Self::ObservedEvidence,
            _: &TypeKind,
        ) -> Option<Self::CheckedEvidence> {
            Some(())
        }

        fn alternative_ordinal(index: &Self::AlternativeIndex) -> u32 {
            u32::from(*index)
        }

        fn client_invariant_source(_: &Self::ClientInvariant) -> Self::Source {
            1
        }

        fn empty_sealed_branch() -> Self::SealedBranchValue {
            Sealed
        }
    }

    #[derive(Clone, Copy)]
    enum CallbackMode {
        Success,
        SourceAlternatives,
        Rejected,
        Fatal,
        ProbeAbort,
        ProbeInvariant,
        GroupRejectedThenSuccess,
        GroupRejectedThenFatal,
        GroupRejectedThenAbort,
        GroupRejectedThenInvariant,
        WrongFatalSource,
        WrongFatalApplication,
        MaterializationWrongApplication,
        MaterializationRejected,
        MaterializationWrongSource,
        MaterializationFatal,
        MaterializationReversedFatal,
        MaterializationAbort,
        MaterializationInvariant,
        OpenProbeFailure,
        OpenMaterializationFailure,
        CloseProbeClientFailure,
        MissingProbePreparation,
        UnexpectedProbePreparation,
        CloseMaterializationClientFailure,
    }

    #[derive(Default)]
    struct Counts {
        callback_finish: AtomicUsize,
        sealed_compare: AtomicUsize,
        probe_begin: AtomicUsize,
        probe_close: AtomicUsize,
        probe_call: AtomicUsize,
        materialize_begin: AtomicUsize,
        materialize_close: AtomicUsize,
        materialize_call: AtomicUsize,
        probe_reservation: AtomicUsize,
        comparison_reservation: AtomicUsize,
        comparison_graph: Mutex<Option<ComparisonGraph>>,
    }

    struct ComparisonGraph {
        graph: PreparedEffectGraph,
        left: (PreparedEffectSchemeRef, PreparedEffectDelta),
        right: (PreparedEffectSchemeRef, PreparedEffectDelta),
        cancel: Option<Arc<AtomicBool>>,
    }

    impl ComparisonGraph {
        fn new(cancel: Option<Arc<AtomicBool>>) -> Self {
            let cancellation = AtomicBool::new(false);
            let mut resolver = ResolverWork::new(4_096);
            let mut work = resolver
                .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
                .unwrap();
            let mut graph = PreparedEffectGraph::default();
            let mut candidate = || {
                let mut checkpoint = graph.checkpoint(&mut work).unwrap();
                let scheme = graph
                    .declare(
                        vec![],
                        crate::effect_row::EffectPredicate::unconstrained(),
                        &mut work,
                    )
                    .unwrap();
                let recursive = graph.application(&scheme, vec![], &mut work).unwrap();
                graph
                    .prepare_body(&scheme, recursive, vec![], &mut work)
                    .unwrap();
                let delta = graph.extract_delta(&mut checkpoint, &mut work).unwrap();
                (scheme, delta)
            };
            let left = candidate();
            let right = candidate();
            Self {
                graph,
                left,
                right,
                cancel,
            }
        }

        fn compare(
            &self,
            work: &mut CandidateConstraintWorkSession<'_>,
        ) -> Result<bool, crate::callable::SealedBranchComparisonFailure<Domain>> {
            if let Some(cancellation) = &self.cancel {
                cancellation.store(true, Ordering::Release);
            }
            match self.graph.replay_delta_mismatch(
                &self.left.1,
                &self.right.1,
                [PreparedEffectReplayRoot::Scheme(
                    &self.left.0,
                    &self.right.0,
                )],
                work,
            ) {
                Ok(mismatch) => Ok(mismatch.is_none()),
                Err(PreparedEffectFailure::Control(error)) => Err(error.into()),
                Err(error) => panic!("valid recursive comparison fixture: {error:?}"),
            }
        }

        fn required_nodes(&self) -> u64 {
            let cancellation = AtomicBool::new(false);
            let mut resolver = ResolverWork::new(4_096);
            let mut work = resolver
                .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
                .unwrap();
            assert!(self.compare(&mut work).unwrap());
            drop(work);
            resolver.type_constraint_report().nodes
        }
    }

    struct Client {
        counts: Arc<Counts>,
        mode: CallbackMode,
    }

    impl TypeConstraintClient<Domain> for Client {
        type ProbeCheckpoint = u8;
        type MaterializationCheckpoint = u8;
        type PreparedSealedBranchValue = u8;

        fn probe_source(
            &mut self,
            _checkpoint: &mut Self::ProbeCheckpoint,
            probe: &mut CandidateConstraintSourceContext<'_, '_, Domain>,
        ) -> Result<SourceProbeOutcome<Domain>, SourceCallbackFailure<Domain>> {
            let source = probe.source();
            self.counts.probe_call.fetch_add(1, Ordering::Relaxed);
            self.counts
                .probe_reservation
                .store(std::ptr::from_mut(probe.work()) as usize, Ordering::Relaxed);
            if matches!(
                self.mode,
                CallbackMode::GroupRejectedThenSuccess
                    | CallbackMode::GroupRejectedThenFatal
                    | CallbackMode::GroupRejectedThenAbort
                    | CallbackMode::GroupRejectedThenInvariant
            ) {
                if source.local() == 1 {
                    return Ok(SourceProbeOutcome::Rejected("group head rejection"));
                }
                return match self.mode {
                    CallbackMode::GroupRejectedThenSuccess => {
                        probe.observe(SourceProbeResult::checked(TypeKind::I32, Branch, 0, ()))
                    }
                    CallbackMode::GroupRejectedThenFatal => Err(SourceCallbackFailure::fatal(
                        SourceError::new(source, SourcePhase::Probe, "group tail fatal"),
                    )),
                    CallbackMode::GroupRejectedThenAbort => {
                        Err(SourceCallbackFailure::Abort(TypeConstraintAbort::Cancelled))
                    }
                    CallbackMode::GroupRejectedThenInvariant => {
                        Err(SourceCallbackFailure::invariant(()))
                    }
                    _ => unreachable!("group callback mode was matched above"),
                };
            }
            match self.mode {
                CallbackMode::SourceAlternatives => {
                    let context = std::ptr::from_ref(&*probe.context);
                    let authority = std::ptr::from_ref(probe.authority);
                    let foreign_graph = crate::callable::PreparedCallGraph::<()>::new();
                    assert_eq!(
                        probe.validate_graph(&foreign_graph),
                        Err(CallConstraintInvariant::ForeignPreparedIssuer)
                    );
                    let first = probe.with_alternative(|alternative| {
                        assert_eq!(alternative.source(), source);
                        assert_eq!(std::ptr::from_ref(&*alternative.context), context);
                        assert_eq!(std::ptr::from_ref(alternative.authority), authority);
                        assert_eq!(
                            alternative.validate_graph(&foreign_graph),
                            Err(CallConstraintInvariant::ForeignPreparedIssuer)
                        );
                        alternative.observe(SourceProbeResult::checked(
                            TypeKind::I32,
                            Branch,
                            0,
                            (),
                        ))
                    })?;
                    let SourceProbeOutcome::Accepted(mut first) = first else {
                        unreachable!("observed branch")
                    };
                    let SourceProbeOutcome::Accepted(second) =
                        probe.observe(SourceProbeResult::checked(TypeKind::I32, Branch, 0, ()))?
                    else {
                        unreachable!("observed branch")
                    };
                    first
                        .append(second)
                        .map_err(SourceCallbackFailure::Constraint)?;
                    Ok(SourceProbeOutcome::Accepted(first))
                }
                CallbackMode::Success
                | CallbackMode::MaterializationRejected
                | CallbackMode::MaterializationWrongSource
                | CallbackMode::MaterializationWrongApplication
                | CallbackMode::MaterializationFatal
                | CallbackMode::MaterializationReversedFatal
                | CallbackMode::MaterializationAbort
                | CallbackMode::MaterializationInvariant
                | CallbackMode::OpenProbeFailure
                | CallbackMode::OpenMaterializationFailure
                | CallbackMode::CloseProbeClientFailure
                | CallbackMode::MissingProbePreparation
                | CallbackMode::CloseMaterializationClientFailure
                | CallbackMode::GroupRejectedThenSuccess
                | CallbackMode::GroupRejectedThenFatal
                | CallbackMode::GroupRejectedThenAbort
                | CallbackMode::GroupRejectedThenInvariant => {
                    probe.observe(SourceProbeResult::checked(TypeKind::I32, Branch, 0, ()))
                }
                CallbackMode::Rejected | CallbackMode::UnexpectedProbePreparation => {
                    Ok(SourceProbeOutcome::Rejected("rejected"))
                }
                CallbackMode::ProbeAbort => {
                    Err(SourceCallbackFailure::Abort(TypeConstraintAbort::Cancelled))
                }
                CallbackMode::ProbeInvariant => Err(SourceCallbackFailure::invariant(())),
                CallbackMode::WrongFatalSource => {
                    Err(SourceCallbackFailure::fatal(SourceError::new(
                        ConstraintSourceId::new(
                            source.application(),
                            source.local().saturating_add(1),
                        ),
                        SourcePhase::Probe,
                        "fatal",
                    )))
                }
                CallbackMode::WrongFatalApplication => {
                    Err(SourceCallbackFailure::fatal(SourceError::new(
                        crate::types::constraints::test_support::source_id(source.local()),
                        SourcePhase::Probe,
                        "another application's fatal",
                    )))
                }
                CallbackMode::Fatal => Err(SourceCallbackFailure::fatal(SourceError::new(
                    source,
                    SourcePhase::Probe,
                    "fatal",
                ))),
            }
        }

        fn open_probe_checkpoint(
            &mut self,
            work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
            _source: ConstraintSourceId<u8>,
        ) -> Result<Self::ProbeCheckpoint, SourceCheckpointFailure<Domain>> {
            self.counts.probe_begin.fetch_add(1, Ordering::Relaxed);
            if matches!(self.mode, CallbackMode::OpenProbeFailure) {
                Err(SourceCheckpointFailure::Protocol(
                    TypeConstraintSourceProtocolInvariant::Checkpoint,
                ))
            } else {
                Ok(1)
            }
        }

        fn close_probe_checkpoint(
            &mut self,
            work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
            _checkpoint: Self::ProbeCheckpoint,
            disposition: ProbeCheckpointDisposition,
        ) -> Result<Option<()>, SourceCheckpointFailure<Domain>> {
            self.counts.probe_close.fetch_add(1, Ordering::Relaxed);
            if matches!(self.mode, CallbackMode::CloseProbeClientFailure) {
                return Err(SourceCheckpointFailure::client(()));
            }
            if matches!(self.mode, CallbackMode::MissingProbePreparation) {
                return Ok(None);
            }
            if matches!(self.mode, CallbackMode::UnexpectedProbePreparation) {
                return Ok(Some(()));
            }
            Ok((disposition == ProbeCheckpointDisposition::Retain).then_some(()))
        }

        fn open_materialization_checkpoint(
            &mut self,
            work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
            sources: &[ConstraintSourceId<u8>],
        ) -> Result<Self::MaterializationCheckpoint, SourceCheckpointFailure<Domain>> {
            self.counts
                .materialize_begin
                .fetch_add(1, Ordering::Relaxed);
            if matches!(self.mode, CallbackMode::MaterializationReversedFatal) {
                assert_eq!(
                    sources
                        .iter()
                        .map(|source| source.local())
                        .collect::<Vec<_>>(),
                    [10, 20]
                );
            }
            if matches!(self.mode, CallbackMode::OpenMaterializationFailure) {
                Err(SourceCheckpointFailure::Protocol(
                    TypeConstraintSourceProtocolInvariant::Checkpoint,
                ))
            } else {
                Ok(2)
            }
        }

        fn materialize_sources<'h, I>(
            &mut self,
            sources: I,
            _checkpoint: &mut Self::MaterializationCheckpoint,
            _work: &mut CandidateConstraintWorkSession<'_>,
        ) -> Result<
            MaterializationOutcome<ConstraintSourceId<u8>, u8, &'static str>,
            SourceCallbackFailure<Domain>,
        >
        where
            I: IntoIterator<Item = MaterializedSourceRequest<'h, Domain>>,
            <Domain as ConstraintDomain>::CheckedEvidence: 'h,
            <Domain as ConstraintDomain>::ProbeSemanticBranch: 'h,
        {
            self.counts.materialize_call.fetch_add(1, Ordering::Relaxed);
            let requests = sources.into_iter().collect::<Vec<_>>();
            let source_id =
                |local| ConstraintSourceId::new(requests[0].source().application(), local);
            match self.mode {
                CallbackMode::Success | CallbackMode::SourceAlternatives => {
                    Ok(MaterializationOutcome::Sealed(3))
                }
                CallbackMode::Rejected | CallbackMode::MaterializationRejected => {
                    Ok(MaterializationOutcome::Rejected {
                        source: source_id(1),
                        cause: "materialized rejection",
                    })
                }
                CallbackMode::MaterializationWrongSource => Ok(MaterializationOutcome::Rejected {
                    source: source_id(9),
                    cause: "wrong source",
                }),
                CallbackMode::MaterializationWrongApplication => {
                    Ok(MaterializationOutcome::Rejected {
                        source: crate::types::constraints::test_support::source_id(
                            requests[0].source().local(),
                        ),
                        cause: "another application's rejection",
                    })
                }
                CallbackMode::Fatal | CallbackMode::MaterializationFatal => {
                    Err(SourceCallbackFailure::fatal(SourceError::new(
                        source_id(1),
                        SourcePhase::Materialize,
                        "materialized fatal",
                    )))
                }
                CallbackMode::MaterializationReversedFatal => {
                    assert_eq!(
                        requests
                            .iter()
                            .map(|request| request.source().local())
                            .collect::<Vec<_>>(),
                        vec![10, 20]
                    );
                    let call = self.counts.materialize_call.load(Ordering::Relaxed);
                    let (source, cause) = if call == 1 {
                        (20, "later source")
                    } else {
                        (10, "earlier source")
                    };
                    Err(SourceCallbackFailure::fatal(SourceError::new(
                        source_id(source),
                        SourcePhase::Materialize,
                        cause,
                    )))
                }
                CallbackMode::MaterializationAbort => {
                    Err(SourceCallbackFailure::Abort(TypeConstraintAbort::Cancelled))
                }
                CallbackMode::MaterializationInvariant => Err(SourceCallbackFailure::invariant(())),
                CallbackMode::OpenProbeFailure
                | CallbackMode::OpenMaterializationFailure
                | CallbackMode::CloseProbeClientFailure
                | CallbackMode::MissingProbePreparation
                | CallbackMode::UnexpectedProbePreparation
                | CallbackMode::CloseMaterializationClientFailure
                | CallbackMode::ProbeAbort
                | CallbackMode::ProbeInvariant
                | CallbackMode::WrongFatalSource
                | CallbackMode::WrongFatalApplication
                | CallbackMode::GroupRejectedThenSuccess
                | CallbackMode::GroupRejectedThenFatal
                | CallbackMode::GroupRejectedThenAbort
                | CallbackMode::GroupRejectedThenInvariant => Ok(MaterializationOutcome::Sealed(3)),
            }
        }

        fn close_materialization_checkpoint(
            &mut self,
            work: &mut crate::callable::CandidateConstraintWorkSession<'_>,
            _checkpoint: Self::MaterializationCheckpoint,
            sealed: Option<Self::PreparedSealedBranchValue>,
        ) -> Result<Option<Sealed>, SourceCheckpointFailure<Domain>> {
            self.counts
                .materialize_close
                .fetch_add(1, Ordering::Relaxed);
            if matches!(self.mode, CallbackMode::CloseMaterializationClientFailure) {
                return Err(SourceCheckpointFailure::client(()));
            }
            Ok(sealed.map(|_| Sealed))
        }

        fn sealed_branches_equal(
            &mut self,
            left: &Sealed,
            right: &Sealed,
            work: &mut CandidateConstraintWorkSession<'_>,
        ) -> Result<bool, crate::callable::SealedBranchComparisonFailure<Domain>> {
            assert_eq!(self.counts.callback_finish.load(Ordering::Relaxed), 1);
            self.counts.sealed_compare.fetch_add(1, Ordering::Relaxed);
            work.check_cancelled()?;
            self.counts
                .comparison_reservation
                .store(std::ptr::from_mut(work) as usize, Ordering::Relaxed);
            if let Some(graph) = &*self.counts.comparison_graph.lock().unwrap() {
                return graph.compare(work);
            }
            Ok(left == right)
        }

        fn finish(&mut self) -> Result<(), SourceCheckpointFailure<Domain>> {
            self.counts.callback_finish.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    fn prepared_source(source: u8) -> PreparedSourceConstraint<Domain> {
        PreparedSourceConstraint::checked(
            source,
            crate::types::constraints::PreparedConstraintSourceProjection::Scalar,
            [],
            PreparedSourceAlternative::new(0, (), TypeKind::I32),
        )
        .expect("one terminal alternative")
    }

    fn prepared() -> PreparedSourceConstraint<Domain> {
        prepared_source(1)
    }

    fn unscoped_parameter() -> GenericTypeParameterId {
        accepted_type(501, 0)
    }

    fn prepared_with_unscoped_hint() -> PreparedSourceConstraint<Domain> {
        PreparedSourceConstraint::checked(
            3,
            crate::types::constraints::PreparedConstraintSourceProjection::Scalar,
            [],
            PreparedSourceAlternative::new(
                0,
                (),
                TypeKind::generic_parameter(unscoped_parameter()),
            ),
        )
        .expect("one terminal alternative")
    }

    fn run_with_limits(
        mode: CallbackMode,
        counts: Arc<Counts>,
        limits: crate::callable::limits::CallableLimits,
    ) -> (
        Result<(), TypeConstraintFailure<Domain>>,
        TypeConstraintWorkReport,
    ) {
        run_with_control(mode, counts, limits, &AtomicBool::new(false))
    }

    fn run_with_control(
        mode: CallbackMode,
        counts: Arc<Counts>,
        limits: crate::callable::limits::CallableLimits,
        cancellation: &AtomicBool,
    ) -> (
        Result<(), TypeConstraintFailure<Domain>>,
        TypeConstraintWorkReport,
    ) {
        let mut work = ResolverWork::new(4_096);
        let session = work
            .begin_candidate_constraint_session(limits, cancellation)
            .expect("candidate session");
        let result = session
            .with_fixture_driver::<Domain, _, _>(
                0,
                initialization(TypeConstraintParameterScope::empty()),
                Client { counts, mode },
                |mut driver| {
                    let result = match driver.probe_prepared_source(
                        prepared(),
                        ConstraintAcceptance::PatternAcceptsActual,
                    ) {
                        Ok(()) => driver.finish().map(|_| ()),
                        Err(error) => {
                            let _ = driver.finish();
                            Err(error)
                        }
                    };

                    result
                },
            )
            .expect("prepared initialization");
        let report = work.type_constraint_report().clone();
        (result, report)
    }

    fn run(mode: CallbackMode, counts: Arc<Counts>) -> Result<(), TypeConstraintFailure<Domain>> {
        run_with_limits(mode, counts, PRODUCTION_CALLABLE_LIMITS).0
    }

    #[test]
    fn effect_access_borrows_the_original_graph_and_rejects_a_foreign_graph_before_work() {
        let cancellation = AtomicBool::new(false);
        let mut resolver = ResolverWork::new(4_096);
        let mut work = resolver
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .unwrap();
        let mut graph = crate::callable::PreparedCallGraph::<()>::new();
        let declarations = crate::callable::PreparedCallableEffectDeclarations::declare(
            std::iter::empty::<(
                &crate::callable::CheckedCallableId,
                &crate::callable::CallableSignatureSchema,
            )>(),
            &mut work,
        )
        .unwrap();
        graph.initialize_effect_declarations(declarations).unwrap();
        let (authority, _, _, _) =
            initialization_from_graph(&graph, TypeConstraintParameterScope::empty())
                .into_lower_parts()
                .unwrap();
        let mut foreign = crate::callable::PreparedCallGraph::<()>::new();
        let mut entered = false;
        let rejected = (CandidateConstraintEffectAccess {
            authority: &authority,
        })
        .with_graph(&mut foreign, |_| entered = true);
        assert_eq!(
            rejected,
            Err(CallConstraintInvariant::ForeignPreparedIssuer)
        );
        assert!(!entered);

        let row = (CandidateConstraintEffectAccess {
            authority: &authority,
        })
        .with_graph(&mut graph, |relation| {
            let scheme = relation
                .declare(
                    vec![],
                    crate::effect_row::EffectPredicate::unconstrained(),
                    &mut work,
                )
                .unwrap();
            relation.application(&scheme, vec![], &mut work).unwrap()
        })
        .unwrap();
        graph
            .effect_declarations_mut()
            .unwrap()
            .relation()
            .0
            .validate_retained_row(&row)
            .unwrap();
        assert!(matches!(
            row,
            crate::effect_row::prepared::PreparedEffectRow::Term(_)
        ));
        drop(work);
        assert!(resolver.type_constraint_report().nodes > 0);
        assert_eq!(resolver.type_constraint_report().source_probes, 0);
        assert_eq!(resolver.type_constraint_report().materializations, 0);
    }

    #[test]
    fn driver_retains_the_initialized_graph_authority_through_source_work() {
        let graph = crate::callable::PreparedCallGraph::<()>::new();
        let foreign = crate::callable::PreparedCallGraph::<()>::new();
        let initialization =
            initialization_from_graph(&graph, TypeConstraintParameterScope::empty());
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut work = ResolverWork::new(4_096);
        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .unwrap();
        let counts = Arc::new(Counts::default());
        let result = session
            .with_fixture_driver::<Domain, _, _>(
                0,
                initialization,
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::SourceAlternatives,
                },
                |mut driver| {
                    assert_eq!(
                        graph.validate_constraint_authority(&driver.callbacks.authority),
                        Ok(())
                    );
                    assert_eq!(
                        foreign.validate_constraint_authority(&driver.callbacks.authority),
                        Err(CallConstraintInvariant::ForeignPreparedIssuer)
                    );
                    driver.probe_prepared_source(
                        prepared_source(1),
                        ConstraintAcceptance::PatternAcceptsActual,
                    )?;
                    assert_eq!(
                        graph.validate_constraint_authority(&driver.callbacks.authority),
                        Ok(())
                    );
                    driver.finish()
                },
            )
            .expect("prepared initialization");
        assert!(
            result.is_ok(),
            "same graph remains valid after source alternatives: {result:?}"
        );
        assert_eq!(counts.probe_begin.load(Ordering::Relaxed), 1);
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 2);
        assert_eq!(work.type_constraint_report().branches, 2);
    }

    #[test]
    fn source_alternatives_share_context_and_close_one_probe_checkpoint() {
        let counts = Arc::new(Counts::default());
        let (result, work) = run_with_limits(
            CallbackMode::SourceAlternatives,
            Arc::clone(&counts),
            PRODUCTION_CALLABLE_LIMITS,
        );
        assert!(
            result.is_ok(),
            "equivalent observations coalesce after completion: {result:?}"
        );
        assert_eq!(counts.callback_finish.load(Ordering::Relaxed), 1);
        assert_eq!(counts.sealed_compare.load(Ordering::Relaxed), 1);
        assert_eq!(work.branches, 2);
        assert_eq!(counts.probe_begin.load(Ordering::Relaxed), 1);
        assert_eq!(counts.probe_call.load(Ordering::Relaxed), 1);
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 2);
        assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 2);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn sealed_comparison_borrows_the_source_reservation_for_recursive_effect_replay() {
        let (baseline, baseline_report) = run_with_limits(
            CallbackMode::SourceAlternatives,
            Arc::new(Counts::default()),
            PRODUCTION_CALLABLE_LIMITS,
        );
        baseline.unwrap();
        let graph = ComparisonGraph::new(None);
        let nodes = graph.required_nodes();
        assert!(nodes > 0);
        let counts = Arc::new(Counts {
            comparison_graph: Mutex::new(Some(graph)),
            ..Counts::default()
        });
        let (result, report) = run_with_limits(
            CallbackMode::SourceAlternatives,
            Arc::clone(&counts),
            PRODUCTION_CALLABLE_LIMITS,
        );
        result.unwrap();
        let source_reservation = counts.probe_reservation.load(Ordering::Relaxed);
        assert_ne!(source_reservation, 0);
        assert_eq!(
            source_reservation,
            counts.comparison_reservation.load(Ordering::Relaxed)
        );
        assert_eq!(report.nodes, baseline_report.nodes + nodes);
        assert_eq!(report.work(), baseline_report.work() + nodes);
        assert_eq!(report.source_probes(), baseline_report.source_probes());
        assert_eq!(
            report.materializations(),
            baseline_report.materializations()
        );
        assert_eq!(counts.callback_finish.load(Ordering::Relaxed), 1);
        assert_eq!(counts.sealed_compare.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn every_effect_comparison_node_boundary_preserves_cleanup_and_typed_abort() {
        let (baseline, baseline_report) = run_with_limits(
            CallbackMode::SourceAlternatives,
            Arc::new(Counts::default()),
            PRODUCTION_CALLABLE_LIMITS,
        );
        baseline.unwrap();
        let required = ComparisonGraph::new(None).required_nodes();
        for limit in baseline_report.nodes..baseline_report.nodes + required {
            let counts = Arc::new(Counts {
                comparison_graph: Mutex::new(Some(ComparisonGraph::new(None))),
                ..Counts::default()
            });
            let (result, report) = run_with_limits(
                CallbackMode::SourceAlternatives,
                Arc::clone(&counts),
                PRODUCTION_CALLABLE_LIMITS.with_type_constraint_limits(256, limit, 128),
            );
            assert!(
                matches!(
                    result,
                    Err(TypeConstraintFailure::Abort(TypeConstraintAbort::NodeLimit {
                        actual,
                        limit: reported,
                    })) if actual == limit + 1 && reported == limit
                ),
                "comparison limit {limit}: {result:?}"
            );
            assert_eq!(report.nodes, limit);
            assert_eq!(counts.sealed_compare.load(Ordering::Relaxed), 1);
            assert_eq!(counts.callback_finish.load(Ordering::Relaxed), 1);
            assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 2);
            assert_eq!(
                counts
                    .comparison_graph
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .required_nodes(),
                required
            );
        }
    }

    #[test]
    fn cancellation_inside_effect_comparison_uses_the_original_signal_after_cleanup() {
        let cancellation = Arc::new(AtomicBool::new(false));
        let counts = Arc::new(Counts {
            comparison_graph: Mutex::new(Some(ComparisonGraph::new(Some(Arc::clone(
                &cancellation,
            ))))),
            ..Counts::default()
        });
        let (result, report) = run_with_control(
            CallbackMode::SourceAlternatives,
            Arc::clone(&counts),
            PRODUCTION_CALLABLE_LIMITS,
            &cancellation,
        );
        assert!(matches!(
            result,
            Err(TypeConstraintFailure::Abort(TypeConstraintAbort::Cancelled))
        ));
        assert_eq!(report.source_probes(), 1);
        assert_eq!(report.materializations(), 2);
        assert_eq!(counts.sealed_compare.load(Ordering::Relaxed), 1);
        assert_eq!(counts.callback_finish.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 2);
        cancellation.store(false, Ordering::Release);
        let mut graph = counts.comparison_graph.lock().unwrap();
        let graph = graph.as_mut().unwrap();
        graph.cancel = None;
        assert!(
            graph.required_nodes() > 0,
            "comparison retained both affine deltas"
        );
    }

    fn run_group(
        mode: CallbackMode,
        counts: Arc<Counts>,
    ) -> Result<(), TypeConstraintFailure<Domain>> {
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut work = ResolverWork::new(4_096);
        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .expect("candidate session");
        session
            .with_fixture_driver::<Domain, _, _>(
                0,
                initialization(TypeConstraintParameterScope::empty()),
                Client { counts, mode },
                |mut driver| {
                    let group = PreparedSourceConstraintGroup::seal([
                        prepared_source(1),
                        prepared_source(2),
                    ])
                    .expect("two-source mapper group");
                    driver.probe_source_group(group, ConstraintAcceptance::PatternAcceptsActual)?;

                    driver.finish().map(|_| ())
                },
            )
            .expect("prepared initialization")
    }

    #[test]
    fn mapper_source_group_finishes_real_tail_callbacks_without_resurrecting_rejection() {
        let counts = Arc::new(Counts::default());
        let result = run_group(CallbackMode::GroupRejectedThenSuccess, Arc::clone(&counts));
        assert!(matches!(result, Err(TypeConstraintFailure::Rejected(_))));
        assert_eq!(counts.probe_begin.load(Ordering::Relaxed), 2);
        assert_eq!(counts.probe_call.load(Ordering::Relaxed), 2);
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 2);
        assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn mapper_source_group_tail_terminal_failures_outrank_deferred_rejection() {
        let cases = [
            CallbackMode::GroupRejectedThenFatal,
            CallbackMode::GroupRejectedThenAbort,
            CallbackMode::GroupRejectedThenInvariant,
        ];
        for mode in cases {
            let counts = Arc::new(Counts::default());
            let result = run_group(mode, Arc::clone(&counts));
            match mode {
                CallbackMode::GroupRejectedThenFatal => assert!(matches!(
                    result,
                    Err(TypeConstraintFailure::FatalSource(ref error)) if error.source().local() == 2
                )),
                CallbackMode::GroupRejectedThenAbort => assert!(matches!(
                    result,
                    Err(TypeConstraintFailure::Abort(TypeConstraintAbort::Cancelled))
                )),
                CallbackMode::GroupRejectedThenInvariant => {
                    assert!(matches!(result, Err(TypeConstraintFailure::Invariant(_))))
                }
                _ => unreachable!("the test table contains only terminal tail modes"),
            }
            assert_eq!(counts.probe_begin.load(Ordering::Relaxed), 2);
            assert_eq!(counts.probe_call.load(Ordering::Relaxed), 2);
            assert_eq!(counts.probe_close.load(Ordering::Relaxed), 2);
            assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 0);
        }
    }

    #[test]
    fn hint_projection_failure_charges_no_callback_or_source_probe() {
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut work = ResolverWork::new(4_096);
        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .expect("candidate session");
        let counts = Arc::new(Counts::default());
        let failure = session
            .with_fixture_driver::<Domain, _, _>(
                0,
                initialization(TypeConstraintParameterScope::empty()),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
                |mut driver| {
                    let failure = driver
                        .probe_prepared_source(
                            prepared_with_unscoped_hint(),
                            ConstraintAcceptance::PatternAcceptsActual,
                        )
                        .expect_err("unscoped hint is rejected before callback");
                    let _ = driver.finish();
                    failure
                },
            )
            .expect("prepared initialization");
        assert!(matches!(
            failure,
            TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Constraint(
                TypeConstraintInvariant::ParameterScope(_)
            ))
        ));
        assert_eq!(counts.probe_begin.load(Ordering::Relaxed), 0);
        assert_eq!(counts.probe_call.load(Ordering::Relaxed), 0);
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 0);
        assert_eq!(work.type_constraint_report().source_probes(), 0);
    }

    #[test]
    fn cancelled_and_seed_limited_initialization_abort_before_callbacks() {
        let cancellation = std::sync::atomic::AtomicBool::new(true);
        let mut work = ResolverWork::new(4_096);
        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .expect("candidate session");
        let counts = Arc::new(Counts::default());
        let result = session.with_fixture_driver::<Domain, _, _>(
            0,
            initialization(TypeConstraintParameterScope::empty()),
            Client {
                counts: Arc::clone(&counts),
                mode: CallbackMode::Success,
            },
            |_driver| (),
        );
        assert!(matches!(
            result,
            Err(CandidateConstraintDriverStartFailure::Lower(
                TypeConstraintInitializationFailure::Abort(TypeConstraintAbort::Cancelled)
            ))
        ));
        drop(result);
        assert_eq!(counts.probe_begin.load(Ordering::Relaxed), 0);
        assert_eq!(counts.probe_call.load(Ordering::Relaxed), 0);
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 0);
        assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 0);
        assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 0);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 0);
        assert_eq!(work.type_constraint_report().work(), 0);

        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let limits = PRODUCTION_CALLABLE_LIMITS.with_type_constraint_limits(0, 64, 64);
        let mut work = ResolverWork::new(4_096);
        let session = work
            .begin_candidate_constraint_session(limits, &cancellation)
            .expect("candidate session");
        let counts = Arc::new(Counts::default());
        let result = session.with_fixture_driver::<Domain, _, _>(
            0,
            initialization(TypeConstraintParameterScope::empty()),
            Client {
                counts: Arc::clone(&counts),
                mode: CallbackMode::Success,
            },
            |_driver| (),
        );
        assert!(matches!(
            result,
            Err(CandidateConstraintDriverStartFailure::Lower(
                TypeConstraintInitializationFailure::Abort(TypeConstraintAbort::BranchLimit {
                    actual: 1,
                    limit: 0,
                })
            ))
        ));
        drop(result);
        assert_eq!(counts.probe_begin.load(Ordering::Relaxed), 0);
        assert_eq!(counts.probe_call.load(Ordering::Relaxed), 0);
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 0);
        assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 0);
        assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 0);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 0);
        // The accepted source context is admitted before the branch budget.
        assert_eq!(work.type_constraint_report().work(), 1);
    }

    #[test]
    fn a_root_rejects_an_original_context_from_another_source_before_callbacks() {
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut work = ResolverWork::new(4_096);
        let original = {
            let session = work
                .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
                .unwrap();
            let mut context = TypeConstraintContext::<_, Domain>::with_accounting(session);
            context.fixture_original_context(None).unwrap()
        };
        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .unwrap();
        let counts = Arc::new(Counts::default());
        let result = session.with_driver::<Domain, _, _>(
            original,
            0,
            initialization(TypeConstraintParameterScope::empty()),
            Client {
                counts: Arc::clone(&counts),
                mode: CallbackMode::Success,
            },
            |_driver| panic!("a foreign source must not start the driver"),
        );
        assert!(matches!(
            result,
            Err(CandidateConstraintDriverStartFailure::Lower(
                TypeConstraintInitializationFailure::Invariant(
                    TypeConstraintInvariant::OriginalProducer
                )
            ))
        ));
        assert_eq!(counts.probe_begin.load(Ordering::Relaxed), 0);
        assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn materialization_limit_charges_first_callback_then_aborts_before_second_open() {
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let limits = PRODUCTION_CALLABLE_LIMITS.with_type_constraint_source_limits(64, 1);
        let mut work = ResolverWork::new(4_096);
        let session = work
            .begin_candidate_constraint_session(limits, &cancellation)
            .expect("candidate session");
        let counts = Arc::new(Counts::default());
        let first = accepted_type(502, 0);
        let second = accepted_type(502, 1);
        let scope = TypeConstraintParameterScope::new([
            (
                first.clone(),
                crate::types::constraints::TypeConstraintParameterEligibility::Bindable,
            ),
            (
                second.clone(),
                crate::types::constraints::TypeConstraintParameterEligibility::Bindable,
            ),
        ])
        .expect("choice scope");
        let result = session
            .with_fixture_driver::<Domain, _, _>(
                0,
                initialization(scope),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
                |mut driver| {
                    driver.constrain(
                        &TypeKind::Choice(vec![
                            TypeKind::Tuple(vec![
                                TypeKind::generic_parameter(first.clone()),
                                TypeKind::generic_parameter(second.clone()),
                            ]),
                            TypeKind::Tuple(vec![
                                TypeKind::generic_parameter(second),
                                TypeKind::generic_parameter(first),
                            ]),
                        ]),
                        &TypeKind::Tuple(vec![TypeKind::I32, TypeKind::String]),
                        ConstraintAcceptance::PatternAcceptsActual,
                    );
                    driver
                        .probe_prepared_source(
                            prepared_source(1),
                            ConstraintAcceptance::PatternAcceptsActual,
                        )
                        .expect("first source probe");
                    driver
                        .probe_prepared_source(
                            prepared_source(2),
                            ConstraintAcceptance::PatternAcceptsActual,
                        )
                        .expect("second source probe");

                    driver.finish()
                },
            )
            .expect("prepared initialization");
        assert!(
            matches!(
                result,
                Err(TypeConstraintFailure::Abort(
                    TypeConstraintAbort::MaterializationLimit {
                        actual: 2,
                        limit: 1,
                    }
                ))
            ),
            "unexpected result: {result:?}, report: {:?}, materialize begin/call/close = {}/{}/{}",
            work.type_constraint_report(),
            counts.materialize_begin.load(Ordering::Relaxed),
            counts.materialize_call.load(Ordering::Relaxed),
            counts.materialize_close.load(Ordering::Relaxed),
        );
        assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 1);
        assert_eq!(work.type_constraint_report().materializations(), 1);
    }

    #[test]
    fn cancelled_materialization_mint_leaves_ready_ticket_for_retry() {
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut work = ResolverWork::new(4_096);
        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .expect("candidate session");
        let counts = Arc::new(Counts::default());
        let result = session
            .with_fixture_driver::<Domain, _, _>(
                0,
                initialization(TypeConstraintParameterScope::empty()),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
                |mut driver| {
                    driver
                        .probe_prepared_source(
                            prepared(),
                            ConstraintAcceptance::PatternAcceptsActual,
                        )
                        .expect("prepared source probe");
                    let mut lower_ticket = driver
                        .lower
                        .next_materialization_ticket(driver.callbacks.context)
                        .expect("lower materialization ticket")
                        .expect("one materialization ticket");

                    cancellation.store(true, std::sync::atomic::Ordering::Release);
                    assert!(matches!(
                        driver.callbacks.begin_materialization_callback(
                            driver.lower.materialization_owner(),
                            &mut lower_ticket
                        ),
                        Err(MaterializationImmediateFailure::Abort(
                            TypeConstraintAbort::Cancelled
                        ))
                    ));
                    assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 0);
                    assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 0);
                    assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 0);

                    cancellation.store(false, std::sync::atomic::Ordering::Release);
                    let (callback_ticket, mut checkpoint) = driver
                        .callbacks
                        .begin_materialization_callback(
                            driver.lower.materialization_owner(),
                            &mut lower_ticket,
                        )
                        .expect("the cancelled ticket remains ready");
                    let attempt = driver.callbacks.with_callback(|client, work| {
                        client.materialize_sources(
                            lower_ticket.requests(),
                            &mut checkpoint.checkpoint,
                            work,
                        )
                    });
                    let closed = driver
                        .callbacks
                        .close_materialization_callback(
                            &mut lower_ticket,
                            callback_ticket,
                            checkpoint,
                            attempt,
                        )
                        .expect("retry closes the callback");
                    driver
                        .lower
                        .submit_closed_materialization(lower_ticket, closed)
                        .expect("retry submits the closed materialization");
                    assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 1);
                    assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 1);
                    assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 1);

                    driver.finish()
                },
            )
            .expect("prepared initialization");
        assert!(
            result.is_ok(),
            "successful retry completes the candidate: {result:?}"
        );
        assert_eq!(work.type_constraint_report().materializations(), 1);
    }

    #[test]
    fn driver_retains_reversed_materialization_fatals_and_chooses_earliest_source() {
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut work = ResolverWork::new(4_096);
        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .expect("candidate session");
        let counts = Arc::new(Counts::default());
        let first = accepted_type(503, 0);
        let second = accepted_type(503, 1);
        let scope = TypeConstraintParameterScope::new([
            (
                first.clone(),
                crate::types::constraints::TypeConstraintParameterEligibility::Bindable,
            ),
            (
                second.clone(),
                crate::types::constraints::TypeConstraintParameterEligibility::Bindable,
            ),
        ])
        .expect("choice scope");
        let result = session
            .with_fixture_driver::<Domain, _, _>(
                0,
                initialization(scope),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::MaterializationReversedFatal,
                },
                |mut driver| {
                    driver.constrain(
                        &TypeKind::Choice(vec![
                            TypeKind::Tuple(vec![
                                TypeKind::generic_parameter(first.clone()),
                                TypeKind::generic_parameter(second.clone()),
                            ]),
                            TypeKind::Tuple(vec![
                                TypeKind::generic_parameter(second),
                                TypeKind::generic_parameter(first),
                            ]),
                        ]),
                        &TypeKind::Tuple(vec![TypeKind::I32, TypeKind::String]),
                        ConstraintAcceptance::PatternAcceptsActual,
                    );
                    driver
                        .probe_prepared_source(
                            prepared_source(10),
                            ConstraintAcceptance::PatternAcceptsActual,
                        )
                        .expect("first source probe");
                    driver
                        .probe_prepared_source(
                            prepared_source(20),
                            ConstraintAcceptance::PatternAcceptsActual,
                        )
                        .expect("second source probe");

                    driver.finish()
                },
            )
            .expect("prepared initialization");
        match result {
            Err(TypeConstraintFailure::FatalSource(error)) => {
                assert_eq!(error.source().local(), 10);
                assert_eq!(error.cause(), &"earlier source");
            }
            other => panic!("expected earliest authored fatal source, got {other:?}"),
        }
        assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 2);
        assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 2);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 2);
        assert_eq!(work.type_constraint_report().materializations(), 2);
    }

    #[test]
    fn affine_driver_closes_probe_and_materialization_once_on_success() {
        let counts = Arc::new(Counts::default());
        run(CallbackMode::Success, Arc::clone(&counts)).expect("successful candidate");
        assert_eq!(counts.probe_begin.load(Ordering::Relaxed), 1);
        assert_eq!(counts.probe_call.load(Ordering::Relaxed), 1);
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn affine_driver_closes_probe_once_on_rejection_and_fatal() {
        for mode in [CallbackMode::Rejected, CallbackMode::Fatal] {
            let counts = Arc::new(Counts::default());
            assert!(run(mode, Arc::clone(&counts)).is_err());
            assert_eq!(counts.probe_begin.load(Ordering::Relaxed), 1);
            assert_eq!(counts.probe_call.load(Ordering::Relaxed), 1);
            assert_eq!(counts.probe_close.load(Ordering::Relaxed), 1);
            assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 0);
            assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 0);
        }
    }

    #[test]
    fn probe_close_must_return_preparation_exactly_for_an_accepted_outcome() {
        for mode in [
            CallbackMode::MissingProbePreparation,
            CallbackMode::UnexpectedProbePreparation,
        ] {
            let counts = Arc::new(Counts::default());
            assert!(matches!(
                run(mode, Arc::clone(&counts)),
                Err(TypeConstraintFailure::Invariant(
                    TypeConstraintFailureInvariant::Constraint(
                        TypeConstraintInvariant::SourceProtocol(
                            TypeConstraintSourceProtocolInvariant::Outcome
                        )
                    )
                ))
            ));
            assert_eq!(counts.probe_close.load(Ordering::Relaxed), 1);
            assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 0);
        }
    }

    #[test]
    fn affine_driver_closes_materialization_once_on_rejection_and_fatal() {
        for mode in [
            CallbackMode::MaterializationRejected,
            CallbackMode::MaterializationFatal,
        ] {
            let counts = Arc::new(Counts::default());
            assert!(run(mode, Arc::clone(&counts)).is_err());
            assert_eq!(counts.probe_begin.load(Ordering::Relaxed), 1);
            assert_eq!(counts.probe_call.load(Ordering::Relaxed), 1);
            assert_eq!(counts.probe_close.load(Ordering::Relaxed), 1);
            assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 1);
            assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 1);
            assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 1);
        }
    }

    #[test]
    fn callback_abort_client_invariant_and_wrong_fatal_are_typed_without_reclassification() {
        let counts = Arc::new(Counts::default());
        let (result, report) = run_with_limits(
            CallbackMode::ProbeAbort,
            Arc::clone(&counts),
            PRODUCTION_CALLABLE_LIMITS,
        );
        assert!(matches!(
            result,
            Err(TypeConstraintFailure::Abort(TypeConstraintAbort::Cancelled))
        ));
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 1);
        assert_eq!(report.source_probes(), 1);

        let counts = Arc::new(Counts::default());
        let (result, report) = run_with_limits(
            CallbackMode::ProbeInvariant,
            Arc::clone(&counts),
            PRODUCTION_CALLABLE_LIMITS,
        );
        assert!(matches!(
            result,
            Err(TypeConstraintFailure::Invariant(
                TypeConstraintFailureInvariant::Client(_)
            ))
        ));
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 1);
        assert_eq!(report.source_probes(), 1);

        let counts = Arc::new(Counts::default());
        let (result, report) = run_with_limits(
            CallbackMode::WrongFatalSource,
            Arc::clone(&counts),
            PRODUCTION_CALLABLE_LIMITS,
        );
        assert!(matches!(
            result,
            Err(TypeConstraintFailure::Invariant(
                TypeConstraintFailureInvariant::Constraint(
                    TypeConstraintInvariant::SourceProtocol(
                        TypeConstraintSourceProtocolInvariant::WrongSource
                    )
                )
            ))
        ));
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 1);
        assert_eq!(report.source_probes(), 1);
    }

    #[test]
    fn callback_source_from_another_application_cannot_authorize_a_submission() {
        for mode in [
            CallbackMode::WrongFatalApplication,
            CallbackMode::MaterializationWrongApplication,
        ] {
            let counts = Arc::new(Counts::default());
            let (result, _) =
                run_with_limits(mode, Arc::clone(&counts), PRODUCTION_CALLABLE_LIMITS);
            assert!(matches!(
                result,
                Err(TypeConstraintFailure::Invariant(
                    TypeConstraintFailureInvariant::Constraint(
                        TypeConstraintInvariant::SourceProtocol(
                            TypeConstraintSourceProtocolInvariant::WrongSource
                        )
                    )
                ))
            ));
            assert_eq!(counts.probe_close.load(Ordering::Relaxed), 1);
            let expected_materialization_closes = usize::from(matches!(
                mode,
                CallbackMode::MaterializationWrongApplication
            ));
            assert_eq!(
                counts.materialize_close.load(Ordering::Relaxed),
                expected_materialization_closes
            );
        }
    }

    #[test]
    fn materialization_abort_and_client_invariant_close_once_and_preserve_payload() {
        let counts = Arc::new(Counts::default());
        let (result, report) = run_with_limits(
            CallbackMode::MaterializationAbort,
            Arc::clone(&counts),
            PRODUCTION_CALLABLE_LIMITS,
        );
        assert!(matches!(
            result,
            Err(TypeConstraintFailure::Abort(TypeConstraintAbort::Cancelled))
        ));
        assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 1);
        assert_eq!(report.materializations(), 1);

        let counts = Arc::new(Counts::default());
        let (result, report) = run_with_limits(
            CallbackMode::MaterializationInvariant,
            Arc::clone(&counts),
            PRODUCTION_CALLABLE_LIMITS,
        );
        assert!(matches!(
            result,
            Err(TypeConstraintFailure::Invariant(
                TypeConstraintFailureInvariant::Client(_)
            ))
        ));
        assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 1);
        assert_eq!(report.materializations(), 1);

        let counts = Arc::new(Counts::default());
        let (result, report) = run_with_limits(
            CallbackMode::MaterializationWrongSource,
            Arc::clone(&counts),
            PRODUCTION_CALLABLE_LIMITS,
        );
        assert!(matches!(
            result,
            Err(TypeConstraintFailure::Invariant(
                TypeConstraintFailureInvariant::Constraint(
                    TypeConstraintInvariant::SourceProtocol(
                        TypeConstraintSourceProtocolInvariant::WrongSource
                    )
                )
            ))
        ));
        assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 1);
        assert_eq!(report.materializations(), 1);
    }

    #[test]
    fn checkpoint_open_failure_precedes_callback_and_close() {
        let counts = Arc::new(Counts::default());
        let (result, report) = run_with_limits(
            CallbackMode::OpenProbeFailure,
            Arc::clone(&counts),
            PRODUCTION_CALLABLE_LIMITS,
        );
        assert!(matches!(
            result,
            Err(TypeConstraintFailure::Invariant(
                TypeConstraintFailureInvariant::Constraint(
                    TypeConstraintInvariant::SourceProtocol(
                        TypeConstraintSourceProtocolInvariant::Checkpoint
                    )
                )
            ))
        ));
        assert_eq!(counts.probe_begin.load(Ordering::Relaxed), 1);
        assert_eq!(counts.probe_call.load(Ordering::Relaxed), 0);
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 0);
        assert_eq!(report.source_probes(), 1);

        let counts = Arc::new(Counts::default());
        let (result, report) = run_with_limits(
            CallbackMode::OpenMaterializationFailure,
            Arc::clone(&counts),
            PRODUCTION_CALLABLE_LIMITS,
        );
        assert!(matches!(
            result,
            Err(TypeConstraintFailure::Invariant(
                TypeConstraintFailureInvariant::Constraint(
                    TypeConstraintInvariant::SourceProtocol(
                        TypeConstraintSourceProtocolInvariant::Checkpoint
                    )
                )
            ))
        ));
        assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 0);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 0);
        assert_eq!(report.materializations(), 1);
    }

    #[test]
    fn foreign_ticket_and_checkpoint_close_once_and_clear_active_state() {
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut work = ResolverWork::new(4_096);
        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .expect("candidate session");
        let counts = Arc::new(Counts::default());
        session
            .with_fixture_driver::<Domain, _, _>(
                0,
                initialization(TypeConstraintParameterScope::empty()),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
                |mut driver| {
                    let (ticket, checkpoint) = driver
                        .callbacks
                        .begin_probe_callback(crate::types::constraints::test_support::source_id(1))
                        .expect("driver mints a probe ticket");
                    let foreign_ticket = SourceCallbackTicket {
                        identity: SourceCallbackTicketIdentity {
                            issuer: Arc::new(SourceCallbackTicketIssuer),
                            ordinal: 0,
                        },
                        authority: SourceCallbackAuthority::Probe {
                            source: crate::types::constraints::test_support::source_id(1),
                        },
                    };
                    assert!(matches!(
                        driver.callbacks.close_probe_callback(
                            foreign_ticket,
                            checkpoint,
                            Ok(SourceProbeOutcome::Rejected("protocol")),
                        ),
                        Err(TypeConstraintFailure::Invariant(
                            TypeConstraintFailureInvariant::Constraint(
                                TypeConstraintInvariant::SourceProtocol(
                                    TypeConstraintSourceProtocolInvariant::Ticket
                                )
                            )
                        ))
                    ));
                    assert_eq!(counts.probe_close.load(Ordering::Relaxed), 1);

                    let (valid_ticket, valid_checkpoint) = driver
                        .callbacks
                        .begin_probe_callback(crate::types::constraints::test_support::source_id(1))
                        .expect("foreign close clears the active ticket");
                    driver
                        .callbacks
                        .close_probe_callback(
                            valid_ticket,
                            valid_checkpoint,
                            Ok(SourceProbeOutcome::Rejected("protocol")),
                        )
                        .expect("valid callback can begin after foreign close");
                    assert_eq!(counts.probe_close.load(Ordering::Relaxed), 2);
                    drop(ticket);

                    drop(driver);
                },
            )
            .expect("prepared initialization");

        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .expect("second candidate session");
        let counts = Arc::new(Counts::default());
        session
            .with_fixture_driver::<Domain, _, _>(
                0,
                initialization(TypeConstraintParameterScope::empty()),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
                |mut driver| {
                    let (ticket, mut checkpoint) = driver
                        .callbacks
                        .begin_probe_callback(crate::types::constraints::test_support::source_id(1))
                        .expect("driver can mint the next generation");
                    checkpoint.identity.issuer = Arc::new(SourceCallbackTicketIssuer);
                    assert!(matches!(
                        driver.callbacks.close_probe_callback(
                            ticket,
                            checkpoint,
                            Ok(SourceProbeOutcome::Rejected("protocol")),
                        ),
                        Err(TypeConstraintFailure::Invariant(
                            TypeConstraintFailureInvariant::Constraint(
                                TypeConstraintInvariant::SourceProtocol(
                                    TypeConstraintSourceProtocolInvariant::Checkpoint
                                )
                            )
                        ))
                    ));
                    assert_eq!(counts.probe_close.load(Ordering::Relaxed), 1);

                    let (valid_ticket, valid_checkpoint) = driver
                        .callbacks
                        .begin_probe_callback(crate::types::constraints::test_support::source_id(1))
                        .expect("foreign checkpoint close clears the active ticket");
                    driver
                        .callbacks
                        .close_probe_callback(
                            valid_ticket,
                            valid_checkpoint,
                            Ok(SourceProbeOutcome::Rejected("protocol")),
                        )
                        .expect("valid callback can begin after foreign checkpoint close");
                    assert_eq!(counts.probe_close.load(Ordering::Relaxed), 2);
                },
            )
            .expect("second prepared initialization");
    }

    #[test]
    fn foreign_materialization_authority_closes_once_and_clears_active_state() {
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut work = ResolverWork::new(4_096);
        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .expect("candidate session");
        let counts = Arc::new(Counts::default());
        session
            .with_fixture_driver::<Domain, _, _>(
                0,
                initialization(TypeConstraintParameterScope::empty()),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
                |mut driver| {
                    driver
                        .probe_prepared_source(
                            prepared(),
                            ConstraintAcceptance::PatternAcceptsActual,
                        )
                        .expect("prepared source probe");
                    let mut materialization = driver
                        .lower
                        .next_materialization_ticket(driver.callbacks.context)
                        .expect("lower materialization ticket")
                        .expect("one materialization ticket");
                    let (_valid_ticket, checkpoint) = driver
                        .callbacks
                        .begin_materialization_callback(
                            driver.lower.materialization_owner(),
                            &mut materialization,
                        )
                        .expect("driver mints a materialization ticket");
                    let foreign_ticket = SourceCallbackTicket {
                        identity: SourceCallbackTicketIdentity {
                            issuer: Arc::new(SourceCallbackTicketIssuer),
                            ordinal: 0,
                        },
                        authority: SourceCallbackAuthority::Probe {
                            source: crate::types::constraints::test_support::source_id(1),
                        },
                    };
                    assert!(matches!(
                        driver.callbacks.close_materialization_callback(
                            &mut materialization,
                            foreign_ticket,
                            checkpoint,
                            Ok(MaterializationOutcome::Sealed(4)),
                        ),
                        Err(MaterializationImmediateFailure::Invariant(
                            TypeConstraintFailureInvariant::Constraint(
                                TypeConstraintInvariant::SourceProtocol(
                                    TypeConstraintSourceProtocolInvariant::Ticket
                                )
                            )
                        ))
                    ));
                    assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 1);
                    let (probe_ticket, probe_checkpoint) = driver
                        .callbacks
                        .begin_probe_callback(crate::types::constraints::test_support::source_id(1))
                        .expect("foreign close clears the active callback ticket");
                    driver
                        .callbacks
                        .close_probe_callback(
                            probe_ticket,
                            probe_checkpoint,
                            Ok(SourceProbeOutcome::Rejected("protocol")),
                        )
                        .expect("a later callback can begin");
                    assert_eq!(counts.probe_close.load(Ordering::Relaxed), 2);

                    drop(driver);
                },
            )
            .expect("prepared initialization");

        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .expect("second candidate session");
        let counts = Arc::new(Counts::default());
        session
            .with_fixture_driver::<Domain, _, _>(
                0,
                initialization(TypeConstraintParameterScope::empty()),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
                |mut driver| {
                    driver
                        .probe_prepared_source(
                            prepared(),
                            ConstraintAcceptance::PatternAcceptsActual,
                        )
                        .expect("second prepared source probe");
                    let mut materialization = driver
                        .lower
                        .next_materialization_ticket(driver.callbacks.context)
                        .expect("second lower materialization ticket")
                        .expect("one second materialization ticket");
                    let (ticket, mut checkpoint) = driver
                        .callbacks
                        .begin_materialization_callback(
                            driver.lower.materialization_owner(),
                            &mut materialization,
                        )
                        .expect("driver mints the next generation");
                    checkpoint.identity.issuer = Arc::new(SourceCallbackTicketIssuer);
                    assert!(matches!(
                        driver.callbacks.close_materialization_callback(
                            &mut materialization,
                            ticket,
                            checkpoint,
                            Ok(MaterializationOutcome::Sealed(4)),
                        ),
                        Err(MaterializationImmediateFailure::Invariant(
                            TypeConstraintFailureInvariant::Constraint(
                                TypeConstraintInvariant::SourceProtocol(
                                    TypeConstraintSourceProtocolInvariant::Checkpoint
                                )
                            )
                        ))
                    ));
                    assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 1);
                    let (probe_ticket, probe_checkpoint) = driver
                        .callbacks
                        .begin_probe_callback(crate::types::constraints::test_support::source_id(1))
                        .expect("foreign checkpoint close clears the active callback ticket");
                    driver
                        .callbacks
                        .close_probe_callback(
                            probe_ticket,
                            probe_checkpoint,
                            Ok(SourceProbeOutcome::Rejected("protocol")),
                        )
                        .expect("a later callback can begin");
                    assert_eq!(counts.probe_close.load(Ordering::Relaxed), 2);
                },
            )
            .expect("second prepared initialization");
    }

    #[test]
    fn client_close_failures_are_preserved_after_exact_cleanup() {
        let probe_counts = Arc::new(Counts::default());
        let probe_result = run(
            CallbackMode::CloseProbeClientFailure,
            Arc::clone(&probe_counts),
        );
        assert!(matches!(
            probe_result,
            Err(TypeConstraintFailure::Invariant(
                TypeConstraintFailureInvariant::Client(_)
            ))
        ));
        assert_eq!(probe_counts.probe_close.load(Ordering::Relaxed), 1);

        let materialize_counts = Arc::new(Counts::default());
        let materialize_result = run(
            CallbackMode::CloseMaterializationClientFailure,
            Arc::clone(&materialize_counts),
        );
        assert!(matches!(
            materialize_result,
            Err(TypeConstraintFailure::Invariant(
                TypeConstraintFailureInvariant::Client(_)
            ))
        ));
        assert_eq!(materialize_counts.probe_close.load(Ordering::Relaxed), 1);
        assert_eq!(
            materialize_counts.materialize_close.load(Ordering::Relaxed),
            1
        );
    }
}
