//! Callable-body call inventory, recursion rejection, and effect closure.

use std::sync::Arc;

use super::{
    BTreeMap, BTreeSet, CallableDeclarationOwner, CallableEffectSchema, CheckedCallableId, ExprId,
    FinalSemanticAnalysisControl, FinalSemanticAnalysisError, RecursiveCallableContractEdge,
    StagedCallableBody, calls::AnalyzerPreparedCallGraph,
};
use crate::{
    callable::{
        CheckedCallSite, CheckedCallableDeclaration, PreparedCallableApplication,
        PreparedCallableEffectProjection, PreparedResolvedCallable,
    },
    effect_row::prepared::{PreparedEffectGraph, PreparedEffectRow},
    effect_row::{DecisionControl, DecisionWork, EffectRow},
    final_analysis::statement_effects::PreparedExecutionEffectCatalog,
    types::{TypeProjectionControl, TypeProjectionError, TypeProjectionNodeKind},
};

type CallableEdges = BTreeMap<CheckedCallableId, BTreeMap<CheckedCallableId, BTreeSet<ExprId>>>;

pub(super) struct CallableEffectCall {
    site: CheckedCallSite,
    selected: Arc<PreparedResolvedCallable>,
    projection: PreparedCallableEffectProjection,
}

impl CallableEffectCall {
    pub(super) fn selected(
        site: CheckedCallSite,
        application: &PreparedCallableApplication,
    ) -> Option<Self> {
        let selected = application.selected();
        if selected.id().is_flow_transfer()
            || application.retains_inputs()
            || !matches!(
                selected.schema().effects(),
                CallableEffectSchema::Project { .. }
            )
            || selected.checked().is_none()
        {
            return None;
        }
        Some(Self {
            site,
            selected: Arc::clone(application.selected_shared()),
            projection: application.effect_projection(),
        })
    }

    pub(super) fn target(&self) -> &CheckedCallableId {
        self.selected
            .checked()
            .expect("effect call retains a checked declaration")
    }

    pub(super) fn candidate(&self) -> &PreparedResolvedCallable {
        &self.selected
    }

    pub(super) fn expression(&self) -> ExprId {
        self.site.expression()
    }

    fn specialize(&self, row: &EffectRow) -> Result<EffectRow, FinalSemanticAnalysisError> {
        self.projection.specialize(row).map_err(|_| {
            FinalSemanticAnalysisError::CallResolutionFailed {
                owner: self.site.expression(),
            }
        })
    }
}

/// Terminal application equations over checked declarations. Both on-demand
/// callable projection and final analysis close the same kind of effect rows.
#[derive(Default)]
pub(super) struct CallableEffectEquations {
    edges: CallableEdges,
    calls: BTreeMap<ExprId, CallableEffectCall>,
}

