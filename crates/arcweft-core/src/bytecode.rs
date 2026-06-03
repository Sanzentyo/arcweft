use crate::line_task::LineTaskGroup;
use crate::plan::{
    EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeFlow, RuntimePlan, RuntimePlanError, RuntimePureHelper,
};
use crate::source::SourcePlan;
use crate::stream::StreamPlan;
use serde::{Deserialize, Serialize};

/// Pure executable bytecode bundle used by VM, AOT, replay, and future JIT tiers.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct BytecodeProgram {
    pub entry_flow: Option<FlowRuntimeId>,
    pub entries: Vec<BytecodeEntry>,
    pub flows: Vec<BytecodeFlow>,
    pub pure_helpers: Vec<RuntimePureHelper>,
    pub line_task_groups: Vec<LineTaskGroup>,
    pub stream_plans: Vec<StreamPlan>,
    pub source_plans: Vec<SourcePlan>,
}

/// Lowered launch entry preserved in bytecode bundles.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BytecodeEntry {
    pub id: EntryRuntimeId,
    pub kind: RuntimeEntryKind,
    pub target: RuntimeEntryTarget,
}

/// One flow's bytecode instruction stream.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct BytecodeFlow {
    pub id: FlowRuntimeId,
    pub instructions: Vec<BytecodeInstruction>,
}

/// VM instruction.
///
/// Phase 2 keeps the structured runtime operation as the instruction payload so
/// bytecode generation remains semantics-preserving while the evaluator is
/// still being split from `RuntimePlan`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum BytecodeInstruction {
    Flow(FlowOp),
}

/// Deterministic bytecode shape counters for profiling and conformance tests.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BytecodeStats {
    pub flows: usize,
    pub instructions: usize,
    pub line_task_groups: usize,
    pub stream_plans: usize,
    pub source_plans: usize,
}

impl BytecodeProgram {
    pub fn from_runtime_plan(plan: RuntimePlan) -> Self {
        Self {
            entry_flow: plan.entry_flow,
            entries: plan.entries.into_iter().map(BytecodeEntry::from).collect(),
            flows: plan.flows.into_iter().map(BytecodeFlow::from).collect(),
            pure_helpers: plan.pure_helpers,
            line_task_groups: plan.line_task_groups,
            stream_plans: plan.stream_plans,
            source_plans: plan.source_plans,
        }
    }

    pub fn into_runtime_plan(self) -> Result<RuntimePlan, RuntimePlanError> {
        RuntimePlan::new(
            self.entry_flow,
            self.flows.into_iter().map(RuntimeFlow::from).collect(),
            self.line_task_groups,
        )
        .map(|plan| {
            plan.with_entries(
                self.entries
                    .into_iter()
                    .map(RuntimeEntrySpec::from)
                    .collect(),
            )
            .with_pure_helpers(self.pure_helpers)
            .with_generation_plans(self.stream_plans, self.source_plans)
        })
    }

    pub fn stats(&self) -> BytecodeStats {
        BytecodeStats {
            flows: self.flows.len(),
            instructions: self.flows.iter().map(|flow| flow.instructions.len()).sum(),
            line_task_groups: self.line_task_groups.len(),
            stream_plans: self.stream_plans.len(),
            source_plans: self.source_plans.len(),
        }
    }
}

impl From<RuntimePlan> for BytecodeProgram {
    fn from(plan: RuntimePlan) -> Self {
        Self::from_runtime_plan(plan)
    }
}

impl From<RuntimeEntrySpec> for BytecodeEntry {
    fn from(entry: RuntimeEntrySpec) -> Self {
        Self {
            id: entry.id,
            kind: entry.kind,
            target: entry.target,
        }
    }
}

impl From<BytecodeEntry> for RuntimeEntrySpec {
    fn from(entry: BytecodeEntry) -> Self {
        Self {
            id: entry.id,
            kind: entry.kind,
            target: entry.target,
        }
    }
}

impl From<RuntimeFlow> for BytecodeFlow {
    fn from(flow: RuntimeFlow) -> Self {
        Self {
            id: flow.id,
            instructions: flow
                .ops
                .into_iter()
                .map(BytecodeInstruction::Flow)
                .collect(),
        }
    }
}

impl From<BytecodeFlow> for RuntimeFlow {
    fn from(flow: BytecodeFlow) -> Self {
        Self {
            id: flow.id,
            ops: flow
                .instructions
                .into_iter()
                .map(|instruction| match instruction {
                    BytecodeInstruction::Flow(op) => op,
                })
                .collect(),
        }
    }
}
