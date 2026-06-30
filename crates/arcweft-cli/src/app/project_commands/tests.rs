use super::{
    CompileEmit, ProfileOptions, ProjectBuildPersistentReuseEvidence, ProjectCommandReport,
    compile_project_command, compile_project_command_with_cache, project_cache_root,
    write_project_build_artifacts,
};
use arcweft_compiler::project::InMemoryProjectCompileCache;
use arcweft_project::{
    fingerprint::BuildDigest,
    incremental::{CacheRecordStatus, InvalidationReason, QueryKind},
};
use arcweft_project_loader::cache::record::CacheRecord;
use arcweft_verify::VerificationMode;
use std::{
    fs,
    path::{Path, PathBuf},
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
fn release_project_diagnostics_reject_dynamic_goto() {
    let root = temp_project_root("release-dynamic-goto");
    let source_root = root.join("src");
    fs::create_dir_all(&source_root).expect("source dir creates");
    fs::write(
        root.join("arcw.toml"),
        r#"
[package]
name = "release_dynamic_goto"
"#,
    )
    .expect("manifest writes");
    fs::write(
        source_root.join("main.arcw"),
        r#"
entry game {
start(@flow.opening)
}

flow opening {
let route = @flow.done
goto route
}

flow done {
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
fn cache_build_writes_persistent_query_evidence_and_preserves_awfb_root() {
    let (root, profile) = cache_test_project("persistent-query-evidence");
    let target_root = root.join("target").join("debug");
    let first =
        compile_project_command(&profile, VerificationMode::Dev).expect("first project compiles");
    let first_report = ProjectCommandReport::from_state(&first);
    let first_artifacts = write_project_build_artifacts(&first, &first_report, &target_root)
        .expect("first artifacts write");
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
            && record.status == "hit_then_rebuilt"
            && matches!(
                &record.reuse_evidence,
                Some(ProjectBuildPersistentReuseEvidence::ActualReusable {
                    producer,
                    identity,
                }) if producer == "full_build_bytecode_unit" && identity.len() == 64
            )
    }));
    assert!(second_artifacts.cache_records.iter().any(|record| {
        record.query == QueryKind::LinkPlan
            && record.status == "hit_then_rebuilt"
            && matches!(
                &record.reuse_evidence,
                Some(ProjectBuildPersistentReuseEvidence::ActualReusable {
                    producer,
                    identity,
                }) if producer == "full_build_link_plan" && identity.len() == 64
            )
    }));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cache_watch_in_memory_hits_take_precedence_over_corrupt_disk_records() {
    let (root, profile) = cache_test_project("watch-memory-precedence");
    let target_root = root.join("target").join("debug");
    let mut compile_cache = InMemoryProjectCompileCache::default();
    let first =
        compile_project_command_with_cache(&profile, VerificationMode::Dev, &mut compile_cache)
            .expect("first watch build compiles");
    let first_report = ProjectCommandReport::from_state(&first);
    write_project_build_artifacts(&first, &first_report, &target_root)
        .expect("first watch artifacts write");
    corrupt_persistent_query_records(&project_cache_root(&target_root));

    let second =
        compile_project_command_with_cache(&profile, VerificationMode::Dev, &mut compile_cache)
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
[package]
name = "cache_persistent_query"
"#,
    )
    .expect("manifest writes");
    fs::write(
        source_root.join("main.arcw"),
        r#"
entry game {
start(@flow.opening)
}

flow opening {
goto @flow.done
}

flow done {
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
