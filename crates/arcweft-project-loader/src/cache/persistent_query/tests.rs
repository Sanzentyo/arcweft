use super::{
    PersistentQueryHitPayload, PersistentQueryMissReason, PersistentQueryReadOutcome,
    PersistentQueryReadRequest, PersistentQueryWriteError, PersistentQueryWriteRequest,
};
use crate::cache::{record::CacheRecord, store::FilesystemCacheStore};
use arcweft_project::{
    artifact::{ArtifactKey, ArtifactKeyInput, ArtifactKind},
    fingerprint::{BuildDigest, NamedDigest},
    incremental::{CACHE_SCHEMA_VERSION, CacheRecordStatus, InvalidationReason, QueryKind},
    persistent_object::{
        AWBO_MAGIC, AWBO_SCHEMA_VERSION, AwboEnvelope, CompilerBuildIdentity, CompilerObjectKey,
        CompilerObjectKind, CompilerObjectPayload, CompilerObjectStability, HirBodyFactsObject,
        HirBodyObject, InterfaceSummaryObject, ParsedSyntaxEvidenceObject, ParsedSyntaxObject,
        PublicSymbolKind, PublicSymbolObject, StableDiagnosticSummaryObject, StableRangeObject,
        StableSourceSpanObject, SyntaxStatsObject,
    },
};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "arcweft-persistent-query-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moves forward")
            .as_nanos()
    ))
}

fn digest(label: &str) -> BuildDigest {
    BuildDigest::of(label.as_bytes())
}

fn compiler() -> CompilerBuildIdentity {
    CompilerBuildIdentity {
        package_version: "0.1.0".to_owned(),
        git_commit: "seq-04-2".to_owned(),
        rustc: "rustc-test".to_owned(),
        target: "x86_64-unknown-linux-gnu".to_owned(),
        enabled_features: vec!["b".to_owned(), "a".to_owned()],
    }
}

fn object_key(kind: CompilerObjectKind) -> CompilerObjectKey {
    CompilerObjectKey {
        kind,
        compiler: compiler(),
        source_digest: digest("source"),
        query_options_digest: digest("options"),
        dependency_interface_digests: vec![NamedDigest::new("dep", digest("dep-interface"))],
        dependency_body_digests: vec![NamedDigest::new("dep", digest("dep-body"))],
        environment_digest: digest("environment"),
    }
}

fn artifact_key(query: QueryKind, key: &CompilerObjectKey) -> ArtifactKey {
    ArtifactKey::derive(&ArtifactKeyInput {
        compiler_build_id: key.compiler.git_commit.clone(),
        query,
        artifact_kind: query.artifact_kind(),
        target_triple: key.compiler.target.clone(),
        target_features: key.compiler.enabled_features.clone(),
        profile: "dev".to_owned(),
        package: "pkg".to_owned(),
        logical_item: "crate::main".to_owned(),
        source_digest: key.source_digest,
        dependency_interface_digests: key.dependency_interface_digests.clone(),
        dependency_body_digests: key.dependency_body_digests.clone(),
        adapter_environment_digest: key.environment_digest,
        launch_profile_digest: BuildDigest::ZERO,
        declared_environment_digest: BuildDigest::ZERO,
        format_options_digest: key.query_options_digest,
    })
}

fn parse_request() -> PersistentQueryReadRequest {
    let key = object_key(CompilerObjectKind::ParsedSyntax);
    PersistentQueryReadRequest::new(QueryKind::Parse, artifact_key(QueryKind::Parse, &key), key)
}

fn hir_request() -> PersistentQueryReadRequest {
    let key = object_key(CompilerObjectKind::HirBody);
    PersistentQueryReadRequest::new(
        QueryKind::HirBody,
        artifact_key(QueryKind::HirBody, &key),
        key,
    )
}

fn interface_request() -> PersistentQueryReadRequest {
    let key = object_key(CompilerObjectKind::InterfaceSummary);
    PersistentQueryReadRequest::new(
        QueryKind::Interface,
        artifact_key(QueryKind::Interface, &key),
        key,
    )
}

