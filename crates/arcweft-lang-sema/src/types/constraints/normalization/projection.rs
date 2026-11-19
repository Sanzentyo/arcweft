//! Metered transitive projection of candidate bindings and scoped type terms.

use crate::effect_row::prepared::{PreparedEffectGraph, PreparedEffectReadView};
use crate::types::{CanonicalEffects, PreparedEffects};

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
#[cfg(test)]
use super::ConstraintClosedEffects;
use super::{
    ConstraintClosurePolicy, ConstraintEffectProjection, ConstraintProjectionView,
    ProjectedConstraintType, RemainingConstraintParameter,
};

mod mode;
mod phase;
use crate::types::constraints::transaction::{
    AdmittedConstraintFunctionHeader, ConstraintConstOperandView, ConstraintFunctionHeaderView,
    ConstraintOperand, ConstraintOperandView,
};
use mode::{
    AdmitOrigins, InspectOperand, InspectValues, ProjectionChildren, ProjectionMode, RetainOrigins,
};
pub(in crate::types::constraints) use phase::ConstraintProjectionPhase;

impl<'ty, D: ConstraintDomain> ConstraintProjectionView<'ty, D> {
    /// Validate every port against its own derivation and actual frames. This
    /// emits no rebuilt type, follows no active binding, and solves no effect.
    pub(in crate::types) fn admit_operand<A: TypeConstraintAccounting>(
        self,
        input: ConstraintOperandView<'ty, D>,
        graph: PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        for _ in input.lexical_scope().shape().binders() {
            context.enter_node()?;
        }
        let scope = input.lexical_scope().as_ref().clone();
        context.with_original_scope(scope, |context| {
            project_type_fold::<A, D, AdmitOrigins>(
                input,
                self,
                ConstraintClosurePolicy::Validation,
                &mut ConstraintProjectionPhase::Admission(graph),
                context,
                &mut BTreeSet::new(),
                &mut BTreeSet::new(),
            )
        })
    }
}

/// Each type/constant occurrence is admitted before lookup or reconstruction.
/// Bindings are followed transitively, with branch-local cycle guards; they
/// are not the simultaneous closed substitutions used by runtime instances.
pub(in crate::types::constraints) fn project_type<A, D>(
    ty: &TypeKind<PreparedEffects>,
    view: ConstraintProjectionView<'_, D>,
    policy: ConstraintClosurePolicy,
    graph: &mut PreparedEffectGraph,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ProjectedConstraintType, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let mut visiting = BTreeSet::new();
    let mut remaining = BTreeSet::new();
    let value = project_type_inner(
        ty,
        view,
        policy,
        graph,
        context,
        &mut visiting,
        &mut remaining,
    )?;
    Ok(ProjectedConstraintType {
        value,
        remaining: remaining.into_iter().collect(),
    })
}

pub(super) fn validate_type_structure<A: TypeConstraintAccounting, D: ConstraintDomain>(
    ty: &TypeKind<PreparedEffects>,
    view: ConstraintProjectionView<'_, D>,
    graph: PreparedEffectReadView<'_>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<(), TypeConstraintError> {
    project_type_fold::<A, D, InspectValues>(
        ty,
        view,
        ConstraintClosurePolicy::Validation,
        &mut ConstraintProjectionPhase::Admission(graph),
        context,
        &mut BTreeSet::new(),
        &mut BTreeSet::new(),
    )
    .map(|_| ())
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
        header: Option<AdmittedConstraintFunctionHeader<'ty, D>>,
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
    graph: &mut PreparedEffectGraph,
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
            &mut ConstraintProjectionPhase::Preparation(graph),
            context,
            visiting,
            &mut BTreeSet::new(),
        )
        .map(|value| value.into_operand())
    })
}

#[cfg(test)]
pub(in crate::types::constraints) fn close_type_operand<'ty, A, D, L>(
    input: ConstraintOperandView<'ty, D>,
    path: &'ty super::super::transaction::ConstraintInferenceBranch<D, L>,
    effects: &std::sync::Arc<ConstraintClosedEffects<D>>,
    graph: PreparedEffectReadView<'_>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ConstraintOperand<D>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    project_closed_operand(
        input,
        path,
        ConstraintClosurePolicy::ProjectionClosed,
        effects,
        graph,
        context,
    )
}

