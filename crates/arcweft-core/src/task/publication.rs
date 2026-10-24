//! The closed owning publications accepted by the Sans-I/O scheduler.

use super::{BoundTaskOutcome, BoundTaskSpec, ProgramTaskPublication, TaskEvent, TaskEventKind};

/// Consumer position within one accepted Need correlation. Program value
/// custody advances independently of the adapter's retirement acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum RuntimeNeedPublicationCursor {
    Task(super::TaskPublicationCursor),
    Program(ProgramTaskValuePhase),
}

/// The original registry state supplies this phase; no second counter is kept.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub enum ProgramTaskValuePhase {
    Pending,
    Terminal,
    Transferred,
}

impl RuntimeNeedPublicationCursor {
    pub fn compare_same_source(self, observed: Self) -> Option<std::cmp::Ordering> {
        match (self, observed) {
            (Self::Task(current), Self::Task(observed)) => current.compare_same_source(observed),
            (Self::Program(current), Self::Program(observed)) => Some(current.cmp(&observed)),
            _ => None,
        }
    }

    pub(crate) fn program<P>(state: &super::RuntimeNeedProducerState<P>) -> Option<Self> {
        use super::RuntimeNeedProducerState as State;
        Some(Self::Program(match state {
            State::NotStarted => return None,
            State::Pending(_) => ProgramTaskValuePhase::Pending,
            State::Ready(_) | State::InfrastructureFailure(_) | State::Cancelled => {
                ProgramTaskValuePhase::Terminal
            }
            State::ReadyTransferred => ProgramTaskValuePhase::Transferred,
        }))
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::TaskEvent {}
    impl Sealed for super::ProgramTaskPublication {}
}

/// Host packets retain their values and Program packets retain Core's original
/// retirement proof while scheduling inspects a borrowed event view. Only these
/// two Core-owned packet kinds implement this interface.
pub trait RuntimeTaskPublication: sealed::Sealed {
    /// Borrows the packet without granting payload replacement or duplication.
    fn event(&self) -> &TaskEvent;
    /// Checks the actual accepted execution authority before batch mutation.
    fn validate_launch(&self, accepted: &BoundTaskSpec) -> Result<(), super::TaskCompletionError>;
    /// Converts an invalid Ready outcome into one terminal protocol failure.
    fn admit_outcome(&mut self, expected: &BoundTaskOutcome);
}

impl TaskEvent {
    fn admit_outcome(&mut self, expected: &BoundTaskOutcome) {
        let TaskEventKind::Ready(payload) = &self.kind else {
            return;
        };
        if let Err(error) = expected.validate_value(payload.value()) {
            self.kind = TaskEventKind::InfrastructureFailure(super::RuntimeTaskFailure::new(
                super::RuntimeTaskFailureKind::AdapterProtocolViolation,
                error.to_string(),
            ));
        }
    }
}

impl RuntimeTaskPublication for TaskEvent {
    fn event(&self) -> &TaskEvent {
        self
    }

    fn validate_launch(&self, accepted: &BoundTaskSpec) -> Result<(), super::TaskCompletionError> {
        if matches!(&self.kind, TaskEventKind::ProgramTerminal(_))
            || matches!(&accepted.spec().request, super::TaskRequest::Program(_))
        {
            return Err(super::TaskCompletionError::DispatchMismatch {
                task_id: self.correlation.task_id,
            });
        }
        Ok(())
    }

    fn admit_outcome(&mut self, expected: &BoundTaskOutcome) {
        Self::admit_outcome(self, expected);
    }
}

impl RuntimeTaskPublication for ProgramTaskPublication {
    fn event(&self) -> &TaskEvent {
        &self.event
    }

    fn validate_launch(&self, accepted: &BoundTaskSpec) -> Result<(), super::TaskCompletionError> {
        if self.launch.handle() != accepted.handle()
            || !self.launch.matches_spec(accepted.spec())
            || self.event.correlation != self.launch.handle().correlation
        {
            return Err(super::TaskCompletionError::DispatchMismatch {
                task_id: self.event.correlation.task_id,
            });
        }
        Ok(())
    }

    fn admit_outcome(&mut self, expected: &BoundTaskOutcome) {
        self.event.admit_outcome(expected);
    }
}
