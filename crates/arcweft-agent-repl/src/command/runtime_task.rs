use arcweft_agent_protocol::protocol::{AgentSessionInfo, ObservationEnvelope};
use arcweft_runtime_driver::task::{
    RuntimeTaskCancelOutcome, RuntimeTaskCancelTarget, RuntimeTaskListOptions, RuntimeTaskOwner,
    RuntimeTaskRecord, RuntimeTaskStatus,
};

use super::host::{ReplCommandHost, ReplCommandHostResult};
use super::types::{
    CancelCommand, ObserveCommand, ReplCancelOutcome, ReplCancelTarget, ReplTaskList,
    ReplTaskRecord, ReplTaskStatus, StepCommand, TasksCommand,
};

/// REPL command-host adapter that delegates observation/stepping to an existing
/// host while using a runtime-driver task owner for `:tasks` and `:cancel`.
pub struct RuntimeTaskReplCommandHost<'a, H, T>
where
    H: ReplCommandHost + ?Sized,
    T: RuntimeTaskOwner + ?Sized,
{
    host: &'a mut H,
    tasks: &'a mut T,
}

impl<'a, H, T> RuntimeTaskReplCommandHost<'a, H, T>
where
    H: ReplCommandHost + ?Sized,
    T: RuntimeTaskOwner + ?Sized,
{
    #[must_use]
    pub fn new(host: &'a mut H, tasks: &'a mut T) -> Self {
        Self { host, tasks }
    }
}

impl<H, T> ReplCommandHost for RuntimeTaskReplCommandHost<'_, H, T>
where
    H: ReplCommandHost + ?Sized,
    T: RuntimeTaskOwner + ?Sized,
{
    fn session_info(&mut self) -> ReplCommandHostResult<AgentSessionInfo> {
        self.host.session_info()
    }

    fn observe(&mut self, command: &ObserveCommand) -> ReplCommandHostResult<ObservationEnvelope> {
        self.host.observe(command)
    }

    fn step(&mut self, command: &StepCommand) -> ReplCommandHostResult<ObservationEnvelope> {
        self.host.step(command)
    }

    fn tasks(&mut self, command: &TasksCommand) -> ReplCommandHostResult<ReplTaskList> {
        let options = RuntimeTaskListOptions {
            include_completed: command.include_completed,
        };
        Ok(ReplTaskList {
            tasks: self
                .tasks
                .runtime_tasks(options)
                .into_iter()
                .map(ReplTaskRecord::from)
                .collect(),
        })
    }

    fn cancel(&mut self, command: &CancelCommand) -> ReplCommandHostResult<ReplCancelOutcome> {
        let target = command.target.clone();
        let outcome = self
            .tasks
            .cancel_runtime_tasks(target.clone().into_runtime_task_target());
        Ok(ReplCancelOutcome::from_runtime_task_outcome(
            target, outcome,
        ))
    }
}

impl From<RuntimeTaskStatus> for ReplTaskStatus {
    fn from(status: RuntimeTaskStatus) -> Self {
        match status {
            RuntimeTaskStatus::Pending => Self::Pending,
            RuntimeTaskStatus::Running => Self::Running,
            RuntimeTaskStatus::Completed => Self::Completed,
            RuntimeTaskStatus::Cancelled => Self::Cancelled,
            RuntimeTaskStatus::Failed => Self::Failed,
        }
    }
}

impl From<RuntimeTaskRecord> for ReplTaskRecord {
    fn from(record: RuntimeTaskRecord) -> Self {
        Self {
            id: record.id,
            status: ReplTaskStatus::from(record.status),
            generation: record.generation,
            logical_epoch: record.logical_epoch,
            sequence: record.sequence,
            cancel_scope: record.cancel_scope,
        }
    }
}

impl ReplCancelTarget {
    fn into_runtime_task_target(self) -> RuntimeTaskCancelTarget {
        match self {
            Self::All => RuntimeTaskCancelTarget::All,
            Self::Task(id) => RuntimeTaskCancelTarget::Task(id),
            Self::Scope(scope) => RuntimeTaskCancelTarget::Scope(scope),
        }
    }
}

impl ReplCancelOutcome {
    fn from_runtime_task_outcome(
        target: ReplCancelTarget,
        outcome: RuntimeTaskCancelOutcome,
    ) -> Self {
        Self {
            target,
            cancelled: outcome.cancelled,
            pending_after: outcome.pending_after,
        }
    }
}
