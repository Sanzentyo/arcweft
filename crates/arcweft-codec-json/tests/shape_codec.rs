// Inline-shape test adapter. Production Codec callers supply ShapeRef and ShapeAccess directly.
use self::InlineCodecTestExt as Codec;

trait InlineCodecTestExt {
    fn encode_value(
        &self,
        value: &arcweft_data::Value,
        shape: &arcweft_data::TypeShape,
        options: &arcweft_data::EncodeOptions,
    ) -> arcweft_data::Result<Vec<u8>>;

    fn decode_value(
        &self,
        input: &[u8],
        shape: &arcweft_data::TypeShape,
        options: &arcweft_data::DecodeOptions,
    ) -> arcweft_data::Result<arcweft_data::Value>;
}

impl<C: arcweft_data::Codec + ?Sized> InlineCodecTestExt for C {
    fn encode_value(
        &self,
        value: &arcweft_data::Value,
        shape: &arcweft_data::TypeShape,
        options: &arcweft_data::EncodeOptions,
    ) -> arcweft_data::Result<Vec<u8>> {
        <C as arcweft_data::Codec>::encode_value(
            self,
            value,
            arcweft_data::ShapeRef::Inline(shape),
            &arcweft_data::EmptyShapeAccess,
            options,
        )
    }

    fn decode_value(
        &self,
        input: &[u8],
        shape: &arcweft_data::TypeShape,
        options: &arcweft_data::DecodeOptions,
    ) -> arcweft_data::Result<arcweft_data::Value> {
        <C as arcweft_data::Codec>::decode_value(
            self,
            input,
            arcweft_data::ShapeRef::Inline(shape),
            &arcweft_data::EmptyShapeAccess,
            options,
        )
    }
}
use std::collections::BTreeMap;

use arcweft_codec_json::JsonCodec;
use arcweft_data::{
    Bytes, BytesFormat, DataErrorKind, DecodeLimits, DecodeOptions, EncodeOptions, EnumRepr,
    EnumTagStyle, FieldShape, MapKind, Number, RecordPolicy, ShapeGraphBuilder, ShapeRef,
    TypeShape, Value, VariantShape,
};

fn asset_shape() -> TypeShape {
    TypeShape::Record {
        name: "Asset".to_owned(),
        fields: vec![
            FieldShape::new(
                "hash",
                "hash",
                TypeShape::Bytes {
                    format: BytesFormat::Binary,
                },
            )
            .with_bytes_format(BytesFormat::Hex),
            FieldShape::new("name", "name", TypeShape::String),
        ],
        policy: RecordPolicy {
            deny_unknown_fields: true,
        },
    }
}

fn enum_byte_graph() -> (arcweft_data::ShapeGraph, arcweft_data::ShapeId) {
    let mut builder = ShapeGraphBuilder::new();
    let binary_bytes = builder.reserve();
    let base64_bytes = builder.reserve();
    let root = builder.reserve();
    builder
        .define(
            binary_bytes,
            TypeShape::Bytes {
                format: BytesFormat::Binary,
            },
        )
        .expect("define binary Bytes child");
    builder
        .define(
            base64_bytes,
            TypeShape::Bytes {
                format: BytesFormat::Base64,
            },
        )
        .expect("define Base64 Bytes child");
    builder
        .define(
            root,
            TypeShape::Enum {
                name: "Adjacent".to_owned(),
                variants: vec![
                    VariantShape::unit("Payload", "one-value")
                        .with_payload(TypeShape::Ref(base64_bytes)),
                    VariantShape::unit("NamedValue", "named-value").with_payload(
                        TypeShape::Record {
                            name: "Adjacent::NamedValue".to_owned(),
                            fields: vec![
                                FieldShape::new(
                                    "byte_value",
                                    "byte-value",
                                    TypeShape::Ref(binary_bytes),
                                )
                                .with_bytes_format(BytesFormat::Hex),
                            ],
                            policy: RecordPolicy {
                                deny_unknown_fields: true,
                            },
                        },
                    ),
                ],
                tag: EnumTagStyle::Adjacent {
                    tag: "kind".to_owned(),
                    content: "body".to_owned(),
                },
                repr: None,
            },
        )
        .expect("define enum root");
    (builder.finish().expect("finish enum graph"), root)
}

