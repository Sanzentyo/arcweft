//! Production recording through the ordinary bundle-session step boundary.

use super::super::root_command::RootCommandHostResultRoute;
use super::super::{
    BundleSession, BundleSessionStep, BundleStepInput, RuntimeClockStep, RuntimeHostCallErrorKind,
};
use super::model::{
    ROOT_REPLAY_ENGINE_IDENTITY, ROOT_REPLAY_SCHEMA_VERSION, RecordedExternalOutcome,
    RecordedExternalOutcomePositionV1, RecordedExternalOutcomeResultV1,
    RecordedHostCallErrorKindV1, RecordedRootOutcomeV1, RecordedRootTransitionV1,
    RootReplayIdentityV1, RootReplayRecorderV1, RootReplayRecordingError, RootReplayStepError,
    RootReplayTraceV1,
};
use arcweft_core::plan::RuntimeEntryRoles;
use arcweft_core::program_types::RuntimeProgramTypes;
use arcweft_core::root::{RootTransitionOutcome, TransitionSequence};
use arcweft_core::value::{AwbcRuntimeValueSnapshot, RuntimeRetainedValueAdmission};
use std::collections::VecDeque;

struct RootReplayStepCapture {
    admission: RuntimeRetainedValueAdmission,
    pending_events: VecDeque<AwbcRuntimeValueSnapshot>,
    external_outcomes: Vec<RecordedExternalOutcome>,
    added_root_events: usize,
}

impl BundleSession {
    /// Starts a recorder bound to this freshly initialized session.
    pub fn start_root_replay_recording(
        &self,
    ) -> Result<RootReplayRecorderV1, RootReplayRecordingError> {
        RootReplayRecorderV1::start(self)
    }

    /// Executes one ordinary session step and records its typed root behavior.
    /// A failure returns the unexecuted input or the actual executed output;
    /// the recorder retains its state from before the call.
    pub fn step_with_clock_recording(
        &mut self,
        clock: RuntimeClockStep,
        input: BundleStepInput,
        recorder: &mut RootReplayRecorderV1,
    ) -> Result<BundleSessionStep, RootReplayStepError> {
        let capture = match recorder.capture_step(self, &input) {
            Ok(capture) => capture,
            Err(error) => {
                return Err(RootReplayStepError::BeforeStep {
                    clock,
                    input: Box::new(input),
                    error,
                });
            }
        };
        let step = self.step_with_clock(clock, input);
        if let Err(error) = recorder.commit_step(self, capture, &step) {
            return Err(RootReplayStepError::AfterStep {
                step: Box::new(step),
                error,
            });
        }
        Ok(step)
    }
}

