use arcweft_bundle::resource_codec::{
    CharacterPresentationCatalogCodecError, CharacterPresentationCatalogSection, FieldId,
    FieldRequirement, ProductSectionCodecKind, SectionCodecError,
    product_catalog::migrated_product_catalog_section_compatibility,
};
use arcweft_bundle::{container::BundleSectionKind, patch::PatchCompatibility};
use arcweft_character::{
    id::CharacterId,
    presentation_name::{
        CharacterDisplayNameInput, CharacterDisplayNameRecordInput, CharacterDisplayNameValue,
        CharacterNameFallbackLocale, CharacterNameLocale, CharacterNameLocalePolicy,
        CharacterNameSourceLocale, CharacterPresentationCatalogData,
        CharacterPresentationCatalogInput, CharacterPresentationRole,
        LocalizedCharacterDisplayNameInput,
    },
};
use arcweft_id::LocaleTag;

#[test]
fn locale_catalog_round_trips_the_only_canonical_compact_family() {
    let catalog = fixture_catalog();
    let bytes =
        CharacterPresentationCatalogSection::encode_canonical(&catalog).expect("catalog encodes");
    let decoded =
        CharacterPresentationCatalogSection::decode_canonical(&bytes).expect("catalog decodes");

    assert_eq!(&bytes[..8], b"AWLC\r\n\x1a\n");
    assert_eq!(read_u32(&bytes, 8), 1);
    assert_eq!(read_u32(&bytes, 12), 14);
    assert_eq!(read_u32(&bytes, 24), 0);
    assert_eq!(read_u32(&bytes, 28), 4);
    assert_eq!(decoded.records().len(), 2);
    assert_eq!(
        decoded
            .policy()
            .fallbacks()
            .iter()
            .map(|fallback| fallback.locale().locale_tag().as_str())
            .collect::<Vec<_>>(),
        ["en", "fr"]
    );
    assert_eq!(decoded.semantic_digest(), catalog.semantic_digest());
    assert_eq!(
        decoded.locale_policy_digest(),
        catalog.locale_policy_digest()
    );
    assert_eq!(
        CharacterPresentationCatalogSection::encode_canonical(&decoded)
            .expect("decoded catalog re-encodes"),
        bytes
    );
}

#[test]
fn locale_catalog_rejects_reordered_fields_and_wrong_requirement_flags() {
    let canonical =
        CharacterPresentationCatalogSection::encode_canonical(&fixture_catalog()).expect("encodes");
    let blocks = field_blocks(&canonical);
    let mut reordered = canonical.clone();
    let first = reordered[blocks[0].0..blocks[0].1].to_vec();
    let second = reordered[blocks[1].0..blocks[1].1].to_vec();
    assert_eq!(first.len(), 100);
    let mut replacement = Vec::with_capacity(first.len() + second.len());
    replacement.extend_from_slice(&second);
    replacement.extend_from_slice(&first);
    reordered.splice(blocks[0].0..blocks[1].1, replacement);
    assert!(matches!(
        CharacterPresentationCatalogSection::decode_canonical(&reordered),
        Err(CharacterPresentationCatalogCodecError::Envelope(
            SectionCodecError::NonCanonicalFieldOrder {
                previous: FieldId(2),
                current: FieldId(1),
            }
        ))
    ));

    let mut wrong_requirement = canonical;
    wrong_requirement[blocks[0].0 + 3] = 0;
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&wrong_requirement)
            .expect_err("optional header field rejects"),
        CharacterPresentationCatalogCodecError::Envelope(
            SectionCodecError::FieldRequirementMismatch {
                field: FieldId(1),
                expected: FieldRequirement::Required,
                actual: FieldRequirement::Optional,
            }
        )
    );
}