#[test]
fn json_enum_payloads_follow_graph_bytes_formats_and_field_overrides() {
    let (graph, root) = enum_byte_graph();
    let values = [
        Value::Enum {
            variant: "one-value".to_owned(),
            payload: Some(Box::new(Value::Bytes(Bytes::from([8_u8, 9].as_slice())))),
        },
        Value::Enum {
            variant: "named-value".to_owned(),
            payload: Some(Box::new(Value::Record(BTreeMap::from([(
                "byte-value".to_owned(),
                Value::Bytes(Bytes::from([8_u8, 9].as_slice())),
            )])))),
        },
    ];
    for (value, expected) in values.into_iter().zip(["CAk=", "0809"]) {
        let encoded = <JsonCodec as arcweft_data::Codec>::encode_value(
            &JsonCodec,
            &value,
            ShapeRef::Id(root),
            &graph,
            &EncodeOptions::default(),
        )
        .expect("enum payload follows its selected shape");
        let text = std::str::from_utf8(&encoded).expect("JSON output is UTF-8");
        assert!(text.contains(expected), "{text}");
        assert!(text.contains("kind") && text.contains("body") && text.contains("value"));
        assert_eq!(
            <JsonCodec as arcweft_data::Codec>::decode_value(
                &JsonCodec,
                &encoded,
                ShapeRef::Id(root),
                &graph,
                &DecodeOptions::default(),
            )
            .expect("enum payload decodes through the same shape graph"),
            value
        );
    }
}

#[test]
fn json_enum_external_and_internal_payload_tags_roundtrip() {
    let external = TypeShape::Enum {
        name: "External".to_owned(),
        variants: vec![
            VariantShape::unit("Payload", "renamed-payload").with_payload(TypeShape::Bytes {
                format: BytesFormat::Hex,
            }),
        ],
        tag: EnumTagStyle::External,
        repr: None,
    };
    let external_value = Value::Enum {
        variant: "renamed-payload".to_owned(),
        payload: Some(Box::new(Value::Bytes(Bytes::from([8_u8, 9].as_slice())))),
    };
    let external_json = JsonCodec
        .encode_value(&external_value, &external, &EncodeOptions::default())
        .expect("encode external enum");
    assert!(
        std::str::from_utf8(&external_json)
            .expect("JSON is UTF-8")
            .contains("0809")
    );
    assert_eq!(
        JsonCodec
            .decode_value(&external_json, &external, &DecodeOptions::default())
            .expect("decode external enum"),
        external_value
    );

    let internal = TypeShape::Enum {
        name: "Internal".to_owned(),
        variants: vec![
            VariantShape::unit("NamedValue", "named-value").with_payload(TypeShape::Record {
                name: "NamedValue".to_owned(),
                fields: vec![
                    FieldShape::new(
                        "byte_value",
                        "byte-value",
                        TypeShape::Bytes {
                            format: BytesFormat::Binary,
                        },
                    )
                    .with_bytes_format(BytesFormat::Hex),
                ],
                policy: RecordPolicy {
                    deny_unknown_fields: true,
                },
            }),
        ],
        tag: EnumTagStyle::Internal {
            tag: "kind".to_owned(),
        },
        repr: None,
    };
    let internal_value = Value::Enum {
        variant: "named-value".to_owned(),
        payload: Some(Box::new(Value::Record(BTreeMap::from([(
            "byte-value".to_owned(),
            Value::Bytes(Bytes::from([8_u8, 9].as_slice())),
        )])))),
    };
    let internal_json = JsonCodec
        .encode_value(&internal_value, &internal, &EncodeOptions::default())
        .expect("encode internal enum");
    assert!(
        std::str::from_utf8(&internal_json)
            .expect("JSON is UTF-8")
            .contains("0809")
    );
    assert_eq!(
        JsonCodec
            .decode_value(&internal_json, &internal, &DecodeOptions::default())
            .expect("decode internal enum"),
        internal_value
    );
}

