//! Sans I/O AWFR archive and external payload carrier contracts.
//!
//! This module deliberately owns only deterministic metadata, validation, and
//! byte/digest checks. Filesystem, network, cache residency, publication clocks,
//! and key access stay in project-loader/CLI/player adapters.

use super::{ReleaseBundleRef, ReleaseManifest, ReleaseManifestError, ReleaseMirror};
use crate::container::{
    ArtifactIdentity, BundleDigest, BundleKind, Compression, ContentPlacement, ContentResidency,
    SectionDescriptor, SectionId, SectionKindCode,
};
use std::collections::BTreeSet;
use thiserror::Error;

pub const AWFR_ARCHIVE_SCHEMA_VERSION: u32 = 1;
pub const EXTERNAL_PAYLOAD_CARRIER_SCHEMA_VERSION: u32 = 1;
pub const EXTERNAL_PAYLOAD_CACHE_KEY_EPOCH: u32 = 1;

/// Release channel bound into AWFR archive identity and signing transcripts.
#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(transparent)]
pub struct ReleaseChannel(String);

/// Media type attached to an external payload descriptor.
#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(transparent)]
pub struct ExternalPayloadMediaType(String);

/// Stable lookup key for a payload descriptor inside one bundle content root.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct ExternalPayloadDescriptorKey {
    pub bundle_content_root: BundleDigest,
    pub descriptor_id: SectionId,
}

/// Cache-key fields for one external payload byte object.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct ExternalPayloadCacheKey {
    pub epoch: u32,
    pub bundle_content_root: BundleDigest,
    pub descriptor_id: SectionId,
    pub compressed_digest: BundleDigest,
}

/// Product-grade descriptor for an external payload carrier referenced by AWFR.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ExternalPayloadCarrier {
    pub schema_version: u32,
    pub descriptor_id: SectionId,
    pub bundle_content_root: BundleDigest,
    pub bundle_kind: BundleKind,
    pub section_kind: SectionKindCode,
    pub section_schema_version: u32,
    pub residency: ContentResidency,
    pub required: bool,
    #[serde(
        default,
        skip_serializing_if = "ExternalPayloadMediaType::is_octet_stream"
    )]
    pub media_type: ExternalPayloadMediaType,
    pub compression: Compression,
    pub decoded_size: u64,
    pub compressed_size: u64,
    pub decoded_digest: BundleDigest,
    pub compressed_digest: BundleDigest,
    pub cache_key: ExternalPayloadCacheKey,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<ReleaseMirror>,
}

/// Payload residency policy used by materializers and offline inspectors.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalPayloadMaterializationMode {
    /// Only descriptor metadata is required. Payload bytes are not read.
    MetadataOnly,
    /// Startup or required sections need payload bytes before entry.
    RequiredResidency,
    /// Every external payload must be present and validated.
    AllPayloads,
}

/// Publication metadata that is stable once emitted by a release adapter.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AwfrPublicationMetadata {
    pub release_name: String,
    pub sequence: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at_epoch_millis: Option<u64>,
}

/// Patch artifact reference carried by an AWFR archive.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AwfrPatchArtifactRef {
    pub patch_artifact: ArtifactIdentity,
    pub target_artifact: ArtifactIdentity,
    pub file_digest: BundleDigest,
    pub byte_len: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<ReleaseMirror>,
}

/// Detached archive-level signature metadata.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AwfrArchiveSignatureRef {
    pub signer_id: String,
    pub algorithm: String,
    pub key_epoch: u64,
    pub signing_digest: BundleDigest,
    pub signature: String,
}

/// AWFR archive manifest binding release bundles, patches, external payloads,
/// publication metadata, channel, and archive-level signatures.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AwfrArchiveManifest {
    pub schema_version: u32,
    pub channel: ReleaseChannel,
    pub release_manifest: ReleaseManifest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<AwfrPublicationMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patches: Vec<AwfrPatchArtifactRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_payloads: Vec<ExternalPayloadCarrier>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<AwfrArchiveSignatureRef>,
}

