use crate::effect::{LineEffectRequest, RuntimeCall, RuntimeEvent, RuntimeLog};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeObservationState {
    pub signals: BTreeMap<String, String>,
    pub metrics: BTreeMap<String, String>,
    pub logs: Vec<RuntimeLog>,
    pub events: Vec<RuntimeEvent>,
    pub calls: Vec<RuntimeCall>,
}

impl RuntimeObservationState {
    pub fn record_effect(&mut self, effect: &LineEffectRequest) {
        match effect {
            LineEffectRequest::Log(log) => self.logs.push(log.clone()),
            LineEffectRequest::Call(call) => self.calls.push(call.clone()),
            LineEffectRequest::SignalWrite(write) => {
                self.signals
                    .insert(write.target.clone(), write.value.clone());
            }
            LineEffectRequest::MetricWrite(write) => {
                self.metrics
                    .insert(write.target.clone(), write.value.clone());
            }
            LineEffectRequest::EmitEvent(event) => self.events.push(event.clone()),
            LineEffectRequest::RegisterHandle { .. }
            | LineEffectRequest::DropHandle { .. }
            | LineEffectRequest::Audio(_)
            | LineEffectRequest::Wait(_)
            | LineEffectRequest::Out(_)
            | LineEffectRequest::Return(_)
            | LineEffectRequest::Goto(_)
            | LineEffectRequest::Panic(_)
            | LineEffectRequest::Fail(_)
            | LineEffectRequest::Bail(_)
            | LineEffectRequest::Ensure { .. }
            | LineEffectRequest::Assert(_)
            | LineEffectRequest::Close(_)
            | LineEffectRequest::Select(_)
            | LineEffectRequest::Break { .. }
            | LineEffectRequest::Continue { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_records_generic_runtime_calls_for_adapters() {
        let mut state = RuntimeObservationState::default();
        state.record_effect(&LineEffectRequest::Call(RuntimeCall {
            callee: "bg".to_owned(),
            args: vec!["@asset.bg.room".to_owned()],
        }));

        assert_eq!(
            state.calls,
            vec![RuntimeCall {
                callee: "bg".to_owned(),
                args: vec!["@asset.bg.room".to_owned()],
            }]
        );
    }
}
