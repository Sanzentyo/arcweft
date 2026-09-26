//! Portable decoding, identity binding, and typed results for bundled assets.

use arcweft_audio_codec::{AudioDecodeLimits, decode_audio};
use arcweft_bundle::logical_identity::LogicalBundleIdentity;
use arcweft_bundle::{
    ArcweftBundle, BundleArtifactIdentity, BundleCodecError, BundleImageAnimation,
    BundleImageAsset, BundleImageFormat,
};
use arcweft_core::pattern::RuntimeBuiltinVariantCaseIdentity;
use arcweft_core::task::{AssetLoadKind, GenerationId};
use arcweft_core::value::{
    RuntimeAssetContentDigest, RuntimeAssetErrorValue, RuntimeAudioHandleValue,
    RuntimeBundleAssetBinding, RuntimeBundleAssetContext, RuntimeBundleAssetFailure,
    RuntimeBundleAssetFailureReason, RuntimeBundleAssetOpaqueRole, RuntimeBundleAssetResourceId,
    RuntimeBundleAssetValueError, RuntimeImageHandleValue, RuntimeValue, RuntimeVoiceErrorValue,
    runtime_bundle_asset_opaque_role,
};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Clone, Debug)]
struct BoundBundleCatalog {
    context: RuntimeBundleAssetContext,
    identity: BundleArtifactIdentity,
    catalog_identity: LogicalBundleIdentity,
    bundle: Arc<ArcweftBundle>,
}

/// Portable generation-indexed catalog for bundle image and voice resources.
///
/// Each catalog is immutable once bound. Old catalogs remain available until
/// the owner explicitly retires their generation, which keeps in-flight asset
/// requests and restored handles tied to the bundle that created them.
#[derive(Clone, Debug, Default)]
pub struct BundleAssetResolver {
    generations: Arc<RwLock<BTreeMap<GenerationId, BoundBundleCatalog>>>,
}