/// External-payload carrier mutation emitted by patch materialization or publish
/// adapters when section placement changes add, replace, or remove payloads.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "operation")]
pub enum ExternalPayloadCarrierOperation {
    Add {
        carrier: ExternalPayloadCarrier,
    },
    Replace {
        bundle_content_root: BundleDigest,
        descriptor_id: SectionId,
        old_decoded_digest: BundleDigest,
        next: ExternalPayloadCarrier,
    },
    Remove {
        bundle_content_root: BundleDigest,
        descriptor_id: SectionId,
        old_decoded_digest: BundleDigest,
    },
}

/// Deterministic AWFR rewrite plan. Sans I/O code can validate the metadata; an
/// adapter owns fetching/publishing bytes and committing or rolling back files.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ReleaseManifestRewritePlan {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_bundle: Option<ReleaseBundleRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_payload_operations: Vec<ExternalPayloadCarrierOperation>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum AwfrArchiveError {
    #[error("unsupported AWFR archive schema version {actual}; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error("invalid release channel: {0}")]
    InvalidChannel(String),
    #[error("invalid external payload media type: {0}")]
    InvalidMediaType(String),
    #[error(transparent)]
    ReleaseManifest(#[from] ReleaseManifestError),
    #[error("external payload carrier {0:?} duplicates an earlier carrier")]
    DuplicatePayloadCarrier(ExternalPayloadDescriptorKey),
    #[error("external payload carrier references missing release bundle {0}")]
    MissingReleaseBundle(BundleDigest),
    #[error(
        "external payload carrier {bundle_content_root}:{descriptor_id} bundle kind mismatch: expected {expected:?}, actual {actual:?}"
    )]
    BundleKindMismatch {
        bundle_content_root: BundleDigest,
        descriptor_id: SectionId,
        expected: BundleKind,
        actual: BundleKind,
    },
    #[error("external payload carrier {0:?} has no mirrors")]
    MissingPayloadMirrors(ExternalPayloadDescriptorKey),
    #[error("external payload carrier {0:?} cache key does not match descriptor fields")]
    CacheKeyMismatch(ExternalPayloadDescriptorKey),
    #[error(
        "external payload carrier {0:?} cannot use different compressed metadata when compression is none"
    )]
    CompressionSizeDigestMismatch(ExternalPayloadDescriptorKey),
    #[error(
        "external payload carrier {key:?} byte length mismatch: expected {expected}, actual {actual}"
    )]
    ByteLengthMismatch {
        key: ExternalPayloadDescriptorKey,
        expected: u64,
        actual: u64,
    },
    #[error(
        "external payload carrier {key:?} digest mismatch: expected {expected}, actual {actual}"
    )]
    DigestMismatch {
        key: ExternalPayloadDescriptorKey,
        expected: BundleDigest,
        actual: BundleDigest,
    },
    #[error("failed to decode compressed external payload {key:?}: {message}")]
    DecodeCompressedPayload {
        key: ExternalPayloadDescriptorKey,
        message: String,
    },
    #[error("section {0} is embedded; only external descriptors may create payload carriers")]
    EmbeddedDescriptor(SectionId),
    #[error("external payload carrier {0:?} is missing")]
    MissingPayloadCarrier(ExternalPayloadDescriptorKey),
    #[error("external payload carrier {0:?} already exists")]
    PayloadCarrierAlreadyExists(ExternalPayloadDescriptorKey),
    #[error(
        "external payload carrier {key:?} old digest mismatch: expected {expected}, actual {actual}"
    )]
    OldDigestMismatch {
        key: ExternalPayloadDescriptorKey,
        expected: BundleDigest,
        actual: BundleDigest,
    },
    #[error("invalid AWFR publication metadata: {0}")]
    InvalidPublication(String),
    #[error("invalid AWFR patch reference: {0}")]
    InvalidPatchRef(String),
    #[error("invalid AWFR archive signature reference: {0}")]
    InvalidSignatureRef(String),
    #[error("failed to encode AWFR archive JSON: {0}")]
    EncodeJson(String),
    #[error("failed to decode AWFR archive JSON: {0}")]
    DecodeJson(String),
}

impl Default for ReleaseChannel {
    fn default() -> Self {
        Self::local_dev()
    }
}

