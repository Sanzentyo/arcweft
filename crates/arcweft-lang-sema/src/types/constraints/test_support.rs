//! Fixtures keep application scope separate from work accounting, as in the
//! production driver. Isolated projection tests receive an actual scoped path;
//! transaction tests enter through the production initializer.

use std::sync::atomic::AtomicBool;

use crate::effect_row::{
    DecisionControl, EffectConstraintEnvironment, EffectConstraintEnvironmentError, EffectRow,
    prepared::{
        PreparedEffectFailure, PreparedEffectGraph, PreparedEffectRelation, PreparedEffectRow,
    },
};
use crate::types::{PreparedEffects, TypeKind, TypeProjectionError};

use super::{
    ConstraintDomain, TypeConstraintError, TypeConstraintInvariant, TypeConstraintParameterScope,
    context::{
        LocalConstraintAccounting, TypeConstraintAccounting, TypeConstraintContext,
        TypeConstraintContextIssuer, TypeConstraintLimits,
    },
    transaction::ConstraintPath,
};

/// Standalone callback-protocol fixtures need an opening even when they do
/// not execute a lower transaction. Tests that project a path use its real ID.
pub(crate) fn source_id<S: Copy>(local: S) -> super::ConstraintSourceId<S> {
    let parameters = TypeConstraintParameterScope::empty();
    super::ConstraintSourceId::new((&parameters).into(), local)
}

pub(in crate::types) fn fixture_relate_selected_call<A, D, L: Clone>(
    graph: &mut PreparedEffectGraph,
    pattern: &TypeKind,
    actual: &TypeKind,
    path: super::transaction::ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    acceptance: super::ConstraintAcceptance,
) -> Result<Vec<super::transaction::ConstraintInferenceBranch<D, L>>, super::TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let pattern = context.prepare_fixture_type(pattern)?;
    let actual = context.prepare_fixture_type(actual)?;
    let pattern = context.fixture_operand(pattern);
    let actual = context.fixture_operand(actual);
    super::relate_selected_call(
        graph,
        &pattern.view(),
        &actual.view(),
        path,
        context,
        acceptance,
    )
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain> TypeConstraintContext<'_, A, D> {
    pub(in crate::types::constraints) fn prepare_fixture_type(
        &mut self,
        value: &TypeKind,
    ) -> Result<TypeKind<PreparedEffects>, TypeConstraintError> {
        let scope = self.lexical_scope().clone();
        value
            .prepare_effects(&scope, self)
            .map_err(|error| match error {
                TypeProjectionError::Control(error) => error,
                TypeProjectionError::Instantiation(error) => {
                    TypeConstraintInvariant::Instantiation(error).into()
                }
            })
    }

    pub(in crate::types::constraints) fn open_fixture_template_type(
        &mut self,
        value: &TypeKind,
        path: &ConstraintPath<D>,
        application: super::application::ConstraintApplicationId,
        graph: &mut PreparedEffectGraph,
    ) -> Result<TypeKind<PreparedEffects>, TypeConstraintError> {
        let prepared = self.prepare_fixture_type(value)?;
        self.open_template_type(&prepared, path, application, graph)
    }
}

impl EffectConstraintEnvironment<PreparedEffects> {
    pub(in crate::types) fn constrain_fixture_subset<C: DecisionControl>(
        &mut self,
        graph: &mut PreparedEffectGraph,
        actual: &EffectRow,
        permitted: &EffectRow,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        let actual = PreparedEffectRow::Canonical(
            actual
                .copy_with_control(control)
                .map_err(PreparedEffectFailure::Control)?,
        );
        let permitted = PreparedEffectRow::Canonical(
            permitted
                .copy_with_control(control)
                .map_err(PreparedEffectFailure::Control)?,
        );
        self.constrain_relation(
            graph,
            PreparedEffectRelation::AdmittedSubset,
            &actual,
            &permitted,
            control,
        )
    }
}

pub(crate) struct ConstraintTestSetup<'c, A: TypeConstraintAccounting, D: ConstraintDomain> {
    context: TypeConstraintContext<'c, A, D>,
    parameters: TypeConstraintParameterScope,
}

impl<'c, D: ConstraintDomain> ConstraintTestSetup<'c, LocalConstraintAccounting<'c>, D> {
    pub(crate) fn new(limits: TypeConstraintLimits, cancellation: &'c AtomicBool) -> Self {
        Self::with_scope(limits, cancellation, TypeConstraintParameterScope::empty())
    }

    pub(crate) fn with_scope(
        limits: TypeConstraintLimits,
        cancellation: &'c AtomicBool,
        parameters: TypeConstraintParameterScope,
    ) -> Self {
        Self::with_accounting(
            LocalConstraintAccounting::new(limits, cancellation),
            parameters,
        )
    }
}

impl<'c, A: TypeConstraintAccounting, D: ConstraintDomain> ConstraintTestSetup<'c, A, D> {
    pub(crate) fn with_accounting(accounting: A, parameters: TypeConstraintParameterScope) -> Self
    where
        A: TypeConstraintContextIssuer<'c>,
    {
        Self {
            context: TypeConstraintContext::with_accounting(accounting),
            parameters,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        TypeConstraintContext<'c, A, D>,
        TypeConstraintParameterScope,
    ) {
        (self.context, self.parameters)
    }

    pub(crate) fn into_path(
        mut self,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    ) -> (TypeConstraintContext<'c, A, D>, ConstraintPath<D>)
    where
        D::Application: Default,
    {
        let application = self
            .context
            .fixture_application_scope(D::Application::default(), self.parameters, graph)
            .expect("fixture application retains its original graph functions");
        let path = self
            .context
            .start_path(application, graph)
            .expect("fixture path admits the actual effect function handles");
        (self.context, path)
    }
}
