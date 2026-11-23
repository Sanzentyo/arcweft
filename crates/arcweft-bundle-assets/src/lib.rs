//! Portable decoding, identity binding, and typed results for bundled assets.

use arcweft_audio_codec::{AudioDecodeLimits, decode_audio};
use arcweft_bundle::{
    ArcweftBundle, BundleArtifact, BundleCodecError, BundleImageAnimation, BundleImageAsset,
    BundleImageFormat,
};
use arcweft_core::pattern::RuntimeBuiltinVariantCaseIdentity;
use arcweft_core::task::{AssetLoadKind, GenerationId};
use arcweft_core::value::{
    RuntimeAssetContentDigest, RuntimeAssetErrorValue, RuntimeAudioHandleValue,
    RuntimeBorrowedValue, RuntimeBundleAssetBinding, RuntimeBundleAssetContext,
    RuntimeBundleAssetFailure, RuntimeBundleAssetFailureReason, RuntimeBundleAssetOpaqueRole,
    RuntimeBundleAssetResourceId, RuntimeBundleAssetValueError, RuntimeImageHandleValue,
    RuntimeValue, RuntimeVoiceErrorValue, runtime_bundle_asset_opaque_role,
};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Clone, Debug)]
struct BoundBundleCatalog {
    context: RuntimeBundleAssetContext,
    artifact: BundleArtifact,
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
    #[error("asset id is not a canonical Asset identity")]
    InvalidResourceId(#[source] RuntimeBundleAssetValueError),
    #[error("bundle catalog lookup failed: {0}")]
    BundleLookup(#[source] BundleCodecError),
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
enum EncodedAssetStatus {
    Valid,
    Decode,
    MetadataMismatch,
}

impl EncodedAssetStatus {
    const fn failure_reason(self) -> Option<RuntimeBundleAssetFailureReason> {
        match self {
            Self::Valid => None,
            Self::Decode => Some(RuntimeBundleAssetFailureReason::Decode),
            Self::MetadataMismatch => Some(RuntimeBundleAssetFailureReason::MetadataMismatch),
        }
    }
}

enum AssetContentState<'a> {
    MissingResource,
    MissingBytes,
    Encoded {
        bytes: &'a [u8],
        digest: RuntimeAssetContentDigest,
        status: EncodedAssetStatus,
    },
}

enum AssetContentAdmission {
    Load,
    RetainedMissing,
    RetainedDigest(RuntimeAssetContentDigest),
}

impl AssetContentAdmission {
    fn inspect_encoded<'a>(
        self,
        bytes: &'a [u8],
        inspect: impl FnOnce() -> EncodedAssetStatus,
    ) -> Result<AssetContentState<'a>, BundleAssetValidationError> {
        if matches!(self, Self::RetainedMissing) {
            return Err(BundleAssetValidationError::ResultReasonMismatch);
        }
        let digest = RuntimeAssetContentDigest::try_for_bytes(bytes)
            .map_err(|_| BundleAssetValidationError::ContentLengthFailure)?;
        if let Self::RetainedDigest(expected) = self
            && digest != expected
        {
            return Err(BundleAssetValidationError::ContentDigestMismatch);
        }
        Ok(AssetContentState::Encoded {
            bytes,
            digest,
            status: inspect(),
        })
    }
}

impl BundleAssetResolver {
    /// Creates a resolver without catalogs. Bind accepted generations before
    /// submitting asset requests or validating restored results.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a resolver bound to one initial generation and its accepted artifact.
    pub fn try_new(
        context: RuntimeBundleAssetContext,
        artifact: BundleArtifact,
    ) -> Result<Self, BundleAssetResolverError> {
        let resolver = Self::new();
        resolver.bind_generation(context, artifact)?;
        Ok(resolver)
    }

