use std::collections::BTreeMap;
use std::io::Cursor;

use arcweft_codec_cbor::CborCodec;
use arcweft_data::{
    Bytes, BytesFormat, Codec, DataErrorKind, DecodeOptions, EncodeOptions, FieldShape,
    RecordPolicy, TypeShape, Value,
};
use ciborium::Value as CborValue;

fn asset_shape() -> TypeShape {
    TypeShape::Record {
        name: "Asset".to_owned(),
        fields: vec![
            FieldShape::new(
                "blob",
                "blob",
                TypeShape::Bytes {
                    format: BytesFormat::Binary,
                },
            ),
            FieldShape::new("name", "name", TypeShape::String),
        ],
        policy: RecordPolicy {
            deny_unknown_fields: true,
        },
    }
}

#[test]
fn cbor_codec_preserves_native_bytes() {
    let value = Value::Record(BTreeMap::from([
        (
            "blob".to_owned(),
            Value::Bytes(Bytes::from([0_u8, 1, 2, 255].as_slice())),
        ),
        ("name".to_owned(), Value::String("hero".to_owned())),
    ]));

    let encoded = CborCodec
        .encode_value(&value, &asset_shape(), &EncodeOptions::default())
        .expect("encode");
    let native =
        ciborium::from_reader::<CborValue, _>(Cursor::new(&encoded)).expect("native decode");
    let CborValue::Map(entries) = native else {
        panic!("expected map");
    };
    assert!(entries.iter().any(|(key, value)| {
        matches!(key, CborValue::Text(key) if key == "blob")
            && matches!(value, CborValue::Bytes(bytes) if bytes == &[0, 1, 2, 255])
    }));

    let decoded = CborCodec
        .decode_value(&encoded, &asset_shape(), &DecodeOptions::default())
        .expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn cbor_codec_rejects_trailing_bytes() {
    let mut encoded = CborCodec
        .encode_value(
            &Value::Bool(true),
            &TypeShape::Bool,
            &EncodeOptions::default(),
        )
        .expect("encode");
    encoded.push(0xf6);

    let error = CborCodec
        .decode_value(&encoded, &TypeShape::Bool, &DecodeOptions::default())
        .expect_err("trailing bytes");
    assert_eq!(error.kind(), &DataErrorKind::TrailingData);
}
