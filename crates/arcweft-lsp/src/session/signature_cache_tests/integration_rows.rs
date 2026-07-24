use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

use arcweft_lang_sema::{
    callable::{
        CallableCandidateId, CallableGroupKind, CallableName, CallableParameterPassing,
        CallableParameterPresence, CallableParameterType, CallableQueryLimitError,
        CapacityMethodId,
    },
    signature::{SignatureNotApplicable, SignatureQueryError, SignatureQueryOutcome},
    types::TypeKind,
};
use arcweft_launch::ProfileId;
use lsp_types::{
    ParameterLabel, SignatureHelp, SignatureHelpParams, TextDocumentIdentifier,
    TextDocumentPositionParams, WorkDoneProgressParams,
};

use super::*;
use crate::{
    profiles::{
        accepted_build_work_snapshot_for_test, accepted_project::AcceptedProjectSnapshot,
        state::AcceptedProfileKey,
    },
    requests::{
        RequestControl,
        signature::{SignatureRequestError, SignatureRequestStale},
    },
    session::tests::position_of,
    uri_key::LspUriKey,
};

fn params(uri: lsp_types::Uri, position: Position) -> SignatureHelpParams {
    SignatureHelpParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        context: None,
    }
}

#[test]
fn cache_miss_uses_the_exact_accepted_query_tuple_without_compiler_work() {
    let fixture = SignatureCacheFixture::new("lsp-signature-exact-query-tuple");
    let accepted = fixture.accepted();
    let before = accepted_build_work_snapshot_for_test();
    let prepared = fixture.prepare(80, position_after(SOURCE, "sum(value,"));
    let work = fixture
        .session
        .read()
        .expect("session read")
        .signature_work(&prepared)
        .expect("exact accepted cache lookup");
    let crate::requests::signature::SignatureRequestWork::Miss(key) = work else {
        panic!("fresh accepted generation must miss")
    };
    let stamp = prepared.stamp();
    let lease = prepared.lease();
    let exact_hir = stamp
        .project()
        .hir(stamp.module())
        .expect("stamped accepted HIR");

    assert!(Arc::ptr_eq(stamp.accepted(), &accepted));
    assert!(Arc::ptr_eq(stamp.project(), accepted.project()));
    assert!(Arc::ptr_eq(
        stamp.hir_project(),
        accepted.project().hir_project()
    ));
    assert!(Arc::ptr_eq(stamp.world(), accepted.world()));
    assert!(std::ptr::eq(lease.world(), stamp.world().as_ref()));
    assert!(std::ptr::eq(
        lease.document(),
        stamp.accepted_document().as_ref()
    ));
    assert!(std::ptr::eq(lease.hir().expect("lease HIR"), exact_hir));
    assert_eq!(
        lease.document().identity(),
        stamp.accepted_document_identity()
    );
    assert_eq!(
        exact_hir.source_identity(),
        Some(stamp.accepted_document_identity())
    );
    assert_eq!(
        key.byte_offset(),
        prepared
            .snapshot()
            .line_index()
            .try_byte_offset_from_position(prepared.position())
            .expect("exact protocol position")
    );

    let result = ArcweftLspSession::compute_signature(&prepared, key)
        .expect("native semantic signature query");
    assert!(matches!(
        result.outcome().as_ref(),
        SignatureQueryOutcome::Help(_)
    ));
    assert_eq!(accepted_build_work_snapshot_for_test(), before);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);

    let cancelled = fixture.prepare(81, position_after(SOURCE, "sum(value,"));
    let work = fixture
        .session
        .read()
        .expect("session read")
        .signature_work(&cancelled)
        .expect("second cache miss");
    let crate::requests::signature::SignatureRequestWork::Miss(key) = work else {
        panic!("unpublished first result leaves a miss")
    };
    fixture
        .runtime
        .as_ref()
        .expect("request runtime")
        .registry()
        .cancel(
            cancelled.request_id(),
            SignatureCancellationReason::ClientCancelled,
        );
    assert!(matches!(
        ArcweftLspSession::compute_signature(&cancelled, key),
        Err(SignatureRequestError::Query(SignatureQueryError::Cancelled))
    ));
    assert_eq!(accepted_build_work_snapshot_for_test(), before);
}

