//! Structured/compact differential observation model.
//!
//! The compact VM does not call the structured VM. This module only provides a
//! normalized observation schema so tests can feed the same scripted host inputs
//! into both executors and compare visible behavior.

use super::schema::{AwbcEffectPlanId, AwbcTrapCode};
use super::vm::{VmExit, VmObservation, VmStepOutput};
use crate::engine::FlowFiberStatus;
use crate::plan::FlowEvent;
use crate::step::{RuntimeStepResult, RuntimeStepStopReason};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParityTrace {
    pub events: Vec<ParityEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParityEvent {
    Dialogue {
        line: String,
    },
    ChoicePresented {
        id: Option<String>,
        options: Vec<(Option<String>, String)>,
    },
    ChoiceSelected {
        id: Option<String>,
        option: String,
    },
    AwaitStarted {
        need: String,
        task: String,
    },
    AwaitReady {
        need: String,
    },
    AwaitProgress {
        need: String,
    },
    Effect {
        id: String,
    },
    Content {
        id: String,
    },
    StreamYield {
        stream: String,
    },
    StreamClose {
        stream: String,
    },
    SourceYield {
        source: String,
    },
    SourceClose {
        source: String,
    },
    Stop {
        reason: String,
    },
    Trap {
        code: String,
        message: Option<String>,
    },
    Status {
        status: String,
    },
}

impl ParityTrace {
    pub fn from_structured(result: &RuntimeStepResult) -> Self {
        let mut trace = Self::default();
        trace
            .events
            .extend(result.output.flow_events.iter().map(flow_event));
        trace.events.extend(
            result
                .output
                .effects
                .line
                .iter()
                .map(|effect| ParityEvent::Effect {
                    id: format!("{effect:?}"),
                }),
        );
        trace.events.push(ParityEvent::Stop {
            reason: stop_reason(result.stop_reason).to_owned(),
        });
        trace.events.push(ParityEvent::Status {
            status: fiber_status(&result.fiber_status),
        });
        trace
    }

    pub fn from_compact(output: &VmStepOutput) -> Self {
        let mut trace = Self::default();
        trace
            .events
            .extend(output.observations.iter().map(vm_observation));
        trace.events.push(match &output.exit {
            VmExit::Running => ParityEvent::Stop {
                reason: "running".to_owned(),
            },
            VmExit::Suspended(reason) => ParityEvent::Stop {
                reason: format!("suspended:{reason:?}"),
            },
            VmExit::Returned(_) => ParityEvent::Stop {
                reason: "returned".to_owned(),
            },
            VmExit::Trapped(trap) => ParityEvent::Trap {
                code: trap_code(trap.code).to_owned(),
                message: trap.message.clone(),
            },
            VmExit::BudgetYield(_) => ParityEvent::Stop {
                reason: "budget_yield".to_owned(),
            },
        });
        trace
    }

    #[must_use]
    pub fn normalize_for_smoke(mut self) -> Self {
        self.events
            .retain(|event| !matches!(event, ParityEvent::Status { .. }));
        self
    }
}

fn flow_event(event: &FlowEvent) -> ParityEvent {
    match event {
        FlowEvent::DialogueLine { line, .. } => ParityEvent::Dialogue {
            line: line.canonical_label(),
        },
        FlowEvent::LineCancelled { trigger } => ParityEvent::Effect {
            id: format!("line_cancelled:{trigger}"),
        },
        FlowEvent::ChoicePresented { id, options } => ParityEvent::ChoicePresented {
            id: id.clone(),
            options: options
                .iter()
                .map(|option| (option.id.clone(), option.label.clone()))
                .collect(),
        },
        FlowEvent::ChoiceSelected { id, option } => ParityEvent::ChoiceSelected {
            id: id.clone(),
            option: option.clone(),
        },
        FlowEvent::AwaitStarted { need, task } => ParityEvent::AwaitStarted {
            need: need.0.clone(),
            task: task.0.clone(),
        },
        FlowEvent::AwaitReady { need, .. } => ParityEvent::AwaitReady {
            need: need.0.clone(),
        },
        FlowEvent::AwaitProgress { need, .. } => ParityEvent::AwaitProgress {
            need: need.0.clone(),
        },
        FlowEvent::Goto { target } => ParityEvent::Effect {
            id: format!("goto:{target}"),
        },
        FlowEvent::Return { value } => ParityEvent::Stop {
            reason: format!("return:{value}"),
        },
        FlowEvent::Done => ParityEvent::Stop {
            reason: "done".to_owned(),
        },
    }
}

fn vm_observation(event: &VmObservation) -> ParityEvent {
    match event {
        VmObservation::Instruction { .. } => ParityEvent::Effect {
            id: "instruction".to_owned(),
        },
        VmObservation::Effect {
            effect: AwbcEffectPlanId(id),
            ..
        } => ParityEvent::Effect {
            id: format!("effect#{id}"),
        },
        VmObservation::TaskStarted { plan, .. } => ParityEvent::AwaitStarted {
            need: format!("task#{}", plan.0),
            task: format!("task#{}", plan.0),
        },
        VmObservation::Goto(function) => ParityEvent::Effect {
            id: format!("goto#{}", function.0),
        },
        VmObservation::FiberSpawned { function, .. } => ParityEvent::Effect {
            id: format!("fiber#{}", function.0),
        },
        VmObservation::EnsureContent(id) => ParityEvent::Content {
            id: format!("content#{}", id.0),
        },
        VmObservation::StreamYield { stream, .. } => ParityEvent::StreamYield {
            stream: format!("stream#{}", stream.0),
        },
        VmObservation::StreamClose(stream) => ParityEvent::StreamClose {
            stream: format!("stream#{}", stream.0),
        },
        VmObservation::SourceYield { source, .. } => ParityEvent::SourceYield {
            source: format!("source#{}", source.0),
        },
        VmObservation::SourceClose(source) => ParityEvent::SourceClose {
            source: format!("source#{}", source.0),
        },
        VmObservation::Trap(trap) => ParityEvent::Trap {
            code: trap_code(trap.code).to_owned(),
            message: trap.message.clone(),
        },
    }
}

fn stop_reason(reason: RuntimeStepStopReason) -> &'static str {
    match reason {
        RuntimeStepStopReason::OneOp => "one_op",
        RuntimeStepStopReason::Blocked => "blocked",
        RuntimeStepStopReason::Output => "output",
        RuntimeStepStopReason::BudgetExhausted => "budget_exhausted",
        RuntimeStepStopReason::Done => "done",
        RuntimeStepStopReason::Failed => "failed",
    }
}

