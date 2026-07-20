use std::{
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

use lsp_server::{Connection, ErrorCode, Message, Notification, RequestId, Response};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, Position, SignatureHelpParams,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentPositionParams,
    VersionedTextDocumentIdentifier, WorkDoneProgressParams,
    notification::{DidChangeTextDocument, DidCloseTextDocument, Notification as LspNotification},
};

use super::{
    ArcweftLspSession,
    tests::{TestProject, file_uri, open_text, position_after},
};
use crate::{
    config::LspConfig,
    requests::{
        SignatureCancellationReason, SignatureRequestRuntime,
        signature::{PreparedSignatureRequest, SignatureRequestError, SignatureRequestStale},
    },
};

const MANIFEST: &str = r#"
schema = 1

[package]
id = "org.arcweft.tests.lsp.signature-cache"
version = "0.1.0"

[profiles.dev]
kind = "server"
entry = "@entry.server.main"
source = "src/main.arcw"
adapter = "inference-tensor"
"#;

const SOURCE: &str = "fn evaluate_tensor(value: TensorF32) -> TensorF32 {\n    infer.add_f32(value, value)\n}\n\
entry server @entry.server.main { goto @flow.main }\n\
flow @flow.main main {}\n";

struct SignatureCacheFixture {
    _project: TestProject,
    uri: lsp_types::Uri,
    session: Arc<RwLock<ArcweftLspSession>>,
    runtime: Option<SignatureRequestRuntime>,
}

impl SignatureCacheFixture {
    fn new(name: &str) -> Self {
        let project = TestProject::new(name);
        project.write("arcw.toml", MANIFEST);
        project.write("src/main.arcw", SOURCE);
        let uri = file_uri(&project.path("src/main.arcw"));
        let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
        open_text(&mut session, uri.clone(), SOURCE);
        let session = Arc::new(RwLock::new(session));
        let (server, _client) = Connection::memory();
        let runtime = SignatureRequestRuntime::new(&server, Arc::clone(&session))
            .expect("signature request runtime");
        Self {
            _project: project,
            uri,
            session,
            runtime: Some(runtime),
        }
    }

    fn prepare(&self, request_id: i32, position: Position) -> PreparedSignatureRequest {
        let id = RequestId::from(request_id);
        self.session
            .read()
            .expect("session read")
            .prepare_signature_request(
                id,
                SignatureHelpParams {
                    text_document_position_params: TextDocumentPositionParams {
                        text_document: TextDocumentIdentifier {
                            uri: self.uri.clone(),
                        },
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
        self.session
            .read()
            .expect("session read")
            .profile_for_uri(&self.uri)
            .accepted_environment()
            .expect("accepted signature environment")
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
    let position = position_after(SOURCE, "infer.add_f32(");

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
    let wait_until = Instant::now() + Duration::from_secs(1);
    while !matches!(
        *expired.control().gate(),
        crate::requests::RequestGateState::Cancelled(SignatureCancellationReason::DeadlineExceeded)
    ) {
        assert!(
            Instant::now() < wait_until,
            "signature request deadline did not fire"
        );
        thread::yield_now();
    }
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
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-stale");
    let accepted = fixture.accepted();
    let position = position_after(SOURCE, "infer.add_f32(");
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
    assert_eq!(
        response.error.expect("stale response").code,
        ErrorCode::ContentModified as i32
    );
    drop(prepared);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn extracted_cache_hit_still_requires_the_complete_final_stamp() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-stale-hit");
    let accepted = fixture.accepted();
    let position = position_after(SOURCE, "infer.add_f32(");
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
    assert_eq!(
        response.error.expect("stale hit response").code,
        ErrorCode::ContentModified as i32
    );
    drop(hit);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn cancellation_after_query_is_enforced_by_the_final_stamp_gate() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-final-cancel");
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(4, position_after(SOURCE, "infer.add_f32("));
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
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-projection-deadline");
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(18, position_after(SOURCE, "infer.add_f32("));
    let result = fixture
        .execute(&prepared)
        .expect("native query completes before the projection deadline");
    let deadline = prepared.control().deadline();

    let response = fixture.publish_after_projection(&prepared, Ok(result), || {
        assert!(
            Instant::now() < deadline,
            "request must reach the post-projection seam before its deadline"
        );
        while Instant::now() < deadline {
            thread::yield_now();
        }
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
fn failed_response_enqueue_does_not_insert_or_finish() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-send-failure");
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(5, position_after(SOURCE, "infer.add_f32("));
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
    let prepared = fixture.prepare(6, position_after(SOURCE, "infer.add_f32("));
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
    let prepared = fixture.prepare(7, position_after(SOURCE, "infer.add_f32("));
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
    let prepared = fixture.prepare(8, position_after(SOURCE, "infer.add_f32("));
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
    let prepared = fixture.prepare(9, position_after(SOURCE, "infer.add_f32("));
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
fn clock_overflow_and_poison_recompute_the_same_native_semantic_result() {
    let fixture = SignatureCacheFixture::new("lsp-signature-cache-fault-recovery");
    let accepted = fixture.accepted();
    let position = position_after(SOURCE, "infer.add_f32(");

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
    let prepared = fixture.prepare(10, position_after(SOURCE, "infer.add_f32("));
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
    let prepared = fixture.prepare(11, position_after(SOURCE, "infer.add_f32("));
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
    let prepared = fixture.prepare(12, position_after(SOURCE, "infer.add_f32("));
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
