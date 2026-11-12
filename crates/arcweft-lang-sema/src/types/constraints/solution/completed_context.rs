//! One authority for completed ambient captures and residual quantification.

use super::*;
use crate::types::constraints::{
    application::{ConstraintApplicationScope, RetainedRigidGenericScope},
    lexical::ConstraintLexicalScope,
};

mod transport;

/// Ambient frames are retained, never generalized. Only `residual` introduces
/// fresh continuation variables. The value scope is a derived inspection view.
#[derive(Debug)]
pub(super) struct CompletedGenericContext {
    pub(super) parameter_scope: CompletedParameterScope,
    pub(super) ambient: ConstraintLexicalScope,
    pub(super) residual: ResidualGenericBinder,
    pub(super) captures: RetainedRigidGenericScope,
    value_scope: ConstraintLexicalScope,
}

impl PartialEq for CompletedGenericContext {
    fn eq(&self, other: &Self) -> bool {
        // Residual quantifiers are alpha-equivalent by their origins. Their
        // private traversal frames are derived, not ambient capture identity.
        self.parameter_scope == other.parameter_scope
            && self.ambient == other.ambient
            && self.residual == other.residual
            && self.captures == other.captures
    }
}

impl Eq for CompletedGenericContext {}

/// A completed result retains the authority which distinguishes its ambient
/// captures from the quantifiers it is allowed to transfer to a function.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::types::constraints) struct CompletedConstraintType {
    value: TypeKind,
    context: Arc<CompletedGenericContext>,
}

impl CompletedConstraintType {
    pub(in crate::types::constraints::solution) fn new(
        value: TypeKind,
        context: Arc<CompletedGenericContext>,
    ) -> Self {
        Self { value, context }
    }

    pub(in crate::types::constraints) fn view(&self) -> ScopedTypeView<'_> {
        ScopedTypeView::sealed(&self.value, self.context.value_scope())
    }

    pub(in crate::types::constraints) fn quantify(
        &self,
    ) -> Result<TypeKind, TypeInstantiationError> {
        let value = self
            .view()
            .quantify_suffix_with_predicate_and_control(
                &crate::effect_row::EffectPredicate::unconstrained(),
                self.context.ambient.shape(),
                &mut crate::types::projection_control::UnmeteredTypeProjection,
            )
            .map_err(crate::types::TypeProjectionError::into_instantiation)?;
        if self.context.ambient.shape().binders().is_empty() {
            return Ok(value.into_value());
        }
        value.view().to_root_type()
    }
}

impl CompletedGenericContext {
    pub(super) fn for_application<A: TypeConstraintAccounting, D: ConstraintDomain>(
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        let mut ambient = context.capture_lexical_scope()?;
        ambient.merge(application.introduction_scope(), context)?;
        path.applications
            .merge_lexical_scopes(&mut ambient, context)?;
        let captures = path.applications.retain_rigid_captures(context)?;
        let residual = ResidualGenericBinder::for_application(path, application)?;
        let value_scope = ambient.with_binder(residual.binder());
        Ok(Self {
            parameter_scope: application.parameters().completed_contract().clone(),
            ambient,
            residual,
            captures,
            value_scope,
        })
    }

    pub(super) fn value_scope(&self) -> &GenericScope {
        self.value_scope.shape()
    }

    pub(super) fn value_lexical_scope(&self) -> &ConstraintLexicalScope {
        &self.value_scope
    }
}
