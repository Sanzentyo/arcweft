use serde::{Deserialize, Serialize};
use std::fmt;

/// Width-preserving signed integer scalar.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeInt {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    ISize(i64),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeSignedIntWidth {
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
}

impl RuntimeInt {
    pub const fn i8(value: i8) -> Self {
        Self::I8(value)
    }

    pub const fn i16(value: i16) -> Self {
        Self::I16(value)
    }

    pub const fn i32(value: i32) -> Self {
        Self::I32(value)
    }

    pub const fn i64(value: i64) -> Self {
        Self::I64(value)
    }

    pub const fn i128(value: i128) -> Self {
        Self::I128(value)
    }

    pub const fn isize(value: i64) -> Self {
        Self::ISize(value)
    }

    pub fn try_sum_as_i64(self) -> Option<i64> {
        match self {
            Self::I8(value) => Some(i64::from(value)),
            Self::I16(value) => Some(i64::from(value)),
            Self::I32(value) => Some(i64::from(value)),
            Self::I64(value) | Self::ISize(value) => Some(value),
            Self::I128(value) => i64::try_from(value).ok(),
        }
    }

    pub fn try_into_i64(self) -> Option<i64> {
        self.try_sum_as_i64()
    }

    pub const fn exact_i64(self) -> Option<i64> {
        match self {
            Self::I64(value) => Some(value),
            Self::I8(_) | Self::I16(_) | Self::I32(_) | Self::I128(_) | Self::ISize(_) => None,
        }
    }

    pub const fn exact_i32(self) -> Option<i32> {
        match self {
            Self::I32(value) => Some(value),
            Self::I8(_) | Self::I16(_) | Self::I64(_) | Self::I128(_) | Self::ISize(_) => None,
        }
    }

    pub fn try_into_i32(self) -> Option<i32> {
        match self {
            Self::I8(value) => Some(i32::from(value)),
            Self::I16(value) => Some(i32::from(value)),
            Self::I32(value) => Some(value),
            Self::I64(value) | Self::ISize(value) => i32::try_from(value).ok(),
            Self::I128(value) => i32::try_from(value).ok(),
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::I8(value) => value.to_string(),
            Self::I16(value) => value.to_string(),
            Self::I32(value) => value.to_string(),
            Self::I64(value) | Self::ISize(value) => value.to_string(),
            Self::I128(value) => value.to_string(),
        }
    }

    pub const fn width(self) -> RuntimeSignedIntWidth {
        match self {
            Self::I8(_) => RuntimeSignedIntWidth::I8,
            Self::I16(_) => RuntimeSignedIntWidth::I16,
            Self::I32(_) => RuntimeSignedIntWidth::I32,
            Self::I64(_) => RuntimeSignedIntWidth::I64,
            Self::I128(_) => RuntimeSignedIntWidth::I128,
            Self::ISize(_) => RuntimeSignedIntWidth::ISize,
        }
    }

    pub const fn as_i128(self) -> i128 {
        match self {
            Self::I8(value) => value as i128,
            Self::I16(value) => value as i128,
            Self::I32(value) => value as i128,
            Self::I64(value) | Self::ISize(value) => value as i128,
            Self::I128(value) => value,
        }
    }

    pub fn from_i128(width: RuntimeSignedIntWidth, value: i128) -> Option<Self> {
        Some(match width {
            RuntimeSignedIntWidth::I8 => Self::I8(i8::try_from(value).ok()?),
            RuntimeSignedIntWidth::I16 => Self::I16(i16::try_from(value).ok()?),
            RuntimeSignedIntWidth::I32 => Self::I32(i32::try_from(value).ok()?),
            RuntimeSignedIntWidth::I64 => Self::I64(i64::try_from(value).ok()?),
            RuntimeSignedIntWidth::I128 => Self::I128(value),
            RuntimeSignedIntWidth::ISize => Self::ISize(i64::try_from(value).ok()?),
        })
    }
}

impl fmt::Display for RuntimeInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

/// Width-preserving unsigned integer scalar.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeUInt {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    USize(u64),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeUnsignedIntWidth {
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
}

impl RuntimeUInt {
    pub const fn u8(value: u8) -> Self {
        Self::U8(value)
    }

    pub const fn u16(value: u16) -> Self {
        Self::U16(value)
    }

    pub const fn u32(value: u32) -> Self {
        Self::U32(value)
    }

