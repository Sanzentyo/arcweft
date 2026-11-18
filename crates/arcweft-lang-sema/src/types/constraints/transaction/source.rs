//! Source trace transitions from probing to normalized materialization evidence.

use std::sync::{Arc, OnceLock};

use crate::types::{PreparedEffects, TypeKind, TypeProjectionControl};

use super::super::context::{TypeConstraintAccounting, TypeConstraintContext};
use super::super::normalization::{ConstraintClosedEffects, close_type_operand};
use super::{ConstraintApplicationId, ConstraintApplicationInventory};
use super::{ConstraintInferenceBranch, ConstraintOperand};

#[cfg(test)]
mod canonical_tests;
#[cfg(test)]
mod tests;
use super::super::{ConstraintSourceValueOwner, OriginalProducerContext};
use super::{
    CheckedConstraintSourceProjection, ConstraintDomain, ConstraintResultProjection,
    ConstraintSourceId, PreparedConstraintSourceProjection, PreparedSourceOrdinal,
    StoredSourceSelection, TypeConstraintError, TypeConstraintInvariant,
    TypeConstraintSourceProtocolInvariant, protocol_error,
};

/// The original branch allocation also owns its closed source preparation.
/// Only a successful affine callback close may initialize the payload; a
/// completed source trace cannot be constructed from an uninitialized record.
pub(in crate::types::constraints) struct SourceProbeObservation<D: ConstraintDomain> {
    branch: D::ProbeSemanticBranch,
    preparation: OnceLock<RetainedProbePreparation<D>>,
}

enum RetainedProbePreparation<D: ConstraintDomain> {
    Owned(D::ProbePreparation),
    Shared(Arc<D::ProbePreparation>),
}

impl<D: ConstraintDomain> SourceProbeObservation<D> {
    pub(in crate::types::constraints) fn new(branch: D::ProbeSemanticBranch) -> Arc<Self> {
        Arc::new(Self {
            branch,
            preparation: OnceLock::new(),
        })
    }

    pub(in crate::types::constraints) fn branch(&self) -> &D::ProbeSemanticBranch {
        &self.branch
    }

    pub(in crate::types::constraints) fn preparation(&self) -> Option<&D::ProbePreparation> {
        self.preparation.get().map(|preparation| match preparation {
            RetainedProbePreparation::Owned(value) => value,
            RetainedProbePreparation::Shared(value) => value.as_ref(),
        })
    }

    pub(in crate::types::constraints) fn retain_owned(
        &self,
        preparation: D::ProbePreparation,
    ) -> Result<(), super::TypeConstraintError> {
        self.preparation
            .set(RetainedProbePreparation::Owned(preparation))
            .map_err(|_| protocol_error(TypeConstraintSourceProtocolInvariant::Outcome))
    }

    pub(in crate::types::constraints) fn retain_shared(
        &self,
        preparation: Arc<D::ProbePreparation>,
    ) -> Result<(), super::TypeConstraintError> {
        self.preparation
            .set(RetainedProbePreparation::Shared(preparation))
            .map_err(|_| protocol_error(TypeConstraintSourceProtocolInvariant::Outcome))
    }

    #[cfg(test)]
    pub(in crate::types::constraints) fn closed_for_test(
        branch: D::ProbeSemanticBranch,
        preparation: D::ProbePreparation,
    ) -> Arc<Self> {
        let observation = Self::new(branch);
        observation
            .retain_owned(preparation)
            .unwrap_or_else(|_| unreachable!("a fresh fixture observation is uninitialized"));
        observation
    }
}

// Preparation is retained input, not normalized semantic equality evidence.
impl<D: ConstraintDomain> PartialEq for SourceProbeObservation<D> {
    fn eq(&self, other: &Self) -> bool {
        self.branch == other.branch
    }
}
impl<D: ConstraintDomain> Eq for SourceProbeObservation<D> {}

pub(in crate::types::constraints) struct ActiveConstraintProbe<D: ConstraintDomain> {
    pub(super) source: ConstraintSourceId<D::Source>,
    pub(super) source_ordinal: PreparedSourceOrdinal,
    pub(super) branch: Arc<SourceProbeObservation<D>>,
    pub(super) selection: StoredSourceSelection<D>,
    pub(super) prepared_source_projection: PreparedConstraintSourceProjection,
    pub(super) value_expected: Option<ConstraintOperand<D>>,
    pub(super) actual: ConstraintOperand<D>,
}

