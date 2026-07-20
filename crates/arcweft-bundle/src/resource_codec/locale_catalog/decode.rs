//! Strict decoder for the closed Character presentation `LocaleCatalog` family.

use super::{
    error::CharacterPresentationCatalogCodecError,
    wire::{
        CHARACTER_RECORD_LEN, FIELD_CATALOG_HEADER, FIELD_CHARACTER_RECORDS,
        FIELD_FALLBACK_LOCALES, FIELD_LOCALIZED_RECORDS, LOCALIZED_RECORD_LEN, MAX_CATALOG_RECORDS,
        MISSING_REF, STRING_ID_LEN, WireCatalogHeader, WireCharacterRecord, WireLocalizedRecord,
        codec_budget, require_multiple_length,
    },
};
use crate::resource_codec::{
    FieldId, FieldRegistry, FieldSpec, ProductResourceEnvelope, ProductSectionCodecKind,
    PublicIdTable, ResourceField, ResourceWireType, StringTable,
};
use arcweft_character::{
    id::CharacterId,
    presentation_name::{
        CharacterDisplayNameInput, CharacterDisplayNameKey, CharacterDisplayNameRecordInput,
        CharacterDisplayNameValue, CharacterNameFallbackLocale, CharacterNameLocale,
        CharacterNameLocalePolicy, CharacterNameSourceLocale, CharacterPresentationCatalogData,
        CharacterPresentationCatalogInput, CharacterPresentationLocalePolicyDigest,
        CharacterPresentationRole, CharacterPresentationSemanticDigest,
        LocalizedCharacterDisplayNameInput, MAX_CATALOG_CHARACTERS, MAX_CATALOG_LOCALIZED_ENTRIES,
        MAX_FALLBACK_LOCALES, MAX_LOCALIZED_NAMES_PER_CHARACTER,
    },
};
use arcweft_id::LocaleTag;

pub(super) fn decode(
    bytes: &[u8],
) -> Result<CharacterPresentationCatalogData, CharacterPresentationCatalogCodecError> {
    let envelope = decode_envelope(bytes)?;
    let layout = CatalogLayout::validate(&envelope)?;
    let mut tables = DecodeTables::new(&envelope.strings, &envelope.public_ids);
    let policy = decode_policy(&layout, &mut tables)?;
    let records = decode_records(&layout, &mut tables)?;
    tables.require_exact_references()?;

    let input = CharacterPresentationCatalogInput::try_new(policy, records)?;
    let catalog = CharacterPresentationCatalogData::try_from_inputs(input)?;
    if catalog.semantic_digest()
        != CharacterPresentationSemanticDigest::from_bytes(layout.header.semantic_digest)
    {
        return Err(CharacterPresentationCatalogCodecError::SemanticDigestMismatch);
    }
    if catalog.locale_policy_digest()
        != CharacterPresentationLocalePolicyDigest::from_bytes(layout.header.locale_policy_digest)
    {
        return Err(CharacterPresentationCatalogCodecError::LocalePolicyDigestMismatch);
    }
    Ok(catalog)
}

fn decode_envelope(
    bytes: &[u8],
) -> Result<ProductResourceEnvelope, CharacterPresentationCatalogCodecError> {
    let registry = FieldRegistry::new([
        FieldSpec::required(FIELD_CATALOG_HEADER, ResourceWireType::Bytes),
        FieldSpec::required(FIELD_FALLBACK_LOCALES, ResourceWireType::Bytes),
        FieldSpec::required(FIELD_CHARACTER_RECORDS, ResourceWireType::Bytes),
        FieldSpec::required(FIELD_LOCALIZED_RECORDS, ResourceWireType::Bytes),
    ])?;
    let decoded = ProductResourceEnvelope::decode_with_registry(
        bytes,
        ProductSectionCodecKind::LocaleCatalog,
        &registry,
        codec_budget(),
    )?;
    if decoded.skipped_unknown_optional_fields != 0 {
        return Err(
            CharacterPresentationCatalogCodecError::UnknownOptionalFields {
                count: decoded.skipped_unknown_optional_fields,
            },
        );
    }
    Ok(decoded.envelope)
}

struct CatalogLayout<'a> {
    header: WireCatalogHeader,
    fallback_payload: &'a [u8],
    character_payload: &'a [u8],
    localized_payload: &'a [u8],
    fallback_count: usize,
    character_count: usize,
}

