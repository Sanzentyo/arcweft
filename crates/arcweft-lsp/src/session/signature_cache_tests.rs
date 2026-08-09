use std::{
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use lsp_server::{Connection, ErrorCode, Message, Notification, RequestId, Response};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams, Position,
    SignatureHelpParams, TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, VersionedTextDocumentIdentifier, WorkDoneProgressParams,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument,
        Notification as LspNotification,
    },
};

use super::{
    ArcweftLspSession,
    tests::{TestProject, file_uri, open_text, position_after},
};
use crate::{
    config::LspConfig,
    profiles::state::{
        AcceptedOverlayEntry, AcceptedOverlaySet, AcceptedOverlaySetError, AcceptedProfileCandidate,
    },
    requests::{
        SignatureCancellationReason, SignatureRequestRuntime,
        registry::SIGNATURE_REQUEST_DEADLINE,
        signature::{
            PreparedSignatureRequest, SignatureAcquireError, SignatureExecutorFaultPoint,
            SignatureExecutorTestControl, SignatureRequestError, SignatureRequestStale,
            SignatureSessionAuthorityObservation,
        },
    },
};

const TEST_WAIT: Duration = Duration::from_secs(5);

const MANIFEST: &str = r#"
schema = 1

[package]
id = "org.arcweft.tests.lsp.signature-cache"
version = "0.1.0"

[profiles.dev]
kind = "server"
entry = "@entry.server.main"
source = "src/main.arcw"
adapter = "sans-io"
"#;

const SOURCE: &str = "fn sum(lhs: i64, rhs: i64) -> i64 { lhs + rhs }\n\
fn evaluate(value: i64) -> i64 {\n    sum(value, value)\n}\n\
entry server @entry.server.main { goto @flow.main }\n\
flow @flow.main main {}\n";

const REMAPPED_MANIFEST: &str = r#"
schema = 1

[package]
id = "org.arcweft.tests.lsp.signature-cache"
version = "0.1.0"

[profiles.dev]
kind = "server"
entry = "@entry.server.main"
source = "src/main.arcw"
adapter = "sans-io"

[profiles.alt]
kind = "server"
entry = "@entry.server.main"
source = "src/main.arcw"
adapter = "sans-io"
"#;

const MULTI_ROOT_SOURCE: &str = "use crate.feature.feature_evaluate\n\
fn sum(lhs: i64, rhs: i64) -> i64 { lhs + rhs }\n\
fn evaluate(value: i64) -> i64 {\n    sum(value, value)\n}\n\
entry server @entry.server.main { goto @flow.main }\n\
flow @flow.main main {}\n";

const MULTI_FEATURE_SOURCE: &str = "mod crate.feature\n\
fn product(lhs: i64, rhs: i64) -> i64 { lhs * rhs }\n\
pub fn feature_evaluate(value: i64) -> i64 {\n    product(value, value)\n}\n";

struct SignatureCacheFixture {
    project: TestProject,
    uri: lsp_types::Uri,
    session: Arc<RwLock<ArcweftLspSession>>,
    runtime: Option<SignatureRequestRuntime>,
    client: Connection,
}

struct PreparedFaultPause {
    caught: crossbeam_channel::Receiver<()>,
    resume: Option<crossbeam_channel::Sender<()>>,
    completed: crossbeam_channel::Receiver<()>,
    session_authority: Option<crossbeam_channel::Receiver<SignatureSessionAuthorityObservation>>,
}

impl PreparedFaultPause {
    fn wait_until_caught(&self) {
        self.caught
            .recv_timeout(TEST_WAIT)
            .expect("prepared worker fault reached panic publication");
    }

    fn release(&mut self) {
        if let Some(resume) = self.resume.take() {
            let _ = resume.send(());
        }
    }

    fn wait_until_completed(&self) {
        self.completed
            .recv_timeout(TEST_WAIT)
            .expect("prepared worker fault completed cleanup");
    }

    fn session_authority_observation(&self) -> SignatureSessionAuthorityObservation {
        self.session_authority
            .as_ref()
            .expect("session authority observer")
            .recv_timeout(TEST_WAIT)
            .expect("panic publisher observed session authority")
    }
}

impl Drop for PreparedFaultPause {
    fn drop(&mut self) {
        self.release();
    }
}

fn install_prepared_fault(
    prepared: &mut PreparedSignatureRequest,
    fault: SignatureExecutorFaultPoint,
    observe_session_authority: bool,
) -> PreparedFaultPause {
    let (caught, caught_rx) = crossbeam_channel::bounded(1);
    let (resume, resume_rx) = crossbeam_channel::bounded(1);
    let (completed, completed_rx) = crossbeam_channel::bounded(1);
    let (session_authority, session_authority_rx) = crossbeam_channel::bounded(1);
    let mut control = SignatureExecutorTestControl::new(fault, caught, resume_rx, completed);
    if observe_session_authority {
        control = control.with_session_authority_observer(session_authority);
    }
    prepared.install_executor_test_control(control);
    PreparedFaultPause {
        caught: caught_rx,
        resume: Some(resume),
        completed: completed_rx,
        session_authority: observe_session_authority.then_some(session_authority_rx),
    }
}

