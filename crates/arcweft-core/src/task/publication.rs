//! The closed owning publications accepted by the Sans-I/O scheduler.

use super::{BoundTaskOutcome, BoundTaskSpec, ProgramTaskPublication, TaskEvent, TaskEventKind};

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
