//! Source schemes and value-use demands in the closed-instance worklist.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arcweft_lang_hir::identity::ExprId;
use arcweft_lang_sema::callable::{
    CheckedFunctionSpecialization, CheckedProjectFunctionCallableSource,
    CheckedProjectFunctionCallableSourceDigest, CheckedProjectFunctionInstanceSolution,
    CheckedProjectFunctionSpecialization, CheckedProjectFunctionSpecializationTarget,
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
    ProjectInstantiationError, ProjectInstantiationSession,
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
    PositionalPrefix,
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct ProjectCallableSourceNode {
    pub(super) checked: CheckedProjectFunctionCallableSource,
    pub(super) fact: RuntimeProjectCallableSourceFact,
}

#[derive(Clone, Eq, PartialEq)]
pub(super) enum ProjectCallableDemandProof {
    PositionalPrefix {
        count: usize,
    },
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
pub(super) enum ProjectCallableUseResolution {
    Specialization(RuntimeCallableSpecializationKey),
    PositionalPrefix,
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
    pub(super) uses: BTreeMap<ProjectCallableValueUse, ProjectCallableUseResolution>,
}

impl ProjectCallableDemandNode {
    pub(super) fn matches(&self, source: &RuntimeProjectCallableSourceKey) -> bool {
        match &self.proof {
            ProjectCallableDemandProof::Value { .. }
            | ProjectCallableDemandProof::PositionalPrefix { .. } => {
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
        let source = analysis
            .project_function_value_runtime_with_control(
                declaration,
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

    pub(in crate::lower) fn callable_input(
        &mut self,
        owner: ExprId,
        selection: &CheckedProjectFunctionRuntimeSelection,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<RuntimeProjectCallableSourceKey, RuntimeSemanticProjectionError> {
        let origin = ProjectInstantiationOrigin::Call(owner);
        let checked = selection
            .input_callable_source(
                analysis.checked_callables(),
                enclosing,
                &mut self.callable_work().type_control(origin),
            )
            .map_err(|error| projection_error(origin, error))?;
        let RuntimeProjectCallableValueTarget::Source(source) =
            self.callable_source(origin, checked, enclosing, symbols, world, analysis)?
        else {
            return Err(origin.error("callable input must retain its source context"));
        };
        Ok(source)
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
        if let Some(closed) = checked.closed_selection() {
            let key = fact
                .body()
                .ok_or_else(|| origin.error("closed callable source has no body"))?;
            self.request(
                key.clone(),
                ProjectInstanceNode {
                    origin,
                    callable: fact.callable().clone(),
                    solution: closed.solution().clone(),
                },
            )?;
        }
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
        let root = if checked.origin()
            == arcweft_lang_sema::callable::CheckedProjectFunctionCallableOrigin::Root
        {
            None
        } else {
            let root = analysis
                .project_function_value_runtime_with_control(
                    checked.declaration(),
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

    pub(in crate::lower) fn discover_callable_uses(
        &mut self,
        owner: ExprId,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        symbols: &ProjectSymbolTable,
        world: &RegisteredSemanticWorld,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<(), RuntimeSemanticProjectionError> {
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
        let mut prefix = None;
        if let Some(application) = analysis
            .call(owner)
            .and_then(|call| call.selected_application())
            && let Some(end) = application.core().positional_partial_end()
            && matches!(
                application.result(),
                arcweft_lang_sema::callable::CheckedCallResult::Value(_)
            )
            && let arcweft_lang_sema::callable::CheckedCallCalleeExecution::Value { source } =
                application.core().callee()
            && let arcweft_lang_sema::callable::CheckedCallArgumentSlotSource::Expression(callee) =
                source.raw()
            && let Some(source) = analysis
                .expression(callee)
                .and_then(|expression| expression.value_type())
            && matches!(source, TypeKind::Function { binder, .. } if !binder.is_empty())
        {
            let target = application
                .result()
                .value_type()
                .ok_or_else(|| origin.error("positional prefix has no function result"))?;
            let count = end
                .get()
                .checked_sub(application.core().position().parameter().get())
                .ok_or_else(|| origin.error("positional prefix has invalid formal coordinates"))?;
            let demand = ProjectCallableDemandNode {
                origin,
                source: normalize(source)?,
                target: normalize(target)?,
                proof: ProjectCallableDemandProof::PositionalPrefix { count },
            };
            prefix = Some(demand);
        }
        let value = analysis
            .expression(owner)
            .and_then(|expression| expression.function_specialization())
            .map(|witness| {
                Ok::<_, RuntimeSemanticProjectionError>(ProjectCallableDemandNode {
                    origin,
                    source: normalize(witness.source_type())?,
                    target: normalize(witness.specialized_type())?,
                    proof: ProjectCallableDemandProof::Value {
                        witness: Arc::new(witness.clone()),
                        enclosing: enclosing.cloned(),
                    },
                })
            })
            .transpose()?;
        if let Some(demand) = prefix {
            self.insert_demand(
                self.callable_use(owner, ProjectCallableUseKind::PositionalPrefix),
                demand,
            )?;
        }
        if let Some(demand) = value {
            self.insert_demand(
                self.callable_use(owner, ProjectCallableUseKind::Value),
                demand,
            )?;
        }
        Ok(())
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
        if !selection.solution().is_fully_instantiated() {
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
            if let ProjectCallableDemandProof::PositionalPrefix { count } = &demand.proof {
                let checked = source
                    .checked
                    .retain_positional_prefix_with_control(
                        *count,
                        &mut self.work.type_control(demand.origin),
                    )
                    .map_err(|error| projection_error(demand.origin, error))?;
                let fact = callable_sources::source(
                    &checked,
                    Some(source.fact.root().clone()),
                    symbols,
                    world,
                    analysis,
                )?;
                if fact.function_type() != &demand.target {
                    return Err(demand
                        .origin
                        .error("positional source prefix disagrees with its checked result"));
                }
                let work = &self.work;
                let instances = self.nodes.len() as u64;
                let edges = self.edges.len() as u64;
                self.callables
                    .insert_source(ProjectCallableSourceNode { checked, fact }, || {
                        work.charge_graph(demand.origin, 1, instances, edges)
                    })?;
                let resolution = ProjectCallableUseResolution::PositionalPrefix;
                if self
                    .callables
                    .uses
                    .get(&use_key)
                    .is_some_and(|existing| existing != &resolution)
                {
                    return Err(demand
                        .origin
                        .error("positional prefix has a conflicting use resolution"));
                }
                self.callables.uses.insert(use_key, resolution);
                continue;
            }
            let checked = match &demand.proof {
                ProjectCallableDemandProof::PositionalPrefix { .. } => {
                    unreachable!("prefix demand was consumed above")
                }
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
            let target = match checked.target() {
                CheckedProjectFunctionSpecializationTarget::Closed(closed) => {
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
                            solution,
                        },
                    )?;
                    RuntimeProjectCallableValueTarget::Closed {
                        instance,
                        position: source.fact.position(),
                    }
                }
                CheckedProjectFunctionSpecializationTarget::Retain(next) => {
                    let root = (next.origin()
                        != arcweft_lang_sema::callable::CheckedProjectFunctionCallableOrigin::Root)
                        .then(|| source.fact.root().clone());
                    let fact = callable_sources::source(next, root, symbols, world, analysis)?;
                    let work = &self.work;
                    let instances = self.nodes.len() as u64;
                    let edges = self.edges.len() as u64;
                    let next_key = self.callables.insert_source(
                        ProjectCallableSourceNode {
                            checked: next.as_ref().clone(),
                            fact,
                        },
                        || work.charge_graph(demand.origin, 1, instances, edges),
                    )?;
                    RuntimeProjectCallableValueTarget::Source(next_key)
                }
            };
            let fact = callable_sources::specialization(
                &source.fact,
                &checked,
                target.clone(),
                symbols,
                world,
                analysis,
            )?;
            if fact.source() != &demand.source || fact.target() != &demand.target {
                return Err(demand
                    .origin
                    .error("checked specialization disagrees with source/use type evidence"));
            }
            let key = fact.key().clone();
            let resolution = ProjectCallableUseResolution::Specialization(key.clone());
            if let Some(existing) = self.callables.uses.get(&use_key)
                && existing != &resolution
            {
                return Err(demand
                    .origin
                    .error("one checked use produced conflicting substitutions"));
            }
            self.callables.uses.insert(use_key, resolution);
            if let Some(existing) = self.callables.specializations.remove(&key) {
                self.callables.specializations.insert(
                    key,
                    existing
                        .with_selection(source_key, target)
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
        let Some(resolution) = self.callables.uses.get(use_key) else {
            return Ok(None);
        };
        let ProjectCallableUseResolution::Specialization(key) = resolution else {
            return Err(ProjectInstantiationError::InvalidQueue.into());
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
