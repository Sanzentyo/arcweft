use crate::container::{
    BundleDigest, BundleView, ExternalSectionPayload, SectionId, SectionInput, SectionKindCode,
};
use crate::resource_codec::{
    CrossSectionRef, ProductResourceEnvelope, ProductSectionCodecKind, SectionCodecBudget,
    ViewStyleResource,
};
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub(super) enum StyleCrossSectionReferenceError {
    #[error("Style cross-section reference targets missing section {section_id}")]
    MissingSection { section_id: SectionId },
    #[error(
        "Style cross-section reference for section {section_id} expects kind {expected_kind} and digest {expected_digest}, but the bundle has kind {actual_kind} and digest {actual_digest}"
    )]
    IdentityMismatch {
        section_id: SectionId,
        expected_kind: u32,
        actual_kind: u32,
        expected_digest: BundleDigest,
        actual_digest: BundleDigest,
    },
    #[error("Style cross-section reference cannot read target section {section_id}: {message}")]
    UnreadableSection {
        section_id: SectionId,
        message: String,
    },
    #[error(
        "Style cross-section reference has out-of-bounds public ID {public_id} in section {section_id}"
    )]
    PublicIdOutOfBounds {
        section_id: SectionId,
        public_id: u32,
    },
    #[error(
        "Style cross-section reference has public ID {public_id}, but section {section_id} kind {section_kind} has no public-ID table"
    )]
    UnsupportedPublicIdTarget {
        section_id: SectionId,
        section_kind: u32,
        public_id: u32,
    },
}

pub(super) fn validate_style_section_inputs(
    style: Option<&ViewStyleResource>,
    sections: &[SectionInput],
) -> Result<(), StyleCrossSectionReferenceError> {
    let Some(style) = style else {
        return Ok(());
    };
    let mut public_id_counts = BTreeMap::new();
    for reference in style_cross_section_refs(style) {
        let target = sections
            .iter()
            .find(|section| section.id() == reference.section_id)
            .ok_or(StyleCrossSectionReferenceError::MissingSection {
                section_id: reference.section_id,
            })?;
        validate_style_cross_section_identity(
            reference,
            target.kind_code(),
            target.content_digest(),
        )?;
        let Some(public_id) = reference.public_id else {
            continue;
        };
        let codec = target
            .known_kind()
            .and_then(ProductSectionCodecKind::from_section_kind)
            .ok_or(StyleCrossSectionReferenceError::UnsupportedPublicIdTarget {
                section_id: target.id(),
                section_kind: target.kind_code().encoded(),
                public_id: public_id.0,
            })?;
        let public_id_count = if let Some(count) = public_id_counts.get(&target.id()) {
            *count
        } else {
            let count = compact_public_id_count(target.stored_bytes(), codec, target.id())?;
            public_id_counts.insert(target.id(), count);
            count
        };
        if public_id.0 >= public_id_count {
            return Err(StyleCrossSectionReferenceError::PublicIdOutOfBounds {
                section_id: reference.section_id,
                public_id: public_id.0,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_style_bundle_view(
    style: Option<&ViewStyleResource>,
    view: &BundleView<'_>,
    external_sections: &[ExternalSectionPayload],
) -> Result<(), StyleCrossSectionReferenceError> {
    let Some(style) = style else {
        return Ok(());
    };
    let mut public_id_counts = BTreeMap::new();
    for reference in style_cross_section_refs(style) {
        let target = view
            .sections()
            .iter()
            .find(|section| section.id() == reference.section_id)
            .ok_or(StyleCrossSectionReferenceError::MissingSection {
                section_id: reference.section_id,
            })?;
        validate_style_cross_section_identity(
            reference,
            target.kind_code(),
            target.content_digest(),
        )?;
        let Some(public_id) = reference.public_id else {
            continue;
        };
        let codec = target
            .known_kind()
            .and_then(ProductSectionCodecKind::from_section_kind)
            .ok_or(StyleCrossSectionReferenceError::UnsupportedPublicIdTarget {
                section_id: target.id(),
                section_kind: target.kind_code().encoded(),
                public_id: public_id.0,
            })?;
        let public_id_count = if let Some(count) = public_id_counts.get(&target.id()) {
            *count
        } else {
            let bytes = view
                .decoded_section_with_external_payloads(target.id(), external_sections)
                .map_err(|error| StyleCrossSectionReferenceError::UnreadableSection {
                    section_id: target.id(),
                    message: error.to_string(),
                })?
                .ok_or_else(|| StyleCrossSectionReferenceError::UnreadableSection {
                    section_id: target.id(),
                    message: "section payload is unavailable".to_owned(),
                })?;
            let count = compact_public_id_count(&bytes, codec, target.id())?;
            public_id_counts.insert(target.id(), count);
            count
        };
        if public_id.0 >= public_id_count {
            return Err(StyleCrossSectionReferenceError::PublicIdOutOfBounds {
                section_id: reference.section_id,
                public_id: public_id.0,
            });
        }
    }
    Ok(())
}

fn validate_style_cross_section_identity(
    reference: &CrossSectionRef,
    actual_kind: SectionKindCode,
    actual_digest: BundleDigest,
) -> Result<(), StyleCrossSectionReferenceError> {
    if reference.section_kind == actual_kind && reference.content_digest == actual_digest {
        return Ok(());
    }
    Err(StyleCrossSectionReferenceError::IdentityMismatch {
        section_id: reference.section_id,
        expected_kind: reference.section_kind.encoded(),
        actual_kind: actual_kind.encoded(),
        expected_digest: reference.content_digest,
        actual_digest,
    })
}

fn compact_public_id_count(
    bytes: &[u8],
    codec: ProductSectionCodecKind,
    section_id: SectionId,
) -> Result<u32, StyleCrossSectionReferenceError> {
    let envelope =
        ProductResourceEnvelope::decode_all_fields(bytes, codec, SectionCodecBudget::default())
            .map_err(|error| StyleCrossSectionReferenceError::UnreadableSection {
                section_id,
                message: error.to_string(),
            })?;
    u32::try_from(envelope.public_ids.len()).map_err(|error| {
        StyleCrossSectionReferenceError::UnreadableSection {
            section_id,
            message: error.to_string(),
        }
    })
}

fn style_cross_section_refs(style: &ViewStyleResource) -> impl Iterator<Item = &CrossSectionRef> {
    style.adapter_requirements.iter()
}
