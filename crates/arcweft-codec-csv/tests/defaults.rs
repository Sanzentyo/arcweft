use arcweft_codec_csv::CsvCodec;
use arcweft_data::{
    Codec, DataErrorKind, DecodeOptions, DecodeShapeAccess, FieldDefaultProvider,
    FieldDefaultRequest, FieldShape, Result, ShapeGraphBuilder, ShapeId, ShapeRef, TypeShape,
    Value,
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
fn csv_missing_and_skipped_columns_use_the_original_record_default_requests() {
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
    let defaults = Defaults {
        record,
        calls: RefCell::new(vec![]),
    };
    let access = DecodeShapeAccess::new(&graph, &defaults);
    let decoded = CsvCodec
        .decode_value(
            b"keep\nfalse\n",
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
        CsvCodec
            .decode_value(
                b"keep\nfalse\n",
                ShapeRef::Id(root),
                &graph,
                &DecodeOptions::default()
            )
            .unwrap_err()
            .kind(),
        &DataErrorKind::MissingField
    );
}
