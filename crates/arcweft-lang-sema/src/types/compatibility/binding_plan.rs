//! Relation and Choice algebra for candidate-wide constraints.

use std::collections::BTreeSet;

use super::super::{TypeCompatibilityFailure, TypeCompatibilityPolicy, TypeKind};
use crate::effect_row::EffectConstraintEnvironmentError;
use crate::types::constraints::context::{TypeConstraintAccounting, TypeConstraintContext};
use crate::types::constraints::transaction::ConstraintPath;
use crate::types::constraints::{
    ChoiceDerivationStep, ChoiceForkRole, ConstraintDomain, TypeConstraintAbort,
    TypeConstraintError, TypeConstraintInvariant, TypeConstraintParameterEligibility,
    TypeConstraintRejection, TypeConstraintShape, TypeConstraintSourceProtocolInvariant,
    map_effect_environment_error, seal_path, seal_type, validate_type,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ConstraintAcceptance {
    PatternAcceptsActual,
    ActualAcceptsPattern,
}

fn fork_choice_path<A, D>(
    path: &ConstraintPath<D>,
    role: ChoiceForkRole,
    expected: Option<usize>,
    actual: Option<usize>,
    direction: ConstraintAcceptance,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ConstraintPath<D>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
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
        direction,
        role,
        expected: to_ordinal(expected)?,
        actual: to_ordinal(actual)?,
    });
    Ok(path)
}

/// Relates one equation and performs directional SelectedCall acceptance on
/// every alternative before the branch can re-enter the candidate frontier.
/// This keeps Choice alternatives branch-local rather than applying a single
/// whole-Choice acceptance after raw paths have been retained.
pub(crate) fn relate_selected_call<A, D>(
    pattern: &TypeKind,
    actual: &TypeKind,
    path: ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
    acceptance: ConstraintAcceptance,
) -> Result<Vec<ConstraintPath<D>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let pattern_shape = pattern.constraint_shape();
    let actual_shape = actual.constraint_shape();
    if matches!(pattern_shape, TypeConstraintShape::Choice(_))
        || matches!(actual_shape, TypeConstraintShape::Choice(_))
    {
        // Choice itself is a semantic node, not merely a branch container;
        // charge its visit before opening the typed branch forks.
        context.enter_node()?;
    }
    let paths = match (pattern_shape, actual_shape) {
        (TypeConstraintShape::Choice(expected), TypeConstraintShape::Choice(found)) => {
            relate_choice_to_choice_selected(expected, found, path, context, acceptance)?
        }
        (TypeConstraintShape::Choice(expected), _) => {
            relate_choice_to_actual_selected(expected, actual, path, context, acceptance)?
        }
        (_, TypeConstraintShape::Choice(found)) => {
            relate_actual_choice_selected(pattern, found, path, context, acceptance)?
        }
        _ => relate_with_policy(pattern, actual, path, context, acceptance)?,
    };
    if matches!(pattern_shape, TypeConstraintShape::Choice(_))
        || matches!(actual_shape, TypeConstraintShape::Choice(_))
    {
        return Ok(paths);
    }
    let mut accepted = Vec::new();
    for path in paths {
        if let Some(path) = accept_branch(pattern, actual, path, context, acceptance)? {
            accepted.push(path);
        }
    }
    Ok(accepted)
}

