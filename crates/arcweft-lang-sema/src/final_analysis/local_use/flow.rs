//! Temporary ownership control flow emitted by the selected evaluation fold.
//! Only typed events are replayed; source occurrences and certificates are
//! created once. This graph is discarded before the local-use seal publishes.

use super::{
    CheckedDisplacedField, CheckedLocalAccess, CheckedLocalPlaceMode, CheckedLocalReadMode,
    CheckedLocalUseError, CheckedLocalUseSite, CheckedPlaceDisplacement,
    CheckedPlaceInitialization, CheckedSyntheticUse, CheckedSyntheticUseOwner, ExprId, LocalId,
};
use crate::record_field::CheckedRecordFieldSemanticId;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

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
                .nominal_field()
                .map(|field| vec![field.field().field()].into_boxed_slice())
                .unwrap_or_default(),
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

#[derive(Clone, Default)]
pub(super) struct Availability {
    pub(super) reachable: bool,
    frontier: BTreeSet<NodeId>,
}

impl Availability {
    pub(super) fn root() -> Self {
        Self {
            reachable: true,
            frontier: BTreeSet::new(),
        }
    }

    pub(super) fn join(mut self, other: Self) -> Self {
        if !self.reachable {
            return other;
        }
        if other.reachable {
            self.frontier.extend(other.frontier);
        }
        self
    }

    pub(super) fn terminate(&mut self) {
        self.reachable = false;
        self.frontier.clear();
    }

    pub(super) fn at(node: NodeId) -> Self {
        Self {
            reachable: true,
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
}

struct Node {
    event: Event,
    successors: BTreeSet<NodeId>,
    entry: bool,
}

#[derive(Default)]
pub(super) struct OwnershipFlow {
    nodes: Vec<Node>,
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
}

impl Initialization {
    fn transfer(
        &mut self,
        event: &Event,
        rows: &BTreeMap<CheckedLocalUseSite, CheckedLocalAccess>,
        synthetic: &BTreeMap<ExprId, CheckedSyntheticUse>,
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
            Event::Join => {}
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
        *self != before
    }
}

pub(super) enum Violation {
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
    pub(super) fn append(&mut self, state: &mut Availability, event: Event) -> NodeId {
        let id = NodeId(self.nodes.len());
        let entry = state.reachable && state.frontier.is_empty();
        self.nodes.push(Node {
            event,
            successors: BTreeSet::new(),
            entry,
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
            state.transfer(&self.nodes[id.0].event, rows, synthetic)?;
            for successor in &self.nodes[id.0].successors {
                let changed = match &mut incoming[successor.0] {
                    Some(previous) => previous.join(&state),
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
            match node.event {
                Event::Access(site) => {
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
