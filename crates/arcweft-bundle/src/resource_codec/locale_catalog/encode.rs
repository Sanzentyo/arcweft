//! Canonical encoder for accepted Character presentation catalogs.

use super::{
    error::CharacterPresentationCatalogCodecError,
    wire::{
        FIELD_CATALOG_HEADER, FIELD_CHARACTER_RECORDS, FIELD_FALLBACK_LOCALES,
        FIELD_LOCALIZED_RECORDS, MISSING_REF, WireCatalogHeader, WireCharacterRecord,
        WireLocalizedRecord, codec_budget,
    },
};
use crate::resource_codec::{
    EnumRegistry, ProductResourceEnvelope, ProductSectionCodecKind, PublicIdTable, ResourceField,
    ResourceWireType, StringTable,
};
use arcweft_character::presentation_name::{
    CharacterDisplayNameEntry, CharacterDisplayNameKey, CharacterDisplayNameRecord,
    CharacterPresentationCatalogData, CharacterPresentationRole,
};
use std::collections::BTreeSet;

pub(super) fn encode(
    catalog: &CharacterPresentationCatalogData,
) -> Result<Vec<u8>, CharacterPresentationCatalogCodecError> {
    let budget = codec_budget();
    let (strings, public_ids) = canonical_tables(catalog);
    let strings = StringTable::with_budget(strings, budget)?;
    let public_ids = PublicIdTable::with_budget(public_ids, budget)?;
    let counts = CatalogCounts::try_from_catalog(catalog)?;
    let header_payload = encode_header(catalog, &strings, counts)?;
    let fallback_payload = encode_fallbacks(catalog, &strings)?;
    let record_payloads = encode_records(catalog, &strings, &public_ids, counts)?;
    ProductResourceEnvelope::with_budget(
        ProductSectionCodecKind::LocaleCatalog,
        strings,
        public_ids,
        EnumRegistry::default(),
        [
            ResourceField::required(
                FIELD_CATALOG_HEADER,
                ResourceWireType::Bytes,
                header_payload,
            ),
            ResourceField::required(
                FIELD_FALLBACK_LOCALES,
                ResourceWireType::Bytes,
                fallback_payload,
            ),
            ResourceField::required(
                FIELD_CHARACTER_RECORDS,
                ResourceWireType::Bytes,
                record_payloads.characters,
            ),
            ResourceField::required(
                FIELD_LOCALIZED_RECORDS,
                ResourceWireType::Bytes,
                record_payloads.localized,
            ),
        ],
        counts.record_count()?,
        budget,
    )?
    .encode_canonical()
    .map_err(Into::into)
}

#[derive(Clone, Copy)]
struct CatalogCounts {
    fallbacks: u32,
    characters: u32,
    localized: u32,
}

impl CatalogCounts {
    fn try_from_catalog(
        catalog: &CharacterPresentationCatalogData,
    ) -> Result<Self, CharacterPresentationCatalogCodecError> {
        let localized = catalog.records().iter().try_fold(0_u32, |count, record| {
            count
                .checked_add(checked_u32(
                    record.localized().len(),
                    "localized record count",
                )?)
                .ok_or(CharacterPresentationCatalogCodecError::ArithmeticOverflow {
                    operation: "total localized record count",
                })
        })?;
        Ok(Self {
            fallbacks: checked_u32(catalog.policy().fallbacks().len(), "fallback count")?,
            characters: checked_u32(catalog.records().len(), "Character count")?,
            localized,
        })
    }

    fn record_count(self) -> Result<u32, CharacterPresentationCatalogCodecError> {
        1_u32
            .checked_add(self.fallbacks)
            .and_then(|count| count.checked_add(self.characters))
            .and_then(|count| count.checked_add(self.localized))
            .ok_or(CharacterPresentationCatalogCodecError::ArithmeticOverflow {
                operation: "LocaleCatalog record count",
            })
    }
}

fn encode_header(
    catalog: &CharacterPresentationCatalogData,
    strings: &StringTable,
    counts: CatalogCounts,
) -> Result<Vec<u8>, CharacterPresentationCatalogCodecError> {
    let mut payload = Vec::with_capacity(88);
    WireCatalogHeader {
        flags: 0,
        default_active_locale: string_id(
            strings,
            catalog.policy().default_active().locale_tag().as_str(),
        )?,
        fallback_count: counts.fallbacks,
        character_count: counts.characters,
        localized_count: counts.localized,
        reserved: 0,
        semantic_digest: *catalog.semantic_digest().as_bytes(),
        locale_policy_digest: *catalog.locale_policy_digest().as_bytes(),
    }
    .encode_into(&mut payload);
    Ok(payload)
}

