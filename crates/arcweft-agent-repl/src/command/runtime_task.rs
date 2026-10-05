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
        let runtime_target = match &target {
            ReplCancelTarget::All => RuntimeTaskCancelTarget::All,
            ReplCancelTarget::Scope(scope) => {
                RuntimeTaskCancelTarget::Scope(arcweft_core::task::CancelScopeId(scope.clone()))
            }
            ReplCancelTarget::Task(selector) => {
                let record = self
                    .tasks
                    .runtime_tasks(RuntimeTaskListOptions {
                        include_completed: true,
                    })
                    .into_iter()
                    .find(|record| record.dispatch.correlation.task_id.to_string() == *selector)
                    .ok_or_else(|| super::host::ReplCommandHostError {
                        code: super::types::ReplCommandDiagnosticCode::InvalidArgument,
                        message: "task selector does not name an accepted runtime task".to_owned(),
                    })?;
                RuntimeTaskCancelTarget::Task(record.dispatch.correlation.task_id)
            }
        };
        let outcome = self.tasks.cancel_runtime_tasks(runtime_target);
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
            dispatch: record.dispatch,
            status: ReplTaskStatus::from(record.status),
            cursor: record.cursor,
            cancel_scope: record.cancel_scope,
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
