//! The sealed authority for one prepared callable application.
//!
//! A graph prefix must not carry a callable, group, and lower solution as
//! independent values.  Those values are sealed together here and can only
//! be inspected through callable-owned projections. The analyzer receives
//! the application as part of its prepared prefix, but cannot construct one
//! or obtain the raw lower solution from this module. The result and solution
//! are issued together by the completed constraint application. Final sealing
//! checks that result against the frozen declaration.

use std::sync::Arc;
use thiserror::Error;

use crate::{
    effect_row::EffectRow,
    types::{
        TypeKind,
        constraints::{
            CompletedConstraintComponent, ConstraintDomain, TypeConstraintInvariant,
            TypeConstraintProjectionInvariant, TypeConstraintSolution,
        },
    },
};

use super::{
    CallConstraintInvariant, CallableGroupIndex, CallableParameterCoordinate, CallableResultSchema,
    CallableSignatureSchema, CallableSignatureSchemaDigest, ContentCallableIdentity,
    DetachedPreparedResolvedCallable, PreparedResolvedCallable,
    PreparedResolvedCallableDetachArena, schema::CallableGroupTransition,
};

/// One selected callable together with the exact group transition and lower
/// solution that produced it.  This is intentionally move-only: a selected
/// application is transferred from the candidate runner into the prepared
/// call graph and is never reconstructed from a public triplet.
pub(crate) struct PreparedCallableApplication {
    selected: Arc<PreparedResolvedCallable>,
    transition: CallableGroupTransition,
    solution: Arc<TypeConstraintSolution>,
    result: PreparedCallResult,
}

/// The selected output determines how this application delivers its result.
/// An ordinary value of function type does not own a continuation.
#[derive(Clone, Eq, PartialEq)]
pub(crate) enum PreparedCallResult {
    Value(TypeKind),
    Continuation(TypeKind),
    ContentEmission(ContentCallableIdentity),
}

/// The result and solution of the same completed application. The component
/// supplies both from one application address, including nested applications.
pub(crate) struct CompletedCallResult {
    schema: CallableSignatureSchemaDigest,
    solution: Arc<TypeConstraintSolution>,
    result: CallableResultSchema,
}

