//! Scope-aware reference mapping over the shared semantic type shape.

use super::super::{ArrayLength, GenericConstReference, GenericTypeReference, TypeKind};
use super::context::{TypeConstraintAccounting, TypeConstraintContext};
use super::{
    ConstraintDomain, TypeConstraintError, TypeConstraintInvariant,
    TypeConstraintParameterScopeInvariant,
};

#[cfg(test)]
mod tests;

pub(super) trait ConstraintReferenceMap {
    fn type_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericTypeReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericTypeReference, TypeConstraintError>;

    fn const_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericConstReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericConstReference, TypeConstraintError>;
}

pub(super) fn map_type<A, D>(
    ty: &TypeKind,
    mapping: &impl ConstraintReferenceMap,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<TypeKind, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    context.check_cancelled()?;
    context.enter_node()?;
    if let TypeKind::GenericParam(reference) = ty {
        return mapping
            .type_reference(reference, context)
            .map(TypeKind::GenericParam);
    }
    let shape = ty.constraint_shape();
    context.with_binder(shape.binder(), |context| {
        let children = shape
            .children()
            .map(|child| map_type(child, mapping, context))
            .collect::<Result<Vec<_>, _>>()?;
        let mut result = shape.rebuild(children)?;
        if let TypeKind::Array { len, .. } = &mut result {
            *len = map_length(len, mapping, context)?;
        }
        Ok(result)
    })
}

pub(super) fn map_length<A, D>(
    length: &ArrayLength,
    mapping: &impl ConstraintReferenceMap,
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
            .const_reference(reference, context)
            .map(ArrayLength::Generic),
        _ => Ok(length.clone()),
    }
}

pub(super) struct OpenTemplateReferences;

impl ConstraintReferenceMap for OpenTemplateReferences {
    fn type_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericTypeReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericTypeReference, TypeConstraintError> {
        let opened = if matches!(reference, GenericTypeReference::Inference(_)) {
            reference.clone()
        } else {
            match reference.template_key(
                context
                    .parameter_scope
                    .completed_contract()
                    .template_scope(),
                context.lexical_scope(),
            )? {
                Some(key) => context
                    .parameter_scope
                    .type_reference(&key)
                    .or_else(|| key.free_parameter().map(|_| key.clone()))
                    .ok_or_else(|| type_out_of_scope(reference))?,
                None => reference.clone(),
            }
        };
        context
            .parameter_eligibility(&opened)
            .ok_or_else(|| type_out_of_scope(&opened))?;
        Ok(opened)
    }

    fn const_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericConstReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericConstReference, TypeConstraintError> {
        let opened = if matches!(reference, GenericConstReference::Inference(_)) {
            reference.clone()
        } else {
            match reference.template_key(
                context
                    .parameter_scope
                    .completed_contract()
                    .template_scope(),
                context.lexical_scope(),
            )? {
                Some(key) => context
                    .parameter_scope
                    .const_reference(&key)
                    .or_else(|| key.free_parameter().map(|_| key.clone()))
                    .ok_or_else(|| const_out_of_scope(reference))?,
                None => reference.clone(),
            }
        };
        context
            .const_parameter_eligibility(&opened)
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