fn fiber_status(status: &FlowFiberStatus) -> String {
    match status {
        FlowFiberStatus::Running => "running".to_owned(),
        FlowFiberStatus::Dialogue(_) => "dialogue".to_owned(),
        FlowFiberStatus::Waiting(_) => "waiting".to_owned(),
        FlowFiberStatus::WaitingMany(_) => "waiting_many".to_owned(),
        FlowFiberStatus::HostCall(_) => "host_call".to_owned(),
        FlowFiberStatus::Choice(_) => "choice".to_owned(),
        FlowFiberStatus::Done(_) => "done".to_owned(),
        FlowFiberStatus::Failed(message) => format!("failed:{message}"),
    }
}

fn trap_code(code: AwbcTrapCode) -> &'static str {
    match code {
        AwbcTrapCode::TypeMismatch => "type_mismatch",
        AwbcTrapCode::UninitializedRegister => "uninitialized_register",
        AwbcTrapCode::InvalidIndex => "invalid_index",
        AwbcTrapCode::DivisionByZero => "division_by_zero",
        AwbcTrapCode::PatternMismatch => "pattern_mismatch",
        AwbcTrapCode::MissingDynamicTarget => "missing_dynamic_target",
        AwbcTrapCode::HostAbiMismatch => "host_abi_mismatch",
        AwbcTrapCode::CapabilityDenied => "capability_denied",
        AwbcTrapCode::ExplicitPanic => "explicit_panic",
        AwbcTrapCode::InternalInvariant => "internal_invariant",
    }
}
