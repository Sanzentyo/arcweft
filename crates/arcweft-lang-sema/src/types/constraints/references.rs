//! Scope-aware reference mapping over the shared semantic type shape.

use super::super::{
    ArrayLength, GenericConstReference, GenericEffectReference, GenericScope, GenericTypeReference,
    TypeKind,
};
use super::application::ConstraintApplicationScope;
use super::context::{TypeConstraintAccounting, TypeConstraintContext};
use super::{
    ConstraintDomain, ConstraintPath, TypeConstraintError, TypeConstraintInvariant,
    TypeConstraintParameterScopeInvariant,
};

#[cfg(test)]
mod tests;

pub(super) trait ConstraintReferenceMap {
    fn type_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericTypeReference,
        scope: &GenericScope,
        application: &ConstraintApplicationScope<D>,
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericTypeReference, TypeConstraintError>;

    fn const_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericConstReference,
        scope: &GenericScope,
        application: &ConstraintApplicationScope<D>,
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericConstReference, TypeConstraintError>;

    fn effect_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericEffectReference,
        scope: &GenericScope,
        application: &ConstraintApplicationScope<D>,
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericEffectReference, TypeConstraintError>;
}

pub(super) fn map_type<A, D>(
    ty: &TypeKind,
    mapping: &impl ConstraintReferenceMap,
    application: &ConstraintApplicationScope<D>,
    path: &ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<TypeKind, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let scope = application
        .parameters()
        .completed_contract()
        .template_scope();
    super::solution::map_term_with_control(
        ty,
        scope,
        scope,
        1,
        context,
        &|reference, scope, _, _, context| {
            mapping
                .type_reference(reference, scope, application, path, context)
                .map(TypeKind::GenericParam)
                .map_err(crate::types::TypeProjectionError::Control)
        },
        &|length, scope, _, _, context| match length {
            ArrayLength::Generic(reference) => mapping
                .const_reference(reference, scope, application, path, context)
                .map(ArrayLength::Generic)
                .map_err(crate::types::TypeProjectionError::Control),
            _ => Ok(length.clone()),
        },
        &|row, scope, _, depth, context| {
            crate::types::projection_control::visit_effect_row(context, row, depth)?;
            map_effect_row(row, mapping, scope, application, path, context)
                .map_err(crate::types::TypeProjectionError::Control)
        },
    )
    .map_err(|error| match error {
        crate::types::TypeProjectionError::Control(error) => error,
        crate::types::TypeProjectionError::Instantiation(
            crate::types::TypeInstantiationError::Scope(error),
        ) => error.into(),
        crate::types::TypeProjectionError::Instantiation(
            crate::types::TypeInstantiationError::Effect(error),
        ) => error.into(),
        crate::types::TypeProjectionError::Instantiation(_) => {
            super::TypeConstraintRejection::UnresolvedType.into()
        }
    })
}

#[cfg(test)]
pub(super) fn map_length<A, D>(
    length: &ArrayLength,
    mapping: &impl ConstraintReferenceMap,
    application: &ConstraintApplicationScope<D>,
    path: &ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ArrayLength, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    context.check_cancelled()?;
    context.enter_node()?;
    match length {
        ArrayLength::Generic(reference) => mapping
            .const_reference(
                reference,
                application
                    .parameters()
                    .completed_contract()
                    .template_scope(),
                application,
                path,
                context,
            )
            .map(ArrayLength::Generic),
        _ => Ok(length.clone()),
    }
}

pub(super) struct OpenTemplateReferences;

