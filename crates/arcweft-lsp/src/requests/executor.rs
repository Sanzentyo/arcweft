//! Fixed-size FIFO signature executor and its coordinated shutdown owner.

use std::{
    collections::VecDeque,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Condvar, Mutex, PoisonError, RwLock},
    thread::{self, JoinHandle},
};

use lsp_server::{Connection, ErrorCode, Message, Response};
use thiserror::Error;

use crate::session::ArcweftLspSession;

use super::{
    RequestAdmissionError, RequestGateState, RequestRegistry, SignatureCancellationReason,
    signature::{PreparedSignatureRequest, SignatureRequestWork},
};

pub(crate) const SIGNATURE_WORKER_COUNT: usize = 4;

#[derive(Debug)]
struct SignatureRequestExecutor {
    shared: Arc<SignatureExecutorShared>,
    workers: Mutex<Vec<JoinHandle<()>>>,
}

#[derive(Debug)]
struct SignatureExecutorShared {
    queue: Mutex<SignatureExecutorQueue>,
    available: Condvar,
    session: Arc<RwLock<ArcweftLspSession>>,
    responses: crossbeam_channel::Sender<Message>,
}

#[derive(Debug)]
struct SignatureExecutorQueue {
    closed: bool,
    jobs: VecDeque<PreparedSignatureRequest>,
}

/// Owns the only registry, deadline scheduler, and worker pool for one connection.
#[derive(Debug)]
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
        let mut queue = self
            .shared
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if queue.closed {
            return Err(RequestAdmissionError::QueueClosed);
        }
        queue.jobs.push_back(request);
        self.shared.available.notify_one();
        Ok(())
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
        while let Some(request) = self.take() {
            let id = request.request_id().clone();
            let control = request.control_arc();
            if catch_unwind(AssertUnwindSafe(|| self.execute(request))).is_err() {
                let mut gate = control.gate();
                if *gate == RequestGateState::Active
                    && self
                        .responses
                        .send(Message::Response(Response::new_err(
                            id,
                            ErrorCode::InternalError as i32,
                            "signature worker panicked".to_owned(),
                        )))
                        .is_ok()
                {
                    *gate = RequestGateState::Finished;
                }
            }
        }
    }

    fn take(&self) -> Option<PreparedSignatureRequest> {
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

    #[allow(
        clippy::needless_pass_by_value,
        reason = "the worker owns the prepared request and its active cleanup guard for the entire execution"
    )]
    fn execute(&self, request: PreparedSignatureRequest) {
        let work = {
            let session = self.session.read().unwrap_or_else(PoisonError::into_inner);
            session.signature_work(&request)
        };
        let result = match work {
            Ok(SignatureRequestWork::Hit(result)) => Ok(result),
            Ok(SignatureRequestWork::Miss(key)) => {
                ArcweftLspSession::compute_signature(&request, key)
            }
            Err(error) => Err(error),
        };
        let session = self.session.read().unwrap_or_else(PoisonError::into_inner);
        session.publish_signature_result(&request, result, &self.responses);
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
