//! Controlled entry into the effect evidence retained during body preparation.

use crate::effect_row::prepared::{
    PreparedEffectFailure, PreparedEffectPredicate, PreparedEffectRow, PreparedEffectTermEdit,
};

use super::*;
use crate::types::projection_control::{
    EffectProjectionControl, visit_effect_predicate, visit_effect_row,
};

mod validation;
pub(crate) use validation::PreparedTypeRetentionFailure;
mod canonical;
mod lifting;
mod type_arguments;
pub(crate) use type_arguments::{PreparedNominalArgumentInference, PreparedTypeArguments};
mod roots;
pub(crate) use roots::PreparedTypeEffectRoots;
mod named_port;
pub(crate) use named_port::{PreparedNamedEffectPort, PreparedNamedEffectUse};

/// Prepared identity is operational. This domain cannot issue a checked type
/// digest, specialization, runtime projection or completed constraint solution.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PreparedEffects;

impl sealed::Sealed for PreparedEffects {}

impl TypeEffectDomain for PreparedEffects {
    type Row = PreparedEffectRow;
    type Predicate = PreparedEffectPredicate;
}

struct EffectPreparation<'a, C> {
    control: &'a mut C,
    depth: u64,
}

impl<C: TypeProjectionControl> TypeProjectionControl for EffectPreparation<'_, C> {
    type Error = C::Error;

    fn check(&mut self) -> Result<(), Self::Error> {
        self.control.check()
    }

    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        self.control.visit_node(kind, depth)?;
        self.depth = depth;
        Ok(())
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.control.visit_binding()
    }
}

impl<C: TypeProjectionControl> TypeEffectProjection<CanonicalEffects, PreparedEffects>
    for EffectPreparation<'_, C>
{
    fn project_row(
        &mut self,
        row: &EffectRow,
        _: &GenericScope,
    ) -> Result<PreparedEffectRow, TypeProjectionError<Self::Error>> {
        visit_effect_row(self.control, row, self.depth)?;
        Ok(PreparedEffectRow::Canonical(row.clone()))
    }

    fn project_predicate(
        &mut self,
        predicate: &EffectPredicate,
        _: &GenericScope,
    ) -> Result<PreparedEffectPredicate, TypeProjectionError<Self::Error>> {
        visit_effect_predicate(self.control, predicate, self.depth)?;
        Ok(PreparedEffectPredicate::Canonical(predicate.clone()))
    }
}

impl TypeKind {
    /// Retains canonical annotations, including omitted rows, in the one shared
    /// type grammar. Subsequent body analysis can carry issuer-owned terms in
    /// any nested arrow without issuing a completed semantic identity.
    pub(crate) fn prepare_effects<C: TypeProjectionControl>(
        &self,
        scope: &GenericScope,
        control: &mut C,
    ) -> Result<TypeKind<PreparedEffects>, TypeProjectionError<C::Error>> {
        self.project_effect_domain(scope, &mut EffectPreparation { control, depth: 1 })
    }
}

