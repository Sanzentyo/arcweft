use crate::{
    CharacterDialogue, CharacterDialogueConfig, CharacterDialogueContentApplication,
    CharacterDialogueContractIdentity, CharacterDialogueCustomFieldId,
    CharacterDialogueCustomValue, CharacterDialogueHookValue, CharacterDialoguePatch,
    CharacterDialogueRichTextValue, CharacterDialogueRuntimeCustomFieldCatalog,
    CharacterDialogueRuntimeCustomFieldDescriptor, CharacterDialogueRuntimeSchema,
    CharacterDialogueStyleValue, CharacterDialogueTypedValue, CharacterDialogueValueError,
    CharacterDialogueVoice, CharacterDialogueVoiceId, DialogueContent, DialogueLocaleId,
    FallbackStylePolicy, InlineFailurePolicy, InlineFallback, LinePlan,
    PRODUCTION_CHARACTER_DIALOGUE_LIMITS, PatchField, RuntimeFieldPath, StructuredPatch,
};
use arcweft_character::{
    catalog::CharacterCatalog,
    id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId},
    manifest::{
        CharacterAssetPath, CharacterBlendMode, CharacterCanvas, CharacterLook, CharacterManifest,
        CharacterPart, CharacterPartSelection, CharacterPoint, CharacterRect, CharacterVariant,
    },
};
use arcweft_core::{
    entry::{RuntimeNominalTypeId, RuntimeValueDigest, TypeLayoutHash},
    plan::RuntimeLineId,
    value::{
        MAX_RUNTIME_VALUE_NESTING_DEPTH, RuntimeFieldValue, RuntimeNominalRecordError,
        RuntimeNominalRecordValue, RuntimeSeq, RuntimeValue,
    },
};
use arcweft_id::TextKey;
use arcweft_source::{SourceAnchor, SourceDocument, SourceDocumentId, SourceName, SourceRange};
use arcweft_view::{RustViewId, ViewDescriptor, ViewId, ViewRegistry, ViewSchemaId};
use std::collections::{BTreeMap, BTreeSet};

fn sample_manifest() -> CharacterManifest {
    let body = CharacterPart::new(
        CharacterPartId::try_new("body").expect("part"),
        0,
        vec![CharacterVariant::new(
            CharacterVariantId::try_new("default").expect("variant"),
            CharacterAssetPath::try_new("layers/body.png").expect("asset path"),
            CharacterRect::new(0, 0, 64, 128),
            u8::MAX,
            CharacterBlendMode::Normal,
            false,
        )],
    );
    let look = CharacterLook::new(
        CharacterLookId::try_new("normal").expect("look"),
        vec![CharacterPartSelection::new(
            CharacterPartId::try_new("body").expect("part"),
            CharacterVariantId::try_new("default").expect("variant"),
        )],
    );
    CharacterManifest::new(
        CharacterId::try_new("character.akane").expect("character"),
        CharacterCanvas::new(64, 128),
        CharacterPoint::new(32, 128),
        CharacterLookId::try_new("normal").expect("look"),
        vec![body],
        vec![look],
        None,
    )
    .expect("manifest")
}

fn nominal_typed(
    id: &str,
    layout: TypeLayoutHash,
    fields: Vec<RuntimeValue>,
) -> CharacterDialogueTypedValue {
    let type_id = RuntimeNominalTypeId::try_new(id).expect("nominal type");
    CharacterDialogueTypedValue::try_new(
        Some(type_id.clone()),
        layout,
        RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(type_id, layout, fields)),
    )
    .expect("typed value")
}

fn style(layout_byte: u8) -> CharacterDialogueStyleValue {
    CharacterDialogueStyleValue::try_new(nominal_typed(
        "std.rich_text_style",
        TypeLayoutHash::from_bytes([layout_byte; 32]),
        Vec::new(),
    ))
    .expect("style")
}

fn rich_text(layout_byte: u8) -> CharacterDialogueRichTextValue {
    CharacterDialogueRichTextValue::try_new(nominal_typed(
        "std.rich_text_style",
        TypeLayoutHash::from_bytes([layout_byte; 32]),
        Vec::new(),
    ))
    .expect("rich text")
}

fn fixture() -> (
    CharacterDialogue,
    CharacterCatalog,
    ViewRegistry,
    CharacterDialogueRuntimeCustomFieldCatalog,
) {
    fixture_with_style(style(5))
}

