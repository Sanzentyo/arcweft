use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    CharacterManifestRootField, CharacterManifestTokenPath, JsonObjectPathSegment,
    SourceBackedCharacterManifest,
};
use crate::id::{CharacterId, CharacterLookId, CharacterPartId};
use crate::manifest::CharacterManifestError;
use crate::manifest::diagnostic::{
    CharacterIdentifierDomain, CharacterRegistrationDecodeError, CharacterRuntimeDecodeError,
};
use crate::manifest::limits::{CharacterManifestLimitKind, CharacterManifestLimits};
use arcweft_source::MAX_REGISTRATION_SOURCE_BYTES;
use serde_json::{Value, json};

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-project://test/character.awchar.json")
            .expect("document id"),
        SourceName::path("character.awchar.json"),
        source,
    )
    .expect("document")
}

#[test]
fn duplicate_root_key_retains_spans() {
    let source = r#"{"format":"arcweft.character","f\u006frmat":"other"}"#;
    let error = SourceBackedCharacterManifest::decode_registration_json(&document(source))
        .expect_err("duplicate");
    let CharacterRegistrationDecodeError::DuplicateKey {
        first, duplicate, ..
    } = error
    else {
        panic!("expected duplicate key");
    };
    assert_eq!(&source[first.range().as_range()], "\"format\"");
    assert_eq!(&source[duplicate.range().as_range()], "\"f\\u006frmat\"");
}

#[test]
fn duplicate_nested_key_retains_spans() {
    let source = r#"{"parts":[{"variants":[{"asset":"first","asset":"second"}]}]}"#;
    let error = SourceBackedCharacterManifest::decode_registration_json(&document(source))
        .expect_err("duplicate nested key");
    let CharacterRegistrationDecodeError::DuplicateKey {
        object,
        key,
        first,
        duplicate,
    } = error
    else {
        panic!("expected duplicate key");
    };
    assert_eq!(
        object.segments(),
        &[
            JsonObjectPathSegment::Key("parts".to_owned()),
            JsonObjectPathSegment::Index(0),
            JsonObjectPathSegment::Key("variants".to_owned()),
            JsonObjectPathSegment::Index(0),
        ]
    );
    assert_eq!(key, "asset");
    assert_eq!(&source[first.range().as_range()], "\"asset\"");
    assert_eq!(&source[duplicate.range().as_range()], "\"asset\"");
    assert_ne!(first.range(), duplicate.range());
}

#[test]
fn escaped_key_equality_retains_raw_range() {
    let source = r#"{"format":"arcweft.character","f\u006frmat":"other"}"#;
    let error = SourceBackedCharacterManifest::decode_registration_json(&document(source))
        .expect_err("escaped duplicate key");
    let CharacterRegistrationDecodeError::DuplicateKey {
        key,
        first,
        duplicate,
        ..
    } = error
    else {
        panic!("expected duplicate key");
    };
    assert_eq!(key, "format");
    assert_eq!(&source[first.range().as_range()], "\"format\"");
    assert_eq!(&source[duplicate.range().as_range()], "\"f\\u006frmat\"");
}

#[test]
fn escaped_value_token_range() {
    let source = include_str!("../../../tests/fixtures/zundamon.awchar/character.awchar.json")
        .replace("character.zundamon", "character.z\\u0075ndamon");
    let manifest = SourceBackedCharacterManifest::decode_registration_json(&document(&source))
        .expect("escaped character id");
    let token = manifest
        .source_map()
        .token(&CharacterManifestTokenPath::Root(
            CharacterManifestRootField::Character,
        ))
        .expect("character token");

    assert_eq!(
        manifest.manifest().character().as_str(),
        "character.zundamon"
    );
    assert_eq!(
        &source[token.value().range().as_range()],
        "\"character.z\\u0075ndamon\""
    );
}

#[test]
fn runtime_decoder_has_no_registration_provenance() {
    let source = r#"{"format":"arcweft.character","format":"other"}"#;
    let error = crate::manifest::CharacterManifest::decode_runtime_json(source)
        .expect_err("runtime duplicate");
    let CharacterRuntimeDecodeError::DuplicateKey {
        first, duplicate, ..
    } = error
    else {
        panic!("expected runtime duplicate key");
    };
    assert_eq!(&source[first.as_range()], "\"format\"");
    assert_eq!(&source[duplicate.as_range()], "\"format\"");
}

#[test]
fn registration_and_runtime_decoders_are_the_only_json_entry_points() {
    let source = include_str!("../../../tests/fixtures/zundamon.awchar/character.awchar.json");
    let registration = SourceBackedCharacterManifest::decode_registration_json(&document(source))
        .expect("registration decode");
    let runtime =
        crate::manifest::CharacterManifest::decode_runtime_json(source).expect("runtime decode");

    assert_eq!(registration.manifest(), &runtime);
    assert!(
        registration
            .source_map()
            .token(&CharacterManifestTokenPath::Root(
                CharacterManifestRootField::Character,
            ))
            .is_some()
    );
}

