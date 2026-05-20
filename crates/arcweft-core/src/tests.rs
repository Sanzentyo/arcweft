use crate::effect::{LineEffectRequest, RuntimeCall};
use crate::engine::Engine;
use crate::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepOutput};

mod executor;
mod flow;
mod line_task;
mod observation;
mod source;
mod step;
mod stream;
mod task;

fn call(name: &str) -> LineEffectRequest {
    LineEffectRequest::Call(RuntimeCall {
        callee: name.to_owned(),
        args: Vec::new(),
    })
}

fn runtime_step(engine: &mut Engine, input: RuntimeStepInput) -> RuntimeStepOutput {
    engine.step(input, RuntimeStepOptions::default()).output
}
