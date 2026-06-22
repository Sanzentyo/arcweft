use arcweft_core::task::{LogicalEpoch, TaskEvent, TaskEventKind, TaskSequence, TaskSpec};
use arcweft_core::value::RuntimePayload;
use serde::{Deserialize, Serialize};

/// Runtime request annotated with the deterministic host-dispatch order.
///
/// The epoch is the logical runtime tick that emitted the request. The sequence
/// is assigned in request order by `BundleSession`; browser completion order is
/// normalized back to this pair before task events enter the VM.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HostTaskDispatch {
    pub logical_epoch: LogicalEpoch,
    pub sequence: TaskSequence,
    pub task: TaskSpec,
}

impl HostTaskDispatch {
    pub fn ready(self, value: RuntimePayload) -> TaskEvent {
        self.into_event(TaskEventKind::Ready(value))
    }

    pub fn failed(self, message: impl Into<String>) -> TaskEvent {
        self.into_event(TaskEventKind::Err(message.into()))
    }

    pub fn cancelled(self) -> TaskEvent {
        self.into_event(TaskEventKind::Cancelled)
    }

    pub fn progress(self, value: RuntimePayload) -> TaskEvent {
        self.into_event(TaskEventKind::Progress(value))
    }

    pub fn into_event(self, kind: TaskEventKind) -> TaskEvent {
        TaskEvent {
            logical_epoch: self.logical_epoch,
            task_id: self.task.id,
            sequence: self.sequence,
            kind,
        }
    }

    pub fn ordering_key(&self) -> (LogicalEpoch, TaskSequence, &str) {
        (self.logical_epoch, self.sequence, self.task.id.0.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::task::{
        CancelScopeId, HostTaskRequest, TaskClass, TaskId, TaskKey, TaskPolicy, TaskPriority,
    };
    use arcweft_core::value::RuntimeValue;

    #[test]
    fn completion_preserves_request_epoch_and_sequence() {
        let dispatch = HostTaskDispatch {
            logical_epoch: LogicalEpoch(12),
            sequence: TaskSequence(7),
            task: TaskSpec::new(
                TaskId("task.test".to_owned()),
                TaskKey("task.test".to_owned()),
                TaskClass::Background,
                TaskPriority(0),
                CancelScopeId("test".to_owned()),
                TaskPolicy::AlwaysStart,
                HostTaskRequest::custom("test", "unit", []),
            ),
        };
        let event = dispatch.ready(RuntimePayload::new(RuntimeValue::Unit));
        assert_eq!(event.logical_epoch, LogicalEpoch(12));
        assert_eq!(event.sequence, TaskSequence(7));
        assert_eq!(event.task_id.0, "task.test");
    }
}
