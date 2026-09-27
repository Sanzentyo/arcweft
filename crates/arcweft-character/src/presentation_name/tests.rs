use super::{
    AcceptedCharacterPresentationCatalog, CharacterDisplayNameEntry, CharacterDisplayNameInput,
    CharacterDisplayNameKey, CharacterDisplayNameLookupError, CharacterDisplayNameRecordInput,
    CharacterDisplayNameResolutionSource, CharacterDisplayNameValue,
    CharacterDisplayNameValueError, CharacterNameLocale, CharacterNameResolutionLocales,
    CharacterNameSourceLocale, CharacterPresentationCatalogData, CharacterPresentationCatalogError,
    CharacterPresentationCatalogInput, CharacterPresentationCatalogRevision,
    CharacterPresentationRole, DigestParseError, LocalizedCharacterDisplayNameInput,
};
use crate::id::CharacterId;
use arcweft_id::LocaleTag;

fn locale(value: &str) -> CharacterNameLocale {
    CharacterNameLocale::new(LocaleTag::try_new(value).expect("canonical locale"))
}

fn tag(value: &str) -> LocaleTag {
    LocaleTag::try_new(value).expect("canonical locale")
}

fn visible(value: &str) -> CharacterDisplayNameInput {
    CharacterDisplayNameInput::Visible(
        CharacterDisplayNameValue::try_new(value).expect("visible display name"),
    )
}

fn localized(
    locale_value: &str,
    entry: CharacterDisplayNameInput,
) -> LocalizedCharacterDisplayNameInput {
    LocalizedCharacterDisplayNameInput::new(locale(locale_value), entry)
}

fn resolution_locales<'a>(
    active: &'a LocaleTag,
    source: &'a LocaleTag,
    fallbacks: &'a [LocaleTag],
) -> CharacterNameResolutionLocales<'a> {
    CharacterNameResolutionLocales::new(active, source, fallbacks)
}

fn record(
    character: &str,
    source_locale: Option<&str>,
    base: Option<CharacterDisplayNameInput>,
    localized_names: Vec<LocalizedCharacterDisplayNameInput>,
    declaration_fallback: Option<&str>,
) -> CharacterDisplayNameRecordInput {
    CharacterDisplayNameRecordInput::try_new(
        CharacterId::try_new(character).expect("Character ID"),
        CharacterPresentationRole::Character,
        source_locale.map(|value| CharacterNameSourceLocale::new(locale(value))),
        base,
        localized_names,
        declaration_fallback
            .map(|value| CharacterDisplayNameValue::try_new(value).expect("declaration fallback")),
    )
    .expect("Character display-name record")
}

fn catalog(records: Vec<CharacterDisplayNameRecordInput>) -> CharacterPresentationCatalogData {
    let input = CharacterPresentationCatalogInput::try_new(records).expect("catalog input");
    CharacterPresentationCatalogData::try_from_inputs(input).expect("accepted catalog")
}

#[test]
fn visible_name_limits_and_scalar_rules_are_exact() {
    let exact_bytes = "😀".repeat(256);
    assert_eq!(exact_bytes.len(), 1_024);
    assert!(CharacterDisplayNameValue::try_new(exact_bytes).is_ok());
    assert_eq!(
        CharacterDisplayNameValue::try_new(format!("{}a", "😀".repeat(256))),
        Err(CharacterDisplayNameValueError::TooManyBytes {
            bytes: 1_025,
            maximum: 1_024,
        })
    );

    assert!(CharacterDisplayNameValue::try_new("a".repeat(256)).is_ok());
    assert_eq!(
        CharacterDisplayNameValue::try_new("a".repeat(257)),
        Err(CharacterDisplayNameValueError::TooManyScalars {
            scalars: 257,
            maximum: 256,
        })
    );
    assert_eq!(
        CharacterDisplayNameValue::try_new(" \u{3000} "),
        Err(CharacterDisplayNameValueError::WhitespaceOnly)
    );
    assert_eq!(
        CharacterDisplayNameValue::try_new(" Alice"),
        Err(CharacterDisplayNameValueError::LeadingWhitespace)
    );
    assert_eq!(
        CharacterDisplayNameValue::try_new("Alice "),
        Err(CharacterDisplayNameValueError::TrailingWhitespace)
    );
    assert_eq!(
        CharacterDisplayNameValue::try_new("Ali\u{0}ce"),
        Err(CharacterDisplayNameValueError::Control { scalar_index: 3 })
    );
}

