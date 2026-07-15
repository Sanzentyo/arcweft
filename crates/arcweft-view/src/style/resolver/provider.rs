//! Bounded retained dependency index for logical-axis providers.

use super::{
    ComputedViewAxes, ViewAxisProviderParticipation, ViewStyleNodeKey, ViewStyleResolveContext,
    ViewStyleResolveError,
};
use crate::ViewMountId;
use crate::style::{
    ViewBoxAxisMode, ViewBoxAxisRevision, ViewBoxAxisSeedSource, ViewInheritedBoxAxes,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Debug, Eq, PartialEq)]
struct ViewAxisProviderRecord {
    parent: Option<ViewStyleNodeKey>,
    inherited: ViewInheritedBoxAxes,
    effective_mode: ViewBoxAxisMode,
    effective_revision: ViewBoxAxisRevision,
    local_barrier: bool,
    ancestor_invalidated: bool,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ViewAxisProviderIndex {
    records: BTreeMap<ViewStyleNodeKey, ViewAxisProviderRecord>,
    children: BTreeMap<ViewStyleNodeKey, BTreeSet<ViewStyleNodeKey>>,
}

#[derive(Clone, Debug)]
pub(super) struct ViewAxisProviderUpdatePlan {
    node: ViewStyleNodeKey,
    previous_parent: Option<ViewStyleNodeKey>,
    proposed: ViewAxisProviderRecord,
    provider_changed: bool,
    visited: Vec<ViewStyleNodeKey>,
    mark_invalidated: Vec<ViewStyleNodeKey>,
}

impl ViewAxisProviderIndex {
    pub(super) fn clear(&mut self) {
        self.records.clear();
        self.children.clear();
    }

    pub(super) fn prepare(
        &self,
        context: &ViewStyleResolveContext<'_>,
        axes: &ComputedViewAxes,
        local_barrier: bool,
        max_invalidation_nodes: usize,
    ) -> Result<Option<ViewAxisProviderUpdatePlan>, ViewStyleResolveError> {
        validate_context_shape(context)?;
        if context.axis_provider_participation == ViewAxisProviderParticipation::ProjectionOnly {
            return Ok(None);
        }

        let parent = context.parent_node_key.cloned();
        if let Some(parent_key) = parent.as_ref() {
            let parent_record = self.records.get(parent_key).ok_or_else(|| {
                ViewStyleResolveError::AxisProviderMissingParent {
                    node: context.node_key.clone(),
                    parent: parent_key.clone(),
                }
            })?;
            validate_parent_record(
                context.node_key,
                parent_key,
                context.inherited_axes,
                parent_record,
            )?;
            self.validate_ancestor_chain(context.node_key, parent_key)?;
        }

        let proposed = ViewAxisProviderRecord {
            parent,
            inherited: context.inherited_axes,
            effective_mode: axes.mode(),
            effective_revision: axes.revision(),
            local_barrier,
            ancestor_invalidated: false,
        };
        let previous = self.records.get(context.node_key);
        let provider_changed = previous.is_none_or(|record| {
            record.effective_mode != proposed.effective_mode
                || record.effective_revision != proposed.effective_revision
                || record.local_barrier != proposed.local_barrier
        });
        let mut visited = Vec::new();
        let mut mark_invalidated = Vec::new();
        if provider_changed && previous.is_none_or(|record| !record.ancestor_invalidated) {
            self.collect_descendants(
                context.node_key,
                max_invalidation_nodes,
                &mut visited,
                &mut mark_invalidated,
            )?;
        }
        Ok(Some(ViewAxisProviderUpdatePlan {
            node: context.node_key.clone(),
            previous_parent: previous.and_then(|record| record.parent.clone()),
            proposed,
            provider_changed,
            visited,
            mark_invalidated,
        }))
    }

