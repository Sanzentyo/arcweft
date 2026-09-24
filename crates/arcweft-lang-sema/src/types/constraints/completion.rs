//! Completed applications and their projections share one component authority.

use std::{collections::BTreeMap, sync::Arc};

use super::{
    ClosedConstraintSourceTrace, ConstraintDomain, KeyedConstraintProjection, SolvedCandidate,
    TypeConstraintFailure, TypeConstraintRejection, TypeConstraintSolution,
};

/// Distinct, fully materialized alternatives left after one constraint path
/// has closed. Candidate ranking belongs to the caller; the lower layer only
/// removes semantically duplicate component/branch pairs.
pub(crate) struct CompletedCandidateAlternatives<D: ConstraintDomain> {
    alternatives: Box<[SolvedCandidate<D>]>,
}

impl<D: ConstraintDomain> CompletedCandidateAlternatives<D> {
    pub(super) fn new(first: SolvedCandidate<D>, remaining: Vec<SolvedCandidate<D>>) -> Self {
        let mut alternatives = Vec::with_capacity(remaining.len().saturating_add(1));
        alternatives.push(first);
        alternatives.extend(remaining);
        Self {
            alternatives: alternatives.into_boxed_slice(),
        }
    }

    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &SolvedCandidate<D>> {
        self.alternatives.iter()
    }

    pub(crate) fn len(&self) -> usize {
        self.alternatives.len()
    }

    /// Consume the frontier into exactly one member. A tied frontier retains
    /// the historical lower ambiguity result for ordinary callers.
    pub(crate) fn into_unique(self) -> Result<SolvedCandidate<D>, TypeConstraintFailure<D>> {
        if self.alternatives.len() == 1 {
            return Ok(self
                .alternatives
                .into_vec()
                .pop()
                .expect("the candidate frontier is nonempty"));
        }
        Err(TypeConstraintFailure::Rejected(
            super::TypeConstraintCandidateFailure::Constraint(
                TypeConstraintRejection::AmbiguousSolution {
                    actual: self.alternatives.len(),
                },
            ),
        ))
    }

    /// Move one ranked row to the caller and hand every other row to its
    /// affine cleanup owner before the winner can be returned. The callback
    /// runs for every loser, including every row when `index` is invalid. If
    /// cleanup fails, no winner is returned so the caller can roll back the
    /// enclosing candidate fact transaction.
    pub(crate) fn into_index_with<E>(
        self,
        index: usize,
        mut discard: impl FnMut(SolvedCandidate<D>) -> Result<(), E>,
    ) -> Result<Option<SolvedCandidate<D>>, E> {
        let mut selected = None;
        let mut first_error = None;
        for (candidate_index, candidate) in self.alternatives.into_vec().into_iter().enumerate() {
            if candidate_index == index {
                selected = Some(candidate);
            } else if let Err(error) = discard(candidate)
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(selected),
        }
    }
}

#[derive(Eq, PartialEq)]
pub(crate) struct CompletedConstraintApplication<A, P> {
    solution: Arc<TypeConstraintSolution>,
    projections: Box<[KeyedConstraintProjection<A, P>]>,
}

impl<A, P> CompletedConstraintApplication<A, P> {
    pub(super) fn new(
        solution: Arc<TypeConstraintSolution>,
        projections: Box<[KeyedConstraintProjection<A, P>]>,
    ) -> Self {
        Self {
            solution,
            projections,
        }
    }

    pub(crate) fn solution(&self) -> &Arc<TypeConstraintSolution> {
        &self.solution
    }

    pub(crate) fn projections(&self) -> &[KeyedConstraintProjection<A, P>] {
        &self.projections
    }
}

/// A dependency names a projection in this completed component by its domain
/// owner. Live opening identities never enter completed value-use evidence.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct CompletedProjectionAddress<A, P> {
    pub(super) application: A,
    pub(super) key: Arc<P>,
}

pub(crate) struct CompletedConstraintComponent<D: ConstraintDomain> {
    selected: D::Application,
    applications:
        BTreeMap<D::Application, CompletedConstraintApplication<D::Application, D::Projection>>,
    sources: ClosedConstraintSourceTrace<D>,
}

impl<D: ConstraintDomain> CompletedConstraintComponent<D> {
    pub(super) fn equal_with<A: super::context::TypeConstraintAccounting>(
        &self,
        other: &Self,
        context: &mut super::context::TypeConstraintContext<'_, A, D>,
    ) -> Result<bool, super::TypeConstraintError> {
        context.check_cancelled()?;
        if self.selected != other.selected || self.applications.len() != other.applications.len() {
            return Ok(false);
        }
        for ((left_id, left), (right_id, right)) in
            self.applications.iter().zip(&other.applications)
        {
            context.enter_node()?;
            if left_id != right_id
                || !left.solution.equal_with(&right.solution, context)?
                || left.projections.len() != right.projections.len()
            {
                return Ok(false);
            }
            for (left, right) in left.projections.iter().zip(&right.projections) {
                context.enter_node()?;
                if left.key() != right.key()
                    || left.source() != right.source()
                    || left.value().scope() != right.value().scope()
                    || !super::normalization::completed_types_equal(
                        left.value().value(),
                        right.value().value(),
                        context,
                    )?
                {
                    return Ok(false);
                }
                match (left.input_type(), right.input_type()) {
                    (Some(left), Some(right)) => {
                        if !super::normalization::completed_types_equal(left, right, context)? {
                            return Ok(false);
                        }
                    }
                    (None, None) => {}
                    _ => return Ok(false),
                }
            }
        }
        self.sources.equal_with(&other.sources, context)
    }

    pub(super) fn new(
        selected: D::Application,
        applications: BTreeMap<
            D::Application,
            CompletedConstraintApplication<D::Application, D::Projection>,
        >,
        sources: ClosedConstraintSourceTrace<D>,
    ) -> Self {
        assert!(
            applications.contains_key(&selected),
            "completed component retains its selected application"
        );
        Self {
            selected,
            applications,
            sources,
        }
    }

    pub(crate) fn selected(
        &self,
    ) -> &CompletedConstraintApplication<D::Application, D::Projection> {
        self.application(self.selected)
            .expect("completed component retains its selected application")
    }

    pub(crate) fn application(
        &self,
        application: D::Application,
    ) -> Option<&CompletedConstraintApplication<D::Application, D::Projection>> {
        self.applications.get(&application)
    }

    pub(crate) fn applications(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            D::Application,
            &CompletedConstraintApplication<D::Application, D::Projection>,
        ),
    > {
        self.applications
            .iter()
            .map(|(application, value)| (*application, value))
    }

    /// Iterate only the source rows owned by one exact admitted application.
    /// The application identity is checked against the completed component;
    /// a source's opening is never inferred from its local coordinate.
    pub(crate) fn sources_for(
        &self,
        application: D::Application,
    ) -> Option<impl Iterator<Item = &super::ClosedConstraintProbe<D>> + '_> {
        self.application(application)?;
        Some(self.sources.all().iter().filter(move |source| {
            self.sources
                .domain_application(source.source().application())
                == application
        }))
    }

    pub(crate) const fn sources(&self) -> &ClosedConstraintSourceTrace<D> {
        &self.sources
    }
}

impl<D: ConstraintDomain> PartialEq for CompletedConstraintComponent<D> {
    fn eq(&self, other: &Self) -> bool {
        self.selected == other.selected
            && self.applications == other.applications
            && self.sources == other.sources
    }
}

impl<D: ConstraintDomain> Eq for CompletedConstraintComponent<D> {}
