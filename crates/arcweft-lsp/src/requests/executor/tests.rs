use super::*;

use std::{
    sync::{
        Arc, Condvar, Mutex, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded};
use lsp_server::{Connection, Message, RequestId, Response};
use lsp_types::Uri;

use crate::{
    config::LspConfig, profiles::state::LspProfileState, requests::SignatureRequestBinding,
    session::ArcweftLspSession, uri_key::LspUriKey,
};

const TEST_WAIT: Duration = Duration::from_secs(5);

struct ExecutorFixture {
    registry: Arc<RequestRegistry>,
    executor: Option<SignatureRequestExecutor>,
    client: Option<Connection>,
    profile_state: Arc<LspProfileState>,
}

impl ExecutorFixture {
    fn new() -> Self {
        let (server, client) = Connection::memory();
        let session = Arc::new(RwLock::new(ArcweftLspSession::new(&LspConfig::default())));
        let executor = SignatureRequestExecutor::new(&server, session).expect("signature executor");
        Self {
            registry: RequestRegistry::try_new_with_deadline(Duration::from_mins(1))
                .expect("request registry"),
            executor: Some(executor),
            client: Some(client),
            profile_state: Arc::new(LspProfileState::new()),
        }
    }

    fn executor(&self) -> &SignatureRequestExecutor {
        self.executor.as_ref().expect("live executor")
    }

    fn probe(
        &self,
        id: i32,
        action: impl FnOnce(&Arc<RequestControl>, &Sender<Message>) + Send + 'static,
    ) -> (ExecutorProbeJob, Arc<RequestControl>, Receiver<()>) {
        let active = self
            .registry
            .admit(RequestId::from(id), binding(&self.profile_state, id))
            .expect("probe admission");
        let control = Arc::clone(active.control());
        let (completed, completion) = bounded(1);
        (
            ExecutorProbeJob {
                id: RequestId::from(id),
                active: Some(active),
                action: Some(Box::new(action)),
                completed,
                panic_checkpoint: None,
            },
            control,
            completion,
        )
    }

    fn panic_probe(
        &self,
        id: i32,
    ) -> (
        ExecutorProbeJob,
        Arc<RequestControl>,
        Receiver<()>,
        PanicPublicationPause,
    ) {
        let (mut probe, control, completion) = self.probe(id, |_, _| {
            panic!("deterministic executor probe panic");
        });
        let (reached, caught) = bounded(1);
        let (resume, resumed) = bounded(1);
        probe.panic_checkpoint = Some(ExecutorPanicCheckpoint {
            reached,
            resume: resumed,
        });
        (
            probe,
            control,
            completion,
            PanicPublicationPause {
                caught,
                resume: Some(resume),
            },
        )
    }

    fn submit(&self, probe: ExecutorProbeJob) {
        self.executor()
            .submit_probe(probe)
            .expect("open worker queue");
    }

    fn client(&self) -> &Connection {
        self.client.as_ref().expect("connected client")
    }
}

struct PanicPublicationPause {
    caught: Receiver<()>,
    resume: Option<Sender<()>>,
}

impl PanicPublicationPause {
    fn wait_until_caught(&self) {
        self.caught
            .recv_timeout(TEST_WAIT)
            .expect("worker panic reaches final publication");
    }

    fn release(mut self) {
        if let Some(resume) = self.resume.take() {
            let _ = resume.send(());
        }
    }
}

impl Drop for PanicPublicationPause {
    fn drop(&mut self) {
        if let Some(resume) = self.resume.take() {
            let _ = resume.send(());
        }
    }
}

impl Drop for ExecutorFixture {
    fn drop(&mut self) {
        if let Some(executor) = self.executor.take() {
            executor.shutdown();
        }
        self.registry.shutdown();
    }
}

struct BlockedWorkers {
    release: Arc<(Mutex<bool>, Condvar)>,
    completions: Vec<Receiver<()>>,
    running: Arc<AtomicUsize>,
    maximum: Arc<AtomicUsize>,
}

impl BlockedWorkers {
    fn release(&self) {
        let (released, changed) = self.release.as_ref();
        *released.lock().unwrap_or_else(PoisonError::into_inner) = true;
        changed.notify_all();
    }

    fn wait(self) {
        self.release();
        for completion in &self.completions {
            completion
                .recv_timeout(TEST_WAIT)
                .expect("blocked worker completion");
        }
    }
}

impl Drop for BlockedWorkers {
    fn drop(&mut self) {
        self.release();
    }
}

fn binding(state: &Arc<LspProfileState>, suffix: i32) -> SignatureRequestBinding {
    let uri = format!("file:///workspace/executor-{suffix}.arcw")
        .parse::<Uri>()
        .expect("URI");
    let workspace = "file:///workspace".parse::<Uri>().expect("workspace URI");
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new(uri.to_string()).expect("document ID"),
        SourceName::path(uri.to_string()),
        format!("flow @flow.executor_{suffix} executor_{suffix} {{}}"),
    )
    .expect("source document");
    SignatureRequestBinding::for_test(
        LspUriKey::from_uri(&uri),
        LspUriKey::from_uri(&workspace),
        state,
        document.identity().clone(),
    )
}

