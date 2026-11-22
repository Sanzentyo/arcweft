//! Prepare the shared cross-shape algebra using actual operand child ports.

use super::super::cross_shape::{AgentValueShape, CrossShapeCompatibility};
use super::*;
use crate::types::constraints::TypeLeaf;

pub(super) fn relate_cross_shape<A, D, L: Clone>(
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    expected: &ConstraintOperandView<'_, D>,
    supplied: &ConstraintOperandView<'_, D>,
    path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    direction: ConstraintAcceptance,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    if !path.deferred_cycles.parameters.is_empty() {
        return Ok(vec![path]);
    }
    match expected.value().cross_shape_compatibility(supplied.value()) {
        Some(CrossShapeCompatibility::ByteCollection(_)) => {
            let element = supplied
                .children(context)?
                .next()
                .transpose()?
                .ok_or(TypeConstraintInvariant::OriginalProducer)?;
            let expected = element.scalar_expectation(TypeLeaf::U8, context)?;
            relate_constraint(
                graph,
                &expected.view(),
                &element,
                path,
                context,
                ConstraintRelation::EQUAL,
            )
        }
        Some(CrossShapeCompatibility::AgentValue) => {
            relate_agent_value(graph, supplied, path, context, direction)
        }
        None => Ok(Vec::new()),
    }
}

pub(super) fn relate_agent_value<A, D, L: Clone>(
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    supplied: &ConstraintOperandView<'_, D>,
    mut path: ConstraintInferenceBranch<D, L>,
    context: &mut TypeConstraintContext<'_, A, D>,
    direction: ConstraintAcceptance,
) -> Result<Vec<ConstraintInferenceBranch<D, L>>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    context.enter_node()?;
    if !path.deferred_cycles.parameters.is_empty()
        && !matches!(
            supplied.value().constraint_shape(),
            TypeConstraintShape::Leaf(_) | TypeConstraintShape::Generic(_)
        )
    {
        return Ok(vec![path]);
    }
    match resolve_type_alias(supplied.clone(), &path, graph, context)? {
        ConstraintAliasResolution::Value(value) => {
            path.projection_view()
                .admit_operand(value.view(), graph.read(), context)?;
            return relate_agent_value(graph, &value.view(), path, context, direction);
        }
        ConstraintAliasResolution::Cycle(parameters) => {
            for parameter in parameters.iter() {
                context.enter_node()?;
                path.deferred_cycles
                    .parameters
                    .insert(parameter.clone().into());
            }
            return Ok(vec![path]);
        }
        ConstraintAliasResolution::Unchanged => {}
    }
    if let TypeConstraintShape::Generic(parameter) = supplied.value().constraint_shape() {
        let expected = supplied.scalar_expectation(TypeLeaf::AgentValue, context)?;
        return bind_type_parameter(graph, parameter, &expected.view(), path, context);
    }
    match supplied.value().agent_value_shape() {
        AgentValueShape::Accept => Ok(vec![path]),
        AgentValueShape::Reject => Ok(Vec::new()),
        AgentValueShape::Recovered => Err(TypeConstraintRejection::UnresolvedType.into()),
        AgentValueShape::Element(_) => {
            let element = supplied
                .children(context)?
                .next()
                .transpose()?
                .ok_or(TypeConstraintInvariant::OriginalProducer)?;
            relate_agent_value(graph, &element, path, context, direction)
        }
        AgentValueShape::Map { .. } => {
            let mut children = supplied.children(context)?;
            let key = children
                .next()
                .transpose()?
                .ok_or(TypeConstraintInvariant::OriginalProducer)?;
            let value = children
                .next()
                .transpose()?
                .ok_or(TypeConstraintInvariant::OriginalProducer)?;
            let expected = key.scalar_expectation(TypeLeaf::String, context)?;
            let paths = match direction {
                ConstraintAcceptance::PatternAcceptsActual => relate_constraint(
                    graph,
                    &expected.view(),
                    &key,
                    path,
                    context,
                    ConstraintRelation::accepts(direction),
                ),
                ConstraintAcceptance::ActualAcceptsPattern => relate_constraint(
                    graph,
                    &key,
                    &expected.view(),
                    path,
                    context,
                    ConstraintRelation::accepts(direction),
                ),
            }?;
            paths.into_iter().try_fold(Vec::new(), |mut output, path| {
                output.extend(relate_agent_value(graph, &value, path, context, direction)?);
                Ok(output)
            })
        }
        AgentValueShape::Alternatives(_) => supplied.children(context)?.enumerate().try_fold(
            vec![path],
            |paths, (index, alternative)| {
                let alternative = alternative?;
                paths.into_iter().try_fold(Vec::new(), |mut output, path| {
                    let (role, expected, actual) = match direction {
                        ConstraintAcceptance::PatternAcceptsActual => {
                            (ChoiceForkRole::ActualAlternative, None, Some(index))
                        }
                        ConstraintAcceptance::ActualAcceptsPattern => {
                            (ChoiceForkRole::ExpectedAlternative, Some(index), None)
                        }
                    };
                    let path = fork_choice_path(
                        &path,
                        role,
                        expected,
                        actual,
                        ConstraintRelation::accepts(direction),
                        context,
                    )?;
                    output.extend(relate_agent_value(
                        graph,
                        &alternative,
                        path,
                        context,
                        direction,
                    )?);
                    Ok(output)
                })
            },
        ),
    }
}
