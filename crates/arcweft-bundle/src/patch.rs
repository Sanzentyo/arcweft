//! Sans I/O AWFB section patch model.

use crate::container::{
    BundleDigest, BundleKind, BundleSectionKind, BundleView, Compression, ContentPlacement,
    ContentResidency, ReadBudget, SectionDescriptor, SectionId, SectionInput, encode_bundle,
};
use crate::release::{ReleaseManifestError, ReleaseSignaturePolicy};
use arcweft_core::bytecode::BYTECODE_ABI_VERSION;
use std::collections::BTreeMap;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchCompatibility {
    ContentOnly,
    CodeCompatible,
    CodeGenerational,
    RestartRequired,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct BundlePatchManifest {
    pub schema_version: u32,
    pub base_content_root: BundleDigest,
    pub target_content_root: BundleDigest,
    pub runtime_abi: RuntimeAbiRange,
    pub compatibility: PatchCompatibility,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct BundlePatchArtifact {
    pub manifest: BundlePatchManifest,
    pub plan: BundlePatchPlan,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_sections: Vec<PatchSectionPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PatchSectionPayload {
    pub descriptor: SectionDescriptor,
    pub bytes: Vec<u8>,
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
    #[error("invalid AWFB patch container: {0}")]
    Container(#[source] crate::container::ContainerError),
    #[error("AWFB patch signature policy failed: {0}")]
    SignaturePolicy(#[source] ReleaseManifestError),
    #[error("AWFB bundle kind {actual:?} is not a patch bundle")]
    WrongBundleKind { actual: BundleKind },
    #[error("AWFB patch bundle is missing its PatchPlan section")]
    MissingPatchPlan,
    #[error("AWFB patch bundle contains more than one PatchPlan section")]
    DuplicatePatchPlan,
    #[error("AWFB patch manifest and PatchPlan section disagree about content roots")]
    ContentRootMismatch,
    #[error("AWFB patch runtime ABI range {min}..={max} does not include current ABI {current}")]
    UnsupportedRuntimeAbi { min: u32, max: u32, current: u32 },
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
    #[error("AWFB patch materialization currently requires embedded base section {0}")]
    ExternalBaseSection(SectionId),
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
        min: BYTECODE_ABI_VERSION,
        max: BYTECODE_ABI_VERSION,
    };

    pub const fn contains(self, abi: u32) -> bool {
        self.min <= abi && abi <= self.max
    }
}

impl PatchCompatibility {
    pub fn conservative_for_plan(plan: &BundlePatchPlan) -> Self {
        plan.operations
            .iter()
            .map(operation_compatibility)
            .max_by_key(|compatibility| compatibility.rank())
            .unwrap_or(Self::ContentOnly)
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

    const fn rank(self) -> u8 {
        match self {
            Self::ContentOnly => 0,
            Self::CodeCompatible => 1,
            Self::CodeGenerational => 2,
            Self::RestartRequired => 3,
        }
    }
}

impl BundlePatchManifest {
    pub fn for_plan(plan: &BundlePatchPlan) -> Self {
        Self {
            schema_version: PATCH_PLAN_SCHEMA_VERSION,
            base_content_root: plan.base_content_root,
            target_content_root: plan.target_content_root,
            runtime_abi: RuntimeAbiRange::CURRENT,
            compatibility: PatchCompatibility::conservative_for_plan(plan),
        }
    }
}

impl BundlePatchArtifact {
    pub fn new(plan: BundlePatchPlan) -> Self {
        Self {
            manifest: BundlePatchManifest::for_plan(&plan),
            plan,
            changed_sections: Vec::new(),
        }
    }

    pub fn from_views(
        base: &BundleView<'_>,
        target: &BundleView<'_>,
    ) -> Result<Self, PatchBundleError> {
        let plan = BundlePatchPlan::diff(base, target);
        let changed_sections = plan
            .operations
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
            .collect::<Result<Vec<_>, _>>()?;
        let artifact = Self {
            manifest: BundlePatchManifest::for_plan(&plan),
            plan,
            changed_sections,
        };
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<(), PatchBundleError> {
        if self.manifest.base_content_root != self.plan.base_content_root
            || self.manifest.target_content_root != self.plan.target_content_root
        {
            return Err(PatchBundleError::ContentRootMismatch);
        }
        if !self.manifest.runtime_abi.contains(BYTECODE_ABI_VERSION) {
            return Err(PatchBundleError::UnsupportedRuntimeAbi {
                min: self.manifest.runtime_abi.min,
                max: self.manifest.runtime_abi.max,
                current: BYTECODE_ABI_VERSION,
            });
        }
        self.validate_changed_section_payloads()?;
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

pub fn encode_patch_bundle(artifact: &BundlePatchArtifact) -> Result<Vec<u8>, PatchBundleError> {
    artifact.validate()?;
    let manifest =
        serde_json::to_vec(&artifact.manifest).map_err(PatchBundleError::EncodePayload)?;
    let plan = serde_json::to_vec(&artifact.plan).map_err(PatchBundleError::EncodePayload)?;
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
    let manifest =
        serde_json::from_slice(view.manifest()).map_err(PatchBundleError::DecodePayload)?;
    let plan = decode_patch_plan_section(&view)?;
    let changed_sections = decode_changed_sections(&view, &plan)?;
    let artifact = BundlePatchArtifact {
        manifest,
        plan,
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
) -> Result<Vec<u8>, PatchBundleError> {
    let artifact = decode_patch_bundle(patch_bytes)?;
    apply_patch_bundle(base_bytes, &artifact)
}

pub fn apply_signed_patch_bundle_bytes(
    base_bytes: &[u8],
    patch_bytes: &[u8],
    signature_policy: &ReleaseSignaturePolicy,
) -> Result<Vec<u8>, PatchBundleError> {
    let artifact = decode_patch_bundle_with_signature_policy(patch_bytes, signature_policy)?;
    apply_patch_bundle(base_bytes, &artifact)
}

pub fn apply_patch_bundle(
    base_bytes: &[u8],
    artifact: &BundlePatchArtifact,
) -> Result<Vec<u8>, PatchBundleError> {
    artifact.validate()?;
    let base = BundleView::parse(base_bytes, ReadBudget::default())
        .map_err(PatchBundleError::Container)?;
    artifact
        .plan
        .validate_base(base.content_root())
        .map_err(|error| match error {
            PatchValidationError::WrongBase { active, expected } => {
                PatchBundleError::WrongBase { active, expected }
            }
        })?;

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
            None => sections.push(section_input_from_base(&base, descriptor)?),
        }
    }

    let base_section_ids = base_by_id
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
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

    let target = encode_bundle(base.kind(), base.manifest(), sections)
        .map_err(PatchBundleError::Container)?;
    let target_view =
        BundleView::parse(&target, ReadBudget::default()).map_err(PatchBundleError::Container)?;
    let actual = target_view.content_root();
    if actual != artifact.plan.target_content_root {
        return Err(PatchBundleError::TargetContentRootMismatch {
            expected: artifact.plan.target_content_root,
            actual,
        });
    }
    Ok(target)
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
            Some(old) if old.content_digest() != next.content_digest() => {
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

fn operation_id(operation: &SectionOperation) -> SectionId {
    match operation {
        SectionOperation::Add(descriptor) => descriptor.id(),
        SectionOperation::Replace { next, .. } => next.id(),
        SectionOperation::Remove { id, .. } => *id,
    }
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
    section_input_from_payload_as_kind(payload, payload.descriptor.kind())
}

fn section_input_from_payload_carrier(
    payload: &PatchSectionPayload,
) -> Result<SectionInput, PatchBundleError> {
    section_input_from_payload_as_kind(payload, PATCH_PAYLOAD_CARRIER_KIND)
}

fn section_input_from_payload_as_kind(
    payload: &PatchSectionPayload,
    kind: BundleSectionKind,
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
    section_input_for_descriptor_as_kind(descriptor, descriptor.kind(), decoded_bytes)
}

fn section_input_for_descriptor_as_kind(
    descriptor: &SectionDescriptor,
    kind: BundleSectionKind,
    decoded_bytes: impl Into<Vec<u8>>,
) -> Result<SectionInput, PatchBundleError> {
    let decoded_bytes = decoded_bytes.into();
    match descriptor.compression() {
        Compression::None => Ok(SectionInput::embedded(
            descriptor.id(),
            kind,
            descriptor.schema_version(),
            descriptor.residency(),
            descriptor.required(),
            decoded_bytes,
        )),
        Compression::Zstd => SectionInput::embedded_zstd(
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
    SectionInput::external_ref(
        descriptor.id(),
        descriptor.kind(),
        descriptor.schema_version(),
        descriptor.residency(),
        descriptor.required(),
        descriptor.decoded_size(),
        descriptor.content_digest(),
    )
}

fn decode_patch_plan_section(view: &BundleView<'_>) -> Result<BundlePatchPlan, PatchBundleError> {
    let mut matches = view
        .sections()
        .iter()
        .filter(|descriptor| descriptor.kind() == BundleSectionKind::PatchPlan);
    let Some(descriptor) = matches.next() else {
        return Err(PatchBundleError::MissingPatchPlan);
    };
    if matches.next().is_some() {
        return Err(PatchBundleError::DuplicatePatchPlan);
    }
    let Some(bytes) = view
        .embedded_section(descriptor.id())
        .map_err(PatchBundleError::Container)?
    else {
        return Err(PatchBundleError::MissingPatchPlan);
    };
    serde_json::from_slice(bytes).map_err(PatchBundleError::DecodePayload)
}

fn decode_changed_sections(
    view: &BundleView<'_>,
    plan: &BundlePatchPlan,
) -> Result<Vec<PatchSectionPayload>, PatchBundleError> {
    let expected = expected_changed_sections(plan);
    view.sections()
        .iter()
        .filter(|descriptor| descriptor.kind() != BundleSectionKind::PatchPlan)
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

fn operation_compatibility(operation: &SectionOperation) -> PatchCompatibility {
    match operation {
        SectionOperation::Add(descriptor) => section_compatibility(descriptor.kind()),
        SectionOperation::Replace { next, .. } => section_compatibility(next.kind()),
        SectionOperation::Remove { .. } => PatchCompatibility::RestartRequired,
    }
}

fn section_compatibility(kind: BundleSectionKind) -> PatchCompatibility {
    match kind {
        BundleSectionKind::AdapterRequirements
        | BundleSectionKind::RuntimeTypes
        | BundleSectionKind::Entrypoints
        | BundleSectionKind::PatchPlan => PatchCompatibility::RestartRequired,
        BundleSectionKind::ProgramBytecode | BundleSectionKind::HotSwapMap => {
            PatchCompatibility::CodeGenerational
        }
        BundleSectionKind::ContentCatalog
        | BundleSectionKind::DisplayCatalog
        | BundleSectionKind::AudioGraph
        | BundleSectionKind::AssetCatalog
        | BundleSectionKind::AssetBlob
        | BundleSectionKind::LocaleCatalog
        | BundleSectionKind::SourceMap
        | BundleSectionKind::DebugSymbols
        | BundleSectionKind::NormalizedSource => PatchCompatibility::ContentOnly,
    }
}

fn logical_descriptor_matches(left: &SectionDescriptor, right: &SectionDescriptor) -> bool {
    left.id() == right.id()
        && left.kind() == right.kind()
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
        && carrier.kind() == PATCH_PAYLOAD_CARRIER_KIND
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
    use crate::container::{
        BundleKind, BundleSectionKind, BundleView, Compression, ContentResidency, ReadBudget,
        SectionInput, encode_bundle,
    };
    use crate::release::{
        RELEASE_SIGNATURE_ALGORITHM_ED25519_V1, ReleaseSignatureEnvelope, ReleaseTrustedPublicKey,
    };
    use ed25519_dalek::Signer as _;

    fn content_pack(asset_blob: &'static [u8], include_catalog: bool) -> Vec<u8> {
        let mut sections = vec![SectionInput::embedded(
            SectionId::from_bytes([1; 16]),
            BundleSectionKind::AssetBlob,
            1,
            ContentResidency::OnDemand,
            false,
            asset_blob,
        )];
        if include_catalog {
            sections.push(SectionInput::embedded(
                SectionId::from_bytes([2; 16]),
                BundleSectionKind::ContentCatalog,
                1,
                ContentResidency::Startup,
                false,
                b"catalog",
            ));
        }
        encode_bundle(BundleKind::ContentPack, br#"{"kind":"content"}"#, sections)
            .expect("content pack encodes")
    }

    fn content_patch_pack(asset_blob: &'static [u8]) -> Vec<u8> {
        encode_bundle(
            BundleKind::ContentPack,
            br#"{"kind":"content"}"#,
            vec![SectionInput::embedded(
                SectionId::from_bytes([1; 16]),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                asset_blob,
            )],
        )
        .expect("content pack encodes")
    }

    fn external_content_pack(asset_blob: &'static [u8]) -> Vec<u8> {
        encode_bundle(
            BundleKind::ContentPack,
            br#"{"kind":"content"}"#,
            vec![SectionInput::external_ref(
                SectionId::from_bytes([1; 16]),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                u64::try_from(asset_blob.len()).expect("fixture length fits in u64"),
                BundleDigest::of(asset_blob),
            )],
        )
        .expect("external content pack encodes")
    }

    fn zstd_content_pack(asset_blob: &'static [u8]) -> Vec<u8> {
        encode_bundle(
            BundleKind::ContentPack,
            br#"{"kind":"content"}"#,
            vec![
                SectionInput::embedded_zstd(
                    SectionId::from_bytes([1; 16]),
                    BundleSectionKind::AssetBlob,
                    1,
                    ContentResidency::OnDemand,
                    false,
                    asset_blob,
                )
                .expect("zstd section encodes"),
            ],
        )
        .expect("zstd content pack encodes")
    }

    fn program_patch_pack(bytecode: &'static [u8], adapter_requirements: &'static [u8]) -> Vec<u8> {
        encode_bundle(
            BundleKind::Program,
            br#"{"kind":"program"}"#,
            vec![
                SectionInput::embedded(
                    SectionId::from_bytes([1; 16]),
                    BundleSectionKind::ProgramBytecode,
                    1,
                    ContentResidency::Startup,
                    true,
                    bytecode,
                ),
                SectionInput::embedded(
                    SectionId::from_bytes([2; 16]),
                    BundleSectionKind::RuntimeTypes,
                    1,
                    ContentResidency::Startup,
                    true,
                    b"runtime-types",
                ),
                SectionInput::embedded(
                    SectionId::from_bytes([3; 16]),
                    BundleSectionKind::Entrypoints,
                    1,
                    ContentResidency::Startup,
                    true,
                    b"entrypoints",
                ),
                SectionInput::embedded(
                    SectionId::from_bytes([4; 16]),
                    BundleSectionKind::AdapterRequirements,
                    1,
                    ContentResidency::Startup,
                    true,
                    adapter_requirements,
                ),
                SectionInput::embedded(
                    SectionId::from_bytes([5; 16]),
                    BundleSectionKind::ContentCatalog,
                    1,
                    ContentResidency::Startup,
                    true,
                    b"content-catalog",
                ),
            ],
        )
        .expect("program bundle encodes")
    }

    #[test]
    fn patch_diff_is_stable_and_detects_add_replace_remove() {
        let base = content_pack(b"old", true);
        let target = content_pack(b"new", false);
        let base = BundleView::parse(&base, ReadBudget::default()).expect("base parses");
        let target = BundleView::parse(&target, ReadBudget::default()).expect("target parses");

        let plan = BundlePatchPlan::diff(&base, &target);

        assert_eq!(plan.operations.len(), 2);
        assert!(matches!(
            plan.operations[0],
            SectionOperation::Replace { .. }
        ));
        assert!(matches!(
            plan.operations[1],
            SectionOperation::Remove { .. }
        ));
        plan.validate_base(base.content_root())
            .expect("active base matches");
        assert_eq!(
            plan.validate_base(target.content_root()),
            Err(PatchValidationError::WrongBase {
                active: target.content_root(),
                expected: base.content_root(),
            })
        );
    }

    #[test]
    fn patch_bundle_requires_patch_plan_section() {
        let error = encode_bundle(BundleKind::Patch, br#"{"kind":"patch"}"#, Vec::new())
            .expect_err("patch bundles require a patch plan");
        assert_eq!(
            error,
            crate::container::ContainerError::MissingRequiredSection(BundleSectionKind::PatchPlan)
        );
    }

    #[test]
    fn patch_bundle_round_trips_patch_plan_section() {
        let base = content_pack(b"old", true);
        let target = content_pack(b"new", false);
        let base = BundleView::parse(&base, ReadBudget::default()).expect("base parses");
        let target = BundleView::parse(&target, ReadBudget::default()).expect("target parses");
        let artifact = BundlePatchArtifact::from_views(&base, &target).expect("patch artifact");

        let bytes = encode_patch_bundle(&artifact).expect("patch encodes");
        let decoded = decode_patch_bundle(&bytes).expect("patch decodes");

        assert_eq!(decoded, artifact);
        decoded
            .plan
            .validate_base(base.content_root())
            .expect("patch base matches");
    }

    #[test]
    fn patch_awfb_uses_asset_blob_carriers_for_changed_payloads() {
        let base = program_patch_pack(b"bytecode-old", b"adapter");
        let target = program_patch_pack(b"bytecode-new", b"adapter");
        let base = BundleView::parse(&base, ReadBudget::default()).expect("base parses");
        let target = BundleView::parse(&target, ReadBudget::default()).expect("target parses");
        let artifact = BundlePatchArtifact::from_views(&base, &target).expect("patch artifact");

        let bytes = encode_patch_bundle(&artifact).expect("patch encodes");
        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("patch AWFB parses");
        let payload_sections = view
            .sections()
            .iter()
            .filter(|section| section.kind() != BundleSectionKind::PatchPlan)
            .collect::<Vec<_>>();
        let decoded = decode_patch_bundle(&bytes).expect("patch decodes");

        assert_eq!(payload_sections.len(), 1);
        assert_eq!(payload_sections[0].kind(), BundleSectionKind::AssetBlob);
        assert_eq!(
            decoded.changed_sections[0].descriptor.kind(),
            BundleSectionKind::ProgramBytecode
        );
        assert_eq!(decoded, artifact);
    }

    #[test]
    fn patch_manifest_records_conservative_content_only_compatibility() {
        let base = content_patch_pack(b"old");
        let target = content_patch_pack(b"new");
        let base = BundleView::parse(&base, ReadBudget::default()).expect("base parses");
        let target = BundleView::parse(&target, ReadBudget::default()).expect("target parses");
        let artifact = BundlePatchArtifact::from_views(&base, &target).expect("patch artifact");

        assert_eq!(
            artifact.manifest.compatibility,
            PatchCompatibility::ContentOnly
        );
        assert_eq!(
            PatchCompatibility::conservative_for_plan(&artifact.plan).label(),
            "content-only"
        );
    }

    #[test]
    fn patch_manifest_records_conservative_code_generational_compatibility() {
        let base = program_patch_pack(b"bytecode-old", b"adapter");
        let target = program_patch_pack(b"bytecode-new", b"adapter");
        let base = BundleView::parse(&base, ReadBudget::default()).expect("base parses");
        let target = BundleView::parse(&target, ReadBudget::default()).expect("target parses");
        let artifact = BundlePatchArtifact::from_views(&base, &target).expect("patch artifact");

        assert_eq!(
            artifact.manifest.compatibility,
            PatchCompatibility::CodeGenerational
        );
        assert!(artifact.manifest.compatibility.keeps_old_generation());
    }

    #[test]
    fn patch_manifest_records_conservative_restart_required_compatibility() {
        let base = program_patch_pack(b"bytecode", b"adapter-old");
        let target = program_patch_pack(b"bytecode", b"adapter-new");
        let base = BundleView::parse(&base, ReadBudget::default()).expect("base parses");
        let target = BundleView::parse(&target, ReadBudget::default()).expect("target parses");
        let artifact = BundlePatchArtifact::from_views(&base, &target).expect("patch artifact");

        assert_eq!(
            artifact.manifest.compatibility,
            PatchCompatibility::RestartRequired
        );
        assert!(!artifact.manifest.compatibility.can_apply_live());
    }

    #[test]
    fn patch_bundle_rejects_root_mismatch_between_manifest_and_plan() {
        let base = content_pack(b"old", true);
        let target = content_pack(b"new", false);
        let base = BundleView::parse(&base, ReadBudget::default()).expect("base parses");
        let target = BundleView::parse(&target, ReadBudget::default()).expect("target parses");
        let mut artifact = BundlePatchArtifact::new(BundlePatchPlan::diff(&base, &target));
        artifact.manifest.target_content_root = base.content_root();

        let error = encode_patch_bundle(&artifact).expect_err("mismatched roots reject");

        assert!(matches!(error, PatchBundleError::ContentRootMismatch));
    }

    #[test]
    fn patch_bundle_requires_payload_for_embedded_add_or_replace() {
        let base = content_pack(b"old", true);
        let target = content_pack(b"new", false);
        let base = BundleView::parse(&base, ReadBudget::default()).expect("base parses");
        let target = BundleView::parse(&target, ReadBudget::default()).expect("target parses");
        let artifact = BundlePatchArtifact::new(BundlePatchPlan::diff(&base, &target));

        let error = encode_patch_bundle(&artifact).expect_err("missing payload rejects");

        assert!(matches!(error, PatchBundleError::MissingSectionPayload(_)));
    }

    #[test]
    fn patch_bundle_rejects_payload_digest_mismatch() {
        let base = content_pack(b"old", true);
        let target = content_pack(b"new", false);
        let base = BundleView::parse(&base, ReadBudget::default()).expect("base parses");
        let target = BundleView::parse(&target, ReadBudget::default()).expect("target parses");
        let mut artifact = BundlePatchArtifact::from_views(&base, &target).expect("patch artifact");
        artifact.changed_sections[0].bytes = b"corrupt".to_vec();

        let error = encode_patch_bundle(&artifact).expect_err("digest mismatch rejects");

        assert!(matches!(
            error,
            PatchBundleError::PayloadDigestMismatch { .. }
        ));
    }

    #[test]
    fn patch_materializes_target_awfb_from_embedded_section_payloads() {
        let base_bytes = content_pack(b"old", true);
        let target_bytes = content_pack(b"new", false);
        let base = BundleView::parse(&base_bytes, ReadBudget::default()).expect("base parses");
        let target =
            BundleView::parse(&target_bytes, ReadBudget::default()).expect("target parses");
        let artifact = BundlePatchArtifact::from_views(&base, &target).expect("patch artifact");

        let patched = apply_patch_bundle(&base_bytes, &artifact).expect("patch applies");
        let patched = BundleView::parse(&patched, ReadBudget::default()).expect("patched parses");

        assert_eq!(patched.kind(), target.kind());
        assert_eq!(patched.manifest(), target.manifest());
        assert_eq!(patched.content_root(), target.content_root());
        assert_eq!(patched.sections().len(), target.sections().len());
    }

    #[test]
    fn patch_materializes_target_awfb_from_encoded_patch_bundle() {
        let base_bytes = content_pack(b"old", true);
        let target_bytes = content_pack(b"new", false);
        let base = BundleView::parse(&base_bytes, ReadBudget::default()).expect("base parses");
        let target =
            BundleView::parse(&target_bytes, ReadBudget::default()).expect("target parses");
        let artifact = BundlePatchArtifact::from_views(&base, &target).expect("patch artifact");
        let patch_bytes = encode_patch_bundle(&artifact).expect("patch encodes");

        let patched =
            apply_patch_bundle_bytes(&base_bytes, &patch_bytes).expect("encoded patch applies");
        let patched = BundleView::parse(&patched, ReadBudget::default()).expect("patched parses");

        assert_eq!(patched.content_root(), target.content_root());
    }

    #[test]
    fn signed_patch_bundle_decodes_and_applies_with_signature_policy() {
        let base_bytes = content_pack(b"old", true);
        let target_bytes = content_pack(b"new", false);
        let base = BundleView::parse(&base_bytes, ReadBudget::default()).expect("base parses");
        let target =
            BundleView::parse(&target_bytes, ReadBudget::default()).expect("target parses");
        let artifact = BundlePatchArtifact::from_views(&base, &target).expect("patch artifact");
        let patch_bytes = encode_patch_bundle(&artifact).expect("patch encodes");
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[11; 32]);
        let policy = ReleaseSignaturePolicy::require_trusted_public_keys(
            Some(64),
            [ReleaseTrustedPublicKey::ed25519_v1(
                "patch-key-main",
                encode_hex(&signing_key.verifying_key().to_bytes()),
            )
            .expect("trusted public key")],
        )
        .expect("signature policy");
        let signed_patch = sign_patch_bundle(patch_bytes.clone(), "patch-key-main", &signing_key);

        let decoded = decode_patch_bundle_with_signature_policy(&signed_patch, &policy)
            .expect("signed patch decodes");
        let patched = apply_signed_patch_bundle_bytes(&base_bytes, &signed_patch, &policy)
            .expect("signed patch applies");
        let patched = BundleView::parse(&patched, ReadBudget::default()).expect("patched parses");

        assert_eq!(decoded, artifact);
        assert_eq!(patched.content_root(), target.content_root());
        assert!(matches!(
            decode_patch_bundle_with_signature_policy(&patch_bytes, &policy),
            Err(PatchBundleError::SignaturePolicy(
                ReleaseManifestError::MissingAwfbSignature { .. }
            ))
        ));
    }

    #[test]
    fn patch_materializes_target_awfb_preserving_zstd_section_compression() {
        let base_bytes = zstd_content_pack(b"old compressed asset bytes");
        let target_bytes = zstd_content_pack(b"new compressed asset bytes");
        let base = BundleView::parse(&base_bytes, ReadBudget::default()).expect("base parses");
        let target =
            BundleView::parse(&target_bytes, ReadBudget::default()).expect("target parses");
        let artifact = BundlePatchArtifact::from_views(&base, &target).expect("patch artifact");
        let patch_bytes = encode_patch_bundle(&artifact).expect("patch encodes");

        let patched =
            apply_patch_bundle_bytes(&base_bytes, &patch_bytes).expect("encoded patch applies");
        let patched = BundleView::parse(&patched, ReadBudget::default()).expect("patched parses");
        let patched_section = patched
            .sections()
            .iter()
            .find(|section| section.kind() == BundleSectionKind::AssetBlob)
            .expect("asset blob exists");

        assert_eq!(patched.content_root(), target.content_root());
        assert_eq!(patched_section.compression(), Compression::Zstd);
    }

    #[test]
    fn patch_materializes_external_section_descriptor_changes_without_payloads() {
        let base_bytes = external_content_pack(b"old external bytes");
        let target_bytes = external_content_pack(b"new external bytes");
        let base = BundleView::parse(&base_bytes, ReadBudget::default()).expect("base parses");
        let target =
            BundleView::parse(&target_bytes, ReadBudget::default()).expect("target parses");
        let artifact = BundlePatchArtifact::from_views(&base, &target).expect("patch artifact");

        assert!(artifact.changed_sections.is_empty());

        let patched = apply_patch_bundle(&base_bytes, &artifact).expect("patch applies");
        let patched = BundleView::parse(&patched, ReadBudget::default()).expect("patched parses");
        let patched_section = patched
            .sections()
            .iter()
            .find(|section| section.kind() == BundleSectionKind::AssetBlob)
            .expect("external asset descriptor exists");

        assert_eq!(patched.content_root(), target.content_root());
        assert_eq!(patched_section.placement(), ContentPlacement::External);
        assert_eq!(
            patched_section.content_digest(),
            BundleDigest::of(b"new external bytes")
        );
        assert!(
            patched
                .embedded_section(patched_section.id())
                .expect("slice")
                .is_none()
        );
    }

    #[test]
    fn patch_materialization_rejects_wrong_base_root() {
        let base_bytes = content_pack(b"old", true);
        let other_base_bytes = content_pack(b"other", true);
        let target_bytes = content_pack(b"new", false);
        let other_base =
            BundleView::parse(&other_base_bytes, ReadBudget::default()).expect("other base parses");
        let target =
            BundleView::parse(&target_bytes, ReadBudget::default()).expect("target parses");
        let artifact =
            BundlePatchArtifact::from_views(&other_base, &target).expect("patch artifact");

        let error = apply_patch_bundle(&base_bytes, &artifact).expect_err("wrong base rejects");

        assert!(matches!(error, PatchBundleError::WrongBase { .. }));
    }

    fn sign_patch_bundle(
        patch_bytes: Vec<u8>,
        signer_id: &str,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Vec<u8> {
        let view = BundleView::parse(&patch_bytes, ReadBudget::default()).expect("patch parses");
        let signing_digest = view.signing_digest().expect("signing digest");
        let mut envelope = ReleaseSignatureEnvelope::new(
            signer_id,
            RELEASE_SIGNATURE_ALGORITHM_ED25519_V1,
            view.content_root(),
            view.kind(),
            signing_digest,
            encode_hex(&[0; 64]),
        )
        .expect("signature envelope");
        let signature = signing_key.sign(&envelope.signing_message());
        envelope.signature = encode_hex(&signature.to_bytes());
        append_signature_block(
            patch_bytes,
            &envelope.to_json_bytes().expect("envelope encodes"),
        )
    }

    fn append_signature_block(mut bytes: Vec<u8>, signature: &[u8]) -> Vec<u8> {
        let signature_offset = bytes.len();
        bytes.extend_from_slice(signature);
        write_u64(&mut bytes, 56, signature_offset as u64);
        write_u64(&mut bytes, 64, signature.len() as u64);
        let file_len = bytes.len() as u64;
        write_u64(&mut bytes, 72, file_len);
        bytes
    }

    fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn encode_hex(bytes: &[u8]) -> String {
        bytes
            .iter()
            .fold(String::with_capacity(bytes.len() * 2), |mut hex, byte| {
                use std::fmt::Write as _;
                write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
                hex
            })
    }
}
