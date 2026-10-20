//! Deterministic root-state replay execution over the ordinary bundle-session boundary.
//!
//! Replay verifies the exact selected artifact and entry contract before
//! constructing a session. Recorded events still enter through ordinary live
//! ingress, so core remains the sole owner of transition sequencing.

use super::super::construction::selected_awbc_entry;
use super::super::root_command::RootCommandHostResultRoute;
use super::super::{
    ArcweftBundle, BundleSession, BundleSessionArtifactIdentity, BundleSessionError,
    BundleSessionOptions, BundleStepInput, BundleView, ReadBudget, RootEventInput,
    RuntimeClockStep, RuntimeHostCallError, RuntimeHostCallErrorKind, RuntimeHostCallResult,
};
use super::model::{
    ROOT_REPLAY_ENGINE_IDENTITY, ROOT_REPLAY_SCHEMA_VERSION, RecordedExternalOutcome,
    RecordedExternalOutcomePositionV1, RecordedExternalOutcomeResultV1,
    RecordedHostCallErrorKindV1, RecordedRootOutcomeV1, RecordedRootTransitionV1, RootReplayError,
    RootReplayReportV1, RootReplayTraceV1,
};
use arcweft_core::entry::{RuntimeStatefulEntryRoles, RuntimeValueDigest};
use arcweft_core::plan::RuntimeEntryRoles;
use arcweft_core::program_types::RuntimeProgramTypes;
use arcweft_core::root::{RootStateSnapshotV1, RootTransitionOutcome, TransitionSequence};
use arcweft_core::value::RuntimeFormatDataIdentity;
use arcweft_core::{task::RuntimeProgramOwner, value::RuntimeRetainedValueAdmission};
use std::collections::BTreeSet;

impl BundleSession {
    /// Replays a logical typed bundle without publishing recorded host requests.
    pub fn replay_root_trace(
        bundle: &ArcweftBundle,
        options: BundleSessionOptions,
        trace: &RootReplayTraceV1,
    ) -> Result<RootReplayReportV1, RootReplayError> {
        let identity =
            bundle
                .logical_identity()
                .map_err(|error| RootReplayError::ArtifactInspection {
                    message: error.to_string(),
                })?;
        let mut replay_options = options.clone();
        replay_options.active_locale = Some(trace.identity.active_locale.clone());
        let start_options = replay_options.clone();
        replay_root_trace_with(
            bundle,
            &options,
            trace,
            BundleSessionArtifactIdentity::LogicalBundle { identity },
            || BundleSession::new(bundle, start_options),
        )
    }

    /// Replays an AWFB container while binding the trace to its exact identity.
    pub fn replay_root_trace_from_awfb_bytes(
        bytes: &[u8],
        options: BundleSessionOptions,
        trace: &RootReplayTraceV1,
    ) -> Result<RootReplayReportV1, RootReplayError> {
        let view = BundleView::parse(bytes, ReadBudget::default()).map_err(|error| {
            RootReplayError::ArtifactInspection {
                message: error.to_string(),
            }
        })?;
        let artifact = BundleSessionArtifactIdentity::AwfbContainer {
            identity: view.artifact_identity(),
        };
        let bundle = ArcweftBundle::from_awfb_slice_with_resource_types(
            bytes,
            options.engine_resource_types.as_ref(),
        )
        .map_err(|error| RootReplayError::ArtifactInspection {
            message: error.to_string(),
        })?;
        let mut replay_options = options.clone();
        replay_options.active_locale = Some(trace.identity.active_locale.clone());
        let start_options = replay_options.clone();
        replay_root_trace_with(&bundle, &options, trace, artifact, || {
            BundleSession::from_awfb_bytes(bytes, start_options)
        })
    }
}

