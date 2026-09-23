use std::{borrow::Cow, collections::BTreeMap};

use arcweft_data::{
    DataErrorKind, Decode, Encode, MapKind, Number, RawValue, Reflect, ShapeAccess,
    ShapeGraphBuilder, ShapeId, ShapeRef, TypeShape, Value, decode_with_shape,
    decode_with_shape_ref, encode_with_shape, encode_with_shape_ref,
};

#[test]
fn options_distinguish_none_from_some_unit_and_have_tagged_format_values() {
    let shape = TypeShape::option(TypeShape::Unit);
    let none = Value::Option(None);
    let some_unit = Value::Option(Some(Box::new(Value::Unit)));
    let encoded_none = Option::<()>::None
        .encode()
        .expect("None encodes dynamically");
    let encoded_some_unit = Some(()).encode().expect("Some(unit) encodes dynamically");

    assert_eq!(encoded_none, Value::Option(None));
    assert_eq!(
        encoded_some_unit,
        Value::Option(Some(Box::new(Value::Unit)))
    );
    assert_eq!(
        Option::<()>::decode(&encoded_none).expect("None decodes dynamically"),
        None
    );
    assert_eq!(
        Option::<()>::decode(&encoded_some_unit).expect("Some(unit) decodes dynamically"),
        Some(())
    );

    let none_raw = encode_with_shape(&none, &shape).expect("None encodes");
    let some_unit_raw = encode_with_shape(&some_unit, &shape).expect("Some(unit) encodes");
    assert_eq!(none_raw, RawValue::Option(None));
    assert_eq!(
        some_unit_raw,
        RawValue::Option(Some(Box::new(RawValue::Null)))
    );
    assert_ne!(none_raw, some_unit_raw);
    assert_eq!(
        decode_with_shape(&none_raw, &shape).expect("None decodes"),
        none
    );
    assert_eq!(
        decode_with_shape(&some_unit_raw, &shape).expect("Some(unit) decodes"),
        some_unit
    );

    assert_eq!(
        none_raw.into_tagged_options(),
        RawValue::Map(vec![
            (
                RawValue::String("$arcweft".to_owned()),
                RawValue::String("option".to_owned()),
            ),
            (
                RawValue::String("present".to_owned()),
                RawValue::Bool(false),
            ),
        ])
    );
    assert_eq!(
        decode_with_shape(&some_unit_raw.clone().into_tagged_options(), &shape,)
            .expect("tagged Some(unit) decodes"),
        some_unit
    );

    let nested_shape = TypeShape::option(TypeShape::option(TypeShape::Unit));
    let some_none = Value::Option(Some(Box::new(Value::Option(None))));
    let nested_raw = encode_with_shape(&some_none, &nested_shape).expect("nested option encodes");
    assert_eq!(
        decode_with_shape(&nested_raw, &nested_shape).expect("nested option decodes"),
        some_none
    );
    assert_eq!(
        decode_with_shape(&nested_raw.into_tagged_options(), &nested_shape)
            .expect("tagged nested option decodes"),
        some_none
    );

    let tagged_record = Value::Record(BTreeMap::from([
        ("$arcweft".to_owned(), Value::String("option".to_owned())),
        ("present".to_owned(), Value::Bool(false)),
    ]));
    assert!(Option::<String>::decode(&tagged_record).is_err());
}

#[test]
fn empty_and_single_item_tuples_remain_distinct_from_unit_and_sequences() {
    let empty = Value::Tuple(Vec::new());
    let singleton = (7_u8,).encode().expect("singleton tuple encodes");
    let empty_shape = TypeShape::tuple([]);
    let singleton_shape = <(u8,) as Reflect>::shape();

    assert_eq!(empty, Value::Tuple(Vec::new()));
    assert_eq!(().encode().expect("unit encodes"), Value::Unit);
    assert_eq!(<() as Reflect>::shape(), TypeShape::Unit);
    assert_eq!(<()>::decode(&Value::Unit).expect("unit decodes"), ());
    assert_eq!(singleton, Value::Tuple(vec![Value::Number(Number::U(7))]));
    assert_eq!(empty_shape, TypeShape::tuple([]));
    assert_eq!(singleton_shape, TypeShape::tuple([TypeShape::U8]));
    assert_ne!(empty_shape, singleton_shape);

    let empty_raw = encode_with_shape(&empty, &empty_shape).expect("empty tuple encodes raw");
    let singleton_raw =
        encode_with_shape(&singleton, &singleton_shape).expect("singleton encodes raw");
    assert_eq!(empty_raw, RawValue::Seq(Vec::new()));
    assert_eq!(singleton_raw, RawValue::Seq(vec![RawValue::Unsigned(7)]));
    assert_eq!(
        decode_with_shape(&empty_raw, &empty_shape).expect("empty tuple decodes"),
        empty
    );
    assert_eq!(
        decode_with_shape(&singleton_raw, &singleton_shape).expect("singleton decodes"),
        singleton
    );
    assert_eq!(
        <(u8,)>::decode(&Value::Tuple(vec![Value::Number(Number::U(7))]))
            .expect("singleton tuple decodes dynamically"),
        (7_u8,)
    );
    assert!(encode_with_shape(&Value::Seq(Vec::new()), &empty_shape).is_err());
}