/// Retain the exact operand's derivation while closing it under this branch's C.
#[cfg(test)]
pub(in crate::types::constraints) fn project_closed_operand<'ty, A, D, L>(
    input: ConstraintOperandView<'ty, D>,
    branch: &'ty super::super::transaction::ConstraintInferenceBranch<D, L>,
    policy: ConstraintClosurePolicy,
    effects: &std::sync::Arc<ConstraintClosedEffects<D>>,
    graph: PreparedEffectReadView<'_>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ConstraintOperand<D>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    effects
        .projection()
        .project_operand(input, branch, policy, graph, context)
}

impl<D: ConstraintDomain> ConstraintEffectProjection<D> {
    /// Verify the original ports while computing the result of an equality
    /// check. The type leaves this fold only for comparison, without a new
    /// source derivation.
    pub(in crate::types::constraints) fn inspect_operand<'ty, A, L>(
        &mut self,
        input: ConstraintOperandView<'ty, D>,
        branch: &'ty super::super::transaction::ConstraintInferenceBranch<D, L>,
        policy: ConstraintClosurePolicy,
        graph: PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        self.owner().require_branch(branch)?;
        let scope = input.lexical_scope().as_ref().clone();
        context.with_original_scope(scope, |context| {
            project_type_fold::<A, D, InspectOperand>(
                input,
                branch.projection_view(),
                policy,
                &mut ConstraintProjectionPhase::Completion {
                    graph,
                    projection: self,
                },
                context,
                &mut BTreeSet::new(),
                &mut BTreeSet::new(),
            )
        })
    }

    pub(in crate::types::constraints) fn project_operand<'ty, A, L>(
        &mut self,
        input: ConstraintOperandView<'ty, D>,
        branch: &'ty super::super::transaction::ConstraintInferenceBranch<D, L>,
        policy: ConstraintClosurePolicy,
        graph: PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ConstraintOperand<D>, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        self.owner().require_branch(branch)?;
        let scope = input.lexical_scope().as_ref().clone();
        context.with_original_scope(scope, |context| {
            project_type_fold::<A, D, RetainOrigins>(
                input,
                branch.projection_view(),
                policy,
                &mut ConstraintProjectionPhase::Completion {
                    graph,
                    projection: self,
                },
                context,
                &mut BTreeSet::new(),
                &mut BTreeSet::new(),
            )
            .map(|value| value.into_operand())
        })
    }
}

/// Inspect the canonical algebra directly in the binding fold for read-only
/// equation checks.
#[cfg(test)]
pub(in crate::types::constraints) fn project_closed_type<A, D, L>(
    ty: &TypeKind<PreparedEffects>,
    branch: &super::super::transaction::ConstraintInferenceBranch<D, L>,
    policy: ConstraintClosurePolicy,
    effects: &std::sync::Arc<ConstraintClosedEffects<D>>,
    graph: PreparedEffectReadView<'_>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<TypeKind, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    effects
        .projection()
        .project_type(ty, branch, policy, graph, context)
}

impl<D: ConstraintDomain> ConstraintEffectProjection<D> {
    pub(in crate::types::constraints) fn project_type<A: TypeConstraintAccounting, L>(
        &mut self,
        ty: &TypeKind<PreparedEffects>,
        branch: &super::super::transaction::ConstraintInferenceBranch<D, L>,
        policy: ConstraintClosurePolicy,
        graph: PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        self.owner().require_branch(branch)?;
        project_type_fold::<A, D, InspectValues<CanonicalEffects>>(
            ty,
            branch.projection_view(),
            policy,
            &mut ConstraintProjectionPhase::Completion {
                graph,
                projection: self,
            },
            context,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
        )
    }
}