impl CompletedCallResult {
    pub(crate) fn from_component<D: ConstraintDomain>(
        component: &CompletedConstraintComponent<D>,
        application: D::Application,
        result_key: &D::Projection,
        schema: &CallableSignatureSchema,
    ) -> Result<Self, CallConstraintInvariant> {
        let application = component
            .application(application)
            .ok_or(CallConstraintInvariant::PreparedCallSiteMismatch)?;
        let projection = application
            .projections()
            .iter()
            .find(|projection| projection.key() == result_key);
        let result = match (schema.result_schema(), projection) {
            (CallableResultSchema::Value(_), Some(projection)) => {
                CallableResultSchema::Value(projection.to_quantified_type()?)
            }
            (CallableResultSchema::ContentEmission(operation), None) => {
                CallableResultSchema::ContentEmission(*operation)
            }
            (CallableResultSchema::Value(_), None) => {
                return Err(CallConstraintInvariant::Lower(
                    TypeConstraintInvariant::Projection(
                        TypeConstraintProjectionInvariant::MissingKey,
                    ),
                ));
            }
            (CallableResultSchema::ContentEmission(_), Some(_)) => {
                return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch);
            }
        };
        Ok(Self {
            schema: schema.semantic_digest(),
            solution: Arc::clone(application.solution()),
            result,
        })
    }
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
        self.solution.apply_call_effects(row).map_err(Into::into)
    }

    pub(crate) fn specialize_with_control<C: crate::types::TypeProjectionControl>(
        &self,
        row: &EffectRow,
        control: &mut C,
    ) -> Result<EffectRow, crate::types::TypeProjectionError<C::Error>> {
        self.solution.apply_call_effects_with_control(row, control)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum PreparedCallableApplicationReplayMismatch {
    #[error("selected callable differs")]
    SelectedCallable,
    #[error("completed group differs")]
    GroupTransition,
    #[error("constraint solution differs")]
    Solution,
    #[error("projected application result differs")]
    Result,
}

/// Detached application after stage one. The selected callable is represented
/// exactly once and the lower solution has crossed into the opaque
/// final-solution seed. Final sealing validates the same prepared result
/// against the frozen declaration.
pub(crate) struct DetachedPreparedCallableApplication {
    selected: DetachedPreparedResolvedCallable,
    solution: super::checked_application::FrozenCallTypeSolutionSeed,
    result: PreparedCallResult,
}

impl DetachedPreparedCallableApplication {
    pub(crate) fn into_parts(
        self,
    ) -> (
        DetachedPreparedResolvedCallable,
        super::checked_application::FrozenCallTypeSolutionSeed,
        PreparedCallResult,
    ) {
        (self.selected, self.solution, self.result)
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
        mapping: &super::PreparedCallArgumentMapping,
        completed: CompletedCallResult,
    ) -> Result<Self, CallConstraintInvariant> {
        let transition = selected.application_transition(mapping)?;
        let CompletedCallResult {
            schema,
            solution,
            result,
        } = completed;
        if schema != selected.schema().semantic_digest() {
            return Err(CallConstraintInvariant::PreparedSchemaMismatch);
        }
        let result = match (transition.continuation_position(), result) {
            (None, CallableResultSchema::Value(value)) => PreparedCallResult::Value(value),
            (None, CallableResultSchema::ContentEmission(operation)) => {
                PreparedCallResult::ContentEmission(operation)
            }
            (Some(_), CallableResultSchema::Value(value @ TypeKind::Function { .. })) => {
                PreparedCallResult::Continuation(value)
            }
            (Some(_), _) => return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch),
        };
        Ok(Self {
            selected,
            transition,
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

    pub(crate) const fn completed_group(&self) -> CallableGroupIndex {
        self.transition.current()
    }

    pub(crate) const fn position(&self) -> CallableParameterCoordinate {
        self.transition.position()
    }

    pub(crate) const fn continuation_position(&self) -> Option<CallableParameterCoordinate> {
        self.transition.continuation_position()
    }

    pub(crate) const fn retains_inputs(&self) -> bool {
        self.transition.retains_inputs()
    }

    pub(super) fn schema(&self) -> CallableSignatureSchemaDigest {
        self.selected.schema().semantic_digest()
    }

    pub(crate) const fn result(&self) -> &PreparedCallResult {
        &self.result
    }

    pub(crate) fn function_input_specialization_with_control<
        C: crate::types::TypeProjectionControl,
    >(
        &self,
        owner: arcweft_lang_hir::identity::ExprId,
        source: &TypeKind,
        control: &mut C,
    ) -> Result<
        Arc<super::CheckedFunctionSpecialization>,
        super::FunctionSpecializationSealFailure<C::Error>,
    > {
        super::CheckedFunctionSpecialization::seal_call_input(
            owner,
            source,
            &self.solution,
            self.transition,
            None,
            control,
        )
    }

    /// Applies the selected application's exact substitutions to storage
    /// originating in its declaration body.
    pub(crate) fn project_body_storage<C: crate::types::TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        under: &crate::types::GenericScope,
        control: &mut C,
    ) -> Result<crate::types::ScopedType, crate::types::TypeProjectionError<C::Error>> {
        self.solution.apply_template_under(ty, under, control)
    }

    pub(crate) fn body_type_arguments(
        &self,
    ) -> impl ExactSizeIterator<Item = crate::types::ScopedTypeView<'_>> {
        self.solution.bindings().map(|(_, value)| value)
    }

    pub(crate) fn body_const_arguments(
        &self,
    ) -> impl ExactSizeIterator<Item = crate::types::ScopedArrayLengthView<'_>> {
        self.solution.const_bindings().map(|(_, value)| value)
    }

    pub(crate) fn project_body_length<C: crate::types::TypeProjectionControl>(
        &self,
        length: &crate::types::ArrayLength,
        under: &crate::types::GenericScope,
        control: &mut C,
    ) -> Result<crate::types::ArrayLength, crate::types::TypeProjectionError<C::Error>> {
        self.solution.apply_length_under(length, under, control)
    }

    pub(crate) fn project_body_effects<C: crate::types::TypeProjectionControl>(
        &self,
        row: &EffectRow,
        under: &crate::types::GenericScope,
        control: &mut C,
    ) -> Result<EffectRow, crate::types::TypeProjectionError<C::Error>> {
        self.solution.apply_effects_under(row, under, control)
    }

    pub(crate) fn localize_body_template<C: crate::types::TypeProjectionControl>(
        &self,
        localizer: &impl crate::types::constraints::TypeTemplateLocalizer<C>,
        control: &mut C,
    ) -> Result<
        crate::types::constraints::LocalizedTypeTemplate,
        crate::types::TypeProjectionError<C::Error>,
    > {
        crate::types::constraints::LocalizedTypeTemplate::new(
            Arc::clone(&self.solution),
            localizer,
            control,
        )
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
        if self.transition != other.transition {
            return Some(PreparedCallableApplicationReplayMismatch::GroupTransition);
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
            transition,
            solution,
            result,
        } = self;
        let schema = selected.schema().semantic_digest();
        let selected = arena.detach(selected)?;
        Ok(DetachedPreparedCallableApplication {
            selected,
            solution: super::checked_application::FrozenCallTypeSolutionSeed::from_prepared(
                schema, transition, solution,
            ),
            result,
        })
    }
}