impl ReleaseChannel {
    pub fn new(value: impl Into<String>) -> Result<Self, AwfrArchiveError> {
        let channel = Self(value.into());
        channel.validate()?;
        Ok(channel)
    }

    pub fn local_dev() -> Self {
        Self("local-dev".to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn validate(&self) -> Result<(), AwfrArchiveError> {
        validate_token("release channel", self.as_str(), 128)
            .map_err(AwfrArchiveError::InvalidChannel)
    }
}

impl std::fmt::Display for ReleaseChannel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Default for ExternalPayloadMediaType {
    fn default() -> Self {
        Self("application/octet-stream".to_owned())
    }
}

impl ExternalPayloadMediaType {
    pub fn new(value: impl Into<String>) -> Result<Self, AwfrArchiveError> {
        let media_type = Self(value.into());
        media_type.validate()?;
        Ok(media_type)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_octet_stream(&self) -> bool {
        self.as_str() == "application/octet-stream"
    }

    pub fn validate(&self) -> Result<(), AwfrArchiveError> {
        let value = self.as_str();
        if value.is_empty() {
            return Err(AwfrArchiveError::InvalidMediaType(
                "media type must not be empty".to_owned(),
            ));
        }
        if value.len() > 128 {
            return Err(AwfrArchiveError::InvalidMediaType(
                "media type must not exceed 128 bytes".to_owned(),
            ));
        }
        if value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || !ch.is_ascii())
        {
            return Err(AwfrArchiveError::InvalidMediaType(
                "media type must be printable ASCII without whitespace".to_owned(),
            ));
        }
        if !value.contains('/') {
            return Err(AwfrArchiveError::InvalidMediaType(
                "media type must contain a type/subtype slash".to_owned(),
            ));
        }
        Ok(())
    }
}

impl ExternalPayloadDescriptorKey {
    pub const fn new(bundle_content_root: BundleDigest, descriptor_id: SectionId) -> Self {
        Self {
            bundle_content_root,
            descriptor_id,
        }
    }
}

impl ExternalPayloadCacheKey {
    pub const fn new(
        bundle_content_root: BundleDigest,
        descriptor_id: SectionId,
        compressed_digest: BundleDigest,
    ) -> Self {
        Self {
            epoch: EXTERNAL_PAYLOAD_CACHE_KEY_EPOCH,
            bundle_content_root,
            descriptor_id,
            compressed_digest,
        }
    }

    pub fn for_carrier(carrier: &ExternalPayloadCarrier) -> Self {
        Self::new(
            carrier.bundle_content_root,
            carrier.descriptor_id,
            carrier.compressed_digest,
        )
    }

    pub fn descriptor_key(self) -> ExternalPayloadDescriptorKey {
        ExternalPayloadDescriptorKey::new(self.bundle_content_root, self.descriptor_id)
    }

    pub fn digest(self) -> BundleDigest {
        let mut transcript = Vec::with_capacity(96);
        transcript.extend_from_slice(b"arcweft.external-payload-cache-key.v1\0");
        transcript.extend_from_slice(&self.epoch.to_le_bytes());
        transcript.extend_from_slice(&self.bundle_content_root.as_bytes());
        transcript.extend_from_slice(&self.descriptor_id.as_bytes());
        transcript.extend_from_slice(&self.compressed_digest.as_bytes());
        BundleDigest::of(&transcript)
    }

    pub fn logical_item(self) -> String {
        format!(
            "external-payload:v{}:{}:{}:{}",
            self.epoch, self.bundle_content_root, self.descriptor_id, self.compressed_digest
        )
    }
}

impl std::fmt::Display for ExternalPayloadCacheKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.logical_item())
    }
}

