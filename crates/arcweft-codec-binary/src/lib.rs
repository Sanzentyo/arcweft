#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use arcweft_data::{
    Bytes, Codec, DataError, DataErrorKind, DecodeBudget, DecodeOptions, EncodeOptions, FormatId,
    Number, Result, TypeShape, Value,
};

const MAGIC: &[u8; 5] = b"AWBN1";

#[derive(Clone, Copy, Debug, Default)]
pub struct ArcweftBinaryCodec;

impl Codec for ArcweftBinaryCodec {
    fn id(&self) -> FormatId {
        FormatId::new("awbin")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/vnd.arcweft.awbin"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["awbin"]
    }

    fn encode_value(
        &self,
        value: &Value,
        _shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        let mut out = MAGIC.to_vec();
        write_value(&mut out, value)?;
        Ok(out)
    }

    fn decode_value(
        &self,
        input: &[u8],
        _shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        let mut reader = Reader::new(input, &options.limits)?;
        reader.expect_magic()?;
        let value = reader.read_value()?;
        if !options.limits.allow_trailing_data && !reader.is_eof() {
            return Err(DataError::new(
                DataErrorKind::TrailingData,
                "binary payload has trailing data",
            ));
        }
        options.limits.validate(&value)?;
        Ok(value)
    }
}

/// Explicit interop boundary for projects that must read external bincode.
///
/// The implementation is intentionally feature-gated so Arcweft's primary
/// binary format is not tied to bincode's API or release cadence.
#[derive(Clone, Copy, Debug, Default)]
pub struct BincodeCompatCodec;

impl Codec for BincodeCompatCodec {
    fn id(&self) -> FormatId {
        FormatId::new("bincode-compat")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/x-bincode"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["bin"]
    }

    fn encode_value(
        &self,
        value: &Value,
        _shape: &TypeShape,
        options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        #[cfg(feature = "bincode-legacy")]
        {
            let json = arcweft_codec_json::to_json_value(value, options.bytes_format)?;
            bincode::serde::encode_to_vec(&json, bincode::config::standard())
                .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
        }
        #[cfg(not(feature = "bincode-legacy"))]
        {
            let _ = (value, options);
            Err(DataError::unsupported(
                "bincode legacy support is disabled; enable feature `bincode-legacy` only for explicit interop",
            ))
        }
    }

    fn decode_value(
        &self,
        input: &[u8],
        _shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value> {
        #[cfg(feature = "bincode-legacy")]
        {
            let (json, consumed): (serde_json::Value, usize) =
                bincode::serde::decode_from_slice(input, bincode::config::standard()).map_err(
                    |error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()),
                )?;
            if !options.limits.allow_trailing_data && consumed != input.len() {
                return Err(DataError::new(
                    DataErrorKind::TrailingData,
                    "bincode payload has trailing data",
                ));
            }
            let value = arcweft_codec_json::from_json_value(&json)?;
            options.limits.validate(&value)?;
            Ok(value)
        }
        #[cfg(not(feature = "bincode-legacy"))]
        {
            let _ = (input, options);
            Err(DataError::unsupported(
                "bincode legacy support is disabled; enable feature `bincode-legacy` only for explicit interop",
            ))
        }
    }
}

fn write_value(out: &mut Vec<u8>, value: &Value) -> Result<()> {
    match value {
        Value::Unit => out.push(0),
        Value::Bool(false) => out.push(1),
        Value::Bool(true) => out.push(2),
        Value::Number(Number::I(value)) => {
            out.push(3);
            out.extend_from_slice(&value.to_le_bytes());
        }
        Value::Number(Number::U(value)) => {
            out.push(4);
            out.extend_from_slice(&value.to_le_bytes());
        }
        Value::Number(Number::F32(value)) => {
            out.push(5);
            out.extend_from_slice(&value.to_le_bytes());
        }
        Value::Number(Number::F64(value)) => {
            out.push(6);
            out.extend_from_slice(&value.to_le_bytes());
        }
        Value::String(value) => {
            out.push(7);
            write_bytes(out, value.as_bytes())?;
        }
        Value::Char(value) => {
            out.push(8);
            write_bytes(out, value.to_string().as_bytes())?;
        }
        Value::Bytes(value) => {
            out.push(9);
            write_bytes(out, value.as_slice())?;
        }
        Value::Seq(values) => {
            out.push(10);
            write_len(out, values.len())?;
            values
                .iter()
                .try_for_each(|value| write_value(out, value))?;
        }
        Value::Map(values) => {
            out.push(11);
            write_map(out, values)?;
        }
        Value::Record(values) => {
            out.push(12);
            write_map(out, values)?;
        }
        Value::Enum { variant, payload } => {
            out.push(13);
            write_bytes(out, variant.as_bytes())?;
            match payload {
                Some(payload) => {
                    out.push(1);
                    write_value(out, payload)?;
                }
                None => out.push(0),
            }
        }
    }
    Ok(())
}

fn write_map(out: &mut Vec<u8>, values: &BTreeMap<String, Value>) -> Result<()> {
    write_len(out, values.len())?;
    values.iter().try_for_each(|(key, value)| {
        write_bytes(out, key.as_bytes())?;
        write_value(out, value)
    })
}

fn write_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    write_len(out, bytes.len())?;
    out.extend_from_slice(bytes);
    Ok(())
}

