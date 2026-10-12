use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use arcweft_data::{DataErrorKind, DecodeLimits};
use arcweft_save::{
    SaveDecodeOptions, SaveEnvelope, SaveSchemaId, TYPED_JSON_CODEC_ID,
    decode_strict_typed_json_save, encode_typed_json_save,
};
use serde::{Deserialize, Serialize};

static TYPED_CONSTRUCTIONS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
struct ConstructionProbe;

impl<'de> Deserialize<'de> for ConstructionProbe {
    fn deserialize<D: serde::Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
        TYPED_CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
        Err(serde::de::Error::custom("typed construction entered"))
    }
}

fn schema() -> SaveSchemaId {
    SaveSchemaId::new("arcweft.test.preconstruction")
}

fn envelope(payload: &[u8]) -> Vec<u8> {
    SaveEnvelope::new(schema(), 1, TYPED_JSON_CODEC_ID, payload.to_vec())
        .encode_bytes()
        .unwrap()
}

#[test]
fn complete_json_resource_limits_reject_before_any_typed_construction() {
    let payload = br#"[{"payload":["captured"]}]"#;
    let bytes = envelope(payload);
    let exact = DecodeLimits {
        max_input_len: payload.len(),
        max_depth: 3,
        max_nodes: 5,
        max_sequence_len: 1,
        max_map_len: 1,
        max_collection_items: 3,
        max_string_len: 8,
        ..DecodeLimits::default()
    };
    for limits in [
        DecodeLimits {
            max_input_len: payload.len() - 1,
            ..exact
        },
        DecodeLimits {
            max_depth: 2,
            ..exact
        },
        DecodeLimits {
            max_nodes: 4,
            ..exact
        },
        DecodeLimits {
            max_sequence_len: 0,
            ..exact
        },
        DecodeLimits {
            max_map_len: 0,
            ..exact
        },
        DecodeLimits {
            max_collection_items: 2,
            ..exact
        },
        DecodeLimits {
            max_string_len: 7,
            ..exact
        },
    ] {
        let mut options = SaveDecodeOptions::default();
        options.codec.limits = limits;
        TYPED_CONSTRUCTIONS.store(0, Ordering::SeqCst);
        let error =
            decode_strict_typed_json_save::<ConstructionProbe>(&bytes, &schema(), 1, &options)
                .unwrap_err();
        assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
        assert_eq!(TYPED_CONSTRUCTIONS.load(Ordering::SeqCst), 0);
    }
    let mut options = SaveDecodeOptions::default();
    options.codec.limits = exact;
    TYPED_CONSTRUCTIONS.store(0, Ordering::SeqCst);
    let error = decode_strict_typed_json_save::<ConstructionProbe>(&bytes, &schema(), 1, &options)
        .unwrap_err();
    assert!(error.to_string().contains("typed construction entered"));
    assert_eq!(TYPED_CONSTRUCTIONS.load(Ordering::SeqCst), 1);
}

#[test]
fn strict_json_rejects_nested_escaped_duplicate_map_keys() {
    #[derive(Debug, Deserialize)]
    struct Payload {
        #[serde(rename = "values")]
        _values: BTreeMap<String, u8>,
    }
    let bytes = envelope(br#"{"values":{"a":1,"\u0061":2}}"#);
    let error = decode_strict_typed_json_save::<Payload>(
        &bytes,
        &schema(),
        1,
        &SaveDecodeOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &DataErrorKind::DuplicateField);
    assert!(error.to_string().contains(".values.a"));
}

#[test]
fn typed_preflight_preserves_wide_numbers_and_runtime_float_bits() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct Payload {
        signed: i128,
        unsigned: u128,
        negative_zero: f64,
        values: Vec<arcweft_core::value::AwbcRuntimeValueSnapshot>,
    }
    let payload = Payload {
        signed: i128::MIN,
        unsigned: u128::MAX,
        negative_zero: -0.0,
        values: vec![
            arcweft_core::value::AwbcRuntimeValueSnapshot::F32(0x7fc0_0042),
            arcweft_core::value::AwbcRuntimeValueSnapshot::F64(0xfff8_0000_0000_0042),
        ],
    };
    let bytes = encode_typed_json_save(&payload, schema(), 1).unwrap();
    let decoded = decode_strict_typed_json_save::<Payload>(
        &bytes,
        &schema(),
        1,
        &SaveDecodeOptions::default(),
    )
    .unwrap();
    assert_eq!(decoded, payload);
    assert_eq!(
        decoded.negative_zero.to_bits(),
        payload.negative_zero.to_bits()
    );
    assert_eq!(
        encode_typed_json_save(&decoded, schema(), 1).unwrap(),
        bytes
    );
}
