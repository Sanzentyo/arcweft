#![forbid(unsafe_code)]

use arcweft_data::{Codec, DecodeOptions, EncodeOptions, FormatId, Result, TypeShape, Value};

#[derive(Clone, Copy, Debug, Default)]
pub struct MessagePackCodec;

impl Codec for MessagePackCodec {
    fn id(&self) -> FormatId {
        FormatId::new("msgpack")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/msgpack"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["mpk"]
    }

    fn encode_value(
        &self,
        value: &Value,
        _shape: &TypeShape,
        options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let json = arcweft_codec_json::to_json_value(value, options.bytes_format)?;
        rmp_serde::to_vec_named(&json).map_err(|error| {
            arcweft_data::DataError::new(
                arcweft_data::DataErrorKind::InvalidEncoding,
                error.to_string(),
            )
        })
    }

    fn decode_value(
        &self,
        input: &[u8],
        _shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let json = rmp_serde::from_slice::<serde_json::Value>(input).map_err(|error| {
            arcweft_data::DataError::new(
                arcweft_data::DataErrorKind::InvalidEncoding,
                error.to_string(),
            )
        })?;
        let value = arcweft_codec_json::from_json_value(&json)?;
        options.limits.validate(&value)?;
        Ok(value)
    }
}