impl CallableEffectEquations {
    pub(super) fn insert_call(
        &mut self,
        call: CallableEffectCall,
    ) -> Result<(), FinalSemanticAnalysisError> {
        if self.calls.insert(call.site.expression(), call).is_some() {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
        Ok(())
    }

    pub(super) fn insert_body(
        &mut self,
        caller: CheckedCallableId,
        expressions: impl IntoIterator<Item = ExprId>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut targets = BTreeMap::<_, BTreeSet<_>>::new();
        for expression in expressions {
            if let Some(call) = self.calls.get(&expression) {
                targets
                    .entry(call.target().clone())
                    .or_default()
                    .insert(expression);
            }
        }
        if self.edges.insert(caller, targets).is_some() {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum IndexedCallableSuspension {
    Project(CheckedCallableId),
    Closure(ExprId),
    NonSuspending,
    MaySuspend,
}

struct IndexedClosureExecution {
    direct_suspension: bool,
    expressions: Box<[ExprId]>,
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
    equations: CallableEffectEquations,
    execution_by_expression: BTreeMap<ExprId, IndexedCallableExecution>,
    closure_execution: BTreeMap<ExprId, IndexedClosureExecution>,
}

pub(super) fn prepared_call_execution_rows(
    prepared_calls: &AnalyzerPreparedCallGraph,
    control: FinalSemanticAnalysisControl<'_>,
) -> Result<
    BTreeMap<ExprId, crate::final_analysis::statement_effects::PreparedCallExecutionRow>,
    FinalSemanticAnalysisError,
> {
    let mut rows = BTreeMap::new();
    for node in prepared_calls.selected_nodes() {
        control.check()?;
        let application = node.prefix().application();
        let selected = application.selected();
        let terminal = !application.retains_inputs();
        let owner = node.site().expression();
        let effects = if selected.id().is_flow_transfer() {
            Some(EffectRow::closed(crate::effects::EffectSet::new()))
        } else {
            terminal
                .then(|| selected.schema().effects().fixed_row())
                .flatten()
                .map(|row| application.specialize_effect_row(row))
                .transpose()
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner })?
        };
        let receiver = node.prefix().receiver_evaluation();
        let row = crate::final_analysis::statement_effects::PreparedCallExecutionRow {
            effects,
            receiver,
        };
        if rows.insert(owner, row).is_some() {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
    }
    Ok(rows)
}

impl CallableEffectGraph {
    pub(super) fn build(
        bodies: &[StagedCallableBody],
        prepared_calls: &AnalyzerPreparedCallGraph,
        execution: &PreparedExecutionEffectCatalog,
        expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
        catalog: &crate::callable::CheckedCallableCatalogBuilder,
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
        let mut equations = CallableEffectEquations::default();
        let mut execution_by_expression = BTreeMap::new();
        let mut closure_execution = BTreeMap::new();

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
            if selected.id().is_flow_transfer() {
                if execution_by_expression
                    .insert(
                        owner,
                        IndexedCallableExecution {
                            suspension: IndexedCallableSuspension::NonSuspending,
                            control:
                                crate::final_analysis::CheckedExecutableControlRole::FlowRequired,
                        },
                    )
                    .is_some()
                {
                    return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                }
                continue;
            }
            let immediate_map = !application.retains_inputs()
                && matches!(selected.schema().validator(),
                    crate::callable::CallableValidator::StandardMap(family) if family.callback_is_immediate());
            let producer = if immediate_map {
                let mut prefix = node.prefix();
                loop {
                    control.check()?;
                    let source = prefix
                        .record()
                        .inputs()
                        .mapping()
                        .arguments()
                        .iter()
                        .flat_map(|argument| argument.slots())
                        .find(|slot| {
                            slot.coordinate().is_some_and(|coordinate| {
                                coordinate.group().get() == 0 && coordinate.parameter().get() == 0
                            })
                        })
                        .map(|slot| slot.source().owner());
                    if source.is_some() {
                        break source;
                    }
                    let Some(reference) = prefix.application().selected().prepared_continuation()
                    else {
                        break None;
                    };
                    prefix = prepared_calls
                        .continuation_prefix(reference)
                        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                }
            } else {
                selected.exact_value_producer()
            };
            let known_target = if let Some(super::CheckedExpressionResolution::Value(
                super::CheckedValueResolution::ProjectCallable(callable),
            )) = producer
                .and_then(|producer| expressions.get(&producer))
                .and_then(super::PreparedExpressionFact::checked_resolution)
            {
                Some(
                    catalog
                        .pending_by_candidate(&crate::callable::CallableCandidateId::Project(
                            callable.declaration().clone(),
                        ))
                        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?
                        .id()
                        .clone(),
                )
            } else {
                selected.checked().cloned()
            };
            let suspension = if application.retains_inputs() {
                // Partial application creates a continuation value. It does
                // not enter the callee frame at this application boundary.
                IndexedCallableSuspension::NonSuspending
            } else if let Some(target) = known_target.as_ref()
                && body_ids.contains(target)
            {
                IndexedCallableSuspension::Project(target.clone())
            } else if let Some(producer) = producer
                && let Some(closure) = execution.closure(producer)
            {
                closure_execution
                    .entry(producer)
                    .or_insert_with(|| IndexedClosureExecution {
                        direct_suspension: closure.direct_suspension(),
                        expressions: closure.expressions().collect(),
                    });
                IndexedCallableSuspension::Closure(producer)
            } else if !immediate_map
                && matches!(
                    known_target.as_ref().map(CheckedCallableId::declaration),
                    Some(
                        CheckedCallableDeclaration::Environment(_)
                            | CheckedCallableDeclaration::Standard(_)
                    )
                )
            {
                // Value dispatch still evaluates its callee, but preserves
                // the exact accepted declaration's non-suspending invocation.
                // Immediate Map must retain its independent callback proof.
                IndexedCallableSuspension::NonSuspending
            } else if immediate_map
                || selected.requires_value_callee()
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
                        || immediate_map
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
            if let Some(call) = CallableEffectCall::selected(node.site(), application)
                && body_ids.contains(call.target())
            {
                equations.insert_call(call)?;
            }
        }

        for body in bodies {
            control.check()?;
            let CheckedCallableDeclaration::Project(declaration) = body.id.declaration() else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            let expressions = execution
                .declaration_effect_expressions(declaration)
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            equations.insert_body(body.id.clone(), expressions)?;
        }

        Ok(Self {
            owners,
            equations,
            execution_by_expression,
            closure_execution,
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
        for iteration in 0..=self.owners.len() {
            control.check()?;
            let previous = rows.clone();
            let mut changed = false;
            for owner in self.owners.keys() {
                let CheckedCallableDeclaration::Project(declaration) = owner.declaration() else {
                    return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                };
                let row = rows
                    .get_mut(owner)
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                if !*row
                    && self.selected_expressions_may_suspend(
                        execution
                            .declaration_expressions(declaration)
                            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?,
                        &previous,
                    )
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
        crate::final_analysis::execution_regions::PreparedExecutableSuspensionCatalog,
        FinalSemanticAnalysisError,
    > {
        let project_row =
            |direct: bool,
             expressions: &BTreeSet<ExprId>,
             children: &BTreeSet<crate::final_analysis::CheckedExecutionOperation>,
             requires_flow: bool| {
                control.check()?;
                let suspension = if direct
                    || self.selected_expressions_may_suspend(expressions.iter().copied(), rows)
                {
                    crate::final_analysis::CheckedSuspensionRole::MaySuspend
                } else {
                    crate::final_analysis::CheckedSuspensionRole::NonSuspending
                };
                let control_role = self
                    .selected_expressions_control_role(expressions.iter().copied(), requires_flow);
                Ok::<_, FinalSemanticAnalysisError>(
                    crate::final_analysis::statement_effects::PreparedExecutableSuspensionRow::new(
                        children.iter().cloned().collect(),
                        suspension,
                        control_role,
                    ),
                )
            };
        let mut closed = BTreeMap::new();
        for (owner, direct, expressions, children, requires_flow) in
            execution.expression_execution_rows()
        {
            if closed
                .insert(
                    owner,
                    project_row(direct, expressions, children, requires_flow)?,
                )
                .is_some()
            {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            }
        }
        let mut bodies = BTreeMap::new();
        for (owner, direct, expressions, children, requires_flow) in execution.body_execution_rows()
        {
            bodies.insert(
                owner,
                project_row(direct, expressions, children, requires_flow)?,
            );
        }
        let mut statements = BTreeMap::new();
        for (owner, direct, expressions, children, requires_flow) in
            execution.statement_execution_rows()
        {
            statements.insert(
                owner,
                project_row(direct, expressions, children, requires_flow)?,
            );
        }
        let mut iterations = BTreeMap::new();
        for (owner, direct, expressions, children, requires_flow) in
            execution.iteration_execution_rows()
        {
            iterations.insert(
                owner,
                project_row(direct, expressions, children, requires_flow)?,
            );
        }
        Ok(
            crate::final_analysis::execution_regions::PreparedExecutableSuspensionCatalog::new(
                closed, statements, bodies, iterations,
            ),
        )
    }

    pub(super) fn selected_expressions_control_role(
        &self,
        expressions: impl IntoIterator<Item = ExprId>,
        requires_flow: bool,
    ) -> crate::final_analysis::CheckedExecutableControlRole {
        if requires_flow
            || expressions.into_iter().any(|expression| {
                self.execution_by_expression
                    .get(&expression)
                    .is_some_and(|call| {
                        call.control
                            == crate::final_analysis::CheckedExecutableControlRole::FlowRequired
                    })
            })
        {
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
        let mut pending = expressions.into_iter().collect::<Vec<_>>();
        let mut visited_closures = BTreeSet::new();
        while let Some(expression) = pending.pop() {
            let Some(call) = self.execution_by_expression.get(&expression) else {
                continue;
            };
            match &call.suspension {
                IndexedCallableSuspension::Project(target) => {
                    if rows.get(target).copied().unwrap_or(true) {
                        return true;
                    }
                }
                IndexedCallableSuspension::Closure(owner) => {
                    let Some(closure) = self.closure_execution.get(owner) else {
                        return true;
                    };
                    if visited_closures.insert(*owner) {
                        if closure.direct_suspension {
                            return true;
                        }
                        pending.extend(closure.expressions.iter().copied());
                    }
                }
                IndexedCallableSuspension::NonSuspending => {}
                IndexedCallableSuspension::MaySuspend => return true,
            }
        }
        false
    }

    pub(super) fn reject_recursive_contracts(
        &self,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut rejected = Vec::new();
        let edges = &self.equations.edges;
        for component in strongly_connected_components(edges) {
            control.check()?;
            let recursive = component.len() > 1
                || component.iter().any(|owner| {
                    edges
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
                let Some(targets) = edges.get(caller) else {
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
        rows: &mut BTreeMap<CheckedCallableId, EffectRow>,
        bounded_call_rows: &BTreeMap<CheckedCallableId, EffectRow>,
        catalog: &crate::callable::CheckedCallableCatalogBuilder,
        work_limit: u64,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        self.equations
            .close(rows, bounded_call_rows, catalog, work_limit, control)
    }

    pub(super) fn close_selected_expression_effects(
        &self,
        expressions: impl IntoIterator<Item = ExprId>,
        base: &EffectRow,
        rows: &BTreeMap<CheckedCallableId, EffectRow>,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<EffectRow, FinalSemanticAnalysisError> {
        let mut effects = base.clone();
        for expression in expressions {
            control.check()?;
            if let Some(call) = self.equations.calls.get(&expression) {
                let target_row = rows
                    .get(call.target())
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                let projected = call.specialize(target_row)?;
                effects = crate::final_analysis::statement_effects::union_effect_rows(
                    &effects, &projected, control,
                )?;
            }
        }
        Ok(effects)
    }
}

impl CallableEffectEquations {
    pub(super) fn close(
        &self,
        rows: &mut BTreeMap<CheckedCallableId, EffectRow>,
        boundary: &BTreeMap<CheckedCallableId, EffectRow>,
        catalog: &crate::callable::CheckedCallableCatalogBuilder,
        work_limit: u64,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        control.check()?;
        let mut work = CallableEffectClosureWork {
            control,
            limit: work_limit,
            remaining: work_limit,
        };
        if rows.len() != self.edges.len()
            || rows.keys().any(|owner| !self.edges.contains_key(owner))
        {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
        let mut graph = PreparedEffectGraph::default();
        let mut declarations = BTreeMap::new();
        // Register the actual headers before preparing any body. Recursive
        // applications refer to these declarations, never a guessed row.
        for owner in rows.keys() {
            work.charge(DecisionWork::Visit)?;
            let schema = catalog
                .pending_by_id(owner)
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?
                .record()
                .schema();
            let mut parameters = Vec::new();
            for parameter in schema.generic_inventory().effects() {
                work.charge(DecisionWork::Visit)?;
                parameters.push(parameter.parameter().clone());
            }
            let scheme = graph.declare(parameters, schema.effect_predicate().clone(), &mut work)?;
            let declaration = graph.declaration(&scheme, &mut work)?;
            declarations.insert(owner.clone(), declaration);
        }
        for (caller, targets) in &self.edges {
            work.charge(DecisionWork::Visit)?;
            let original = rows
                .get(caller)
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            let mut terms = vec![PreparedEffectRow::Canonical(
                original.clone_with_control(&mut work)?,
            )];
            for (target, expressions) in targets {
                for expression in expressions {
                    work.charge(DecisionWork::Visit)?;
                    let call = self
                        .calls
                        .get(expression)
                        .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                    if call.target() != target {
                        return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                    }
                    let specialize = |row: &EffectRow, work: &mut CallableEffectClosureWork<'_>| {
                        call.projection
                            .specialize_with_control(row, work)
                            .map_err(|error| match error {
                                TypeProjectionError::Control(error) => error,
                                TypeProjectionError::Instantiation(_) => {
                                    FinalSemanticAnalysisError::CallResolutionFailed {
                                        owner: *expression,
                                    }
                                }
                            })
                    };
                    let term = if let Some(row) = boundary.get(target) {
                        PreparedEffectRow::Canonical(specialize(row, &mut work)?)
                    } else {
                        let declaration = declarations
                            .get(target)
                            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                        let mut arguments = Vec::new();
                        for parameter in declaration.parameters() {
                            work.charge(DecisionWork::Visit)?;
                            let row = EffectRow::open(
                                crate::effects::EffectSet::new(),
                                parameter.clone(),
                            );
                            arguments
                                .push(PreparedEffectRow::Canonical(specialize(&row, &mut work)?));
                        }
                        graph.application(declaration.scheme(), arguments, &mut work)?
                    };
                    terms.push(term);
                }
            }
            let body = graph.union(terms, &mut work)?;
            graph.prepare_body(declarations[caller].scheme(), body, vec![], &mut work)?;
        }
        let closure = graph.solve_principal(&mut work)?;
        let mut completed = BTreeMap::new();
        for (owner, declaration) in declarations {
            work.charge(DecisionWork::Visit)?;
            let row =
                closure
                    .scheme(declaration.scheme())
                    .map_err(
                        crate::effect_row::prepared::PreparedEffectFailure::<
                            FinalSemanticAnalysisError,
                        >::from,
                    )?
                    .clone_with_control(&mut work)?;
            completed.insert(owner, row);
        }
        // All rows become observable together. Failure or cancellation above
        // leaves the caller's original typed body inputs untouched.
        *rows = completed;
        Ok(())
    }
}

struct CallableEffectClosureWork<'a> {
    control: FinalSemanticAnalysisControl<'a>,
    limit: u64,
    remaining: u64,
}

impl DecisionControl for CallableEffectClosureWork<'_> {
    type Error = FinalSemanticAnalysisError;

    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        self.control.check()?;
        self.remaining = self.remaining.checked_sub(1).ok_or(
            FinalSemanticAnalysisError::CallableEffectClosureWorkLimit { limit: self.limit },
        )?;
        Ok(())
    }
}

impl TypeProjectionControl for CallableEffectClosureWork<'_> {
    type Error = FinalSemanticAnalysisError;

    fn check(&mut self) -> Result<(), Self::Error> {
        self.control.check()
    }

    fn visit_node(&mut self, _: TypeProjectionNodeKind, _: u64) -> Result<(), Self::Error> {
        self.charge(DecisionWork::Visit)
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.charge(DecisionWork::Visit)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn staged_fixture(source: &str) -> super::super::StagedCheckedCallables {
        let fixture = crate::final_analysis::tests::fixture(source, None);
        let cancellation = AtomicBool::new(false);
        let mut analyzer = super::super::Analyzer::new(
            fixture.project.analysis_view().unwrap(),
            &fixture.symbols,
            super::super::FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .unwrap();
        analyzer.resolve_region_types(None).unwrap();
        analyzer.seed_local_types(None).unwrap();
        analyzer.stage_checked_callables().unwrap()
    }

    #[test]
    fn declaration_closure_retains_unused_formals_and_publishes_rows_atomically() {
        let staged =
            staged_fixture("fn choose(used: i64 -> i64, unused: i64 -> i64) -> i64 { used(0i64) }");
        let [body] = staged.bodies.as_slice() else {
            panic!("one actual declaration");
        };
        let schema = staged
            .builder
            .pending_by_id(&body.id)
            .unwrap()
            .record()
            .schema();
        assert_eq!(schema.generic_inventory().effects().len(), 2);
        let crate::types::TypeKind::Function { effects, .. } =
            schema.groups()[0].parameters()[0].declared_type().unwrap()
        else {
            panic!("used callback has its actual declaration-owned effect row");
        };
        let original = BTreeMap::from([(body.id.clone(), effects.clone())]);
        let mut equations = CallableEffectEquations::default();
        equations.insert_body(body.id.clone(), []).unwrap();
        let cancellation = AtomicBool::new(false);
        let control = FinalSemanticAnalysisControl::new(&cancellation);
        let boundary = BTreeMap::new();
        let production_limit = crate::callable::PRODUCTION_CALLABLE_LIMITS.max_catalog_build_work();
        let mut full = original.clone();
        equations
            .close(
                &mut full,
                &boundary,
                &staged.builder,
                production_limit,
                control,
            )
            .unwrap();
        assert_eq!(full, original);
        // Determine this real declaration's required closure work. The query
        // budget is a different owner; do not substitute it for catalog closure.
        let (mut low, mut high) = (0, production_limit);
        while low < high {
            let limit = low + (high - low) / 2;
            let mut rows = original.clone();
            match equations.close(&mut rows, &boundary, &staged.builder, limit, control) {
                Ok(()) => {
                    assert_eq!(rows, original);
                    high = limit;
                }
                Err(FinalSemanticAnalysisError::CallableEffectClosureWorkLimit {
                    limit: actual,
                }) => {
                    assert_eq!(actual, limit);
                    assert_eq!(rows, original);
                    low = limit + 1;
                }
                Err(error) => panic!("valid production declaration closure: {error:?}"),
            }
        }
        let required = low;
        let mut succeeded = false;
        for limit in 0..=required {
            let mut rows = original.clone();
            match equations.close(&mut rows, &boundary, &staged.builder, limit, control) {
                Ok(()) => {
                    assert_eq!(rows, original);
                    assert_eq!(limit, required);
                    succeeded = true;
                    break;
                }
                Err(FinalSemanticAnalysisError::CallableEffectClosureWorkLimit {
                    limit: actual,
                }) => {
                    assert_eq!(actual, limit);
                    assert_eq!(rows, original, "no publication at budget {limit}");
                }
                Err(error) => panic!("valid declaration closure at budget {limit}: {error:?}"),
            }
        }
        assert!(succeeded, "the full bounded preparation executes");
        cancellation.store(true, Ordering::Release);
        let mut rows = original.clone();
        assert_eq!(
            equations.close(
                &mut rows,
                &boundary,
                &staged.builder,
                production_limit,
                control
            ),
            Err(FinalSemanticAnalysisError::Cancelled)
        );
        assert_eq!(rows, original);
        cancellation.store(false, Ordering::Release);
        let foreign =
            staged_fixture("fn other(used: i64 -> i64, unused: i64 -> i64) -> i64 { used(0i64) }");
        assert_eq!(
            equations.close(
                &mut rows,
                &boundary,
                &foreign.builder,
                production_limit,
                control
            ),
            Err(FinalSemanticAnalysisError::CheckedCallableCatalog)
        );
        assert_eq!(rows, original);
    }
}
