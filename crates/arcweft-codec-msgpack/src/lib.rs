#![forbid(unsafe_code)]

use std::io::Cursor;

use arcweft_data::{
    Codec, DataError, DataErrorKind, DecodeOptions, EncodeOptions, FormatId, RawValue, Result,
    TypeShape, Value, decode_with_shape, encode_with_shape,
};
use rmpv::Value as MessagePackValue;

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
        shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let raw = encode_with_shape(value, shape)?;
        let message_pack = raw_to_message_pack(raw)?;
        let mut out = Vec::new();
        rmpv::encode::write_value(&mut out, &message_pack)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        Ok(out)
    }

    fn decode_value(
        &self,
        input: &[u8],
        shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        if input.len() > options.limits.max_input_len {
            return Err(DataError::limit(format!(
                "input length {} exceeds {}",
                input.len(),
                options.limits.max_input_len
            )));
        }
        let mut cursor = Cursor::new(input);
        let message_pack = rmpv::decode::read_value(&mut cursor)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        if usize::try_from(cursor.position()).ok() != Some(input.len()) {
            return Err(DataError::new(
                DataErrorKind::TrailingData,
                "trailing MessagePack bytes",
            ));
        }
        let raw = message_pack_to_raw(message_pack)?;
        let value = decode_with_shape(&raw, shape)?;
        options.limits.validate(&value)?;
        Ok(value)
    }
}

fn raw_to_message_pack(raw: RawValue) -> Result<MessagePackValue> {
    Ok(match raw {
        RawValue::Null => MessagePackValue::Nil,
        RawValue::Bool(value) => MessagePackValue::Boolean(value),
        RawValue::Signed(value) => i64::try_from(value)
            .map(MessagePackValue::from)
            .map_err(|_| integer_range_error("signed integer"))?,
        RawValue::Unsigned(value) => u64::try_from(value)
            .map(MessagePackValue::from)
            .map_err(|_| integer_range_error("unsigned integer"))?,
        RawValue::F32(value) => MessagePackValue::F32(value),
        RawValue::F64(value) => MessagePackValue::F64(value),
        RawValue::String(value) => MessagePackValue::String(value.into()),
        RawValue::Bytes(value) => MessagePackValue::Binary(value),
        RawValue::Seq(values) => MessagePackValue::Array(
            values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    raw_to_message_pack(value).map_err(|error| error.at_index(index))
                })
                .collect::<Result<Vec<_>>>()?,
        ),
        RawValue::Map(entries) => MessagePackValue::Map(
            entries
                .into_iter()
                .map(|(key, value)| {
                    let key = raw_to_message_pack(key)?;
                    let value = raw_to_message_pack(value)?;
                    Ok((key, value))
                })
                .collect::<Result<Vec<_>>>()?,
        ),
    })
}

fn message_pack_to_raw(value: MessagePackValue) -> Result<RawValue> {
    Ok(match value {
        MessagePackValue::Nil => RawValue::Null,
        MessagePackValue::Boolean(value) => RawValue::Bool(value),
        MessagePackValue::Integer(value) => {
            if let Some(value) = value.as_i64() {
                RawValue::Signed(i128::from(value))
            } else if let Some(value) = value.as_u64() {
                RawValue::Unsigned(u128::from(value))
            } else {
                return Err(integer_range_error("MessagePack integer"));
            }
        }
        MessagePackValue::F32(value) => RawValue::F32(value),
        MessagePackValue::F64(value) => RawValue::F64(value),
        MessagePackValue::String(value) => {
            let Some(value) = value.into_str() else {
                return Err(DataError::new(
                    DataErrorKind::InvalidEncoding,
                    "MessagePack string is not valid UTF-8",
                ));
            };
            RawValue::String(value)
        }
        MessagePackValue::Binary(value) => RawValue::Bytes(value),
        MessagePackValue::Array(values) => RawValue::Seq(
            values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    message_pack_to_raw(value).map_err(|error| error.at_index(index))
                })
                .collect::<Result<Vec<_>>>()?,
        ),
        MessagePackValue::Map(entries) => RawValue::Map(
            entries
                .into_iter()
                .map(|(key, value)| {
                    let key = message_pack_to_raw(key)?;
                    let value = message_pack_to_raw(value)?;
                    Ok((key, value))
                })
                .collect::<Result<Vec<_>>>()?,
        ),
        MessagePackValue::Ext(_, _) => {
            return Err(DataError::unsupported(
                "MessagePack extension values are not supported by Arcweft data",
            ));
        }
    })
}

fn integer_range_error(label: &str) -> DataError {
    DataError::new(
        DataErrorKind::NumberOutOfRange,
        format!("{label} is outside Arcweft MessagePack integer range"),
    )
}
