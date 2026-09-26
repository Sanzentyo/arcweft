//! Typed Core carriers for bundle-owned image and voice resource handles.

use super::{RuntimeOpaqueValueError, RuntimeValue, runtime_sequence_dense_bytes};
use crate::pattern::{RuntimeOpaqueTypeOwner, runtime_standard_opaque_type};
use crate::task::GenerationId;
use arcweft_id::{DeclarationIdentityFamily, PublicId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Shape marker for Core-owned bundle asset handle and domain error payloads.
pub const RUNTIME_BUNDLE_ASSET_VALUE_VERSION: u8 = 1;

/// Tagged digest of the complete bundle artifact that owns one resource.
///
/// The producer hashes the complete `BundleSessionArtifactIdentity`, including
/// its identity variant tag, before constructing this value. Core retains the
/// digest without depending on bundle or runtime-driver crates.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeBundleAssetArtifactDigest([u8; 32]);

/// Digest of the encoded bytes admitted for one bundled asset.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeAssetContentDigest([u8; 32]);

/// Exact generation and complete artifact identity for one bundle-backed task.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct RuntimeBundleAssetContext {
    generation: GenerationId,
    artifact: RuntimeBundleAssetArtifactDigest,
}

/// Checked public identity of an image or voice resource in a bundle.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RuntimeBundleAssetResourceId(PublicId);

/// Complete identity carried by a successful image or audio handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBundleAssetBinding {
    context: RuntimeBundleAssetContext,
    id: RuntimeBundleAssetResourceId,
    content_digest: RuntimeAssetContentDigest,
}

/// Closed domain failure for a bundle-backed image or voice load.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBundleAssetFailureReason {
    Missing,
    Decode,
    MetadataMismatch,
}

/// Complete identity and typed reason carried by a failed asset load.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBundleAssetFailure {
    context: RuntimeBundleAssetContext,
    id: RuntimeBundleAssetResourceId,
    reason: RuntimeBundleAssetFailureReason,
    content_digest: Option<RuntimeAssetContentDigest>,
}

/// Standard `ImageHandle` opaque value constructed by a bundle resource host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeImageHandleValue(RuntimeBundleAssetBinding);

/// Standard `AudioHandle` opaque value constructed by a bundle resource host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAudioHandleValue(RuntimeBundleAssetBinding);

/// Standard `AssetError` opaque value constructed by a bundle resource host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAssetErrorValue(RuntimeBundleAssetFailure);

/// Standard `VoiceError` opaque value constructed by a bundle resource host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeVoiceErrorValue(RuntimeBundleAssetFailure);

/// Role of one exact standard bundle asset opaque owner.
///
/// This identifies the owner independently of payload validity so snapshot
/// validation can fail closed when a standard-owned payload is malformed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBundleAssetOpaqueRole {
    ImageHandle,
    AudioHandle,
    AssetError,
    VoiceError,
}

impl RuntimeBundleAssetOpaqueRole {
    const fn nominal_name(self) -> &'static str {
        match self {
            Self::ImageHandle => "ImageHandle",
            Self::AudioHandle => "AudioHandle",
            Self::AssetError => "AssetError",
            Self::VoiceError => "VoiceError",
        }
    }
}