fn encode_fallbacks(
    catalog: &CharacterPresentationCatalogData,
    strings: &StringTable,
) -> Result<Vec<u8>, CharacterPresentationCatalogCodecError> {
    let mut payload = Vec::with_capacity(catalog.policy().fallbacks().len().saturating_mul(4));
    for fallback in catalog.policy().fallbacks() {
        payload.extend_from_slice(
            &string_id(strings, fallback.locale().locale_tag().as_str())?.to_le_bytes(),
        );
    }
    Ok(payload)
}

struct RecordPayloads {
    characters: Vec<u8>,
    localized: Vec<u8>,
}

fn encode_records(
    catalog: &CharacterPresentationCatalogData,
    strings: &StringTable,
    public_ids: &PublicIdTable,
    counts: CatalogCounts,
) -> Result<RecordPayloads, CharacterPresentationCatalogCodecError> {
    let mut payloads = RecordPayloads {
        characters: Vec::with_capacity(catalog.records().len().saturating_mul(36)),
        localized: Vec::with_capacity(
            usize::try_from(counts.localized)
                .unwrap_or(0)
                .saturating_mul(16),
        ),
    };
    let mut localized_first = 0_u32;
    for record in catalog.records() {
        let (base_tag, base_key, base_value) =
            encode_entry(record.character(), None, record.base(), strings)?;
        let (declaration_tag, declaration_key, declaration_value) =
            encode_declaration(record, strings)?;
        let localized_count = checked_u32(record.localized().len(), "localized record count")?;
        WireCharacterRecord {
            character: public_id(public_ids, record.character().as_str())?,
            role: match record.role() {
                CharacterPresentationRole::Character => 1,
                CharacterPresentationRole::Narrator => 2,
            },
            base_tag,
            declaration_tag,
            reserved: 0,
            source_locale: record.source_locale().map_or(Ok(MISSING_REF), |locale| {
                string_id(strings, locale.locale().locale_tag().as_str())
            })?,
            base_key,
            base_value,
            declaration_key,
            declaration_value,
            localized_first,
            localized_count,
        }
        .encode_into(&mut payloads.characters);
        encode_localized_records(record, strings, &mut payloads.localized)?;
        localized_first = localized_first.checked_add(localized_count).ok_or(
            CharacterPresentationCatalogCodecError::ArithmeticOverflow {
                operation: "localized record span",
            },
        )?;
    }
    Ok(payloads)
}

fn encode_declaration(
    record: &CharacterDisplayNameRecord,
    strings: &StringTable,
) -> Result<(u8, u32, u32), CharacterPresentationCatalogCodecError> {
    let Some(declaration) = record.declaration_fallback() else {
        return Ok((0, MISSING_REF, MISSING_REF));
    };
    let expected = CharacterDisplayNameKey::for_declaration(record.character())
        .map_err(|source| CharacterPresentationCatalogCodecError::InvalidGeneratedKey { source })?;
    require_generated_key(
        record.character().as_str(),
        &expected,
        declaration.key().as_str(),
    )?;
    Ok((
        1,
        string_id(strings, declaration.key().as_str())?,
        string_id(strings, declaration.value().as_str())?,
    ))
}

fn encode_localized_records(
    record: &CharacterDisplayNameRecord,
    strings: &StringTable,
    payload: &mut Vec<u8>,
) -> Result<(), CharacterPresentationCatalogCodecError> {
    for localized in record.localized() {
        let (entry_tag, key, value) = encode_required_entry(
            record.character(),
            Some(localized.locale()),
            localized.entry(),
            strings,
        )?;
        WireLocalizedRecord {
            locale: string_id(strings, localized.locale().locale_tag().as_str())?,
            entry_tag,
            reserved: [0; 3],
            key,
            value,
        }
        .encode_into(payload);
    }
    Ok(())
}

