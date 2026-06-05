use crate::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
use crate::pattern::RuntimePattern;
use crate::plan::{RuntimePureHelperId, RuntimePureInputType, RuntimePureOutputType};
use crate::time::LogicalDuration;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeBinding {
    pub name: String,
    pub value: RuntimeValue,
}

/// Structured payload exchanged at the host/runtime boundary.
///
/// Payloads intentionally retain `RuntimeValue` shape instead of collapsing
/// source and stream items to debug strings. Hosts may still display `label()`
/// for logs, but replay and downstream runtime consumers keep typed data.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimePayload(pub RuntimeValue);

/// Deterministic value domain used by the Sans I/O flow runtime.
///
/// Typed floats use Rust's native `f32`/`f64` values. Exact bit identity is an
/// explicit operation rather than language equality.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeValue {
    Unit,
    Bool(bool),
    Int(RuntimeInt),
    UInt(RuntimeUInt),
    F32(f32),
    F64(f64),
    MatrixF32(DenseMatrixF32),
    MatrixF64(DenseMatrixF64),
    TensorF32(DenseTensorF32),
    TensorF64(DenseTensorF64),
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
}

impl fmt::Display for RuntimeUInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

/// Runtime call target after syntax lowering.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeCallTarget {
    Intrinsic(RuntimeIntrinsic),
    Named(String),
}

impl RuntimeCallTarget {
    pub fn from_label(label: impl Into<String>) -> Self {
        let label = label.into();
        RuntimeIntrinsic::from_label(&label).map_or(Self::Named(label), Self::Intrinsic)
    }

    pub const fn intrinsic(intrinsic: RuntimeIntrinsic) -> Self {
        Self::Intrinsic(intrinsic)
    }

    pub fn named(label: impl Into<String>) -> Self {
        Self::Named(label.into())
    }

    pub const fn as_intrinsic(&self) -> Option<RuntimeIntrinsic> {
        match self {
            Self::Intrinsic(intrinsic) => Some(*intrinsic),
            Self::Named(_) => None,
        }
    }

    pub fn as_label(&self) -> &str {
        match self {
            Self::Intrinsic(intrinsic) => intrinsic.as_label(),
            Self::Named(label) => label,
        }
    }
}

impl fmt::Display for RuntimeCallTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_label())
    }
}

/// Built-in runtime calls that use typed dispatch instead of string matching.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIntrinsic {
    Add,
    StdF32Abs,
    StdF32Floor,
    StdF32Ceil,
    StdF32Round,
    StdF32Trunc,
    StdF32Fract,
    StdF32Sqrt,
    StdF32Sin,
    StdF32Cos,
    StdF32Tan,
    StdF32Exp,
    StdF32Exp2,
    StdF32Ln,
    StdF32Log2,
    StdF32Log10,
    StdF32Powf,
    StdF32Atan2,
    StdF32MulAdd,
    StdF32IsNan,
    StdF32IsInfinite,
    StdF32IsFinite,
    StdF32IsSignPositive,
    StdF32IsSignNegative,
    StdF32ToBits,
    StdF32FromBits,
    StdF32ToF64,
    StdF64Abs,
    StdF64Floor,
    StdF64Ceil,
    StdF64Round,
    StdF64Trunc,
    StdF64Fract,
    StdF64Sqrt,
    StdF64Sin,
    StdF64Cos,
    StdF64Tan,
    StdF64Exp,
    StdF64Exp2,
    StdF64Ln,
    StdF64Log2,
    StdF64Log10,
    StdF64Powf,
    StdF64Atan2,
    StdF64MulAdd,
    StdF64IsNan,
    StdF64IsInfinite,
    StdF64IsFinite,
    StdF64IsSignPositive,
    StdF64IsSignNegative,
    StdF64ToBits,
    StdF64FromBits,
    StdF64ToF32,
    MathMatmulF32,
    MathMatrixAddF32,
    MathTensorAddF32,
    MathMatmulF64,
    MathMatrixAddF64,
    MathTensorAddF64,
    PathSave,
    PathAsset,
    PathTemp,
    PathExport,
}

impl RuntimeIntrinsic {
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "add" => Some(Self::Add),
            "std.f32.abs" => Some(Self::StdF32Abs),
            "std.f32.floor" => Some(Self::StdF32Floor),
            "std.f32.ceil" => Some(Self::StdF32Ceil),
            "std.f32.round" => Some(Self::StdF32Round),
            "std.f32.trunc" => Some(Self::StdF32Trunc),
            "std.f32.fract" => Some(Self::StdF32Fract),
            "std.f32.sqrt" => Some(Self::StdF32Sqrt),
            "std.f32.sin" => Some(Self::StdF32Sin),
            "std.f32.cos" => Some(Self::StdF32Cos),
            "std.f32.tan" => Some(Self::StdF32Tan),
            "std.f32.exp" => Some(Self::StdF32Exp),
            "std.f32.exp2" => Some(Self::StdF32Exp2),
            "std.f32.ln" => Some(Self::StdF32Ln),
            "std.f32.log2" => Some(Self::StdF32Log2),
            "std.f32.log10" => Some(Self::StdF32Log10),
            "std.f32.powf" => Some(Self::StdF32Powf),
            "std.f32.atan2" => Some(Self::StdF32Atan2),
            "std.f32.mul_add" => Some(Self::StdF32MulAdd),
            "std.f32.is_nan" => Some(Self::StdF32IsNan),
            "std.f32.is_infinite" => Some(Self::StdF32IsInfinite),
            "std.f32.is_finite" => Some(Self::StdF32IsFinite),
            "std.f32.is_sign_positive" => Some(Self::StdF32IsSignPositive),
            "std.f32.is_sign_negative" => Some(Self::StdF32IsSignNegative),
            "std.f32.to_bits" => Some(Self::StdF32ToBits),
            "std.f32.from_bits" => Some(Self::StdF32FromBits),
            "std.f32.to_f64" => Some(Self::StdF32ToF64),
            "std.f64.abs" => Some(Self::StdF64Abs),
            "std.f64.floor" => Some(Self::StdF64Floor),
            "std.f64.ceil" => Some(Self::StdF64Ceil),
            "std.f64.round" => Some(Self::StdF64Round),
            "std.f64.trunc" => Some(Self::StdF64Trunc),
            "std.f64.fract" => Some(Self::StdF64Fract),
            "std.f64.sqrt" => Some(Self::StdF64Sqrt),
            "std.f64.sin" => Some(Self::StdF64Sin),
            "std.f64.cos" => Some(Self::StdF64Cos),
            "std.f64.tan" => Some(Self::StdF64Tan),
            "std.f64.exp" => Some(Self::StdF64Exp),
            "std.f64.exp2" => Some(Self::StdF64Exp2),
            "std.f64.ln" => Some(Self::StdF64Ln),
            "std.f64.log2" => Some(Self::StdF64Log2),
            "std.f64.log10" => Some(Self::StdF64Log10),
            "std.f64.powf" => Some(Self::StdF64Powf),
            "std.f64.atan2" => Some(Self::StdF64Atan2),
            "std.f64.mul_add" => Some(Self::StdF64MulAdd),
            "std.f64.is_nan" => Some(Self::StdF64IsNan),
            "std.f64.is_infinite" => Some(Self::StdF64IsInfinite),
            "std.f64.is_finite" => Some(Self::StdF64IsFinite),
            "std.f64.is_sign_positive" => Some(Self::StdF64IsSignPositive),
            "std.f64.is_sign_negative" => Some(Self::StdF64IsSignNegative),
            "std.f64.to_bits" => Some(Self::StdF64ToBits),
            "std.f64.from_bits" => Some(Self::StdF64FromBits),
            "std.f64.to_f32" => Some(Self::StdF64ToF32),
            "math.matmul_f32" => Some(Self::MathMatmulF32),
            "math.matrix_add_f32" => Some(Self::MathMatrixAddF32),
            "math.tensor_add_f32" => Some(Self::MathTensorAddF32),
            "math.matmul_f64" => Some(Self::MathMatmulF64),
            "math.matrix_add_f64" => Some(Self::MathMatrixAddF64),
            "math.tensor_add_f64" => Some(Self::MathTensorAddF64),
            "path.save" => Some(Self::PathSave),
            "path.asset" => Some(Self::PathAsset),
            "path.temp" => Some(Self::PathTemp),
            "path.export" => Some(Self::PathExport),
            _ => None,
        }
    }

    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::StdF32Abs => "std.f32.abs",
            Self::StdF32Floor => "std.f32.floor",
            Self::StdF32Ceil => "std.f32.ceil",
            Self::StdF32Round => "std.f32.round",
            Self::StdF32Trunc => "std.f32.trunc",
            Self::StdF32Fract => "std.f32.fract",
            Self::StdF32Sqrt => "std.f32.sqrt",
            Self::StdF32Sin => "std.f32.sin",
            Self::StdF32Cos => "std.f32.cos",
            Self::StdF32Tan => "std.f32.tan",
            Self::StdF32Exp => "std.f32.exp",
            Self::StdF32Exp2 => "std.f32.exp2",
            Self::StdF32Ln => "std.f32.ln",
            Self::StdF32Log2 => "std.f32.log2",
            Self::StdF32Log10 => "std.f32.log10",
            Self::StdF32Powf => "std.f32.powf",
            Self::StdF32Atan2 => "std.f32.atan2",
            Self::StdF32MulAdd => "std.f32.mul_add",
            Self::StdF32IsNan => "std.f32.is_nan",
            Self::StdF32IsInfinite => "std.f32.is_infinite",
            Self::StdF32IsFinite => "std.f32.is_finite",
            Self::StdF32IsSignPositive => "std.f32.is_sign_positive",
            Self::StdF32IsSignNegative => "std.f32.is_sign_negative",
            Self::StdF32ToBits => "std.f32.to_bits",
            Self::StdF32FromBits => "std.f32.from_bits",
            Self::StdF32ToF64 => "std.f32.to_f64",
            Self::StdF64Abs => "std.f64.abs",
            Self::StdF64Floor => "std.f64.floor",
            Self::StdF64Ceil => "std.f64.ceil",
            Self::StdF64Round => "std.f64.round",
            Self::StdF64Trunc => "std.f64.trunc",
            Self::StdF64Fract => "std.f64.fract",
            Self::StdF64Sqrt => "std.f64.sqrt",
            Self::StdF64Sin => "std.f64.sin",
            Self::StdF64Cos => "std.f64.cos",
            Self::StdF64Tan => "std.f64.tan",
            Self::StdF64Exp => "std.f64.exp",
            Self::StdF64Exp2 => "std.f64.exp2",
            Self::StdF64Ln => "std.f64.ln",
            Self::StdF64Log2 => "std.f64.log2",
            Self::StdF64Log10 => "std.f64.log10",
            Self::StdF64Powf => "std.f64.powf",
            Self::StdF64Atan2 => "std.f64.atan2",
            Self::StdF64MulAdd => "std.f64.mul_add",
            Self::StdF64IsNan => "std.f64.is_nan",
            Self::StdF64IsInfinite => "std.f64.is_infinite",
            Self::StdF64IsFinite => "std.f64.is_finite",
            Self::StdF64IsSignPositive => "std.f64.is_sign_positive",
            Self::StdF64IsSignNegative => "std.f64.is_sign_negative",
            Self::StdF64ToBits => "std.f64.to_bits",
            Self::StdF64FromBits => "std.f64.from_bits",
            Self::StdF64ToF32 => "std.f64.to_f32",
            Self::MathMatmulF32 => "math.matmul_f32",
            Self::MathMatrixAddF32 => "math.matrix_add_f32",
            Self::MathTensorAddF32 => "math.tensor_add_f32",
            Self::MathMatmulF64 => "math.matmul_f64",
            Self::MathMatrixAddF64 => "math.matrix_add_f64",
            Self::MathTensorAddF64 => "math.tensor_add_f64",
            Self::PathSave => "path.save",
            Self::PathAsset => "path.asset",
            Self::PathTemp => "path.temp",
            Self::PathExport => "path.export",
        }
    }

    pub const fn path_space(self) -> Option<&'static str> {
        match self {
            Self::PathSave => Some("save"),
            Self::PathAsset => Some("asset"),
            Self::PathTemp => Some("temp"),
            Self::PathExport => Some("export"),
            Self::Add
            | Self::StdF32Abs
            | Self::StdF32Floor
            | Self::StdF32Ceil
            | Self::StdF32Round
            | Self::StdF32Trunc
            | Self::StdF32Fract
            | Self::StdF32Sqrt
            | Self::StdF32Sin
            | Self::StdF32Cos
            | Self::StdF32Tan
            | Self::StdF32Exp
            | Self::StdF32Exp2
            | Self::StdF32Ln
            | Self::StdF32Log2
            | Self::StdF32Log10
            | Self::StdF32Powf
            | Self::StdF32Atan2
            | Self::StdF32MulAdd
            | Self::StdF32IsNan
            | Self::StdF32IsInfinite
            | Self::StdF32IsFinite
            | Self::StdF32IsSignPositive
            | Self::StdF32IsSignNegative
            | Self::StdF32ToBits
            | Self::StdF32FromBits
            | Self::StdF32ToF64
            | Self::StdF64Abs
            | Self::StdF64Floor
            | Self::StdF64Ceil
            | Self::StdF64Round
            | Self::StdF64Trunc
            | Self::StdF64Fract
            | Self::StdF64Sqrt
            | Self::StdF64Sin
            | Self::StdF64Cos
            | Self::StdF64Tan
            | Self::StdF64Exp
            | Self::StdF64Exp2
            | Self::StdF64Ln
            | Self::StdF64Log2
            | Self::StdF64Log10
            | Self::StdF64Powf
            | Self::StdF64Atan2
            | Self::StdF64MulAdd
            | Self::StdF64IsNan
            | Self::StdF64IsInfinite
            | Self::StdF64IsFinite
            | Self::StdF64IsSignPositive
            | Self::StdF64IsSignNegative
            | Self::StdF64ToBits
            | Self::StdF64FromBits
            | Self::StdF64ToF32
            | Self::MathMatmulF32
            | Self::MathMatrixAddF32
            | Self::MathTensorAddF32
            | Self::MathMatmulF64
            | Self::MathMatrixAddF64
            | Self::MathTensorAddF64 => None,
        }
    }
}

