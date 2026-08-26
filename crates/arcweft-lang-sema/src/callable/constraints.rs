//! Callable-owned driver for the lower candidate constraint transaction.
//!
//! The driver owns callback checkpoints.  A callback attempt is always handed
//! to the client's affine close operation; the driver never unconditionally
//! rolls a checkpoint back and therefore cannot double-close a materialization.

use super::continuation::{CallConstraintInvariant, PreparedConstraintInitialization};
pub(crate) use super::limits::CandidateConstraintWorkSession;
use crate::types::constraints::context::TypeConstraintContext;
use crate::types::constraints::transaction::{
    ClosedMaterialization, MaterializationCallbackBinding, MaterializationTicket, ProbeStart,
    ProbeSubmission, TypeConstraintRun, TypeConstraintTransaction,
};
use crate::types::constraints::{
    ClosedMaterializationSubmission, ConstraintDomain, ExpectedHint,
    MaterializationImmediateFailure, MaterializationOutcome, MaterializedSourceRequest,
    PreparedSourceConstraint, SourceError, SourcePhase, SourceProbeOutcome, TypeConstraintAbort,
    TypeConstraintFailure, TypeConstraintFailureInvariant, TypeConstraintInitializationFailure,
    TypeConstraintInvariant, TypeConstraintSourceProtocolInvariant,
};
use crate::types::{ConstraintAcceptance, TypeKind};
use std::sync::Arc;

/// The callback vocabulary is deliberately owned by callable.  Source facts
/// cross the boundary only as typed lower results, while checkpoint closure
/// remains client-owned because it moves analyzer semantic state.
pub(crate) trait TypeConstraintClient<D: ConstraintDomain> {
    type ProbeCheckpoint;
    type MaterializationCheckpoint;
    type PreparedSealedBranchValue;

    fn probe_source<'h>(
        &mut self,
        source: D::Source,
        hint: ExpectedHint<'h, D>,
        checkpoint: &mut Self::ProbeCheckpoint,
        work: &mut CandidateConstraintWorkSession<'_>,
    ) -> Result<SourceProbeOutcome<D>, SourceCallbackFailure<D>>;

    fn open_probe_checkpoint(
        &mut self,
        source: D::Source,
    ) -> Result<Self::ProbeCheckpoint, SourceCheckpointFailure<D>>;

    fn close_probe_checkpoint(
        &mut self,
        checkpoint: Self::ProbeCheckpoint,
    ) -> Result<(), SourceCheckpointFailure<D>>;

    fn open_materialization_checkpoint(
        &mut self,
        sources: &[D::Source],
    ) -> Result<Self::MaterializationCheckpoint, SourceCheckpointFailure<D>>;

    fn materialize_sources<'h, I>(
        &mut self,
        sources: I,
        checkpoint: &mut Self::MaterializationCheckpoint,
        work: &mut CandidateConstraintWorkSession<'_>,
    ) -> Result<
        MaterializationOutcome<D::Source, Self::PreparedSealedBranchValue, D::SourceErrorCause>,
        SourceCallbackFailure<D>,
    >
    where
        I: IntoIterator<Item = MaterializedSourceRequest<'h, D>>,
        D::CheckedEvidence: 'h,
        D::ProbeSemanticBranch: 'h;

    fn close_materialization_checkpoint(
        &mut self,
        checkpoint: Self::MaterializationCheckpoint,
        sealed: Option<Self::PreparedSealedBranchValue>,
    ) -> Result<Option<D::SealedBranchValue>, SourceCheckpointFailure<D>>;

    fn finish(self) -> Result<(), SourceCheckpointFailure<D>>
    where
        Self: Sized;
}

/// Callback failures are deliberately separate from ordinary semantic
/// rejections.  Only the driver may validate and promote a fatal source error.
pub(crate) enum SourceCallbackFailure<D: ConstraintDomain> {
    Fatal(Box<SourceError<D::Source, D::SourceErrorCause>>),
    Abort(TypeConstraintAbort),
    Invariant(Box<D::ClientInvariant>),
}

pub(crate) enum SourceCheckpointFailure<D: ConstraintDomain> {
    Protocol(TypeConstraintSourceProtocolInvariant),
    Client(Box<D::ClientInvariant>),
}

