#![forbid(unsafe_code)]

use std::io::Cursor;

use arcweft_data::{
    Codec, DataError, DataErrorKind, DecodeBudget, DecodeOptions, EncodeOptions, FormatId,
    RawValue, Result, TypeShape, Value, decode_with_shape, encode_with_shape,
};
use ciborium::Value as CborValue;
use ciborium::value::Integer as CborInteger;
use ciborium_io::Read as CborRead;
use ciborium_ll::{Decoder as CborDecoder, Header as CborHeader};

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
        let mut budget = DecodeBudget::new(input.len(), &options.limits)?;
        let mut decoder = CborDecoder::from(Cursor::new(input));
        let raw = read_cbor_raw(&mut decoder, &mut budget)?;
        if decoder.offset() != input.len() {
            return Err(DataError::new(
                DataErrorKind::TrailingData,
                "trailing CBOR bytes",
            ));
        }
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

fn read_cbor_raw<R: CborRead>(
    decoder: &mut CborDecoder<R>,
    budget: &mut DecodeBudget<'_>,
) -> Result<RawValue>
where
    R::Error: std::fmt::Debug,
{
    budget.enter_node()?;
    let result = read_cbor_raw_inner(decoder, budget);
    budget.exit_node();
    result
}

fn read_cbor_raw_inner<R: CborRead>(
    decoder: &mut CborDecoder<R>,
    budget: &mut DecodeBudget<'_>,
) -> Result<RawValue>
where
    R::Error: std::fmt::Debug,
{
    Ok(match decoder.pull().map_err(cbor_error)? {
        CborHeader::Positive(value) => RawValue::Unsigned(u128::from(value)),
        CborHeader::Negative(value) => RawValue::Signed(i128::from(value) ^ !0),
        CborHeader::Float(value) => RawValue::F64(value),
        CborHeader::Simple(20) => RawValue::Bool(false),
        CborHeader::Simple(21) => RawValue::Bool(true),
        CborHeader::Simple(22) => RawValue::Null,
        CborHeader::Simple(other) => {
            return Err(DataError::unsupported(format!(
                "CBOR simple value {other} is not supported by Arcweft data"
            )));
        }
        CborHeader::Bytes(len) => read_cbor_bytes(decoder, budget, len)?,
        CborHeader::Text(len) => read_cbor_text(decoder, budget, len)?,
        CborHeader::Array(len) => read_cbor_seq(decoder, budget, len)?,
        CborHeader::Map(len) => read_cbor_map(decoder, budget, len)?,
        CborHeader::Tag(_) => {
            return Err(DataError::unsupported(
                "CBOR tag values are not supported by Arcweft data",
            ));
        }
        CborHeader::Break => {
            return Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "unexpected CBOR break marker",
            ));
        }
    })
}

fn read_cbor_bytes<R: CborRead>(
    decoder: &mut CborDecoder<R>,
    budget: &DecodeBudget<'_>,
    len: Option<usize>,
) -> Result<RawValue>
where
    R::Error: std::fmt::Debug,
{
    if let Some(len) = len {
        budget.bytes_len(len)?;
        let mut bytes = vec![0; len];
        CborRead::read_exact(decoder, &mut bytes).map_err(cbor_error)?;
        Ok(RawValue::Bytes(bytes))
    } else {
        let mut out = Vec::new();
        let mut segments = decoder.bytes(None);
        let mut buffer = [0_u8; 4096];
        while let Some(mut segment) = segments.pull().map_err(cbor_error)? {
            while let Some(chunk) = segment.pull(&mut buffer).map_err(cbor_error)? {
                budget.bytes_len(out.len().saturating_add(chunk.len()))?;
                out.extend_from_slice(chunk);
            }
        }
        Ok(RawValue::Bytes(out))
    }
}

fn read_cbor_text<R: CborRead>(
    decoder: &mut CborDecoder<R>,
    budget: &DecodeBudget<'_>,
    len: Option<usize>,
) -> Result<RawValue>
where
    R::Error: std::fmt::Debug,
{
    if let Some(len) = len {
        budget.string_len(len)?;
        let mut bytes = vec![0; len];
        CborRead::read_exact(decoder, &mut bytes).map_err(cbor_error)?;
        String::from_utf8(bytes)
            .map(RawValue::String)
            .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
    } else {
        let mut out = String::new();
        let mut segments = decoder.text(None);
        let mut buffer = [0_u8; 4096];
        while let Some(mut segment) = segments.pull().map_err(cbor_error)? {
            while let Some(chunk) = segment.pull(&mut buffer).map_err(cbor_error)? {
                budget.string_len(out.len().saturating_add(chunk.len()))?;
                out.push_str(chunk);
            }
        }
        Ok(RawValue::String(out))
    }
}

fn read_cbor_seq<R: CborRead>(
    decoder: &mut CborDecoder<R>,
    budget: &mut DecodeBudget<'_>,
    len: Option<usize>,
) -> Result<RawValue>
where
    R::Error: std::fmt::Debug,
{
    let values = if let Some(len) = len {
        budget.sequence_len(len)?;
        (0..len)
            .map(|index| read_cbor_raw(decoder, budget).map_err(|error| error.at_index(index)))
            .collect::<Result<Vec<_>>>()?
    } else {
        let mut values = Vec::new();
        loop {
            let header = decoder.pull().map_err(cbor_error)?;
            if matches!(header, CborHeader::Break) {
                break;
            }
            decoder.push(header);
            budget.sequence_item(values.len().saturating_add(1))?;
            let index = values.len();
            values.push(read_cbor_raw(decoder, budget).map_err(|error| error.at_index(index))?);
        }
        values
    };
    Ok(RawValue::Seq(values))
}

fn read_cbor_map<R: CborRead>(
    decoder: &mut CborDecoder<R>,
    budget: &mut DecodeBudget<'_>,
    len: Option<usize>,
) -> Result<RawValue>
where
    R::Error: std::fmt::Debug,
{
    let entries = if let Some(len) = len {
        budget.map_len(len)?;
        (0..len)
            .map(|_| {
                let key = read_cbor_raw(decoder, budget)?;
                let value = read_cbor_raw(decoder, budget)?;
                Ok((key, value))
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        let mut entries = Vec::new();
        loop {
            let header = decoder.pull().map_err(cbor_error)?;
            if matches!(header, CborHeader::Break) {
                break;
            }
            decoder.push(header);
            budget.map_item(entries.len().saturating_add(1))?;
            let key = read_cbor_raw(decoder, budget)?;
            let value = read_cbor_raw(decoder, budget)?;
            entries.push((key, value));
        }
        entries
    };
    Ok(RawValue::Map(entries))
}

fn integer_range_error(label: &str) -> DataError {
    DataError::new(
        DataErrorKind::NumberOutOfRange,
        format!("{label} is outside Arcweft CBOR integer range"),
    )
}

fn cbor_error(error: impl std::fmt::Debug) -> DataError {
    DataError::new(DataErrorKind::InvalidEncoding, format!("{error:?}"))
}
