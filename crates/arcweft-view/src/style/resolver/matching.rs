//! Selector matching and traversal-budget accounting.

use super::{ResolveBudget, ViewStyleNodeFacts};
use crate::style::{
    ViewStyleApplication, ViewStyleCombinator, ViewStylePredicate, ViewStyleScopeId,
    ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleTraceRejection,
};

pub(super) fn selector_matches(
    selector: &ViewStyleSelector,
    ancestors: &[ViewStyleNodeFacts],
    node: &ViewStyleNodeFacts,
    application: &ViewStyleApplication,
    budget: &mut ResolveBudget,
    selector_limit: usize,
) -> Result<(), ViewStyleTraceRejection> {
    let sequences = selector.sequences();
    let last_index = sequences
        .len()
        .checked_sub(1)
        .ok_or(ViewStyleTraceRejection::SelectorMismatch)?;
    if application.boundary().is_nested_view_boundary() {
        let target = &sequences[last_index];
        let targets_inherited_root = application.boundary().allows_inherited_root();
        let targets_exported_part = application.boundary().is_exported_part()
            && target.part().is_some()
            && application.boundary().matches_part(
                target.part().expect("checked above"),
                node.implementation_part(),
                node.exported_part(),
            );
        // A public part is one target capability, not permission to expose the
        // private child ancestry. Until facts carry explicit boundary segments,
        // structural selectors stop at every crossed View boundary.
        if !(targets_inherited_root || targets_exported_part) || last_index != 0 {
            return Err(ViewStyleTraceRejection::BoundaryTraversalBlocked);
        }
    }
    match_sequence(
        &sequences[last_index],
        node,
        Some(application),
        budget,
        selector_limit,
    )?;
    let mut ancestor_limit = ancestors.len();
    for index in (0..last_index).rev() {
        let sequence = &sequences[index];
        match sequences[index + 1]
            .relation_to_previous()
            .unwrap_or(ViewStyleCombinator::Descendant)
        {
            ViewStyleCombinator::Child => {
                ancestor_limit = ancestor_limit
                    .checked_sub(1)
                    .ok_or(ViewStyleTraceRejection::SelectorMismatch)?;
                match_sequence(
                    sequence,
                    &ancestors[ancestor_limit],
                    application
                        .boundary()
                        .is_nested_view_boundary()
                        .then_some(application),
                    budget,
                    selector_limit,
                )?;
            }
            ViewStyleCombinator::Descendant => {
                let mut matched = None;
                for candidate in (0..ancestor_limit).rev() {
                    let result = match_sequence(
                        sequence,
                        &ancestors[candidate],
                        application
                            .boundary()
                            .is_nested_view_boundary()
                            .then_some(application),
                        budget,
                        selector_limit,
                    );
                    if budget.selector_exhausted {
                        return Err(ViewStyleTraceRejection::BoundaryTraversalBlocked);
                    }
                    if result.is_ok() {
                        matched = Some(candidate);
                        break;
                    }
                }
                ancestor_limit = matched.ok_or(ViewStyleTraceRejection::SelectorMismatch)?;
            }
        }
    }
    Ok(())
}

pub(super) fn scoped_ancestors(
    ancestors: &[ViewStyleNodeFacts],
    scope: ViewStyleScopeId,
) -> &[ViewStyleNodeFacts] {
    ancestors
        .iter()
        .position(|facts| facts.active_scopes().contains(&scope))
        .map_or(&[][..], |scope_root| &ancestors[scope_root..])
}

fn match_sequence(
    sequence: &ViewStyleSelectorSequence,
    node: &ViewStyleNodeFacts,
    application: Option<&ViewStyleApplication>,
    budget: &mut ResolveBudget,
    selector_limit: usize,
) -> Result<(), ViewStyleTraceRejection> {
    if !consume_selector_step(budget, selector_limit) {
        return Err(ViewStyleTraceRejection::BoundaryTraversalBlocked);
    }
    if sequence
        .element()
        .is_some_and(|element| node.element() != Some(element))
    {
        return Err(ViewStyleTraceRejection::SelectorMismatch);
    }
    let part_matches = sequence.part().is_none_or(|part| {
        application.map_or_else(
            || {
                node.implementation_part()
                    .is_some_and(|local| local.as_str() == part.as_str())
            },
            |application| {
                application.boundary().matches_part(
                    part,
                    node.implementation_part(),
                    node.exported_part(),
                )
            },
        )
    });
    if !part_matches {
        return Err(ViewStyleTraceRejection::SelectorMismatch);
    }
    for predicate in sequence.predicates() {
        if !consume_selector_step(budget, selector_limit) {
            return Err(ViewStyleTraceRejection::BoundaryTraversalBlocked);
        }
        match predicate {
            ViewStylePredicate::Interaction(state) if !node.interactions().contains(*state) => {
                return Err(ViewStyleTraceRejection::InteractionStateMismatch);
            }
            ViewStylePredicate::ElementState(state) if !node.element_states().contains(*state) => {
                return Err(ViewStyleTraceRejection::ElementStateMismatch);
            }
            ViewStylePredicate::Container(_) => {
                return Err(ViewStyleTraceRejection::ContainerFactsUnavailable);
            }
            ViewStylePredicate::Interaction(_) | ViewStylePredicate::ElementState(_) => {}
        }
    }
    Ok(())
}

fn consume_selector_step(budget: &mut ResolveBudget, limit: usize) -> bool {
    consume_selector_steps(budget, limit, 1)
}

pub(super) fn consume_selector_steps(
    budget: &mut ResolveBudget,
    limit: usize,
    steps: usize,
) -> bool {
    if steps > limit.saturating_sub(budget.selector_steps) {
        budget.selector_exhausted = true;
        false
    } else {
        budget.selector_steps += steps;
        true
    }
}