fn replay_root_trace_with(
    bundle: &ArcweftBundle,
    options: &BundleSessionOptions,
    trace: &RootReplayTraceV1,
    artifact: BundleSessionArtifactIdentity,
    start: impl FnOnce() -> Result<BundleSession, BundleSessionError>,
) -> Result<RootReplayReportV1, RootReplayError> {
    let roles = preflight(bundle, options, trace, artifact)?;
    let program = RuntimeProgramTypes::Awbc(bundle.product_awbc_program());
    validate_trace_shape(trace)?;
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&bundle.product_awbc.program)),
        roles.command_policy.root_limits.schema,
    );
    admission
        .validate_metadata_collection(trace.transitions.len())
        .map_err(|error| RootReplayError::ArtifactInspection {
            message: error.to_string(),
        })?;
    admission
        .validate_metadata_collection(trace.external_outcomes.len())
        .map_err(|error| RootReplayError::ArtifactInspection {
            message: error.to_string(),
        })?;
    for transition in &trace.transitions {
        admission
            .validate_typed_snapshot(&transition.event, roles.event.semantic_identity)
            .map_err(|error| RootReplayError::EventDivergence {
                entry: trace.identity.entry.canonical_label(),
                transition: transition.sequence.get(),
                message: error.to_string(),
            })?;
    }
    for (index, external) in trace.external_outcomes.iter().enumerate() {
        admission
            .validate_metadata_string(&external.request.0)
            .map_err(|error| external_error(trace, index, &error.to_string()))?;
        match &external.outcome {
            RecordedExternalOutcomeResultV1::Success(value) => {
                admission
                    .validate_snapshot(value)
                    .map_err(|error| external_error(trace, index, &error.to_string()))?
            }
            RecordedExternalOutcomeResultV1::Failure { message, .. } => admission
                .validate_metadata_string(message)
                .map_err(|error| external_error(trace, index, &error.to_string()))?,
        }
    }

    let mut session = start().map_err(|error| RootReplayError::SessionStart {
        message: error.to_string(),
    })?;
    let mut initialization_steps = 0_u64;
    let mut initialization_host_requests = 0_usize;
    let initial = loop {
        let initial = session
            .executor
            .product_root_state_snapshot()
            .ok_or(RootReplayError::MissingRoot)?;
        if initial.phase.state_value().is_some() {
            break initial;
        }
        initialization_steps = initialization_steps.checked_add(1).ok_or_else(|| {
            RootReplayError::InvalidInitializerState {
                message: "initializer replay step counter overflowed".to_owned(),
            }
        })?;
        let clock = RuntimeClockStep::from_millis(initialization_steps, 1)
            .map_err(|_| RootReplayError::UnexpectedTransition)?;
        let step = session.step_with_clock(clock, BundleStepInput::default());
        if step.stats.executed_ops == 0
            || matches!(
                step.fiber_status,
                arcweft_core::engine::FlowFiberStatus::Failed(_)
            )
        {
            return Err(RootReplayError::InvalidInitializerState {
                message: "initializer did not make executable progress".to_owned(),
            });
        }
        let initialized = session
            .executor
            .product_root_state_snapshot()
            .is_some_and(|root| root.phase.state_value().is_some());
        if !step.root_transitions.is_empty()
            || (!initialized && !step.requested_host_calls.is_empty())
        {
            return Err(RootReplayError::UnexpectedTransition);
        }
        initialization_host_requests = initialization_host_requests
            .checked_add(step.requested_host_calls.len())
            .ok_or(RootReplayError::UnexpectedTransition)?;
        for outcome in &step.root_initializations {
            if outcome.entry != trace.identity.entry
                || outcome.binding != trace.identity.binding
                || outcome.state_digest != trace.initializer_state_digest
            {
                return Err(RootReplayError::InitializerDigestMismatch);
            }
        }
    };
    let durable_state_digest = roles
        .state
        .accepts_payload(
            program,
            initial.phase.state_value().ok_or_else(|| {
                RootReplayError::InvalidInitializerState {
                    message: "root initialization is pending".to_owned(),
                }
            })?,
            roles.command_policy.root_limits.schema,
        )
        .map_err(|error| RootReplayError::InvalidInitializerState {
            message: error.to_string(),
        })?;
    if durable_state_digest != trace.initializer_state_digest {
        return Err(RootReplayError::InitializerDigestMismatch);
    }

    let entry_label = trace.identity.entry.public_label().into_string();
    let mut progress = ReplayProgress::new(trace, durable_state_digest);
    progress.replay_step = initialization_steps;
    progress.suppressed_host_requests = initialization_host_requests;
    while progress.transition_index < trace.transitions.len() {
        let batch =
            progress.prepare_transition_batch(&session, trace, &roles, &entry_label, program)?;
        progress.execute_transition_batch(
            &mut session,
            trace,
            &roles,
            &entry_label,
            batch,
            program,
        )?;
    }
    progress.inject_after_transitions(
        &mut session,
        trace,
        roles.command_policy.root_limits.schema,
    )?;

    Ok(RootReplayReportV1 {
        entry: trace.identity.entry.clone(),
        transitions_verified: progress.transition_index,
        external_outcomes_injected: progress.external_outcomes_injected,
        suppressed_host_requests: progress.suppressed_host_requests,
        terminal_trap: progress.terminal_trap,
    })
}

