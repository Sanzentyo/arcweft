use crate::effect::LineEffectRequest;
use crate::frame::{FrameInput, FrameOutput, RuntimeDiagnostic};
use crate::line_task::run_line_task_group_for_input;
use crate::observation::RuntimeObservationState;
use crate::pattern::{RuntimePattern, match_runtime_pattern};
use crate::plan::{
    ChoiceRuntimeOption, FlowEvent, FlowOp, FlowRuntimeId, RuntimeMatchArm, RuntimeMatchSelection,
    RuntimePlan,
};
use crate::source::{
    SourceEvent, SourceEventKind, SourceHandlerPlan, SourceId, SourceOp, SourcePlan, SourcePolicy,
    SourceRuntimeState, normalize_source_events,
};
use crate::stream::{StreamEvent, StreamMatchArm, StreamOp, StreamRuntimeId, StreamRuntimeState};
use crate::task::{
    AwaitTarget, CancelScopeId, TaskClass, TaskEvent, TaskEventKind, TaskKey, TaskPolicy,
    TaskPriority, TaskSource, TaskSpec, normalize_task_events,
};
use crate::value::{
    RuntimeBinding, RuntimeEnv, RuntimeEvalError, RuntimeExpr, RuntimeExprMatchArm,
    RuntimeFieldValue, RuntimeValue, evaluate_binary, evaluate_unary, expr_runtime_label,
    runtime_value_label,
};
use std::collections::{BTreeMap, VecDeque};
pub mod eval;
pub mod flow;
pub mod line;
pub mod source;
pub mod stream;
pub mod suspend;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Engine {
    plan: RuntimePlan,
    fiber: FlowFiber,
}

/// Current flow execution cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowFiber {
    pub line_cursor: usize,
    pub cursor: Option<FlowCursor>,
    pub pending_ops: VecDeque<FlowOp>,
    pub frames: Vec<RuntimeFrame>,
    pub env: RuntimeEnv,
    pub observations: RuntimeObservationState,
    pub source_states: BTreeMap<SourceId, SourceRuntimeState>,
    pub stream_states: BTreeMap<StreamRuntimeId, StreamRuntimeState>,
    pub status: FlowFiberStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFrame {
    pub kind: RuntimeFrameKind,
}

/// Structured frame kind for the minimal flow executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeFrameKind {
    Scope,
    Loop {
        body: Vec<FlowOp>,
        result: Option<RuntimePattern>,
    },
    While {
        condition: RuntimeExpr,
        body: Vec<FlowOp>,
    },
    WhileLet {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: Vec<FlowOp>,
    },
}

/// Position in a lowered flow program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowCursor {
    pub flow: FlowRuntimeId,
    pub op_index: usize,
}

/// High-level flow status for the minimal runtime spine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowFiberStatus {
    Running,
    Waiting(AwaitState),
    Choice(ChoiceState),
    Done(FlowExit),
    Failed(String),
}

/// Suspended `await ... with` state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwaitState {
    pub target: AwaitTarget,
    pub resume: FlowCursor,
}

/// Suspended choice state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceState {
    pub id: Option<String>,
    pub options: Vec<ChoiceRuntimeOption>,
    pub resume: FlowCursor,
}

/// Terminal flow result observed by the minimal runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowExit {
    Done,
    Return(String),
}

impl Default for FlowFiber {
    fn default() -> Self {
        Self {
            line_cursor: 0,
            cursor: None,
            pending_ops: VecDeque::new(),
            frames: Vec::new(),
            env: RuntimeEnv::default(),
            observations: RuntimeObservationState::default(),
            source_states: BTreeMap::new(),
            stream_states: BTreeMap::new(),
            status: FlowFiberStatus::Done(FlowExit::Done),
        }
    }
}

impl FlowCursor {
    fn advanced(&self) -> Self {
        Self {
            flow: self.flow.clone(),
            op_index: self.op_index + 1,
        }
    }
}

impl Default for FlowCursor {
    fn default() -> Self {
        Self {
            flow: FlowRuntimeId(String::new()),
            op_index: 0,
        }
    }
}

impl Engine {
    pub fn new(plan: RuntimePlan) -> Self {
        let cursor = plan.entry_cursor();
        let status = if plan.is_empty() {
            FlowFiberStatus::Done(FlowExit::Done)
        } else {
            FlowFiberStatus::Running
        };
        let source_states = plan
            .source_plans
            .iter()
            .map(|plan| {
                (
                    plan.id.clone(),
                    SourceRuntimeState::new(plan.id.clone(), plan.policy.clone()),
                )
            })
            .collect();
        let stream_states = plan
            .stream_plans
            .iter()
            .map(|plan| (plan.id.clone(), StreamRuntimeState::new(plan.id.clone())))
            .collect();
        Self {
            plan,
            fiber: FlowFiber {
                line_cursor: 0,
                cursor,
                pending_ops: VecDeque::new(),
                frames: Vec::new(),
                env: RuntimeEnv::default(),
                observations: RuntimeObservationState::default(),
                source_states,
                stream_states,
                status,
            },
        }
    }

    pub const fn fiber(&self) -> &FlowFiber {
        &self.fiber
    }

    pub fn step(&mut self, mut input: FrameInput) -> FrameOutput {
        let mut output = FrameOutput::default();
        self.fiber
            .env
            .bind_all_root(input.external_values.iter().cloned());
        let events = normalize_task_events(std::mem::take(&mut input.task_events));
        let source_events = normalize_source_events(std::mem::take(&mut input.source_events));
        output
            .diagnostics
            .extend(events.iter().map(|event| RuntimeDiagnostic {
                message: format!(
                    "task {} sequence {} delivered",
                    event.task_id.0, event.sequence.0
                ),
            }));
        self.apply_source_events(source_events, &mut output);
        self.step_stream_plans(&mut output);

        if self.resume_suspended(&input, &events, &mut output) {
            self.record_observations(&output.line_effects);
            return output;
        }
        if !matches!(self.fiber.status, FlowFiberStatus::Running) {
            self.record_observations(&output.line_effects);
            return output;
        }
        if self.fiber.cursor.is_some() {
            self.step_flow(&input, &mut output);
        } else {
            self.step_line_only(&input, &mut output);
        }
        self.record_observations(&output.line_effects);
        output
    }

    fn record_observations(&mut self, effects: &[LineEffectRequest]) {
        for effect in effects {
            self.fiber.observations.record_effect(effect);
        }
    }

    fn diagnose_runtime_error(error: impl std::fmt::Display, output: &mut FrameOutput) {
        output.diagnostics.push(RuntimeDiagnostic {
            message: error.to_string(),
        });
    }
}
