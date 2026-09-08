//! Discovery and sealing of the closed project-function instance graph.
//!
//! Discovery owns the work queue. Consumers receive an immutable graph after
//! the queue is exhausted; materialization can only reference admitted keys.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, atomic::AtomicBool};

use arcweft_lang_hir::identity::{ExprId, ItemId};
use arcweft_lang_hir::symbol::CallableDeclarationKey;
use arcweft_lang_sema::callable::{
    CallableGroupIndex, CheckedProjectFunctionInstanceSolution,
    CheckedProjectFunctionRootRuntimeSelection, CheckedProjectFunctionRuntimeSelection,
};
use arcweft_lang_sema::effects::EffectSet;
use arcweft_runtime_plan::semantic_facts::{
    RuntimeProjectCallable, RuntimeProjectFunctionInstanceKey,
};
use thiserror::Error;

use super::RuntimeSemanticProjectionError;

mod work;
use work::ProjectInstantiationWork;
mod types;
pub(super) use types::ProjectInstanceTypes;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectInstantiationOrigin {
    Call(ExprId),
    Root(ItemId),
}

impl ProjectInstantiationOrigin {
    pub(super) fn error(self, reason: impl Into<String>) -> RuntimeSemanticProjectionError {
        let reason = reason.into();
        match self {
            Self::Call(owner) => RuntimeSemanticProjectionError::Call { owner, reason },
            Self::Root(owner) => {
                RuntimeSemanticProjectionError::ProjectFunctionInstance { owner, reason }
            }
        }
    }
}

/// Inclusive bounds shared by graph admission and controlled semantic type,
/// constant and effect projection in one compilation transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectInstantiationLimits {
    instances: u64,
    edges: u64,
    structural_nodes: u64,
    type_depth: u64,
    work: u64,
}

impl ProjectInstantiationLimits {
    pub const PRODUCTION: Self = Self::new(4_096, 65_536, 1_048_576, 128, 4_194_304);

    /// Zero admits an empty graph; each bound includes its exact maximum.
    pub const fn new(
        instances: u64,
        edges: u64,
        structural_nodes: u64,
        type_depth: u64,
        work: u64,
    ) -> Self {
        Self {
            instances,
            edges,
            structural_nodes,
            type_depth,
            work,
        }
    }
}

/// Inputs shared by one compiler transaction. Counters belong to its private
/// discovery session, so reusing a configuration does not retain prior work.
#[derive(Clone, Debug)]
pub struct ProjectInstantiationControl {
    cancellation: Arc<AtomicBool>,
    limits: ProjectInstantiationLimits,
}

impl Default for ProjectInstantiationControl {
    fn default() -> Self {
        Self::new(Arc::new(AtomicBool::new(false)))
    }
}

impl ProjectInstantiationControl {
    pub const fn new(cancellation: Arc<AtomicBool>) -> Self {
        Self {
            cancellation,
            limits: ProjectInstantiationLimits::PRODUCTION,
        }
    }

