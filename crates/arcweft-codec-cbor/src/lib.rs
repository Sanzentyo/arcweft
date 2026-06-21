#![forbid(unsafe_code)]

use arcweft_data::{Codec, DecodeOptions, EncodeOptions, FormatId, Result, TypeShape, Value};

#[derive(Clone, Copy, Debug, Default)]
pub struct CborCodec;

impl Codec for CborCodec {
    fn id(&self) -> FormatId {
        FormatId::new("cbor")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/cbor"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["cbor"]
    }

    fn encode_value(
        &self,
        value: &Value,
        _shape: &TypeShape,
        options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let json = arcweft_codec_json::to_json_value(value, options.bytes_format)?;
        {
            let mut out = Vec::new();
            ciborium::into_writer(&json, &mut out).map_err(|error| {
                arcweft_data::DataError::new(
                    arcweft_data::DataErrorKind::InvalidEncoding,
                    error.to_string(),
                )
            })?;
            Ok(out)
        }
    }

    fn decode_value(
        &self,
        input: &[u8],
        _shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let json = ciborium::from_reader::<serde_json::Value, _>(std::io::Cursor::new(input))
            .map_err(|error| {
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
