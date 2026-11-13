//! Relation and Choice algebra for candidate-wide constraints.

use std::collections::BTreeSet;

use super::super::{
    ArrayLength, GenericConstReference, GenericTypeReference, TypeCompatibilityFailure,
    TypeCompatibilityPolicy, TypeKind,
};
use crate::types::constraints::context::{TypeConstraintAccounting, TypeConstraintContext};
use crate::types::constraints::transaction::{
    ConstraintConstOperandView, ConstraintInferenceBranch, ConstraintOperand, ConstraintOperandView,
};
use crate::types::constraints::{
    ChoiceDerivationStep, ChoiceForkRole, ConstraintAliasResolution, ConstraintClosedEffects,
    ConstraintClosurePolicy, ConstraintDomain, TypeConstraintAbort, TypeConstraintConstEligibility,
    TypeConstraintError, TypeConstraintInvariant, TypeConstraintParameterEligibility,
    TypeConstraintRejection, TypeConstraintShape, TypeConstraintSourceProtocolInvariant,
    project_closed_operand, resolve_const_alias, resolve_type_alias, seal_path, validate_type,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ConstraintAcceptance {
    PatternAcceptsActual,
    ActualAcceptsPattern,
}

#[derive(Clone, Copy)]
enum ConstraintRelation {
    Accepts(ConstraintAcceptance),
    Equal,
}

impl ConstraintRelation {
    fn direction(self) -> ConstraintAcceptance {
        match self {
            Self::Accepts(direction) => direction,
            Self::Equal => ConstraintAcceptance::PatternAcceptsActual,
        }
    }

    fn reverse(self) -> Self {
        match self {
            Self::Accepts(ConstraintAcceptance::PatternAcceptsActual) => {
                Self::Accepts(ConstraintAcceptance::ActualAcceptsPattern)
            }
            Self::Accepts(ConstraintAcceptance::ActualAcceptsPattern) => {
                Self::Accepts(ConstraintAcceptance::PatternAcceptsActual)
            }
            Self::Equal => Self::Equal,
        }
    }
}

pub(in crate::types) fn relate_selected_call<A, D, L: Clone>(
    pattern: &ConstraintOperandView<'_, D>,
    actual: &ConstraintOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    acceptance: ConstraintAcceptance,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    relate_constraint(
        pattern,
        actual,
        path,
        context,
        ConstraintRelation::Accepts(acceptance),
    )
}

/// Equality shares the ordinary binder, alias, constant, occurs and effect
/// relation. It does not turn a subtype or a Choice member into an equal type.
pub(in crate::types) fn unify_original_binding<A, D, L: Clone>(
    pattern: &ConstraintOperandView<'_, D>,
    actual: &ConstraintOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    relate_constraint(pattern, actual, path, context, ConstraintRelation::Equal)
}

pub(in crate::types) fn unify_original_length<A, D, L: Clone>(
    pattern: &ConstraintConstOperandView<'_, D>,
    actual: &ConstraintConstOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<Option<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    relate_array_lengths(pattern, actual, path, context, ConstraintRelation::Equal)
}

fn fork_choice_path<A, D, L>(
    path: &ConstraintInferenceBranch<D, L>,
    role: ChoiceForkRole,
    expected: Option<usize>,
    actual: Option<usize>,
    relation: ConstraintRelation,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ConstraintInferenceBranch<D, L>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    let mut path = context.fork_path(path)?;
    let to_ordinal = |index: Option<usize>| {
        index
            .map(u32::try_from)
            .transpose()
            .map_err(|_| TypeConstraintError::Abort(TypeConstraintAbort::ArithmeticOverflow))
    };
    path.choice_key.push(ChoiceDerivationStep {
        equation: path
            .equations
            .last()
            .ok_or(TypeConstraintError::Invariant(
                TypeConstraintInvariant::SourceProtocol(
                    TypeConstraintSourceProtocolInvariant::Outcome,
                ),
            ))?
            .ordinal,
        direction: relation.direction(),
        role,
        expected: to_ordinal(expected)?,
        actual: to_ordinal(actual)?,
    });
    Ok(path)
}

/// Relates one equation and performs directional SelectedCall relation on
/// every alternative before the branch can re-enter the candidate frontier.
/// This keeps Choice alternatives branch-local rather than applying a single
/// whole-Choice relation after raw paths have been retained.
fn relate_constraint<A, D, L>(
    pattern: &ConstraintOperandView<'_, D>,
    actual: &ConstraintOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    relation: ConstraintRelation,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    if matches!(relation, ConstraintRelation::Equal) {
        // Original products close after every original binding/effect relation
        // has contributed. A temporary nonprincipal effect relation may still
        // be resolved by another original root, so do not seal it here.
        return relate_with_policy(pattern, actual, path, context, relation);
    }
    let pattern_shape = pattern.value().constraint_shape();
    let actual_shape = actual.value().constraint_shape();
    if matches!(pattern_shape, TypeConstraintShape::Choice(_))
        || matches!(actual_shape, TypeConstraintShape::Choice(_))
    {
        // Choice itself is a semantic node, not merely a branch container;
        // charge its visit before opening the typed branch forks.
        context.enter_node()?;
    }
    let paths = match (pattern_shape, actual_shape) {
        (TypeConstraintShape::Choice(_), TypeConstraintShape::Choice(_)) => {
            relate_choice_to_choice_selected(
                &pattern.children(context)?.collect::<Result<Vec<_>, _>>()?,
                &actual.children(context)?.collect::<Result<Vec<_>, _>>()?,
                path,
                context,
                relation,
            )?
        }
        (TypeConstraintShape::Choice(_), _) => relate_choice_to_actual_selected(
            &pattern.children(context)?.collect::<Result<Vec<_>, _>>()?,
            actual,
            path,
            context,
            relation,
        )?,
        (_, TypeConstraintShape::Choice(_)) => relate_actual_choice_selected(
            pattern,
            &actual.children(context)?.collect::<Result<Vec<_>, _>>()?,
            path,
            context,
            relation,
        )?,
        _ => relate_with_policy(pattern, actual, path, context, relation)?,
    };
    if matches!(pattern_shape, TypeConstraintShape::Choice(_))
        || matches!(actual_shape, TypeConstraintShape::Choice(_))
    {
        return Ok(paths);
    }
    let mut accepted = Vec::new();
    for path in paths {
        if let Some(path) = accept_branch(pattern, actual, path, context, relation)? {
            accepted.push(path);
        }
    }
    Ok(accepted)
}

fn accept_branch<A, D, L>(
    pattern: &ConstraintOperandView<'_, D>,
    actual: &ConstraintOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    relation: ConstraintRelation,
) -> Result<Option<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    if !path.deferred_cycles.parameters.is_empty() {
        // Provisional back-edges are evidence only.  Do not let this local
        // relation pass turn a later source failure into an early mismatch;
        // the close phase owns cycle pruning after all source work.
        return Ok(Some(path));
    }
    let path = seal_path(path, context)?;
    let effects = ConstraintClosedEffects::for_branch(&path, context)?;
    let projected_pattern = project_closed_operand(
        pattern.clone(),
        &path,
        ConstraintClosurePolicy::Hint,
        &effects,
        context,
    )?;
    let projected_actual = project_closed_operand(
        actual.clone(),
        &path,
        ConstraintClosurePolicy::Hint,
        &effects,
        context,
    )?;
    let projected_pattern = projected_pattern.value();
    let projected_actual = projected_actual.value();
    let compatible = match relation {
        ConstraintRelation::Equal => crate::types::constraints::completed_types_equal(
            projected_pattern,
            projected_actual,
            context,
        ),
        ConstraintRelation::Accepts(ConstraintAcceptance::PatternAcceptsActual) => {
            projected_pattern
                .accepts_with(
                    projected_actual,
                    TypeCompatibilityPolicy::SelectedCall,
                    context,
                )
                .map_err(map_compatibility_error)
        }
        ConstraintRelation::Accepts(ConstraintAcceptance::ActualAcceptsPattern) => projected_actual
            .accepts_with(
                projected_pattern,
                TypeCompatibilityPolicy::SelectedCall,
                context,
            )
            .map_err(map_compatibility_error),
    };
    Ok(compatible?.then_some(path))
}

