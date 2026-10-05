//! Host task adapter for portable bundle asset resolution.

use arcweft_adapter_context::{manifest::AdapterManifest, standard};
use arcweft_bundle::ArcweftBundle;
use arcweft_bundle::BundleArtifactIdentity;
use arcweft_bundle_assets::{
    BundleAssetResolver, BundleAssetResolverError, BundleAssetValidationError,
};
use arcweft_core::task::{
    BoundTaskOutcome, BoundTaskSpec, GenerationId, HostTaskRequest, TaskSpec,
};
use arcweft_core::value::{RuntimeBundleAssetContext, RuntimeValue};
use arcweft_host_adapter::{
    HostAdapter, HostTaskCompletion, HostTaskMetrics, HostTaskOutcome, HostTaskSubmission,
    HostTaskSubmissionContext,
};
use std::sync::Arc;
use thiserror::Error;

/// Standard `asset.image` and `asset.voice` Need producer host adapter.
#[derive(Clone, Debug)]
pub struct BundleAssetAdapter {
    manifest: AdapterManifest,
    resolver: BundleAssetResolver,
}

/// Failure to create or update the portable resolver owned by this adapter.
#[derive(Debug, Error)]
pub enum BundleAssetAdapterError {
    #[error(transparent)]
    Resolver(#[from] BundleAssetResolverError),
}

impl BundleAssetAdapter {
    /// Creates an adapter without bundle catalogs.
    #[must_use]
    pub fn new() -> Self {
        Self {
            manifest: standard::bundle_asset_manifest(),
            resolver: BundleAssetResolver::new(),
        }
    }

    /// Creates an adapter bound to one initial generation and exact bundle
    /// identity. AWFB identities must be obtained from validated original
    /// container bytes.
    pub fn try_new(
        context: RuntimeBundleAssetContext,
        identity: BundleArtifactIdentity,
        bundle: Arc<ArcweftBundle>,
    ) -> Result<Self, BundleAssetAdapterError> {
        Ok(Self {
            manifest: standard::bundle_asset_manifest(),
            resolver: BundleAssetResolver::try_new(context, identity, bundle)?,
        })
    }

    /// Binds the immutable catalog for an accepted generation.
    pub fn bind_generation(
        &self,
        context: RuntimeBundleAssetContext,
        identity: BundleArtifactIdentity,
        bundle: Arc<ArcweftBundle>,
    ) -> Result<(), BundleAssetAdapterError> {
        self.resolver.bind_generation(context, identity, bundle)?;
        Ok(())
    }

    /// Retires a generation catalog when no tasks or saved handles reference it.
    pub fn retire_generation(
        &self,
        generation: GenerationId,
    ) -> Result<bool, BundleAssetAdapterError> {
        self.resolver
            .retire_generation(generation)
            .map_err(Into::into)
    }

    /// Returns a shared resolver clone for renderers and restore validation.
    #[must_use]
    pub fn resolver(&self) -> BundleAssetResolver {
        self.resolver.clone()
    }

    /// Revalidates a saved image success/error against its retained catalog.
    pub fn validate_image_result(
        &self,
        value: &RuntimeValue,
    ) -> Result<(), BundleAssetValidationError> {
        self.resolver.validate_image_result(value)
    }

    /// Revalidates a saved voice success/error against its retained catalog.
    pub fn validate_voice_result(
        &self,
        value: &RuntimeValue,
    ) -> Result<(), BundleAssetValidationError> {
        self.resolver.validate_voice_result(value)
    }

    fn submit_asset(
        &self,
        task: &TaskSpec,
        outcome: &BoundTaskOutcome,
        submission: HostTaskSubmissionContext,
    ) -> Option<HostTaskOutcome> {
        let HostTaskRequest::AssetLoad(request) = &task.request else {
            return None;
        };
        let Some(context) = submission.bundle_asset_context() else {
            return Some(failed(
                "bundle asset task has no retained generation artifact identity",
            ));
        };
        let result = match request.kind.as_str() {
            "image" => self.resolver.load_image(context, &request.id),
            "voice" => self.resolver.load_voice(context, &request.id),
            kind => {
                return Some(failed(format!(
                    "unsupported bundle asset kind `{kind}` for task {:?}",
                    task.debug_label
                )));
            }
        };
        let completion = match result {
            Ok(value) => outcome.try_payload(value).map_or_else(
                |error| HostTaskCompletion::Failed(error.to_string()),
                HostTaskCompletion::Ready,
            ),
            Err(error) => HostTaskCompletion::Failed(error.to_string()),
        };
        Some(HostTaskOutcome {
            completion,
            metrics: HostTaskMetrics::default(),
        })
    }
}

impl Default for BundleAssetAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HostAdapter for BundleAssetAdapter {
    fn manifest(&self) -> &AdapterManifest {
        &self.manifest
    }

    fn submit(
        &self,
        task: &BoundTaskSpec,
        context: HostTaskSubmissionContext,
    ) -> Option<HostTaskSubmission> {
        self.submit_asset(task.spec(), task.outcome(), context)
            .map(HostTaskSubmission::Completed)
    }

    fn can_complete_in_parallel(&self, request: &HostTaskRequest) -> bool {
        matches!(request, HostTaskRequest::AssetLoad(_))
    }
}

fn failed(message: impl Into<String>) -> HostTaskOutcome {
    HostTaskOutcome {
        completion: HostTaskCompletion::Failed(message.into()),
        metrics: HostTaskMetrics::default(),
    }
}
