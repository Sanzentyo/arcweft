use crate::reachability::{
    AvailabilityDomain, ContentUnitId, LinkGraph, LinkNode, LinkNodeId, ReachabilityReport,
};
use std::collections::{BTreeMap, BTreeSet};

/// Placement decision for a reachable linker graph node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentPlacementDecision {
    Startup,
    Unit(ContentUnitId),
    Shared(Vec<ContentUnitId>),
    Omitted,
}

/// Deterministic partitioning options.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContentPartitionOptions {
    pub shared_hoist_threshold_bytes: u64,
}

/// Reachability and placement decisions for one linked graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentPartitionPlan {
    decisions: BTreeMap<LinkNodeId, ContentPlacementDecision>,
    reachability: ReachabilityReport,
}

impl ContentPartitionPlan {
    pub fn decision(&self, node: LinkNodeId) -> Option<&ContentPlacementDecision> {
        self.decisions.get(&node)
    }

    pub fn decisions(&self) -> &BTreeMap<LinkNodeId, ContentPlacementDecision> {
        &self.decisions
    }

    pub const fn reachability(&self) -> &ReachabilityReport {
        &self.reachability
    }
}

/// Partitions a link graph into startup and on-demand content domains.
pub fn partition_content(
    graph: &LinkGraph,
    entrypoints: impl IntoIterator<Item = LinkNodeId>,
    options: ContentPartitionOptions,
) -> ContentPartitionPlan {
    let reachability = graph.reachability(entrypoints);
    let decisions = graph
        .nodes()
        .map(|node| {
            (
                node.id(),
                decide_node(node, reachability.domains(node.id()), options),
            )
        })
        .collect();
    ContentPartitionPlan {
        decisions,
        reachability,
    }
}

fn decide_node(
    node: &LinkNode,
    domains: Option<&BTreeSet<AvailabilityDomain>>,
    options: ContentPartitionOptions,
) -> ContentPlacementDecision {
    let Some(domains) = domains else {
        return ContentPlacementDecision::Omitted;
    };
    if domains.contains(&AvailabilityDomain::Startup) || node.kind().is_executable() {
        return ContentPlacementDecision::Startup;
    }
    let units = domains
        .iter()
        .filter_map(|domain| match domain {
            AvailabilityDomain::Startup => None,
            AvailabilityDomain::OnDemand(unit) => Some(unit.clone()),
        })
        .collect::<Vec<_>>();
    match units.as_slice() {
        [] => ContentPlacementDecision::Omitted,
        [unit] => ContentPlacementDecision::Unit(unit.clone()),
        _ if node.decoded_size() <= options.shared_hoist_threshold_bytes => {
            ContentPlacementDecision::Startup
        }
        _ => ContentPlacementDecision::Shared(units),
    }
}

#[cfg(test)]
mod tests {
    use super::{ContentPartitionOptions, ContentPlacementDecision, partition_content};
    use crate::reachability::{
        ContentUnitId, FiniteRefSet, LinkEdge, LinkEdgeKind, LinkGraph, LinkNode, LinkNodeId,
        LinkNodeKind,
    };

    fn node(id: u32, kind: LinkNodeKind, size: u64) -> LinkNode {
        LinkNode::new(LinkNodeId::new(id), kind, format!("node.{id}"), size)
    }

    #[test]
    fn unreachable_nodes_are_omitted_and_ensure_partitions_content() {
        let chapter = ContentUnitId::new("content.chapter_two");
        let graph = LinkGraph::new(
            [
                node(1, LinkNodeKind::Entrypoint, 0),
                node(2, LinkNodeKind::Flow, 0),
                node(3, LinkNodeKind::Asset, 100),
                node(4, LinkNodeKind::Asset, 100),
            ],
            [
                LinkEdge::new(LinkNodeId::new(1), LinkNodeId::new(2), LinkEdgeKind::Normal),
                LinkEdge::new(
                    LinkNodeId::new(2),
                    LinkNodeId::new(3),
                    LinkEdgeKind::Ensure(chapter.clone()),
                ),
            ],
        );
        let plan = partition_content(
            &graph,
            [LinkNodeId::new(1)],
            ContentPartitionOptions::default(),
        );

        assert_eq!(
            plan.decision(LinkNodeId::new(3)),
            Some(&ContentPlacementDecision::Unit(chapter))
        );
        assert_eq!(
            plan.decision(LinkNodeId::new(4)),
            Some(&ContentPlacementDecision::Omitted)
        );
    }

    #[test]
    fn startup_reachability_wins_over_on_demand() {
        let unit = ContentUnitId::new("content.optional");
        let graph = LinkGraph::new(
            [
                node(1, LinkNodeKind::Entrypoint, 0),
                node(2, LinkNodeKind::Flow, 0),
                node(3, LinkNodeKind::Asset, 100),
            ],
            [
                LinkEdge::new(LinkNodeId::new(1), LinkNodeId::new(3), LinkEdgeKind::Normal),
                LinkEdge::new(LinkNodeId::new(1), LinkNodeId::new(2), LinkEdgeKind::Normal),
                LinkEdge::new(
                    LinkNodeId::new(2),
                    LinkNodeId::new(3),
                    LinkEdgeKind::Ensure(unit),
                ),
            ],
        );
        let plan = partition_content(
            &graph,
            [LinkNodeId::new(1)],
            ContentPartitionOptions::default(),
        );

        assert_eq!(
            plan.decision(LinkNodeId::new(3)),
            Some(&ContentPlacementDecision::Startup)
        );
    }

    #[test]
    fn finite_dynamic_sets_retain_all_members_in_current_domain() {
        let graph = LinkGraph::with_finite_ref_sets(
            [
                node(1, LinkNodeKind::Entrypoint, 0),
                node(2, LinkNodeKind::Function, 0),
                node(3, LinkNodeKind::Asset, 100),
                node(4, LinkNodeKind::Asset, 100),
            ],
            [LinkEdge::new(
                LinkNodeId::new(1),
                LinkNodeId::new(2),
                LinkEdgeKind::Normal,
            )],
            [FiniteRefSet::new(
                LinkNodeId::new(2),
                [LinkNodeId::new(3), LinkNodeId::new(4)],
            )],
        );
        let plan = partition_content(
            &graph,
            [LinkNodeId::new(1)],
            ContentPartitionOptions::default(),
        );

        assert_eq!(
            plan.decision(LinkNodeId::new(3)),
            Some(&ContentPlacementDecision::Startup)
        );
        assert_eq!(
            plan.decision(LinkNodeId::new(4)),
            Some(&ContentPlacementDecision::Startup)
        );
    }
}
