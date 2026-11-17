//! Relation and Choice algebra for candidate-wide constraints.

use super::super::{
    ArrayLength, GenericConstReference, GenericTypeReference, TypeCompatibilityFailure, TypeKind,
};
use crate::types::constraints::context::{TypeConstraintAccounting, TypeConstraintContext};
use crate::types::constraints::transaction::{
    ConstraintConstOperandView, ConstraintInferenceBranch, ConstraintOperand, ConstraintOperandView,
};
use crate::types::constraints::{
    ChoiceDerivationStep, ChoiceForkRole, ConstraintAliasResolution, ConstraintDomain,
    TypeConstraintAbort, TypeConstraintConstEligibility, TypeConstraintError,
    TypeConstraintInvariant, TypeConstraintParameterEligibility, TypeConstraintRejection,
    TypeConstraintShape, TypeConstraintSourceProtocolInvariant, resolve_const_alias,
    resolve_type_alias, validate_type,
};

mod value_membership;
use value_membership::{relate_agent_value, relate_cross_shape};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ConstraintAcceptance {
    PatternAcceptsActual,
    ActualAcceptsPattern,
}

#[derive(Clone, Copy)]
enum ConstraintRelation {
    Accepts(ConstraintAcceptance),
    Structural(ConstraintAcceptance),
    Equal,
}

impl ConstraintRelation {
    fn direction(self) -> ConstraintAcceptance {
        match self {
            Self::Accepts(direction) | Self::Structural(direction) => direction,
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
            Self::Structural(ConstraintAcceptance::PatternAcceptsActual) => {
                Self::Structural(ConstraintAcceptance::ActualAcceptsPattern)
            }
            Self::Structural(ConstraintAcceptance::ActualAcceptsPattern) => {
                Self::Structural(ConstraintAcceptance::PatternAcceptsActual)
            }
            Self::Equal => Self::Equal,
        }
    }

    fn structural(self) -> Self {
        match self {
            Self::Equal => Self::Equal,
            _ => Self::Structural(self.direction()),
        }
    }

    fn exact_header(self) -> bool {
        matches!(self, Self::Equal | Self::Structural(_))
    }
}

pub(in crate::types) fn relate_selected_call<A, D, L: Clone>(
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
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
        graph,
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
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    pattern: &ConstraintOperandView<'_, D>,
    actual: &ConstraintOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    relate_constraint(
        graph,
        pattern,
        actual,
        path,
        context,
        ConstraintRelation::Equal,
    )
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

/// Prepares one equation's structure and conditional effects on each branch.
/// Final semantic confirmation belongs to the original forest after inputs.
/// This keeps Choice alternatives branch-local rather than applying a single
/// whole-Choice relation after raw paths have been retained.
fn relate_constraint<A, D, L>(
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
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
    // Strict relation preparation validates both complete input trees before a
    // header mismatch can hide a bad descendant. Read each actual operand's
    // frames without solving its effects or following active type bindings.
    for operand in [pattern, actual] {
        path.projection_view()
            .admit_operand(operand.clone(), graph.read(), context)?;
    }
    if relation.exact_header() {
        // Original products close after every original binding/effect relation
        // has contributed. A temporary nonprincipal effect relation may still
        // be resolved by another original root, so do not seal it here.
        return relate_with_policy(graph, pattern, actual, path, context, relation);
    }
    let (expected, supplied) = match relation.direction() {
        ConstraintAcceptance::PatternAcceptsActual => (pattern, actual),
        ConstraintAcceptance::ActualAcceptsPattern => (actual, pattern),
    };
    if matches!(expected.value(), TypeKind::AgentValue)
        && !matches!(supplied.value(), TypeKind::Never)
    {
        return relate_agent_value(graph, supplied, path, context, relation.direction());
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
                graph,
                &pattern.children(context)?.collect::<Result<Vec<_>, _>>()?,
                &actual.children(context)?.collect::<Result<Vec<_>, _>>()?,
                path,
                context,
                relation,
            )?
        }
        (TypeConstraintShape::Choice(_), _) => relate_choice_to_actual_selected(
            graph,
            &pattern.children(context)?.collect::<Result<Vec<_>, _>>()?,
            actual,
            path,
            context,
            relation,
        )?,
        (_, TypeConstraintShape::Choice(_)) => relate_actual_choice_selected(
            graph,
            pattern,
            &actual.children(context)?.collect::<Result<Vec<_>, _>>()?,
            path,
            context,
            relation,
        )?,
        _ => relate_with_policy(graph, pattern, actual, path, context, relation)?,
    };
    Ok(paths)
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
/// selected-call entry point as a non-Choice equation. That entry point owns
/// branch-local structure and conditions; keeping it here (rather than
/// accepting the projected Choice after the raw relation) prevents a
/// rejected alternative from re-entering the frontier.
fn relate_choice_to_actual_selected<A, D, L>(
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
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
                        graph, expected, actual, path, context, relation,
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
                            graph, expected, actual, path, context, relation,
                        )?);
                        Ok(output)
                    })
                })
        }
    }
}