struct ReplayProgress {
    transition_index: usize,
    queued_through: Option<usize>,
    used_external: Vec<bool>,
    external_outcomes_injected: usize,
    suppressed_host_requests: usize,
    replay_step: u64,
    terminal_trap: bool,
    durable_state_digest: RuntimeValueDigest,
}

struct PreparedReplayBatch {
    expected_sequence: TransitionSequence,
    batch_end: usize,
    root_events: Vec<RootEventInput>,
    host_results: Vec<RuntimeHostCallResult>,
}

impl ReplayProgress {
    fn new(trace: &RootReplayTraceV1, durable_state_digest: RuntimeValueDigest) -> Self {
        Self {
            transition_index: 0,
            queued_through: None,
            used_external: vec![false; trace.external_outcomes.len()],
            external_outcomes_injected: 0,
            suppressed_host_requests: 0,
            replay_step: 0,
            terminal_trap: false,
            durable_state_digest,
        }
    }

    fn prepare_transition_batch(
        &mut self,
        session: &BundleSession,
        trace: &RootReplayTraceV1,
        roles: &RuntimeStatefulEntryRoles,
        entry_label: &str,
        program: RuntimeProgramTypes<'_>,
    ) -> Result<PreparedReplayBatch, RootReplayError> {
        let expected_sequence = trace.transitions[self.transition_index].sequence;
        let existing_queue_end = self
            .queued_through
            .filter(|end| *end >= self.transition_index);
        let external_indices = external_indices_at(
            trace,
            &self.used_external,
            RecordedExternalOutcomePositionV1::BeforeTransition(expected_sequence),
        );
        let (host_results, supplied_sequences) = prepare_external_batch(
            session,
            trace,
            &external_indices,
            roles.command_policy.root_limits.schema,
        )?;
        self.mark_external_used(&external_indices)?;

        let mut batch_end = existing_queue_end.unwrap_or(self.transition_index);
        let mut explicit_end = existing_queue_end
            .is_none()
            .then_some(self.transition_index);
        if let Some(first_supplied) = supplied_sequences.first().copied() {
            let first_index = trace_index_for_sequence(trace, first_supplied)?;
            let expected_first = existing_queue_end.map_or(self.transition_index, |end| end + 1);
            if first_index < expected_first {
                return Err(external_error(
                    trace,
                    external_indices[0],
                    "root-event result overlaps events already queued in core",
                ));
            }
            explicit_end = (first_index > expected_first).then_some(first_index - 1);
            batch_end =
                consecutive_external_batch_end(trace, &external_indices, &supplied_sequences)?;
        }

        let explicit_start = existing_queue_end.map_or(self.transition_index, |end| end + 1);
        validate_event_slice(
            trace,
            self.transition_index,
            batch_end,
            roles,
            entry_label,
            program,
        )?;
        validate_supplied_events(
            trace,
            &external_indices,
            &supplied_sequences,
            roles,
            entry_label,
            program,
        )?;
        let mut root_events = Vec::new();
        if let Some(end) = explicit_end {
            let owner = session.executor.program_owner();
            for transition in &trace.transitions[explicit_start..=end] {
                let value = transition
                    .event
                    .copy_unrestricted_typed_runtime_value_for_program_with_limits(
                        &owner,
                        roles.event.semantic_identity,
                        roles.command_policy.root_limits.schema,
                    )
                    .map_err(|error| RootReplayError::EventDivergence {
                        entry: entry_label.to_owned(),
                        transition: transition.sequence.get(),
                        message: error.to_string(),
                    })?;
                root_events.push(RootEventInput::new(value.into()));
            }
        }
        Ok(PreparedReplayBatch {
            expected_sequence,
            batch_end,
            root_events,
            host_results,
        })
    }

