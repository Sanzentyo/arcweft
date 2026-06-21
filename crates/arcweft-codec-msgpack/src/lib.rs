#![forbid(unsafe_code)]

use std::io::Cursor;

use arcweft_data::{
    Codec, DataError, DataErrorKind, DecodeBudget, DecodeOptions, EncodeOptions, FormatId,
    RawValue, Result, TypeShape, Value, decode_with_shape, encode_with_shape,
};
use rmp::Marker;
use rmp::decode::{RmpRead, read_marker};
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
        let mut budget = DecodeBudget::new(input.len(), &options.limits)?;
        let mut cursor = Cursor::new(input);
        let raw = read_message_pack_raw(&mut cursor, &mut budget)?;
        if usize::try_from(cursor.position()).ok() != Some(input.len()) {
            return Err(DataError::new(
                DataErrorKind::TrailingData,
                "trailing MessagePack bytes",
            ));
        }
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

fn read_message_pack_raw<R: RmpRead>(
    reader: &mut R,
    budget: &mut DecodeBudget<'_>,
) -> Result<RawValue> {
    budget.enter_node()?;
    let result = read_message_pack_raw_inner(reader, budget);
    budget.exit_node();
    result
}

fn read_message_pack_raw_inner<R: RmpRead>(
    reader: &mut R,
    budget: &mut DecodeBudget<'_>,
) -> Result<RawValue> {
    let marker = read_marker(reader).map_err(msgpack_error)?;
    Ok(match marker {
        Marker::FixPos(value) => RawValue::Unsigned(u128::from(value)),
        Marker::FixNeg(value) => RawValue::Signed(i128::from(value)),
        Marker::Null => RawValue::Null,
        Marker::False => RawValue::Bool(false),
        Marker::True => RawValue::Bool(true),
        Marker::U8 => RawValue::Unsigned(u128::from(reader.read_data_u8().map_err(msgpack_error)?)),
        Marker::U16 => {
            RawValue::Unsigned(u128::from(reader.read_data_u16().map_err(msgpack_error)?))
        }
        Marker::U32 => {
            RawValue::Unsigned(u128::from(reader.read_data_u32().map_err(msgpack_error)?))
        }
        Marker::U64 => {
            RawValue::Unsigned(u128::from(reader.read_data_u64().map_err(msgpack_error)?))
        }
        Marker::I8 => RawValue::Signed(i128::from(reader.read_data_i8().map_err(msgpack_error)?)),
        Marker::I16 => RawValue::Signed(i128::from(reader.read_data_i16().map_err(msgpack_error)?)),
        Marker::I32 => RawValue::Signed(i128::from(reader.read_data_i32().map_err(msgpack_error)?)),
        Marker::I64 => RawValue::Signed(i128::from(reader.read_data_i64().map_err(msgpack_error)?)),
        Marker::F32 => RawValue::F32(reader.read_data_f32().map_err(msgpack_error)?),
        Marker::F64 => RawValue::F64(reader.read_data_f64().map_err(msgpack_error)?),
        Marker::FixStr(len) => read_message_pack_string(reader, budget, usize::from(len))?,
        Marker::Str8 => {
            let len = usize::from(reader.read_data_u8().map_err(msgpack_error)?);
            read_message_pack_string(reader, budget, len)?
        }
        Marker::Str16 => {
            let len = usize::from(reader.read_data_u16().map_err(msgpack_error)?);
            read_message_pack_string(reader, budget, len)?
        }
        Marker::Str32 => {
            let len = usize::try_from(reader.read_data_u32().map_err(msgpack_error)?)
                .map_err(|_| integer_range_error("MessagePack string length"))?;
            read_message_pack_string(reader, budget, len)?
        }
        Marker::Bin8 => {
            let len = usize::from(reader.read_data_u8().map_err(msgpack_error)?);
            read_message_pack_bytes(reader, budget, len)?
        }
        Marker::Bin16 => {
            let len = usize::from(reader.read_data_u16().map_err(msgpack_error)?);
            read_message_pack_bytes(reader, budget, len)?
        }
        Marker::Bin32 => {
            let len = usize::try_from(reader.read_data_u32().map_err(msgpack_error)?)
                .map_err(|_| integer_range_error("MessagePack binary length"))?;
            read_message_pack_bytes(reader, budget, len)?
        }
        Marker::FixArray(len) => read_message_pack_seq(reader, budget, usize::from(len))?,
        Marker::Array16 => {
            let len = usize::from(reader.read_data_u16().map_err(msgpack_error)?);
            read_message_pack_seq(reader, budget, len)?
        }
        Marker::Array32 => {
            let len = usize::try_from(reader.read_data_u32().map_err(msgpack_error)?)
                .map_err(|_| integer_range_error("MessagePack array length"))?;
            read_message_pack_seq(reader, budget, len)?
        }
        Marker::FixMap(len) => read_message_pack_map(reader, budget, usize::from(len))?,
        Marker::Map16 => {
            let len = usize::from(reader.read_data_u16().map_err(msgpack_error)?);
            read_message_pack_map(reader, budget, len)?
        }
        Marker::Map32 => {
            let len = usize::try_from(reader.read_data_u32().map_err(msgpack_error)?)
                .map_err(|_| integer_range_error("MessagePack map length"))?;
            read_message_pack_map(reader, budget, len)?
        }
        Marker::Reserved => {
            return Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "reserved MessagePack marker",
            ));
        }
        Marker::Ext8
        | Marker::Ext16
        | Marker::Ext32
        | Marker::FixExt1
        | Marker::FixExt2
        | Marker::FixExt4
        | Marker::FixExt8
        | Marker::FixExt16 => {
            return Err(DataError::unsupported(
                "MessagePack extension values are not supported by Arcweft data",
            ));
        }
    })
}

