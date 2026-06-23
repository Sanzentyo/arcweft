use crate::line_task::LineTaskGroup;
use crate::plan::{
    EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeFlow, RuntimePlan, RuntimePlanError, RuntimePureHelper,
};
use crate::source::SourcePlan;
use crate::stream::StreamPlan;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// Pure executable bytecode bundle used by VM, AOT, replay, and future JIT tiers.
pub const BYTECODE_ABI_VERSION: u32 = 1;

/// Stable runtime layout signature for the current structured bytecode model.
pub const BYTECODE_RUNTIME_LAYOUT_SIGNATURE: &str =
    "arcweft.bytecode.runtime-layout.v1.structured-json-flow-op";

/// Pure executable bytecode bundle used by VM, AOT, replay, and future JIT tiers.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BytecodeProgram {
    #[serde(default = "default_bytecode_abi_version")]
    pub abi_version: u32,
    #[serde(default)]
    pub runtime_layout: BytecodeRuntimeLayout,
    pub entry_flow: Option<FlowRuntimeId>,
    pub entries: Vec<BytecodeEntry>,
    pub flows: Vec<BytecodeFlow>,
    pub pure_helpers: Vec<RuntimePureHelper>,
    pub line_task_groups: Vec<LineTaskGroup>,
    pub stream_plans: Vec<StreamPlan>,
    pub source_plans: Vec<SourcePlan>,
}