impl SignatureCacheFixture {
    fn new(name: &str) -> Self {
        Self::new_with_deadline(name, SIGNATURE_REQUEST_DEADLINE)
    }

    fn new_with_deadline(name: &str, request_deadline: Duration) -> Self {
        Self::new_with_source_tree(name, SOURCE, &[], request_deadline)
    }

    fn new_with_source_tree(
        name: &str,
        root_source: &str,
        extra_sources: &[(&str, &str)],
        request_deadline: Duration,
    ) -> Self {
        let project = TestProject::new(name);
        project.write("arcw.toml", MANIFEST);
        project.write("src/main.arcw", root_source);
        for (path, source) in extra_sources {
            project.write(path, source);
        }
        let uri = file_uri(&project.path("src/main.arcw"));
        let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
        open_text(&mut session, uri.clone(), root_source);
        let session = Arc::new(RwLock::new(session));
        let (server, client) = Connection::memory();
        let runtime = SignatureRequestRuntime::new_with_deadline_for_test(
            &server,
            Arc::clone(&session),
            request_deadline,
        )
        .expect("signature request runtime");
        Self {
            project,
            uri,
            session,
            runtime: Some(runtime),
            client,
        }
    }

    fn prepare(&self, request_id: i32, position: Position) -> PreparedSignatureRequest {
        self.prepare_for_uri(request_id, self.uri.clone(), position)
    }

    fn prepare_for_uri(
        &self,
        request_id: i32,
        uri: lsp_types::Uri,
        position: Position,
    ) -> PreparedSignatureRequest {
        let id = RequestId::from(request_id);
        self.session
            .read()
            .expect("session read")
            .prepare_signature_request(
                id,
                SignatureHelpParams {
                    text_document_position_params: TextDocumentPositionParams {
                        text_document: TextDocumentIdentifier { uri },
                        position,
                    },
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    context: None,
                },
                self.runtime
                    .as_ref()
                    .expect("active request runtime")
                    .registry(),
            )
            .expect("prepared signature request")
    }

    fn accepted(&self) -> Arc<crate::profiles::state::AcceptedProfileEnvironment> {
        let session = self.session.read().expect("session read");
        let profile = session.profile_for_uri(&self.uri);
        profile.accepted_environment().unwrap_or_else(|| {
            panic!(
                "accepted signature environment; profile diagnostics: {:?}",
                profile.diagnostics()
            )
        })
    }

    #[allow(
        clippy::result_large_err,
        reason = "the test helper preserves the production request error for exact assertions"
    )]
    fn execute(
        &self,
        prepared: &PreparedSignatureRequest,
    ) -> Result<
        crate::requests::signature::SignatureRequestResult,
        crate::requests::signature::SignatureRequestError,
    > {
        let work = self
            .session
            .read()
            .expect("session read")
            .signature_work(prepared)?;
        match work {
            crate::requests::signature::SignatureRequestWork::Hit(result) => Ok(result),
            crate::requests::signature::SignatureRequestWork::Miss(key) => {
                ArcweftLspSession::compute_signature(prepared, key)
            }
        }
    }

    fn publish(
        &self,
        prepared: &PreparedSignatureRequest,
        result: Result<
            crate::requests::signature::SignatureRequestResult,
            crate::requests::signature::SignatureRequestError,
        >,
    ) -> Response {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        self.session
            .read()
            .expect("session read")
            .publish_signature_result(prepared, result, &sender);
        match receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("signature response")
        {
            Message::Response(response) => response,
            other => panic!("unexpected signature message: {other:?}"),
        }
    }

    fn publish_after_projection(
        &self,
        prepared: &PreparedSignatureRequest,
        result: Result<
            crate::requests::signature::SignatureRequestResult,
            crate::requests::signature::SignatureRequestError,
        >,
        after_projection: impl FnOnce(),
    ) -> Response {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        self.session
            .read()
            .expect("session read")
            .publish_signature_result_after_projection(prepared, result, &sender, after_projection);
        match receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("signature response")
        {
            Message::Response(response) => response,
            other => panic!("unexpected signature message: {other:?}"),
        }
    }

    fn runtime_response(&self) -> Response {
        match self
            .client
            .receiver
            .recv_timeout(TEST_WAIT)
            .expect("signature runtime response")
        {
            Message::Response(response) => response,
            other => panic!("unexpected signature runtime message: {other:?}"),
        }
    }
}

impl Drop for SignatureCacheFixture {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown();
        }
    }
}