#[test]
fn typed_map_keys_and_entry_order_survive_shape_transcoding() {
    let shape = TypeShape::map(TypeShape::U8, TypeShape::String, MapKind::Ordered);
    let value = Value::map(
        MapKind::Ordered,
        [
            (
                Value::Number(Number::U(2)),
                Value::String("second".to_owned()),
            ),
            (
                Value::Number(Number::U(1)),
                Value::String("first".to_owned()),
            ),
        ],
    );

    let raw = encode_with_shape(&value, &shape).expect("ordered map encodes");
    assert_eq!(
        raw,
        RawValue::Map(vec![
            (RawValue::Unsigned(2), RawValue::String("second".to_owned())),
            (RawValue::Unsigned(1), RawValue::String("first".to_owned())),
        ])
    );
    assert_eq!(
        decode_with_shape(&raw, &shape).expect("ordered map decodes"),
        value
    );

    let duplicate = Value::map(
        MapKind::Ordered,
        [
            (Value::String("same".to_owned()), Value::Bool(false)),
            (Value::String("same".to_owned()), Value::Bool(true)),
        ],
    );
    let duplicate_shape = TypeShape::map(TypeShape::String, TypeShape::Bool, MapKind::Ordered);
    assert!(encode_with_shape(&duplicate, &duplicate_shape).is_err());
}

#[test]
fn btree_map_reflection_carries_its_key_type_and_ordering_kind() {
    let value = BTreeMap::from([(2_u32, "second".to_owned()), (1_u32, "first".to_owned())]);
    let encoded = value.encode().expect("BTreeMap encodes");
    let shape = BTreeMap::<u32, String>::shape();

    assert_eq!(
        shape,
        TypeShape::map(TypeShape::U32, TypeShape::String, MapKind::BTree)
    );
    assert_eq!(
        encoded,
        Value::map(
            MapKind::BTree,
            [
                (
                    Value::Number(Number::U(1)),
                    Value::String("first".to_owned()),
                ),
                (
                    Value::Number(Number::U(2)),
                    Value::String("second".to_owned()),
                ),
            ],
        )
    );
    assert_eq!(
        BTreeMap::<u32, String>::decode(&encoded).expect("map decodes"),
        value
    );
}

#[test]
fn recursive_shape_graph_transcodes_through_identity_edges() {
    let mut builder = ShapeGraphBuilder::new();
    let node = builder.reserve();
    builder
        .define(
            node,
            TypeShape::record(
                "Node",
                [arcweft_data::FieldShape::new(
                    "next",
                    "next",
                    TypeShape::option(TypeShape::Ref(node)),
                )],
            ),
        )
        .expect("node shape is defined");
    let graph = builder.finish().expect("recursive graph is closed");
    let leaf = Value::Record(BTreeMap::from([("next".to_owned(), Value::Option(None))]));
    let root = Value::Record(BTreeMap::from([(
        "next".to_owned(),
        Value::Option(Some(Box::new(leaf))),
    )]));

    let raw =
        encode_with_shape_ref(&root, ShapeRef::id(node), &graph).expect("recursive value encodes");
    assert_eq!(
        decode_with_shape_ref(&raw, ShapeRef::id(node), &graph).expect("recursive value decodes"),
        root
    );
}

#[test]
fn shape_access_rejects_unproductive_reference_cycles() {
    struct SelfReference;

    impl ShapeAccess for SelfReference {
        fn get_shape(&self, id: ShapeId) -> Option<Cow<'_, TypeShape>> {
            Some(Cow::Owned(TypeShape::Ref(id)))
        }
    }

    let error = encode_with_shape_ref(&Value::Unit, ShapeRef::id(ShapeId::new(0)), &SelfReference)
        .expect_err("reference-only cycles are invalid");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);
}