fn span() -> StableSourceSpanObject {
    StableSourceSpanObject {
        range: StableRangeObject { start: 0, end: 4 },
        start_line: 0,
        start_column: 0,
        end_line: 0,
        end_column: 4,
    }
}

fn parsed_payload(key: &CompilerObjectKey) -> CompilerObjectPayload {
    CompilerObjectPayload::ParsedSyntax(ParsedSyntaxObject {
        schema_version: AWBO_SCHEMA_VERSION,
        compiler_namespace: key.identity_namespace(),
        source_label: "src/main.arcw".to_owned(),
        source_digest: key.source_digest,
        source_span: span(),
        stats: SyntaxStatsObject {
            bytes: 4,
            lines: 1,
            cst_lex_passes: 1,
            punctuation_scans: 0,
            punctuation_scan_bytes: 0,
            line_owned_bytes: 0,
            block_owned_bytes: 0,
            raw_owned_bytes: 0,
            wiki_scan_performed: 0,
            dot_normalization_owned: 0,
            dialogue_rescue_expr_parse_attempts: 0,
            numeric_seq_summaries: 0,
        },
        diagnostics: StableDiagnosticSummaryObject::empty(),
        stage_inputs: key.stage_inputs(),
        evidence: ParsedSyntaxEvidenceObject {
            root_kind: "source_file".to_owned(),
            cst_shape_digest: digest("cst"),
            line_index_digest: digest("line-index"),
            cst_node_count: 1,
            cst_token_count: 1,
            cst_error_node_count: 0,
            typed_attribute_count: 0,
            typed_use_count: 0,
            typed_item_count: 1,
            wiki_link_count: 0,
        },
    })
}

fn hir_payload(key: &CompilerObjectKey) -> CompilerObjectPayload {
    let body_digest = digest("hir-body-shape");
    CompilerObjectPayload::HirBody(HirBodyObject {
        schema_version: AWBO_SCHEMA_VERSION,
        compiler_namespace: key.identity_namespace(),
        module: "main".to_owned(),
        source_digest: key.source_digest,
        source_span: span(),
        diagnostics: StableDiagnosticSummaryObject::empty(),
        stage_inputs: key.stage_inputs(),
        body_digest,
        facts: HirBodyFactsObject {
            attribute_count: 0,
            flow_count: 1,
            function_count: 0,
            agent_count: 0,
            declaration_count: 0,
            top_level_item_count: 1,
            flow_item_count: 1,
            statement_count: 0,
            dialogue_count: 0,
            choice_count: 0,
            loop_count: 0,
            await_count: 0,
            thread_count: 0,
            include_count: 0,
            symbol_digest: digest("hir-symbols"),
            body_shape_digest: body_digest,
        },
    })
}

fn interface_payload(key: &CompilerObjectKey) -> CompilerObjectPayload {
    let stage_inputs = key.stage_inputs();
    let public_symbols = InterfaceSummaryObject::canonical_public_symbols([
        PublicSymbolObject {
            name: "main::opening".to_owned(),
            kind: PublicSymbolKind::Flow,
            signature_digest: digest("opening-signature"),
        },
        PublicSymbolObject {
            name: "main::done".to_owned(),
            kind: PublicSymbolKind::Flow,
            signature_digest: digest("done-signature"),
        },
    ]);
    let imports_digest = stage_inputs.dependency_interface_digest_root();
    CompilerObjectPayload::InterfaceSummary(InterfaceSummaryObject {
        schema_version: AWBO_SCHEMA_VERSION,
        compiler_namespace: key.identity_namespace(),
        module: "main".to_owned(),
        source_digest: key.source_digest,
        source_span: span(),
        diagnostics: StableDiagnosticSummaryObject::empty(),
        stage_inputs,
        exports_digest: InterfaceSummaryObject::exports_digest_for(&public_symbols),
        imports_digest,
        public_symbols,
    })
}

fn envelope_bytes(key: &CompilerObjectKey) -> Vec<u8> {
    let payload = match key.kind {
        CompilerObjectKind::ParsedSyntax => parsed_payload(key),
        CompilerObjectKind::InterfaceSummary => interface_payload(key),
        CompilerObjectKind::HirBody => hir_payload(key),
        other => panic!("test helper does not support {other:?}"),
    };
    AwboEnvelope::new(key, payload)
        .expect("envelope builds")
        .encode()
        .expect("envelope encodes")
}

