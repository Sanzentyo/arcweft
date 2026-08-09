//! Fixed-size FIFO signature executor and its coordinated shutdown owner.

use std::{
    collections::VecDeque,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Condvar, Mutex, PoisonError, RwLock},
    thread::{self, JoinHandle},
};

use lsp_server::{Connection, Message};
use thiserror::Error;

#[cfg(test)]
use lsp_server::{ErrorCode, Response};

use crate::session::ArcweftLspSession;

#[cfg(test)]
use super::{ActiveRequest, RequestControl, RequestGateState};
use super::{
    RequestAdmissionError, RequestRegistry, SignatureCancellationReason,
    signature::{PreparedSignatureRequest, SignatureRequestWork},
};

pub(crate) const SIGNATURE_WORKER_COUNT: usize = 4;

#[cfg(not(test))]
type SignatureExecutorJob = PreparedSignatureRequest;

#[cfg(test)]
enum SignatureExecutorJob {
    Request(Box<PreparedSignatureRequest>),
    Probe(ExecutorProbeJob),
}

#[cfg(test)]
type ExecutorProbeAction =
    Box<dyn FnOnce(&Arc<RequestControl>, &crossbeam_channel::Sender<Message>) + Send + 'static>;

#[cfg(test)]
struct ExecutorProbeJob {
    id: lsp_server::RequestId,
    active: Option<ActiveRequest>,
    action: Option<ExecutorProbeAction>,
    completed: crossbeam_channel::Sender<()>,
    panic_checkpoint: Option<ExecutorPanicCheckpoint>,
}

#[cfg(test)]
#[derive(Debug)]
struct ExecutorPanicCheckpoint {
    reached: crossbeam_channel::Sender<()>,
    resume: crossbeam_channel::Receiver<()>,
}

