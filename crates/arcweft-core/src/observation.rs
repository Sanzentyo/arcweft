use crate::effect::{LineEffectRequest, RuntimeCall, RuntimeEvent, RuntimeLog};
use crate::task::RuntimeProgramOwner;
use crate::value::{AwbcRuntimeValueSnapshot, RuntimePayload, RuntimeValue};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

impl<P> RuntimeObservationState<P> {
    /// Names and diagnostic rows share the enclosing resource/work bank.
    fn admit_metadata(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        for count in [
            self.signals.len(),
            self.metrics.len(),
            self.logs.len(),
            self.events.len(),
            self.calls.len(),
        ] {
            admission.validate_metadata_collection(count)?;
        }
        for name in self.signals.keys().chain(self.metrics.keys()) {
            admission.validate_metadata_string(name)?;
        }
        for log in &self.logs {
            admission.validate_metadata_string(&log.level)?;
            admission.validate_metadata_string(&log.message)?;
        }
        for event in &self.events {
            admission.validate_metadata_string(&event.event)?;
        }
        for fields in self
            .logs
            .iter()
            .map(|row| row.fields.as_slice())
            .chain(self.events.iter().map(|row| row.fields.as_slice()))
        {
            admission.validate_metadata_collection(fields.len())?;
            for field in fields {
                admission.validate_metadata_string(&field.name)?;
                admission.validate_metadata_string(&field.value)?;
            }
        }
        for call in &self.calls {
            admission.validate_metadata_string(&call.callee)?;
            admission.validate_metadata_collection(call.args.len())?;
            for argument in &call.args {
                admission.validate_metadata_string(argument)?;
            }
        }
        Ok(())
    }

    pub(crate) fn admit_values(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        view: impl Fn(&P) -> crate::value::RuntimeBorrowedValue<'_>,
        operation: &'static str,
    ) -> Result<(), String> {
        self.admit_metadata(admission)
            .map_err(|error| error.to_string())?;
        for (name, value) in self.signals.iter().chain(&self.metrics) {
            admission
                .validate_borrowed(view(value))
                .map_err(|error| match error {
                    crate::value::AwbcRuntimeValueSnapshotError::AffineValue => {
                        format!("observation '{name}' cannot retain an affine value")
                    }
                    error => format!("observation '{name}' cannot be {operation}: {error}"),
                })?;
        }
        Ok(())
    }
}

/// Retained evaluated observations use one live/inert storage grammar.
/// Live values are unrestricted before entering this cloneable owner.
/// Decoding creates only the inert specialization.
///
/// ```compile_fail
/// use arcweft_core::observation::RuntimeObservationState;
/// let _: RuntimeObservationState = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(deserialize = "P: crate::value::InertRuntimeValueStorage")
)]
pub struct RuntimeObservationState<P = RuntimePayload> {
    signals: BTreeMap<String, P>,
    metrics: BTreeMap<String, P>,
    pub logs: Vec<RuntimeLog>,
    pub events: Vec<RuntimeEvent>,
    pub calls: Vec<RuntimeCall>,
}

impl<P> Default for RuntimeObservationState<P> {
    fn default() -> Self {
        Self {
            signals: BTreeMap::new(),
            metrics: BTreeMap::new(),
            logs: Vec::new(),
            events: Vec::new(),
            calls: Vec::new(),
        }
    }
}
impl RuntimeObservationState {
    /// Preflights borrowed live observations in the enclosing retained group.
    pub(crate) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), String> {
        self.admit_values(admission, |value| value.value().into(), "saved")
    }

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

/// Inert storage of the same observation owner, bound on restoration.
pub type RuntimeObservationSaveSnapshot = RuntimeObservationState<AwbcRuntimeValueSnapshot>;

