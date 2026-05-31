use crate::pattern::RuntimePattern;
use crate::plan::{RuntimePureHelperId, RuntimePureInputType, RuntimePureOutputType};
use crate::time::LogicalDuration;
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBinding {
    pub name: String,
    pub value: RuntimeValue,
}

/// Structured payload exchanged at the host/runtime boundary.
///
/// Payloads intentionally retain `RuntimeValue` shape instead of collapsing
/// source and stream items to debug strings. Hosts may still display `label()`
/// for logs, but replay and downstream runtime consumers keep typed data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePayload(pub RuntimeValue);

/// Deterministic `f32` value stored by IEEE-754 bits.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RuntimeF32(u32);

impl RuntimeF32 {
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn to_bits(self) -> u32 {
        self.0
    }

    pub fn from_f32(value: f32) -> Self {
        Self(value.to_bits())
    }

    pub fn to_f32(self) -> f32 {
        f32::from_bits(self.0)
    }
}

/// Deterministic `f64` value stored by IEEE-754 bits.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RuntimeF64(u64);

impl RuntimeF64 {
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    pub const fn to_bits(self) -> u64 {
        self.0
    }

    pub fn from_f64(value: f64) -> Self {
        Self(value.to_bits())
    }

    pub fn to_f64(self) -> f64 {
        f64::from_bits(self.0)
    }
}

/// Deterministic value domain used by the Sans I/O flow runtime.
///
/// Typed floats are stored by bits to keep equality and replay deterministic.
/// Untyped float literals remain as source strings until a later semantic pass
/// chooses their concrete representation and unit rules.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeValue {
    Unit,
    Bool(bool),
    Int(i64),
    I128(i128),
    UInt(u64),
    U128(u128),
    ISize(i64),
    USize(u64),
    F32(RuntimeF32),
    F64(RuntimeF64),
    Float(String),
    String(String),
    Char(char),
    Duration(LogicalDuration),
    EntityRef(String),
    Tuple(Vec<RuntimeValue>),
    Seq(RuntimeSeq),
    Record(Vec<RuntimeFieldValue>),
    Variant {
        path: Option<String>,
        name: String,
        payload: Option<Box<RuntimeValue>>,
    },
}

/// Storage strategy for runtime sequence values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeSeq {
    Values(Vec<RuntimeValue>),
    Dense(DenseSeq),
}

/// Exact integer storage that can cross runtime pure-helper fast paths without widening.
pub trait RuntimeExactInteger: Copy + 'static {
    const INPUT_TYPE: RuntimePureInputType;
    const OUTPUT_TYPE: RuntimePureOutputType;

    fn into_runtime_value(self) -> RuntimeValue;
    fn try_from_runtime_value(helper: &str, value: RuntimeValue) -> Result<Self, RuntimeEvalError>;
    fn try_sum_as_i64(self, helper: &str) -> Result<i64, RuntimeEvalError>;
    fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]>;
    fn dense_sequence(values: Vec<Self>) -> RuntimeValue;
}

macro_rules! impl_runtime_exact_signed_integer {
    ($ty:ty, $input_type:ident, $output_type:ident, $slice:ident, $dense:ident) => {
        impl RuntimeExactInteger for $ty {
            const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::$input_type;
            const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::$output_type;

            fn into_runtime_value(self) -> RuntimeValue {
                RuntimeValue::Int(i64::from(self))
            }

            fn try_from_runtime_value(
                helper: &str,
                value: RuntimeValue,
            ) -> Result<Self, RuntimeEvalError> {
                match value {
                    RuntimeValue::Int(value) => {
                        <$ty>::try_from(value).map_err(|_| RuntimeEvalError::UnsupportedPure {
                            name: helper.to_owned(),
                            reason: format!(
                                "pure {} result `{value}` is outside {} range",
                                stringify!($ty),
                                stringify!($ty)
                            ),
                        })
                    }
                    value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
                }
            }

            fn try_sum_as_i64(self, _helper: &str) -> Result<i64, RuntimeEvalError> {
                Ok(i64::from(self))
            }

            fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]> {
                seq.$slice()
            }

            fn dense_sequence(values: Vec<Self>) -> RuntimeValue {
                $dense(values)
            }
        }
    };
}

macro_rules! impl_runtime_exact_unsigned_integer {
    ($ty:ty, $input_type:ident, $output_type:ident, $slice:ident, $dense:ident) => {
        impl RuntimeExactInteger for $ty {
            const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::$input_type;
            const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::$output_type;

            fn into_runtime_value(self) -> RuntimeValue {
                RuntimeValue::UInt(u64::from(self))
            }

            fn try_from_runtime_value(
                helper: &str,
                value: RuntimeValue,
            ) -> Result<Self, RuntimeEvalError> {
                match value {
                    RuntimeValue::UInt(value) => {
                        <$ty>::try_from(value).map_err(|_| RuntimeEvalError::UnsupportedPure {
                            name: helper.to_owned(),
                            reason: format!(
                                "pure {} result `{value}` is outside {} range",
                                stringify!($ty),
                                stringify!($ty)
                            ),
                        })
                    }
                    value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
                }
            }

            fn try_sum_as_i64(self, helper: &str) -> Result<i64, RuntimeEvalError> {
                i64::try_from(self).map_err(|_| RuntimeEvalError::UnsupportedPure {
                    name: helper.to_owned(),
                    reason: format!(
                        "pure {} result `{self}` cannot be represented as an i64 sum",
                        stringify!($ty)
                    ),
                })
            }

            fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]> {
                seq.$slice()
            }

            fn dense_sequence(values: Vec<Self>) -> RuntimeValue {
                $dense(values)
            }
        }
    };
}

macro_rules! impl_runtime_exact_wide_signed_integer {
    ($ty:ty, $input_type:ident, $output_type:ident, $slice:ident, $dense:ident, $variant:ident) => {
        impl RuntimeExactInteger for $ty {
            const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::$input_type;
            const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::$output_type;

            fn into_runtime_value(self) -> RuntimeValue {
                RuntimeValue::$variant(self)
            }

            fn try_from_runtime_value(
                helper: &str,
                value: RuntimeValue,
            ) -> Result<Self, RuntimeEvalError> {
                match value {
                    RuntimeValue::$variant(value) => Ok(value),
                    value => Err(RuntimeEvalError::UnsupportedPure {
                        name: helper.to_owned(),
                        reason: format!(
                            "pure {} result expected {}, got {}",
                            stringify!($ty),
                            stringify!($ty),
                            runtime_value_label(&value)
                        ),
                    }),
                }
            }

            fn try_sum_as_i64(self, helper: &str) -> Result<i64, RuntimeEvalError> {
                i64::try_from(self).map_err(|_| RuntimeEvalError::UnsupportedPure {
                    name: helper.to_owned(),
                    reason: format!(
                        "pure {} result `{self}` cannot be represented as an i64 sum",
                        stringify!($ty)
                    ),
                })
            }

            fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]> {
                seq.$slice()
            }

            fn dense_sequence(values: Vec<Self>) -> RuntimeValue {
                $dense(values)
            }
        }
    };
}

macro_rules! impl_runtime_exact_wide_unsigned_integer {
    ($ty:ty, $input_type:ident, $output_type:ident, $slice:ident, $dense:ident, $variant:ident) => {
        impl RuntimeExactInteger for $ty {
            const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::$input_type;
            const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::$output_type;

            fn into_runtime_value(self) -> RuntimeValue {
                RuntimeValue::$variant(self)
            }

            fn try_from_runtime_value(
                helper: &str,
                value: RuntimeValue,
            ) -> Result<Self, RuntimeEvalError> {
                match value {
                    RuntimeValue::$variant(value) => Ok(value),
                    value => Err(RuntimeEvalError::UnsupportedPure {
                        name: helper.to_owned(),
                        reason: format!(
                            "pure {} result expected {}, got {}",
                            stringify!($ty),
                            stringify!($ty),
                            runtime_value_label(&value)
                        ),
                    }),
                }
            }

            fn try_sum_as_i64(self, helper: &str) -> Result<i64, RuntimeEvalError> {
                i64::try_from(self).map_err(|_| RuntimeEvalError::UnsupportedPure {
                    name: helper.to_owned(),
                    reason: format!(
                        "pure {} result `{self}` cannot be represented as an i64 sum",
                        stringify!($ty)
                    ),
                })
            }

            fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]> {
                seq.$slice()
            }

            fn dense_sequence(values: Vec<Self>) -> RuntimeValue {
                $dense(values)
            }
        }
    };
}

impl RuntimeSeq {
    pub fn values(values: Vec<RuntimeValue>) -> Self {
        Self::Values(values)
    }

    pub const fn dense_units(len: usize) -> Self {
        Self::Dense(DenseSeq::units(len))
    }

    pub fn dense_i64(values: Vec<i64>) -> Self {
        Self::Dense(DenseSeq::i64(values))
    }

    pub fn dense_i128(values: Vec<i128>) -> Self {
        Self::Dense(DenseSeq::i128(values))
    }

    pub fn dense_isize(values: Vec<i64>) -> Self {
        Self::Dense(DenseSeq::isize(values))
    }

    pub fn dense_i8(values: Vec<i8>) -> Self {
        Self::Dense(DenseSeq::i8(values))
    }

    pub fn dense_i16(values: Vec<i16>) -> Self {
        Self::Dense(DenseSeq::i16(values))
    }

    pub fn dense_i32(values: Vec<i32>) -> Self {
        Self::Dense(DenseSeq::i32(values))
    }