#[test]
fn generated_keys_hex_encode_exact_canonical_bytes() {
    let character = CharacterId::try_new("character.alice").unwrap();
    let base = CharacterDisplayNameKey::for_base(&character).unwrap();
    let localized = CharacterDisplayNameKey::for_locale(&character, &locale("ja-JP")).unwrap();
    let declaration = CharacterDisplayNameKey::for_declaration(&character).unwrap();

    assert_eq!(
        base.as_str(),
        "character.display_name.6368617261637465722e616c696365.base"
    );
    assert_eq!(
        localized.as_str(),
        "character.display_name.6368617261637465722e616c696365.locale.6a612d4a50"
    );
    assert_eq!(
        declaration.as_str(),
        "character.display_name.6368617261637465722e616c696365.declaration"
    );
}

#[test]
fn catalog_canonicalizes_records_and_localized_entries() {
    let data = catalog(vec![
        record(
            "character.zed",
            None,
            Some(visible("Zed")),
            Vec::new(),
            None,
        ),
        record(
            "character.alice",
            None,
            None,
            vec![
                localized("fr", visible("Alice FR")),
                localized("en", visible("Alice")),
            ],
            None,
        ),
    ]);

    assert_eq!(data.records()[0].character().as_str(), "character.alice");
    assert_eq!(
        data.records()[0]
            .localized()
            .iter()
            .map(|entry| entry.locale().locale_tag().as_str())
            .collect::<Vec<_>>(),
        ["en", "fr"]
    );
    assert!(matches!(
        data.records()[1].base(),
        Some(CharacterDisplayNameEntry::Visible { .. })
    ));
}

#[test]
fn resolution_obeys_exact_order_and_hidden_is_terminal() {
    let data = catalog(vec![record(
        "character.alice",
        Some("de"),
        Some(visible("Base")),
        vec![
            localized("en", CharacterDisplayNameInput::Hidden),
            localized("fr", visible("Alice FR")),
            localized("de", visible("Alice DE")),
        ],
        Some("Alice declaration"),
    )]);
    let character = CharacterId::try_new("character.alice").unwrap();

    let hidden = data
        .resolve(
            &character,
            resolution_locales(&tag("ja-JP"), &tag("ja-JP"), &[tag("en"), tag("fr")]),
        )
        .unwrap();
    assert!(hidden.is_hidden());
    assert_eq!(hidden.value(), "");
    assert_eq!(hidden.key(), None);
    assert_eq!(
        hidden.source(),
        CharacterDisplayNameResolutionSource::ProjectFallback { ordinal: 0 }
    );

    let active = data
        .resolve(
            &character,
            resolution_locales(&tag("fr"), &tag("ja-JP"), &[tag("en")]),
        )
        .unwrap();
    assert_eq!(active.value(), "Alice FR");
    assert_eq!(
        active.source(),
        CharacterDisplayNameResolutionSource::ActiveLocale
    );
}

#[test]
fn resolution_uses_source_base_and_declaration_without_inference() {
    let source = catalog(vec![record(
        "character.source",
        Some("de"),
        None,
        vec![localized("de", visible("Quelle"))],
        None,
    )]);
    let resolved = source
        .resolve(
            &CharacterId::try_new("character.source").unwrap(),
            resolution_locales(&tag("ja-JP"), &tag("ja-JP"), &[tag("fr")]),
        )
        .unwrap();
    assert_eq!(resolved.value(), "Quelle");
    assert_eq!(
        resolved.source(),
        CharacterDisplayNameResolutionSource::CharacterSourceLocale
    );

    let base = catalog(vec![record(
        "character.base",
        None,
        Some(visible("Base")),
        Vec::new(),
        Some("Declaration"),
    )]);
    assert_eq!(
        base.resolve(
            &CharacterId::try_new("character.base").unwrap(),
            resolution_locales(&tag("ja-JP"), &tag("ja-JP"), &[])
        )
        .unwrap()
        .source(),
        CharacterDisplayNameResolutionSource::Base
    );

    let declaration = catalog(vec![record(
        "character.declaration",
        None,
        None,
        vec![localized("de", visible("Nicht ausgewählt"))],
        Some("Declaration"),
    )]);
    assert_eq!(
        declaration
            .resolve(
                &CharacterId::try_new("character.declaration").unwrap(),
                resolution_locales(&tag("ja-JP"), &tag("ja-JP"), &[])
            )
            .unwrap()
            .source(),
        CharacterDisplayNameResolutionSource::DeclarationName
    );
}