impl ExternalPayloadCarrier {
    pub fn from_descriptor(
        descriptor: &SectionDescriptor,
        bundle_artifact: ArtifactIdentity,
        media_type: ExternalPayloadMediaType,
        compressed_size: u64,
        compressed_digest: BundleDigest,
        mirrors: impl IntoIterator<Item = ReleaseMirror>,
    ) -> Result<Self, AwfrArchiveError> {
        if descriptor.placement() != ContentPlacement::External {
            return Err(AwfrArchiveError::EmbeddedDescriptor(descriptor.id()));
        }
        let carrier = Self {
            schema_version: EXTERNAL_PAYLOAD_CARRIER_SCHEMA_VERSION,
            descriptor_id: descriptor.id(),
            bundle_content_root: bundle_artifact.content_root,
            bundle_kind: bundle_artifact.kind,
            section_kind: descriptor.kind_code(),
            section_schema_version: descriptor.schema_version(),
            residency: descriptor.residency(),
            required: descriptor.required(),
            media_type,
            compression: descriptor.compression(),
            decoded_size: descriptor.decoded_size(),
            compressed_size,
            decoded_digest: descriptor.content_digest(),
            compressed_digest,
            cache_key: ExternalPayloadCacheKey::new(
                bundle_artifact.content_root,
                descriptor.id(),
                compressed_digest,
            ),
            mirrors: mirrors.into_iter().collect(),
        };
        carrier.validate()?;
        Ok(carrier)
    }

    pub fn descriptor_key(&self) -> ExternalPayloadDescriptorKey {
        ExternalPayloadDescriptorKey::new(self.bundle_content_root, self.descriptor_id)
    }

    pub fn validate(&self) -> Result<(), AwfrArchiveError> {
        if self.schema_version != EXTERNAL_PAYLOAD_CARRIER_SCHEMA_VERSION {
            return Err(AwfrArchiveError::UnsupportedSchema {
                actual: self.schema_version,
                expected: EXTERNAL_PAYLOAD_CARRIER_SCHEMA_VERSION,
            });
        }
        self.media_type.validate()?;
        if self.mirrors.is_empty() {
            return Err(AwfrArchiveError::MissingPayloadMirrors(
                self.descriptor_key(),
            ));
        }
        for mirror in &self.mirrors {
            mirror.validate()?;
        }
        if self.cache_key != ExternalPayloadCacheKey::for_carrier(self) {
            return Err(AwfrArchiveError::CacheKeyMismatch(self.descriptor_key()));
        }
        if self.compression == Compression::None
            && (self.compressed_size != self.decoded_size
                || self.compressed_digest != self.decoded_digest)
        {
            return Err(AwfrArchiveError::CompressionSizeDigestMismatch(
                self.descriptor_key(),
            ));
        }
        Ok(())
    }

    pub fn verify_stored_bytes(&self, bytes: &[u8]) -> Result<Vec<u8>, AwfrArchiveError> {
        self.validate()?;
        let actual_size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if actual_size != self.compressed_size {
            return Err(AwfrArchiveError::ByteLengthMismatch {
                key: self.descriptor_key(),
                expected: self.compressed_size,
                actual: actual_size,
            });
        }
        let actual_digest = BundleDigest::of(bytes);
        if actual_digest != self.compressed_digest {
            return Err(AwfrArchiveError::DigestMismatch {
                key: self.descriptor_key(),
                expected: self.compressed_digest,
                actual: actual_digest,
            });
        }
        let decoded = match self.compression {
            Compression::None => bytes.to_vec(),
            Compression::Zstd => zstd::bulk::decompress(
                bytes,
                usize::try_from(self.decoded_size).unwrap_or(usize::MAX),
            )
            .map_err(|error| AwfrArchiveError::DecodeCompressedPayload {
                key: self.descriptor_key(),
                message: error.to_string(),
            })?,
        };
        self.verify_decoded_bytes(&decoded)?;
        Ok(decoded)
    }

    pub fn verify_decoded_bytes(&self, bytes: &[u8]) -> Result<(), AwfrArchiveError> {
        let actual_size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if actual_size != self.decoded_size {
            return Err(AwfrArchiveError::ByteLengthMismatch {
                key: self.descriptor_key(),
                expected: self.decoded_size,
                actual: actual_size,
            });
        }
        let actual_digest = BundleDigest::of(bytes);
        if actual_digest != self.decoded_digest {
            return Err(AwfrArchiveError::DigestMismatch {
                key: self.descriptor_key(),
                expected: self.decoded_digest,
                actual: actual_digest,
            });
        }
        Ok(())
    }

