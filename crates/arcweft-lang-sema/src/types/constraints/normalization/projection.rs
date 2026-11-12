//! Metered transitive projection of candidate bindings and scoped type terms.

use std::collections::BTreeSet;

use super::super::lexical::ConstraintLexicalScope;
use crate::types::{
    ArrayLength, GenericBinder, GenericConstReference, GenericTypeReference, TypeKind,
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

mod mode;
use crate::types::constraints::transaction::{
    ConstraintConstOperandView, ConstraintOperand, ConstraintOperandEffects, ConstraintOperandView,
};
use mode::{InspectValues, ProjectionMode, RetainOrigins};

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

enum ProjectionFrame<'ty, D: ConstraintDomain, M: ProjectionMode<'ty, D>> {
    Binding {
        parameter: &'ty GenericTypeReference,
        input: M::TypeInput,
        binding: &'ty ConstraintOperand<D>,
        enclosing: ConstraintLexicalScope,
    },
    Shape {
        seed: M::ShapeSeed,
        children: M::Children,
        projected: Vec<M::TypeOutput>,
        length: Option<M::ConstOutput>,
        enclosing: Option<ConstraintLexicalScope>,
    },
}

pub(in crate::types::constraints) fn seal_type_operand<'ty, A, D>(
    input: ConstraintOperandView<'ty, D>,
    view: ConstraintProjectionView<'ty, D>,
    visiting: &mut BTreeSet<GenericTypeReference>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ConstraintOperand<D>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let scope = input.lexical_scope().as_ref().clone();
    context.with_original_scope(scope, |context| {
        project_type_fold::<A, D, RetainOrigins>(
            input,
            view,
            ConstraintClosurePolicy::Hint,
            context,
            visiting,
            &mut BTreeSet::new(),
            &ConstraintOperandEffects::Original,
        )
        .map(|value| value.into_operand())
    })
}

pub(in crate::types::constraints) fn close_type_operand<'ty, A, D>(
    input: ConstraintOperandView<'ty, D>,
    path: &'ty super::super::transaction::ConstraintPath<D>,
    effects: &std::sync::Arc<super::ConstraintClosedEffects<D>>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ConstraintOperand<D>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    effects.require_path(path)?;
    project_closed_operand(
        input,
        path,
        ConstraintClosurePolicy::ProjectionClosed,
        effects,
        context,
    )
}

/// Retain the exact operand's derivation while closing it under this branch's C.
pub(in crate::types) fn project_closed_operand<'ty, A, D, L>(
    input: ConstraintOperandView<'ty, D>,
    branch: &'ty super::super::transaction::ConstraintInferenceBranch<D, L>,
    policy: ConstraintClosurePolicy,
    effects: &std::sync::Arc<super::ConstraintClosedEffects<D>>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ConstraintOperand<D>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    effects.require_branch(branch)?;
    let scope = input.lexical_scope().as_ref().clone();
    context.with_original_scope(scope, |context| {
        project_type_fold::<A, D, RetainOrigins>(
            input,
            branch.projection_view(),
            policy,
            context,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
            &ConstraintOperandEffects::Closed(std::sync::Arc::clone(effects)),
        )
        .map(|value| value.into_operand())
    })
}

/// Read-only closure uses the same scoped header substitution as retained
/// operands. Its raw result cannot acquire provenance or Free authorization.
pub(in crate::types::constraints) fn project_closed_type<A, D, L>(
    ty: &TypeKind,
    branch: &super::super::transaction::ConstraintInferenceBranch<D, L>,
    policy: ConstraintClosurePolicy,
    effects: &std::sync::Arc<super::ConstraintClosedEffects<D>>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<TypeKind, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    effects.require_branch(branch)?;
    project_type_fold::<A, D, InspectValues>(
        ty,
        branch.projection_view(),
        policy,
        context,
        &mut BTreeSet::new(),
        &mut BTreeSet::new(),
        &ConstraintOperandEffects::Closed(std::sync::Arc::clone(effects)),
    )
}