#[test]
fn project_source_is_the_final_locale_probe_and_duplicate_steps_are_skipped() {
    let data = catalog(vec![record(
        "character.alice",
        None,
        None,
        vec![localized("ja-JP", visible("アリス"))],
        None,
    )]);
    let character = CharacterId::try_new("character.alice").unwrap();
    let resolved = data
        .resolve(
            &character,
            resolution_locales(&tag("fr"), &tag("ja-JP"), &[tag("fr"), tag("en")]),
        )
        .unwrap();
    assert_eq!(resolved.value(), "アリス");
    assert_eq!(
        resolved.source(),
        CharacterDisplayNameResolutionSource::ProjectSourceLocale
    );
}

#[test]
fn exhausted_resolution_reports_exact_attempted_locales() {
    let data = catalog(vec![record(
        "character.alice",
        None,
        None,
        vec![localized("de", visible("Alice DE"))],
        None,
    )]);
    let character = CharacterId::try_new("character.alice").unwrap();
    assert_eq!(
        data.resolve(
            &character,
            resolution_locales(&tag("ja-JP"), &tag("en"), &[tag("fr")]),
        ),
        Err(CharacterDisplayNameLookupError::MissingAcceptedName {
            character,
            active: locale("ja-JP"),
            attempted_locales: vec![locale("ja-JP"), locale("fr"), locale("en")].into_boxed_slice(),
            has_base: false,
            has_declaration: false,
        })
    );

    let parent_only = catalog(vec![record(
        "character.parent_only",
        None,
        None,
        vec![localized("ja", visible("親ロケール"))],
        None,
    )]);
    assert!(matches!(
        parent_only.resolve(
            &CharacterId::try_new("character.parent_only").unwrap(),
            resolution_locales(&tag("ja-JP"), &tag("ja-JP"), &[]),
        ),
        Err(CharacterDisplayNameLookupError::MissingAcceptedName { .. })
    ));
}

#[test]
fn record_constraints_reject_duplicates_and_invalid_roles() {
    let duplicate = CharacterDisplayNameRecordInput::try_new(
        CharacterId::try_new("character.alice").unwrap(),
        CharacterPresentationRole::Character,
        None,
        None,
        vec![localized("en", visible("A")), localized("en", visible("B"))],
        None,
    );
    assert!(matches!(
        duplicate,
        Err(CharacterPresentationCatalogError::DuplicateLocale {
            first: 0,
            duplicate: 1,
            ..
        })
    ));

    let narrator = CharacterDisplayNameRecordInput::try_new(
        CharacterId::try_new("character.narrator").unwrap(),
        CharacterPresentationRole::Narrator,
        None,
        None,
        Vec::new(),
        None,
    );
    assert!(matches!(
        narrator,
        Err(CharacterPresentationCatalogError::NarratorRequiresBase { .. })
    ));

    let narrator_hidden = CharacterDisplayNameRecordInput::try_new(
        CharacterId::try_new("character.narrator").unwrap(),
        CharacterPresentationRole::Narrator,
        None,
        Some(CharacterDisplayNameInput::Hidden),
        Vec::new(),
        None,
    )
    .unwrap();
    let narrator_catalog = catalog(vec![narrator_hidden]);
    let resolved = narrator_catalog
        .resolve(
            &CharacterId::try_new("character.narrator").unwrap(),
            resolution_locales(&tag("ja-JP"), &tag("ja-JP"), &[]),
        )
        .unwrap();
    assert!(resolved.is_hidden());

    let narrator_with_declaration = CharacterDisplayNameRecordInput::try_new(
        CharacterId::try_new("character.narrator").unwrap(),
        CharacterPresentationRole::Narrator,
        None,
        Some(visible("Narrator")),
        Vec::new(),
        Some(CharacterDisplayNameValue::try_new("narrator").unwrap()),
    );
    assert!(matches!(
        narrator_with_declaration,
        Err(CharacterPresentationCatalogError::NarratorForbidsDeclarationFallback { .. })
    ));
}