    #[must_use]
    pub const fn with_limits(mut self, limits: ProjectInstantiationLimits) -> Self {
        self.limits = limits;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectInstantiationLimitKind {
    Instances,
    Edges,
    StructuralNodes,
    TypeDepth,
    Work,
}

#[derive(Clone, Debug, Error)]
pub enum ProjectInstantiationError {
    #[error("project-function instantiation was cancelled at {origin:?}")]
    Cancelled {
        origin: Option<ProjectInstantiationOrigin>,
    },
    #[error(
        "project-function instantiation exceeded {kind:?} limit {limit}: attempted {attempted} at {origin:?}"
    )]
    LimitExceeded {
        kind: ProjectInstantiationLimitKind,
        limit: u64,
        attempted: u64,
        origin: ProjectInstantiationOrigin,
    },
    #[error("project-function instantiation {kind:?} counter overflowed at {origin:?}")]
    ArithmeticOverflow {
        kind: ProjectInstantiationLimitKind,
        origin: ProjectInstantiationOrigin,
    },
    #[error("project-function discovery still has pending instances")]
    IncompleteDiscovery,
    #[error("project-function discovery queue contains an absent instance")]
    InvalidQueue,
    #[error("project-function discovery has an unfinished work item")]
    ActiveDiscovery,
    #[error("project-function discovery received a stale or foreign work item")]
    ForeignWorkItem,
    #[error(
        "project-function materialization at {origin:?} requested an undiscovered instance {key:?}"
    )]
    UndiscoveredInstance {
        origin: ProjectInstantiationOrigin,
        key: RuntimeProjectFunctionInstanceKey,
    },
    #[error("project-function instance {key:?} has conflicting closed evidence at {origin:?}")]
    ConflictingInstance {
        origin: ProjectInstantiationOrigin,
        key: RuntimeProjectFunctionInstanceKey,
    },
    #[error(
        "project-function instance key {key:?} disagrees with its closed evidence at {origin:?}"
    )]
    InvalidInstanceKey {
        origin: ProjectInstantiationOrigin,
        key: RuntimeProjectFunctionInstanceKey,
    },
    #[error(
        "project-function materialization at {origin:?} requested an undiscovered dependency from {caller:?} to {callee:?}"
    )]
    UndiscoveredDependency {
        origin: ProjectInstantiationOrigin,
        caller: Box<RuntimeProjectFunctionInstanceKey>,
        callee: Box<RuntimeProjectFunctionInstanceKey>,
    },
    #[error(
        "project-function materialization at {origin:?} requested an undiscovered root {key:?}"
    )]
    UndiscoveredRoot {
        origin: ProjectInstantiationOrigin,
        key: RuntimeProjectFunctionInstanceKey,
    },
}

#[cfg_attr(test, derive(Clone))]
pub(super) struct ProjectInstanceNode {
    pub(super) origin: ProjectInstantiationOrigin,
    pub(super) callable: RuntimeProjectCallable,
    pub(super) selection: ProjectInstanceSelection,
    pub(super) solution: CheckedProjectFunctionInstanceSolution,
}

#[derive(Eq, PartialEq)]
#[cfg_attr(test, derive(Clone))]
pub(super) struct ProjectInstanceSelection {
    pub(super) declaration: CallableDeclarationKey,
    pub(super) group: CallableGroupIndex,
    pub(super) effects: EffectSet,
}

impl ProjectInstanceSelection {
    pub(super) fn from_call(selection: &CheckedProjectFunctionRuntimeSelection) -> Self {
        Self {
            declaration: selection.declaration().clone(),
            group: selection.group(),
            effects: selection.effects().clone(),
        }
    }

    pub(super) fn from_root(selection: &CheckedProjectFunctionRootRuntimeSelection) -> Self {
        Self {
            declaration: selection.declaration().clone(),
            group: selection.group(),
            effects: selection.effects().clone(),
        }
    }
}

pub(super) struct ProjectInstantiationSession {
    work: ProjectInstantiationWork,
    nodes: BTreeMap<RuntimeProjectFunctionInstanceKey, Arc<ProjectInstanceNode>>,
    roots: BTreeSet<(
        ProjectInstantiationOrigin,
        RuntimeProjectFunctionInstanceKey,
    )>,
    edges: BTreeSet<(
        RuntimeProjectFunctionInstanceKey,
        RuntimeProjectFunctionInstanceKey,
    )>,
    pending: BTreeSet<RuntimeProjectFunctionInstanceKey>,
    active: Option<RuntimeProjectFunctionInstanceKey>,
}

/// Completion permission for one admitted queue item. Dropping it leaves the
/// session unsealable; it cannot be cloned or issued outside this owner.
#[must_use = "complete the discovery work item before sealing the session"]
pub(super) struct ProjectInstanceWorkItem {
    key: RuntimeProjectFunctionInstanceKey,
    node: Arc<ProjectInstanceNode>,
}

impl ProjectInstanceWorkItem {
    pub(super) fn node(&self) -> &ProjectInstanceNode {
        &self.node
    }
}

impl ProjectInstantiationSession {
    pub(super) fn new(control: ProjectInstantiationControl) -> Self {
        Self {
            work: ProjectInstantiationWork::new(control),
            nodes: BTreeMap::new(),
            roots: BTreeSet::new(),
            edges: BTreeSet::new(),
            pending: BTreeSet::new(),
            active: None,
        }
    }

    fn check_cancelled(
        &self,
        origin: Option<ProjectInstantiationOrigin>,
    ) -> Result<(), ProjectInstantiationError> {
        self.work.check(origin)
    }