    /// Binds the accepted artifact to the generation and digest in the Core context.
    pub fn bind_generation(
        &self,
        context: RuntimeBundleAssetContext,
        artifact: BundleArtifact,
    ) -> Result<(), BundleAssetResolverError> {
        let generation = context.generation();
        let identity = artifact.artifact_identity();
        if identity.binding_digest().as_bytes() != *context.artifact().as_bytes() {
            return Err(BundleAssetResolverError::ArtifactContextMismatch { generation });
        }
        let mut generations = self
            .generations
            .write()
            .map_err(|_| BundleAssetResolverError::GenerationTablePoisoned)?;
        if let Some(existing) = generations.get(&generation) {
            if existing.context == context && existing.artifact.artifact_identity() == identity {
                return Ok(());
            }
            return Err(BundleAssetResolverError::GenerationAlreadyBound { generation });
        }
        generations.insert(generation, BoundBundleCatalog { context, artifact });
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
    pub fn validate_owned_value<'a>(
        &self,
        value: impl Into<RuntimeBorrowedValue<'a>>,
    ) -> Result<bool, BundleAssetValidationError> {
        let value = value.into();
        let Some(role) = runtime_bundle_asset_opaque_role(value) else {
            return Ok(false);
        };
        self.validate_asset_value(role, value)?;
        Ok(true)
    }

    fn validate_asset_value(
        &self,
        role: RuntimeBundleAssetOpaqueRole,
        value: RuntimeBorrowedValue<'_>,
    ) -> Result<(), BundleAssetValidationError> {
        match role {
            RuntimeBundleAssetOpaqueRole::ImageHandle => {
                let handle = RuntimeImageHandleValue::try_from_runtime_value(value)
                    .map_err(|_| BundleAssetValidationError::InvalidImageResult)?;
                self.validate_binding(AssetLoadKind::Image, handle.binding())
            }
            RuntimeBundleAssetOpaqueRole::AudioHandle => {
                let handle = RuntimeAudioHandleValue::try_from_runtime_value(value)
                    .map_err(|_| BundleAssetValidationError::InvalidVoiceResult)?;
                self.validate_binding(AssetLoadKind::Voice, handle.binding())
            }
            RuntimeBundleAssetOpaqueRole::AssetError => {
                let error = RuntimeAssetErrorValue::try_from_runtime_value(value)
                    .map_err(|_| BundleAssetValidationError::InvalidImageResult)?;
                self.validate_failure(
                    AssetLoadKind::Image,
                    error.failure().context(),
                    error.failure().id(),
                    error.failure().reason(),
                    error.failure().content_digest(),
                )
            }
            RuntimeBundleAssetOpaqueRole::VoiceError => {
                let error = RuntimeVoiceErrorValue::try_from_runtime_value(value)
                    .map_err(|_| BundleAssetValidationError::InvalidVoiceResult)?;
                self.validate_failure(
                    AssetLoadKind::Voice,
                    error.failure().context(),
                    error.failure().id(),
                    error.failure().reason(),
                    error.failure().content_digest(),
                )
            }
        }
    }

    /// Returns validated encoded image bytes for a successfully loaded handle.
    pub fn resolve_image_handle(
        &self,
        handle: &RuntimeImageHandleValue,
    ) -> Result<Arc<[u8]>, BundleAssetValidationError> {
        self.resolve_handle(AssetLoadKind::Image, handle.binding())
    }

    /// Returns validated encoded voice bytes for a successfully loaded handle.
    pub fn resolve_voice_handle(
        &self,
        handle: &RuntimeAudioHandleValue,
    ) -> Result<Arc<[u8]>, BundleAssetValidationError> {
        self.resolve_handle(AssetLoadKind::Voice, handle.binding())
    }