    fn execute_transition_batch(
        &mut self,
        session: &mut BundleSession,
        trace: &RootReplayTraceV1,
        roles: &RuntimeStatefulEntryRoles,
        entry_label: &str,
        batch: PreparedReplayBatch,
        program: RuntimeProgramTypes<'_>,
    ) -> Result<(), RootReplayError> {
        self.replay_step =
            self.replay_step
                .checked_add(1)
                .ok_or_else(|| RootReplayError::OutcomeDivergence {
                    entry: entry_label.to_owned(),
                    transition: batch.expected_sequence.get(),
                    message: "replay step counter overflowed".to_owned(),
                })?;
        let before = session
            .executor
            .product_root_state_snapshot()
            .ok_or(RootReplayError::MissingRoot)?;
        let clock = RuntimeClockStep::from_millis(self.replay_step, 1)
            .map_err(|_| RootReplayError::UnexpectedTransition)?;
        let step = session.step_with_clock(
            clock,
            BundleStepInput {
                root_events: batch.root_events,
                host_call_results: batch.host_results,
                ..BundleStepInput::default()
            },
        );
        self.add_suppressed_requests(
            step.requested_host_calls.len(),
            entry_label,
            batch.expected_sequence,
        )?;
        if step.root_transitions.is_empty() {
            return Err(RootReplayError::MissingOutcome {
                transition: batch.expected_sequence.get(),
            });
        }
        let expected_batch_len = batch.batch_end - self.transition_index + 1;
        if step.root_transitions.len() > expected_batch_len {
            return Err(RootReplayError::UnexpectedTransition);
        }
        let after = session
            .executor
            .product_root_state_snapshot()
            .ok_or(RootReplayError::MissingRoot)?;
        self.durable_state_digest = compare_outcomes(
            &trace.transitions
                [self.transition_index..self.transition_index + step.root_transitions.len()],
            &step.root_transitions,
            &before,
            &after,
            self.durable_state_digest,
            roles,
            entry_label,
            program,
        )?;
        self.terminal_trap = matches!(
            step.root_transitions.last(),
            Some(RootTransitionOutcome::Trapped { .. })
        );
        self.transition_index += step.root_transitions.len();
        self.queued_through = if self.terminal_trap || self.transition_index > batch.batch_end {
            None
        } else {
            Some(batch.batch_end)
        };
        if self.terminal_trap && self.transition_index != trace.transitions.len() {
            return Err(RootReplayError::NonTerminalTrap {
                transition: trace.transitions[self.transition_index - 1].sequence.get(),
            });
        }
        Ok(())
    }

    fn inject_after_transitions(
        &mut self,
        session: &mut BundleSession,
        trace: &RootReplayTraceV1,
        limits: arcweft_core::entry::RuntimeSchemaLimits,
    ) -> Result<(), RootReplayError> {
        let indices = external_indices_at(
            trace,
            &self.used_external,
            RecordedExternalOutcomePositionV1::AfterTransitions,
        );
        if !indices.is_empty() {
            let (host_results, supplied_sequences) =
                prepare_external_batch(session, trace, &indices, limits)?;
            if !supplied_sequences.is_empty() {
                return Err(external_error(
                    trace,
                    indices[0],
                    "an after-transitions external outcome cannot produce another root event",
                ));
            }
            self.mark_external_used(&indices)?;
            self.replay_step = self
                .replay_step
                .checked_add(1)
                .ok_or(RootReplayError::UnexpectedTransition)?;
            let clock = RuntimeClockStep::from_millis(self.replay_step, 1)
                .map_err(|_| RootReplayError::UnexpectedTransition)?;
            let step = session.step_with_clock(
                clock,
                BundleStepInput {
                    host_call_results: host_results,
                    ..BundleStepInput::default()
                },
            );
            self.suppressed_host_requests = self
                .suppressed_host_requests
                .checked_add(step.requested_host_calls.len())
                .ok_or(RootReplayError::UnexpectedTransition)?;
            if !step.root_transitions.is_empty() {
                return Err(RootReplayError::UnexpectedTransition);
            }
        }
        if let Some(index) = self.used_external.iter().position(|used| !used) {
            return Err(external_error(
                trace,
                index,
                "position does not correspond to a recorded transition",
            ));
        }
        Ok(())
    }

    fn mark_external_used(&mut self, indices: &[usize]) -> Result<(), RootReplayError> {
        for index in indices {
            self.used_external[*index] = true;
        }
        self.external_outcomes_injected = self
            .external_outcomes_injected
            .checked_add(indices.len())
            .ok_or(RootReplayError::UnexpectedTransition)?;
        Ok(())
    }

