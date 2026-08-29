//! Callable-body call inventory, recursion rejection, and effect closure.

use super::{
    BTreeMap, BTreeSet, CallableDeclarationOwner, CallableEffectSchema, CheckedCallableId,
    EffectSet, ExprId, FinalSemanticAnalysisControl, FinalSemanticAnalysisError,
    RecursiveCallableContractEdge, StagedCallableBody, calls::AnalyzerPreparedCallGraph,
};
use crate::{
    callable::CheckedCallableDeclaration,
    final_analysis::statement_effects::PreparedExecutionEffectCatalog,
};

type CallableEdges = BTreeMap<CheckedCallableId, BTreeMap<CheckedCallableId, BTreeSet<ExprId>>>;

struct IndexedCallableCall {
    target: CheckedCallableId,
}

/// Sole owner of project-call edges used by callable effect inference.
///
/// Pending call facts are resolved to scopes exactly once. Body rows, closure
/// rows, and recursion diagnostics all consume this immutable inventory.
pub(super) struct CallableEffectGraph {
    owners: BTreeMap<CheckedCallableId, CallableDeclarationOwner>,
    edges: CallableEdges,
    calls_by_expression: BTreeMap<ExprId, IndexedCallableCall>,
}

impl CallableEffectGraph {
    pub(super) fn build(
        bodies: &[StagedCallableBody],
        prepared_calls: &AnalyzerPreparedCallGraph,
        execution: &PreparedExecutionEffectCatalog,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let owners = bodies
            .iter()
            .map(|body| (body.id.clone(), body.owner))
            .collect::<BTreeMap<_, _>>();
        if owners.len() != bodies.len() {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
        let body_ids = owners.keys().cloned().collect::<BTreeSet<_>>();
        let mut calls_by_expression = BTreeMap::<ExprId, IndexedCallableCall>::new();

        for node in prepared_calls.selected_nodes() {
            control.check()?;
            let application = node.prefix().application();
            if !matches!(
                application.selected().schema().effects(),
                CallableEffectSchema::Project { .. }
            ) {
                continue;
            }
            let Some(target) = application.selected().checked() else {
                continue;
            };
            if !body_ids.contains(target) {
                continue;
            }
            let owner = match node.site() {
                crate::callable::CheckedCallSite::HirCall(owner)
                | crate::callable::CheckedCallSite::DialogueApplication(owner) => owner,
            };
            if calls_by_expression
                .insert(
                    owner,
                    IndexedCallableCall {
                        target: target.clone(),
                    },
                )
                .is_some()
            {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            }
        }

        let mut edges = body_ids
            .into_iter()
            .map(|body| (body, BTreeMap::new()))
            .collect::<CallableEdges>();
        for body in bodies {
            control.check()?;
            let CheckedCallableDeclaration::Project(declaration) = body.id.declaration() else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            let targets = edges
                .get_mut(&body.id)
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            let expressions = execution
                .declaration_expressions(declaration)
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            for expression in expressions {
                if let Some(call) = calls_by_expression.get(&expression) {
                    targets
                        .entry(call.target.clone())
                        .or_default()
                        .insert(expression);
                }
            }
        }

        Ok(Self {
            owners,
            edges,
            calls_by_expression,
        })
    }

    pub(super) fn reject_recursive_contracts(
        &self,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut rejected = Vec::new();
        for component in strongly_connected_components(&self.edges) {
            control.check()?;
            let recursive = component.len() > 1
                || component.iter().any(|owner| {
                    self.edges
                        .get(owner)
                        .is_some_and(|targets| targets.contains_key(owner))
                });
            let contains_contract = component.iter().any(|owner| {
                matches!(
                    self.owners.get(owner).copied(),
                    Some(CallableDeclarationOwner::Predicate | CallableDeclarationOwner::Proof)
                )
            });
            if !recursive || !contains_contract {
                continue;
            }
            for caller in &component {
                let Some(targets) = self.edges.get(caller) else {
                    continue;
                };
                for (callee, expressions) in targets {
                    if !component.contains(callee) {
                        continue;
                    }
                    rejected.extend(expressions.iter().map(|expression| {
                        RecursiveCallableContractEdge::new(
                            caller.clone(),
                            callee.clone(),
                            *expression,
                        )
                    }));
                }
            }
        }
        if rejected.is_empty() {
            return Ok(());
        }
        rejected.sort();
        Err(FinalSemanticAnalysisError::RecursiveCallableContract {
            edges: rejected.into_boxed_slice(),
        })
    }

    pub(super) fn close_effect_rows(
        &self,
        rows: &mut BTreeMap<CheckedCallableId, EffectSet>,
        bounded_call_rows: &BTreeMap<CheckedCallableId, EffectSet>,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        for iteration in 0..=self.owners.len() {
            control.check()?;
            let previous = rows.clone();
            let mut changed = false;
            for (caller, targets) in &self.edges {
                let row = rows
                    .get_mut(caller)
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                for target in targets.keys() {
                    if let Some(target_row) = bounded_call_rows
                        .get(target)
                        .or_else(|| previous.get(target))
                    {
                        changed |= row.union_with(target_row);
                    }
                }
            }
            if !changed {
                return Ok(());
            }
            if iteration == self.owners.len() {
                return Err(FinalSemanticAnalysisError::AccountingOverflow);
            }
        }
        Ok(())
    }

    pub(super) fn close_selected_expression_effects(
        &self,
        expressions: impl IntoIterator<Item = ExprId>,
        base: &EffectSet,
        rows: &BTreeMap<CheckedCallableId, EffectSet>,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<EffectSet, FinalSemanticAnalysisError> {
        let mut effects = base.clone();
        for expression in expressions {
            control.check()?;
            if let Some(call) = self.calls_by_expression.get(&expression)
                && let Some(target_row) = rows.get(&call.target)
            {
                effects.union_with(target_row);
            }
        }
        Ok(effects)
    }
}

fn strongly_connected_components(edges: &CallableEdges) -> Vec<BTreeSet<CheckedCallableId>> {
    let mut visited = BTreeSet::new();
    let mut finish_order = Vec::with_capacity(edges.len());
    for root in edges.keys() {
        if visited.contains(root) {
            continue;
        }
        let mut stack = vec![(root.clone(), false)];
        while let Some((owner, expanded)) = stack.pop() {
            if expanded {
                finish_order.push(owner);
                continue;
            }
            if !visited.insert(owner.clone()) {
                continue;
            }
            stack.push((owner.clone(), true));
            if let Some(targets) = edges.get(&owner) {
                for target in targets.keys().rev() {
                    if edges.contains_key(target) && !visited.contains(target) {
                        stack.push((target.clone(), false));
                    }
                }
            }
        }
    }

    let mut reverse = edges
        .keys()
        .cloned()
        .map(|owner| (owner, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (caller, targets) in edges {
        for callee in targets.keys() {
            if let Some(callers) = reverse.get_mut(callee) {
                callers.insert(caller.clone());
            }
        }
    }

    visited.clear();
    let mut components = Vec::new();
    for root in finish_order.into_iter().rev() {
        if !visited.insert(root.clone()) {
            continue;
        }
        let mut component = BTreeSet::new();
        let mut stack = vec![root];
        while let Some(owner) = stack.pop() {
            component.insert(owner.clone());
            if let Some(callers) = reverse.get(&owner) {
                for caller in callers.iter().rev() {
                    if visited.insert(caller.clone()) {
                        stack.push(caller.clone());
                    }
                }
            }
        }
        components.push(component);
    }
    components
}