fn write_len(out: &mut Vec<u8>, len: usize) -> Result<()> {
    let len = u64::try_from(len)
        .map_err(|_| DataError::new(DataErrorKind::NumberOutOfRange, "length does not fit u64"))?;
    out.extend_from_slice(&len.to_le_bytes());
    Ok(())
}

struct Reader<'a> {
    input: &'a [u8],
    offset: usize,
    budget: DecodeBudget<'a>,
}

impl<'a> Reader<'a> {
    fn new(input: &'a [u8], limits: &'a arcweft_data::DecodeLimits) -> Result<Self> {
        Ok(Self {
            input,
            offset: 0,
            budget: DecodeBudget::new(input.len(), limits)?,
        })
    }

    fn expect_magic(&mut self) -> Result<()> {
        let magic = self.read_exact(MAGIC.len())?;
        if magic == MAGIC {
            Ok(())
        } else {
            Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "bad Arcweft binary magic",
            ))
        }
    }

    const fn is_eof(&self) -> bool {
        self.offset == self.input.len()
    }

    fn read_value(&mut self) -> Result<Value> {
        self.budget.enter_node()?;
        let value = self.read_value_body();
        self.budget.exit_node();
        value
    }

    fn read_value_body(&mut self) -> Result<Value> {
        match self.read_u8()? {
            0 => Ok(Value::Unit),
            1 => Ok(Value::Bool(false)),
            2 => Ok(Value::Bool(true)),
            3 => Ok(Value::Number(Number::I(i128::from_le_bytes(
                self.read_array()?,
            )))),
            4 => Ok(Value::Number(Number::U(u128::from_le_bytes(
                self.read_array()?,
            )))),
            5 => Ok(Value::Number(Number::F32(f32::from_le_bytes(
                self.read_array()?,
            )))),
            6 => Ok(Value::Number(Number::F64(f64::from_le_bytes(
                self.read_array()?,
            )))),
            7 => String::from_utf8(self.read_string_bytes()?.to_vec())
                .map(Value::String)
                .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string())),
            8 => {
                let string =
                    String::from_utf8(self.read_string_bytes()?.to_vec()).map_err(|error| {
                        DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                    })?;
                let mut chars = string.chars();
                match (chars.next(), chars.next()) {
                    (Some(ch), None) => Ok(Value::Char(ch)),
                    _ => Err(DataError::new(
                        DataErrorKind::InvalidEncoding,
                        "invalid char payload",
                    )),
                }
            }
            9 => Ok(Value::Bytes(Bytes::new(self.read_blob_bytes()?.to_vec()))),
            10 => {
                let len = self.read_len()?;
                self.budget.sequence_len(len)?;
                let mut values = Vec::with_capacity(len);
                for index in 0..len {
                    values.push(self.read_value().map_err(|err| err.at_index(index))?);
                }
                Ok(Value::Seq(values))
            }
            11 => self.read_map().map(Value::Map),
            12 => self.read_map().map(Value::Record),
            13 => {
                let variant =
                    String::from_utf8(self.read_string_bytes()?.to_vec()).map_err(|error| {
                        DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
                    })?;
                let has_payload = self.read_u8()?;
                let payload = match has_payload {
                    0 => None,
                    1 => Some(Box::new(
                        self.read_value()
                            .map_err(|err| err.at_variant(variant.clone()))?,
                    )),
                    flag => {
                        return Err(DataError::new(
                            DataErrorKind::InvalidEncoding,
                            format!("invalid enum payload flag {flag}"),
                        )
                        .at_variant(variant));
                    }
                };
                Ok(Value::Enum { variant, payload })
            }
            tag => Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                format!("unknown binary tag {tag}"),
            )),
        }
    }

    fn read_map(&mut self) -> Result<BTreeMap<String, Value>> {
        let len = self.read_len()?;
        self.budget.map_len(len)?;
        let mut out = BTreeMap::new();
        for _ in 0..len {
            let key = String::from_utf8(self.read_string_bytes()?.to_vec()).map_err(|error| {
                DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
            })?;
            let value = self.read_value().map_err(|err| err.at_field(key.clone()))?;
            if out.insert(key.clone(), value).is_some() {
                return Err(DataError::new(
                    DataErrorKind::DuplicateField,
                    format!("duplicate binary map key `{key}`"),
                )
                .at_field(key));
            }
        }
        Ok(out)
    }

    fn read_len(&mut self) -> Result<usize> {
        let raw = u64::from_le_bytes(self.read_array()?);
        usize::try_from(raw).map_err(|_| {
            DataError::new(DataErrorKind::NumberOutOfRange, "length does not fit usize")
        })
    }

    fn read_string_bytes(&mut self) -> Result<&'a [u8]> {
        let len = self.read_len()?;
        self.budget.string_len(len)?;
        self.read_exact(len)
    }

    fn read_blob_bytes(&mut self) -> Result<&'a [u8]> {
        let len = self.read_len()?;
        self.budget.bytes_len(len)?;
        self.read_exact(len)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.read_exact(N)?;
        let mut out = [0; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn read_u8(&mut self) -> Result<u8> {
        self.read_exact(1).map(|bytes| bytes[0])
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| DataError::new(DataErrorKind::InvalidEncoding, "offset overflow"))?;
        if end > self.input.len() {
            return Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "unexpected end of binary payload",
            ));
        }
        let bytes = &self.input[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }
}