#[test]
fn cancelled_and_expired_requests_never_insert_cache_entries() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-cancel-deadline");
    let accepted = fixture.accepted();
    let position = position_after(SOURCE, "sum(");

    let cancelled = fixture.prepare(1, position);
    fixture
        .runtime
        .as_ref()
        .expect("active runtime")
        .registry()
        .cancel(
            cancelled.request_id(),
            SignatureCancellationReason::ClientCancelled,
        );
    let response = fixture.publish(&cancelled, fixture.execute(&cancelled));
    assert_eq!(
        response.error.expect("cancelled response").code,
        ErrorCode::RequestCanceled as i32
    );
    drop(cancelled);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);

    let expired = fixture.prepare(2, position);
    assert_eq!(
        fixture
            .runtime
            .as_ref()
            .expect("active runtime")
            .registry()
            .fire_deadlines_at(expired.control().deadline()),
        1
    );
    assert_eq!(
        *expired.control().gate(),
        crate::requests::RequestGateState::Cancelled(SignatureCancellationReason::DeadlineExceeded)
    );
    let response = fixture.publish(&expired, fixture.execute(&expired));
    assert_eq!(
        response.error.expect("expired response").code,
        ErrorCode::ServerCancelled as i32
    );
    drop(expired);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn stale_request_and_final_stamp_rejection_never_insert_cache_entries() {
    let fixture = SignatureCacheFixture::new_with_deadline(
        "lsp-signature-cache-stale",
        Duration::from_secs(10),
    );
    let accepted = fixture.accepted();
    let position = position_after(SOURCE, "sum(");
    let prepared = fixture.prepare(3, position);
    let result = fixture
        .execute(&prepared)
        .expect("query completes before the final stamp gate");
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);

    let changed = SOURCE.replace("value, value", "value,  value");
    fixture
        .session
        .write()
        .expect("session write")
        .handle_notification(Notification::new(
            DidChangeTextDocument::METHOD.to_owned(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: fixture.uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: changed,
                }],
            },
        ))
        .expect("document change");
    let current = fixture.accepted();
    assert!(!Arc::ptr_eq(&accepted, &current));
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 0);

    let response = fixture.publish(&prepared, Ok(result));
    let error = response.error.expect("stale response");
    assert_eq!(error.code, ErrorCode::ContentModified as i32);
    assert_eq!(
        error.data,
        Some(serde_json::json!({
            "code": "aw.signature.stale.document_changed"
        }))
    );
    drop(prepared);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn extracted_cache_hit_still_requires_the_complete_final_stamp() {
    let fixture = SignatureCacheFixture::new_with_deadline(
        "lsp-signature-cache-stale-hit",
        Duration::from_secs(10),
    );
    let accepted = fixture.accepted();
    let position = position_after(SOURCE, "sum(");
    let first = fixture.prepare(13, position);
    let response = fixture.publish(&first, fixture.execute(&first));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(first);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 1);

    let hit = fixture.prepare(14, position);
    let result = fixture
        .execute(&hit)
        .expect("exact cache hit is extracted before the lifecycle change");
    assert_eq!(accepted.signature_cache_snapshot_for_test().hits, 1);

    let changed = SOURCE.replace("value, value", "value,  value");
    fixture
        .session
        .write()
        .expect("session write")
        .handle_notification(Notification::new(
            DidChangeTextDocument::METHOD.to_owned(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: fixture.uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: changed,
                }],
            },
        ))
        .expect("document change");
    let current = fixture.accepted();
    assert!(!Arc::ptr_eq(&accepted, &current));

    let response = fixture.publish(&hit, Ok(result));
    let error = response.error.expect("stale hit response");
    assert_eq!(error.code, ErrorCode::ContentModified as i32);
    assert_eq!(
        error.data,
        Some(serde_json::json!({
            "code": "aw.signature.stale.document_changed"
        }))
    );
    drop(hit);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn cancellation_after_query_is_enforced_by_the_final_stamp_gate() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-final-cancel");
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(4, position_after(SOURCE, "sum("));
    let result = fixture
        .execute(&prepared)
        .expect("native query completes before cancellation");

    fixture
        .runtime
        .as_ref()
        .expect("active runtime")
        .registry()
        .cancel(
            prepared.request_id(),
            SignatureCancellationReason::ClientCancelled,
        );
    let response = fixture.publish(&prepared, Ok(result));
    assert_eq!(
        response.error.expect("cancelled response").code,
        ErrorCode::RequestCanceled as i32
    );
    drop(prepared);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn deadline_expiring_during_projection_is_rechecked_before_enqueue() {
    let fixture = SignatureCacheFixture::new_with_deadline(
        "lsp-signature-cache-projection-deadline",
        Duration::from_mins(1),
    );
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(18, position_after(SOURCE, "sum("));
    let result = fixture
        .execute(&prepared)
        .expect("native query completes before the projection deadline");

    let response = fixture.publish_after_projection(&prepared, Ok(result), || {
        prepared.control().expire_deadline_for_test();
    });

    let error = response.error.expect("expired projection response");
    assert_eq!(error.code, ErrorCode::ServerCancelled as i32);
    assert_eq!(
        error.data,
        Some(serde_json::json!({
            "code": "aw.signature.stale.deadline_exceeded"
        }))
    );
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn prepared_worker_panic_publishes_one_internal_error_and_cleans_the_registry() {
    let fixture = SignatureCacheFixture::new_with_deadline(
        "lsp-signature-worker-panic-request",
        Duration::from_mins(1),
    );
    let accepted = fixture.accepted();
    let mut prepared = fixture.prepare(181, position_after(SOURCE, "sum("));
    let control = prepared.control_arc();
    let mut fault = install_prepared_fault(
        &mut prepared,
        SignatureExecutorFaultPoint::BeforeWork,
        false,
    );

    fixture
        .runtime
        .as_ref()
        .expect("active runtime")
        .submit(prepared)
        .expect("prepared request submission");
    fault.wait_until_caught();
    fault.release();

    let response = fixture.runtime_response();
    let error = response.error.expect("worker panic response");
    assert_eq!(error.code, ErrorCode::InternalError as i32);
    assert_eq!(error.message, "signature worker panicked");
    fault.wait_until_completed();
    assert_eq!(*control.gate(), crate::requests::RequestGateState::Finished);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(
        fixture
            .runtime
            .as_ref()
            .expect("active runtime")
            .registry()
            .test_snapshot()
            .active,
        0
    );
    assert!(matches!(
        fixture.client.receiver.try_recv(),
        Err(crossbeam_channel::TryRecvError::Empty)
    ));
}

#[test]
fn worker_side_deadline_maps_panic_to_server_cancelled_before_the_scheduler_runs() {
    let fixture = SignatureCacheFixture::new_with_deadline(
        "lsp-signature-worker-panic-deadline",
        Duration::from_mins(1),
    );
    let accepted = fixture.accepted();
    let mut prepared = fixture.prepare(182, position_after(SOURCE, "sum("));
    let control = prepared.control_arc();
    let mut fault = install_prepared_fault(
        &mut prepared,
        SignatureExecutorFaultPoint::BeforeWork,
        false,
    );

    fixture
        .runtime
        .as_ref()
        .expect("active runtime")
        .submit(prepared)
        .expect("prepared request submission");
    fault.wait_until_caught();
    control.expire_deadline_for_test();
    fault.release();
    fault.wait_until_completed();

    let response = fixture.runtime_response();
    let error = response.error.expect("worker-side deadline response");
    assert_eq!(error.code, ErrorCode::ServerCancelled as i32);
    assert_eq!(
        error.data,
        Some(serde_json::json!({
            "code": "aw.signature.stale.deadline_exceeded"
        }))
    );
    assert_eq!(*control.gate(), crate::requests::RequestGateState::Finished);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    assert!(matches!(
        fixture.client.receiver.try_recv(),
        Err(crossbeam_channel::TryRecvError::Empty)
    ));
    assert_eq!(
        fixture
            .runtime
            .as_ref()
            .expect("active runtime")
            .registry()
            .test_snapshot(),
        crate::requests::registry::RequestRegistryTestSnapshot {
            active: 0,
            deadlines: 0,
            fired: 0,
        }
    );
}

#[test]
fn actual_profile_remap_holds_session_authority_across_panic_publication() {
    let fixture = SignatureCacheFixture::new_with_deadline(
        "lsp-signature-worker-panic-remap",
        Duration::from_mins(1),
    );
    let previous = fixture.accepted();
    let mut prepared = fixture.prepare(183, position_after(SOURCE, "sum("));
    let control = prepared.control_arc();
    let mut fault =
        install_prepared_fault(&mut prepared, SignatureExecutorFaultPoint::BeforeWork, true);
    fixture.project.write("arcw.toml", REMAPPED_MANIFEST);
    fixture
        .session
        .write()
        .expect("session write")
        .profile_resolver
        .select_profile_for_test("alt");

    fixture
        .runtime
        .as_ref()
        .expect("active runtime")
        .submit(prepared)
        .expect("prepared request submission");
    fault.wait_until_caught();

    let (remap_published, remap_published_rx) = crossbeam_channel::bounded(1);
    let (resume_remap, resume_remap_rx) = crossbeam_channel::bounded(1);
    let session = Arc::clone(&fixture.session);
    let registry = Arc::clone(fixture.runtime.as_ref().expect("active runtime").registry());
    let uri = fixture.uri.clone();
    thread::scope(|scope| {
        let remap = scope.spawn(move || {
            session
                .write()
                .expect("session write")
                .refresh_profile_for_uri_with_remap_checkpoint(&uri, &registry, || {
                    remap_published
                        .send(())
                        .expect("remap publication observer");
                    let _ = resume_remap_rx.recv();
                });
        });
        remap_published_rx
            .recv_timeout(TEST_WAIT)
            .expect("new remap authority published before previous cancellation");

        fault.release();
        assert_eq!(
            fault.session_authority_observation(),
            SignatureSessionAuthorityObservation::Blocked
        );
        resume_remap.send(()).expect("release actual profile remap");
        remap.join().expect("profile remap thread");
    });
    fault.wait_until_completed();

    let response = fixture.runtime_response();
    let error = response.error.expect("profile-remapped panic response");
    assert_eq!(error.code, ErrorCode::ContentModified as i32);
    assert_eq!(
        error.data,
        Some(serde_json::json!({
            "code": "aw.signature.stale.cancelled"
        }))
    );
    assert_eq!(*control.gate(), crate::requests::RequestGateState::Finished);
    let current = fixture.accepted();
    assert_eq!(current.profile().profile_id().as_str(), "alt");
    assert!(!Arc::ptr_eq(&previous, &current));
    assert_eq!(previous.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 0);
    assert!(matches!(
        fixture.client.receiver.try_recv(),
        Err(crossbeam_channel::TryRecvError::Empty)
    ));
    assert_eq!(
        fixture
            .runtime
            .as_ref()
            .expect("active runtime")
            .registry()
            .test_snapshot()
            .active,
        0
    );
}

#[test]
fn post_enqueue_panic_cannot_publish_a_second_terminal_response() {
    let fixture = SignatureCacheFixture::new_with_deadline(
        "lsp-signature-worker-post-enqueue-panic",
        Duration::from_mins(1),
    );
    let accepted = fixture.accepted();
    let mut prepared = fixture.prepare(184, position_after(SOURCE, "sum("));
    let control = prepared.control_arc();
    let mut fault = install_prepared_fault(
        &mut prepared,
        SignatureExecutorFaultPoint::AfterResponseEnqueue,
        false,
    );

    fixture
        .runtime
        .as_ref()
        .expect("active runtime")
        .submit(prepared)
        .expect("prepared request submission");
    let response = fixture.runtime_response();
    assert!(
        response.error.is_none(),
        "semantic response must enqueue first"
    );
    fault.wait_until_caught();
    assert_eq!(*control.gate(), crate::requests::RequestGateState::Finished);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    fault.release();
    fault.wait_until_completed();

    assert!(matches!(
        fixture.client.receiver.try_recv(),
        Err(crossbeam_channel::TryRecvError::Empty)
    ));
    assert_eq!(
        fixture
            .runtime
            .as_ref()
            .expect("active runtime")
            .registry()
            .test_snapshot()
            .active,
        0
    );
}

#[test]
fn failed_response_enqueue_does_not_insert_or_finish() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-send-failure");
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(5, position_after(SOURCE, "sum("));
    let result = fixture.execute(&prepared);
    let (sender, receiver) = crossbeam_channel::bounded(1);
    drop(receiver);

    fixture
        .session
        .read()
        .expect("session read")
        .publish_signature_result(&prepared, result, &sender);

    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(
        *prepared.control().gate(),
        crate::requests::RequestGateState::Active
    );
}