/// The selected source's type and constant references are closed against the
/// candidate application. Declaration-Free references and function-local
/// binders retain their own owners. Residual callee quantifiers belong to the
/// continuation result, not to an already evaluated operand.
pub(crate) struct ClosedConstraintProbe<D: ConstraintDomain> {
    completion: Arc<ConstraintClosedEffects<D>>,
    source: ConstraintSourceId<D::Source>,
    original: OriginalProducerContext,
    retained_value: Option<Arc<D::SourceValue>>,
    source_ordinal: PreparedSourceOrdinal,
    branch: D::ProbeSemanticBranch,
    selection: ClosedSourceSelection<D>,
    prepared_source_projection: PreparedConstraintSourceProjection,
    actual: ConstraintOperand<D, TypeKind>,
    source_projection: CheckedConstraintSourceProjection,
}

/// A completed component trace keeps the application selected by its result.
/// Consumers of that application's argument mapping see only its own sources;
/// nested application evidence remains owned by the same trace.
pub(crate) struct ClosedConstraintSourceTrace<D: ConstraintDomain> {
    application: ConstraintApplicationId,
    applications: Arc<ConstraintApplicationInventory<D>>,
    sources: Box<[ClosedConstraintProbe<D>]>,
}

impl<D: ConstraintDomain> ClosedConstraintSourceTrace<D> {
    pub(in crate::types::constraints) fn equal_with<A: TypeConstraintAccounting>(
        &self,
        other: &Self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<bool, TypeConstraintError> {
        if self.domain_application(self.application) != other.domain_application(other.application)
            || self.sources.len() != other.sources.len()
        {
            return Ok(false);
        }
        for (left, right) in self.sources.iter().zip(&other.sources) {
            context.enter_node()?;
            if self.domain_application(left.source.application())
                != other.domain_application(right.source.application())
                || left.source.local() != right.source.local()
                || left.original != right.original
                || left.source_ordinal.ordinal != right.source_ordinal.ordinal
                || left.branch != right.branch
                || !self.same_result_origin(left, other, right)
                || left.prepared_source_projection != right.prepared_source_projection
                || !super::super::normalization::completed_types_equal(
                    left.actual.value(),
                    right.actual.value(),
                    context,
                )?
            {
                return Ok(false);
            }
            match (&left.selection, &right.selection) {
                (ClosedSourceSelection::Unchecked, ClosedSourceSelection::Unchecked) => {}
                (
                    ClosedSourceSelection::Checked {
                        alternative: left_alternative,
                        evidence: left_evidence,
                        expected: left_expected,
                    },
                    ClosedSourceSelection::Checked {
                        alternative: right_alternative,
                        evidence: right_evidence,
                        expected: right_expected,
                    },
                ) if left_alternative == right_alternative
                    && left_evidence == right_evidence
                    && super::super::normalization::completed_types_equal(
                        left_expected.value(),
                        right_expected.value(),
                        context,
                    )? => {}
                _ => return Ok(false),
            }
            // The lower seal derives this constructor from the actual above.
            // Any map key it contains has already been admitted by that fold.
            if left.source_projection != right.source_projection {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(in crate::types::constraints) fn new(
        application: ConstraintApplicationId,
        applications: Arc<ConstraintApplicationInventory<D>>,
        sources: Box<[ClosedConstraintProbe<D>]>,
    ) -> Self {
        Self {
            application,
            applications,
            sources,
        }
    }

    pub(crate) fn selected(&self) -> impl Iterator<Item = &ClosedConstraintProbe<D>> {
        self.sources
            .iter()
            .filter(|source| source.source().application() == self.application)
    }

    pub(crate) fn all(&self) -> &[ClosedConstraintProbe<D>] {
        &self.sources
    }

    pub(in crate::types::constraints) fn domain_application(
        &self,
        opening: ConstraintApplicationId,
    ) -> D::Application {
        self.applications
            .require_application(opening)
            .expect("completed trace retains its admitted application authority")
            .application()
    }

    fn same_result_origin(
        &self,
        left: &ClosedConstraintProbe<D>,
        other: &Self,
        right: &ClosedConstraintProbe<D>,
    ) -> bool {
        match (
            left.actual.associated_result(),
            right.actual.associated_result(),
        ) {
            (None, None) => true,
            (Some(left), Some(right)) => {
                self.domain_application(left.application())
                    == other.domain_application(right.application())
                    && left.key() == right.key()
            }
            (None, Some(_)) | (Some(_), None) => false,
        }
    }
}

impl<D: ConstraintDomain> PartialEq for ClosedConstraintSourceTrace<D> {
    fn eq(&self, other: &Self) -> bool {
        self.domain_application(self.application) == other.domain_application(other.application)
            && self.sources.len() == other.sources.len()
            && self
                .sources
                .iter()
                .zip(&other.sources)
                .all(|(left, right)| {
                    self.domain_application(left.source.application())
                        == other.domain_application(right.source.application())
                        && left.same_observation_except_origin(right)
                        && self.same_result_origin(left, other, right)
                })
    }
}

impl<D: ConstraintDomain> Eq for ClosedConstraintSourceTrace<D> {}

pub(crate) enum ClosedSourceSelection<D: ConstraintDomain> {
    Unchecked,
    Checked {
        alternative: D::AlternativeIndex,
        evidence: Arc<D::CheckedEvidence>,
        expected: ConstraintOperand<D, TypeKind>,
    },
}

impl<D: ConstraintDomain> ActiveConstraintProbe<D> {
    pub(in crate::types::constraints::transaction) fn original_query_types(
        &self,
    ) -> impl Iterator<Item = &TypeKind<crate::types::PreparedEffects>> {
        std::iter::once(self.actual.value())
            .chain(self.value_expected.as_ref().map(ConstraintOperand::value))
    }

    pub(super) const fn source(&self) -> ConstraintSourceId<D::Source> {
        self.source
    }

    pub(super) const fn ordinal(&self) -> PreparedSourceOrdinal {
        self.source_ordinal
    }

    pub(super) fn close<A: TypeConstraintAccounting, L>(
        self,
        path: &ConstraintInferenceBranch<D, L>,
        effects: &Arc<ConstraintClosedEffects<D>>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ClosedConstraintProbe<D>, TypeConstraintError> {
        if self.branch.preparation().is_none() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            ));
        }
        effects.require_branch(path)?;
        path.projection_view()
            .applications()
            .require_application(self.source.application())?;
        if self.source_ordinal.application != self.source.application() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            ));
        }
        let (original, retained_value) = self.actual.source_origin(self.source)?;
        let original = original.clone();
        let retained_value = retained_value.map(Arc::clone);
        let projected = close_type_operand(self.actual.view(), path, effects, graph, context)?;
        let prepared_projection = CheckedConstraintSourceProjection::derive(
            self.prepared_source_projection,
            projected.value(),
        )
        .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::Outcome))?;
        let actual = projected
            .view()
            .close_canonical(path, effects, graph, context)?;
        let source_projection = CheckedConstraintSourceProjection::derive(
            self.prepared_source_projection,
            actual.value(),
        )
        .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::Outcome))?;
        let selection = match (self.selection, self.value_expected) {
            (StoredSourceSelection::Unchecked, None) => ClosedSourceSelection::Unchecked,
            (
                StoredSourceSelection::Checked {
                    alternative,
                    evidence,
                },
                Some(expected),
            ) => {
                let value_expected =
                    close_type_operand(expected.view(), path, effects, graph, context)?;
                let evidence = D::project_checked_evidence(evidence.as_ref(), actual.value())
                    .ok_or_else(|| {
                        protocol_error(TypeConstraintSourceProtocolInvariant::InvalidEvidence)
                    })?;
                ClosedSourceSelection::Checked {
                    alternative,
                    evidence: Arc::new(evidence),
                    expected: prepared_projection
                        .compose_operand(&projected.view(), &value_expected.view())?
                        .view()
                        .close_canonical(path, effects, graph, context)?,
                }
            }
            _ => {
                return Err(protocol_error(
                    TypeConstraintSourceProtocolInvariant::Outcome,
                ));
            }
        };
        context.enter_node()?;
        Ok(ClosedConstraintProbe {
            completion: Arc::clone(effects),
            source: self.source,
            original,
            retained_value,
            source_ordinal: self.source_ordinal,
            branch: self.branch.branch().clone(),
            selection,
            prepared_source_projection: self.prepared_source_projection,
            actual,
            source_projection,
        })
    }
}

