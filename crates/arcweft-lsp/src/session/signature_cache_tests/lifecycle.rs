use std::{
    sync::{Arc, RwLock, TryLockError, Weak, mpsc},
    thread,
    time::{Duration, Instant},
};

use lsp_server::{ErrorCode, Notification, RequestId};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, Position, SignatureHelpParams,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentPositionParams,
    VersionedTextDocumentIdentifier, WorkDoneProgressParams,
    notification::{DidChangeTextDocument, DidCloseTextDocument, Notification as LspNotification},
};

use crate::{
    profiles::state::{
        AcceptedProfileCandidate, AcceptedProfileEnvironment, ProfileEnvironmentLifecycle,
    },
    requests::{
        RequestAdmissionError, RequestControl, RequestGateState, SignatureCancellationReason,
        SignatureRequestBinding, registry::MAX_ACTIVE_SIGNATURE_REQUESTS,
    },
    uri_key::LspUriKey,
};

use super::{ArcweftLspSession, SOURCE, SignatureCacheFixture, position_after};

fn invalid_change(uri: lsp_types::Uri, version: i32) -> Notification {
    Notification::new(
        DidChangeTextDocument::METHOD.to_owned(),
        DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "this is not Arcweft source".to_owned(),
            }],
        },
    )
}

fn wait_for_gate(control: &RequestControl, expected: RequestGateState) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while *control.gate() != expected {
        assert!(
            Instant::now() < deadline,
            "request lifecycle gate did not reach {expected:?}"
        );
        thread::yield_now();
    }
}

fn wait_for_session_writer(session: &Arc<RwLock<ArcweftLspSession>>) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match session.try_read() {
            Err(TryLockError::WouldBlock) => break,
            Err(TryLockError::Poisoned(error)) => {
                panic!("session lock poisoned while awaiting lifecycle writer: {error}")
            }
            Ok(guard) => drop(guard),
        }
        assert!(
            Instant::now() < deadline,
            "lifecycle notification did not acquire the session writer"
        );
        thread::yield_now();
    }
}

fn binding_for(
    prepared: &crate::requests::signature::PreparedSignatureRequest,
) -> SignatureRequestBinding {
    SignatureRequestBinding::new(
        prepared.stamp().uri().clone(),
        prepared.stamp().profile().workspace_key().clone(),
        prepared.stamp().profile_state(),
        prepared.stamp().accepted(),
        prepared.stamp().accepted_document_identity().clone(),
    )
}

#[test]
fn document_change_cancels_and_evicts_before_failed_rebuild_waits_for_publication() {
    let fixture = SignatureCacheFixture::new_with_deadline(
        "lsp-signature-lifecycle-change-order",
        Duration::from_secs(10),
    );
    let accepted = fixture.accepted();
    let seed = fixture.prepare(100, position_after(SOURCE, "sum("));
    let response = fixture.publish(&seed, fixture.execute(&seed));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(seed);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 1);

    let prepared = fixture.prepare(101, position_after(SOURCE, "sum(value,"));
    let result = fixture
        .execute(&prepared)
        .expect("request computes before the document mutation");
    let gate = prepared.control().gate();
    let session = Arc::clone(&fixture.session);
    let registry = Arc::clone(fixture.runtime.as_ref().expect("active runtime").registry());
    let uri = fixture.uri.clone();
    let (completed_tx, completed_rx) = mpsc::sync_channel(1);
    let notification = thread::spawn(move || {
        let outcome = session
            .write()
            .expect("session write")
            .handle_notification_with_requests(invalid_change(uri, 2), &registry);
        completed_tx
            .send(outcome)
            .expect("notification completion receiver");
    });

    wait_for_session_writer(&fixture.session);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 1);
    assert!(matches!(
        completed_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));

    drop(gate);
    completed_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("notification completes after publication lock release")
        .expect("failed rebuild remains a handled notification");
    notification.join().expect("notification thread");
    wait_for_gate(
        prepared.control(),
        RequestGateState::Cancelled(SignatureCancellationReason::DocumentChanged),
    );
    assert!(Arc::ptr_eq(&accepted, &fixture.accepted()));
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);

    let response = fixture.publish(&prepared, Ok(result));
    assert_eq!(
        response.error.expect("changed request is stale").code,
        ErrorCode::ContentModified as i32
    );
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn document_close_cancels_and_evicts_before_profile_unmapping() {
    let fixture = SignatureCacheFixture::new_with_deadline(
        "lsp-signature-lifecycle-close-order",
        Duration::from_secs(10),
    );
    let accepted = fixture.accepted();
    let seed = fixture.prepare(110, position_after(SOURCE, "sum("));
    let response = fixture.publish(&seed, fixture.execute(&seed));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(seed);
    let prepared = fixture.prepare(111, position_after(SOURCE, "sum(value,"));
    let result = fixture
        .execute(&prepared)
        .expect("request computes before document close");
    let state = Arc::clone(prepared.stamp().profile_state());
    let gate = prepared.control().gate();
    let session = Arc::clone(&fixture.session);
    let registry = Arc::clone(fixture.runtime.as_ref().expect("active runtime").registry());
    let uri = fixture.uri.clone();
    let (completed_tx, completed_rx) = mpsc::sync_channel(1);
    let notification = thread::spawn(move || {
        let outcome = session
            .write()
            .expect("session write")
            .handle_notification_with_requests(
                Notification::new(
                    DidCloseTextDocument::METHOD.to_owned(),
                    DidCloseTextDocumentParams {
                        text_document: TextDocumentIdentifier { uri },
                    },
                ),
                &registry,
            );
        completed_tx
            .send(outcome)
            .expect("notification completion receiver");
    });

    wait_for_session_writer(&fixture.session);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 1);
    assert!(matches!(
        completed_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));

    drop(gate);
    completed_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("close completes after publication lock release")
        .expect("document close notification");
    notification.join().expect("notification thread");
    wait_for_gate(
        prepared.control(),
        RequestGateState::Cancelled(SignatureCancellationReason::DocumentClosed),
    );
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(state.lifecycle(), ProfileEnvironmentLifecycle::Closed);
    assert!(state.current().is_none());

    let response = fixture.publish(&prepared, Ok(result));
    assert_eq!(
        response.error.expect("closed request is stale").code,
        ErrorCode::ContentModified as i32
    );
}

