use super::{
    CompileEmit, ProfileOptions, ProjectBuildPersistentReuseEvidence, ProjectCommandReport,
    ProjectCommandSession, RUNTIME_PLAN_ARTIFACT_SCHEMA_VERSION,
    compile_accepted_project_runtime_plan, compile_project_command,
    compile_project_command_in_session, decode_runtime_plan_artifact, encode_runtime_plan_artifact,
    project_build_artifact_key, project_build_input_digest, project_cache_root,
    read_cached_project_bundle, runtime_plan_fingerprint, write_project_build_artifacts,
};
use arcweft_bundle::{ArcweftBundle, BundleFormat};
use arcweft_compiler::incremental::runtime_plan_artifact_key;
use arcweft_compiler::project::InMemoryProjectCompileCache;
use arcweft_project::{
    artifact::{ArtifactKeyInput, RuntimePlanArtifactKey},
    fingerprint::{BuildDigest, NamedDigest},
    incremental::{CacheRecordStatus, InvalidationReason, QueryKind},
    persistent_object::{BytecodeLinkIdentityOwner, BytecodeLinkProducerFamily},
};
use arcweft_project_loader::cache::record::CacheRecord;
use arcweft_verify::VerificationMode;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn compile_emit_owns_output_path_policy() {
    assert_eq!(
        CompileEmit::Hir
            .output_path(Path::new("src/main.arcw"), None)
            .expect("HIR output resolves")
            .as_deref(),
        Some(Path::new("src/main.hir"))
    );
    assert_eq!(
        CompileEmit::Plan
            .output_path(
                Path::new("src/main.arcw"),
                Some(Path::new("target/main.plan")),
            )
            .expect("explicit plan output resolves")
            .as_deref(),
        Some(Path::new("target/main.plan"))
    );
    assert!(
        CompileEmit::Check
            .output_path(Path::new("src/main.arcw"), Some(Path::new("unused")))
            .is_err()
    );
}

#[test]
fn runtime_plan_cache_envelope_rejects_wrong_schema_and_fingerprint() {
    let key = runtime_plan_test_key("accepted");
    let encoded =
        encode_runtime_plan_artifact(key).expect("canonical runtime plan receipt encodes");
    let decoded =
        decode_runtime_plan_artifact(key, &encoded).expect("canonical runtime plan decodes");
    assert_eq!(decoded.schema_version, RUNTIME_PLAN_ARTIFACT_SCHEMA_VERSION);

    let mut wrong_schema: serde_json::Value =
        serde_json::from_slice(&encoded).expect("runtime plan JSON");
    wrong_schema["schema_version"] = serde_json::json!(0);
    let wrong_schema = serde_json::to_vec(&wrong_schema).expect("wrong-schema JSON");
    assert!(
        decode_runtime_plan_artifact(key, &wrong_schema)
            .expect_err("old runtime-plan schema must be rejected")
            .contains("unsupported runtime-plan artifact schema")
    );

    let foreign = runtime_plan_test_key("foreign");
    assert!(
        decode_runtime_plan_artifact(foreign, &encoded)
            .expect_err("foreign artifact key must not join cached plan")
            .contains("fingerprint does not match")
    );

    let mut zero_fingerprint: serde_json::Value =
        serde_json::from_slice(&encoded).expect("runtime plan JSON");
    zero_fingerprint["artifact_fingerprint"] = serde_json::json!(vec![0_u8; 32]);
    let zero_fingerprint = serde_json::to_vec(&zero_fingerprint).expect("zero-fingerprint JSON");
    assert!(decode_runtime_plan_artifact(key, &zero_fingerprint).is_err());
}

fn runtime_plan_test_key(source: &str) -> RuntimePlanArtifactKey {
    let digest = BuildDigest::of(source.as_bytes());
    RuntimePlanArtifactKey::try_derive(&ArtifactKeyInput {
        compiler_build_id: "cli-test".to_owned(),
        query: QueryKind::RuntimePlan,
        artifact_kind: QueryKind::RuntimePlan.artifact_kind(),
        target_triple: "native".to_owned(),
        target_features: Vec::new(),
        profile: "dev".to_owned(),
        package: "org.arcweft.test".to_owned(),
        logical_item: "runtime-plan".to_owned(),
        source_digest: digest,
        dependency_interface_digests: vec![NamedDigest::new("crate", digest)],
        dependency_body_digests: vec![NamedDigest::new("crate", digest)],
        adapter_environment_digest: BuildDigest::of(b"adapter"),
        launch_profile_digest: BuildDigest::of(b"launch"),
        declared_environment_digest: BuildDigest::of(b"environment"),
        format_options_digest: BuildDigest::of(b"runtime-plan"),
    })
    .expect("canonical runtime-plan key")
}

