use std::collections::BTreeMap;
use std::convert::TryFrom;

use crate::error::{DataError, DataErrorKind, Result};
use crate::shape::MapKind;
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
            Value::Option(None) => Ok(None),
            Value::Option(Some(value)) => T::decode(value).map(Some),
            other => Err(DataError::invalid_type("option", other.type_name())),
        }
    }
}

impl Decode for () {
    fn decode(value: &Value) -> Result<Self> {
        match value {
            Value::Unit => Ok(()),
            other => Err(DataError::invalid_type("unit", other.type_name())),
        }
    }
}

fn tuple_values(value: &Value, expected_len: usize) -> Result<&[Value]> {
    let values = value.as_tuple()?;
    if values.len() == expected_len {
        Ok(values)
    } else {
        Err(DataError::invalid_type(
            format!("tuple with {expected_len} fields"),
            format!("tuple with {} fields", values.len()),
        ))
    }
}

macro_rules! tuple_impls {
    ($arity:literal; $($ty:ident:$index:tt),+ $(,)?) => {
        impl<$($ty: Decode),+> Decode for ($($ty,)+) {
            fn decode(value: &Value) -> Result<Self> {
                let values = tuple_values(value, $arity)?;
                Ok(($(
                    $ty::decode(&values[$index]).map_err(|error| error.at_index($index))?,
                )+))
            }
        }
    };
}

tuple_impls!(1; A:0);
tuple_impls!(2; A:0, B:1);
tuple_impls!(3; A:0, B:1, C:2);
tuple_impls!(4; A:0, B:1, C:2, D:3);
tuple_impls!(5; A:0, B:1, C:2, D:3, E:4);
tuple_impls!(6; A:0, B:1, C:2, D:3, E:4, F:5);
tuple_impls!(7; A:0, B:1, C:2, D:3, E:4, F:5, G:6);
tuple_impls!(8; A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);
tuple_impls!(9; A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8);
tuple_impls!(10; A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9);
tuple_impls!(11; A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10);
tuple_impls!(12; A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11);

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

impl<K, T> Decode for BTreeMap<K, T>
where
    K: Decode + Ord,
    T: Decode,
{
    fn decode(value: &Value) -> Result<Self> {
        match value {
            Value::Map {
                kind: MapKind::BTree,
                entries,
            } => entries.iter().enumerate().try_fold(
                BTreeMap::new(),
                |mut decoded, (index, (key, value))| {
                    let key = K::decode(key).map_err(|error| error.at_index(index))?;
                    let value = T::decode(value).map_err(|error| error.at_index(index))?;
                    if decoded.insert(key, value).is_some() {
                        return Err(DataError::new(
                            DataErrorKind::DuplicateField,
                            format!("duplicate key in map entry {index}"),
                        )
                        .at_index(index));
                    }
                    Ok(decoded)
                },
            ),
            Value::Map { kind, .. } => Err(DataError::invalid_type(
                "BTree map",
                format!("{kind:?} map"),
            )),
            Value::Record(values) => {
                values
                    .iter()
                    .try_fold(BTreeMap::new(), |mut decoded, (key_name, value)| {
                        let key = K::decode(&Value::String(key_name.clone()))
                            .map_err(|error| error.at_field(key_name.clone()))?;
                        let value =
                            T::decode(value).map_err(|error| error.at_field(key_name.clone()))?;
                        if decoded.insert(key, value).is_some() {
                            return Err(DataError::new(
                                DataErrorKind::DuplicateField,
                                "record contains duplicate decoded map keys",
                            ));
                        }
                        Ok(decoded)
                    })
            }
            other => Err(DataError::invalid_type("map", other.type_name())),
        }
    }
}