impl RootReplayRecorderV1 {
    fn start(session: &BundleSession) -> Result<Self, RootReplayRecordingError> {
        let generation = session.executor.generation();
        let runtime = session
            .runtime_images
            .get(generation)
            .map_err(|error| RootReplayRecordingError::RuntimeInspection {
                message: error.to_string(),
            })?
            .runtime();
        let entry = runtime
            .program
            .entries
            .get(runtime.entry.index())
            .ok_or_else(|| RootReplayRecordingError::RuntimeInspection {
                message: "active AWBC entry is missing".to_owned(),
            })?;
        let RuntimeEntryRoles::Stateful(roles) = &entry.roles else {
            return Err(RootReplayRecordingError::EntryNotStateful);
        };
        let root = session
            .executor
            .product_root_state_snapshot()
            .map_err(|error| RootReplayRecordingError::RootState(Box::new(error)))?
            .ok_or(RootReplayRecordingError::MissingRoot)?;
        let blockers = session
            .executor
            .product_root_save_blockers()
            .ok_or(RootReplayRecordingError::MissingRoot)?;
        if root.next_sequence != TransitionSequence::ZERO
            || root.failure.is_some()
            || blockers.reducer_active
            || blockers.pending_events > 0
            || blockers.pending_commands > 0
        {
            return Err(RootReplayRecordingError::AlreadyAdvanced);
        }
        let initializer_state_digest = root
            .phase
            .state_value()
            .map(|value| {
                roles
                    .state
                    .accepts_payload(
                        RuntimeProgramTypes::Awbc(&runtime.program),
                        value,
                        roles.command_policy.root_limits.schema,
                    )
                    .map_err(|error| RootReplayRecordingError::RuntimeInspection {
                        message: error.to_string(),
                    })
            })
            .transpose()?;
        let entry_kind = entry
            .kind
            .runtime_kind(&runtime.program.strings)
            .ok_or_else(|| RootReplayRecordingError::RuntimeInspection {
                message: "active entry kind is invalid".to_owned(),
            })?;
        let recorder = Self {
            identity: RootReplayIdentityV1 {
                schema_version: ROOT_REPLAY_SCHEMA_VERSION,
                engine_identity: ROOT_REPLAY_ENGINE_IDENTITY.to_owned(),
                artifact: session.active_generation().artifact_identity,
                active_locale: session.active_locale.clone(),
                format_data_identity: session.executor.format_context().data_identity(),
                entry: entry.runtime_id.clone(),
                entry_kind,
                binding: entry.binding,
                state_identity: roles.state.identity.clone(),
                state_layout: roles.state.layout,
                event_identity: roles.event.identity.clone(),
                event_layout: roles.event.layout,
            },
            initializer_state_digest,
            transitions: Vec::new(),
            external_outcomes: Vec::new(),
            roles: roles.as_ref().clone(),
            pending_events: VecDeque::new(),
        };
        recorder.validate_session_identity(session)?;
        Ok(recorder)
    }

