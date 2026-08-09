//! Canonical fresh-session runtime artifact identity for CLI compilation.

use crate::app::project::SourceSelection;
use arcweft_compiler::{
    incremental::{BuildSnapshotRequest, runtime_plan_artifact_key, snapshot_compiled_project},
    project::CompiledProject,
    runtime_diagnostics::ExecutionDiagnosticContext,
};
use arcweft_project::{
    artifact::RuntimePlanArtifactKey, incremental::BuildSnapshot, sources::ProjectSources,
};
use std::{env, process::ExitCode, sync::Arc};

pub(in crate::app) fn accepted_build_snapshot(
    selection: &SourceSelection,
    sources: &ProjectSources,
    compiled: &CompiledProject,
) -> BuildSnapshot {
    snapshot_compiled_project(
        sources,
        compiled,
        BuildSnapshotRequest {
            build_id: compiled.program_hash().as_str().to_owned(),
            compiler_build_id: env!("CARGO_PKG_VERSION").to_owned(),
            target_triple: format!("{}-{}", env::consts::ARCH, env::consts::OS),
            target_features: Vec::new(),
            profile: selection.profile().map_or_else(
                || "default".to_owned(),
                |profile| profile.id().as_str().to_owned(),
            ),
            selected_entries: selected_snapshot_entries(selection),
        },
    )
}

pub(in crate::app) fn accepted_runtime_plan_artifact_key(
    selection: &SourceSelection,
    sources: &ProjectSources,
    compiled: &CompiledProject,
) -> RuntimePlanArtifactKey {
    let snapshot = accepted_build_snapshot(selection, sources, compiled);
    runtime_plan_artifact_key(&snapshot, compiled)
}

pub(in crate::app) fn bind_execution_diagnostics(
    selection: &SourceSelection,
    sources: &ProjectSources,
    compiled: &CompiledProject,
) -> Result<Arc<ExecutionDiagnosticContext>, ExitCode> {
    let artifact_key = accepted_runtime_plan_artifact_key(selection, sources, compiled);
    compiled
        .execution_diagnostic_context(artifact_key)
        .map(Arc::new)
        .map_err(|error| {
            eprintln!("error: failed to bind runtime diagnostic identity: {error}");
            ExitCode::FAILURE
        })
}

pub(in crate::app) fn selected_snapshot_entries(selection: &SourceSelection) -> Vec<String> {
    selection.profile().map_or_else(
        || vec![selection.path().display().to_string()],
        |profile| vec![profile.id().as_str().to_owned()],
    )
}
