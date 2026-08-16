use arcweft_core::plan::EntryRuntimeId;

use crate::binding::ReplBindingRecord;
use crate::evidence::{ReplExecutionRecord, ReplGenerationId};

/// Stable identifier assigned to one committed REPL cell.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReplCellId(u64);

/// Cell family accepted by the transaction substrate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplCellKind {
    Item,
    Statement,
    Expression,
}

/// Source input submitted through the typed REPL API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplCellInput {
    source: String,
    expected_kind: Option<ReplCellKind>,
}

/// Execution state recorded after commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplCellExecutionStatus {
    PendingExecution,
    Executed,
    ExecutionFailed,
    Invalidated,
}

/// Deterministic Product AWBC counters retained in cell records and snapshots.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReplProgramStats {
    pub flows: usize,
    pub instructions: usize,
    pub line_task_groups: usize,
    pub stream_plans: usize,
}

/// Public projection of one committed cell.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplCellRecord {
    pub id: ReplCellId,
    pub ordinal: u64,
    pub kind: ReplCellKind,
    pub source: String,
    pub source_hash: String,
    pub synthetic_source_hash: String,
    pub synthetic_controller_name: String,
    pub base_program_hash: String,
    pub generation: ReplGenerationId,
    pub commit_hash: String,
    pub overlay_hash: String,
    pub entry: Option<String>,
    pub program_stats: ReplProgramStats,
    pub verified_effects: Vec<String>,
    pub bindings: Vec<ReplBindingRecord>,
    pub execution: ReplExecutionRecord,
}

/// Cell-list filter for command adapters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReplCellFilter {
    pub include_invalidated: bool,
}

/// Stable list projection returned to command adapters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReplCellList {
    pub cells: Vec<ReplCellRecord>,
}

/// Result of evaluating one typed cell input.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplEvaluateOutcome {
    pub record: ReplCellRecord,
    pub committed: bool,
}

/// Undo options. Undo never attempts to reverse external host effects.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReplUndoOptions {
    pub preserve_execution_evidence: bool,
}

/// Result of removing the latest committed cell.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplUndoOutcome {
    pub removed: ReplCellRecord,
    pub remaining_cells: usize,
    pub overlay_hash: String,
}

/// Reset options. Reset never attempts to reverse external host effects.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReplResetOptions {
    pub preserve_generation: bool,
}

/// Result of returning the overlay to the base snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplResetOutcome {
    pub removed_cells: usize,
    pub retained_generation: ReplGenerationId,
    pub overlay_hash: String,
}

#[derive(Debug)]
pub(crate) struct CommittedReplCell {
    pub(crate) record: ReplCellRecord,
    pub(crate) bundle: arcweft_bundle::ArcweftBundle,
}

impl ReplCellId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    pub fn label(self) -> String {
        format!("cell.{}", self.0)
    }
}

impl ReplCellKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Statement => "statement",
            Self::Expression => "expression",
        }
    }
}

impl ReplCellInput {
    #[must_use]
    pub fn source(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            expected_kind: None,
        }
    }

    #[must_use]
    pub fn item(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            expected_kind: Some(ReplCellKind::Item),
        }
    }

    #[must_use]
    pub fn statement(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            expected_kind: Some(ReplCellKind::Statement),
        }
    }

    #[must_use]
    pub fn expression(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            expected_kind: Some(ReplCellKind::Expression),
        }
    }

    #[must_use]
    pub fn source_text(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub const fn expected_kind(&self) -> Option<ReplCellKind> {
        self.expected_kind
    }
}

impl ReplCellRecord {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        id: ReplCellId,
        kind: ReplCellKind,
        source: String,
        source_hash: String,
        synthetic_source_hash: String,
        synthetic_controller_name: String,
        base_program_hash: String,
        generation: ReplGenerationId,
        commit_hash: String,
        entry: Option<EntryRuntimeId>,
        program_stats: ReplProgramStats,
        verified_effects: Vec<String>,
        bindings: Vec<ReplBindingRecord>,
    ) -> Self {
        let ordinal = id.as_u64();
        Self {
            id,
            ordinal,
            kind,
            source,
            source_hash,
            synthetic_source_hash,
            synthetic_controller_name,
            base_program_hash,
            generation,
            commit_hash,
            overlay_hash: String::new(),
            entry: entry.map(|entry| entry.public_label().into_string()),
            program_stats,
            verified_effects,
            bindings,
            execution: ReplExecutionRecord::pending(),
        }
    }

    pub(crate) fn set_overlay_hash(&mut self, overlay_hash: String) {
        self.overlay_hash = overlay_hash;
    }

    pub(crate) fn mark_invalidated(&mut self) {
        self.execution.status = ReplCellExecutionStatus::Invalidated;
    }
}

impl CommittedReplCell {
    pub(crate) fn new(record: ReplCellRecord, bundle: arcweft_bundle::ArcweftBundle) -> Self {
        Self { record, bundle }
    }
}