#[test]
fn locale_catalog_rejects_digest_key_reference_and_size_tampering() {
    let canonical =
        CharacterPresentationCatalogSection::encode_canonical(&fixture_catalog()).expect("encodes");

    let mut stale_digest = canonical.clone();
    let header_payload = field_payload_offset(&stale_digest, 1);
    stale_digest[header_payload + 24] ^= 1;
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&stale_digest)
            .expect_err("stale digest rejects"),
        CharacterPresentationCatalogCodecError::SemanticDigestMismatch
    );

    let mut wrong_key = canonical.clone();
    let key = CharacterDisplayNameRecordInput::try_new(
        character("character.alice"),
        CharacterPresentationRole::Character,
        None,
        Some(visible("Alice")),
        Vec::new(),
        None,
    )
    .expect("record");
    let key_catalog = CharacterPresentationCatalogData::try_from_inputs(
        CharacterPresentationCatalogInput::try_new(policy("ja-JP", &[]), vec![key]).expect("input"),
    )
    .expect("catalog");
    let expected_key = key_catalog.records()[0]
        .base()
        .and_then(|entry| entry.key())
        .expect("visible base")
        .as_str()
        .to_owned();
    let key_range = string_entry_range(&wrong_key, &expected_key);
    *wrong_key
        .get_mut(key_range.end - 1)
        .expect("key byte exists") = b'f';
    assert!(matches!(
        CharacterPresentationCatalogSection::decode_canonical(&wrong_key),
        Err(CharacterPresentationCatalogCodecError::GeneratedKeyMismatch { .. })
    ));

    let mut unreferenced_string = canonical.clone();
    let insertion = string_table_end(&unreferenced_string);
    unreferenced_string.splice(insertion..insertion, [4, 0, 0, 0, 0xf4, 0x8f, 0xbf, 0xbf]);
    let string_count = read_u32(&unreferenced_string, 16);
    write_u32(&mut unreferenced_string, 16, string_count + 1);
    let body_len = read_u64(&unreferenced_string, 40);
    write_u64(&mut unreferenced_string, 40, body_len + 8);
    assert!(matches!(
        CharacterPresentationCatalogSection::decode_canonical(&unreferenced_string),
        Err(CharacterPresentationCatalogCodecError::UnreferencedString { .. })
    ));

    let mut too_many_strings = canonical.clone();
    write_u32(&mut too_many_strings, 16, 1_000_001);
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&too_many_strings)
            .expect_err("one-over String table count rejects"),
        CharacterPresentationCatalogCodecError::Envelope(SectionCodecError::BudgetExceeded(
            "strings"
        ))
    );
}

#[test]
fn locale_catalog_rejects_truncation_trailing_bytes_and_invalid_character_family() {
    let canonical =
        CharacterPresentationCatalogSection::encode_canonical(&fixture_catalog()).expect("encodes");
    assert!(matches!(
        CharacterPresentationCatalogSection::decode_canonical(&canonical[..canonical.len() - 1]),
        Err(CharacterPresentationCatalogCodecError::Envelope(
            SectionCodecError::Truncated
        ))
    ));

    let mut trailing = canonical.clone();
    trailing.push(0);
    assert!(matches!(
        CharacterPresentationCatalogSection::decode_canonical(&trailing),
        Err(CharacterPresentationCatalogCodecError::Envelope(
            SectionCodecError::TrailingBytes
        ))
    ));

    let mut wrong_family = canonical;
    let id_range = public_id_entry_range(&wrong_family, "character.alice");
    assert_eq!(id_range.len(), "character.alice".len());
    wrong_family[id_range].copy_from_slice(b"assetxxxx.alice");
    assert!(matches!(
        CharacterPresentationCatalogSection::decode_canonical(&wrong_family),
        Err(CharacterPresentationCatalogCodecError::InvalidCharacterId { .. })
    ));
}