fn fixture_with_style(
    style: CharacterDialogueStyleValue,
) -> (
    CharacterDialogue,
    CharacterCatalog,
    ViewRegistry,
    CharacterDialogueRuntimeCustomFieldCatalog,
) {
    let manifest = sample_manifest();
    let character_manifest =
        RuntimeValueDigest::from_bytes(*manifest.semantic_fingerprint_v1().as_bytes());
    let catalog = CharacterCatalog::try_from_manifests([manifest]).expect("catalog");
    let view = ViewId::try_new_engine_owned("std.view.dialogue").expect("View");
    let mut views = ViewRegistry::default();
    views
        .register(ViewDescriptor::public_rust(
            view.clone(),
            ViewSchemaId(1),
            RustViewId(1),
        ))
        .expect("register View");
    let custom_digest = RuntimeValueDigest::from_bytes([3; 32]);
    let custom = CharacterDialogueRuntimeCustomFieldCatalog::try_new(custom_digest, [])
        .expect("custom catalog");
    let contract = CharacterDialogueContractIdentity::new(
        character_manifest,
        RuntimeValueDigest::from_bytes([2; 32]),
        custom_digest,
        RuntimeValueDigest::from_bytes([4; 32]),
    );
    let config = CharacterDialogueConfig::try_new(view, style, rich_text(6)).expect("config");
    let dialogue = CharacterDialogue::try_new(
        CharacterId::try_new("character.akane").expect("character"),
        TypeLayoutHash::from_bytes([9; 32]),
        contract,
        config,
    )
    .expect("dialogue");
    (dialogue, catalog, views, custom)
}

fn option_some(value: RuntimeValue) -> RuntimeValue {
    RuntimeValue::Variant {
        path: None,
        name: "Some".to_owned(),
        payload: Some(Box::new(value)),
    }
}

#[test]
fn character_ownership_and_patch_clear_are_immutable() {
    let (base, _, _, _) = fixture();
    let configured = base
        .patched(
            &CharacterDialoguePatch::default()
                .with_voice(PatchField::Set(CharacterDialogueVoice::Auto))
                .with_source_locale(PatchField::Set(
                    DialogueLocaleId::try_new("ja-jp").expect("locale"),
                ))
                .with_view(PatchField::Clear),
        )
        .expect("patch");

    assert_eq!(base.character().as_str(), "character.akane");
    assert_eq!(base.config().voice(), None);
    assert!(matches!(
        configured.config().voice(),
        Some(CharacterDialogueVoice::Auto)
    ));
    assert_eq!(
        configured
            .config()
            .source_locale()
            .expect("locale")
            .as_str(),
        "ja-JP"
    );
    assert_eq!(configured.config().view().as_str(), "std.view.dialogue");

    let cleared = configured
        .patched(
            &CharacterDialoguePatch::default()
                .with_voice(PatchField::Clear)
                .with_source_locale(PatchField::Clear),
        )
        .expect("clear");
    assert_eq!(cleared.config().voice(), None);
    assert_eq!(cleared.config().source_locale(), None);
    assert!(matches!(
        configured.config().voice(),
        Some(CharacterDialogueVoice::Auto)
    ));
}

#[test]
fn dialogue_locale_is_a_domain_newtype_over_the_shared_canonical_owner() {
    let locale = DialogueLocaleId::try_new("zh-hant-tw").unwrap();
    assert_eq!(locale.as_str(), "zh-Hant-TW");
    assert_eq!(locale.locale_id().as_str(), "zh-Hant-TW");
    assert_eq!(
        DialogueLocaleId::try_new("de-de").unwrap().as_str(),
        "de-DE"
    );
    assert!(DialogueLocaleId::try_new("e").is_err());
    assert!(DialogueLocaleId::try_new("en-abcdefghi").is_err());
    assert!(DialogueLocaleId::try_new("é-JP").is_err());
}

