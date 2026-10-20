use arcweft_data::{
    Codec, DataErrorKind, DecodeOptions, EmptyShapeAccess, EncodeOptions, FieldShape, ShapeRef,
    TypeShape, Value,
};
use std::collections::BTreeMap;

fn assert_bounded(codec: &dyn Codec, value: &Value, shape: &TypeShape) {
    for pretty in [false, true] {
        let options = EncodeOptions {
            pretty,
            ..EncodeOptions::default()
        };
        let bytes = options
            .encode_value(codec, value, ShapeRef::inline(shape), &EmptyShapeAccess)
            .unwrap();
        assert!(!bytes.is_empty());
        for maximum in [0, bytes.len() - 1] {
            let short = EncodeOptions {
                max_output_len: maximum,
                ..options.clone()
            };
            let error = short
                .encode_value(codec, value, ShapeRef::inline(shape), &EmptyShapeAccess)
                .unwrap_err();
            assert_eq!(
                error.kind(),
                &DataErrorKind::LimitExceeded,
                "{} limit {maximum}: {error}",
                codec.id().as_str()
            );
        }
        let exact = EncodeOptions {
            max_output_len: bytes.len(),
            ..options
        };
        let accepted = exact
            .encode_value(codec, value, ShapeRef::inline(shape), &EmptyShapeAccess)
            .unwrap();
        assert_eq!(accepted.len(), bytes.len());
        let decoded = codec
            .decode_value(
                &accepted,
                ShapeRef::inline(shape),
                &EmptyShapeAccess,
                &DecodeOptions::default(),
            )
            .unwrap();
        assert_eq!(&decoded, value);
    }
}

#[test]
fn encoded_output_limits_reject_truncation_and_accept_the_exact_boundary() {
    let shape = TypeShape::record(
        "Bounded",
        [FieldShape::new("text", "text", TypeShape::String)],
    );
    let value = Value::Record(BTreeMap::from([(
        "text".to_owned(),
        Value::String("\0\"\\\n蟹".to_owned()),
    )]));

    assert_bounded(&arcweft_codec_yaml::YamlCodec, &value, &shape);
}