#[test]
fn accepted_replacement_cancels_old_request_and_clears_only_old_cache_namespace() {
    let fixture = SignatureCacheFixture::new("lsp-signature-lifecycle-replacement");
    let previous = fixture.accepted();
    let seed = fixture.prepare(120, position_after(SOURCE, "sum("));
    let response = fixture.publish(&seed, fixture.execute(&seed));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(seed);
    let prepared = fixture.prepare(121, position_after(SOURCE, "sum(value,"));
    let result = fixture
        .execute(&prepared)
        .expect("request computes before accepted replacement");
    let candidate = AcceptedProfileCandidate::try_from_unchanged_project(
        &previous,
        previous.overlays().clone(),
    )
    .expect("metadata-only candidate");
    let state = Arc::clone(prepared.stamp().profile_state());
    let current = fixture
        .session
        .write()
        .expect("session write")
        .publish_accepted_candidate(
            &state,
            Some(&previous),
            candidate,
            fixture.runtime.as_ref().expect("active runtime").registry(),
        )
        .expect("accepted replacement");

    assert_eq!(
        *prepared.control().gate(),
        RequestGateState::Cancelled(SignatureCancellationReason::AcceptedReplaced)
    );
    assert_eq!(previous.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 0);
    assert!(Arc::ptr_eq(previous.world(), current.world()));
    assert!(Arc::ptr_eq(previous.project(), current.project()));
    let response = fixture.publish(&prepared, Ok(result));
    assert_eq!(
        response.error.expect("old request is stale").code,
        ErrorCode::ContentModified as i32
    );
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn retained_old_accepted_reader_is_safe_but_its_stamp_cannot_publish() {
    let fixture = SignatureCacheFixture::new("lsp-signature-lifecycle-old-reader");
    let previous = fixture.accepted();
    let prepared = fixture.prepare(130, position_after(SOURCE, "sum("));
    let result = fixture
        .execute(&prepared)
        .expect("request computes against the old accepted reader");
    let candidate = AcceptedProfileCandidate::try_from_unchanged_project(
        &previous,
        previous.overlays().clone(),
    )
    .expect("metadata-only candidate");
    let state = Arc::clone(prepared.stamp().profile_state());
    let current = fixture
        .session
        .write()
        .expect("session write")
        .publish_accepted_candidate(
            &state,
            Some(&previous),
            candidate,
            fixture.runtime.as_ref().expect("active runtime").registry(),
        )
        .expect("accepted replacement");

    let old_source = previous
        .project()
        .source_identity_by_uri(&LspUriKey::from_uri(&fixture.uri))
        .expect("old source remains readable");
    assert_eq!(old_source, prepared.stamp().accepted_document_identity());
    let old_hir = previous
        .project()
        .hir(prepared.stamp().module())
        .expect("old HIR remains readable");
    assert_eq!(
        old_hir.source_identity(),
        Some(prepared.stamp().accepted_document_identity())
    );
    assert!(!Arc::ptr_eq(&previous, &current));

    let response = fixture.publish(&prepared, Ok(result));
    assert_eq!(
        response.error.expect("old generation stamp is stale").code,
        ErrorCode::ContentModified as i32
    );
    assert_eq!(previous.signature_cache_snapshot_for_test().entries, 0);
    assert_eq!(current.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
fn workspace_removal_cancels_bound_work_before_clearing_the_environment() {
    let fixture = SignatureCacheFixture::new("lsp-signature-lifecycle-workspace-remove");
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(140, position_after(SOURCE, "sum("));
    let result = fixture
        .execute(&prepared)
        .expect("request computes before workspace removal");
    let state = Arc::clone(prepared.stamp().profile_state());
    fixture
        .session
        .write()
        .expect("session write")
        .remove_workspace(
            accepted.profile().workspace_key(),
            fixture.runtime.as_ref().expect("active runtime").registry(),
        );

    assert_eq!(
        *prepared.control().gate(),
        RequestGateState::Cancelled(SignatureCancellationReason::WorkspaceRemoved)
    );
    assert_eq!(state.lifecycle(), ProfileEnvironmentLifecycle::Closed);
    assert!(state.current().is_none());
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    let response = fixture.publish(&prepared, Ok(result));
    assert_eq!(
        response.error.expect("removed request is stale").code,
        ErrorCode::ServerCancelled as i32
    );
}

#[test]
fn shutdown_closes_admission_before_waiting_for_request_cancellation() {
    let fixture = SignatureCacheFixture::new_with_deadline(
        "lsp-signature-lifecycle-shutdown-order",
        Duration::from_secs(10),
    );
    let prepared = fixture.prepare(150, position_after(SOURCE, "sum("));
    let gate = prepared.control().gate();
    let session = Arc::clone(&fixture.session);
    let registry = Arc::clone(fixture.runtime.as_ref().expect("active runtime").registry());
    let registry_for_shutdown = Arc::clone(&registry);
    let (completed_tx, completed_rx) = mpsc::sync_channel(1);
    let shutdown = thread::spawn(move || {
        session
            .write()
            .expect("session write")
            .begin_shutdown(&registry_for_shutdown);
        completed_tx.send(()).expect("shutdown completion receiver");
    });

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut probe_id = 151;
    loop {
        match registry.admit(RequestId::from(probe_id), binding_for(&prepared)) {
            Err(RequestAdmissionError::AdmissionClosed) => break,
            Ok(probe) => drop(probe),
            Err(error) => panic!("unexpected shutdown admission result: {error:?}"),
        }
        assert!(
            Instant::now() < deadline,
            "shutdown did not close admission before cancellation"
        );
        probe_id += 1;
        thread::yield_now();
    }
    assert!(matches!(
        completed_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));

    drop(gate);
    completed_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("shutdown completes after cancellation gate release");
    shutdown.join().expect("shutdown thread");
    assert_eq!(
        *prepared.control().gate(),
        RequestGateState::Cancelled(SignatureCancellationReason::SessionShutdown)
    );
}

#[test]
fn active_limit_bounds_old_generation_retention_across_replacements() {
    let fixture = SignatureCacheFixture::new_with_deadline(
        "lsp-signature-lifecycle-retention-cap",
        Duration::from_secs(30),
    );
    let mut prepared = Vec::with_capacity(MAX_ACTIVE_SIGNATURE_REQUESTS);
    let mut retained =
        Vec::<Weak<AcceptedProfileEnvironment>>::with_capacity(MAX_ACTIVE_SIGNATURE_REQUESTS);

    for offset in 0..MAX_ACTIVE_SIGNATURE_REQUESTS {
        let accepted = fixture.accepted();
        let request = fixture.prepare(
            i32::try_from(200 + offset).expect("bounded request ID"),
            position_after(SOURCE, "sum("),
        );
        retained.push(Arc::downgrade(&accepted));
        let candidate = AcceptedProfileCandidate::try_from_unchanged_project(
            &accepted,
            accepted.overlays().clone(),
        )
        .expect("metadata-only candidate");
        let state = Arc::clone(request.stamp().profile_state());
        fixture
            .session
            .write()
            .expect("session write")
            .publish_accepted_candidate(
                &state,
                Some(&accepted),
                candidate,
                fixture.runtime.as_ref().expect("active runtime").registry(),
            )
            .expect("accepted replacement");
        prepared.push(request);
    }

    assert_eq!(
        retained
            .iter()
            .filter(|accepted| accepted.upgrade().is_some())
            .count(),
        MAX_ACTIVE_SIGNATURE_REQUESTS
    );
    let one_over = fixture
        .session
        .read()
        .expect("session read")
        .prepare_signature_request(
            RequestId::from(999),
            SignatureHelpParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: fixture.uri.clone(),
                    },
                    position: Position::new(0, 0),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                context: None,
            },
            fixture.runtime.as_ref().expect("active runtime").registry(),
        );
    assert!(matches!(
        one_over,
        Err(
            crate::requests::signature::SignatureAcquireError::Admission(
                RequestAdmissionError::ActiveLimit {
                    observed: 33,
                    maximum: MAX_ACTIVE_SIGNATURE_REQUESTS,
                }
            )
        )
    ));

    drop(prepared);
    assert!(retained.iter().all(|accepted| accepted.upgrade().is_none()));
    let after_release = fixture.prepare(1000, position_after(SOURCE, "sum("));
    drop(after_release);
}
