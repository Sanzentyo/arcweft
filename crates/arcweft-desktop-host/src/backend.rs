use crate::dispatcher::DesktopTaskId;
use arcweft_desktop_contract::{DesktopError, DesktopRequest, DesktopResponse};

/// Required execution context for one desktop request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionLane {
    AnyThread,
    HostMainThread,
}

/// Result of starting one backend request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendSubmission {
    Completed(Result<DesktopResponse, DesktopError>),
    Pending,
}

/// Completion emitted later by an asynchronous backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendCompletion {
    pub task: DesktopTaskId,
    pub result: Result<DesktopResponse, DesktopError>,
}

/// Platform implementation behind the Arcweft desktop adapters.
pub trait DesktopBackend: Send + Sync + 'static {
    fn execution_lane(&self, request: &DesktopRequest) -> ExecutionLane;

    fn submit(&self, task: DesktopTaskId, request: DesktopRequest) -> BackendSubmission;

    /// Drains completions produced by platform callbacks or futures.
    fn drain_completions(&self) -> Vec<BackendCompletion> {
        Vec::new()
    }

    /// Requests cancellation of an asynchronous platform operation.
    fn cancel(&self, _task: DesktopTaskId) -> bool {
        false
    }
}
