//! Production recording through the ordinary bundle-session step boundary.

use super::super::root_command::RootCommandHostResultRoute;
use super::super::{
    BundleSession, BundleSessionStep, BundleStepInput, RuntimeClockStep, RuntimeHostCallError,
    RuntimeHostCallErrorKind, RuntimePayload,
};
use super::model::{
    ROOT_REPLAY_ENGINE_IDENTITY, ROOT_REPLAY_SCHEMA_VERSION, RecordedExternalOutcome,
    RecordedExternalOutcomePositionV1, RecordedExternalOutcomeResultV1,
    RecordedHostCallErrorKindV1, RecordedRootOutcomeV1, RecordedRootTransitionV1,
    RootReplayRecorderV1, RootReplayRecordingError, RootReplayTraceV1,
};
use arcweft_core::plan::RuntimeEntryRoles;
use arcweft_core::root::{RootTransitionOutcome, TransitionSequence};
use std::collections::VecDeque;

#[derive(Clone, Debug)]
struct RootReplayStepCapture {
    pending_events: VecDeque<RuntimePayload>,
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
    pub fn step_with_clock_recording(
        &mut self,
        clock: RuntimeClockStep,
        input: BundleStepInput,
        recorder: &mut RootReplayRecorderV1,
    ) -> Result<BundleSessionStep, RootReplayRecordingError> {
        let capture = recorder.capture_step(self, &input)?;
        let step = self.step_with_clock(clock, input);
        recorder.commit_step(self, capture, &step)?;
        Ok(step)
    }
}