#[test]
fn invalid_id_precedes_registration() {
    let valid = r#"{
  "format": "arcweft.character",
  "version": 1,
  "character": "character.akane",
  "canvas": { "width": 64, "height": 128 },
  "anchor": { "x": 32, "y": 128 },
  "default_look": "normal",
  "parts": [{
    "id": "body",
    "z": 0,
    "variants": [{
      "id": "default",
      "asset": "layers/body.png",
      "rect": { "x": 0, "y": 0, "width": 64, "height": 128 },
      "opacity": 255,
      "blend": "normal",
      "clipping": false
    }]
  }],
  "looks": [{
    "id": "normal",
    "select": [{ "part": "body", "variant": "default" }]
  }]
}"#;
    for (needle, replacement, domain) in [
        (
            "\"character\": \"character.akane\"",
            "\"character\": \"\"",
            CharacterIdentifierDomain::Character,
        ),
        (
            "\"id\": \"body\"",
            "\"id\": \"\"",
            CharacterIdentifierDomain::Part,
        ),
        (
            "\"id\": \"default\"",
            "\"id\": \"\"",
            CharacterIdentifierDomain::Variant,
        ),
        (
            "\"id\": \"normal\"",
            "\"id\": \"\"",
            CharacterIdentifierDomain::Look,
        ),
    ] {
        let invalid = valid.replacen(needle, replacement, 1);
        let error = SourceBackedCharacterManifest::decode_registration_json(&document(&invalid))
            .expect_err("invalid identifier is rejected before registration");
        let CharacterRegistrationDecodeError::InvalidIdentifier {
            domain: actual,
            value,
            span,
        } = error
        else {
            panic!("expected typed identifier failure, got {error:?}");
        };
        assert_eq!(actual, domain);
        assert!(value.is_empty());
        assert_eq!(&invalid[span.range().as_range()], "\"\"");
    }
}

#[test]
fn valid_document_exposes_exact_raw_value_tokens() {
    let source = include_str!("../../../tests/fixtures/zundamon.awchar/character.awchar.json");
    let manifest = SourceBackedCharacterManifest::decode_registration_json(&document(source))
        .expect("source-backed manifest");
    let token = manifest
        .source_map()
        .token(&CharacterManifestTokenPath::Root(
            CharacterManifestRootField::Character,
        ))
        .expect("character token");
    assert_eq!(
        &source[token.value().range().as_range()],
        format!("\"{}\"", manifest.manifest().character())
    );
}

#[test]
fn unknown_selection_part_has_typed_token() {
    let source = r#"{
  "format": "arcweft.character",
  "version": 1,
  "character": "character.akane",
  "canvas": { "width": 64, "height": 128 },
  "anchor": { "x": 32, "y": 128 },
  "default_look": "normal",
  "parts": [{
    "id": "body",
    "z": 0,
    "variants": [{
      "id": "default",
      "asset": "layers/body.png",
      "rect": { "x": 0, "y": 0, "width": 64, "height": 128 },
      "opacity": 255,
      "blend": "normal",
      "clipping": false
    }]
  }],
  "looks": [{
    "id": "normal",
    "select": [{ "part": "face", "variant": "default" }]
  }]
}"#;
    let error = SourceBackedCharacterManifest::decode_registration_json(&document(source))
        .expect_err("unknown selection part");
    let CharacterRegistrationDecodeError::Validation { error, span } = error else {
        panic!("expected typed validation failure");
    };
    assert_eq!(
        error,
        CharacterManifestError::UnknownLookPart {
            character: CharacterId::try_new("character.akane").expect("character id"),
            look: CharacterLookId::try_new("normal").expect("look id"),
            part: CharacterPartId::try_new("face").expect("part id"),
        }
    );
    assert_eq!(&source[span.range().as_range()], "\"face\"");
}