fn store_good_object(store: &FilesystemCacheStore, request: &PersistentQueryReadRequest) {
    store
        .store_artifact(
            request.query,
            request.artifact_key,
            request.query.artifact_kind(),
            &envelope_bytes(&request.object_key),
        )
        .expect("artifact stores");
}

fn write_file(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().expect("test path has parent")).expect("dir created");
    fs::write(path, bytes).expect("file written");
}

fn miss_reason(outcome: PersistentQueryReadOutcome) -> PersistentQueryMissReason {
    match outcome {
        PersistentQueryReadOutcome::Miss(miss) => miss.reason,
        PersistentQueryReadOutcome::Hit(_) => panic!("expected persistent query soft miss"),
    }
}

#[test]
fn persistent_query_hit_returns_payload_and_snapshot_status() {
    let store = FilesystemCacheStore::new(temp_root("hit"));
    let request = parse_request();
    store_good_object(&store, &request);

    let outcome = store.read_persistent_query(&request);
    assert!(outcome.is_hit());
    assert_eq!(outcome.cache_record_status(), CacheRecordStatus::Hit);
    let PersistentQueryReadOutcome::Hit(hit) = outcome else {
        panic!("expected hit");
    };
    assert_eq!(hit.artifact_kind, ArtifactKind::ParsedSyntax);
    assert!(matches!(
        hit.payload,
        PersistentQueryHitPayload::ParsedSyntax(_)
    ));
}

#[test]
fn persistent_query_write_through_stores_parse_object_for_read_through() {
    let store = FilesystemCacheStore::new(temp_root("write-through-parse"));
    let request = parse_request();
    let receipt = store
        .write_persistent_query(&PersistentQueryWriteRequest::new(
            request.query,
            request.artifact_key,
            request.object_key.clone(),
            "crate::main",
            parsed_payload(&request.object_key),
        ))
        .expect("persistent query writes");

    assert_eq!(receipt.query, QueryKind::Parse);
    assert_eq!(receipt.artifact_kind, ArtifactKind::ParsedSyntax);
    assert!(receipt.record_path.is_file());
    assert!(receipt.object_path.is_file());
    assert!(receipt.object_len > receipt.payload_len);

    let outcome = store.read_persistent_query(&request);
    assert!(outcome.is_hit());
    assert!(matches!(
        outcome,
        PersistentQueryReadOutcome::Hit(hit)
            if matches!(hit.payload, PersistentQueryHitPayload::ParsedSyntax(_))
    ));
}

#[test]
fn persistent_query_write_through_stores_interface_summary_for_read_through() {
    let store = FilesystemCacheStore::new(temp_root("write-through-interface"));
    let request = interface_request();
    let receipt = store
        .write_persistent_query(&PersistentQueryWriteRequest::new(
            request.query,
            request.artifact_key,
            request.object_key.clone(),
            "interface-summary:main",
            interface_payload(&request.object_key),
        ))
        .expect("persistent interface query writes");

    assert_eq!(receipt.query, QueryKind::Interface);
    assert_eq!(receipt.artifact_kind, ArtifactKind::InterfaceSummary);
    assert!(receipt.record_path.is_file());
    assert!(receipt.object_path.is_file());

    let outcome = store.read_persistent_query(&request);
    assert!(outcome.is_hit());
    assert!(matches!(
        outcome,
        PersistentQueryReadOutcome::Hit(hit)
            if matches!(hit.payload, PersistentQueryHitPayload::InterfaceSummary(_))
    ));
}

#[test]
fn persistent_query_write_through_rejects_payload_kind_mismatch() {
    let store = FilesystemCacheStore::new(temp_root("write-kind-mismatch"));
    let request = parse_request();
    let hir_key = object_key(CompilerObjectKind::HirBody);
    let error = store
        .write_persistent_query(&PersistentQueryWriteRequest::new(
            request.query,
            request.artifact_key,
            request.object_key.clone(),
            "crate::main",
            hir_payload(&hir_key),
        ))
        .expect_err("wrong payload kind rejects");

    assert!(matches!(
        error,
        PersistentQueryWriteError::PayloadKindMismatch {
            key: CompilerObjectKind::ParsedSyntax,
            payload: CompilerObjectKind::HirBody,
        }
    ));
}

