//! Metered transitive projection of candidate bindings and scoped type terms.

use std::collections::BTreeSet;

use crate::types::{
    ArrayLength, GenericBinder, GenericConstReference, GenericScope, GenericTypeReference, TypeKind,
};

use super::super::{
    ConstraintDomain, TypeConstraintConstEligibility, TypeConstraintError, TypeConstraintInvariant,
    TypeConstraintParameterEligibility, TypeConstraintParameterScopeInvariant,
    TypeConstraintRejection, TypeConstraintShape,
    context::{TypeConstraintAccounting, TypeConstraintContext},
    shape::TypeConstraintChildren,
};
use super::{
    ConstraintClosurePolicy, ConstraintProjectionView, ProjectedConstraintType,
    RemainingConstraintParameter,
};

/// Each type/constant occurrence is admitted before lookup or reconstruction.
/// Bindings are followed transitively, with branch-local cycle guards; they
/// are not the simultaneous closed substitutions used by runtime instances.
pub(in crate::types::constraints) fn project_type<A, D>(
    ty: &TypeKind,
    view: ConstraintProjectionView<'_, D>,
    policy: ConstraintClosurePolicy,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ProjectedConstraintType, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let mut visiting = BTreeSet::new();
    let mut remaining = BTreeSet::new();
    let value = project_type_inner(ty, view, policy, context, &mut visiting, &mut remaining)?;
    Ok(ProjectedConstraintType {
        value,
        remaining: remaining.into_iter().collect(),
    })
}

enum ProjectionFrame<'ty> {
    Binding(&'ty GenericTypeReference),
    Shape {
        shape: TypeConstraintShape<'ty>,
        children: TypeConstraintChildren<'ty>,
        projected: Vec<TypeKind>,
        length: Option<ArrayLength>,
        enclosing: Option<GenericScope>,
    },
}

