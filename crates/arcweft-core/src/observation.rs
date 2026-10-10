use crate::effect::{
    LineEffectRequest, RuntimeAssignmentError, RuntimeCall, RuntimeEvent, RuntimeLog,
};
use crate::task::RuntimeProgramOwner;
use crate::value::{AwbcRuntimeValueSnapshot, RuntimePayload, RuntimeValue};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Retained evaluated observations. Every value is unrestricted before it can
/// enter this cloneable owner; display labels are produced only by display APIs.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(try_from = "RuntimeObservationParts")]
pub struct RuntimeObservationState {
    signals: BTreeMap<String, RuntimePayload>,
    metrics: BTreeMap<String, RuntimePayload>,
    pub logs: Vec<RuntimeLog>,
    pub events: Vec<RuntimeEvent>,
    pub calls: Vec<RuntimeCall>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeObservationParts {
    signals: BTreeMap<String, RuntimePayload>,
    metrics: BTreeMap<String, RuntimePayload>,
    logs: Vec<RuntimeLog>,
    events: Vec<RuntimeEvent>,
    calls: Vec<RuntimeCall>,
}

impl TryFrom<RuntimeObservationParts> for RuntimeObservationState {
    type Error = RuntimeAssignmentError;

    fn try_from(parts: RuntimeObservationParts) -> Result<Self, Self::Error> {
        let state = Self {
            signals: parts.signals,
            metrics: parts.metrics,
            logs: parts.logs,
            events: parts.events,
            calls: parts.calls,
        };
        state.validate_unrestricted()?;
        Ok(state)
    }
}

impl RuntimeObservationState {
    #[must_use]
    pub fn signals(&self) -> &BTreeMap<String, RuntimePayload> {
        &self.signals
    }

    #[must_use]
    pub fn metrics(&self) -> &BTreeMap<String, RuntimePayload> {
        &self.metrics
    }

    /// Visits every retained value through the existing borrowed graph owner.
    pub fn visit_runtime_values<E>(
        &self,
        mut visitor: impl FnMut(&RuntimeValue) -> Result<(), E>,
    ) -> Result<(), E> {
        for value in self.signals.values().chain(self.metrics.values()) {
            crate::value::visit_runtime_value_graph(value.value(), &mut visitor)?;
        }
        Ok(())
    }

    pub(crate) fn validate_unrestricted(&self) -> Result<(), RuntimeAssignmentError> {
        // The ownership query is already transitive; inspect each root once.
        for value in self.signals.values().chain(self.metrics.values()) {
            if !value.value().ownership().permits_copy() {
                return Err(RuntimeAssignmentError::AffineValue);
            }
        }
        Ok(())
    }

