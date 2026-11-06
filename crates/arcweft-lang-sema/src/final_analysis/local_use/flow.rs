//! Temporary ownership control flow emitted by the selected evaluation fold.
//! Only typed events are replayed; source occurrences and certificates are
//! created once. This graph is discarded before the local-use seal publishes.

use super::{
    CheckedDisplacedField, CheckedLocalAccess, CheckedLocalPlaceMode, CheckedLocalReadMode,
    CheckedLocalUseError, CheckedLocalUseSite, CheckedPlaceDisplacement,
    CheckedPlaceInitialization, CheckedSyntheticUse, CheckedSyntheticUseOwner, ExprId, LocalId,
};
use crate::final_analysis::completion::NormalContinuation;
use crate::record_field::CheckedRecordFieldSemanticId;
use crate::types::TypeKind;
use arcweft_core::effect::RuntimeRegistryReadKind;
use arcweft_id::{EffectId, LifetimeRegistryKey};
use arcweft_lang_syntax::ast::line_plan::DeferOutcome;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

mod registry;
pub(super) use registry::{RegistryAddress, RegistryDeferOwner, RegistryOwner, RegistrySite};
use registry::{RegistryFact, RegistryFacts, RegistryInterference, RegistryUnwind};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct MovePath {
    root: LocalId,
    fields: Box<[CheckedRecordFieldSemanticId]>,
}

impl From<&CheckedLocalAccess> for MovePath {
    fn from(access: &CheckedLocalAccess) -> Self {
        let fields = match access {
            CheckedLocalAccess::ValueTransfer(transfer) => transfer
                .fields()
                .iter()
                .map(|field| field.field())
                .collect(),
            CheckedLocalAccess::PlaceAccess(access) => access
                .place()
                .fields()
                .iter()
                .map(|field| field.field())
                .collect(),
        };
        Self {
            root: access.local(),
            fields,
        }
    }
}