impl<'a> CatalogLayout<'a> {
    fn validate(
        envelope: &'a ProductResourceEnvelope,
    ) -> Result<Self, CharacterPresentationCatalogCodecError> {
        require_header_value("field_count", 4, envelope.header.field_count)?;
        require_header_value("enum_registry_len", 0, envelope.header.enum_registry_len)?;
        if !envelope.enums.is_empty() {
            return Err(CharacterPresentationCatalogCodecError::HeaderValue {
                name: "enum_registry_len",
                expected: 0,
                actual: checked_u32(envelope.enums.len(), "enum registry length")?,
            });
        }

        let header = WireCatalogHeader::decode(field(&envelope.fields, FIELD_CATALOG_HEADER)?)?;
        if header.flags != 0 {
            return Err(CharacterPresentationCatalogCodecError::NonzeroReserved {
                field: FIELD_CATALOG_HEADER,
                offset: 0,
            });
        }
        if header.reserved != 0 {
            return Err(CharacterPresentationCatalogCodecError::NonzeroReserved {
                field: FIELD_CATALOG_HEADER,
                offset: 20,
            });
        }
        require_limit(
            "fallback",
            header.fallback_count,
            checked_u32(MAX_FALLBACK_LOCALES, "fallback limit")?,
        )?;
        require_limit(
            "Character",
            header.character_count,
            checked_u32(MAX_CATALOG_CHARACTERS, "Character limit")?,
        )?;
        require_limit(
            "localized",
            header.localized_count,
            checked_u32(MAX_CATALOG_LOCALIZED_ENTRIES, "localized limit")?,
        )?;
        require_header_value(
            "public_id_table_len",
            header.character_count,
            envelope.header.public_id_table_len,
        )?;

        let fallback_count = checked_usize(header.fallback_count, "fallback count")?;
        let character_count = checked_usize(header.character_count, "Character count")?;
        let localized_count = checked_usize(header.localized_count, "localized count")?;
        let fallback_payload = field(&envelope.fields, FIELD_FALLBACK_LOCALES)?;
        let character_payload = field(&envelope.fields, FIELD_CHARACTER_RECORDS)?;
        let localized_payload = field(&envelope.fields, FIELD_LOCALIZED_RECORDS)?;
        require_multiple_length(
            FIELD_FALLBACK_LOCALES,
            fallback_payload,
            fallback_count,
            STRING_ID_LEN,
        )?;
        require_multiple_length(
            FIELD_CHARACTER_RECORDS,
            character_payload,
            character_count,
            CHARACTER_RECORD_LEN,
        )?;
        require_multiple_length(
            FIELD_LOCALIZED_RECORDS,
            localized_payload,
            localized_count,
            LOCALIZED_RECORD_LEN,
        )?;

        let expected_records = 1_u32
            .checked_add(header.fallback_count)
            .and_then(|count| count.checked_add(header.character_count))
            .and_then(|count| count.checked_add(header.localized_count))
            .ok_or(CharacterPresentationCatalogCodecError::ArithmeticOverflow {
                operation: "LocaleCatalog record count",
            })?;
        require_header_value(
            "record_count",
            expected_records,
            envelope.header.record_count,
        )?;
        require_limit(
            "record",
            expected_records,
            checked_u32(MAX_CATALOG_RECORDS, "record limit")?,
        )?;
        Ok(Self {
            header,
            fallback_payload,
            character_payload,
            localized_payload,
            fallback_count,
            character_count,
        })
    }
}

fn decode_policy(
    layout: &CatalogLayout<'_>,
    tables: &mut DecodeTables<'_>,
) -> Result<CharacterNameLocalePolicy, CharacterPresentationCatalogCodecError> {
    let default_active = decode_locale(tables, layout.header.default_active_locale)?;
    let mut fallbacks = Vec::with_capacity(layout.fallback_count);
    for index in 0..layout.fallback_count {
        let offset = checked_mul(index, STRING_ID_LEN, "fallback record offset")?;
        let reference = u32::from_le_bytes(
            layout.fallback_payload[offset..offset + STRING_ID_LEN]
                .try_into()
                .expect("validated fallback record length"),
        );
        fallbacks.push(CharacterNameFallbackLocale::new(decode_locale(
            tables, reference,
        )?));
    }
    CharacterNameLocalePolicy::try_new(default_active, fallbacks).map_err(Into::into)
}

fn decode_records(
    layout: &CatalogLayout<'_>,
    tables: &mut DecodeTables<'_>,
) -> Result<Vec<CharacterDisplayNameRecordInput>, CharacterPresentationCatalogCodecError> {
    let mut records = Vec::with_capacity(layout.character_count);
    let mut previous_character = None;
    let mut expected_localized_first = 0_u32;
    for index in 0..layout.character_count {
        let (record, character, localized_end) = decode_record(
            layout,
            tables,
            index,
            expected_localized_first,
            previous_character.as_ref(),
        )?;
        previous_character = Some(character.clone());
        expected_localized_first = localized_end;
        records.push(record);
    }
    if expected_localized_first != layout.header.localized_count {
        return Err(CharacterPresentationCatalogCodecError::InvalidLocalizedSpan);
    }
    Ok(records)
}