    pub const fn u64(value: u64) -> Self {
        Self::U64(value)
    }

    pub const fn u128(value: u128) -> Self {
        Self::U128(value)
    }

    pub const fn usize(value: u64) -> Self {
        Self::USize(value)
    }

    pub fn try_sum_as_i64(self) -> Option<i64> {
        match self {
            Self::U8(value) => Some(i64::from(value)),
            Self::U16(value) => Some(i64::from(value)),
            Self::U32(value) => Some(i64::from(value)),
            Self::U64(value) | Self::USize(value) => i64::try_from(value).ok(),
            Self::U128(value) => i64::try_from(value).ok(),
        }
    }

    pub fn try_into_i64(self) -> Option<i64> {
        self.try_sum_as_i64()
    }

    pub fn try_into_u64(self) -> Option<u64> {
        match self {
            Self::U8(value) => Some(u64::from(value)),
            Self::U16(value) => Some(u64::from(value)),
            Self::U32(value) => Some(u64::from(value)),
            Self::U64(value) | Self::USize(value) => Some(value),
            Self::U128(value) => u64::try_from(value).ok(),
        }
    }

    pub fn try_into_i32(self) -> Option<i32> {
        match self {
            Self::U8(value) => Some(i32::from(value)),
            Self::U16(value) => Some(i32::from(value)),
            Self::U32(value) => i32::try_from(value).ok(),
            Self::U64(value) | Self::USize(value) => i32::try_from(value).ok(),
            Self::U128(value) => i32::try_from(value).ok(),
        }
    }

    pub const fn exact_u32(self) -> Option<u32> {
        match self {
            Self::U32(value) => Some(value),
            Self::U8(_) | Self::U16(_) | Self::U64(_) | Self::U128(_) | Self::USize(_) => None,
        }
    }

    pub fn try_into_u32(self) -> Option<u32> {
        match self {
            Self::U8(value) => Some(u32::from(value)),
            Self::U16(value) => Some(u32::from(value)),
            Self::U32(value) => Some(value),
            Self::U64(value) | Self::USize(value) => u32::try_from(value).ok(),
            Self::U128(value) => u32::try_from(value).ok(),
        }
    }

    pub const fn exact_u64(self) -> Option<u64> {
        match self {
            Self::U64(value) => Some(value),
            Self::U8(_) | Self::U16(_) | Self::U32(_) | Self::U128(_) | Self::USize(_) => None,
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::U8(value) => value.to_string(),
            Self::U16(value) => value.to_string(),
            Self::U32(value) => value.to_string(),
            Self::U64(value) | Self::USize(value) => value.to_string(),
            Self::U128(value) => value.to_string(),
        }
    }

    pub const fn width(self) -> RuntimeUnsignedIntWidth {
        match self {
            Self::U8(_) => RuntimeUnsignedIntWidth::U8,
            Self::U16(_) => RuntimeUnsignedIntWidth::U16,
            Self::U32(_) => RuntimeUnsignedIntWidth::U32,
            Self::U64(_) => RuntimeUnsignedIntWidth::U64,
            Self::U128(_) => RuntimeUnsignedIntWidth::U128,
            Self::USize(_) => RuntimeUnsignedIntWidth::USize,
        }
    }

    pub const fn as_u128(self) -> u128 {
        match self {
            Self::U8(value) => value as u128,
            Self::U16(value) => value as u128,
            Self::U32(value) => value as u128,
            Self::U64(value) | Self::USize(value) => value as u128,
            Self::U128(value) => value,
        }
    }

    pub fn from_u128(width: RuntimeUnsignedIntWidth, value: u128) -> Option<Self> {
        Some(match width {
            RuntimeUnsignedIntWidth::U8 => Self::U8(u8::try_from(value).ok()?),
            RuntimeUnsignedIntWidth::U16 => Self::U16(u16::try_from(value).ok()?),
            RuntimeUnsignedIntWidth::U32 => Self::U32(u32::try_from(value).ok()?),
            RuntimeUnsignedIntWidth::U64 => Self::U64(u64::try_from(value).ok()?),
            RuntimeUnsignedIntWidth::U128 => Self::U128(value),
            RuntimeUnsignedIntWidth::USize => Self::USize(u64::try_from(value).ok()?),
        })
    }
}

impl fmt::Display for RuntimeUInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}