#[test]
fn json_roundtrips_typed_options_tuples_ordered_maps_and_marker_records() {
    let option_shape = TypeShape::option(TypeShape::Unit);
    for value in [
        Value::Option(None),
        Value::Option(Some(Box::new(Value::Unit))),
    ] {
        let encoded = JsonCodec
            .encode_value(&value, &option_shape, &EncodeOptions::default())
            .expect("encode option");
        assert_eq!(
            JsonCodec
                .decode_value(&encoded, &option_shape, &DecodeOptions::default())
                .expect("decode option"),
            value
        );
    }

    for (shape, value) in [
        (TypeShape::Tuple(vec![]), Value::Tuple(vec![])),
        (
            TypeShape::Tuple(vec![TypeShape::Bool]),
            Value::Tuple(vec![Value::Bool(true)]),
        ),
    ] {
        let encoded = JsonCodec
            .encode_value(&value, &shape, &EncodeOptions::default())
            .expect("encode tuple");
        assert_eq!(
            JsonCodec
                .decode_value(&encoded, &shape, &DecodeOptions::default())
                .expect("decode tuple"),
            value
        );
    }

    let map_shape = TypeShape::Map {
        key: Box::new(TypeShape::U8),
        value: Box::new(TypeShape::String),
        kind: MapKind::Ordered,
    };
    let map = Value::map(
        MapKind::Ordered,
        [
            (Value::Number(Number::U(2)), Value::String("two".to_owned())),
            (Value::Number(Number::U(1)), Value::String("one".to_owned())),
        ],
    );
    let encoded = JsonCodec
        .encode_value(&map, &map_shape, &EncodeOptions::default())
        .expect("encode pair-array map");
    assert_eq!(
        std::str::from_utf8(&encoded).expect("utf8"),
        "[[2,\"two\"],[1,\"one\"]]"
    );
    assert_eq!(
        JsonCodec
            .decode_value(&encoded, &map_shape, &DecodeOptions::default())
            .expect("decode pair-array map"),
        map
    );

    let string_map_shape = TypeShape::map(TypeShape::String, TypeShape::String, MapKind::Ordered);
    let string_map = Value::map(
        MapKind::Ordered,
        [
            (
                Value::String("b".to_owned()),
                Value::String("bee".to_owned()),
            ),
            (
                Value::String("a".to_owned()),
                Value::String("aye".to_owned()),
            ),
        ],
    );
    let encoded = JsonCodec
        .encode_value(&string_map, &string_map_shape, &EncodeOptions::default())
        .expect("encode ordered object map");
    assert_eq!(
        std::str::from_utf8(&encoded).expect("utf8"),
        r#"{"b":"bee","a":"aye"}"#
    );
    assert_eq!(
        JsonCodec
            .decode_value(&encoded, &string_map_shape, &DecodeOptions::default())
            .expect("decode ordered object map"),
        string_map
    );

    let marker_shape = TypeShape::record(
        "MarkerLookingRecord",
        [
            FieldShape::new("$arcweft", "$arcweft", TypeShape::String),
            FieldShape::new("present", "present", TypeShape::Bool),
            FieldShape::new("value", "value", TypeShape::String),
        ],
    );
    let marker_record = Value::Record(BTreeMap::from([
        ("$arcweft".to_owned(), Value::String("option".to_owned())),
        ("present".to_owned(), Value::Bool(true)),
        ("value".to_owned(), Value::String("literal".to_owned())),
    ]));
    let encoded = JsonCodec
        .encode_value(&marker_record, &marker_shape, &EncodeOptions::default())
        .expect("encode marker-looking record");
    assert_eq!(
        JsonCodec
            .decode_value(&encoded, &marker_shape, &DecodeOptions::default())
            .expect("decode marker-looking record"),
        marker_record
    );
}

