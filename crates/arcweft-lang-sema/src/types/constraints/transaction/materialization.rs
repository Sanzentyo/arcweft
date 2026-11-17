//! Callback tickets and sealed results for already canonical components.

use super::*;

pub(crate) struct ConstraintMaterialization<D: ConstraintDomain> {
    graph_owner: crate::effect_row::prepared::PreparedEffectGraphOwner,
    next_correlation_ordinal: u32,
    next_materialization_ticket_ordinal: u64,
    materialization_issuer: Arc<MaterializationTicketIssuer>,
    active_materialization: Option<MaterializationTicketIdentity>,
    materialization: VecDeque<MaterializationTicket<D>>,
    materialized: Vec<MaterializedRecord<D>>,
    first_failure: Option<TypeConstraintFailure<D>>,
    first_cycle: Option<super::super::ConstraintGenericParameterId>,
}

impl<D: ConstraintDomain> ConstraintMaterialization<D> {
    pub(in crate::types::constraints::transaction) fn new<A: TypeConstraintAccounting>(
        graph_owner: crate::effect_row::prepared::PreparedEffectGraphOwner,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        context.enter_node()?;
        Ok(Self {
            graph_owner,
            next_correlation_ordinal: 0,
            next_materialization_ticket_ordinal: 0,
            materialization_issuer: Arc::new(MaterializationTicketIssuer),
            active_materialization: None,
            materialization: VecDeque::new(),
            materialized: Vec::new(),
            first_failure: None,
            first_cycle: None,
        })
    }

    pub(in crate::types::constraints::transaction) fn from_component<
        A: TypeConstraintAccounting,
    >(
        component: super::super::CompletedConstraintComponent<D>,
        graph_owner: &crate::effect_row::prepared::PreparedEffectGraphOwner,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        context.enter_node()?;
        let mut owner = Self::new(graph_owner.clone(), context)?;
        owner.push_component(component, context)?;
        Ok(owner)
    }

    /// Validate the callback's prepared relation before borrowing source facts.
    /// This retains identity and canonical values, never the live graph.
    pub(crate) fn validate_graph(
        &self,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
    ) -> Result<(), TypeConstraintError> {
        graph
            .validate_owner(&self.graph_owner)
            .map_err(TypeConstraintError::from)
    }

    pub(in crate::types::constraints::transaction) fn push_component<
        A: TypeConstraintAccounting,
    >(
        &mut self,
        component: super::super::CompletedConstraintComponent<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        context.enter_node()?;
        if component.sources().all().is_empty() {
            self.materialized.push(MaterializedRecord::Sealed {
                component,
                value: D::empty_sealed_branch(),
            });
            return Ok(());
        }
        let next_correlation = self
            .next_correlation_ordinal
            .checked_add(1)
            .ok_or(TypeConstraintAbort::ArithmeticOverflow)?;
        let next_ticket = self
            .next_materialization_ticket_ordinal
            .checked_add(1)
            .ok_or(TypeConstraintAbort::ArithmeticOverflow)?;
        context.enter_node()?;
        let ticket = MaterializationTicket {
            identity: MaterializationTicketIdentity {
                issuer: Arc::clone(&self.materialization_issuer),
                ordinal: self.next_materialization_ticket_ordinal,
            },
            correlation: MaterializationCorrelationOrdinal(self.next_correlation_ordinal),
            component,
            phase: MaterializationTicketPhase::Ready,
        };
        self.materialization.push_back(ticket);
        self.next_correlation_ordinal = next_correlation;
        self.next_materialization_ticket_ordinal = next_ticket;
        Ok(())
    }

    pub(crate) fn record_failure(&mut self, failure: TypeConstraintFailure<D>) {
        if self.first_failure.is_none() {
            self.first_failure = Some(failure);
        }
    }

    pub(in crate::types::constraints::transaction) fn retain_cycle(
        &mut self,
        cycle: Option<super::super::ConstraintGenericParameterId>,
    ) {
        if self.first_cycle.is_none() {
            self.first_cycle = cycle;
        }
    }