    pub fn dense_u8(values: Vec<u8>) -> Self {
        Self::Dense(DenseSeq::u8(values))
    }

    pub fn dense_u16(values: Vec<u16>) -> Self {
        Self::Dense(DenseSeq::u16(values))
    }

    pub fn dense_u32(values: Vec<u32>) -> Self {
        Self::Dense(DenseSeq::u32(values))
    }

    pub fn dense_u64(values: Vec<u64>) -> Self {
        Self::Dense(DenseSeq::u64(values))
    }

    pub fn dense_u128(values: Vec<u128>) -> Self {
        Self::Dense(DenseSeq::u128(values))
    }

    pub fn dense_usize(values: Vec<u64>) -> Self {
        Self::Dense(DenseSeq::usize(values))
    }

    pub fn dense_f32(values: Vec<RuntimeF32>) -> Self {
        Self::Dense(DenseSeq::f32(values))
    }

    pub fn dense_f64(values: Vec<RuntimeF64>) -> Self {
        Self::Dense(DenseSeq::f64(values))
    }

    pub fn dense_bool(values: Vec<bool>) -> Self {
        Self::Dense(DenseSeq::bool(values))
    }

    pub fn dense_bytes(values: Vec<u8>) -> Self {
        Self::Dense(DenseSeq::bytes(values))
    }

    pub fn dense_chars(values: Vec<char>) -> Self {
        Self::Dense(DenseSeq::chars(values))
    }

    pub fn dense_durations(values: Vec<LogicalDuration>) -> Self {
        Self::Dense(DenseSeq::durations(values))
    }

    pub fn dense_strings(values: Vec<String>) -> Self {
        Self::Dense(DenseSeq::strings(values))
    }

    pub fn dense_float_literals(values: Vec<String>) -> Self {
        Self::Dense(DenseSeq::float_literals(values))
    }