pub(crate) fn map_compatibility_error(
    error: TypeCompatibilityFailure<TypeConstraintError>,
) -> TypeConstraintError {
    match error {
        TypeCompatibilityFailure::Forbidden { .. } => {
            TypeConstraintError::Rejected(TypeConstraintRejection::UnresolvedType)
        }
        TypeCompatibilityFailure::Control(error) => error,
    }
}

/// Relate each expected Choice alternative against one actual value.
///
/// Each alternative receives an independent fork and goes through the same
/// selected-call entry point as a non-Choice equation.  That entry point is
/// the branch-local seal/relation boundary; keeping it here (rather than
/// accepting the projected Choice after the raw relation) prevents a
/// rejected alternative from re-entering the frontier.
fn relate_choice_to_actual_selected<A, D, L>(
    expected: &[ConstraintOperandView<'_, D>],
    actual: &ConstraintOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    relation: ConstraintRelation,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    match relation.direction() {
        ConstraintAcceptance::PatternAcceptsActual => {
            expected
                .iter()
                .enumerate()
                .try_fold(Vec::new(), |mut output, (index, expected)| {
                    let path = fork_choice_path(
                        &path,
                        ChoiceForkRole::ExpectedAlternative,
                        Some(index),
                        None,
                        relation,
                        context,
                    )?;
                    output.extend(relate_constraint(
                        expected, actual, path, context, relation,
                    )?);
                    Ok(output)
                })
        }
        ConstraintAcceptance::ActualAcceptsPattern => {
            expected
                .iter()
                .enumerate()
                .try_fold(vec![path], |paths, (index, expected)| {
                    paths.into_iter().try_fold(Vec::new(), |mut output, path| {
                        let path = fork_choice_path(
                            &path,
                            ChoiceForkRole::ExpectedAlternative,
                            Some(index),
                            None,
                            relation,
                            context,
                        )?;
                        output.extend(relate_constraint(
                            expected, actual, path, context, relation,
                        )?);
                        Ok(output)
                    })
                })
        }
    }
}

/// Relate one expected value against each actual Choice alternative.
fn relate_actual_choice_selected<A, D, L>(
    expected: &ConstraintOperandView<'_, D>,
    actual: &[ConstraintOperandView<'_, D>],
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    relation: ConstraintRelation,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    match relation.direction() {
        ConstraintAcceptance::PatternAcceptsActual => {
            actual
                .iter()
                .enumerate()
                .try_fold(vec![path], |paths, (index, actual)| {
                    paths.into_iter().try_fold(Vec::new(), |mut output, path| {
                        let path = fork_choice_path(
                            &path,
                            ChoiceForkRole::ActualAlternative,
                            None,
                            Some(index),
                            relation,
                            context,
                        )?;
                        output.extend(relate_constraint(
                            expected, actual, path, context, relation,
                        )?);
                        Ok(output)
                    })
                })
        }
        ConstraintAcceptance::ActualAcceptsPattern => {
            actual
                .iter()
                .enumerate()
                .try_fold(Vec::new(), |mut output, (index, actual)| {
                    let path = fork_choice_path(
                        &path,
                        ChoiceForkRole::ActualAlternative,
                        None,
                        Some(index),
                        relation,
                        context,
                    )?;
                    output.extend(relate_constraint(
                        expected, actual, path, context, relation,
                    )?);
                    Ok(output)
                })
        }
    }
}

/// Relate every expected/actual Choice pair in its own branch.
fn relate_choice_to_choice_selected<A, D, L>(
    expected: &[ConstraintOperandView<'_, D>],
    actual: &[ConstraintOperandView<'_, D>],
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    relation: ConstraintRelation,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    match relation.direction() {
        ConstraintAcceptance::PatternAcceptsActual => {
            actual
                .iter()
                .enumerate()
                .try_fold(vec![path], |paths, (actual_index, actual)| {
                    paths.into_iter().try_fold(Vec::new(), |mut output, path| {
                        for (expected_index, expected) in expected.iter().enumerate() {
                            let path = fork_choice_path(
                                &path,
                                ChoiceForkRole::ExpectedActualPair,
                                Some(expected_index),
                                Some(actual_index),
                                relation,
                                context,
                            )?;
                            output.extend(relate_constraint(
                                expected, actual, path, context, relation,
                            )?);
                        }
                        Ok(output)
                    })
                })
        }
        ConstraintAcceptance::ActualAcceptsPattern => {
            expected
                .iter()
                .enumerate()
                .try_fold(vec![path], |paths, (expected_index, expected)| {
                    paths.into_iter().try_fold(Vec::new(), |mut output, path| {
                        for (actual_index, actual) in actual.iter().enumerate() {
                            let path = fork_choice_path(
                                &path,
                                ChoiceForkRole::ExpectedActualPair,
                                Some(expected_index),
                                Some(actual_index),
                                relation,
                                context,
                            )?;
                            output.extend(relate_constraint(
                                expected, actual, path, context, relation,
                            )?);
                        }
                        Ok(output)
                    })
                })
        }
    }
}

fn relate_with_policy<A, D, L>(
    pattern: &ConstraintOperandView<'_, D>,
    actual: &ConstraintOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    relation: ConstraintRelation,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    context.enter_node()?;
    relate_entered_with_policy(
        pattern,
        actual,
        pattern.value().constraint_shape(),
        actual.value().constraint_shape(),
        path,
        context,
        relation,
    )
}

fn type_parameter_eligibility<A, D, L>(
    parameter: &GenericTypeReference,
    path: &ConstraintInferenceBranch<D, L>,
    context: &TypeConstraintContext<'_, A, D>,
) -> Result<TypeConstraintParameterEligibility, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    if matches!(parameter, GenericTypeReference::Bound(_)) {
        return Ok(TypeConstraintParameterEligibility::Rigid);
    }
    context
        .parameter_eligibility(parameter, path.projection_view())
        .ok_or_else(|| {
            TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
                crate::types::constraints::TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope {
                    parameter: parameter.clone(),
                },
            ))
        })
}

fn bind_type_parameter<A, D, L>(
    parameter: &GenericTypeReference,
    value: &ConstraintOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    if matches!(
        type_parameter_eligibility(parameter, &path, context)?,
        TypeConstraintParameterEligibility::Rigid
    ) {
        return Ok(Vec::new());
    }
    context
        .add_binding(path, parameter.clone(), value.clone())
        .map(|path| path.into_iter().collect())
}

fn relate_entered_with_policy<A, D, L>(
    pattern: &ConstraintOperandView<'_, D>,
    actual: &ConstraintOperandView<'_, D>,
    pattern_shape: TypeConstraintShape<'_>,
    actual_shape: TypeConstraintShape<'_>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    relation: ConstraintRelation,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    if matches!(pattern_shape, TypeConstraintShape::Unresolved(_))
        || matches!(actual_shape, TypeConstraintShape::Unresolved(_))
    {
        return Err(TypeConstraintError::Rejected(
            TypeConstraintRejection::UnresolvedType,
        ));
    }
    context.validate_type_header(pattern_shape, path.projection_view())?;
    context.validate_type_header(actual_shape, path.projection_view())?;

    let pattern_alias = resolve_type_alias(pattern.clone(), &path, context)?;
    let actual_alias = resolve_type_alias(actual.clone(), &path, context)?;
    let mut cycle = match (&pattern_alias, &actual_alias) {
        (ConstraintAliasResolution::Cycle(left), ConstraintAliasResolution::Cycle(right)) => {
            left.iter().chain(right.iter()).cloned().collect::<Vec<_>>()
        }
        (ConstraintAliasResolution::Cycle(parameters), _)
        | (_, ConstraintAliasResolution::Cycle(parameters)) => parameters.to_vec(),
        _ => Vec::new(),
    };
    if !cycle.is_empty() {
        cycle.sort();
        cycle.dedup();
        let mut path = path;
        for parameter in cycle {
            path.deferred_cycles.parameters.insert(parameter.into());
        }
        return Ok(vec![path]);
    }
    let pattern_value = match pattern_alias {
        ConstraintAliasResolution::Unchanged => None,
        ConstraintAliasResolution::Value(value) => Some(value),
        ConstraintAliasResolution::Cycle(_) => unreachable!("cycles handled above"),
    };
    let actual_value = match actual_alias {
        ConstraintAliasResolution::Unchanged => None,
        ConstraintAliasResolution::Value(value) => Some(value),
        ConstraintAliasResolution::Cycle(_) => unreachable!("cycles handled above"),
    };
    if pattern_value.is_some() || actual_value.is_some() {
        let pattern_view = pattern_value.as_ref().map(ConstraintOperand::<D>::view);
        let actual_view = actual_value.as_ref().map(ConstraintOperand::<D>::view);
        return relate_with_policy(
            pattern_view.as_ref().unwrap_or(pattern),
            actual_view.as_ref().unwrap_or(actual),
            path,
            context,
            relation,
        );
    }
    // Bottom contributes no binding, but unresolved descendants in the
    // expected shape still cannot reach a selected seal.
    if matches!(relation, ConstraintRelation::Accepts(_))
        && matches!(actual_shape, TypeConstraintShape::Never)
    {
        validate_type(pattern.value(), path.projection_view(), context)?;
        return Ok(vec![path]);
    }

    match (pattern_shape, actual_shape) {
        (TypeConstraintShape::Choice(_), TypeConstraintShape::Choice(_))
            if matches!(relation, ConstraintRelation::Equal) =>
        {
            return relate_sliced_children(pattern, actual, path, context, relation);
        }
        (
            TypeConstraintShape::Generic(pattern_parameter),
            TypeConstraintShape::Generic(actual_parameter),
        ) => {
            if pattern_parameter == actual_parameter {
                return Ok(vec![path]);
            }
            let pattern_active = type_parameter_eligibility(pattern_parameter, &path, context)?
                != TypeConstraintParameterEligibility::Rigid;
            let actual_active = type_parameter_eligibility(actual_parameter, &path, context)?
                != TypeConstraintParameterEligibility::Rigid;
            let (parameter, value) = match relation.direction() {
                ConstraintAcceptance::PatternAcceptsActual if pattern_active => {
                    (pattern_parameter, actual)
                }
                ConstraintAcceptance::ActualAcceptsPattern if actual_active => {
                    (actual_parameter, pattern)
                }
                _ if pattern_active => (pattern_parameter, actual),
                _ if actual_active => (actual_parameter, pattern),
                _ => return Ok(Vec::new()),
            };
            return bind_type_parameter(parameter, value, path, context);
        }
        (TypeConstraintShape::Generic(parameter), _)
            if type_parameter_eligibility(parameter, &path, context)?
                != TypeConstraintParameterEligibility::Rigid =>
        {
            return bind_type_parameter(parameter, actual, path, context);
        }
        (_, TypeConstraintShape::Generic(parameter)) => {
            if type_parameter_eligibility(parameter, &path, context)?
                != TypeConstraintParameterEligibility::Rigid
            {
                return bind_type_parameter(parameter, pattern, path, context);
            }
        }
        _ => {}
    }

    match (pattern_shape, actual_shape) {
        (TypeConstraintShape::Choice(_), _) | (_, TypeConstraintShape::Choice(_))
            if matches!(relation, ConstraintRelation::Equal) =>
        {
            Ok(Vec::new())
        }
        (TypeConstraintShape::Choice(_), TypeConstraintShape::Choice(_)) => {
            relate_choice_to_choice_selected(
                &pattern.children(context)?.collect::<Result<Vec<_>, _>>()?,
                &actual.children(context)?.collect::<Result<Vec<_>, _>>()?,
                path,
                context,
                relation,
            )
        }
        (TypeConstraintShape::Choice(_), _) => relate_choice_to_actual_selected(
            &pattern.children(context)?.collect::<Result<Vec<_>, _>>()?,
            actual,
            path,
            context,
            relation,
        ),
        (_, TypeConstraintShape::Choice(_)) => relate_actual_choice_selected(
            pattern,
            &actual.children(context)?.collect::<Result<Vec<_>, _>>()?,
            path,
            context,
            relation,
        ),
        (
            TypeConstraintShape::Nominal {
                nominal: expected,
                arguments: expected_arguments,
            },
            TypeConstraintShape::Nominal {
                nominal: found,
                arguments: found_arguments,
            },
        ) if expected.same_owner(found) && expected_arguments.len() == found_arguments.len() => {
            let expected_arguments = pattern.children(context)?.collect::<Result<Vec<_>, _>>()?;
            let found_arguments = actual.children(context)?.collect::<Result<Vec<_>, _>>()?;
            let paths = relate_slices(
                &expected_arguments,
                &found_arguments,
                path,
                context,
                if matches!(relation, ConstraintRelation::Equal) {
                    relation
                } else {
                    ConstraintRelation::Accepts(ConstraintAcceptance::PatternAcceptsActual)
                },
            )?;
            if matches!(relation, ConstraintRelation::Equal) {
                return Ok(paths);
            }
            expected_arguments.iter().zip(&found_arguments).try_fold(
                paths,
                |paths, (expected, found)| {
                    relate_many(
                        expected,
                        found,
                        paths,
                        context,
                        ConstraintRelation::Accepts(ConstraintAcceptance::ActualAcceptsPattern),
                    )
                },
            )
        }
        (TypeConstraintShape::Ref(expected), TypeConstraintShape::Ref(found))
            if expected.kind() == found.kind() =>
        {
            match (expected.value(), found.value()) {
                (Some(_), Some(_)) => relate_children(pattern, actual, path, context, relation),
                (Some(_), None) | (None, None) => Ok(vec![path]),
                (None, Some(found)) => {
                    validate_type(found, path.projection_view(), context)?;
                    Ok(vec![path])
                }
            }
        }
        (
            TypeConstraintShape::Unary {
                kind: expected_kind,
                ..
            },
            TypeConstraintShape::Unary {
                kind: found_kind, ..
            },
        ) if expected_kind == found_kind => {
            relate_children(pattern, actual, path, context, relation)
        }
        (
            TypeConstraintShape::Iterator {
                family: expected_family,
                ..
            },
            TypeConstraintShape::Iterator {
                family: found_family,
                ..
            },
        ) if expected_family == found_family => {
            relate_children(pattern, actual, path, context, relation)
        }
        (TypeConstraintShape::Array { .. }, TypeConstraintShape::Array { .. }) => {
            let expected = pattern
                .array_length()?
                .expect("array shape has a length port");
            let found = actual
                .array_length()?
                .expect("array shape has a length port");
            let Some(path) = relate_array_lengths(&expected, &found, path, context, relation)?
            else {
                return Ok(Vec::new());
            };
            relate_children(pattern, actual, path, context, relation)
        }
        (
            TypeConstraintShape::Map {
                kind: expected_kind,
                ..
            },
            TypeConstraintShape::Map {
                kind: found_kind, ..
            },
        ) if expected_kind == found_kind => {
            relate_children(pattern, actual, path, context, relation)
        }
        (
            TypeConstraintShape::Borrow {
                kind: expected_kind,
                lifetime: expected_lifetime,
                ..
            },
            TypeConstraintShape::Borrow {
                kind: found_kind,
                lifetime: found_lifetime,
                ..
            },
        ) if expected_kind == found_kind && expected_lifetime == found_lifetime => {
            relate_children(pattern, actual, path, context, relation)
        }
        (
            TypeConstraintShape::Pair {
                kind: expected_kind,
                ..
            },
            TypeConstraintShape::Pair {
                kind: found_kind, ..
            },
        ) if expected_kind == found_kind => {
            relate_children(pattern, actual, path, context, relation)
        }
        (
            TypeConstraintShape::Function {
                binder: expected_binder,
                params: expected_params,
                ..
            },
            TypeConstraintShape::Function {
                binder: found_binder,
                params: found_params,
                ..
            },
        ) if expected_binder == found_binder && expected_params.len() == found_params.len() => {
            context.with_binder(*expected_binder, |context| {
                let mut expected_children = pattern.children(context)?;
                let mut found_children = actual.children(context)?;
                let expected_header = expected_children
                    .function_header()?
                    .expect("function header");
                let found_header = found_children.function_header()?.expect("function header");
                if !expected_header
                    .predicate()
                    .equal_with(found_header.predicate(), context)?
                {
                    return Ok(Vec::new());
                }
                let expected_params = expected_children
                    .by_ref()
                    .take(expected_params.len())
                    .collect::<Result<Vec<_>, _>>()?;
                let found_params = found_children
                    .by_ref()
                    .take(found_params.len())
                    .collect::<Result<Vec<_>, _>>()?;
                let expected_result = expected_children
                    .next()
                    .expect("function children end in a result")?;
                let found_result = found_children
                    .next()
                    .expect("function children end in a result")?;
                let paths = relate_slices(
                    &expected_params,
                    &found_params,
                    path,
                    context,
                    relation.reverse(),
                )?;
                let mut effect_paths = Vec::with_capacity(paths.len());
                for mut path in paths {
                    let (actual_effects, permitted_effects) = match relation.direction() {
                        ConstraintAcceptance::PatternAcceptsActual => {
                            (found_header.row(), expected_header.row())
                        }
                        ConstraintAcceptance::ActualAcceptsPattern => {
                            (expected_header.row(), found_header.row())
                        }
                    };
                    match path
                        .effects
                        .constrain_subset(actual_effects, permitted_effects, context)
                    {
                        Ok(()) => {
                            if matches!(relation, ConstraintRelation::Equal) {
                                match path.effects.constrain_subset(
                                    permitted_effects,
                                    actual_effects,
                                    context,
                                ) {
                                    Ok(()) => {}
                                    Err(TypeConstraintError::Rejected(_)) => continue,
                                    Err(error) => return Err(error),
                                }
                            }
                            effect_paths.push(path)
                        }
                        Err(TypeConstraintError::Rejected(_)) => {}
                        Err(error) => return Err(error),
                    }
                }
                relate_many(
                    &expected_result,
                    &found_result,
                    effect_paths,
                    context,
                    relation,
                )
            })
        }
        (TypeConstraintShape::Tuple(expected), TypeConstraintShape::Tuple(found))
            if expected.len() == found.len() =>
        {
            relate_sliced_children(pattern, actual, path, context, relation)
        }
        (TypeConstraintShape::Payload(expected), TypeConstraintShape::Payload(found))
            if expected.has_same_header(found) =>
        {
            relate_sliced_children(pattern, actual, path, context, relation)
        }
        (TypeConstraintShape::Leaf(_), _)
        | (TypeConstraintShape::Never, TypeConstraintShape::Never) => Ok(vec![path]),
        _ => Ok(vec![path]),
    }
}
fn const_parameter_eligibility<A, D, L>(
    parameter: &GenericConstReference,
    path: &ConstraintInferenceBranch<D, L>,
    context: &TypeConstraintContext<'_, A, D>,
) -> Result<TypeConstraintConstEligibility, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    if matches!(parameter, GenericConstReference::Bound(_)) {
        return Ok(TypeConstraintConstEligibility::Rigid);
    }
    context
        .const_parameter_eligibility(parameter, path.projection_view())
        .ok_or_else(|| {
            TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
                crate::types::constraints::TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope {
                    parameter: parameter.clone(),
                },
            ))
        })
}

