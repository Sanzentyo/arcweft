use std::collections::BTreeMap;
use std::convert::TryFrom;

use crate::error::{DataError, DataErrorKind, Result};
use crate::value::{Bytes, Number, Value};

/// Format-independent decoder from Arcweft's dynamic value tree.
pub trait Decode: Sized {
    fn decode(value: &Value) -> Result<Self>;
}

macro_rules! signed_int_impl {
    ($ty:ty) => {
        impl Decode for $ty {
            fn decode(value: &Value) -> Result<Self> {
                match value {
                    Value::Number(Number::I(value)) => <$ty>::try_from(*value).map_err(|_| {
                        DataError::new(
                            DataErrorKind::NumberOutOfRange,
                            format!("cannot fit {value} into {}", stringify!($ty)),
                        )
                    }),
                    Value::Number(Number::U(value)) => <$ty>::try_from(*value).map_err(|_| {
                        DataError::new(
                            DataErrorKind::NumberOutOfRange,
                            format!("cannot fit {value} into {}", stringify!($ty)),
                        )
                    }),
                    other => Err(DataError::invalid_type(stringify!($ty), other.type_name())),
                }
            }
        }
    };
}

macro_rules! unsigned_int_impl {
    ($ty:ty) => {
        impl Decode for $ty {
            fn decode(value: &Value) -> Result<Self> {
                match value {
                    Value::Number(Number::U(value)) => <$ty>::try_from(*value).map_err(|_| {
                        DataError::new(
                            DataErrorKind::NumberOutOfRange,
                            format!("cannot fit {value} into {}", stringify!($ty)),
                        )
                    }),
                    Value::Number(Number::I(value)) => <$ty>::try_from(*value).map_err(|_| {
                        DataError::new(
                            DataErrorKind::NumberOutOfRange,
                            format!("cannot fit {value} into {}", stringify!($ty)),
                        )
                    }),
                    other => Err(DataError::invalid_type(stringify!($ty), other.type_name())),
                }
            }
        }
    };
}

signed_int_impl!(i8);
signed_int_impl!(i16);
signed_int_impl!(i32);
signed_int_impl!(i64);
signed_int_impl!(i128);
signed_int_impl!(isize);
unsigned_int_impl!(u8);
unsigned_int_impl!(u16);
unsigned_int_impl!(u32);
unsigned_int_impl!(u64);
unsigned_int_impl!(u128);
unsigned_int_impl!(usize);

impl Decode for bool {
    fn decode(value: &Value) -> Result<Self> {
        match value {
            Value::Bool(value) => Ok(*value),
            other => Err(DataError::invalid_type("bool", other.type_name())),
        }
    }
}

impl Decode for f32 {
    fn decode(value: &Value) -> Result<Self> {
        match value {
            Value::Number(Number::F32(value)) => Ok(*value),
            Value::Number(Number::F64(value)) => parse_float::<f32>(value),
            Value::Number(Number::I(value)) => parse_float::<f32>(value),
            Value::Number(Number::U(value)) => parse_float::<f32>(value),
            other => Err(DataError::invalid_type("f32", other.type_name())),
        }
    }
}

impl Decode for f64 {
    fn decode(value: &Value) -> Result<Self> {
        match value {
            Value::Number(Number::F32(value)) => Ok(f64::from(*value)),
            Value::Number(Number::F64(value)) => Ok(*value),
            Value::Number(Number::I(value)) => parse_float::<f64>(value),
            Value::Number(Number::U(value)) => parse_float::<f64>(value),
            other => Err(DataError::invalid_type("f64", other.type_name())),
        }
    }
}

fn parse_float<T>(value: &impl ToString) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value.to_string().parse::<T>().map_err(|error| {
        DataError::new(
            DataErrorKind::NumberOutOfRange,
            format!("cannot decode floating-point value: {error}"),
        )
    })
}

impl Decode for String {
    fn decode(value: &Value) -> Result<Self> {
        match value {
            Value::String(value) => Ok(value.clone()),
            Value::Char(value) => Ok(value.to_string()),
            other => Err(DataError::invalid_type("string", other.type_name())),
        }
    }
}

impl Decode for char {
    fn decode(value: &Value) -> Result<Self> {
        match value {
            Value::Char(value) => Ok(*value),
            Value::String(value) => {
                let mut chars = value.chars();
                match (chars.next(), chars.next()) {
                    (Some(ch), None) => Ok(ch),
                    _ => Err(DataError::invalid_type("single-character string", "string")),
                }
            }
            other => Err(DataError::invalid_type("char", other.type_name())),
        }
    }
}

impl Decode for Bytes {
    fn decode(value: &Value) -> Result<Self> {
        match value {
            Value::Bytes(bytes) => Ok(bytes.clone()),
            other => Err(DataError::invalid_type("bytes", other.type_name())),
        }
    }
}

impl<T: Decode> Decode for Option<T> {
    fn decode(value: &Value) -> Result<Self> {
        match value {
            Value::Unit => Ok(None),
            other => T::decode(other).map(Some),
        }
    }
}

impl<T: Decode> Decode for Vec<T> {
    fn decode(value: &Value) -> Result<Self> {
        match value {
            Value::Seq(values) => values
                .iter()
                .enumerate()
                .map(|(index, value)| T::decode(value).map_err(|err| err.at_index(index)))
                .collect(),
            other => Err(DataError::invalid_type("sequence", other.type_name())),
        }
    }
}

impl<T: Decode> Decode for BTreeMap<String, T> {
    fn decode(value: &Value) -> Result<Self> {
        match value {
            Value::Map(values) | Value::Record(values) => values
                .iter()
                .map(|(key, value)| {
                    T::decode(value)
                        .map(|decoded| (key.clone(), decoded))
                        .map_err(|err| err.at_field(key.clone()))
                })
                .collect(),
            other => Err(DataError::invalid_type("map", other.type_name())),
        }
    }
}