impl RuntimeObservationSaveSnapshot {
    /// Borrows all inert observation values before any live construction.
    pub fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), String> {
        self.admit_values(admission, |value| value.into(), "restored")
    }
    pub(crate) fn visit_saved_value_roots<E>(
        &self,
        visit: &mut impl FnMut(&AwbcRuntimeValueSnapshot) -> Result<(), E>,
    ) -> Result<(), E> {
        for value in self.signals.values().chain(self.metrics.values()) {
            visit(value)?;
        }
        Ok(())
    }

    /// Captures observations through their exact immutable program lease.
    /// Restore may bind the inert result to an independently checked artifact.
    pub fn from_live_for_program(
        state: &RuntimeObservationState,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Self::capture(state, owner)
    }

    fn capture(
        state: &RuntimeObservationState,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, String> {
        state.admit_retained(
            &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                crate::entry::RuntimeSchemaLimits::engine_default(),
            ),
        )?;
        Self::from_admitted_live_for_program(state, owner)
    }

    pub(crate) fn from_admitted_live_for_program(
        state: &RuntimeObservationState,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, String> {
        let capture = |rows: &BTreeMap<String, RuntimePayload>| {
            rows.iter()
                .map(|(name, value)| {
                    let saved =
                        AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value.value(), owner)
                            .map_err(|error| {
                                format!("observation '{name}' cannot be saved: {error}")
                            })?;
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
        self.admit_retained(
            &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                crate::entry::RuntimeSchemaLimits::engine_default(),
            ),
        )?;
        self.into_admitted_live_for_program(owner)
    }

    pub(crate) fn into_admitted_live_for_program<'a>(
        self,
        owner: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> Result<RuntimeObservationState, String> {
        let owner = owner.into();
        let restore = |rows: BTreeMap<String, AwbcRuntimeValueSnapshot>| {
            rows.into_iter()
                .map(|(name, saved)| {
                    let value = saved.into_admitted_runtime_value(owner).map_err(|error| {
                        format!("observation '{name}' cannot be restored: {error}")
                    })?;
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
    use crate::effect::{RuntimeAssignment, RuntimeAssignmentError};
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
        let owner =
            RuntimeProgramOwner::Awbc(Arc::new(crate::awbc::schema::AwbcProgram::default()));
        let bytes = serde_json::to_vec(
            &RuntimeObservationSaveSnapshot::from_live_for_program(&state, &owner).unwrap(),
        )
        .unwrap();
        let decoded: RuntimeObservationSaveSnapshot = serde_json::from_slice(&bytes).unwrap();
        let restored = decoded.into_live_for_program(&owner).unwrap();
        assert_eq!(
            serde_json::to_vec(
                &RuntimeObservationSaveSnapshot::from_live_for_program(&restored, &owner).unwrap()
            )
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
            "signals": {"signal.current": AwbcRuntimeValueSnapshot::from_standalone_value(&affine).unwrap()},
            "metrics": {}, "logs": [], "events": [], "calls": [],
        });
        let saved: RuntimeObservationSaveSnapshot = serde_json::from_value(parts).unwrap();
        let owner =
            RuntimeProgramOwner::Awbc(Arc::new(crate::awbc::schema::AwbcProgram::default()));
        assert!(
            saved
                .admit_retained(
                    &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                        owner,
                        crate::entry::RuntimeSchemaLimits::engine_default(),
                    )
                )
                .is_err()
        );
        assert_eq!(serde_json::to_vec(&state).unwrap(), prior);
    }

    #[test]
    fn observations_share_metadata_and_value_resources_before_construction() {
        let saved = RuntimeObservationSaveSnapshot {
            signals: BTreeMap::from([("first".into(), AwbcRuntimeValueSnapshot::Unit)]),
            metrics: BTreeMap::from([("second".into(), AwbcRuntimeValueSnapshot::Unit)]),
            ..RuntimeObservationSaveSnapshot::default()
        };
        let before = serde_json::to_vec(&saved).unwrap();
        let owner =
            RuntimeProgramOwner::Awbc(Arc::new(crate::awbc::schema::AwbcProgram::default()));
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits {
                max_nodes: 2,
                ..crate::entry::RuntimeSchemaLimits::engine_default()
            },
        );
        saved.admit_retained(&mut admission).unwrap();
        assert!(
            admission
                .validate_snapshot(&AwbcRuntimeValueSnapshot::Unit)
                .is_err()
        );
        let mut too_small = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits {
                max_nodes: 1,
                ..crate::entry::RuntimeSchemaLimits::engine_default()
            },
        );
        assert!(saved.admit_retained(&mut too_small).is_err());
        assert_eq!(serde_json::to_vec(&saved).unwrap(), before);
        let mut metadata = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner,
            crate::entry::RuntimeSchemaLimits {
                max_string_bytes: 5,
                ..crate::entry::RuntimeSchemaLimits::engine_default()
            },
        );
        assert!(saved.admit_retained(&mut metadata).is_err());
        assert!(
            metadata
                .validate_snapshot(&AwbcRuntimeValueSnapshot::Unit)
                .is_err()
        );
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
        let saved_value = AwbcRuntimeValueSnapshot::from_standalone_value(&affine)
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