    pub const fn requires_payload_bytes(&self, mode: ExternalPayloadMaterializationMode) -> bool {
        match mode {
            ExternalPayloadMaterializationMode::MetadataOnly => false,
            ExternalPayloadMaterializationMode::RequiredResidency => {
                self.required || self.residency.must_be_ready_before_entry()
            }
            ExternalPayloadMaterializationMode::AllPayloads => true,
        }
    }
}

impl AwfrPublicationMetadata {
    pub fn validate(&self) -> Result<(), AwfrArchiveError> {
        validate_token("release name", &self.release_name, 128)
            .map_err(AwfrArchiveError::InvalidPublication)
    }
}

impl AwfrPatchArtifactRef {
    pub fn validate(&self) -> Result<(), AwfrArchiveError> {
        if self.byte_len == 0 {
            return Err(AwfrArchiveError::InvalidPatchRef(
                "patch byte_len must be greater than zero".to_owned(),
            ));
        }
        if self.mirrors.is_empty() {
            return Err(AwfrArchiveError::InvalidPatchRef(
                "patch reference must have at least one mirror".to_owned(),
            ));
        }
        for mirror in &self.mirrors {
            mirror.validate()?;
        }
        Ok(())
    }
}

impl AwfrArchiveSignatureRef {
    pub fn validate(&self) -> Result<(), AwfrArchiveError> {
        validate_token("signer id", &self.signer_id, 128)
            .map_err(AwfrArchiveError::InvalidSignatureRef)?;
        if self.algorithm.is_empty() {
            return Err(AwfrArchiveError::InvalidSignatureRef(
                "algorithm must not be empty".to_owned(),
            ));
        }
        if self.signature.is_empty() {
            return Err(AwfrArchiveError::InvalidSignatureRef(
                "signature must not be empty".to_owned(),
            ));
        }
        Ok(())
    }
}

impl AwfrArchiveManifest {
    pub fn new(
        channel: ReleaseChannel,
        release_manifest: ReleaseManifest,
        external_payloads: impl IntoIterator<Item = ExternalPayloadCarrier>,
    ) -> Result<Self, AwfrArchiveError> {
        let archive = Self {
            schema_version: AWFR_ARCHIVE_SCHEMA_VERSION,
            channel,
            release_manifest,
            publication: None,
            patches: Vec::new(),
            external_payloads: external_payloads.into_iter().collect(),
            signatures: Vec::new(),
        };
        archive.validate()?;
        Ok(archive)
    }

    pub fn validate(&self) -> Result<(), AwfrArchiveError> {
        if self.schema_version != AWFR_ARCHIVE_SCHEMA_VERSION {
            return Err(AwfrArchiveError::UnsupportedSchema {
                actual: self.schema_version,
                expected: AWFR_ARCHIVE_SCHEMA_VERSION,
            });
        }
        self.channel.validate()?;
        self.release_manifest.validate()?;
        if let Some(publication) = &self.publication {
            publication.validate()?;
        }
        for patch in &self.patches {
            patch.validate()?;
        }
        for signature in &self.signatures {
            signature.validate()?;
        }
        let mut seen = BTreeSet::new();
        for carrier in &self.external_payloads {
            carrier.validate()?;
            let key = carrier.descriptor_key();
            if !seen.insert(key) {
                return Err(AwfrArchiveError::DuplicatePayloadCarrier(key));
            }
            let Some(bundle) = self.release_manifest.bundle(carrier.bundle_content_root) else {
                return Err(AwfrArchiveError::MissingReleaseBundle(
                    carrier.bundle_content_root,
                ));
            };
            if bundle.kind != carrier.bundle_kind {
                return Err(AwfrArchiveError::BundleKindMismatch {
                    bundle_content_root: carrier.bundle_content_root,
                    descriptor_id: carrier.descriptor_id,
                    expected: bundle.kind,
                    actual: carrier.bundle_kind,
                });
            }
        }
        Ok(())
    }

