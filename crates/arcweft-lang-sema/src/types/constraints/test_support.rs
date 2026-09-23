//! Fixtures keep application scope separate from work accounting, as in the
//! production driver. Isolated projection tests receive an actual scoped path;
//! transaction tests enter through the production initializer.

use std::sync::atomic::AtomicBool;

use super::{
    ConstraintDomain, TypeConstraintParameterScope,
    application::ConstraintApplicationScope,
    context::{
        LocalConstraintAccounting, TypeConstraintAccounting, TypeConstraintContext,
        TypeConstraintContextIssuer, TypeConstraintLimits,
    },
    transaction::ConstraintPath,
};

/// Standalone callback-protocol fixtures need an opening even when they do
/// not execute a lower transaction. Tests that project a path use its real ID.
pub(crate) fn source_id<S: Copy>(local: S) -> super::ConstraintSourceId<S> {
    let scope = ConstraintApplicationScope::<super::NoConstraintClient>::new(
        (),
        TypeConstraintParameterScope::empty(),
    );
    super::ConstraintSourceId::new(scope.id(), local)
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

    pub(crate) fn into_path(self) -> (TypeConstraintContext<'c, A, D>, ConstraintPath<D>)
    where
        D::Application: Default,
    {
        let effects = crate::effect_row::EffectConstraintEnvironment::new(
            &self.parameters.opened_effect_variables(),
        )
        .expect("fixture effect inventory is canonical");
        let application =
            ConstraintApplicationScope::new(D::Application::default(), self.parameters);
        (self.context, ConstraintPath::empty(application, effects))
    }
}