pub(super) fn project_type_inner<'ty, A, D>(
    ty: &'ty TypeKind,
    view: ConstraintProjectionView<'ty, D>,
    policy: ConstraintClosurePolicy,
    context: &mut TypeConstraintContext<'_, A, D>,
    visiting: &mut BTreeSet<GenericTypeReference>,
    remaining: &mut BTreeSet<RemainingConstraintParameter>,
) -> Result<TypeKind, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let mut frames = Vec::new();
    let result = context.with_binder(GenericBinder::EMPTY, |context| {
        let mut current = ty;
        loop {
            context.check_cancelled()?;
            context.enter_node()?;
            let shape = current.constraint_shape();
            let mut value = match shape {
                TypeConstraintShape::Unresolved => {
                    return Err(TypeConstraintRejection::UnresolvedType.into());
                }
                TypeConstraintShape::Generic(parameter) => {
                    let eligibility = context.parameter_eligibility(parameter, view).ok_or_else(
                        || {
                            TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
                                TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope {
                                    parameter: parameter.clone(),
                                },
                            ))
                        },
                    )?;
                    if !matches!(policy, ConstraintClosurePolicy::Validation)
                        && let Some(bound) = view.binding(parameter)
                    {
                        if visiting.insert(parameter.clone()) {
                            frames.push(ProjectionFrame::Binding(parameter));
                            current = bound;
                            continue;
                        }
                        match policy {
                            ConstraintClosurePolicy::Validation | ConstraintClosurePolicy::Hint => {
                                remaining
                                    .insert(RemainingConstraintParameter(parameter.clone().into()));
                                TypeKind::GenericParam(parameter.clone())
                            }
                            ConstraintClosurePolicy::ProjectionClosed
                            | ConstraintClosurePolicy::ProjectionFuture
                            | ConstraintClosurePolicy::SolutionCompletion => {
                                return Err(TypeConstraintRejection::CyclicInstantiation {
                                    parameter: parameter.clone().into(),
                                }
                                .into());
                            }
                        }
                    } else if allows_unbound_type(policy, eligibility) {
                        if eligibility != TypeConstraintParameterEligibility::Rigid {
                            remaining
                                .insert(RemainingConstraintParameter(parameter.clone().into()));
                        }
                        TypeKind::GenericParam(parameter.clone())
                    } else {
                        return Err(TypeConstraintRejection::IncompleteInstantiation {
                            parameter: parameter.clone().into(),
                        }
                        .into());
                    }
                }
                shape => {
                    // Scalar lengths precede type children in this relation's
                    // admission/error order. Rebuild consumes that result once.
                    let length = match shape {
                        TypeConstraintShape::Array { len, .. } => Some(project_array_length(
                            len,
                            view,
                            policy,
                            context,
                            &mut BTreeSet::new(),
                            remaining,
                        )?),
                        _ => None,
                    };
                    let enclosing = (!shape.binder().is_empty())
                        .then(|| context.enter_binder_scope(shape.binder()));
                    if let TypeConstraintShape::Function { effects, .. } = shape {
                        if effects.is_known() {
                            context.validate_effect_row(effects, view)?;
                            for variable in effects.variables().expect("validated effect row") {
                                if context.effect_eligibility(variable, view)
                                    != Some(crate::effect_row::EffectConstraintEligibility::Rigid)
                                {
                                    remaining.insert(RemainingConstraintParameter(
                                        variable.clone().into(),
                                    ));
                                }
                            }
                        } else if policy != ConstraintClosurePolicy::ProjectionFuture {
                            // A future projection may carry an unresolved source
                            // function row whose owner closes it later. Keep it
                            // unknown; only effect constraints may require a
                            // concrete row or an issuer-backed variable.
                            context.validate_effect_row(effects, view)?;
                        }
                    }
                    let mut children = shape.children();
                    if let Some(child) = children.next() {
                        frames.push(ProjectionFrame::Shape {
                            shape,
                            children,
                            projected: Vec::new(),
                            length,
                            enclosing,
                        });
                        current = child;
                        continue;
                    }
                    rebuild(shape, Vec::new(), length, enclosing, context)?
                }
            };
            loop {
                match frames.pop() {
                    None => return Ok(value),
                    Some(ProjectionFrame::Binding(parameter)) => {
                        visiting.remove(parameter);
                    }
                    Some(ProjectionFrame::Shape {
                        shape,
                        mut children,
                        mut projected,
                        length,
                        enclosing,
                    }) => {
                        projected.push(value);
                        if let Some(child) = children.next() {
                            frames.push(ProjectionFrame::Shape {
                                shape,
                                children,
                                projected,
                                length,
                                enclosing,
                            });
                            current = child;
                            break;
                        }
                        value = rebuild(shape, projected, length, enclosing, context)?;
                    }
                }
            }
        }
    });
    // On failure, only binding guards still on our own stack were introduced
    // by this call. Preserve any caller-supplied cycle guard exactly.
    for frame in frames {
        if let ProjectionFrame::Binding(parameter) = frame {
            visiting.remove(parameter);
        }
    }
    result
}