/// Runtime layout contract expected by bytecode consumers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BytecodeRuntimeLayout {
    #[serde(default = "default_bytecode_abi_version")]
    pub abi_version: u32,
    pub signature: String,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BytecodeVerificationBudget {
    pub flows: usize,
    pub entries: usize,
    pub instructions: usize,
    pub line_task_groups: usize,
    pub stream_plans: usize,
    pub source_plans: usize,
    pub pure_helpers: usize,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum BytecodeVerificationError {
    #[error("unsupported bytecode ABI version {actual}; expected {expected}")]
    UnsupportedAbi { actual: u32, expected: u32 },
    #[error("unsupported bytecode runtime layout `{actual}`; expected `{expected}`")]
    UnsupportedRuntimeLayout { actual: String, expected: String },
    #[error("bytecode artifact is missing its entrypoint flow")]
    MissingEntrypoint,
    #[error("bytecode artifact exceeds verification budget `{budget}`")]
    BudgetExceeded { budget: &'static str },
    #[error("duplicate bytecode flow `{0}`")]
    DuplicateFlow(String),
    #[error("duplicate bytecode entry `{0}`")]
    DuplicateEntry(String),
    #[error("bytecode entry flow `{0}` does not exist")]
    MissingEntryFlow(String),
    #[error("bytecode entry `{entry}` targets missing flow `{flow}`")]
    MissingEntryTarget { entry: String, flow: String },
    #[error("bytecode route entry `{entry}` targets missing flow `{flow}`")]
    MissingRouteTarget { entry: String, flow: String },
    #[error("bytecode flow `{flow}` uses missing line task group {task_group}")]
    MissingLineTaskGroup { flow: String, task_group: usize },
    #[error("bytecode flow `{flow}` jumps to missing flow `{target}`")]
    MissingGotoTarget { flow: String, target: String },
    #[error("bytecode flow `{flow}` choice option targets missing flow `{target}`")]
    MissingChoiceTarget { flow: String, target: String },
}

impl Default for BytecodeProgram {
    fn default() -> Self {
        Self {
            abi_version: BYTECODE_ABI_VERSION,
            runtime_layout: BytecodeRuntimeLayout::current(),
            entry_flow: None,
            entries: Vec::new(),
            flows: Vec::new(),
            pure_helpers: Vec::new(),
            line_task_groups: Vec::new(),
            stream_plans: Vec::new(),
            source_plans: Vec::new(),
        }
    }
}

impl Default for BytecodeRuntimeLayout {
    fn default() -> Self {
        Self::current()
    }
}

impl BytecodeRuntimeLayout {
    pub fn current() -> Self {
        Self {
            abi_version: BYTECODE_ABI_VERSION,
            signature: BYTECODE_RUNTIME_LAYOUT_SIGNATURE.to_owned(),
        }
    }

    pub fn label(&self) -> String {
        format!("{}:{}", self.abi_version, self.signature)
    }
}

impl Default for BytecodeVerificationBudget {
    fn default() -> Self {
        Self {
            flows: 16_384,
            entries: 16_384,
            instructions: 1_000_000,
            line_task_groups: 262_144,
            stream_plans: 65_536,
            source_plans: 65_536,
            pure_helpers: 262_144,
        }
    }
}

impl BytecodeProgram {
    pub fn from_runtime_plan(plan: RuntimePlan) -> Self {
        Self {
            abi_version: BYTECODE_ABI_VERSION,
            runtime_layout: BytecodeRuntimeLayout::current(),
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

    pub fn verify(
        &self,
        budget: BytecodeVerificationBudget,
    ) -> Result<(), BytecodeVerificationError> {
        if self.abi_version != BYTECODE_ABI_VERSION {
            return Err(BytecodeVerificationError::UnsupportedAbi {
                actual: self.abi_version,
                expected: BYTECODE_ABI_VERSION,
            });
        }
        let expected_layout = BytecodeRuntimeLayout::current();
        if self.runtime_layout != expected_layout {
            return Err(BytecodeVerificationError::UnsupportedRuntimeLayout {
                actual: self.runtime_layout.label(),
                expected: expected_layout.label(),
            });
        }
        verify_budget("flows", self.flows.len(), budget.flows)?;
        verify_budget("entries", self.entries.len(), budget.entries)?;
        verify_budget(
            "line_task_groups",
            self.line_task_groups.len(),
            budget.line_task_groups,
        )?;
        verify_budget("stream_plans", self.stream_plans.len(), budget.stream_plans)?;
        verify_budget("source_plans", self.source_plans.len(), budget.source_plans)?;
        verify_budget("pure_helpers", self.pure_helpers.len(), budget.pure_helpers)?;

        let mut flow_ids = BTreeSet::new();
        for flow in &self.flows {
            if !flow_ids.insert(flow.id.clone()) {
                return Err(BytecodeVerificationError::DuplicateFlow(flow.id.0.clone()));
            }
        }
        let Some(entry_flow) = self.entry_flow.as_ref() else {
            return Err(BytecodeVerificationError::MissingEntrypoint);
        };
        if !flow_ids.contains(entry_flow) {
            return Err(BytecodeVerificationError::MissingEntryFlow(
                entry_flow.0.clone(),
            ));
        }

        let mut entry_ids = BTreeSet::new();
        for entry in &self.entries {
            if !entry_ids.insert(entry.id.clone()) {
                return Err(BytecodeVerificationError::DuplicateEntry(
                    entry.id.0.clone(),
                ));
            }
            verify_entry_target(entry, &flow_ids)?;
        }

        let mut instruction_count = 0_usize;
        for flow in &self.flows {
            verify_flow_ops(
                &flow.id,
                &flow.instructions,
                &flow_ids,
                self.line_task_groups.len(),
                &mut instruction_count,
                budget.instructions,
            )?;
        }
        Ok(())
    }
}

const fn default_bytecode_abi_version() -> u32 {
    BYTECODE_ABI_VERSION
}

fn verify_budget(
    label: &'static str,
    actual: usize,
    limit: usize,
) -> Result<(), BytecodeVerificationError> {
    if actual > limit {
        Err(BytecodeVerificationError::BudgetExceeded { budget: label })
    } else {
        Ok(())
    }
}

fn verify_entry_target(
    entry: &BytecodeEntry,
    flow_ids: &BTreeSet<FlowRuntimeId>,
) -> Result<(), BytecodeVerificationError> {
    match &entry.target {
        RuntimeEntryTarget::Flow(flow) => {
            if !flow_ids.contains(flow) {
                return Err(BytecodeVerificationError::MissingEntryTarget {
                    entry: entry.id.0.clone(),
                    flow: flow.0.clone(),
                });
            }
        }
        RuntimeEntryTarget::Routes(routes) => {
            for route in routes {
                if !flow_ids.contains(&route.target) {
                    return Err(BytecodeVerificationError::MissingRouteTarget {
                        entry: entry.id.0.clone(),
                        flow: route.target.0.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn verify_flow_ops(
    flow: &FlowRuntimeId,
    instructions: &[BytecodeInstruction],
    flow_ids: &BTreeSet<FlowRuntimeId>,
    line_task_groups: usize,
    instruction_count: &mut usize,
    instruction_limit: usize,
) -> Result<(), BytecodeVerificationError> {
    for instruction in instructions {
        *instruction_count = instruction_count.saturating_add(1);
        verify_budget("instructions", *instruction_count, instruction_limit)?;
        match instruction {
            BytecodeInstruction::Flow(op) => {
                verify_flow_op(
                    flow,
                    op,
                    flow_ids,
                    line_task_groups,
                    instruction_count,
                    instruction_limit,
                )?;
            }
        }
    }
    Ok(())
}

fn verify_flow_op(
    flow: &FlowRuntimeId,
    op: &FlowOp,
    flow_ids: &BTreeSet<FlowRuntimeId>,
    line_task_groups: usize,
    instruction_count: &mut usize,
    instruction_limit: usize,
) -> Result<(), BytecodeVerificationError> {
    match op {
        FlowOp::Dialogue { task_group, .. } => {
            verify_line_task_group(flow, *task_group, line_task_groups)?;
        }
        FlowOp::Choice { options, .. } => verify_choice_targets(flow, options, flow_ids)?,
        FlowOp::Goto(target) => verify_goto_target(flow, target, flow_ids)?,
        FlowOp::LetElse { else_ops, .. } => verify_nested_ops(
            flow,
            else_ops,
            flow_ids,
            line_task_groups,
            instruction_count,
            instruction_limit,
        )?,
        FlowOp::If {
            then_ops, else_ops, ..
        }
        | FlowOp::IfLet {
            then_ops, else_ops, ..
        } => verify_two_nested_ops(
            flow,
            then_ops,
            else_ops,
            flow_ids,
            line_task_groups,
            instruction_count,
            instruction_limit,
        )?,
        FlowOp::Match { arms, .. } => verify_match_ops(
            flow,
            arms,
            flow_ids,
            line_task_groups,
            instruction_count,
            instruction_limit,
        )?,
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
        | FlowOp::While { body, .. }
        | FlowOp::WhileLet { body, .. }
        | FlowOp::For { body, .. }
        | FlowOp::Thread { body, .. } => verify_nested_ops(
            flow,
            body,
            flow_ids,
            line_task_groups,
            instruction_count,
            instruction_limit,
        )?,
        FlowOp::LoopNext { body }
        | FlowOp::WhileNext { body, .. }
        | FlowOp::WhileLetNext { body, .. } => {
            verify_nested_ops(
                flow,
                body,
                flow_ids,
                line_task_groups,
                instruction_count,
                instruction_limit,
            )?;
        }
        FlowOp::ForNext { body, .. } => verify_nested_ops(
            flow,
            body,
            flow_ids,
            line_task_groups,
            instruction_count,
            instruction_limit,
        )?,
        FlowOp::Scope(ops) | FlowOp::LetScope { ops, .. } => verify_nested_ops(
            flow,
            ops,
            flow_ids,
            line_task_groups,
            instruction_count,
            instruction_limit,
        )?,
        FlowOp::Bind(_)
        | FlowOp::Let { .. }
        | FlowOp::Await { .. }
        | FlowOp::AwaitMany { .. }
        | FlowOp::Break(_)
        | FlowOp::Continue
        | FlowOp::GotoExpr(_)
        | FlowOp::Return(_)
        | FlowOp::ReturnExpr(_)
        | FlowOp::Effect(_)
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::ExitScopeBind { .. }
        | FlowOp::Noop => {}
    }
    Ok(())
}

fn verify_line_task_group(
    flow: &FlowRuntimeId,
    task_group: usize,
    line_task_groups: usize,
) -> Result<(), BytecodeVerificationError> {
    if task_group >= line_task_groups {
        Err(BytecodeVerificationError::MissingLineTaskGroup {
            flow: flow.0.clone(),
            task_group,
        })
    } else {
        Ok(())
    }
}

fn verify_choice_targets(
    flow: &FlowRuntimeId,
    options: &[crate::plan::ChoiceRuntimeOption],
    flow_ids: &BTreeSet<FlowRuntimeId>,
) -> Result<(), BytecodeVerificationError> {
    for option in options {
        if let Some(target) = option.target.as_ref()
            && !flow_ids.contains(target)
        {
            return Err(BytecodeVerificationError::MissingChoiceTarget {
                flow: flow.0.clone(),
                target: target.0.clone(),
            });
        }
    }
    Ok(())
}

fn verify_goto_target(
    flow: &FlowRuntimeId,
    target: &FlowRuntimeId,
    flow_ids: &BTreeSet<FlowRuntimeId>,
) -> Result<(), BytecodeVerificationError> {
    if flow_ids.contains(target) {
        Ok(())
    } else {
        Err(BytecodeVerificationError::MissingGotoTarget {
            flow: flow.0.clone(),
            target: target.0.clone(),
        })
    }
}

fn verify_nested_ops(
    flow: &FlowRuntimeId,
    ops: &[FlowOp],
    flow_ids: &BTreeSet<FlowRuntimeId>,
    line_task_groups: usize,
    instruction_count: &mut usize,
    instruction_limit: usize,
) -> Result<(), BytecodeVerificationError> {
    for op in ops {
        *instruction_count = instruction_count.saturating_add(1);
        verify_budget("instructions", *instruction_count, instruction_limit)?;
        verify_flow_op(
            flow,
            op,
            flow_ids,
            line_task_groups,
            instruction_count,
            instruction_limit,
        )?;
    }
    Ok(())
}

fn verify_two_nested_ops(
    flow: &FlowRuntimeId,
    first: &[FlowOp],
    second: &[FlowOp],
    flow_ids: &BTreeSet<FlowRuntimeId>,
    line_task_groups: usize,
    instruction_count: &mut usize,
    instruction_limit: usize,
) -> Result<(), BytecodeVerificationError> {
    verify_nested_ops(
        flow,
        first,
        flow_ids,
        line_task_groups,
        instruction_count,
        instruction_limit,
    )?;
    verify_nested_ops(
        flow,
        second,
        flow_ids,
        line_task_groups,
        instruction_count,
        instruction_limit,
    )
}

fn verify_match_ops(
    flow: &FlowRuntimeId,
    arms: &[crate::plan::RuntimeMatchArm],
    flow_ids: &BTreeSet<FlowRuntimeId>,
    line_task_groups: usize,
    instruction_count: &mut usize,
    instruction_limit: usize,
) -> Result<(), BytecodeVerificationError> {
    for arm in arms {
        verify_nested_ops(
            flow,
            &arm.ops,
            flow_ids,
            line_task_groups,
            instruction_count,
            instruction_limit,
        )?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::{
        BYTECODE_ABI_VERSION, BytecodeEntry, BytecodeFlow, BytecodeInstruction, BytecodeProgram,
        BytecodeRuntimeLayout, BytecodeVerificationBudget, BytecodeVerificationError, FlowOp,
    };
    use crate::line_task::LineTaskGroup;
    use crate::plan::{
        ChoiceRuntimeOption, EntryRuntimeId, FlowRuntimeId, RuntimeEntryKind, RuntimeEntryTarget,
    };

    #[test]
    fn verifies_well_formed_structured_bytecode() {
        sample_program()
            .verify(BytecodeVerificationBudget::default())
            .expect("bytecode verifies");
    }

    #[test]
    fn rejects_unsupported_bytecode_abi() {
        let mut program = sample_program();
        program.abi_version = BYTECODE_ABI_VERSION + 1;

        assert!(matches!(
            program.verify(BytecodeVerificationBudget::default()),
            Err(BytecodeVerificationError::UnsupportedAbi { actual, expected })
                if actual == BYTECODE_ABI_VERSION + 1 && expected == BYTECODE_ABI_VERSION
        ));
    }

    #[test]
    fn rejects_runtime_layout_signature_mismatch() {
        let mut program = sample_program();
        program.runtime_layout = BytecodeRuntimeLayout {
            abi_version: BYTECODE_ABI_VERSION,
            signature: "arcweft.bytecode.runtime-layout.v0.test".to_owned(),
        };

        assert!(matches!(
            program.verify(BytecodeVerificationBudget::default()),
            Err(BytecodeVerificationError::UnsupportedRuntimeLayout { actual, expected })
                if actual.contains("v0.test") && expected.contains("structured-json-flow-op")
        ));
    }

    #[test]
    fn rejects_missing_entrypoint() {
        let mut program = sample_program();
        program.entry_flow = None;

        assert!(matches!(
            program.verify(BytecodeVerificationBudget::default()),
            Err(BytecodeVerificationError::MissingEntrypoint)
        ));
    }

    #[test]
    fn rejects_missing_line_task_group() {
        let mut program = sample_program();
        program.line_task_groups.clear();

        assert!(matches!(
            program.verify(BytecodeVerificationBudget::default()),
            Err(BytecodeVerificationError::MissingLineTaskGroup { flow, task_group })
                if flow == "flow.main" && task_group == 0
        ));
    }

    #[test]
    fn rejects_missing_choice_target() {
        let mut program = sample_program();
        program.flows[0]
            .instructions
            .push(BytecodeInstruction::Flow(FlowOp::Choice {
                id: None,
                options: vec![ChoiceRuntimeOption {
                    id: Some("choice.missing".to_owned()),
                    label: "Missing".to_owned(),
                    target: Some(FlowRuntimeId("flow.missing".to_owned())),
                    out: None,
                    effects: Vec::new(),
                }],
            }));

        assert!(matches!(
            program.verify(BytecodeVerificationBudget::default()),
            Err(BytecodeVerificationError::MissingChoiceTarget { flow, target })
                if flow == "flow.main" && target == "flow.missing"
        ));
    }

    #[test]
    fn rejects_instruction_budget_excess_in_nested_ops() {
        let mut program = sample_program();
        program.flows[0]
            .instructions
            .push(BytecodeInstruction::Flow(FlowOp::Scope(vec![FlowOp::Noop])));

        assert!(matches!(
            program.verify(BytecodeVerificationBudget {
                instructions: 2,
                ..BytecodeVerificationBudget::default()
            }),
            Err(BytecodeVerificationError::BudgetExceeded {
                budget: "instructions"
            })
        ));
    }

    fn sample_program() -> BytecodeProgram {
        BytecodeProgram {
            abi_version: BYTECODE_ABI_VERSION,
            runtime_layout: BytecodeRuntimeLayout::current(),
            entry_flow: Some(FlowRuntimeId("flow.main".to_owned())),
            entries: vec![BytecodeEntry {
                id: EntryRuntimeId("entry.main".to_owned()),
                kind: RuntimeEntryKind::Game,
                target: RuntimeEntryTarget::Flow(FlowRuntimeId("flow.main".to_owned())),
            }],
            flows: vec![
                BytecodeFlow {
                    id: FlowRuntimeId("flow.main".to_owned()),
                    instructions: vec![
                        BytecodeInstruction::Flow(FlowOp::Dialogue {
                            line: "line.opening".into(),
                            task_group: 0,
                        }),
                        BytecodeInstruction::Flow(FlowOp::Goto(FlowRuntimeId(
                            "flow.done".to_owned(),
                        ))),
                    ],
                },
                BytecodeFlow {
                    id: FlowRuntimeId("flow.done".to_owned()),
                    instructions: vec![BytecodeInstruction::Flow(FlowOp::Return(
                        "done".to_owned(),
                    ))],
                },
            ],
            pure_helpers: Vec::new(),
            line_task_groups: vec![LineTaskGroup::default()],
            stream_plans: Vec::new(),
            source_plans: Vec::new(),
        }
    }
}