impl RuntimeValue {
    pub const fn i8(value: i8) -> Self {
        Self::Int(RuntimeInt::i8(value))
    }

    pub const fn i16(value: i16) -> Self {
        Self::Int(RuntimeInt::i16(value))
    }

    pub const fn i32(value: i32) -> Self {
        Self::Int(RuntimeInt::i32(value))
    }

    pub const fn i64(value: i64) -> Self {
        Self::Int(RuntimeInt::i64(value))
    }

    pub const fn i128(value: i128) -> Self {
        Self::Int(RuntimeInt::i128(value))
    }

    pub const fn isize(value: i64) -> Self {
        Self::Int(RuntimeInt::isize(value))
    }

    pub const fn u8(value: u8) -> Self {
        Self::UInt(RuntimeUInt::u8(value))
    }

    pub const fn u16(value: u16) -> Self {
        Self::UInt(RuntimeUInt::u16(value))
    }

    pub const fn u32(value: u32) -> Self {
        Self::UInt(RuntimeUInt::u32(value))
    }

    pub const fn u64(value: u64) -> Self {
        Self::UInt(RuntimeUInt::u64(value))
    }

    pub const fn u128(value: u128) -> Self {
        Self::UInt(RuntimeUInt::u128(value))
    }

    pub const fn usize(value: u64) -> Self {
        Self::UInt(RuntimeUInt::usize(value))
    }

    pub const fn f32(value: f32) -> Self {
        Self::F32(value)
    }

    pub const fn f64(value: f64) -> Self {
        Self::F64(value)
    }

    pub fn matrix_f32(value: DenseMatrixF32) -> Self {
        Self::MatrixF32(value)
    }

    pub fn matrix_f64(value: DenseMatrixF64) -> Self {
        Self::MatrixF64(value)
    }

    pub fn tensor_f32(value: DenseTensorF32) -> Self {
        Self::TensorF32(value)
    }

    pub fn tensor_f64(value: DenseTensorF64) -> Self {
        Self::TensorF64(value)
    }
}

/// Storage strategy for runtime sequence values.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeSeq {
    Values(Vec<RuntimeValue>),
    Dense(DenseSeq),
    TupleColumns(TupleSeq),
    RecordColumns(RecordSeq),
}

/// Columnar storage for a sequence of homogeneous tuple values.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TupleSeq {
    len: usize,
    columns: Vec<RuntimeSeq>,
}