    fn add_suppressed_requests(
        &mut self,
        count: usize,
        entry: &str,
        sequence: TransitionSequence,
    ) -> Result<(), RootReplayError> {
        self.suppressed_host_requests = self
            .suppressed_host_requests
            .checked_add(count)
            .ok_or_else(|| RootReplayError::OutcomeDivergence {
                entry: entry.to_owned(),
                transition: sequence.get(),
                message: "suppressed host-request count overflowed".to_owned(),
            })?;
        Ok(())
    }
}

fn consecutive_external_batch_end(
    trace: &RootReplayTraceV1,
    external_indices: &[usize],
    supplied_sequences: &[TransitionSequence],
) -> Result<usize, RootReplayError> {
    let mut expected_index = trace_index_for_sequence(trace, supplied_sequences[0])?;
    for sequence in supplied_sequences {
        let actual_index = trace_index_for_sequence(trace, *sequence)?;
        if actual_index != expected_index {
            return Err(external_error(
                trace,
                external_indices[0],
                "root-event result sequences must be consecutive in outcome Vec order",
            ));
        }
        expected_index += 1;
    }
    Ok(expected_index - 1)
}

fn preflight(
    bundle: &ArcweftBundle,
    options: &BundleSessionOptions,
    trace: &RootReplayTraceV1,
    artifact: BundleSessionArtifactIdentity,
) -> Result<RuntimeStatefulEntryRoles, RootReplayError> {
    if trace.identity.schema_version != ROOT_REPLAY_SCHEMA_VERSION {
        return Err(RootReplayError::UnsupportedSchema {
            actual: trace.identity.schema_version,
        });
    }
    if trace.identity.engine_identity != ROOT_REPLAY_ENGINE_IDENTITY {
        return Err(RootReplayError::EngineIdentity {
            expected: ROOT_REPLAY_ENGINE_IDENTITY,
            actual: trace.identity.engine_identity.clone(),
        });
    }
    if trace.identity.artifact != artifact {
        return Err(RootReplayError::ArtifactMismatch);
    }
    if trace.identity.format_data_identity != RuntimeFormatDataIdentity::CURRENT {
        return Err(RootReplayError::FormatDataMismatch);
    }
    if options
        .active_locale
        .as_ref()
        .is_some_and(|locale| locale != &trace.identity.active_locale)
    {
        return Err(RootReplayError::LocaleMismatch);
    }
    let program = bundle.product_awbc_program();
    bundle
        .product_awbc()
        .verify_product_executable()
        .map_err(|error| RootReplayError::ArtifactInspection {
            message: error.to_string(),
        })?;
    let selected = selected_awbc_entry(program, bundle, options).map_err(|error| {
        RootReplayError::SessionStart {
            message: error.to_string(),
        }
    })?;
    let entry = &program.entries[selected.index()];
    if trace.identity.entry != entry.runtime_id {
        return Err(RootReplayError::EntryMismatch {
            recorded: trace.identity.entry.public_label().into_string(),
            selected: entry.runtime_id.public_label().into_string(),
        });
    }
    let kind = entry
        .kind
        .runtime_kind(&program.strings)
        .ok_or(RootReplayError::InvalidEntryKind)?;
    if trace.identity.entry_kind != kind {
        return Err(RootReplayError::EntryKindMismatch);
    }
    if trace.identity.binding != entry.binding {
        return Err(RootReplayError::BindingMismatch);
    }
    let RuntimeEntryRoles::Stateful(roles) = &entry.roles else {
        return Err(RootReplayError::EntryNotStateful {
            entry: trace.identity.entry.public_label().into_string(),
        });
    };
    if trace.identity.binding != roles.binding {
        return Err(RootReplayError::BindingMismatch);
    }
    if trace.identity.state_identity != roles.state.identity
        || trace.identity.state_layout != roles.state.layout
    {
        return Err(RootReplayError::StateRoleMismatch);
    }
    if trace.identity.event_identity != roles.event.identity
        || trace.identity.event_layout != roles.event.layout
    {
        return Err(RootReplayError::EventRoleMismatch);
    }
    Ok(roles.as_ref().clone())
}

