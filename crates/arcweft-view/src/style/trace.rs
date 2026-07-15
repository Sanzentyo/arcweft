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

/// Resolver-owned collection state. The mode is selected once per resolve so
/// a call site cannot turn an `Off` or `Winners` request into full collection.
pub(super) enum ViewStyleTraceRecorder {
    Off,
    Winners,
    Full(Vec<ViewStyleTraceEntry>),
}

impl ViewStyleTrace {
    pub fn entries(&self) -> &[ViewStyleTraceEntry] {
        &self.entries
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl ViewStyleTraceRecorder {
    pub(super) const fn new(mode: ViewStyleTraceMode) -> Self {
        match mode {
            ViewStyleTraceMode::Off => Self::Off,
            ViewStyleTraceMode::Winners => Self::Winners,
            ViewStyleTraceMode::Full => Self::Full(Vec::new()),
        }
    }

    pub(super) const fn is_full(&self) -> bool {
        matches!(self, Self::Full(_))
    }

    pub(super) fn contribution(
        &mut self,
        property: ViewPropertyKind,
        priority: ViewStylePriority,
        source: ViewStyleContributionSource,
        accepted: bool,
    ) {
        let Self::Full(entries) = self else {
            return;
        };
        entries.push(ViewStyleTraceEntry::Contribution {
            property,
            priority,
            source,
            accepted,
        });
    }

    pub(super) fn rule_rejected(
        &mut self,
        sheet: &ViewStyleSheetId,
        source_order: u32,
        reason: ViewStyleTraceRejection,
    ) {
        let Self::Full(entries) = self else {
            return;
        };
        entries.push(ViewStyleTraceEntry::RuleRejected {
            sheet: sheet.clone(),
            source_order,
            reason,
        });
    }

    pub(super) fn patch_rejected(
        &mut self,
        patch: ViewStylePatchId,
        declaration: ViewStyleSourceId,
        reason: ViewStyleTraceRejection,
    ) {
        let Self::Full(entries) = self else {
            return;
        };
        entries.push(ViewStyleTraceEntry::PatchRejected {
            patch,
            declaration,
            reason,
        });
    }

    pub(super) fn finish(self, computed: &ComputedViewStyle) -> ViewStyleTrace {
        let entries = match self {
            Self::Off => Vec::new(),
            Self::Winners => computed
                .properties()
                .map(|(property, value)| ViewStyleTraceEntry::Winner {
                    property,
                    priority: value.priority(),
                    source: value.source().clone(),
                })
                .collect(),
            Self::Full(entries) => entries,
        };
        ViewStyleTrace { entries }
    }
}
