//! Sans I/O AWFB patch schema 1 model.
//!
//! Patch artifacts are a single schema-1 format in this implementation. The
//! patch bundle manifest binds base and target artifact identities, the `PatchPlan`
//! section carries the descriptor operations plus optional target manifest bytes,
//! changed embedded payloads are carried as `AssetBlob` sections, and core
//! materialization always returns an unsigned target plus a typed report.

use crate::container::{
    ArtifactIdentity, BundleDigest, BundleKind, BundleSectionKind, BundleView, Compression,
    ContentPlacement, ContentResidency, ReadBudget, SectionDescriptor, SectionId, SectionInput,
    SectionKindCode, encode_bundle,
};
use crate::release::{ReleaseManifestError, ReleaseSignaturePolicy};
use crate::resource_codec::product_catalog::migrated_product_catalog_section_compatibility;
use crate::resource_codec::runtime::{
    RuntimeResourceCompatibility, migrated_runtime_section_compatibility,
};
use crate::resource_codec::view::migrated_view_section_compatibility;
use arcweft_core::awbc::codec::AwbcDecodeBudget;
use arcweft_core::awbc::schema::{
    AWBC_ABI_VERSION, AwbcBlock, AwbcFrameLayout, AwbcFunction, AwbcInstruction, AwbcProgram,
    AwbcSignature, AwbcTableRange,
};
use arcweft_core::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const PATCH_PLAN_SCHEMA_VERSION: u32 = 1;
const PATCH_PAYLOAD_CARRIER_KIND: BundleSectionKind = BundleSectionKind::AssetBlob;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionOperation {
    Add(SectionDescriptor),
    Replace {
        old: BundleDigest,
        next: SectionDescriptor,
    },
    Remove {
        id: SectionId,
        old: BundleDigest,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct BundlePatchPlan {
    pub base_content_root: BundleDigest,
    pub target_content_root: BundleDigest,
    pub operations: Vec<SectionOperation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct RuntimeAbiRange {
    pub min: u32,
    pub max: u32,
}

#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum PatchCompatibility {
    ContentOnly,
    CodeCompatible,
    CodeGenerational,
    RestartRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchDescriptorMergeMode {
    ReplaceBySectionId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchManifestRewrite {
    PreserveBaseManifest,
    ReplaceWithTargetManifestBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchExternalDescriptorPolicy {
    MetadataOnlyAllowed,
    PayloadRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchTargetSignaturePolicy {
    StripBaseSignature,
    AdapterMaySignTarget,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PatchMaterializationContract {
    pub descriptor_merge: PatchDescriptorMergeMode,
    pub manifest_rewrite: PatchManifestRewrite,
    pub external_descriptor_policy: PatchExternalDescriptorPolicy,
    pub target_signature: PatchTargetSignaturePolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionChangeOperation {
    Add,
    Replace,
    Remove,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionChangeDerivation {
    RuntimeCompactCodec,
    ProductCatalogCompactCodec,
    ViewCompactCodec,
    AwbcExecutableFingerprint,
    ExternalDescriptor,
    SectionKindDefault,
    UnknownOptionalSectionKind,
    RemovalRequiresRestart,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SectionCompatibilityFingerprint {
    pub id: SectionId,
    pub operation: SectionChangeOperation,
    pub raw_kind_code: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known_kind: Option<BundleSectionKind>,
    pub required: bool,
    pub compatibility: PatchCompatibility,
    pub derivation: SectionChangeDerivation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_descriptor_fingerprint: Option<BundleDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_descriptor_fingerprint: Option<BundleDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_content_fingerprint: Option<BundleDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_content_fingerprint: Option<BundleDigest>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct BundlePatchManifest {
    pub schema_version: u32,
    pub min_reader_schema_version: u32,
    pub runtime_abi: RuntimeAbiRange,
    pub base_artifact: ArtifactIdentity,
    pub target_artifact: ArtifactIdentity,
    pub base_content_root: BundleDigest,
    pub target_content_root: BundleDigest,
    pub compatibility: PatchCompatibility,
    pub materialization: PatchMaterializationContract,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compatibility_fingerprints: Vec<SectionCompatibilityFingerprint>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct BundlePatchArtifact {
    pub manifest: BundlePatchManifest,
    pub plan: BundlePatchPlan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_manifest_bytes: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_sections: Vec<PatchSectionPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PatchSectionPayload {
    pub descriptor: SectionDescriptor,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct PatchPlanSection {
    schema_version: u32,
    plan: BundlePatchPlan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_manifest_bytes: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchMaterializationState {
    Planned,
    BaseValidated,
    DescriptorsMerged,
    ManifestRewritten,
    TargetEncoded,
    TargetValidated,
    Materialized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchTargetSignatureState {
    BaseSignatureInvalidated,
    UnsignedMaterializedTarget,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PatchMaterializationReport {
    pub completed_states: Vec<PatchMaterializationState>,
    pub base_artifact: ArtifactIdentity,
    pub target_artifact: ArtifactIdentity,
    pub compatibility: PatchCompatibility,
    pub target_signature: PatchTargetSignatureState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchMaterializedTarget {
    bytes: Vec<u8>,
    report: PatchMaterializationReport,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PatchValidationError {
    #[error("patch base root mismatch: active {active}, expected {expected}")]
    WrongBase {
        active: BundleDigest,
        expected: BundleDigest,
    },
}

#[derive(Debug, Error)]
pub enum PatchBundleError {
    #[error("failed to encode AWFB patch payload: {0}")]
    EncodePayload(#[source] serde_json::Error),
    #[error("failed to decode AWFB patch payload: {0}")]
    DecodePayload(#[source] serde_json::Error),
    #[error("failed to classify patch compatibility: {message}")]
    Compatibility { message: String },
    #[error("invalid AWFB patch container: {0}")]
    Container(#[source] crate::container::ContainerError),
    #[error("AWFB patch signature policy failed: {0}")]
    SignaturePolicy(#[source] ReleaseManifestError),
    #[error("AWFB bundle kind {actual:?} is not a patch bundle")]
    WrongBundleKind { actual: BundleKind },
    #[error("unsupported AWFB patch schema {actual}; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error("AWFB patch min reader schema {required} is newer than this reader {reader}")]
    UnsupportedMinReader { required: u32, reader: u32 },
    #[error("AWFB patch bundle is missing its PatchPlan section")]
    MissingPatchPlan,
    #[error("AWFB patch bundle contains more than one PatchPlan section")]
    DuplicatePatchPlan,
    #[error("AWFB patch manifest and PatchPlan section disagree about content roots")]
    ContentRootMismatch,
    #[error("AWFB patch runtime ABI range {min}..={max} does not include current ABI {current}")]
    UnsupportedRuntimeAbi { min: u32, max: u32, current: u32 },
    #[error("AWFB patch section change fingerprints do not match PatchPlan operations")]
    SectionFingerprintMismatch,
    #[error("AWFB PatchPlan contains duplicate operation for section {0}")]
    DuplicateSectionOperation(SectionId),
    #[error("AWFB patch manifest compatibility does not match section fingerprints")]
    CompatibilityMismatch,
    #[error(
        "AWFB patch section {id} compatibility fingerprint does not match the materialized target"
    )]
    MaterializedFingerprintMismatch {
        id: SectionId,
        declared: Box<SectionCompatibilityFingerprint>,
        derived: Box<SectionCompatibilityFingerprint>,
    },
    #[error("AWFB patch target manifest bytes are required for target manifest digest {expected}")]
    MissingTargetManifest { expected: BundleDigest },
    #[error("AWFB patch target manifest digest mismatch: expected {expected}, actual {actual}")]
    TargetManifestDigestMismatch {
        expected: BundleDigest,
        actual: BundleDigest,
    },
    #[error("AWFB patch is missing changed-section payload {0}")]
    MissingSectionPayload(SectionId),
    #[error("AWFB patch contains unexpected changed-section payload {0}")]
    UnexpectedSectionPayload(SectionId),
    #[error("AWFB patch contains duplicate changed-section payload {0}")]
    DuplicateSectionPayload(SectionId),
    #[error("AWFB patch payload {id} digest mismatch: expected {expected}, actual {actual}")]
    PayloadDigestMismatch {
        id: SectionId,
        expected: BundleDigest,
        actual: BundleDigest,
    },
    #[error("AWFB patch payload {id} descriptor does not match the PatchPlan operation")]
    PayloadDescriptorMismatch { id: SectionId },
    #[error("AWFB patch can only inline embedded changed sections; section {0} is external")]
    ExternalSectionPayload(SectionId),
    #[error("patch base root mismatch: active {active}, expected {expected}")]
    WrongBase {
        active: BundleDigest,
        expected: BundleDigest,
    },
    #[error("AWFB patch base artifact mismatch: active {active:?}, expected {expected:?}")]
    BaseIdentityMismatch {
        active: Box<ArtifactIdentity>,
        expected: Box<ArtifactIdentity>,
    },
    #[error("AWFB patch target artifact mismatch: actual {actual:?}, expected {expected:?}")]
    TargetIdentityMismatch {
        actual: Box<ArtifactIdentity>,
        expected: Box<ArtifactIdentity>,
    },
    #[error("AWFB patch base section {0} is missing")]
    MissingBaseSection(SectionId),
    #[error("AWFB patch add operation targets existing base section {0}")]
    BaseSectionAlreadyExists(SectionId),
    #[error("AWFB patch base section {id} digest mismatch: expected {expected}, actual {actual}")]
    BaseDigestMismatch {
        id: SectionId,
        expected: BundleDigest,
        actual: BundleDigest,
    },
    #[error("AWFB patch materialized target root mismatch: expected {expected}, actual {actual}")]
    TargetContentRootMismatch {
        expected: BundleDigest,
        actual: BundleDigest,
    },
}

impl BundlePatchPlan {
    pub fn diff(base: &BundleView<'_>, target: &BundleView<'_>) -> Self {
        diff_sections(
            base.content_root(),
            target.content_root(),
            base.sections(),
            target.sections(),
        )
    }

    pub fn validate_base(&self, active: BundleDigest) -> Result<(), PatchValidationError> {
        if active == self.base_content_root {
            Ok(())
        } else {
            Err(PatchValidationError::WrongBase {
                active,
                expected: self.base_content_root,
            })
        }
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

impl RuntimeAbiRange {
    pub const CURRENT: Self = Self {
        min: AWBC_ABI_VERSION,
        max: AWBC_ABI_VERSION,
    };

    pub const fn contains(self, abi: u32) -> bool {
        self.min <= abi && abi <= self.max
    }
}

impl PatchCompatibility {
    pub fn conservative_for_plan(plan: &BundlePatchPlan) -> Self {
        plan.operations
            .iter()
            .map(operation_default_compatibility)
            .fold(Self::ContentOnly, Self::max)
    }

    pub const fn can_apply_live(self) -> bool {
        !matches!(self, Self::RestartRequired)
    }

    pub const fn requires_quiescence(self) -> bool {
        !matches!(self, Self::ContentOnly)
    }

    pub const fn keeps_old_generation(self) -> bool {
        matches!(self, Self::CodeGenerational)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::ContentOnly => "content-only",
            Self::CodeCompatible => "code-compatible",
            Self::CodeGenerational => "code-generational",
            Self::RestartRequired => "restart-required",
        }
    }

    #[must_use]
    pub const fn max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::ContentOnly => 0,
            Self::CodeCompatible => 1,
            Self::CodeGenerational => 2,
            Self::RestartRequired => 3,
        }
    }
}

impl PatchMaterializedTarget {
    /// Returns the verified target container bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the materialization evidence for this target.
    #[must_use]
    pub const fn report(&self) -> &PatchMaterializationReport {
        &self.report
    }

    /// Returns compatibility derived from the active base and materialized target.
    #[must_use]
    pub const fn compatibility(&self) -> PatchCompatibility {
        self.report.compatibility
    }

    /// Consumes the verified target and returns its container bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl std::fmt::Display for PatchCompatibility {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

impl Default for PatchMaterializationContract {
    fn default() -> Self {
        Self {
            descriptor_merge: PatchDescriptorMergeMode::ReplaceBySectionId,
            manifest_rewrite: PatchManifestRewrite::PreserveBaseManifest,
            external_descriptor_policy: PatchExternalDescriptorPolicy::MetadataOnlyAllowed,
            target_signature: PatchTargetSignaturePolicy::StripBaseSignature,
        }
    }
}

impl PatchMaterializationContract {
    pub const fn for_manifest_rewrite(rewrites_manifest: bool) -> Self {
        Self {
            descriptor_merge: PatchDescriptorMergeMode::ReplaceBySectionId,
            manifest_rewrite: if rewrites_manifest {
                PatchManifestRewrite::ReplaceWithTargetManifestBytes
            } else {
                PatchManifestRewrite::PreserveBaseManifest
            },
            external_descriptor_policy: PatchExternalDescriptorPolicy::MetadataOnlyAllowed,
            target_signature: PatchTargetSignaturePolicy::StripBaseSignature,
        }
    }
}

impl BundlePatchManifest {
    pub fn for_artifact_parts(
        base: &BundleView<'_>,
        target: &BundleView<'_>,
        plan: &BundlePatchPlan,
        compatibility_fingerprints: Vec<SectionCompatibilityFingerprint>,
    ) -> Self {
        let compatibility = compatibility_for_fingerprints(&compatibility_fingerprints);
        Self {
            schema_version: PATCH_PLAN_SCHEMA_VERSION,
            min_reader_schema_version: PATCH_PLAN_SCHEMA_VERSION,
            runtime_abi: RuntimeAbiRange::CURRENT,
            base_artifact: base.artifact_identity(),
            target_artifact: target.artifact_identity(),
            base_content_root: plan.base_content_root,
            target_content_root: plan.target_content_root,
            compatibility,
            materialization: PatchMaterializationContract::for_manifest_rewrite(
                base.manifest() != target.manifest(),
            ),
            compatibility_fingerprints,
        }
    }
}

impl BundlePatchArtifact {
    pub fn from_views(
        base: &BundleView<'_>,
        target: &BundleView<'_>,
    ) -> Result<Self, PatchBundleError> {
        let plan = BundlePatchPlan::diff(base, target);
        let changed_sections = changed_section_payloads(&plan, target)?;
        let compatibility_fingerprints = section_fingerprints_for_plan(base, target, &plan)?;
        let manifest = BundlePatchManifest::for_artifact_parts(
            base,
            target,
            &plan,
            compatibility_fingerprints,
        );
        let target_manifest_bytes =
            (base.manifest() != target.manifest()).then(|| target.manifest().to_vec());
        let artifact = Self {
            manifest,
            plan,
            target_manifest_bytes,
            changed_sections,
        };
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<(), PatchBundleError> {
        validate_schema(self.manifest.schema_version)?;
        if self.manifest.min_reader_schema_version > PATCH_PLAN_SCHEMA_VERSION {
            return Err(PatchBundleError::UnsupportedMinReader {
                required: self.manifest.min_reader_schema_version,
                reader: PATCH_PLAN_SCHEMA_VERSION,
            });
        }
        if self.manifest.base_content_root != self.plan.base_content_root
            || self.manifest.target_content_root != self.plan.target_content_root
            || self.manifest.base_artifact.content_root != self.plan.base_content_root
            || self.manifest.target_artifact.content_root != self.plan.target_content_root
        {
            return Err(PatchBundleError::ContentRootMismatch);
        }
        if !self.manifest.runtime_abi.contains(AWBC_ABI_VERSION) {
            return Err(PatchBundleError::UnsupportedRuntimeAbi {
                min: self.manifest.runtime_abi.min,
                max: self.manifest.runtime_abi.max,
                current: AWBC_ABI_VERSION,
            });
        }
        validate_operation_ids(&self.plan)?;
        self.validate_changed_section_payloads()?;
        validate_section_fingerprints(&self.plan, &self.manifest.compatibility_fingerprints)?;
        if compatibility_for_fingerprints(&self.manifest.compatibility_fingerprints)
            != self.manifest.compatibility
        {
            return Err(PatchBundleError::CompatibilityMismatch);
        }
        if self.manifest.materialization.manifest_rewrite
            == PatchManifestRewrite::ReplaceWithTargetManifestBytes
            && self.target_manifest_bytes.is_none()
        {
            return Err(PatchBundleError::MissingTargetManifest {
                expected: self.manifest.target_artifact.manifest_digest,
            });
        }
        if let Some(bytes) = &self.target_manifest_bytes {
            let actual = BundleDigest::of(bytes);
            if actual != self.manifest.target_artifact.manifest_digest {
                return Err(PatchBundleError::TargetManifestDigestMismatch {
                    expected: self.manifest.target_artifact.manifest_digest,
                    actual,
                });
            }
        }
        Ok(())
    }

    fn validate_changed_section_payloads(&self) -> Result<(), PatchBundleError> {
        let expected = expected_changed_sections(&self.plan);
        let mut seen = BTreeMap::new();
        for payload in &self.changed_sections {
            let id = payload.descriptor.id();
            if seen.insert(id, payload).is_some() {
                return Err(PatchBundleError::DuplicateSectionPayload(id));
            }
            let Some(expected_descriptor) = expected.get(&id) else {
                return Err(PatchBundleError::UnexpectedSectionPayload(id));
            };
            if !logical_descriptor_matches(&payload.descriptor, expected_descriptor) {
                return Err(PatchBundleError::PayloadDescriptorMismatch { id });
            }
            if payload.descriptor.placement() != ContentPlacement::Embedded {
                return Err(PatchBundleError::ExternalSectionPayload(id));
            }
            let actual = BundleDigest::of(&payload.bytes);
            if actual != expected_descriptor.content_digest() {
                return Err(PatchBundleError::PayloadDigestMismatch {
                    id,
                    expected: expected_descriptor.content_digest(),
                    actual,
                });
            }
        }
        for id in expected.keys() {
            if !seen.contains_key(id) {
                return Err(PatchBundleError::MissingSectionPayload(*id));
            }
        }
        Ok(())
    }
}

impl From<PatchValidationError> for PatchBundleError {
    fn from(value: PatchValidationError) -> Self {
        match value {
            PatchValidationError::WrongBase { active, expected } => {
                Self::WrongBase { active, expected }
            }
        }
    }
}

pub fn encode_patch_bundle(artifact: &BundlePatchArtifact) -> Result<Vec<u8>, PatchBundleError> {
    artifact.validate()?;
    let manifest =
        serde_json::to_vec(&artifact.manifest).map_err(PatchBundleError::EncodePayload)?;
    let plan = serde_json::to_vec(&PatchPlanSection {
        schema_version: PATCH_PLAN_SCHEMA_VERSION,
        plan: artifact.plan.clone(),
        target_manifest_bytes: artifact.target_manifest_bytes.clone(),
    })
    .map_err(PatchBundleError::EncodePayload)?;
    let mut sections = vec![SectionInput::embedded(
        patch_plan_section_id(),
        BundleSectionKind::PatchPlan,
        PATCH_PLAN_SCHEMA_VERSION,
        ContentResidency::Startup,
        true,
        plan,
    )];
    sections.extend(
        artifact
            .changed_sections
            .iter()
            .map(section_input_from_payload_carrier)
            .collect::<Result<Vec<_>, _>>()?,
    );
    encode_bundle(BundleKind::Patch, &manifest, sections).map_err(PatchBundleError::Container)
}

pub fn decode_patch_bundle(bytes: &[u8]) -> Result<BundlePatchArtifact, PatchBundleError> {
    let view =
        BundleView::parse(bytes, ReadBudget::default()).map_err(PatchBundleError::Container)?;
    if view.kind() != BundleKind::Patch {
        return Err(PatchBundleError::WrongBundleKind {
            actual: view.kind(),
        });
    }
    let manifest: BundlePatchManifest =
        serde_json::from_slice(view.manifest()).map_err(PatchBundleError::DecodePayload)?;
    validate_schema(manifest.schema_version)?;
    let plan_section = decode_patch_plan_section(&view)?;
    validate_operation_ids(&plan_section.plan)?;
    let changed_sections = decode_changed_sections(&view, &plan_section.plan)?;
    let artifact = BundlePatchArtifact {
        manifest,
        plan: plan_section.plan,
        target_manifest_bytes: plan_section.target_manifest_bytes,
        changed_sections,
    };
    artifact.validate()?;
    Ok(artifact)
}

pub fn decode_patch_bundle_with_signature_policy(
    bytes: &[u8],
    signature_policy: &ReleaseSignaturePolicy,
) -> Result<BundlePatchArtifact, PatchBundleError> {
    let view =
        BundleView::parse(bytes, ReadBudget::default()).map_err(PatchBundleError::Container)?;
    if view.kind() != BundleKind::Patch {
        return Err(PatchBundleError::WrongBundleKind {
            actual: view.kind(),
        });
    }
    signature_policy
        .verify_awfb_bytes(view.content_root(), bytes)
        .map_err(PatchBundleError::SignaturePolicy)?;
    decode_patch_bundle(bytes)
}

pub fn apply_patch_bundle_bytes(
    base_bytes: &[u8],
    patch_bytes: &[u8],
) -> Result<PatchMaterializedTarget, PatchBundleError> {
    let artifact = decode_patch_bundle(patch_bytes)?;
    apply_patch_bundle(base_bytes, &artifact)
}

pub fn apply_signed_patch_bundle_bytes(
    base_bytes: &[u8],
    patch_bytes: &[u8],
    signature_policy: &ReleaseSignaturePolicy,
) -> Result<PatchMaterializedTarget, PatchBundleError> {
    let artifact = decode_patch_bundle_with_signature_policy(patch_bytes, signature_policy)?;
    apply_patch_bundle(base_bytes, &artifact)
}

pub fn apply_patch_bundle(
    base_bytes: &[u8],
    artifact: &BundlePatchArtifact,
) -> Result<PatchMaterializedTarget, PatchBundleError> {
    artifact.validate()?;
    let base = BundleView::parse(base_bytes, ReadBudget::default())
        .map_err(PatchBundleError::Container)?;
    artifact.plan.validate_base(base.content_root())?;
    let active_identity = base.artifact_identity();
    if active_identity != artifact.manifest.base_artifact {
        return Err(PatchBundleError::BaseIdentityMismatch {
            active: Box::new(active_identity),
            expected: Box::new(artifact.manifest.base_artifact),
        });
    }

    let mut states = vec![
        PatchMaterializationState::Planned,
        PatchMaterializationState::BaseValidated,
    ];
    let sections = materialized_sections(&base, artifact)?;
    states.push(PatchMaterializationState::DescriptorsMerged);
    let manifest = target_manifest_bytes(&base, artifact)?;
    states.push(PatchMaterializationState::ManifestRewritten);
    let target =
        encode_bundle(base.kind(), manifest, sections).map_err(PatchBundleError::Container)?;
    states.push(PatchMaterializationState::TargetEncoded);
    let target_view =
        BundleView::parse(&target, ReadBudget::default()).map_err(PatchBundleError::Container)?;
    let actual_root = target_view.content_root();
    if actual_root != artifact.manifest.target_content_root {
        return Err(PatchBundleError::TargetContentRootMismatch {
            expected: artifact.manifest.target_content_root,
            actual: actual_root,
        });
    }
    let actual_identity = target_view.artifact_identity();
    if actual_identity != artifact.manifest.target_artifact {
        return Err(PatchBundleError::TargetIdentityMismatch {
            actual: Box::new(actual_identity),
            expected: Box::new(artifact.manifest.target_artifact),
        });
    }
    let compatibility = verify_materialized_fingerprints(&base, &target_view, artifact)?;
    states.push(PatchMaterializationState::TargetValidated);
    states.push(PatchMaterializationState::Materialized);
    Ok(PatchMaterializedTarget {
        bytes: target,
        report: PatchMaterializationReport {
            completed_states: states,
            base_artifact: artifact.manifest.base_artifact,
            target_artifact: artifact.manifest.target_artifact,
            compatibility,
            target_signature: PatchTargetSignatureState::BaseSignatureInvalidated,
        },
    })
}

pub fn diff_sections(
    base_content_root: BundleDigest,
    target_content_root: BundleDigest,
    base: &[SectionDescriptor],
    target: &[SectionDescriptor],
) -> BundlePatchPlan {
    let base_by_id = section_index(base);
    let target_by_id = section_index(target);
    let mut operations = target_by_id
        .iter()
        .filter_map(|(id, next)| match base_by_id.get(id) {
            None => Some(SectionOperation::Add((*next).clone())),
            Some(old)
                if old.content_digest() != next.content_digest()
                    || old.kind_code() != next.kind_code()
                    || old.schema_version() != next.schema_version()
                    || old.residency() != next.residency()
                    || old.placement() != next.placement()
                    || old.compression() != next.compression()
                    || old.decoded_size() != next.decoded_size()
                    || old.required() != next.required() =>
            {
                Some(SectionOperation::Replace {
                    old: old.content_digest(),
                    next: (*next).clone(),
                })
            }
            Some(_) => None,
        })
        .collect::<Vec<_>>();

    operations.extend(base_by_id.iter().filter_map(|(id, old)| {
        (!target_by_id.contains_key(id)).then_some(SectionOperation::Remove {
            id: *id,
            old: old.content_digest(),
        })
    }));

    BundlePatchPlan {
        base_content_root,
        target_content_root,
        operations,
    }
}

fn validate_schema(actual: u32) -> Result<(), PatchBundleError> {
    if actual == PATCH_PLAN_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(PatchBundleError::UnsupportedSchema {
            actual,
            expected: PATCH_PLAN_SCHEMA_VERSION,
        })
    }
}

fn changed_section_payloads(
    plan: &BundlePatchPlan,
    target: &BundleView<'_>,
) -> Result<Vec<PatchSectionPayload>, PatchBundleError> {
    plan.operations
        .iter()
        .filter_map(operation_next_descriptor)
        .filter(|descriptor| descriptor.placement().is_embedded())
        .map(|descriptor| {
            target
                .decoded_section(descriptor.id())
                .map_err(PatchBundleError::Container)?
                .ok_or(PatchBundleError::MissingSectionPayload(descriptor.id()))
                .map(|bytes| PatchSectionPayload {
                    descriptor: descriptor.clone(),
                    bytes,
                })
        })
        .collect()
}

fn materialized_sections(
    base: &BundleView<'_>,
    artifact: &BundlePatchArtifact,
) -> Result<Vec<SectionInput>, PatchBundleError> {
    let base_by_id = section_index(base.sections());
    let changed_by_id = artifact
        .changed_sections
        .iter()
        .map(|payload| (payload.descriptor.id(), payload))
        .collect::<BTreeMap<_, _>>();
    let operations_by_id = artifact
        .plan
        .operations
        .iter()
        .map(operation_id)
        .zip(&artifact.plan.operations)
        .collect::<BTreeMap<_, _>>();

    let mut sections = Vec::new();
    for descriptor in base.sections() {
        let operation = operations_by_id.get(&descriptor.id()).copied();
        match operation {
            Some(SectionOperation::Remove { old, .. }) => {
                validate_old_digest(descriptor, *old)?;
            }
            Some(SectionOperation::Replace { old, next }) => {
                validate_old_digest(descriptor, *old)?;
                sections.push(section_input_from_patch_descriptor(next, &changed_by_id)?);
            }
            Some(SectionOperation::Add(_)) => {
                return Err(PatchBundleError::BaseSectionAlreadyExists(descriptor.id()));
            }
            None => sections.push(section_input_from_base(base, descriptor)?),
        }
    }

    let base_section_ids = base_by_id.keys().copied().collect::<BTreeSet<_>>();
    for operation in &artifact.plan.operations {
        match operation {
            SectionOperation::Add(descriptor) => {
                if base_section_ids.contains(&descriptor.id()) {
                    return Err(PatchBundleError::BaseSectionAlreadyExists(descriptor.id()));
                }
                sections.push(section_input_from_patch_descriptor(
                    descriptor,
                    &changed_by_id,
                )?);
            }
            SectionOperation::Replace { next, .. } => {
                if !base_section_ids.contains(&next.id()) {
                    return Err(PatchBundleError::MissingBaseSection(next.id()));
                }
            }
            SectionOperation::Remove { id, .. } => {
                if !base_section_ids.contains(id) {
                    return Err(PatchBundleError::MissingBaseSection(*id));
                }
            }
        }
    }
    Ok(sections)
}

fn target_manifest_bytes<'a>(
    base: &'a BundleView<'_>,
    artifact: &'a BundlePatchArtifact,
) -> Result<&'a [u8], PatchBundleError> {
    if let Some(bytes) = &artifact.target_manifest_bytes {
        let actual = BundleDigest::of(bytes);
        if actual != artifact.manifest.target_artifact.manifest_digest {
            return Err(PatchBundleError::TargetManifestDigestMismatch {
                expected: artifact.manifest.target_artifact.manifest_digest,
                actual,
            });
        }
        Ok(bytes)
    } else {
        let actual = BundleDigest::of(base.manifest());
        if actual != artifact.manifest.target_artifact.manifest_digest {
            return Err(PatchBundleError::MissingTargetManifest {
                expected: artifact.manifest.target_artifact.manifest_digest,
            });
        }
        Ok(base.manifest())
    }
}

fn operation_id(operation: &SectionOperation) -> SectionId {
    match operation {
        SectionOperation::Add(descriptor) => descriptor.id(),
        SectionOperation::Replace { next, .. } => next.id(),
        SectionOperation::Remove { id, .. } => *id,
    }
}

fn validate_operation_ids(plan: &BundlePatchPlan) -> Result<(), PatchBundleError> {
    let mut ids = BTreeSet::new();
    for id in plan.operations.iter().map(operation_id) {
        if !ids.insert(id) {
            return Err(PatchBundleError::DuplicateSectionOperation(id));
        }
    }
    Ok(())
}

fn validate_old_digest(
    descriptor: &SectionDescriptor,
    expected: BundleDigest,
) -> Result<(), PatchBundleError> {
    let actual = descriptor.content_digest();
    if actual == expected {
        Ok(())
    } else {
        Err(PatchBundleError::BaseDigestMismatch {
            id: descriptor.id(),
            expected,
            actual,
        })
    }
}

fn section_input_from_payload(
    payload: &PatchSectionPayload,
) -> Result<SectionInput, PatchBundleError> {
    section_input_from_payload_as_kind(payload, payload.descriptor.kind_code())
}

fn section_input_from_payload_carrier(
    payload: &PatchSectionPayload,
) -> Result<SectionInput, PatchBundleError> {
    section_input_from_payload_as_kind(payload, PATCH_PAYLOAD_CARRIER_KIND)
}

fn section_input_from_payload_as_kind(
    payload: &PatchSectionPayload,
    kind: impl Into<SectionKindCode>,
) -> Result<SectionInput, PatchBundleError> {
    if payload.descriptor.placement() != ContentPlacement::Embedded {
        return Err(PatchBundleError::ExternalSectionPayload(
            payload.descriptor.id(),
        ));
    }
    section_input_for_descriptor_as_kind(&payload.descriptor, kind, payload.bytes.clone())
}

fn section_input_from_patch_descriptor(
    descriptor: &SectionDescriptor,
    changed_by_id: &BTreeMap<SectionId, &PatchSectionPayload>,
) -> Result<SectionInput, PatchBundleError> {
    match descriptor.placement() {
        ContentPlacement::Embedded => {
            let payload = changed_by_id
                .get(&descriptor.id())
                .ok_or(PatchBundleError::MissingSectionPayload(descriptor.id()))?;
            section_input_from_payload(payload)
        }
        ContentPlacement::External => Ok(section_input_from_external_descriptor(descriptor)),
    }
}

fn section_input_from_base(
    base: &BundleView<'_>,
    descriptor: &SectionDescriptor,
) -> Result<SectionInput, PatchBundleError> {
    if descriptor.placement() == ContentPlacement::External {
        return Ok(section_input_from_external_descriptor(descriptor));
    }
    let bytes = base
        .decoded_section(descriptor.id())
        .map_err(PatchBundleError::Container)?
        .ok_or(PatchBundleError::MissingBaseSection(descriptor.id()))?;
    section_input_for_descriptor(descriptor, bytes)
}

fn section_input_for_descriptor(
    descriptor: &SectionDescriptor,
    decoded_bytes: impl Into<Vec<u8>>,
) -> Result<SectionInput, PatchBundleError> {
    section_input_for_descriptor_as_kind(descriptor, descriptor.kind_code(), decoded_bytes)
}

fn section_input_for_descriptor_as_kind(
    descriptor: &SectionDescriptor,
    kind: impl Into<SectionKindCode>,
    decoded_bytes: impl Into<Vec<u8>>,
) -> Result<SectionInput, PatchBundleError> {
    let decoded_bytes = decoded_bytes.into();
    let kind = kind.into();
    match descriptor.compression() {
        Compression::None => SectionInput::embedded_raw_optional(
            descriptor.id(),
            kind,
            descriptor.schema_version(),
            descriptor.residency(),
            descriptor.required(),
            decoded_bytes,
        )
        .map_err(PatchBundleError::Container),
        Compression::Zstd => SectionInput::embedded_raw_optional_zstd(
            descriptor.id(),
            kind,
            descriptor.schema_version(),
            descriptor.residency(),
            descriptor.required(),
            decoded_bytes,
        )
        .map_err(PatchBundleError::Container),
    }
}

fn section_input_from_external_descriptor(descriptor: &SectionDescriptor) -> SectionInput {
    SectionInput::external_raw_optional_ref(
        descriptor.id(),
        descriptor.kind_code(),
        descriptor.schema_version(),
        descriptor.residency(),
        descriptor.required(),
        descriptor.decoded_size(),
        descriptor.content_digest(),
    )
    .expect("parsed descriptor cannot contain required unknown section")
}

fn decode_patch_plan_section(view: &BundleView<'_>) -> Result<PatchPlanSection, PatchBundleError> {
    let mut matches = view
        .sections()
        .iter()
        .filter(|descriptor| descriptor.known_kind() == Some(BundleSectionKind::PatchPlan));
    let Some(descriptor) = matches.next() else {
        return Err(PatchBundleError::MissingPatchPlan);
    };
    if matches.next().is_some() {
        return Err(PatchBundleError::DuplicatePatchPlan);
    }
    validate_schema(descriptor.schema_version())?;
    let Some(bytes) = view
        .embedded_section(descriptor.id())
        .map_err(PatchBundleError::Container)?
    else {
        return Err(PatchBundleError::MissingPatchPlan);
    };
    let plan: PatchPlanSection =
        serde_json::from_slice(bytes).map_err(PatchBundleError::DecodePayload)?;
    validate_schema(plan.schema_version)?;
    Ok(plan)
}

fn decode_changed_sections(
    view: &BundleView<'_>,
    plan: &BundlePatchPlan,
) -> Result<Vec<PatchSectionPayload>, PatchBundleError> {
    let expected = expected_changed_sections(plan);
    view.sections()
        .iter()
        .filter(|descriptor| descriptor.known_kind() != Some(BundleSectionKind::PatchPlan))
        .filter(|descriptor| descriptor.placement().is_embedded())
        .map(|descriptor| {
            let Some(expected_descriptor) = expected.get(&descriptor.id()) else {
                return Err(PatchBundleError::UnexpectedSectionPayload(descriptor.id()));
            };
            if !patch_payload_carrier_matches(descriptor, expected_descriptor) {
                return Err(PatchBundleError::PayloadDescriptorMismatch {
                    id: descriptor.id(),
                });
            }
            view.decoded_section(descriptor.id())
                .map_err(PatchBundleError::Container)?
                .ok_or(PatchBundleError::MissingSectionPayload(descriptor.id()))
                .map(|bytes| PatchSectionPayload {
                    descriptor: (*expected_descriptor).clone(),
                    bytes,
                })
        })
        .collect()
}

fn expected_changed_sections(plan: &BundlePatchPlan) -> BTreeMap<SectionId, &SectionDescriptor> {
    plan.operations
        .iter()
        .filter_map(operation_next_descriptor)
        .filter(|descriptor| descriptor.placement().is_embedded())
        .map(|descriptor| (descriptor.id(), descriptor))
        .collect()
}

fn operation_next_descriptor(operation: &SectionOperation) -> Option<&SectionDescriptor> {
    match operation {
        SectionOperation::Add(descriptor)
        | SectionOperation::Replace {
            next: descriptor, ..
        } => Some(descriptor),
        SectionOperation::Remove { .. } => None,
    }
}

fn validate_section_fingerprints(
    plan: &BundlePatchPlan,
    fingerprints: &[SectionCompatibilityFingerprint],
) -> Result<(), PatchBundleError> {
    let expected_ids = plan
        .operations
        .iter()
        .map(operation_id)
        .collect::<BTreeSet<_>>();
    let actual_ids = fingerprints
        .iter()
        .map(|fingerprint| fingerprint.id)
        .collect::<BTreeSet<_>>();
    if expected_ids != actual_ids || fingerprints.len() != actual_ids.len() {
        return Err(PatchBundleError::SectionFingerprintMismatch);
    }
    Ok(())
}

fn section_fingerprints_for_plan(
    base: &BundleView<'_>,
    target: &BundleView<'_>,
    plan: &BundlePatchPlan,
) -> Result<Vec<SectionCompatibilityFingerprint>, PatchBundleError> {
    plan.operations
        .iter()
        .map(|operation| section_fingerprint_for_operation(base, target, operation))
        .collect()
}

fn verify_materialized_fingerprints(
    base: &BundleView<'_>,
    target: &BundleView<'_>,
    artifact: &BundlePatchArtifact,
) -> Result<PatchCompatibility, PatchBundleError> {
    let derived = section_fingerprints_for_plan(base, target, &artifact.plan)?;
    for derived_fingerprint in &derived {
        let declared_fingerprint = artifact
            .manifest
            .compatibility_fingerprints
            .iter()
            .find(|fingerprint| fingerprint.id == derived_fingerprint.id)
            .ok_or(PatchBundleError::SectionFingerprintMismatch)?;
        if declared_fingerprint != derived_fingerprint {
            return Err(PatchBundleError::MaterializedFingerprintMismatch {
                id: derived_fingerprint.id,
                declared: Box::new(declared_fingerprint.clone()),
                derived: Box::new(derived_fingerprint.clone()),
            });
        }
    }
    Ok(compatibility_for_fingerprints(&derived))
}

fn section_fingerprint_for_operation(
    base: &BundleView<'_>,
    target: &BundleView<'_>,
    operation: &SectionOperation,
) -> Result<SectionCompatibilityFingerprint, PatchBundleError> {
    let id = operation_id(operation);
    let base_descriptor = base
        .sections()
        .iter()
        .find(|descriptor| descriptor.id() == id);
    let target_descriptor = operation_next_descriptor(operation);
    let descriptor =
        target_descriptor
            .or(base_descriptor)
            .ok_or_else(|| PatchBundleError::Compatibility {
                message: format!("section operation {id} has no descriptor"),
            })?;
    let (compatibility, derivation) = operation_compatibility(base, target, operation)?;
    Ok(SectionCompatibilityFingerprint {
        id,
        operation: match operation {
            SectionOperation::Add(_) => SectionChangeOperation::Add,
            SectionOperation::Replace { .. } => SectionChangeOperation::Replace,
            SectionOperation::Remove { .. } => SectionChangeOperation::Remove,
        },
        raw_kind_code: descriptor.kind_code().encoded(),
        known_kind: descriptor.known_kind(),
        required: descriptor.required(),
        compatibility,
        derivation,
        base_descriptor_fingerprint: base_descriptor.map(descriptor_fingerprint),
        target_descriptor_fingerprint: target_descriptor.map(descriptor_fingerprint),
        base_content_fingerprint: base_descriptor.map(SectionDescriptor::content_digest),
        target_content_fingerprint: target_descriptor.map(SectionDescriptor::content_digest),
    })
}

fn operation_compatibility(
    base: &BundleView<'_>,
    target: &BundleView<'_>,
    operation: &SectionOperation,
) -> Result<(PatchCompatibility, SectionChangeDerivation), PatchBundleError> {
    match operation {
        SectionOperation::Remove { .. } => Ok((
            PatchCompatibility::RestartRequired,
            SectionChangeDerivation::RemovalRequiresRestart,
        )),
        SectionOperation::Add(descriptor) => Ok((
            descriptor_default_compatibility(descriptor),
            descriptor_default_derivation(descriptor),
        )),
        SectionOperation::Replace { next, .. } => replace_compatibility(base, target, next),
    }
}

fn replace_compatibility(
    base: &BundleView<'_>,
    target: &BundleView<'_>,
    next: &SectionDescriptor,
) -> Result<(PatchCompatibility, SectionChangeDerivation), PatchBundleError> {
    let Some(kind) = next.known_kind() else {
        return Ok((
            PatchCompatibility::ContentOnly,
            SectionChangeDerivation::UnknownOptionalSectionKind,
        ));
    };
    if next.placement() == ContentPlacement::External {
        return Ok((
            kind.patch_default_compatibility(),
            SectionChangeDerivation::ExternalDescriptor,
        ));
    }
    if crate::resource_codec::runtime::runtime_codec_for_section(kind).is_some() {
        let old = decoded_section(
            base,
            next.id(),
            PatchBundleError::MissingBaseSection(next.id()),
        )?;
        let new = decoded_section(
            target,
            next.id(),
            PatchBundleError::MissingSectionPayload(next.id()),
        )?;
        let compatibility = migrated_runtime_section_compatibility(kind, &old, &new)
            .map_err(|error| PatchBundleError::Compatibility {
                message: error.to_string(),
            })?
            .map_or_else(
                || kind.patch_default_compatibility(),
                runtime_resource_patch_compatibility,
            );
        return Ok((compatibility, SectionChangeDerivation::RuntimeCompactCodec));
    }
    if let Some(compatibility) = product_catalog_compatibility(kind, base, target, next.id())? {
        return Ok((
            compatibility,
            SectionChangeDerivation::ProductCatalogCompactCodec,
        ));
    }
    if let Some(compatibility) = view_resource_compatibility(kind, base, target, next.id())? {
        return Ok((compatibility, SectionChangeDerivation::ViewCompactCodec));
    }
    if kind == BundleSectionKind::ProgramBytecode {
        let old = decoded_section(
            base,
            next.id(),
            PatchBundleError::MissingBaseSection(next.id()),
        )?;
        let new = decoded_section(
            target,
            next.id(),
            PatchBundleError::MissingSectionPayload(next.id()),
        )?;
        return Ok((
            awbc_executable_compatibility(&old, &new)?,
            SectionChangeDerivation::AwbcExecutableFingerprint,
        ));
    }
    Ok((
        kind.patch_default_compatibility(),
        SectionChangeDerivation::SectionKindDefault,
    ))
}

fn operation_default_compatibility(operation: &SectionOperation) -> PatchCompatibility {
    match operation {
        SectionOperation::Add(descriptor) => descriptor_default_compatibility(descriptor),
        SectionOperation::Replace { next, .. } => descriptor_default_compatibility(next),
        SectionOperation::Remove { .. } => PatchCompatibility::RestartRequired,
    }
}

fn descriptor_default_compatibility(descriptor: &SectionDescriptor) -> PatchCompatibility {
    descriptor.known_kind().map_or(
        PatchCompatibility::ContentOnly,
        BundleSectionKind::patch_default_compatibility,
    )
}

fn descriptor_default_derivation(descriptor: &SectionDescriptor) -> SectionChangeDerivation {
    if descriptor.known_kind().is_none() {
        SectionChangeDerivation::UnknownOptionalSectionKind
    } else if descriptor.placement() == ContentPlacement::External {
        SectionChangeDerivation::ExternalDescriptor
    } else {
        SectionChangeDerivation::SectionKindDefault
    }
}

fn product_catalog_compatibility(
    kind: BundleSectionKind,
    base: &BundleView<'_>,
    target: &BundleView<'_>,
    id: SectionId,
) -> Result<Option<PatchCompatibility>, PatchBundleError> {
    let old = decoded_section(base, id, PatchBundleError::MissingBaseSection(id))?;
    let new = decoded_section(target, id, PatchBundleError::MissingSectionPayload(id))?;
    migrated_product_catalog_section_compatibility(kind, &old, &new).map_err(|error| {
        PatchBundleError::Compatibility {
            message: error.to_string(),
        }
    })
}

fn view_resource_compatibility(
    kind: BundleSectionKind,
    base: &BundleView<'_>,
    target: &BundleView<'_>,
    id: SectionId,
) -> Result<Option<PatchCompatibility>, PatchBundleError> {
    let old = decoded_section(base, id, PatchBundleError::MissingBaseSection(id))?;
    let new = decoded_section(target, id, PatchBundleError::MissingSectionPayload(id))?;
    migrated_view_section_compatibility(kind, &old, &new).map_err(|error| {
        PatchBundleError::Compatibility {
            message: error.to_string(),
        }
    })
}

fn decoded_section(
    view: &BundleView<'_>,
    id: SectionId,
    missing: PatchBundleError,
) -> Result<Vec<u8>, PatchBundleError> {
    view.decoded_section(id)
        .map_err(PatchBundleError::Container)?
        .ok_or(missing)
}

const fn runtime_resource_patch_compatibility(
    compatibility: RuntimeResourceCompatibility,
) -> PatchCompatibility {
    match compatibility {
        RuntimeResourceCompatibility::ContentOnly => PatchCompatibility::ContentOnly,
        RuntimeResourceCompatibility::CodeCompatible => PatchCompatibility::CodeCompatible,
        RuntimeResourceCompatibility::CodeGenerational => PatchCompatibility::CodeGenerational,
        RuntimeResourceCompatibility::RestartRequired => PatchCompatibility::RestartRequired,
    }
}

fn compatibility_for_fingerprints(
    fingerprints: &[SectionCompatibilityFingerprint],
) -> PatchCompatibility {
    fingerprints
        .iter()
        .map(|fingerprint| fingerprint.compatibility)
        .fold(PatchCompatibility::ContentOnly, PatchCompatibility::max)
}

fn descriptor_fingerprint(descriptor: &SectionDescriptor) -> BundleDigest {
    let mut bytes = Vec::with_capacity(16 + 4 + 4 + 1 + 1 + 1 + 8 + 32 + 1);
    bytes.extend_from_slice(&descriptor.id().as_bytes());
    bytes.extend_from_slice(&descriptor.kind_code().encoded().to_le_bytes());
    bytes.extend_from_slice(&descriptor.schema_version().to_le_bytes());
    bytes.push(descriptor.residency().encoded());
    bytes.push(descriptor.placement().encoded());
    bytes.push(descriptor.compression().encoded());
    bytes.extend_from_slice(&descriptor.decoded_size().to_le_bytes());
    bytes.extend_from_slice(&descriptor.content_digest().as_bytes());
    bytes.push(u8::from(descriptor.required()));
    BundleDigest::of(&bytes)
}

fn awbc_executable_compatibility(
    old: &[u8],
    new: &[u8],
) -> Result<PatchCompatibility, PatchBundleError> {
    let old = decode_awbc_program(old)?;
    let new = decode_awbc_program(new)?;
    if old.header.abi_version != new.header.abi_version {
        return Ok(PatchCompatibility::RestartRequired);
    }
    let old_functions = awbc_function_fingerprints(&old)?;
    let new_functions = awbc_function_fingerprints(&new)?;
    let old_executable =
        old.executable_identity()
            .map_err(|error| PatchBundleError::Compatibility {
                message: error.to_string(),
            })?;
    let new_executable =
        new.executable_identity()
            .map_err(|error| PatchBundleError::Compatibility {
                message: error.to_string(),
            })?;
    if old_functions
        .keys()
        .any(|id| !new_functions.contains_key(id))
    {
        return Ok(PatchCompatibility::RestartRequired);
    }
    let changed_existing_interface = old_functions.iter().any(|(id, old)| {
        new_functions
            .get(id)
            .is_some_and(|new| old.interface != new.interface)
    });
    if changed_existing_interface {
        return Ok(PatchCompatibility::CodeGenerational);
    }
    if old_functions.iter().any(|(id, old)| {
        new_functions
            .get(id)
            .is_some_and(|new| old.body != new.body)
    }) || old_functions.len() != new_functions.len()
        || old_executable != new_executable
    {
        Ok(PatchCompatibility::CodeCompatible)
    } else {
        Ok(PatchCompatibility::ContentOnly)
    }
}

fn decode_awbc_program(bytes: &[u8]) -> Result<AwbcProgram, PatchBundleError> {
    let program =
        AwbcProgram::decode_canonical(bytes, AwbcDecodeBudget::default()).map_err(|error| {
            PatchBundleError::Compatibility {
                message: error.to_string(),
            }
        })?;
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .map_err(|error| PatchBundleError::Compatibility {
            message: error.to_string(),
        })?;
    Ok(program)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AwbcFunctionFingerprint {
    interface: BundleDigest,
    body: BundleDigest,
}

fn awbc_function_fingerprints(
    program: &AwbcProgram,
) -> Result<BTreeMap<String, AwbcFunctionFingerprint>, PatchBundleError> {
    program
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| {
            let function_id = arcweft_core::awbc::schema::AwbcFunctionId(
                u32::try_from(index).unwrap_or(u32::MAX),
            );
            let id = program.flow_identity(function_id).map_or_else(
                || {
                    function
                        .public_id
                        .and_then(|id| program.strings.get(id.index()).cloned())
                        .map_or_else(
                            || format!("function:{index}"),
                            |public_id| format!("public:{public_id}"),
                        )
                },
                |flow| format!("flow:{}", flow.canonical_label()),
            );
            let signature = program.signatures.get(function.signature.index());
            let frame_layout = program.frame_layouts.get(function.frame_layout.index());
            let blocks = awbc_function_blocks(program, function)?;
            Ok((
                id,
                AwbcFunctionFingerprint {
                    interface: serde_digest(&AwbcFunctionInterfaceFingerprint {
                        public_id: function
                            .public_id
                            .and_then(|id| program.strings.get(id.index())),
                        kind: function.kind,
                        signature,
                        frame_layout,
                        flags: function.flags,
                    })?,
                    body: serde_digest(&AwbcFunctionBodyFingerprint {
                        function,
                        signature,
                        frame_layout,
                        blocks,
                    })?,
                },
            ))
        })
        .collect()
}

#[derive(Serialize)]
struct AwbcFunctionInterfaceFingerprint<'a> {
    public_id: Option<&'a String>,
    kind: arcweft_core::awbc::schema::AwbcFunctionKind,
    signature: Option<&'a AwbcSignature>,
    frame_layout: Option<&'a AwbcFrameLayout>,
    flags: arcweft_core::awbc::schema::AwbcFunctionFlags,
}

#[derive(Serialize)]
struct AwbcFunctionBodyFingerprint<'a> {
    function: &'a AwbcFunction,
    signature: Option<&'a AwbcSignature>,
    frame_layout: Option<&'a AwbcFrameLayout>,
    blocks: Vec<AwbcFunctionBlockFingerprint<'a>>,
}

#[derive(Serialize)]
struct AwbcFunctionBlockFingerprint<'a> {
    block: &'a AwbcBlock,
    instructions: &'a [AwbcInstruction],
}

fn awbc_function_blocks<'a>(
    program: &'a AwbcProgram,
    function: &AwbcFunction,
) -> Result<Vec<AwbcFunctionBlockFingerprint<'a>>, PatchBundleError> {
    awbc_table_range_slice(&program.blocks, function.blocks, "blocks")?
        .iter()
        .map(|block| {
            Ok(AwbcFunctionBlockFingerprint {
                block,
                instructions: awbc_table_range_slice(
                    &program.instructions,
                    block.instructions,
                    "instructions",
                )?,
            })
        })
        .collect()
}

fn awbc_table_range_slice<'a, T>(
    table: &'a [T],
    range: AwbcTableRange,
    table_name: &'static str,
) -> Result<&'a [T], PatchBundleError> {
    let start = usize::try_from(range.start).map_err(|_| PatchBundleError::Compatibility {
        message: format!("AWBC {table_name} range start does not fit usize"),
    })?;
    let end =
        usize::try_from(
            range
                .checked_end()
                .ok_or_else(|| PatchBundleError::Compatibility {
                    message: format!("AWBC {table_name} range overflows u32"),
                })?,
        )
        .map_err(|_| PatchBundleError::Compatibility {
            message: format!("AWBC {table_name} range end does not fit usize"),
        })?;
    table
        .get(start..end)
        .ok_or_else(|| PatchBundleError::Compatibility {
            message: format!(
                "AWBC {table_name} range {start}..{end} exceeds table length {}",
                table.len()
            ),
        })
}

fn serde_digest(value: &impl Serialize) -> Result<BundleDigest, PatchBundleError> {
    serde_json::to_vec(value)
        .map(|bytes| BundleDigest::of(&bytes))
        .map_err(PatchBundleError::EncodePayload)
}

fn logical_descriptor_matches(left: &SectionDescriptor, right: &SectionDescriptor) -> bool {
    left.id() == right.id()
        && left.kind_code() == right.kind_code()
        && left.schema_version() == right.schema_version()
        && left.residency() == right.residency()
        && left.placement() == right.placement()
        && left.compression() == right.compression()
        && left.decoded_size() == right.decoded_size()
        && left.content_digest() == right.content_digest()
        && left.required() == right.required()
}

fn patch_payload_carrier_matches(carrier: &SectionDescriptor, logical: &SectionDescriptor) -> bool {
    carrier.id() == logical.id()
        && carrier.known_kind() == Some(PATCH_PAYLOAD_CARRIER_KIND)
        && carrier.schema_version() == logical.schema_version()
        && carrier.residency() == logical.residency()
        && carrier.placement() == logical.placement()
        && carrier.compression() == logical.compression()
        && carrier.decoded_size() == logical.decoded_size()
        && carrier.content_digest() == logical.content_digest()
        && carrier.required() == logical.required()
}

fn patch_plan_section_id() -> SectionId {
    let mut id = [0_u8; 16];
    id[..4].copy_from_slice(&BundleSectionKind::PatchPlan.encoded().to_le_bytes());
    id[4..].copy_from_slice(&BundleDigest::of(b"arcweft-awfb-v1-patch-plan").as_bytes()[..12]);
    SectionId::from_bytes(id)
}

fn section_index(sections: &[SectionDescriptor]) -> BTreeMap<SectionId, &SectionDescriptor> {
    sections
        .iter()
        .map(|section| (section.id(), section))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::awbc::schema::{
        AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget, AwbcFlowBinding,
        AwbcFlowExecutable, AwbcFrameLayoutId, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind,
        AwbcSafePointKind, AwbcSignatureId, AwbcStringId, AwbcTerminator,
    };
    use arcweft_core::entry::{
        EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
    };
    use arcweft_core::plan::{EntryRuntimeId, FlowRuntimeId};

    fn same_label_flow_program() -> AwbcProgram {
        let first_flow = FlowRuntimeId::from_checked_declaration_digest([0x91; 32], "flow.main")
            .expect("first checked Flow identity");
        let second_flow = FlowRuntimeId::from_checked_declaration_digest([0x92; 32], "flow.main")
            .expect("second checked Flow identity");
        AwbcProgram {
            strings: vec![
                "entry.main".to_owned(),
                "entry.second".to_owned(),
                "flow.main".to_owned(),
            ],
            signatures: vec![AwbcSignature {
                params: Vec::new(),
                result: None,
                effects: AwbcEffectSetId(0),
            }],
            frame_layouts: vec![AwbcFrameLayout {
                slots: Vec::new(),
                max_scope_depth: 0,
            }],
            functions: vec![
                AwbcFunction {
                    public_id: Some(AwbcStringId(2)),
                    kind: AwbcFunctionKind::Flow,
                    signature: AwbcSignatureId(0),
                    frame_layout: AwbcFrameLayoutId(0),
                    blocks: AwbcTableRange::new(0, 1),
                    entry_block: AwbcBlockId(0),
                    flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
                },
                AwbcFunction {
                    public_id: Some(AwbcStringId(2)),
                    kind: AwbcFunctionKind::Flow,
                    signature: AwbcSignatureId(0),
                    frame_layout: AwbcFrameLayoutId(0),
                    blocks: AwbcTableRange::new(1, 1),
                    entry_block: AwbcBlockId(1),
                    flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
                },
            ],
            blocks: vec![
                AwbcBlock {
                    owner: AwbcFunctionId(0),
                    instructions: AwbcTableRange::new(0, 0),
                    terminator: AwbcTerminator::Return { value: None },
                    safe_point: AwbcSafePointKind::FlowEntry,
                    source_map: None,
                },
                AwbcBlock {
                    owner: AwbcFunctionId(1),
                    instructions: AwbcTableRange::new(0, 0),
                    terminator: AwbcTerminator::Return { value: None },
                    safe_point: AwbcSafePointKind::FlowEntry,
                    source_map: None,
                },
            ],
            flow_bindings: vec![
                AwbcFlowBinding {
                    flow: first_flow.clone(),
                    function: AwbcFunctionId(0),
                },
                AwbcFlowBinding {
                    flow: second_flow.clone(),
                    function: AwbcFunctionId(1),
                },
            ],
            flow_executables: vec![
                AwbcFlowExecutable {
                    metadata: RuntimeFlowExecutable {
                        flow: first_flow,
                        contract: FlowContractHash::from_bytes([0xa1; 32]),
                        controller: None,
                    },
                    function: AwbcFunctionId(0),
                },
                AwbcFlowExecutable {
                    metadata: RuntimeFlowExecutable {
                        flow: second_flow,
                        contract: FlowContractHash::from_bytes([0xa2; 32]),
                        controller: None,
                    },
                    function: AwbcFunctionId(1),
                },
            ],
            entries: vec![
                AwbcEntry {
                    runtime_id: EntryRuntimeId::from_source_entity_body("entry.main")
                        .expect("test entry identity"),
                    binding: EntryBindingIdentity::from_bytes([1; 32]),
                    public_id: AwbcStringId(0),
                    kind: AwbcEntryKind::Cli,
                    target: AwbcEntryTarget::Function {
                        function: AwbcFunctionId(0),
                    },
                    roles: RuntimeEntryRoles::None,
                },
                AwbcEntry {
                    runtime_id: EntryRuntimeId::from_source_entity_body("entry.second")
                        .expect("second test entry identity"),
                    binding: EntryBindingIdentity::from_bytes([2; 32]),
                    public_id: AwbcStringId(1),
                    kind: AwbcEntryKind::Cli,
                    target: AwbcEntryTarget::Function {
                        function: AwbcFunctionId(1),
                    },
                    roles: RuntimeEntryRoles::None,
                },
            ],
            ..AwbcProgram::default()
        }
    }

    #[test]
    fn awbc_patch_fingerprints_keep_same_label_checked_flows_distinct() {
        let program = same_label_flow_program();
        let fingerprints = awbc_function_fingerprints(&program).expect("fingerprints build");
        let first = format!("flow:{}", program.flow_bindings[0].flow.canonical_label());
        let second = format!("flow:{}", program.flow_bindings[1].flow.canonical_label());

        assert_eq!(fingerprints.len(), 2);
        assert!(fingerprints.contains_key(&first));
        assert!(fingerprints.contains_key(&second));
        assert!(!fingerprints.contains_key("public:flow.main"));
    }

    #[test]
    fn awbc_patch_classifies_each_same_label_flow_independently() {
        let base = same_label_flow_program();
        let base_bytes = base.encode_canonical().expect("base AWBC encodes");

        let mut body_change = base.clone();
        body_change.blocks[1].terminator = AwbcTerminator::Trap {
            code: arcweft_core::awbc::schema::AwbcTrapCode::ExplicitPanic,
            message: None,
        };
        assert_eq!(
            awbc_executable_compatibility(
                &base_bytes,
                &body_change.encode_canonical().expect("body AWBC encodes")
            )
            .expect("body compatibility"),
            PatchCompatibility::CodeCompatible
        );

        let mut interface_change = base.clone();
        interface_change.functions[1].flags =
            AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC | AwbcFunctionFlags::MAY_SUSPEND);
        assert_eq!(
            awbc_executable_compatibility(
                &base_bytes,
                &interface_change
                    .encode_canonical()
                    .expect("interface AWBC encodes")
            )
            .expect("interface compatibility"),
            PatchCompatibility::CodeGenerational
        );

        let mut removed = base;
        removed.functions.pop();
        removed.blocks.pop();
        removed.flow_bindings.pop();
        removed.flow_executables.pop();
        removed.entries.pop();
        assert_eq!(
            awbc_executable_compatibility(
                &base_bytes,
                &removed.encode_canonical().expect("removed AWBC encodes")
            )
            .expect("removal compatibility"),
            PatchCompatibility::RestartRequired
        );
    }
}
