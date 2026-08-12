use crate::effect::{LineEffectRequest, RuntimeCall};
use crate::engine::Engine;
use crate::entry::EntryBindingIdentity;
use crate::line_task::LineTaskGroup;
use crate::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeEntryKind, RuntimeEntryRoles, RuntimeEntrySpec,
    RuntimeEntryTarget, RuntimeFlow, RuntimeLineId, RuntimePlan, RuntimePlanError,
};
use crate::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepOutput};
use arcweft_interaction_model::{
    id::Identifier,
    input::{InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent},
    payload::InteractionPayload,
};

macro_rules! runtime_record {
    ([$(RuntimeFieldValue { name: $name:expr, value: $value:expr, }),* $(,)?]) => {
        $crate::value::RuntimeValue::try_record(vec![$(($name, $value)),*])
            .expect("test record fields are unique")
    };
}

pub(crate) use runtime_record;

mod executor;
mod flow;
mod line_task;
mod observation;
mod pure;
mod root_state;
mod source;
mod step;
mod step_stats_delta;
mod stream;
mod task;
mod value;

fn call(name: &str) -> LineEffectRequest {
    LineEffectRequest::Call(RuntimeCall {
        callee: name.to_owned(),
        args: Vec::new(),
    })
}

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::from_runtime_target_value(value).expect("test flow ID is valid")
}

fn entry_id(value: &str) -> EntryRuntimeId {
    EntryRuntimeId::from_source_entity_body(value).expect("test entry ID is valid")
}

fn line_id(value: &str) -> RuntimeLineId {
    RuntimeLineId::from_runtime_line_value(value).expect("test line ID is valid")
}

fn runtime_plan(
    entry_flow: Option<FlowRuntimeId>,
    flows: Vec<RuntimeFlow>,
    line_task_groups: Vec<LineTaskGroup>,
) -> Result<RuntimePlan, RuntimePlanError> {
    RuntimePlan::new(flows, line_task_groups).map(|plan| {
        if let Some(flow) = entry_flow {
            plan.with_entries(vec![RuntimeEntrySpec {
                id: entry_id("entry.core_test"),
                kind: RuntimeEntryKind::Cli,
                binding: EntryBindingIdentity::from_bytes([1; 32]),
                target: RuntimeEntryTarget::Flow(flow),
                roles: RuntimeEntryRoles::None,
            }])
        } else {
            plan
        }
    })
}

fn engine_for_test_plan(plan: RuntimePlan) -> Engine {
    let entry = entry_id("entry.core_test");
    if plan.entries.iter().any(|candidate| candidate.id == entry) {
        Engine::for_entry(plan, &entry).expect("test entry selects an existing flow")
    } else {
        Engine::new(plan)
    }
}

fn runtime_step(engine: &mut Engine, input: RuntimeStepInput) -> RuntimeStepOutput {
    engine.step(input, RuntimeStepOptions::default()).output
}

fn input_event(kind: &str, payload: Option<&str>) -> RoutedInputEvent {
    let mut event = RoutedInputEvent::new(
        InputEpoch::default(),
        InputSequence::default(),
        InteractionTarget::new("runtime").expect("runtime target"),
        InputEventKind::Custom {
            name: Identifier::new(kind).expect("input kind"),
        },
    );
    if let Some(payload) = payload {
        event = event.with_payload(InteractionPayload::Text(payload.to_owned()));
    }
    event
}

fn dialogue_advance(line: &RuntimeLineId) -> RoutedInputEvent {
    RuntimeStepInput::dialogue_advance_event(line)
}