    pub fn canonicalize(&mut self) {
        self.release_manifest
            .bundles
            .sort_by_key(|bundle| bundle.content_root);
        for bundle in &mut self.release_manifest.bundles {
            bundle.mirrors.sort_by(|left, right| {
                left.priority
                    .cmp(&right.priority)
                    .then_with(|| left.uri.cmp(&right.uri))
            });
        }
        self.patches
            .sort_by_key(|patch| patch.patch_artifact.digest());
        for patch in &mut self.patches {
            patch.mirrors.sort_by(|left, right| {
                left.priority
                    .cmp(&right.priority)
                    .then_with(|| left.uri.cmp(&right.uri))
            });
        }
        self.external_payloads
            .sort_by_key(ExternalPayloadCarrier::descriptor_key);
        for carrier in &mut self.external_payloads {
            carrier.mirrors.sort_by(|left, right| {
                left.priority
                    .cmp(&right.priority)
                    .then_with(|| left.uri.cmp(&right.uri))
            });
        }
        self.signatures.sort_by(|left, right| {
            left.signer_id
                .cmp(&right.signer_id)
                .then_with(|| left.key_epoch.cmp(&right.key_epoch))
        });
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, AwfrArchiveError> {
        let mut archive = self.clone();
        archive.canonicalize();
        archive.validate()?;
        serde_json::to_vec_pretty(&archive)
            .map_err(|error| AwfrArchiveError::EncodeJson(error.to_string()))
    }

    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, AwfrArchiveError> {
        let mut archive = serde_json::from_slice::<Self>(bytes)
            .map_err(|error| AwfrArchiveError::DecodeJson(error.to_string()))?;
        archive.canonicalize();
        archive.validate()?;
        Ok(archive)
    }

    pub fn external_payload(
        &self,
        key: ExternalPayloadDescriptorKey,
    ) -> Option<&ExternalPayloadCarrier> {
        self.external_payloads
            .iter()
            .find(|carrier| carrier.descriptor_key() == key)
    }

    pub fn payloads_for_bundle(
        &self,
        bundle_content_root: BundleDigest,
    ) -> impl Iterator<Item = &ExternalPayloadCarrier> {
        self.external_payloads
            .iter()
            .filter(move |carrier| carrier.bundle_content_root == bundle_content_root)
    }

    pub fn unsigned_identity_digest(&self) -> Result<BundleDigest, AwfrArchiveError> {
        let mut archive = self.clone();
        archive.signatures.clear();
        let bytes = archive.to_json_bytes()?;
        let mut transcript = Vec::with_capacity(bytes.len() + 64);
        transcript.extend_from_slice(b"arcweft.awfr-archive-identity.v1\0");
        transcript.extend_from_slice(&BundleDigest::of(&bytes).as_bytes());
        Ok(BundleDigest::of(&transcript))
    }

    pub fn external_payloads_digest(&self) -> BundleDigest {
        let mut carriers = self.external_payloads.clone();
        carriers.sort_by_key(ExternalPayloadCarrier::descriptor_key);
        let mut transcript = Vec::new();
        transcript.extend_from_slice(b"arcweft.awfr-external-payloads.v1\0");
        for carrier in carriers {
            transcript.extend_from_slice(&carrier.bundle_content_root.as_bytes());
            transcript.extend_from_slice(&carrier.descriptor_id.as_bytes());
            transcript.extend_from_slice(&carrier.section_kind.encoded().to_le_bytes());
            transcript.extend_from_slice(&carrier.section_schema_version.to_le_bytes());
            transcript.push(carrier.residency.encoded());
            transcript.push(u8::from(carrier.required));
            transcript.push(carrier.compression.encoded());
            put_len_prefixed_bytes(&mut transcript, carrier.media_type.as_str().as_bytes());
            transcript.extend_from_slice(&carrier.decoded_size.to_le_bytes());
            transcript.extend_from_slice(&carrier.compressed_size.to_le_bytes());
            transcript.extend_from_slice(&carrier.decoded_digest.as_bytes());
            transcript.extend_from_slice(&carrier.compressed_digest.as_bytes());
            transcript.extend_from_slice(&carrier.cache_key.digest().as_bytes());
        }
        BundleDigest::of(&transcript)
    }
}

impl ReleaseManifestRewritePlan {
    pub fn apply_to(
        &self,
        archive: &AwfrArchiveManifest,
    ) -> Result<AwfrArchiveManifest, AwfrArchiveError> {
        let mut next = archive.clone();
        if let Some(target_bundle) = &self.target_bundle {
            replace_or_insert_bundle(&mut next.release_manifest.bundles, target_bundle.clone());
        }
        for operation in &self.external_payload_operations {
            match operation {
                ExternalPayloadCarrierOperation::Add { carrier } => {
                    let key = carrier.descriptor_key();
                    if next.external_payload(key).is_some() {
                        return Err(AwfrArchiveError::PayloadCarrierAlreadyExists(key));
                    }
                    next.external_payloads.push(carrier.clone());
                }
                ExternalPayloadCarrierOperation::Replace {
                    bundle_content_root,
                    descriptor_id,
                    old_decoded_digest,
                    next: replacement,
                } => {
                    let key =
                        ExternalPayloadDescriptorKey::new(*bundle_content_root, *descriptor_id);
                    let Some(index) = next
                        .external_payloads
                        .iter()
                        .position(|carrier| carrier.descriptor_key() == key)
                    else {
                        return Err(AwfrArchiveError::MissingPayloadCarrier(key));
                    };
                    let actual = next.external_payloads[index].decoded_digest;
                    if actual != *old_decoded_digest {
                        return Err(AwfrArchiveError::OldDigestMismatch {
                            key,
                            expected: *old_decoded_digest,
                            actual,
                        });
                    }
                    next.external_payloads[index] = replacement.clone();
                }
                ExternalPayloadCarrierOperation::Remove {
                    bundle_content_root,
                    descriptor_id,
                    old_decoded_digest,
                } => {
                    let key =
                        ExternalPayloadDescriptorKey::new(*bundle_content_root, *descriptor_id);
                    let Some(index) = next
                        .external_payloads
                        .iter()
                        .position(|carrier| carrier.descriptor_key() == key)
                    else {
                        return Err(AwfrArchiveError::MissingPayloadCarrier(key));
                    };
                    let actual = next.external_payloads[index].decoded_digest;
                    if actual != *old_decoded_digest {
                        return Err(AwfrArchiveError::OldDigestMismatch {
                            key,
                            expected: *old_decoded_digest,
                            actual,
                        });
                    }
                    next.external_payloads.remove(index);
                }
            }
        }
        next.canonicalize();
        next.validate()?;
        Ok(next)
    }
}

fn replace_or_insert_bundle(bundles: &mut Vec<ReleaseBundleRef>, replacement: ReleaseBundleRef) {
    if let Some(index) = bundles
        .iter()
        .position(|bundle| bundle.content_root == replacement.content_root)
    {
        bundles[index] = replacement;
    } else {
        bundles.push(replacement);
    }
}

fn validate_token(label: &str, value: &str, max_len: usize) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.len() > max_len {
        return Err(format!("{label} must not exceed {max_len} bytes"));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ':' | '/'))
    {
        return Err(format!(
            "{label} must contain only ASCII alphanumeric, '.', '_', '-', ':', or '/' characters"
        ));
    }
    Ok(())
}