    pub(super) fn commit(&mut self, plan: ViewAxisProviderUpdatePlan) {
        if plan.previous_parent != plan.proposed.parent {
            if let Some(previous_parent) = plan.previous_parent.as_ref()
                && let Some(children) = self.children.get_mut(previous_parent)
            {
                children.remove(&plan.node);
                if children.is_empty() {
                    self.children.remove(previous_parent);
                }
            }
            if let Some(parent) = plan.proposed.parent.as_ref() {
                self.children
                    .entry(parent.clone())
                    .or_default()
                    .insert(plan.node.clone());
            }
        }
        self.records.insert(plan.node, plan.proposed);
        for node in plan.mark_invalidated {
            if let Some(record) = self.records.get_mut(&node) {
                record.ancestor_invalidated = true;
            }
        }
    }

    pub(super) fn invalidate_mount(&mut self, mount: ViewMountId) -> usize {
        let removed: BTreeSet<_> = self
            .records
            .keys()
            .filter(|node| node.mount() == mount)
            .cloned()
            .collect();
        if removed.is_empty() {
            return 0;
        }
        self.records.retain(|node, _| !removed.contains(node));
        self.children.retain(|parent, children| {
            if removed.contains(parent) {
                return false;
            }
            children.retain(|child| !removed.contains(child));
            !children.is_empty()
        });
        removed.len()
    }

    fn validate_ancestor_chain(
        &self,
        node: &ViewStyleNodeKey,
        proposed_parent: &ViewStyleNodeKey,
    ) -> Result<(), ViewStyleResolveError> {
        let mut cursor = Some(proposed_parent);
        let mut visited = BTreeSet::new();
        while let Some(parent) = cursor {
            if parent == node || !visited.insert(parent.clone()) {
                return Err(ViewStyleResolveError::AxisProviderCycle {
                    node: node.clone(),
                    parent: parent.clone(),
                });
            }
            let record = self.records.get(parent).ok_or_else(|| {
                ViewStyleResolveError::AxisProviderMissingParent {
                    node: node.clone(),
                    parent: parent.clone(),
                }
            })?;
            cursor = record.parent.as_ref();
        }
        Ok(())
    }

    fn collect_descendants(
        &self,
        node: &ViewStyleNodeKey,
        limit: usize,
        visited: &mut Vec<ViewStyleNodeKey>,
        mark_invalidated: &mut Vec<ViewStyleNodeKey>,
    ) -> Result<(), ViewStyleResolveError> {
        let mut queue = VecDeque::new();
        if let Some(children) = self.children.get(node) {
            queue.extend(children.iter().cloned().map(|child| (node.clone(), child)));
        }
        while let Some((indexed_parent, child)) = queue.pop_front() {
            if visited.len() == limit {
                return Err(ViewStyleResolveError::AxisProviderInvalidationBudget {
                    node: node.clone(),
                    limit,
                });
            }
            let record = self.records.get(&child).ok_or_else(|| {
                ViewStyleResolveError::AxisProviderCorruptChildIndex {
                    parent: indexed_parent.clone(),
                    child: child.clone(),
                }
            })?;
            if record.parent.as_ref() != Some(&indexed_parent) {
                return Err(ViewStyleResolveError::AxisProviderCorruptChildIndex {
                    parent: indexed_parent,
                    child,
                });
            }
            visited.push(child.clone());
            if record.local_barrier {
                continue;
            }
            mark_invalidated.push(child.clone());
            if let Some(children) = self.children.get(&child) {
                queue.extend(
                    children
                        .iter()
                        .cloned()
                        .map(|descendant| (child.clone(), descendant)),
                );
            }
        }
        Ok(())
    }
}

impl ViewAxisProviderUpdatePlan {
    pub(super) const fn provider_changed(&self) -> bool {
        self.provider_changed
    }