#[test]
fn failed_patch_leaves_base_and_successful_candidate_unchanged() {
    let (base, _, _, _) = fixture();
    let hook = CharacterDialogueHookValue::try_new(nominal_typed(
        "std.dialogue_hook",
        TypeLayoutHash::from_bytes([7; 32]),
        Vec::new(),
    ))
    .expect("hook");
    let too_many = vec![hook; 65];
    let before = base.digest().expect("digest");
    let error = base
        .patched(&CharacterDialoguePatch::default().with_hooks(PatchField::Set(too_many)))
        .expect_err("hook limit");

    assert!(matches!(
        error,
        CharacterDialogueValueError::Limit { limit: "hooks", .. }
    ));
    assert_eq!(base.digest().expect("digest"), before);
    assert_eq!(base.config().hooks(), &[]);
}

#[test]
fn structured_clear_preserves_nominal_shape_but_not_anonymous_record_fields() {
    let layout = TypeLayoutHash::from_bytes([11; 32]);
    let nominal_style = CharacterDialogueStyleValue::try_new(nominal_typed(
        "std.rich_text_style",
        layout,
        vec![
            option_some(RuntimeValue::String("red".to_owned())),
            option_some(RuntimeValue::Bool(true)),
        ],
    ))
    .expect("nominal style");
    let (base, _, _, _) = fixture_with_style(nominal_style);
    let clear_first = StructuredPatch::try_new(
        false,
        BTreeMap::from([(
            RuntimeFieldPath::try_new(vec![0]).expect("path"),
            PatchField::Clear,
        )]),
    )
    .expect("structured patch");
    let patched = base
        .patched(&CharacterDialoguePatch::default().with_style(clear_first))
        .expect("nominal leaf clear");
    let RuntimeValue::NominalRecord(record) = patched.config().style().typed().value() else {
        panic!("style remains nominal");
    };
    assert_eq!(record.fields().len(), 2);
    assert!(matches!(
        &record.fields()[0],
        RuntimeValue::Variant {
            name,
            payload: None,
            ..
        } if name == "None"
    ));
    assert!(matches!(
        &record.fields()[1],
        RuntimeValue::Variant {
            name,
            payload: Some(_),
            ..
        } if name == "Some"
    ));

    let cleared = patched
        .patched(&CharacterDialoguePatch::default().with_style(StructuredPatch::clear_all()))
        .expect("nominal clear all");
    let RuntimeValue::NominalRecord(record) = cleared.config().style().typed().value() else {
        panic!("style remains nominal");
    };
    assert_eq!(record.fields().len(), 2);
    assert!(record.fields().iter().all(|field| matches!(
        field,
        RuntimeValue::Variant {
            name,
            payload: None,
            ..
        } if name == "None"
    )));

    let structural_style = CharacterDialogueStyleValue::try_new(
        CharacterDialogueTypedValue::try_new(
            None,
            layout,
            RuntimeValue::Record(vec![
                RuntimeFieldValue {
                    name: "alpha".to_owned(),
                    value: RuntimeValue::Bool(true),
                },
                RuntimeFieldValue {
                    name: "beta".to_owned(),
                    value: RuntimeValue::Bool(false),
                },
            ]),
        )
        .expect("structural value"),
    )
    .expect("structural style");
    let (base, _, _, _) = fixture_with_style(structural_style);
    let patched = base
        .patched(
            &CharacterDialoguePatch::default().with_style(
                StructuredPatch::try_new(
                    false,
                    BTreeMap::from([(
                        RuntimeFieldPath::try_new(vec![0]).expect("path"),
                        PatchField::Clear,
                    )]),
                )
                .expect("structured patch"),
            ),
        )
        .expect("anonymous field clear");
    let RuntimeValue::Record(fields) = patched.config().style().typed().value() else {
        panic!("style remains anonymous");
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "beta");
}

#[test]
fn structured_clear_rejects_non_option_some_leaf_atomically() {
    let layout = TypeLayoutHash::from_bytes([12; 32]);
    let non_option_some = RuntimeValue::Variant {
        path: Some("custom.Choice".to_owned()),
        name: "Some".to_owned(),
        payload: Some(Box::new(RuntimeValue::Bool(true))),
    };
    let style = CharacterDialogueStyleValue::try_new(nominal_typed(
        "std.rich_text_style",
        layout,
        vec![non_option_some.clone()],
    ))
    .expect("nominal style");
    let (base, _, _, _) = fixture_with_style(style);
    let before = base.digest().expect("digest");
    let patch = StructuredPatch::try_new(
        false,
        BTreeMap::from([(
            RuntimeFieldPath::try_new(vec![0]).expect("path"),
            PatchField::Clear,
        )]),
    )
    .expect("structured patch");

    assert!(matches!(
        base.patched(&CharacterDialoguePatch::default().with_style(patch)),
        Err(CharacterDialogueValueError::Field {
            field: "structured_patch",
            ..
        })
    ));
    assert_eq!(base.digest().expect("digest"), before);
    let RuntimeValue::NominalRecord(record) = base.config().style().typed().value() else {
        panic!("style remains nominal");
    };
    assert_eq!(record.fields(), &[non_option_some]);
}