fn put_len_prefixed_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::{
        BundleSectionKind, BundleView, ReadBudget, SectionInput, encode_bundle,
    };

    fn external_content_pack(payload: &'static [u8]) -> (Vec<u8>, ExternalPayloadCarrier) {
        let descriptor_id = SectionId::from_bytes([3; 16]);
        let bundle = encode_bundle(
            BundleKind::ContentPack,
            br#"{"kind":"content"}"#,
            vec![SectionInput::external_ref(
                descriptor_id,
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                payload.len() as u64,
                BundleDigest::of(payload),
            )],
        )
        .expect("external content pack encodes");
        let view = BundleView::parse(&bundle, ReadBudget::default()).expect("bundle parses");
        let carrier = ExternalPayloadCarrier::from_descriptor(
            &view.sections()[0],
            view.artifact_identity(),
            ExternalPayloadMediaType::default(),
            payload.len() as u64,
            BundleDigest::of(payload),
            [ReleaseMirror::new("file:payload.bin").expect("payload mirror")],
        )
        .expect("carrier builds");
        (bundle, carrier)
    }

    #[test]
    fn external_payload_carrier_binds_descriptor_and_verifies_bytes() {
        let payload = b"voice-payload";
        let (_bundle, carrier) = external_content_pack(payload);

        assert_eq!(carrier.decoded_size, payload.len() as u64);
        assert_eq!(carrier.compressed_size, payload.len() as u64);
        assert!(!carrier.requires_payload_bytes(ExternalPayloadMaterializationMode::MetadataOnly));
        assert!(carrier.requires_payload_bytes(ExternalPayloadMaterializationMode::AllPayloads));
        assert_eq!(
            carrier
                .verify_stored_bytes(payload)
                .expect("payload verifies"),
            payload.to_vec()
        );
    }

    #[test]
    fn awfr_archive_json_is_deterministic() {
        let payload = b"voice-payload";
        let (bundle, carrier) = external_content_pack(payload);
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::with_priority("file:content.awfb", 1).expect("bundle mirror")],
        )
        .expect("bundle ref");
        let release_manifest = ReleaseManifest::new([bundle_ref]).expect("release manifest");
        let archive = AwfrArchiveManifest::new(
            ReleaseChannel::new("nightly").expect("channel"),
            release_manifest,
            [carrier],
        )
        .expect("archive");

        let first = archive.to_json_bytes().expect("archive encodes");
        let decoded = AwfrArchiveManifest::from_json_slice(&first).expect("archive decodes");
        let second = decoded.to_json_bytes().expect("archive re-encodes");

        assert_eq!(first, second);
        assert_eq!(
            archive.unsigned_identity_digest(),
            decoded.unsigned_identity_digest()
        );
    }

    #[test]
    fn rewrite_plan_add_replace_remove_payload_carriers() {
        let payload = b"voice-payload";
        let replacement_payload = b"voice-payload-v2";
        let (bundle, carrier) = external_content_pack(payload);
        let (replacement_bundle, replacement) = external_content_pack(replacement_payload);
        let replacement_bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &replacement_bundle,
            [ReleaseMirror::new("file:content-v2.awfb").expect("replacement bundle mirror")],
        )
        .expect("replacement bundle ref");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("file:content.awfb").expect("bundle mirror")],
        )
        .expect("bundle ref");
        let release_manifest = ReleaseManifest::new([bundle_ref]).expect("release manifest");
        let archive = AwfrArchiveManifest::new(
            ReleaseChannel::new("dev").expect("channel"),
            release_manifest,
            [carrier.clone()],
        )
        .expect("archive");
        let key = carrier.descriptor_key();

        let replaced = ReleaseManifestRewritePlan {
            target_bundle: Some(replacement_bundle_ref),
            external_payload_operations: vec![ExternalPayloadCarrierOperation::Replace {
                bundle_content_root: key.bundle_content_root,
                descriptor_id: key.descriptor_id,
                old_decoded_digest: carrier.decoded_digest,
                next: replacement.clone(),
            }],
        }
        .apply_to(&archive)
        .expect("replace applies");
        let replacement_key = replacement.descriptor_key();
        assert_eq!(
            replaced
                .external_payload(replacement_key)
                .expect("carrier exists")
                .decoded_digest,
            replacement.decoded_digest
        );

        let removed = ReleaseManifestRewritePlan {
            target_bundle: None,
            external_payload_operations: vec![ExternalPayloadCarrierOperation::Remove {
                bundle_content_root: replacement_key.bundle_content_root,
                descriptor_id: replacement_key.descriptor_id,
                old_decoded_digest: replacement.decoded_digest,
            }],
        }
        .apply_to(&replaced)
        .expect("remove applies");
        assert!(removed.external_payload(replacement_key).is_none());

        let added = ReleaseManifestRewritePlan {
            target_bundle: None,
            external_payload_operations: vec![ExternalPayloadCarrierOperation::Add {
                carrier: replacement,
            }],
        }
        .apply_to(&removed)
        .expect("add applies");
        assert!(added.external_payload(replacement_key).is_some());
    }
}