fn accept_branch<A, D>(
    pattern: &TypeKind,
    actual: &TypeKind,
    path: ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
    acceptance: ConstraintAcceptance,
) -> Result<Option<ConstraintPath<D>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    if !path.deferred_cycles.parameters.is_empty() {
        // Provisional back-edges are evidence only.  Do not let this local
        // acceptance pass turn a later source failure into an early mismatch;
        // the close phase owns cycle pruning after all source work.
        return Ok(Some(path));
    }
    let path = seal_path(path, context)?;
    let effects = path
        .effects
        .substitution()
        .map_err(map_effect_environment_error)?;
    let projected_pattern = seal_type(pattern, &path.bindings, &mut BTreeSet::new(), context)?
        .substitute_effect_rows(&effects)
        .map_err(|_| {
            crate::types::constraints::effect_invariant(
                crate::types::constraints::TypeConstraintEffectInvariantKind::NonCanonicalInherited,
                None,
            )
        })?;
    let projected_actual = seal_type(actual, &path.bindings, &mut BTreeSet::new(), context)?
        .substitute_effect_rows(&effects)
        .map_err(|_| {
            crate::types::constraints::effect_invariant(
                crate::types::constraints::TypeConstraintEffectInvariantKind::NonCanonicalInherited,
                None,
            )
        })?;
    let compatible = match acceptance {
        ConstraintAcceptance::PatternAcceptsActual => projected_pattern
            .accepts_with(
                &projected_actual,
                TypeCompatibilityPolicy::SelectedCall,
                context,
            )
            .map_err(map_compatibility_error),
        ConstraintAcceptance::ActualAcceptsPattern => projected_actual
            .accepts_with(
                &projected_pattern,
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
/// the branch-local seal/acceptance boundary; keeping it here (rather than
/// accepting the projected Choice after the raw relation) prevents a
/// rejected alternative from re-entering the frontier.
fn relate_choice_to_actual_selected<A, D>(
    expected: &[TypeKind],
    actual: &TypeKind,
    path: ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
    acceptance: ConstraintAcceptance,
) -> Result<Vec<ConstraintPath<D>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    match acceptance {
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
                        acceptance,
                        context,
                    )?;
                    output.extend(relate_selected_call(
                        expected, actual, path, context, acceptance,
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
                            acceptance,
                            context,
                        )?;
                        output.extend(relate_selected_call(
                            expected, actual, path, context, acceptance,
                        )?);
                        Ok(output)
                    })
                })
        }
    }
}

/// Relate one expected value against each actual Choice alternative.
fn relate_actual_choice_selected<A, D>(
    expected: &TypeKind,
    actual: &[TypeKind],
    path: ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
    acceptance: ConstraintAcceptance,
) -> Result<Vec<ConstraintPath<D>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    match acceptance {
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
                            acceptance,
                            context,
                        )?;
                        output.extend(relate_selected_call(
                            expected, actual, path, context, acceptance,
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
                        acceptance,
                        context,
                    )?;
                    output.extend(relate_selected_call(
                        expected, actual, path, context, acceptance,
                    )?);
                    Ok(output)
                })
        }
    }
}

/// Relate every expected/actual Choice pair in its own branch.
fn relate_choice_to_choice_selected<A, D>(
    expected: &[TypeKind],
    actual: &[TypeKind],
    path: ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
    acceptance: ConstraintAcceptance,
) -> Result<Vec<ConstraintPath<D>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    match acceptance {
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
                                acceptance,
                                context,
                            )?;
                            output.extend(relate_selected_call(
                                expected, actual, path, context, acceptance,
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
                                acceptance,
                                context,
                            )?;
                            output.extend(relate_selected_call(
                                expected, actual, path, context, acceptance,
                            )?);
                        }
                        Ok(output)
                    })
                })
        }
    }
}

fn relate_with_policy<A, D>(
    pattern: &TypeKind,
    actual: &TypeKind,
    path: ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
    acceptance: ConstraintAcceptance,
) -> Result<Vec<ConstraintPath<D>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    context.enter_node()?;
    relate_entered_with_policy(
        pattern,
        actual,
        pattern.constraint_shape(),
        actual.constraint_shape(),
        path,
        context,
        acceptance,
    )
}