fn canonical_tables(catalog: &CharacterPresentationCatalogData) -> (Vec<String>, Vec<String>) {
    let mut strings = BTreeSet::new();
    strings.insert(
        catalog
            .policy()
            .default_active()
            .locale_tag()
            .as_str()
            .to_owned(),
    );
    for fallback in catalog.policy().fallbacks() {
        strings.insert(fallback.locale().locale_tag().as_str().to_owned());
    }

    let mut public_ids = Vec::with_capacity(catalog.records().len());
    for record in catalog.records() {
        public_ids.push(record.character().as_public_id().as_str().to_owned());
        if let Some(source_locale) = record.source_locale() {
            strings.insert(source_locale.locale().locale_tag().as_str().to_owned());
        }
        add_entry_strings(record.base(), &mut strings);
        for localized in record.localized() {
            strings.insert(localized.locale().locale_tag().as_str().to_owned());
            add_entry_strings(Some(localized.entry()), &mut strings);
        }
        if let Some(declaration) = record.declaration_fallback() {
            strings.insert(declaration.key().as_str().to_owned());
            strings.insert(declaration.value().as_str().to_owned());
        }
    }
    (strings.into_iter().collect(), public_ids)
}

fn add_entry_strings(entry: Option<&CharacterDisplayNameEntry>, strings: &mut BTreeSet<String>) {
    if let Some(CharacterDisplayNameEntry::Visible { key, value }) = entry {
        strings.insert(key.as_str().to_owned());
        strings.insert(value.as_str().to_owned());
    }
}

fn encode_entry(
    character: &arcweft_character::id::CharacterId,
    locale: Option<&arcweft_character::presentation_name::CharacterNameLocale>,
    entry: Option<&CharacterDisplayNameEntry>,
    strings: &StringTable,
) -> Result<(u8, u32, u32), CharacterPresentationCatalogCodecError> {
    entry.map_or(Ok((0, MISSING_REF, MISSING_REF)), |entry| {
        encode_required_entry(character, locale, entry, strings)
    })
}

fn encode_required_entry(
    character: &arcweft_character::id::CharacterId,
    locale: Option<&arcweft_character::presentation_name::CharacterNameLocale>,
    entry: &CharacterDisplayNameEntry,
    strings: &StringTable,
) -> Result<(u8, u32, u32), CharacterPresentationCatalogCodecError> {
    match entry {
        CharacterDisplayNameEntry::Visible { key, value } => {
            let expected = locale.map_or_else(
                || CharacterDisplayNameKey::for_base(character),
                |locale| CharacterDisplayNameKey::for_locale(character, locale),
            );
            let expected = expected.map_err(|source| {
                CharacterPresentationCatalogCodecError::InvalidGeneratedKey { source }
            })?;
            require_generated_key(character.as_str(), &expected, key.as_str())?;
            Ok((
                1,
                string_id(strings, key.as_str())?,
                string_id(strings, value.as_str())?,
            ))
        }
        CharacterDisplayNameEntry::Hidden => Ok((2, MISSING_REF, MISSING_REF)),
    }
}

fn require_generated_key(
    character: &str,
    expected: &CharacterDisplayNameKey,
    actual: &str,
) -> Result<(), CharacterPresentationCatalogCodecError> {
    if expected.as_str() != actual {
        return Err(
            CharacterPresentationCatalogCodecError::GeneratedKeyMismatch {
                character: character.to_owned(),
                expected: expected.as_str().to_owned(),
                actual: actual.to_owned(),
            },
        );
    }
    Ok(())
}

fn string_id(
    table: &StringTable,
    value: &str,
) -> Result<u32, CharacterPresentationCatalogCodecError> {
    table.id_for(value).map(|id| id.0).ok_or_else(|| {
        CharacterPresentationCatalogCodecError::MissingCanonicalEntry {
            table: "String",
            value: value.to_owned(),
        }
    })
}

fn public_id(
    table: &PublicIdTable,
    value: &str,
) -> Result<u32, CharacterPresentationCatalogCodecError> {
    table.id_for(value).map(|id| id.0).ok_or_else(|| {
        CharacterPresentationCatalogCodecError::MissingCanonicalEntry {
            table: "PublicId",
            value: value.to_owned(),
        }
    })
}

fn checked_u32(
    value: usize,
    operation: &'static str,
) -> Result<u32, CharacterPresentationCatalogCodecError> {
    u32::try_from(value)
        .map_err(|_| CharacterPresentationCatalogCodecError::ArithmeticOverflow { operation })
}
