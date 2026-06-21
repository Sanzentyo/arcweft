use arcweft_data::{
    Codec, CodecRegistry, DataErrorKind, DecodeLimits, DecodeOptions, EncodeOptions, FormatId,
    TypeShape, Value,
};
use arcweft_http_codec::{
    HttpCodecOptions, decode_request_body, decode_request_body_with_options, encode_response_body,
};
use http::HeaderMap;
use http::header::{ACCEPT, CONTENT_TYPE};

#[derive(Clone, Copy)]
struct StaticCodec {
    id: &'static str,
    media_types: &'static [&'static str],
}

impl Codec for StaticCodec {
    fn id(&self) -> FormatId {
        FormatId::new(self.id)
    }

    fn media_types(&self) -> &'static [&'static str] {
        self.media_types
    }

    fn encode_value(
        &self,
        _value: &Value,
        _shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> arcweft_data::Result<Vec<u8>> {
        Ok(self.id.as_bytes().to_vec())
    }

    fn decode_value(
        &self,
        input: &[u8],
        _shape: &TypeShape,
        _options: &DecodeOptions,
    ) -> arcweft_data::Result<Value> {
        Ok(Value::String(format!(
            "{}:{}",
            self.id,
            std::str::from_utf8(input).expect("test input is utf8")
        )))
    }
}

fn registry() -> CodecRegistry {
    CodecRegistry::new()
        .with(StaticCodec {
            id: "json",
            media_types: &["application/json"],
        })
        .expect("json codec registers")
        .with(StaticCodec {
            id: "yaml",
            media_types: &["application/yaml"],
        })
        .expect("yaml codec registers")
        .with(StaticCodec {
            id: "csv",
            media_types: &["text/csv"],
        })
        .expect("csv codec registers")
}

#[test]
fn decode_request_body_accepts_content_type_parameters() {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        "application/json; charset=utf-8".parse().expect("header"),
    );

    let decoded =
        decode_request_body(&headers, b"body", &TypeShape::String, &registry()).expect("decode");

    assert_eq!(decoded, Value::String("json:body".to_owned()));
}

#[test]
fn decode_request_body_rejects_invalid_content_type_parameters() {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        "application/json; charset".parse().expect("header"),
    );

    let error = decode_request_body(&headers, b"body", &TypeShape::String, &registry())
        .expect_err("invalid parameter");

    assert_eq!(error.kind(), &DataErrorKind::UnsupportedFormat);
}

#[test]
fn decode_request_body_enforces_body_limit_before_codec_decode() {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().expect("header"));
    let options = HttpCodecOptions {
        decode: DecodeOptions {
            limits: DecodeLimits {
                max_input_len: 3,
                ..DecodeLimits::default()
            },
        },
        ..HttpCodecOptions::default()
    };

    let error = decode_request_body_with_options(
        &headers,
        b"body",
        &TypeShape::String,
        &registry(),
        &options,
    )
    .expect_err("body cap");

    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn encode_response_body_uses_q_weighted_accept_order() {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        "application/json;q=0.4, text/*;q=0.9, application/*;q=0.6"
            .parse()
            .expect("header"),
    );

    let encoded = encode_response_body(
        &headers,
        &Value::String("body".to_owned()),
        &TypeShape::String,
        &registry(),
    )
    .expect("encode");

    assert_eq!(encoded.content_type, "text/csv");
    assert_eq!(encoded.body, b"csv");
}

#[test]
fn encode_response_body_prefers_more_specific_media_range_on_equal_q() {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        "application/*;q=0.8, application/yaml;q=0.8"
            .parse()
            .expect("header"),
    );

    let encoded = encode_response_body(
        &headers,
        &Value::String("body".to_owned()),
        &TypeShape::String,
        &registry(),
    )
    .expect("encode");

    assert_eq!(encoded.content_type, "application/yaml");
}

#[test]
fn encode_response_body_rejects_q_zero_only_matches() {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        "application/json;q=0, */*;q=0".parse().expect("header"),
    );

    let error = encode_response_body(
        &headers,
        &Value::String("body".to_owned()),
        &TypeShape::String,
        &registry(),
    )
    .expect_err("no accepted format");

    assert_eq!(error.kind(), &DataErrorKind::UnsupportedFormat);
}

#[test]
fn encode_response_body_rejects_invalid_q_values() {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, "application/json;q=1.1".parse().expect("header"));

    let error = encode_response_body(
        &headers,
        &Value::String("body".to_owned()),
        &TypeShape::String,
        &registry(),
    )
    .expect_err("invalid q");

    assert_eq!(error.kind(), &DataErrorKind::UnsupportedFormat);
}

#[test]
fn encode_response_body_defaults_to_json_when_accept_is_missing() {
    let encoded = encode_response_body(
        &HeaderMap::new(),
        &Value::String("body".to_owned()),
        &TypeShape::String,
        &registry(),
    )
    .expect("encode");

    assert_eq!(encoded.content_type, "application/json");
    assert_eq!(encoded.body, b"json");
}