pub(super) fn map_effect_row<A: TypeConstraintAccounting, D: ConstraintDomain>(
    row: &crate::effect_row::EffectRow,
    mapping: &impl ConstraintReferenceMap,
    scope: &GenericScope,
    application: &ConstraintApplicationScope<D>,
    path: &ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<crate::effect_row::EffectRow, TypeConstraintError> {
    row.try_map_variables(context, &mut |reference, context| {
        mapping.effect_reference(reference, scope, application, path, context)
    })
}

impl ConstraintReferenceMap for OpenTemplateReferences {
    fn effect_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericEffectReference,
        scope: &GenericScope,
        application: &ConstraintApplicationScope<D>,
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericEffectReference, TypeConstraintError> {
        let opened = if matches!(reference, GenericEffectReference::Inference(_)) {
            if !application.admits_template_parameter(&reference.clone().into()) {
                return Err(effect_out_of_scope(reference));
            }
            reference.clone()
        } else {
            match reference.template_key(
                application
                    .parameters()
                    .completed_contract()
                    .template_scope(),
                scope,
            )? {
                Some(key) => application
                    .parameters()
                    .effect_reference(&key)
                    .or_else(|| {
                        (key.free_parameter().is_some()
                            && application.admits_template_parameter(&key.clone().into()))
                        .then(|| key.clone())
                    })
                    .ok_or_else(|| effect_out_of_scope(reference))?,
                None => reference.clone(),
            }
        };
        if let GenericEffectReference::Bound(bound) = &opened {
            scope.bound_effect(bound.depth(), bound.slot())?;
            return Ok(opened);
        }
        context
            .effect_eligibility(&opened, path.projection_view())
            .ok_or_else(|| effect_out_of_scope(&opened))?;
        Ok(opened)
    }
    fn type_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericTypeReference,
        scope: &GenericScope,
        application: &ConstraintApplicationScope<D>,
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericTypeReference, TypeConstraintError> {
        let opened = if matches!(reference, GenericTypeReference::Inference(_)) {
            if !application.admits_template_parameter(&reference.clone().into()) {
                return Err(type_out_of_scope(reference));
            }
            reference.clone()
        } else {
            match reference.template_key(
                application
                    .parameters()
                    .completed_contract()
                    .template_scope(),
                scope,
            )? {
                Some(key) => application
                    .parameters()
                    .type_reference(&key)
                    .or_else(|| {
                        (key.free_parameter().is_some()
                            && application.admits_template_parameter(&key.clone().into()))
                        .then(|| key.clone())
                    })
                    .ok_or_else(|| type_out_of_scope(reference))?,
                None => reference.clone(),
            }
        };
        if let GenericTypeReference::Bound(bound) = &opened {
            scope.bound_type(bound.depth(), bound.slot())?;
            return Ok(opened);
        }
        context
            .parameter_eligibility(&opened, path.projection_view())
            .ok_or_else(|| type_out_of_scope(&opened))?;
        Ok(opened)
    }

    fn const_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericConstReference,
        scope: &GenericScope,
        application: &ConstraintApplicationScope<D>,
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericConstReference, TypeConstraintError> {
        let opened = if matches!(reference, GenericConstReference::Inference(_)) {
            if !application.admits_template_parameter(&reference.clone().into()) {
                return Err(const_out_of_scope(reference));
            }
            reference.clone()
        } else {
            match reference.template_key(
                application
                    .parameters()
                    .completed_contract()
                    .template_scope(),
                scope,
            )? {
                Some(key) => application
                    .parameters()
                    .const_reference(&key)
                    .or_else(|| {
                        (key.free_parameter().is_some()
                            && application.admits_template_parameter(&key.clone().into()))
                        .then(|| key.clone())
                    })
                    .ok_or_else(|| const_out_of_scope(reference))?,
                None => reference.clone(),
            }
        };
        if let GenericConstReference::Bound(bound) = &opened {
            scope.bound_const(bound.depth(), bound.slot())?;
            return Ok(opened);
        }
        context
            .const_parameter_eligibility(&opened, path.projection_view())
            .ok_or_else(|| const_out_of_scope(&opened))?;
        Ok(opened)
    }
}
pub(super) fn type_out_of_scope(reference: &GenericTypeReference) -> TypeConstraintError {
    TypeConstraintInvariant::ParameterScope(
        TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope {
            parameter: reference.clone(),
        },
    )
    .into()
}

pub(super) fn const_out_of_scope(reference: &GenericConstReference) -> TypeConstraintError {
    TypeConstraintInvariant::ParameterScope(
        TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope {
            parameter: reference.clone(),
        },
    )
    .into()
}

pub(super) fn effect_out_of_scope(reference: &GenericEffectReference) -> TypeConstraintError {
    super::effect_invariant(
        super::TypeConstraintEffectInvariantKind::ForeignVariable,
        Some(reference.clone()),
    )
}
