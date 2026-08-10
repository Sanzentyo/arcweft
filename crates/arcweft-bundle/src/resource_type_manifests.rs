//! Deterministic AWFB framing for canonical resource extension manifests.

use crate::container::{BundleSectionKind, ContentResidency, SectionId, SectionInput};
use arcweft_manifest_model::RawDigest;
use arcweft_resource_manifest::{
    PublishedResourceTypeManifestSetV1, ResourceManifestDecodeLimits,
    ResourceManifestDiagnosticCode, ResourceManifestPackageExpectation,
    ResourceManifestPublicationLimits, ResourceManifestReport, decode_resource_type_manifest,
    publish_resource_type_manifests_v1,
};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::{fmt, sync::Arc};

pub const RESOURCE_TYPE_MANIFESTS_SECTION_SCHEMA: u32 = 1;
pub const RESOURCE_TYPE_MANIFESTS_SECTION_MAGIC: [u8; 8] = *b"AWRM\r\n\x1a\n";

const HEADER_LEN: usize = 8 + 4 + 4 + 32;
const ENTRY_HEADER_LEN: usize = 8 + 32;

#[derive(Debug)]
pub enum ResourceTypeManifestSectionError {
    ArtifactMalformed { message: String },
    ArtifactDigestMismatch { entry: u32 },
    ArtifactNonCanonicalManifest { entry: u32 },
    Manifest(ResourceManifestReport),
    RegistryDigestMismatch,
}

impl ResourceTypeManifestSectionError {
    pub fn code(&self) -> ResourceManifestDiagnosticCode {
        match self {
            Self::ArtifactMalformed { .. } => ResourceManifestDiagnosticCode::ArtifactMalformed,
            Self::ArtifactDigestMismatch { .. } => {
                ResourceManifestDiagnosticCode::ArtifactDigestMismatch
            }
            Self::ArtifactNonCanonicalManifest { .. } => {
                ResourceManifestDiagnosticCode::ArtifactNonCanonicalManifest
            }
            Self::Manifest(report) => report.diagnostics()[0].code(),
            Self::RegistryDigestMismatch => ResourceManifestDiagnosticCode::RegistryDigestMismatch,
        }
    }
}

impl fmt::Display for ResourceTypeManifestSectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArtifactMalformed { message } => {
                write!(formatter, "malformed resource manifest section: {message}")
            }
            Self::ArtifactDigestMismatch { entry } => {
                write!(
                    formatter,
                    "resource manifest entry {entry} digest does not match"
                )
            }
            Self::ArtifactNonCanonicalManifest { entry } => {
                write!(
                    formatter,
                    "resource manifest entry {entry} is not canonical"
                )
            }
            Self::Manifest(_) => formatter.write_str("embedded resource manifest was rejected"),
            Self::RegistryDigestMismatch => {
                formatter.write_str("resource manifest registry digest does not match")
            }
        }
    }
}

impl std::error::Error for ResourceTypeManifestSectionError {}

/// Encodes one required section payload, or `None` when no extension manifests
/// were selected and the section must be omitted.
pub fn encode_resource_type_manifest_section_v1(
    published: &PublishedResourceTypeManifestSetV1,
) -> Result<Option<Vec<u8>>, ResourceTypeManifestSectionError> {
    if published.manifests().is_empty() {
        return Ok(None);
    }
    let count = u32::try_from(published.manifests().len())
        .map_err(|_| malformed("manifest count exceeds u32"))?;
    let payload_bytes = published
        .manifests()
        .iter()
        .try_fold(0_usize, |total, manifest| {
            total
                .checked_add(ENTRY_HEADER_LEN)
                .and_then(|value| value.checked_add(manifest.canonical_bytes().len()))
                .ok_or_else(|| malformed("section length overflow"))
        })?;
    let capacity = HEADER_LEN
        .checked_add(payload_bytes)
        .ok_or_else(|| malformed("section length overflow"))?;
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(&RESOURCE_TYPE_MANIFESTS_SECTION_MAGIC);
    bytes.extend_from_slice(&RESOURCE_TYPE_MANIFESTS_SECTION_SCHEMA.to_le_bytes());
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.extend_from_slice(published.registry_digest().semantic_digest().as_bytes());
    for manifest in published.manifests() {
        let canonical = manifest.canonical_bytes();
        let len = u64::try_from(canonical.len())
            .map_err(|_| malformed("canonical manifest length exceeds u64"))?;
        bytes.extend_from_slice(&len.to_le_bytes());
        bytes.extend_from_slice(RawDigest::for_bytes(canonical).as_bytes());
        bytes.extend_from_slice(canonical);
    }
    Ok(Some(bytes))
}

