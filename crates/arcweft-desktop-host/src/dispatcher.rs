use crate::{BackendCompletion, BackendSubmission, DesktopBackend, ExecutionLane};
use arcweft_desktop_contract::{DesktopError, DesktopRequest, DesktopResponse};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, ThreadId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DesktopTaskId(u64);

impl DesktopTaskId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for DesktopTaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesktopSubmission {
    Completed(Result<DesktopResponse, DesktopError>),
    Pending(DesktopTaskId),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PumpReport {
    pub started: usize,
    pub completed: usize,
    pub ignored_stale_completions: usize,
}

struct PendingRequest {
    id: DesktopTaskId,
    request: DesktopRequest,
}

#[derive(Default)]
struct QueueState {
    next_id: u64,
    pending_main: VecDeque<PendingRequest>,
    in_flight: BTreeSet<DesktopTaskId>,
    completed: BTreeMap<DesktopTaskId, Result<DesktopResponse, DesktopError>>,
}

struct MainThreadQueue {
    bound_thread: ThreadId,
    state: Mutex<QueueState>,
}

impl MainThreadQueue {
    fn new(bound_thread: ThreadId) -> Self {
        Self {
            bound_thread,
            state: Mutex::new(QueueState::default()),
        }
    }

    fn allocate(&self) -> DesktopTaskId {
        let mut state = lock_or_recover(&self.state);
        state.next_id = state.next_id.saturating_add(1).max(1);
        let id = DesktopTaskId(state.next_id);
        state.in_flight.insert(id);
        id
    }

    fn queue_main(&self, id: DesktopTaskId, request: DesktopRequest) {
        lock_or_recover(&self.state)
            .pending_main
            .push_back(PendingRequest { id, request });
    }

    fn take_main_requests(&self) -> Vec<PendingRequest> {
        lock_or_recover(&self.state)
            .pending_main
            .drain(..)
            .collect()
    }

    fn finish(&self, id: DesktopTaskId, result: Result<DesktopResponse, DesktopError>) -> bool {
        let mut state = lock_or_recover(&self.state);
        if !state.in_flight.remove(&id) {
            return false;
        }
        state.completed.insert(id, result);
        true
    }

    fn finish_immediate(&self, id: DesktopTaskId) {
        lock_or_recover(&self.state).in_flight.remove(&id);
    }

    fn poll(&self, id: DesktopTaskId) -> Option<Result<DesktopResponse, DesktopError>> {
        lock_or_recover(&self.state).completed.remove(&id)
    }

    fn cancel_local(&self, id: DesktopTaskId) -> bool {
        let mut state = lock_or_recover(&self.state);
        let was_in_flight = state.in_flight.remove(&id);
        let had_completion = state.completed.remove(&id).is_some();
        let old_len = state.pending_main.len();
        state.pending_main.retain(|pending| pending.id != id);
        was_in_flight || had_completion || old_len != state.pending_main.len()
    }

    fn pending_len(&self) -> usize {
        lock_or_recover(&self.state).in_flight.len()
    }

    fn is_bound_thread(&self) -> bool {
        thread::current().id() == self.bound_thread
    }
}

/// Dispatches host requests without hiding main-thread affinity or async work.
pub struct DesktopHost<B: DesktopBackend> {
    backend: Arc<B>,
    queue: MainThreadQueue,
}

impl<B: DesktopBackend> DesktopHost<B> {
    /// Binds host-window work to the calling thread.
    pub fn bind_current_thread(backend: B) -> Self {
        Self {
            backend: Arc::new(backend),
            queue: MainThreadQueue::new(thread::current().id()),
        }
    }

    pub fn backend(&self) -> &B {
        self.backend.as_ref()
    }

    pub fn submit(&self, request: DesktopRequest) -> DesktopSubmission {
        let id = self.queue.allocate();
        match self.backend.execution_lane(&request) {
            ExecutionLane::AnyThread => match self.backend.submit(id, request) {
                BackendSubmission::Completed(result) => {
                    self.queue.finish_immediate(id);
                    DesktopSubmission::Completed(result)
                }
                BackendSubmission::Pending => DesktopSubmission::Pending(id),
            },
            ExecutionLane::HostMainThread => {
                self.queue.queue_main(id, request);
                DesktopSubmission::Pending(id)
            }
        }
    }

    /// Starts queued host-window work and collects asynchronous completions.
    ///
    /// Native players call this from their event-loop thread once per turn.
    pub fn pump_main_thread(&self) -> Result<PumpReport, DesktopError> {
        if !self.queue.is_bound_thread() {
            return Err(DesktopError::MainThreadRequired);
        }

        let pending = self.queue.take_main_requests();
        let started = pending.len();
        let mut completed = 0;
        for pending in pending {
            match self.backend.submit(pending.id, pending.request) {
                BackendSubmission::Completed(result) => {
                    if self.queue.finish(pending.id, result) {
                        completed += 1;
                    }
                }
                BackendSubmission::Pending => {}
            }
        }

        let (async_completed, ignored_stale_completions) = self.collect_backend_completions();
        Ok(PumpReport {
            started,
            completed: completed + async_completed,
            ignored_stale_completions,
        })
    }

    /// Polls one task and consumes its completion.
    pub fn poll(&self, task: DesktopTaskId) -> Option<Result<DesktopResponse, DesktopError>> {
        self.collect_backend_completions();
        self.queue.poll(task)
    }

    /// Cancels queued work and asks the backend to cancel work already started.
    pub fn cancel(&self, task: DesktopTaskId) -> bool {
        let backend_cancelled = self.backend.cancel(task);
        self.queue.cancel_local(task) || backend_cancelled
    }

    pub fn pending_count(&self) -> usize {
        self.queue.pending_len()
    }

    fn collect_backend_completions(&self) -> (usize, usize) {
        let mut completed = 0;
        let mut ignored = 0;
        for BackendCompletion { task, result } in self.backend.drain_completions() {
            if self.queue.finish(task, result) {
                completed += 1;
            } else {
                ignored += 1;
            }
        }
        (completed, ignored)
    }
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