fn rebuild<A: TypeConstraintAccounting, D: ConstraintDomain>(
    shape: TypeConstraintShape<'_>,
    children: Vec<TypeKind>,
    length: Option<ArrayLength>,
    enclosing: Option<GenericScope>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<TypeKind, TypeConstraintError> {
    if let Some(enclosing) = enclosing {
        context.restore_binder_scope(enclosing);
    }
    shape.rebuild_with(
        children,
        &mut (),
        |(), _| Ok(length.expect("array length was projected before its children")),
        |(), row| Ok(row.clone()),
    )
}

fn allows_unbound_type(
    policy: ConstraintClosurePolicy,
    eligibility: TypeConstraintParameterEligibility,
) -> bool {
    match policy {
        ConstraintClosurePolicy::Validation | ConstraintClosurePolicy::Hint => true,
        ConstraintClosurePolicy::ProjectionClosed => {
            eligibility == TypeConstraintParameterEligibility::Rigid
        }
        ConstraintClosurePolicy::ProjectionFuture | ConstraintClosurePolicy::SolutionCompletion => {
            matches!(
                eligibility,
                TypeConstraintParameterEligibility::Rigid
                    | TypeConstraintParameterEligibility::FutureEligible
            )
        }
    }
}

pub(in crate::types::constraints) fn project_const_argument<A, D>(
    value: &ArrayLength,
    view: ConstraintProjectionView<'_, D>,
    policy: ConstraintClosurePolicy,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ArrayLength, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    project_array_length(
        value,
        view,
        policy,
        context,
        &mut BTreeSet::new(),
        &mut BTreeSet::new(),
    )
}

pub(super) fn project_array_length<'ty, A, D>(
    length: &'ty ArrayLength,
    view: ConstraintProjectionView<'ty, D>,
    policy: ConstraintClosurePolicy,
    context: &mut TypeConstraintContext<'_, A, D>,
    visiting: &mut BTreeSet<GenericConstReference>,
    remaining: &mut BTreeSet<RemainingConstraintParameter>,
) -> Result<ArrayLength, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let mut entered = Vec::new();
    let result = (|| {
        let mut current = length;
        loop {
            context.check_cancelled()?;
            context.enter_node()?;
            match current {
                ArrayLength::Const(_) => return Ok(current.clone()),
                ArrayLength::Generic(parameter) => {
                    let eligibility = context
                        .const_parameter_eligibility(parameter, view)
                        .ok_or_else(|| {
                            TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
                                TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope {
                                    parameter: parameter.clone(),
                                },
                            ))
                        })?;
                    if !matches!(policy, ConstraintClosurePolicy::Validation)
                        && let Some(bound) = view.const_binding(parameter)
                    {
                        if !visiting.insert(parameter.clone()) {
                            return match policy {
                                ConstraintClosurePolicy::Validation
                                | ConstraintClosurePolicy::Hint => {
                                    remaining.insert(RemainingConstraintParameter(
                                        parameter.clone().into(),
                                    ));
                                    Ok(ArrayLength::Generic(parameter.clone()))
                                }
                                ConstraintClosurePolicy::ProjectionClosed
                                | ConstraintClosurePolicy::ProjectionFuture
                                | ConstraintClosurePolicy::SolutionCompletion => {
                                    Err(TypeConstraintRejection::CyclicInstantiation {
                                        parameter: parameter.clone().into(),
                                    }
                                    .into())
                                }
                            };
                        }
                        entered.push(parameter);
                        current = bound;
                        continue;
                    }
                    if allows_unbound_const(policy, eligibility) {
                        if eligibility != TypeConstraintConstEligibility::Rigid {
                            remaining
                                .insert(RemainingConstraintParameter(parameter.clone().into()));
                        }
                        return Ok(ArrayLength::Generic(parameter.clone()));
                    }
                    return Err(TypeConstraintRejection::IncompleteInstantiation {
                        parameter: parameter.clone().into(),
                    }
                    .into());
                }
                ArrayLength::Error(_) | ArrayLength::Inferred => {
                    return Err(TypeConstraintRejection::UnresolvedType.into());
                }
            }
        }
    })();
    for parameter in entered {
        visiting.remove(parameter);
    }
    result
}

fn allows_unbound_const(
    policy: ConstraintClosurePolicy,
    eligibility: TypeConstraintConstEligibility,
) -> bool {
    match policy {
        ConstraintClosurePolicy::Validation | ConstraintClosurePolicy::Hint => true,
        ConstraintClosurePolicy::ProjectionClosed => {
            eligibility == TypeConstraintConstEligibility::Rigid
        }
        ConstraintClosurePolicy::ProjectionFuture | ConstraintClosurePolicy::SolutionCompletion => {
            matches!(
                eligibility,
                TypeConstraintConstEligibility::Rigid
                    | TypeConstraintConstEligibility::FutureEligible
            )
        }
    }
}