#[test]
fn cache_miss_releases_session_and_gate_before_cancellable_sema_work() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-unlocked-query");
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(6, position_after(SOURCE, "sum("));
    let work = {
        let session = fixture.session.read().expect("session read");
        session
            .signature_work(&prepared)
            .expect("validated cache miss")
    };
    let crate::requests::signature::SignatureRequestWork::Miss(key) = work else {
        panic!("fresh request must miss the cache")
    };

    let session_write = fixture
        .session
        .try_write()
        .expect("pre-work returned without retaining the session read lock");
    fixture
        .runtime
        .as_ref()
        .expect("active runtime")
        .registry()
        .cancel(
            prepared.request_id(),
            SignatureCancellationReason::ClientCancelled,
        );
    drop(session_write);

    assert!(matches!(
        ArcweftLspSession::compute_signature(&prepared, key),
        Err(SignatureRequestError::Query(
            arcweft_lang_sema::signature::SignatureQueryError::Cancelled
        ))
    ));
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn document_close_evicts_exact_document_entries_before_unmapping() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-close");
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(7, position_after(SOURCE, "sum("));
    let result = fixture.execute(&prepared);
    let response = fixture.publish(&prepared, result);
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(prepared);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 1);

    fixture
        .session
        .write()
        .expect("session write")
        .handle_notification(Notification::new(
            DidCloseTextDocument::METHOD.to_owned(),
            DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier {
                    uri: fixture.uri.clone(),
                },
            },
        ))
        .expect("document close");

    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn successful_document_edit_clears_old_entries_and_publishes_a_fresh_cache() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-edit");
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(8, position_after(SOURCE, "sum("));
    let response = fixture.publish(&prepared, fixture.execute(&prepared));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(prepared);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 1);

    let changed = SOURCE.replace("value, value", "value,  value");
    fixture
        .session
        .write()
        .expect("session write")
        .handle_notification(Notification::new(
            DidChangeTextDocument::METHOD.to_owned(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: fixture.uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: changed,
                }],
            },
        ))
        .expect("document change");

    let current = fixture.accepted();
    assert!(!Arc::ptr_eq(&accepted, &current));
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn failed_document_rebuild_evicts_changed_document_entries_from_retained_generation() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-failed-edit");
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(9, position_after(SOURCE, "sum("));
    let response = fixture.publish(&prepared, fixture.execute(&prepared));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(prepared);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 1);

    fixture
        .session
        .write()
        .expect("session write")
        .handle_notification(Notification::new(
            DidChangeTextDocument::METHOD.to_owned(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: fixture.uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "this is not Arcweft source".to_owned(),
                }],
            },
        ))
        .expect("failed rebuild is retained as profile diagnostics");

    let current = fixture.accepted();
    assert!(Arc::ptr_eq(&accepted, &current));
    assert_eq!(
        current.signature_cache_snapshot_for_test().entries,
        0,
        "the changed document is invalidated before the failed rebuild"
    );
}