#[test]
fn json_codec_traverses_shape_graph_references() {
    let mut builder = ShapeGraphBuilder::new();
    let node = builder.reserve();
    builder
        .define(
            node,
            TypeShape::record(
                "Node",
                [
                    FieldShape::new("name", "name", TypeShape::String),
                    FieldShape::new("next", "next", TypeShape::option(TypeShape::Ref(node))),
                ],
            ),
        )
        .expect("define recursive node shape");
    let graph = builder.finish().expect("shape graph");
    let value = Value::Record(BTreeMap::from([
        ("name".to_owned(), Value::String("root".to_owned())),
        ("next".to_owned(), Value::Option(None)),
    ]));
    let encoded = <JsonCodec as arcweft_data::Codec>::encode_value(
        &JsonCodec,
        &value,
        ShapeRef::Id(node),
        &graph,
        &EncodeOptions::default(),
    )
    .expect("encode graph shape");
    assert_eq!(
        <JsonCodec as arcweft_data::Codec>::decode_value(
            &JsonCodec,
            &encoded,
            ShapeRef::Id(node),
            &graph,
            &DecodeOptions::default(),
        )
        .expect("decode graph shape"),
        value
    );
}

#[test]
fn json_codec_applies_distinct_bytes_formats_to_shared_graph_fields() {
    let mut builder = ShapeGraphBuilder::new();
    let bytes_node = builder.reserve();
    let record_node = builder.reserve();
    builder
        .define(
            bytes_node,
            TypeShape::Bytes {
                format: BytesFormat::Binary,
            },
        )
        .expect("define shared bytes shape");
    builder
        .define(
            record_node,
            TypeShape::record(
                "Payload",
                [
                    FieldShape::new("base64", "base64", TypeShape::Ref(bytes_node))
                        .with_bytes_format(BytesFormat::Base64),
                    FieldShape::new("hex", "hex", TypeShape::Ref(bytes_node))
                        .with_bytes_format(BytesFormat::Hex),
                ],
            ),
        )
        .expect("define graph record");
    let graph = builder.finish().expect("shape graph");
    let payload = Bytes::from([1_u8, 2, 255].as_slice());
    let value = Value::Record(BTreeMap::from([
        ("base64".to_owned(), Value::Bytes(payload.clone())),
        ("hex".to_owned(), Value::Bytes(payload)),
    ]));

    let encoded = <JsonCodec as arcweft_data::Codec>::encode_value(
        &JsonCodec,
        &value,
        ShapeRef::Id(record_node),
        &graph,
        &EncodeOptions::default(),
    )
    .expect("encode graph fields");
    assert_eq!(
        std::str::from_utf8(&encoded).expect("utf8"),
        r#"{"base64":"AQL/","hex":"0102ff"}"#
    );
    assert_eq!(
        <JsonCodec as arcweft_data::Codec>::decode_value(
            &JsonCodec,
            &encoded,
            ShapeRef::Id(record_node),
            &graph,
            &DecodeOptions::default(),
        )
        .expect("decode graph fields"),
        value
    );
}

#[test]
fn json_codec_uses_shape_bytes_policy_for_records() {
    let value = Value::Record(BTreeMap::from([
        (
            "hash".to_owned(),
            Value::Bytes(Bytes::from([1_u8, 2, 255].as_slice())),
        ),
        ("name".to_owned(), Value::String("hero".to_owned())),
    ]));
    let json = JsonCodec
        .encode_value(&value, &asset_shape(), &EncodeOptions::default())
        .expect("encode");
    assert_eq!(
        std::str::from_utf8(&json).expect("utf8"),
        r#"{"hash":"0102ff","name":"hero"}"#
    );

    let decoded = JsonCodec
        .decode_value(&json, &asset_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn json_codec_rejects_unknown_record_fields_through_shape() {
    let error = JsonCodec
        .decode_value(
            br#"{"hash":"00","name":"hero","extra":true}"#,
            &asset_shape(),
            &DecodeOptions::default(),
        )
        .expect_err("unknown field");
    assert_eq!(error.kind(), &DataErrorKind::UnknownField);
}

#[test]
fn json_codec_checks_input_limit_before_parse() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_input_len: 2,
            ..DecodeLimits::default()
        },
    };
    let error = JsonCodec
        .decode_value(br"null", &TypeShape::Unit, &options)
        .expect_err("input cap");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn json_codec_rejects_trailing_non_whitespace() {
    let error = JsonCodec
        .decode_value(br"null true", &TypeShape::Unit, &DecodeOptions::default())
        .expect_err("trailing");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);
}