#[test]
fn release_project_diagnostics_reject_dynamic_goto() {
    let root = temp_project_root("release-dynamic-goto");
    let source_root = root.join("src");
    fs::create_dir_all(&source_root).expect("source dir creates");
    fs::write(
        root.join("arcw.toml"),
        r#"
schema = 1

[package]
id = "org.arcweft.test.release-dynamic-goto"
version = "0.1.0"
"#,
    )
    .expect("manifest writes");
    fs::write(
        source_root.join("main.arcw"),
        r#"
entry cli @entry.main { goto @flow.opening }

flow opening {
let route = @flow.done
goto route
}

flow done() -> String {
return "done"
}
"#,
    )
    .expect("source writes");
    let profile = ProfileOptions {
        profile: None,
        manifest: root.join("arcw.toml"),
    };

    let dev =
        compile_project_command(&profile, VerificationMode::Dev).expect("dev project compiles");
    assert!(
        !dev.verification
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.id.starts_with("diagnostic.release.dynamic_goto") })
    );
    let release = compile_project_command(&profile, VerificationMode::Release)
        .expect("release project compiles before release diagnostics");

    assert!(release.verification.has_errors());
    assert!(release.verification.diagnostics.iter().any(|diagnostic| {
        diagnostic.id.starts_with("diagnostic.release.dynamic_goto")
            && diagnostic.message.contains("flow.opening")
    }));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_report_counts_flows_across_modules() {
    let (root, profile) = cache_test_project("module-preserving-flow-count");
    fs::write(
        root.join("src").join("support.arcw"),
        r#"mod support

flow support_ready() -> String {
return "support"
}
"#,
    )
    .expect("support module writes");

    let state = compile_project_command(&profile, VerificationMode::Dev).expect("project compiles");
    let report = ProjectCommandReport::from_state(&state);

    assert_eq!(report.modules.len(), 2);
    assert_eq!(report.flows, 3);
    let _ = fs::remove_dir_all(root);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the test verifies one cache identity across compilation, persisted artifacts, and replay"
)]
fn cache_build_writes_persistent_query_evidence_and_preserves_awfb_root() {
    let (root, profile) = cache_test_project("persistent-query-evidence");
    let target_root = root.join("target").join("debug");
    let first =
        compile_project_command(&profile, VerificationMode::Dev).expect("first project compiles");
    let mut phases = Vec::new();
    let lowered = compile_accepted_project_runtime_plan(
        first.loaded.sources(),
        &first.selection,
        Arc::clone(&first.source_document),
        Arc::clone(&first.compiled),
        &mut phases,
    )
    .expect("accepted project lowers without recompilation");
    assert!(
        Arc::ptr_eq(&lowered.compiled, &first.compiled),
        "runtime-product lowering must retain the exact accepted CompiledProject Arc"
    );
    let first_report = ProjectCommandReport::from_state(&first);
    let first_artifacts = write_project_build_artifacts(&first, &first_report, &target_root)
        .expect("first artifacts write");
    let runtime_plan_key = runtime_plan_artifact_key(&first.snapshot, &first.compiled);
    let runtime_plan = decode_runtime_plan_artifact(
        runtime_plan_key,
        &fs::read(&first_artifacts.plan_path).expect("runtime-plan artifact reads"),
    )
    .expect("runtime-plan artifact retains its canonical identity");
    let bundle =
        ArcweftBundle::from_format_slice(BundleFormat::Awfb, &first_artifacts.bundle_bytes)
            .expect("project build AWFB decodes");
    let expected_runtime_artifact =
        runtime_plan_fingerprint(runtime_plan_key).expect("canonical runtime artifact identity");
    assert_eq!(runtime_plan.artifact_fingerprint, expected_runtime_artifact);
    assert_eq!(
        bundle.manifest.runtime.artifact_fingerprint, expected_runtime_artifact,
        "the persisted plan and bundle must retain the exact accepted project artifact identity"
    );
    let bundle_key = project_build_artifact_key(
        &first,
        QueryKind::BundleIndex,
        QueryKind::BundleIndex.artifact_kind(),
        "program-awfb",
        project_build_input_digest(&first).expect("project build input digest"),
    );
    assert!(
        read_cached_project_bundle(
            &first_artifacts.cache_root,
            bundle_key,
            expected_runtime_artifact,
        )
        .is_some(),
        "the exact typed runtime artifact must remain cache-readable"
    );
    assert!(
        read_cached_project_bundle(
            &first_artifacts.cache_root,
            bundle_key,
            runtime_plan_fingerprint(runtime_plan_test_key("foreign"))
                .expect("foreign runtime artifact identity"),
        )
        .is_none(),
        "a valid AWFB stored under the generic key must not cross a runtime artifact boundary"
    );
    assert!(first_artifacts.snapshot.queries().iter().any(|query| {
        query.query() == QueryKind::Parse
            && matches!(
                query.status(),
                CacheRecordStatus::Rebuilt {
                    reason: InvalidationReason::MissingRecord
                }
            )
    }));
    assert!(first_artifacts.snapshot.queries().iter().any(|query| {
        query.query() == QueryKind::Interface
            && matches!(
                query.status(),
                CacheRecordStatus::Rebuilt {
                    reason: InvalidationReason::MissingRecord
                }
            )
    }));

    let second =
        compile_project_command(&profile, VerificationMode::Dev).expect("second project compiles");
    let second_report = ProjectCommandReport::from_state(&second);
    let second_artifacts = write_project_build_artifacts(&second, &second_report, &target_root)
        .expect("second artifacts write");
    let second_runtime_plan_key = runtime_plan_artifact_key(&second.snapshot, &second.compiled);
    let second_runtime_plan = decode_runtime_plan_artifact(
        second_runtime_plan_key,
        &fs::read(&second_artifacts.plan_path).expect("second runtime-plan artifact reads"),
    )
    .expect("second runtime-plan artifact retains its canonical identity");
    let second_bundle =
        ArcweftBundle::from_format_slice(BundleFormat::Awfb, &second_artifacts.bundle_bytes)
            .expect("second project build AWFB decodes");
    assert_eq!(
        second_runtime_plan.artifact_fingerprint,
        second_bundle.manifest.runtime.artifact_fingerprint,
        "a bundle-cache hit must retain the current runtime-plan artifact identity"
    );

    assert_eq!(
        first_artifacts.snapshot.content_root(),
        second_artifacts.snapshot.content_root()
    );
    assert!(second_artifacts.snapshot.queries().iter().any(|query| {
        matches!(query.query(), QueryKind::Interface | QueryKind::HirBody)
            && matches!(
                query.status(),
                CacheRecordStatus::HitThenRebuilt {
                    reason: InvalidationReason::ConservativeInvalidation { .. }
                }
            )
    }));
    assert!(second_artifacts.cache_records.iter().any(|record| {
        record.query == QueryKind::BytecodeUnit
            && record.status == "hit"
            && matches!(
                &record.reuse_evidence,
                Some(ProjectBuildPersistentReuseEvidence::ActualReusable {
                    producer_family,
                    identity_owner,
                    identity,
                }) if *producer_family == BytecodeLinkProducerFamily::FullBuild
                    && *identity_owner == BytecodeLinkIdentityOwner::FullBuildBytecodeUnitArtifact
                    && identity.len() == 64
            )
    }));
    assert!(second_artifacts.cache_records.iter().any(|record| {
        record.query == QueryKind::LinkPlan
            && record.status == "hit"
            && matches!(
                &record.reuse_evidence,
                Some(ProjectBuildPersistentReuseEvidence::ActualReusable {
                    producer_family,
                    identity_owner,
                    identity,
                }) if *producer_family == BytecodeLinkProducerFamily::FullBuild
                    && *identity_owner == BytecodeLinkIdentityOwner::FullBuildLinkPlanArtifact
                    && identity.len() == 64
            )
    }));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cache_watch_in_memory_hits_take_precedence_over_corrupt_disk_records() {
    let (root, profile) = cache_test_project("watch-memory-precedence");
    let target_root = root.join("target").join("debug");
    let mut compile_cache = InMemoryProjectCompileCache::default();
    let session = Arc::new(Mutex::new(
        ProjectCommandSession::try_new().expect("project command session"),
    ));
    let first = compile_project_command_in_session(
        &profile,
        VerificationMode::Dev,
        &mut compile_cache,
        Arc::clone(&session),
        None,
    )
    .expect("first watch build compiles");
    let first_report = ProjectCommandReport::from_state(&first);
    write_project_build_artifacts(&first, &first_report, &target_root)
        .expect("first watch artifacts write");
    corrupt_persistent_query_records(&project_cache_root(&target_root));
    let mut selection = first.selection.clone();
    selection.refresh().expect("project selection reloads");

    let second = compile_project_command_in_session(
        &profile,
        VerificationMode::Dev,
        &mut compile_cache,
        session,
        Some(selection),
    )
    .expect("second watch build compiles");
    assert!(
        second
            .compiled
            .compile_units()
            .iter()
            .all(|unit| unit.cache_status().is_hit())
    );
    let second_report = ProjectCommandReport::from_state(&second);
    let second_artifacts = write_project_build_artifacts(&second, &second_report, &target_root)
        .expect("second watch artifacts write");
    assert!(second_artifacts.snapshot.queries().iter().any(|query| {
        matches!(
            query.query(),
            QueryKind::Parse | QueryKind::Interface | QueryKind::HirBody
        ) && query.status().is_hit()
    }));
    assert!(!second_artifacts.snapshot.queries().iter().any(|query| {
        query.status().rebuild_reason().is_some_and(|reason| {
            matches!(
                reason,
                InvalidationReason::CorruptRecord | InvalidationReason::CorruptObject
            )
        })
    }));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cache_clean_build_records_corrupt_object_rebuild_reason() {
    let (root, profile) = cache_test_project("corrupt-object-rebuild");
    let target_root = root.join("target").join("debug");
    let first =
        compile_project_command(&profile, VerificationMode::Dev).expect("first project compiles");
    let first_report = ProjectCommandReport::from_state(&first);
    write_project_build_artifacts(&first, &first_report, &target_root)
        .expect("first artifacts write");
    corrupt_persistent_query_objects(&project_cache_root(&target_root));

    let second =
        compile_project_command(&profile, VerificationMode::Dev).expect("second project compiles");
    let second_report = ProjectCommandReport::from_state(&second);
    let second_artifacts = write_project_build_artifacts(&second, &second_report, &target_root)
        .expect("second artifacts write");
    assert!(second_artifacts.snapshot.queries().iter().any(|query| {
        matches!(query.query(), QueryKind::Parse | QueryKind::Interface)
            && matches!(
                query.status(),
                CacheRecordStatus::Rebuilt {
                    reason: InvalidationReason::CorruptObject
                }
            )
    }));
    let _ = fs::remove_dir_all(root);
}

fn temp_project_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "arcweft-project-command-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn cache_test_project(label: &str) -> (PathBuf, ProfileOptions) {
    let root = temp_project_root(label);
    let source_root = root.join("src");
    fs::create_dir_all(&source_root).expect("source dir creates");
    fs::write(
        root.join("arcw.toml"),
        r#"
schema = 1

[package]
id = "org.arcweft.test.cache-persistent-query"
version = "0.1.0"
"#,
    )
    .expect("manifest writes");
    fs::write(
        source_root.join("main.arcw"),
        r#"
entry cli @entry.main { goto @flow.opening }

flow opening {
goto @flow.done
}

flow done() -> String {
return "done"
}
"#,
    )
    .expect("source writes");
    let profile = ProfileOptions {
        profile: None,
        manifest: root.join("arcw.toml"),
    };
    (root, profile)
}

fn corrupt_persistent_query_records(cache_root: &Path) {
    for namespace in ["parse", "interface", "hir-body"] {
        corrupt_records_under(&cache_root.join("records").join(namespace));
    }
}

fn corrupt_records_under(path: &Path) {
    if path.is_file() {
        if path
            .extension()
            .is_some_and(|extension| extension == "awci")
        {
            fs::write(path, b"not-json").expect("record corrupts");
        }
        return;
    }
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            corrupt_records_under(&entry.path());
        }
    }
}

fn corrupt_persistent_query_objects(cache_root: &Path) {
    for namespace in ["parse", "interface", "hir-body"] {
        corrupt_objects_for_records(cache_root, &cache_root.join("records").join(namespace));
    }
}

fn corrupt_objects_for_records(cache_root: &Path, path: &Path) {
    if path.is_file() {
        if path
            .extension()
            .is_some_and(|extension| extension == "awci")
        {
            let bytes = fs::read(path).expect("record reads");
            let record = CacheRecord::from_slice(&bytes).expect("record decodes");
            fs::write(
                cache_object_path(cache_root, record.object_digest()),
                b"not-awbo",
            )
            .expect("object corrupts");
        }
        return;
    }
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            corrupt_objects_for_records(cache_root, &entry.path());
        }
    }
}

fn cache_object_path(cache_root: &Path, digest: BuildDigest) -> PathBuf {
    let hex = digest.to_hex();
    cache_root
        .join("objects")
        .join("blake3")
        .join(&hex[..2])
        .join(&hex[2..])
}
