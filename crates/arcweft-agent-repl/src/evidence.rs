use arcweft_agent_runner::config::AgentControllerRunReport;
use arcweft_core::engine::FlowFiberStatus;
use arcweft_debug_model::event::DebugEventKind;

use crate::binding::ReplBindingRecord;
use crate::cell::ReplCellExecutionStatus;

/// Monotonic generation id visible to tiering and binding projections.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReplGenerationId(u64);

/// Generation projection for command adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplGenerationEvidence {
    pub active_generation: ReplGenerationId,
    pub base_program_hash: String,
    pub overlay_hash: String,
    pub committed_cells: usize,
    pub invalidation_events: usize,
}

/// Binding projection for command adapters.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplBindingEvidence {
    pub base_program_hash: String,
    pub generation: ReplGenerationId,
    pub bindings: Vec<ReplBindingRecord>,
}

/// Count of one debug event family observed during VM execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplDebugEventCount {
    pub kind: DebugEventKind,
    pub count: usize,
}

/// Host-effect evidence retained when execution succeeds or fails after commit.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReplHostEffectEvidence {
    pub host_calls: usize,
    pub events_emitted: u64,
    pub partially_effectful: bool,
    pub event_kinds: Vec<ReplDebugEventCount>,
}

/// Deterministic VM execution record for one committed cell.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplExecutionRecord {
    pub status: ReplCellExecutionStatus,
    pub steps: usize,
    pub host_calls: usize,
    pub responses: usize,
    pub events_emitted: u64,
    pub final_status: Option<String>,
    pub error: Option<String>,
    pub host_effects: ReplHostEffectEvidence,
}

impl ReplGenerationId {
    #[must_use]
    pub const fn base() -> Self {
        Self(0)
    }

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

impl ReplExecutionRecord {
    #[must_use]
    pub fn pending() -> Self {
        Self {
            status: ReplCellExecutionStatus::PendingExecution,
            steps: 0,
            host_calls: 0,
            responses: 0,
            events_emitted: 0,
            final_status: None,
            error: None,
            host_effects: ReplHostEffectEvidence::default(),
        }
    }

    #[must_use]
    pub fn from_report(
        report: &AgentControllerRunReport,
        host_effects: ReplHostEffectEvidence,
    ) -> Self {
        Self {
            status: ReplCellExecutionStatus::Executed,
            steps: report.steps,
            host_calls: report.host_calls,
            responses: report.responses.len(),
            events_emitted: report.events_emitted,
            final_status: report.final_status.as_ref().map(format_flow_status),
            error: None,
            host_effects,
        }
    }

    #[must_use]
    pub fn from_error(message: String, host_effects: ReplHostEffectEvidence) -> Self {
        Self {
            status: ReplCellExecutionStatus::ExecutionFailed,
            steps: 0,
            host_calls: host_effects.host_calls,
            responses: 0,
            events_emitted: host_effects.events_emitted,
            final_status: None,
            error: Some(message),
            host_effects,
        }
    }
}

fn format_flow_status(status: &FlowFiberStatus) -> String {
    format!("{status:?}")
}