pub(super) fn seal_const_operand<'ty, A, D>(
    input: ConstraintConstOperandView<'ty, D>,
    view: ConstraintProjectionView<'ty, D>,
    visiting: &mut BTreeSet<GenericConstReference>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ConstraintOperand<D, ArrayLength>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let scope = input.lexical_scope().as_ref().clone();
    context.with_original_scope(scope, |context| {
        project_length_fold::<A, D, RetainOrigins>(
            input,
            view,
            ConstraintClosurePolicy::Hint,
            context,
            visiting,
            &mut BTreeSet::new(),
        )
        .map(|value| value.into_operand())
    })
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
    project_type_fold::<A, D, InspectValues>(
        ty,
        view,
        policy,
        context,
        visiting,
        remaining,
        &ConstraintOperandEffects::Original,
    )
}
fn project_type_fold<'ty, A, D, M>(
    ty: M::TypeInput,
    view: ConstraintProjectionView<'ty, D>,
    policy: ConstraintClosurePolicy,
    context: &mut TypeConstraintContext<'_, A, D>,
    visiting: &mut BTreeSet<GenericTypeReference>,
    remaining: &mut BTreeSet<RemainingConstraintParameter>,
    effects: &M::Effects,
) -> Result<M::TypeOutput, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    M: ProjectionMode<'ty, D>,
{
    let mut frames: Vec<ProjectionFrame<'ty, D, M>> = Vec::new();
    let result = context.with_binder(GenericBinder::EMPTY, |context| {
        let mut current = ty;
        loop {
            context.check_cancelled()?;
            context.enter_node()?;
            let shape = M::ty(&current).constraint_shape();
            let mut value = match shape {
                TypeConstraintShape::Unresolved(_) => {
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
                        && let Some(bound) = view.binding(parameter)?
                    {
                        if visiting.insert(parameter.clone()) {
                            let enclosing = context.replace_lexical_scope(bound.scope().clone());
                            frames.push(ProjectionFrame::Binding {
                                parameter,
                                input: current,
                                binding: bound,
                                enclosing,
                            });
                            current = M::type_binding(bound);
                            continue;
                        }
                        match policy {
                            ConstraintClosurePolicy::Validation
                            | ConstraintClosurePolicy::Hint
                            | ConstraintClosurePolicy::ResultAnnotation => {
                                remaining
                                    .insert(RemainingConstraintParameter(parameter.clone().into()));
                                M::type_leaf(&current)
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
                        M::type_leaf(&current)
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
                    let length = match M::array_length(&current) {
                        Some(length) => Some(project_length_fold::<A, D, M>(
                            length,
                            view,
                            policy,
                            context,
                            &mut BTreeSet::new(),
                            remaining,
                        )?),
                        None => None,
                    };
                    let enclosing = (!shape.binder().is_empty())
                        .then(|| context.enter_binder_scope(shape.binder()));
                    if let TypeConstraintShape::Function {
                        effects, predicate, ..
                    } = shape
                    {
                        context.validate_effect_predicate(predicate, view)?;
                        for variable in predicate.variables() {
                            if context.effect_eligibility(variable, view)
                                != Some(crate::effect_row::EffectConstraintEligibility::Rigid)
                            {
                                remaining
                                    .insert(RemainingConstraintParameter(variable.clone().into()));
                            }
                        }
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
                        } else if !matches!(
                            policy,
                            ConstraintClosurePolicy::ProjectionFuture
                                | ConstraintClosurePolicy::ResultAnnotation
                        ) {
                            // A future projection may carry an unresolved source
                            // function row whose owner closes it later. Keep it
                            // unknown; only effect constraints may require a
                            // concrete row or an issuer-backed variable.
                            context.validate_effect_row(effects, view)?;
                        }
                    }
                    let (mut children, seed) = M::children(&current, context)?;
                    if let Some(child) = children.next() {
                        frames.push(ProjectionFrame::Shape {
                            seed,
                            children,
                            projected: Vec::new(),
                            length,
                            enclosing,
                        });
                        current = child;
                        continue;
                    }
                    rebuild::<A, D, M>(seed, Vec::new(), length, enclosing, effects, context)?
                }
            };
            loop {
                match frames.pop() {
                    None => return Ok(value),
                    Some(ProjectionFrame::Binding {
                        parameter,
                        input,
                        binding,
                        enclosing,
                    }) => {
                        visiting.remove(parameter);
                        let owner = context.replace_lexical_scope(enclosing.clone());
                        value = M::substitute_type(
                            &input, binding, value, &enclosing, &owner, parameter, context,
                        )?;
                    }
                    Some(ProjectionFrame::Shape {
                        seed,
                        mut children,
                        mut projected,
                        length,
                        enclosing,
                    }) => {
                        projected.push(value);
                        if let Some(child) = children.next() {
                            frames.push(ProjectionFrame::Shape {
                                seed,
                                children,
                                projected,
                                length,
                                enclosing,
                            });
                            current = child;
                            break;
                        }
                        value = rebuild::<A, D, M>(
                            seed, projected, length, enclosing, effects, context,
                        )?;
                    }
                }
            }
        }
    });
    // On failure, only binding guards still on our own stack were introduced
    // by this call. Preserve any caller-supplied cycle guard exactly.
    for frame in frames {
        if let ProjectionFrame::Binding { parameter, .. } = frame {
            visiting.remove(parameter);
        }
    }
    result
}

fn rebuild<'ty, A, D, M>(
    seed: M::ShapeSeed,
    children: Vec<M::TypeOutput>,
    length: Option<M::ConstOutput>,
    enclosing: Option<ConstraintLexicalScope>,
    effects: &M::Effects,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<M::TypeOutput, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    M: ProjectionMode<'ty, D>,
{
    // Headers are expressed inside their actual local binder. Both output
    // modes rebuild there, and restore the enclosing scope even on failure.
    let result = M::rebuild(seed, children, length, effects, context);
    if let Some(enclosing) = enclosing {
        context.restore_binder_scope(enclosing);
    }
    result
}
fn allows_unbound_type(
    policy: ConstraintClosurePolicy,
    eligibility: TypeConstraintParameterEligibility,
) -> bool {
    match policy {
        ConstraintClosurePolicy::Validation
        | ConstraintClosurePolicy::Hint
        | ConstraintClosurePolicy::ResultAnnotation => true,
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
    project_length_fold::<A, D, InspectValues>(length, view, policy, context, visiting, remaining)
}
fn project_length_fold<'ty, A, D, M>(
    length: M::ConstInput,
    view: ConstraintProjectionView<'ty, D>,
    policy: ConstraintClosurePolicy,
    context: &mut TypeConstraintContext<'_, A, D>,
    visiting: &mut BTreeSet<GenericConstReference>,
    remaining: &mut BTreeSet<RemainingConstraintParameter>,
) -> Result<M::ConstOutput, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    M: ProjectionMode<'ty, D>,
{
    let mut entered = Vec::new();
    let result = context.with_binder(GenericBinder::EMPTY, |context| {
        let mut current = length;
        loop {
            context.check_cancelled()?;
            context.enter_node()?;
            match M::length(&current) {
                ArrayLength::Const(_) => return Ok(M::const_leaf(&current)),
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
                        && let Some(bound) = view.const_binding(parameter)?
                    {
                        if !visiting.insert(parameter.clone()) {
                            return match policy {
                                ConstraintClosurePolicy::Validation
                                | ConstraintClosurePolicy::Hint
                                | ConstraintClosurePolicy::ResultAnnotation => {
                                    remaining.insert(RemainingConstraintParameter(
                                        parameter.clone().into(),
                                    ));
                                    Ok(M::const_leaf(&current))
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
                        let enclosing = context.replace_lexical_scope(bound.scope().clone());
                        entered.push((parameter, current, bound, enclosing));
                        current = M::const_binding(bound);
                        continue;
                    }
                    if allows_unbound_const(policy, eligibility) {
                        if eligibility != TypeConstraintConstEligibility::Rigid {
                            remaining
                                .insert(RemainingConstraintParameter(parameter.clone().into()));
                        }
                        return Ok(M::const_leaf(&current));
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
    });
    // The scoped loop restored its caller on return; replay each edge's owner
    // frame explicitly while lifting the final scalar back to that caller.
    let mut result = result;
    for (parameter, input, binding, enclosing) in entered.into_iter().rev() {
        visiting.remove(parameter);
        if let Ok(value) = result {
            result = M::substitute_const(
                &input,
                binding,
                value,
                &enclosing,
                binding.scope(),
                parameter,
                context,
            );
        }
    }
    result
}

fn allows_unbound_const(
    policy: ConstraintClosurePolicy,
    eligibility: TypeConstraintConstEligibility,
) -> bool {
    match policy {
        ConstraintClosurePolicy::Validation
        | ConstraintClosurePolicy::Hint
        | ConstraintClosurePolicy::ResultAnnotation => true,
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
