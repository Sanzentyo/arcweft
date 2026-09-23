use arcweft_codec_avro::codec::AvroCodec;
use arcweft_data::{
    Codec, DataErrorKind, DecodeOptions, DecodeShapeAccess, EmptyShapeAccess, EncodeOptions,
    FieldDefaultProvider, FieldDefaultRequest, FieldShape, Result, ShapeGraphBuilder, ShapeId,
    ShapeRef, TypeShape, Value,
};
use std::{cell::RefCell, collections::BTreeMap};

struct Defaults {
    record: ShapeId,
    calls: RefCell<Vec<usize>>,
}
impl FieldDefaultProvider for Defaults {
    fn default_value(&self, request: FieldDefaultRequest<'_>) -> Result<Value> {
        assert_eq!(request.record_id(), Some(self.record));
        self.calls.borrow_mut().push(request.field_ordinal());
        Ok(Value::Bool(request.field_ordinal() == 1))
    }
}

#[test]
fn avro_missing_and_skipped_fields_request_defaults_at_the_original_graph_row() {
    let codec = AvroCodec::new(
        r#"{"type":"record","name":"Wire","fields":[{"name":"keep","type":"boolean"}]}"#,
    )
    .unwrap();
    let mut builder = ShapeGraphBuilder::new();
    let record = builder.reserve();
    let root = builder.reserve();
    builder
        .define(
            record,
            TypeShape::record(
                "Wire",
                [
                    FieldShape::new("keep", "keep", TypeShape::Bool),
                    FieldShape::new("flag", "flag", TypeShape::Bool).with_default(),
                    FieldShape::new("cache", "cache", TypeShape::Bool).skipped(),
                ],
            ),
        )
        .unwrap();
    builder
        .define(root, TypeShape::seq(TypeShape::Ref(record)))
        .unwrap();
    let graph = builder.finish().unwrap();
    let writer_shape = TypeShape::seq(TypeShape::record(
        "Wire",
        [FieldShape::new("keep", "keep", TypeShape::Bool)],
    ));
    let input = Value::Seq(vec![Value::Record(BTreeMap::from([(
        "keep".to_owned(),
        Value::Bool(false),
    )]))]);
    let encoded = codec
        .encode_value(
            &input,
            ShapeRef::Inline(&writer_shape),
            &EmptyShapeAccess,
            &EncodeOptions::default(),
        )
        .unwrap();
    let defaults = Defaults {
        record,
        calls: RefCell::new(vec![]),
    };
    let access = DecodeShapeAccess::new(&graph, &defaults);
    let decoded = codec
        .decode_value(
            &encoded,
            ShapeRef::Id(root),
            &access,
            &DecodeOptions::default(),
        )
        .unwrap();
    assert_eq!(
        decoded,
        Value::Seq(vec![Value::Record(BTreeMap::from([
            ("keep".to_owned(), Value::Bool(false)),
            ("flag".to_owned(), Value::Bool(true)),
            ("cache".to_owned(), Value::Bool(false)),
        ]))])
    );
    assert_eq!(*defaults.calls.borrow(), [1, 2]);
    assert_eq!(
        codec
            .decode_value(
                &encoded,
                ShapeRef::Id(root),
                &graph,
                &DecodeOptions::default()
            )
            .unwrap_err()
            .kind(),
        &DataErrorKind::MissingField
    );
}