impl<D: ConstraintDomain> ClosedConstraintProbe<D> {
    pub(in crate::types::constraints) fn retained_value(&self) -> Option<&D::SourceValue> {
        self.retained_value.as_deref()
    }

    /// The expression checker operates in the source's Γ. Move the closed
    /// requirement back to those frames before embedding it as prepared rows.
    pub(in crate::types::constraints) fn prepare_expected<
        C: TypeProjectionControl<Error = TypeConstraintError>,
    >(
        &self,
        control: &mut C,
    ) -> Result<Option<TypeKind<PreparedEffects>>, TypeConstraintError> {
        let Some(expected) = self.expected_operand() else {
            return Ok(None);
        };
        let gamma = self.original.gamma();
        if gamma == expected.scope() {
            return Ok(Some(
                expected.value().prepare_effects(gamma.shape(), control)?,
            ));
        }
        let value = gamma.transport_canonical_type(expected.scope(), expected.value(), control)?;
        Ok(Some(value.prepare_effects(gamma.shape(), control)?))
    }

    /// Rechecks carry their exact source and original Γ. Resolve any retained
    /// effect terms through the frozen principal, then compare in the frames
    /// of the completed source. This operation cannot add terms to the graph.
    pub(in crate::types::constraints) fn canonicalize_observed_type<
        C: TypeProjectionControl<Error = TypeConstraintError>,
    >(
        &self,
        observed: &dyn ConstraintSourceValueOwner<D>,
        control: &mut C,
    ) -> Result<TypeKind, TypeConstraintError> {
        control.check()?;
        if observed.source() != self.source {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::WrongSource,
            ));
        }
        if observed.probe_receipt().is_some() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::WrongPhase,
            ));
        }
        if observed.original_context() != &self.original {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        let gamma = self.original.gamma();
        let canonical =
            self.completion
                .canonicalize(observed.value_type(), gamma.shape(), control)?;
        if self.actual.scope() == gamma {
            return Ok(canonical);
        }
        Ok(self
            .actual
            .scope()
            .transport_canonical_type(gamma, &canonical, control)?)
    }

    pub(in crate::types::constraints::transaction) fn require_completion(
        &self,
        owner: &Arc<ConstraintClosedEffects<D>>,
    ) -> Result<(), TypeConstraintError> {
        if !Arc::ptr_eq(&self.completion, owner)
            || self
                .expected_operand()
                .is_some_and(|expected| expected.scope() != self.actual.scope())
        {
            return Err(super::TypeConstraintInvariant::OriginalProducer.into());
        }
        Ok(())
    }

    pub(super) fn actual_operand(&self) -> &ConstraintOperand<D, TypeKind> {
        &self.actual
    }

    pub(super) fn expected_operand(&self) -> Option<&ConstraintOperand<D, TypeKind>> {
        match &self.selection {
            ClosedSourceSelection::Unchecked => None,
            ClosedSourceSelection::Checked { expected, .. } => Some(expected),
        }
    }

    /// Application identity is compared by the owning trace. A replay has a
    /// fresh opening, but must preserve this source's schema and observation.
    fn same_observation(&self, other: &Self) -> bool {
        self.same_observation_except_origin(other)
            && self.actual.associated_result() == other.actual.associated_result()
    }

    fn same_observation_except_origin(&self, other: &Self) -> bool {
        self.source.local() == other.source.local()
            && self.original == other.original
            && self.source_ordinal.ordinal == other.source_ordinal.ordinal
            && self.branch == other.branch
            && self.selection == other.selection
            && self.prepared_source_projection == other.prepared_source_projection
            && self.actual.value() == other.actual.value()
            && self.source_projection == other.source_projection
    }
    pub(crate) const fn source(&self) -> ConstraintSourceId<D::Source> {
        self.source
    }
    pub(in crate::types::constraints) const fn source_ref(&self) -> &ConstraintSourceId<D::Source> {
        &self.source
    }
    pub(super) const fn ordinal(&self) -> PreparedSourceOrdinal {
        self.source_ordinal
    }
    pub(in crate::types::constraints) fn branch(&self) -> &D::ProbeSemanticBranch {
        &self.branch
    }
    pub(crate) fn actual(&self) -> &TypeKind {
        self.actual.value()
    }
    pub(crate) fn final_expected(&self) -> Option<&TypeKind> {
        match &self.selection {
            ClosedSourceSelection::Unchecked => None,
            ClosedSourceSelection::Checked { expected, .. } => Some(expected.value()),
        }
    }
    pub(crate) const fn selection(&self) -> &ClosedSourceSelection<D> {
        &self.selection
    }
    pub(crate) const fn prepared_source_projection(&self) -> PreparedConstraintSourceProjection {
        self.prepared_source_projection
    }
    pub(crate) const fn source_projection(&self) -> &CheckedConstraintSourceProjection {
        &self.source_projection
    }

    pub(crate) fn result_origin(&self) -> Option<ConstraintResultProjection<D>> {
        self.actual.associated_result()
    }
}

