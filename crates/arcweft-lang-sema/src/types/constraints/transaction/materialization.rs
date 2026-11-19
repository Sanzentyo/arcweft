//! Callback tickets and sealed results for already canonical components.

use super::super::{CompletedConstraintApplication, CompletedConstraintComponent};
use super::*;

/// The original callback close remains available while its canonical source
/// and branch are being selected. The semantic component never owns it.
pub(crate) struct RetainedSourcePreparations<D: ConstraintDomain> {
    rows: Box<[SourcePreparation<D>]>,
}

pub(crate) struct OwnedSourcePreparation<D: ConstraintDomain> {
    pub(crate) source: ConstraintSourceId<D::Source>,
    pub(crate) preparation: D::ProbePreparation,
}

pub(crate) enum SourcePreparationTakeError<E> {
    Shared,
    Control(E),
}

pub(crate) struct SourcePreparationTakeFailure<D: ConstraintDomain, E> {
    pub(crate) source: ConstraintSourceId<D::Source>,
    pub(crate) error: SourcePreparationTakeError<E>,
    pub(crate) retained: RetainedSourcePreparations<D>,
}

struct SourcePreparation<D: ConstraintDomain> {
    source: ConstraintSourceId<D::Source>,
    observation: Arc<SourceProbeObservation<D>>,
}

impl<D: ConstraintDomain> SourcePreparation<D> {
    fn preparation(&self) -> &D::ProbePreparation {
        self.observation
            .preparation()
            .expect("closing the source already required the original callback preparation")
    }
}

pub(super) struct MaterializationInput<D: ConstraintDomain> {
    pub(super) component: super::super::CompletedConstraintComponent<D>,
    pub(super) preparations: RetainedSourcePreparations<D>,
}

/// A closed source and its callback preparation are appended by the same
/// original probe. Only the semantic trace survives component publication.
pub(super) struct ClosedSourceMaterialization<D: ConstraintDomain> {
    sources: Box<[ClosedConstraintProbe<D>]>,
    preparations: RetainedSourcePreparations<D>,
}

impl<D: ConstraintDomain> ConstraintSourceLineage<D> {
    pub(super) fn close_trace<A: TypeConstraintAccounting, L>(
        &self,
        branch: &ConstraintInferenceBranch<D, L>,
        projection: &mut ConstraintEffectProjection<D>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ClosedSourceMaterialization<D>, TypeConstraintError> {
        projection.owner().require_branch(branch)?;
        let mut sources = Vec::new();
        let mut rows = Vec::new();
        for probe in &self.probe_trace {
            context.enter_node()?;
            context.enter_node()?;
            let closed = probe.clone().close(branch, projection, graph, context)?;
            context.enter_node()?;
            rows.push(SourcePreparation {
                source: closed.source(),
                observation: Arc::clone(&probe.branch),
            });
            sources.push(closed);
        }
        Ok(ClosedSourceMaterialization {
            sources: sources.into_boxed_slice(),
            preparations: RetainedSourcePreparations {
                rows: rows.into_boxed_slice(),
            },
        })
    }
}

impl<D: ConstraintDomain> ClosedSourceMaterialization<D> {
    pub(super) fn sources(&self) -> &[ClosedConstraintProbe<D>] {
        &self.sources
    }

    pub(super) fn finish(
        self,
        selected: ConstraintApplicationId,
        selected_domain: D::Application,
        completed: BTreeMap<
            D::Application,
            CompletedConstraintApplication<D::Application, D::Projection>,
        >,
        applications: Arc<ConstraintApplicationInventory<D>>,
    ) -> MaterializationInput<D> {
        MaterializationInput {
            component: CompletedConstraintComponent::new(
                selected_domain,
                completed,
                ClosedConstraintSourceTrace::new(selected, applications, self.sources),
            ),
            preparations: self.preparations,
        }
    }
}

impl<D: ConstraintDomain> RetainedSourcePreparations<D> {
    pub(super) fn requests<'h>(
        &'h self,
        component: &'h super::super::CompletedConstraintComponent<D>,
    ) -> impl ExactSizeIterator<Item = MaterializedSourceRequest<'h, D>> + 'h {
        debug_assert_eq!(self.rows.len(), component.sources().all().len());
        component
            .sources()
            .all()
            .iter()
            .zip(self.rows.iter())
            .map(|(source, row)| {
                debug_assert!(source.source() == row.source);
                MaterializedSourceRequest::for_source(component, source, row.preparation())
            })
    }

