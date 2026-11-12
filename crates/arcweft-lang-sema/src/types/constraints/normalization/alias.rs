//! Direct aliases retain each actual occurrence, replacement and owner edge.

use super::*;
use crate::types::constraints::transaction::{
    ConstraintAliasStep, ConstraintConstOperandView, ConstraintOperandView,
};

pub(in crate::types) enum ConstraintAliasResolution<T, P> {
    Unchanged,
    Value(T),
    Cycle(Box<[P]>),
}

pub(in crate::types) fn resolve_type_alias<A, D, L>(
    input: ConstraintOperandView<'_, D>,
    path: &ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<
    ConstraintAliasResolution<ConstraintOperand<D>, GenericTypeReference>,
    TypeConstraintError,
>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let TypeKind::GenericParam(parameter) = input.value() else {
        return Ok(ConstraintAliasResolution::Unchanged);
    };
    if matches!(parameter, GenericTypeReference::Bound(_)) {
        return Ok(ConstraintAliasResolution::Unchanged);
    }
    context.with_binder(super::super::super::GenericBinder::EMPTY, |context| {
        let view = path.projection_view();
        let mut parameter = parameter.clone();
        let mut positions = BTreeMap::new();
        let mut chain = Vec::new();
        let mut steps = Vec::new();
        let mut current: Option<&ConstraintOperand<D>> = None;
        let mut value = loop {
            context.check_cancelled()?;
            context.enter_node()?;
            if context.parameter_eligibility(&parameter, view).is_none() {
                return Err(TypeConstraintInvariant::ParameterScope(
                    super::super::TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope {
                        parameter,
                    },
                )
                .into());
            }
            if let Some(start) = positions.get(&parameter).copied() {
                let mut cycle = chain[start..].to_vec();
                cycle.sort();
                cycle.dedup();
                return Ok(ConstraintAliasResolution::Cycle(cycle.into_boxed_slice()));
            }
            positions.insert(parameter.clone(), chain.len());
            chain.push(parameter.clone());
            let Some(bound) = view.binding(&parameter)? else {
                let Some(current) = current else {
                    return Ok(ConstraintAliasResolution::Unchanged);
                };
                break current.clone();
            };
            let occurrence = current.map_or_else(|| input.retained(), Clone::clone);
            context.replace_lexical_scope(bound.scope().clone());
            steps.push(ConstraintAliasStep {
                occurrence,
                replacement: bound.clone(),
            });
            current = Some(bound);
            match bound.value() {
                TypeKind::GenericParam(next) => parameter = next.clone(),
                _ => break bound.clone(),
            }
        };
        let terminal = value.clone();
        for step in steps.iter().rev() {
            let TypeKind::GenericParam(parameter) = step.occurrence.value() else {
                unreachable!("a direct alias occurrence is a generic reference")
            };
            let target = step.occurrence.shared_scope();
            context.replace_lexical_scope(target.as_ref().clone());
            if value.scope() != target.as_ref() {
                value = value.transport(&target, context).map_err(|error| {
                    TypeConstraintContext::<A, D>::binding_transport_error(
                        error,
                        parameter.clone().into(),
                    )
                })?;
            }
        }
        Ok(ConstraintAliasResolution::Value(
            value.aliased(steps.into_boxed_slice(), terminal)?,
        ))
    })
}

pub(in crate::types) fn resolve_const_alias<A, D, L>(
    input: ConstraintConstOperandView<'_, D>,
    path: &ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<
    ConstraintAliasResolution<ConstraintOperand<D, ArrayLength>, GenericConstReference>,
    TypeConstraintError,
>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let ArrayLength::Generic(parameter) = input.value() else {
        return Ok(ConstraintAliasResolution::Unchanged);
    };
    if matches!(parameter, GenericConstReference::Bound(_)) {
        return Ok(ConstraintAliasResolution::Unchanged);
    }
    context.with_binder(super::super::super::GenericBinder::EMPTY, |context| {
        let view = path.projection_view();
        let mut parameter = parameter.clone();
        let mut positions = BTreeMap::new();
        let mut chain = Vec::new();
        let mut steps = Vec::new();
        let mut current: Option<&ConstraintOperand<D, ArrayLength>> = None;
        let mut value = loop {
            context.check_cancelled()?;
            context.enter_node()?;
            if context
                .const_parameter_eligibility(&parameter, view)
                .is_none()
            {
                return Err(TypeConstraintInvariant::ParameterScope(
                    super::super::TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope {
                        parameter,
                    },
                )
                .into());
            }
            if let Some(start) = positions.get(&parameter).copied() {
                let mut cycle = chain[start..].to_vec();
                cycle.sort();
                cycle.dedup();
                return Ok(ConstraintAliasResolution::Cycle(cycle.into_boxed_slice()));
            }
            positions.insert(parameter.clone(), chain.len());
            chain.push(parameter.clone());
            let Some(bound) = view.const_binding(&parameter)? else {
                let Some(current) = current else {
                    return Ok(ConstraintAliasResolution::Unchanged);
                };
                break current.clone();
            };
            let occurrence = current.map_or_else(|| input.retained(), Clone::clone);
            context.replace_lexical_scope(bound.scope().clone());
            steps.push(ConstraintAliasStep {
                occurrence,
                replacement: bound.clone(),
            });
            current = Some(bound);
            match bound.value() {
                ArrayLength::Generic(next) => parameter = next.clone(),
                _ => break bound.clone(),
            }
        };
        let terminal = value.clone();
        for step in steps.iter().rev() {
            let ArrayLength::Generic(parameter) = step.occurrence.value() else {
                unreachable!("a direct alias occurrence is a generic reference")
            };
            let target = step.occurrence.shared_scope();
            context.replace_lexical_scope(target.as_ref().clone());
            if value.scope() != target.as_ref() {
                value = value.transport(&target, context).map_err(|error| {
                    TypeConstraintContext::<A, D>::binding_transport_error(
                        error,
                        parameter.clone().into(),
                    )
                })?;
            }
        }
        Ok(ConstraintAliasResolution::Value(
            value.aliased(steps.into_boxed_slice(), terminal)?,
        ))
    })
}
