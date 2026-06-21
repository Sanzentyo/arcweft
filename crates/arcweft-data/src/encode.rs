use std::collections::BTreeMap;

use crate::Reflect;
use crate::error::{DataError, DataErrorKind, Result};
use crate::shape::TypeShape;
use crate::value::{Bytes, Number, Value};

/// Format-independent encoder into Arcweft's dynamic value tree.
pub trait Encode {
    fn encode(&self) -> Result<Value>;
}

macro_rules! signed_int_impl {
    ($ty:ty, $shape:expr) => {
        impl Encode for $ty {
            fn encode(&self) -> Result<Value> {
                Ok(Value::Number(Number::I(i128::from(*self))))
            }
        }

        impl Reflect for $ty {
            fn shape() -> TypeShape {
                $shape
            }
        }
    };
}

macro_rules! unsigned_int_impl {
    ($ty:ty, $shape:expr) => {
        impl Encode for $ty {
            fn encode(&self) -> Result<Value> {
                Ok(Value::Number(Number::U(u128::from(*self))))
            }
        }

        impl Reflect for $ty {
            fn shape() -> TypeShape {
                $shape
            }
        }
    };
}

signed_int_impl!(i8, TypeShape::I8);
signed_int_impl!(i16, TypeShape::I16);
signed_int_impl!(i32, TypeShape::I32);
signed_int_impl!(i64, TypeShape::I64);
signed_int_impl!(i128, TypeShape::I128);
unsigned_int_impl!(u8, TypeShape::U8);
unsigned_int_impl!(u16, TypeShape::U16);
unsigned_int_impl!(u32, TypeShape::U32);
unsigned_int_impl!(u64, TypeShape::U64);
unsigned_int_impl!(u128, TypeShape::U128);

impl Encode for isize {
    fn encode(&self) -> Result<Value> {
        Ok(Value::Number(Number::I(*self as i128)))
    }
}

impl Reflect for isize {
    fn shape() -> TypeShape {
        TypeShape::Isize
    }
}

impl Encode for usize {
    fn encode(&self) -> Result<Value> {
        Ok(Value::Number(Number::U(*self as u128)))
    }
}

impl Reflect for usize {
    fn shape() -> TypeShape {
        TypeShape::Usize
    }
}

impl Encode for f32 {
    fn encode(&self) -> Result<Value> {
        if self.is_finite() {
            Ok(Value::Number(Number::F32(*self)))
        } else {
            Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "non-finite f32 is not serializable by default",
            ))
        }
    }
}

impl Reflect for f32 {
    fn shape() -> TypeShape {
        TypeShape::F32
    }
}

impl Encode for f64 {
    fn encode(&self) -> Result<Value> {
        if self.is_finite() {
            Ok(Value::Number(Number::F64(*self)))
        } else {
            Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "non-finite f64 is not serializable by default",
            ))
        }
    }
}

impl Reflect for f64 {
    fn shape() -> TypeShape {
        TypeShape::F64
    }
}

impl Encode for bool {
    fn encode(&self) -> Result<Value> {
        Ok(Value::Bool(*self))
    }
}

impl Reflect for bool {
    fn shape() -> TypeShape {
        TypeShape::Bool
    }
}

impl Encode for String {
    fn encode(&self) -> Result<Value> {
        Ok(Value::String(self.clone()))
    }
}

impl Encode for str {
    fn encode(&self) -> Result<Value> {
        Ok(Value::String(self.to_owned()))
    }
}

impl Reflect for String {
    fn shape() -> TypeShape {
        TypeShape::String
    }
}

impl Encode for char {
    fn encode(&self) -> Result<Value> {
        Ok(Value::Char(*self))
    }
}

impl Reflect for char {
    fn shape() -> TypeShape {
        TypeShape::Char
    }
}

impl Encode for Bytes {
    fn encode(&self) -> Result<Value> {
        Ok(Value::Bytes(self.clone()))
    }
}

impl Reflect for Bytes {
    fn shape() -> TypeShape {
        TypeShape::Bytes {
            format: crate::BytesFormat::Binary,
        }
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encode(&self) -> Result<Value> {
        self.as_ref().map_or(Ok(Value::Unit), Encode::encode)
    }
}

impl<T: Reflect> Reflect for Option<T> {
    fn shape() -> TypeShape {
        TypeShape::option(T::shape())
    }
}

impl<T: Encode> Encode for Vec<T> {
    fn encode(&self) -> Result<Value> {
        self.iter()
            .enumerate()
            .map(|(index, item)| item.encode().map_err(|err| err.at_index(index)))
            .collect::<Result<Vec<_>>>()
            .map(Value::Seq)
    }
}

impl<T: Reflect> Reflect for Vec<T> {
    fn shape() -> TypeShape {
        TypeShape::seq(T::shape())
    }
}

impl<T: Encode> Encode for BTreeMap<String, T> {
    fn encode(&self) -> Result<Value> {
        self.iter()
            .map(|(key, value)| {
                value
                    .encode()
                    .map(|encoded| (key.clone(), encoded))
                    .map_err(|err| err.at_field(key.clone()))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(Value::Map)
    }
}

impl<T: Reflect> Reflect for BTreeMap<String, T> {
    fn shape() -> TypeShape {
        TypeShape::map(TypeShape::String, T::shape())
    }
}
