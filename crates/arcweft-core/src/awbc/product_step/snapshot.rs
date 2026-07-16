use super::{
    ActiveChoice, ActiveDialogue, AwbcProductStepBuildError, AwbcProductStepExecutor,
    PendingHostCall, source_id_for, stream_id_for,
};
use crate::awbc::fiber::FiberState;
use crate::awbc::schema::{
    AwbcChoiceId, AwbcContentUnitId, AwbcHostCallId, AwbcLineTaskGroupId, AwbcLineTaskNodeId,
    AwbcSourcePlanId, AwbcStreamPlanId,
};
use crate::observation::RuntimeObservationState;
use crate::plan::ChoiceRuntimeOption;
use crate::source::SourceRuntimeState;
use crate::step::RuntimeHostCallId;
use crate::stream::StreamRuntimeState;
use crate::task::TaskId;
use crate::value::RuntimePayload;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
pub struct AwbcProductExecutorSnapshot {
    pub fiber: FiberState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub child_fibers: Vec<FiberState>,
    #[serde(default)]
    pub entry_bound: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_dialogue: Option<AwbcProductActiveDialogueSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_choice: Option<AwbcProductActiveChoiceSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_host_call: Option<AwbcProductPendingHostCallSnapshot>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub started_tasks: BTreeSet<TaskId>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub emitted_content: BTreeSet<AwbcContentUnitId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stream_sequences: BTreeMap<AwbcStreamPlanId, u64>,
    pub next_generation: u64,
    pub next_host_call_sequence: u64,
    pub next_audio_sequence: u64,
    #[serde(default)]
    pub compact_pure_stats: crate::step::RuntimePureCallStats,
    #[serde(default)]
    pub observations: RuntimeObservationState,
}

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct AwbcProductActiveDialogueSnapshot {
    pub content: AwbcContentUnitId,
    pub group: AwbcLineTaskGroupId,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub started_nodes: BTreeSet<AwbcLineTaskNodeId>,
    pub elapsed_nanos: u64,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
pub struct AwbcProductActiveChoiceSnapshot {
    pub choice: AwbcChoiceId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<ChoiceRuntimeOption>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub option_indices: Vec<usize>,
}

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct AwbcProductPendingHostCallSnapshot {
    pub call: AwbcHostCallId,
    pub id: String,
}

impl AwbcProductStepExecutor {
    #[must_use]
    pub fn snapshot(&self) -> AwbcProductExecutorSnapshot {
        AwbcProductExecutorSnapshot {
            fiber: self.fiber.clone(),
            child_fibers: self.child_fibers.iter().cloned().collect(),
            entry_bound: self.entry_bound,
            active_dialogue: self.active_dialogue.as_ref().map(|active| {
                AwbcProductActiveDialogueSnapshot {
                    content: active.content,
                    group: active.group,
                    started_nodes: active.started_nodes.clone(),
                    elapsed_nanos: active.elapsed_nanos,
                }
            }),
            active_choice: self.active_choice.as_ref().map(|active| {
                AwbcProductActiveChoiceSnapshot {
                    choice: active.choice,
                    public_id: active.public_id.clone(),
                    options: active.options.clone(),
                    option_indices: active.option_indices.clone(),
                }
            }),
            pending_host_call: self.pending_host_call.as_ref().map(|pending| {
                AwbcProductPendingHostCallSnapshot {
                    call: pending.call,
                    id: pending.id.0.clone(),
                }
            }),
            started_tasks: self.started_tasks.clone(),
            emitted_content: self.emitted_content.clone(),
            stream_sequences: self.stream_sequences.clone(),
            next_generation: self.next_generation,
            next_host_call_sequence: self.next_host_call_sequence,
            next_audio_sequence: self.next_audio_sequence,
            compact_pure_stats: self.compact_pure_stats,
            observations: self.facade_fiber.observations.clone(),
        }
    }

    pub fn restore_snapshot(
        &mut self,
        snapshot: AwbcProductExecutorSnapshot,
    ) -> Result<(), AwbcProductStepBuildError> {
        self.validate_snapshot(&snapshot)?;
        self.fiber = snapshot.fiber;
        self.child_fibers = snapshot.child_fibers.into_iter().collect();
        self.entry_bound = snapshot.entry_bound;
        self.active_dialogue = snapshot.active_dialogue.map(|active| ActiveDialogue {
            content: active.content,
            group: active.group,
            started_nodes: active.started_nodes,
            elapsed_nanos: active.elapsed_nanos,
        });
        self.active_choice = snapshot.active_choice.map(|active| ActiveChoice {
            choice: active.choice,
            public_id: active.public_id,
            options: active.options,
            option_indices: active.option_indices,
        });
        self.pending_host_call = snapshot.pending_host_call.map(|pending| PendingHostCall {
            call: pending.call,
            id: RuntimeHostCallId(pending.id),
        });
        self.started_tasks = snapshot.started_tasks;
        self.emitted_content = snapshot.emitted_content;
        self.stream_sequences = snapshot.stream_sequences;
        self.next_generation = snapshot.next_generation;
        self.next_host_call_sequence = snapshot.next_host_call_sequence;
        self.next_audio_sequence = snapshot.next_audio_sequence;
        self.compact_pure_stats = snapshot.compact_pure_stats;
        self.facade_fiber.observations = snapshot.observations;
        self.rebuild_facade_source_states_from_compact();
        self.rebuild_facade_stream_states_from_compact();
        self.sync_facade();
        Ok(())
    }

    pub(super) fn validate_snapshot(
        &self,
        snapshot: &AwbcProductExecutorSnapshot,
    ) -> Result<(), AwbcProductStepBuildError> {
        snapshot
            .fiber
            .validate_for_program(&self.program)
            .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            })?;
        for child in &snapshot.child_fibers {
            child.validate_for_program(&self.program).map_err(|error| {
                AwbcProductStepBuildError::RestoreSnapshot {
                    message: error.to_string(),
                }
            })?;
        }
        if let Some(active) = &snapshot.active_dialogue
            && (self
                .program
                .content_units
                .get(active.content.index())
                .is_none()
                || self
                    .program
                    .line_task_groups
                    .get(active.group.index())
                    .is_none()
                || active
                    .started_nodes
                    .iter()
                    .any(|node| self.program.line_task_nodes.get(node.index()).is_none()))
        {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "active dialogue snapshot references missing AWBC tables".to_owned(),
            });
        }
        if let Some(active) = &snapshot.active_choice
            && self.program.choices.get(active.choice.index()).is_none()
        {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "active choice snapshot references missing AWBC choice".to_owned(),
            });
        }
        if let Some(pending) = &snapshot.pending_host_call
            && self.program.host_calls.get(pending.call.index()).is_none()
        {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "pending host-call snapshot references missing AWBC host call".to_owned(),
            });
        }
        if snapshot
            .emitted_content
            .iter()
            .any(|content| self.program.content_units.get(content.index()).is_none())
            || snapshot
                .stream_sequences
                .keys()
                .any(|stream| self.program.stream_plans.get(stream.index()).is_none())
        {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "executor snapshot references missing AWBC content or stream table"
                    .to_owned(),
            });
        }
        Ok(())
    }

    pub(super) fn rebuild_facade_source_states_from_compact(&mut self) {
        self.facade_fiber.source_states.clear();
        for (index, source) in self.program.source_plans.iter().enumerate() {
            let Some(index) = u32::try_from(index).ok() else {
                continue;
            };
            let plan = AwbcSourcePlanId(index);
            let id = source_id_for(&self.program, plan);
            let mut runtime = SourceRuntimeState::new(id.clone(), source.policy.runtime_policy());
            if let Some(compact) = self.fiber.sources.iter().find(|state| state.plan == plan) {
                runtime.queue = compact
                    .queue
                    .iter()
                    .cloned()
                    .map(RuntimePayload::from)
                    .collect();
                runtime.closed = compact.closed;
                runtime.last_error = compact.last_error.clone().map(RuntimePayload::from);
                runtime.overflow_count = compact.overflow_count;
            }
            self.facade_fiber.source_states.insert(id, runtime);
        }
    }

    pub(super) fn rebuild_facade_stream_states_from_compact(&mut self) {
        self.facade_fiber.stream_states.clear();
        for (index, _) in self.program.stream_plans.iter().enumerate() {
            let Some(index) = u32::try_from(index).ok() else {
                continue;
            };
            let plan = AwbcStreamPlanId(index);
            let id = stream_id_for(&self.program, plan);
            let mut runtime = StreamRuntimeState::new(id.clone());
            if let Some(compact) = self.fiber.streams.iter().find(|state| state.plan == plan) {
                runtime.queue = compact
                    .queue
                    .iter()
                    .cloned()
                    .map(RuntimePayload::from)
                    .collect();
                runtime.closed = compact.closed;
                runtime.emitted_count = compact.emitted_count;
            }
            self.facade_fiber.stream_states.insert(id, runtime);
        }
    }
}