#[test]
fn runtime_schema_round_trips_exact_nominal_record() {
    let (dialogue, characters, views, custom) = fixture();
    let schema =
        CharacterDialogueRuntimeSchema::new(&characters, &views, &custom, dialogue.layout());
    let encoded = schema.encode(&dialogue).expect("encode");
    let decoded = schema.decode(encoded.record()).expect("decode");

    assert_eq!(decoded.dialogue(), &dialogue);
    assert_eq!(
        encoded.record().type_id().as_str(),
        "std.character_dialogue"
    );
    assert_eq!(encoded.record().fields().len(), 18);
}

#[test]
fn runtime_schema_decode_returns_the_normalized_canonical_record() {
    let (dialogue, characters, views, custom) = fixture();
    let schema =
        CharacterDialogueRuntimeSchema::new(&characters, &views, &custom, dialogue.layout());
    let encoded = schema.encode(&dialogue).expect("encode");
    let mut fields = encoded.record().clone().into_fields();
    let RuntimeValue::NominalRecord(style) = &fields[14] else {
        panic!("style field is nominal");
    };
    fields[14] = RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
        style.type_id().clone(),
        style.layout(),
        vec![RuntimeValue::F64(-0.0)],
    ));
    let input = RuntimeNominalRecordValue::new(
        encoded.record().type_id().clone(),
        encoded.record().layout(),
        fields,
    );

    let decoded = schema.decode(&input).expect("decode and normalize");
    let RuntimeValue::NominalRecord(record_style) = &decoded.record().fields()[14] else {
        panic!("decoded record style remains nominal");
    };
    let RuntimeValue::F64(record_zero) = &record_style.fields()[0] else {
        panic!("decoded record contains the normalized style leaf");
    };
    let RuntimeValue::NominalRecord(dialogue_style) =
        decoded.dialogue().config().style().typed().value()
    else {
        panic!("decoded dialogue style remains nominal");
    };
    let RuntimeValue::F64(dialogue_zero) = &dialogue_style.fields()[0] else {
        panic!("decoded dialogue contains the normalized style leaf");
    };

    assert_eq!(record_zero.to_bits(), 0.0_f64.to_bits());
    assert_eq!(dialogue_zero.to_bits(), 0.0_f64.to_bits());
    assert_eq!(
        decoded.record(),
        schema
            .encode(decoded.dialogue())
            .expect("re-encode decoded dialogue")
            .record()
    );
}

#[test]
fn runtime_schema_rejects_wrong_type_layout_and_field_count() {
    let (dialogue, characters, views, custom) = fixture();
    let schema =
        CharacterDialogueRuntimeSchema::new(&characters, &views, &custom, dialogue.layout());
    let encoded = schema.encode(&dialogue).expect("encode");
    let fields = encoded.record().fields().to_vec();

    let wrong_type = RuntimeNominalRecordValue::new(
        RuntimeNominalTypeId::try_new("std.other_dialogue").expect("type"),
        encoded.record().layout(),
        fields.clone(),
    );
    assert!(matches!(
        schema.decode(&wrong_type),
        Err(CharacterDialogueValueError::Nominal(
            RuntimeNominalRecordError::Type { .. }
        ))
    ));

    let wrong_layout = RuntimeNominalRecordValue::new(
        encoded.record().type_id().clone(),
        TypeLayoutHash::from_bytes([1; 32]),
        fields.clone(),
    );
    assert!(matches!(
        schema.decode(&wrong_layout),
        Err(CharacterDialogueValueError::Nominal(
            RuntimeNominalRecordError::Layout { .. }
        ))
    ));

    let mut short_fields = fields;
    short_fields.pop();
    let wrong_count = RuntimeNominalRecordValue::new(
        encoded.record().type_id().clone(),
        encoded.record().layout(),
        short_fields,
    );
    assert!(matches!(
        schema.decode(&wrong_count),
        Err(CharacterDialogueValueError::Nominal(
            RuntimeNominalRecordError::FieldCount { .. }
        ))
    ));
}