/// Constructs the original container owner's required section descriptor.
pub fn resource_type_manifest_section_input_v1(
    id: SectionId,
    published: &PublishedResourceTypeManifestSetV1,
) -> Result<Option<SectionInput>, ResourceTypeManifestSectionError> {
    encode_resource_type_manifest_section_v1(published).map(|bytes| {
        bytes.map(|bytes| {
            SectionInput::embedded(
                id,
                BundleSectionKind::ResourceTypeManifests,
                RESOURCE_TYPE_MANIFESTS_SECTION_SCHEMA,
                ContentResidency::Startup,
                true,
                bytes,
            )
        })
    })
}

/// Verifies framing, entry digests and canonical bytes before atomically
/// reconstructing the registry against the supplied engine base.
pub fn decode_resource_type_manifest_section_v1(
    bytes: &[u8],
    base: &ResourceTypeRegistry,
    decode_limits: ResourceManifestDecodeLimits,
    publication_limits: ResourceManifestPublicationLimits,
) -> Result<PublishedResourceTypeManifestSetV1, ResourceTypeManifestSectionError> {
    if framing_work(bytes.len()) > publication_limits.work_units() {
        return Err(malformed(
            "section framing exceeds deterministic work limit",
        ));
    }
    let mut cursor = Cursor::new(bytes);
    if cursor.take(8)? != RESOURCE_TYPE_MANIFESTS_SECTION_MAGIC {
        return Err(malformed("magic does not match"));
    }
    let schema = cursor.u32()?;
    if schema != RESOURCE_TYPE_MANIFESTS_SECTION_SCHEMA {
        return Err(malformed("unsupported internal schema version"));
    }
    let count = cursor.u32()?;
    let count_usize = usize::try_from(count).map_err(|_| malformed("manifest count overflow"))?;
    if count_usize > publication_limits.semantic_records() {
        return Err(malformed("manifest count exceeds publication record limit"));
    }
    let claimed_registry_digest = cursor.array_32()?;
    let mut manifests = Vec::with_capacity(count_usize);
    let mut previous = None;
    for entry in 0..count {
        let length = cursor.u64()?;
        let length = usize::try_from(length).map_err(|_| malformed("entry length overflow"))?;
        if length > decode_limits.bytes() {
            return Err(malformed("embedded manifest exceeds document byte limit"));
        }
        let claimed_digest = cursor.array_32()?;
        let canonical = cursor.take(length)?;
        if RawDigest::for_bytes(canonical).as_bytes() != &claimed_digest {
            return Err(ResourceTypeManifestSectionError::ArtifactDigestMismatch { entry });
        }
        let text = std::str::from_utf8(canonical)
            .map_err(|_| malformed("embedded manifest is not UTF-8"))?;
        let document_id = SourceDocumentId::try_new(format!(
            "resource-type-manifest-artifact:{entry}:{}",
            RawDigest::for_bytes(canonical)
        ))
        .map_err(|error| malformed(error.to_string()))?;
        let document = Arc::new(
            SourceDocument::try_new(document_id, SourceName::Generated, text)
                .map_err(|error| malformed(error.to_string()))?,
        );
        let accepted = decode_resource_type_manifest(
            document,
            ResourceManifestPackageExpectation::EmbeddedArtifact,
            decode_limits,
        )
        .map_err(ResourceTypeManifestSectionError::Manifest)?;
        let coordinate = accepted.typed().package();
        if previous.as_ref().is_some_and(|value| value >= coordinate) {
            return Err(malformed("manifest coordinates are not strictly ordered"));
        }
        previous = Some(coordinate.clone());
        if accepted.canonical_bytes() != canonical {
            return Err(ResourceTypeManifestSectionError::ArtifactNonCanonicalManifest { entry });
        }
        manifests.push(accepted);
    }
    if !cursor.is_empty() {
        return Err(malformed("section has trailing bytes"));
    }
    let published = publish_resource_type_manifests_v1(base, manifests, publication_limits)
        .map_err(ResourceTypeManifestSectionError::Manifest)?;
    if published.registry_digest().semantic_digest().as_bytes() != &claimed_registry_digest {
        return Err(ResourceTypeManifestSectionError::RegistryDigestMismatch);
    }
    Ok(published)
}

fn framing_work(bytes: usize) -> u64 {
    let chunks = bytes.saturating_add(63) / 64;
    u64::try_from(chunks).unwrap_or(u64::MAX)
}

fn malformed(message: impl Into<String>) -> ResourceTypeManifestSectionError {
    ResourceTypeManifestSectionError::ArtifactMalformed {
        message: message.into(),
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], ResourceTypeManifestSectionError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| malformed("section offset overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| malformed("section is truncated"))?;
        self.offset = end;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, ResourceTypeManifestSectionError> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .expect("exact cursor length was requested");
        Ok(u32::from_le_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, ResourceTypeManifestSectionError> {
        let bytes: [u8; 8] = self
            .take(8)?
            .try_into()
            .expect("exact cursor length was requested");
        Ok(u64::from_le_bytes(bytes))
    }

    fn array_32(&mut self) -> Result<[u8; 32], ResourceTypeManifestSectionError> {
        Ok(self
            .take(32)?
            .try_into()
            .expect("exact cursor length was requested"))
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
