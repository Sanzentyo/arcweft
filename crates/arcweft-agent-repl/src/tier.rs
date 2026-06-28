use arcweft_core::bytecode::BytecodeProgram;

use crate::cell::ReplCellId;
use crate::evidence::ReplGenerationId;

/// Cursor into the ordered tier invalidation stream.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReplTierCursor(u64);

/// Reason a cached tier projection may no longer match committed REPL state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplTierInvalidationReason {
    CellCommitted,
    CellExecutionFailed,
    CellUndone,
    ResetToBase,
    BaseProjectChanged,
    GenerationChanged,
    TierStatusRecorded,
}

/// Ordered token visible to seq05.3 without exposing overlay internals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplTierInvalidationToken {
    pub cursor: ReplTierCursor,
    pub reason: ReplTierInvalidationReason,
    pub generation: ReplGenerationId,
    pub overlay_hash: String,
    pub cell_id: Option<ReplCellId>,
    pub detail: Option<String>,
}

/// One executable committed-cell projection for tiering.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplExecutableCell {
    pub cell_id: ReplCellId,
    pub ordinal: u64,
    pub commit_hash: String,
    pub source_hash: String,
    pub synthetic_agent_id: String,
    pub entry_flow: Option<String>,
    pub bytecode: BytecodeProgram,
}

/// Executable snapshot consumed by VM/JIT/AOT tiering packages.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplExecutableSnapshot {
    pub base_program_hash: String,
    pub generation: ReplGenerationId,
    pub overlay_hash: String,
    pub cells: Vec<ReplExecutableCell>,
}

/// Tier status reported back by seq05.3.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplTierStatusRecord {
    pub generation: ReplGenerationId,
    pub overlay_hash: String,
    pub cell_id: Option<ReplCellId>,
    pub tier: String,
    pub status: String,
    pub detail: Option<String>,
}

/// Latest tier status projection.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReplTierStatusProjection {
    pub records: Vec<ReplTierStatusRecord>,
}

impl ReplTierCursor {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}