#[test]
fn parser_owned_argument_range_is_the_only_successful_carrier() {
    let fixture = SignatureCacheFixture::new("lsp-signature-parser-owned-carrier");
    let before = accepted_build_work_snapshot_for_test();

    let inside = fixture.prepare(82, position_after(SOURCE, "sum(value,"));
    let result = fixture.execute(&inside).expect("inside argument list");
    assert!(matches!(
        result.outcome().as_ref(),
        SignatureQueryOutcome::Help(_)
    ));

    let outside = fixture.prepare(83, position_of(SOURCE, "sum(value"));
    let result = fixture.execute(&outside).expect("outside argument list");
    assert_eq!(
        result.outcome().as_ref(),
        &SignatureQueryOutcome::NotApplicable(SignatureNotApplicable::CursorOutsideArgumentList,)
    );
    assert_eq!(accepted_build_work_snapshot_for_test(), before);
}

#[test]
fn cache_hit_preserves_the_exact_native_result_and_wire_projection() {
    let fixture = SignatureCacheFixture::new("lsp-signature-exact-cache-semantics");
    let accepted = fixture.accepted();
    let before = accepted_build_work_snapshot_for_test();
    let first = fixture.prepare(86, position_after(SOURCE, "sum(value,"));
    let first_result = fixture.execute(&first).expect("native cache miss result");
    let expected = Arc::clone(first_result.outcome());
    let first_response = fixture.publish(&first, Ok(first_result));
    assert!(first_response.error.is_none(), "{:?}", first_response.error);
    drop(first);

    let second = fixture.prepare(87, position_after(SOURCE, "sum(value,"));
    let hit = fixture.execute(&second).expect("exact native cache hit");
    assert!(Arc::ptr_eq(hit.outcome(), &expected));
    let second_response = fixture.publish(&second, Ok(hit));
    assert!(
        second_response.error.is_none(),
        "{:?}",
        second_response.error
    );
    assert_eq!(second_response.result, first_response.result);
    assert_eq!(accepted_build_work_snapshot_for_test(), before);
    let cache = accepted.signature_cache_snapshot_for_test();
    assert_eq!(cache.entries, 1);
    assert_eq!(cache.hits, 1);
}

#[test]
fn exhausted_native_query_work_publishes_no_help_or_cache_state() {
    let fixture = SignatureCacheFixture::new("lsp-signature-exhausted-native-query-work");
    let accepted = fixture.accepted();
    let prepared = fixture.prepare(88, position_after(SOURCE, "sum(value,"));
    let before = accepted.signature_cache_snapshot_for_test();

    let response = fixture.publish(
        &prepared,
        Err(SignatureRequestError::Query(
            SignatureQueryError::CallableLimitExceeded(CallableQueryLimitError::Work {
                requested: 1,
                consumed: 3,
                limit: 3,
            }),
        )),
    );

    assert!(response.result.is_none());
    let error = response.error.expect("bounded-work failure response");
    assert_eq!(error.code, lsp_server::ErrorCode::ServerCancelled as i32);
    assert_eq!(
        error.data,
        Some(serde_json::json!({
            "code": "aw.signature.query.limit_exceeded"
        }))
    );
    assert_eq!(accepted.signature_cache_snapshot_for_test(), before);
}