#[test]
fn locale_catalog_rejects_header_table_and_locale_policy_tampering() {
    let canonical =
        CharacterPresentationCatalogSection::encode_canonical(&fixture_catalog()).expect("encodes");

    let mut bad_magic = canonical.clone();
    bad_magic[0] ^= 1;
    assert!(matches!(
        CharacterPresentationCatalogSection::decode_canonical(&bad_magic),
        Err(CharacterPresentationCatalogCodecError::Envelope(
            SectionCodecError::BadMagic { .. }
        ))
    ));

    let mut bad_schema = canonical.clone();
    write_u32(&mut bad_schema, 8, 2);
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&bad_schema)
            .expect_err("unsupported schema rejects"),
        CharacterPresentationCatalogCodecError::Envelope(SectionCodecError::UnsupportedSchema {
            actual: 2,
            expected: 1,
        })
    );

    let mut bad_codec = canonical.clone();
    write_u32(&mut bad_codec, 12, 999);
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&bad_codec)
            .expect_err("unknown codec rejects"),
        CharacterPresentationCatalogCodecError::Envelope(SectionCodecError::UnsupportedCodecTag(
            999
        ))
    );

    let mut malformed_utf8 = canonical.clone();
    let alice = string_entry_range(&malformed_utf8, "Alice");
    malformed_utf8[alice.start] = 0xff;
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&malformed_utf8)
            .expect_err("malformed UTF-8 rejects"),
        CharacterPresentationCatalogCodecError::Envelope(SectionCodecError::InvalidUtf8("strings"))
    );

    let mut duplicate_string = canonical.clone();
    let en = string_entry_range(&duplicate_string, "en");
    let fr = string_entry_range(&duplicate_string, "fr");
    let en_bytes = duplicate_string[en].to_vec();
    duplicate_string[fr].copy_from_slice(&en_bytes);
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&duplicate_string)
            .expect_err("duplicate String rejects"),
        CharacterPresentationCatalogCodecError::Envelope(SectionCodecError::DuplicateString(
            "en".to_owned()
        ))
    );

    let mut invalid_locale = canonical.clone();
    let fr = string_entry_range(&invalid_locale, "fr");
    invalid_locale[fr].copy_from_slice(b"f_");
    assert!(matches!(
        CharacterPresentationCatalogSection::decode_canonical(&invalid_locale),
        Err(CharacterPresentationCatalogCodecError::InvalidLocale { .. })
    ));

    let mut stale_policy = canonical;
    let header_payload = field_payload_offset(&stale_policy, 1);
    stale_policy[header_payload + 56] ^= 1;
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&stale_policy)
            .expect_err("stale policy digest rejects"),
        CharacterPresentationCatalogCodecError::LocalePolicyDigestMismatch
    );
}