impl TupleSeq {
    pub fn new(len: usize, columns: Vec<RuntimeSeq>) -> Result<Self, RuntimeSeqError> {
        if let Some((ordinal, actual)) = columns
            .iter()
            .enumerate()
            .find_map(|(ordinal, column)| (column.len() != len).then_some((ordinal, column.len())))
        {
            return Err(RuntimeSeqError::ColumnLength {
                ordinal,
                expected: len,
                actual,
            });
        }
        Ok(Self { len, columns })
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn columns(&self) -> &[RuntimeSeq] {
        &self.columns
    }

    pub fn column(&self, ordinal: usize) -> Option<&RuntimeSeq> {
        self.columns.get(ordinal)
    }

    fn into_values(self) -> Vec<RuntimeValue> {
        let row_count = self.len;
        let columns = self.columns;
        (0..row_count)
            .map(|row| {
                RuntimeValue::Tuple(
                    columns
                        .iter()
                        .map(|column| column.value_at(row))
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    }

    #[must_use]
    fn tail_from(&self, index: usize) -> Self {
        Self {
            len: self.len.saturating_sub(index),
            columns: self
                .columns
                .iter()
                .map(|column| column.tail_from(index))
                .collect(),
        }
    }

    fn value_at(&self, index: usize) -> RuntimeValue {
        assert!(
            index < self.len,
            "tuple column sequence index out of bounds"
        );
        RuntimeValue::Tuple(
            self.columns
                .iter()
                .map(|column| column.value_at(index))
                .collect(),
        )
    }
}

/// Columnar storage for a sequence of records with a stable field order.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordSeq {
    len: usize,
    fields: Vec<RecordSeqField>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordSeqField {
    pub name: String,
    pub values: RuntimeSeq,
}

impl RecordSeq {
    pub fn new(len: usize, fields: Vec<RecordSeqField>) -> Result<Self, RuntimeSeqError> {
        for (ordinal, field) in fields.iter().enumerate() {
            if field.values.len() != len {
                return Err(RuntimeSeqError::ColumnLength {
                    ordinal,
                    expected: len,
                    actual: field.values.len(),
                });
            }
            if fields[..ordinal]
                .iter()
                .any(|candidate| candidate.name == field.name)
            {
                return Err(RuntimeSeqError::DuplicateRecordField {
                    field: field.name.clone(),
                });
            }
        }
        Ok(Self { len, fields })
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn fields(&self) -> &[RecordSeqField] {
        &self.fields
    }

    pub fn field_by_ordinal(&self, ordinal: usize) -> Option<&RuntimeSeq> {
        self.fields.get(ordinal).map(|field| &field.values)
    }

    pub fn field_by_name(&self, name: &str) -> Option<&RuntimeSeq> {
        self.fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| &field.values)
    }

    fn into_values(self) -> Vec<RuntimeValue> {
        let row_count = self.len;
        let fields = self.fields;
        (0..row_count)
            .map(|row| {
                RuntimeValue::Record(
                    fields
                        .iter()
                        .map(|field| RuntimeFieldValue {
                            name: field.name.clone(),
                            value: field.values.value_at(row),
                        })
                        .collect(),
                )
            })
            .collect()
    }

    #[must_use]
    fn tail_from(&self, index: usize) -> Self {
        Self {
            len: self.len.saturating_sub(index),
            fields: self
                .fields
                .iter()
                .map(|field| RecordSeqField {
                    name: field.name.clone(),
                    values: field.values.tail_from(index),
                })
                .collect(),
        }
    }

    fn value_at(&self, index: usize) -> RuntimeValue {
        assert!(
            index < self.len,
            "record column sequence index out of bounds"
        );
        RuntimeValue::Record(
            self.fields
                .iter()
                .map(|field| RuntimeFieldValue {
                    name: field.name.clone(),
                    value: field.values.value_at(index),
                })
                .collect(),
        )
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimeSeqError {
    #[error("sequence column {ordinal} length {actual} does not match expected length {expected}")]
    ColumnLength {
        ordinal: usize,
        expected: usize,
        actual: usize,
    },
    #[error("record sequence contains duplicate field `{field}`")]
    DuplicateRecordField { field: String },
}

/// Storage value for `isize`-semantic runtime integers.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct RuntimeISizeValue(i64);

impl RuntimeISizeValue {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    #[must_use]
    pub fn wrapping_neg(self) -> Self {
        Self(self.0.wrapping_neg())
    }
}

impl std::fmt::Display for RuntimeISizeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Storage value for `usize`-semantic runtime integers.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct RuntimeUSizeValue(u64);

impl RuntimeUSizeValue {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for RuntimeUSizeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Exact integer storage that can cross runtime pure-helper fast paths without widening.
pub trait RuntimeExactInteger: Copy + 'static {
    const INPUT_TYPE: RuntimePureInputType;
    const OUTPUT_TYPE: RuntimePureOutputType;

    fn into_runtime_value(self) -> RuntimeValue;
    fn try_from_runtime_value(helper: &str, value: RuntimeValue) -> Result<Self, RuntimeEvalError>;
    fn try_sum_as_i64(self, helper: &str) -> Result<i64, RuntimeEvalError>;
    fn exact_slice(values: &[Self]) -> RuntimeExactIntegerSlice<'_>;
    fn exact_slice_mut(values: &mut [Self]) -> RuntimeExactIntegerSliceMut<'_>;
    fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]>;
    fn dense_sequence(values: Vec<Self>) -> RuntimeValue;
}

/// Borrowed exact-width integer slice used at runtime fast-path boundaries.
#[derive(Clone, Copy, Debug)]
pub enum RuntimeExactIntegerSlice<'a> {
    I8(&'a [i8]),
    I16(&'a [i16]),
    I32(&'a [i32]),
    I128(&'a [i128]),
    ISize(&'a [RuntimeISizeValue]),
    U8(&'a [u8]),
    U16(&'a [u16]),
    U32(&'a [u32]),
    U64(&'a [u64]),
    U128(&'a [u128]),
    USize(&'a [RuntimeUSizeValue]),
}

/// Mutable exact-width integer slice used at runtime fast-path output boundaries.
#[derive(Debug)]
pub enum RuntimeExactIntegerSliceMut<'a> {
    I8(&'a mut [i8]),
    I16(&'a mut [i16]),
    I32(&'a mut [i32]),
    I128(&'a mut [i128]),
    ISize(&'a mut [RuntimeISizeValue]),
    U8(&'a mut [u8]),
    U16(&'a mut [u16]),
    U32(&'a mut [u32]),
    U64(&'a mut [u64]),
    U128(&'a mut [u128]),
    USize(&'a mut [RuntimeUSizeValue]),
}

macro_rules! impl_runtime_exact_signed_integer {
    ($ty:ty, $input_type:ident, $output_type:ident, $slice:ident, $dense:ident, $variant:ident) => {
        impl RuntimeExactInteger for $ty {
            const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::$input_type;
            const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::$output_type;

            fn into_runtime_value(self) -> RuntimeValue {
                RuntimeValue::Int(RuntimeInt::$variant(self))
            }

            fn try_from_runtime_value(
                helper: &str,
                value: RuntimeValue,
            ) -> Result<Self, RuntimeEvalError> {
                match value {
                    RuntimeValue::Int(RuntimeInt::$variant(value)) => Ok(value),
                    RuntimeValue::Int(value) => Err(RuntimeEvalError::UnsupportedPure {
                        name: helper.to_owned(),
                        reason: format!(
                            "pure {} result expected {}, got {}",
                            stringify!($ty),
                            stringify!($ty),
                            value.label()
                        ),
                    }),
                    value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
                }
            }

            fn try_sum_as_i64(self, _helper: &str) -> Result<i64, RuntimeEvalError> {
                Ok(i64::from(self))
            }

            fn exact_slice(values: &[Self]) -> RuntimeExactIntegerSlice<'_> {
                RuntimeExactIntegerSlice::$variant(values)
            }

            fn exact_slice_mut(values: &mut [Self]) -> RuntimeExactIntegerSliceMut<'_> {
                RuntimeExactIntegerSliceMut::$variant(values)
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
    ($ty:ty, $input_type:ident, $output_type:ident, $slice:ident, $dense:ident, $variant:ident) => {
        impl RuntimeExactInteger for $ty {
            const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::$input_type;
            const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::$output_type;

            fn into_runtime_value(self) -> RuntimeValue {
                RuntimeValue::UInt(RuntimeUInt::$variant(self))
            }

            fn try_from_runtime_value(
                helper: &str,
                value: RuntimeValue,
            ) -> Result<Self, RuntimeEvalError> {
                match value {
                    RuntimeValue::UInt(RuntimeUInt::$variant(value)) => Ok(value),
                    RuntimeValue::UInt(value) => Err(RuntimeEvalError::UnsupportedPure {
                        name: helper.to_owned(),
                        reason: format!(
                            "pure {} result expected {}, got {}",
                            stringify!($ty),
                            stringify!($ty),
                            value.label()
                        ),
                    }),
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

            fn exact_slice(values: &[Self]) -> RuntimeExactIntegerSlice<'_> {
                RuntimeExactIntegerSlice::$variant(values)
            }

            fn exact_slice_mut(values: &mut [Self]) -> RuntimeExactIntegerSliceMut<'_> {
                RuntimeExactIntegerSliceMut::$variant(values)
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
                RuntimeValue::Int(RuntimeInt::$variant(self))
            }

            fn try_from_runtime_value(
                helper: &str,
                value: RuntimeValue,
            ) -> Result<Self, RuntimeEvalError> {
                match value {
                    RuntimeValue::Int(RuntimeInt::$variant(value)) => Ok(value),
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

            fn exact_slice(values: &[Self]) -> RuntimeExactIntegerSlice<'_> {
                RuntimeExactIntegerSlice::$variant(values)
            }

            fn exact_slice_mut(values: &mut [Self]) -> RuntimeExactIntegerSliceMut<'_> {
                RuntimeExactIntegerSliceMut::$variant(values)
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
                RuntimeValue::UInt(RuntimeUInt::$variant(self))
            }

            fn try_from_runtime_value(
                helper: &str,
                value: RuntimeValue,
            ) -> Result<Self, RuntimeEvalError> {
                match value {
                    RuntimeValue::UInt(RuntimeUInt::$variant(value)) => Ok(value),
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

            fn exact_slice(values: &[Self]) -> RuntimeExactIntegerSlice<'_> {
                RuntimeExactIntegerSlice::$variant(values)
            }

            fn exact_slice_mut(values: &mut [Self]) -> RuntimeExactIntegerSliceMut<'_> {
                RuntimeExactIntegerSliceMut::$variant(values)
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

    pub fn dense_f32(values: Vec<f32>) -> Self {
        Self::Dense(DenseSeq::f32(values))
    }

    pub fn dense_f64(values: Vec<f64>) -> Self {
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

    pub fn dense_entity_refs(values: Vec<String>) -> Self {
        Self::Dense(DenseSeq::entity_refs(values))
    }

    pub fn tuple_columns(len: usize, columns: Vec<RuntimeSeq>) -> Result<Self, RuntimeSeqError> {
        TupleSeq::new(len, columns).map(Self::TupleColumns)
    }

    pub fn record_columns(
        len: usize,
        fields: Vec<RecordSeqField>,
    ) -> Result<Self, RuntimeSeqError> {
        RecordSeq::new(len, fields).map(Self::RecordColumns)
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Dense(values) => values.len(),
            Self::TupleColumns(values) => values.len(),
            Self::RecordColumns(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_values(&self) -> Option<&[RuntimeValue]> {
        match self {
            Self::Values(values) => Some(values),
            Self::Dense(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn unit_len(&self) -> Option<usize> {
        match self {
            Self::Dense(values) => values.unit_len(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn dense_kind(&self) -> Option<DenseSeqKind> {
        match self {
            Self::Dense(values) => Some(values.kind()),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_i64_slice(&self) -> Option<&[i64]> {
        match self {
            Self::Dense(values) => values.as_i64_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn copy_i64_values_to(&self, out: &mut Vec<i64>) -> bool {
        match self {
            Self::Dense(values) => values.copy_i64_values_to(out),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => false,
        }
    }

    pub fn try_for_each_i64<E>(&self, visit: impl FnMut(i64) -> Result<(), E>) -> Result<bool, E> {
        match self {
            Self::Dense(values) => values.try_for_each_i64(visit),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => Ok(false),
        }
    }

    pub fn first_i64(&self) -> Option<Option<i64>> {
        match self {
            Self::Dense(values) => values.first_i64(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_i128_slice(&self) -> Option<&[i128]> {
        match self {
            Self::Dense(values) => values.as_i128_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_isize_values(&self) -> Option<Vec<i64>> {
        match self {
            Self::Dense(values) => values.as_isize_values(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_isize_storage(&self) -> Option<&[RuntimeISizeValue]> {
        match self {
            Self::Dense(values) => values.as_isize_storage(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_i8_slice(&self) -> Option<&[i8]> {
        match self {
            Self::Dense(values) => values.as_i8_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_i16_slice(&self) -> Option<&[i16]> {
        match self {
            Self::Dense(values) => values.as_i16_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_i32_slice(&self) -> Option<&[i32]> {
        match self {
            Self::Dense(values) => values.as_i32_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_u8_slice(&self) -> Option<&[u8]> {
        match self {
            Self::Dense(values) => values.as_u8_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_u16_slice(&self) -> Option<&[u16]> {
        match self {
            Self::Dense(values) => values.as_u16_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_u32_slice(&self) -> Option<&[u32]> {
        match self {
            Self::Dense(values) => values.as_u32_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_u64_slice(&self) -> Option<&[u64]> {
        match self {
            Self::Dense(values) => values.as_u64_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_u128_slice(&self) -> Option<&[u128]> {
        match self {
            Self::Dense(values) => values.as_u128_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_usize_values(&self) -> Option<Vec<u64>> {
        match self {
            Self::Dense(values) => values.as_usize_values(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_usize_storage(&self) -> Option<&[RuntimeUSizeValue]> {
        match self {
            Self::Dense(values) => values.as_usize_storage(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_f32_slice(&self) -> Option<&[f32]> {
        match self {
            Self::Dense(values) => values.as_f32_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_f64_slice(&self) -> Option<&[f64]> {
        match self {
            Self::Dense(values) => values.as_f64_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_bool_slice(&self) -> Option<&[bool]> {
        match self {
            Self::Dense(values) => values.as_bool_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Dense(values) => values.as_bytes(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_chars(&self) -> Option<&[char]> {
        match self {
            Self::Dense(values) => values.as_chars(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_durations(&self) -> Option<&[LogicalDuration]> {
        match self {
            Self::Dense(values) => values.as_durations(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_strings(&self) -> Option<&[String]> {
        match self {
            Self::Dense(values) => values.as_strings(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_entity_refs(&self) -> Option<&[String]> {
        match self {
            Self::Dense(values) => values.as_entity_refs(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn into_values(self) -> Vec<RuntimeValue> {
        match self {
            Self::Values(values) => values,
            Self::Dense(values) => values.into_values(),
            Self::TupleColumns(values) => values.into_values(),
            Self::RecordColumns(values) => values.into_values(),
        }
    }

    /// Returns the runtime value at `index`.
    ///
    /// # Panics
    ///
    /// Panics when `index` is outside this sequence.
    pub fn value_at(&self, index: usize) -> RuntimeValue {
        match self {
            Self::Values(values) => values[index].clone(),
            Self::Dense(values) => values.value_at(index),
            Self::TupleColumns(values) => values.value_at(index),
            Self::RecordColumns(values) => values.value_at(index),
        }
    }

    #[must_use]
    pub fn tail_from(&self, index: usize) -> Self {
        match self {
            Self::Values(values) => Self::Values(values[index..].to_vec()),
            Self::Dense(values) => Self::Dense(values.tail_from(index)),
            Self::TupleColumns(values) => Self::TupleColumns(values.tail_from(index)),
            Self::RecordColumns(values) => Self::RecordColumns(values.tail_from(index)),
        }
    }

    pub fn into_i64_vec(self) -> Option<Vec<i64>> {
        match self {
            Self::Dense(values) => values.into_i64_vec(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn sum_as_i64(&self) -> Option<i64> {
        match self {
            Self::Dense(values) => values.sum_as_i64(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }
}

impl_runtime_exact_signed_integer!(i8, I8, I8, as_i8_slice, runtime_sequence_dense_i8, I8);
impl_runtime_exact_signed_integer!(i16, I16, I16, as_i16_slice, runtime_sequence_dense_i16, I16);
impl_runtime_exact_signed_integer!(i32, I32, I32, as_i32_slice, runtime_sequence_dense_i32, I32);
impl_runtime_exact_wide_signed_integer!(
    i128,
    I128,
    I128,
    as_i128_slice,
    runtime_sequence_dense_i128,
    I128
);
impl_runtime_exact_unsigned_integer!(u8, U8, U8, as_u8_slice, runtime_sequence_dense_u8, U8);
impl_runtime_exact_unsigned_integer!(u16, U16, U16, as_u16_slice, runtime_sequence_dense_u16, U16);
impl_runtime_exact_unsigned_integer!(u32, U32, U32, as_u32_slice, runtime_sequence_dense_u32, U32);
impl_runtime_exact_unsigned_integer!(u64, U64, U64, as_u64_slice, runtime_sequence_dense_u64, U64);
impl_runtime_exact_wide_unsigned_integer!(
    u128,
    U128,
    U128,
    as_u128_slice,
    runtime_sequence_dense_u128,
    U128
);

impl RuntimeExactInteger for RuntimeISizeValue {
    const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::ISize;
    const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::ISize;

    fn into_runtime_value(self) -> RuntimeValue {
        RuntimeValue::isize(self.0)
    }

    fn try_from_runtime_value(helper: &str, value: RuntimeValue) -> Result<Self, RuntimeEvalError> {
        match value {
            RuntimeValue::Int(RuntimeInt::ISize(value)) => Ok(Self(value)),
            value => Err(RuntimeEvalError::UnsupportedPure {
                name: helper.to_owned(),
                reason: format!(
                    "pure isize result expected isize, got {}",
                    runtime_value_label(&value)
                ),
            }),
        }
    }

    fn try_sum_as_i64(self, _helper: &str) -> Result<i64, RuntimeEvalError> {
        Ok(self.0)
    }

    fn exact_slice(values: &[Self]) -> RuntimeExactIntegerSlice<'_> {
        RuntimeExactIntegerSlice::ISize(values)
    }

    fn exact_slice_mut(values: &mut [Self]) -> RuntimeExactIntegerSliceMut<'_> {
        RuntimeExactIntegerSliceMut::ISize(values)
    }

    fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]> {
        match seq {
            RuntimeSeq::Dense(DenseSeq::ISize(values)) => Some(values.as_slice()),
            RuntimeSeq::Values(_) | RuntimeSeq::TupleColumns(_) | RuntimeSeq::RecordColumns(_) => {
                None
            }
            RuntimeSeq::Dense(_) => None,
        }
    }

    fn dense_sequence(values: Vec<Self>) -> RuntimeValue {
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::ISize(DenseSeqStorage::new(
            values,
        ))))
    }
}

impl RuntimeExactInteger for RuntimeUSizeValue {
    const INPUT_TYPE: RuntimePureInputType = RuntimePureInputType::USize;
    const OUTPUT_TYPE: RuntimePureOutputType = RuntimePureOutputType::USize;

    fn into_runtime_value(self) -> RuntimeValue {
        RuntimeValue::usize(self.0)
    }

    fn try_from_runtime_value(helper: &str, value: RuntimeValue) -> Result<Self, RuntimeEvalError> {
        match value {
            RuntimeValue::UInt(RuntimeUInt::USize(value)) => Ok(Self(value)),
            value => Err(RuntimeEvalError::UnsupportedPure {
                name: helper.to_owned(),
                reason: format!(
                    "pure usize result expected usize, got {}",
                    runtime_value_label(&value)
                ),
            }),
        }
    }

    fn try_sum_as_i64(self, helper: &str) -> Result<i64, RuntimeEvalError> {
        i64::try_from(self.0).map_err(|_| RuntimeEvalError::UnsupportedPure {
            name: helper.to_owned(),
            reason: format!("pure usize result `{self}` cannot be represented as an i64 sum"),
        })
    }

    fn exact_slice(values: &[Self]) -> RuntimeExactIntegerSlice<'_> {
        RuntimeExactIntegerSlice::USize(values)
    }

    fn exact_slice_mut(values: &mut [Self]) -> RuntimeExactIntegerSliceMut<'_> {
        RuntimeExactIntegerSliceMut::USize(values)
    }

    fn seq_slice(seq: &RuntimeSeq) -> Option<&[Self]> {
        match seq {
            RuntimeSeq::Dense(DenseSeq::USize(values)) => Some(values.as_slice()),
            RuntimeSeq::Values(_) | RuntimeSeq::TupleColumns(_) | RuntimeSeq::RecordColumns(_) => {
                None
            }
            RuntimeSeq::Dense(_) => None,
        }
    }

    fn dense_sequence(values: Vec<Self>) -> RuntimeValue {
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::USize(DenseSeqStorage::new(
            values,
        ))))
    }
}

/// Homogeneous storage kind used by a dense runtime sequence.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
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
    EntityRefs,
}

/// Dense sequence storage for homogeneous scalar data.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum DenseSeq {
    Units(usize),
    I8(DenseSeqStorage<i8>),
    I16(DenseSeqStorage<i16>),
    I32(DenseSeqStorage<i32>),
    I64(DenseSeqStorage<i64>),
    I128(DenseSeqStorage<i128>),
    ISize(DenseSeqStorage<RuntimeISizeValue>),
    U8(DenseSeqStorage<u8>),
    U16(DenseSeqStorage<u16>),
    U32(DenseSeqStorage<u32>),
    U64(DenseSeqStorage<u64>),
    U128(DenseSeqStorage<u128>),
    USize(DenseSeqStorage<RuntimeUSizeValue>),
    F32(DenseSeqStorage<f32>),
    F64(DenseSeqStorage<f64>),
    Bool(DenseSeqStorage<bool>),
    Bytes(DenseSeqStorage<u8>),
    Chars(DenseSeqStorage<char>),
    Durations(DenseSeqStorage<LogicalDuration>),
    Strings(DenseSeqStorage<String>),
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
        Self::ISize(DenseSeqStorage::new(
            values.into_iter().map(RuntimeISizeValue::new).collect(),
        ))
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
        Self::USize(DenseSeqStorage::new(
            values.into_iter().map(RuntimeUSizeValue::new).collect(),
        ))
    }

    pub fn f32(values: Vec<f32>) -> Self {
        Self::F32(DenseSeqStorage::new(values))
    }

    pub fn f64(values: Vec<f64>) -> Self {
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

    pub fn entity_refs(values: Vec<String>) -> Self {
        Self::EntityRefs(DenseSeqStorage::new(values))
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Units(len) => *len,
            Self::I8(values) => values.len(),
            Self::I16(values) => values.len(),
            Self::I32(values) => values.len(),
            Self::I64(values) => values.len(),
            Self::ISize(values) => values.len(),
            Self::I128(values) => values.len(),
            Self::U8(values) | Self::Bytes(values) => values.len(),
            Self::U16(values) => values.len(),
            Self::U32(values) => values.len(),
            Self::U64(values) => values.len(),
            Self::USize(values) => values.len(),
            Self::U128(values) => values.len(),
            Self::F32(values) => values.len(),
            Self::F64(values) => values.len(),
            Self::Bool(values) => values.len(),
            Self::Chars(values) => values.len(),
            Self::Durations(values) => values.len(),
            Self::Strings(values) | Self::EntityRefs(values) => values.len(),
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

    pub fn as_isize_values(&self) -> Option<Vec<i64>> {
        self.as_isize_storage()
            .map(|values| values.iter().copied().map(RuntimeISizeValue::get).collect())
    }

    pub fn as_isize_storage(&self) -> Option<&[RuntimeISizeValue]> {
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

    pub fn as_usize_values(&self) -> Option<Vec<u64>> {
        self.as_usize_storage()
            .map(|values| values.iter().copied().map(RuntimeUSizeValue::get).collect())
    }

    pub fn as_usize_storage(&self) -> Option<&[RuntimeUSizeValue]> {
        match self {
            Self::USize(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_f32_slice(&self) -> Option<&[f32]> {
        match self {
            Self::F32(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_f64_slice(&self) -> Option<&[f64]> {
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

    pub fn as_entity_refs(&self) -> Option<&[String]> {
        match self {
            Self::EntityRefs(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn into_values(self) -> Vec<RuntimeValue> {
        match self {
            Self::Units(len) => vec![RuntimeValue::Unit; len],
            Self::I8(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::i8)
                .collect(),
            Self::I16(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::i16)
                .collect(),
            Self::I32(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::i32)
                .collect(),
            Self::I64(values) => materialize_i64_sequence(values.into_vec()),
            Self::I128(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::i128)
                .collect(),
            Self::ISize(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeISizeValue::get)
                .map(RuntimeValue::isize)
                .collect(),
            Self::U8(values) | Self::Bytes(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::u8)
                .collect(),
            Self::U16(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::u16)
                .collect(),
            Self::U32(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::u32)
                .collect(),
            Self::U64(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::u64)
                .collect(),
            Self::U128(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::u128)
                .collect(),
            Self::USize(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeUSizeValue::get)
                .map(RuntimeValue::usize)
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
            Self::I8(values) => RuntimeValue::i8(values.as_slice()[index]),
            Self::I16(values) => RuntimeValue::i16(values.as_slice()[index]),
            Self::I32(values) => RuntimeValue::i32(values.as_slice()[index]),
            Self::I64(values) => RuntimeValue::i64(values.as_slice()[index]),
            Self::I128(values) => RuntimeValue::i128(values.as_slice()[index]),
            Self::ISize(values) => RuntimeValue::isize(values.as_slice()[index].get()),
            Self::U8(values) | Self::Bytes(values) => RuntimeValue::u8(values.as_slice()[index]),
            Self::U16(values) => RuntimeValue::u16(values.as_slice()[index]),
            Self::U32(values) => RuntimeValue::u32(values.as_slice()[index]),
            Self::U64(values) => RuntimeValue::u64(values.as_slice()[index]),
            Self::U128(values) => RuntimeValue::u128(values.as_slice()[index]),
            Self::USize(values) => RuntimeValue::usize(values.as_slice()[index].get()),
            Self::F32(values) => RuntimeValue::F32(values.as_slice()[index]),
            Self::F64(values) => RuntimeValue::F64(values.as_slice()[index]),
            Self::Bool(values) => RuntimeValue::Bool(values.as_slice()[index]),
            Self::Chars(values) => RuntimeValue::Char(values.as_slice()[index]),
            Self::Durations(values) => RuntimeValue::Duration(values.as_slice()[index]),
            Self::Strings(values) => RuntimeValue::String(values.as_slice()[index].clone()),
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
            | Self::EntityRefs(_) => None,
        }
    }

    pub fn sum_as_i64(&self) -> Option<i64> {
        match self {
            Self::I8(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::I16(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::I32(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::I64(values) => Some(values.as_slice().iter().sum()),
            Self::ISize(values) => Some(
                values
                    .as_slice()
                    .iter()
                    .copied()
                    .map(RuntimeISizeValue::get)
                    .sum(),
            ),
            Self::I128(values) => values.as_slice().iter().try_fold(0_i64, |acc, value| {
                i64::try_from(*value).ok().map(|value| acc + value)
            }),
            Self::U8(values) | Self::Bytes(values) => {
                Some(values.as_slice().iter().copied().map(i64::from).sum())
            }
            Self::U16(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::U32(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::U64(values) => values.as_slice().iter().try_fold(0_i64, |acc, value| {
                i64::try_from(*value).ok().map(|value| acc + value)
            }),
            Self::USize(values) => values.as_slice().iter().try_fold(0_i64, |acc, value| {
                i64::try_from(value.get()).ok().map(|value| acc + value)
            }),
            Self::U128(values) => values.as_slice().iter().try_fold(0_i64, |acc, value| {
                i64::try_from(*value).ok().map(|value| acc + value)
            }),
            Self::Units(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Chars(_)
            | Self::Durations(_)
            | Self::Strings(_)
            | Self::EntityRefs(_) => None,
        }
    }
}

/// Generic backing store for one dense homogeneous sequence.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeFieldValue {
    pub name: String,
    pub value: RuntimeValue,
}

/// Expression subset executable by the Sans I/O flow runtime.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
    ProjectTuple {
        target: Box<RuntimeExpr>,
        ordinal: usize,
    },
    ProjectRecord {
        target: Box<RuntimeExpr>,
        ordinal: usize,
    },
    Call {
        callee: RuntimeCallTarget,
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
            Self::Value(
                RuntimeValue::Bool(_)
                | RuntimeValue::Int(_)
                | RuntimeValue::UInt(_)
                | RuntimeValue::F32(_)
                | RuntimeValue::F64(_),
            )
            | Self::Local(_) => true,
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
            | Self::ProjectTuple { .. }
            | Self::ProjectRecord { .. }
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

impl fmt::Display for RuntimeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(value) => f.write_str(&runtime_value_label(value)),
            Self::Local(name) => f.write_str(name),
            Self::EntityRef(target) => write!(f, "@{target}"),
            Self::Let { name, .. } => write!(f, "let {name}"),
            Self::Tuple(items) => write!(f, "tuple/{}", items.len()),
            Self::BracketSeq(items) => write!(f, "bracket_seq/{}", items.len()),
            Self::RepeatSeq { len, .. } => write!(f, "repeat_seq/{len}"),
            Self::Record(fields) => write!(f, "record/{}", fields.len()),
            Self::Variant { name, .. } => write!(f, ".{name}"),
            Self::Field { field, .. } => write!(f, ".{field}"),
            Self::ProjectTuple { ordinal, .. } => write!(f, ".{ordinal}"),
            Self::ProjectRecord { ordinal, .. } => write!(f, ".#{ordinal}"),
            Self::Call { callee, .. } => write!(f, "{callee}()"),
            Self::PureCall { helper, .. } => write!(f, "pure#{}()", helper.0),
            Self::SpreadArg(expr) => write!(f, "{expr}..."),
            Self::MethodCall { method, .. } => write!(f, ".{method}()"),
            Self::Map { .. } => f.write_str("map"),
            Self::Sum { .. } => f.write_str("sum"),
            Self::Unary { op, .. } => f.write_str(runtime_unary_op_label(*op)),
            Self::Binary { op, .. } => f.write_str(runtime_binary_op_label(*op)),
            Self::If { .. } => f.write_str("if"),
            Self::IfLet { .. } => f.write_str("if let"),
            Self::Match { .. } => f.write_str("match"),
        }
    }
}

impl fmt::Display for RuntimeUnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(runtime_unary_op_label(*self))
    }
}

impl fmt::Display for RuntimeBinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(runtime_binary_op_label(*self))
    }
}

/// One value-producing `match` arm in a runtime expression.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeExprMatchArm {
    pub pattern: RuntimePattern,
    pub guard: Option<RuntimeExpr>,
    pub value: RuntimeExpr,
}

/// One field inside a runtime record expression.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeFieldExpr {
    pub name: String,
    pub value: RuntimeExpr,
}

/// Unary operator supported by the Sans I/O expression evaluator.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeUnaryOp {
    Not,
    Neg,
}

/// Binary operator supported by the Sans I/O expression evaluator.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
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

#[derive(Debug)]
pub struct RuntimeEnv {
    scopes: Vec<RuntimeScope>,
    spare_scopes: Vec<RuntimeScope>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct RuntimeScope {
    bindings: Vec<RuntimeBinding>,
}

/// Pure runtime program consumed by the minimal Sans I/O engine.

#[derive(Clone, Debug, Error, PartialEq)]
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

    pub(crate) fn replace_root_f32_bindings(&mut self, input_names: &[String], args: &[f32]) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_f32_bindings(input_names, args);
        }
    }

    pub(crate) fn replace_root_f64_bindings(&mut self, input_names: &[String], args: &[f64]) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_f64_bindings(input_names, args);
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
                .for_each(|(binding, value)| binding.value = RuntimeValue::i64(value));
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args.iter().copied())
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: RuntimeValue::i64(value),
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
                .for_each(|(binding, value)| binding.value = RuntimeValue::i32(value));
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args.iter().copied())
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: RuntimeValue::i32(value),
                }),
        );
    }

    fn replace_f32_bindings(&mut self, input_names: &[String], args: &[f32]) {
        self.replace_float_bindings(input_names, args, RuntimeValue::F32);
    }

    fn replace_f64_bindings(&mut self, input_names: &[String], args: &[f64]) {
        self.replace_float_bindings(input_names, args, RuntimeValue::F64);
    }

    fn replace_float_bindings<T: Copy>(
        &mut self,
        input_names: &[String],
        args: &[T],
        wrap: impl Fn(T) -> RuntimeValue,
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
                .for_each(|(binding, value)| binding.value = wrap(value));
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args.iter().copied())
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: wrap(value),
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
        (RuntimeUnaryOp::Neg, RuntimeValue::Int(value)) => {
            Ok(RuntimeValue::Int(evaluate_signed_integer_neg(value)))
        }
        (RuntimeUnaryOp::Neg, RuntimeValue::F32(value)) => Ok(RuntimeValue::F32(-value)),
        (RuntimeUnaryOp::Neg, RuntimeValue::F64(value)) => Ok(RuntimeValue::F64(-value)),
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
                (RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)) => {
                    evaluate_signed_integer_compare(lhs, op, rhs).map(RuntimeValue::Bool)
                }
                (RuntimeValue::UInt(lhs), RuntimeValue::UInt(rhs)) => {
                    evaluate_unsigned_integer_compare(lhs, op, rhs).map(RuntimeValue::Bool)
                }
                (RuntimeValue::F32(lhs), RuntimeValue::F32(rhs)) => {
                    Ok(RuntimeValue::Bool(compare_float(&lhs, op, &rhs)))
                }
                (RuntimeValue::F64(lhs), RuntimeValue::F64(rhs)) => {
                    Ok(RuntimeValue::Bool(compare_float(&lhs, op, &rhs)))
                }
                (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
            }
        }
        RuntimeBinaryOp::Add
        | RuntimeBinaryOp::Sub
        | RuntimeBinaryOp::Mul
        | RuntimeBinaryOp::Div => match (lhs, rhs) {
            (RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)) => {
                evaluate_signed_integer_op(lhs, op, rhs).map(RuntimeValue::Int)
            }
            (RuntimeValue::UInt(lhs), RuntimeValue::UInt(rhs)) => {
                evaluate_unsigned_integer_op(lhs, op, rhs).map(RuntimeValue::UInt)
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

pub fn evaluate_std_float_intrinsic(
    intrinsic: RuntimeIntrinsic,
    args: &[RuntimeValue],
) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
    if let Some(value) = evaluate_std_f32_intrinsic(intrinsic, args)? {
        return Ok(Some(value));
    }
    evaluate_std_f64_intrinsic(intrinsic, args)
}

fn evaluate_std_f32_intrinsic(
    intrinsic: RuntimeIntrinsic,
    args: &[RuntimeValue],
) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
    let value = match intrinsic {
        RuntimeIntrinsic::StdF32Abs => RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.abs()),
        RuntimeIntrinsic::StdF32Floor => {
            RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.floor())
        }
        RuntimeIntrinsic::StdF32Ceil => {
            RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.ceil())
        }
        RuntimeIntrinsic::StdF32Round => {
            RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.round())
        }
        RuntimeIntrinsic::StdF32Trunc => {
            RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.trunc())
        }
        RuntimeIntrinsic::StdF32Fract => {
            RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.fract())
        }
        RuntimeIntrinsic::StdF32Sqrt => {
            RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.sqrt())
        }
        RuntimeIntrinsic::StdF32Sin => RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.sin()),
        RuntimeIntrinsic::StdF32Cos => RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.cos()),
        RuntimeIntrinsic::StdF32Tan => RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.tan()),
        RuntimeIntrinsic::StdF32Exp => RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.exp()),
        RuntimeIntrinsic::StdF32Exp2 => {
            RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.exp2())
        }
        RuntimeIntrinsic::StdF32Ln => RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.ln()),
        RuntimeIntrinsic::StdF32Log2 => {
            RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.log2())
        }
        RuntimeIntrinsic::StdF32Log10 => {
            RuntimeValue::F32(expect_unary_f32(intrinsic, args)?.log10())
        }
        RuntimeIntrinsic::StdF32Powf => {
            let (lhs, rhs) = expect_binary_f32(intrinsic, args)?;
            RuntimeValue::F32(lhs.powf(rhs))
        }
        RuntimeIntrinsic::StdF32Atan2 => {
            let (lhs, rhs) = expect_binary_f32(intrinsic, args)?;
            RuntimeValue::F32(lhs.atan2(rhs))
        }
        RuntimeIntrinsic::StdF32MulAdd => {
            let (a, b, c) = expect_ternary_f32(intrinsic, args)?;
            RuntimeValue::F32(a.mul_add(b, c))
        }
        RuntimeIntrinsic::StdF32IsNan => {
            RuntimeValue::Bool(expect_unary_f32(intrinsic, args)?.is_nan())
        }
        RuntimeIntrinsic::StdF32IsInfinite => {
            RuntimeValue::Bool(expect_unary_f32(intrinsic, args)?.is_infinite())
        }
        RuntimeIntrinsic::StdF32IsFinite => {
            RuntimeValue::Bool(expect_unary_f32(intrinsic, args)?.is_finite())
        }
        RuntimeIntrinsic::StdF32IsSignPositive => {
            RuntimeValue::Bool(expect_unary_f32(intrinsic, args)?.is_sign_positive())
        }
        RuntimeIntrinsic::StdF32IsSignNegative => {
            RuntimeValue::Bool(expect_unary_f32(intrinsic, args)?.is_sign_negative())
        }
        RuntimeIntrinsic::StdF32ToBits => {
            RuntimeValue::u32(expect_unary_f32(intrinsic, args)?.to_bits())
        }
        RuntimeIntrinsic::StdF32FromBits => {
            RuntimeValue::F32(f32::from_bits(expect_unary_u32(intrinsic, args)?))
        }
        RuntimeIntrinsic::StdF32ToF64 => {
            RuntimeValue::F64(f64::from(expect_unary_f32(intrinsic, args)?))
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

fn evaluate_std_f64_intrinsic(
    intrinsic: RuntimeIntrinsic,
    args: &[RuntimeValue],
) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
    let value = match intrinsic {
        RuntimeIntrinsic::StdF64Abs => RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.abs()),
        RuntimeIntrinsic::StdF64Floor => {
            RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.floor())
        }
        RuntimeIntrinsic::StdF64Ceil => {
            RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.ceil())
        }
        RuntimeIntrinsic::StdF64Round => {
            RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.round())
        }
        RuntimeIntrinsic::StdF64Trunc => {
            RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.trunc())
        }
        RuntimeIntrinsic::StdF64Fract => {
            RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.fract())
        }
        RuntimeIntrinsic::StdF64Sqrt => {
            RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.sqrt())
        }
        RuntimeIntrinsic::StdF64Sin => RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.sin()),
        RuntimeIntrinsic::StdF64Cos => RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.cos()),
        RuntimeIntrinsic::StdF64Tan => RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.tan()),
        RuntimeIntrinsic::StdF64Exp => RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.exp()),
        RuntimeIntrinsic::StdF64Exp2 => {
            RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.exp2())
        }
        RuntimeIntrinsic::StdF64Ln => RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.ln()),
        RuntimeIntrinsic::StdF64Log2 => {
            RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.log2())
        }
        RuntimeIntrinsic::StdF64Log10 => {
            RuntimeValue::F64(expect_unary_f64(intrinsic, args)?.log10())
        }
        RuntimeIntrinsic::StdF64Powf => {
            let (lhs, rhs) = expect_binary_f64(intrinsic, args)?;
            RuntimeValue::F64(lhs.powf(rhs))
        }
        RuntimeIntrinsic::StdF64Atan2 => {
            let (lhs, rhs) = expect_binary_f64(intrinsic, args)?;
            RuntimeValue::F64(lhs.atan2(rhs))
        }
        RuntimeIntrinsic::StdF64MulAdd => {
            let (a, b, c) = expect_ternary_f64(intrinsic, args)?;
            RuntimeValue::F64(a.mul_add(b, c))
        }
        RuntimeIntrinsic::StdF64IsNan => {
            RuntimeValue::Bool(expect_unary_f64(intrinsic, args)?.is_nan())
        }
        RuntimeIntrinsic::StdF64IsInfinite => {
            RuntimeValue::Bool(expect_unary_f64(intrinsic, args)?.is_infinite())
        }
        RuntimeIntrinsic::StdF64IsFinite => {
            RuntimeValue::Bool(expect_unary_f64(intrinsic, args)?.is_finite())
        }
        RuntimeIntrinsic::StdF64IsSignPositive => {
            RuntimeValue::Bool(expect_unary_f64(intrinsic, args)?.is_sign_positive())
        }
        RuntimeIntrinsic::StdF64IsSignNegative => {
            RuntimeValue::Bool(expect_unary_f64(intrinsic, args)?.is_sign_negative())
        }
        RuntimeIntrinsic::StdF64ToBits => {
            RuntimeValue::u64(expect_unary_f64(intrinsic, args)?.to_bits())
        }
        RuntimeIntrinsic::StdF64FromBits => {
            RuntimeValue::F64(f64::from_bits(expect_unary_u64(intrinsic, args)?))
        }
        RuntimeIntrinsic::StdF64ToF32 => {
            RuntimeValue::F32(narrow_f64_to_f32(expect_unary_f64(intrinsic, args)?))
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "std.f64.to_f32 is the explicit Arcweft narrowing operation"
)]
fn narrow_f64_to_f32(value: f64) -> f32 {
    value as f32
}

fn expect_unary_f32(
    intrinsic: RuntimeIntrinsic,
    args: &[RuntimeValue],
) -> Result<f32, RuntimeEvalError> {
    match args {
        [RuntimeValue::F32(value)] => Ok(*value),
        _ => Err(float_intrinsic_arg_error(intrinsic, "f32", args)),
    }
}

fn expect_binary_f32(
    intrinsic: RuntimeIntrinsic,
    args: &[RuntimeValue],
) -> Result<(f32, f32), RuntimeEvalError> {
    match args {
        [RuntimeValue::F32(lhs), RuntimeValue::F32(rhs)] => Ok((*lhs, *rhs)),
        _ => Err(float_intrinsic_arg_error(intrinsic, "f32, f32", args)),
    }
}

fn expect_ternary_f32(
    intrinsic: RuntimeIntrinsic,
    args: &[RuntimeValue],
) -> Result<(f32, f32, f32), RuntimeEvalError> {
    match args {
        [
            RuntimeValue::F32(a),
            RuntimeValue::F32(b),
            RuntimeValue::F32(c),
        ] => Ok((*a, *b, *c)),
        _ => Err(float_intrinsic_arg_error(intrinsic, "f32, f32, f32", args)),
    }
}

fn expect_unary_f64(
    intrinsic: RuntimeIntrinsic,
    args: &[RuntimeValue],
) -> Result<f64, RuntimeEvalError> {
    match args {
        [RuntimeValue::F64(value)] => Ok(*value),
        _ => Err(float_intrinsic_arg_error(intrinsic, "f64", args)),
    }
}

fn expect_binary_f64(
    intrinsic: RuntimeIntrinsic,
    args: &[RuntimeValue],
) -> Result<(f64, f64), RuntimeEvalError> {
    match args {
        [RuntimeValue::F64(lhs), RuntimeValue::F64(rhs)] => Ok((*lhs, *rhs)),
        _ => Err(float_intrinsic_arg_error(intrinsic, "f64, f64", args)),
    }
}

fn expect_ternary_f64(
    intrinsic: RuntimeIntrinsic,
    args: &[RuntimeValue],
) -> Result<(f64, f64, f64), RuntimeEvalError> {
    match args {
        [
            RuntimeValue::F64(a),
            RuntimeValue::F64(b),
            RuntimeValue::F64(c),
        ] => Ok((*a, *b, *c)),
        _ => Err(float_intrinsic_arg_error(intrinsic, "f64, f64, f64", args)),
    }
}

fn expect_unary_u32(
    intrinsic: RuntimeIntrinsic,
    args: &[RuntimeValue],
) -> Result<u32, RuntimeEvalError> {
    match args {
        [RuntimeValue::UInt(value)] => value
            .try_into_u32()
            .ok_or_else(|| float_intrinsic_arg_error(intrinsic, "u32", args)),
        _ => Err(float_intrinsic_arg_error(intrinsic, "u32", args)),
    }
}

fn expect_unary_u64(
    intrinsic: RuntimeIntrinsic,
    args: &[RuntimeValue],
) -> Result<u64, RuntimeEvalError> {
    match args {
        [RuntimeValue::UInt(value)] => value
            .exact_u64()
            .ok_or_else(|| float_intrinsic_arg_error(intrinsic, "u64", args)),
        _ => Err(float_intrinsic_arg_error(intrinsic, "u64", args)),
    }
}

fn float_intrinsic_arg_error(
    intrinsic: RuntimeIntrinsic,
    expected: &'static str,
    args: &[RuntimeValue],
) -> RuntimeEvalError {
    RuntimeEvalError::UnsupportedPure {
        name: intrinsic.as_label().to_owned(),
        reason: format!(
            "expected ({expected}), got ({})",
            args.iter()
                .map(runtime_value_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
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

fn evaluate_signed_integer_compare(
    lhs: RuntimeInt,
    op: RuntimeBinaryOp,
    rhs: RuntimeInt,
) -> Result<bool, RuntimeEvalError> {
    match (lhs, rhs) {
        (RuntimeInt::I8(lhs), RuntimeInt::I8(rhs)) => Ok(compare_ordered(&lhs, op, &rhs)),
        (RuntimeInt::I16(lhs), RuntimeInt::I16(rhs)) => Ok(compare_ordered(&lhs, op, &rhs)),
        (RuntimeInt::I32(lhs), RuntimeInt::I32(rhs)) => Ok(compare_ordered(&lhs, op, &rhs)),
        (RuntimeInt::I64(lhs), RuntimeInt::I64(rhs))
        | (RuntimeInt::ISize(lhs), RuntimeInt::ISize(rhs)) => Ok(compare_ordered(&lhs, op, &rhs)),
        (RuntimeInt::I128(lhs), RuntimeInt::I128(rhs)) => Ok(compare_ordered(&lhs, op, &rhs)),
        (lhs, rhs) => Err(unsupported_binary_error(
            op,
            &RuntimeValue::Int(lhs),
            &RuntimeValue::Int(rhs),
        )),
    }
}

fn evaluate_unsigned_integer_compare(
    lhs: RuntimeUInt,
    op: RuntimeBinaryOp,
    rhs: RuntimeUInt,
) -> Result<bool, RuntimeEvalError> {
    match (lhs, rhs) {
        (RuntimeUInt::U8(lhs), RuntimeUInt::U8(rhs)) => Ok(compare_ordered(&lhs, op, &rhs)),
        (RuntimeUInt::U16(lhs), RuntimeUInt::U16(rhs)) => Ok(compare_ordered(&lhs, op, &rhs)),
        (RuntimeUInt::U32(lhs), RuntimeUInt::U32(rhs)) => Ok(compare_ordered(&lhs, op, &rhs)),
        (RuntimeUInt::U64(lhs), RuntimeUInt::U64(rhs))
        | (RuntimeUInt::USize(lhs), RuntimeUInt::USize(rhs)) => Ok(compare_ordered(&lhs, op, &rhs)),
        (RuntimeUInt::U128(lhs), RuntimeUInt::U128(rhs)) => Ok(compare_ordered(&lhs, op, &rhs)),
        (lhs, rhs) => Err(unsupported_binary_error(
            op,
            &RuntimeValue::UInt(lhs),
            &RuntimeValue::UInt(rhs),
        )),
    }
}

fn evaluate_signed_integer_neg(value: RuntimeInt) -> RuntimeInt {
    match value {
        RuntimeInt::I8(value) => RuntimeInt::I8(value.wrapping_neg()),
        RuntimeInt::I16(value) => RuntimeInt::I16(value.wrapping_neg()),
        RuntimeInt::I32(value) => RuntimeInt::I32(value.wrapping_neg()),
        RuntimeInt::I64(value) => RuntimeInt::I64(value.wrapping_neg()),
        RuntimeInt::I128(value) => RuntimeInt::I128(value.wrapping_neg()),
        RuntimeInt::ISize(value) => RuntimeInt::ISize(value.wrapping_neg()),
    }
}

fn evaluate_signed_integer_op(
    lhs: RuntimeInt,
    op: RuntimeBinaryOp,
    rhs: RuntimeInt,
) -> Result<RuntimeInt, RuntimeEvalError> {
    match (lhs, rhs) {
        (RuntimeInt::I8(lhs), RuntimeInt::I8(rhs)) => {
            Ok(RuntimeInt::I8(evaluate_numeric_op(lhs, op, rhs)))
        }
        (RuntimeInt::I16(lhs), RuntimeInt::I16(rhs)) => {
            Ok(RuntimeInt::I16(evaluate_numeric_op(lhs, op, rhs)))
        }
        (RuntimeInt::I32(lhs), RuntimeInt::I32(rhs)) => {
            Ok(RuntimeInt::I32(evaluate_numeric_op(lhs, op, rhs)))
        }
        (RuntimeInt::I64(lhs), RuntimeInt::I64(rhs)) => {
            Ok(RuntimeInt::I64(evaluate_numeric_op(lhs, op, rhs)))
        }
        (RuntimeInt::I128(lhs), RuntimeInt::I128(rhs)) => {
            Ok(RuntimeInt::I128(evaluate_numeric_op(lhs, op, rhs)))
        }
        (RuntimeInt::ISize(lhs), RuntimeInt::ISize(rhs)) => {
            Ok(RuntimeInt::ISize(evaluate_numeric_op(lhs, op, rhs)))
        }
        (lhs, rhs) => Err(unsupported_binary_error(
            op,
            &RuntimeValue::Int(lhs),
            &RuntimeValue::Int(rhs),
        )),
    }
}

fn evaluate_unsigned_integer_op(
    lhs: RuntimeUInt,
    op: RuntimeBinaryOp,
    rhs: RuntimeUInt,
) -> Result<RuntimeUInt, RuntimeEvalError> {
    match (lhs, rhs) {
        (RuntimeUInt::U8(lhs), RuntimeUInt::U8(rhs)) => {
            Ok(RuntimeUInt::U8(evaluate_numeric_op(lhs, op, rhs)))
        }
        (RuntimeUInt::U16(lhs), RuntimeUInt::U16(rhs)) => {
            Ok(RuntimeUInt::U16(evaluate_numeric_op(lhs, op, rhs)))
        }
        (RuntimeUInt::U32(lhs), RuntimeUInt::U32(rhs)) => {
            Ok(RuntimeUInt::U32(evaluate_numeric_op(lhs, op, rhs)))
        }
        (RuntimeUInt::U64(lhs), RuntimeUInt::U64(rhs)) => {
            Ok(RuntimeUInt::U64(evaluate_numeric_op(lhs, op, rhs)))
        }
        (RuntimeUInt::U128(lhs), RuntimeUInt::U128(rhs)) => {
            Ok(RuntimeUInt::U128(evaluate_numeric_op(lhs, op, rhs)))
        }
        (RuntimeUInt::USize(lhs), RuntimeUInt::USize(rhs)) => {
            Ok(RuntimeUInt::USize(evaluate_numeric_op(lhs, op, rhs)))
        }
        (lhs, rhs) => Err(unsupported_binary_error(
            op,
            &RuntimeValue::UInt(lhs),
            &RuntimeValue::UInt(rhs),
        )),
    }
}

fn evaluate_f32_op(lhs: f32, op: RuntimeBinaryOp, rhs: f32) -> f32 {
    evaluate_numeric_op(lhs, op, rhs)
}

fn evaluate_f64_op(lhs: f64, op: RuntimeBinaryOp, rhs: f64) -> f64 {
    evaluate_numeric_op(lhs, op, rhs)
}

pub(crate) trait RuntimeDeterministicNumeric: Copy {
    fn add(lhs: Self, rhs: Self) -> Self;
    fn sub(lhs: Self, rhs: Self) -> Self;
    fn mul(lhs: Self, rhs: Self) -> Self;
    fn div(lhs: Self, rhs: Self) -> Self;
}

macro_rules! impl_wrapping_numeric {
    ($($ty:ty),* $(,)?) => {
        $(
            impl RuntimeDeterministicNumeric for $ty {
                fn add(lhs: Self, rhs: Self) -> Self {
                    lhs.wrapping_add(rhs)
                }

                fn sub(lhs: Self, rhs: Self) -> Self {
                    lhs.wrapping_sub(rhs)
                }

                fn mul(lhs: Self, rhs: Self) -> Self {
                    lhs.wrapping_mul(rhs)
                }

                fn div(lhs: Self, rhs: Self) -> Self {
                    lhs.wrapping_div(rhs)
                }
            }
        )*
    };
}

macro_rules! impl_float_numeric {
    ($($ty:ty),* $(,)?) => {
        $(
            impl RuntimeDeterministicNumeric for $ty {
                fn add(lhs: Self, rhs: Self) -> Self {
                    lhs + rhs
                }

                fn sub(lhs: Self, rhs: Self) -> Self {
                    lhs - rhs
                }

                fn mul(lhs: Self, rhs: Self) -> Self {
                    lhs * rhs
                }

                fn div(lhs: Self, rhs: Self) -> Self {
                    lhs / rhs
                }
            }
        )*
    };
}

impl_wrapping_numeric!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);
impl_float_numeric!(f32, f64);

impl RuntimeDeterministicNumeric for RuntimeISizeValue {
    fn add(lhs: Self, rhs: Self) -> Self {
        Self(lhs.0.wrapping_add(rhs.0))
    }

    fn sub(lhs: Self, rhs: Self) -> Self {
        Self(lhs.0.wrapping_sub(rhs.0))
    }

    fn mul(lhs: Self, rhs: Self) -> Self {
        Self(lhs.0.wrapping_mul(rhs.0))
    }

    fn div(lhs: Self, rhs: Self) -> Self {
        Self(lhs.0.wrapping_div(rhs.0))
    }
}

impl RuntimeDeterministicNumeric for RuntimeUSizeValue {
    fn add(lhs: Self, rhs: Self) -> Self {
        Self(lhs.0.wrapping_add(rhs.0))
    }

    fn sub(lhs: Self, rhs: Self) -> Self {
        Self(lhs.0.wrapping_sub(rhs.0))
    }

    fn mul(lhs: Self, rhs: Self) -> Self {
        Self(lhs.0.wrapping_mul(rhs.0))
    }

    fn div(lhs: Self, rhs: Self) -> Self {
        Self(lhs.0.wrapping_div(rhs.0))
    }
}

pub(crate) fn evaluate_numeric_op<T: RuntimeDeterministicNumeric>(
    lhs: T,
    op: RuntimeBinaryOp,
    rhs: T,
) -> T {
    match op {
        RuntimeBinaryOp::Add => T::add(lhs, rhs),
        RuntimeBinaryOp::Sub => T::sub(lhs, rhs),
        RuntimeBinaryOp::Mul => T::mul(lhs, rhs),
        RuntimeBinaryOp::Div => T::div(lhs, rhs),
        _ => unreachable!(),
    }
}

pub(crate) fn sum_i64_sequence_ref(items: &[RuntimeValue]) -> Result<i64, RuntimeEvalError> {
    items.iter().try_fold(0_i64, |acc, item| match item {
        RuntimeValue::Int(value) => {
            value
                .try_sum_as_i64()
                .map(|value| acc + value)
                .ok_or_else(|| RuntimeEvalError::UnsupportedBinary {
                    op: "+",
                    lhs: "int".to_owned(),
                    rhs: runtime_value_label(item),
                })
        }
        RuntimeValue::UInt(value) => {
            value
                .try_sum_as_i64()
                .map(|value| acc + value)
                .ok_or_else(|| RuntimeEvalError::UnsupportedBinary {
                    op: "+",
                    lhs: "int".to_owned(),
                    rhs: runtime_value_label(item),
                })
        }
        value => Err(RuntimeEvalError::UnsupportedBinary {
            op: "+",
            lhs: "int".to_owned(),
            rhs: runtime_value_label(value),
        }),
    })
}

pub(crate) fn materialize_i64_sequence(items: Vec<i64>) -> Vec<RuntimeValue> {
    items.into_iter().map(RuntimeValue::i64).collect()
}

pub fn runtime_sequence_values(values: Vec<RuntimeValue>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::values(values))
}

pub fn runtime_sequence_from_literal_values(values: Vec<RuntimeValue>) -> RuntimeValue {
    match values.first().cloned() {
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
        Some(RuntimeValue::Int(value)) => collect_runtime_int_dense_or_values(values, value),
        Some(RuntimeValue::UInt(value)) => collect_runtime_uint_dense_or_values(values, value),
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
        Some(RuntimeValue::EntityRef(_)) => collect_dense_or_values(
            values,
            take_entity_ref_value,
            RuntimeValue::EntityRef,
            runtime_sequence_dense_entity_refs,
        ),
        Some(RuntimeValue::Tuple(_)) => collect_tuple_columns_or_values(values),
        Some(RuntimeValue::Record(_)) => collect_record_columns_or_values(values),
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

fn collect_runtime_int_dense_or_values(
    values: Vec<RuntimeValue>,
    first: RuntimeInt,
) -> RuntimeValue {
    match first {
        RuntimeInt::I8(_) => collect_dense_or_values(
            values,
            take_i8_value,
            RuntimeValue::i8,
            runtime_sequence_dense_i8,
        ),
        RuntimeInt::I16(_) => collect_dense_or_values(
            values,
            take_i16_value,
            RuntimeValue::i16,
            runtime_sequence_dense_i16,
        ),
        RuntimeInt::I32(_) => collect_dense_or_values(
            values,
            take_i32_value,
            RuntimeValue::i32,
            runtime_sequence_dense_i32,
        ),
        RuntimeInt::I64(_) => collect_dense_or_values(
            values,
            take_i64_value,
            RuntimeValue::i64,
            runtime_sequence_dense_i64,
        ),
        RuntimeInt::I128(_) => collect_dense_or_values(
            values,
            take_i128_value,
            RuntimeValue::i128,
            runtime_sequence_dense_i128,
        ),
        RuntimeInt::ISize(_) => collect_dense_or_values(
            values,
            take_isize_value,
            RuntimeValue::isize,
            runtime_sequence_dense_isize,
        ),
    }
}

fn collect_runtime_uint_dense_or_values(
    values: Vec<RuntimeValue>,
    first: RuntimeUInt,
) -> RuntimeValue {
    match first {
        RuntimeUInt::U8(_) => collect_dense_or_values(
            values,
            take_u8_value,
            RuntimeValue::u8,
            runtime_sequence_dense_u8,
        ),
        RuntimeUInt::U16(_) => collect_dense_or_values(
            values,
            take_u16_value,
            RuntimeValue::u16,
            runtime_sequence_dense_u16,
        ),
        RuntimeUInt::U32(_) => collect_dense_or_values(
            values,
            take_u32_value,
            RuntimeValue::u32,
            runtime_sequence_dense_u32,
        ),
        RuntimeUInt::U64(_) => collect_dense_or_values(
            values,
            take_u64_value,
            RuntimeValue::u64,
            runtime_sequence_dense_u64,
        ),
        RuntimeUInt::U128(_) => collect_dense_or_values(
            values,
            take_u128_value,
            RuntimeValue::u128,
            runtime_sequence_dense_u128,
        ),
        RuntimeUInt::USize(_) => collect_dense_or_values(
            values,
            take_usize_value,
            RuntimeValue::usize,
            runtime_sequence_dense_usize,
        ),
    }
}

fn take_i8_value(value: RuntimeValue) -> Result<i8, RuntimeValue> {
    match value {
        RuntimeValue::Int(RuntimeInt::I8(value)) => Ok(value),
        value => Err(value),
    }
}

fn take_i16_value(value: RuntimeValue) -> Result<i16, RuntimeValue> {
    match value {
        RuntimeValue::Int(RuntimeInt::I16(value)) => Ok(value),
        value => Err(value),
    }
}

fn take_i32_value(value: RuntimeValue) -> Result<i32, RuntimeValue> {
    match value {
        RuntimeValue::Int(RuntimeInt::I32(value)) => Ok(value),
        value => Err(value),
    }
}

fn take_i64_value(value: RuntimeValue) -> Result<i64, RuntimeValue> {
    match value {
        RuntimeValue::Int(RuntimeInt::I64(value)) => Ok(value),
        value => Err(value),
    }
}

fn take_i128_value(value: RuntimeValue) -> Result<i128, RuntimeValue> {
    match value {
        RuntimeValue::Int(RuntimeInt::I128(value)) => Ok(value),
        value => Err(value),
    }
}

fn take_isize_value(value: RuntimeValue) -> Result<i64, RuntimeValue> {
    match value {
        RuntimeValue::Int(RuntimeInt::ISize(value)) => Ok(value),
        value => Err(value),
    }
}

fn take_u8_value(value: RuntimeValue) -> Result<u8, RuntimeValue> {
    match value {
        RuntimeValue::UInt(RuntimeUInt::U8(value)) => Ok(value),
        value => Err(value),
    }
}

fn take_u16_value(value: RuntimeValue) -> Result<u16, RuntimeValue> {
    match value {
        RuntimeValue::UInt(RuntimeUInt::U16(value)) => Ok(value),
        value => Err(value),
    }
}

fn take_u32_value(value: RuntimeValue) -> Result<u32, RuntimeValue> {
    match value {
        RuntimeValue::UInt(RuntimeUInt::U32(value)) => Ok(value),
        value => Err(value),
    }
}

fn take_u64_value(value: RuntimeValue) -> Result<u64, RuntimeValue> {
    match value {
        RuntimeValue::UInt(RuntimeUInt::U64(value)) => Ok(value),
        value => Err(value),
    }
}

fn take_u128_value(value: RuntimeValue) -> Result<u128, RuntimeValue> {
    match value {
        RuntimeValue::UInt(RuntimeUInt::U128(value)) => Ok(value),
        value => Err(value),
    }
}

fn take_usize_value(value: RuntimeValue) -> Result<u64, RuntimeValue> {
    match value {
        RuntimeValue::UInt(RuntimeUInt::USize(value)) => Ok(value),
        value => Err(value),
    }
}

fn take_f32_value(value: RuntimeValue) -> Result<f32, RuntimeValue> {
    match value {
        RuntimeValue::F32(value) => Ok(value),
        value => Err(value),
    }
}

fn take_f64_value(value: RuntimeValue) -> Result<f64, RuntimeValue> {
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

fn take_entity_ref_value(value: RuntimeValue) -> Result<String, RuntimeValue> {
    match value {
        RuntimeValue::EntityRef(value) => Ok(value),
        value => Err(value),
    }
}

fn collect_tuple_columns_or_values(values: Vec<RuntimeValue>) -> RuntimeValue {
    let mut rows = Vec::with_capacity(values.len());
    let mut iter = values.into_iter();
    while let Some(value) = iter.next() {
        let RuntimeValue::Tuple(items) = value else {
            let mut fallback = rows
                .into_iter()
                .map(RuntimeValue::Tuple)
                .collect::<Vec<_>>();
            fallback.push(value);
            fallback.extend(iter);
            return runtime_sequence_values(fallback);
        };
        rows.push(items);
    }
    tuple_rows_to_columnar(rows).map_or_else(runtime_sequence_values, |seq| {
        RuntimeValue::Seq(RuntimeSeq::TupleColumns(seq))
    })
}

fn tuple_rows_to_columnar(mut rows: Vec<Vec<RuntimeValue>>) -> Result<TupleSeq, Vec<RuntimeValue>> {
    let len = rows.len();
    let Some(first) = rows.first() else {
        return TupleSeq::new(0, Vec::new()).map_err(|_| Vec::new());
    };
    let width = first.len();
    if rows.iter().any(|row| row.len() != width) {
        return Err(rows.into_iter().map(RuntimeValue::Tuple).collect());
    }
    let mut columns = (0..width)
        .map(|_| Vec::with_capacity(len))
        .collect::<Vec<_>>();
    for row in &mut rows {
        for (ordinal, value) in row.drain(..).enumerate() {
            columns[ordinal].push(value);
        }
    }
    let columns = columns
        .into_iter()
        .map(runtime_sequence_from_literal_values)
        .map(|value| match value {
            RuntimeValue::Seq(seq) => seq,
            value => RuntimeSeq::Values(vec![value]),
        })
        .collect();
    TupleSeq::new(len, columns).map_err(|_| rows.into_iter().map(RuntimeValue::Tuple).collect())
}

fn collect_record_columns_or_values(values: Vec<RuntimeValue>) -> RuntimeValue {
    let mut rows = Vec::with_capacity(values.len());
    let mut iter = values.into_iter();
    while let Some(value) = iter.next() {
        let RuntimeValue::Record(fields) = value else {
            let mut fallback = rows
                .into_iter()
                .map(RuntimeValue::Record)
                .collect::<Vec<_>>();
            fallback.push(value);
            fallback.extend(iter);
            return runtime_sequence_values(fallback);
        };
        rows.push(fields);
    }
    record_rows_to_columnar(rows).map_or_else(runtime_sequence_values, |seq| {
        RuntimeValue::Seq(RuntimeSeq::RecordColumns(seq))
    })
}

fn record_rows_to_columnar(
    mut rows: Vec<Vec<RuntimeFieldValue>>,
) -> Result<RecordSeq, Vec<RuntimeValue>> {
    let len = rows.len();
    let Some(first) = rows.first() else {
        return RecordSeq::new(0, Vec::new()).map_err(|_| Vec::new());
    };
    let names = first
        .iter()
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();
    if rows
        .iter()
        .any(|row| !record_field_order_matches(row, &names))
    {
        return Err(rows.into_iter().map(RuntimeValue::Record).collect());
    }
    let mut columns = names
        .iter()
        .map(|name| (name.clone(), Vec::with_capacity(len)))
        .collect::<Vec<_>>();
    for row in &mut rows {
        for (ordinal, field) in row.drain(..).enumerate() {
            columns[ordinal].1.push(field.value);
        }
    }
    let fields = columns
        .into_iter()
        .map(|(name, values)| {
            let value = runtime_sequence_from_literal_values(values);
            let RuntimeValue::Seq(values) = value else {
                unreachable!("sequence literal lowering always returns a sequence");
            };
            RecordSeqField { name, values }
        })
        .collect();
    RecordSeq::new(len, fields).map_err(|_| rows.into_iter().map(RuntimeValue::Record).collect())
}

fn record_field_order_matches(row: &[RuntimeFieldValue], names: &[String]) -> bool {
    row.len() == names.len()
        && row
            .iter()
            .zip(names)
            .all(|(field, name)| field.name == *name)
}

pub fn runtime_sequence_repeat_value(value: &RuntimeValue, len: usize) -> RuntimeValue {
    match value {
        RuntimeValue::Unit => runtime_sequence_dense_units(len),
        RuntimeValue::Bool(value) => runtime_sequence_dense_bool(vec![*value; len]),
        RuntimeValue::Int(value) => repeat_runtime_int(*value, len),
        RuntimeValue::UInt(value) => repeat_runtime_uint(*value, len),
        RuntimeValue::F32(value) => runtime_sequence_dense_f32(vec![*value; len]),
        RuntimeValue::F64(value) => runtime_sequence_dense_f64(vec![*value; len]),
        RuntimeValue::Char(value) => runtime_sequence_dense_chars(vec![*value; len]),
        RuntimeValue::Duration(value) => runtime_sequence_dense_durations(vec![*value; len]),
        RuntimeValue::String(value) => runtime_sequence_dense_strings(vec![value.clone(); len]),
        RuntimeValue::EntityRef(value) => {
            runtime_sequence_dense_entity_refs(vec![value.clone(); len])
        }
        value => runtime_sequence_values(vec![value.clone(); len]),
    }
}

fn repeat_runtime_int(value: RuntimeInt, len: usize) -> RuntimeValue {
    match value {
        RuntimeInt::I8(value) => runtime_sequence_dense_i8(vec![value; len]),
        RuntimeInt::I16(value) => runtime_sequence_dense_i16(vec![value; len]),
        RuntimeInt::I32(value) => runtime_sequence_dense_i32(vec![value; len]),
        RuntimeInt::I64(value) => runtime_sequence_dense_i64(vec![value; len]),
        RuntimeInt::I128(value) => runtime_sequence_dense_i128(vec![value; len]),
        RuntimeInt::ISize(value) => runtime_sequence_dense_isize(vec![value; len]),
    }
}

fn repeat_runtime_uint(value: RuntimeUInt, len: usize) -> RuntimeValue {
    match value {
        RuntimeUInt::U8(value) => runtime_sequence_dense_u8(vec![value; len]),
        RuntimeUInt::U16(value) => runtime_sequence_dense_u16(vec![value; len]),
        RuntimeUInt::U32(value) => runtime_sequence_dense_u32(vec![value; len]),
        RuntimeUInt::U64(value) => runtime_sequence_dense_u64(vec![value; len]),
        RuntimeUInt::U128(value) => runtime_sequence_dense_u128(vec![value; len]),
        RuntimeUInt::USize(value) => runtime_sequence_dense_usize(vec![value; len]),
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

pub fn runtime_sequence_dense_f32(values: Vec<f32>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_f32(values))
}

pub fn runtime_sequence_dense_f64(values: Vec<f64>) -> RuntimeValue {
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
    Err(unsupported_binary_error(op, lhs, rhs))
}

fn unsupported_binary_error(
    op: RuntimeBinaryOp,
    lhs: &RuntimeValue,
    rhs: &RuntimeValue,
) -> RuntimeEvalError {
    RuntimeEvalError::UnsupportedBinary {
        op: runtime_binary_op_label(op),
        lhs: runtime_value_label(lhs),
        rhs: runtime_value_label(rhs),
    }
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

pub(crate) fn runtime_value_label(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "()".to_owned(),
        RuntimeValue::Bool(value) => value.to_string(),
        RuntimeValue::Int(value) => value.label(),
        RuntimeValue::UInt(value) => value.label(),
        RuntimeValue::F32(value) => value.to_string(),
        RuntimeValue::F64(value) => value.to_string(),
        RuntimeValue::MatrixF32(value) => {
            format!("matrix/f32/{}x{}", value.rows(), value.cols())
        }
        RuntimeValue::MatrixF64(value) => {
            format!("matrix/f64/{}x{}", value.rows(), value.cols())
        }
        RuntimeValue::TensorF32(value) => {
            format!("tensor/f32/{:?}", value.shape().dims())
        }
        RuntimeValue::TensorF64(value) => {
            format!("tensor/f64/{:?}", value.shape().dims())
        }
        RuntimeValue::String(value) | RuntimeValue::EntityRef(value) => value.clone(),
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
            RuntimeSeq::Dense(DenseSeq::EntityRefs(values)) => {
                format!("seq/entity_refs/{}", values.len())
            }
            RuntimeSeq::TupleColumns(values) => format!("seq/tuple_columns/{}", values.len()),
            RuntimeSeq::RecordColumns(values) => format!("seq/record_columns/{}", values.len()),
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
