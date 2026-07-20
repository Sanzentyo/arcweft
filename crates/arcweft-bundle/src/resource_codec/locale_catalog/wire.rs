//! Fixed record layout and budget constants for `LocaleCatalog` schema 1.

use super::error::CharacterPresentationCatalogCodecError;
use crate::resource_codec::{
    FieldId, SectionCodecBudget,
    codec_io::{read_array, read_u32},
};
use arcweft_character::presentation_name::MAX_CATALOG_CHARACTERS;

pub(super) const FIELD_CATALOG_HEADER: FieldId = FieldId(1);
pub(super) const FIELD_FALLBACK_LOCALES: FieldId = FieldId(2);
pub(super) const FIELD_CHARACTER_RECORDS: FieldId = FieldId(3);
pub(super) const FIELD_LOCALIZED_RECORDS: FieldId = FieldId(4);

pub(super) const CATALOG_HEADER_LEN: usize = 88;
pub(super) const CHARACTER_RECORD_LEN: usize = 36;
pub(super) const LOCALIZED_RECORD_LEN: usize = 16;
pub(super) const STRING_ID_LEN: usize = 4;
pub(super) const MISSING_REF: u32 = u32::MAX;
pub(super) const MAX_CATALOG_RECORDS: usize = 327_697;
pub(super) const MAX_CATALOG_SECTION_BYTES: usize = 67_108_864;
pub(super) const MAX_CATALOG_STRINGS: usize = 1_000_000;
pub(super) const MAX_CATALOG_STRING_BYTES: usize = 50_331_648;
const MAX_CATALOG_FIELDS: usize = 4;
const MAX_TABLE_FAN_OUT: usize = MAX_CATALOG_STRINGS + MAX_CATALOG_CHARACTERS + MAX_CATALOG_FIELDS;

#[derive(Clone, Copy, Debug)]
pub(super) struct WireCatalogHeader {
    pub flags: u32,
    pub default_active_locale: u32,
    pub fallback_count: u32,
    pub character_count: u32,
    pub localized_count: u32,
    pub reserved: u32,
    pub semantic_digest: [u8; 32],
    pub locale_policy_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug)]
pub(super) struct WireCharacterRecord {
    pub character: u32,
    pub role: u8,
    pub base_tag: u8,
    pub declaration_tag: u8,
    pub reserved: u8,
    pub source_locale: u32,
    pub base_key: u32,
    pub base_value: u32,
    pub declaration_key: u32,
    pub declaration_value: u32,
    pub localized_first: u32,
    pub localized_count: u32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct WireLocalizedRecord {
    pub locale: u32,
    pub entry_tag: u8,
    pub reserved: [u8; 3],
    pub key: u32,
    pub value: u32,
}

pub(super) const fn codec_budget() -> SectionCodecBudget {
    SectionCodecBudget {
        bytes: MAX_CATALOG_SECTION_BYTES,
        items: MAX_CATALOG_FIELDS,
        records: MAX_CATALOG_RECORDS,
        strings: MAX_CATALOG_STRINGS,
        string_bytes: MAX_CATALOG_STRING_BYTES,
        public_ids: MAX_CATALOG_CHARACTERS,
        references: 0,
        depth: 0,
        table_fan_out: MAX_TABLE_FAN_OUT,
    }
}

impl WireCatalogHeader {
    pub(super) fn decode(payload: &[u8]) -> Result<Self, CharacterPresentationCatalogCodecError> {
        require_length(FIELD_CATALOG_HEADER, payload, CATALOG_HEADER_LEN)?;
        Ok(Self {
            flags: read_u32(payload, 0)?,
            default_active_locale: read_u32(payload, 4)?,
            fallback_count: read_u32(payload, 8)?,
            character_count: read_u32(payload, 12)?,
            localized_count: read_u32(payload, 16)?,
            reserved: read_u32(payload, 20)?,
            semantic_digest: read_array::<32>(payload, 24)?,
            locale_policy_digest: read_array::<32>(payload, 56)?,
        })
    }