#[test]
fn runtime_schema_rejects_reversed_custom_order() {
    let (base, characters, views, _) = fixture();
    let view = base.config().view().clone();
    let layout = TypeLayoutHash::from_bytes([8; 32]);
    let first =
        CharacterDialogueCustomFieldId::try_new("character_dialogue_field.alpha").expect("field");
    let second =
        CharacterDialogueCustomFieldId::try_new("character_dialogue_field.beta").expect("field");
    let descriptors = [first.clone(), second.clone()].map(|id| {
        CharacterDialogueRuntimeCustomFieldDescriptor::new(
            id,
            None,
            layout,
            true,
            BTreeSet::from([view.clone()]),
        )
    });
    let custom = CharacterDialogueRuntimeCustomFieldCatalog::try_new(
        base.contract().custom_schema(),
        descriptors,
    )
    .expect("catalog");
    let value = |text: &str| {
        CharacterDialogueCustomValue::try_new(
            CharacterDialogueTypedValue::try_new(
                None,
                layout,
                RuntimeValue::String(text.to_owned()),
            )
            .expect("typed"),
        )
        .expect("custom")
    };
    let dialogue = base
        .patched(
            &CharacterDialoguePatch::default()
                .with_custom(first, PatchField::Set(value("a")))
                .with_custom(second, PatchField::Set(value("b"))),
        )
        .expect("patch");
    let schema =
        CharacterDialogueRuntimeSchema::new(&characters, &views, &custom, dialogue.layout());
    let encoded = schema.encode(&dialogue).expect("encode");
    let mut fields = encoded.record().clone().into_fields();
    let RuntimeValue::Seq(entries) = &fields[17] else {
        panic!("custom field is a sequence");
    };
    let mut entries = entries.clone().into_values();
    entries.reverse();
    fields[17] = RuntimeValue::Seq(RuntimeSeq::values(entries));
    let reversed = RuntimeNominalRecordValue::new(
        encoded.record().type_id().clone(),
        encoded.record().layout(),
        fields,
    );

    assert_eq!(
        schema.decode(&reversed).expect_err("order must fail"),
        CharacterDialogueValueError::NonCanonicalCustomOrder
    );
}

fn nested_nominal(depth: usize) -> CharacterDialogueTypedValue {
    let type_id = RuntimeNominalTypeId::try_new("test.NestedDialogueValue").expect("type");
    let layout = TypeLayoutHash::from_bytes([21; 32]);
    let value = (0..depth).fold(RuntimeValue::Unit, |value, _| {
        RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
            type_id.clone(),
            layout,
            vec![value],
        ))
    });
    CharacterDialogueTypedValue::try_new(Some(type_id), layout, value).expect("nested typed value")
}

fn nominal_with_encoded_size(type_name: &str, target: usize) -> CharacterDialogueTypedValue {
    let type_id = RuntimeNominalTypeId::try_new(type_name).expect("type");
    let layout = TypeLayoutHash::from_bytes([22; 32]);
    let empty = RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
        type_id.clone(),
        layout,
        Vec::new(),
    ));
    let header_bytes = empty
        .try_canonical_bytes(target)
        .expect("empty encoded value")
        .len();
    assert!(target >= header_bytes + 5);
    let encoded_payload = target - header_bytes;
    let max_string_bytes = PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_string_bytes as usize;
    let field_count = encoded_payload.div_ceil(max_string_bytes + 5);
    assert!(encoded_payload >= field_count * 5);
    let mut remaining_text = encoded_payload - field_count * 5;
    let fields = (0..field_count)
        .map(|_| {
            let bytes = remaining_text.min(max_string_bytes);
            remaining_text -= bytes;
            RuntimeValue::String("x".repeat(bytes))
        })
        .collect::<Vec<_>>();
    assert_eq!(remaining_text, 0);
    let value = RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
        type_id.clone(),
        layout,
        fields,
    ));
    assert_eq!(
        value
            .try_canonical_bytes(target)
            .expect("target-sized value")
            .len(),
        target
    );
    CharacterDialogueTypedValue::try_new(Some(type_id), layout, value).expect("typed value")
}