fn relate_entered_with_policy<A, D>(
    pattern: &TypeKind,
    actual: &TypeKind,
    pattern_shape: TypeConstraintShape<'_>,
    actual_shape: TypeConstraintShape<'_>,
    path: ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
    acceptance: ConstraintAcceptance,
) -> Result<Vec<ConstraintPath<D>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    if matches!(pattern_shape, TypeConstraintShape::Unresolved)
        || matches!(actual_shape, TypeConstraintShape::Unresolved)
    {
        return Err(TypeConstraintError::Rejected(
            TypeConstraintRejection::UnresolvedType,
        ));
    }

    // Bottom contributes no binding, but unresolved descendants in the
    // expected shape still cannot reach a selected seal.
    if matches!(actual_shape, TypeConstraintShape::Never) {
        validate_type(pattern, context)?;
        return Ok(vec![path]);
    }

    if let TypeConstraintShape::Generic(parameter) = pattern_shape {
        if matches!(actual_shape, TypeConstraintShape::Generic(candidate) if candidate == parameter)
        {
            return Ok(vec![path]);
        }
        if matches!(
            context.parameter_eligibility(parameter),
            Some(TypeConstraintParameterEligibility::Rigid)
        ) {
            // A rigid enclosing parameter is an exact atom, not a
            // candidate-owned variable. A non-identical equation prunes
            // only this branch so a surrounding Choice can still select a
            // valid alternative.
            return Ok(Vec::new());
        }
        if let Some(bound) = path.bindings.get(parameter).cloned() {
            let bound_shape = bound.constraint_shape();
            return relate_entered_with_policy(
                &bound,
                actual,
                bound_shape,
                actual_shape,
                path,
                context,
                acceptance,
            );
        }
        return context
            .add_binding(path, parameter.clone(), actual, actual_shape, false)
            .map(|path| path.into_iter().collect());
    }

    match (pattern_shape, actual_shape) {
        (TypeConstraintShape::Choice(expected), TypeConstraintShape::Choice(found)) => {
            relate_choice_to_choice_selected(expected, found, path, context, acceptance)
        }
        (TypeConstraintShape::Choice(expected), _) => {
            relate_choice_to_actual_selected(expected, actual, path, context, acceptance)
        }
        (_, TypeConstraintShape::Choice(found)) => {
            relate_actual_choice_selected(pattern, found, path, context, acceptance)
        }
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
            relate_slices(
                expected_arguments,
                found_arguments,
                path,
                context,
                acceptance,
            )
        }
        (TypeConstraintShape::Ref(expected), TypeConstraintShape::Ref(found))
            if expected.kind() == found.kind() =>
        {
            match (expected.value(), found.value()) {
                (Some(expected), Some(found)) => {
                    relate_with_policy(expected, found, path, context, acceptance)
                }
                (Some(_), None) => Ok(vec![path]),
                (None, Some(found)) => {
                    validate_type(found, context)?;
                    Ok(vec![path])
                }
                (None, None) => Ok(vec![path]),
            }
        }
        (
            TypeConstraintShape::Unary {
                kind: expected_kind,
                child: expected,
            },
            TypeConstraintShape::Unary {
                kind: found_kind,
                child: found,
            },
        ) if expected_kind == found_kind => {
            relate_with_policy(expected, found, path, context, acceptance)
        }
        (
            TypeConstraintShape::Iterator {
                family: expected_family,
                item: expected,
            },
            TypeConstraintShape::Iterator {
                family: found_family,
                item: found,
            },
        ) if expected_family == found_family => {
            relate_with_policy(expected, found, path, context, acceptance)
        }
        (
            TypeConstraintShape::Array {
                item: expected,
                len: expected_len,
            },
            TypeConstraintShape::Array {
                item: found,
                len: found_len,
            },
        ) if expected_len == found_len => {
            relate_with_policy(expected, found, path, context, acceptance)
        }
        (
            TypeConstraintShape::Map {
                kind: expected_kind,
                key: expected_key,
                value: expected_value,
            },
            TypeConstraintShape::Map {
                kind: found_kind,
                key: found_key,
                value: found_value,
            },
        ) if expected_kind == found_kind => relate_pair(
            expected_key,
            found_key,
            expected_value,
            found_value,
            path,
            context,
            acceptance,
        ),
        (
            TypeConstraintShape::Borrow {
                kind: expected_kind,
                lifetime: expected_lifetime,
                inner: expected,
            },
            TypeConstraintShape::Borrow {
                kind: found_kind,
                lifetime: found_lifetime,
                inner: found,
            },
        ) if expected_kind == found_kind && expected_lifetime == found_lifetime => {
            relate_with_policy(expected, found, path, context, acceptance)
        }
        (
            TypeConstraintShape::Pair {
                kind: expected_kind,
                first: expected_first,
                second: expected_second,
            },
            TypeConstraintShape::Pair {
                kind: found_kind,
                first: found_first,
                second: found_second,
            },
        ) if expected_kind == found_kind => relate_pair(
            expected_first,
            found_first,
            expected_second,
            found_second,
            path,
            context,
            acceptance,
        ),
        (
            TypeConstraintShape::Function {
                params: expected_params,
                result: expected_result,
                effects: expected_effects,
            },
            TypeConstraintShape::Function {
                params: found_params,
                result: found_result,
                effects: found_effects,
            },
        ) if expected_params.len() == found_params.len() => {
            let parameter_acceptance = match acceptance {
                ConstraintAcceptance::PatternAcceptsActual => {
                    ConstraintAcceptance::ActualAcceptsPattern
                }
                ConstraintAcceptance::ActualAcceptsPattern => {
                    ConstraintAcceptance::PatternAcceptsActual
                }
            };
            let paths = relate_slices(
                expected_params,
                found_params,
                path,
                context,
                parameter_acceptance,
            )?;
            let mut effect_paths = Vec::with_capacity(paths.len());
            for mut path in paths {
                let (actual_effects, permitted_effects) = match acceptance {
                    ConstraintAcceptance::PatternAcceptsActual => (found_effects, expected_effects),
                    ConstraintAcceptance::ActualAcceptsPattern => (expected_effects, found_effects),
                };
                match path
                    .effects
                    .constrain_subset(actual_effects, permitted_effects)
                {
                    Ok(()) => effect_paths.push(path),
                    Err(EffectConstraintEnvironmentError::MissingEffects { .. }) => {}
                    Err(error) => return Err(map_effect_environment_error(error)),
                }
            }
            relate_many(
                expected_result,
                found_result,
                effect_paths,
                context,
                acceptance,
            )
        }
        (TypeConstraintShape::Tuple(expected), TypeConstraintShape::Tuple(found))
            if expected.len() == found.len() =>
        {
            relate_slices(expected, found, path, context, acceptance)
        }
        (TypeConstraintShape::Leaf(_), _) => Ok(vec![path]),
        (TypeConstraintShape::Never, TypeConstraintShape::Never) => Ok(vec![path]),
        _ => Ok(vec![path]),
    }
}