    pub(in crate::types::constraints::transaction) fn is_pristine(&self) -> bool {
        self.active_materialization.is_none()
            && self.materialization.is_empty()
            && self.materialized.is_empty()
            && self.first_failure.is_none()
            && self.first_cycle.is_none()
    }

    pub(in crate::types::constraints::transaction) fn discard_pending(&mut self) {
        self.materialization.clear();
        self.materialized.clear();
    }

    #[cfg(test)]
    pub(in crate::types::constraints::transaction) fn records_are_empty(&self) -> bool {
        self.materialized.is_empty()
    }

    #[cfg(test)]
    pub(in crate::types::constraints::transaction) fn pending_len(&self) -> usize {
        self.materialization.len()
    }

    #[cfg(test)]
    pub(in crate::types::constraints::transaction) fn ticket_issuer(
        &self,
    ) -> &Arc<MaterializationTicketIssuer> {
        &self.materialization_issuer
    }

    pub(crate) fn next_materialization_ticket<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Option<MaterializationTicket<D>>, MaterializationImmediateFailure<D>> {
        context
            .check_cancelled()
            .map_err(materialization_immediate)?;
        if self.first_failure.is_some() {
            return Ok(None);
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
        match self.submit_in(ticket, closed) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.record_failure(error.clone().into());
                Err(materialization_immediate(error))
            }
        }
    }

    fn submit_in(
        &mut self,
        ticket: MaterializationTicket<D>,
        closed: ClosedMaterialization<D>,
    ) -> Result<(), TypeConstraintError> {
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
            return Err(TypeConstraintError::Invariant(
                TypeConstraintInvariant::SourceProtocol(
                    TypeConstraintSourceProtocolInvariant::Ticket,
                ),
            ));
        }
        if !matches!(ticket.phase, MaterializationTicketPhase::Closed)
            || !materialization_identity_matches(&ticket.identity, &closed.identity)
        {
            return Err(TypeConstraintError::Invariant(
                TypeConstraintInvariant::SourceProtocol(
                    TypeConstraintSourceProtocolInvariant::Outcome,
                ),
            ));
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
                let source_ordinal = unique_request_ordinal(component.sources().all(), source)?;
                self.materialized.push(MaterializedRecord::Rejected {
                    source_ordinal,
                    correlation,
                    source,
                    cause,
                });
            }
            ClosedMaterializationSubmission::Fatal(error) => {
                if error.phase() != SourcePhase::Materialize {
                    return Err(protocol_error(
                        TypeConstraintSourceProtocolInvariant::WrongPhase,
                    ));
                }
                let source_ordinal =
                    unique_request_ordinal(component.sources().all(), *error.source())?;
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
        context: &mut TypeConstraintContext<'_, A, D>,
        mut compare: impl FnMut(
            &D::SealedBranchValue,
            &D::SealedBranchValue,
            &mut A,
        ) -> Result<bool, TypeConstraintFailure<D>>,
    ) -> Result<super::super::CompletedCandidateAlternatives<D>, TypeConstraintFailure<D>>
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
        let mut candidates: Vec<(
            super::super::CompletedConstraintComponent<D>,
            D::SealedBranchValue,
        )> = Vec::new();
        let mut rejected = Vec::new();
        for record in core::mem::take(&mut self.materialized) {
            match record {
                MaterializedRecord::Sealed { component, value } => {
                    let mut duplicate = false;
                    for (existing_component, existing_value) in &candidates {
                        if existing_component.equal_with(&component, context)? {
                            // The opaque owner observes the same running reservation.
                            // Dispatch itself is bounded even for an empty payload;
                            // any controlled traversal consumes that reservation too.
                            context.enter_node()?;
                            if compare(existing_value, &value, context.accounting_mut())? {
                                duplicate = true;
                                break;
                            }
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
        Ok(super::super::CompletedCandidateAlternatives::new(
            first, remaining,
        ))
    }
}