#[test]
fn catalog_input_rejects_missing_source_locale_and_duplicate_characters() {
    let missing_source = CharacterDisplayNameRecordInput::try_new(
        CharacterId::try_new("character.alice").unwrap(),
        CharacterPresentationRole::Character,
        Some(CharacterNameSourceLocale::new(locale("ja-JP"))),
        Some(visible("Alice")),
        vec![localized("en", visible("Alice"))],
        None,
    );
    assert!(matches!(
        missing_source,
        Err(CharacterPresentationCatalogError::SourceLocaleWithoutEntry { .. })
    ));

    let first = record(
        "character.alice",
        None,
        Some(visible("Alice")),
        Vec::new(),
        None,
    );
    let duplicate = record(
        "character.alice",
        None,
        Some(visible("Alice 2")),
        Vec::new(),
        None,
    );
    assert!(matches!(
        CharacterPresentationCatalogInput::try_new(vec![first, duplicate]),
        Err(CharacterPresentationCatalogError::DuplicateCharacter {
            first: 0,
            duplicate: 1,
            ..
        })
    ));
}

#[test]
fn localized_entry_limit_is_exact() {
    let exact = (0..64)
        .map(|index| localized(&format!("en-x{index:02}"), visible("Name")))
        .collect();
    assert!(
        CharacterDisplayNameRecordInput::try_new(
            CharacterId::try_new("character.exact").unwrap(),
            CharacterPresentationRole::Character,
            None,
            None,
            exact,
            None,
        )
        .is_ok()
    );

    let one_over = (0..65)
        .map(|index| localized(&format!("en-x{index:02}"), visible("Name")))
        .collect();
    assert!(matches!(
        CharacterDisplayNameRecordInput::try_new(
            CharacterId::try_new("character.too_many").unwrap(),
            CharacterPresentationRole::Character,
            None,
            None,
            one_over,
            None,
        ),
        Err(CharacterPresentationCatalogError::Limit {
            observed: 65,
            maximum: 64,
            ..
        })
    ));
}

#[test]
fn semantic_digest_is_canonical_across_record_order() {
    let first = catalog(vec![
        record(
            "character.zed",
            None,
            Some(visible("Zed")),
            Vec::new(),
            None,
        ),
        record(
            "character.alice",
            None,
            Some(visible("Alice")),
            Vec::new(),
            None,
        ),
    ]);
    let reordered = catalog(vec![
        record(
            "character.alice",
            None,
            Some(visible("Alice")),
            Vec::new(),
            None,
        ),
        record(
            "character.zed",
            None,
            Some(visible("Zed")),
            Vec::new(),
            None,
        ),
    ]);
    assert_eq!(first.semantic_digest(), reordered.semantic_digest());
}

#[test]
fn digest_text_and_serde_are_strict_lowercase_hex() {
    let data = catalog(vec![record(
        "character.alice",
        None,
        Some(visible("Alice")),
        Vec::new(),
        None,
    )]);
    let digest = data.semantic_digest();
    let text = digest.to_lower_hex();
    assert_eq!(text.len(), 64);
    assert_eq!(
        super::CharacterPresentationSemanticDigest::parse_lower_hex(&text).unwrap(),
        digest
    );
    assert_eq!(
        super::CharacterPresentationSemanticDigest::parse_lower_hex(&text.to_uppercase()),
        Err(DigestParseError::InvalidText)
    );
    let encoded = serde_json::to_string(&digest).unwrap();
    assert_eq!(encoded, format!("\"{text}\""));
    assert_eq!(
        serde_json::from_str::<super::CharacterPresentationSemanticDigest>(&encoded).unwrap(),
        digest
    );
}

#[test]
fn publication_candidates_do_not_mutate_the_prior_generation() {
    let first = catalog(vec![record(
        "character.alice",
        None,
        Some(visible("Alice")),
        Vec::new(),
        None,
    )]);
    let accepted = AcceptedCharacterPresentationCatalog::publish_initial(first).unwrap();
    assert_eq!(
        accepted.revision(),
        CharacterPresentationCatalogRevision::INITIAL
    );

    let replacement = catalog(vec![record(
        "character.alice",
        None,
        Some(visible("アリス")),
        Vec::new(),
        None,
    )]);
    let candidate = accepted.candidate_replacement(replacement).unwrap();
    assert_eq!(accepted.revision().get(), 1);
    assert_eq!(candidate.revision().get(), 2);
    assert_ne!(
        accepted.data().semantic_digest(),
        candidate.data().semantic_digest()
    );
}
