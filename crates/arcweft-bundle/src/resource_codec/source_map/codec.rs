use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{MAX_SOURCE_MAP_DOCUMENTS, ProductSourceId, SourceMapCodecError, SourceMapSection};
use crate::resource_codec::budget::SectionCodecBudget;
use crate::resource_codec::codec_io::{Cursor, u32_from_usize, usize_from_u32, usize_from_u64};
use crate::resource_codec::field::{
    FieldId, FieldRegistry, FieldRequirement, FieldSpec, ResourceField, ResourceWireType,
};
use crate::resource_codec::kind::ProductSectionCodecKind;
use crate::resource_codec::table::{
    EnumRegistry, PublicIdRef, PublicIdTable, StringId, StringTable,
};
use crate::resource_codec::wire::ProductResourceEnvelope;

const SOURCE_MAP_SCHEMA: u32 = 3;
const FIELD_SOURCE_MAP_TRANSCRIPT: FieldId = FieldId(1);
const DISPLAY_PATH: u8 = 1;
const DISPLAY_MEMORY: u8 = 2;
const DISPLAY_GENERATED: u8 = 3;
const NO_STRING_REF: u32 = u32::MAX;

struct DecodedSourceMapTranscript {
    source_set_revision: [u8; 32],
    primary_document_id: Option<SourceDocumentId>,
    documents: Vec<SourceDocument>,
}

