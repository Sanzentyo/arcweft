//! Source schemes and value-use demands in the closed-instance worklist.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arcweft_lang_hir::identity::ExprId;
use arcweft_lang_sema::callable::{
    CheckedFunctionSpecialization, CheckedProjectFunctionCallableSource,
    CheckedProjectFunctionCallableSourceDigest, CheckedProjectFunctionInstanceSolution,
    CheckedProjectFunctionSpecialization,
};
use arcweft_runtime_plan::semantic_facts::{
    RuntimeCallableSpecializationFact, RuntimeCallableSpecializationKey, RuntimeNormalizedType,
    RuntimeProjectCallableSourceFact, RuntimeProjectCallableSourceKey,
    RuntimeProjectFunctionInstanceKey,
};
use thiserror::Error;

use super::super::*;
use super::ProjectInstantiationOrigin;
use super::{
    DiscoveredProjectInstances, ProjectInstanceNode, ProjectInstanceProjection,
    ProjectInstanceSelection, ProjectInstantiationError, ProjectInstantiationSession,
};

/// The owner is scoped before use; the same HIR expression in two closed
/// instances contributes two independent checked demands.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ProjectCallableValueUse {
    pub(super) caller: Option<RuntimeProjectFunctionInstanceKey>,
    pub(super) expression: ExprId,
    pub(super) kind: ProjectCallableUseKind,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ProjectCallableUseKind {
    Value,
    CallInput,
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct ProjectCallableSourceNode {
    pub(super) checked: CheckedProjectFunctionCallableSource,
    pub(super) fact: RuntimeProjectCallableSourceFact,
}

#[derive(Clone, Eq, PartialEq)]
pub(super) enum ProjectCallableDemandProof {
    Value {
        witness: Arc<CheckedFunctionSpecialization>,
        enclosing: Option<CheckedProjectFunctionInstanceSolution>,
    },
    Input {
        source: CheckedProjectFunctionCallableSourceDigest,
        checked: CheckedProjectFunctionSpecialization,
    },
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct ProjectCallableDemandNode {
    pub(super) origin: ProjectInstantiationOrigin,
    pub(super) source: RuntimeNormalizedType,
    pub(super) target: RuntimeNormalizedType,
    pub(super) proof: ProjectCallableDemandProof,
}

/// This owner is consumed together with `ProjectInstantiationSession::seal`.
/// New sources and new demands both enqueue every matching pair; no scan order
/// or first observed use chooses the code origin of a function value.
#[derive(Default)]
pub(super) struct ProjectCallableDiscovery {
    pub(super) sources: BTreeMap<RuntimeProjectCallableSourceKey, Arc<ProjectCallableSourceNode>>,
    pub(super) demands: BTreeMap<ProjectCallableValueUse, Arc<ProjectCallableDemandNode>>,
    pub(super) pending: BTreeSet<(RuntimeProjectCallableSourceKey, ProjectCallableValueUse)>,
    pub(super) specializations:
        BTreeMap<RuntimeCallableSpecializationKey, RuntimeCallableSpecializationFact>,
    pub(super) uses: BTreeMap<ProjectCallableValueUse, RuntimeCallableSpecializationKey>,
}

impl ProjectCallableDemandNode {
    pub(super) fn matches(&self, source: &RuntimeProjectCallableSourceKey) -> bool {
        match &self.proof {
            ProjectCallableDemandProof::Value { .. } => {
                source.function_type() == self.source.identity()
            }
            ProjectCallableDemandProof::Input {
                source: expected, ..
            } => source.digest() == *expected,
        }
    }
}

#[derive(Clone, Debug, Error)]
pub enum ProjectCallableDiscoveryError {
    #[error("one callable source key has conflicting checked evidence: {key:?}")]
    ConflictingSource {
        key: RuntimeProjectCallableSourceKey,
    },
    #[error("one scoped callable value use has conflicting checked evidence at {expression:?}")]
    ConflictingDemand { expression: ExprId },
    #[error("callable specialization discovery still has unprocessed source/use pairs")]
    Incomplete,
    #[error("callable value use at {expression:?} has no reachable checked code origin")]
    MissingOrigin { expression: ExprId },
}

impl ProjectCallableDiscovery {
    pub(super) fn insert_source<E>(
        &mut self,
        source: ProjectCallableSourceNode,
        mut charge: impl FnMut() -> Result<(), E>,
    ) -> Result<RuntimeProjectCallableSourceKey, E>
    where
        E: From<ProjectCallableDiscoveryError>,
    {
        charge()?;
        let key = source.fact.key().clone();
        if let Some(existing) = self.sources.get(&key) {
            if existing.fact != source.fact {
                return Err(ProjectCallableDiscoveryError::ConflictingSource { key }.into());
            }
            return Ok(key);
        }
        for (use_key, demand) in &self.demands {
            charge()?;
            if demand.matches(&key) {
                self.pending.insert((key.clone(), use_key.clone()));
            }
        }
        self.sources.insert(key.clone(), Arc::new(source));
        Ok(key)
    }

    pub(super) fn insert_demand<E>(
        &mut self,
        use_key: ProjectCallableValueUse,
        demand: ProjectCallableDemandNode,
        mut charge: impl FnMut() -> Result<(), E>,
    ) -> Result<(), E>
    where
        E: From<ProjectCallableDiscoveryError>,
    {
        charge()?;
        if let Some(existing) = self.demands.get(&use_key) {
            if **existing != demand {
                return Err(ProjectCallableDiscoveryError::ConflictingDemand {
                    expression: use_key.expression,
                }
                .into());
            }
            return Ok(());
        }
        for source in self.sources.keys() {
            charge()?;
            if demand.matches(source) {
                self.pending.insert((source.clone(), use_key.clone()));
            }
        }
        self.demands.insert(use_key, Arc::new(demand));
        Ok(())
    }

    pub(super) fn validate_complete(&self) -> Result<(), ProjectCallableDiscoveryError> {
        if !self.pending.is_empty() {
            return Err(ProjectCallableDiscoveryError::Incomplete);
        }
        for use_key in self.demands.keys() {
            if !self.uses.contains_key(use_key) {
                return Err(ProjectCallableDiscoveryError::MissingOrigin {
                    expression: use_key.expression,
                });
            }
        }
        Ok(())
    }
}

impl ProjectInstanceProjection<'_> {
    fn callable_use(
        &self,
        expression: ExprId,
        kind: ProjectCallableUseKind,
    ) -> ProjectCallableValueUse {
        ProjectCallableValueUse {
            caller: match self {
                Self::Discover(session) => session.active.clone(),
                Self::Materialize { caller, .. } => caller.cloned(),
            },
            expression,
            kind,
        }
    }

    fn callable_work(&self) -> &super::work::ProjectInstantiationWork {
        match self {
            Self::Discover(session) => &session.work,
            Self::Materialize { graph, .. } => &graph.work,
        }
    }

    pub(in crate::lower) fn callable_root(
        &mut self,
        owner: ExprId,
        declaration: &CallableDeclarationKey,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<RuntimeProjectCallableValueTarget, RuntimeSemanticProjectionError> {
        let origin = ProjectInstantiationOrigin::CallableValue(owner);
        let source = arcweft_lang_sema::callable::select_project_function_value_runtime(
            declaration,
            analysis.checked_callables(),
            enclosing,
            &mut self.callable_work().type_control(origin),
        )
        .map_err(|error| projection_error(origin, error))?;
        self.callable_source(origin, source, enclosing, symbols, world, analysis)
    }

    pub(in crate::lower) fn callable_result(
        &mut self,
        owner: ExprId,
        selection: &CheckedProjectFunctionRuntimeSelection,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<RuntimeProjectCallableValueTarget, RuntimeSemanticProjectionError> {
        let origin = ProjectInstantiationOrigin::Call(owner);
        let source = selection
            .callable_value_source_with_control(
                analysis.checked_callables(),
                enclosing,
                &mut self.callable_work().type_control(origin),
            )
            .map_err(|error| projection_error(origin, error))?;
        self.callable_source(origin, source, enclosing, symbols, world, analysis)
    }

    fn insert_source(
        &mut self,
        origin: ProjectInstantiationOrigin,
        checked: CheckedProjectFunctionCallableSource,
        root: Option<RuntimeProjectCallableSourceKey>,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<RuntimeProjectCallableSourceKey, RuntimeSemanticProjectionError> {
        let fact = callable_sources::source(&checked, root, symbols, world, analysis)?;
        let key = fact.key().clone();
        match self {
            Self::Discover(session) => {
                let work = &session.work;
                let instances = session.nodes.len() as u64;
                let edges = session.edges.len() as u64;
                session
                    .callables
                    .insert_source(ProjectCallableSourceNode { checked, fact }, || {
                        work.charge_graph(origin, 1, instances, edges)
                    })?;
            }
            Self::Materialize { graph, .. } => {
                if graph
                    .callables
                    .sources
                    .get(&key)
                    .is_none_or(|source| source.fact != fact)
                {
                    return Err(origin.error("callable source was not sealed by discovery"));
                }
            }
        }
        Ok(key)
    }

    fn callable_source(
        &mut self,
        origin: ProjectInstantiationOrigin,
        checked: CheckedProjectFunctionCallableSource,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<RuntimeProjectCallableValueTarget, RuntimeSemanticProjectionError> {
        if let Some(closed) = checked.closed_selection() {
            let callable =
                runtime_project_callable(checked.declaration(), symbols, world, analysis)
                    .map_err(|reason| origin.error(reason))?;
            let solution = closed.solution().clone();
            let key = RuntimeProjectFunctionInstanceKey::new(
                callable.runtime().clone(),
                solution.instantiation(),
                closed.group(),
            );
            self.request(
                key.clone(),
                ProjectInstanceNode {
                    origin,
                    callable,
                    selection: ProjectInstanceSelection::from_root(closed),
                    solution,
                },
            )?;
            return Ok(RuntimeProjectCallableValueTarget::Closed(key));
        }
        let root = if checked.origin()
            == arcweft_lang_sema::callable::CheckedProjectFunctionCallableOrigin::Root
        {
            None
        } else {
            let root = arcweft_lang_sema::callable::select_project_function_value_runtime(
                checked.declaration(),
                analysis.checked_callables(),
                enclosing,
                &mut self.callable_work().type_control(origin),
            )
            .map_err(|error| projection_error(origin, error))?;
            Some(self.insert_source(origin, root, None, symbols, world, analysis)?)
        };
        Ok(RuntimeProjectCallableValueTarget::Source(
            self.insert_source(origin, checked, root, symbols, world, analysis)?,
        ))
    }

    pub(in crate::lower) fn application_function_type(
        &self,
        owner: ExprId,
        selection: &CheckedProjectFunctionRuntimeSelection,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<TypeKind, RuntimeSemanticProjectionError> {
        let origin = ProjectInstantiationOrigin::Call(owner);
        selection
            .application_function_type_with_control(
                analysis.checked_callables(),
                enclosing,
                &mut self.callable_work().type_control(origin),
            )
            .map_err(|error| projection_error(origin, error))
    }

    pub(in crate::lower) fn discover_value_specialization(
        &mut self,
        owner: ExprId,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<(), RuntimeSemanticProjectionError> {
        let Some(witness) = analysis
            .expression(owner)
            .and_then(|expression| expression.function_specialization())
        else {
            return Ok(());
        };
        let origin = ProjectInstantiationOrigin::CallableValue(owner);
        let normalize = |ty: &TypeKind| {
            let closed = enclosing
                .map(|enclosing| {
                    enclosing.instantiate_type_with_control(
                        ty,
                        &mut self.callable_work().type_control(origin),
                    )
                })
                .transpose()
                .map_err(|error| projection_error(origin, error.into()))?;
            runtime_type(closed.as_ref().unwrap_or(ty), symbols, world, analysis)
        };
        let demand = ProjectCallableDemandNode {
            origin,
            source: normalize(witness.source_type())?,
            target: normalize(witness.specialized_type())?,
            proof: ProjectCallableDemandProof::Value {
                witness: Arc::new(witness.clone()),
                enclosing: enclosing.cloned(),
            },
        };
        let use_key = self.callable_use(owner, ProjectCallableUseKind::Value);
        self.insert_demand(use_key, demand)
    }

    pub(in crate::lower) fn call_input_specialization(
        &mut self,
        owner: ExprId,
        callee: ExprId,
        selection: &CheckedProjectFunctionRuntimeSelection,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<Option<RuntimeProjectFunctionCallSpecialization>, RuntimeSemanticProjectionError>
    {
        let Some(expression) = analysis.expression(callee) else {
            return Err(ProjectInstantiationOrigin::Call(owner)
                .error("call input lacks checked expression"));
        };
        if expression.function_specialization().is_some() {
            return Ok(None);
        }
        let origin = ProjectInstantiationOrigin::Call(owner);
        let ty = expression
            .source_value_type()
            .ok_or_else(|| origin.error("call input has no source value type"))?;
        let closed = enclosing
            .map(|enclosing| {
                enclosing.instantiate_type_with_control(
                    ty,
                    &mut self.callable_work().type_control(origin),
                )
            })
            .transpose()
            .map_err(|error| projection_error(origin, error.into()))?;
        let TypeKind::Function { binder, .. } = closed.as_ref().unwrap_or(ty) else {
            return Err(origin.error("call input is not a Function"));
        };
        if binder.is_empty() {
            return Ok(None);
        }
        let checked = selection
            .specialize_input_callable_with_control(
                analysis.checked_callables(),
                enclosing,
                &mut self.callable_work().type_control(origin),
            )
            .map_err(|error| projection_error(origin, error))?;
        let (source, target, arguments) =
            callable_sources::specialization_types(&checked, symbols, world, analysis)?;
        let key = RuntimeCallableSpecializationKey::from_types(&source, &target, &arguments);
        let value = RuntimeProjectFunctionCallSpecialization::new(
            RuntimeCallableValueSpecialization::try_new(source.clone(), key)
                .map_err(|error| origin.error(error.to_string()))?,
            checked.source_digest(),
        );
        let use_key = self.callable_use(owner, ProjectCallableUseKind::CallInput);
        self.insert_demand(
            use_key,
            ProjectCallableDemandNode {
                origin,
                source,
                target,
                proof: ProjectCallableDemandProof::Input {
                    source: checked.source_digest(),
                    checked,
                },
            },
        )?;
        Ok(Some(value))
    }

    fn insert_demand(
        &mut self,
        use_key: ProjectCallableValueUse,
        demand: ProjectCallableDemandNode,
    ) -> Result<(), RuntimeSemanticProjectionError> {
        match self {
            Self::Discover(session) => {
                let work = &session.work;
                let instances = session.nodes.len() as u64;
                let edges = session.edges.len() as u64;
                let origin = demand.origin;
                session.callables.insert_demand(use_key, demand, || {
                    work.charge_graph(origin, 1, instances, edges)
                })?;
            }
            Self::Materialize { graph, .. } => {
                if graph
                    .callables
                    .demands
                    .get(&use_key)
                    .is_none_or(|known| **known != demand)
                {
                    return Err(demand
                        .origin
                        .error("callable demand was not sealed by discovery"));
                }
            }
        }
        Ok(())
    }

    pub(in crate::lower) fn value_specialization(
        &self,
        owner: ExprId,
    ) -> Result<Option<RuntimeCallableValueSpecialization>, RuntimeSemanticProjectionError> {
        let Self::Materialize { graph, .. } = self else {
            return Err(ProjectInstantiationOrigin::CallableValue(owner)
                .error("specialization read before discovery seal"));
        };
        graph.callable_specialization(&self.callable_use(owner, ProjectCallableUseKind::Value))
    }
}

fn projection_error(
    origin: ProjectInstantiationOrigin,
    source: arcweft_lang_sema::callable::CheckedProjectFunctionInstanceProjectionError<
        ProjectInstantiationError,
    >,
) -> RuntimeSemanticProjectionError {
    match source {
        arcweft_lang_sema::callable::CheckedProjectFunctionInstanceProjectionError::Projection(
            arcweft_lang_sema::types::TypeProjectionError::Control(error),
        ) => RuntimeSemanticProjectionError::ProjectInstantiation(error),
        source => RuntimeSemanticProjectionError::ProjectFunctionProjection {
            origin,
            source: Box::new(source),
        },
    }
}

impl ProjectInstantiationSession {
    pub(in crate::lower) fn discover_callable_specializations(
        &mut self,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<bool, RuntimeSemanticProjectionError> {
        let mut progress = false;
        while let Some((source_key, use_key)) = self.callables.pending.pop_first() {
            progress = true;
            let source = self
                .callables
                .sources
                .get(&source_key)
                .cloned()
                .ok_or(ProjectInstantiationError::InvalidQueue)?;
            let demand = self
                .callables
                .demands
                .get(&use_key)
                .cloned()
                .ok_or(ProjectInstantiationError::InvalidQueue)?;
            self.charge(demand.origin, 1, false, false)?;
            let checked = match &demand.proof {
                ProjectCallableDemandProof::Value { witness, enclosing } => source
                    .checked
                    .specialize_callable_value_with_control(
                        analysis.checked_callables(),
                        witness,
                        enclosing.as_ref(),
                        &mut self.work.type_control(demand.origin),
                    )
                    .map_err(|error| projection_error(demand.origin, error))?,
                ProjectCallableDemandProof::Input { checked, .. } => checked.clone(),
            };
            let fact =
                callable_sources::specialization(&source.fact, &checked, symbols, world, analysis)?;
            if fact.source() != &demand.source || fact.target() != &demand.target {
                return Err(demand
                    .origin
                    .error("checked specialization disagrees with source/use type evidence"));
            }
            let closed = checked.closed_selection();
            let solution = closed.solution().clone();
            let instance = RuntimeProjectFunctionInstanceKey::new(
                source.fact.callable().runtime().clone(),
                solution.instantiation(),
                closed.group(),
            );
            self.request_from(
                use_key.caller.clone(),
                instance.clone(),
                ProjectInstanceNode {
                    origin: demand.origin,
                    callable: source.fact.callable().clone(),
                    selection: ProjectInstanceSelection::from_root(closed),
                    solution,
                },
            )?;
            let key = fact.key().clone();
            if let Some(existing) = self.callables.uses.get(&use_key)
                && existing != &key
            {
                return Err(demand
                    .origin
                    .error("one checked use produced conflicting substitutions"));
            }
            self.callables.uses.insert(use_key, key.clone());
            if let Some(existing) = self.callables.specializations.remove(&key) {
                self.callables.specializations.insert(
                    key,
                    existing
                        .with_selection(source_key, instance)
                        .map_err(|error| demand.origin.error(error.to_string()))?,
                );
            } else {
                self.callables.specializations.insert(key, fact);
            }
        }
        Ok(progress)
    }
}

impl DiscoveredProjectInstances {
    fn callable_specialization(
        &self,
        use_key: &ProjectCallableValueUse,
    ) -> Result<Option<RuntimeCallableValueSpecialization>, RuntimeSemanticProjectionError> {
        let Some(key) = self.callables.uses.get(use_key) else {
            return Ok(None);
        };
        let fact = self
            .callables
            .specializations
            .get(key)
            .ok_or(ProjectInstantiationError::InvalidQueue)?;
        Ok(Some(
            RuntimeCallableValueSpecialization::try_new(fact.source().clone(), key.clone())
                .map_err(|error| {
                    ProjectInstantiationOrigin::CallableValue(use_key.expression)
                        .error(error.to_string())
                })?,
        ))
    }
    pub(in crate::lower) fn root_value_specialization(
        &self,
        owner: ExprId,
    ) -> Result<Option<RuntimeCallableValueSpecialization>, RuntimeSemanticProjectionError> {
        self.callable_specialization(&ProjectCallableValueUse {
            caller: None,
            expression: owner,
            kind: ProjectCallableUseKind::Value,
        })
    }
    pub(in crate::lower) fn append_callable_facts(&self, input: &mut RuntimePlanSemanticFactInput) {
        for source in self.callables.sources.values() {
            input.push_callable_source(source.fact.clone());
        }
        for fact in self.callables.specializations.values() {
            input.push_callable_specialization(fact.clone());
        }
    }
}