fn source_anchor() -> SourceAnchor {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("character-dialogue-limit-test").expect("source id"),
        SourceName::Memory,
        "",
    )
    .expect("source");
    SourceAnchor::from_span(
        document
            .span(SourceRange::new(0, 0))
            .expect("empty source span"),
    )
}

#[test]
fn production_limits_have_the_exact_typed_contract_values() {
    let limits = PRODUCTION_CHARACTER_DIALOGUE_LIMITS;
    assert_eq!(limits.max_patch_fields, 64_u16);
    assert_eq!(limits.max_patch_work, 1_024_u32);
    assert_eq!(limits.max_custom_fields, 32_u16);
    assert_eq!(limits.max_custom_field_id_bytes, 128_u16);
    assert_eq!(limits.max_hooks, 64_u16);
    assert_eq!(limits.max_config_string_bytes, 16_384_u32);
    assert_eq!(limits.max_locale_bytes, 64_u16);
    assert_eq!(limits.max_structured_depth, 8_u8);
    assert_eq!(limits.max_structured_leaves, 256_u16);
    assert_eq!(limits.max_fx_applications, 128_u16);
    assert_eq!(limits.max_field_value_bytes, 65_536_u32);
    assert_eq!(limits.max_config_encoded_bytes, 524_288_u32);
    assert_eq!(limits.max_values_per_sequence, 4_096_u32);
    assert_eq!(limits.max_captured_values_per_function, 256_u16);
    assert_eq!(limits.max_defaults_entries, 4_096_u32);
    assert_eq!(limits.max_line_id_bytes, 256_u16);
}

#[test]
fn generic_typed_values_use_runtime_depth_64_not_structured_depth_8() {
    nested_nominal(MAX_RUNTIME_VALUE_NESTING_DEPTH);
    let type_id = RuntimeNominalTypeId::try_new("test.NestedDialogueValue").expect("type");
    let layout = TypeLayoutHash::from_bytes([21; 32]);
    let value = (0..=MAX_RUNTIME_VALUE_NESTING_DEPTH).fold(RuntimeValue::Unit, |value, _| {
        RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
            type_id.clone(),
            layout,
            vec![value],
        ))
    });
    assert!(matches!(
        CharacterDialogueTypedValue::try_new(Some(type_id), layout, value),
        Err(CharacterDialogueValueError::Limit {
            limit: "runtime_value_nesting_depth",
            maximum: MAX_RUNTIME_VALUE_NESTING_DEPTH,
        })
    ));
}

#[test]
fn structured_paths_and_values_enforce_depth_8_and_total_leaves_256() {
    RuntimeFieldPath::try_new(vec![0; 8]).expect("path depth 8");
    assert!(matches!(
        RuntimeFieldPath::try_new(vec![0; 9]),
        Err(CharacterDialogueValueError::Limit {
            limit: "structured_depth",
            maximum: 8,
        })
    ));

    CharacterDialogueStyleValue::try_new(nested_nominal(8)).expect("style depth 8");
    assert!(matches!(
        CharacterDialogueStyleValue::try_new(nested_nominal(9)),
        Err(CharacterDialogueValueError::Limit {
            limit: "structured_depth",
            maximum: 8,
        })
    ));

    let layout = TypeLayoutHash::from_bytes([23; 32]);
    let exact = nominal_typed(
        "std.rich_text_style",
        layout,
        vec![RuntimeValue::Bool(true); 256],
    );
    CharacterDialogueStyleValue::try_new(exact).expect("256 structured leaves");
    let over = nominal_typed(
        "std.rich_text_style",
        layout,
        vec![RuntimeValue::Bool(true); 257],
    );
    assert!(matches!(
        CharacterDialogueStyleValue::try_new(over),
        Err(CharacterDialogueValueError::Limit {
            limit: "structured_leaves",
            maximum: 256,
        })
    ));
}