    pub(super) fn encode_into(self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.flags.to_le_bytes());
        output.extend_from_slice(&self.default_active_locale.to_le_bytes());
        output.extend_from_slice(&self.fallback_count.to_le_bytes());
        output.extend_from_slice(&self.character_count.to_le_bytes());
        output.extend_from_slice(&self.localized_count.to_le_bytes());
        output.extend_from_slice(&self.reserved.to_le_bytes());
        output.extend_from_slice(&self.semantic_digest);
        output.extend_from_slice(&self.locale_policy_digest);
    }
}

impl WireCharacterRecord {
    pub(super) fn decode(
        payload: &[u8],
        offset: usize,
    ) -> Result<Self, CharacterPresentationCatalogCodecError> {
        let record = payload
            .get(offset..offset.saturating_add(CHARACTER_RECORD_LEN))
            .ok_or(CharacterPresentationCatalogCodecError::FieldLength {
                field: FIELD_CHARACTER_RECORDS,
                expected: offset.saturating_add(CHARACTER_RECORD_LEN),
                actual: payload.len(),
            })?;
        Ok(Self {
            character: read_u32(record, 0)?,
            role: record[4],
            base_tag: record[5],
            declaration_tag: record[6],
            reserved: record[7],
            source_locale: read_u32(record, 8)?,
            base_key: read_u32(record, 12)?,
            base_value: read_u32(record, 16)?,
            declaration_key: read_u32(record, 20)?,
            declaration_value: read_u32(record, 24)?,
            localized_first: read_u32(record, 28)?,
            localized_count: read_u32(record, 32)?,
        })
    }

    pub(super) fn encode_into(self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.character.to_le_bytes());
        output.push(self.role);
        output.push(self.base_tag);
        output.push(self.declaration_tag);
        output.push(self.reserved);
        output.extend_from_slice(&self.source_locale.to_le_bytes());
        output.extend_from_slice(&self.base_key.to_le_bytes());
        output.extend_from_slice(&self.base_value.to_le_bytes());
        output.extend_from_slice(&self.declaration_key.to_le_bytes());
        output.extend_from_slice(&self.declaration_value.to_le_bytes());
        output.extend_from_slice(&self.localized_first.to_le_bytes());
        output.extend_from_slice(&self.localized_count.to_le_bytes());
    }
}

impl WireLocalizedRecord {
    pub(super) fn decode(
        payload: &[u8],
        offset: usize,
    ) -> Result<Self, CharacterPresentationCatalogCodecError> {
        let record = payload
            .get(offset..offset.saturating_add(LOCALIZED_RECORD_LEN))
            .ok_or(CharacterPresentationCatalogCodecError::FieldLength {
                field: FIELD_LOCALIZED_RECORDS,
                expected: offset.saturating_add(LOCALIZED_RECORD_LEN),
                actual: payload.len(),
            })?;
        Ok(Self {
            locale: read_u32(record, 0)?,
            entry_tag: record[4],
            reserved: [record[5], record[6], record[7]],
            key: read_u32(record, 8)?,
            value: read_u32(record, 12)?,
        })
    }

    pub(super) fn encode_into(self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.locale.to_le_bytes());
        output.push(self.entry_tag);
        output.extend_from_slice(&self.reserved);
        output.extend_from_slice(&self.key.to_le_bytes());
        output.extend_from_slice(&self.value.to_le_bytes());
    }
}

pub(super) fn require_multiple_length(
    field: FieldId,
    payload: &[u8],
    count: usize,
    record_len: usize,
) -> Result<(), CharacterPresentationCatalogCodecError> {
    let expected = count.checked_mul(record_len).ok_or(
        CharacterPresentationCatalogCodecError::ArithmeticOverflow {
            operation: "LocaleCatalog field byte length",
        },
    )?;
    require_length(field, payload, expected)
}

fn require_length(
    field: FieldId,
    payload: &[u8],
    expected: usize,
) -> Result<(), CharacterPresentationCatalogCodecError> {
    if payload.len() != expected {
        return Err(CharacterPresentationCatalogCodecError::FieldLength {
            field,
            expected,
            actual: payload.len(),
        });
    }
    Ok(())
}