#[test]
fn failed_source_rebuild_preserves_unchanged_document_cache_and_blocks_only_changed_uri() {
    let fixture = SignatureCacheFixture::new_with_source_tree(
        "lsp-signature-cache-failed-feature",
        MULTI_ROOT_SOURCE,
        &[("src/feature.arcw", MULTI_FEATURE_SOURCE)],
        SIGNATURE_REQUEST_DEADLINE,
    );
    let feature_uri = file_uri(&fixture.project.path("src/feature.arcw"));
    {
        let mut session = fixture.session.write().expect("session write");
        open_text(&mut session, feature_uri.clone(), MULTI_FEATURE_SOURCE);
    }
    let accepted = fixture.accepted();

    let root = fixture.prepare(27, position_after(MULTI_ROOT_SOURCE, "sum("));
    let response = fixture.publish(&root, fixture.execute(&root));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(root);
    let feature = fixture.prepare_for_uri(
        28,
        feature_uri.clone(),
        position_after(MULTI_FEATURE_SOURCE, "product("),
    );
    let response = fixture.publish(&feature, fixture.execute(&feature));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(feature);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 2);

    fixture
        .session
        .write()
        .expect("session write")
        .handle_notification_with_requests(
            Notification::new(
                DidChangeTextDocument::METHOD.to_owned(),
                DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: feature_uri.clone(),
                        version: 2,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: "this is not Arcweft source".to_owned(),
                    }],
                },
            ),
            fixture.runtime.as_ref().expect("active runtime").registry(),
        )
        .expect("failed feature rebuild notification");

    let current = fixture.accepted();
    assert!(Arc::ptr_eq(&accepted, &current));
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 1);
    let root_hit = fixture.prepare(29, position_after(MULTI_ROOT_SOURCE, "sum("));
    let root_result = fixture
        .execute(&root_hit)
        .expect("unchanged source keeps its exact old-generation cache hit");
    let response = fixture.publish(&root_hit, Ok(root_result));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(root_hit);
    assert_eq!(current.signature_cache_snapshot_for_test().hits, 1);

    let changed = fixture
        .session
        .read()
        .expect("session read")
        .prepare_signature_request(
            RequestId::from(30),
            SignatureHelpParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: feature_uri },
                    position: Position::new(0, 0),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                context: None,
            },
            fixture.runtime.as_ref().expect("active runtime").registry(),
        );
    assert!(matches!(
        changed,
        Err(SignatureAcquireError::DocumentNotAccepted { .. })
    ));
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 1);
}