impl SourceMapSection {
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SourceMapCodecError> {
        let budget = source_map_budget();
        let strings = StringTable::with_budget(
            self.documents().flat_map(|document| {
                let display = match document.display_name() {
                    SourceName::Path(path) => Some(path.clone()),
                    SourceName::Memory | SourceName::Generated => None,
                };
                std::iter::once(document.document_id().as_str().to_owned()).chain(display)
            }),
            budget,
        )?;
        let public_ids = PublicIdTable::with_budget(
            self.documents()
                .map(|document| document.id().as_str().to_owned()),
            budget,
        )?;
        let enums = EnumRegistry::with_budget(std::iter::empty(), &strings, budget)?;
        let mut transcript = Vec::new();
        transcript.extend_from_slice(&SOURCE_MAP_SCHEMA.to_le_bytes());
        transcript.extend_from_slice(self.source_set_revision().as_bytes());
        let primary = self
            .primary_document_id()
            .map(|id| {
                strings
                    .id_for(id.as_str())
                    .map(|id| id.0)
                    .ok_or(SourceMapCodecError::ArithmeticOverflow)
            })
            .transpose()?
            .unwrap_or(NO_STRING_REF);
        transcript.extend_from_slice(&primary.to_le_bytes());
        transcript.extend_from_slice(&u32_from_usize(self.documents().len())?.to_le_bytes());
        for document in self.documents() {
            let product = public_ids
                .id_for(document.id().as_str())
                .ok_or(SourceMapCodecError::ArithmeticOverflow)?;
            let document_id = strings
                .id_for(document.document_id().as_str())
                .ok_or(SourceMapCodecError::ArithmeticOverflow)?;
            transcript.extend_from_slice(&product.0.to_le_bytes());
            transcript.extend_from_slice(&document_id.0.to_le_bytes());
            match document.display_name() {
                SourceName::Path(path) => {
                    transcript.push(DISPLAY_PATH);
                    let display = strings
                        .id_for(path)
                        .ok_or(SourceMapCodecError::ArithmeticOverflow)?;
                    transcript.extend_from_slice(&display.0.to_le_bytes());
                }
                SourceName::Memory => {
                    transcript.push(DISPLAY_MEMORY);
                    transcript.extend_from_slice(&NO_STRING_REF.to_le_bytes());
                }
                SourceName::Generated => {
                    transcript.push(DISPLAY_GENERATED);
                    transcript.extend_from_slice(&NO_STRING_REF.to_le_bytes());
                }
            }
            transcript.extend_from_slice(document.revision().as_bytes());
            transcript.extend_from_slice(&document.source_len().to_le_bytes());
            transcript.extend_from_slice(&document.source_len().to_le_bytes());
            transcript.extend_from_slice(document.text().as_bytes());
        }
        let record_count = u32_from_usize(self.documents().len())?;
        let reference_count = u16::try_from(self.documents().len()).unwrap_or(u16::MAX);
        ProductResourceEnvelope::with_budget(
            ProductSectionCodecKind::SourceMap,
            strings,
            public_ids,
            enums,
            [ResourceField::new(
                FIELD_SOURCE_MAP_TRANSCRIPT,
                FieldRequirement::Required,
                ResourceWireType::Bytes,
                1,
                reference_count,
                transcript,
            )],
            record_count,
            budget,
        )?
        .encode_canonical()
        .map_err(Into::into)
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SourceMapCodecError> {
        let budget = source_map_budget();
        let registry = FieldRegistry::new([FieldSpec::required(
            FIELD_SOURCE_MAP_TRANSCRIPT,
            ResourceWireType::Bytes,
        )])?;
        let decoded = ProductResourceEnvelope::decode_with_registry(
            bytes,
            ProductSectionCodecKind::SourceMap,
            &registry,
            budget,
        )?;
        if decoded.skipped_unknown_optional_fields != 0 {
            return Err(SourceMapCodecError::NonCanonicalEncoding);
        }
        let transcript = decode_transcript(&decoded.envelope)?;
        let references = transcript.documents.iter().collect::<Vec<_>>();
        let mut section = Self::try_from_documents(&references)?;
        if let Some(primary) = transcript.primary_document_id {
            if !section
                .documents()
                .any(|document| document.document_id() == &primary)
            {
                return Err(SourceMapCodecError::PrimaryDocumentMissing(primary));
            }
            section.primary_document_id = Some(primary);
        } else if section.documents().len() != 0 {
            return Err(SourceMapCodecError::MissingPrimaryDocument);
        }
        if transcript.source_set_revision != *section.source_set_revision().as_bytes() {
            return Err(SourceMapCodecError::SourceSetRevisionMismatch);
        }
        if section.encode_canonical_section()? != bytes {
            return Err(SourceMapCodecError::NonCanonicalEncoding);
        }
        Ok(section)
    }
}

fn decode_transcript(
    envelope: &ProductResourceEnvelope,
) -> Result<DecodedSourceMapTranscript, SourceMapCodecError> {
    let field = envelope
        .fields
        .iter()
        .find(|field| field.id == FIELD_SOURCE_MAP_TRANSCRIPT)
        .ok_or(
            crate::resource_codec::SectionCodecError::MissingRequiredField(
                FIELD_SOURCE_MAP_TRANSCRIPT,
            ),
        )?;
    let mut cursor = Cursor::new(&field.payload);
    let schema = cursor.read_u32()?;
    if schema != SOURCE_MAP_SCHEMA {
        return Err(SourceMapCodecError::UnsupportedSchema {
            actual: schema,
            expected: SOURCE_MAP_SCHEMA,
        });
    }
    let encoded_source_set = read_array::<32>(&mut cursor)?;
    let primary_ref = cursor.read_u32()?;
    let primary_document_id = if primary_ref == NO_STRING_REF {
        None
    } else {
        let value = envelope.strings.get(StringId(primary_ref))?.to_owned();
        Some(
            SourceDocumentId::try_new(value.clone())
                .map_err(|_| SourceMapCodecError::InvalidDocumentId(value))?,
        )
    };
    let count = cursor.read_u32()?;
    if count != envelope.header.record_count {
        return Err(SourceMapCodecError::RecordCountMismatch);
    }
    let count = usize_from_u32(count)?;
    if count > MAX_SOURCE_MAP_DOCUMENTS {
        return Err(super::SourceMapBuildError::TooManyDocuments {
            actual: count,
            limit: MAX_SOURCE_MAP_DOCUMENTS,
        }
        .into());
    }
    let documents = (0..count)
        .map(|_| decode_document(&mut cursor, envelope))
        .collect::<Result<Vec<_>, _>>()?;
    if cursor.remaining() != 0 {
        return Err(crate::resource_codec::SectionCodecError::TrailingBytes.into());
    }
    Ok(DecodedSourceMapTranscript {
        source_set_revision: encoded_source_set,
        primary_document_id,
        documents,
    })
}

fn decode_document(
    cursor: &mut Cursor<'_>,
    envelope: &ProductResourceEnvelope,
) -> Result<SourceDocument, SourceMapCodecError> {
    let product_ref = PublicIdRef(cursor.read_u32()?);
    let document_ref = StringId(cursor.read_u32()?);
    let display_tag = cursor.read_u8()?;
    let display_ref = cursor.read_u32()?;
    let encoded_revision = read_array::<32>(cursor)?;
    let encoded_extent = read_u64(cursor)?;
    let utf8_len = read_u64(cursor)?;
    let utf8 = cursor.read_bytes(usize_from_u64(utf8_len)?)?;
    let text = std::str::from_utf8(utf8).map_err(|_| SourceMapCodecError::InvalidUtf8)?;
    let document_text = envelope.strings.get(document_ref)?.to_owned();
    let document_id = SourceDocumentId::try_new(document_text.clone())
        .map_err(|_| SourceMapCodecError::InvalidDocumentId(document_text))?;
    let display_name = decode_display_name(display_tag, display_ref, &envelope.strings)?;
    let product_text = envelope.public_ids.get(product_ref)?.to_owned();
    let actual_product = ProductSourceId::try_from_encoded(product_text.clone())
        .map_err(|_| SourceMapCodecError::InvalidProductSourceId(product_text))?;
    let expected_product = ProductSourceId::try_for_document_id(&document_id)?;
    if actual_product != expected_product {
        return Err(SourceMapCodecError::ProductSourceIdMismatch {
            document: document_id,
            expected: expected_product,
            actual: actual_product,
        });
    }
    let document = SourceDocument::try_new(document_id, display_name, text)
        .map_err(|_| SourceMapCodecError::ArithmeticOverflow)?;
    let actual_extent = document.identity().source_len();
    if encoded_extent != actual_extent || utf8_len != actual_extent {
        return Err(SourceMapCodecError::ExtentMismatch {
            id: actual_product,
            declared: encoded_extent,
            payload: utf8_len,
            actual: actual_extent,
        });
    }
    let actual_revision = document.identity().revision();
    if encoded_revision != *actual_revision.as_bytes() {
        return Err(SourceMapCodecError::RevisionMismatch {
            id: actual_product,
            encoded: encoded_revision,
            actual: actual_revision,
        });
    }
    Ok(document)
}

fn decode_display_name(
    tag: u8,
    reference: u32,
    strings: &StringTable,
) -> Result<SourceName, SourceMapCodecError> {
    match tag {
        DISPLAY_PATH => {
            if reference == NO_STRING_REF {
                return Err(SourceMapCodecError::InvalidDisplayNameReference);
            }
            strings
                .get(StringId(reference))
                .map(|path| SourceName::path(path.to_owned()))
                .map_err(Into::into)
        }
        DISPLAY_MEMORY if reference == NO_STRING_REF => Ok(SourceName::Memory),
        DISPLAY_GENERATED if reference == NO_STRING_REF => Ok(SourceName::Generated),
        DISPLAY_MEMORY | DISPLAY_GENERATED => Err(SourceMapCodecError::InvalidDisplayNameReference),
        other => Err(SourceMapCodecError::InvalidDisplayNameTag(other)),
    }
}

fn read_array<const N: usize>(cursor: &mut Cursor<'_>) -> Result<[u8; N], SourceMapCodecError> {
    cursor
        .read_bytes(N)?
        .try_into()
        .map_err(|_| SourceMapCodecError::ArithmeticOverflow)
}

fn read_u64(cursor: &mut Cursor<'_>) -> Result<u64, SourceMapCodecError> {
    cursor
        .read_bytes(8)?
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| SourceMapCodecError::ArithmeticOverflow)
}

fn source_map_budget() -> SectionCodecBudget {
    SectionCodecBudget {
        records: MAX_SOURCE_MAP_DOCUMENTS,
        strings: MAX_SOURCE_MAP_DOCUMENTS.saturating_mul(2),
        public_ids: MAX_SOURCE_MAP_DOCUMENTS,
        ..SectionCodecBudget::default()
    }
}
