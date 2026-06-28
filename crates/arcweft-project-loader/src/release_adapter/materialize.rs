use arcweft_bundle::{
    container::{BundleView, ContentPlacement, ContentResidency, ReadBudget, SectionDescriptor, SectionId},
    patch::{
        BundlePatchArtifact, PatchBundleError, PatchMaterializedTarget, SectionOperation,
        apply_patch_bundle, decode_patch_bundle,
    },
    release::archive::ExternalPayloadMaterializationMode,
};
use std::collections::BTreeMap;
use thiserror::Error;

pub fn materialize_patch_target_with_external_payload_mode(
    base_bytes: &[u8],
    patch_bytes: &[u8],
    payload_mode: ExternalPayloadMaterializationMode,
) -> Result<PatchMaterializedTarget, ReleaseTargetMaterializationError> {
    let artifact = decode_patch_bundle(patch_bytes)
        .map_err(ReleaseTargetMaterializationError::DecodePatchBundle)?;
    materialize_patch_artifact_with_external_payload_mode(base_bytes, &artifact, payload_mode)
}

pub fn materialize_patch_artifact_with_external_payload_mode(
    base_bytes: &[u8],
    artifact: &BundlePatchArtifact,
    payload_mode: ExternalPayloadMaterializationMode,
) -> Result<PatchMaterializedTarget, ReleaseTargetMaterializationError> {
    validate_external_payload_mode(base_bytes, artifact, payload_mode)?;
    apply_patch_bundle(base_bytes, artifact).map_err(ReleaseTargetMaterializationError::ApplyPatch)
}

#[derive(Debug, Error)]
pub enum ReleaseTargetMaterializationError {
    #[error("failed to decode patch bundle: {0}")]
    DecodePatchBundle(#[source] PatchBundleError),
    #[error("failed to inspect base bundle before target materialization: {0}")]
    InspectBase(String),
    #[error("external payload materialization mode {mode:?} requires section payload {descriptor_id}")]
    ExternalPayloadRequired {
        mode: ExternalPayloadMaterializationMode,
        descriptor_id: SectionId,
    },
    #[error("failed to apply patch bundle: {0}")]
    ApplyPatch(#[source] PatchBundleError),
}

fn validate_external_payload_mode(
    base_bytes: &[u8],
    artifact: &BundlePatchArtifact,
    payload_mode: ExternalPayloadMaterializationMode,
) -> Result<(), ReleaseTargetMaterializationError> {
    if payload_mode == ExternalPayloadMaterializationMode::MetadataOnly {
        return Ok(());
    }
    let base = BundleView::parse(base_bytes, ReadBudget::default())
        .map_err(|error| ReleaseTargetMaterializationError::InspectBase(error.to_string()))?;
    let operations = artifact
        .plan
        .operations
        .iter()
        .map(|operation| (operation_id(operation), operation))
        .collect::<BTreeMap<_, _>>();

    for descriptor in base.sections() {
        match operations.get(&descriptor.id()).copied() {
            Some(SectionOperation::Remove { .. }) => {}
            Some(SectionOperation::Replace { next, .. }) => {
                require_payload_if_needed(next, payload_mode)?;
            }
            Some(SectionOperation::Add(_)) | None => {
                require_payload_if_needed(descriptor, payload_mode)?;
            }
        }
    }
    for operation in &artifact.plan.operations {
        if let SectionOperation::Add(descriptor) = operation {
            require_payload_if_needed(descriptor, payload_mode)?;
        }
    }
    Ok(())
}

fn require_payload_if_needed(
    descriptor: &SectionDescriptor,
    payload_mode: ExternalPayloadMaterializationMode,
) -> Result<(), ReleaseTargetMaterializationError> {
    if descriptor.placement() != ContentPlacement::External {
        return Ok(());
    }
    let required = match payload_mode {
        ExternalPayloadMaterializationMode::MetadataOnly => false,
        ExternalPayloadMaterializationMode::RequiredResidency => {
            descriptor.required() || descriptor.residency() == ContentResidency::Startup
        }
        ExternalPayloadMaterializationMode::AllPayloads => true,
    };
    if required {
        Err(ReleaseTargetMaterializationError::ExternalPayloadRequired {
            mode: payload_mode,
            descriptor_id: descriptor.id(),
        })
    } else {
        Ok(())
    }
}

fn operation_id(operation: &SectionOperation) -> SectionId {
    match operation {
        SectionOperation::Add(descriptor) => descriptor.id(),
        SectionOperation::Replace { next, .. } => next.id(),
        SectionOperation::Remove { id, .. } => *id,
    }
}