fn decode_record(
    layout: &CatalogLayout<'_>,
    tables: &mut DecodeTables<'_>,
    index: usize,
    expected_localized_first: u32,
    previous_character: Option<&CharacterId>,
) -> Result<
    (CharacterDisplayNameRecordInput, CharacterId, u32),
    CharacterPresentationCatalogCodecError,
> {
    let record_offset = checked_mul(index, CHARACTER_RECORD_LEN, "Character record offset")?;
    let wire = WireCharacterRecord::decode(layout.character_payload, record_offset)?;
    let record_offset = checked_u32(record_offset, "Character record wire offset")?;
    if wire.reserved != 0 {
        return Err(CharacterPresentationCatalogCodecError::NonzeroReserved {
            field: FIELD_CHARACTER_RECORDS,
            offset: record_offset + 7,
        });
    }
    if wire.localized_first != expected_localized_first {
        return Err(CharacterPresentationCatalogCodecError::InvalidLocalizedSpan);
    }
    require_limit(
        "localized entries per Character",
        wire.localized_count,
        checked_u32(
            MAX_LOCALIZED_NAMES_PER_CHARACTER,
            "per-Character localized limit",
        )?,
    )?;
    let localized_end = wire
        .localized_first
        .checked_add(wire.localized_count)
        .ok_or(CharacterPresentationCatalogCodecError::ArithmeticOverflow {
            operation: "localized record span",
        })?;
    if localized_end > layout.header.localized_count {
        return Err(CharacterPresentationCatalogCodecError::InvalidLocalizedSpan);
    }

    let character = decode_character(tables, wire.character)?;
    if previous_character.is_some_and(|previous| previous >= &character) {
        return Err(
            CharacterPresentationCatalogCodecError::NonCanonicalRecordOrder { table: "Character" },
        );
    }
    let role = match wire.role {
        1 => CharacterPresentationRole::Character,
        2 => CharacterPresentationRole::Narrator,
        actual => {
            return Err(CharacterPresentationCatalogCodecError::UnsupportedTag {
                field: FIELD_CHARACTER_RECORDS,
                offset: record_offset + 4,
                kind: "role",
                actual,
            });
        }
    };
    let source_locale = if wire.source_locale == MISSING_REF {
        None
    } else {
        Some(CharacterNameSourceLocale::new(decode_locale(
            tables,
            wire.source_locale,
        )?))
    };
    let base = decode_entry(
        tables,
        &character,
        None,
        wire.base_tag,
        wire.base_key,
        wire.base_value,
        FIELD_CHARACTER_RECORDS,
        record_offset + 5,
        true,
    )?;
    let declaration_fallback = decode_declaration(
        tables,
        &character,
        wire.declaration_tag,
        wire.declaration_key,
        wire.declaration_value,
        record_offset + 6,
    )?;
    let localized = decode_localized_entries(
        layout,
        tables,
        &character,
        wire.localized_first,
        localized_end,
    )?;
    let input = CharacterDisplayNameRecordInput::try_new(
        character.clone(),
        role,
        source_locale,
        base,
        localized,
        declaration_fallback,
    )?;
    Ok((input, character, localized_end))
}

fn decode_localized_entries(
    layout: &CatalogLayout<'_>,
    tables: &mut DecodeTables<'_>,
    character: &CharacterId,
    first: u32,
    end: u32,
) -> Result<Vec<LocalizedCharacterDisplayNameInput>, CharacterPresentationCatalogCodecError> {
    let first = checked_usize(first, "localized record first index")?;
    let end = checked_usize(end, "localized record end index")?;
    let mut localized = Vec::with_capacity(end - first);
    let mut previous_locale = None;
    for index in first..end {
        let offset = checked_mul(index, LOCALIZED_RECORD_LEN, "localized record offset")?;
        let wire = WireLocalizedRecord::decode(layout.localized_payload, offset)?;
        let offset = checked_u32(offset, "localized wire offset")?;
        if wire.reserved != [0; 3] {
            return Err(CharacterPresentationCatalogCodecError::NonzeroReserved {
                field: FIELD_LOCALIZED_RECORDS,
                offset: offset + 5,
            });
        }
        let locale = decode_locale(tables, wire.locale)?;
        if previous_locale
            .as_ref()
            .is_some_and(|previous: &CharacterNameLocale| previous >= &locale)
        {
            return Err(
                CharacterPresentationCatalogCodecError::NonCanonicalRecordOrder {
                    table: "localized",
                },
            );
        }
        previous_locale = Some(locale.clone());
        let entry = decode_entry(
            tables,
            character,
            Some(&locale),
            wire.entry_tag,
            wire.key,
            wire.value,
            FIELD_LOCALIZED_RECORDS,
            offset + 4,
            false,
        )?
        .expect("localized entry rejects absent tag");
        localized.push(LocalizedCharacterDisplayNameInput::new(locale, entry));
    }
    Ok(localized)
}

