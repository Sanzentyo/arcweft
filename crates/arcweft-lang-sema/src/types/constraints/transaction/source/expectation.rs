//! Open one probe's expected types in their actual application and lexical scopes.

use std::sync::Arc;

use crate::effect_row::prepared::{PreparedEffectGraph, PreparedEffectReadScope};
use crate::types::constraints::{
    ConstraintClosurePolicy, ConstraintDomain, OriginalProducerContext, PreparedSourceAlternative,
    PreparedSourceConstraint, TypeConstraintError,
    context::{TypeConstraintAccounting, TypeConstraintContext},
    hints::PreparedSourceExpectation,
    lexical::ConstraintLexicalScope,
    normalization::project_type,
};

use super::super::{
    ConstraintApplicationId, ConstraintOperand, ConstraintPath, OpenedSourceAlternative,
    operand::ConstraintEffectPhase,
};

pub(in crate::types::constraints::transaction) fn open_source_alternatives<
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
>(
    prepared: &PreparedSourceConstraint<D>,
    original: &OriginalProducerContext,
    application: ConstraintApplicationId,
    path: &ConstraintPath<D>,
    graph: &mut PreparedEffectGraph,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<
    (
        Vec<OpenedSourceAlternative<D>>,
        Option<PreparedEffectReadScope>,
    ),
    TypeConstraintError,
> {
    let reads = context.compose_effect_reads(graph, path.lineage.effect_read_scope())?;
    if prepared.is_unchecked() {
        // This probe still allocates an input even though it opens no types.
        context.enter_node()?;
        return Ok((Vec::new(), reads));
    }
    context.check_cancelled()?;
    let mut alternatives = prepared
        .alternatives()
        .map(PreparedSourceAlternative::expectation)
        .peekable();
    let mut opened = Vec::new();
    let mut scope = None;

    // The shared shape grammar proves that a closed leaf has no ports to
    // project.
    while let Some(leaf) = alternatives.next_if(|expected| expected.is_closed_leaf()) {
        opened.push(retain_leaf(
            leaf,
            original,
            application,
            path,
            &mut scope,
            context,
        )?);
    }
    if alternatives.peek().is_none() {
        return Ok((opened, reads));
    }

    // Open the first contextual expectation and all subsequent projections
    // under one graph edit. Leaves in the suffix still share their values.
    context.with_effect_read_scope(graph, reads.as_ref(), |context, graph| {
        for expected in alternatives {
            let alternative = if expected.is_closed_leaf() {
                retain_leaf(expected, original, application, path, &mut scope, context)?
            } else {
                let scope = retain_scope(&mut scope, context)?;
                open_contextual(expected, original, application, path, scope, graph, context)?
            };
            opened.push(alternative);
        }
        Ok(opened)
    })
}

fn retain_scope<A: TypeConstraintAccounting, D: ConstraintDomain>(
    scope: &mut Option<Arc<ConstraintLexicalScope>>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<Arc<ConstraintLexicalScope>, TypeConstraintError> {
    if let Some(scope) = scope {
        return Ok(Arc::clone(scope));
    }
    let retained = Arc::new(context.capture_lexical_scope()?);
    *scope = Some(Arc::clone(&retained));
    Ok(retained)
}

fn retain_leaf<A: TypeConstraintAccounting, D: ConstraintDomain>(
    expected: &PreparedSourceExpectation,
    original: &OriginalProducerContext,
    application: ConstraintApplicationId,
    path: &ConstraintPath<D>,
    scope: &mut Option<Arc<ConstraintLexicalScope>>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<OpenedSourceAlternative<D>, TypeConstraintError> {
    context.enter_node()?;
    let scope = retain_scope(scope, context)?;
    let operand = match expected {
        PreparedSourceExpectation::Declaration(value) => ConstraintOperand::template(
            Arc::clone(value),
            scope,
            path.applications.require_application_owner(application)?,
        ),
        PreparedSourceExpectation::Original(value) => {
            ConstraintOperand::original(Arc::clone(value), scope, original.clone())
        }
    };
    Ok(OpenedSourceAlternative {
        operand,
        projected_hint: None,
        unbound: Box::default(),
        scope_lease: None,
    })
}

fn open_contextual<A: TypeConstraintAccounting, D: ConstraintDomain>(
    expected: &PreparedSourceExpectation,
    original: &OriginalProducerContext,
    application: ConstraintApplicationId,
    path: &ConstraintPath<D>,
    scope: Arc<ConstraintLexicalScope>,
    graph: &mut PreparedEffectGraph,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<OpenedSourceAlternative<D>, TypeConstraintError> {
    let operand = match expected {
        PreparedSourceExpectation::Declaration(value) => {
            let value = context.open_template_type(value, path, application, graph)?;
            ConstraintOperand::template(
                value,
                scope,
                path.applications.require_application_owner(application)?,
            )
        }
        PreparedSourceExpectation::Original(value) => ConstraintOperand::from_prepared_original_in(
            value,
            scope,
            original.clone(),
            &mut ConstraintEffectPhase::new(graph),
            context,
        )?,
    };
    let (projected_hint, unbound) = if operand.value().constraint_shape().is_closed_leaf() {
        (None, Box::default())
    } else {
        let projected = project_type(
            operand.value(),
            path.projection_view(),
            ConstraintClosurePolicy::Hint,
            graph,
            context,
        )?;
        let unbound = projected
            .remaining
            .iter()
            .map(|parameter| parameter.parameter().clone())
            .collect::<Box<[_]>>();
        (Some(projected.value), unbound)
    };
    let scope_lease = if unbound.is_empty() {
        None
    } else {
        Some(path.applications.import_parameters(&unbound, context)?)
    };
    Ok(OpenedSourceAlternative {
        operand,
        projected_hint,
        unbound,
        scope_lease,
    })
}
