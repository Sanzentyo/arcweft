use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Stable node identifier in the linker graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LinkNodeId(u32);

/// Stable content unit identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentUnitId(String);

/// Link graph node family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkNodeKind {
    Entrypoint,
    Flow,
    Function,
    RuntimeType,
    DisplayObject,
    Asset,
    AdapterRequirement,
    HostCall,
}

/// Edge family used by reachability and partitioning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinkEdgeKind {
    Normal,
    Ensure(ContentUnitId),
    DynamicSet,
}

/// Availability domain reached during graph traversal.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AvailabilityDomain {
    Startup,
    OnDemand(ContentUnitId),
}

/// One graph node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkNode {
    id: LinkNodeId,
    kind: LinkNodeKind,
    stable_name: String,
    decoded_size: u64,
}

/// Directed graph edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkEdge {
    from: LinkNodeId,
    to: LinkNodeId,
    kind: LinkEdgeKind,
}

/// Linker-internal finite closure for dynamic references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FiniteRefSet {
    owner: LinkNodeId,
    members: Vec<LinkNodeId>,
}

/// Whole-program graph used by linker and content partitioning.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LinkGraph {
    nodes: BTreeMap<LinkNodeId, LinkNode>,
    edges: Vec<LinkEdge>,
}

/// Reachability domains per node.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReachabilityReport {
    domains: BTreeMap<LinkNodeId, BTreeSet<AvailabilityDomain>>,
}

impl LinkNodeId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl ContentUnitId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl LinkNodeKind {
    pub const fn is_executable(self) -> bool {
        matches!(self, Self::Entrypoint | Self::Flow | Self::Function)
    }
}

impl LinkNode {
    pub fn new(
        id: LinkNodeId,
        kind: LinkNodeKind,
        stable_name: impl Into<String>,
        decoded_size: u64,
    ) -> Self {
        Self {
            id,
            kind,
            stable_name: stable_name.into(),
            decoded_size,
        }
    }

    pub const fn id(&self) -> LinkNodeId {
        self.id
    }

    pub const fn kind(&self) -> LinkNodeKind {
        self.kind
    }

    pub fn stable_name(&self) -> &str {
        &self.stable_name
    }

    pub const fn decoded_size(&self) -> u64 {
        self.decoded_size
    }
}

impl LinkEdge {
    pub const fn new(from: LinkNodeId, to: LinkNodeId, kind: LinkEdgeKind) -> Self {
        Self { from, to, kind }
    }

    pub const fn from(&self) -> LinkNodeId {
        self.from
    }

    pub const fn to(&self) -> LinkNodeId {
        self.to
    }

    pub const fn kind(&self) -> &LinkEdgeKind {
        &self.kind
    }
}

impl FiniteRefSet {
    pub fn new(owner: LinkNodeId, members: impl IntoIterator<Item = LinkNodeId>) -> Self {
        let mut members = members.into_iter().collect::<Vec<_>>();
        members.sort();
        members.dedup();
        Self { owner, members }
    }

    pub const fn owner(&self) -> LinkNodeId {
        self.owner
    }

    pub fn members(&self) -> &[LinkNodeId] {
        &self.members
    }

    fn into_edges(self) -> impl Iterator<Item = LinkEdge> {
        let owner = self.owner;
        self.members
            .into_iter()
            .map(move |member| LinkEdge::new(owner, member, LinkEdgeKind::DynamicSet))
    }
}

impl LinkGraph {
    pub fn new(
        nodes: impl IntoIterator<Item = LinkNode>,
        edges: impl IntoIterator<Item = LinkEdge>,
    ) -> Self {
        Self::with_finite_ref_sets(nodes, edges, [])
    }

    pub fn with_finite_ref_sets(
        nodes: impl IntoIterator<Item = LinkNode>,
        edges: impl IntoIterator<Item = LinkEdge>,
        finite_ref_sets: impl IntoIterator<Item = FiniteRefSet>,
    ) -> Self {
        let nodes = nodes
            .into_iter()
            .map(|node| (node.id(), node))
            .collect::<BTreeMap<_, _>>();
        let mut edges = edges.into_iter().collect::<Vec<_>>();
        edges.extend(
            finite_ref_sets
                .into_iter()
                .flat_map(FiniteRefSet::into_edges),
        );
        edges.sort_by_key(|edge| (edge.from(), edge.to(), edge_kind_rank(edge.kind())));
        Self { nodes, edges }
    }

    pub fn nodes(&self) -> impl Iterator<Item = &LinkNode> {
        self.nodes.values()
    }

    pub fn node(&self, id: LinkNodeId) -> Option<&LinkNode> {
        self.nodes.get(&id)
    }

    pub fn edges(&self) -> &[LinkEdge] {
        &self.edges
    }

    pub fn reachability(
        &self,
        entrypoints: impl IntoIterator<Item = LinkNodeId>,
    ) -> ReachabilityReport {
        let mut outgoing: BTreeMap<LinkNodeId, Vec<&LinkEdge>> = BTreeMap::new();
        for edge in &self.edges {
            outgoing.entry(edge.from()).or_default().push(edge);
        }
        let mut domains: BTreeMap<LinkNodeId, BTreeSet<AvailabilityDomain>> = BTreeMap::new();
        let mut queue = VecDeque::new();
        let mut roots = entrypoints.into_iter().collect::<Vec<_>>();
        roots.sort();
        roots.dedup();
        for root in roots {
            queue.push_back((root, AvailabilityDomain::Startup));
        }
        while let Some((node, domain)) = queue.pop_front() {
            if !self.nodes.contains_key(&node) {
                continue;
            }
            if !domains.entry(node).or_default().insert(domain.clone()) {
                continue;
            }
            for edge in outgoing.get(&node).into_iter().flatten() {
                let next = match edge.kind() {
                    LinkEdgeKind::Ensure(unit) => AvailabilityDomain::OnDemand(unit.clone()),
                    LinkEdgeKind::Normal | LinkEdgeKind::DynamicSet => domain.clone(),
                };
                queue.push_back((edge.to(), next));
            }
        }
        ReachabilityReport { domains }
    }
}

impl ReachabilityReport {
    pub fn domains(&self, node: LinkNodeId) -> Option<&BTreeSet<AvailabilityDomain>> {
        self.domains.get(&node)
    }
}

const fn edge_kind_rank(kind: &LinkEdgeKind) -> u8 {
    match kind {
        LinkEdgeKind::Normal => 0,
        LinkEdgeKind::Ensure(_) => 1,
        LinkEdgeKind::DynamicSet => 2,
    }
}