#[test]
fn locale_catalog_rejects_public_id_record_order_span_and_field_tampering() {
    let canonical =
        CharacterPresentationCatalogSection::encode_canonical(&fixture_catalog()).expect("encodes");

    let mut duplicate_public_id = canonical.clone();
    let alice = public_id_entry_range(&duplicate_public_id, "character.alice");
    let bob = public_id_entry_range(&duplicate_public_id, "character.bobxx");
    let alice_bytes = duplicate_public_id[alice].to_vec();
    duplicate_public_id[bob].copy_from_slice(&alice_bytes);
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&duplicate_public_id)
            .expect_err("duplicate PublicId rejects"),
        CharacterPresentationCatalogCodecError::Envelope(SectionCodecError::DuplicatePublicId(
            "character.alice".to_owned()
        ))
    );

    let order_catalog = CharacterPresentationCatalogData::try_from_inputs(
        CharacterPresentationCatalogInput::try_new(
            policy("ja-JP", &[]),
            vec![
                CharacterDisplayNameRecordInput::try_new(
                    character("character.alice"),
                    CharacterPresentationRole::Character,
                    None,
                    Some(CharacterDisplayNameInput::Hidden),
                    Vec::new(),
                    None,
                )
                .expect("Alice order record"),
                CharacterDisplayNameRecordInput::try_new(
                    character("character.bobxx"),
                    CharacterPresentationRole::Character,
                    None,
                    Some(CharacterDisplayNameInput::Hidden),
                    Vec::new(),
                    None,
                )
                .expect("Bob order record"),
            ],
        )
        .expect("order catalog input"),
    )
    .expect("order catalog");
    let mut character_order =
        CharacterPresentationCatalogSection::encode_canonical(&order_catalog).expect("encodes");
    let character_payload = field_payload_offset(&character_order, 3);
    let first = character_order[character_payload..character_payload + 36].to_vec();
    let second = character_order[character_payload + 36..character_payload + 72].to_vec();
    character_order[character_payload..character_payload + 36].copy_from_slice(&second);
    character_order[character_payload + 36..character_payload + 72].copy_from_slice(&first);
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&character_order)
            .expect_err("Character order rejects"),
        CharacterPresentationCatalogCodecError::NonCanonicalRecordOrder { table: "Character" }
    );

    let mut localized_order = canonical.clone();
    let localized_payload = field_payload_offset(&localized_order, 4);
    let first = localized_order[localized_payload..localized_payload + 16].to_vec();
    let second = localized_order[localized_payload + 16..localized_payload + 32].to_vec();
    localized_order[localized_payload..localized_payload + 16].copy_from_slice(&second);
    localized_order[localized_payload + 16..localized_payload + 32].copy_from_slice(&first);
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&localized_order)
            .expect_err("localized order rejects"),
        CharacterPresentationCatalogCodecError::NonCanonicalRecordOrder { table: "localized" }
    );

    let mut invalid_span = canonical.clone();
    let character_payload = field_payload_offset(&invalid_span, 3);
    write_u32(&mut invalid_span, character_payload + 28, 1);
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&invalid_span)
            .expect_err("localized span rejects"),
        CharacterPresentationCatalogCodecError::InvalidLocalizedSpan
    );

    let blocks = field_blocks(&canonical);
    let mut unknown_field = canonical.clone();
    unknown_field[blocks[3].0..blocks[3].0 + 2].copy_from_slice(&5_u16.to_le_bytes());
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&unknown_field)
            .expect_err("unknown closed-family field rejects"),
        CharacterPresentationCatalogCodecError::Envelope(SectionCodecError::UnknownRequiredField(
            FieldId(5)
        ))
    );

    let mut missing_field = canonical;
    let removed = blocks[3].1 - blocks[3].0;
    missing_field.truncate(blocks[3].0);
    write_u32(&mut missing_field, 28, 3);
    let body_len = read_u64(&missing_field, 40);
    write_u64(
        &mut missing_field,
        40,
        body_len - u64::try_from(removed).expect("field length"),
    );
    assert_eq!(
        CharacterPresentationCatalogSection::decode_canonical(&missing_field)
            .expect_err("missing field rejects"),
        CharacterPresentationCatalogCodecError::Envelope(SectionCodecError::MissingRequiredField(
            FieldId(4)
        ))
    );
}

#[test]
fn locale_catalog_codec_kind_owns_tag_magic_section_and_patch_policy() {
    let codec = ProductSectionCodecKind::LocaleCatalog;
    assert_eq!(codec.encoded(), 14);
    assert_eq!(codec.magic(), *b"AWLC\r\n\x1a\n");
    assert_eq!(codec.as_str(), "locale_catalog");
    assert_eq!(
        ProductSectionCodecKind::from_encoded(14),
        Some(ProductSectionCodecKind::LocaleCatalog)
    );
    assert!(!codec.affects_code_compatibility());

    let bytes =
        CharacterPresentationCatalogSection::encode_canonical(&fixture_catalog()).expect("encodes");
    assert_eq!(
        migrated_product_catalog_section_compatibility(
            BundleSectionKind::LocaleCatalog,
            &bytes,
            &bytes,
        )
        .expect("canonical sections are comparable"),
        Some(PatchCompatibility::ContentOnly)
    );
    let mut malformed = bytes;
    malformed[0] ^= 1;
    assert!(
        migrated_product_catalog_section_compatibility(
            BundleSectionKind::LocaleCatalog,
            &malformed,
            &malformed,
        )
        .is_err(),
        "patch compatibility must strictly decode both LocaleCatalog sections"
    );
}