impl<D: ConstraintDomain> SourceCallbackFailure<D> {
    pub(crate) fn fatal(error: SourceError<D::Source, D::SourceErrorCause>) -> Self {
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
        source: D::Source,
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

pub(crate) struct CandidateConstraintDriver<'a, D: ConstraintDomain, C: TypeConstraintClient<D>> {
    context: TypeConstraintContext<'a, CandidateConstraintWorkSession<'a>, D>,
    lower: TypeConstraintTransaction<D>,
    client: C,
    ticket_issuer: Arc<SourceCallbackTicketIssuer>,
    next_ticket_ordinal: u64,
    active_ticket: Option<SourceCallbackTicketIdentity>,
}

#[derive(Debug)]
pub(crate) enum CandidateConstraintDriverStartFailure {
    Prepared(CallConstraintInvariant),
    Lower(TypeConstraintInitializationFailure),
}

impl<'a> CandidateConstraintWorkSession<'a> {
    /// Starts the only production lower candidate driver.
    pub(crate) fn start<D, C>(
        self,
        initialization: PreparedConstraintInitialization,
        client: C,
    ) -> Result<CandidateConstraintDriver<'a, D, C>, CandidateConstraintDriverStartFailure>
    where
        D: ConstraintDomain,
        C: TypeConstraintClient<D>,
    {
        let (parameter_scope, effect_scope, inherited) = initialization
            .into_lower_parts()
            .map_err(CandidateConstraintDriverStartFailure::Prepared)?;
        let mut driver = CandidateConstraintDriver {
            context: TypeConstraintContext::with_accounting(self, parameter_scope, effect_scope),
            lower: TypeConstraintTransaction::new(),
            client,
            ticket_issuer: Arc::new(SourceCallbackTicketIssuer),
            next_ticket_ordinal: 0,
            active_ticket: None,
        };
        driver
            .lower
            .initialize(&mut driver.context, inherited)
            .map_err(CandidateConstraintDriverStartFailure::Lower)?;
        Ok(driver)
    }
}

impl<'a, D, C> CandidateConstraintDriver<'a, D, C>
where
    D: ConstraintDomain,
    C: TypeConstraintClient<D>,
{
    pub(crate) fn request_projection(
        &mut self,
        key: D::Projection,
        value: &TypeKind,
        closure: crate::types::constraints::TypeConstraintProjectionClosure,
    ) {
        self.lower.request_projection(key, value, closure);
    }

    fn with_callback<R>(
        &mut self,
        operation: impl FnOnce(&mut C, &mut CandidateConstraintWorkSession<'_>) -> R,
    ) -> R {
        let session = self.context.accounting_mut();
        operation(&mut self.client, session)
    }

    fn protocol_failure(kind: TypeConstraintSourceProtocolInvariant) -> TypeConstraintFailure<D> {
        TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Constraint(
            TypeConstraintInvariant::SourceProtocol(kind),
        ))
    }

    fn checkpoint_failure(failure: SourceCheckpointFailure<D>) -> TypeConstraintFailure<D> {
        match failure {
            SourceCheckpointFailure::Protocol(kind) => Self::protocol_failure(kind),
            SourceCheckpointFailure::Client(invariant) => {
                TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Client(invariant))
            }
        }
    }

    fn materialization_protocol_failure(
        kind: TypeConstraintSourceProtocolInvariant,
    ) -> MaterializationImmediateFailure<D> {
        MaterializationImmediateFailure::Invariant(TypeConstraintFailureInvariant::Constraint(
            TypeConstraintInvariant::SourceProtocol(kind),
        ))
    }

    fn materialization_checkpoint_failure(
        failure: SourceCheckpointFailure<D>,
    ) -> MaterializationImmediateFailure<D> {
        match failure {
            SourceCheckpointFailure::Protocol(kind) => Self::materialization_protocol_failure(kind),
            SourceCheckpointFailure::Client(invariant) => {
                MaterializationImmediateFailure::Invariant(TypeConstraintFailureInvariant::Client(
                    invariant,
                ))
            }
        }
    }

    fn mint_ticket(
        &mut self,
        authority: SourceCallbackAuthority<D>,
        charge: impl FnOnce(
            &mut TypeConstraintContext<'a, CandidateConstraintWorkSession<'a>, D>,
        ) -> Result<(), crate::types::constraints::TypeConstraintError>,
    ) -> Result<SourceCallbackTicket<D>, TypeConstraintFailure<D>> {
        let ordinal = self.reserve_ticket_ordinal(charge)?;
        Ok(self.ticket_from_ordinal(ordinal, authority))
    }

    fn reserve_ticket_ordinal(
        &mut self,
        charge: impl FnOnce(
            &mut TypeConstraintContext<'a, CandidateConstraintWorkSession<'a>, D>,
        ) -> Result<(), crate::types::constraints::TypeConstraintError>,
    ) -> Result<u64, TypeConstraintFailure<D>> {
        let ordinal = self.next_ticket_ordinal;
        let next_ordinal = ordinal.checked_add(1).ok_or(TypeConstraintFailure::Abort(
            TypeConstraintAbort::ArithmeticOverflow,
        ))?;
        charge(&mut self.context).map_err(TypeConstraintFailure::from)?;
        self.next_ticket_ordinal = next_ordinal;
        Ok(ordinal)
    }

    fn ticket_from_ordinal(
        &self,
        ordinal: u64,
        authority: SourceCallbackAuthority<D>,
    ) -> SourceCallbackTicket<D> {
        SourceCallbackTicket {
            identity: SourceCallbackTicketIdentity {
                issuer: Arc::clone(&self.ticket_issuer),
                ordinal,
            },
            authority,
        }
    }

    fn identity_matches(
        left: &SourceCallbackTicketIdentity,
        right: &SourceCallbackTicketIdentity,
    ) -> bool {
        Arc::ptr_eq(&left.issuer, &right.issuer) && left.ordinal == right.ordinal
    }

    fn close_probe_checkpoint_once(
        &mut self,
        checkpoint: C::ProbeCheckpoint,
    ) -> Result<(), SourceCheckpointFailure<D>> {
        let result = self.with_callback(|client, _| client.close_probe_checkpoint(checkpoint));
        self.active_ticket = None;
        result
    }

    fn close_materialization_checkpoint_once(
        &mut self,
        checkpoint: C::MaterializationCheckpoint,
        sealed: Option<C::PreparedSealedBranchValue>,
    ) -> Result<Option<D::SealedBranchValue>, SourceCheckpointFailure<D>> {
        let result = self
            .with_callback(|client, _| client.close_materialization_checkpoint(checkpoint, sealed));
        self.active_ticket = None;
        result
    }

    fn finish_materialization_close(
        authority_failure: Option<TypeConstraintSourceProtocolInvariant>,
        invalid: Option<TypeConstraintSourceProtocolInvariant>,
        close: Result<Option<D::SealedBranchValue>, SourceCheckpointFailure<D>>,
    ) -> Result<Option<D::SealedBranchValue>, MaterializationImmediateFailure<D>> {
        if let Some(failure) = authority_failure {
            return Err(Self::materialization_protocol_failure(failure));
        }
        if let Some(failure) = invalid {
            return Err(Self::materialization_protocol_failure(failure));
        }
        close.map_err(Self::materialization_checkpoint_failure)
    }