impl<D: ConstraintDomain> ClosedSourceSelection<D> {
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

impl<D: ConstraintDomain> Clone for ActiveConstraintProbe<D> {
    fn clone(&self) -> Self {
        Self {
            source: self.source,
            source_ordinal: self.source_ordinal,
            branch: Arc::clone(&self.branch),
            selection: self.selection.clone(),
            prepared_source_projection: self.prepared_source_projection,
            value_expected: self.value_expected.clone(),
            actual: self.actual.clone(),
        }
    }
}

impl<D: ConstraintDomain> PartialEq for ActiveConstraintProbe<D> {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.source_ordinal == other.source_ordinal
            && self.branch == other.branch
            && self.selection == other.selection
            && self.prepared_source_projection == other.prepared_source_projection
            && self.actual.associated_result() == other.actual.associated_result()
            && self.value_expected.as_ref().map(ConstraintOperand::value)
                == other.value_expected.as_ref().map(ConstraintOperand::value)
            && self.actual.value() == other.actual.value()
    }
}

impl<D: ConstraintDomain> Eq for ActiveConstraintProbe<D> {}

impl<D: ConstraintDomain> Clone for ClosedSourceSelection<D> {
    fn clone(&self) -> Self {
        match self {
            Self::Unchecked => Self::Unchecked,
            Self::Checked {
                alternative,
                evidence,
                expected,
            } => Self::Checked {
                alternative: *alternative,
                evidence: Arc::clone(evidence),
                expected: expected.clone(),
            },
        }
    }
}

