#![forbid(unsafe_code)]

use std::io::Cursor;

use arcweft_data::{
    Codec, DataError, DataErrorKind, DecodeOptions, EncodeOptions, FormatId, RawValue, Result,
    TypeShape, Value, decode_with_shape, encode_with_shape,
};
use ciborium::Value as CborValue;
use ciborium::value::Integer as CborInteger;

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
        shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let raw = encode_with_shape(value, shape)?;
        let cbor = raw_to_cbor(raw)?;
        let mut out = Vec::new();
        ciborium::into_writer(&cbor, &mut out)
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
        let cbor = ciborium::from_reader::<CborValue, _>(&mut cursor)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
        if usize::try_from(cursor.position()).ok() != Some(input.len()) {
            return Err(DataError::new(
                DataErrorKind::TrailingData,
                "trailing CBOR bytes",
            ));
        }
        let raw = cbor_to_raw(cbor)?;
        let value = decode_with_shape(&raw, shape)?;
        options.limits.validate(&value)?;
        Ok(value)
    }
}

fn raw_to_cbor(raw: RawValue) -> Result<CborValue> {
    Ok(match raw {
        RawValue::Null => CborValue::Null,
        RawValue::Bool(value) => CborValue::Bool(value),
        RawValue::Signed(value) => CborValue::Integer(
            CborInteger::try_from(value).map_err(|_| integer_range_error("signed integer"))?,
        ),
        RawValue::Unsigned(value) => CborValue::Integer(
            CborInteger::try_from(value).map_err(|_| integer_range_error("unsigned integer"))?,
        ),
        RawValue::F32(value) => CborValue::Float(f64::from(value)),
        RawValue::F64(value) => CborValue::Float(value),
        RawValue::String(value) => CborValue::Text(value),
        RawValue::Bytes(value) => CborValue::Bytes(value),
        RawValue::Seq(values) => CborValue::Array(
            values
                .into_iter()
                .enumerate()
                .map(|(index, value)| raw_to_cbor(value).map_err(|error| error.at_index(index)))
                .collect::<Result<Vec<_>>>()?,
        ),
        RawValue::Map(entries) => CborValue::Map(
            entries
                .into_iter()
                .map(|(key, value)| {
                    let key = raw_to_cbor(key)?;
                    let value = raw_to_cbor(value)?;
                    Ok((key, value))
                })
                .collect::<Result<Vec<_>>>()?,
        ),
    })
}

fn cbor_to_raw(value: CborValue) -> Result<RawValue> {
    Ok(match value {
        CborValue::Integer(value) => {
            let signed = i128::from(value);
            if signed >= 0 {
                RawValue::Unsigned(u128::try_from(signed).expect("nonnegative i128"))
            } else {
                RawValue::Signed(signed)
            }
        }
        CborValue::Bytes(value) => RawValue::Bytes(value),
        CborValue::Float(value) => RawValue::F64(value),
        CborValue::Text(value) => RawValue::String(value),
        CborValue::Bool(value) => RawValue::Bool(value),
        CborValue::Null => RawValue::Null,
        CborValue::Array(values) => RawValue::Seq(
            values
                .into_iter()
                .enumerate()
                .map(|(index, value)| cbor_to_raw(value).map_err(|error| error.at_index(index)))
                .collect::<Result<Vec<_>>>()?,
        ),
        CborValue::Map(entries) => RawValue::Map(
            entries
                .into_iter()
                .map(|(key, value)| {
                    let key = cbor_to_raw(key)?;
                    let value = cbor_to_raw(value)?;
                    Ok((key, value))
                })
                .collect::<Result<Vec<_>>>()?,
        ),
        CborValue::Tag(_, _) => {
            return Err(DataError::unsupported(
                "CBOR tag values are not supported by Arcweft data",
            ));
        }
        _ => {
            return Err(DataError::unsupported(
                "unknown CBOR value variant is not supported by Arcweft data",
            ));
        }
    })
}

fn integer_range_error(label: &str) -> DataError {
    DataError::new(
        DataErrorKind::NumberOutOfRange,
        format!("{label} is outside Arcweft CBOR integer range"),
    )
}