#[test]
fn persistent_query_missing_record_is_soft_miss() {
    let store = FilesystemCacheStore::new(temp_root("missing-record"));

    assert_eq!(
        miss_reason(store.read_persistent_query(&parse_request())),
        PersistentQueryMissReason::MissingRecord,
    );
}

#[test]
fn persistent_query_corrupt_record_is_soft_miss() {
    let store = FilesystemCacheStore::new(temp_root("corrupt-record"));
    let request = parse_request();
    write_file(
        &store.record_path(request.query, request.artifact_key),
        b"not-json",
    );

    assert!(matches!(
        miss_reason(store.read_persistent_query(&request)),
        PersistentQueryMissReason::CorruptRecord { .. }
    ));
}

#[test]
fn persistent_query_record_schema_mismatch_is_soft_miss() {
    let store = FilesystemCacheStore::new(temp_root("record-schema"));
    let request = parse_request();
    let record = CacheRecord::new(
        request.artifact_key,
        ArtifactKind::ParsedSyntax,
        digest("object"),
        4,
    );
    let bytes = String::from_utf8(record.to_bytes().expect("record encodes"))
        .expect("record is utf-8")
        .replace(
            &format!("\"schema_version\": {CACHE_SCHEMA_VERSION}"),
            "\"schema_version\": 999",
        );
    write_file(
        &store.record_path(request.query, request.artifact_key),
        bytes.as_bytes(),
    );

    assert_eq!(
        miss_reason(store.read_persistent_query(&request)),
        PersistentQueryMissReason::RecordSchemaMismatch {
            actual: 999,
            expected: CACHE_SCHEMA_VERSION,
        },
    );
}

#[test]
fn persistent_query_missing_object_is_soft_miss() {
    let store = FilesystemCacheStore::new(temp_root("missing-object"));
    let request = parse_request();
    let object_digest = digest("missing-object");
    let record = CacheRecord::new(
        request.artifact_key,
        ArtifactKind::ParsedSyntax,
        object_digest,
        128,
    );
    write_file(
        &store.record_path(request.query, request.artifact_key),
        &record.to_bytes().expect("record encodes"),
    );

    assert_eq!(
        miss_reason(store.read_persistent_query(&request)),
        PersistentQueryMissReason::MissingObject { object_digest },
    );
}

#[test]
fn persistent_query_object_digest_mismatch_is_soft_miss() {
    let store = FilesystemCacheStore::new(temp_root("object-digest"));
    let request = parse_request();
    let expected = digest("expected-object");
    let record = CacheRecord::new(
        request.artifact_key,
        ArtifactKind::ParsedSyntax,
        expected,
        5,
    );
    write_file(
        &store.record_path(request.query, request.artifact_key),
        &record.to_bytes().expect("record encodes"),
    );
    write_file(&store.object_path(expected), b"wrong");

    assert_eq!(
        miss_reason(store.read_persistent_query(&request)),
        PersistentQueryMissReason::ObjectDigestMismatch {
            expected,
            actual: BuildDigest::of(b"wrong"),
        },
    );
}

#[test]
fn persistent_query_object_length_mismatch_is_soft_miss() {
    let store = FilesystemCacheStore::new(temp_root("object-length"));
    let request = parse_request();
    let bytes = envelope_bytes(&request.object_key);
    let object_digest = BuildDigest::of(&bytes);
    let actual = u64::try_from(bytes.len()).expect("test object length fits u64");
    let record = CacheRecord::new(
        request.artifact_key,
        ArtifactKind::ParsedSyntax,
        object_digest,
        actual + 1,
    );
    write_file(
        &store.record_path(request.query, request.artifact_key),
        &record.to_bytes().expect("record encodes"),
    );
    write_file(&store.object_path(object_digest), &bytes);

    assert_eq!(
        miss_reason(store.read_persistent_query(&request)),
        PersistentQueryMissReason::ObjectLengthMismatch {
            expected: actual + 1,
            actual,
        },
    );
}