    /// RuntimeAssignment's private admitted payload makes this copy safe.
    pub fn record_effect(&mut self, effect: &LineEffectRequest) {
        match effect {
            LineEffectRequest::Log(log) => self.logs.push(log.clone()),
            LineEffectRequest::Call(call) => self.calls.push(call.clone()),
            LineEffectRequest::SignalWrite(write) => {
                self.signals
                    .insert(write.target.clone(), write.payload().clone());
            }
            LineEffectRequest::MetricWrite(write) => {
                self.metrics
                    .insert(write.target.clone(), write.payload().clone());
            }
            LineEffectRequest::EmitEvent(event) => self.events.push(event.clone()),
            LineEffectRequest::Audio(_)
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

/// Inert observation projection used by session saves and native rollback.
/// The existing value snapshot schema owns reconstruction and program binding.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeObservationSaveSnapshot {
    signals: BTreeMap<String, AwbcRuntimeValueSnapshot>,
    metrics: BTreeMap<String, AwbcRuntimeValueSnapshot>,
    logs: Vec<RuntimeLog>,
    events: Vec<RuntimeEvent>,
    calls: Vec<RuntimeCall>,
}

impl RuntimeObservationSaveSnapshot {
    /// Portable AWBC state is rebound through the selected program at restore.
    pub fn from_live(state: &RuntimeObservationState) -> Result<Self, String> {
        Self::capture(state, None)
    }

    /// Native rollback requires the exact immutable program lease at capture.
    pub(crate) fn from_live_for_program(
        state: &RuntimeObservationState,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Self::capture(state, Some(owner))
    }

    fn capture(
        state: &RuntimeObservationState,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Self, String> {
        state
            .validate_unrestricted()
            .map_err(|error| error.to_string())?;
        let capture = |rows: &BTreeMap<String, RuntimePayload>| {
            rows.iter()
                .map(|(name, value)| {
                    let saved = match owner {
                        Some(owner) => AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                            value.value(),
                            owner,
                        ),
                        None => AwbcRuntimeValueSnapshot::from_runtime_value(value.value()),
                    }
                    .map_err(|error| format!("observation '{name}' cannot be saved: {error}"))?;
                    Ok((name.clone(), saved))
                })
                .collect::<Result<BTreeMap<_, _>, String>>()
        };
        Ok(Self {
            signals: capture(&state.signals)?,
            metrics: capture(&state.metrics)?,
            logs: state.logs.clone(),
            events: state.events.clone(),
            calls: state.calls.clone(),
        })
    }

    pub fn into_live_for_program(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<RuntimeObservationState, String> {
        let restore = |rows: BTreeMap<String, AwbcRuntimeValueSnapshot>| {
            rows.into_iter()
                .map(|(name, saved)| {
                    let value = saved
                        .into_runtime_value_for_program(owner)
                        .map_err(|error| {
                            format!("observation '{name}' cannot be restored: {error}")
                        })?;
                    if !value.ownership().permits_copy() {
                        return Err(format!(
                            "observation '{name}' cannot retain an affine value"
                        ));
                    }
                    Ok((name, RuntimePayload::new(value)))
                })
                .collect::<Result<BTreeMap<_, _>, String>>()
        };
        Ok(RuntimeObservationState {
            signals: restore(self.signals)?,
            metrics: restore(self.metrics)?,
            logs: self.logs,
            events: self.events,
            calls: self.calls,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::RuntimeAssignment;
    use crate::value::{RuntimeEntityReference, RuntimeImportedProjectEntityReference};
    use std::sync::Arc;

    #[test]
    fn observation_records_generic_runtime_calls_for_adapters() {
        let mut state = RuntimeObservationState::default();
        state.record_effect(&LineEffectRequest::Call(RuntimeCall {
            callee: "bg".to_owned(),
            args: vec!["@asset:.bg.room".to_owned()],
        }));
        assert_eq!(
            state.calls,
            vec![RuntimeCall {
                callee: "bg".to_owned(),
                args: vec!["@asset:.bg.room".to_owned()],
            }]
        );
    }

    #[test]
    fn typed_observation_save_roundtrip_retains_flow_identity_and_literal_strings() {
        let flow =
            crate::plan::FlowRuntimeId::from_checked_declaration_digest([7; 32], "flow.opening")
                .unwrap();
        let imported = RuntimeImportedProjectEntityReference::try_new(
            arcweft_id::ProjectEntityReferenceFamily::Flow,
            arcweft_id::PublicId::try_new("flow.opening").unwrap(),
            [11; 32],
            [12; 32],
            [13; 32],
            Some(flow.clone()),
        )
        .unwrap();
        let mut state = RuntimeObservationState::default();
        for (name, value) in [
            (
                "signal.local",
                RuntimeValue::EntityRef(RuntimeEntityReference::StructuralFlow(flow)),
            ),
            (
                "signal.imported",
                RuntimeValue::EntityRef(RuntimeEntityReference::ImportedProject(imported.clone())),
            ),
            (
                "signal.literal",
                RuntimeValue::String("@flow.opening".to_owned()),
            ),
            (
                "signal.boolean_text",
                RuntimeValue::String("true".to_owned()),
            ),
            ("signal.boolean", RuntimeValue::Bool(true)),
        ] {
            state.record_effect(&LineEffectRequest::SignalWrite(
                RuntimeAssignment::try_new(name.to_owned(), value).unwrap(),
            ));
        }
        state.record_effect(&LineEffectRequest::MetricWrite(
            RuntimeAssignment::try_new("metric.wide".to_owned(), RuntimeValue::u64(u64::MAX))
                .unwrap(),
        ));
        let bytes = serde_json::to_vec(&RuntimeObservationSaveSnapshot::from_live(&state).unwrap())
            .unwrap();
        let decoded: RuntimeObservationSaveSnapshot = serde_json::from_slice(&bytes).unwrap();
        let owner =
            RuntimeProgramOwner::Awbc(Arc::new(crate::awbc::schema::AwbcProgram::default()));
        let restored = decoded.into_live_for_program(&owner).unwrap();
        assert_eq!(
            serde_json::to_vec(&RuntimeObservationSaveSnapshot::from_live(&restored).unwrap())
                .unwrap(),
            bytes
        );
        assert_eq!(restored, state);
        assert_eq!(
            restored.signals()["signal.literal"].value(),
            &RuntimeValue::String("@flow.opening".to_owned())
        );
        assert_eq!(
            restored.metrics()["metric.wide"].value(),
            &RuntimeValue::u64(u64::MAX)
        );
        let RuntimeValue::EntityRef(RuntimeEntityReference::ImportedProject(value)) =
            restored.signals()["signal.imported"].value()
        else {
            panic!("exact imported entity survives")
        };
        assert_eq!(value.target_generation(), imported.target_generation());
        assert_eq!(value.semantic_identity(), imported.semantic_identity());
        assert_eq!(value.value_type(), imported.value_type());
        assert_eq!(value.flow(), imported.flow());
    }

    #[test]
    fn affine_observation_admission_and_decode_do_not_replace_retained_values() {
        let mut state = RuntimeObservationState::default();
        state.record_effect(&LineEffectRequest::SignalWrite(
            RuntimeAssignment::try_new("signal.current".to_owned(), RuntimeValue::Bool(true))
                .unwrap(),
        ));
        let prior = serde_json::to_vec(&state).unwrap();
        let affine = RuntimeValue::Tuple(vec![
            RuntimeValue::Bool(false),
            RuntimeValue::NeedHandle(crate::tests::reusable_need("need.observation")),
        ]);
        assert_eq!(
            RuntimeAssignment::try_copy("signal.current".to_owned(), &affine),
            Err(RuntimeAssignmentError::AffineValue),
        );
        let parts = serde_json::json!({
            "signals": {"signal.current": RuntimePayload::new(affine)},
            "metrics": {}, "logs": [], "events": [], "calls": [],
        });
        assert!(serde_json::from_value::<RuntimeObservationState>(parts).is_err());
        assert_eq!(serde_json::to_vec(&state).unwrap(), prior);
    }

    #[test]
    fn inert_observation_decode_refuses_nested_affine_values_before_state_publication() {
        let mut prior = RuntimeObservationState::default();
        prior.record_effect(&LineEffectRequest::SignalWrite(
            RuntimeAssignment::try_new("signal.current".to_owned(), RuntimeValue::Bool(true))
                .expect("unrestricted initial signal"),
        ));
        let prior_bytes = serde_json::to_vec(&prior).expect("prior state serializes");
        let affine = RuntimeValue::Tuple(vec![
            RuntimeValue::Bool(false),
            RuntimeValue::NeedHandle(crate::tests::reusable_need("need.observation.saved")),
        ]);
        let saved_value = AwbcRuntimeValueSnapshot::from_runtime_value(&affine)
            .expect("inert value schema can represent an affine value");
        drop(affine);
        let saved = RuntimeObservationSaveSnapshot {
            signals: BTreeMap::from([("signal.current".to_owned(), saved_value)]),
            ..RuntimeObservationSaveSnapshot::default()
        };
        let bytes = serde_json::to_vec(&saved).expect("inert observation serializes");
        let decoded: RuntimeObservationSaveSnapshot =
            serde_json::from_slice(&bytes).expect("decode creates only an inert DTO");
        let owner =
            RuntimeProgramOwner::Awbc(Arc::new(crate::awbc::schema::AwbcProgram::default()));
        assert_eq!(
            decoded.into_live_for_program(&owner),
            Err("observation 'signal.current' cannot retain an affine value".to_owned()),
        );
        assert_eq!(serde_json::to_vec(&prior).unwrap(), prior_bytes);
    }
}