fn validate_trace_shape(trace: &RootReplayTraceV1) -> Result<(), RootReplayError> {
    for (index, transition) in trace.transitions.iter().enumerate() {
        let expected = u64::try_from(index).expect("Vec index fits u64 on supported platforms");
        if transition.sequence.get() != expected {
            return Err(RootReplayError::SequenceMismatch {
                entry: trace.identity.entry.public_label().into_string(),
                transition: expected,
                expected,
                actual: transition.sequence.get(),
            });
        }
        if matches!(transition.outcome, RecordedRootOutcomeV1::Trapped { .. })
            && index + 1 != trace.transitions.len()
        {
            return Err(RootReplayError::NonTerminalTrap {
                transition: transition.sequence.get(),
            });
        }
    }
    let mut requests = BTreeSet::new();
    for outcome in &trace.external_outcomes {
        if !requests.insert(outcome.request.clone()) {
            return Err(RootReplayError::DuplicateExternalOutcome {
                request: outcome.request.0.clone(),
            });
        }
    }
    Ok(())
}

fn external_indices_at(
    trace: &RootReplayTraceV1,
    used: &[bool],
    position: RecordedExternalOutcomePositionV1,
) -> Vec<usize> {
    trace
        .external_outcomes
        .iter()
        .enumerate()
        .filter_map(|(index, outcome)| {
            (!used[index] && outcome.position == position).then_some(index)
        })
        .collect()
}

fn prepare_external_batch(
    session: &BundleSession,
    trace: &RootReplayTraceV1,
    indices: &[usize],
    limits: arcweft_core::entry::RuntimeSchemaLimits,
) -> Result<(Vec<RuntimeHostCallResult>, Vec<TransitionSequence>), RootReplayError> {
    let mut requests = BTreeSet::new();
    let mut supplied_sequences = Vec::new();
    for index in indices {
        let recorded = &trace.external_outcomes[*index];
        if !requests.insert(recorded.request.clone()) {
            return Err(RootReplayError::DuplicateExternalOutcome {
                request: recorded.request.0.clone(),
            });
        }
        let pending_root_result = session.pending_root_command_results.get(&recorded.request);
        if let (Some(pending), RecordedExternalOutcomeResultV1::Success(payload)) =
            (pending_root_result, &recorded.outcome)
        {
            pending.bound.validate_snapshot(payload).map_err(|error| {
                external_error(
                    trace,
                    *index,
                    &format!(
                        "successful root-command result violates its bound result type: {error}"
                    ),
                )
            })?;
        }
        let root_route = pending_root_result.map(|pending| pending.route);
        let successful_payload = match &recorded.outcome {
            RecordedExternalOutcomeResultV1::Success(payload) => Some(payload),
            RecordedExternalOutcomeResultV1::Failure { .. } => None,
        };
        match (root_route, successful_payload, recorded.root_event_sequence) {
            (Some(RootCommandHostResultRoute::RootEventPayload), Some(_), Some(sequence)) => {
                supplied_sequences.push(sequence);
            }
            (Some(RootCommandHostResultRoute::RootEventPayload), Some(_), None) => {
                return Err(external_error(
                    trace,
                    *index,
                    "successful RootEventPayload result is missing root_event_sequence",
                ));
            }
            (Some(RootCommandHostResultRoute::RootEventPayload), None, Some(_)) => {
                return Err(external_error(
                    trace,
                    *index,
                    "failed host result cannot produce a root event",
                ));
            }
            (Some(RootCommandHostResultRoute::Ignore) | None, _, Some(_)) => {
                return Err(external_error(
                    trace,
                    *index,
                    "root_event_sequence does not refer to a pending RootEventPayload result",
                ));
            }
            _ => {}
        }
    }
    let owner = session.executor.program_owner();
    let results = indices
        .iter()
        .map(|index| trace.external_outcomes[*index].runtime_result(&owner, limits))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((results, supplied_sequences))
}