#[test]
fn persistent_query_corrupt_object_is_soft_miss() {
    let store = FilesystemCacheStore::new(temp_root("corrupt-object"));
    let request = parse_request();
    store
        .store_artifact(
            request.query,
            request.artifact_key,
            ArtifactKind::ParsedSyntax,
            b"not-awbo",
        )
        .expect("corrupt test object stores");

    let outcome = store.read_persistent_query(&request);
    assert!(matches!(
        miss_reason(outcome.clone()),
        PersistentQueryMissReason::CorruptObject { .. }
    ));
    assert_eq!(
        outcome.cache_record_status(),
        CacheRecordStatus::Miss {
            reason: InvalidationReason::CorruptObject,
        }
    );
}

#[test]
fn persistent_query_object_schema_mismatch_is_soft_miss() {
    let store = FilesystemCacheStore::new(temp_root("object-schema"));
    let request = parse_request();
    let mut bytes = envelope_bytes(&request.object_key);
    bytes[AWBO_MAGIC.len()..AWBO_MAGIC.len() + 4].copy_from_slice(&999_u32.to_le_bytes());
    store
        .store_artifact(
            request.query,
            request.artifact_key,
            ArtifactKind::ParsedSyntax,
            &bytes,
        )
        .expect("schema test object stores");

    assert_eq!(
        miss_reason(store.read_persistent_query(&request)),
        PersistentQueryMissReason::ObjectSchemaMismatch {
            actual: 999,
            expected: AWBO_SCHEMA_VERSION,
        },
    );
}

#[test]
fn persistent_query_payload_schema_mismatch_is_soft_miss() {
    let store = FilesystemCacheStore::new(temp_root("payload-schema"));
    let request = parse_request();
    let CompilerObjectPayload::ParsedSyntax(mut payload) = parsed_payload(&request.object_key)
    else {
        panic!("parse helper returns parse payload");
    };
    payload.schema_version = 999;
    let payload = CompilerObjectPayload::ParsedSyntax(payload);
    let payload_bytes = payload.encode_payload_bytes().expect("payload encodes");
    let envelope = AwboEnvelope {
        magic: AWBO_MAGIC,
        schema_version: AWBO_SCHEMA_VERSION,
        kind: CompilerObjectKind::ParsedSyntax,
        stability: CompilerObjectStability::ExactCompilerIdentity,
        key_digest: request.object_key.digest(),
        payload_digest: BuildDigest::of(&payload_bytes),
        payload_len: u64::try_from(payload_bytes.len()).expect("payload length fits"),
        payload,
    };
    let bytes = envelope
        .encode()
        .expect("bad payload schema envelope encodes");
    store
        .store_artifact(
            request.query,
            request.artifact_key,
            ArtifactKind::ParsedSyntax,
            &bytes,
        )
        .expect("payload schema object stores");

    assert_eq!(
        miss_reason(store.read_persistent_query(&request)),
        PersistentQueryMissReason::PayloadSchemaMismatch {
            actual: 999,
            expected: AWBO_SCHEMA_VERSION,
        },
    );
}

#[test]
fn persistent_query_payload_digest_mismatch_is_soft_miss() {
    let store = FilesystemCacheStore::new(temp_root("payload-digest"));
    let request = parse_request();
    let mut bytes = envelope_bytes(&request.object_key);
    let label_offset = bytes
        .windows(b"src/main.arcw".len())
        .position(|window| window == b"src/main.arcw")
        .expect("payload label is encoded");
    bytes[label_offset] = b'x';
    store
        .store_artifact(
            request.query,
            request.artifact_key,
            ArtifactKind::ParsedSyntax,
            &bytes,
        )
        .expect("payload digest object stores");

    assert_eq!(
        miss_reason(store.read_persistent_query(&request)),
        PersistentQueryMissReason::PayloadDigestMismatch,
    );
}

#[test]
fn persistent_query_compiler_identity_mismatch_is_soft_miss() {
    let store = FilesystemCacheStore::new(temp_root("compiler"));
    let request = parse_request();
    store_good_object(&store, &request);
    let mut changed = request.clone();
    changed.object_key.compiler.git_commit = "changed".to_owned();

    assert!(matches!(
        miss_reason(store.read_persistent_query(&changed)),
        PersistentQueryMissReason::CompilerIdentityMismatch { .. }
    ));
}