fn assert_capacity_native_signature(
    outcome: &SignatureQueryOutcome,
    expected_candidate: &CallableCandidateId,
    slot: usize,
) -> SignatureHelp {
    let SignatureQueryOutcome::Help(native_help) = outcome else {
        panic!("argument slot {slot} must produce native semantic help")
    };
    assert_eq!(native_help.active_signature().get(), 0);
    let active = native_help
        .active_parameter()
        .expect("each authored argument has an active parameter");
    assert_eq!(active.group().get(), 0);
    assert_eq!(active.parameter().get(), 0);

    let [native_signature] = native_help.signatures() else {
        panic!("Capacity must project exactly one semantic signature")
    };
    assert_eq!(native_signature.candidate(), expected_candidate);
    assert_eq!(native_signature.authored_callee(), "String.with_capacity");
    assert_eq!(native_signature.result(), &TypeKind::String);
    let CallableCandidateId::CapacityMethod(capacity) = native_signature.candidate() else {
        panic!("semantic signature must retain CapacityMethod identity")
    };
    assert_eq!(capacity.receiver(), &TypeKind::String);
    assert_eq!(capacity.arity(), 3);

    let [group] = native_signature.groups() else {
        panic!("Capacity must retain one semantic parameter group")
    };
    assert_eq!(group.index().get(), 0);
    assert_eq!(group.kind(), CallableGroupKind::Initial);
    let [parameter] = group.parameters() else {
        panic!("Capacity must retain one unchecked rest parameter")
    };
    assert_eq!(parameter.coordinate(), active);
    assert_eq!(parameter.label(), "...args: _?");
    assert_eq!(parameter.name().map(CallableName::as_str), Some("args"));
    assert_eq!(parameter.ty(), &CallableParameterType::Unchecked);
    assert_eq!(
        parameter.passing(),
        CallableParameterPassing::RestPositional
    );
    assert_eq!(parameter.presence(), CallableParameterPresence::Optional);

    crate::features::signature::signature_help(outcome)
        .expect("native semantic help projects without lookup")
        .expect("Capacity signature is applicable")
}

fn assert_capacity_wire_projection(
    response: Response,
    native_projection: &SignatureHelp,
) -> SignatureHelp {
    assert!(response.error.is_none(), "{:?}", response.error);
    let wire = serde_json::from_value::<SignatureHelp>(
        response.result.expect("Capacity signature response"),
    )
    .expect("valid LSP SignatureHelp");
    assert_eq!(&wire, native_projection);
    assert_eq!(wire.active_signature, Some(0));
    assert_eq!(wire.active_parameter, Some(0));
    let [wire_signature] = wire.signatures.as_slice() else {
        panic!("LSP must expose exactly one Capacity signature")
    };
    assert_eq!(
        wire_signature.label,
        "String.with_capacity(...args: _?) -> String"
    );
    let Some([wire_parameter]) = wire_signature.parameters.as_deref() else {
        panic!("LSP must expose exactly one Capacity parameter")
    };
    assert_eq!(wire_parameter.label, ParameterLabel::LabelOffsets([21, 32]));
    wire
}

#[test]
fn associated_capacity_native_lsp_projection_parity() {
    const CAPACITY_SOURCE: &str = "fn allocate() -> String {\n\
    String.with_capacity(1usize, 2usize, 3usize)\n\
}\n\
entry server @entry.server.main { goto @flow.main }\n\
flow @flow.main main {}\n";

    let fixture = SignatureCacheFixture::new_with_source_tree(
        "lsp-associated-capacity-projection-parity",
        CAPACITY_SOURCE,
        &[],
        SIGNATURE_REQUEST_DEADLINE,
    );
    let accepted = fixture.accepted();
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
    let expected_candidate = CallableCandidateId::CapacityMethod(
        CapacityMethodId::try_new(
            TypeKind::String,
            CallableName::try_new("with_capacity").expect("capacity method name"),
            3,
        )
        .expect("capacity candidate identity"),
    );
    let slot_positions = [
        position_after(CAPACITY_SOURCE, "String.with_capacity(1us"),
        position_after(CAPACITY_SOURCE, "1usize, 2us"),
        position_after(CAPACITY_SOURCE, "2usize, 3us"),
    ];
    let mut first_native = None;
    let mut first_wire = None;

    for (slot, (request_id, position)) in [900, 901, 902]
        .into_iter()
        .zip(slot_positions.iter().copied())
        .enumerate()
    {
        let prepared = fixture.prepare(request_id, position);
        let result = fixture
            .execute(&prepared)
            .expect("associated Capacity native signature result");
        let native_projection =
            assert_capacity_native_signature(result.outcome().as_ref(), &expected_candidate, slot);
        if slot == 0 {
            first_native = Some(Arc::clone(result.outcome()));
        }
        let wire = assert_capacity_wire_projection(
            fixture.publish(&prepared, Ok(result)),
            &native_projection,
        );
        if slot == 0 {
            first_wire = Some(wire);
        }
    }

    let repeated = fixture.prepare(903, slot_positions[0]);
    let hit = fixture
        .execute(&repeated)
        .expect("repeated Capacity signature cache hit");
    assert!(Arc::ptr_eq(
        hit.outcome(),
        first_native.as_ref().expect("first native Capacity result")
    ));
    let _ = assert_capacity_wire_projection(
        fixture.publish(&repeated, Ok(hit)),
        first_wire.as_ref().expect("first Capacity wire response"),
    );

    let cache = accepted.signature_cache_snapshot_for_test();
    assert_eq!(cache.entries, 3);
    assert_eq!(cache.misses, 3);
    assert_eq!(cache.insertions, 3);
    assert_eq!(cache.hits, 1);
}

