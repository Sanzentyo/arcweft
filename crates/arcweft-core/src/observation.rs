use crate::effect::{LineEffectRequest, RuntimeEvent, RuntimeLog};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeObservationState {
    pub signals: BTreeMap<String, String>,
    pub metrics: BTreeMap<String, String>,
    pub logs: Vec<RuntimeLog>,
    pub events: Vec<RuntimeEvent>,
}

impl RuntimeObservationState {
    pub fn record_effect(&mut self, effect: &LineEffectRequest) {
        match effect {
            LineEffectRequest::Log(log) => self.logs.push(log.clone()),
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
            | LineEffectRequest::Wait(_)
            | LineEffectRequest::Call(_)
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
