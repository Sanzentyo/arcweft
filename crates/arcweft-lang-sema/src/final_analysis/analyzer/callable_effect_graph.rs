//! Callable-body call inventory, recursion rejection, and effect closure.

use super::{
    BTreeMap, BTreeSet, CallableDeclarationOwner, CallableEffectSchema, CheckedCallableId,
    EffectSet, ExprId, FinalSemanticAnalysisControl, FinalSemanticAnalysisError, HirModule,
    HirModuleId, PendingCallAnalysis, RecursiveCallableContractEdge, ScopeId, StagedCallableBody,
    statements::scope_executes_within,
};

type CallableEdges = BTreeMap<CheckedCallableId, BTreeMap<CheckedCallableId, BTreeSet<ExprId>>>;

struct IndexedCallableCall {
    scope: ScopeId,
    target: CheckedCallableId,
    expression: ExprId,
}

/// Sole owner of project-call edges used by callable effect inference.
///
/// Pending call facts are resolved to scopes exactly once. Body rows, closure
/// rows, and recursion diagnostics all consume this immutable inventory.
pub(super) struct CallableEffectGraph {
    owners: BTreeMap<CheckedCallableId, CallableDeclarationOwner>,
    edges: CallableEdges,
    calls_by_module: BTreeMap<HirModuleId, Vec<IndexedCallableCall>>,
}

impl CallableEffectGraph {
    pub(super) fn build(
        bodies: &[StagedCallableBody],
        pending_calls: &BTreeMap<ExprId, PendingCallAnalysis>,
        modules: &BTreeMap<HirModuleId, &HirModule>,
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
        let mut calls_by_module = BTreeMap::<HirModuleId, Vec<IndexedCallableCall>>::new();

        for pending in pending_calls.values() {
            control.check()?;
            if !matches!(
                pending.selected.schema().effects(),
                CallableEffectSchema::Project { .. }
            ) {
                continue;
            }
            let Some(target) = pending.selected.checked() else {
                continue;
            };
            if !body_ids.contains(target) {
                continue;
            }
            let module = resolve_module(modules, pending.expression.module())?;
            let expression = module
                .resolve_expr(pending.expression)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            calls_by_module
                .entry(module.module_id())
                .or_default()
                .push(IndexedCallableCall {
                    scope: expression.scope(),
                    target: target.clone(),
                    expression: pending.expression,
                });
        }

        let mut edges = body_ids
            .into_iter()
            .map(|body| (body, BTreeMap::new()))
            .collect::<CallableEdges>();
        for body in bodies {
            control.check()?;
            let module = resolve_module(modules, body.module)?;
            let targets = edges
                .get_mut(&body.id)
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            for call in calls_by_module.get(&body.module).into_iter().flatten() {
                if scope_executes_within(module, call.scope, body.scope)? {
                    targets
                        .entry(call.target.clone())
                        .or_default()
                        .insert(call.expression);
                }
            }
        }

        Ok(Self {
            owners,
            edges,
            calls_by_module,
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
                    if let Some(target_row) = previous.get(target) {
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

    pub(super) fn close_scope_effects(
        &self,
        module: &HirModule,
        scope: ScopeId,
        base: &EffectSet,
        rows: &BTreeMap<CheckedCallableId, EffectSet>,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<EffectSet, FinalSemanticAnalysisError> {
        let mut effects = base.clone();
        for call in self
            .calls_by_module
            .get(&module.module_id())
            .into_iter()
            .flatten()
        {
            control.check()?;
            if scope_executes_within(module, call.scope, scope)?
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

fn resolve_module<'a>(
    modules: &BTreeMap<HirModuleId, &'a HirModule>,
    owner: HirModuleId,
) -> Result<&'a HirModule, FinalSemanticAnalysisError> {
    modules
        .get(&owner)
        .copied()
        .ok_or(FinalSemanticAnalysisError::InvalidOwner)
}
