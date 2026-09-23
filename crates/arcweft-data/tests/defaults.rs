use std::{cell::RefCell, collections::BTreeMap};

use arcweft_data::{
    DataErrorKind, DecodeShapeAccess, FieldDefaultProvider, FieldDefaultRequest, FieldShape,
    RawValue, Result, ShapeGraph, ShapeGraphBuilder, ShapeId, ShapeRef, TypeShape, Value,
    decode_with_shape_ref,
};

struct Defaults {
    record: ShapeId,
    calls: RefCell<Vec<usize>>,
    wrong_type: bool,
}

impl FieldDefaultProvider for Defaults {
    fn default_value(&self, request: FieldDefaultRequest<'_>) -> Result<Value> {
        assert_eq!(request.record_id(), Some(self.record));
        self.calls.borrow_mut().push(request.field_ordinal());
        Ok(if self.wrong_type {
            Value::String("not a bool".to_owned())
        } else {
            Value::Bool(request.field_ordinal() == 0)
        })
    }
}

fn graph() -> (ShapeGraph, ShapeId, ShapeId) {
    let mut builder = ShapeGraphBuilder::new();
    let boolean = builder.reserve();
    let inner = builder.reserve();
    let outer = builder.reserve();
    builder.define(boolean, TypeShape::Bool).unwrap();
    builder
        .define(
            inner,
            TypeShape::record(
                "Inner",
                [
                    FieldShape::new("flag", "wire_flag", TypeShape::Ref(boolean)).with_default(),
                    FieldShape::new("cache", "cache", TypeShape::Ref(boolean)).skipped(),
                    FieldShape::new(
                        "optional",
                        "optional",
                        TypeShape::option(TypeShape::Ref(boolean)),
                    ),
                ],
            ),
        )
        .unwrap();
    builder
        .define(
            outer,
            TypeShape::record(
                "Outer",
                [FieldShape::new("inner", "inner", TypeShape::Ref(inner))],
            ),
        )
        .unwrap();
    (builder.finish().unwrap(), outer, inner)
}

fn input(inner: Vec<(RawValue, RawValue)>) -> RawValue {
    RawValue::Map(vec![(
        RawValue::String("inner".to_owned()),
        RawValue::Map(inner),
    )])
}

#[test]
fn missing_and_skipped_fields_request_the_exact_nested_record_occurrence() {
    let (graph, root, record) = graph();
    let defaults = Defaults {
        record,
        calls: RefCell::new(vec![]),
        wrong_type: false,
    };
    let access = DecodeShapeAccess::new(&graph, &defaults);
    let value = decode_with_shape_ref(&input(vec![]), ShapeRef::Id(root), &access).unwrap();
    assert_eq!(*defaults.calls.borrow(), [0, 1]);
    assert_eq!(
        value,
        Value::Record(BTreeMap::from([(
            "inner".to_owned(),
            Value::Record(BTreeMap::from([
                ("wire_flag".to_owned(), Value::Bool(true)),
                ("cache".to_owned(), Value::Bool(false)),
                ("optional".to_owned(), Value::Option(None)),
            ]))
        )]))
    );
    defaults.calls.borrow_mut().clear();
    let value = decode_with_shape_ref(
        &input(vec![
            (
                RawValue::String("wire_flag".to_owned()),
                RawValue::Bool(false),
            ),
            (
                RawValue::String("cache".to_owned()),
                RawValue::String("wire cache is ignored".to_owned()),
            ),
        ]),
        ShapeRef::Id(root),
        &access,
    )
    .unwrap();
    assert_eq!(*defaults.calls.borrow(), [1]);
    assert_eq!(
        value.as_record().unwrap()["inner"].as_record().unwrap()["wire_flag"],
        Value::Bool(false)
    );
}

#[test]
fn missing_provider_and_wrong_producer_results_are_explicit_errors() {
    let (graph, root, record) = graph();
    let missing = decode_with_shape_ref(&input(vec![]), ShapeRef::Id(root), &graph).unwrap_err();
    assert_eq!(missing.kind(), &DataErrorKind::MissingField);
    assert_eq!(missing.path().to_string(), "$.inner.wire_flag");
    assert!(missing.message().contains("admitted default producer"));
    let defaults = Defaults {
        record,
        calls: RefCell::new(vec![]),
        wrong_type: true,
    };
    let access = DecodeShapeAccess::new(&graph, &defaults);
    let wrong = decode_with_shape_ref(&input(vec![]), ShapeRef::Id(root), &access).unwrap_err();
    assert_eq!(wrong.kind(), &DataErrorKind::InvalidType);
    assert_eq!(wrong.path().to_string(), "$.inner.wire_flag");
    assert_eq!(*defaults.calls.borrow(), [0]);
    let required =
        decode_with_shape_ref(&RawValue::Map(vec![]), ShapeRef::Id(root), &access).unwrap_err();
    assert_eq!(required.kind(), &DataErrorKind::MissingField);
    assert_eq!(*defaults.calls.borrow(), [0]);
}
