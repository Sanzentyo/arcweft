//! The sealed authority for one prepared callable application.
//!
//! A graph prefix must not carry a callable, group, and lower solution as
//! independent values.  Those values are sealed together here and can only
//! be inspected through callable-owned projections. The analyzer receives
//! the application as part of its prepared prefix, but cannot construct one
//! or obtain the raw lower solution from this module. Its result is projected
//! once from the borrowed checked-catalog row and retained as an application
//! result, never as a second effect-row authority.

use std::sync::Arc;
use thiserror::Error;

use crate::{
    effect_row::{EffectRow, EffectSubstitution},
    types::{TypeKind, constraints::TypeConstraintSolution},
};

use super::{
    CallConstraintInvariant, CallableGroupIndex, CallableResultSchema,
    CallableSignatureSchemaDigest, CallableTerminalEffectProjection,
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
    result: CallableResultSchema,
}

/// Read-only effect projection capability for one exact prepared application.
/// It can outlive a graph-node borrow without losing that application's
/// effect substitutions or becoming a declaration-wide cache.
#[derive(Clone)]
pub(crate) struct PreparedCallableEffectProjection {
    solution: Arc<TypeConstraintSolution>,
}

impl PreparedCallableEffectProjection {
    pub(crate) fn specialize(&self, row: &EffectRow) -> Result<EffectRow, CallConstraintInvariant> {
        let substitutions = EffectSubstitution::from_rows(
            self.solution
                .effect_bindings()
                .map(|(parameter, value)| (parameter.value().clone(), value.value().clone())),
        );
        row.resolve_partial(&substitutions).map_err(Into::into)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum PreparedCallableApplicationReplayMismatch {
    #[error("selected callable differs")]
    SelectedCallable,
    #[error("completed group differs")]
    CompletedGroup,
    #[error("constraint solution differs")]
    Solution,
    #[error("projected application result differs")]
    Result,
}

/// Detached application after stage one. The selected callable is represented
/// exactly once and the lower solution has crossed into the opaque
/// final-solution seed; the prepared projection is discarded so final sealing
/// can recompute it from the frozen checked-catalog row.
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
        terminal_effects: CallableTerminalEffectProjection<'_>,
    ) -> Result<Self, CallConstraintInvariant> {
        let completed_group = selected.call_group();
        if selected.schema().group(completed_group).is_none() {
            return Err(CallConstraintInvariant::PreparedGroupMismatch);
        }
        let declared = selected
            .result_schema_for_group(completed_group, terminal_effects)?
            .into_ready()?;
        let result = match declared {
            CallableResultSchema::Value(value) => {
                CallableResultSchema::Value(solution.apply_result_template(&value)?)
            }
            CallableResultSchema::ContentEmission(operation) => {
                CallableResultSchema::ContentEmission(operation)
            }
        };
        Ok(Self {
            selected,
            completed_group,
            solution,
            result,
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

    pub(crate) fn result_schema(&self) -> Result<CallableResultSchema, CallConstraintInvariant> {
        Ok(self.result.clone())
    }

    pub(crate) fn result_type(&self) -> Result<TypeKind, CallConstraintInvariant> {
        self.result_schema()?
            .value_type()
            .cloned()
            .ok_or(CallConstraintInvariant::PreparedFunctionTypeMismatch)
    }

    pub(crate) fn function_type(&self) -> Result<TypeKind, CallConstraintInvariant> {
        let result = self.result_type()?;
        if !matches!(result, TypeKind::Function { .. }) {
            return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch);
        }
        Ok(result)
    }

    /// Applies this exact selected application's effect bindings while
    /// retaining any declaration-owned residual rows. Body-effect inference
    /// uses the formula before the application closes it; a different call of
    /// the same declaration may specialize the same formula differently.
    pub(crate) fn specialize_effect_row(
        &self,
        row: &EffectRow,
    ) -> Result<EffectRow, CallConstraintInvariant> {
        self.effect_projection().specialize(row)
    }

    pub(crate) fn effect_projection(&self) -> PreparedCallableEffectProjection {
        PreparedCallableEffectProjection {
            solution: Arc::clone(&self.solution),
        }
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
        if self.solution != other.solution {
            return Some(PreparedCallableApplicationReplayMismatch::Solution);
        }
        if self.result != other.result {
            return Some(PreparedCallableApplicationReplayMismatch::Result);
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
            result: _,
        } = self;
        let schema = selected.schema().semantic_digest();
        let selected = arena.detach(selected)?;
        Ok(DetachedPreparedCallableApplication {
            selected,
            solution: super::checked_application::FrozenCallTypeSolutionSeed::from_prepared(
                schema,
                completed_group,
                solution,
            ),
        })
    }
}