/// Retains the original control failure separately from malformed graph or
/// effect constraints. Scope/instantiation errors keep their projection kind.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PreparedTypeEffectProjectionFailure<E: std::error::Error + 'static> {
    #[error("prepared type projection aborted")]
    Control(#[source] E),
    #[error("prepared effect projection failed")]
    Effects(#[source] PreparedEffectFailure<TypeProjectionError<E>>),
}

impl<E: std::error::Error + 'static> PreparedTypeEffectProjectionFailure<E> {
    /// A rejected candidate may lack a principal observation. An optional
    /// diagnostic records only an observable type and preserves real failures.
    pub(crate) fn optional_observation<T>(
        observation: Result<T, TypeProjectionError<Self>>,
    ) -> Result<Option<T>, TypeProjectionError<Self>> {
        match observation {
            Ok(value) => Ok(Some(value)),
            Err(TypeProjectionError::Control(Self::Effects(
                PreparedEffectFailure::ObservationNeedsBody(_)
                | PreparedEffectFailure::ObservationNotTotal
                | PreparedEffectFailure::NonPointwiseObservation,
            ))) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(in crate::types) fn from_projection(
        error: TypeProjectionError<E>,
    ) -> TypeProjectionError<Self> {
        match error {
            TypeProjectionError::Instantiation(error) => TypeProjectionError::Instantiation(error),
            TypeProjectionError::Control(error) => {
                TypeProjectionError::Control(Self::Control(error))
            }
        }
    }

    pub(in crate::types) fn from_effect(
        failure: PreparedEffectFailure<TypeProjectionError<E>>,
    ) -> TypeProjectionError<Self> {
        match failure {
            PreparedEffectFailure::Control(error) => Self::from_projection(error),
            failure => TypeProjectionError::Control(Self::Effects(failure)),
        }
    }
}

struct EffectParameterProjection<'a, 'g, C, P> {
    edit: PreparedEffectTermEdit<'g>,
    control: &'a mut C,
    project: &'a mut P,
    depth: u64,
}

impl<C: TypeProjectionControl, P> TypeProjectionControl
    for EffectParameterProjection<'_, '_, C, P>
{
    type Error = PreparedTypeEffectProjectionFailure<C::Error>;

    fn check(&mut self) -> Result<(), Self::Error> {
        self.control
            .check()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }

    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        self.control
            .visit_node(kind, depth)
            .map_err(PreparedTypeEffectProjectionFailure::Control)?;
        self.depth = depth;
        Ok(())
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.control
            .visit_binding()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }
}

impl<C, P> TypeEffectProjection<PreparedEffects, PreparedEffects>
    for EffectParameterProjection<'_, '_, C, P>
where
    C: TypeProjectionControl,
    P: FnMut(
        &super::super::GenericEffectReference,
        &GenericScope,
        &mut C,
    ) -> Result<Option<PreparedEffectRow>, TypeProjectionError<C::Error>>,
{
    fn project_row(
        &mut self,
        row: &PreparedEffectRow,
        scope: &GenericScope,
    ) -> Result<PreparedEffectRow, TypeProjectionError<Self::Error>> {
        let project = &mut self.project;
        self.edit
            .map_row_parameters(
                row,
                &mut EffectProjectionControl {
                    control: self.control,
                    depth: self.depth,
                },
                &mut |reference, control| {
                    project(reference, scope, control.control)
                        .map_err(PreparedEffectFailure::Control)
                },
            )
            .map_err(PreparedTypeEffectProjectionFailure::from_effect)
    }

    fn project_predicate(
        &mut self,
        predicate: &PreparedEffectPredicate,
        scope: &GenericScope,
    ) -> Result<PreparedEffectPredicate, TypeProjectionError<Self::Error>> {
        let project = &mut self.project;
        self.edit
            .map_predicate_parameters(
                predicate,
                &mut EffectProjectionControl {
                    control: self.control,
                    depth: self.depth,
                },
                &mut |reference, control| {
                    project(reference, scope, control.control)
                        .map_err(PreparedEffectFailure::Control)
                },
            )
            .map_err(PreparedTypeEffectProjectionFailure::from_effect)
    }
}

impl TypeKind<PreparedEffects> {
    pub(crate) fn same_continuation_type(&self, other: &Self) -> bool {
        let (mut left, mut right) = (self, other);
        while let (
            Self::Function {
                binder: left_binder,
                predicate: left_predicate,
                params: left_params,
                return_type: left_result,
                effects: left_effects,
            },
            Self::Function {
                binder: right_binder,
                predicate: right_predicate,
                params: right_params,
                return_type: right_result,
                effects: right_effects,
            },
        ) = (left, right)
        {
            if left_binder != right_binder
                || left_predicate != right_predicate
                || left_params != right_params
                || !left_effects.same_continuation_row(right_effects)
            {
                return false;
            }
            left = left_result;
            right = right_result;
        }
        left == right
    }

    pub(crate) fn function_with_canonical_effects(
        params: impl IntoIterator<Item = Self>,
        return_type: Self,
        effects: EffectRow,
    ) -> Self {
        Self::function_with_contract(
            crate::types::GenericBinder::EMPTY,
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            params,
            return_type,
            PreparedEffectRow::Canonical(effects),
        )
    }

    /// Issued terms are known annotations. Only an authored canonical omitted
    /// row requests initializer inference; use the shared shape grammar.
    pub(crate) fn has_omitted_function_effect_rows_with_control<C: TypeProjectionControl>(
        &self,
        control: &mut C,
    ) -> Result<bool, TypeProjectionError<C::Error>> {
        let roots = PreparedTypeEffectRoots::collect([self], std::iter::empty(), control)?;
        Ok(roots
            .rows()
            .iter()
            .any(|row| matches!(row, PreparedEffectRow::Canonical(row) if !row.is_known())))
    }

    /// Projects free effect parameters under each exact nested binder. One edit
    /// spans the whole structural fold: a later failure invalidates every term
    /// built by this projection, while its input types and graph remain intact.
    pub(crate) fn project_effect_parameters<C, P>(
        &self,
        scope: &GenericScope,
        edit: PreparedEffectTermEdit<'_>,
        control: &mut C,
        project: &mut P,
    ) -> Result<Self, TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>>
    where
        C: TypeProjectionControl,
        P: FnMut(
            &super::super::GenericEffectReference,
            &GenericScope,
            &mut C,
        ) -> Result<Option<PreparedEffectRow>, TypeProjectionError<C::Error>>,
    {
        let mut projection = EffectParameterProjection {
            edit,
            control,
            project,
            depth: 1,
        };
        let result = self.project_effect_domain(scope, &mut projection)?;
        projection
            .edit
            .commit()
            .map_err(|error| PreparedTypeEffectProjectionFailure::from_effect(error.into()))?;
        Ok(result)
    }
}