#[test]
fn acquisition_failure_returns_directly_without_build_or_fallback() {
    let fixture = SignatureCacheFixture::new("lsp-signature-no-acquisition-fallback");
    let accepted = fixture.accepted();
    let accepted_cache = accepted.signature_cache_snapshot_for_test();
    let before = accepted_build_work_snapshot_for_test();
    let missing_uri = "file:///not-open-and-not-mapped.arcw"
        .parse()
        .expect("missing URI");

    let result = fixture
        .session
        .read()
        .expect("session read")
        .prepare_signature_request(
            lsp_server::RequestId::from(84),
            params(missing_uri, Position::new(0, 0)),
            fixture
                .runtime
                .as_ref()
                .expect("request runtime")
                .registry(),
        );

    assert!(matches!(
        result,
        Err(SignatureAcquireError::DocumentNotOpen { .. })
    ));
    assert_eq!(accepted_build_work_snapshot_for_test(), before);
    let retained = fixture.accepted();
    assert!(Arc::ptr_eq(&retained, &accepted));
    assert_eq!(retained.signature_cache_snapshot_for_test(), accepted_cache);
}

#[test]
fn worker_transfer_types_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<Arc<RwLock<ArcweftLspSession>>>();
    assert_send_sync::<AcceptedProjectSnapshot>();
    assert_send_sync::<RequestControl>();
    assert_send_sync::<PreparedSignatureRequest>();
}

#[test]
fn typed_uri_and_profile_keys_preserve_exact_lookup_and_remap_rejection() {
    let fixture = SignatureCacheFixture::new("lsp-signature-typed-uri-profile");
    let prepared = fixture.prepare(85, position_after(SOURCE, "sum(value,"));
    let before = accepted_build_work_snapshot_for_test();
    let exact_uri = LspUriKey::from_uri(&fixture.uri);
    let distinct_uri = "file:///definitely-distinct.arcw"
        .parse()
        .expect("distinct URI");
    let distinct_uri = LspUriKey::from_uri(&distinct_uri);
    let stamped = prepared.stamp().profile().clone();
    let alternate = AcceptedProfileKey::new(
        &stamped.workspace_key().to_uri(),
        &stamped.manifest_key().to_uri(),
        ProfileId::new("typed-other").expect("alternate profile ID"),
    );
    let mut typed = BTreeMap::new();
    typed.insert(exact_uri.clone(), stamped.clone());

    assert_eq!(typed.get(&exact_uri), Some(&stamped));
    assert_eq!(typed.get(&distinct_uri), None);
    assert_ne!(stamped, alternate);

    fixture
        .session
        .write()
        .expect("session write")
        .profile_keys_by_uri
        .insert(exact_uri, alternate.clone());
    assert!(matches!(
        fixture
            .session
            .read()
            .expect("session read")
            .signature_work(&prepared),
        Err(SignatureRequestError::Stale(
            SignatureRequestStale::ProfileRemapped {
                expected,
                actual: Some(actual),
            }
        )) if expected == stamped && actual == alternate
    ));
    assert_eq!(accepted_build_work_snapshot_for_test(), before);
}