fn validate_event_slice(
    trace: &RootReplayTraceV1,
    start: usize,
    end: usize,
    roles: &RuntimeStatefulEntryRoles,
    entry: &str,
    program: RuntimeProgramTypes<'_>,
) -> Result<(), RootReplayError> {
    for transition in &trace.transitions[start..=end] {
        let digest = roles
            .event
            .accepts_snapshot(
                program,
                &transition.event,
                roles.command_policy.root_limits.schema,
            )
            .map_err(|error| RootReplayError::EventDivergence {
                entry: entry.to_owned(),
                transition: transition.sequence.get(),
                message: error.to_string(),
            })?;
        if digest != transition.event_digest {
            return Err(RootReplayError::EventDivergence {
                entry: entry.to_owned(),
                transition: transition.sequence.get(),
                message: "recorded event digest does not match the recorded payload".to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_supplied_events(
    trace: &RootReplayTraceV1,
    external_indices: &[usize],
    supplied_sequences: &[TransitionSequence],
    roles: &RuntimeStatefulEntryRoles,
    entry: &str,
    program: RuntimeProgramTypes<'_>,
) -> Result<(), RootReplayError> {
    let mut supplied = supplied_sequences.iter();
    for index in external_indices {
        let outcome = &trace.external_outcomes[*index];
        let (RecordedExternalOutcomeResultV1::Success(payload), Some(recorded_sequence)) =
            (&outcome.outcome, outcome.root_event_sequence)
        else {
            continue;
        };
        let sequence = supplied
            .next()
            .expect("successful root-event outcomes and sequences have equal cardinality");
        debug_assert_eq!(*sequence, recorded_sequence);
        let transition_index = trace_index_for_sequence(trace, *sequence)?;
        let digest = roles
            .event
            .accepts_snapshot(program, payload, roles.command_policy.root_limits.schema)
            .map_err(|error| RootReplayError::EventDivergence {
                entry: entry.to_owned(),
                transition: sequence.get(),
                message: error.to_string(),
            })?;
        if digest != trace.transitions[transition_index].event_digest {
            return Err(RootReplayError::EventDivergence {
                entry: entry.to_owned(),
                transition: sequence.get(),
                message: "recorded external result does not produce the recorded event".to_owned(),
            });
        }
    }
    Ok(())
}

fn compare_outcomes(
    expected: &[RecordedRootTransitionV1],
    actual: &[RootTransitionOutcome],
    before: &RootStateSnapshotV1,
    after: &RootStateSnapshotV1,
    mut durable_state_digest: RuntimeValueDigest,
    roles: &RuntimeStatefulEntryRoles,
    entry: &str,
    program: RuntimeProgramTypes<'_>,
) -> Result<RuntimeValueDigest, RootReplayError> {
    for (recorded, observed) in expected.iter().zip(actual) {
        durable_state_digest = compare_transition(recorded, observed, durable_state_digest, entry)?;
    }
    let Some(last) = actual.last() else {
        return Err(RootReplayError::UnexpectedTransition);
    };
    let actual_digest = roles
        .state
        .accepts_payload(
            program,
            after.phase.state_value().ok_or_else(|| {
                outcome_divergence(entry, last.sequence(), "root initialization is pending")
            })?,
            roles.command_policy.root_limits.schema,
        )
        .map_err(|error| outcome_divergence(entry, last.sequence(), &error.to_string()))?;
    if actual_digest != durable_state_digest {
        return Err(outcome_divergence(
            entry,
            last.sequence(),
            "final durable state does not equal the last committed state",
        ));
    }
    let expected_next = match last {
        RootTransitionOutcome::Trapped { sequence, .. } => *sequence,
        RootTransitionOutcome::Committed { sequence, .. }
        | RootTransitionOutcome::Rejected { sequence, .. } => {
            let next = sequence.get().checked_add(1).ok_or_else(|| {
                outcome_divergence(
                    entry,
                    *sequence,
                    "consumed transition has no representable successor",
                )
            })?;
            TransitionSequence::from_u64(next)
        }
    };
    if after.next_sequence != expected_next {
        return Err(outcome_divergence(
            entry,
            last.sequence(),
            "root transition cursor did not preserve the recorded consumption rule",
        ));
    }
    if matches!(last, RootTransitionOutcome::Trapped { .. })
        && actual.len() == 1
        && before.next_sequence != after.next_sequence
    {
        return Err(outcome_divergence(
            entry,
            last.sequence(),
            "reducer trap consumed a transition sequence",
        ));
    }
    Ok(durable_state_digest)
}

fn compare_transition(
    recorded: &RecordedRootTransitionV1,
    observed: &RootTransitionOutcome,
    durable_state_digest: RuntimeValueDigest,
    entry: &str,
) -> Result<RuntimeValueDigest, RootReplayError> {
    if observed.sequence() != recorded.sequence {
        return Err(RootReplayError::SequenceMismatch {
            entry: entry.to_owned(),
            transition: recorded.sequence.get(),
            expected: recorded.sequence.get(),
            actual: observed.sequence().get(),
        });
    }
    match (&recorded.outcome, observed) {
        (
            RecordedRootOutcomeV1::Committed {
                state_digest,
                command_digests,
            },
            RootTransitionOutcome::Committed {
                state_digest: actual_state,
                command_digests: actual_commands,
                ..
            },
        ) => {
            if state_digest != actual_state {
                return Err(outcome_divergence(
                    entry,
                    recorded.sequence,
                    "post-state digest differs",
                ));
            }
            let command_count = command_digests.len().max(actual_commands.len());
            for command_index in 0..command_count {
                if command_digests.get(command_index) != actual_commands.get(command_index) {
                    return Err(RootReplayError::CommandDivergence {
                        entry: entry.to_owned(),
                        transition: recorded.sequence.get(),
                        command_index,
                    });
                }
            }
            Ok(*actual_state)
        }
        (
            RecordedRootOutcomeV1::Rejected { error_digest },
            RootTransitionOutcome::Rejected {
                error_digest: actual_error,
                ..
            },
        ) if error_digest == actual_error => Ok(durable_state_digest),
        (
            RecordedRootOutcomeV1::Trapped { failure_digest },
            RootTransitionOutcome::Trapped {
                failure_digest: actual_failure,
                ..
            },
        ) if failure_digest == actual_failure => Ok(durable_state_digest),
        (RecordedRootOutcomeV1::Rejected { .. }, RootTransitionOutcome::Rejected { .. }) => Err(
            outcome_divergence(entry, recorded.sequence, "reducer rejection digest differs"),
        ),
        (RecordedRootOutcomeV1::Trapped { .. }, RootTransitionOutcome::Trapped { .. }) => Err(
            outcome_divergence(entry, recorded.sequence, "reducer failure digest differs"),
        ),
        _ => Err(outcome_divergence(
            entry,
            recorded.sequence,
            "root outcome variant differs",
        )),
    }
}

fn trace_index_for_sequence(
    trace: &RootReplayTraceV1,
    sequence: TransitionSequence,
) -> Result<usize, RootReplayError> {
    let index = usize::try_from(sequence.get()).map_err(|_| RootReplayError::SequenceMismatch {
        entry: trace.identity.entry.public_label().into_string(),
        transition: sequence.get(),
        expected: u64::MAX,
        actual: sequence.get(),
    })?;
    if trace
        .transitions
        .get(index)
        .is_none_or(|transition| transition.sequence != sequence)
    {
        return Err(RootReplayError::SequenceMismatch {
            entry: trace.identity.entry.public_label().into_string(),
            transition: sequence.get(),
            expected: u64::try_from(trace.transitions.len()).unwrap_or(u64::MAX),
            actual: sequence.get(),
        });
    }
    Ok(index)
}

fn outcome_divergence(
    entry: &str,
    transition: TransitionSequence,
    message: &str,
) -> RootReplayError {
    RootReplayError::OutcomeDivergence {
        entry: entry.to_owned(),
        transition: transition.get(),
        message: message.to_owned(),
    }
}

fn external_error(trace: &RootReplayTraceV1, index: usize, message: &str) -> RootReplayError {
    RootReplayError::ExternalOutcome {
        request: trace.external_outcomes[index].request.0.clone(),
        message: message.to_owned(),
    }
}

impl RecordedExternalOutcome {
    fn runtime_result(
        &self,
        owner: &RuntimeProgramOwner,
        limits: arcweft_core::entry::RuntimeSchemaLimits,
    ) -> Result<RuntimeHostCallResult, RootReplayError> {
        Ok(RuntimeHostCallResult {
            id: self.request.clone(),
            outcome: match &self.outcome {
                RecordedExternalOutcomeResultV1::Success(payload) => Ok(payload
                    .copy_unrestricted_runtime_value_for_program_with_limits(owner, limits)
                    .map_err(|error| RootReplayError::ExternalOutcome {
                        request: self.request.0.clone(),
                        message: error.to_string(),
                    })?
                    .into()),
                RecordedExternalOutcomeResultV1::Failure { kind, message } => {
                    Err(RuntimeHostCallError {
                        kind: (*kind).into(),
                        message: message.clone(),
                    })
                }
            },
        })
    }
}

impl From<RecordedHostCallErrorKindV1> for RuntimeHostCallErrorKind {
    fn from(value: RecordedHostCallErrorKindV1) -> Self {
        match value {
            RecordedHostCallErrorKindV1::UnsupportedCapability => Self::UnsupportedCapability,
            RecordedHostCallErrorKindV1::Rejected => Self::Rejected,
            RecordedHostCallErrorKindV1::Failed => Self::Failed,
        }
    }
}