    fn load(
        &self,
        kind: AssetLoadKind,
        context: RuntimeBundleAssetContext,
        id: &str,
    ) -> Result<RuntimeValue, BundleAssetResolverError> {
        let id = RuntimeBundleAssetResourceId::try_new(id)
            .map_err(BundleAssetResolverError::InvalidResourceId)?;
        let catalog = self.catalog_for_context(context)?;
        let digest = match asset_content_state(
            catalog.artifact.bundle(),
            kind,
            id.as_str(),
            AssetContentAdmission::Load,
        )? {
            AssetContentState::MissingResource | AssetContentState::MissingBytes => {
                return domain_failure(
                    kind,
                    context,
                    id,
                    RuntimeBundleAssetFailureReason::Missing,
                    None,
                );
            }
            AssetContentState::Encoded { digest, status, .. } => {
                if let Some(reason) = status.failure_reason() {
                    return domain_failure(kind, context, id, reason, Some(digest));
                }
                digest
            }
        };
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
        use RuntimeBuiltinVariantCaseIdentity::{ResultErr, ResultOk};
        use RuntimeBundleAssetOpaqueRole::{AssetError, AudioHandle, ImageHandle, VoiceError};

        let Some((case, Some(payload))) = RuntimeBorrowedValue::from(value).builtin_variant_case()
        else {
            return Err(invalid_result(kind));
        };
        let role = runtime_bundle_asset_opaque_role(payload).ok_or_else(|| invalid_result(kind))?;
        if !matches!(
            (kind, case, role),
            (AssetLoadKind::Image, ResultOk, ImageHandle)
                | (AssetLoadKind::Image, ResultErr, AssetError)
                | (AssetLoadKind::Voice, ResultOk, AudioHandle)
                | (AssetLoadKind::Voice, ResultErr, VoiceError)
        ) {
            return Err(invalid_result(kind));
        }
        self.validate_asset_value(role, payload)
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
        let admission = expected_digest.map_or(
            AssetContentAdmission::RetainedMissing,
            AssetContentAdmission::RetainedDigest,
        );
        let actual =
            match asset_content_state(catalog.artifact.bundle(), kind, id.as_str(), admission)? {
                AssetContentState::MissingResource | AssetContentState::MissingBytes => {
                    Some(RuntimeBundleAssetFailureReason::Missing)
                }
                AssetContentState::Encoded { status, .. } => status.failure_reason(),
            };
        if actual != Some(reason) {
            return Err(BundleAssetValidationError::ResultReasonMismatch);
        }
        Ok(())
    }

    fn resolve_handle(
        &self,
        kind: AssetLoadKind,
        binding: &RuntimeBundleAssetBinding,
    ) -> Result<Arc<[u8]>, BundleAssetValidationError> {
        let catalog = self.catalog_for_context(binding.context())?;
        Ok(Arc::from(catalog.validated_asset_bytes(kind, binding)?))
    }