    fn charge(
        &self,
        origin: ProjectInstantiationOrigin,
        work: u64,
        instance: bool,
        edge: bool,
    ) -> Result<(), ProjectInstantiationError> {
        self.work.check(Some(origin))?;
        let count = |current: usize, increment: bool, kind| {
            u64::try_from(current)
                .ok()
                .and_then(|current| current.checked_add(u64::from(increment)))
                .ok_or_else(|| {
                    self.work
                        .abort(ProjectInstantiationError::ArithmeticOverflow { kind, origin })
                })
        };
        let instances = count(
            self.nodes.len(),
            instance,
            ProjectInstantiationLimitKind::Instances,
        )?;
        let edges = count(self.edges.len(), edge, ProjectInstantiationLimitKind::Edges)?;
        self.work.charge_graph(origin, work, instances, edges)
    }
    fn request(
        &mut self,
        key: RuntimeProjectFunctionInstanceKey,
        node: ProjectInstanceNode,
    ) -> Result<(), ProjectInstantiationError> {
        self.charge(node.origin, 1, false, false)?;
        node.validate_key(&key)?;
        if let Some(existing) = self.nodes.get(&key) {
            existing.validate_same_instance(&key, &node)?;
        }
        let new_instance = !self.nodes.contains_key(&key);
        let edge = self
            .active
            .as_ref()
            .map(|caller| (caller.clone(), key.clone()));
        let new_edge = edge.as_ref().is_some_and(|edge| !self.edges.contains(edge));
        // The visit is charged above. Admission charges the transition into
        // Queued and both distinct-graph budgets before mutating any table.
        self.charge(node.origin, u64::from(new_instance), new_instance, new_edge)?;
        if let Some(edge) = edge {
            self.edges.insert(edge);
        } else {
            self.roots.insert((node.origin, key.clone()));
        }
        if new_instance {
            self.nodes.insert(key.clone(), Arc::new(node));
            self.pending.insert(key);
        }
        Ok(())
    }

    pub(super) fn next(
        &mut self,
    ) -> Result<Option<ProjectInstanceWorkItem>, ProjectInstantiationError> {
        self.check_cancelled(None)?;
        if self.active.is_some() {
            return Err(ProjectInstantiationError::ActiveDiscovery);
        }
        let Some(key) = self.pending.first().cloned() else {
            return Ok(None);
        };
        let node = self
            .nodes
            .get(&key)
            .cloned()
            .ok_or(ProjectInstantiationError::InvalidQueue)?;
        self.charge(node.origin, 1, false, false)?;
        self.pending.remove(&key);
        self.active = Some(key.clone());
        Ok(Some(ProjectInstanceWorkItem { key, node }))
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "completion consumes the affine work permission so it cannot be replayed"
    )]
    pub(super) fn complete(
        &mut self,
        work: ProjectInstanceWorkItem,
    ) -> Result<(), ProjectInstantiationError> {
        self.check_cancelled(Some(work.node.origin))?;
        if self.active.as_ref() != Some(&work.key)
            || !self
                .nodes
                .get(&work.key)
                .is_some_and(|node| Arc::ptr_eq(node, &work.node))
        {
            return Err(ProjectInstantiationError::ForeignWorkItem);
        }
        self.charge(work.node.origin, 1, false, false)?;
        self.active = None;
        Ok(())
    }

    pub(super) fn seal(self) -> Result<DiscoveredProjectInstances, ProjectInstantiationError> {
        self.check_cancelled(None)?;
        if self.active.is_some() {
            return Err(ProjectInstantiationError::ActiveDiscovery);
        }
        if !self.pending.is_empty() {
            return Err(ProjectInstantiationError::IncompleteDiscovery);
        }
        Ok(DiscoveredProjectInstances {
            work: self.work,
            nodes: self.nodes,
            roots: self.roots,
            edges: self.edges,
        })
    }
}

impl ProjectInstanceNode {
    fn validate_key(
        &self,
        key: &RuntimeProjectFunctionInstanceKey,
    ) -> Result<(), ProjectInstantiationError> {
        if key.callable() != self.callable.runtime()
            || key.group() != self.selection.group
            || key.instantiation() != self.solution.instantiation()
            || &self.selection.declaration != self.callable.declaration()
        {
            return Err(ProjectInstantiationError::InvalidInstanceKey {
                origin: self.origin,
                key: key.clone(),
            });
        }
        Ok(())
    }