    fn begin_probe_callback(
        &mut self,
        source: D::Source,
    ) -> Result<
        (
            SourceCallbackTicket<D>,
            BoundSourceCheckpoint<C::ProbeCheckpoint>,
        ),
        TypeConstraintFailure<D>,
    > {
        if self.active_ticket.is_some() {
            return Err(Self::protocol_failure(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        let ticket = self.mint_ticket(SourceCallbackAuthority::Probe { source }, |context| {
            context.charge_source_probe()
        })?;
        let checkpoint = self
            .with_callback(|client, _| client.open_probe_checkpoint(source))
            .map_err(Self::checkpoint_failure)?;
        let identity = ticket.identity.clone();
        self.active_ticket = Some(ticket.identity.clone());
        Ok((
            ticket,
            BoundSourceCheckpoint {
                identity,
                checkpoint,
            },
        ))
    }

    fn close_probe_callback(
        &mut self,
        ticket: SourceCallbackTicket<D>,
        checkpoint: BoundSourceCheckpoint<C::ProbeCheckpoint>,
        attempt: Result<SourceProbeOutcome<D>, SourceCallbackFailure<D>>,
    ) -> Result<SourceProbeOutcome<D>, TypeConstraintFailure<D>> {
        let authority_source = match &ticket.authority {
            SourceCallbackAuthority::Probe { source } => Some(*source),
            SourceCallbackAuthority::Materialize { .. } => None,
        };
        let authority_failure = if authority_source.is_none() {
            Some(TypeConstraintSourceProtocolInvariant::Ticket)
        } else if !self
            .active_ticket
            .as_ref()
            .is_some_and(|identity| Self::identity_matches(identity, &ticket.identity))
        {
            Some(TypeConstraintSourceProtocolInvariant::Ticket)
        } else if !Self::identity_matches(&ticket.identity, &checkpoint.identity) {
            Some(TypeConstraintSourceProtocolInvariant::Checkpoint)
        } else {
            None
        };
        let invalid = match (authority_source, &attempt) {
            (Some(source), Err(SourceCallbackFailure::Fatal(error))) => {
                if error.phase() != SourcePhase::Probe {
                    Some(TypeConstraintSourceProtocolInvariant::WrongPhase)
                } else if error.source() != &source {
                    Some(TypeConstraintSourceProtocolInvariant::WrongSource)
                } else {
                    None
                }
            }
            (Some(source), Err(SourceCallbackFailure::Invariant(invariant))) => {
                if D::client_invariant_source(invariant) != source {
                    Some(TypeConstraintSourceProtocolInvariant::WrongSource)
                } else {
                    None
                }
            }
            _ => None,
        };
        let close_failure = self
            .close_probe_checkpoint_once(checkpoint.checkpoint)
            .err();
        if let Some(failure) = authority_failure {
            return Err(Self::protocol_failure(failure));
        }
        if let Some(invalid) = invalid {
            return Err(Self::protocol_failure(invalid));
        }
        if let Some(failure) = close_failure {
            return Err(Self::checkpoint_failure(failure));
        }
        match attempt {
            Ok(outcome) => Ok(outcome),
            Err(SourceCallbackFailure::Fatal(error)) => {
                Err(TypeConstraintFailure::FatalSource(error))
            }
            Err(SourceCallbackFailure::Abort(error)) => Err(TypeConstraintFailure::Abort(error)),
            Err(SourceCallbackFailure::Invariant(invariant)) => Err(
                TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Client(invariant)),
            ),
        }
    }

    fn begin_materialization_callback(
        &mut self,
        materialization: &mut MaterializationTicket<D>,
    ) -> Result<
        (
            SourceCallbackTicket<D>,
            BoundSourceCheckpoint<C::MaterializationCheckpoint>,
        ),
        MaterializationImmediateFailure<D>,
    > {
        if self.active_ticket.is_some() {
            return Err(Self::materialization_protocol_failure(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        self.lower
            .validate_materialization_callback_begin(materialization)
            .map_err(Self::materialization_protocol_failure)?;
        let ordinal = self
            .reserve_ticket_ordinal(|context| context.charge_materialization())
            .map_err(materialization_immediate_from_failure)?;
        let binding = materialization
            .bind_callback()
            .map_err(Self::materialization_protocol_failure)?;
        let checkpoint = self
            .with_callback(|client, _| client.open_materialization_checkpoint(binding.sources()))
            .map_err(Self::materialization_checkpoint_failure)?;
        let ticket =
            self.ticket_from_ordinal(ordinal, SourceCallbackAuthority::Materialize { binding });
        let identity = ticket.identity.clone();
        self.active_ticket = Some(ticket.identity.clone());
        Ok((
            ticket,
            BoundSourceCheckpoint {
                identity,
                checkpoint,
            },
        ))
    }

    fn close_materialization_callback(
        &mut self,
        materialization: &mut MaterializationTicket<D>,
        ticket: SourceCallbackTicket<D>,
        checkpoint: BoundSourceCheckpoint<C::MaterializationCheckpoint>,
        attempt: Result<
            MaterializationOutcome<D::Source, C::PreparedSealedBranchValue, D::SourceErrorCause>,
            SourceCallbackFailure<D>,
        >,
    ) -> Result<ClosedMaterialization<D>, MaterializationImmediateFailure<D>> {
        let authority_binding = match &ticket.authority {
            SourceCallbackAuthority::Materialize { binding } => Some(binding),
            SourceCallbackAuthority::Probe { .. } => None,
        };
        let authority_failure = if authority_binding.is_none() {
            Some(TypeConstraintSourceProtocolInvariant::Ticket)
        } else if !self
            .active_ticket
            .as_ref()
            .is_some_and(|identity| Self::identity_matches(identity, &ticket.identity))
        {
            Some(TypeConstraintSourceProtocolInvariant::Ticket)
        } else if !Self::identity_matches(&ticket.identity, &checkpoint.identity) {
            Some(TypeConstraintSourceProtocolInvariant::Checkpoint)
        } else {
            authority_binding
                .and_then(|binding| materialization.validate_callback_binding(binding).err())
        };
        let invalid = match (authority_binding, &attempt) {
            (Some(binding), Ok(MaterializationOutcome::Rejected { source, .. })) => (!binding
                .authorizes(source))
            .then_some(TypeConstraintSourceProtocolInvariant::WrongSource),
            (Some(binding), Err(SourceCallbackFailure::Fatal(error))) => {
                if error.phase() != SourcePhase::Materialize {
                    Some(TypeConstraintSourceProtocolInvariant::WrongPhase)
                } else if !binding.authorizes(error.source()) {
                    Some(TypeConstraintSourceProtocolInvariant::WrongSource)
                } else {
                    None
                }
            }
            (Some(binding), Err(SourceCallbackFailure::Invariant(invariant))) => (!binding
                .authorizes(&D::client_invariant_source(invariant)))
            .then_some(TypeConstraintSourceProtocolInvariant::WrongSource),
            _ => None,
        };
        let may_extract = authority_failure.is_none() && invalid.is_none();
        match attempt {
            Ok(MaterializationOutcome::Sealed(value)) => {
                let sealed = Self::finish_materialization_close(
                    authority_failure,
                    invalid,
                    self.close_materialization_checkpoint_once(
                        checkpoint.checkpoint,
                        may_extract.then_some(value),
                    ),
                )?;
                let Some(sealed) = sealed else {
                    return Err(Self::materialization_protocol_failure(
                        TypeConstraintSourceProtocolInvariant::Outcome,
                    ));
                };
                materialization
                    .bind_closed_submission(ClosedMaterializationSubmission::Sealed(sealed))
                    .map_err(Self::materialization_protocol_failure)
            }
            Ok(MaterializationOutcome::Rejected { source, cause }) => {
                let sealed = Self::finish_materialization_close(
                    authority_failure,
                    invalid,
                    self.close_materialization_checkpoint_once(checkpoint.checkpoint, None),
                )?;
                if sealed.is_some() {
                    return Err(Self::materialization_protocol_failure(
                        TypeConstraintSourceProtocolInvariant::Outcome,
                    ));
                }
                materialization
                    .bind_closed_submission(ClosedMaterializationSubmission::Rejected {
                        source,
                        cause,
                    })
                    .map_err(Self::materialization_protocol_failure)
            }
            Err(SourceCallbackFailure::Fatal(error)) => {
                let sealed = Self::finish_materialization_close(
                    authority_failure,
                    invalid,
                    self.close_materialization_checkpoint_once(checkpoint.checkpoint, None),
                )?;
                if sealed.is_some() {
                    return Err(Self::materialization_protocol_failure(
                        TypeConstraintSourceProtocolInvariant::Outcome,
                    ));
                }
                materialization
                    .bind_closed_submission(ClosedMaterializationSubmission::Fatal(*error))
                    .map_err(Self::materialization_protocol_failure)
            }
            Err(SourceCallbackFailure::Abort(error)) => {
                let sealed = Self::finish_materialization_close(
                    authority_failure,
                    invalid,
                    self.close_materialization_checkpoint_once(checkpoint.checkpoint, None),
                )?;
                if sealed.is_some() {
                    return Err(Self::materialization_protocol_failure(
                        TypeConstraintSourceProtocolInvariant::Outcome,
                    ));
                }
                Err(MaterializationImmediateFailure::Abort(error))
            }
            Err(SourceCallbackFailure::Invariant(invariant)) => {
                let sealed = Self::finish_materialization_close(
                    authority_failure,
                    invalid,
                    self.close_materialization_checkpoint_once(checkpoint.checkpoint, None),
                )?;
                if sealed.is_some() {
                    return Err(Self::materialization_protocol_failure(
                        TypeConstraintSourceProtocolInvariant::Outcome,
                    ));
                }
                Err(MaterializationImmediateFailure::Invariant(
                    TypeConstraintFailureInvariant::Client(invariant),
                ))
            }
        }
    }

    pub(crate) fn constrain(
        &mut self,
        pattern: &TypeKind,
        actual: &TypeKind,
        acceptance: ConstraintAcceptance,
    ) {
        self.lower
            .constrain(&mut self.context, pattern, actual, acceptance);
    }

    /// Run one prepared source through every correlated frontier row.
    pub(crate) fn probe_prepared_source(
        &mut self,
        prepared: PreparedSourceConstraint<D>,
        acceptance: ConstraintAcceptance,
    ) -> Result<(), TypeConstraintFailure<D>> {
        let started = self
            .lower
            .begin_prepared_probe(&mut self.context, prepared, acceptance)
            .map_err(TypeConstraintFailure::from)?;
        if matches!(started, ProbeStart::Skipped) {
            return Ok(());
        }
        loop {
            let ticket = self
                .lower
                .next_probe(&mut self.context)
                .map_err(TypeConstraintFailure::from)?;
            let Some(lower_ticket) = ticket else {
                break;
            };
            let source = lower_ticket.source();
            let (callback_ticket, mut checkpoint) = match self.begin_probe_callback(source) {
                Ok(callback) => callback,
                Err(error) => {
                    self.lower.record_failure(error);
                    break;
                }
            };
            let attempt = self.with_callback(|client, work| {
                lower_ticket.with_hint(|hint| {
                    client.probe_source(source, hint, &mut checkpoint.checkpoint, work)
                })
            });
            let submission = match self.close_probe_callback(callback_ticket, checkpoint, attempt) {
                Ok(SourceProbeOutcome::Accepted(result)) => ProbeSubmission::Accepted(result),
                Ok(SourceProbeOutcome::Rejected(cause)) => ProbeSubmission::Rejected(cause),
                Err(error) => {
                    self.lower.record_failure(error);
                    break;
                }
            };
            if let Err(error) = self
                .lower
                .submit_probe(&mut self.context, lower_ticket, submission)
            {
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

    fn materialize_all(&mut self) -> Result<(), TypeConstraintFailure<D>> {
        loop {
            let ticket = self
                .lower
                .next_materialization_ticket(&mut self.context)
                .map_err(TypeConstraintFailure::from)?;
            let Some(mut lower_ticket) = ticket else {
                break;
            };
            let (callback_ticket, mut checkpoint) =
                match self.begin_materialization_callback(&mut lower_ticket) {
                    Ok(callback) => callback,
                    Err(error) => {
                        self.lower.record_failure(error.into());
                        break;
                    }
                };
            let attempt = self.with_callback(|client, work| {
                client.materialize_sources(
                    lower_ticket.requests(),
                    &mut checkpoint.checkpoint,
                    work,
                )
            });
            let closed = match self.close_materialization_callback(
                &mut lower_ticket,
                callback_ticket,
                checkpoint,
                attempt,
            ) {
                Ok(closed) => closed,
                Err(error) => {
                    self.lower.record_failure(error.into());
                    break;
                }
            };
            if let Err(error) = self
                .lower
                .submit_closed_materialization(lower_ticket, closed)
            {
                self.lower.record_failure(error.into());
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn finish(mut self) -> TypeConstraintRun<'a, CandidateConstraintWorkSession<'a>, D> {
        if let Err(failure) = self.materialize_all() {
            self.lower.record_failure(failure);
        }
        if let Err(failure) = self.client.finish() {
            self.lower.record_failure(Self::checkpoint_failure(failure));
        }
        self.lower.finish(self.context)
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

    fn probe_source<'h>(
        &mut self,
        source: (),
        _hint: ExpectedHint<'h, crate::types::NoConstraintClient>,
        _checkpoint: &mut Self::ProbeCheckpoint,
        _work: &mut CandidateConstraintWorkSession<'_>,
    ) -> Result<
        SourceProbeOutcome<crate::types::NoConstraintClient>,
        SourceCallbackFailure<crate::types::NoConstraintClient>,
    > {
        Err(SourceCallbackFailure::fatal(SourceError::new(
            source,
            crate::types::constraints::SourcePhase::Probe,
            (),
        )))
    }

    fn open_probe_checkpoint(
        &mut self,
        _source: (),
    ) -> Result<Self::ProbeCheckpoint, SourceCheckpointFailure<crate::types::NoConstraintClient>>
    {
        Ok(())
    }

    fn close_probe_checkpoint(
        &mut self,
        _checkpoint: Self::ProbeCheckpoint,
    ) -> Result<(), SourceCheckpointFailure<crate::types::NoConstraintClient>> {
        Ok(())
    }

    fn open_materialization_checkpoint(
        &mut self,
        _sources: &[()],
    ) -> Result<
        Self::MaterializationCheckpoint,
        SourceCheckpointFailure<crate::types::NoConstraintClient>,
    > {
        Ok(())
    }

    fn materialize_sources<'h, I>(
        &mut self,
        _sources: I,
        _checkpoint: &mut Self::MaterializationCheckpoint,
        _work: &mut CandidateConstraintWorkSession<'_>,
    ) -> Result<
        MaterializationOutcome<(), (), ()>,
        SourceCallbackFailure<crate::types::NoConstraintClient>,
    >
    where
        I: IntoIterator<Item = MaterializedSourceRequest<'h, crate::types::NoConstraintClient>>,
        (): 'h,
    {
        Err(SourceCallbackFailure::fatal(SourceError::new(
            (),
            crate::types::constraints::SourcePhase::Materialize,
            (),
        )))
    }

    fn close_materialization_checkpoint(
        &mut self,
        _checkpoint: Self::MaterializationCheckpoint,
        _sealed: Option<Self::PreparedSealedBranchValue>,
    ) -> Result<Option<()>, SourceCheckpointFailure<crate::types::NoConstraintClient>> {
        Ok(None)
    }

    fn finish(self) -> Result<(), SourceCheckpointFailure<crate::types::NoConstraintClient>> {
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::callable::limits::{PRODUCTION_CALLABLE_LIMITS, ResolverWork};
    use crate::env::nominal::{AcceptedNominalId, AcceptedNominalOwnerId};
    use crate::types::constraints::context::TypeConstraintWorkReport;
    use crate::types::constraints::{
        PreparedSourceAlternative, PreparedSourceConstraint, SourcePhase, SourceProbeResult,
        TypeConstraintParameterScope,
    };
    use crate::types::{GenericParameterOwnerId, GenericTypeParameterId, TypeKind};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
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

    fn accepted_type(owner: u64, ordinal: u16) -> GenericTypeParameterId {
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
        let parameters = scope
            .iter()
            .map(|(parameter, _)| parameter.clone())
            .collect::<Vec<_>>();
        let issuer = match parameters
            .first()
            .map(|parameter| parameter.owner().clone())
        {
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
                                crate::callable::CallableName::try_new(format!("arg{index}"))
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
            crate::callable::CallableCandidateId::Fx(crate::callable::FxCallableSignatureId::Style),
            crate::callable::SignatureOrigin::Language {
                family: crate::callable::LanguageCallableFamily::Fx,
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
        )
        .expect("test enclosing scope");
        crate::callable::PreparedCallGraph::<()>::new()
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
        type Source = u8;
        type AlternativeIndex = u8;
        type EvidenceRule = ();
        type CheckedEvidence = ();
        type ProbeSemanticBranch = Branch;
        type SealedBranchValue = Sealed;
        type Projection = u8;
        type SourceErrorCause = &'static str;
        type ClientInvariant = ();

        fn evidence_accepts(_: &Self::EvidenceRule, _: &Self::CheckedEvidence) -> bool {
            true
        }

        fn project_checked_evidence(
            _: &Self::CheckedEvidence,
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
        Rejected,
        Fatal,
        ProbeAbort,
        ProbeInvariant,
        GroupRejectedThenSuccess,
        GroupRejectedThenFatal,
        GroupRejectedThenAbort,
        GroupRejectedThenInvariant,
        WrongFatalSource,
        MaterializationRejected,
        MaterializationWrongSource,
        MaterializationFatal,
        MaterializationReversedFatal,
        MaterializationAbort,
        MaterializationInvariant,
        OpenProbeFailure,
        OpenMaterializationFailure,
        CloseProbeClientFailure,
        CloseMaterializationClientFailure,
    }

    #[derive(Default)]
    struct Counts {
        probe_begin: AtomicUsize,
        probe_close: AtomicUsize,
        probe_call: AtomicUsize,
        materialize_begin: AtomicUsize,
        materialize_close: AtomicUsize,
        materialize_call: AtomicUsize,
    }

    struct Client {
        counts: Arc<Counts>,
        mode: CallbackMode,
    }

    impl TypeConstraintClient<Domain> for Client {
        type ProbeCheckpoint = u8;
        type MaterializationCheckpoint = u8;
        type PreparedSealedBranchValue = u8;

        fn probe_source<'h>(
            &mut self,
            source: u8,
            _hint: ExpectedHint<'h, Domain>,
            _checkpoint: &mut Self::ProbeCheckpoint,
            _work: &mut CandidateConstraintWorkSession<'_>,
        ) -> Result<SourceProbeOutcome<Domain>, SourceCallbackFailure<Domain>> {
            self.counts.probe_call.fetch_add(1, Ordering::Relaxed);
            if matches!(
                self.mode,
                CallbackMode::GroupRejectedThenSuccess
                    | CallbackMode::GroupRejectedThenFatal
                    | CallbackMode::GroupRejectedThenAbort
                    | CallbackMode::GroupRejectedThenInvariant
            ) {
                if source == 1 {
                    return Ok(SourceProbeOutcome::Rejected("group head rejection"));
                }
                return match self.mode {
                    CallbackMode::GroupRejectedThenSuccess => Ok(SourceProbeOutcome::Accepted(
                        SourceProbeResult::checked(TypeKind::I32, Branch, 0, ()),
                    )),
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
                CallbackMode::Success
                | CallbackMode::MaterializationRejected
                | CallbackMode::MaterializationWrongSource
                | CallbackMode::MaterializationFatal
                | CallbackMode::MaterializationReversedFatal
                | CallbackMode::MaterializationAbort
                | CallbackMode::MaterializationInvariant
                | CallbackMode::OpenProbeFailure
                | CallbackMode::OpenMaterializationFailure
                | CallbackMode::CloseProbeClientFailure
                | CallbackMode::CloseMaterializationClientFailure
                | CallbackMode::GroupRejectedThenSuccess
                | CallbackMode::GroupRejectedThenFatal
                | CallbackMode::GroupRejectedThenAbort
                | CallbackMode::GroupRejectedThenInvariant => Ok(SourceProbeOutcome::Accepted(
                    SourceProbeResult::checked(TypeKind::I32, Branch, 0, ()),
                )),
                CallbackMode::Rejected => Ok(SourceProbeOutcome::Rejected("rejected")),
                CallbackMode::ProbeAbort => {
                    Err(SourceCallbackFailure::Abort(TypeConstraintAbort::Cancelled))
                }
                CallbackMode::ProbeInvariant => Err(SourceCallbackFailure::invariant(())),
                CallbackMode::WrongFatalSource => Err(SourceCallbackFailure::fatal(
                    SourceError::new(source.saturating_add(1), SourcePhase::Probe, "fatal"),
                )),
                CallbackMode::Fatal => Err(SourceCallbackFailure::fatal(SourceError::new(
                    source,
                    SourcePhase::Probe,
                    "fatal",
                ))),
            }
        }

        fn open_probe_checkpoint(
            &mut self,
            _source: u8,
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
            _checkpoint: Self::ProbeCheckpoint,
        ) -> Result<(), SourceCheckpointFailure<Domain>> {
            self.counts.probe_close.fetch_add(1, Ordering::Relaxed);
            if matches!(self.mode, CallbackMode::CloseProbeClientFailure) {
                return Err(SourceCheckpointFailure::client(()));
            }
            Ok(())
        }

        fn open_materialization_checkpoint(
            &mut self,
            sources: &[u8],
        ) -> Result<Self::MaterializationCheckpoint, SourceCheckpointFailure<Domain>> {
            self.counts
                .materialize_begin
                .fetch_add(1, Ordering::Relaxed);
            if matches!(self.mode, CallbackMode::MaterializationReversedFatal) {
                assert_eq!(sources, &[10, 20]);
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
        ) -> Result<MaterializationOutcome<u8, u8, &'static str>, SourceCallbackFailure<Domain>>
        where
            I: IntoIterator<Item = MaterializedSourceRequest<'h, Domain>>,
            <Domain as ConstraintDomain>::CheckedEvidence: 'h,
            <Domain as ConstraintDomain>::ProbeSemanticBranch: 'h,
        {
            self.counts.materialize_call.fetch_add(1, Ordering::Relaxed);
            let requests = sources.into_iter().collect::<Vec<_>>();
            match self.mode {
                CallbackMode::Success => Ok(MaterializationOutcome::Sealed(3)),
                CallbackMode::Rejected | CallbackMode::MaterializationRejected => {
                    Ok(MaterializationOutcome::Rejected {
                        source: 1,
                        cause: "materialized rejection",
                    })
                }
                CallbackMode::MaterializationWrongSource => Ok(MaterializationOutcome::Rejected {
                    source: 9,
                    cause: "wrong source",
                }),
                CallbackMode::Fatal | CallbackMode::MaterializationFatal => {
                    Err(SourceCallbackFailure::fatal(SourceError::new(
                        1,
                        SourcePhase::Materialize,
                        "materialized fatal",
                    )))
                }
                CallbackMode::MaterializationReversedFatal => {
                    assert_eq!(
                        requests
                            .iter()
                            .map(|request| *request.source())
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
                        source,
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
                | CallbackMode::CloseMaterializationClientFailure
                | CallbackMode::ProbeAbort
                | CallbackMode::ProbeInvariant
                | CallbackMode::WrongFatalSource
                | CallbackMode::GroupRejectedThenSuccess
                | CallbackMode::GroupRejectedThenFatal
                | CallbackMode::GroupRejectedThenAbort
                | CallbackMode::GroupRejectedThenInvariant => Ok(MaterializationOutcome::Sealed(3)),
            }
        }

        fn close_materialization_checkpoint(
            &mut self,
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

        fn finish(self) -> Result<(), SourceCheckpointFailure<Domain>> {
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
            PreparedSourceAlternative::new(0, (), TypeKind::GenericParam(unscoped_parameter())),
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
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut work = ResolverWork::new(4_096);
        let session = work
            .begin_candidate_constraint_session(limits, &cancellation)
            .expect("candidate session");
        let mut driver = session
            .start::<Domain, _>(
                initialization(TypeConstraintParameterScope::empty()),
                Client { counts, mode },
            )
            .expect("prepared initialization");
        let result = match driver
            .probe_prepared_source(prepared(), ConstraintAcceptance::PatternAcceptsActual)
        {
            Ok(()) => driver.finish().complete().map(|_| ()),
            Err(error) => {
                let _ = driver.finish().complete();
                Err(error)
            }
        };
        let report = work.type_constraint_report().clone();
        (result, report)
    }

    fn run(mode: CallbackMode, counts: Arc<Counts>) -> Result<(), TypeConstraintFailure<Domain>> {
        run_with_limits(mode, counts, PRODUCTION_CALLABLE_LIMITS).0
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
        let mut driver = session
            .start::<Domain, _>(
                initialization(TypeConstraintParameterScope::empty()),
                Client { counts, mode },
            )
            .expect("prepared initialization");
        let group = PreparedSourceConstraintGroup::seal([prepared_source(1), prepared_source(2)])
            .expect("two-source mapper group");
        driver.probe_source_group(group, ConstraintAcceptance::PatternAcceptsActual)?;
        driver.finish().complete().map(|_| ())
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
                    Err(TypeConstraintFailure::FatalSource(ref error)) if error.source() == &2
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
        let mut driver = session
            .start::<Domain, _>(
                initialization(TypeConstraintParameterScope::empty()),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
            )
            .expect("prepared initialization");
        let failure = driver
            .probe_prepared_source(
                prepared_with_unscoped_hint(),
                ConstraintAcceptance::PatternAcceptsActual,
            )
            .expect_err("unscoped hint is rejected before callback");
        let _ = driver.finish().complete();
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
        let result = session.start::<Domain, _>(
            initialization(TypeConstraintParameterScope::empty()),
            Client {
                counts: Arc::clone(&counts),
                mode: CallbackMode::Success,
            },
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
        let result = session.start::<Domain, _>(
            initialization(TypeConstraintParameterScope::empty()),
            Client {
                counts: Arc::clone(&counts),
                mode: CallbackMode::Success,
            },
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
        assert_eq!(work.type_constraint_report().work(), 0);
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
        let mut driver = session
            .start::<Domain, _>(
                initialization(scope),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
            )
            .expect("prepared initialization");
        driver.constrain(
            &TypeKind::Choice(vec![
                TypeKind::GenericParam(first),
                TypeKind::GenericParam(second),
            ]),
            &TypeKind::I32,
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
        let result = driver.finish().complete();
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
        let mut driver = session
            .start::<Domain, _>(
                initialization(TypeConstraintParameterScope::empty()),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
            )
            .expect("prepared initialization");
        driver
            .probe_prepared_source(prepared(), ConstraintAcceptance::PatternAcceptsActual)
            .expect("prepared source probe");
        let mut lower_ticket = driver
            .lower
            .next_materialization_ticket(&mut driver.context)
            .expect("lower materialization ticket")
            .expect("one materialization ticket");

        cancellation.store(true, std::sync::atomic::Ordering::Release);
        assert!(matches!(
            driver.begin_materialization_callback(&mut lower_ticket),
            Err(MaterializationImmediateFailure::Abort(
                TypeConstraintAbort::Cancelled
            ))
        ));
        assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 0);
        assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 0);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 0);

        cancellation.store(false, std::sync::atomic::Ordering::Release);
        let (callback_ticket, mut checkpoint) = driver
            .begin_materialization_callback(&mut lower_ticket)
            .expect("the cancelled ticket remains ready");
        let attempt = driver.with_callback(|client, work| {
            client.materialize_sources(lower_ticket.requests(), &mut checkpoint.checkpoint, work)
        });
        let closed = driver
            .close_materialization_callback(&mut lower_ticket, callback_ticket, checkpoint, attempt)
            .expect("retry closes the callback");
        driver
            .lower
            .submit_closed_materialization(lower_ticket, closed)
            .expect("retry submits the closed materialization");
        assert_eq!(counts.materialize_begin.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_call.load(Ordering::Relaxed), 1);
        assert_eq!(counts.materialize_close.load(Ordering::Relaxed), 1);
        let result = driver.finish().complete();
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
        let mut driver = session
            .start::<Domain, _>(
                initialization(scope),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::MaterializationReversedFatal,
                },
            )
            .expect("prepared initialization");
        driver.constrain(
            &TypeKind::Choice(vec![
                TypeKind::GenericParam(first),
                TypeKind::GenericParam(second),
            ]),
            &TypeKind::I32,
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

        match driver.finish().complete() {
            Err(TypeConstraintFailure::FatalSource(error)) => {
                assert_eq!(error.source(), &10);
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
        let mut driver = session
            .start::<Domain, _>(
                initialization(TypeConstraintParameterScope::empty()),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
            )
            .expect("prepared initialization");

        let (ticket, checkpoint) = driver
            .begin_probe_callback(1)
            .expect("driver mints a probe ticket");
        let foreign_ticket = SourceCallbackTicket {
            identity: SourceCallbackTicketIdentity {
                issuer: Arc::new(SourceCallbackTicketIssuer),
                ordinal: 0,
            },
            authority: SourceCallbackAuthority::Probe { source: 1 },
        };
        assert!(matches!(
            driver.close_probe_callback(
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
            .begin_probe_callback(1)
            .expect("foreign close clears the active ticket");
        driver
            .close_probe_callback(
                valid_ticket,
                valid_checkpoint,
                Ok(SourceProbeOutcome::Rejected("protocol")),
            )
            .expect("valid callback can begin after foreign close");
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 2);
        drop(ticket);
        drop(driver);

        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .expect("second candidate session");
        let counts = Arc::new(Counts::default());
        let mut driver = session
            .start::<Domain, _>(
                initialization(TypeConstraintParameterScope::empty()),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
            )
            .expect("second prepared initialization");

        let (ticket, mut checkpoint) = driver
            .begin_probe_callback(1)
            .expect("driver can mint the next generation");
        checkpoint.identity.issuer = Arc::new(SourceCallbackTicketIssuer);
        assert!(matches!(
            driver.close_probe_callback(
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
            .begin_probe_callback(1)
            .expect("foreign checkpoint close clears the active ticket");
        driver
            .close_probe_callback(
                valid_ticket,
                valid_checkpoint,
                Ok(SourceProbeOutcome::Rejected("protocol")),
            )
            .expect("valid callback can begin after foreign checkpoint close");
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn foreign_materialization_authority_closes_once_and_clears_active_state() {
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut work = ResolverWork::new(4_096);
        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .expect("candidate session");
        let counts = Arc::new(Counts::default());
        let mut driver = session
            .start::<Domain, _>(
                initialization(TypeConstraintParameterScope::empty()),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
            )
            .expect("prepared initialization");
        driver
            .probe_prepared_source(prepared(), ConstraintAcceptance::PatternAcceptsActual)
            .expect("prepared source probe");
        let mut materialization = driver
            .lower
            .next_materialization_ticket(&mut driver.context)
            .expect("lower materialization ticket")
            .expect("one materialization ticket");
        let (_valid_ticket, checkpoint) = driver
            .begin_materialization_callback(&mut materialization)
            .expect("driver mints a materialization ticket");
        let foreign_ticket = SourceCallbackTicket {
            identity: SourceCallbackTicketIdentity {
                issuer: Arc::new(SourceCallbackTicketIssuer),
                ordinal: 0,
            },
            authority: SourceCallbackAuthority::Probe { source: 1 },
        };
        assert!(matches!(
            driver.close_materialization_callback(
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
            .begin_probe_callback(1)
            .expect("foreign close clears the active callback ticket");
        driver
            .close_probe_callback(
                probe_ticket,
                probe_checkpoint,
                Ok(SourceProbeOutcome::Rejected("protocol")),
            )
            .expect("a later callback can begin");
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 2);
        drop(driver);

        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .expect("second candidate session");
        let counts = Arc::new(Counts::default());
        let mut driver = session
            .start::<Domain, _>(
                initialization(TypeConstraintParameterScope::empty()),
                Client {
                    counts: Arc::clone(&counts),
                    mode: CallbackMode::Success,
                },
            )
            .expect("second prepared initialization");
        driver
            .probe_prepared_source(prepared(), ConstraintAcceptance::PatternAcceptsActual)
            .expect("second prepared source probe");
        let mut materialization = driver
            .lower
            .next_materialization_ticket(&mut driver.context)
            .expect("second lower materialization ticket")
            .expect("one second materialization ticket");
        let (ticket, mut checkpoint) = driver
            .begin_materialization_callback(&mut materialization)
            .expect("driver mints the next generation");
        checkpoint.identity.issuer = Arc::new(SourceCallbackTicketIssuer);
        assert!(matches!(
            driver.close_materialization_callback(
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
            .begin_probe_callback(1)
            .expect("foreign checkpoint close clears the active callback ticket");
        driver
            .close_probe_callback(
                probe_ticket,
                probe_checkpoint,
                Ok(SourceProbeOutcome::Rejected("protocol")),
            )
            .expect("a later callback can begin");
        assert_eq!(counts.probe_close.load(Ordering::Relaxed), 2);
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