#[test]
fn json_codec_uses_adjacent_enum_tag_shape() {
    let shape = TypeShape::Enum {
        name: "Event".to_owned(),
        variants: vec![VariantShape::unit("Score", "score").with_payload(TypeShape::U32)],
        tag: EnumTagStyle::Adjacent {
            tag: "kind".to_owned(),
            content: "value".to_owned(),
        },
        repr: None,
    };
    let value = Value::Enum {
        variant: "score".to_owned(),
        payload: Some(Box::new(Value::Number(Number::U(42)))),
    };

    let json = JsonCodec
        .encode_value(&value, &shape, &EncodeOptions::default())
        .expect("encode");
    assert_eq!(
        std::str::from_utf8(&json).expect("utf8"),
        r#"{"kind":"score","value":42}"#
    );
    assert_eq!(
        JsonCodec
            .decode_value(&json, &shape, &DecodeOptions::default())
            .expect("decode"),
        value
    );
}

#[test]
fn json_codec_roundtrips_repr_enum_as_number() {
    let shape = TypeShape::Enum {
        name: "SaveKind".to_owned(),
        variants: vec![
            VariantShape::unit("Full", "full").with_discriminant(1),
            VariantShape::unit("Quick", "quick").with_discriminant(2),
        ],
        tag: EnumTagStyle::External,
        repr: Some(EnumRepr::U8),
    };
    let value = Value::Enum {
        variant: "quick".to_owned(),
        payload: None,
    };

    let json = JsonCodec
        .encode_value(&value, &shape, &EncodeOptions::default())
        .expect("encode");
    assert_eq!(std::str::from_utf8(&json).expect("utf8"), "2");
    assert_eq!(
        JsonCodec
            .decode_value(&json, &shape, &DecodeOptions::default())
            .expect("decode"),
        value
    );
}

#[test]
fn json_codec_enforces_numeric_edge_policy() {
    let error = JsonCodec
        .encode_value(
            &Value::Number(Number::F64(f64::NAN)),
            &TypeShape::F64,
            &EncodeOptions::default(),
        )
        .expect_err("nan rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let error = JsonCodec
        .decode_value(b"1.5", &TypeShape::U8, &DecodeOptions::default())
        .expect_err("float to integer rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let error = JsonCodec
        .decode_value(b"-1", &TypeShape::U8, &DecodeOptions::default())
        .expect_err("negative unsigned rejected");
    assert_eq!(error.kind(), &DataErrorKind::NumberOutOfRange);

    let error = JsonCodec
        .encode_value(
            &Value::Number(Number::U(u128::MAX)),
            &TypeShape::U128,
            &EncodeOptions::default(),
        )
        .expect_err("u128 beyond JSON number rejected");
    assert_eq!(error.kind(), &DataErrorKind::NumberOutOfRange);
}

#[test]
fn json_decode_consumes_string_budget_during_parse() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_string_len: 3,
            ..DecodeLimits::default()
        },
    };

    let error = JsonCodec
        .decode_value(br#""hero""#, &TypeShape::String, &options)
        .expect_err("string budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn json_decode_consumes_collection_budget_during_parse() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_sequence_len: 2,
            ..DecodeLimits::default()
        },
    };

    let error = JsonCodec
        .decode_value(
            br"[null,null,null]",
            &TypeShape::Seq(Box::new(TypeShape::Unit)),
            &options,
        )
        .expect_err("array budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn json_decode_consumes_node_budget_during_parse() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_nodes: 2,
            ..DecodeLimits::default()
        },
    };

    let error = JsonCodec
        .decode_value(
            br"[null,null]",
            &TypeShape::Seq(Box::new(TypeShape::Unit)),
            &options,
        )
        .expect_err("node budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}