#[test]
fn typed_values_normalize_nested_negative_zero_and_reject_record_reordering() {
    let layout = TypeLayoutHash::from_bytes([24; 32]);
    let value = CharacterDialogueTypedValue::try_new(
        None,
        layout,
        RuntimeValue::Record(vec![RuntimeFieldValue {
            name: "value".to_owned(),
            value: RuntimeValue::Tuple(vec![RuntimeValue::F32(-0.0), RuntimeValue::F64(-0.0)]),
        }]),
    )
    .expect("typed value");
    let RuntimeValue::Record(fields) = value.value() else {
        panic!("record");
    };
    let RuntimeValue::Tuple(values) = &fields[0].value else {
        panic!("tuple");
    };
    assert!(matches!(&values[0], RuntimeValue::F32(value) if value.to_bits() == 0));
    assert!(matches!(&values[1], RuntimeValue::F64(value) if value.to_bits() == 0));

    assert!(matches!(
        CharacterDialogueTypedValue::try_new(
            None,
            layout,
            RuntimeValue::Record(vec![
                RuntimeFieldValue {
                    name: "beta".to_owned(),
                    value: RuntimeValue::Bool(true),
                },
                RuntimeFieldValue {
                    name: "alpha".to_owned(),
                    value: RuntimeValue::Bool(false),
                },
            ]),
        ),
        Err(CharacterDialogueValueError::Field {
            field: "typed_value",
            ..
        })
    ));
}

#[test]
fn role_deserialization_revalidates_structured_limits() {
    let generic = nested_nominal(9);
    let serialized = serde_json::to_value(generic).expect("serialize generic typed value");
    assert!(
        serde_json::from_value::<CharacterDialogueStyleValue>(serialized).is_err(),
        "transparent role deserialization must call the role constructor"
    );
}

#[test]
fn role_encoded_sizes_accept_exact_limits_and_reject_one_over() {
    let field_max = PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_field_value_bytes as usize;
    crate::CharacterDialogueStageValue::try_new(nominal_with_encoded_size(
        "std.dialogue_stage",
        field_max,
    ))
    .expect("field value exact limit");
    assert!(matches!(
        crate::CharacterDialogueStageValue::try_new(nominal_with_encoded_size(
            "std.dialogue_stage",
            field_max + 1,
        )),
        Err(CharacterDialogueValueError::Limit {
            limit: "field_value_bytes",
            maximum,
        }) if maximum == field_max
    ));

    let aggregate_max = field_max * 4;
    CharacterDialogueStyleValue::try_new(nominal_with_encoded_size(
        "std.rich_text_style",
        aggregate_max,
    ))
    .expect("style exact aggregate");
    assert!(matches!(
        CharacterDialogueStyleValue::try_new(nominal_with_encoded_size(
            "std.rich_text_style",
            aggregate_max + 1,
        )),
        Err(CharacterDialogueValueError::Limit {
            limit: "typed_aggregate_bytes",
            maximum,
        }) if maximum == aggregate_max
    ));
}

#[test]
fn hooks_enforce_exact_256_kib_aggregate() {
    let (base, _, _, _) = fixture();
    let aggregate_max = (PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_field_value_bytes as usize) * 4;
    let hook_count = usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_hooks);
    let exact_hook_bytes = aggregate_max / hook_count;
    let hook = CharacterDialogueHookValue::try_new(nominal_with_encoded_size(
        "std.dialogue_hook",
        exact_hook_bytes,
    ))
    .expect("hook");
    base.patched(
        &CharacterDialoguePatch::default()
            .with_hooks(PatchField::Set(vec![hook.clone(); hook_count])),
    )
    .expect("exact aggregate");

    let larger = CharacterDialogueHookValue::try_new(nominal_with_encoded_size(
        "std.dialogue_hook",
        exact_hook_bytes + 1,
    ))
    .expect("larger hook");
    let mut over = vec![hook; hook_count];
    over[0] = larger;
    assert!(matches!(
        base.patched(
            &CharacterDialoguePatch::default().with_hooks(PatchField::Set(over))
        ),
        Err(CharacterDialogueValueError::Limit {
            limit: "hook_aggregate_bytes",
            maximum,
        }) if maximum == aggregate_max
    ));
}

