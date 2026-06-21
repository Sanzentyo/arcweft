#![forbid(unsafe_code)]

use arcweft_data::{
    CodecRegistry, DataError, DataErrorKind, DecodeOptions, EncodeOptions, Result, TypeShape, Value,
};
use http::HeaderMap;
use http::header::{ACCEPT, CONTENT_TYPE};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedBody {
    pub content_type: String,
    pub body: Vec<u8>,
}

pub fn decode_request_body(
    headers: &HeaderMap,
    body: &[u8],
    shape: &TypeShape,
    registry: &CodecRegistry,
) -> Result<Value> {
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| DataError::new(DataErrorKind::UnsupportedFormat, "missing Content-Type"))?;
    let codec = registry.by_media_type(content_type)?;
    codec.decode_value(body, shape, &DecodeOptions::default())
}

pub fn encode_response_body(
    headers: &HeaderMap,
    value: &Value,
    shape: &TypeShape,
    registry: &CodecRegistry,
) -> Result<EncodedBody> {
    let accept = headers
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/json");
    let media_type = negotiate_accept(accept, registry)?;
    let codec = registry.by_media_type(&media_type)?;
    let body = codec.encode_value(value, shape, &EncodeOptions::default())?;
    Ok(EncodedBody {
        content_type: media_type,
        body,
    })
}

fn negotiate_accept(accept: &str, registry: &CodecRegistry) -> Result<String> {
    accept
        .split(',')
        .map(|part| part.split(';').next().unwrap_or(part).trim())
        .find_map(|media_type| {
            registry
                .by_media_type(media_type)
                .ok()
                .map(|_| media_type.to_owned())
        })
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::UnsupportedFormat,
                format!("no registered codec satisfies Accept: {accept}"),
            )
        })
}