    fn capture_step(
        &self,
        session: &BundleSession,
        input: &BundleStepInput,
    ) -> Result<RootReplayStepCapture, RootReplayRecordingError> {
        self.validate_session_identity(session)?;
        let root = session
            .executor
            .product_root_state_snapshot()
            .map_err(|error| RootReplayRecordingError::RootState(Box::new(error)))?
            .ok_or(RootReplayRecordingError::MissingRoot)?;
        let position = RecordedExternalOutcomePositionV1::BeforeTransition(root.next_sequence);
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            session.executor.program_owner(),
            self.roles.command_policy.root_limits.schema,
        );
        admission.validate_metadata(&(&self.identity, self.initializer_state_digest))?;
        admission.validate_metadata_collection(self.transitions.len())?;
        admission.validate_metadata_collection(self.pending_events.len())?;
        admission.validate_metadata_collection(self.external_outcomes.len())?;
        admission.validate_metadata_collection(input.host_call_results.len())?;
        admission.validate_metadata_collection(
            self.external_outcomes
                .len()
                .checked_add(input.host_call_results.len())
                .ok_or_else(|| RootReplayRecordingError::RuntimeInspection {
                    message: "external outcome count overflowed".into(),
                })?,
        )?;
        for transition in &self.transitions {
            transition.admit_retained(&mut admission, self.roles.event.semantic_identity)?;
        }
        for event in &self.pending_events {
            admission.validate_typed_snapshot(event, self.roles.event.semantic_identity)?;
        }
        for outcome in &self.external_outcomes {
            outcome.admit_retained(&mut admission)?;
        }
        let explicit_count = session
            .pending_deferred_root_events
            .len()
            .checked_add(input.root_events.len())
            .ok_or_else(|| RootReplayRecordingError::RuntimeInspection {
                message: "replay event count overflowed".into(),
            })?;
        let successes = input
            .host_call_results
            .iter()
            .filter(|result| result.outcome.is_ok())
            .count();
        let routed_inputs = input
            .host_call_results
            .iter()
            .filter(|result| {
                result.outcome.is_ok()
                    && session
                        .pending_root_command_results
                        .get(&result.id)
                        .is_some_and(|pending| {
                            pending.route == RootCommandHostResultRoute::RootEventPayload
                        })
            })
            .count();
        let maximum_inputs = successes
            .checked_add(routed_inputs)
            .and_then(|count| count.checked_add(explicit_count))
            .ok_or_else(|| RootReplayRecordingError::RuntimeInspection {
                message: "replay input count overflowed".into(),
            })?;
        admission.validate_metadata_collection(maximum_inputs)?;
        let mut inputs = Vec::with_capacity(maximum_inputs);
        for event in session
            .pending_deferred_root_events
            .iter()
            .chain(&input.root_events)
        {
            inputs.push((&event.payload.0, Some(self.roles.event.semantic_identity)));
        }
        let mut routed_count = 0;
        let explicit_pending = self
            .pending_events
            .len()
            .checked_add(explicit_count)
            .ok_or_else(|| RootReplayRecordingError::RuntimeInspection {
                message: "pending replay event count overflowed".into(),
            })?;
        for result in &input.host_call_results {
            let mut root_event_sequence = None;
            let pending = session.pending_root_command_results.get(&result.id);
            match &result.outcome {
                Ok(payload) => {
                    if let Some(pending) = pending {
                        pending.bound.validate_value(&payload.0).map_err(|error| {
                            RootReplayRecordingError::RuntimeInspection {
                                message: format!(
                                    "root-command result violates its bound result type: {error}"
                                ),
                            }
                        })?;
                    }
                    let routed = pending.is_some_and(|pending| {
                        pending.route == RootCommandHostResultRoute::RootEventPayload
                    });
                    let expected = routed.then_some(self.roles.event.semantic_identity);
                    inputs.push((&payload.0, expected));
                    if routed {
                        let offset = explicit_pending
                            .checked_add(routed_count)
                            .and_then(|offset| u64::try_from(offset).ok())
                            .ok_or_else(|| RootReplayRecordingError::RuntimeInspection {
                                message: "pending replay event count does not fit u64".into(),
                            })?;
                        root_event_sequence = Some(
                            root.next_sequence
                                .get()
                                .checked_add(offset)
                                .map(TransitionSequence::from_u64)
                                .ok_or_else(|| RootReplayRecordingError::RuntimeInspection {
                                    message: "recorded root-event sequence overflowed".into(),
                                })?,
                        );
                        inputs.push((&payload.0, expected));
                        routed_count += 1;
                    }
                }
                Err(error) => admission.validate_metadata(&(
                    RecordedHostCallErrorKindV1::from(error.kind),
                    &error.message,
                ))?,
            }
            admission.validate_metadata(&(position, &result.id, root_event_sequence))?;
        }
        let pending_count = explicit_pending.checked_add(routed_count).ok_or_else(|| {
            RootReplayRecordingError::RuntimeInspection {
                message: "pending replay event count overflowed".into(),
            }
        })?;
        admission.validate_metadata_collection(
            self.transitions
                .len()
                .checked_add(pending_count)
                .ok_or_else(|| RootReplayRecordingError::RuntimeInspection {
                    message: "replay transition count overflowed".into(),
                })?,
        )?;
        if !self
            .roles
            .command_policy
            .root_limits
            .permits_pending_events(pending_count)
        {
            return Err(RootReplayRecordingError::RuntimeInspection {
                message: "pending replay events exceed the selected Root limit".into(),
            });
        }
        let mut captured = admission.capture_unrestricted_values(&inputs)?.into_iter();
        let mut pending_events = self.pending_events.clone();
        let prior_pending_events = pending_events.len();
        pending_events.extend(captured.by_ref().take(explicit_count));
        let mut external_outcomes = Vec::with_capacity(input.host_call_results.len());
        for result in &input.host_call_results {
            let mut root_event_sequence = None;
            let outcome = match &result.outcome {
                Ok(_) => {
                    let snapshot = captured
                        .next()
                        .expect("every successful input has an admitted snapshot");
                    if session
                        .pending_root_command_results
                        .get(&result.id)
                        .is_some_and(|pending| {
                            pending.route == RootCommandHostResultRoute::RootEventPayload
                        })
                    {
                        let offset = u64::try_from(pending_events.len())
                            .expect("every routed offset passed borrowed preflight");
                        let sequence = root
                            .next_sequence
                            .get()
                            .checked_add(offset)
                            .map(TransitionSequence::from_u64)
                            .expect("every routed sequence passed borrowed preflight");
                        pending_events.push_back(
                            captured
                                .next()
                                .expect("routed inputs have a second admitted owner"),
                        );
                        root_event_sequence = Some(sequence);
                    }
                    RecordedExternalOutcomeResultV1::Success(snapshot)
                }
                Err(error) => RecordedExternalOutcomeResultV1::Failure {
                    kind: error.kind.into(),
                    message: error.message.clone(),
                },
            };
            external_outcomes.push(RecordedExternalOutcome {
                position,
                request: result.id.clone(),
                outcome,
                root_event_sequence,
            });
        }
        Ok(RootReplayStepCapture {
            admission,
            added_root_events: pending_events.len() - prior_pending_events,
            pending_events,
            external_outcomes,
        })
    }

    fn commit_step(
        &mut self,
        session: &BundleSession,
        mut capture: RootReplayStepCapture,
        step: &BundleSessionStep,
    ) -> Result<(), RootReplayRecordingError> {
        self.validate_session_identity(session)?;
        let mut initializer_state_digest = self.initializer_state_digest;
        for initialization in &step.root_initializations {
            if initializer_state_digest.is_some()
                || initialization.entry != self.identity.entry
                || initialization.binding != self.identity.binding
            {
                return Err(RootReplayRecordingError::InitializationMismatch);
            }
            initializer_state_digest = Some(initialization.state_digest);
        }
        if initializer_state_digest != self.initializer_state_digest {
            capture
                .admission
                .validate_metadata(&initializer_state_digest)?;
        }
        if step.root_transitions.is_empty() && capture.added_root_events > 0 {
            let pending = session
                .executor
                .product_root_save_blockers()
                .map(|blockers| {
                    blockers.pending_events as usize + usize::from(blockers.reducer_active)
                });
            if pending != Some(capture.pending_events.len()) {
                return Err(RootReplayRecordingError::RootIngressRejected);
            }
        }
        if step.root_transitions.len() > capture.pending_events.len() {
            return Err(RootReplayRecordingError::OutcomeCardinality {
                outcomes: step.root_transitions.len(),
                events: capture.pending_events.len(),
            });
        }
        let generation = session.executor.generation();
        let runtime = session
            .runtime_images
            .get(generation)
            .map_err(|error| RootReplayRecordingError::RuntimeInspection {
                message: error.to_string(),
            })?
            .runtime();
        let program = RuntimeProgramTypes::Awbc(&runtime.program);
        capture
            .admission
            .validate_metadata_collection(step.root_transitions.len())?;
        let mut admitted = Vec::with_capacity(step.root_transitions.len());
        for (outcome, event) in step.root_transitions.iter().zip(&capture.pending_events) {
            let expected_sequence = u64::try_from(self.transitions.len() + admitted.len())
                .expect("Vec index fits u64 on supported platforms");
            if outcome.sequence().get() != expected_sequence {
                return Err(RootReplayRecordingError::SequenceMismatch {
                    expected: expected_sequence,
                    actual: outcome.sequence().get(),
                });
            }
            let event_digest = self
                .roles
                .event
                .accepts_snapshot(program, event, self.roles.command_policy.root_limits.schema)
                .map_err(|error| RootReplayRecordingError::InvalidEvent {
                    transition: expected_sequence,
                    message: error.to_string(),
                })?;
            let borrowed = RecordedRootOutcomeV1::from(outcome);
            capture
                .admission
                .validate_metadata(&(outcome.sequence(), event_digest, &borrowed))?;
            admitted.push((outcome.sequence(), event_digest, borrowed));
        }
        let transitions = admitted
            .into_iter()
            .map(
                |(sequence, event_digest, outcome)| RecordedRootTransitionV1 {
                    sequence,
                    event: capture
                        .pending_events
                        .pop_front()
                        .expect("cardinality was checked"),
                    event_digest,
                    outcome: outcome.capture_admitted(),
                },
            );
        let transitions: Vec<_> = transitions.collect();
        if matches!(
            step.root_transitions.last(),
            Some(RootTransitionOutcome::Trapped { .. })
        ) {
            capture.pending_events.clear();
        }
        self.initializer_state_digest = initializer_state_digest;
        self.transitions.extend(transitions);
        self.pending_events = capture.pending_events;
        self.external_outcomes.extend(capture.external_outcomes);
        Ok(())
    }

    fn validate_session_identity(
        &self,
        session: &BundleSession,
    ) -> Result<(), RootReplayRecordingError> {
        if session.executor.generation() != session.active_generation().id
            || session.active_generation().artifact_identity != self.identity.artifact
            || session.active_locale != self.identity.active_locale
            || session.executor.format_context().data_identity()
                != self.identity.format_data_identity
        {
            return Err(RootReplayRecordingError::SessionIdentityChanged);
        }
        let active = session
            .executor
            .product_active_entry_snapshot_identity()
            .map_err(|error| RootReplayRecordingError::RuntimeInspection {
                message: error.to_string(),
            })?
            .ok_or(RootReplayRecordingError::MissingRoot)?;
        let root = session
            .executor
            .product_root_state_snapshot()
            .map_err(|error| RootReplayRecordingError::RootState(Box::new(error)))?
            .ok_or(RootReplayRecordingError::MissingRoot)?;
        if active.id != self.identity.entry
            || active.kind != self.identity.entry_kind
            || active.binding != self.identity.binding
            || root.state_identity != self.identity.state_identity
            || root.state_layout != self.identity.state_layout
            || root.event_identity != self.identity.event_identity
            || root.event_layout != self.identity.event_layout
        {
            return Err(RootReplayRecordingError::SessionIdentityChanged);
        }
        Ok(())
    }

    /// Finishes the trace after every accepted root event has produced an
    /// outcome. External results after the last transition are marked
    /// explicitly rather than attached to a nonexistent transition.
    pub fn finish(mut self) -> Result<RootReplayTraceV1, RootReplayRecordingError> {
        if !self.pending_events.is_empty() {
            return Err(RootReplayRecordingError::PendingEvents {
                count: self.pending_events.len(),
            });
        }
        let transition_count = u64::try_from(self.transitions.len()).unwrap_or(u64::MAX);
        for outcome in &mut self.external_outcomes {
            if matches!(
                outcome.position,
                RecordedExternalOutcomePositionV1::BeforeTransition(sequence)
                    if sequence.get() >= transition_count
            ) {
                outcome.position = RecordedExternalOutcomePositionV1::AfterTransitions;
            }
        }
        Ok(RootReplayTraceV1 {
            identity: self.identity,
            initializer_state_digest: self
                .initializer_state_digest
                .ok_or(RootReplayRecordingError::InitializationPending)?,
            transitions: self.transitions,
            external_outcomes: self.external_outcomes,
        })
    }
}

impl From<RuntimeHostCallErrorKind> for RecordedHostCallErrorKindV1 {
    fn from(value: RuntimeHostCallErrorKind) -> Self {
        match value {
            RuntimeHostCallErrorKind::UnsupportedCapability => Self::UnsupportedCapability,
            RuntimeHostCallErrorKind::Rejected => Self::Rejected,
            RuntimeHostCallErrorKind::Failed => Self::Failed,
        }
    }
}