impl MovePath {
    fn contains(&self, child: &Self) -> bool {
        self.root == child.root && child.fields.starts_with(&self.fields)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct NodeId(usize);

#[derive(Clone, Copy)]
pub(super) struct RegistryScheduleId(usize);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct RegistryScopeId(usize);

struct RegistryScope {
    parent: Option<RegistryScopeId>,
    depth: usize,
}

#[derive(Clone, Copy)]
pub(super) enum RegistryConcurrency {
    Sequential,
    Unknown(RegistrySite),
    Scheduled(RegistryScheduleId),
}

struct RegistrySchedule {
    parent: RegistryConcurrency,
    interference: Option<RegistryInterference>,
}

#[derive(Clone, Copy)]
pub(super) struct RegistryEventMark(usize);

#[derive(Clone)]
pub(super) struct Availability {
    pub(super) reachable: bool,
    // Unit denotes a normal continuation; Never denotes a divergent body.
    pub(super) result_type: NormalContinuation,
    frontier: BTreeSet<NodeId>,
}

impl Availability {
    fn root() -> Self {
        Self {
            reachable: true,
            result_type: NormalContinuation::Unit,
            frontier: BTreeSet::new(),
        }
    }

    pub(super) fn join(mut self, mut other: Self) -> Self {
        let result_type = self.result_type.join(other.result_type);
        if !self.reachable {
            other.result_type = result_type;
            return other;
        }
        if other.reachable {
            self.frontier.extend(other.frontier);
        }
        self.result_type = result_type;
        self
    }

    pub(super) fn terminate(&mut self) {
        self.reachable = false;
        self.result_type = NormalContinuation::Never;
        self.frontier.clear();
    }

    pub(super) fn keep_ownership_unreachable(&mut self) {
        self.reachable = false;
        self.frontier.clear();
    }

    pub(super) fn at(node: NodeId) -> Self {
        Self {
            reachable: true,
            result_type: NormalContinuation::Unit,
            frontier: BTreeSet::from([node]),
        }
    }
}

pub(super) enum Event {
    Join,
    Access(CheckedLocalUseSite),
    Synthetic(ExprId),
    Bind(Box<[LocalId]>),
    BindSynthetic(CheckedSyntheticUseOwner),
    RegistryWrite {
        address: RegistryAddress,
        ty: Arc<TypeKind>,
    },
    RegistryRead {
        owner: ExprId,
        address: RegistryAddress,
        ty: Arc<TypeKind>,
        kind: RuntimeRegistryReadKind,
        concurrency: RegistryConcurrency,
    },
    RegistryDrop {
        owner: ExprId,
        address: RegistryAddress,
        ty: Arc<TypeKind>,
        kind: RuntimeRegistryReadKind,
        concurrency: RegistryConcurrency,
    },
    ForgetRegistryWrites(RegistrySite, Box<[EffectId]>),
    DeferRegistryWrites {
        owner: RegistryDeferOwner,
        site: RegistrySite,
        outcome: DeferOutcome,
        effects: Box<[EffectId]>,
    },
    ScheduledRegistryWrites(RegistrySite, Option<Box<[EffectId]>>),
    EnterRegistry(RegistryOwner),
    ExitLineRegistry {
        line: ExprId,
        schedule: Option<RegistryScheduleId>,
    },
    RegistryBoundary(RegistrySite),
}

struct Node {
    event: Event,
    successors: BTreeSet<NodeId>,
    entry: bool,
    registry_scope: Option<RegistryScopeId>,
    registry_latent: bool,
}

#[derive(Default)]
pub(super) struct OwnershipFlow {
    nodes: Vec<Node>,
    registry_schedules: Vec<RegistrySchedule>,
    registry_scopes: Vec<RegistryScope>,
    registry_scope: Option<RegistryScopeId>,
    registry_latent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalInitialization {
    state: InitializationState,
    moves: BTreeSet<CheckedLocalUseSite>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InitializationState {
    Initialized,
    Uninitialized,
    MaybeInitialized,
}

impl InitializationState {
    fn checked(self) -> CheckedPlaceInitialization {
        match self {
            Self::Initialized => CheckedPlaceInitialization::Initialized,
            Self::Uninitialized => CheckedPlaceInitialization::Uninitialized,
            Self::MaybeInitialized => CheckedPlaceInitialization::Conditional,
        }
    }
    fn join(self, incoming: Self) -> Self {
        if self == incoming {
            self
        } else {
            Self::MaybeInitialized
        }
    }
}

impl Default for LocalInitialization {
    fn default() -> Self {
        Self {
            state: InitializationState::Initialized,
            moves: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Default, Eq, PartialEq)]
struct Initialization {
    locals: BTreeMap<MovePath, LocalInitialization>,
    synthetic: BTreeMap<CheckedSyntheticUseOwner, BTreeSet<ExprId>>,
    registry: RegistryFacts,
}

impl Initialization {
    fn transfer(
        &mut self,
        event: &Event,
        rows: &BTreeMap<CheckedLocalUseSite, CheckedLocalAccess>,
        synthetic: &BTreeMap<ExprId, CheckedSyntheticUse>,
        registry_keys: &BTreeMap<RegistryAddress, EffectId>,
        registry_schedules: &[RegistrySchedule],
    ) -> Result<(), CheckedLocalUseError> {
        match event {
            Event::Access(site) => {
                let access = rows
                    .get(site)
                    .ok_or(CheckedLocalUseError::InvalidTopology)?;
                let path = MovePath::from(access);
                match access {
                    CheckedLocalAccess::ValueTransfer(row)
                        if row.mode == CheckedLocalReadMode::Move =>
                    {
                        self.locals.retain(|child, _| !path.contains(child));
                        self.locals.insert(
                            path,
                            LocalInitialization {
                                state: InitializationState::Uninitialized,
                                moves: BTreeSet::from([*site]),
                            },
                        );
                    }
                    CheckedLocalAccess::PlaceAccess(access)
                        if access.mode() == CheckedLocalPlaceMode::Assign =>
                    {
                        self.locals.retain(|child, _| !path.contains(child));
                    }
                    _ => {}
                }
            }
            Event::Synthetic(expression) => {
                let row = synthetic
                    .get(expression)
                    .ok_or(CheckedLocalUseError::InvalidTopology)?;
                if row.mode == CheckedLocalReadMode::Move {
                    self.synthetic
                        .insert(row.owner, BTreeSet::from([*expression]));
                }
            }
            Event::Bind(locals) => {
                for local in locals {
                    self.locals.retain(|path, _| path.root != *local);
                }
            }
            Event::BindSynthetic(owner) => {
                self.synthetic.remove(owner);
            }
            Event::RegistryWrite { address, ty } => {
                if registry_keys.contains_key(address) {
                    self.registry.register(address, ty);
                }
            }
            Event::RegistryDrop { address, .. } => {
                if registry_keys.contains_key(address) {
                    self.registry.remove(address);
                }
            }
            Event::ForgetRegistryWrites(site, effects) => {
                self.registry
                    .invalidate_writes(*site, effects, registry_keys);
            }
            Event::DeferRegistryWrites {
                owner,
                site,
                outcome,
                effects,
            } => {
                self.registry
                    .defer_writes(*owner, *site, *outcome, effects, registry_keys);
            }
            Event::ExitLineRegistry { line, schedule } => {
                if let Some(schedule) = schedule {
                    registry_schedules
                        .get(schedule.0)
                        .and_then(|schedule| schedule.interference.as_ref())
                        .ok_or(CheckedLocalUseError::InvalidTopology)?
                        .apply_to_flow(&mut self.registry, registry_keys);
                }
                self.registry
                    .unwind(RegistryDeferOwner::Line(*line), RegistryUnwind::Any);
                self.registry.leave(RegistryOwner::Line(*line));
            }
            Event::EnterRegistry(owner) => self.registry.enter(*owner),
            Event::RegistryBoundary(site) => self.registry.forget(*site),
            Event::Join | Event::RegistryRead { .. } | Event::ScheduledRegistryWrites(..) => {}
        }

        Ok(())
    }

    fn join(&mut self, other: &Self) -> bool {
        let before = self.clone();
        let keys = self
            .locals
            .keys()
            .chain(other.locals.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        for local in keys {
            let left = self.locals.get(&local).cloned().unwrap_or_default();
            let right = other.locals.get(&local).cloned().unwrap_or_default();
            let target = self.locals.entry(local).or_default();
            target.state = left.state.join(right.state);
            target.moves.extend(right.moves);
        }
        for (owner, sites) in &other.synthetic {
            self.synthetic.entry(*owner).or_default().extend(sites);
        }
        self.registry.join(&other.registry);
        *self != before
    }
}

pub(super) enum Violation {
    RegistryRead {
        owner: ExprId,
        key: Arc<LifetimeRegistryKey>,
        kind: RuntimeRegistryReadKind,
    },
    Local {
        local: LocalId,
        site: CheckedLocalUseSite,
        repeated: bool,
    },
    Synthetic {
        owner: CheckedSyntheticUseOwner,
        expression: ExprId,
    },
}

pub(super) struct OwnershipFlowSolution {
    pub(super) violations: Vec<Violation>,
    pub(super) displacements: BTreeMap<CheckedLocalUseSite, CheckedPlaceDisplacement>,
}

impl OwnershipFlow {
    pub(super) fn push_registry_scope(&mut self) -> Option<RegistryScopeId> {
        let scope = RegistryScopeId(self.registry_scopes.len());
        self.registry_scopes.push(RegistryScope {
            parent: self.registry_scope,
            depth: self.registry_scope_depth(self.registry_scope) + 1,
        });
        self.replace_registry_scope(Some(scope))
    }

    pub(super) fn replace_registry_scope(
        &mut self,
        scope: Option<RegistryScopeId>,
    ) -> Option<RegistryScopeId> {
        std::mem::replace(&mut self.registry_scope, scope)
    }

    pub(super) const fn lexical_defer_owner(&self) -> RegistryDeferOwner {
        RegistryDeferOwner::Lexical(self.registry_scope)
    }

    pub(super) fn replace_registry_latency(&mut self, latent: bool) -> bool {
        std::mem::replace(&mut self.registry_latent, latent)
    }

    fn registry_scope_depth(&self, scope: Option<RegistryScopeId>) -> usize {
        scope.map_or(0, |scope| self.registry_scopes[scope.0].depth)
    }

    fn unwind_registry_scopes(
        &self,
        registry: &mut RegistryFacts,
        mut source: Option<RegistryScopeId>,
        mut target: Option<RegistryScopeId>,
    ) {
        while source != target {
            if self.registry_scope_depth(source) < self.registry_scope_depth(target) {
                target = target.and_then(|scope| self.registry_scopes[scope.0].parent);
            } else if let Some(scope) = source {
                registry.unwind(
                    RegistryDeferOwner::Lexical(Some(scope)),
                    RegistryUnwind::Completed,
                );
                source = self.registry_scopes[scope.0].parent;
            }
        }
    }

    pub(super) fn reserve_registry_schedule(
        &mut self,
        parent: RegistryConcurrency,
    ) -> RegistryScheduleId {
        let id = RegistryScheduleId(self.registry_schedules.len());
        self.registry_schedules.push(RegistrySchedule {
            parent,
            interference: None,
        });
        id
    }

    pub(super) fn mark_registry_events(&self) -> RegistryEventMark {
        RegistryEventMark(self.nodes.len())
    }

    pub(super) fn seal_registry_schedule(
        &mut self,
        id: RegistryScheduleId,
        eager: RegistryEventMark,
        scheduled: RegistryEventMark,
    ) -> Result<(), CheckedLocalUseError> {
        let mut interference = RegistryInterference::default();
        let eager_events = self
            .nodes
            .get(eager.0..scheduled.0)
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        let scheduled_events = self
            .nodes
            .get(scheduled.0..)
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        for node in eager_events.iter().filter(|node| !node.registry_latent) {
            if let Event::ScheduledRegistryWrites(site, effects) = &node.event {
                interference.opaque_writes(*site, effects.as_deref());
            }
        }
        for node in scheduled_events.iter().filter(|node| !node.registry_latent) {
            match &node.event {
                Event::RegistryWrite { address, ty } => interference.write(address, ty),
                Event::RegistryDrop { address, .. } => interference.remove(address),
                Event::ForgetRegistryWrites(site, effects)
                | Event::DeferRegistryWrites {
                    owner: RegistryDeferOwner::Lexical(_),
                    site,
                    effects,
                    ..
                } => {
                    interference.opaque_writes(*site, Some(effects));
                }
                Event::ScheduledRegistryWrites(site, effects) => {
                    interference.opaque_writes(*site, effects.as_deref());
                }
                Event::RegistryBoundary(site) => interference.opaque_writes(*site, None),
                _ => {}
            }
        }
        let slot = self
            .registry_schedules
            .get_mut(id.0)
            .filter(|slot| slot.interference.is_none())
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        slot.interference = Some(interference);
        Ok(())
    }

    fn registry_fact_at_read(
        &self,
        address: &RegistryAddress,
        required: &EffectId,
        mut concurrency: RegistryConcurrency,
        mut fact: RegistryFact,
    ) -> Result<RegistryFact, CheckedLocalUseError> {
        loop {
            match concurrency {
                RegistryConcurrency::Sequential => return Ok(fact),
                RegistryConcurrency::Unknown(site) => {
                    return Ok(if site.reaches(address) {
                        RegistryFact::Unknown
                    } else {
                        fact
                    });
                }
                RegistryConcurrency::Scheduled(id) => {
                    let schedule = self
                        .registry_schedules
                        .get(id.0)
                        .ok_or(CheckedLocalUseError::InvalidTopology)?;
                    let interference = schedule
                        .interference
                        .as_ref()
                        .ok_or(CheckedLocalUseError::InvalidTopology)?;
                    fact = interference.consider(address, required, fact);
                    concurrency = schedule.parent;
                }
            }
        }
    }

    /// Anchor the empty execution path before it can fork.
    pub(super) fn entry(&mut self) -> Availability {
        let mut state = Availability::root();
        self.append(&mut state, Event::Join);
        state
    }

    pub(super) fn registry_reads(&self) -> BTreeSet<ExprId> {
        self.nodes
            .iter()
            .filter_map(|node| match node.event {
                Event::RegistryRead { owner, .. } | Event::RegistryDrop { owner, .. } => {
                    Some(owner)
                }
                _ => None,
            })
            .collect()
    }

    pub(super) fn append(&mut self, state: &mut Availability, event: Event) -> NodeId {
        let id = NodeId(self.nodes.len());
        let entry = state.reachable && state.frontier.is_empty();
        self.nodes.push(Node {
            event,
            successors: BTreeSet::new(),
            entry,
            registry_scope: self.registry_scope,
            registry_latent: self.registry_latent,
        });
        self.connect(state, id);
        if state.reachable {
            state.frontier = BTreeSet::from([id]);
        }
        id
    }

    pub(super) fn detached(&mut self) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(Node {
            event: Event::Join,
            successors: BTreeSet::new(),
            entry: false,
            registry_scope: self.registry_scope,
            registry_latent: self.registry_latent,
        });
        id
    }

    pub(super) fn connect(&mut self, state: &Availability, target: NodeId) {
        if state.reachable {
            for predecessor in &state.frontier {
                self.nodes[predecessor.0].successors.insert(target);
            }
        }
    }

    pub(super) fn solve(
        &self,
        rows: &BTreeMap<CheckedLocalUseSite, CheckedLocalAccess>,
        synthetic: &BTreeMap<ExprId, CheckedSyntheticUse>,
    ) -> Result<OwnershipFlowSolution, CheckedLocalUseError> {
        let registry_keys =
            self.nodes
                .iter()
                .filter_map(|node| {
                    let address = match &node.event {
                        Event::RegistryRead { address, .. }
                        | Event::RegistryDrop { address, .. } => address,
                        _ => return None,
                    };
                    Some((address.clone(), EffectId::state_write(address.key())))
                })
                .collect::<BTreeMap<_, _>>();
        let mut incoming = vec![None; self.nodes.len()];
        let mut queue = VecDeque::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if node.entry {
                incoming[index] = Some(Initialization::default());
                queue.push_back(NodeId(index));
            }
        }
        while let Some(id) = queue.pop_front() {
            let mut state = incoming[id.0]
                .clone()
                .ok_or(CheckedLocalUseError::InvalidTopology)?;
            state.transfer(
                &self.nodes[id.0].event,
                rows,
                synthetic,
                &registry_keys,
                &self.registry_schedules,
            )?;
            for successor in &self.nodes[id.0].successors {
                let source_scope = self.nodes[id.0].registry_scope;
                let target_scope = self.nodes[successor.0].registry_scope;
                let mut unwound;
                let state = if source_scope != target_scope && state.registry.has_lexical_defers() {
                    unwound = state.clone();
                    self.unwind_registry_scopes(&mut unwound.registry, source_scope, target_scope);
                    &unwound
                } else {
                    &state
                };
                let changed = match &mut incoming[successor.0] {
                    Some(previous) => previous.join(state),
                    target @ None => {
                        *target = Some(state.clone());
                        true
                    }
                };
                if changed {
                    queue.push_back(*successor);
                }
            }
        }
        let mut violations = Vec::new();
        let mut displacements = BTreeMap::new();
        for (node, state) in self.nodes.iter().zip(incoming) {
            let Some(state) = state else {
                if let Event::Access(site) = node.event
                    && rows
                        .get(&site)
                        .and_then(CheckedLocalAccess::place_access)
                        .is_some_and(|access| access.mode() == CheckedLocalPlaceMode::Assign)
                {
                    displacements.insert(site, CheckedPlaceDisplacement::Unreachable);
                }
                continue;
            };
            match &node.event {
                Event::RegistryRead {
                    owner,
                    address,
                    ty,
                    kind,
                    concurrency,
                }
                | Event::RegistryDrop {
                    owner,
                    address,
                    ty,
                    kind,
                    concurrency,
                } => {
                    let fact = self.registry_fact_at_read(
                        address,
                        &registry_keys[address],
                        *concurrency,
                        state.registry.fact(address).clone(),
                    )?;
                    let admitted = if matches!(node.event, Event::RegistryDrop { .. }) {
                        fact.admits_drop(*kind, ty)
                    } else {
                        fact.admits(*kind, ty)
                    };
                    if !admitted {
                        violations.push(Violation::RegistryRead {
                            owner: *owner,
                            key: Arc::clone(address.key()),
                            kind: *kind,
                        });
                    }
                }
                Event::Access(site) => {
                    let site = *site;
                    let row = rows
                        .get(&site)
                        .ok_or(CheckedLocalUseError::InvalidTopology)?;
                    let assignment = row
                        .place_access()
                        .is_some_and(|access| access.mode() == CheckedLocalPlaceMode::Assign);
                    let path = MovePath::from(row);
                    if assignment {
                        let initialization = state
                            .locals
                            .get(&path)
                            .cloned()
                            .unwrap_or_default()
                            .state
                            .checked();
                        let mut fields = Vec::new();
                        for (changed, fact) in &state.locals {
                            if changed.fields.len() <= path.fields.len()
                                || !path.contains(changed)
                                || fact.state == InitializationState::Initialized
                            {
                                continue;
                            }
                            let evidence = fact
                                .moves
                                .iter()
                                .find_map(|site| {
                                    let access = rows.get(site)?;
                                    (MovePath::from(access) == *changed)
                                        .then(|| access.value_transfer())
                                        .flatten()
                                })
                                .ok_or(CheckedLocalUseError::InvalidTopology)?;
                            fields.push(CheckedDisplacedField::new(
                                evidence.fields()[path.fields.len()..]
                                    .to_vec()
                                    .into_boxed_slice(),
                                fact.state.checked(),
                            ));
                        }
                        if displacements
                            .insert(
                                site,
                                CheckedPlaceDisplacement::Reachable {
                                    initialization,
                                    fields: fields.into_boxed_slice(),
                                },
                            )
                            .is_some()
                        {
                            return Err(CheckedLocalUseError::InvalidTopology);
                        }
                    }
                    let unavailable = state.locals.iter().filter(|(changed, fact)| {
                        fact.state != InitializationState::Initialized
                            && if assignment {
                                changed.fields.len() < path.fields.len() && changed.contains(&path)
                            } else {
                                changed.contains(&path) || path.contains(changed)
                            }
                    });
                    for (_, fact) in unavailable {
                        violations.push(Violation::Local {
                            local: row.local(),
                            site,
                            repeated: fact.moves.contains(&site),
                        });
                    }
                }
                Event::Synthetic(expression) => {
                    let expression = *expression;
                    let row = synthetic
                        .get(&expression)
                        .ok_or(CheckedLocalUseError::InvalidTopology)?;
                    if state.synthetic.contains_key(&row.owner) {
                        violations.push(Violation::Synthetic {
                            owner: row.owner,
                            expression,
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(OwnershipFlowSolution {
            violations,
            displacements,
        })
    }
}
