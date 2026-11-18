//! One affine callback owner shared by preparation and canonical materialization.

use super::*;

impl<'driver, 'control, D, C> CandidateConstraintCallbacks<'driver, 'control, D, C>
where
    D: ConstraintDomain,
    C: TypeConstraintClient<D>,
{
    pub(super) fn new(
        context: &'driver mut TypeConstraintContext<
            'control,
            CandidateConstraintWorkSession<'control>,
            D,
        >,
        authority: PreparedConstraintAuthority,
        client: C,
    ) -> Result<Self, crate::types::constraints::TypeConstraintError> {
        context.enter_node()?;
        Ok(Self {
            context,
            authority,
            client,
            ticket_issuer: Arc::new(SourceCallbackTicketIssuer),
            next_ticket_ordinal: 0,
            active_ticket: None,
        })
    }

    /// Close the actual callback client once, even after a protocol failure.
    pub(super) fn finish_callbacks(&mut self) -> Result<(), TypeConstraintFailure<D>> {
        let active = self.active_ticket.is_some();
        let close = self.client.finish().map_err(Self::checkpoint_failure);
        if active {
            return Err(Self::protocol_failure(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        close
    }

    pub(super) fn finish_materialization(
        mut self,
        mut owner: ConstraintMaterialization<D>,
    ) -> Result<CompletedCandidateAlternatives<D>, TypeConstraintFailure<D>> {
        if let Err(failure) = self.materialize_all(&mut owner) {
            owner.record_failure(failure);
        }
        if let Err(failure) = self.finish_callbacks() {
            owner.record_failure(failure);
        }
        owner.finish(self.context, |left, right, work| {
            self.client
                .sealed_branches_equal(left, right, work)
                .map_err(|failure| match failure {
                    SealedBranchComparisonFailure::Constraint(error) => error.into(),
                    SealedBranchComparisonFailure::Client(error) => {
                        TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Client(
                            error,
                        ))
                    }
                })
        })
    }

    fn materialize_all(
        &mut self,
        owner: &mut ConstraintMaterialization<D>,
    ) -> Result<(), TypeConstraintFailure<D>> {
        loop {
            self.context.check_cancelled()?;
            self.client
                .with_effect_relation(
                    CandidateConstraintEffectAccess {
                        authority: &self.authority,
                    },
                    |graph| owner.validate_graph(graph.read()),
                )
                .map_err(Self::checkpoint_failure)??;
            let Some(mut ticket) = owner.next_materialization_ticket(self.context)? else {
                break;
            };
            let (callback, mut checkpoint) =
                self.begin_materialization_callback(owner, &mut ticket)?;
            let attempt = self.with_callback(|client, work| {
                client.materialize_sources(ticket.requests(), &mut checkpoint.checkpoint, work)
            });
            let closed =
                self.close_materialization_callback(&mut ticket, callback, checkpoint, attempt)?;
            owner.submit_closed_materialization(ticket, closed)?;
        }
        Ok(())
    }

    pub(super) fn with_callback<R>(
        &mut self,
        operation: impl FnOnce(&mut C, &mut CandidateConstraintWorkSession<'_>) -> R,
    ) -> R {
        let session = self.context.accounting_mut();
        operation(&mut self.client, session)
    }

    pub(super) fn protocol_failure(
        kind: TypeConstraintSourceProtocolInvariant,
    ) -> TypeConstraintFailure<D> {
        TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Constraint(
            TypeConstraintInvariant::SourceProtocol(kind),
        ))
    }

    pub(super) fn checkpoint_failure(
        failure: SourceCheckpointFailure<D>,
    ) -> TypeConstraintFailure<D> {
        match failure {
            SourceCheckpointFailure::Constraint(error) => error.into(),
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
            SourceCheckpointFailure::Constraint(error) => match error {
                crate::types::constraints::TypeConstraintError::Abort(error) => {
                    MaterializationImmediateFailure::Abort(error)
                }
                crate::types::constraints::TypeConstraintError::Invariant(error) => {
                    MaterializationImmediateFailure::Invariant(
                        TypeConstraintFailureInvariant::Constraint(error),
                    )
                }
                crate::types::constraints::TypeConstraintError::Rejected(_) => {
                    Self::materialization_protocol_failure(
                        TypeConstraintSourceProtocolInvariant::Outcome,
                    )
                }
            },
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
            &mut TypeConstraintContext<'control, CandidateConstraintWorkSession<'control>, D>,
        ) -> Result<(), crate::types::constraints::TypeConstraintError>,
    ) -> Result<SourceCallbackTicket<D>, TypeConstraintFailure<D>> {
        let ordinal = self.reserve_ticket_ordinal(charge)?;
        Ok(self.ticket_from_ordinal(ordinal, authority))
    }

    fn reserve_ticket_ordinal(
        &mut self,
        charge: impl FnOnce(
            &mut TypeConstraintContext<'control, CandidateConstraintWorkSession<'control>, D>,
        ) -> Result<(), crate::types::constraints::TypeConstraintError>,
    ) -> Result<u64, TypeConstraintFailure<D>> {
        let ordinal = self.next_ticket_ordinal;
        let next_ordinal = ordinal.checked_add(1).ok_or(TypeConstraintFailure::Abort(
            TypeConstraintAbort::ArithmeticOverflow,
        ))?;
        charge(self.context).map_err(TypeConstraintFailure::from)?;
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
        disposition: ProbeCheckpointDisposition,
    ) -> Result<Option<D::ProbePreparation>, SourceCheckpointFailure<D>> {
        let result = self.with_callback(|client, work| {
            client.close_probe_checkpoint(work, checkpoint, disposition)
        });
        self.active_ticket = None;
        result
    }

    fn close_materialization_checkpoint_once(
        &mut self,
        checkpoint: C::MaterializationCheckpoint,
        sealed: Option<C::PreparedSealedBranchValue>,
    ) -> Result<Option<D::SealedBranchValue>, SourceCheckpointFailure<D>> {
        let result = self.with_callback(|client, work| {
            client.close_materialization_checkpoint(work, checkpoint, sealed)
        });
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

    pub(super) fn begin_probe_callback(
        &mut self,
        source: ConstraintSourceId<D::Source>,
        reads: Option<&PreparedEffectReadScope>,
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
            .with_callback(|client, work| client.open_probe_checkpoint(work, source, reads))
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

    pub(super) fn close_probe_callback(
        &mut self,
        ticket: SourceCallbackTicket<D>,
        checkpoint: BoundSourceCheckpoint<C::ProbeCheckpoint>,
        attempt: Result<SourceProbeOutcome<D>, SourceCallbackFailure<D>>,
    ) -> Result<ProbeSubmission<D>, TypeConstraintFailure<D>> {
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
                if D::client_invariant_source(invariant) != source.local() {
                    Some(TypeConstraintSourceProtocolInvariant::WrongSource)
                } else {
                    None
                }
            }
            _ => None,
        };
        let disposition = if authority_failure.is_none()
            && invalid.is_none()
            && matches!(&attempt, Ok(SourceProbeOutcome::Accepted(_)))
        {
            ProbeCheckpointDisposition::Retain
        } else {
            ProbeCheckpointDisposition::Discard
        };
        let close = self.close_probe_checkpoint_once(checkpoint.checkpoint, disposition);
        if let Some(failure) = authority_failure {
            return Err(Self::protocol_failure(failure));
        }
        if let Some(invalid) = invalid {
            return Err(Self::protocol_failure(invalid));
        }
        let preparation = close.map_err(Self::checkpoint_failure)?;
        if preparation.is_some() != (disposition == ProbeCheckpointDisposition::Retain) {
            return Err(Self::protocol_failure(
                TypeConstraintSourceProtocolInvariant::Outcome,
            ));
        }
        match attempt {
            Ok(SourceProbeOutcome::Accepted(contribution)) => {
                let preparation = preparation.ok_or_else(|| {
                    Self::protocol_failure(TypeConstraintSourceProtocolInvariant::Outcome)
                })?;
                Ok(ProbeSubmission::Accepted(
                    contribution.close_preparation(preparation),
                ))
            }
            Ok(SourceProbeOutcome::Rejected(cause)) => Ok(ProbeSubmission::Rejected(cause)),
            Err(SourceCallbackFailure::Constraint(error)) => Err(error.into()),
            Err(SourceCallbackFailure::Fatal(error)) => {
                Err(TypeConstraintFailure::FatalSource(error))
            }
            Err(SourceCallbackFailure::Abort(error)) => Err(TypeConstraintFailure::Abort(error)),
            Err(SourceCallbackFailure::Invariant(invariant)) => Err(
                TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Client(invariant)),
            ),
        }
    }

    pub(super) fn begin_materialization_callback(
        &mut self,
        owner: &ConstraintMaterialization<D>,
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
        owner
            .validate_materialization_callback_begin(materialization)
            .map_err(Self::materialization_protocol_failure)?;
        let ordinal = self
            .reserve_ticket_ordinal(|context| context.charge_materialization())
            .map_err(materialization_immediate_from_failure)?;
        let binding = materialization
            .bind_callback()
            .map_err(Self::materialization_protocol_failure)?;
        let checkpoint = self
            .with_callback(|client, work| {
                client.open_materialization_checkpoint(work, binding.sources())
            })
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

    pub(super) fn close_materialization_callback(
        &mut self,
        materialization: &mut MaterializationTicket<D>,
        ticket: SourceCallbackTicket<D>,
        checkpoint: BoundSourceCheckpoint<C::MaterializationCheckpoint>,
        attempt: Result<
            MaterializationOutcome<
                ConstraintSourceId<D::Source>,
                C::PreparedSealedBranchValue,
                D::SourceErrorCause,
            >,
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
                .sources()
                .iter()
                .any(|source| source.local() == D::client_invariant_source(invariant)))
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
            Err(SourceCallbackFailure::Constraint(error)) => {
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
                Err(match error {
                    crate::types::constraints::TypeConstraintError::Abort(error) => {
                        MaterializationImmediateFailure::Abort(error)
                    }
                    crate::types::constraints::TypeConstraintError::Invariant(error) => {
                        MaterializationImmediateFailure::Invariant(
                            TypeConstraintFailureInvariant::Constraint(error),
                        )
                    }
                    crate::types::constraints::TypeConstraintError::Rejected(_) => {
                        Self::materialization_protocol_failure(
                            TypeConstraintSourceProtocolInvariant::Outcome,
                        )
                    }
                })
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
}