fn read_message_pack_string<R: RmpRead>(
    reader: &mut R,
    budget: &DecodeBudget<'_>,
    len: usize,
) -> Result<RawValue> {
    budget.string_len(len)?;
    let bytes = read_message_pack_bytes_vec(reader, len)?;
    String::from_utf8(bytes)
        .map(RawValue::String)
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
}

fn read_message_pack_bytes<R: RmpRead>(
    reader: &mut R,
    budget: &DecodeBudget<'_>,
    len: usize,
) -> Result<RawValue> {
    budget.bytes_len(len)?;
    read_message_pack_bytes_vec(reader, len).map(RawValue::Bytes)
}

fn read_message_pack_bytes_vec<R: RmpRead>(reader: &mut R, len: usize) -> Result<Vec<u8>> {
    let mut bytes = vec![0; len];
    reader
        .read_exact_buf(&mut bytes)
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
    Ok(bytes)
}

fn read_message_pack_seq<R: RmpRead>(
    reader: &mut R,
    budget: &mut DecodeBudget<'_>,
    len: usize,
) -> Result<RawValue> {
    budget.sequence_len(len)?;
    (0..len)
        .map(|index| read_message_pack_raw(reader, budget).map_err(|error| error.at_index(index)))
        .collect::<Result<Vec<_>>>()
        .map(RawValue::Seq)
}

fn read_message_pack_map<R: RmpRead>(
    reader: &mut R,
    budget: &mut DecodeBudget<'_>,
    len: usize,
) -> Result<RawValue> {
    budget.map_len(len)?;
    (0..len)
        .map(|_| {
            let key = read_message_pack_raw(reader, budget)?;
            let value = read_message_pack_raw(reader, budget)?;
            Ok((key, value))
        })
        .collect::<Result<Vec<_>>>()
        .map(RawValue::Map)
}

fn integer_range_error(label: &str) -> DataError {
    DataError::new(
        DataErrorKind::NumberOutOfRange,
        format!("{label} is outside Arcweft MessagePack integer range"),
    )
}

fn msgpack_error(error: impl std::fmt::Debug) -> DataError {
    DataError::new(DataErrorKind::InvalidEncoding, format!("{error:?}"))
}
