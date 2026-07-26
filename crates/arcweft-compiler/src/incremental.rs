use crate::project::{CompiledProject, ProjectCompileCacheStatus};
use arcweft_project::{
    artifact::{ArtifactKey, ArtifactKeyInput},
    fingerprint::{BuildDigest, ProjectFingerprint, ProjectFingerprintInput},
    incremental::{
        BuildSnapshot, CacheRecordStatus, InvalidationReason, ModuleBuildFingerprint, QueryKind,
        QuerySnapshot,
    },
    sources::ProjectSources,
};

/// Snapshot options for the current conservative project compiler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildSnapshotRequest {
    pub build_id: String,
    pub compiler_build_id: String,
    pub target_triple: String,
    pub target_features: Vec<String>,
    pub profile: String,
    pub selected_entries: Vec<String>,
}

/// Builds a conservative snapshot for the current linked-HIR semantic pass.
pub fn snapshot_compiled_project(
    sources: &ProjectSources,
    compiled: &CompiledProject,
    request: BuildSnapshotRequest,
) -> BuildSnapshot {
    let environment_digest = BuildDigest::from_bytes(
        *compiled
            .registered_environment()
            .environment_digest()
            .as_bytes(),
    );
    let project_fingerprint = project_fingerprint(sources, &request, environment_digest);
    let modules = compiled
        .modules()
        .iter()
        .map(|module| {
            let source_digest = BuildDigest::from(module.source().revision());
            let interface_digest = BuildDigest::of(
                format!("{}:interface:{source_digest}", module.module()).as_bytes(),
            );
            let body_digest =
                BuildDigest::of(format!("{}:body:{source_digest}", module.module()).as_bytes());
            ModuleBuildFingerprint::new(
                module.module().to_string(),
                source_digest,
                interface_digest,
                body_digest,
            )
        })
        .collect::<Vec<_>>();
    let queries = compiled
        .compile_units()
        .iter()
        .map(|unit| {
            let artifact_digest = BuildDigest::from_bytes(unit.fingerprint().as_bytes());
            let key = ArtifactKey::derive(&ArtifactKeyInput {
                compiler_build_id: request.compiler_build_id.clone(),
                query: QueryKind::HirBody,
                artifact_kind: QueryKind::HirBody.artifact_kind(),
                target_triple: request.target_triple.clone(),
                target_features: request.target_features.clone(),
                profile: request.profile.clone(),
                package: sources.package().id.as_str().to_owned(),
                logical_item: unit
                    .modules()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("+"),
                source_digest: artifact_digest,
                dependency_interface_digests: Vec::new(),
                dependency_body_digests: Vec::new(),
                adapter_environment_digest: environment_digest,
                launch_profile_digest: BuildDigest::ZERO,
                declared_environment_digest: BuildDigest::ZERO,
                format_options_digest: BuildDigest::ZERO,
            });
            QuerySnapshot::new(
                QueryKind::HirBody,
                key,
                artifact_digest,
                cache_status(unit.cache_status()),
            )
        })
        .collect::<Vec<_>>();
    BuildSnapshot::new(
        request.build_id,
        project_fingerprint,
        request.selected_entries,
        modules,
        queries,
    )
}

fn project_fingerprint(
    sources: &ProjectSources,
    request: &BuildSnapshotRequest,
    environment_digest: BuildDigest,
) -> ProjectFingerprint {
    let mut source_bytes = Vec::new();
    for module in sources.modules() {
        source_bytes.extend_from_slice(module.module().to_string().as_bytes());
        source_bytes.extend_from_slice(module.source_revision().as_bytes());
    }
    ProjectFingerprint::new(ProjectFingerprintInput {
        package: sources.package().id.as_str().to_owned(),
        compiler_build_id: request.compiler_build_id.clone(),
        target_triple: request.target_triple.clone(),
        target_features: request.target_features.clone(),
        profile: request.profile.clone(),
        source_root_digest: BuildDigest::of(&source_bytes),
        manifest_digest: BuildDigest::of(sources.manifest_path().to_string_lossy().as_bytes()),
        // This existing field is also the source for
        // `CompilerObjectKey::environment_digest`. Once registration has
        // succeeded it carries the digest of the complete accepted semantic
        // environment, not an adapter-only projection.
        adapter_environment_digest: environment_digest,
        launch_profile_digest: BuildDigest::ZERO,
        declared_environment_digest: BuildDigest::ZERO,
    })
}

const fn cache_status(status: ProjectCompileCacheStatus) -> CacheRecordStatus {
    match status {
        ProjectCompileCacheStatus::Hit => CacheRecordStatus::Hit,
        ProjectCompileCacheStatus::Miss => CacheRecordStatus::Rebuilt {
            reason: InvalidationReason::MissingRecord,
        },
        ProjectCompileCacheStatus::Disabled => CacheRecordStatus::Rebuilt {
            reason: InvalidationReason::OptionsChanged,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::cache_status;
    use crate::project::ProjectCompileCacheStatus;
    use arcweft_project::incremental::{CacheRecordStatus, InvalidationReason};

    #[test]
    fn persistent_query_snapshot_status_records_rebuild_reason() {
        assert_eq!(
            cache_status(ProjectCompileCacheStatus::Miss),
            CacheRecordStatus::Rebuilt {
                reason: InvalidationReason::MissingRecord,
            }
        );
        assert_eq!(
            cache_status(ProjectCompileCacheStatus::Disabled),
            CacheRecordStatus::Rebuilt {
                reason: InvalidationReason::OptionsChanged,
            }
        );
    }
}