#[test]
fn dialogue_ids_enforce_their_field_table_byte_limits() {
    let exact_voice = format!("voice.{}", "v".repeat(250));
    CharacterDialogueVoiceId::try_new(exact_voice).expect("voice id 256");
    assert!(matches!(
        CharacterDialogueVoiceId::try_new(format!("voice.{}", "v".repeat(251))),
        Err(CharacterDialogueValueError::Limit {
            limit: "voice_id_bytes",
            maximum: 256,
        })
    ));

    let exact_view = ViewId::try_new(format!("view.{}", "v".repeat(251))).expect("View id");
    CharacterDialogueConfig::try_new(exact_view, style(5), rich_text(6)).expect("View id 256");
    let over_view = ViewId::try_new(format!("view.{}", "v".repeat(252))).expect("View id");
    assert!(matches!(
        CharacterDialogueConfig::try_new(over_view, style(5), rich_text(6)),
        Err(CharacterDialogueValueError::Limit {
            limit: "view_id_bytes",
            maximum: 256,
        })
    ));

    let (base, _, _, _) = fixture();
    let exact_look = CharacterLookId::try_new("l".repeat(128)).expect("look");
    base.patched(&CharacterDialoguePatch::default().with_look(PatchField::Set(exact_look)))
        .expect("look id 128");
    let over_look = CharacterLookId::try_new("l".repeat(129)).expect("look");
    assert!(matches!(
        base.patched(&CharacterDialoguePatch::default().with_look(PatchField::Set(over_look))),
        Err(CharacterDialogueValueError::Limit {
            limit: "look_id_bytes",
            maximum: 128,
        })
    ));
}

#[test]
fn content_application_checks_complete_line_and_text_key_ids() {
    let (dialogue, _, _, _) = fixture();
    let exact_line = RuntimeLineId::canonical(&"l".repeat(252)).expect("line");
    let exact_text = TextKey::try_new(format!("text.{}", "t".repeat(251))).expect("text key");
    CharacterDialogueContentApplication::try_new(
        dialogue.clone(),
        exact_line,
        exact_text,
        DialogueContent::text("hello"),
        LinePlan::default(),
        source_anchor(),
    )
    .expect("256-byte line and text IDs");

    let over_line = RuntimeLineId::canonical(&"l".repeat(253)).expect("line");
    assert!(matches!(
        CharacterDialogueContentApplication::try_new(
            dialogue.clone(),
            over_line,
            TextKey::try_new("text.ok").expect("text key"),
            DialogueContent::text("hello"),
            LinePlan::default(),
            source_anchor(),
        ),
        Err(CharacterDialogueValueError::Limit {
            limit: "line_id_bytes",
            maximum: 256,
        })
    ));

    let over_text = TextKey::try_new(format!("text.{}", "t".repeat(252))).expect("text key");
    assert!(matches!(
        CharacterDialogueContentApplication::try_new(
            dialogue,
            RuntimeLineId::canonical("ok").expect("line"),
            over_text,
            DialogueContent::text("hello"),
            LinePlan::default(),
            source_anchor(),
        ),
        Err(CharacterDialogueValueError::Limit {
            limit: "text_key_bytes",
            maximum: 256,
        })
    ));
}

#[test]
fn inline_failure_policies_are_strict_manifest_values_at_the_dialogue_owner() {
    let policies = [
        InlineFailurePolicy::FailLine,
        InlineFailurePolicy::Discard,
        InlineFailurePolicy::Fallback {
            fallback: InlineFallback::Text {
                text: "[unavailable]".to_owned(),
                style: FallbackStylePolicy::Plain,
            },
        },
        InlineFailurePolicy::Fallback {
            fallback: InlineFallback::ExprSource {
                style: FallbackStylePolicy::InheritSurrounding,
            },
        },
        InlineFailurePolicy::Fallback {
            fallback: InlineFallback::CallSource {
                style: FallbackStylePolicy::Apply {
                    styles: vec![style(31)],
                },
            },
        },
        InlineFailurePolicy::Fallback {
            fallback: InlineFallback::ValuePlain,
        },
    ];

    for policy in policies {
        let encoded = serde_json::to_value(&policy).expect("serialize policy");
        let decoded =
            serde_json::from_value::<InlineFailurePolicy>(encoded).expect("deserialize policy");
        assert_eq!(decoded, policy);
    }

    assert_eq!(
        InlineFailurePolicy::default(),
        InlineFailurePolicy::FailLine
    );

    for malformed in [
        r#"{"kind":"fail_line","unexpected":true}"#,
        r#"{"kind":"fallback","fallback":{"kind":"value_plain","unexpected":true}}"#,
        r#"{"kind":"fallback","fallback":{"kind":"text","text":"x","style":{"kind":"plain","unexpected":true}}}"#,
        r#"{"kind":"unknown"}"#,
    ] {
        assert!(
            serde_json::from_str::<InlineFailurePolicy>(malformed).is_err(),
            "policy must reject {malformed}"
        );
    }
}
