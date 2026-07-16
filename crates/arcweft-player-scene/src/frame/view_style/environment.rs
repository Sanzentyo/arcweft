//! Field-local environment invalidation for retained View Style nodes.

use super::PlayerViewStyleState;
use arcweft_presentation::appearance::PresentationEnvironmentFieldSet;
use arcweft_runtime_driver::session::PresentationEnvironmentUpdate;

/// Exact retained-node effects of one committed session environment update.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::frame) struct ViewStyleEnvironmentInvalidation {
    pub(in crate::frame) selected: usize,
    pub(in crate::frame) projected: usize,
    pub(in crate::frame) unchanged: usize,
}

impl PlayerViewStyleState {
    /// Environment fields used by any currently retained Style node.
    pub(in crate::frame) fn environment_fields(&self) -> PresentationEnvironmentFieldSet {
        self.environment_usage
            .values()
            .fold(PresentationEnvironmentFieldSet::NONE, |fields, usage| {
                fields.union(usage.all())
            })
    }

    /// Applies the session-computed changed set without recomputing that diff.
    pub(in crate::frame) fn apply_environment_update(
        &mut self,
        update: PresentationEnvironmentUpdate,
    ) -> ViewStyleEnvironmentInvalidation {
        let changed = update.effective_changed_fields();
        let mut invalidation = ViewStyleEnvironmentInvalidation::default();
        for (node, usage) in &self.environment_usage {
            if !usage.selection().intersection(changed).is_empty() {
                self.resolver.invalidate_node(node);
                invalidation.selected += 1;
            } else if !usage.projection().intersection(changed).is_empty() {
                invalidation.projected += 1;
            } else {
                invalidation.unchanged += 1;
            }
        }
        invalidation
    }
}