fn occupy_every_worker(fixture: &ExecutorFixture, first_id: i32) -> BlockedWorkers {
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let running = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let (started, starts) = bounded(SIGNATURE_WORKER_COUNT);
    let mut completions = Vec::with_capacity(SIGNATURE_WORKER_COUNT);

    for offset in 0..SIGNATURE_WORKER_COUNT {
        let release_for_job = Arc::clone(&release);
        let running_for_job = Arc::clone(&running);
        let maximum_for_job = Arc::clone(&maximum);
        let started_for_job = started.clone();
        let id = first_id + i32::try_from(offset).expect("worker offset fits i32");
        let (probe, _, completion) = fixture.probe(id, move |_, _| {
            let concurrent = running_for_job.fetch_add(1, Ordering::AcqRel) + 1;
            maximum_for_job.fetch_max(concurrent, Ordering::AcqRel);
            started_for_job.send(()).expect("start observer");
            let (released, changed) = release_for_job.as_ref();
            let mut released = released.lock().unwrap_or_else(PoisonError::into_inner);
            while !*released {
                released = changed
                    .wait(released)
                    .unwrap_or_else(PoisonError::into_inner);
            }
            running_for_job.fetch_sub(1, Ordering::AcqRel);
        });
        fixture.submit(probe);
        completions.push(completion);
    }
    drop(started);
    for _ in 0..SIGNATURE_WORKER_COUNT {
        starts
            .recv_timeout(TEST_WAIT)
            .expect("all fixed workers started");
    }

    BlockedWorkers {
        release,
        completions,
        running,
        maximum,
    }
}

fn finished_response(id: i32) -> Message {
    Message::Response(Response::new_ok(
        RequestId::from(id),
        serde_json::Value::Null,
    ))
}

#[test]
fn worker_pool_runs_exactly_four_jobs_concurrently() {
    let fixture = ExecutorFixture::new();
    assert_eq!(fixture.executor().worker_count(), SIGNATURE_WORKER_COUNT);
    let blocked = occupy_every_worker(&fixture, 100);
    assert_eq!(
        blocked.running.load(Ordering::Acquire),
        SIGNATURE_WORKER_COUNT
    );
    assert_eq!(
        blocked.maximum.load(Ordering::Acquire),
        SIGNATURE_WORKER_COUNT
    );

    let running = Arc::clone(&blocked.running);
    let maximum = Arc::clone(&blocked.maximum);
    let (fifth_started, fifth_start) = bounded(1);
    let (fifth, _, fifth_completion) = fixture.probe(104, move |_, _| {
        let concurrent = running.fetch_add(1, Ordering::AcqRel) + 1;
        maximum.fetch_max(concurrent, Ordering::AcqRel);
        fifth_started.send(()).expect("fifth start observer");
        running.fetch_sub(1, Ordering::AcqRel);
    });
    fixture.submit(fifth);

    assert_eq!(fifth_start.try_recv(), Err(TryRecvError::Empty));
    blocked.release();
    fifth_start
        .recv_timeout(TEST_WAIT)
        .expect("fifth job starts after one worker is released");
    fifth_completion
        .recv_timeout(TEST_WAIT)
        .expect("fifth job completion");
    assert_eq!(
        blocked.maximum.load(Ordering::Acquire),
        SIGNATURE_WORKER_COUNT
    );
    blocked.wait();
    assert_eq!(
        fixture.registry.test_snapshot(),
        crate::requests::registry::RequestRegistryTestSnapshot {
            active: 0,
            deadlines: 0,
            fired: 0,
        }
    );
}

