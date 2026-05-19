use arcweft_lang_hir::syntax::{LifetimeKey, Stmt};
use arcweft_lang_syntax::DeferOutcome;
use std::collections::HashSet;

/// Why a scope path is leaving the current continuation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExitReason {
    Completed,
    Cancelled,
    Failed,
    Break,
    Continue,
}

/// Cleanup registered by `defer` in the current runtime scope.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DeferredCleanup {
    outcome: DeferOutcome,
    drops: HashSet<LifetimeKey>,
}

/// Path-sensitive semantic facts carried through statement analysis.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct FlowFacts {
    pub(super) live_must_drop: HashSet<LifetimeKey>,
    pub(super) touched_must_drop: HashSet<LifetimeKey>,
    pub(super) deferred_cleanups: Vec<DeferredCleanup>,
}

/// A non-fallthrough path produced by control transfer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExitPath {
    pub(super) reason: ExitReason,
    pub(super) facts: FlowFacts,
}

/// Result of analyzing a list of statements.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct BlockFlow {
    pub(super) fallthrough: Vec<FlowFacts>,
    pub(super) exits: Vec<ExitPath>,
}

impl DeferredCleanup {
    pub(super) fn new(outcome: DeferOutcome, drops: HashSet<LifetimeKey>) -> Self {
        Self { outcome, drops }
    }

    fn applies_to(&self, reason: ExitReason) -> bool {
        let reason = match reason {
            ExitReason::Break | ExitReason::Continue => ExitReason::Completed,
            reason => reason,
        };
        matches!(self.outcome, DeferOutcome::Always)
            || matches!(
                (self.outcome, reason),
                (DeferOutcome::Completed, ExitReason::Completed)
                    | (DeferOutcome::Cancelled, ExitReason::Cancelled)
                    | (DeferOutcome::Failed, ExitReason::Failed)
            )
    }
}

impl FlowFacts {
    pub(super) fn add_must_drop(&mut self, key: LifetimeKey) {
        self.live_must_drop.insert(key.clone());
        self.touched_must_drop.insert(key);
    }

    pub(super) fn remove_must_drop(&mut self, key: &LifetimeKey) {
        self.live_must_drop.remove(key);
    }

    pub(super) fn has_touched_must_drop(&self) -> bool {
        !self.touched_must_drop.is_empty()
    }

    pub(super) fn register_cleanup(&mut self, cleanup: DeferredCleanup) {
        self.deferred_cleanups.push(cleanup);
    }

    pub(super) fn live_after_cleanup(&self, reason: ExitReason) -> HashSet<LifetimeKey> {
        let mut live = self.live_must_drop.clone();
        for cleanup in self
            .deferred_cleanups
            .iter()
            .rev()
            .filter(|cleanup| cleanup.applies_to(reason))
        {
            for key in &cleanup.drops {
                live.remove(key);
            }
        }
        live
    }

    pub(super) fn merge_from(&mut self, other: &Self) -> bool {
        let before = self.clone();
        self.live_must_drop
            .extend(other.live_must_drop.iter().cloned());
        self.touched_must_drop
            .extend(other.touched_must_drop.iter().cloned());
        for cleanup in &other.deferred_cleanups {
            if !self.deferred_cleanups.contains(cleanup) {
                self.deferred_cleanups.push(cleanup.clone());
            }
        }
        *self != before
    }
}

impl ExitPath {
    pub(super) fn new(reason: ExitReason, facts: FlowFacts) -> Self {
        Self { reason, facts }
    }
}

impl BlockFlow {
    pub(super) fn from_fallthrough(facts: FlowFacts) -> Self {
        Self {
            fallthrough: vec![facts],
            exits: Vec::new(),
        }
    }

    pub(super) fn from_exit(reason: ExitReason, facts: FlowFacts) -> Self {
        Self {
            fallthrough: Vec::new(),
            exits: vec![ExitPath::new(reason, facts)],
        }
    }

    pub(super) fn append(&mut self, other: Self) {
        self.fallthrough.extend(other.fallthrough);
        self.exits.extend(other.exits);
    }

    pub(super) fn has_touched_must_drop(&self) -> bool {
        self.fallthrough
            .iter()
            .any(FlowFacts::has_touched_must_drop)
            || self
                .exits
                .iter()
                .any(|exit| exit.facts.has_touched_must_drop())
    }
}

pub(super) fn transfer_reason(stmt: &Stmt, context: ExitReason) -> Option<ExitReason> {
    match stmt {
        Stmt::Return(_) | Stmt::Close(_) | Stmt::Goto(_) | Stmt::Yield(_) | Stmt::Out { .. } => {
            Some(context)
        }
        Stmt::Break { .. } => Some(ExitReason::Break),
        Stmt::Continue { .. } => Some(ExitReason::Continue),
        Stmt::Panic(_) | Stmt::Fail(_) | Stmt::Bail(_) => Some(ExitReason::Failed),
        _ => None,
    }
}
