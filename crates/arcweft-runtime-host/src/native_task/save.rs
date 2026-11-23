//! Host-owned dispatch checkpoints for source sessions.

use super::NativeTaskBridge;
use arcweft_core::task::{RuntimeNeedProducerDispatch, TaskDispatchStart};
use std::collections::BTreeMap;
use thiserror::Error;

/// Host dispatch coordinates retained alongside a source executor save.
/// Each row is rebound against the executor's accepted Need before use.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeTaskBridgeSaveSnapshot {
    pub(super) next_sequence: u64,
    pub(super) dispatches: Vec<TaskDispatchStart>,
}

#[derive(Debug, Error)]
pub enum NativeTaskBridgeSaveError {
    #[error("direct host calls or their results are still in flight")]
    HostCalls,
    #[error("host dispatch journal differs from the executor's Restartable Needs")]
    DispatchJournal,
    #[error("host dispatch resume requires a fresh bridge")]
    BridgeAlreadyStarted,
}

impl NativeTaskBridge {
    /// Captures accepted dispatches at a drained host ingress boundary.
    /// The session owner must first deliver any buffered task and call results.
    pub fn snapshot_dispatch_journal(
        &self,
        launches: &[RuntimeNeedProducerDispatch],
    ) -> Result<NativeTaskBridgeSaveSnapshot, NativeTaskBridgeSaveError> {
        if !self.pending_host_calls.is_empty()
            || !self.retired_host_call_tasks.is_empty()
            || !self.ready_host_call_results.is_empty()
        {
            return Err(NativeTaskBridgeSaveError::HostCalls);
        }
        if launches.len() != self.dispatches.len() + self.restored_dispatches.len() {
            return Err(NativeTaskBridgeSaveError::DispatchJournal);
        }
        let mut starts = BTreeMap::new();
        for launch in launches {
            let task = launch.submission.task_id();
            let start = if launch.needs_reensure {
                let (start, submission) = self
                    .restored_dispatches
                    .get(&task)
                    .ok_or(NativeTaskBridgeSaveError::DispatchJournal)?;
                if submission != &launch.submission {
                    return Err(NativeTaskBridgeSaveError::DispatchJournal);
                }
                start.clone()
            } else {
                let attempt = self
                    .dispatches
                    .get(&task)
                    .ok_or(NativeTaskBridgeSaveError::DispatchJournal)?;
                let accepted = self
                    .scheduler
                    .bound_spec(&task)
                    .ok_or(NativeTaskBridgeSaveError::DispatchJournal)?;
                if accepted.submission() != &launch.submission {
                    return Err(NativeTaskBridgeSaveError::DispatchJournal);
                }
                TaskDispatchStart::new(
                    attempt.identity().clone(),
                    self.publication_frontiers.get(&task).copied(),
                )
            };
            if !launch.matches_dispatch_start(&start)
                || start.identity().sequence.0 >= self.sequence
                || starts.insert(start.identity().sequence, start).is_some()
            {
                return Err(NativeTaskBridgeSaveError::DispatchJournal);
            }
        }
        Ok(NativeTaskBridgeSaveSnapshot {
            next_sequence: self.sequence,
            dispatches: starts.into_values().collect(),
        })
    }

    /// Admits the full journal before transferring its sequence or submitting
    /// any host work. The executor must emit the accepted requests to re-ensure.
    pub fn restore_dispatch_journal(
        &mut self,
        snapshot: NativeTaskBridgeSaveSnapshot,
        launches: &[RuntimeNeedProducerDispatch],
    ) -> Result<(), NativeTaskBridgeSaveError> {
        if self.sequence != 0
            || !self.seen_host_calls.is_empty()
            || !self.restored_dispatches.is_empty()
        {
            return Err(NativeTaskBridgeSaveError::BridgeAlreadyStarted);
        }
        if snapshot.dispatches.len() != launches.len() {
            return Err(NativeTaskBridgeSaveError::DispatchJournal);
        }
        let mut by_correlation = BTreeMap::new();
        for launch in launches {
            if !launch.needs_reensure
                || by_correlation
                    .insert(launch.submission.handle().correlation, launch)
                    .is_some()
            {
                return Err(NativeTaskBridgeSaveError::DispatchJournal);
            }
        }
        let mut restored = BTreeMap::new();
        let mut last_sequence = None;
        for start in snapshot.dispatches {
            let identity = start.identity();
            let launch = by_correlation
                .remove(&identity.correlation)
                .ok_or(NativeTaskBridgeSaveError::DispatchJournal)?;
            if !launch.matches_dispatch_start(&start)
                || identity.sequence.0 >= snapshot.next_sequence
                || last_sequence.is_some_and(|last| identity.sequence <= last)
                || !launch
                    .submission
                    .spec()
                    .request
                    .as_host()
                    .is_some_and(|request| {
                        self.policy.allows(request)
                            && self.registry.contains(&request.host_call_id())
                    })
            {
                return Err(NativeTaskBridgeSaveError::DispatchJournal);
            }
            last_sequence = Some(identity.sequence);
            if restored
                .insert(identity.correlation.task_id, (start, &launch.submission))
                .is_some()
            {
                return Err(NativeTaskBridgeSaveError::DispatchJournal);
            }
        }
        self.sequence = snapshot.next_sequence;
        self.restored_dispatches = restored
            .into_iter()
            .map(|(task, (start, submission))| (task, (start, submission.clone())))
            .collect();
        Ok(())
    }
}