/// Failure to bind or query an exact bundle generation.
#[derive(Debug, Error)]
pub enum BundleAssetResolverError {
    #[error("bundle asset generation table is poisoned")]
    GenerationTablePoisoned,
    #[error("generation {generation:?} is already bound to another bundle asset catalog")]
    GenerationAlreadyBound { generation: GenerationId },
    #[error("bundle artifact identity differs from generation {generation:?} context")]
    ArtifactContextMismatch { generation: GenerationId },
    #[error("logical bundle identity differs from generation {generation:?} catalog")]
    LogicalCatalogMismatch { generation: GenerationId },
    #[error("failed to compute complete logical bundle identity: {0}")]
    LogicalIdentity(#[source] BundleCodecError),
    #[error("asset id is not a canonical Asset identity")]
    InvalidResourceId(#[source] RuntimeBundleAssetValueError),
    #[error("bundle catalog lookup failed: {0}")]
    BundleLookup(#[source] BundleCodecError),
    #[error("asset content length cannot be represented by the digest contract")]
    ContentLengthOverflow,
    #[error(transparent)]
    Validation(#[from] BundleAssetValidationError),
    #[error(transparent)]
    Value(#[from] RuntimeBundleAssetValueError),
}

/// Revalidation failure for a saved image/audio result or a handle resolution.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum BundleAssetValidationError {
    #[error("runtime value is not the exact typed image or asset-error result")]
    InvalidImageResult,
    #[error("runtime value is not the exact typed audio or voice-error result")]
    InvalidVoiceResult,
    #[error("bundle asset generation is not retained")]
    GenerationNotRetained,
    #[error("bundle asset context differs from the retained catalog identity")]
    ArtifactIdentityMismatch,
    #[error("asset resource is missing from the retained bundle catalog")]
    MissingResource,
    #[error("asset resource bytes are missing from the retained bundle")]
    MissingBytes,
    #[error("asset resource content digest differs from the retained bundle bytes")]
    ContentDigestMismatch,
    #[error("saved handle or domain error does not match retained resource validity")]
    ResultReasonMismatch,
    #[error("retained bundle asset catalog could not resolve resource bytes")]
    CatalogLookupFailure,
    #[error("retained bundle asset content length cannot be represented")]
    ContentLengthFailure,
    #[error("bundle asset generation table is poisoned")]
    GenerationTablePoisoned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AssetContentStatus {
    MissingResource,
    MissingBytes,
    Valid,
    Decode,
    MetadataMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AssetContentState {
    status: AssetContentStatus,
    digest: Option<RuntimeAssetContentDigest>,
}

impl BundleAssetResolver {
    /// Creates a resolver without catalogs. Bind accepted generations before
    /// submitting asset requests or validating restored results.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a resolver bound to one initial generation and exact bundle
    /// identity. AWFB identity values must come from the validated original
    /// container bytes before calling this constructor.
    pub fn try_new(
        context: RuntimeBundleAssetContext,
        identity: BundleArtifactIdentity,
        bundle: Arc<ArcweftBundle>,
    ) -> Result<Self, BundleAssetResolverError> {
        let resolver = Self::new();
        resolver.bind_generation(context, identity, bundle)?;
        Ok(resolver)
    }

    /// Binds an immutable catalog to the generation and artifact digest in the
    /// Core context. Logical identities are recomputed from the typed bundle;
    /// AWFB identities must have been obtained from its original bytes.
    pub fn bind_generation(
        &self,
        context: RuntimeBundleAssetContext,
        identity: BundleArtifactIdentity,
        bundle: Arc<ArcweftBundle>,
    ) -> Result<(), BundleAssetResolverError> {
        let generation = context.generation();
        if identity.binding_digest().as_bytes() != *context.artifact().as_bytes() {
            return Err(BundleAssetResolverError::ArtifactContextMismatch { generation });
        }
        let catalog_identity = bundle
            .logical_identity()
            .map_err(BundleAssetResolverError::LogicalIdentity)?;
        if matches!(identity, BundleArtifactIdentity::LogicalBundle { identity } if identity != catalog_identity)
        {
            return Err(BundleAssetResolverError::LogicalCatalogMismatch { generation });
        }

        let mut generations = self
            .generations
            .write()
            .map_err(|_| BundleAssetResolverError::GenerationTablePoisoned)?;
        if let Some(existing) = generations.get(&generation) {
            if existing.context == context
                && existing.identity == identity
                && existing.catalog_identity == catalog_identity
            {
                return Ok(());
            }
            return Err(BundleAssetResolverError::GenerationAlreadyBound { generation });
        }
        generations.insert(
            generation,
            BoundBundleCatalog {
                context,
                identity,
                catalog_identity,
                bundle,
            },
        );
        Ok(())
    }

    /// Retires a catalog after its generation has no in-flight tasks or saved
    /// state that may refer to it.
    pub fn retire_generation(
        &self,
        generation: GenerationId,
    ) -> Result<bool, BundleAssetResolverError> {
        self.generations
            .write()
            .map(|mut generations| generations.remove(&generation).is_some())
            .map_err(|_| BundleAssetResolverError::GenerationTablePoisoned)
    }

    /// Resolves an image request into the exact `Result<ImageHandle, AssetError>`
    /// runtime value. Missing and invalid resources are typed `Result::Err`.
    pub fn load_image(
        &self,
        context: RuntimeBundleAssetContext,
        id: &str,
    ) -> Result<RuntimeValue, BundleAssetResolverError> {
        self.load(AssetLoadKind::Image, context, id)
    }

    /// Resolves a voice request into the exact `Result<AudioHandle, VoiceError>`
    /// runtime value. Missing and invalid resources are typed `Result::Err`.
    pub fn load_voice(
        &self,
        context: RuntimeBundleAssetContext,
        id: &str,
    ) -> Result<RuntimeValue, BundleAssetResolverError> {
        self.load(AssetLoadKind::Voice, context, id)
    }

    /// Revalidates a saved image success or `AssetError` against its embedded
    /// generation and complete artifact identity.
    pub fn validate_image_result(
        &self,
        value: &RuntimeValue,
    ) -> Result<(), BundleAssetValidationError> {
        self.validate_result(AssetLoadKind::Image, value)
    }

    /// Revalidates a saved voice success or `VoiceError` against its embedded
    /// generation and complete artifact identity.
    pub fn validate_voice_result(
        &self,
        value: &RuntimeValue,
    ) -> Result<(), BundleAssetValidationError> {
        self.validate_result(AssetLoadKind::Voice, value)
    }

    /// Validates any standalone opaque asset value visited during restore.
    /// Returns `false` for unrelated runtime values; malformed standard asset
    /// opaque values return an error instead of being treated as unrelated.
    pub fn validate_owned_value(
        &self,
        value: &RuntimeValue,
    ) -> Result<bool, BundleAssetValidationError> {
        match runtime_bundle_asset_opaque_role(value) {
            Some(RuntimeBundleAssetOpaqueRole::ImageHandle) => {
                let handle = RuntimeImageHandleValue::try_from_runtime_value(value)
                    .map_err(|_| BundleAssetValidationError::InvalidImageResult)?;
                let _ = self.resolve_image_handle(&handle)?;
                Ok(true)
            }
            Some(RuntimeBundleAssetOpaqueRole::AudioHandle) => {
                let handle = RuntimeAudioHandleValue::try_from_runtime_value(value)
                    .map_err(|_| BundleAssetValidationError::InvalidVoiceResult)?;
                let _ = self.resolve_voice_handle(&handle)?;
                Ok(true)
            }
            Some(RuntimeBundleAssetOpaqueRole::AssetError) => {
                let error = RuntimeAssetErrorValue::try_from_runtime_value(value)
                    .map_err(|_| BundleAssetValidationError::InvalidImageResult)?;
                self.validate_failure(
                    AssetLoadKind::Image,
                    error.failure().context(),
                    error.failure().id(),
                    error.failure().reason(),
                    error.failure().content_digest(),
                )?;
                Ok(true)
            }
            Some(RuntimeBundleAssetOpaqueRole::VoiceError) => {
                let error = RuntimeVoiceErrorValue::try_from_runtime_value(value)
                    .map_err(|_| BundleAssetValidationError::InvalidVoiceResult)?;
                self.validate_failure(
                    AssetLoadKind::Voice,
                    error.failure().context(),
                    error.failure().id(),
                    error.failure().reason(),
                    error.failure().content_digest(),
                )?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Returns validated encoded image bytes for a successfully loaded handle.
    pub fn resolve_image_handle(
        &self,
        handle: &RuntimeImageHandleValue,
    ) -> Result<Arc<[u8]>, BundleAssetValidationError> {
        self.resolve_handle(
            AssetLoadKind::Image,
            handle.binding().context(),
            handle.binding().id(),
            handle.binding().content_digest(),
        )
    }

    /// Returns validated encoded voice bytes for a successfully loaded handle.
    pub fn resolve_voice_handle(
        &self,
        handle: &RuntimeAudioHandleValue,
    ) -> Result<Arc<[u8]>, BundleAssetValidationError> {
        self.resolve_handle(
            AssetLoadKind::Voice,
            handle.binding().context(),
            handle.binding().id(),
            handle.binding().content_digest(),
        )
    }

    fn load(
        &self,
        kind: AssetLoadKind,
        context: RuntimeBundleAssetContext,
        id: &str,
    ) -> Result<RuntimeValue, BundleAssetResolverError> {
        let id = RuntimeBundleAssetResourceId::try_new(id.to_owned())
            .map_err(BundleAssetResolverError::InvalidResourceId)?;
        let catalog = self.catalog_for_context(context)?;
        let state = asset_content_state(&catalog.bundle, kind, id.as_str())?;
        match state.status {
            AssetContentStatus::MissingResource | AssetContentStatus::MissingBytes => {
                return domain_failure(
                    kind,
                    context,
                    id,
                    RuntimeBundleAssetFailureReason::Missing,
                    None,
                );
            }
            AssetContentStatus::Decode => {
                return domain_failure(
                    kind,
                    context,
                    id,
                    RuntimeBundleAssetFailureReason::Decode,
                    state.digest,
                );
            }
            AssetContentStatus::MetadataMismatch => {
                return domain_failure(
                    kind,
                    context,
                    id,
                    RuntimeBundleAssetFailureReason::MetadataMismatch,
                    state.digest,
                );
            }
            AssetContentStatus::Valid => {}
        }
        let digest = state
            .digest
            .ok_or(BundleAssetResolverError::ContentLengthOverflow)?;
        let binding = RuntimeBundleAssetBinding::try_new(context, id, digest)?;
        let handle = match kind {
            AssetLoadKind::Image => {
                RuntimeImageHandleValue::from_binding(binding).into_runtime_value()?
            }
            AssetLoadKind::Voice => {
                RuntimeAudioHandleValue::from_binding(binding).into_runtime_value()?
            }
        };
        Ok(RuntimeValue::result_ok(handle))
    }

    fn validate_result(
        &self,
        kind: AssetLoadKind,
        value: &RuntimeValue,
    ) -> Result<(), BundleAssetValidationError> {
        let (case, payload) = value
            .clone()
            .try_into_builtin_variant_case()
            .map_err(|_| invalid_result(kind))?;
        match (kind, case, payload) {
            (AssetLoadKind::Image, RuntimeBuiltinVariantCaseIdentity::ResultOk, Some(value)) => {
                let handle = RuntimeImageHandleValue::try_from_runtime_value(&value)
                    .map_err(|_| invalid_result(kind))?;
                let _ = self.resolve_image_handle(&handle)?;
                Ok(())
            }
            (AssetLoadKind::Voice, RuntimeBuiltinVariantCaseIdentity::ResultOk, Some(value)) => {
                let handle = RuntimeAudioHandleValue::try_from_runtime_value(&value)
                    .map_err(|_| invalid_result(kind))?;
                let _ = self.resolve_voice_handle(&handle)?;
                Ok(())
            }
            (AssetLoadKind::Image, RuntimeBuiltinVariantCaseIdentity::ResultErr, Some(value)) => {
                let error = RuntimeAssetErrorValue::try_from_runtime_value(&value)
                    .map_err(|_| invalid_result(kind))?;
                self.validate_failure(
                    kind,
                    error.failure().context(),
                    error.failure().id(),
                    error.failure().reason(),
                    error.failure().content_digest(),
                )
            }
            (AssetLoadKind::Voice, RuntimeBuiltinVariantCaseIdentity::ResultErr, Some(value)) => {
                let error = RuntimeVoiceErrorValue::try_from_runtime_value(&value)
                    .map_err(|_| invalid_result(kind))?;
                self.validate_failure(
                    kind,
                    error.failure().context(),
                    error.failure().id(),
                    error.failure().reason(),
                    error.failure().content_digest(),
                )
            }
            _ => Err(invalid_result(kind)),
        }
    }

    fn validate_failure(
        &self,
        kind: AssetLoadKind,
        context: RuntimeBundleAssetContext,
        id: &RuntimeBundleAssetResourceId,
        reason: RuntimeBundleAssetFailureReason,
        expected_digest: Option<RuntimeAssetContentDigest>,
    ) -> Result<(), BundleAssetValidationError> {
        let catalog = self.catalog_for_context(context)?;
        let state = asset_content_state(&catalog.bundle, kind, id.as_str())?;
        let reason_matches = match reason {
            RuntimeBundleAssetFailureReason::Missing => matches!(
                state.status,
                AssetContentStatus::MissingResource | AssetContentStatus::MissingBytes
            ),
            RuntimeBundleAssetFailureReason::Decode => state.status == AssetContentStatus::Decode,
            RuntimeBundleAssetFailureReason::MetadataMismatch => {
                state.status == AssetContentStatus::MetadataMismatch
            }
        };
        if !reason_matches {
            return Err(BundleAssetValidationError::ResultReasonMismatch);
        }
        if state.digest != expected_digest {
            return Err(BundleAssetValidationError::ContentDigestMismatch);
        }
        Ok(())
    }

    fn resolve_handle(
        &self,
        kind: AssetLoadKind,
        context: RuntimeBundleAssetContext,
        id: &RuntimeBundleAssetResourceId,
        expected_digest: RuntimeAssetContentDigest,
    ) -> Result<Arc<[u8]>, BundleAssetValidationError> {
        let catalog = self.catalog_for_context(context)?;
        let state = asset_content_state(&catalog.bundle, kind, id.as_str())?;
        match state.status {
            AssetContentStatus::MissingResource => {
                return Err(BundleAssetValidationError::MissingResource);
            }
            AssetContentStatus::MissingBytes => {
                return Err(BundleAssetValidationError::MissingBytes);
            }
            AssetContentStatus::Valid => {}
            AssetContentStatus::Decode | AssetContentStatus::MetadataMismatch => {
                return Err(BundleAssetValidationError::ResultReasonMismatch);
            }
        }
        if state.digest != Some(expected_digest) {
            return Err(BundleAssetValidationError::ContentDigestMismatch);
        }
        let bytes = match kind {
            AssetLoadKind::Image => catalog
                .bundle
                .image_asset_bytes(id.as_str())
                .map_err(|_| BundleAssetValidationError::CatalogLookupFailure)?,
            AssetLoadKind::Voice => catalog
                .bundle
                .audio_asset_bytes(id.as_str())
                .map_err(|_| BundleAssetValidationError::CatalogLookupFailure)?,
        }
        .ok_or(BundleAssetValidationError::MissingBytes)?;
        Ok(Arc::from(bytes))
    }

    fn catalog_for_context(
        &self,
        context: RuntimeBundleAssetContext,
    ) -> Result<BoundBundleCatalog, BundleAssetValidationError> {
        let generations = self
            .generations
            .read()
            .map_err(|_| BundleAssetValidationError::GenerationTablePoisoned)?;
        let catalog = generations
            .get(&context.generation())
            .ok_or(BundleAssetValidationError::GenerationNotRetained)?;
        if catalog.context != context {
            return Err(BundleAssetValidationError::ArtifactIdentityMismatch);
        }
        Ok(catalog.clone())
    }
}

fn domain_failure(
    kind: AssetLoadKind,
    context: RuntimeBundleAssetContext,
    id: RuntimeBundleAssetResourceId,
    reason: RuntimeBundleAssetFailureReason,
    content_digest: Option<RuntimeAssetContentDigest>,
) -> Result<RuntimeValue, BundleAssetResolverError> {
    let failure = RuntimeBundleAssetFailure::try_new(context, id, reason, content_digest)?;
    let error = match kind {
        AssetLoadKind::Image => {
            RuntimeAssetErrorValue::from_failure(failure).into_runtime_value()?
        }
        AssetLoadKind::Voice => {
            RuntimeVoiceErrorValue::from_failure(failure).into_runtime_value()?
        }
    };
    Ok(RuntimeValue::result_err(error))
}

fn asset_content_state(
    bundle: &ArcweftBundle,
    kind: AssetLoadKind,
    id: &str,
) -> Result<AssetContentState, BundleAssetValidationError> {
    let (bytes, status) = match kind {
        AssetLoadKind::Image => {
            let Some(asset) = bundle.image_asset(id) else {
                return Ok(AssetContentState {
                    status: AssetContentStatus::MissingResource,
                    digest: None,
                });
            };
            let bytes = match bundle.image_asset_bytes(id) {
                Ok(Some(bytes)) => bytes,
                Ok(None) | Err(BundleCodecError::MissingImageFile { .. }) => {
                    return Ok(AssetContentState {
                        status: AssetContentStatus::MissingBytes,
                        digest: None,
                    });
                }
                Err(_) => return Err(BundleAssetValidationError::CatalogLookupFailure),
            };
            let status = image_status(asset, bytes);
            (bytes, status)
        }
        AssetLoadKind::Voice => {
            let Some(asset) = bundle
                .audio
                .as_ref()
                .and_then(|graph| graph.assets.iter().find(|asset| asset.id.as_str() == id))
            else {
                return Ok(AssetContentState {
                    status: AssetContentStatus::MissingResource,
                    digest: None,
                });
            };
            let bytes = match bundle.audio_asset_bytes(id) {
                Ok(Some(bytes)) => bytes,
                Ok(None) | Err(BundleCodecError::MissingAudioFile { .. }) => {
                    return Ok(AssetContentState {
                        status: AssetContentStatus::MissingBytes,
                        digest: None,
                    });
                }
                Err(_) => return Err(BundleAssetValidationError::CatalogLookupFailure),
            };
            let status = if asset.validate().is_err() {
                AssetContentStatus::MetadataMismatch
            } else if decode_audio(bytes, asset.format, AudioDecodeLimits::default()).is_err() {
                AssetContentStatus::Decode
            } else {
                AssetContentStatus::Valid
            };
            (bytes, status)
        }
    };
    let digest = RuntimeAssetContentDigest::try_for_bytes(bytes)
        .map_err(|_| BundleAssetValidationError::ContentLengthFailure)?;
    Ok(AssetContentState {
        status,
        digest: Some(digest),
    })
}

fn image_status(asset: &BundleImageAsset, bytes: &[u8]) -> AssetContentStatus {
    let Ok(image) = arcweft_image::decode_image_bytes(
        image_decode_format(asset.format),
        bytes,
        arcweft_image::ImageDecodeOptions::default(),
    ) else {
        return AssetContentStatus::Decode;
    };
    let animation = if image.is_animated() {
        BundleImageAnimation::Animated
    } else {
        BundleImageAnimation::Static
    };
    if asset.animation != animation {
        return AssetContentStatus::MetadataMismatch;
    }
    if let Some(expected) = asset.dimensions {
        let actual = image.dimensions();
        if expected != arcweft_bundle::BundleImageDimensions::new(actual.width(), actual.height()) {
            return AssetContentStatus::MetadataMismatch;
        }
    }
    AssetContentStatus::Valid
}

const fn image_decode_format(format: BundleImageFormat) -> arcweft_image::ImageFormat {
    match format {
        BundleImageFormat::Png => arcweft_image::ImageFormat::Png,
        BundleImageFormat::Jpeg => arcweft_image::ImageFormat::Jpeg,
        BundleImageFormat::Gif => arcweft_image::ImageFormat::Gif,
        BundleImageFormat::WebP => arcweft_image::ImageFormat::WebP,
    }
}

fn invalid_result(kind: AssetLoadKind) -> BundleAssetValidationError {
    match kind {
        AssetLoadKind::Image => BundleAssetValidationError::InvalidImageResult,
        AssetLoadKind::Voice => BundleAssetValidationError::InvalidVoiceResult,
    }
}