pub(crate) fn relate_slices<A, D>(
    expected: &[TypeKind],
    found: &[TypeKind],
    path: ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
    acceptance: ConstraintAcceptance,
) -> Result<Vec<ConstraintPath<D>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    if expected.len() != found.len() {
        return Ok(Vec::new());
    }
    expected
        .iter()
        .zip(found)
        .try_fold(vec![path], |paths, (expected, found)| {
            relate_many(expected, found, paths, context, acceptance)
        })
}

pub(crate) fn relate_pair<A, D>(
    expected_first: &TypeKind,
    found_first: &TypeKind,
    expected_second: &TypeKind,
    found_second: &TypeKind,
    path: ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
    acceptance: ConstraintAcceptance,
) -> Result<Vec<ConstraintPath<D>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let paths = relate_with_policy(expected_first, found_first, path, context, acceptance)?;
    relate_many(expected_second, found_second, paths, context, acceptance)
}

pub(crate) fn relate_many<A, D>(
    expected: &TypeKind,
    found: &TypeKind,
    paths: Vec<ConstraintPath<D>>,
    context: &mut TypeConstraintContext<'_, A, D>,
    acceptance: ConstraintAcceptance,
) -> Result<Vec<ConstraintPath<D>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    paths.into_iter().try_fold(Vec::new(), |mut output, path| {
        let path = context.fork_path(&path)?;
        output.extend(relate_with_policy(
            expected, found, path, context, acceptance,
        )?);
        Ok(output)
    })
}