    fn validate_binding(
        &self,
        kind: AssetLoadKind,
        binding: &RuntimeBundleAssetBinding,
    ) -> Result<(), BundleAssetValidationError> {
        let catalog = self.catalog_for_context(binding.context())?;
        catalog.validated_asset_bytes(kind, binding).map(|_| ())
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

impl BoundBundleCatalog {
    fn validated_asset_bytes(
        &self,
        kind: AssetLoadKind,
        binding: &RuntimeBundleAssetBinding,
    ) -> Result<&[u8], BundleAssetValidationError> {
        match asset_content_state(
            self.artifact.bundle(),
            kind,
            binding.id().as_str(),
            AssetContentAdmission::RetainedDigest(binding.content_digest()),
        )? {
            AssetContentState::MissingResource => Err(BundleAssetValidationError::MissingResource),
            AssetContentState::MissingBytes => Err(BundleAssetValidationError::MissingBytes),
            AssetContentState::Encoded {
                bytes,
                status: EncodedAssetStatus::Valid,
                ..
            } => Ok(bytes),
            AssetContentState::Encoded { .. } => {
                Err(BundleAssetValidationError::ResultReasonMismatch)
            }
        }
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

fn asset_content_state<'a>(
    bundle: &'a ArcweftBundle,
    kind: AssetLoadKind,
    id: &str,
    admission: AssetContentAdmission,
) -> Result<AssetContentState<'a>, BundleAssetValidationError> {
    match kind {
        AssetLoadKind::Image => {
            let Some(asset) = bundle.image_asset(id) else {
                return Ok(AssetContentState::MissingResource);
            };
            let bytes = match bundle.image_asset_bytes(id) {
                Ok(Some(bytes)) => bytes,
                Ok(None) | Err(BundleCodecError::MissingImageFile { .. }) => {
                    return Ok(AssetContentState::MissingBytes);
                }
                Err(_) => return Err(BundleAssetValidationError::CatalogLookupFailure),
            };
            admission.inspect_encoded(bytes, || image_status(asset, bytes))
        }
        AssetLoadKind::Voice => {
            let Some(asset) = bundle
                .audio
                .as_ref()
                .and_then(|graph| graph.assets.iter().find(|asset| asset.id.as_str() == id))
            else {
                return Ok(AssetContentState::MissingResource);
            };
            let bytes = match bundle.audio_asset_bytes(id) {
                Ok(Some(bytes)) => bytes,
                Ok(None) | Err(BundleCodecError::MissingAudioFile { .. }) => {
                    return Ok(AssetContentState::MissingBytes);
                }
                Err(_) => return Err(BundleAssetValidationError::CatalogLookupFailure),
            };
            admission.inspect_encoded(bytes, || {
                if asset.validate().is_err() {
                    EncodedAssetStatus::MetadataMismatch
                } else if decode_audio(bytes, asset.format, AudioDecodeLimits::default()).is_err() {
                    EncodedAssetStatus::Decode
                } else {
                    EncodedAssetStatus::Valid
                }
            })
        }
    }
}

fn image_status(asset: &BundleImageAsset, bytes: &[u8]) -> EncodedAssetStatus {
    let Ok(image) = arcweft_image::decode_image_bytes(
        image_decode_format(asset.format),
        bytes,
        arcweft_image::ImageDecodeOptions::default(),
    ) else {
        return EncodedAssetStatus::Decode;
    };
    let animation = if image.is_animated() {
        BundleImageAnimation::Animated
    } else {
        BundleImageAnimation::Static
    };
    if asset.animation != animation {
        return EncodedAssetStatus::MetadataMismatch;
    }
    if let Some(expected) = asset.dimensions {
        let actual = image.dimensions();
        if expected != arcweft_bundle::BundleImageDimensions::new(actual.width(), actual.height()) {
            return EncodedAssetStatus::MetadataMismatch;
        }
    }
    EncodedAssetStatus::Valid
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_asset_admits_borrowed_bytes_before_decoder_work() {
        let bytes: &[u8] = b"retained encoded bytes";
        let digest = RuntimeAssetContentDigest::try_for_bytes(bytes).unwrap();
        let wrong = RuntimeAssetContentDigest::try_for_bytes(b"another resource").unwrap();
        assert!(matches!(
            AssetContentAdmission::RetainedDigest(wrong)
                .inspect_encoded(bytes, || panic!("wrong digest reached the decoder")),
            Err(BundleAssetValidationError::ContentDigestMismatch)
        ));
        assert!(matches!(
            AssetContentAdmission::RetainedMissing
                .inspect_encoded(bytes, || panic!("present bytes reached the decoder")),
            Err(BundleAssetValidationError::ResultReasonMismatch)
        ));

        let state = AssetContentAdmission::RetainedDigest(digest)
            .inspect_encoded(bytes, || EncodedAssetStatus::Valid)
            .expect("matching content is inspected");
        let AssetContentState::Encoded {
            bytes: admitted, ..
        } = state
        else {
            panic!("matching bytes must be returned");
        };
        assert!(std::ptr::eq(admitted, bytes));
    }
}