#[test]
fn persistent_query_source_digest_mismatch_is_soft_miss() {
    let store = FilesystemCacheStore::new(temp_root("source"));
    let request = parse_request();
    store_good_object(&store, &request);
    let mut changed = request.clone();
    changed.object_key.source_digest = digest("changed-source");

    assert_eq!(
        miss_reason(store.read_persistent_query(&changed)),
        PersistentQueryMissReason::SourceDigestMismatch {
            expected: digest("changed-source"),
            actual: digest("source"),
        },
    );
}

#[test]
fn persistent_query_query_kind_mismatch_is_soft_miss() {
    let store = FilesystemCacheStore::new(temp_root("query-kind"));
    let mut request = parse_request();
    request.query = QueryKind::HirBody;

    assert_eq!(
        miss_reason(store.read_persistent_query(&request)),
        PersistentQueryMissReason::QueryKindMismatch {
            expected: QueryKind::Parse,
            actual: QueryKind::HirBody,
        },
    );
}

#[test]
fn persistent_query_dependency_mismatches_are_soft_misses() {
    let parse_store = FilesystemCacheStore::new(temp_root("dep-interface"));
    let parse_request = parse_request();
    store_good_object(&parse_store, &parse_request);
    let mut changed_parse = parse_request.clone();
    changed_parse.object_key.dependency_interface_digests =
        vec![NamedDigest::new("dep", digest("changed"))];
    assert!(matches!(
        miss_reason(parse_store.read_persistent_query(&changed_parse)),
        PersistentQueryMissReason::DependencyInterfaceDigestMismatch { .. }
    ));

    let summary_store = FilesystemCacheStore::new(temp_root("summary-dep-interface"));
    let summary_request = interface_request();
    store_good_object(&summary_store, &summary_request);
    let mut changed_summary = summary_request.clone();
    changed_summary.object_key.dependency_interface_digests =
        vec![NamedDigest::new("dep", digest("changed"))];
    assert!(matches!(
        miss_reason(summary_store.read_persistent_query(&changed_summary)),
        PersistentQueryMissReason::DependencyInterfaceDigestMismatch { .. }
    ));

    let body_store = FilesystemCacheStore::new(temp_root("dep-body"));
    let body_request = hir_request();
    store_good_object(&body_store, &body_request);
    let mut changed_body = body_request.clone();
    changed_body.object_key.dependency_body_digests =
        vec![NamedDigest::new("dep", digest("changed"))];
    assert!(matches!(
        miss_reason(body_store.read_persistent_query(&changed_body)),
        PersistentQueryMissReason::DependencyBodyDigestMismatch { .. }
    ));
}

#[test]
fn persistent_query_unsupported_object_kind_and_status_are_typed() {
    let store = FilesystemCacheStore::new(temp_root("unsupported"));
    let mut request = parse_request();
    request.object_key.kind = CompilerObjectKind::RuntimePlanUnit;
    let outcome = store.read_persistent_query(&request);

    assert_eq!(
        miss_reason(outcome),
        PersistentQueryMissReason::UnsupportedObjectKind {
            object_kind: CompilerObjectKind::RuntimePlanUnit,
        },
    );
    assert_eq!(
        FilesystemCacheStore::new(temp_root("status"))
            .read_persistent_query(&parse_request())
            .cache_record_status(),
        CacheRecordStatus::Miss {
            reason: InvalidationReason::MissingRecord,
        },
    );
}

#[test]
fn runtime_bytecode_and_link_plan_objects_remain_unsupported() {
    let store = FilesystemCacheStore::new(temp_root("unsupported-later-families"));
    for object_kind in [
        CompilerObjectKind::RuntimePlanUnit,
        CompilerObjectKind::BytecodeUnit,
        CompilerObjectKind::LinkPlan,
    ] {
        let mut request = parse_request();
        request.object_key.kind = object_kind;
        assert_eq!(
            miss_reason(store.read_persistent_query(&request)),
            PersistentQueryMissReason::UnsupportedObjectKind { object_kind },
        );
    }
}