struct DecodeTables<'a> {
    strings: &'a StringTable,
    public_ids: &'a PublicIdTable,
    string_referenced: Vec<bool>,
    public_id_references: Vec<u32>,
}

impl<'a> DecodeTables<'a> {
    fn new(strings: &'a StringTable, public_ids: &'a PublicIdTable) -> Self {
        Self {
            strings,
            public_ids,
            string_referenced: vec![false; strings.len()],
            public_id_references: vec![0; public_ids.len()],
        }
    }

    fn string(&mut self, reference: u32) -> Result<String, CharacterPresentationCatalogCodecError> {
        let index = checked_usize(reference, "String reference")?;
        let Some(value) = self.strings.values().get(index) else {
            return Err(
                CharacterPresentationCatalogCodecError::ReferenceOutOfBounds {
                    table: "String",
                    index: reference,
                },
            );
        };
        self.string_referenced[index] = true;
        Ok(value.clone())
    }

    fn character(
        &mut self,
        reference: u32,
    ) -> Result<String, CharacterPresentationCatalogCodecError> {
        let index = checked_usize(reference, "PublicId reference")?;
        let Some(value) = self.public_ids.values().get(index) else {
            return Err(
                CharacterPresentationCatalogCodecError::ReferenceOutOfBounds {
                    table: "PublicId",
                    index: reference,
                },
            );
        };
        self.public_id_references[index] = self.public_id_references[index].checked_add(1).ok_or(
            CharacterPresentationCatalogCodecError::ArithmeticOverflow {
                operation: "PublicId reference count",
            },
        )?;
        Ok(value.clone())
    }