#[test]
fn did_open_equal_manifest_publishes_one_metadata_generation_on_the_existing_state() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-equal-manifest-open");
    let previous = fixture.accepted();
    let previous_state = {
        let session = fixture.session.read().expect("session read");
        Arc::clone(session.profile_for_uri(&fixture.uri).state())
    };
    let prepared = fixture.prepare(19, position_after(SOURCE, "sum("));
    let response = fixture.publish(&prepared, fixture.execute(&prepared));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(prepared);
    assert_eq!(previous.signature_cache_snapshot_for_test().entries, 1);

    let manifest_uri = file_uri(&fixture.project.path("arcw.toml"));
    fixture
        .session
        .write()
        .expect("session write")
        .handle_notification_with_requests(
            Notification::new(
                DidOpenTextDocument::METHOD.to_owned(),
                DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: manifest_uri.clone(),
                        language_id: "toml".to_owned(),
                        version: 7,
                        text: MANIFEST.to_owned(),
                    },
                },
            ),
            fixture.runtime.as_ref().expect("active runtime").registry(),
        )
        .expect("equal manifest didOpen");

    let session = fixture.session.read().expect("session read");
    let current = session
        .profile_for_uri(&fixture.uri)
        .accepted_environment()
        .expect("metadata-only accepted generation");
    let manifest_state = session
        .profiles_by_uri
        .get(&crate::uri_key::LspUriKey::from_uri(&manifest_uri))
        .expect("manifest profile mapping")
        .state();
    assert!(Arc::ptr_eq(&previous_state, manifest_state));
    assert_eq!(current.generation().get(), previous.generation().get() + 1);
    assert!(Arc::ptr_eq(
        current.executable().expect("current executable"),
        previous.executable().expect("previous executable")
    ));
    assert!(Arc::ptr_eq(current.project(), previous.project()));
    assert_eq!(
        current
            .overlays()
            .get(current.profile().manifest_key())
            .expect("accepted manifest overlay")
            .version(),
        7
    );
    assert_eq!(previous.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn failed_manifest_rebuild_preserves_the_prior_generation_but_blocks_new_queries() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-failed-manifest");
    let previous = fixture.accepted();
    let prepared = fixture.prepare(20, position_after(SOURCE, "sum("));
    let response = fixture.publish(&prepared, fixture.execute(&prepared));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(prepared);
    assert_eq!(previous.signature_cache_snapshot_for_test().entries, 1);

    let extracted_hit = fixture.prepare(21, position_after(SOURCE, "sum("));
    let hit_result = fixture
        .execute(&extracted_hit)
        .expect("extract existing cache hit before manifest change");
    let computed = fixture.prepare(22, position_after(SOURCE, "sum(value,"));
    let computed_result = fixture
        .execute(&computed)
        .expect("compute a distinct result before manifest change");
    assert_eq!(previous.signature_cache_snapshot_for_test().entries, 1);

    let manifest_uri = file_uri(&fixture.project.path("arcw.toml"));
    fixture
        .session
        .write()
        .expect("session write")
        .handle_notification(Notification::new(
            DidOpenTextDocument::METHOD.to_owned(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: manifest_uri,
                    language_id: "toml".to_owned(),
                    version: 2,
                    text: "[".to_owned(),
                },
            },
        ))
        .expect("failed manifest rebuild notification");

    {
        let session = fixture.session.read().expect("session read");
        let current = session
            .profile_for_uri(&fixture.uri)
            .accepted_environment()
            .expect("prior accepted generation retained");
        assert!(Arc::ptr_eq(&previous, &current));
        assert_eq!(current.generation(), previous.generation());
        assert_eq!(current.profile(), previous.profile());
        assert_eq!(current.signature_cache_snapshot_for_test().entries, 1);

        let result = session.prepare_signature_request(
            RequestId::from(23),
            SignatureHelpParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: fixture.uri.clone(),
                    },
                    position: position_after(SOURCE, "sum("),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                context: None,
            },
            fixture.runtime.as_ref().expect("active runtime").registry(),
        );
        assert!(matches!(
            result,
            Err(SignatureAcquireError::DocumentNotAccepted { uri, .. })
                if uri == *previous.profile().manifest_key()
        ));
    }

    for (prepared, result) in [(extracted_hit, hit_result), (computed, computed_result)] {
        let response = fixture.publish(&prepared, Ok(result));
        let error = response
            .error
            .expect("pending authority rejects old result");
        assert_eq!(error.code, ErrorCode::ContentModified as i32);
        assert_eq!(
            error.data,
            Some(serde_json::json!({
                "code": "aw.signature.stale.document_changed"
            }))
        );
        drop(prepared);
    }
    assert_eq!(previous.signature_cache_snapshot_for_test().entries, 1);
}

