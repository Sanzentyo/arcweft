//! Canonical compact-envelope transcript encoding for View resource families.

use super::{ViewResourceBudget, ViewResourceExport, unique_strings};
use crate::container::BundleDigest;
use crate::resource_codec::budget::check_budget;
use crate::resource_codec::error::SectionCodecError;
use crate::resource_codec::field::{
    FieldId, FieldRegistry, FieldRequirement, FieldSpec, ResourceField, ResourceWireType,
};
use crate::resource_codec::header::PRODUCT_SECTION_SCHEMA_VERSION;
use crate::resource_codec::kind::ProductSectionCodecKind;
use crate::resource_codec::table::{EnumRegistry, EnumSymbol, PublicIdTable, StringTable};
use crate::resource_codec::wire::ProductResourceEnvelope;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

const FIELD_VIEW_TRANSCRIPT: FieldId = FieldId(1);

pub(super) fn encode_view_section<T>(
    codec: ProductSectionCodecKind,
    family_label: &'static str,
    value: &T,
    public_ids: impl IntoIterator<Item = String>,
    record_count: u32,
    budget: &ViewResourceBudget,
) -> Result<Vec<u8>, SectionCodecError>
where
    T: Serialize,
{
    validate_view_transcript_budget(value, budget)?;
    let transcript = serde_json::to_vec(value)
        .map_err(|_| SectionCodecError::NonCanonicalTable(family_label))?;
    let strings = StringTable::with_budget(
        [
            family_label.to_owned(),
            "canonical_view_resource_transcript_v1".to_owned(),
        ],
        budget.common,
    )?;
    let public_ids = PublicIdTable::with_budget(unique_strings(public_ids), budget.common)?;
    let enums = EnumRegistry::with_budget(
        [EnumSymbol {
            code: 1,
            name: strings
                .id_for(family_label)
                .ok_or(SectionCodecError::NonCanonicalTable(family_label))?,
        }],
        &strings,
        budget.common,
    )?;
    let field = ResourceField::new(
        FIELD_VIEW_TRANSCRIPT,
        FieldRequirement::Required,
        ResourceWireType::Bytes,
        1,
        u16::try_from(public_ids.len()).map_err(|_| SectionCodecError::LengthOverflow)?,
        transcript,
    );
    ProductResourceEnvelope::with_budget(
        codec,
        strings,
        public_ids,
        enums,
        [field],
        record_count,
        budget.common,
    )?
    .encode_canonical()
}

fn validate_view_transcript_budget<T>(
    value: &T,
    budget: &ViewResourceBudget,
) -> Result<(), SectionCodecError>
where
    T: Serialize,
{
    let mut counter = JsonByteCounter::default();
    serde_json::to_writer(&mut counter, value)
        .map_err(|_| SectionCodecError::NonCanonicalTable("view_transcript_canonical"))?;
    check_budget(
        counter.bytes,
        budget.transcript_bytes,
        "view_transcript_bytes",
    )
}

#[derive(Default)]
struct JsonByteCounter {
    bytes: usize,
}

impl Write for JsonByteCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("canonical View transcript length overflow"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn decode_view_section<T, P, R>(
    bytes: &[u8],
    codec: ProductSectionCodecKind,
    family_label: &'static str,
    budget: &ViewResourceBudget,
    public_ids: P,
    record_count: R,
) -> Result<(T, Vec<u8>), SectionCodecError>
where
    T: for<'de> Deserialize<'de>,
    P: FnOnce(&T) -> Vec<String>,
    R: FnOnce(&T) -> u32,
{
    let decoded = ProductResourceEnvelope::decode_with_registry(
        bytes,
        codec,
        &view_registry()?,
        budget.common,
    )?;
    let field = decoded
        .envelope
        .fields
        .iter()
        .find(|field| field.id == FIELD_VIEW_TRANSCRIPT)
        .ok_or(SectionCodecError::MissingRequiredField(
            FIELD_VIEW_TRANSCRIPT,
        ))?;
    check_budget(
        field.payload.len(),
        budget.transcript_bytes,
        "view_transcript_bytes",
    )?;
    let resource: T = serde_json::from_slice(&field.payload)
        .map_err(|_| SectionCodecError::NonCanonicalTable(family_label))?;
    let expected_public_ids = PublicIdTable::with_budget(public_ids(&resource), budget.common)?;
    if decoded.envelope.public_ids != expected_public_ids {
        return Err(SectionCodecError::NonCanonicalTable(
            "view_envelope_public_ids",
        ));
    }
    if decoded.envelope.header.record_count != record_count(&resource) {
        return Err(SectionCodecError::NonCanonicalTable(
            "view_envelope_record_count",
        ));
    }
    Ok((resource, field.payload.clone()))
}

pub(super) fn validate_canonical_view_transcript<T>(
    transcript: &[u8],
    resource: &T,
) -> Result<(), SectionCodecError>
where
    T: Serialize,
{
    let canonical = serde_json::to_vec(resource)
        .map_err(|_| SectionCodecError::NonCanonicalTable("view_transcript_canonical"))?;
    if transcript != canonical {
        return Err(SectionCodecError::NonCanonicalTable(
            "view_transcript_canonical",
        ));
    }
    Ok(())
}

pub(super) fn export_json_bytes<T>(
    codec: ProductSectionCodecKind,
    resource: &T,
    canonical_digest: BundleDigest,
) -> Result<Vec<u8>, SectionCodecError>
where
    T: Clone + Serialize,
{
    let export = ViewResourceExport {
        schema_version: PRODUCT_SECTION_SCHEMA_VERSION,
        codec,
        codec_name: codec.as_str().to_owned(),
        canonical_digest,
        resource: resource.clone(),
    };
    serde_json::to_vec_pretty(&export)
        .map_err(|_| SectionCodecError::NonCanonicalTable("view_export_json"))
}

fn view_registry() -> Result<FieldRegistry, SectionCodecError> {
    FieldRegistry::new([FieldSpec::required(
        FIELD_VIEW_TRANSCRIPT,
        ResourceWireType::Bytes,
    )])
}