    fn require_exact_references(&self) -> Result<(), CharacterPresentationCatalogCodecError> {
        if let Some((index, _)) = self
            .string_referenced
            .iter()
            .enumerate()
            .find(|(_, referenced)| !**referenced)
        {
            return Err(CharacterPresentationCatalogCodecError::UnreferencedString {
                index: checked_u32(index, "String table index")?,
            });
        }
        if let Some((index, actual)) = self
            .public_id_references
            .iter()
            .copied()
            .enumerate()
            .find(|(_, references)| *references != 1)
        {
            return Err(
                CharacterPresentationCatalogCodecError::PublicIdReferenceCount {
                    index: checked_u32(index, "PublicId table index")?,
                    actual,
                },
            );
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_entry(
    tables: &mut DecodeTables<'_>,
    character: &CharacterId,
    locale: Option<&CharacterNameLocale>,
    tag: u8,
    key: u32,
    value: u32,
    field: FieldId,
    offset: u32,
    allow_absent: bool,
) -> Result<Option<CharacterDisplayNameInput>, CharacterPresentationCatalogCodecError> {
    match tag {
        0 if allow_absent && key == MISSING_REF && value == MISSING_REF => Ok(None),
        0 if allow_absent => {
            Err(CharacterPresentationCatalogCodecError::InvalidSentinel { field, offset })
        }
        1 if key != MISSING_REF && value != MISSING_REF => {
            let actual_key = tables.string(key)?;
            let expected_key = locale.map_or_else(
                || CharacterDisplayNameKey::for_base(character),
                |locale| CharacterDisplayNameKey::for_locale(character, locale),
            );
            let expected_key = expected_key.map_err(|source| {
                CharacterPresentationCatalogCodecError::InvalidGeneratedKey { source }
            })?;
            require_generated_key(character, &expected_key, actual_key)?;
            let value = tables.string(value)?;
            let value = CharacterDisplayNameValue::try_new(value).map_err(|source| {
                CharacterPresentationCatalogCodecError::InvalidDisplayName { source }
            })?;
            Ok(Some(CharacterDisplayNameInput::Visible(value)))
        }
        2 if key == MISSING_REF && value == MISSING_REF => {
            Ok(Some(CharacterDisplayNameInput::Hidden))
        }
        1 | 2 => Err(CharacterPresentationCatalogCodecError::InvalidSentinel { field, offset }),
        actual => Err(CharacterPresentationCatalogCodecError::UnsupportedTag {
            field,
            offset,
            kind: "display-name entry",
            actual,
        }),
    }
}

fn decode_declaration(
    tables: &mut DecodeTables<'_>,
    character: &CharacterId,
    tag: u8,
    key: u32,
    value: u32,
    offset: u32,
) -> Result<Option<CharacterDisplayNameValue>, CharacterPresentationCatalogCodecError> {
    match tag {
        0 if key == MISSING_REF && value == MISSING_REF => Ok(None),
        1 if key != MISSING_REF && value != MISSING_REF => {
            let actual_key = tables.string(key)?;
            let expected_key =
                CharacterDisplayNameKey::for_declaration(character).map_err(|source| {
                    CharacterPresentationCatalogCodecError::InvalidGeneratedKey { source }
                })?;
            require_generated_key(character, &expected_key, actual_key)?;
            let value = tables.string(value)?;
            CharacterDisplayNameValue::try_new(value)
                .map(Some)
                .map_err(
                    |source| CharacterPresentationCatalogCodecError::InvalidDisplayName { source },
                )
        }
        0 | 1 => Err(CharacterPresentationCatalogCodecError::InvalidSentinel {
            field: FIELD_CHARACTER_RECORDS,
            offset,
        }),
        actual => Err(CharacterPresentationCatalogCodecError::UnsupportedTag {
            field: FIELD_CHARACTER_RECORDS,
            offset,
            kind: "declaration",
            actual,
        }),
    }
}

fn require_generated_key(
    character: &CharacterId,
    expected: &CharacterDisplayNameKey,
    actual: String,
) -> Result<(), CharacterPresentationCatalogCodecError> {
    if actual != expected.as_str() {
        return Err(
            CharacterPresentationCatalogCodecError::GeneratedKeyMismatch {
                character: character.as_str().to_owned(),
                expected: expected.as_str().to_owned(),
                actual,
            },
        );
    }
    Ok(())
}

fn decode_locale(
    tables: &mut DecodeTables<'_>,
    reference: u32,
) -> Result<CharacterNameLocale, CharacterPresentationCatalogCodecError> {
    let value = tables.string(reference)?;
    LocaleTag::try_new(&value)
        .map(CharacterNameLocale::new)
        .map_err(|source| CharacterPresentationCatalogCodecError::InvalidLocale { value, source })
}

fn decode_character(
    tables: &mut DecodeTables<'_>,
    reference: u32,
) -> Result<CharacterId, CharacterPresentationCatalogCodecError> {
    let value = tables.character(reference)?;
    CharacterId::try_new(&value).map_err(|source| {
        CharacterPresentationCatalogCodecError::InvalidCharacterId { value, source }
    })
}

fn field(
    fields: &[ResourceField],
    id: FieldId,
) -> Result<&[u8], CharacterPresentationCatalogCodecError> {
    fields
        .iter()
        .find(|field| field.id == id)
        .map(|field| field.payload.as_slice())
        .ok_or_else(|| crate::resource_codec::SectionCodecError::MissingRequiredField(id).into())
}

fn require_header_value(
    name: &'static str,
    expected: u32,
    actual: u32,
) -> Result<(), CharacterPresentationCatalogCodecError> {
    if actual != expected {
        return Err(CharacterPresentationCatalogCodecError::HeaderValue {
            name,
            expected,
            actual,
        });
    }
    Ok(())
}

fn require_limit(
    name: &'static str,
    actual: u32,
    maximum: u32,
) -> Result<(), CharacterPresentationCatalogCodecError> {
    if actual > maximum {
        return Err(CharacterPresentationCatalogCodecError::Limit {
            name,
            maximum,
            actual,
        });
    }
    Ok(())
}

fn checked_mul(
    left: usize,
    right: usize,
    operation: &'static str,
) -> Result<usize, CharacterPresentationCatalogCodecError> {
    left.checked_mul(right)
        .ok_or(CharacterPresentationCatalogCodecError::ArithmeticOverflow { operation })
}

fn checked_u32(
    value: usize,
    operation: &'static str,
) -> Result<u32, CharacterPresentationCatalogCodecError> {
    u32::try_from(value)
        .map_err(|_| CharacterPresentationCatalogCodecError::ArithmeticOverflow { operation })
}

fn checked_usize(
    value: u32,
    operation: &'static str,
) -> Result<usize, CharacterPresentationCatalogCodecError> {
    usize::try_from(value)
        .map_err(|_| CharacterPresentationCatalogCodecError::ArithmeticOverflow { operation })
}