impl RootReplayRecorderV1 {
    fn start(session: &BundleSession) -> Result<Self, RootReplayRecordingError> {
        let generation = session
            .current_fiber_generation()
            .ok_or(RootReplayRecordingError::NoActiveGeneration)?;
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
            .ok_or(RootReplayRecordingError::MissingRoot)?;
        if root.next_sequence != TransitionSequence::ZERO {
            return Err(RootReplayRecordingError::AlreadyAdvanced);
        }
        let initializer_state_digest = roles
            .state
            .schema
            .validate_payload(&root.value, roles.command_policy.root_limits.schema)
            .map_err(|error| RootReplayRecordingError::RuntimeInspection {
                message: error.to_string(),
            })?;
        let entry_kind = entry
            .kind
            .runtime_kind(&runtime.program.strings)
            .ok_or_else(|| RootReplayRecordingError::RuntimeInspection {
                message: "active entry kind is invalid".to_owned(),
            })?;
        Ok(Self {
            trace: RootReplayTraceV1 {
                schema_version: ROOT_REPLAY_SCHEMA_VERSION,
                engine_identity: ROOT_REPLAY_ENGINE_IDENTITY.to_owned(),
                artifact: session.active_artifact_identity,
                entry: entry.runtime_id.clone(),
                entry_kind,
                binding: entry.binding,
                state_identity: roles.state.identity.clone(),
                state_layout: roles.state.layout,
                event_identity: roles.event.identity.clone(),
                event_layout: roles.event.layout,
                initializer_state_digest,
                transitions: Vec::new(),
                external_outcomes: Vec::new(),
            },
            roles: roles.as_ref().clone(),
            pending_events: VecDeque::new(),
        })
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
            .ok_or(RootReplayRecordingError::MissingRoot)?;
        let position = RecordedExternalOutcomePositionV1::BeforeTransition(root.next_sequence);
        let mut pending_events = self.pending_events.clone();
        let prior_pending_events = pending_events.len();
        pending_events.extend(
            session
                .pending_deferred_root_events
                .iter()
                .map(|event| event.payload.clone()),
        );
        pending_events.extend(input.root_events.iter().map(|event| event.payload.clone()));
        let mut external_outcomes = Vec::with_capacity(input.host_call_results.len());
        for result in &input.host_call_results {
            let route = session
                .pending_root_command_results
                .get(&result.id)
                .copied();
            let root_event_sequence = match (&route, &result.outcome) {
                (Some(RootCommandHostResultRoute::RootEventPayload), Ok(payload)) => {
                    let offset = u64::try_from(pending_events.len()).map_err(|_| {
                        RootReplayRecordingError::RuntimeInspection {
                            message: "pending replay event count does not fit u64".to_owned(),
                        }
                    })?;
                    let sequence = root
                        .next_sequence
                        .get()
                        .checked_add(offset)
                        .map(TransitionSequence::from_u64)
                        .ok_or_else(|| RootReplayRecordingError::RuntimeInspection {
                            message: "recorded root-event sequence overflowed".to_owned(),
                        })?;
                    pending_events.push_back(payload.clone());
                    Some(sequence)
                }
                _ => None,
            };
            external_outcomes.push(RecordedExternalOutcome {
                position,
                request: result.id.clone(),
                outcome: RecordedExternalOutcomeResultV1::from(&result.outcome),
                root_event_sequence,
            });
        }
        Ok(RootReplayStepCapture {
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
        if step.root_transitions.is_empty() && capture.added_root_events > 0 {
            return Err(RootReplayRecordingError::RootIngressRejected);
        }
        if step.root_transitions.len() > capture.pending_events.len() {
            return Err(RootReplayRecordingError::OutcomeCardinality {
                outcomes: step.root_transitions.len(),
                events: capture.pending_events.len(),
            });
        }
        for outcome in &step.root_transitions {
            let event = capture
                .pending_events
                .pop_front()
                .expect("cardinality was checked");
            let expected_sequence = u64::try_from(self.trace.transitions.len())
                .expect("Vec index fits u64 on supported platforms");
            if outcome.sequence().get() != expected_sequence {
                return Err(RootReplayRecordingError::SequenceMismatch {
                    expected: expected_sequence,
                    actual: outcome.sequence().get(),
                });
            }
            let event = RuntimePayload(event.0);
            let event_digest = self
                .roles
                .event
                .schema
                .validate_payload(&event, self.roles.command_policy.root_limits.schema)
                .map_err(|error| RootReplayRecordingError::InvalidEvent {
                    transition: expected_sequence,
                    message: error.to_string(),
                })?;
            self.trace.transitions.push(RecordedRootTransitionV1 {
                sequence: outcome.sequence(),
                event,
                event_digest,
                outcome: RecordedRootOutcomeV1::from(outcome),
            });
        }
        if matches!(
            step.root_transitions.last(),
            Some(RootTransitionOutcome::Trapped { .. })
        ) {
            capture.pending_events.clear();
        }
        self.pending_events = capture.pending_events;
        self.trace
            .external_outcomes
            .extend(capture.external_outcomes);
        Ok(())
    }

    fn validate_session_identity(
        &self,
        session: &BundleSession,
    ) -> Result<(), RootReplayRecordingError> {
        if session.active_artifact_identity != self.trace.artifact {
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
            .ok_or(RootReplayRecordingError::MissingRoot)?;
        if active.id != self.trace.entry
            || active.kind != self.trace.entry_kind
            || active.binding != self.trace.binding
            || root.state_identity != self.trace.state_identity
            || root.state_layout != self.trace.state_layout
            || root.event_identity != self.trace.event_identity
            || root.event_layout != self.trace.event_layout
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
        let transition_count = u64::try_from(self.trace.transitions.len()).unwrap_or(u64::MAX);
        for outcome in &mut self.trace.external_outcomes {
            if matches!(
                outcome.position,
                RecordedExternalOutcomePositionV1::BeforeTransition(sequence)
                    if sequence.get() >= transition_count
            ) {
                outcome.position = RecordedExternalOutcomePositionV1::AfterTransitions;
            }
        }
        Ok(self.trace)
    }
}

impl From<&RootTransitionOutcome> for RecordedRootOutcomeV1 {
    fn from(value: &RootTransitionOutcome) -> Self {
        match value {
            RootTransitionOutcome::Committed {
                state_digest,
                command_digests,
                ..
            } => Self::Committed {
                state_digest: *state_digest,
                command_digests: command_digests.clone(),
            },
            RootTransitionOutcome::Rejected { error_digest, .. } => Self::Rejected {
                error_digest: *error_digest,
            },
            RootTransitionOutcome::Trapped { failure_digest, .. } => Self::Trapped {
                failure_digest: *failure_digest,
            },
        }
    }
}

impl From<&Result<RuntimePayload, RuntimeHostCallError>> for RecordedExternalOutcomeResultV1 {
    fn from(value: &Result<RuntimePayload, RuntimeHostCallError>) -> Self {
        match value {
            Ok(payload) => Self::Success(payload.clone()),
            Err(error) => Self::Failure {
                kind: error.kind.into(),
                message: error.message.clone(),
            },
        }
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
