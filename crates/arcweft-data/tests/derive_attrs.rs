#![cfg(feature = "derive")]

use std::collections::BTreeMap;

use arcweft_data::{
    ArcweftDecode, ArcweftEncode, ArcweftReflect, Bytes, BytesFormat, DataErrorKind, Decode,
    Encode, EnumRepr, EnumTagStyle, Number, Reflect, TypeShape, Value,
};

#[derive(Debug, Default, PartialEq, ArcweftEncode, ArcweftDecode, ArcweftReflect)]
#[arcweft(rename_all = "kebab-case", deny_unknown_fields)]
struct PlayerSave {
    schema_version: u32,
    #[arcweft(rename = "player")]
    player_id: String,
    #[arcweft(default)]
    flags: Vec<String>,
    #[arcweft(skip)]
    cache: String,
    #[arcweft(bytes = "hex")]
    screenshot_hash: Bytes,
}

#[derive(Debug, PartialEq, ArcweftEncode, ArcweftDecode, ArcweftReflect)]
#[arcweft(repr = "u8", rename_all = "snake_case")]
enum SaveKind {
    Full = 1,
    Quick = 2,
}

#[derive(Debug, PartialEq, ArcweftEncode, ArcweftDecode, ArcweftReflect)]
#[arcweft(rename_all = "snake_case", tag = "kind", content = "value")]
enum AdjacentEvent {
    Started,
    LineShown { line_id: String, speaker: String },
    Score(u32),
}

#[derive(Debug, PartialEq, ArcweftEncode, ArcweftDecode, ArcweftReflect)]
#[arcweft(rename_all = "snake_case", tag = "kind")]
enum InternalEvent {
    Started,
    LineShown { line_id: String },
}

#[derive(Debug, PartialEq, ArcweftEncode, ArcweftDecode, ArcweftReflect)]
#[arcweft(rename_all = "snake_case")]
enum ExternalEvent {
    Started,
}

#[test]
fn struct_attrs_drive_wire_names_defaults_skip_bytes_shape_and_unknown_policy() {
    let save = PlayerSave {
        schema_version: 7,
        player_id: "alice".to_owned(),
        flags: vec!["intro".to_owned()],
        cache: "local".to_owned(),
        screenshot_hash: Bytes::from([1_u8, 2, 3].as_slice()),
    };

    let encoded = save.encode().expect("encode");
    let Value::Record(record) = &encoded else {
        panic!("expected record");
    };
    assert!(record.contains_key("schema-version"));
    assert!(record.contains_key("player"));
    assert!(record.contains_key("flags"));
    assert!(record.contains_key("screenshot-hash"));
    assert!(!record.contains_key("cache"));

    let mut missing_default = record.clone();
    missing_default.remove("flags");
    let decoded = PlayerSave::decode(&Value::Record(missing_default)).expect("decode");
    assert_eq!(decoded.flags, Vec::<String>::new());
    assert_eq!(decoded.cache, String::new());

    let mut with_unknown = record.clone();
    with_unknown.insert("extra".to_owned(), Value::Bool(true));
    let error = PlayerSave::decode(&Value::Record(with_unknown)).expect_err("unknown field");
    assert_eq!(error.kind(), &DataErrorKind::UnknownField);

    let TypeShape::Record { fields, policy, .. } = PlayerSave::shape() else {
        panic!("expected record shape");
    };
    assert!(policy.deny_unknown_fields);
    let screenshot = fields
        .iter()
        .find(|field| field.rust_name == "screenshot_hash")
        .expect("screenshot shape");
    assert_eq!(screenshot.wire_name, "screenshot-hash");
    assert_eq!(screenshot.bytes_format, Some(BytesFormat::Hex));
    let cache = fields
        .iter()
        .find(|field| field.rust_name == "cache")
        .expect("cache shape");
    assert!(cache.skip);
}

#[test]
fn repr_enum_uses_numeric_value_and_reflects_discriminants() {
    assert_eq!(
        SaveKind::Full.encode().expect("encode"),
        Value::Number(Number::U(1))
    );
    assert_eq!(
        SaveKind::decode(&Value::Number(Number::U(2))).expect("decode"),
        SaveKind::Quick
    );

    let TypeShape::Enum { variants, repr, .. } = SaveKind::shape() else {
        panic!("expected enum shape");
    };
    assert_eq!(repr, Some(EnumRepr::U8));
    assert_eq!(variants[0].wire_name, "full");
    assert_eq!(variants[0].discriminant, Some(1));
    assert_eq!(variants[1].discriminant, Some(2));
}

#[test]
fn adjacent_tagged_enum_round_trips_named_unit_and_newtype_variants() {
    let event = AdjacentEvent::LineShown {
        line_id: "l001".to_owned(),
        speaker: "alice".to_owned(),
    };
    let encoded = event.encode().expect("encode");
    assert_eq!(AdjacentEvent::decode(&encoded).expect("decode"), event);

    let Value::Record(record) = &encoded else {
        panic!("expected adjacent record");
    };
    assert_eq!(
        record.get("kind"),
        Some(&Value::String("line_shown".to_owned()))
    );
    assert!(matches!(record.get("value"), Some(Value::Record(_))));

    let score = AdjacentEvent::Score(42);
    assert_eq!(
        AdjacentEvent::decode(&score.encode().expect("encode")).expect("decode"),
        score
    );

    let started = AdjacentEvent::Started;
    assert_eq!(
        AdjacentEvent::decode(&started.encode().expect("encode")).expect("decode"),
        started
    );

    let TypeShape::Enum { tag, .. } = AdjacentEvent::shape() else {
        panic!("expected enum shape");
    };
    assert_eq!(
        tag,
        EnumTagStyle::Adjacent {
            tag: "kind".to_owned(),
            content: "value".to_owned()
        }
    );
}

#[test]
fn external_unit_variant_rejects_unexpected_payload() {
    let error = ExternalEvent::decode(&Value::Enum {
        variant: "started".to_owned(),
        payload: Some(Box::new(Value::Bool(true))),
    })
    .expect_err("payload should be rejected");

    assert_eq!(error.kind(), &DataErrorKind::UnknownField);
}

#[test]
fn internal_tagged_enum_merges_named_fields_with_tag() {
    let event = InternalEvent::LineShown {
        line_id: "l002".to_owned(),
    };
    let encoded = event.encode().expect("encode");
    assert_eq!(InternalEvent::decode(&encoded).expect("decode"), event);

    let Value::Record(record) = encoded else {
        panic!("expected internal record");
    };
    assert_eq!(
        record.get("kind"),
        Some(&Value::String("line_shown".to_owned()))
    );
    assert_eq!(
        record.get("line_id"),
        Some(&Value::String("l002".to_owned()))
    );
    assert!(!record.contains_key("value"));

    let mut unit = BTreeMap::new();
    unit.insert("kind".to_owned(), Value::String("started".to_owned()));
    assert_eq!(
        InternalEvent::decode(&Value::Record(unit)).expect("unit decode"),
        InternalEvent::Started
    );

    let TypeShape::Enum { tag, .. } = InternalEvent::shape() else {
        panic!("expected enum shape");
    };
    assert_eq!(
        tag,
        EnumTagStyle::Internal {
            tag: "kind".to_owned()
        }
    );
}
