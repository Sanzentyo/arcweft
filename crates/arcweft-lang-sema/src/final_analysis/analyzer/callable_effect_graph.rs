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

#[derive(Clone, Debug, Eq, PartialEq)]
enum IndexedCallableSuspension {
    Project(CheckedCallableId),
    NonSuspending,
    MaySuspend,
}

struct IndexedCallableExecution {
    suspension: IndexedCallableSuspension,
    control: crate::final_analysis::CheckedExecutableControlRole,
}

/// Sole owner of project-call edges used by callable effect inference.
///
/// Pending call facts are resolved to scopes exactly once. Body rows, closure
/// rows, and recursion diagnostics all consume this immutable inventory.
pub(super) struct CallableEffectGraph {
    owners: BTreeMap<CheckedCallableId, CallableDeclarationOwner>,
    edges: CallableEdges,
    calls_by_expression: BTreeMap<ExprId, IndexedCallableCall>,
    execution_by_expression: BTreeMap<ExprId, IndexedCallableExecution>,
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
        let mut execution_by_expression = BTreeMap::new();

        for node in prepared_calls.selected_nodes() {
            control.check()?;
            let application = node.prefix().application();
            let owner = match node.site() {
                crate::callable::CheckedCallSite::HirCall(owner)
                | crate::callable::CheckedCallSite::AttachedContentApplication {
                    expression: owner,
                    ..
                } => owner,
            };
            let selected = application.selected();
            let suspension = if selected
                .next_group_for(application.completed_group())
                .is_some()
            {
                // Partial application creates a continuation value. It does
                // not enter the callee frame at this application boundary.
                IndexedCallableSuspension::NonSuspending
            } else if let Some(target) = selected.checked()
                && body_ids.contains(target)
            {
                IndexedCallableSuspension::Project(target.clone())
            } else if selected.requires_value_callee()
                || matches!(
                    selected.checked().map(CheckedCallableId::declaration),
                    Some(
                        CheckedCallableDeclaration::Project(_)
                            | CheckedCallableDeclaration::Detached(_)
                    )
                )
            {
                // A dynamic function value and any project/detached body not
                // represented by this complete body graph are deliberately
                // fail-closed. Language-owned direct intrinsics and accepted
                // environment/standard runtime records do not suspend the
                // current frame; suspension remains an explicit checked
                // project executable property.
                IndexedCallableSuspension::MaySuspend
            } else {
                IndexedCallableSuspension::NonSuspending
            };
            let control_role = match node.site() {
                crate::callable::CheckedCallSite::AttachedContentApplication {
                    family: crate::callable::CheckedAttachedContentApplicationFamily::DialogueLine,
                    ..
                } => crate::final_analysis::CheckedExecutableControlRole::FlowRequired,
                crate::callable::CheckedCallSite::HirCall(_)
                | crate::callable::CheckedCallSite::AttachedContentApplication {
                    family: crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
                    ..
                } => {
                    if selected.checked().and_then(|target| owners.get(target))
                        == Some(&CallableDeclarationOwner::Function)
                        || selected.requires_value_callee()
                    {
                        crate::final_analysis::CheckedExecutableControlRole::FlowRequired
                    } else {
                        crate::final_analysis::CheckedExecutableControlRole::ExpressionCompatible
                    }
                }
            };
            if execution_by_expression
                .insert(
                    owner,
                    IndexedCallableExecution {
                        suspension,
                        control: control_role,
                    },
                )
                .is_some()
            {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            }
            if selected
                .next_group_for(application.completed_group())
                .is_none()
                && matches!(
                    selected.schema().effects(),
                    CallableEffectSchema::Project { .. }
                )
                && let Some(target) = selected.checked()
                && body_ids.contains(target)
                && calls_by_expression
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
            execution_by_expression,
        })
    }

    pub(super) fn close_suspension_roles(
        &self,
        rows: &mut BTreeMap<CheckedCallableId, bool>,
        execution: &PreparedExecutionEffectCatalog,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        if rows.len() != self.owners.len()
            || rows.keys().any(|owner| !self.owners.contains_key(owner))
        {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
        let direct = rows.clone();
        for owner in self.owners.keys() {
            let CheckedCallableDeclaration::Project(declaration) = owner.declaration() else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            let may_suspend = self.selected_expressions_may_suspend(
                execution
                    .declaration_expressions(declaration)
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?,
                &direct,
            );
            if may_suspend {
                *rows
                    .get_mut(owner)
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)? = true;
            }
        }
        for iteration in 0..=self.owners.len() {
            control.check()?;
            let previous = rows.clone();
            let mut changed = false;
            for (caller, targets) in &self.edges {
                let row = rows
                    .get_mut(caller)
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                if !*row
                    && targets
                        .keys()
                        .any(|target| previous.get(target).copied().unwrap_or(true))
                {
                    *row = true;
                    changed = true;
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

    pub(super) fn close_selected_expression_suspension(
        &self,
        expressions: impl IntoIterator<Item = ExprId>,
        direct: bool,
        rows: &BTreeMap<CheckedCallableId, bool>,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<bool, FinalSemanticAnalysisError> {
        let expressions = expressions.into_iter().collect::<Vec<_>>();
        for _ in &expressions {
            control.check()?;
        }
        Ok(direct || self.selected_expressions_may_suspend(expressions, rows))
    }

    pub(super) fn close_executable_expression_suspensions(
        &self,
        execution: &PreparedExecutionEffectCatalog,
        rows: &BTreeMap<CheckedCallableId, bool>,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<
        BTreeMap<ExprId, crate::final_analysis::statement_effects::PreparedExecutableSuspensionRow>,
        FinalSemanticAnalysisError,
    > {
        let mut closed = BTreeMap::new();
        for (owner, direct, expressions) in execution.expression_execution_rows() {
            control.check()?;
            let suspension = if direct
                || self.selected_expressions_may_suspend(expressions.iter().copied(), rows)
            {
                crate::final_analysis::CheckedSuspensionRole::MaySuspend
            } else {
                crate::final_analysis::CheckedSuspensionRole::NonSuspending
            };
            let control_role = self.selected_expressions_control_role(expressions.iter().copied());
            let expressions = expressions
                .iter()
                .copied()
                .collect::<Vec<_>>()
                .into_boxed_slice();
            if closed
                .insert(
                    owner,
                    crate::final_analysis::statement_effects::PreparedExecutableSuspensionRow::new(
                        expressions,
                        suspension,
                        control_role,
                    ),
                )
                .is_some()
            {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            }
        }
        Ok(closed)
    }

    pub(super) fn selected_expressions_control_role(
        &self,
        expressions: impl IntoIterator<Item = ExprId>,
    ) -> crate::final_analysis::CheckedExecutableControlRole {
        if expressions.into_iter().any(|expression| {
            self.execution_by_expression
                .get(&expression)
                .is_some_and(|call| {
                    call.control
                        == crate::final_analysis::CheckedExecutableControlRole::FlowRequired
                })
        }) {
            crate::final_analysis::CheckedExecutableControlRole::FlowRequired
        } else {
            crate::final_analysis::CheckedExecutableControlRole::ExpressionCompatible
        }
    }

    fn selected_expressions_may_suspend(
        &self,
        expressions: impl IntoIterator<Item = ExprId>,
        rows: &BTreeMap<CheckedCallableId, bool>,
    ) -> bool {
        expressions.into_iter().any(|expression| {
            self.execution_by_expression
                .get(&expression)
                .is_some_and(|call| match &call.suspension {
                    IndexedCallableSuspension::Project(target) => {
                        rows.get(target).copied().unwrap_or(true)
                    }
                    IndexedCallableSuspension::NonSuspending => false,
                    IndexedCallableSuspension::MaySuspend => true,
                })
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