fn fixture_catalog() -> CharacterPresentationCatalogData {
    let alice = CharacterDisplayNameRecordInput::try_new(
        character("character.alice"),
        CharacterPresentationRole::Character,
        Some(CharacterNameSourceLocale::new(locale("ja-JP"))),
        Some(visible("Alice")),
        vec![
            LocalizedCharacterDisplayNameInput::new(
                locale("en"),
                CharacterDisplayNameInput::Hidden,
            ),
            LocalizedCharacterDisplayNameInput::new(locale("ja-JP"), visible("アリス")),
        ],
        Some(name("Alice declaration")),
    )
    .expect("Alice record");
    let narrator = CharacterDisplayNameRecordInput::try_new(
        character("character.bobxx"),
        CharacterPresentationRole::Narrator,
        None,
        Some(CharacterDisplayNameInput::Hidden),
        Vec::new(),
        None,
    )
    .expect("narrator record");
    CharacterPresentationCatalogData::try_from_inputs(
        CharacterPresentationCatalogInput::try_new(
            policy("ja-JP", &["en", "fr"]),
            vec![narrator, alice],
        )
        .expect("catalog input"),
    )
    .expect("accepted catalog")
}

fn character(value: &str) -> CharacterId {
    CharacterId::try_new(value).expect("Character id")
}

fn locale(value: &str) -> CharacterNameLocale {
    CharacterNameLocale::new(LocaleTag::try_new(value).expect("locale"))
}

fn policy(active: &str, fallbacks: &[&str]) -> CharacterNameLocalePolicy {
    CharacterNameLocalePolicy::try_new(
        locale(active),
        fallbacks
            .iter()
            .map(|fallback| CharacterNameFallbackLocale::new(locale(fallback)))
            .collect(),
    )
    .expect("policy")
}

fn name(value: &str) -> CharacterDisplayNameValue {
    CharacterDisplayNameValue::try_new(value).expect("display name")
}

fn visible(value: &str) -> CharacterDisplayNameInput {
    CharacterDisplayNameInput::Visible(name(value))
}

fn field_blocks(bytes: &[u8]) -> Vec<(usize, usize)> {
    let mut offset = table_end(bytes);
    let mut fields = Vec::new();
    for _ in 0..read_u32(bytes, 28) {
        let payload_len = read_u32(bytes, offset + 8) as usize;
        let end = offset + 12 + payload_len;
        fields.push((offset, end));
        offset = end;
    }
    fields
}

fn field_payload_offset(bytes: &[u8], field: u16) -> usize {
    field_blocks(bytes)
        .into_iter()
        .find(|(start, _)| read_u16(bytes, *start) == field)
        .map(|(start, _)| start + 12)
        .expect("field exists")
}

fn table_end(bytes: &[u8]) -> usize {
    let mut offset = string_table_end(bytes);
    for _ in 0..read_u32(bytes, 20) {
        offset += 4 + read_u32(bytes, offset) as usize;
    }
    offset + read_u32(bytes, 24) as usize * 8
}

fn string_table_end(bytes: &[u8]) -> usize {
    let mut offset = 48;
    for _ in 0..read_u32(bytes, 16) {
        offset += 4 + read_u32(bytes, offset) as usize;
    }
    offset
}

fn string_entry_range(bytes: &[u8], expected: &str) -> std::ops::Range<usize> {
    let mut offset = 48;
    for _ in 0..read_u32(bytes, 16) {
        let len = read_u32(bytes, offset) as usize;
        let range = offset + 4..offset + 4 + len;
        if &bytes[range.clone()] == expected.as_bytes() {
            return range;
        }
        offset = range.end;
    }
    panic!("String entry `{expected}` exists")
}

fn public_id_entry_range(bytes: &[u8], expected: &str) -> std::ops::Range<usize> {
    let mut offset = string_table_end(bytes);
    for _ in 0..read_u32(bytes, 20) {
        let len = read_u32(bytes, offset) as usize;
        let range = offset + 4..offset + 4 + len;
        if &bytes[range.clone()] == expected.as_bytes() {
            return range;
        }
        offset = range.end;
    }
    panic!("PublicId entry `{expected}` exists")
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("u16"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32"))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64"))
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
