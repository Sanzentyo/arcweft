//! Host task adapter for portable bundle asset resolution.

use arcweft_adapter_context::{manifest::AdapterManifest, standard};
use arcweft_bundle::BundleArtifact;
use arcweft_bundle_assets::{
    BundleAssetResolver, BundleAssetResolverError, BundleAssetValidationError,
};
use arcweft_core::task::{
    BoundTaskOutcome, BoundTaskSpec, GenerationId, HostTaskRequest, TaskId, TaskSpec,
};
use arcweft_core::value::{RuntimeBundleAssetContext, RuntimeValue};
use arcweft_host_adapter::{
    HostAdapter, HostAdapterCompletion, HostTaskCompletion, HostTaskMetrics, HostTaskOutcome,
    HostTaskSubmission, HostTaskSubmissionContext,
};
use std::sync::{Arc, Mutex, MutexGuard};
use thiserror::Error;

/// Standard `asset.image` and `asset.voice` Need producer host adapter.
#[derive(Clone, Debug)]
pub struct BundleAssetAdapter {
    manifest: AdapterManifest,
    resolver: BundleAssetResolver,
    completions: Arc<Mutex<Vec<HostAdapterCompletion>>>,
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
            completions: Arc::default(),
        }
    }

    /// Creates an adapter bound to one initial generation and its accepted artifact.
    pub fn try_new(
        context: RuntimeBundleAssetContext,
        artifact: BundleArtifact,
    ) -> Result<Self, BundleAssetAdapterError> {
        Ok(Self {
            manifest: standard::bundle_asset_manifest(),
            resolver: BundleAssetResolver::try_new(context, artifact)?,
            completions: Arc::default(),
        })
    }

    /// Binds the immutable catalog for an accepted generation.
    pub fn bind_generation(
        &self,
        context: RuntimeBundleAssetContext,
        artifact: BundleArtifact,
    ) -> Result<(), BundleAssetAdapterError> {
        self.resolver.bind_generation(context, artifact)?;
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

    fn resolve_asset(
        &self,
        task: &TaskSpec,
        outcome: &BoundTaskOutcome,
        submission: &HostTaskSubmissionContext,
    ) -> Option<HostTaskOutcome> {
        let HostTaskRequest::AssetLoad(request) = task.request.as_host()? else {
            return None;
        };
        let Some(context) = submission.bundle_asset_context() else {
            return Some(failed(
                "bundle asset task has no retained generation artifact identity",
            ));
        };
        let result = self.resolver.load(request.kind, context, &request.id);
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

    fn completions(&self) -> MutexGuard<'_, Vec<HostAdapterCompletion>> {
        self.completions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
        let outcome = self.resolve_asset(task.spec(), task.outcome(), &context)?;
        // Keep the accepted attempt live until the host drains its result at
        // step ingress. A checkpoint can retain the dispatch at that boundary.
        self.completions().push(HostAdapterCompletion {
            dispatch: context.attempt().clone(),
            publication_revision: context.next_publication_revision(),
            outcome,
        });
        Some(HostTaskSubmission::Pending)
    }

    fn drain_completions(&self) -> Vec<HostAdapterCompletion> {
        std::mem::take(&mut *self.completions())
    }

    fn cancel(&self, task_id: &TaskId) -> bool {
        let mut completions = self.completions();
        let before = completions.len();
        completions
            .retain(|completion| &completion.dispatch.identity().correlation.task_id != task_id);
        completions.len() != before
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