/// Invalid Core bundle asset identity or opaque payload.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeBundleAssetValueError {
    #[error("bundle artifact identity digest must not be all zero")]
    ZeroArtifactDigest,
    #[error("bundle asset resource id is not a canonical Asset id")]
    InvalidResourceId,
    #[error("bundle asset content length does not fit the digest length field")]
    InvalidContentLength,
    #[error("bundle asset failure evidence does not match its failure reason")]
    InvalidFailureEvidence,
    #[error("runtime opaque payload is not the canonical {role} carrier")]
    InvalidPayload { role: &'static str },
    #[error("runtime opaque value is not owned by the exact standard {role} type")]
    InvalidOwner { role: &'static str },
    #[error(transparent)]
    Opaque(#[from] RuntimeOpaqueValueError),
}

impl RuntimeBundleAssetArtifactDigest {
    /// Constructs a tagged complete-artifact digest, rejecting the reserved zero value.
    pub fn try_from_bytes(bytes: [u8; 32]) -> Result<Self, RuntimeBundleAssetValueError> {
        if bytes == [0; 32] {
            return Err(RuntimeBundleAssetValueError::ZeroArtifactDigest);
        }
        Ok(Self(bytes))
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RuntimeBundleAssetArtifactDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = <[u8; 32]>::deserialize(deserializer)?;
        Self::try_from_bytes(bytes).map_err(serde::de::Error::custom)
    }
}

impl RuntimeAssetContentDigest {
    /// Hashes exact encoded asset bytes under the Core asset-content domain.
    pub fn try_for_bytes(bytes: &[u8]) -> Result<Self, RuntimeBundleAssetValueError> {
        let length = u64::try_from(bytes.len())
            .map_err(|_| RuntimeBundleAssetValueError::InvalidContentLength)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.bundle-asset-content.v1\0");
        hasher.update(&length.to_le_bytes());
        hasher.update(bytes);
        Ok(Self(*hasher.finalize().as_bytes()))
    }

    /// Constructs one exact fixed-width content digest.
    pub fn try_from_bytes(bytes: [u8; 32]) -> Result<Self, RuntimeBundleAssetValueError> {
        Ok(Self(bytes))
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RuntimeAssetContentDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = <[u8; 32]>::deserialize(deserializer)?;
        Self::try_from_bytes(bytes).map_err(serde::de::Error::custom)
    }
}

impl RuntimeBundleAssetContext {
    #[must_use]
    pub const fn new(generation: GenerationId, artifact: RuntimeBundleAssetArtifactDigest) -> Self {
        Self {
            generation,
            artifact,
        }
    }

    #[must_use]
    pub const fn generation(&self) -> GenerationId {
        self.generation
    }

    #[must_use]
    pub const fn artifact(&self) -> RuntimeBundleAssetArtifactDigest {
        self.artifact
    }
}

impl RuntimeBundleAssetResourceId {
    /// Validates an engine-owned id in the Asset declaration family.
    pub fn try_new(value: impl Into<String>) -> Result<Self, RuntimeBundleAssetValueError> {
        let id = PublicId::try_new_engine_owned(value.into())
            .map_err(|_| RuntimeBundleAssetValueError::InvalidResourceId)?;
        DeclarationIdentityFamily::Asset
            .validate_public_id(&id)
            .map_err(|_| RuntimeBundleAssetValueError::InvalidResourceId)?;
        Ok(Self(id))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Serialize for RuntimeBundleAssetResourceId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for RuntimeBundleAssetResourceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl RuntimeBundleAssetBinding {
    pub fn try_new(
        context: RuntimeBundleAssetContext,
        id: RuntimeBundleAssetResourceId,
        content_digest: RuntimeAssetContentDigest,
    ) -> Result<Self, RuntimeBundleAssetValueError> {
        Ok(Self {
            context,
            id,
            content_digest,
        })
    }

    #[must_use]
    pub const fn context(&self) -> RuntimeBundleAssetContext {
        self.context
    }

    #[must_use]
    pub const fn id(&self) -> &RuntimeBundleAssetResourceId {
        &self.id
    }

    #[must_use]
    pub const fn content_digest(&self) -> RuntimeAssetContentDigest {
        self.content_digest
    }
}

impl RuntimeBundleAssetFailure {
    pub fn try_new(
        context: RuntimeBundleAssetContext,
        id: RuntimeBundleAssetResourceId,
        reason: RuntimeBundleAssetFailureReason,
        content_digest: Option<RuntimeAssetContentDigest>,
    ) -> Result<Self, RuntimeBundleAssetValueError> {
        if matches!(reason, RuntimeBundleAssetFailureReason::Missing) != content_digest.is_none() {
            return Err(RuntimeBundleAssetValueError::InvalidFailureEvidence);
        }
        Ok(Self {
            context,
            id,
            reason,
            content_digest,
        })
    }

    #[must_use]
    pub const fn context(&self) -> RuntimeBundleAssetContext {
        self.context
    }

    #[must_use]
    pub const fn id(&self) -> &RuntimeBundleAssetResourceId {
        &self.id
    }

    #[must_use]
    pub const fn reason(&self) -> RuntimeBundleAssetFailureReason {
        self.reason
    }

    #[must_use]
    pub const fn content_digest(&self) -> Option<RuntimeAssetContentDigest> {
        self.content_digest
    }
}

impl RuntimeImageHandleValue {
    #[must_use]
    pub const fn from_binding(binding: RuntimeBundleAssetBinding) -> Self {
        Self(binding)
    }

    #[must_use]
    pub const fn binding(&self) -> &RuntimeBundleAssetBinding {
        &self.0
    }

    pub fn into_runtime_value(self) -> Result<RuntimeValue, RuntimeBundleAssetValueError> {
        wrap_standard_asset_value("ImageHandle", encode_binding(&self.0))
    }

    pub fn try_from_runtime_value(
        value: &RuntimeValue,
    ) -> Result<Self, RuntimeBundleAssetValueError> {
        let payload = exact_standard_asset_payload(value, "ImageHandle")?;
        decode_binding(payload).map(Self)
    }
}

impl RuntimeAudioHandleValue {
    #[must_use]
    pub const fn from_binding(binding: RuntimeBundleAssetBinding) -> Self {
        Self(binding)
    }

    #[must_use]
    pub const fn binding(&self) -> &RuntimeBundleAssetBinding {
        &self.0
    }

    pub fn into_runtime_value(self) -> Result<RuntimeValue, RuntimeBundleAssetValueError> {
        wrap_standard_asset_value("AudioHandle", encode_binding(&self.0))
    }

    pub fn try_from_runtime_value(
        value: &RuntimeValue,
    ) -> Result<Self, RuntimeBundleAssetValueError> {
        let payload = exact_standard_asset_payload(value, "AudioHandle")?;
        decode_binding(payload).map(Self)
    }
}

impl RuntimeAssetErrorValue {
    #[must_use]
    pub const fn from_failure(failure: RuntimeBundleAssetFailure) -> Self {
        Self(failure)
    }

    #[must_use]
    pub const fn failure(&self) -> &RuntimeBundleAssetFailure {
        &self.0
    }

    pub fn into_runtime_value(self) -> Result<RuntimeValue, RuntimeBundleAssetValueError> {
        wrap_standard_asset_value("AssetError", encode_failure(&self.0))
    }

    pub fn try_from_runtime_value(
        value: &RuntimeValue,
    ) -> Result<Self, RuntimeBundleAssetValueError> {
        let payload = exact_standard_asset_payload(value, "AssetError")?;
        decode_failure(payload).map(Self)
    }
}

impl RuntimeVoiceErrorValue {
    #[must_use]
    pub const fn from_failure(failure: RuntimeBundleAssetFailure) -> Self {
        Self(failure)
    }

    #[must_use]
    pub const fn failure(&self) -> &RuntimeBundleAssetFailure {
        &self.0
    }

    pub fn into_runtime_value(self) -> Result<RuntimeValue, RuntimeBundleAssetValueError> {
        wrap_standard_asset_value("VoiceError", encode_failure(&self.0))
    }

    pub fn try_from_runtime_value(
        value: &RuntimeValue,
    ) -> Result<Self, RuntimeBundleAssetValueError> {
        let payload = exact_standard_asset_payload(value, "VoiceError")?;
        decode_failure(payload).map(Self)
    }
}

/// Checks the producer-owned payload shape for standard asset handles/errors.
/// Called from opaque wrapping and admission so deserialized or hand-built
/// opaque values cannot bypass the typed constructors above.
pub(crate) fn standard_asset_opaque_payload_is_valid(
    owner: &RuntimeOpaqueTypeOwner,
    payload: &RuntimeValue,
) -> bool {
    standard_asset_role_for_owner(owner).is_none_or(|role| match role {
        RuntimeBundleAssetOpaqueRole::ImageHandle | RuntimeBundleAssetOpaqueRole::AudioHandle => {
            decode_binding(payload).is_ok()
        }
        RuntimeBundleAssetOpaqueRole::AssetError | RuntimeBundleAssetOpaqueRole::VoiceError => {
            decode_failure(payload).is_ok()
        }
    })
}

/// Identifies an exact standard asset handle/error owner without validating
/// its payload. `None` means the value is not one of these four owners.
#[must_use]
pub fn runtime_bundle_asset_opaque_role(
    value: &RuntimeValue,
) -> Option<RuntimeBundleAssetOpaqueRole> {
    let RuntimeValue::Opaque(value) = value else {
        return None;
    };
    [
        RuntimeBundleAssetOpaqueRole::ImageHandle,
        RuntimeBundleAssetOpaqueRole::AudioHandle,
        RuntimeBundleAssetOpaqueRole::AssetError,
        RuntimeBundleAssetOpaqueRole::VoiceError,
    ]
    .into_iter()
    .find(|role| {
        standard_owner(role.nominal_name()).is_some_and(|owner| {
            value.producer() == owner.producer()
                && value.semantic_identity() == owner.semantic_identity()
                && value.value_class() == owner.value_class()
                && value.persistence() == owner.persistence()
        })
    })
}

fn standard_asset_role_for_owner(
    owner: &RuntimeOpaqueTypeOwner,
) -> Option<RuntimeBundleAssetOpaqueRole> {
    [
        RuntimeBundleAssetOpaqueRole::ImageHandle,
        RuntimeBundleAssetOpaqueRole::AudioHandle,
        RuntimeBundleAssetOpaqueRole::AssetError,
        RuntimeBundleAssetOpaqueRole::VoiceError,
    ]
    .into_iter()
    .find(|role| standard_owner(role.nominal_name()).is_some_and(|standard| standard == *owner))
}

fn encode_binding(binding: &RuntimeBundleAssetBinding) -> RuntimeValue {
    RuntimeValue::Tuple(vec![
        RuntimeValue::u8(RUNTIME_BUNDLE_ASSET_VALUE_VERSION),
        RuntimeValue::u64(binding.context.generation.get()),
        runtime_sequence_dense_bytes(binding.context.artifact.0.to_vec()),
        RuntimeValue::String(binding.id.as_str().to_owned()),
        runtime_sequence_dense_bytes(binding.content_digest.0.to_vec()),
    ])
}

fn decode_binding(
    value: &RuntimeValue,
) -> Result<RuntimeBundleAssetBinding, RuntimeBundleAssetValueError> {
    let RuntimeValue::Tuple(fields) = value else {
        return Err(invalid_payload("asset handle"));
    };
    let [version, generation, artifact, id, content_digest] = fields.as_slice() else {
        return Err(invalid_payload("asset handle"));
    };
    if decode_u8(version)? != RUNTIME_BUNDLE_ASSET_VALUE_VERSION {
        return Err(invalid_payload("asset handle"));
    }
    let generation = GenerationId::new(decode_u64(generation)?);
    let artifact = RuntimeBundleAssetArtifactDigest::try_from_bytes(decode_digest(artifact)?)?;
    let id = RuntimeBundleAssetResourceId::try_new(decode_string(id)?.to_owned())?;
    let content_digest = RuntimeAssetContentDigest::try_from_bytes(decode_digest(content_digest)?)?;
    RuntimeBundleAssetBinding::try_new(
        RuntimeBundleAssetContext::new(generation, artifact),
        id,
        content_digest,
    )
}

fn encode_failure(failure: &RuntimeBundleAssetFailure) -> RuntimeValue {
    RuntimeValue::Tuple(vec![
        RuntimeValue::u8(RUNTIME_BUNDLE_ASSET_VALUE_VERSION),
        RuntimeValue::u64(failure.context.generation.get()),
        runtime_sequence_dense_bytes(failure.context.artifact.0.to_vec()),
        RuntimeValue::String(failure.id.as_str().to_owned()),
        RuntimeValue::u8(failure.reason.encoded()),
        failure
            .content_digest
            .map_or_else(RuntimeValue::option_none, |digest| {
                RuntimeValue::option_some(runtime_sequence_dense_bytes(digest.0.to_vec()))
            }),
    ])
}

fn decode_failure(
    value: &RuntimeValue,
) -> Result<RuntimeBundleAssetFailure, RuntimeBundleAssetValueError> {
    let RuntimeValue::Tuple(fields) = value else {
        return Err(invalid_payload("asset error"));
    };
    let [version, generation, artifact, id, reason, content_digest] = fields.as_slice() else {
        return Err(invalid_payload("asset error"));
    };
    if decode_u8(version)? != RUNTIME_BUNDLE_ASSET_VALUE_VERSION {
        return Err(invalid_payload("asset error"));
    }
    let context = RuntimeBundleAssetContext::new(
        GenerationId::new(decode_u64(generation)?),
        RuntimeBundleAssetArtifactDigest::try_from_bytes(decode_digest(artifact)?)?,
    );
    let id = RuntimeBundleAssetResourceId::try_new(decode_string(id)?.to_owned())?;
    let reason = RuntimeBundleAssetFailureReason::from_encoded(decode_u8(reason)?)
        .ok_or_else(|| invalid_payload("asset error"))?;
    let content_digest = decode_option_digest(content_digest)?;
    RuntimeBundleAssetFailure::try_new(context, id, reason, content_digest)
}

fn decode_option_digest(
    value: &RuntimeValue,
) -> Result<Option<RuntimeAssetContentDigest>, RuntimeBundleAssetValueError> {
    match value.builtin_variant_case() {
        Some((crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionNone, None)) => Ok(None),
        Some((crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value))) => {
            RuntimeAssetContentDigest::try_from_bytes(decode_digest(value)?).map(Some)
        }
        _ => Err(invalid_payload("asset error")),
    }
}

fn wrap_standard_asset_value(
    role: &'static str,
    payload: RuntimeValue,
) -> Result<RuntimeValue, RuntimeBundleAssetValueError> {
    standard_owner(role)
        .ok_or(RuntimeBundleAssetValueError::InvalidOwner { role })?
        .try_wrap(payload)
        .map_err(Into::into)
}

fn exact_standard_asset_payload<'a>(
    value: &'a RuntimeValue,
    role: &'static str,
) -> Result<&'a RuntimeValue, RuntimeBundleAssetValueError> {
    let RuntimeValue::Opaque(opaque) = value else {
        return Err(RuntimeBundleAssetValueError::InvalidOwner { role });
    };
    let owner = standard_owner(role).ok_or(RuntimeBundleAssetValueError::InvalidOwner { role })?;
    owner
        .accepts_opaque_value(opaque)
        .then_some(opaque.payload())
        .ok_or(RuntimeBundleAssetValueError::InvalidOwner { role })
}

fn standard_owner(role: &str) -> Option<RuntimeOpaqueTypeOwner> {
    runtime_standard_opaque_type(&[role])?.monomorphic_owner()
}

fn invalid_payload(role: &'static str) -> RuntimeBundleAssetValueError {
    RuntimeBundleAssetValueError::InvalidPayload { role }
}

fn decode_u8(value: &RuntimeValue) -> Result<u8, RuntimeBundleAssetValueError> {
    match value {
        RuntimeValue::UInt(crate::value::RuntimeUInt::U8(value)) => Ok(*value),
        _ => Err(invalid_payload("bundle asset")),
    }
}

fn decode_u64(value: &RuntimeValue) -> Result<u64, RuntimeBundleAssetValueError> {
    match value {
        RuntimeValue::UInt(crate::value::RuntimeUInt::U64(value)) => Ok(*value),
        _ => Err(invalid_payload("bundle asset")),
    }
}

fn decode_string(value: &RuntimeValue) -> Result<&str, RuntimeBundleAssetValueError> {
    match value {
        RuntimeValue::String(value) => Ok(value),
        _ => Err(invalid_payload("bundle asset")),
    }
}

fn decode_digest(value: &RuntimeValue) -> Result<[u8; 32], RuntimeBundleAssetValueError> {
    let RuntimeValue::Seq(sequence) = value else {
        return Err(invalid_payload("bundle asset digest"));
    };
    if sequence.dense_kind() != Some(super::DenseSeqKind::Bytes) {
        return Err(invalid_payload("bundle asset digest"));
    }
    sequence
        .as_bytes()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .ok_or_else(|| invalid_payload("bundle asset digest"))
}

impl RuntimeBundleAssetFailureReason {
    const fn encoded(self) -> u8 {
        match self {
            Self::Missing => 0,
            Self::Decode => 1,
            Self::MetadataMismatch => 2,
        }
    }

    const fn from_encoded(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Missing,
            1 => Self::Decode,
            2 => Self::MetadataMismatch,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::RuntimeCheckedType;
    use crate::value::{RuntimeOpaquePersistence, RuntimeOpaqueValue, RuntimeOpaqueValueClass};

    fn context() -> RuntimeBundleAssetContext {
        RuntimeBundleAssetContext::new(
            GenerationId::new(7),
            RuntimeBundleAssetArtifactDigest::try_from_bytes([0x71; 32])
                .expect("tagged artifact digest"),
        )
    }

    fn binding() -> RuntimeBundleAssetBinding {
        RuntimeBundleAssetBinding::try_new(
            context(),
            RuntimeBundleAssetResourceId::try_new("asset.bg.room").expect("asset resource id"),
            RuntimeAssetContentDigest::try_for_bytes(b"decoded source bytes")
                .expect("content digest"),
        )
        .expect("bundle asset binding")
    }

    #[test]
    fn image_and_audio_handles_round_trip_full_bundle_identity() {
        for handle in [
            RuntimeImageHandleValue::from_binding(binding())
                .into_runtime_value()
                .expect("typed image handle"),
            RuntimeAudioHandleValue::from_binding(binding())
                .into_runtime_value()
                .expect("typed audio handle"),
        ] {
            let image = RuntimeImageHandleValue::try_from_runtime_value(&handle);
            let audio = RuntimeAudioHandleValue::try_from_runtime_value(&handle);
            assert!(image.is_ok() ^ audio.is_ok());
            let decoded = image
                .map(|value| value.binding().clone())
                .or_else(|_| audio.map(|value| value.binding().clone()))
                .expect("one exact handle owner decodes");
            assert_eq!(decoded, binding());
            assert_eq!(decoded.context(), context());
            assert_eq!(decoded.id().as_str(), "asset.bg.room");
        }
    }

    #[test]
    fn standard_asset_owner_detection_recognizes_malformed_payloads_only_for_exact_owner() {
        let image_owner = standard_owner("ImageHandle").expect("standard image owner");
        let malformed = RuntimeValue::Opaque(RuntimeOpaqueValue::new_exact(
            &image_owner,
            RuntimeValue::Unit,
        ));
        assert_eq!(
            runtime_bundle_asset_opaque_role(&malformed),
            Some(RuntimeBundleAssetOpaqueRole::ImageHandle)
        );
        assert!(RuntimeImageHandleValue::try_from_runtime_value(&malformed).is_err());

        let unrelated_owner = RuntimeOpaqueTypeOwner::exact_with(
            image_owner.producer().clone(),
            crate::pattern::RuntimeSemanticTypeId::from_bytes([0xA5; 32]),
            RuntimeOpaqueValueClass::Plain,
            RuntimeOpaquePersistence::SnapshotOnly,
        );
        let unrelated = RuntimeValue::Opaque(RuntimeOpaqueValue::new_exact(
            &unrelated_owner,
            RuntimeValue::Unit,
        ));
        assert_eq!(runtime_bundle_asset_opaque_role(&unrelated), None);
    }

    #[test]
    fn missing_and_decode_failures_are_distinct_typed_domain_values() {
        let id = RuntimeBundleAssetResourceId::try_new("asset.voice.opening")
            .expect("voice resource id");
        let missing = RuntimeBundleAssetFailure::try_new(
            context(),
            id.clone(),
            RuntimeBundleAssetFailureReason::Missing,
            None,
        )
        .expect("missing asset evidence");
        let decode = RuntimeBundleAssetFailure::try_new(
            context(),
            id,
            RuntimeBundleAssetFailureReason::Decode,
            Some(
                RuntimeAssetContentDigest::try_for_bytes(b"bad audio bytes")
                    .expect("content digest"),
            ),
        )
        .expect("decode failure evidence");
        let missing_value = RuntimeAssetErrorValue::from_failure(missing)
            .into_runtime_value()
            .expect("AssetError carrier");
        let decode_value = RuntimeVoiceErrorValue::from_failure(decode)
            .into_runtime_value()
            .expect("VoiceError carrier");
        assert_eq!(
            RuntimeAssetErrorValue::try_from_runtime_value(&missing_value)
                .expect("missing error")
                .failure()
                .reason(),
            RuntimeBundleAssetFailureReason::Missing
        );
        assert_eq!(
            RuntimeVoiceErrorValue::try_from_runtime_value(&decode_value)
                .expect("decode error")
                .failure()
                .reason(),
            RuntimeBundleAssetFailureReason::Decode
        );
    }

    #[test]
    fn standard_asset_opaque_owners_reject_untyped_payloads_and_are_snapshot_only() {
        for role in ["ImageHandle", "AudioHandle", "AssetError", "VoiceError"] {
            let owner = standard_owner(role).expect("standard asset opaque owner");
            assert_eq!(owner.value_class(), RuntimeOpaqueValueClass::Plain);
            assert_eq!(owner.persistence(), RuntimeOpaquePersistence::SnapshotOnly);
            assert!(
                owner
                    .try_wrap(RuntimeValue::String("forged".to_owned()))
                    .is_err()
            );
            let invalid = RuntimeOpaqueValue::new_exact(&owner, RuntimeValue::Unit);
            assert!(
                !(RuntimeCheckedType::Opaque { owner })
                    .accepts_value(&RuntimeValue::Opaque(invalid))
            );
        }
    }

    #[test]
    fn resource_id_and_failure_evidence_are_checked() {
        assert!(RuntimeBundleAssetResourceId::try_new("voice.opening").is_err());
        assert!(
            RuntimeBundleAssetFailure::try_new(
                context(),
                RuntimeBundleAssetResourceId::try_new("asset.voice.opening").expect("asset id"),
                RuntimeBundleAssetFailureReason::Missing,
                Some(
                    RuntimeAssetContentDigest::try_for_bytes(b"no bytes").expect("content digest"),
                ),
            )
            .is_err()
        );
    }
}