#[test]
fn identical_did_change_reuses_project_arcs_and_accepts_the_exact_new_version() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-identical-change");
    let previous = fixture.accepted();
    let prepared = fixture.prepare(22, position_after(SOURCE, "sum("));
    let response = fixture.publish(&prepared, fixture.execute(&prepared));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(prepared);
    assert_eq!(previous.signature_cache_snapshot_for_test().entries, 1);

    fixture
        .session
        .write()
        .expect("session write")
        .handle_notification_with_requests(
            Notification::new(
                DidChangeTextDocument::METHOD.to_owned(),
                DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: fixture.uri.clone(),
                        version: 2,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: SOURCE.to_owned(),
                    }],
                },
            ),
            fixture.runtime.as_ref().expect("active runtime").registry(),
        )
        .expect("identical didChange");

    let current = fixture.accepted();
    assert_eq!(current.generation().get(), previous.generation().get() + 1);
    assert!(Arc::ptr_eq(
        current.executable().expect("current executable"),
        previous.executable().expect("previous executable")
    ));
    assert!(Arc::ptr_eq(current.project(), previous.project()));
    assert_eq!(previous.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(
        current
            .overlays()
            .get(&crate::uri_key::LspUriKey::from_uri(&fixture.uri))
            .expect("accepted source overlay")
            .version(),
        2
    );
    let accepted_version = fixture.prepare(23, position_after(SOURCE, "sum("));
    drop(accepted_version);
}

#[test]
fn live_version_mismatch_is_rejected_before_an_existing_cache_hit() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-version-mismatch");
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(24, position_after(SOURCE, "sum("));
    let response = fixture.publish(&prepared, fixture.execute(&prepared));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(prepared);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 1);

    {
        let mut session = fixture.session.write().expect("session write");
        let encoding = session.position_encoding;
        session
            .documents
            .change(
                DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: fixture.uri.clone(),
                        version: 2,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: SOURCE.to_owned(),
                    }],
                },
                encoding,
            )
            .expect("install live version without accepted metadata publication");
    }

    let result = fixture
        .session
        .read()
        .expect("session read")
        .prepare_signature_request(
            RequestId::from(25),
            SignatureHelpParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: fixture.uri.clone(),
                    },
                    position: position_after(SOURCE, "sum("),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                context: None,
            },
            fixture.runtime.as_ref().expect("active runtime").registry(),
        );
    assert!(matches!(
        result,
        Err(SignatureAcquireError::OverlayVersionNotAccepted {
            expected: 1,
            actual: 2,
            ..
        })
    ));
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 1);
}