    fn validate_same_instance(
        &self,
        key: &RuntimeProjectFunctionInstanceKey,
        other: &Self,
    ) -> Result<(), ProjectInstantiationError> {
        if self.callable != other.callable
            || self.selection != other.selection
            || self.solution != other.solution
        {
            return Err(ProjectInstantiationError::ConflictingInstance {
                origin: other.origin,
                key: key.clone(),
            });
        }
        Ok(())
    }
}

pub(super) struct DiscoveredProjectInstances {
    work: ProjectInstantiationWork,
    nodes: BTreeMap<RuntimeProjectFunctionInstanceKey, Arc<ProjectInstanceNode>>,
    roots: BTreeSet<(
        ProjectInstantiationOrigin,
        RuntimeProjectFunctionInstanceKey,
    )>,
    edges: BTreeSet<(
        RuntimeProjectFunctionInstanceKey,
        RuntimeProjectFunctionInstanceKey,
    )>,
}

impl DiscoveredProjectInstances {
    pub(super) fn types<'a>(&'a self, node: &'a ProjectInstanceNode) -> ProjectInstanceTypes<'a> {
        ProjectInstanceTypes::new(node.origin, &node.solution, &self.work)
    }

    pub(super) fn check_cancelled(
        &self,
        origin: ProjectInstantiationOrigin,
    ) -> Result<(), ProjectInstantiationError> {
        self.work.check(Some(origin))
    }

    pub(super) fn nodes(
        &self,
    ) -> impl Iterator<Item = (&RuntimeProjectFunctionInstanceKey, &ProjectInstanceNode)> {
        self.nodes.iter().map(|(key, node)| (key, node.as_ref()))
    }
}

/// A call projection either discovers its exact target or proves membership
/// in the already sealed graph. The latter route cannot extend the graph.
pub(super) enum ProjectInstanceProjection<'session> {
    Discover(&'session mut ProjectInstantiationSession),
    Materialize {
        graph: &'session DiscoveredProjectInstances,
        caller: Option<&'session RuntimeProjectFunctionInstanceKey>,
    },
}

impl ProjectInstanceProjection<'_> {
    pub(super) fn close_instance(
        &self,
        origin: ProjectInstantiationOrigin,
        selection: &CheckedProjectFunctionRuntimeSelection,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
    ) -> Result<CheckedProjectFunctionInstanceSolution, RuntimeSemanticProjectionError> {
        let work = match self {
            Self::Discover(session) => &session.work,
            Self::Materialize { graph, .. } => &graph.work,
        };
        selection.close_instance_with_control(enclosing, &mut work.type_control(origin)).map_err(|source| {
            match source {
                arcweft_lang_sema::callable::CheckedProjectFunctionInstanceProjectionError::Projection(
                    arcweft_lang_sema::types::TypeProjectionError::Control(error)
                ) => RuntimeSemanticProjectionError::ProjectInstantiation(error),
                source => RuntimeSemanticProjectionError::ProjectFunctionProjection { origin, source: Box::new(source) },
            }
        })
    }

    pub(super) fn request(
        &mut self,
        key: RuntimeProjectFunctionInstanceKey,
        node: ProjectInstanceNode,
    ) -> Result<(), ProjectInstantiationError> {
        match self {
            Self::Discover(session) => session.request(key, node),
            Self::Materialize { graph, caller } => {
                graph.check_cancelled(node.origin)?;
                node.validate_key(&key)?;
                let Some(existing) = graph.nodes.get(&key) else {
                    return Err(ProjectInstantiationError::UndiscoveredInstance {
                        origin: node.origin,
                        key,
                    });
                };
                existing.validate_same_instance(&key, &node)?;
                match caller {
                    Some(caller) if !graph.edges.contains(&((*caller).clone(), key.clone())) => {
                        return Err(ProjectInstantiationError::UndiscoveredDependency {
                            origin: node.origin,
                            caller: Box::new((*caller).clone()),
                            callee: Box::new(key),
                        });
                    }
                    None if !graph.roots.contains(&(node.origin, key.clone())) => {
                        return Err(ProjectInstantiationError::UndiscoveredRoot {
                            origin: node.origin,
                            key,
                        });
                    }
                    Some(_) | None => {}
                }
                Ok(())
            }
        }
    }
}