fn bind_const_parameter<A, D, L>(
    parameter: &GenericConstReference,
    value: &ConstraintConstOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<Option<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    if matches!(
        const_parameter_eligibility(parameter, &path, context)?,
        TypeConstraintConstEligibility::Rigid
    ) {
        return Ok(None);
    }
    context.add_const_binding(path, parameter.clone(), value.clone())
}

fn relate_array_lengths<A, D, L>(
    expected: &ConstraintConstOperandView<'_, D>,
    found: &ConstraintConstOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    relation: ConstraintRelation,
) -> Result<Option<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    context.enter_node()?;
    let expected_alias = resolve_const_alias(expected.clone(), &path, context)?;
    let found_alias = resolve_const_alias(found.clone(), &path, context)?;
    let mut cycle = match (&expected_alias, &found_alias) {
        (ConstraintAliasResolution::Cycle(left), ConstraintAliasResolution::Cycle(right)) => {
            left.iter().chain(right.iter()).cloned().collect::<Vec<_>>()
        }
        (ConstraintAliasResolution::Cycle(parameters), _)
        | (_, ConstraintAliasResolution::Cycle(parameters)) => parameters.to_vec(),
        _ => Vec::new(),
    };
    if !cycle.is_empty() {
        cycle.sort();
        cycle.dedup();
        let mut path = path;
        for parameter in cycle {
            path.deferred_cycles.parameters.insert(parameter.into());
        }
        return Ok(Some(path));
    }
    let expected_value = match expected_alias {
        ConstraintAliasResolution::Unchanged => None,
        ConstraintAliasResolution::Value(value) => Some(value),
        ConstraintAliasResolution::Cycle(_) => unreachable!("cycles handled above"),
    };
    let found_value = match found_alias {
        ConstraintAliasResolution::Unchanged => None,
        ConstraintAliasResolution::Value(value) => Some(value),
        ConstraintAliasResolution::Cycle(_) => unreachable!("cycles handled above"),
    };
    let expected_view = expected_value
        .as_ref()
        .map(ConstraintOperand::<D, ArrayLength>::view);
    let found_view = found_value
        .as_ref()
        .map(ConstraintOperand::<D, ArrayLength>::view);
    let expected = expected_view.as_ref().unwrap_or(expected);
    let found = found_view.as_ref().unwrap_or(found);
    match (expected.value(), found.value()) {
        (ArrayLength::Error(_) | ArrayLength::Inferred, _)
        | (_, ArrayLength::Error(_) | ArrayLength::Inferred) => {
            Err(TypeConstraintRejection::UnresolvedType.into())
        }
        (ArrayLength::Const(expected), ArrayLength::Const(found)) => {
            Ok((expected == found).then_some(path))
        }
        (ArrayLength::Generic(expected_parameter), ArrayLength::Generic(found_parameter)) => {
            if expected_parameter == found_parameter {
                return Ok(Some(path));
            }
            let pattern_active = const_parameter_eligibility(expected_parameter, &path, context)?
                != TypeConstraintConstEligibility::Rigid;
            let actual_active = const_parameter_eligibility(found_parameter, &path, context)?
                != TypeConstraintConstEligibility::Rigid;
            let (parameter, value) = match relation.direction() {
                ConstraintAcceptance::PatternAcceptsActual if pattern_active => {
                    (expected_parameter, found)
                }
                ConstraintAcceptance::ActualAcceptsPattern if actual_active => {
                    (found_parameter, expected)
                }
                _ if pattern_active => (expected_parameter, found),
                _ if actual_active => (found_parameter, expected),
                _ => return Ok(None),
            };
            context.add_const_binding(path, parameter.clone(), value.clone())
        }
        (ArrayLength::Generic(parameter), _) => {
            bind_const_parameter(parameter, found, path, context)
        }
        (_, ArrayLength::Generic(parameter)) => {
            bind_const_parameter(parameter, expected, path, context)
        }
    }
}
fn relate_slices<A, D, L>(
    expected: &[ConstraintOperandView<'_, D>],
    found: &[ConstraintOperandView<'_, D>],
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    relation: ConstraintRelation,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    if expected.len() != found.len() {
        return Ok(Vec::new());
    }
    expected
        .iter()
        .zip(found)
        .try_fold(vec![path], |paths, (expected, found)| {
            relate_many(expected, found, paths, context, relation)
        })
}

fn relate_children<A, D, L>(
    expected: &ConstraintOperandView<'_, D>,
    found: &ConstraintOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    relation: ConstraintRelation,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    let mut expected = expected.children(context)?;
    let mut found = found.children(context)?;
    let (Some(first_expected), Some(first_found)) =
        (expected.next().transpose()?, found.next().transpose()?)
    else {
        return Ok(Vec::new());
    };
    let paths = relate_with_policy(&first_expected, &first_found, path, context, relation)?;
    expected
        .zip(found)
        .try_fold(paths, |paths, (expected, found)| {
            relate_many(&expected?, &found?, paths, context, relation)
        })
}
fn relate_sliced_children<A, D, L>(
    expected: &ConstraintOperandView<'_, D>,
    found: &ConstraintOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    relation: ConstraintRelation,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    let expected = expected.children(context)?.collect::<Result<Vec<_>, _>>()?;
    let found = found.children(context)?.collect::<Result<Vec<_>, _>>()?;
    relate_slices(&expected, &found, path, context, relation)
}
fn relate_many<A, D, L>(
    expected: &ConstraintOperandView<'_, D>,
    found: &ConstraintOperandView<'_, D>,
    paths: Vec<ConstraintInferenceBranch<D, L>>,
    context: &mut TypeConstraintContext<'_, A, D>,
    relation: ConstraintRelation,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L: Clone,
{
    paths.into_iter().try_fold(Vec::new(), |mut output, path| {
        let path = context.fork_path(&path)?;
        output.extend(relate_with_policy(
            expected, found, path, context, relation,
        )?);
        Ok(output)
    })
}
