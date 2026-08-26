//! The sealed authority for one prepared callable application.
//!
//! A graph prefix must not carry a callable, group, and lower solution as
//! independent values.  Those values are sealed together here and can only
//! be inspected through callable-owned projections.  The analyzer receives
//! the application as part of its prepared prefix, but cannot construct one
//! or obtain the raw lower solution from this module.

use std::sync::Arc;
use thiserror::Error;

use crate::types::{TypeKind, constraints::TypeConstraintSolution};

use super::{
    CallConstraintInvariant, CallableGroupIndex, CallableSignatureSchemaDigest,
    DetachedPreparedResolvedCallable, PreparedResolvedCallable,
    PreparedResolvedCallableDetachArena,
};

/// One selected callable together with the exact completed group and lower
/// solution that produced it.  This is intentionally move-only: a selected
/// application is transferred from the candidate runner into the prepared
/// call graph and is never reconstructed from a public triplet.
pub(crate) struct PreparedCallableApplication {
    selected: Arc<PreparedResolvedCallable>,
    completed_group: CallableGroupIndex,
    solution: Arc<TypeConstraintSolution>,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum PreparedCallableApplicationReplayMismatch {
    #[error("selected callable differs")]
    SelectedCallable,
    #[error("completed group differs")]
    CompletedGroup,
    #[error("constraint solution differs")]
    Solution,
}

/// Projection-free application after stage-one candidate detachment.  The
/// selected callable is represented exactly once and the lower solution has
/// already crossed into the opaque final-solution seed; no selected `Arc` is
/// cloned into a parallel candidate inventory.
pub(crate) struct DetachedPreparedCallableApplication {
    selected: DetachedPreparedResolvedCallable,
    solution: super::checked_application::FrozenCallTypeSolutionSeed,
}

impl DetachedPreparedCallableApplication {
    pub(crate) fn into_parts(
        self,
    ) -> (
        DetachedPreparedResolvedCallable,
        super::checked_application::FrozenCallTypeSolutionSeed,
    ) {
        (self.selected, self.solution)
    }
}

impl PreparedCallableApplication {
    /// Callable-owned sealing point for the consuming selected-transaction
    /// path.  The completed group is derived from the selected callable so a
    /// caller cannot pair a candidate with an independently supplied group.
    /// The returned application is move-only and no alternate constructor is
    /// available to tests or analyzer code.
    pub(crate) fn seal_from_selected_transaction(
        selected: Arc<PreparedResolvedCallable>,
        solution: Arc<TypeConstraintSolution>,
    ) -> Result<Self, CallConstraintInvariant> {
        let completed_group = selected.call_group();
        if selected.schema().group(completed_group).is_none() {
            return Err(CallConstraintInvariant::PreparedGroupMismatch);
        }
        Ok(Self {
            selected,
            completed_group,
            solution,
        })
    }

    pub(crate) fn selected(&self) -> &PreparedResolvedCallable {
        self.selected.as_ref()
    }

    pub(crate) fn selected_shared(&self) -> &Arc<PreparedResolvedCallable> {
        &self.selected
    }

    pub(crate) fn completed_group(&self) -> CallableGroupIndex {
        self.completed_group
    }

    pub(super) fn schema(&self) -> CallableSignatureSchemaDigest {
        self.selected.schema().semantic_digest()
    }

    pub(crate) fn result_type(&self) -> Result<TypeKind, CallConstraintInvariant> {
        let declared = self
            .selected
            .result_type_for_group(self.completed_group)
            .ok_or(CallConstraintInvariant::PreparedFunctionTypeMismatch)?;
        Ok(self.solution.apply(&declared))
    }

    pub(crate) fn function_type(&self) -> Result<TypeKind, CallConstraintInvariant> {
        let result = self.result_type()?;
        if !matches!(result, TypeKind::Function { .. }) {
            return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch);
        }
        Ok(result)
    }

    pub(super) fn base_matches(&self, candidate: &PreparedResolvedCallable) -> bool {
        let selected = &self.selected;
        selected.id() == candidate.id()
            && selected.family() == candidate.family()
            && selected.origin() == candidate.origin()
            && selected.checked() == candidate.checked()
            && selected.record() == candidate.record()
            && selected.instantiation() == candidate.instantiation()
            && selected.equivalent_sources() == candidate.equivalent_sources()
            && selected.authority() == candidate.authority()
            && selected.schema().semantic_digest() == candidate.schema().semantic_digest()
            && selected.prepared_effect_instantiation().issuer()
                == candidate.prepared_effect_instantiation().issuer()
    }

    pub(super) fn solution(&self) -> &Arc<TypeConstraintSolution> {
        &self.solution
    }

    pub(crate) fn replay_eq(&self, other: &Self) -> bool {
        self.replay_mismatch(other).is_none()
    }

    pub(crate) fn replay_mismatch(
        &self,
        other: &Self,
    ) -> Option<PreparedCallableApplicationReplayMismatch> {
        if !self.selected.replay_eq(&other.selected) {
            return Some(PreparedCallableApplicationReplayMismatch::SelectedCallable);
        }
        if self.completed_group != other.completed_group {
            return Some(PreparedCallableApplicationReplayMismatch::CompletedGroup);
        }
        if !self
            .selected
            .prepared_effect_instantiation()
            .solution_replay_eq(
                &self.solution,
                other.selected.prepared_effect_instantiation(),
                &other.solution,
            )
        {
            return Some(PreparedCallableApplicationReplayMismatch::Solution);
        }
        None
    }

    /// Stage-one detach for the selected application.  Callers must detach the
    /// selected marker before expanding the remaining producer-order
    /// inventory, so the selected outer `Arc` is consumed exactly once.
    pub(crate) fn detach(
        self,
        arena: &mut PreparedResolvedCallableDetachArena,
    ) -> Result<DetachedPreparedCallableApplication, CallConstraintInvariant> {
        let Self {
            selected,
            completed_group,
            solution,
        } = self;
        let schema = selected.schema().semantic_digest();
        let effect_instantiation = selected.prepared_effect_instantiation().evidence();
        let selected = arena.detach(selected)?;
        Ok(DetachedPreparedCallableApplication {
            selected,
            solution: super::checked_application::FrozenCallTypeSolutionSeed::from_prepared(
                schema,
                completed_group,
                solution,
                effect_instantiation,
            ),
        })
    }
}