#[test]
fn queued_client_cancellation_reaches_no_semantic_work_or_result() {
    let fixture = ExecutorFixture::new();
    let blocked = occupy_every_worker(&fixture, 200);
    let semantic_work = Arc::new(AtomicUsize::new(0));
    let semantic_work_for_job = Arc::clone(&semantic_work);
    let (queued, control, completion) = fixture.probe(204, move |control, _| {
        if *control.gate() == RequestGateState::Active {
            semantic_work_for_job.fetch_add(1, Ordering::AcqRel);
        }
    });
    fixture.submit(queued);

    fixture.registry.cancel(
        &RequestId::from(204),
        SignatureCancellationReason::ClientCancelled,
    );
    assert_eq!(
        *control.gate(),
        RequestGateState::Cancelled(SignatureCancellationReason::ClientCancelled)
    );
    blocked.release();
    completion
        .recv_timeout(TEST_WAIT)
        .expect("cancelled queued job completion");
    blocked.wait();

    assert_eq!(semantic_work.load(Ordering::Acquire), 0);
    assert!(matches!(
        fixture.client().receiver.try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert_eq!(fixture.registry.test_snapshot().active, 0);
    assert_eq!(fixture.registry.test_snapshot().deadlines, 0);
}

#[test]
fn queued_deadline_includes_queue_time_and_reaches_no_semantic_work() {
    let fixture = ExecutorFixture::new();
    let semantic_work = Arc::new(AtomicUsize::new(0));
    let semantic_work_for_job = Arc::clone(&semantic_work);
    let (queued, control, completion) = fixture.probe(304, move |control, _| {
        if *control.gate() == RequestGateState::Active {
            semantic_work_for_job.fetch_add(1, Ordering::AcqRel);
        }
    });
    let blocked = occupy_every_worker(&fixture, 300);
    fixture.submit(queued);

    assert_eq!(fixture.registry.fire_deadlines_at(control.deadline()), 1);
    assert_eq!(
        *control.gate(),
        RequestGateState::Cancelled(SignatureCancellationReason::DeadlineExceeded)
    );
    blocked.release();
    completion
        .recv_timeout(TEST_WAIT)
        .expect("expired queued job completion");
    blocked.wait();

    assert_eq!(semantic_work.load(Ordering::Acquire), 0);
    assert_eq!(fixture.registry.test_snapshot().active, 0);
    assert_eq!(fixture.registry.test_snapshot().deadlines, 0);
}

#[test]
fn running_cancellation_is_seen_before_worker_cleanup() {
    let fixture = ExecutorFixture::new();
    let (started, start) = bounded(1);
    let (resume, resumed) = bounded(1);
    let cancellation_observed = Arc::new(AtomicUsize::new(0));
    let observed_for_job = Arc::clone(&cancellation_observed);
    let (probe, control, completion) = fixture.probe(400, move |control, _| {
        started.send(()).expect("running observer");
        resumed.recv().expect("cancellation release");
        if control.cancellation_flag().load(Ordering::Acquire) {
            observed_for_job.fetch_add(1, Ordering::AcqRel);
        }
    });
    fixture.submit(probe);
    start
        .recv_timeout(TEST_WAIT)
        .expect("probe reached running work");

    fixture.registry.cancel(
        &RequestId::from(400),
        SignatureCancellationReason::ClientCancelled,
    );
    resume.send(()).expect("release running probe");
    completion
        .recv_timeout(TEST_WAIT)
        .expect("running cancellation cleanup");

    assert_eq!(cancellation_observed.load(Ordering::Acquire), 1);
    assert_eq!(
        *control.gate(),
        RequestGateState::Cancelled(SignatureCancellationReason::ClientCancelled)
    );
    assert_eq!(fixture.registry.test_snapshot().active, 0);
    assert_eq!(fixture.registry.test_snapshot().deadlines, 0);
}

#[test]
fn running_deadline_is_seen_before_worker_cleanup() {
    let fixture = ExecutorFixture::new();
    let (started, start) = bounded(1);
    let (resume, resumed) = bounded(1);
    let deadline_observed = Arc::new(AtomicUsize::new(0));
    let observed_for_job = Arc::clone(&deadline_observed);
    let (probe, control, completion) = fixture.probe(500, move |control, _| {
        started.send(()).expect("running observer");
        resumed.recv().expect("deadline release");
        if control.cancellation_flag().load(Ordering::Acquire) {
            observed_for_job.fetch_add(1, Ordering::AcqRel);
        }
    });
    fixture.submit(probe);
    start
        .recv_timeout(TEST_WAIT)
        .expect("probe reached running work");

    assert_eq!(fixture.registry.fire_deadlines_at(control.deadline()), 1);
    resume.send(()).expect("release running probe");
    completion
        .recv_timeout(TEST_WAIT)
        .expect("running deadline cleanup");

    assert_eq!(deadline_observed.load(Ordering::Acquire), 1);
    assert_eq!(
        *control.gate(),
        RequestGateState::Cancelled(SignatureCancellationReason::DeadlineExceeded)
    );
    assert_eq!(fixture.registry.test_snapshot().active, 0);
    assert_eq!(fixture.registry.test_snapshot().deadlines, 0);
}

#[test]
fn normal_and_error_publication_paths_finish_and_cleanup() {
    for (id, response) in [
        (600, finished_response(600)),
        (
            601,
            Message::Response(Response::new_err(
                RequestId::from(601),
                ErrorCode::RequestFailed as i32,
                "typed probe failure".to_owned(),
            )),
        ),
    ] {
        let fixture = ExecutorFixture::new();
        let (probe, control, completion) = fixture.probe(id, move |control, responses| {
            let mut gate = control.gate();
            if *gate == RequestGateState::Active && responses.send(response).is_ok() {
                *gate = RequestGateState::Finished;
            }
        });
        fixture.submit(probe);
        completion
            .recv_timeout(TEST_WAIT)
            .expect("published probe cleanup");
        let message = fixture
            .client()
            .receiver
            .recv_timeout(TEST_WAIT)
            .expect("probe response");
        let Message::Response(response) = message else {
            panic!("expected response message");
        };
        assert_eq!(response.id, RequestId::from(id));
        assert_eq!(*control.gate(), RequestGateState::Finished);
        assert_eq!(fixture.registry.test_snapshot().active, 0);
        assert_eq!(fixture.registry.test_snapshot().deadlines, 0);
    }
}

#[test]
fn response_send_failure_neither_finishes_nor_retains_the_request() {
    let mut fixture = ExecutorFixture::new();
    drop(fixture.client.take());
    let cache_insertions = Arc::new(AtomicUsize::new(0));
    let insertions_for_job = Arc::clone(&cache_insertions);
    let (probe, control, completion) = fixture.probe(700, move |control, responses| {
        let mut gate = control.gate();
        if *gate == RequestGateState::Active && responses.send(finished_response(700)).is_ok() {
            insertions_for_job.fetch_add(1, Ordering::AcqRel);
            *gate = RequestGateState::Finished;
        }
    });
    fixture.submit(probe);
    completion
        .recv_timeout(TEST_WAIT)
        .expect("send-failed probe cleanup");

    assert_eq!(cache_insertions.load(Ordering::Acquire), 0);
    assert_eq!(*control.gate(), RequestGateState::Active);
    assert_eq!(fixture.registry.test_snapshot().active, 0);
    assert_eq!(fixture.registry.test_snapshot().deadlines, 0);
}

#[test]
fn closed_queue_drops_the_unsubmitted_guard_without_queue_or_deadline_leak() {
    let fixture = ExecutorFixture::new();
    fixture.executor().shared.close_queue();
    let (probe, _, completion) = fixture.probe(800, |_, _| {});
    assert_eq!(fixture.registry.test_snapshot().active, 1);
    assert_eq!(fixture.registry.test_snapshot().deadlines, 1);

    assert!(matches!(
        fixture.executor().submit_probe(probe),
        Err(RequestAdmissionError::QueueClosed)
    ));
    assert!(matches!(
        completion.recv_timeout(TEST_WAIT),
        Err(crossbeam_channel::RecvTimeoutError::Disconnected)
    ));
    assert_eq!(fixture.registry.test_snapshot().active, 0);
    assert_eq!(fixture.registry.test_snapshot().deadlines, 0);
}

#[test]
fn worker_panic_sends_one_internal_error_and_cleans_exactly_once() {
    let fixture = ExecutorFixture::new();
    let (probe, control, completion, pause) = fixture.panic_probe(900);
    fixture.submit(probe);
    pause.wait_until_caught();

    assert_eq!(fixture.registry.test_snapshot().active, 1);
    assert_eq!(fixture.registry.test_snapshot().deadlines, 1);
    pause.release();

    let message = fixture
        .client()
        .receiver
        .recv_timeout(TEST_WAIT)
        .expect("panic response");
    let Message::Response(response) = message else {
        panic!("expected response message");
    };
    let error = response.error.expect("internal error");
    assert_eq!(response.id, RequestId::from(900));
    assert_eq!(error.code, ErrorCode::InternalError as i32);
    assert_eq!(error.message, "signature worker panicked");
    assert!(matches!(
        fixture.client().receiver.try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert!(matches!(
        completion.recv_timeout(TEST_WAIT),
        Err(crossbeam_channel::RecvTimeoutError::Disconnected)
    ));
    assert_eq!(*control.gate(), RequestGateState::Finished);
    assert_eq!(fixture.registry.test_snapshot().active, 0);
    assert_eq!(fixture.registry.test_snapshot().deadlines, 0);
}

#[test]
fn caught_worker_panic_keeps_active_guard_until_client_cancel_linearizes() {
    let fixture = ExecutorFixture::new();
    let (probe, control, completion, pause) = fixture.panic_probe(910);
    fixture.submit(probe);
    pause.wait_until_caught();

    assert_eq!(fixture.registry.test_snapshot().active, 1);
    assert_eq!(fixture.registry.test_snapshot().deadlines, 1);
    assert!(matches!(
        fixture
            .registry
            .admit(RequestId::from(910), binding(&fixture.profile_state, 911)),
        Err(RequestAdmissionError::DuplicateRequestId { .. })
    ));
    fixture.registry.cancel(
        &RequestId::from(910),
        SignatureCancellationReason::ClientCancelled,
    );
    assert_eq!(
        *control.gate(),
        RequestGateState::Cancelled(SignatureCancellationReason::ClientCancelled)
    );

    pause.release();
    assert!(matches!(
        completion.recv_timeout(TEST_WAIT),
        Err(crossbeam_channel::RecvTimeoutError::Disconnected)
    ));
    assert!(matches!(
        fixture.client().receiver.try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert_eq!(fixture.registry.test_snapshot().active, 0);
    assert_eq!(fixture.registry.test_snapshot().deadlines, 0);
}

#[test]
fn caught_worker_panic_keeps_active_guard_until_remap_or_shutdown_linearizes() {
    for (id, reason) in [
        (920, SignatureCancellationReason::ProfileRemapped),
        (930, SignatureCancellationReason::SessionShutdown),
    ] {
        let fixture = ExecutorFixture::new();
        let (probe, control, completion, pause) = fixture.panic_probe(id);
        fixture.submit(probe);
        pause.wait_until_caught();

        assert_eq!(fixture.registry.test_snapshot().active, 1);
        assert_eq!(fixture.registry.test_snapshot().deadlines, 1);
        match reason {
            SignatureCancellationReason::ProfileRemapped => fixture
                .registry
                .cancel_profile_state(&fixture.profile_state, reason),
            SignatureCancellationReason::SessionShutdown => {
                fixture.registry.close_admission();
                fixture.registry.cancel_all(reason);
            }
            _ => unreachable!("test enumerates remap and shutdown only"),
        }
        assert_eq!(*control.gate(), RequestGateState::Cancelled(reason));

        pause.release();
        assert!(matches!(
            completion.recv_timeout(TEST_WAIT),
            Err(crossbeam_channel::RecvTimeoutError::Disconnected)
        ));
        assert!(matches!(
            fixture.client().receiver.try_recv(),
            Err(TryRecvError::Empty)
        ));
        assert_eq!(fixture.registry.test_snapshot().active, 0);
        assert_eq!(fixture.registry.test_snapshot().deadlines, 0);
    }
}