#[cfg(test)]
impl std::fmt::Debug for ExecutorProbeJob {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExecutorProbeJob")
            .field("id", &self.id)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

struct SignatureRequestExecutor {
    shared: Arc<SignatureExecutorShared>,
    workers: Mutex<Vec<JoinHandle<()>>>,
}

struct SignatureExecutorShared {
    queue: Mutex<SignatureExecutorQueue>,
    available: Condvar,
    session: Arc<RwLock<ArcweftLspSession>>,
    responses: crossbeam_channel::Sender<Message>,
}

struct SignatureExecutorQueue {
    closed: bool,
    jobs: VecDeque<SignatureExecutorJob>,
}

/// Owns the only registry, deadline scheduler, and worker pool for one connection.
pub(crate) struct SignatureRequestRuntime {
    registry: Arc<RequestRegistry>,
    executor: SignatureRequestExecutor,
}

/// Request-runtime construction failed before protocol intake began.
#[derive(Debug, Error)]
pub(crate) enum RequestRuntimeError {
    #[error("failed to spawn a signature worker")]
    WorkerSpawn(#[source] std::io::Error),
    #[error("failed to spawn the signature deadline scheduler")]
    DeadlineSchedulerSpawn(#[source] std::io::Error),
}

impl SignatureRequestRuntime {
    pub(crate) fn new(
        connection: &Connection,
        session: Arc<RwLock<ArcweftLspSession>>,
    ) -> Result<Self, RequestRuntimeError> {
        let registry =
            RequestRegistry::try_new().map_err(RequestRuntimeError::DeadlineSchedulerSpawn)?;
        let executor = SignatureRequestExecutor::new(connection, session)?;
        Ok(Self { registry, executor })
    }

    #[cfg(test)]
    pub(crate) fn new_with_deadline_for_test(
        connection: &Connection,
        session: Arc<RwLock<ArcweftLspSession>>,
        request_deadline: std::time::Duration,
    ) -> Result<Self, RequestRuntimeError> {
        let registry = RequestRegistry::try_new_with_deadline(request_deadline)
            .map_err(RequestRuntimeError::DeadlineSchedulerSpawn)?;
        let executor = SignatureRequestExecutor::new(connection, session)?;
        Ok(Self { registry, executor })
    }

    pub(crate) const fn registry(&self) -> &Arc<RequestRegistry> {
        &self.registry
    }

    pub(crate) fn submit(
        &self,
        request: PreparedSignatureRequest,
    ) -> Result<(), RequestAdmissionError> {
        self.executor.submit(request)
    }

    pub(crate) fn shutdown(self) {
        self.registry.close_admission();
        self.registry
            .cancel_all(SignatureCancellationReason::SessionShutdown);
        self.executor.shutdown();
        self.registry.shutdown();
    }
}

impl SignatureRequestExecutor {
    fn new(
        connection: &Connection,
        session: Arc<RwLock<ArcweftLspSession>>,
    ) -> Result<Self, RequestRuntimeError> {
        let shared = Arc::new(SignatureExecutorShared {
            queue: Mutex::new(SignatureExecutorQueue {
                closed: false,
                jobs: VecDeque::new(),
            }),
            available: Condvar::new(),
            session,
            responses: connection.sender.clone(),
        });
        let mut workers = Vec::with_capacity(SIGNATURE_WORKER_COUNT);
        for index in 0..SIGNATURE_WORKER_COUNT {
            let worker_shared = Arc::clone(&shared);
            match thread::Builder::new()
                .name(format!("arcweft-signature-{index}"))
                .spawn(move || worker_shared.run())
            {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    shared.close_queue();
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(RequestRuntimeError::WorkerSpawn(error));
                }
            }
        }
        Ok(Self {
            shared,
            workers: Mutex::new(workers),
        })
    }

    fn submit(&self, request: PreparedSignatureRequest) -> Result<(), RequestAdmissionError> {
        self.submit_job(signature_job(request))
    }

    fn submit_job(&self, job: SignatureExecutorJob) -> Result<(), RequestAdmissionError> {
        let mut queue = self
            .shared
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if queue.closed {
            return Err(RequestAdmissionError::QueueClosed);
        }
        queue.jobs.push_back(job);
        self.shared.available.notify_one();
        Ok(())
    }

    #[cfg(test)]
    fn submit_probe(&self, job: ExecutorProbeJob) -> Result<(), RequestAdmissionError> {
        self.submit_job(SignatureExecutorJob::Probe(job))
    }

    #[cfg(test)]
    fn worker_count(&self) -> usize {
        self.workers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    fn shutdown(self) {
        self.shared.close_queue();
        let workers = self
            .workers
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        for worker in workers {
            let _ = worker.join();
        }
    }
}

impl SignatureExecutorShared {
    fn run(&self) {
        while let Some(mut job) = self.take() {
            if catch_unwind(AssertUnwindSafe(|| self.execute_job(&mut job))).is_err() {
                #[cfg(test)]
                wait_at_panic_checkpoint(&job);
                self.publish_worker_panic(&job);
            }
        }
    }

    /// Linearizes a caught panic while the job still owns its active registry guard.
    #[cfg(not(test))]
    fn publish_worker_panic(&self, job: &SignatureExecutorJob) {
        let session = self.session.read().unwrap_or_else(PoisonError::into_inner);
        session.publish_signature_worker_panic(job, &self.responses);
    }

    /// Test builds retain probe jobs, but real prepared requests still use the
    /// complete session/profile/stamp/deadline publication authority.
    #[cfg(test)]
    fn publish_worker_panic(&self, job: &SignatureExecutorJob) {
        match job {
            SignatureExecutorJob::Request(request) => {
                if request.observes_session_authority() {
                    let observation = match self.session.try_read() {
                        Ok(session) => {
                            drop(session);
                            super::signature::SignatureSessionAuthorityObservation::Available
                        }
                        Err(std::sync::TryLockError::WouldBlock) => {
                            super::signature::SignatureSessionAuthorityObservation::Blocked
                        }
                        Err(std::sync::TryLockError::Poisoned(error)) => {
                            drop(error.into_inner());
                            super::signature::SignatureSessionAuthorityObservation::Available
                        }
                    };
                    request.record_session_authority(observation);
                }
                let session = self.session.read().unwrap_or_else(PoisonError::into_inner);
                session.publish_signature_worker_panic(request, &self.responses);
            }
            SignatureExecutorJob::Probe(probe) => self.publish_probe_worker_panic(probe),
        }
    }

    #[cfg(test)]
    fn publish_probe_worker_panic(&self, probe: &ExecutorProbeJob) {
        let control = Arc::clone(
            probe
                .active
                .as_ref()
                .expect("panicking probe retains active authority until publication")
                .control(),
        );
        let mut gate = control.gate();
        if *gate == RequestGateState::Active
            && !control
                .cancellation_flag()
                .load(std::sync::atomic::Ordering::Acquire)
            && std::time::Instant::now() < control.deadline()
            && self
                .responses
                .send(Message::Response(Response::new_err(
                    probe.id.clone(),
                    ErrorCode::InternalError as i32,
                    "signature worker panicked".to_owned(),
                )))
                .is_ok()
        {
            *gate = RequestGateState::Finished;
        }
    }

    fn take(&self) -> Option<SignatureExecutorJob> {
        let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if let Some(request) = queue.jobs.pop_front() {
                return Some(request);
            }
            if queue.closed {
                return None;
            }
            queue = self
                .available
                .wait(queue)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn execute_prepared(&self, request: &PreparedSignatureRequest) {
        #[cfg(test)]
        request.trigger_executor_fault(super::signature::SignatureExecutorFaultPoint::BeforeWork);
        let work = {
            let session = self.session.read().unwrap_or_else(PoisonError::into_inner);
            session.signature_work(request)
        };
        let result = match work {
            Ok(SignatureRequestWork::Hit(result)) => Ok(result),
            Ok(SignatureRequestWork::Miss(key)) => {
                ArcweftLspSession::compute_signature(request, key)
            }
            Err(error) => Err(error),
        };
        let session = self.session.read().unwrap_or_else(PoisonError::into_inner);
        session.publish_signature_result(request, result, &self.responses);
    }

    #[cfg(not(test))]
    fn execute_job(&self, request: &mut SignatureExecutorJob) {
        self.execute_prepared(request);
    }

    #[cfg(test)]
    fn execute_job(&self, job: &mut SignatureExecutorJob) {
        match job {
            SignatureExecutorJob::Request(request) => self.execute_prepared(request.as_ref()),
            SignatureExecutorJob::Probe(probe) => {
                let action = probe.action.take().expect("probe action executes once");
                action(
                    probe
                        .active
                        .as_ref()
                        .expect("probe retains active authority during execution")
                        .control(),
                    &self.responses,
                );
                drop(probe.active.take());
                let _ = probe.completed.send(());
            }
        }
    }

    fn close_queue(&self) {
        let queued = {
            let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
            queue.closed = true;
            self.available.notify_all();
            std::mem::take(&mut queue.jobs)
        };
        drop(queued);
    }
}

#[cfg(not(test))]
fn signature_job(request: PreparedSignatureRequest) -> SignatureExecutorJob {
    request
}

#[cfg(test)]
fn signature_job(request: PreparedSignatureRequest) -> SignatureExecutorJob {
    SignatureExecutorJob::Request(Box::new(request))
}

#[cfg(test)]
fn wait_at_panic_checkpoint(job: &SignatureExecutorJob) {
    match job {
        SignatureExecutorJob::Request(request) => request.wait_at_executor_panic_checkpoint(),
        SignatureExecutorJob::Probe(probe) => {
            let Some(checkpoint) = &probe.panic_checkpoint else {
                return;
            };
            let _ = checkpoint.reached.send(());
            let _ = checkpoint.resume.recv();
        }
    }
}

#[cfg(test)]
mod tests;