    pub(super) fn invalidated_nodes(&self) -> impl Iterator<Item = &ViewStyleNodeKey> {
        std::iter::once(&self.node).chain(self.visited.iter())
    }
}

fn validate_context_shape(
    context: &ViewStyleResolveContext<'_>,
) -> Result<(), ViewStyleResolveError> {
    match (context.parent, context.parent_node_key) {
        (None, None) => match context.inherited_axes.source() {
            ViewBoxAxisSeedSource::HostDefault | ViewBoxAxisSeedSource::HostExplicit => Ok(()),
            seed_source @ ViewBoxAxisSeedSource::Parent => {
                Err(ViewStyleResolveError::AxisProviderInvalidRootSeed {
                    node: context.node_key.clone(),
                    seed_source,
                })
            }
        },
        (Some(parent), Some(parent_key)) => {
            if context.inherited_axes.source() != ViewBoxAxisSeedSource::Parent {
                return Err(ViewStyleResolveError::AxisProviderInvalidChildSeed {
                    node: context.node_key.clone(),
                    seed_source: context.inherited_axes.source(),
                });
            }
            validate_parent_axes(
                context.node_key,
                parent_key,
                context.inherited_axes,
                parent.axes(),
            )
        }
        _ => Err(ViewStyleResolveError::AxisProviderParentShape {
            node: context.node_key.clone(),
        }),
    }
}

fn validate_parent_record(
    node: &ViewStyleNodeKey,
    parent: &ViewStyleNodeKey,
    inherited: ViewInheritedBoxAxes,
    record: &ViewAxisProviderRecord,
) -> Result<(), ViewStyleResolveError> {
    if inherited.mode() != record.effective_mode {
        return Err(ViewStyleResolveError::AxisProviderModeMismatch {
            node: node.clone(),
            parent: parent.clone(),
            expected: record.effective_mode,
            actual: inherited.mode(),
        });
    }
    if inherited.revision() != record.effective_revision {
        return Err(ViewStyleResolveError::AxisProviderRevisionMismatch {
            node: node.clone(),
            parent: parent.clone(),
            expected: record.effective_revision,
            actual: inherited.revision(),
        });
    }
    Ok(())
}

fn validate_parent_axes(
    node: &ViewStyleNodeKey,
    parent: &ViewStyleNodeKey,
    inherited: ViewInheritedBoxAxes,
    parent_axes: &ComputedViewAxes,
) -> Result<(), ViewStyleResolveError> {
    if inherited.mode() != parent_axes.mode() {
        return Err(ViewStyleResolveError::AxisProviderModeMismatch {
            node: node.clone(),
            parent: parent.clone(),
            expected: parent_axes.mode(),
            actual: inherited.mode(),
        });
    }
    if inherited.revision() != parent_axes.revision() {
        return Err(ViewStyleResolveError::AxisProviderRevisionMismatch {
            node: node.clone(),
            parent: parent.clone(),
            expected: parent_axes.revision(),
            actual: inherited.revision(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{
        ComputedViewStyleBuilder, ComputedViewStyleRevision, ViewBoxAxisHostSeed,
        ViewBoxAxisSeedGeneration, ViewStyleNodeFacts, ViewStyleRevisionSet, ViewStyleTraceMode,
    };
    use arcweft_presentation::appearance::PresentationEnvironment;

    fn host_seed(mount: ViewMountId, generation: u64) -> ViewInheritedBoxAxes {
        let mut generation_value = ViewBoxAxisSeedGeneration::INITIAL;
        for _ in 0..generation {
            generation_value = generation_value.checked_next().unwrap();
        }
        ViewInheritedBoxAxes::for_host_seed(mount, generation_value, ViewBoxAxisHostSeed::Default)
    }

    fn record(
        parent: Option<ViewStyleNodeKey>,
        inherited: ViewInheritedBoxAxes,
        local_barrier: bool,
    ) -> ViewAxisProviderRecord {
        ViewAxisProviderRecord {
            parent,
            inherited,
            effective_mode: inherited.mode(),
            effective_revision: inherited.revision(),
            local_barrier,
            ancestor_invalidated: false,
        }
    }

    #[test]
    fn corrupt_child_index_is_a_typed_error_without_partial_output() {
        let parent = ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 0);
        let child = ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 1);
        let mut index = ViewAxisProviderIndex::default();
        index
            .children
            .insert(parent.clone(), BTreeSet::from([child.clone()]));
        let mut visited = Vec::new();
        let mut marked = Vec::new();

        assert_eq!(
            index.collect_descendants(&parent, 1, &mut visited, &mut marked),
            Err(ViewStyleResolveError::AxisProviderCorruptChildIndex {
                parent: parent.clone(),
                child: child.clone(),
            })
        );
        assert!(visited.is_empty());
        assert!(marked.is_empty());

        let unrelated = ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 2);
        index.records.insert(
            child.clone(),
            record(
                Some(unrelated),
                ViewInheritedBoxAxes::from_parent(
                    ViewBoxAxisMode::HorizontalLtr,
                    ViewBoxAxisRevision::from_raw(1),
                ),
                false,
            ),
        );
        assert_eq!(
            index.collect_descendants(&parent, 1, &mut visited, &mut marked),
            Err(ViewStyleResolveError::AxisProviderCorruptChildIndex { parent, child })
        );
        assert!(visited.is_empty());
        assert!(marked.is_empty());
    }

    #[test]
    fn descendant_walk_is_breadth_first_sorted_bounded_and_barrier_aware() {
        let mount = ViewMountId::from_raw(7);
        let root = ViewStyleNodeKey::new(mount, Vec::new(), 0);
        let first = ViewStyleNodeKey::new(mount, Vec::new(), 1);
        let second = ViewStyleNodeKey::new(mount, Vec::new(), 2);
        let grandchild = ViewStyleNodeKey::new(mount, Vec::new(), 3);
        let below_barrier = ViewStyleNodeKey::new(mount, Vec::new(), 4);
        let root_axes = host_seed(mount, 0);
        let inherited = ViewInheritedBoxAxes::from_parent(root_axes.mode(), root_axes.revision());
        let mut index = ViewAxisProviderIndex::default();
        index
            .records
            .insert(root.clone(), record(None, root_axes, false));
        index
            .records
            .insert(first.clone(), record(Some(root.clone()), inherited, false));
        index
            .records
            .insert(second.clone(), record(Some(root.clone()), inherited, true));
        index.records.insert(
            grandchild.clone(),
            record(Some(first.clone()), inherited, false),
        );
        index.records.insert(
            below_barrier.clone(),
            record(Some(second.clone()), inherited, false),
        );
        index.children.insert(
            root.clone(),
            BTreeSet::from([second.clone(), first.clone()]),
        );
        index
            .children
            .insert(first.clone(), BTreeSet::from([grandchild.clone()]));
        index
            .children
            .insert(second.clone(), BTreeSet::from([below_barrier]));

        let mut visited = Vec::new();
        let mut marked = Vec::new();
        index
            .collect_descendants(&root, 3, &mut visited, &mut marked)
            .unwrap();
        assert_eq!(visited, [first.clone(), second.clone(), grandchild.clone()]);
        assert_eq!(marked, [first.clone(), grandchild.clone()]);

        let mut exact = Vec::new();
        let mut exact_marked = Vec::new();
        index
            .collect_descendants(&root, 3, &mut exact, &mut exact_marked)
            .unwrap();
        assert_eq!(exact, visited);
        let mut over_budget = Vec::new();
        let mut over_budget_marked = Vec::new();
        assert_eq!(
            index.collect_descendants(&root, 2, &mut over_budget, &mut over_budget_marked,),
            Err(ViewStyleResolveError::AxisProviderInvalidationBudget {
                node: root.clone(),
                limit: 2,
            })
        );
        assert_eq!(over_budget, [first, second]);

        let leaf = ViewStyleNodeKey::new(mount, Vec::new(), 9);
        index
            .records
            .insert(leaf.clone(), record(None, host_seed(mount, 0), false));
        let mut none = Vec::new();
        let mut none_marked = Vec::new();
        index
            .collect_descendants(&leaf, 0, &mut none, &mut none_marked)
            .unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn ancestor_markers_suppress_duplicate_walks_until_each_node_commits() {
        let mount = ViewMountId::from_raw(8);
        let root = ViewStyleNodeKey::new(mount, Vec::new(), 0);
        let child = ViewStyleNodeKey::new(mount, Vec::new(), 1);
        let grandchild = ViewStyleNodeKey::new(mount, Vec::new(), 2);
        let initial = host_seed(mount, 0);
        let inherited = ViewInheritedBoxAxes::from_parent(initial.mode(), initial.revision());
        let mut index = ViewAxisProviderIndex::default();
        index
            .records
            .insert(root.clone(), record(None, initial, false));
        index
            .records
            .insert(child.clone(), record(Some(root.clone()), inherited, false));
        index.records.insert(
            grandchild.clone(),
            record(Some(child.clone()), inherited, false),
        );
        index
            .children
            .insert(root.clone(), BTreeSet::from([child.clone()]));
        index
            .children
            .insert(child.clone(), BTreeSet::from([grandchild.clone()]));

        let changed = host_seed(mount, 1);
        let facts = ViewStyleNodeFacts::default();
        let root_context = ViewStyleResolveContext {
            node_key: &root,
            node: &facts,
            ancestors: &[],
            applications: &[],
            parent: None,
            parent_node_key: None,
            inherited_axes: changed,
            axis_provider_participation: ViewAxisProviderParticipation::RetainedPrimary,
            environment: &PresentationEnvironment::ENGINE_DEFAULT,
            revisions: ViewStyleRevisionSet::default(),
            trace: ViewStyleTraceMode::Off,
        };
        let root_plan = index
            .prepare(
                &root_context,
                &ComputedViewAxes::from_inherited_seed(changed),
                false,
                2,
            )
            .unwrap()
            .unwrap();
        assert_eq!(root_plan.visited, [child.clone(), grandchild.clone()]);
        index.commit(root_plan);
        assert!(index.records[&child].ancestor_invalidated);
        assert!(index.records[&grandchild].ancestor_invalidated);

        let parent_axes = ComputedViewAxes::from_inherited_seed(changed);
        let parent = ComputedViewStyleBuilder::inherit(None, parent_axes)
            .finish(ComputedViewStyleRevision::new(1));
        let child_inherited = parent.axes().inherited_snapshot();
        let child_context = ViewStyleResolveContext {
            node_key: &child,
            node: &facts,
            ancestors: &[],
            applications: &[],
            parent: Some(&parent),
            parent_node_key: Some(&root),
            inherited_axes: child_inherited,
            axis_provider_participation: ViewAxisProviderParticipation::RetainedPrimary,
            environment: &PresentationEnvironment::ENGINE_DEFAULT,
            revisions: ViewStyleRevisionSet::default(),
            trace: ViewStyleTraceMode::Off,
        };
        let abandoned = index
            .prepare(
                &child_context,
                &ComputedViewAxes::from_inherited_seed(child_inherited),
                false,
                0,
            )
            .unwrap()
            .unwrap();
        assert!(abandoned.visited.is_empty());
        drop(abandoned);
        assert!(index.records[&child].ancestor_invalidated);
        assert_eq!(index.records[&grandchild].parent.as_ref(), Some(&child));

        let retry = index
            .prepare(
                &child_context,
                &ComputedViewAxes::from_inherited_seed(child_inherited),
                false,
                0,
            )
            .unwrap()
            .unwrap();
        assert!(retry.visited.is_empty());
        index.commit(retry);
        assert!(!index.records[&child].ancestor_invalidated);
        assert!(index.records[&grandchild].ancestor_invalidated);
    }
}
