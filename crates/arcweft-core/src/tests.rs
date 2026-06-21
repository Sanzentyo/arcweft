use crate::effect::{LineEffectRequest, RuntimeCall};
use crate::engine::Engine;
use crate::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepOutput};
use arcweft_interaction_model::{
    id::Identifier,
    input::{InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent},
    payload::InteractionPayload,
};

mod executor;
mod flow;
mod line_task;
mod observation;
mod pure;
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