    pub fn dense_entity_refs(values: Vec<String>) -> Self {
        Self::Dense(DenseSeq::entity_refs(values))
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Dense(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_values(&self) -> Option<&[RuntimeValue]> {
        match self {
            Self::Values(values) => Some(values),
            Self::Dense(_) => None,
        }
    }

    pub fn unit_len(&self) -> Option<usize> {
        match self {
            Self::Dense(values) => values.unit_len(),
            Self::Values(_) => None,
        }
    }

    pub fn dense_kind(&self) -> Option<DenseSeqKind> {
        match self {
            Self::Dense(values) => Some(values.kind()),
            Self::Values(_) => None,
        }
    }

    pub fn as_i64_slice(&self) -> Option<&[i64]> {
        match self {
            Self::Dense(values) => values.as_i64_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn copy_i64_values_to(&self, out: &mut Vec<i64>) -> bool {
        match self {
            Self::Dense(values) => values.copy_i64_values_to(out),
            Self::Values(_) => false,
        }
    }

    pub fn try_for_each_i64<E>(&self, visit: impl FnMut(i64) -> Result<(), E>) -> Result<bool, E> {
        match self {
            Self::Dense(values) => values.try_for_each_i64(visit),
            Self::Values(_) => Ok(false),
        }
    }

    pub fn first_i64(&self) -> Option<Option<i64>> {
        match self {
            Self::Dense(values) => values.first_i64(),
            Self::Values(_) => None,
        }
    }

    pub fn as_i128_slice(&self) -> Option<&[i128]> {
        match self {
            Self::Dense(values) => values.as_i128_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_isize_values(&self) -> Option<&[i64]> {
        match self {
            Self::Dense(values) => values.as_isize_values(),
            Self::Values(_) => None,
        }
    }

    pub fn as_i8_slice(&self) -> Option<&[i8]> {
        match self {
            Self::Dense(values) => values.as_i8_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_i16_slice(&self) -> Option<&[i16]> {
        match self {
            Self::Dense(values) => values.as_i16_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_i32_slice(&self) -> Option<&[i32]> {
        match self {
            Self::Dense(values) => values.as_i32_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_u8_slice(&self) -> Option<&[u8]> {
        match self {
            Self::Dense(values) => values.as_u8_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_u16_slice(&self) -> Option<&[u16]> {
        match self {
            Self::Dense(values) => values.as_u16_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_u32_slice(&self) -> Option<&[u32]> {
        match self {
            Self::Dense(values) => values.as_u32_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_u64_slice(&self) -> Option<&[u64]> {
        match self {
            Self::Dense(values) => values.as_u64_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_u128_slice(&self) -> Option<&[u128]> {
        match self {
            Self::Dense(values) => values.as_u128_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_usize_values(&self) -> Option<&[u64]> {
        match self {
            Self::Dense(values) => values.as_usize_values(),
            Self::Values(_) => None,
        }
    }

    pub fn as_f32_slice(&self) -> Option<&[RuntimeF32]> {
        match self {
            Self::Dense(values) => values.as_f32_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_f64_slice(&self) -> Option<&[RuntimeF64]> {
        match self {
            Self::Dense(values) => values.as_f64_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_bool_slice(&self) -> Option<&[bool]> {
        match self {
            Self::Dense(values) => values.as_bool_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Dense(values) => values.as_bytes(),
            Self::Values(_) => None,
        }
    }

    pub fn as_chars(&self) -> Option<&[char]> {
        match self {
            Self::Dense(values) => values.as_chars(),
            Self::Values(_) => None,
        }
    }

    pub fn as_durations(&self) -> Option<&[LogicalDuration]> {
        match self {
            Self::Dense(values) => values.as_durations(),
            Self::Values(_) => None,
        }
    }

    pub fn as_strings(&self) -> Option<&[String]> {
        match self {
            Self::Dense(values) => values.as_strings(),
            Self::Values(_) => None,
        }
    }

    pub fn as_float_literals(&self) -> Option<&[String]> {
        match self {
            Self::Dense(values) => values.as_float_literals(),
            Self::Values(_) => None,
        }
    }

    pub fn as_entity_refs(&self) -> Option<&[String]> {
        match self {
            Self::Dense(values) => values.as_entity_refs(),
            Self::Values(_) => None,
        }
    }

    pub fn into_values(self) -> Vec<RuntimeValue> {
        match self {
            Self::Values(values) => values,
            Self::Dense(values) => values.into_values(),
        }
    }

    #[must_use]
    pub fn tail_from(&self, index: usize) -> Self {
        match self {
            Self::Values(values) => Self::Values(values[index..].to_vec()),
            Self::Dense(values) => Self::Dense(values.tail_from(index)),
        }
    }

    pub fn into_i64_vec(self) -> Option<Vec<i64>> {
        match self {
            Self::Dense(values) => values.into_i64_vec(),
            Self::Values(_) => None,
        }
    }

    pub fn sum_as_i64(&self) -> Option<i64> {
        match self {
            Self::Dense(values) => values.sum_as_i64(),
            Self::Values(_) => None,
        }
    }
}

impl_runtime_exact_signed_integer!(i8, I8, I8, as_i8_slice, runtime_sequence_dense_i8);
impl_runtime_exact_signed_integer!(i16, I16, I16, as_i16_slice, runtime_sequence_dense_i16);
impl_runtime_exact_signed_integer!(i32, I32, I32, as_i32_slice, runtime_sequence_dense_i32);
impl_runtime_exact_wide_signed_integer!(
    i128,
    I128,
    I128,
    as_i128_slice,
    runtime_sequence_dense_i128,
    I128
);
impl_runtime_exact_unsigned_integer!(u8, U8, U8, as_u8_slice, runtime_sequence_dense_u8);
impl_runtime_exact_unsigned_integer!(u16, U16, U16, as_u16_slice, runtime_sequence_dense_u16);
impl_runtime_exact_unsigned_integer!(u32, U32, U32, as_u32_slice, runtime_sequence_dense_u32);
impl_runtime_exact_unsigned_integer!(u64, U64, U64, as_u64_slice, runtime_sequence_dense_u64);
impl_runtime_exact_wide_unsigned_integer!(
    u128,
    U128,
    U128,
    as_u128_slice,
    runtime_sequence_dense_u128,
    U128
);

/// Homogeneous storage kind used by a dense runtime sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DenseSeqKind {
    Units,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
    Bool,
    Bytes,
    Chars,
    Durations,
    Strings,
    FloatLiterals,
    EntityRefs,
}

/// Dense sequence storage for homogeneous scalar data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DenseSeq {
    Units(usize),
    I8(DenseSeqStorage<i8>),
    I16(DenseSeqStorage<i16>),
    I32(DenseSeqStorage<i32>),
    I64(DenseSeqStorage<i64>),
    I128(DenseSeqStorage<i128>),
    ISize(DenseSeqStorage<i64>),
    U8(DenseSeqStorage<u8>),
    U16(DenseSeqStorage<u16>),
    U32(DenseSeqStorage<u32>),
    U64(DenseSeqStorage<u64>),
    U128(DenseSeqStorage<u128>),
    USize(DenseSeqStorage<u64>),
    F32(DenseSeqStorage<RuntimeF32>),
    F64(DenseSeqStorage<RuntimeF64>),
    Bool(DenseSeqStorage<bool>),
    Bytes(DenseSeqStorage<u8>),
    Chars(DenseSeqStorage<char>),
    Durations(DenseSeqStorage<LogicalDuration>),
    Strings(DenseSeqStorage<String>),
    FloatLiterals(DenseSeqStorage<String>),
    EntityRefs(DenseSeqStorage<String>),
}

impl DenseSeq {
    pub const fn units(len: usize) -> Self {
        Self::Units(len)
    }

    pub fn i8(values: Vec<i8>) -> Self {
        Self::I8(DenseSeqStorage::new(values))
    }

    pub fn i16(values: Vec<i16>) -> Self {
        Self::I16(DenseSeqStorage::new(values))
    }

    pub fn i32(values: Vec<i32>) -> Self {
        Self::I32(DenseSeqStorage::new(values))
    }

    pub fn i64(values: Vec<i64>) -> Self {
        Self::I64(DenseSeqStorage::new(values))
    }

    pub fn i128(values: Vec<i128>) -> Self {
        Self::I128(DenseSeqStorage::new(values))
    }

    pub fn isize(values: Vec<i64>) -> Self {
        Self::ISize(DenseSeqStorage::new(values))
    }

    pub fn u8(values: Vec<u8>) -> Self {
        Self::U8(DenseSeqStorage::new(values))
    }

    pub fn u16(values: Vec<u16>) -> Self {
        Self::U16(DenseSeqStorage::new(values))
    }

    pub fn u32(values: Vec<u32>) -> Self {
        Self::U32(DenseSeqStorage::new(values))
    }

    pub fn u64(values: Vec<u64>) -> Self {
        Self::U64(DenseSeqStorage::new(values))
    }

    pub fn u128(values: Vec<u128>) -> Self {
        Self::U128(DenseSeqStorage::new(values))
    }

    pub fn usize(values: Vec<u64>) -> Self {
        Self::USize(DenseSeqStorage::new(values))
    }

    pub fn f32(values: Vec<RuntimeF32>) -> Self {
        Self::F32(DenseSeqStorage::new(values))
    }

    pub fn f64(values: Vec<RuntimeF64>) -> Self {
        Self::F64(DenseSeqStorage::new(values))
    }

    pub fn bool(values: Vec<bool>) -> Self {
        Self::Bool(DenseSeqStorage::new(values))
    }

    pub fn bytes(values: Vec<u8>) -> Self {
        Self::Bytes(DenseSeqStorage::new(values))
    }

    pub fn chars(values: Vec<char>) -> Self {
        Self::Chars(DenseSeqStorage::new(values))
    }

    pub fn durations(values: Vec<LogicalDuration>) -> Self {
        Self::Durations(DenseSeqStorage::new(values))
    }

    pub fn strings(values: Vec<String>) -> Self {
        Self::Strings(DenseSeqStorage::new(values))
    }

    pub fn float_literals(values: Vec<String>) -> Self {
        Self::FloatLiterals(DenseSeqStorage::new(values))
    }

    pub fn entity_refs(values: Vec<String>) -> Self {
        Self::EntityRefs(DenseSeqStorage::new(values))
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Units(len) => *len,
            Self::I8(values) => values.len(),
            Self::I16(values) => values.len(),
            Self::I32(values) => values.len(),
            Self::I64(values) | Self::ISize(values) => values.len(),
            Self::I128(values) => values.len(),
            Self::U8(values) | Self::Bytes(values) => values.len(),
            Self::U16(values) => values.len(),
            Self::U32(values) => values.len(),
            Self::U64(values) | Self::USize(values) => values.len(),
            Self::U128(values) => values.len(),
            Self::F32(values) => values.len(),
            Self::F64(values) => values.len(),
            Self::Bool(values) => values.len(),
            Self::Chars(values) => values.len(),
            Self::Durations(values) => values.len(),
            Self::Strings(values) | Self::FloatLiterals(values) | Self::EntityRefs(values) => {
                values.len()
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub const fn kind(&self) -> DenseSeqKind {
        match self {
            Self::Units(_) => DenseSeqKind::Units,
            Self::I8(_) => DenseSeqKind::I8,
            Self::I16(_) => DenseSeqKind::I16,
            Self::I32(_) => DenseSeqKind::I32,
            Self::I64(_) => DenseSeqKind::I64,
            Self::I128(_) => DenseSeqKind::I128,
            Self::ISize(_) => DenseSeqKind::ISize,
            Self::U8(_) => DenseSeqKind::U8,
            Self::U16(_) => DenseSeqKind::U16,
            Self::U32(_) => DenseSeqKind::U32,
            Self::U64(_) => DenseSeqKind::U64,
            Self::U128(_) => DenseSeqKind::U128,
            Self::USize(_) => DenseSeqKind::USize,
            Self::F32(_) => DenseSeqKind::F32,
            Self::F64(_) => DenseSeqKind::F64,
            Self::Bool(_) => DenseSeqKind::Bool,
            Self::Bytes(_) => DenseSeqKind::Bytes,
            Self::Chars(_) => DenseSeqKind::Chars,
            Self::Durations(_) => DenseSeqKind::Durations,
            Self::Strings(_) => DenseSeqKind::Strings,
            Self::FloatLiterals(_) => DenseSeqKind::FloatLiterals,
            Self::EntityRefs(_) => DenseSeqKind::EntityRefs,
        }
    }

    pub const fn unit_len(&self) -> Option<usize> {
        match self {
            Self::Units(len) => Some(*len),
            Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I64(_)
            | Self::I128(_)
            | Self::ISize(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::U128(_)
            | Self::USize(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Bytes(_)
            | Self::Chars(_)
            | Self::Durations(_)
            | Self::Strings(_)
            | Self::FloatLiterals(_)
            | Self::EntityRefs(_) => None,
        }
    }

    pub fn as_i64_slice(&self) -> Option<&[i64]> {
        match self {
            Self::I64(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn copy_i64_values_to(&self, out: &mut Vec<i64>) -> bool {
        match self {
            Self::I64(values) => {
                out.extend(values.as_slice().iter().copied());
                true
            }
            Self::Units(_)
            | Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I128(_)
            | Self::ISize(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::U128(_)
            | Self::USize(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bytes(_)
            | Self::Bool(_)
            | Self::Chars(_)
            | Self::Durations(_)
            | Self::Strings(_)
            | Self::FloatLiterals(_)
            | Self::EntityRefs(_) => false,
        }
    }

    pub fn try_for_each_i64<E>(
        &self,
        mut visit: impl FnMut(i64) -> Result<(), E>,
    ) -> Result<bool, E> {
        match self {
            Self::I64(values) => {
                for value in values.as_slice().iter().copied() {
                    visit(value)?;
                }
                Ok(true)
            }
            Self::Units(_)
            | Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I128(_)
            | Self::ISize(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::U128(_)
            | Self::USize(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Bytes(_)
            | Self::Chars(_)
            | Self::Durations(_)
            | Self::Strings(_)
            | Self::FloatLiterals(_)
            | Self::EntityRefs(_) => Ok(false),
        }
    }

    pub fn first_i64(&self) -> Option<Option<i64>> {
        let first = match self {
            Self::I64(values) => values.as_slice().first().copied(),
            Self::Units(_)
            | Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I128(_)
            | Self::ISize(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::U128(_)
            | Self::USize(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bytes(_)
            | Self::Bool(_)
            | Self::Chars(_)
            | Self::Durations(_)
            | Self::Strings(_)
            | Self::FloatLiterals(_)
            | Self::EntityRefs(_) => return None,
        };
        Some(first)
    }

    pub fn as_i128_slice(&self) -> Option<&[i128]> {
        match self {
            Self::I128(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_isize_values(&self) -> Option<&[i64]> {
        match self {
            Self::ISize(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_i8_slice(&self) -> Option<&[i8]> {
        match self {
            Self::I8(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_i16_slice(&self) -> Option<&[i16]> {
        match self {
            Self::I16(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_i32_slice(&self) -> Option<&[i32]> {
        match self {
            Self::I32(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_u8_slice(&self) -> Option<&[u8]> {
        match self {
            Self::U8(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_u16_slice(&self) -> Option<&[u16]> {
        match self {
            Self::U16(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_u32_slice(&self) -> Option<&[u32]> {
        match self {
            Self::U32(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_u64_slice(&self) -> Option<&[u64]> {
        match self {
            Self::U64(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_u128_slice(&self) -> Option<&[u128]> {
        match self {
            Self::U128(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_usize_values(&self) -> Option<&[u64]> {
        match self {
            Self::USize(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_f32_slice(&self) -> Option<&[RuntimeF32]> {
        match self {
            Self::F32(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_f64_slice(&self) -> Option<&[RuntimeF64]> {
        match self {
            Self::F64(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_bool_slice(&self) -> Option<&[bool]> {
        match self {
            Self::Bool(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(values) | Self::U8(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_chars(&self) -> Option<&[char]> {
        match self {
            Self::Chars(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_durations(&self) -> Option<&[LogicalDuration]> {
        match self {
            Self::Durations(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_strings(&self) -> Option<&[String]> {
        match self {
            Self::Strings(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_float_literals(&self) -> Option<&[String]> {
        match self {
            Self::FloatLiterals(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_entity_refs(&self) -> Option<&[String]> {
        match self {
            Self::EntityRefs(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn into_values(self) -> Vec<RuntimeValue> {
        match self {
            Self::Units(len) => vec![RuntimeValue::Unit; len],
            Self::I8(values) => {
                materialize_i64_sequence(values.into_vec().into_iter().map(i64::from).collect())
            }
            Self::I16(values) => {
                materialize_i64_sequence(values.into_vec().into_iter().map(i64::from).collect())
            }
            Self::I32(values) => {
                materialize_i64_sequence(values.into_vec().into_iter().map(i64::from).collect())
            }
            Self::I64(values) => materialize_i64_sequence(values.into_vec()),
            Self::I128(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::I128)
                .collect(),
            Self::ISize(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::ISize)
                .collect(),
            Self::U8(values) => values
                .into_vec()
                .into_iter()
                .map(|value| RuntimeValue::UInt(u64::from(value)))
                .collect(),
            Self::U16(values) => values
                .into_vec()
                .into_iter()
                .map(|value| RuntimeValue::UInt(u64::from(value)))
                .collect(),
            Self::U32(values) => values
                .into_vec()
                .into_iter()
                .map(|value| RuntimeValue::UInt(u64::from(value)))
                .collect(),
            Self::U64(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::UInt)
                .collect(),
            Self::U128(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::U128)
                .collect(),
            Self::USize(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::USize)
                .collect(),
            Self::F32(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::F32)
                .collect(),
            Self::F64(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::F64)
                .collect(),
            Self::Bool(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::Bool)
                .collect(),
            Self::Bytes(values) => values
                .into_vec()
                .into_iter()
                .map(|value| RuntimeValue::Int(i64::from(value)))
                .collect(),
            Self::Chars(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::Char)
                .collect(),
            Self::Durations(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::Duration)
                .collect(),
            Self::Strings(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::String)
                .collect(),
            Self::FloatLiterals(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::Float)
                .collect(),
            Self::EntityRefs(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::EntityRef)
                .collect(),
        }
    }

    /// Returns the runtime value at `index`.
    ///
    /// # Panics
    ///
    /// Panics when `index` is outside this sequence.
    pub fn value_at(&self, index: usize) -> RuntimeValue {
        match self {
            Self::Units(len) => {
                assert!(index < *len, "unit dense sequence index out of bounds");
                RuntimeValue::Unit
            }
            Self::I8(values) => RuntimeValue::Int(i64::from(values.as_slice()[index])),
            Self::I16(values) => RuntimeValue::Int(i64::from(values.as_slice()[index])),
            Self::I32(values) => RuntimeValue::Int(i64::from(values.as_slice()[index])),
            Self::I64(values) => RuntimeValue::Int(values.as_slice()[index]),
            Self::I128(values) => RuntimeValue::I128(values.as_slice()[index]),
            Self::ISize(values) => RuntimeValue::ISize(values.as_slice()[index]),
            Self::U8(values) => RuntimeValue::UInt(u64::from(values.as_slice()[index])),
            Self::U16(values) => RuntimeValue::UInt(u64::from(values.as_slice()[index])),
            Self::U32(values) => RuntimeValue::UInt(u64::from(values.as_slice()[index])),
            Self::U64(values) => RuntimeValue::UInt(values.as_slice()[index]),
            Self::U128(values) => RuntimeValue::U128(values.as_slice()[index]),
            Self::USize(values) => RuntimeValue::USize(values.as_slice()[index]),
            Self::F32(values) => RuntimeValue::F32(values.as_slice()[index]),
            Self::F64(values) => RuntimeValue::F64(values.as_slice()[index]),
            Self::Bool(values) => RuntimeValue::Bool(values.as_slice()[index]),
            Self::Bytes(values) => RuntimeValue::Int(i64::from(values.as_slice()[index])),
            Self::Chars(values) => RuntimeValue::Char(values.as_slice()[index]),
            Self::Durations(values) => RuntimeValue::Duration(values.as_slice()[index]),
            Self::Strings(values) => RuntimeValue::String(values.as_slice()[index].clone()),
            Self::FloatLiterals(values) => RuntimeValue::Float(values.as_slice()[index].clone()),
            Self::EntityRefs(values) => RuntimeValue::EntityRef(values.as_slice()[index].clone()),
        }
    }

    #[must_use]
    pub fn tail_from(&self, index: usize) -> Self {
        match self {
            Self::Units(len) => Self::Units(len.saturating_sub(index)),
            Self::I8(values) => Self::I8(values.tail_from(index)),
            Self::I16(values) => Self::I16(values.tail_from(index)),
            Self::I32(values) => Self::I32(values.tail_from(index)),
            Self::I64(values) => Self::I64(values.tail_from(index)),
            Self::I128(values) => Self::I128(values.tail_from(index)),
            Self::ISize(values) => Self::ISize(values.tail_from(index)),
            Self::U8(values) => Self::U8(values.tail_from(index)),
            Self::U16(values) => Self::U16(values.tail_from(index)),
            Self::U32(values) => Self::U32(values.tail_from(index)),
            Self::U64(values) => Self::U64(values.tail_from(index)),
            Self::U128(values) => Self::U128(values.tail_from(index)),
            Self::USize(values) => Self::USize(values.tail_from(index)),
            Self::F32(values) => Self::F32(values.tail_from(index)),
            Self::F64(values) => Self::F64(values.tail_from(index)),
            Self::Bool(values) => Self::Bool(values.tail_from(index)),
            Self::Bytes(values) => Self::Bytes(values.tail_from(index)),
            Self::Chars(values) => Self::Chars(values.tail_from(index)),
            Self::Durations(values) => Self::Durations(values.tail_from(index)),
            Self::Strings(values) => Self::Strings(values.tail_from(index)),
            Self::FloatLiterals(values) => Self::FloatLiterals(values.tail_from(index)),
            Self::EntityRefs(values) => Self::EntityRefs(values.tail_from(index)),
        }
    }

    pub fn into_i64_vec(self) -> Option<Vec<i64>> {
        match self {
            Self::I64(values) => Some(values.into_vec()),
            Self::Units(_)
            | Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I128(_)
            | Self::ISize(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::U128(_)
            | Self::USize(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Bytes(_)
            | Self::Chars(_)
            | Self::Durations(_)
            | Self::Strings(_)
            | Self::FloatLiterals(_)
            | Self::EntityRefs(_) => None,
        }
    }

    pub fn sum_as_i64(&self) -> Option<i64> {
        match self {
            Self::I8(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::I16(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::I32(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::I64(values) | Self::ISize(values) => Some(values.as_slice().iter().sum()),
            Self::I128(values) => values.as_slice().iter().try_fold(0_i64, |acc, value| {
                i64::try_from(*value).ok().map(|value| acc + value)
            }),
            Self::U8(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::U16(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::U32(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::U64(values) | Self::USize(values) => {
                values.as_slice().iter().try_fold(0_i64, |acc, value| {
                    i64::try_from(*value).ok().map(|value| acc + value)
                })
            }
            Self::U128(values) => values.as_slice().iter().try_fold(0_i64, |acc, value| {
                i64::try_from(*value).ok().map(|value| acc + value)
            }),
            Self::Units(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Bytes(_)
            | Self::Chars(_)
            | Self::Durations(_)
            | Self::Strings(_)
            | Self::FloatLiterals(_)
            | Self::EntityRefs(_) => None,
        }
    }
}

/// Generic backing store for one dense homogeneous sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DenseSeqStorage<T> {
    values: Vec<T>,
}

impl<T> DenseSeqStorage<T> {
    pub fn new(values: Vec<T>) -> Self {
        Self { values }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.values
    }

    pub fn into_vec(self) -> Vec<T> {
        self.values
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl<T: Clone> DenseSeqStorage<T> {
    #[must_use]
    pub fn tail_from(&self, index: usize) -> Self {
        Self::new(self.values[index..].to_vec())
    }
}

/// One field inside a runtime record value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFieldValue {
    pub name: String,
    pub value: RuntimeValue,
}

/// Expression subset executable by the Sans I/O flow runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeExpr {
    Value(RuntimeValue),
    Local(String),
    EntityRef(String),
    Let {
        name: String,
        expr: Box<RuntimeExpr>,
        body: Box<RuntimeExpr>,
    },
    Tuple(Vec<RuntimeExpr>),
    BracketSeq(Vec<RuntimeExpr>),
    RepeatSeq {
        value: Box<RuntimeExpr>,
        len: usize,
    },
    Record(Vec<RuntimeFieldExpr>),
    Variant {
        path: Option<String>,
        name: String,
        payload: Option<Box<RuntimeExpr>>,
    },
    Field {
        target: Box<RuntimeExpr>,
        field: String,
    },
    Call {
        callee: String,
        args: Vec<RuntimeExpr>,
    },
    PureCall {
        helper: RuntimePureHelperId,
        args: Vec<RuntimeExpr>,
    },
    SpreadArg(Box<RuntimeExpr>),
    MethodCall {
        receiver: Box<RuntimeExpr>,
        method: String,
        args: Vec<RuntimeExpr>,
    },
    Map {
        source: Box<RuntimeExpr>,
        param: String,
        body: Box<RuntimeExpr>,
    },
    Sum {
        source: Box<RuntimeExpr>,
    },
    Unary {
        op: RuntimeUnaryOp,
        expr: Box<RuntimeExpr>,
    },
    Binary {
        lhs: Box<RuntimeExpr>,
        op: RuntimeBinaryOp,
        rhs: Box<RuntimeExpr>,
    },
    If {
        condition: Box<RuntimeExpr>,
        then_expr: Box<RuntimeExpr>,
        else_expr: Box<RuntimeExpr>,
    },
    IfLet {
        pattern: RuntimePattern,
        expr: Box<RuntimeExpr>,
        guard: Option<Box<RuntimeExpr>>,
        then_expr: Box<RuntimeExpr>,
        else_expr: Box<RuntimeExpr>,
    },
    Match {
        scrutinee: Box<RuntimeExpr>,
        arms: Vec<RuntimeExprMatchArm>,
    },
}

impl RuntimeExpr {
    /// Returns whether this expression can use the allocation-light scalar pure evaluator.
    pub fn supports_scalar_pure_eval(&self) -> bool {
        match self {
            Self::Value(RuntimeValue::Bool(_) | RuntimeValue::Int(_)) | Self::Local(_) => true,
            Self::Let { expr, body, .. } => {
                expr.supports_scalar_pure_eval() && body.supports_scalar_pure_eval()
            }
            Self::Unary { expr, .. } => expr.supports_scalar_pure_eval(),
            Self::Binary { lhs, rhs, .. } => {
                lhs.supports_scalar_pure_eval() && rhs.supports_scalar_pure_eval()
            }
            Self::If {
                condition,
                then_expr,
                else_expr,
            } => {
                condition.supports_scalar_pure_eval()
                    && then_expr.supports_scalar_pure_eval()
                    && else_expr.supports_scalar_pure_eval()
            }
            Self::Value(_)
            | Self::EntityRef(_)
            | Self::Tuple(_)
            | Self::BracketSeq(_)
            | Self::RepeatSeq { .. }
            | Self::Record(_)
            | Self::Variant { .. }
            | Self::Field { .. }
            | Self::Call { .. }
            | Self::PureCall { .. }
            | Self::SpreadArg(_)
            | Self::MethodCall { .. }
            | Self::Map { .. }
            | Self::Sum { .. }
            | Self::IfLet { .. }
            | Self::Match { .. } => false,
        }
    }
}

/// One value-producing `match` arm in a runtime expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExprMatchArm {
    pub pattern: RuntimePattern,
    pub guard: Option<RuntimeExpr>,
    pub value: RuntimeExpr,
}

/// One field inside a runtime record expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFieldExpr {
    pub name: String,
    pub value: RuntimeExpr,
}

/// Unary operator supported by the Sans I/O expression evaluator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeUnaryOp {
    Not,
    Neg,
}

/// Binary operator supported by the Sans I/O expression evaluator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBinaryOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    And,
    Or,
}

#[derive(Debug, Eq)]
pub struct RuntimeEnv {
    scopes: Vec<RuntimeScope>,
    spare_scopes: Vec<RuntimeScope>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RuntimeScope {
    bindings: Vec<RuntimeBinding>,
}

/// Pure runtime program consumed by the minimal Sans I/O engine.

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeEvalError {
    #[error("unknown runtime binding `{0}`")]
    UnknownBinding(String),
    #[error("expected bool expression, found {0}")]
    ExpectedBool(String),
    #[error("expected integer expression, found {0}")]
    ExpectedInt(String),
    #[error("expected entity reference expression, found {0}")]
    ExpectedEntityRef(String),
    #[error("expected bracket sequence expression, found {0}")]
    ExpectedBracketSeq(String),
    #[error("field `{field}` does not exist on {value}")]
    MissingField { field: String, value: String },
    #[error("spread argument requires a tuple or bracket sequence, found {0}")]
    InvalidSpread(String),
    #[error("spread argument cannot be evaluated outside a call argument list")]
    SpreadOutsideCall,
    #[error(
        "runtime pure helper `{helper}` expected at most {max} fast-path argument(s), found {found}"
    )]
    TooManyPureArgs {
        helper: String,
        max: usize,
        found: usize,
    },
    #[error("runtime pure helper id {0} does not exist")]
    UnknownPureHelper(usize),
    #[error("operator `{op}` is not supported for {lhs} and {rhs}")]
    UnsupportedBinary {
        op: &'static str,
        lhs: String,
        rhs: String,
    },
    #[error("operator `{op}` is not supported for {value}")]
    UnsupportedUnary { op: &'static str, value: String },
    #[error("pure helper `{name}` cannot evaluate {reason}")]
    UnsupportedPure { name: String, reason: String },
    #[error("pattern did not match {0}")]
    PatternMismatch(String),
    #[error("pattern binds `{0}` more than once")]
    DuplicateBinding(String),
    #[error("break value can only be consumed by a value-producing loop")]
    BreakValueOutsideValueLoop,
    #[error("loop control `{0}` reached a non-loop runtime context")]
    MisplacedLoopControl(&'static str),
}

impl Default for RuntimeEnv {
    fn default() -> Self {
        Self {
            scopes: vec![RuntimeScope::default()],
            spare_scopes: Vec::new(),
        }
    }
}

impl Clone for RuntimeEnv {
    fn clone(&self) -> Self {
        Self {
            scopes: self.scopes.clone(),
            spare_scopes: Vec::new(),
        }
    }
}

impl PartialEq for RuntimeEnv {
    fn eq(&self, other: &Self) -> bool {
        self.scopes == other.scopes
    }
}

impl RuntimeEnv {
    pub fn push_scope(&mut self) {
        self.push_scope_with_capacity(0);
    }

    pub(crate) fn push_scope_with_capacity(&mut self, binding_capacity: usize) {
        let mut scope = self
            .spare_scopes
            .pop()
            .unwrap_or_else(|| RuntimeScope::with_capacity(binding_capacity));
        scope.clear();
        scope.reserve_bindings(binding_capacity);
        self.scopes.push(scope);
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            if let Some(mut scope) = self.scopes.pop() {
                scope.clear();
                self.spare_scopes.push(scope);
            }
        } else if let Some(scope) = self.scopes.last_mut() {
            scope.clear();
        }
    }

    pub fn set(&mut self, name: impl Into<String>, value: RuntimeValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.set(name.into(), value);
        }
    }

    pub(crate) fn set_ref(&mut self, name: &str, value: &RuntimeValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.set_ref(name, value);
        }
    }

    pub(crate) fn set_i64(&mut self, name: &str, value: i64) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.set_i64(name, value);
        }
    }

    pub fn set_root(&mut self, name: impl Into<String>, value: RuntimeValue) {
        if let Some(scope) = self.scopes.first_mut() {
            scope.set(name.into(), value);
        }
    }

    pub fn get(&self, name: &str) -> Option<&RuntimeValue> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    pub fn bind_all(&mut self, bindings: impl IntoIterator<Item = RuntimeBinding>) {
        for binding in bindings {
            self.set(binding.name, binding.value);
        }
    }

    pub(crate) fn bind_all_ref(&mut self, bindings: &[RuntimeBinding]) {
        for binding in bindings {
            self.set_ref(&binding.name, &binding.value);
        }
    }

    pub fn bind_all_root(&mut self, bindings: impl IntoIterator<Item = RuntimeBinding>) {
        for binding in bindings {
            self.set_root(binding.name, binding.value);
        }
    }

    pub fn bind_all_root_ref(&mut self, bindings: &[RuntimeBinding]) {
        if self.replace_root_bindings_ref(bindings) {
            return;
        }
        for binding in bindings {
            self.set_root_ref(&binding.name, &binding.value);
        }
    }

    fn replace_root_bindings_ref(&mut self, bindings: &[RuntimeBinding]) -> bool {
        if self.scopes.is_empty() {
            return bindings.is_empty();
        }
        self.scopes
            .first_mut()
            .is_some_and(|scope| scope.replace_binding_values_ref(bindings))
    }

    fn set_root_ref(&mut self, name: &str, value: &RuntimeValue) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        if let Some(scope) = self.scopes.first_mut() {
            scope.set_ref(name, value);
        }
    }

    pub(crate) fn replace_root_i64_bindings(&mut self, input_names: &[String], args: &[i64]) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_i64_bindings(input_names, args);
        }
    }

    pub(crate) fn replace_root_i32_bindings(&mut self, input_names: &[String], args: &[i32]) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_i32_bindings(input_names, args);
        }
    }

    pub(crate) fn replace_root_exact_int_bindings<T: RuntimeExactInteger>(
        &mut self,
        input_names: &[String],
        args: &[T],
    ) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_exact_int_bindings(input_names, args);
        }
    }

    pub(crate) fn replace_root_value_bindings_ref(
        &mut self,
        input_names: &[String],
        args: &[RuntimeValue],
    ) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_value_bindings_ref(input_names, args);
        }
    }
}

impl RuntimeScope {
    fn with_capacity(binding_capacity: usize) -> Self {
        Self {
            bindings: Vec::with_capacity(binding_capacity),
        }
    }

    fn reserve_bindings(&mut self, binding_capacity: usize) {
        let additional = binding_capacity.saturating_sub(self.bindings.capacity());
        self.bindings.reserve(additional);
    }

    fn set(&mut self, name: String, value: RuntimeValue) {
        if let Some(binding) = self
            .bindings
            .iter_mut()
            .find(|binding| binding.name == name)
        {
            binding.value = value;
        } else {
            self.bindings.push(RuntimeBinding { name, value });
        }
    }

    fn set_ref(&mut self, name: &str, value: &RuntimeValue) {
        if let Some(binding) = self
            .bindings
            .iter_mut()
            .find(|binding| binding.name == name)
        {
            binding.value = value.clone();
        } else {
            self.bindings.push(RuntimeBinding {
                name: name.to_owned(),
                value: value.clone(),
            });
        }
    }

    fn set_i64(&mut self, name: &str, value: i64) {
        if let Some(binding) = self
            .bindings
            .iter_mut()
            .find(|binding| binding.name == name)
        {
            binding.value = RuntimeValue::Int(value);
        } else {
            self.bindings.push(RuntimeBinding {
                name: name.to_owned(),
                value: RuntimeValue::Int(value),
            });
        }
    }

    fn get(&self, name: &str) -> Option<&RuntimeValue> {
        self.bindings
            .iter()
            .rev()
            .find(|binding| binding.name == name)
            .map(|binding| &binding.value)
    }

    fn clear(&mut self) {
        self.bindings.clear();
    }

    fn replace_i64_bindings(&mut self, input_names: &[String], args: &[i64]) {
        if self.bindings.len() == input_names.len()
            && self
                .bindings
                .iter()
                .zip(input_names)
                .all(|(binding, name)| binding.name == *name)
        {
            self.bindings
                .iter_mut()
                .zip(args.iter().copied())
                .for_each(|(binding, value)| binding.value = RuntimeValue::Int(value));
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args.iter().copied())
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: RuntimeValue::Int(value),
                }),
        );
    }

    fn replace_i32_bindings(&mut self, input_names: &[String], args: &[i32]) {
        if self.bindings.len() == input_names.len()
            && self
                .bindings
                .iter()
                .zip(input_names)
                .all(|(binding, name)| binding.name == *name)
        {
            self.bindings
                .iter_mut()
                .zip(args.iter().copied())
                .for_each(|(binding, value)| binding.value = RuntimeValue::Int(i64::from(value)));
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args.iter().copied())
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: RuntimeValue::Int(i64::from(value)),
                }),
        );
    }

    fn replace_exact_int_bindings<T: RuntimeExactInteger>(
        &mut self,
        input_names: &[String],
        args: &[T],
    ) {
        if self.bindings.len() == input_names.len()
            && self
                .bindings
                .iter()
                .zip(input_names)
                .all(|(binding, name)| binding.name == *name)
        {
            self.bindings
                .iter_mut()
                .zip(args.iter().copied())
                .for_each(|(binding, value)| binding.value = value.into_runtime_value());
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args.iter().copied())
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: value.into_runtime_value(),
                }),
        );
    }

    fn replace_value_bindings_ref(&mut self, input_names: &[String], args: &[RuntimeValue]) {
        if self.bindings.len() == input_names.len()
            && self
                .bindings
                .iter()
                .zip(input_names)
                .all(|(binding, name)| binding.name == *name)
        {
            self.bindings
                .iter_mut()
                .zip(args)
                .for_each(|(binding, value)| binding.value = value.clone());
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args)
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: value.clone(),
                }),
        );
    }

    fn replace_binding_values_ref(&mut self, bindings: &[RuntimeBinding]) -> bool {
        if self.bindings.len() != bindings.len() {
            return false;
        }
        if !self
            .bindings
            .iter()
            .zip(bindings)
            .all(|(current, next)| current.name == next.name)
        {
            return false;
        }
        self.bindings
            .iter_mut()
            .zip(bindings)
            .for_each(|(current, next)| current.value = next.value.clone());
        true
    }
}

impl RuntimePayload {
    pub const fn new(value: RuntimeValue) -> Self {
        Self(value)
    }

    pub const fn value(&self) -> &RuntimeValue {
        &self.0
    }

    pub fn into_value(self) -> RuntimeValue {
        self.0
    }

    pub fn label(&self) -> String {
        runtime_value_label(&self.0)
    }
}

impl From<RuntimeValue> for RuntimePayload {
    fn from(value: RuntimeValue) -> Self {
        Self(value)
    }
}

impl From<String> for RuntimePayload {
    fn from(value: String) -> Self {
        Self(RuntimeValue::String(value))
    }
}

impl From<&str> for RuntimePayload {
    fn from(value: &str) -> Self {
        Self(RuntimeValue::String(value.to_owned()))
    }
}

pub(crate) fn evaluate_unary(
    op: RuntimeUnaryOp,
    value: RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    match (op, value) {
        (RuntimeUnaryOp::Not, RuntimeValue::Bool(value)) => Ok(RuntimeValue::Bool(!value)),
        (RuntimeUnaryOp::Neg, RuntimeValue::Int(value)) => Ok(RuntimeValue::Int(-value)),
        (RuntimeUnaryOp::Neg, RuntimeValue::F32(value)) => {
            Ok(RuntimeValue::F32(RuntimeF32::from_f32(-value.to_f32())))
        }
        (RuntimeUnaryOp::Neg, RuntimeValue::F64(value)) => {
            Ok(RuntimeValue::F64(RuntimeF64::from_f64(-value.to_f64())))
        }
        (op, value) => Err(RuntimeEvalError::UnsupportedUnary {
            op: runtime_unary_op_label(op),
            value: runtime_value_label(&value),
        }),
    }
}

pub(crate) fn evaluate_binary(
    lhs: RuntimeValue,
    op: RuntimeBinaryOp,
    rhs: RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    match op {
        RuntimeBinaryOp::Eq => Ok(RuntimeValue::Bool(lhs == rhs)),
        RuntimeBinaryOp::Ne => Ok(RuntimeValue::Bool(lhs != rhs)),
        RuntimeBinaryOp::And => match (lhs, rhs) {
            (RuntimeValue::Bool(lhs), RuntimeValue::Bool(rhs)) => {
                Ok(RuntimeValue::Bool(lhs && rhs))
            }
            (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
        },
        RuntimeBinaryOp::Or => match (lhs, rhs) {
            (RuntimeValue::Bool(lhs), RuntimeValue::Bool(rhs)) => {
                Ok(RuntimeValue::Bool(lhs || rhs))
            }
            (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
        },
        RuntimeBinaryOp::Lt | RuntimeBinaryOp::Le | RuntimeBinaryOp::Gt | RuntimeBinaryOp::Ge => {
            match (lhs, rhs) {
                (RuntimeValue::Int(lhs), RuntimeValue::Int(rhs))
                | (RuntimeValue::ISize(lhs), RuntimeValue::ISize(rhs)) => {
                    Ok(RuntimeValue::Bool(compare_ordered(&lhs, op, &rhs)))
                }
                (RuntimeValue::I128(lhs), RuntimeValue::I128(rhs)) => {
                    Ok(RuntimeValue::Bool(compare_ordered(&lhs, op, &rhs)))
                }
                (RuntimeValue::UInt(lhs), RuntimeValue::UInt(rhs))
                | (RuntimeValue::USize(lhs), RuntimeValue::USize(rhs)) => {
                    Ok(RuntimeValue::Bool(compare_ordered(&lhs, op, &rhs)))
                }
                (RuntimeValue::U128(lhs), RuntimeValue::U128(rhs)) => {
                    Ok(RuntimeValue::Bool(compare_ordered(&lhs, op, &rhs)))
                }
                (RuntimeValue::F32(lhs), RuntimeValue::F32(rhs)) => Ok(RuntimeValue::Bool(
                    compare_float(&lhs.to_f32(), op, &rhs.to_f32()),
                )),
                (RuntimeValue::F64(lhs), RuntimeValue::F64(rhs)) => Ok(RuntimeValue::Bool(
                    compare_float(&lhs.to_f64(), op, &rhs.to_f64()),
                )),
                (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
            }
        }
        RuntimeBinaryOp::Add
        | RuntimeBinaryOp::Sub
        | RuntimeBinaryOp::Mul
        | RuntimeBinaryOp::Div => match (lhs, rhs) {
            (RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)) => {
                Ok(RuntimeValue::Int(evaluate_numeric_op(lhs, op, rhs)))
            }
            (RuntimeValue::I128(lhs), RuntimeValue::I128(rhs)) => {
                Ok(RuntimeValue::I128(evaluate_numeric_op(lhs, op, rhs)))
            }
            (RuntimeValue::ISize(lhs), RuntimeValue::ISize(rhs)) => {
                Ok(RuntimeValue::ISize(evaluate_numeric_op(lhs, op, rhs)))
            }
            (RuntimeValue::UInt(lhs), RuntimeValue::UInt(rhs)) => {
                Ok(RuntimeValue::UInt(evaluate_numeric_op(lhs, op, rhs)))
            }
            (RuntimeValue::U128(lhs), RuntimeValue::U128(rhs)) => {
                Ok(RuntimeValue::U128(evaluate_numeric_op(lhs, op, rhs)))
            }
            (RuntimeValue::USize(lhs), RuntimeValue::USize(rhs)) => {
                Ok(RuntimeValue::USize(evaluate_numeric_op(lhs, op, rhs)))
            }
            (RuntimeValue::F32(lhs), RuntimeValue::F32(rhs)) => {
                Ok(RuntimeValue::F32(evaluate_f32_op(lhs, op, rhs)))
            }
            (RuntimeValue::F64(lhs), RuntimeValue::F64(rhs)) => {
                Ok(RuntimeValue::F64(evaluate_f64_op(lhs, op, rhs)))
            }
            (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
        },
    }
}

fn compare_ordered<T: Ord>(lhs: &T, op: RuntimeBinaryOp, rhs: &T) -> bool {
    match op {
        RuntimeBinaryOp::Lt => lhs < rhs,
        RuntimeBinaryOp::Le => lhs <= rhs,
        RuntimeBinaryOp::Gt => lhs > rhs,
        RuntimeBinaryOp::Ge => lhs >= rhs,
        _ => unreachable!(),
    }
}

fn compare_float<T: PartialOrd>(lhs: &T, op: RuntimeBinaryOp, rhs: &T) -> bool {
    match op {
        RuntimeBinaryOp::Lt => lhs < rhs,
        RuntimeBinaryOp::Le => lhs <= rhs,
        RuntimeBinaryOp::Gt => lhs > rhs,
        RuntimeBinaryOp::Ge => lhs >= rhs,
        _ => unreachable!(),
    }
}

fn evaluate_f32_op(lhs: RuntimeF32, op: RuntimeBinaryOp, rhs: RuntimeF32) -> RuntimeF32 {
    RuntimeF32::from_f32(evaluate_numeric_op(lhs.to_f32(), op, rhs.to_f32()))
}

fn evaluate_f64_op(lhs: RuntimeF64, op: RuntimeBinaryOp, rhs: RuntimeF64) -> RuntimeF64 {
    RuntimeF64::from_f64(evaluate_numeric_op(lhs.to_f64(), op, rhs.to_f64()))
}

fn evaluate_numeric_op<T>(lhs: T, op: RuntimeBinaryOp, rhs: T) -> T
where
    T: Copy
        + std::ops::Add<Output = T>
        + std::ops::Sub<Output = T>
        + std::ops::Mul<Output = T>
        + std::ops::Div<Output = T>,
{
    match op {
        RuntimeBinaryOp::Add => lhs + rhs,
        RuntimeBinaryOp::Sub => lhs - rhs,
        RuntimeBinaryOp::Mul => lhs * rhs,
        RuntimeBinaryOp::Div => lhs / rhs,
        _ => unreachable!(),
    }
}

pub(crate) fn sum_i64_sequence_ref(items: &[RuntimeValue]) -> Result<i64, RuntimeEvalError> {
    items.iter().try_fold(0_i64, |acc, item| match item {
        RuntimeValue::Int(value) | RuntimeValue::ISize(value) => Ok(acc + value),
        RuntimeValue::I128(value) => i64::try_from(*value).map(|value| acc + value).map_err(|_| {
            RuntimeEvalError::UnsupportedBinary {
                op: "+",
                lhs: "int".to_owned(),
                rhs: runtime_value_label(item),
            }
        }),
        RuntimeValue::UInt(value) | RuntimeValue::USize(value) => i64::try_from(*value)
            .map(|value| acc + value)
            .map_err(|_| RuntimeEvalError::UnsupportedBinary {
                op: "+",
                lhs: "int".to_owned(),
                rhs: runtime_value_label(item),
            }),
        RuntimeValue::U128(value) => i64::try_from(*value).map(|value| acc + value).map_err(|_| {
            RuntimeEvalError::UnsupportedBinary {
                op: "+",
                lhs: "int".to_owned(),
                rhs: runtime_value_label(item),
            }
        }),
        value => Err(RuntimeEvalError::UnsupportedBinary {
            op: "+",
            lhs: "int".to_owned(),
            rhs: runtime_value_label(value),
        }),
    })
}

pub(crate) fn materialize_i64_sequence(items: Vec<i64>) -> Vec<RuntimeValue> {
    items.into_iter().map(RuntimeValue::Int).collect()
}

pub fn runtime_sequence_values(values: Vec<RuntimeValue>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::values(values))
}

pub fn runtime_sequence_from_literal_values(values: Vec<RuntimeValue>) -> RuntimeValue {
    match values.first() {
        Some(RuntimeValue::Unit)
            if values
                .iter()
                .all(|value| matches!(value, RuntimeValue::Unit)) =>
        {
            runtime_sequence_dense_units(values.len())
        }
        Some(RuntimeValue::Bool(_)) => collect_dense_or_values(
            values,
            take_bool_value,
            RuntimeValue::Bool,
            runtime_sequence_dense_bool,
        ),
        Some(RuntimeValue::Int(_)) => collect_dense_or_values(
            values,
            take_int_value,
            RuntimeValue::Int,
            runtime_sequence_dense_i64,
        ),
        Some(RuntimeValue::I128(_)) => collect_dense_or_values(
            values,
            take_i128_value,
            RuntimeValue::I128,
            runtime_sequence_dense_i128,
        ),
        Some(RuntimeValue::ISize(_)) => collect_dense_or_values(
            values,
            take_isize_value,
            RuntimeValue::ISize,
            runtime_sequence_dense_isize,
        ),
        Some(RuntimeValue::UInt(_)) => collect_dense_or_values(
            values,
            take_uint_value,
            RuntimeValue::UInt,
            runtime_sequence_dense_u64,
        ),
        Some(RuntimeValue::U128(_)) => collect_dense_or_values(
            values,
            take_u128_value,
            RuntimeValue::U128,
            runtime_sequence_dense_u128,
        ),
        Some(RuntimeValue::USize(_)) => collect_dense_or_values(
            values,
            take_usize_value,
            RuntimeValue::USize,
            runtime_sequence_dense_usize,
        ),
        Some(RuntimeValue::F32(_)) => collect_dense_or_values(
            values,
            take_f32_value,
            RuntimeValue::F32,
            runtime_sequence_dense_f32,
        ),
        Some(RuntimeValue::F64(_)) => collect_dense_or_values(
            values,
            take_f64_value,
            RuntimeValue::F64,
            runtime_sequence_dense_f64,
        ),
        Some(RuntimeValue::Char(_)) => collect_dense_or_values(
            values,
            take_char_value,
            RuntimeValue::Char,
            runtime_sequence_dense_chars,
        ),
        Some(RuntimeValue::Duration(_)) => collect_dense_or_values(
            values,
            take_duration_value,
            RuntimeValue::Duration,
            runtime_sequence_dense_durations,
        ),
        Some(RuntimeValue::String(_)) => collect_dense_or_values(
            values,
            take_string_value,
            RuntimeValue::String,
            runtime_sequence_dense_strings,
        ),
        Some(RuntimeValue::Float(_)) => collect_dense_or_values(
            values,
            take_float_value,
            RuntimeValue::Float,
            runtime_sequence_dense_float_literals,
        ),
        Some(RuntimeValue::EntityRef(_)) => collect_dense_or_values(
            values,
            take_entity_ref_value,
            RuntimeValue::EntityRef,
            runtime_sequence_dense_entity_refs,
        ),
        _ => runtime_sequence_values(values),
    }
}

fn collect_dense_or_values<T>(
    values: Vec<RuntimeValue>,
    extract: impl Fn(RuntimeValue) -> Result<T, RuntimeValue>,
    rebuild: impl Fn(T) -> RuntimeValue,
    wrap: impl Fn(Vec<T>) -> RuntimeValue,
) -> RuntimeValue {
    let mut dense = Vec::with_capacity(values.len());
    let mut iter = values.into_iter();
    while let Some(value) = iter.next() {
        match extract(value) {
            Ok(value) => dense.push(value),
            Err(value) => {
                let mut fallback = dense.into_iter().map(rebuild).collect::<Vec<_>>();
                fallback.push(value);
                fallback.extend(iter);
                return runtime_sequence_values(fallback);
            }
        }
    }
    wrap(dense)
}

fn take_bool_value(value: RuntimeValue) -> Result<bool, RuntimeValue> {
    match value {
        RuntimeValue::Bool(value) => Ok(value),
        value => Err(value),
    }
}

fn take_int_value(value: RuntimeValue) -> Result<i64, RuntimeValue> {
    match value {
        RuntimeValue::Int(value) => Ok(value),
        value => Err(value),
    }
}

fn take_i128_value(value: RuntimeValue) -> Result<i128, RuntimeValue> {
    match value {
        RuntimeValue::I128(value) => Ok(value),
        value => Err(value),
    }
}

fn take_isize_value(value: RuntimeValue) -> Result<i64, RuntimeValue> {
    match value {
        RuntimeValue::ISize(value) => Ok(value),
        value => Err(value),
    }
}

fn take_uint_value(value: RuntimeValue) -> Result<u64, RuntimeValue> {
    match value {
        RuntimeValue::UInt(value) => Ok(value),
        value => Err(value),
    }
}

fn take_u128_value(value: RuntimeValue) -> Result<u128, RuntimeValue> {
    match value {
        RuntimeValue::U128(value) => Ok(value),
        value => Err(value),
    }
}

fn take_usize_value(value: RuntimeValue) -> Result<u64, RuntimeValue> {
    match value {
        RuntimeValue::USize(value) => Ok(value),
        value => Err(value),
    }
}

fn take_f32_value(value: RuntimeValue) -> Result<RuntimeF32, RuntimeValue> {
    match value {
        RuntimeValue::F32(value) => Ok(value),
        value => Err(value),
    }
}

fn take_f64_value(value: RuntimeValue) -> Result<RuntimeF64, RuntimeValue> {
    match value {
        RuntimeValue::F64(value) => Ok(value),
        value => Err(value),
    }
}

fn take_char_value(value: RuntimeValue) -> Result<char, RuntimeValue> {
    match value {
        RuntimeValue::Char(value) => Ok(value),
        value => Err(value),
    }
}

fn take_duration_value(value: RuntimeValue) -> Result<LogicalDuration, RuntimeValue> {
    match value {
        RuntimeValue::Duration(value) => Ok(value),
        value => Err(value),
    }
}

fn take_string_value(value: RuntimeValue) -> Result<String, RuntimeValue> {
    match value {
        RuntimeValue::String(value) => Ok(value),
        value => Err(value),
    }
}

fn take_float_value(value: RuntimeValue) -> Result<String, RuntimeValue> {
    match value {
        RuntimeValue::Float(value) => Ok(value),
        value => Err(value),
    }
}

fn take_entity_ref_value(value: RuntimeValue) -> Result<String, RuntimeValue> {
    match value {
        RuntimeValue::EntityRef(value) => Ok(value),
        value => Err(value),
    }
}

pub fn runtime_sequence_repeat_value(value: &RuntimeValue, len: usize) -> RuntimeValue {
    match value {
        RuntimeValue::Unit => runtime_sequence_dense_units(len),
        RuntimeValue::Bool(value) => runtime_sequence_dense_bool(vec![*value; len]),
        RuntimeValue::Int(value) => runtime_sequence_dense_i64(vec![*value; len]),
        RuntimeValue::I128(value) => runtime_sequence_dense_i128(vec![*value; len]),
        RuntimeValue::ISize(value) => runtime_sequence_dense_isize(vec![*value; len]),
        RuntimeValue::UInt(value) => runtime_sequence_dense_u64(vec![*value; len]),
        RuntimeValue::U128(value) => runtime_sequence_dense_u128(vec![*value; len]),
        RuntimeValue::USize(value) => runtime_sequence_dense_usize(vec![*value; len]),
        RuntimeValue::F32(value) => runtime_sequence_dense_f32(vec![*value; len]),
        RuntimeValue::F64(value) => runtime_sequence_dense_f64(vec![*value; len]),
        RuntimeValue::Char(value) => runtime_sequence_dense_chars(vec![*value; len]),
        RuntimeValue::Duration(value) => runtime_sequence_dense_durations(vec![*value; len]),
        RuntimeValue::String(value) => runtime_sequence_dense_strings(vec![value.clone(); len]),
        RuntimeValue::Float(value) => {
            runtime_sequence_dense_float_literals(vec![value.clone(); len])
        }
        RuntimeValue::EntityRef(value) => {
            runtime_sequence_dense_entity_refs(vec![value.clone(); len])
        }
        value => runtime_sequence_values(vec![value.clone(); len]),
    }
}

pub fn runtime_sequence_dense_units(len: usize) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_units(len))
}

pub fn runtime_sequence_dense_i64(values: Vec<i64>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i64(values))
}

pub fn runtime_sequence_dense_i128(values: Vec<i128>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i128(values))
}

pub fn runtime_sequence_dense_isize(values: Vec<i64>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_isize(values))
}

pub fn runtime_sequence_dense_i8(values: Vec<i8>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i8(values))
}

pub fn runtime_sequence_dense_i16(values: Vec<i16>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i16(values))
}

pub fn runtime_sequence_dense_i32(values: Vec<i32>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i32(values))
}

pub fn runtime_sequence_dense_u8(values: Vec<u8>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u8(values))
}

pub fn runtime_sequence_dense_u16(values: Vec<u16>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u16(values))
}

pub fn runtime_sequence_dense_u32(values: Vec<u32>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u32(values))
}

pub fn runtime_sequence_dense_u64(values: Vec<u64>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u64(values))
}

pub fn runtime_sequence_dense_u128(values: Vec<u128>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u128(values))
}

pub fn runtime_sequence_dense_usize(values: Vec<u64>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_usize(values))
}

pub fn runtime_sequence_dense_f32(values: Vec<RuntimeF32>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_f32(values))
}

pub fn runtime_sequence_dense_f64(values: Vec<RuntimeF64>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_f64(values))
}

pub fn runtime_sequence_dense_bool(values: Vec<bool>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_bool(values))
}

pub fn runtime_sequence_dense_bytes(values: Vec<u8>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_bytes(values))
}

pub fn runtime_sequence_dense_chars(values: Vec<char>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_chars(values))
}

pub fn runtime_sequence_dense_durations(values: Vec<LogicalDuration>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_durations(values))
}

pub fn runtime_sequence_dense_strings(values: Vec<String>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_strings(values))
}

pub fn runtime_sequence_dense_float_literals(values: Vec<String>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_float_literals(values))
}

pub fn runtime_sequence_dense_entity_refs(values: Vec<String>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_entity_refs(values))
}

pub(crate) fn runtime_value_into_sequence_values(
    value: RuntimeValue,
) -> Result<Vec<RuntimeValue>, RuntimeValue> {
    match value {
        RuntimeValue::Seq(seq) => Ok(seq.into_values()),
        RuntimeValue::Tuple(values) => Ok(values),
        value => Err(value),
    }
}

fn unsupported_binary(
    op: RuntimeBinaryOp,
    lhs: &RuntimeValue,
    rhs: &RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    Err(RuntimeEvalError::UnsupportedBinary {
        op: runtime_binary_op_label(op),
        lhs: runtime_value_label(lhs),
        rhs: runtime_value_label(rhs),
    })
}

pub(crate) fn runtime_unary_op_label(op: RuntimeUnaryOp) -> &'static str {
    match op {
        RuntimeUnaryOp::Not => "!",
        RuntimeUnaryOp::Neg => "-",
    }
}

pub(crate) fn runtime_binary_op_label(op: RuntimeBinaryOp) -> &'static str {
    match op {
        RuntimeBinaryOp::Eq => "==",
        RuntimeBinaryOp::Ne => "!=",
        RuntimeBinaryOp::Lt => "<",
        RuntimeBinaryOp::Le => "<=",
        RuntimeBinaryOp::Gt => ">",
        RuntimeBinaryOp::Ge => ">=",
        RuntimeBinaryOp::Add => "+",
        RuntimeBinaryOp::Sub => "-",
        RuntimeBinaryOp::Mul => "*",
        RuntimeBinaryOp::Div => "/",
        RuntimeBinaryOp::And => "&&",
        RuntimeBinaryOp::Or => "||",
    }
}

pub(crate) fn expr_runtime_label(expr: &RuntimeExpr) -> String {
    match expr {
        RuntimeExpr::Value(value) => runtime_value_label(value),
        RuntimeExpr::Local(name) => name.clone(),
        RuntimeExpr::EntityRef(target) => format!("@{target}"),
        RuntimeExpr::Let { name, .. } => format!("let {name}"),
        RuntimeExpr::Tuple(items) => format!("tuple/{}", items.len()),
        RuntimeExpr::BracketSeq(items) => format!("bracket_seq/{}", items.len()),
        RuntimeExpr::RepeatSeq { len, .. } => format!("repeat_seq/{len}"),
        RuntimeExpr::Record(fields) => format!("record/{}", fields.len()),
        RuntimeExpr::Variant { name, .. } => format!(".{name}"),
        RuntimeExpr::Field { field, .. } => format!(".{field}"),
        RuntimeExpr::Call { callee, .. } => format!("{callee}()"),
        RuntimeExpr::PureCall { helper, .. } => format!("pure#{}()", helper.0),
        RuntimeExpr::SpreadArg(expr) => format!("{}...", expr_runtime_label(expr)),
        RuntimeExpr::MethodCall { method, .. } => format!(".{method}()"),
        RuntimeExpr::Map { .. } => "map".to_owned(),
        RuntimeExpr::Sum { .. } => "sum".to_owned(),
        RuntimeExpr::Unary { op, .. } => runtime_unary_op_label(*op).to_owned(),
        RuntimeExpr::Binary { op, .. } => runtime_binary_op_label(*op).to_owned(),
        RuntimeExpr::If { .. } => "if".to_owned(),
        RuntimeExpr::IfLet { .. } => "if let".to_owned(),
        RuntimeExpr::Match { .. } => "match".to_owned(),
    }
}

pub(crate) fn runtime_value_label(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "()".to_owned(),
        RuntimeValue::Bool(value) => value.to_string(),
        RuntimeValue::Int(value) | RuntimeValue::ISize(value) => value.to_string(),
        RuntimeValue::I128(value) => value.to_string(),
        RuntimeValue::UInt(value) | RuntimeValue::USize(value) => value.to_string(),
        RuntimeValue::U128(value) => value.to_string(),
        RuntimeValue::F32(value) => value.to_f32().to_string(),
        RuntimeValue::F64(value) => value.to_f64().to_string(),
        RuntimeValue::Float(value)
        | RuntimeValue::String(value)
        | RuntimeValue::EntityRef(value) => value.clone(),
        RuntimeValue::Char(value) => value.to_string(),
        RuntimeValue::Duration(value) => format!("{}ns", value.as_nanos()),
        RuntimeValue::Tuple(values) => format!("tuple/{}", values.len()),
        RuntimeValue::Seq(seq) => match seq {
            RuntimeSeq::Values(values) => format!("seq/values/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::Units(len)) => format!("seq/units/{len}"),
            RuntimeSeq::Dense(DenseSeq::I8(values)) => format!("seq/i8/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::I16(values)) => format!("seq/i16/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::I32(values)) => format!("seq/i32/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::I64(values)) => format!("seq/i64/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::I128(values)) => format!("seq/i128/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::ISize(values)) => format!("seq/isize/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::U8(values)) => format!("seq/u8/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::U16(values)) => format!("seq/u16/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::U32(values)) => format!("seq/u32/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::U64(values)) => format!("seq/u64/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::U128(values)) => format!("seq/u128/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::USize(values)) => format!("seq/usize/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::F32(values)) => format!("seq/f32/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::F64(values)) => format!("seq/f64/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::Bool(values)) => format!("seq/bool/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::Bytes(values)) => format!("seq/bytes/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::Chars(values)) => format!("seq/chars/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::Durations(values)) => {
                format!("seq/durations/{}", values.len())
            }
            RuntimeSeq::Dense(DenseSeq::Strings(values)) => format!("seq/strings/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::FloatLiterals(values)) => {
                format!("seq/float_literals/{}", values.len())
            }
            RuntimeSeq::Dense(DenseSeq::EntityRefs(values)) => {
                format!("seq/entity_refs/{}", values.len())
            }
        },
        RuntimeValue::Record(fields) => format!("record/{}", fields.len()),
        RuntimeValue::Variant { name, payload, .. } => {
            if payload.is_some() {
                format!(".{name}(...)")
            } else {
                format!(".{name}")
            }
        }
    }
}
