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

use arcweft_codec_yaml::YamlCodec;
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
            FieldShape::new("tag", "tag", TypeShape::option(TypeShape::String)),
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
fn yaml_enum_payloads_follow_graph_bytes_formats_and_field_overrides() {
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
        let encoded = <YamlCodec as arcweft_data::Codec>::encode_value(
            &YamlCodec,
            &value,
            ShapeRef::Id(root),
            &graph,
            &EncodeOptions::default(),
        )
        .expect("enum payload follows its selected shape");
        let text = std::str::from_utf8(&encoded).expect("YAML output is UTF-8");
        assert!(text.contains(expected), "{text}");
        assert!(text.contains("kind") && text.contains("body") && text.contains("value"));
        assert_eq!(
            <YamlCodec as arcweft_data::Codec>::decode_value(
                &YamlCodec,
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

fn assert_yaml_enum_roundtrip(shape: &TypeShape, value: &Value) {
    let encoded = YamlCodec
        .encode_value(value, shape, &EncodeOptions::default())
        .expect("encode enum");
    assert_eq!(
        YamlCodec
            .decode_value(&encoded, shape, &DecodeOptions::default())
            .expect("decode enum"),
        *value
    );
}

#[test]
fn yaml_enum_external_internal_and_repr_shapes_roundtrip() {
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
    assert_yaml_enum_roundtrip(
        &external,
        &Value::Enum {
            variant: "renamed-payload".to_owned(),
            payload: Some(Box::new(Value::Bytes(Bytes::from([8_u8, 9].as_slice())))),
        },
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
    assert_yaml_enum_roundtrip(
        &internal,
        &Value::Enum {
            variant: "named-value".to_owned(),
            payload: Some(Box::new(Value::Record(BTreeMap::from([(
                "byte-value".to_owned(),
                Value::Bytes(Bytes::from([8_u8, 9].as_slice())),
            )])))),
        },
    );

    let repr = TypeShape::Enum {
        name: "Repr".to_owned(),
        variants: vec![
            VariantShape::unit("Full", "full").with_discriminant(1),
            VariantShape::unit("Quick", "quick").with_discriminant(2),
        ],
        tag: EnumTagStyle::External,
        repr: Some(EnumRepr::U8),
    };
    assert_yaml_enum_roundtrip(
        &repr,
        &Value::Enum {
            variant: "quick".to_owned(),
            payload: None,
        },
    );
}

#[test]
fn yaml_codec_uses_shape_bytes_policy_for_records() {
    let value = Value::Record(BTreeMap::from([
        (
            "hash".to_owned(),
            Value::Bytes(Bytes::from([1_u8, 2, 255].as_slice())),
        ),
        ("name".to_owned(), Value::String("hero".to_owned())),
        (
            "tag".to_owned(),
            Value::Option(Some(Box::new(Value::String("npc".to_owned())))),
        ),
    ]));

    let yaml = YamlCodec
        .encode_value(&value, &asset_shape(), &EncodeOptions::default())
        .expect("encode");
    let text = std::str::from_utf8(&yaml).expect("utf8");
    assert!(text.contains("0102ff"));
    assert!(text.contains("hero"));

    let decoded = YamlCodec
        .decode_value(&yaml, &asset_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn yaml_codec_roundtrips_record_option_none_as_null() {
    let value = Value::Record(BTreeMap::from([
        (
            "hash".to_owned(),
            Value::Bytes(Bytes::from([0_u8, 255].as_slice())),
        ),
        ("name".to_owned(), Value::String("hero".to_owned())),
        ("tag".to_owned(), Value::Option(None)),
    ]));

    let yaml = YamlCodec
        .encode_value(&value, &asset_shape(), &EncodeOptions::default())
        .expect("encode");
    let text = std::str::from_utf8(&yaml).expect("utf8");
    assert!(text.contains("tag"));

    let decoded = YamlCodec
        .decode_value(&yaml, &asset_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn yaml_codec_roundtrips_typed_options_tuples_ordered_maps_and_marker_records() {
    let shape = TypeShape::record(
        "TypedValues",
        [
            FieldShape::new("none", "none", TypeShape::option(TypeShape::Unit)),
            FieldShape::new("some_unit", "some_unit", TypeShape::option(TypeShape::Unit)),
            FieldShape::new("empty_tuple", "empty_tuple", TypeShape::Tuple(vec![])),
            FieldShape::new(
                "one_tuple",
                "one_tuple",
                TypeShape::Tuple(vec![TypeShape::Bool]),
            ),
            FieldShape::new(
                "ordered",
                "ordered",
                TypeShape::map(TypeShape::U8, TypeShape::String, MapKind::Ordered),
            ),
            FieldShape::new(
                "marker",
                "marker",
                TypeShape::record(
                    "MarkerLookingRecord",
                    [
                        FieldShape::new("$arcweft", "$arcweft", TypeShape::String),
                        FieldShape::new("present", "present", TypeShape::Bool),
                        FieldShape::new("value", "value", TypeShape::String),
                    ],
                ),
            ),
        ],
    );
    let value = Value::Record(BTreeMap::from([
        ("none".to_owned(), Value::Option(None)),
        (
            "some_unit".to_owned(),
            Value::Option(Some(Box::new(Value::Unit))),
        ),
        ("empty_tuple".to_owned(), Value::Tuple(vec![])),
        (
            "one_tuple".to_owned(),
            Value::Tuple(vec![Value::Bool(true)]),
        ),
        (
            "ordered".to_owned(),
            Value::map(
                MapKind::Ordered,
                [
                    (Value::Number(Number::U(2)), Value::String("two".to_owned())),
                    (Value::Number(Number::U(1)), Value::String("one".to_owned())),
                ],
            ),
        ),
        (
            "marker".to_owned(),
            Value::Record(BTreeMap::from([
                ("$arcweft".to_owned(), Value::String("option".to_owned())),
                ("present".to_owned(), Value::Bool(true)),
                ("value".to_owned(), Value::String("literal".to_owned())),
            ])),
        ),
    ]));

    let encoded = YamlCodec
        .encode_value(&value, &shape, &EncodeOptions::default())
        .expect("encode typed values");
    let decoded = YamlCodec
        .decode_value(&encoded, &shape, &DecodeOptions::default())
        .expect("decode typed values");
    assert_eq!(decoded, value);
}

#[test]
fn yaml_codec_preserves_ordered_string_map_entries() {
    let shape = TypeShape::map(TypeShape::String, TypeShape::String, MapKind::Ordered);
    let value = Value::map(
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

    let encoded = YamlCodec
        .encode_value(&value, &shape, &EncodeOptions::default())
        .expect("encode ordered string map");
    let text = std::str::from_utf8(&encoded).expect("utf8");
    let b_position = text.find("b: bee").expect("first map entry");
    let a_position = text.find("a: aye").expect("second map entry");
    assert!(
        b_position < a_position,
        "YAML string map entry order changed: {text}"
    );
    assert_eq!(
        YamlCodec
            .decode_value(&encoded, &shape, &DecodeOptions::default())
            .expect("decode ordered string map"),
        value
    );
}

#[test]
fn yaml_codec_rejects_unknown_record_fields_through_shape() {
    let error = YamlCodec
        .decode_value(
            b"hash: '00'\nname: hero\nextra: true\n",
            &asset_shape(),
            &DecodeOptions::default(),
        )
        .expect_err("unknown field");
    assert_eq!(error.kind(), &DataErrorKind::UnknownField);
}

#[test]
fn yaml_codec_checks_input_limit_before_parse() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_input_len: 2,
            ..DecodeLimits::default()
        },
    };
    let error = YamlCodec
        .decode_value(b"name: hero\n", &asset_shape(), &options)
        .expect_err("input cap");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn yaml_codec_rejects_multiple_documents() {
    let error = YamlCodec
        .decode_value(
            b"hash: '00'\nname: hero\n---\nhash: '01'\nname: villain\n",
            &asset_shape(),
            &DecodeOptions::default(),
        )
        .expect_err("second document");
    assert_eq!(error.kind(), &DataErrorKind::TrailingData);
}

#[test]
fn yaml_codec_enforces_numeric_edge_policy() {
    let error = YamlCodec
        .encode_value(
            &Value::Number(Number::F32(f32::NAN)),
            &TypeShape::F32,
            &EncodeOptions::default(),
        )
        .expect_err("nan rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let shape = TypeShape::record(
        "Numeric",
        [FieldShape::new("count", "count", TypeShape::U8)],
    );
    let error = YamlCodec
        .decode_value(b"count: 1.5\n", &shape, &DecodeOptions::default())
        .expect_err("float to integer rejected");
    assert_eq!(error.kind(), &DataErrorKind::InvalidType);

    let error = YamlCodec
        .decode_value(b"count: -1\n", &shape, &DecodeOptions::default())
        .expect_err("negative unsigned rejected");
    assert_eq!(error.kind(), &DataErrorKind::NumberOutOfRange);
}

#[test]
fn yaml_decode_consumes_string_budget_before_loader_tree() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_string_len: 3,
            ..DecodeLimits::default()
        },
    };

    let error = YamlCodec
        .decode_value(b"hash: '00'\nname: hero\n", &asset_shape(), &options)
        .expect_err("string budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn yaml_decode_preflights_quoted_scalar_budget_before_event_allocation() {
    let mut input = b"'".to_vec();
    input.extend(std::iter::repeat_n(b'a', 16 * 1024));
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_string_len: 8,
            ..DecodeLimits::default()
        },
    };

    let error = YamlCodec
        .decode_value(&input, &TypeShape::String, &options)
        .expect_err("source quoted scalar budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn yaml_decode_preflights_plain_scalar_budget_before_event_allocation() {
    let mut input = Vec::new();
    input.extend(std::iter::repeat_n(b'a', 16 * 1024));
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_string_len: 8,
            ..DecodeLimits::default()
        },
    };

    let error = YamlCodec
        .decode_value(&input, &TypeShape::String, &options)
        .expect_err("source plain scalar budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn yaml_decode_preflights_block_scalar_budget_before_event_allocation() {
    let mut input = b"|\n  ".to_vec();
    input.extend(std::iter::repeat_n(b'a', 16 * 1024));
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_string_len: 8,
            ..DecodeLimits::default()
        },
    };

    let error = YamlCodec
        .decode_value(&input, &TypeShape::String, &options)
        .expect_err("source block scalar budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn yaml_decode_consumes_sequence_budget_before_loader_tree() {
    let shape = TypeShape::record(
        "Tags",
        [FieldShape::new(
            "tags",
            "tags",
            TypeShape::seq(TypeShape::String),
        )],
    );
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_sequence_len: 2,
            ..DecodeLimits::default()
        },
    };

    let error = YamlCodec
        .decode_value(b"tags: [a, b, c]\n", &shape, &options)
        .expect_err("sequence budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn yaml_decode_consumes_node_budget_before_loader_tree() {
    let options = DecodeOptions {
        limits: DecodeLimits {
            max_nodes: 2,
            ..DecodeLimits::default()
        },
    };

    let error = YamlCodec
        .decode_value(b"hash: '00'\nname: hero\n", &asset_shape(), &options)
        .expect_err("node budget");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}
