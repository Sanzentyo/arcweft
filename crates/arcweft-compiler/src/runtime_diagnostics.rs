//! Fresh-session capability for projecting persisted runtime failures.

use std::{fmt, sync::Arc};

use arcweft_core::effect::{
    RuntimeArtifactFingerprint, RuntimeAssertionFailure, RuntimeIdentityDecodeError,
};
use arcweft_project::artifact::RuntimePlanArtifactKey;
use arcweft_runtime_plan::assertion_identity::{
    RuntimeAssertionFault, RuntimeAssertionInventory, RuntimeAssertionProjectionError,
};
use arcweft_runtime_plan::flow::RuntimePlanLowerReport;
use thiserror::Error;

/// Exact runtime artifact and its matching fresh-session assertion inventory.
#[derive(Clone)]
pub struct ExecutionDiagnosticContext {
    artifact: RuntimeArtifactFingerprint,
    assertions: Arc<RuntimeAssertionInventory>,
}

impl fmt::Debug for ExecutionDiagnosticContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExecutionDiagnosticContext")
            .field("artifact", &self.artifact)
            .finish_non_exhaustive()
    }
}

impl ExecutionDiagnosticContext {
    /// Binds the fresh-session assertion inventory to the exact existing
    /// runtime-plan artifact key. No second artifact digest is derived.
    pub fn try_from_runtime_plan_artifact(
        artifact_key: RuntimePlanArtifactKey,
        runtime_plan: &RuntimePlanLowerReport,
    ) -> Result<Self, ExecutionDiagnosticContextError> {
        let artifact = runtime_artifact_fingerprint(artifact_key)?;
        Self::try_new(
            artifact,
            Arc::new(runtime_plan.bind_assertion_inventory(artifact)),
        )
    }

    pub fn try_new(
        artifact: RuntimeArtifactFingerprint,
        assertions: Arc<RuntimeAssertionInventory>,
    ) -> Result<Self, ExecutionDiagnosticContextError> {
        let actual = assertions.artifact();
        if artifact != actual {
            return Err(ExecutionDiagnosticContextError::ArtifactMismatch {
                expected: artifact,
                actual,
            });
        }
        Ok(Self {
            artifact,
            assertions,
        })
    }

    pub const fn artifact(&self) -> RuntimeArtifactFingerprint {
        self.artifact
    }

    pub fn assertions(&self) -> &RuntimeAssertionInventory {
        &self.assertions
    }

    pub fn project_assertion_failure(
        &self,
        failure: RuntimeAssertionFailure,
    ) -> Result<RuntimeAssertionFault, RuntimeAssertionProjectionError> {
        self.assertions.project_failure(self.artifact, failure)
    }
}

fn runtime_artifact_fingerprint(
    artifact_key: RuntimePlanArtifactKey,
) -> Result<RuntimeArtifactFingerprint, RuntimeIdentityDecodeError> {
    RuntimeArtifactFingerprint::try_from_bytes(artifact_key.digest().as_bytes())
}

/// Rejection raised before a fresh-session diagnostic capability is published.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ExecutionDiagnosticContextError {
    #[error(transparent)]
    InvalidArtifactFingerprint(#[from] RuntimeIdentityDecodeError),
    #[error("runtime assertion inventory belongs to another artifact")]
    ArtifactMismatch {
        expected: RuntimeArtifactFingerprint,
        actual: RuntimeArtifactFingerprint,
    },
}

#[cfg(test)]
mod tests {
    use arcweft_project::{
        artifact::{ArtifactKeyInput, RuntimePlanArtifactKey},
        fingerprint::BuildDigest,
        incremental::QueryKind,
    };

    use super::runtime_artifact_fingerprint;

    #[test]
    fn runtime_artifact_fingerprint_copies_the_canonical_artifact_key_digest() {
        let key = RuntimePlanArtifactKey::try_derive(&ArtifactKeyInput {
            compiler_build_id: "compiler".to_owned(),
            query: QueryKind::RuntimePlan,
            artifact_kind: QueryKind::RuntimePlan.artifact_kind(),
            target_triple: "native".to_owned(),
            target_features: vec!["base".to_owned()],
            profile: "debug".to_owned(),
            package: "story".to_owned(),
            logical_item: "runtime-plan".to_owned(),
            source_digest: BuildDigest::of(b"source"),
            dependency_interface_digests: Vec::new(),
            dependency_body_digests: Vec::new(),
            adapter_environment_digest: BuildDigest::of(b"adapter"),
            launch_profile_digest: BuildDigest::of(b"launch"),
            declared_environment_digest: BuildDigest::of(b"environment"),
            format_options_digest: BuildDigest::of(b"options"),
        })
        .expect("typed runtime-plan artifact key");

        let fingerprint = runtime_artifact_fingerprint(key).expect("non-zero BLAKE3 key");
        assert_eq!(fingerprint.as_bytes(), &key.digest().as_bytes());
    }
}
