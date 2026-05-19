use crate::effect::{LineEffectRequest, RuntimeCall};

mod flow;
mod frame;
mod line_task;
mod observation;
mod source;
mod stream;
mod task;

fn call(name: &str) -> LineEffectRequest {
    LineEffectRequest::Call(RuntimeCall {
        callee: name.to_owned(),
        args: Vec::new(),
    })
}