pub(in crate::types::constraints) fn project_const_operand<'ty, A, D>(
    input: ConstraintConstOperandView<'ty, D>,
    view: ConstraintProjectionView<'ty, D>,
    policy: ConstraintClosurePolicy,
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
            policy,
            context,
            visiting,
            &mut BTreeSet::new(),
        )
        .map(|value| value.into_operand())
    })
}
pub(super) fn project_type_inner<'ty, A, D>(
    ty: &'ty TypeKind<PreparedEffects>,
    view: ConstraintProjectionView<'ty, D>,
    policy: ConstraintClosurePolicy,
    graph: &mut PreparedEffectGraph,
    context: &mut TypeConstraintContext<'_, A, D>,
    visiting: &mut BTreeSet<GenericTypeReference>,
    remaining: &mut BTreeSet<RemainingConstraintParameter>,
) -> Result<TypeKind<PreparedEffects>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    project_type_fold::<A, D, InspectValues>(
        ty,
        view,
        policy,
        &mut ConstraintProjectionPhase::Preparation(graph),
        context,
        visiting,
        remaining,
    )
}
fn project_type_fold<'ty, A, D, M>(
    ty: M::TypeInput,
    view: ConstraintProjectionView<'ty, D>,
    policy: ConstraintClosurePolicy,
    phase: &mut ConstraintProjectionPhase<'_, D>,
    context: &mut TypeConstraintContext<'_, A, D>,
    visiting: &mut BTreeSet<GenericTypeReference>,
    remaining: &mut BTreeSet<RemainingConstraintParameter>,
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
                    M::admit_type_reference(&current, parameter, context)?;
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
                    let length = match M::array_length(&current)? {
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
                    let ProjectionChildren {
                        mut children,
                        seed,
                        header,
                    } = M::children(&current, context)?;
                    let header = if let TypeConstraintShape::Function {
                        effects, predicate, ..
                    } = shape
                    {
                        let header = ConstraintFunctionHeaderView::admit_projection(
                            header,
                            effects,
                            predicate,
                            phase.read(),
                            context,
                            |variable, context| {
                                context.validate_effect_reference(variable, view)?;
                                if context.effect_eligibility(variable, view)
                                    != Some(crate::effect_row::EffectConstraintEligibility::Rigid)
                                {
                                    remaining.insert(RemainingConstraintParameter(
                                        variable.clone().into(),
                                    ));
                                }
                                Ok(())
                            },
                        )?;
                        // Scope admission is not a decision about a live row or
                        // predicate. The original relation closes them jointly.
                        // Preserve the existing authored-unknown policy only.
                        if let crate::effect_row::prepared::PreparedEffectRow::Canonical(row) =
                            effects
                            && !row.is_known()
                            && !matches!(
                                policy,
                                ConstraintClosurePolicy::ProjectionFuture
                                    | ConstraintClosurePolicy::ResultAnnotation
                            )
                        {
                            context.validate_effect_row(row, view)?;
                        }
                        header
                    } else {
                        if header.is_some() {
                            return Err(TypeConstraintInvariant::OriginalProducer.into());
                        }
                        None
                    };
                    if let Some(child) = M::next_child(&mut children)? {
                        frames.push(ProjectionFrame::Shape {
                            seed,
                            header,
                            children,
                            projected: Vec::new(),
                            length,
                            enclosing,
                        });
                        current = child;
                        continue;
                    }
                    rebuild::<A, D, M>(seed, header, Vec::new(), length, enclosing, phase, context)?
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
                            &input, binding, value, &enclosing, &owner, parameter, phase, context,
                        )?;
                    }
                    Some(ProjectionFrame::Shape {
                        seed,
                        header,
                        mut children,
                        mut projected,
                        length,
                        enclosing,
                    }) => {
                        projected.push(value);
                        if let Some(child) = M::next_child(&mut children)? {
                            frames.push(ProjectionFrame::Shape {
                                seed,
                                header,
                                children,
                                projected,
                                length,
                                enclosing,
                            });
                            current = child;
                            break;
                        }
                        value = rebuild::<A, D, M>(
                            seed, header, projected, length, enclosing, phase, context,
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
    header: Option<AdmittedConstraintFunctionHeader<'ty, D>>,
    children: Vec<M::TypeOutput>,
    length: Option<M::ConstOutput>,
    enclosing: Option<ConstraintLexicalScope>,
    phase: &mut ConstraintProjectionPhase<'_, D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<M::TypeOutput, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    M: ProjectionMode<'ty, D>,
{
    // Headers are expressed inside their actual local binder. Both output
    // modes rebuild there, and restore the enclosing scope even on failure.
    let result = M::rebuild(seed, children, length, header, phase, context);
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
                    M::admit_const_reference(&current, parameter, context)?;
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