#[test]
fn publication_rejects_missing_open_overlay_without_mutating_the_current_generation() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-missing-overlay");
    let previous = fixture.accepted();
    let prepared = fixture.prepare(26, position_after(SOURCE, "sum("));
    let response = fixture.publish(&prepared, fixture.execute(&prepared));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(prepared);
    assert_eq!(previous.signature_cache_snapshot_for_test().entries, 1);

    let mut session = fixture.session.write().expect("session write");
    let profile = session.profile_for_uri(&fixture.uri).clone();
    let candidate = AcceptedProfileCandidate::try_from_unchanged_project(
        &previous,
        AcceptedOverlaySet::default(),
    )
    .expect("candidate construction does not own live overlay coverage");
    let error = session
        .publish_accepted_candidate(
            profile.state(),
            Some(&previous),
            candidate,
            fixture.runtime.as_ref().expect("active runtime").registry(),
        )
        .expect_err("missing open overlay must fail before publication");
    let super::lifecycle::AcceptedPublicationError::OverlayCoverageMismatch {
        missing,
        extra,
        mismatched,
    } = error
    else {
        panic!("unexpected publication error: {error:?}")
    };
    assert_eq!(
        missing.as_ref(),
        &[crate::uri_key::LspUriKey::from_uri(&fixture.uri)]
    );
    assert!(extra.is_empty());
    assert!(mismatched.is_empty());
    let current = profile
        .accepted_environment()
        .expect("current generation retained");
    assert!(Arc::ptr_eq(&previous, &current));
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 1);
}

#[test]
fn duplicate_overlay_uri_is_rejected_without_overwriting_the_first_entry() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-duplicate-overlay");
    let accepted = fixture.accepted();
    let uri = crate::uri_key::LspUriKey::from_uri(&fixture.uri);
    let identity = accepted
        .project()
        .source_identity_by_uri(&uri)
        .expect("accepted source identity")
        .clone();
    let entry = AcceptedOverlayEntry::new(1, identity);

    assert!(matches!(
        AcceptedOverlaySet::try_new([
            (uri.clone(), entry.clone()),
            (uri.clone(), entry),
        ]),
        Err(AcceptedOverlaySetError::DuplicateUri { uri: duplicate })
            if duplicate == uri
    ));
}

#[test]
fn clock_overflow_and_poison_recompute_the_same_native_semantic_result() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-fault-recovery");
    let accepted = fixture.accepted();
    let position = position_after(SOURCE, "sum(");

    let first = fixture.prepare(15, position);
    let first_response = fixture.publish(&first, fixture.execute(&first));
    let expected = first_response.result.expect("initial signature result");
    drop(first);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 1);

    accepted.set_signature_access_clock_for_test(u64::MAX);
    let overflow = fixture.prepare(16, position);
    let overflow_response = fixture.publish(&overflow, fixture.execute(&overflow));
    assert_eq!(overflow_response.result, Some(expected.clone()));
    drop(overflow);
    let after_overflow = accepted.signature_cache_snapshot_for_test();
    assert_eq!(after_overflow.entries, 1);
    assert_eq!(after_overflow.access_clock, 1);
    assert_eq!(after_overflow.clock_resets, 1);

    accepted.poison_signature_cache_for_test();
    let poison = fixture.prepare(17, position);
    let poison_response = fixture.publish(&poison, fixture.execute(&poison));
    assert_eq!(poison_response.result, Some(expected));
    drop(poison);
    let after_poison = accepted.signature_cache_snapshot_for_test();
    assert_eq!(after_poison.entries, 1);
    assert_eq!(after_poison.poison_recoveries, 1);
}

#[test]
fn workspace_removal_clears_cache_and_closes_profile_state() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-workspace-remove");
    let accepted = fixture.accepted();
    let state = {
        let session = fixture.session.read().expect("session read");
        Arc::clone(session.profile_for_uri(&fixture.uri).state())
    };
    let workspace = accepted.profile().workspace_key().clone();
    let prepared = fixture.prepare(10, position_after(SOURCE, "sum("));
    let response = fixture.publish(&prepared, fixture.execute(&prepared));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(prepared);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 1);

    fixture
        .session
        .write()
        .expect("session write")
        .remove_workspace(
            &workspace,
            fixture.runtime.as_ref().expect("active runtime").registry(),
        );

    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(
        state.lifecycle(),
        crate::profiles::state::ProfileEnvironmentLifecycle::Closed
    );
    assert!(state.current().is_none());
}

#[test]
fn session_shutdown_clears_cache_and_closes_profile_state() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-session-shutdown");
    let accepted = fixture.accepted();
    let state = {
        let session = fixture.session.read().expect("session read");
        Arc::clone(session.profile_for_uri(&fixture.uri).state())
    };
    let prepared = fixture.prepare(11, position_after(SOURCE, "sum("));
    let response = fixture.publish(&prepared, fixture.execute(&prepared));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(prepared);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 1);

    fixture
        .session
        .write()
        .expect("session write")
        .begin_shutdown(fixture.runtime.as_ref().expect("active runtime").registry());

    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(
        state.lifecycle(),
        crate::profiles::state::ProfileEnvironmentLifecycle::Closed
    );
    assert!(state.current().is_none());
}

#[test]
fn stale_error_is_typed_before_publication() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-stale-error");
    let prepared = fixture.prepare(12, position_after(SOURCE, "sum("));
    fixture
        .runtime
        .as_ref()
        .expect("active runtime")
        .registry()
        .cancel(
            prepared.request_id(),
            SignatureCancellationReason::DocumentChanged,
        );

    assert!(matches!(
        fixture.execute(&prepared),
        Err(SignatureRequestError::Stale(
            SignatureRequestStale::Cancelled {
                reason: SignatureCancellationReason::DocumentChanged
            }
        ))
    ));
}

mod integration_rows;
mod lifecycle;