/// Relate one expected value against each actual Choice alternative.
fn relate_actual_choice_selected<A, D, L>(
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
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
                            graph, expected, actual, path, context, relation,
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
                        graph, expected, actual, path, context, relation,
                    )?);
                    Ok(output)
                })
        }
    }
}

/// Relate every expected/actual Choice pair in its own branch.
fn relate_choice_to_choice_selected<A, D, L>(
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
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
                                graph, expected, actual, path, context, relation,
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
                                graph, expected, actual, path, context, relation,
                            )?);
                        }
                        Ok(output)
                    })
                })
        }
    }
}

fn relate_with_policy<A, D, L>(
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
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
        graph,
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
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    pattern: &ConstraintOperandView<'_, D>,
    actual: &ConstraintOperandView<'_, D>,
    pattern_shape: TypeConstraintShape<'_, crate::types::PreparedEffects>,
    actual_shape: TypeConstraintShape<'_, crate::types::PreparedEffects>,
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
        return relate_constraint(
            graph,
            pattern_view.as_ref().unwrap_or(pattern),
            actual_view.as_ref().unwrap_or(actual),
            path,
            context,
            relation,
        );
    }
    // Bottom contributes no binding, but unresolved descendants in the
    // expected shape still cannot reach a selected seal.
    let (expected, supplied) = match relation.direction() {
        ConstraintAcceptance::PatternAcceptsActual => (pattern, actual),
        ConstraintAcceptance::ActualAcceptsPattern => (actual, pattern),
    };
    if !relation.exact_header()
        && matches!(
            supplied.value().constraint_shape(),
            TypeConstraintShape::Never
        )
    {
        validate_type(
            expected.value(),
            path.projection_view(),
            graph.read(),
            context,
        )?;
        return Ok(vec![path]);
    }
    if !relation.exact_header()
        && matches!(
            expected.value().constraint_shape(),
            TypeConstraintShape::Never
        )
    {
        return Ok(Vec::new());
    }

    match (pattern_shape, actual_shape) {
        (TypeConstraintShape::Choice(_), TypeConstraintShape::Choice(_))
            if relation.exact_header() =>
        {
            return relate_sliced_children(graph, pattern, actual, path, context, relation);
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
            return Ok(Vec::new());
        }
        (TypeConstraintShape::Generic(_), _) => return Ok(Vec::new()),
        _ => {}
    }

    match (pattern_shape, actual_shape) {
        (TypeConstraintShape::Choice(_), _) | (_, TypeConstraintShape::Choice(_))
            if relation.exact_header() =>
        {
            Ok(Vec::new())
        }
        (TypeConstraintShape::Choice(_), TypeConstraintShape::Choice(_)) => {
            relate_choice_to_choice_selected(
                graph,
                &pattern.children(context)?.collect::<Result<Vec<_>, _>>()?,
                &actual.children(context)?.collect::<Result<Vec<_>, _>>()?,
                path,
                context,
                relation,
            )
        }
        (TypeConstraintShape::Choice(_), _) => relate_choice_to_actual_selected(
            graph,
            &pattern.children(context)?.collect::<Result<Vec<_>, _>>()?,
            actual,
            path,
            context,
            relation,
        ),
        (_, TypeConstraintShape::Choice(_)) => relate_actual_choice_selected(
            graph,
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
                graph,
                &expected_arguments,
                &found_arguments,
                path,
                context,
                relation,
            )?;
            if matches!(relation, ConstraintRelation::Equal) {
                return Ok(paths);
            }
            expected_arguments.iter().zip(&found_arguments).try_fold(
                paths,
                |paths, (expected, found)| {
                    relate_many(graph, expected, found, paths, context, relation.reverse())
                },
            )
        }
        (TypeConstraintShape::Ref(expected), TypeConstraintShape::Ref(found)) => {
            let compatible = match relation {
                ConstraintRelation::Equal => expected.accepts_header(found, true),
                ConstraintRelation::Structural(ConstraintAcceptance::PatternAcceptsActual) => {
                    expected.accepts_header(found, true)
                }
                ConstraintRelation::Structural(ConstraintAcceptance::ActualAcceptsPattern) => {
                    found.accepts_header(expected, true)
                }
                ConstraintRelation::Accepts(ConstraintAcceptance::PatternAcceptsActual) => {
                    expected.accepts_header(found, false)
                }
                ConstraintRelation::Accepts(ConstraintAcceptance::ActualAcceptsPattern) => {
                    found.accepts_header(expected, false)
                }
            };
            if !compatible {
                return Ok(Vec::new());
            }
            match (expected.value(), found.value()) {
                (Some(_), Some(_)) => {
                    relate_children(graph, pattern, actual, path, context, relation)
                }
                (None, None) => Ok(vec![path]),
                (Some(value), None) | (None, Some(value)) => {
                    validate_type(value, path.projection_view(), graph.read(), context)?;
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
            relate_children(graph, pattern, actual, path, context, relation)
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
            relate_children(graph, pattern, actual, path, context, relation)
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
            relate_children(graph, pattern, actual, path, context, relation)
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
            relate_children(graph, pattern, actual, path, context, relation)
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
            relate_children(graph, pattern, actual, path, context, relation)
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
            relate_children(graph, pattern, actual, path, context, relation)
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
                expected_header.admit(graph.read(), context)?;
                found_header.admit(graph.read(), context)?;
                let mut path = path;
                path.effects
                    .constrain_predicate_equality(
                        graph,
                        expected_header.predicate(),
                        found_header.predicate(),
                        context,
                    )
                    .map_err(TypeConstraintError::from)?;
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
                    graph,
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
                    let effect_relation = if relation.exact_header() {
                        crate::effect_row::prepared::PreparedEffectRelation::Equal
                    } else {
                        crate::effect_row::prepared::PreparedEffectRelation::AdmittedSubset
                    };
                    match path
                        .effects
                        .constrain_relation(
                            graph,
                            effect_relation,
                            actual_effects,
                            permitted_effects,
                            context,
                        )
                        .map_err(TypeConstraintError::from)
                    {
                        Ok(()) => effect_paths.push(path),
                        Err(TypeConstraintError::Rejected(_)) => {}
                        Err(error) => return Err(error),
                    }
                }
                relate_many(
                    graph,
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
            relate_sliced_children(graph, pattern, actual, path, context, relation)
        }
        (TypeConstraintShape::Payload(expected), TypeConstraintShape::Payload(found))
            if expected.has_same_header(found) =>
        {
            relate_sliced_children(graph, pattern, actual, path, context, relation.structural())
        }
        (TypeConstraintShape::Leaf(pattern), TypeConstraintShape::Leaf(actual)) => {
            let accepted = match relation {
                ConstraintRelation::Equal => pattern == actual,
                ConstraintRelation::Structural(ConstraintAcceptance::PatternAcceptsActual) => {
                    pattern.accepts(actual, true)
                }
                ConstraintRelation::Structural(ConstraintAcceptance::ActualAcceptsPattern) => {
                    actual.accepts(pattern, true)
                }
                ConstraintRelation::Accepts(ConstraintAcceptance::PatternAcceptsActual) => {
                    pattern.accepts(actual, false)
                }
                ConstraintRelation::Accepts(ConstraintAcceptance::ActualAcceptsPattern) => {
                    actual.accepts(pattern, false)
                }
            };
            Ok(if accepted { vec![path] } else { Vec::new() })
        }
        (TypeConstraintShape::Never, TypeConstraintShape::Never) => Ok(vec![path]),
        _ if !relation.exact_header()
            && expected
                .value()
                .cross_shape_compatibility(supplied.value())
                .is_some() =>
        {
            Ok(relate_cross_shape(
                graph,
                expected,
                supplied,
                path,
                context,
                relation.direction(),
            )?)
        }
        _ => Ok(Vec::new()),
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
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
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
            relate_many(graph, expected, found, paths, context, relation)
        })
}

fn relate_children<A, D, L>(
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
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
    let paths = relate_with_policy(
        graph,
        &first_expected,
        &first_found,
        path,
        context,
        relation,
    )?;
    expected
        .zip(found)
        .try_fold(paths, |paths, (expected, found)| {
            relate_many(graph, &expected?, &found?, paths, context, relation)
        })
}
fn relate_sliced_children<A, D, L>(
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
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
    relate_slices(graph, &expected, &found, path, context, relation)
}
fn relate_many<A, D, L>(
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
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
            graph, expected, found, path, context, relation,
        )?);
        Ok(output)
    })
}
