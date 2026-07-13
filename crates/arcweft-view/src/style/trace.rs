//! Optional deterministic native Style resolution trace.

use super::{
    ComputedViewStyle, ViewPropertyKind, ViewStyleContributionSource, ViewStylePatchId,
    ViewStylePriority, ViewStyleSheetId, ViewStyleSourceId,
};

/// Trace collection policy. Runtime resolution defaults to no allocation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewStyleTraceMode {
    #[default]
    Off,
    Winners,
    Full,
}

/// Why a selector/rule did not contribute to the target node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewStyleTraceRejection {
    SelectorMismatch,
    BoundaryTraversalBlocked,
    InteractionStateMismatch,
    ElementStateMismatch,
    EnvironmentMismatch,
    ContainerFactsUnavailable,
    PropertyNotApplicable,
    LowerPriority,
}

/// One deterministic resolution event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewStyleTraceEntry {
    Winner {
        property: ViewPropertyKind,
        priority: ViewStylePriority,
        source: ViewStyleContributionSource,
    },
    Contribution {
        property: ViewPropertyKind,
        priority: ViewStylePriority,
        source: ViewStyleContributionSource,
        accepted: bool,
    },
    RuleRejected {
        sheet: ViewStyleSheetId,
        source_order: u32,
        reason: ViewStyleTraceRejection,
    },
    PatchRejected {
        patch: ViewStylePatchId,
        declaration: ViewStyleSourceId,
        reason: ViewStyleTraceRejection,
    },
}

/// Trace returned beside a computed result when requested.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewStyleTrace {
    entries: Vec<ViewStyleTraceEntry>,
}

impl ViewStyleTrace {
    pub fn entries(&self) -> &[ViewStyleTraceEntry] {
        &self.entries
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn push(&mut self, mode: ViewStyleTraceMode, entry: ViewStyleTraceEntry) {
        if mode == ViewStyleTraceMode::Full {
            self.entries.push(entry);
        }
    }

    pub(super) fn finish_winners(
        &mut self,
        mode: ViewStyleTraceMode,
        computed: &ComputedViewStyle,
    ) {
        if mode != ViewStyleTraceMode::Winners {
            return;
        }
        self.entries
            .extend(
                computed
                    .properties()
                    .map(|(property, value)| ViewStyleTraceEntry::Winner {
                        property,
                        priority: value.priority(),
                        source: value.source().clone(),
                    }),
            );
    }
}