impl<D: ConstraintDomain> PartialEq for ClosedSourceSelection<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unchecked, Self::Unchecked) => true,
            (
                Self::Checked {
                    alternative: left,
                    evidence: left_evidence,
                    expected: left_expected,
                },
                Self::Checked {
                    alternative: right,
                    evidence: right_evidence,
                    expected: right_expected,
                },
            ) => {
                left == right
                    && left_evidence == right_evidence
                    && left_expected.value() == right_expected.value()
            }
            _ => false,
        }
    }
}

impl<D: ConstraintDomain> Eq for ClosedSourceSelection<D> {}

impl<D: ConstraintDomain> Clone for ClosedConstraintProbe<D> {
    fn clone(&self) -> Self {
        Self {
            completion: Arc::clone(&self.completion),
            source: self.source,
            original: self.original.clone(),
            retained_value: self.retained_value.as_ref().map(Arc::clone),
            source_ordinal: self.source_ordinal,
            branch: self.branch.clone(),
            selection: self.selection.clone(),
            prepared_source_projection: self.prepared_source_projection,
            actual: self.actual.clone(),
            source_projection: self.source_projection.clone(),
        }
    }
}

impl<D: ConstraintDomain> PartialEq for ClosedConstraintProbe<D> {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.source_ordinal == other.source_ordinal
            && self.same_observation(other)
    }
}

impl<D: ConstraintDomain> Eq for ClosedConstraintProbe<D> {}