fn generated_manifest(
    part_variant_counts: &[usize],
    look_count: usize,
    selections_per_look: usize,
    extra_selection: bool,
) -> String {
    let parts = part_variant_counts
        .iter()
        .enumerate()
        .map(|(part_index, &variant_count)| {
            let variants = (0..variant_count)
                .map(|variant_index| {
                    json!({
                        "id": format!("variant{variant_index}"),
                        "asset": format!("layers/part{part_index}-{variant_index}.png"),
                        "rect": { "x": 0, "y": 0, "width": 1, "height": 1 },
                        "opacity": 255,
                        "blend": "normal",
                        "clipping": false
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "id": format!("part{part_index}"),
                "z": part_index,
                "variants": variants
            })
        })
        .collect::<Vec<_>>();
    let looks = (0..look_count)
        .map(|look_index| {
            let mut selections = (0..selections_per_look)
                .map(|part_index| {
                    json!({
                        "part": format!("part{part_index}"),
                        "variant": "variant0"
                    })
                })
                .collect::<Vec<Value>>();
            if extra_selection && look_index == 0 {
                selections.push(json!({ "part": "part0", "variant": "variant0" }));
            }
            json!({ "id": format!("look{look_index}"), "select": selections })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&json!({
        "format": "arcweft.character",
        "version": 1,
        "character": "character.limit-test",
        "canvas": { "width": 1, "height": 1 },
        "anchor": { "x": 0, "y": 1 },
        "default_look": "look0",
        "parts": parts,
        "looks": looks
    }))
    .expect("generated manifest JSON")
}

fn assert_registration_limit(
    source: &str,
    expected_kind: CharacterManifestLimitKind,
    expected_observed: u64,
    expected_maximum: u64,
) {
    let error = SourceBackedCharacterManifest::decode_registration_json(&document(source))
        .expect_err("one-over manifest must be rejected");
    assert!(matches!(
        error,
        CharacterRegistrationDecodeError::Limit {
            kind,
            observed,
            maximum,
            span: Some(_),
        } if kind == expected_kind
            && observed == expected_observed
            && maximum == expected_maximum
    ));
}

#[test]
fn limit_source_bytes_exact_and_one_over() {
    let base = generated_manifest(&[1], 1, 1, false);
    let maximum = usize::try_from(MAX_REGISTRATION_SOURCE_BYTES).expect("byte limit fits usize");
    let mut exact = base.clone();
    exact.push_str(&" ".repeat(maximum - exact.len()));
    SourceBackedCharacterManifest::decode_registration_json(&document(&exact))
        .expect("exact byte limit is accepted");

    exact.push(' ');
    assert_eq!(
        SourceBackedCharacterManifest::decode_registration_json(&document(&exact)),
        Err(CharacterRegistrationDecodeError::SourceBytesLimit {
            observed: MAX_REGISTRATION_SOURCE_BYTES + 1,
            maximum: MAX_REGISTRATION_SOURCE_BYTES,
        })
    );
}

#[test]
fn limit_parts_exact_and_one_over() {
    let limits = CharacterManifestLimits::PRODUCTION;
    let exact = usize::try_from(limits.parts()).expect("parts limit fits usize");
    SourceBackedCharacterManifest::decode_registration_json(&document(&generated_manifest(
        &vec![1; exact],
        1,
        exact,
        false,
    )))
    .expect("exact parts limit is accepted");
    let one_over = exact + 1;
    assert_registration_limit(
        &generated_manifest(&vec![1; one_over], 1, one_over, false),
        CharacterManifestLimitKind::Parts,
        u64::try_from(one_over).expect("observed parts fit u64"),
        limits.parts(),
    );
}

#[test]
fn limit_variants_per_part_exact_and_one_over() {
    let limits = CharacterManifestLimits::PRODUCTION;
    let exact = usize::try_from(limits.variants_per_part()).expect("variant limit fits usize");
    SourceBackedCharacterManifest::decode_registration_json(&document(&generated_manifest(
        &[exact],
        1,
        1,
        false,
    )))
    .expect("exact variants-per-part limit is accepted");
    assert_registration_limit(
        &generated_manifest(&[exact + 1], 1, 1, false),
        CharacterManifestLimitKind::VariantsPerPart,
        limits.variants_per_part() + 1,
        limits.variants_per_part(),
    );
}

#[test]
fn limit_variants_per_manifest_exact_and_one_over() {
    let limits = CharacterManifestLimits::PRODUCTION;
    let per_part = usize::try_from(limits.variants_per_part()).expect("variant limit fits usize");
    let total = usize::try_from(limits.variants_per_manifest()).expect("total limit fits usize");
    let part_count = total / per_part;
    SourceBackedCharacterManifest::decode_registration_json(&document(&generated_manifest(
        &vec![per_part; part_count],
        1,
        part_count,
        false,
    )))
    .expect("exact variants-per-manifest limit is accepted");
    let mut one_over = vec![per_part; part_count];
    one_over.push(1);
    assert_registration_limit(
        &generated_manifest(&one_over, 1, one_over.len(), false),
        CharacterManifestLimitKind::VariantsPerManifest,
        limits.variants_per_manifest() + 1,
        limits.variants_per_manifest(),
    );
}

#[test]
fn limit_looks_exact_and_one_over() {
    let limits = CharacterManifestLimits::PRODUCTION;
    let exact = usize::try_from(limits.looks()).expect("looks limit fits usize");
    SourceBackedCharacterManifest::decode_registration_json(&document(&generated_manifest(
        &[1],
        exact,
        1,
        false,
    )))
    .expect("exact looks limit is accepted");
    assert_registration_limit(
        &generated_manifest(&[1], exact + 1, 1, false),
        CharacterManifestLimitKind::Looks,
        limits.looks() + 1,
        limits.looks(),
    );
}

#[test]
fn limit_selections_exact_and_one_over() {
    let limits = CharacterManifestLimits::PRODUCTION;
    let part_count = usize::try_from(limits.parts()).expect("parts limit fits usize");
    let look_count =
        usize::try_from(limits.selections()).expect("selection limit fits usize") / part_count;
    SourceBackedCharacterManifest::decode_registration_json(&document(&generated_manifest(
        &vec![1; part_count],
        look_count,
        part_count,
        false,
    )))
    .expect("exact selections limit is accepted");
    assert_registration_limit(
        &generated_manifest(&vec![1; part_count], look_count, part_count, true),
        CharacterManifestLimitKind::Selections,
        limits.selections() + 1,
        limits.selections(),
    );
}