    #[cfg(test)]
    pub(crate) fn into_owned<C: crate::effect_row::DecisionControl>(
        self,
        control: &mut C,
    ) -> Result<Box<[OwnedSourcePreparation<D>]>, SourcePreparationTakeFailure<D, C::Error>> {
        self.into_owned_where(|_| true, control)
    }

    /// Take only the requested callback allocations in source order. A failed
    /// uniqueness or accounting check returns the entire original batch.
    pub(crate) fn into_owned_selected<V, C: crate::effect_row::DecisionControl>(
        self,
        selected: &BTreeMap<ConstraintSourceId<D::Source>, V>,
        control: &mut C,
    ) -> Result<Box<[OwnedSourcePreparation<D>]>, SourcePreparationTakeFailure<D, C::Error>> {
        self.into_owned_where(|source| selected.contains_key(&source), control)
    }

    fn into_owned_where<C: crate::effect_row::DecisionControl>(
        mut self,
        selected: impl Fn(ConstraintSourceId<D::Source>) -> bool,
        control: &mut C,
    ) -> Result<Box<[OwnedSourcePreparation<D>]>, SourcePreparationTakeFailure<D, C::Error>> {
        use crate::effect_row::DecisionWork;
        for row in &mut self.rows {
            let source = row.source;
            let error = match control.charge(DecisionWork::Visit).and_then(|()| {
                if selected(source) {
                    control.charge(DecisionWork::Node).map(|()| true)
                } else {
                    Ok(false)
                }
            }) {
                Err(error) => SourcePreparationTakeError::Control(error),
                Ok(false) => continue,
                Ok(true)
                    if Arc::get_mut(&mut row.observation)
                        .is_some_and(SourceProbeObservation::has_unique_preparation) =>
                {
                    continue;
                }
                Ok(true) => SourcePreparationTakeError::Shared,
            };
            return Err(SourcePreparationTakeFailure {
                source,
                error,
                retained: self,
            });
        }
        // No Arc or Weak escapes a uniquely preflighted row. Only this batch
        // owns the observations and payloads, so no owner can appear here.
        Ok(self
            .rows
            .into_vec()
            .into_iter()
            .filter(|row| selected(row.source))
            .map(|row| {
                let observation =
                    Arc::into_inner(row.observation).expect("checked sole observation owner");
                OwnedSourcePreparation {
                    source: row.source,
                    preparation: observation.into_unique_preparation(),
                }
            })
            .collect())
    }
}

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
        input: MaterializationInput<D>,
        graph_owner: &crate::effect_row::prepared::PreparedEffectGraphOwner,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        context.enter_node()?;
        let mut owner = Self::new(graph_owner.clone(), context)?;
        owner.push_component(input, context)?;
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
        input: MaterializationInput<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        context.enter_node()?;
        if input.component.sources().all().is_empty() {
            self.materialized.push(MaterializedRecord::Sealed {
                input,
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
            input,
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
            correlation, input, ..
        } = ticket;
        match closed.submission {
            ClosedMaterializationSubmission::Sealed(value) => {
                self.materialized
                    .push(MaterializedRecord::Sealed { input, value });
            }
            ClosedMaterializationSubmission::Rejected { source, cause } => {
                let source_ordinal =
                    unique_request_ordinal(input.component.sources().all(), source)?;
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
                    unique_request_ordinal(input.component.sources().all(), *error.source())?;
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
        let mut candidates: Vec<SolvedCandidate<D>> = Vec::new();
        let mut rejected = Vec::new();
        for record in core::mem::take(&mut self.materialized) {
            match record {
                MaterializedRecord::Sealed { input, value } => {
                    let mut duplicate = false;
                    for existing in &candidates {
                        if existing.component.equal_with(&input.component, context)? {
                            // The opaque owner observes the same running reservation.
                            // Dispatch itself is bounded even for an empty payload;
                            // any controlled traversal consumes that reservation too.
                            context.enter_node()?;
                            if compare(&existing.sealed_branch, &value, context.accounting_mut())? {
                                duplicate = true;
                                break;
                            }
                        }
                    }
                    if !duplicate {
                        candidates.push(SolvedCandidate {
                            component: input.component,
                            sealed_branch: value,
                            source_preparations: input.preparations,
                        });
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
        let Some(first) = candidates.pop() else {
            return Err(TypeConstraintError::Rejected(TypeConstraintRejection::Mismatch).into());
        };
        context.check_cancelled()?;
        Ok(super::super::CompletedCandidateAlternatives::new(
            first, candidates,
        ))
    }
}
