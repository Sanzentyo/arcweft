use crate::awbc::schema::AwbcFunctionId;
use crate::entry::{
    RuntimeCallableId, RuntimeIdentityError, RuntimeSchemaError, RuntimeValueDigest,
};
use crate::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
use crate::pattern::{RuntimeCheckedType, RuntimePattern, RuntimeVariantIdentity};
use crate::plan::{
    RuntimeIteratorEvidence, RuntimePureHelperId, RuntimePureInputType, RuntimePureOutputType,
    RuntimeReceiverMode, RuntimeTraitMethodId,
};
use crate::time::LogicalDuration;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

mod env;
mod integer;
mod nesting;
mod nominal_record;
mod nominal_record_expr;
mod opaque;
mod option_value;
pub mod ownership;
mod range;
mod record_id;
mod sequence_constructors;
mod sequence_impls;

pub use integer::{RuntimeInt, RuntimeSignedIntWidth, RuntimeUInt, RuntimeUnsignedIntWidth};
pub use nesting::{MAX_RUNTIME_VALUE_NESTING_DEPTH, RuntimeValueNestingError};
pub use nominal_record::{
    RuntimeNominalRecordError, RuntimeNominalRecordLayout, RuntimeNominalRecordLayoutError,
    RuntimeNominalRecordLayoutField, RuntimeNominalRecordValue,
};
pub use nominal_record_expr::{
    RuntimeNominalRecordExpr, RuntimeNominalRecordFieldExpr, RuntimeNominalRecordInitializerError,
};
pub use opaque::{RuntimeOpaqueValue, RuntimeOpaqueValueError};
pub use option_value::{
    evaluate_core_option_is_some_intrinsic, evaluate_core_option_unwrap_intrinsic,
};
pub use range::{RuntimeIterator, RuntimeRange, RuntimeRangeIterator};
pub use record_id::{RuntimeRecordFieldId, RuntimeRecordFieldIdError};
pub use sequence_constructors::{
    runtime_sequence_dense_bool, runtime_sequence_dense_bytes, runtime_sequence_dense_chars,
    runtime_sequence_dense_durations, runtime_sequence_dense_entity_refs,
    runtime_sequence_dense_f32, runtime_sequence_dense_f64, runtime_sequence_dense_i8,
    runtime_sequence_dense_i16, runtime_sequence_dense_i32, runtime_sequence_dense_i64,
    runtime_sequence_dense_i128, runtime_sequence_dense_isize, runtime_sequence_dense_strings,
    runtime_sequence_dense_u8, runtime_sequence_dense_u16, runtime_sequence_dense_u32,
    runtime_sequence_dense_u64, runtime_sequence_dense_u128, runtime_sequence_dense_units,
    runtime_sequence_dense_usize, runtime_sequence_repeat_value,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeBinding {
    pub name: String,
    pub value: RuntimeValue,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeFunctionBody {
    Expr(Box<RuntimeExpr>),
    Awbc(AwbcFunctionId),
}

/// Captured runtime function value.
///
/// Captures are deterministic runtime bindings collected when a function
/// expression is evaluated. They are rebound before call arguments when the
/// function is applied.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeFunctionValue {
    pub params: Vec<String>,
    pub body: RuntimeFunctionBody,
    pub captures: Vec<RuntimeBinding>,
}

impl RuntimeFunctionValue {
    pub fn new(params: Vec<String>, body: RuntimeExpr, captures: Vec<RuntimeBinding>) -> Self {
        Self {
            params,
            body: RuntimeFunctionBody::Expr(Box::new(body)),
            captures,
        }
    }

    pub fn new_awbc(
        params: Vec<String>,
        function: AwbcFunctionId,
        captures: Vec<RuntimeBinding>,
    ) -> Self {
        Self {
            params,
            body: RuntimeFunctionBody::Awbc(function),
            captures,
        }
    }

    pub const fn expr_body(&self) -> Option<&RuntimeExpr> {
        match &self.body {
            RuntimeFunctionBody::Expr(body) => Some(body),
            RuntimeFunctionBody::Awbc(_) => None,
        }
    }

    pub fn arity(&self) -> usize {
        self.params.len()
    }

    #[must_use]
    pub fn partially_apply(&self, args: &[RuntimeValue]) -> Self {
        let mut captures = self.captures.clone();
        captures.extend(
            self.params
                .iter()
                .take(args.len())
                .zip(args)
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: value.clone(),
                }),
        );
        Self {
            params: self.params[args.len()..].to_vec(),
            body: self.body.clone(),
            captures,
        }
    }
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
    Range(RuntimeRange),
    Iterator(RuntimeIterator),
    EntityRef(String),
    Tuple(Vec<RuntimeValue>),
    Seq(RuntimeSeq),
    Record(Vec<RuntimeFieldValue>),
    NominalRecord(RuntimeNominalRecordValue),
    Opaque(RuntimeOpaqueValue),
    Function(RuntimeFunctionValue),
    Variant {
        owner: RuntimeVariantIdentity,
        ordinal: u32,
        name: String,
        payload: Option<Box<RuntimeValue>>,
    },
}

/// Runtime call target after syntax lowering.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeCallTarget {
    Intrinsic(RuntimeIntrinsic),
    Callable(RuntimeCallableId),
}

impl RuntimeCallTarget {
    pub fn try_from_label(label: impl Into<String>) -> Result<Self, RuntimeIdentityError> {
        let label = label.into();
        RuntimeIntrinsic::from_label(&label)
            .map(Self::Intrinsic)
            .map_or_else(|| RuntimeCallableId::try_new(label).map(Self::Callable), Ok)
    }

    pub const fn intrinsic(intrinsic: RuntimeIntrinsic) -> Self {
        Self::Intrinsic(intrinsic)
    }

    pub const fn callable(callable: RuntimeCallableId) -> Self {
        Self::Callable(callable)
    }

    pub const fn as_intrinsic(&self) -> Option<RuntimeIntrinsic> {
        match self {
            Self::Intrinsic(intrinsic) => Some(*intrinsic),
            Self::Callable(_) => None,
        }
    }

    pub fn as_label(&self) -> &str {
        match self {
            Self::Intrinsic(intrinsic) => intrinsic.as_label(),
            Self::Callable(callable) => callable.as_str(),
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
    CoreRange,
    CoreIterCollect,
    CoreIterIntoIter,
    CoreIterNext,
    CoreOptionIsSome,
    CoreOptionUnwrap,
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
            "core.range" => Some(Self::CoreRange),
            "core.iter.collect" => Some(Self::CoreIterCollect),
            "core.iter.into_iter" => Some(Self::CoreIterIntoIter),
            "core.iter.next" => Some(Self::CoreIterNext),
            "core.option.is_some" => Some(Self::CoreOptionIsSome),
            "core.option.unwrap" => Some(Self::CoreOptionUnwrap),
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
            Self::CoreRange => "core.range",
            Self::CoreIterCollect => "core.iter.collect",
            Self::CoreIterIntoIter => "core.iter.into_iter",
            Self::CoreIterNext => "core.iter.next",
            Self::CoreOptionIsSome => "core.option.is_some",
            Self::CoreOptionUnwrap => "core.option.unwrap",
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
            | Self::CoreRange
            | Self::CoreIterCollect
            | Self::CoreIterIntoIter
            | Self::CoreIterNext
            | Self::CoreOptionIsSome
            | Self::CoreOptionUnwrap
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
    /// Materializes the canonical runtime representation of `Result::Ok`.
    pub fn result_ok(value: RuntimeValue) -> Self {
        Self::Variant {
            owner: RuntimeVariantIdentity::Result,
            ordinal: 0,
            name: "Ok".to_owned(),
            payload: Some(Box::new(value)),
        }
    }

    /// Materializes the canonical runtime representation of `Result::Err`.
    pub fn result_err(error: RuntimeValue) -> Self {
        Self::Variant {
            owner: RuntimeVariantIdentity::Result,
            ordinal: 1,
            name: "Err".to_owned(),
            payload: Some(Box::new(error)),
        }
    }

    /// Returns this value as a nominal record without accepting anonymous
    /// structural records.
    #[must_use]
    pub const fn as_nominal_record(&self) -> Option<&RuntimeNominalRecordValue> {
        match self {
            Self::NominalRecord(record) => Some(record),
            _ => None,
        }
    }

    /// Encodes the deterministic replay/save identity of this value.
    pub fn try_canonical_bytes(
        &self,
        max_encoded_bytes: usize,
    ) -> Result<Vec<u8>, RuntimeSchemaError> {
        crate::entry::canonical_runtime_value_bytes(self, max_encoded_bytes)
    }

    /// Hashes the deterministic replay/save identity of this value.
    pub fn try_digest(
        &self,
        max_encoded_bytes: usize,
    ) -> Result<RuntimeValueDigest, RuntimeSchemaError> {
        let bytes = self.try_canonical_bytes(max_encoded_bytes)?;
        Ok(RuntimeValueDigest::from_bytes(blake3::hash(&bytes).into()))
    }

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

    pub fn as_identifier(&self) -> Option<&str> {
        match self {
            Self::EntityRef(value) | Self::String(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn try_i64(&self) -> Option<i64> {
        match self {
            Self::Int(value) => value.try_into_i64(),
            Self::UInt(value) => value.try_into_i64(),
            _ => None,
        }
    }

    pub fn try_u64(&self) -> Option<u64> {
        match self {
            Self::UInt(value) => value.try_into_u64(),
            Self::Int(value) => value
                .try_into_i64()
                .and_then(|value| u64::try_from(value).ok()),
            _ => None,
        }
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

/// Columnar storage for a sequence of homogeneous record values.
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

/// Generic backing store for one dense homogeneous sequence.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DenseSeqStorage<T> {
    values: Vec<T>,
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
    Range {
        start: Option<Box<RuntimeExpr>>,
        end: Option<Box<RuntimeExpr>>,
        inclusive: bool,
    },
    Record(Vec<RuntimeFieldExpr>),
    NominalRecord(RuntimeNominalRecordExpr),
    Variant {
        owner: RuntimeCheckedType,
        ordinal: u32,
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
    AssignField {
        target: Box<RuntimeExpr>,
        field: String,
        expr: Box<RuntimeExpr>,
        body: Box<RuntimeExpr>,
    },
    Call {
        callee: RuntimeCallTarget,
        args: Vec<RuntimeExpr>,
    },
    Function {
        params: Vec<String>,
        body: Box<RuntimeExpr>,
    },
    Apply {
        callee: Box<RuntimeExpr>,
        args: Vec<RuntimeExpr>,
    },
    TraitCall {
        callable: RuntimeTraitMethodId,
        receiver: Box<RuntimeExpr>,
        receiver_mode: RuntimeReceiverMode,
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
    Filter {
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
    /// Revalidates every checked nominal-record carrier reachable from this
    /// expression after an interim plan has been deserialized.
    #[allow(
        clippy::too_many_lines,
        reason = "carrier validation exhaustively traverses every recursive RuntimeExpr variant"
    )]
    pub fn validate_nominal_record_carriers(
        &self,
    ) -> Result<(), RuntimeNominalRecordInitializerError> {
        match self {
            Self::NominalRecord(record) => {
                record.validate()?;
                for initializer in record.initializers() {
                    initializer.value().validate_nominal_record_carriers()?;
                }
            }
            Self::Let { expr, body, .. } => {
                expr.validate_nominal_record_carriers()?;
                body.validate_nominal_record_carriers()?;
            }
            Self::Tuple(items) | Self::BracketSeq(items) => {
                validate_nominal_record_exprs(items)?;
            }
            Self::RepeatSeq { value, .. }
            | Self::Field { target: value, .. }
            | Self::ProjectTuple { target: value, .. }
            | Self::ProjectRecord { target: value, .. }
            | Self::Function { body: value, .. }
            | Self::SpreadArg(value)
            | Self::Sum { source: value }
            | Self::Unary { expr: value, .. } => value.validate_nominal_record_carriers()?,
            Self::Range { start, end, .. } => {
                if let Some(start) = start {
                    start.validate_nominal_record_carriers()?;
                }
                if let Some(end) = end {
                    end.validate_nominal_record_carriers()?;
                }
            }
            Self::Record(fields) => {
                for field in fields {
                    field.value.validate_nominal_record_carriers()?;
                }
            }
            Self::Variant { payload, .. } => {
                if let Some(payload) = payload {
                    payload.validate_nominal_record_carriers()?;
                }
            }
            Self::AssignField {
                target, expr, body, ..
            } => {
                target.validate_nominal_record_carriers()?;
                expr.validate_nominal_record_carriers()?;
                body.validate_nominal_record_carriers()?;
            }
            Self::Call { args, .. } | Self::PureCall { args, .. } => {
                validate_nominal_record_exprs(args)?;
            }
            Self::Apply { callee, args } => {
                callee.validate_nominal_record_carriers()?;
                validate_nominal_record_exprs(args)?;
            }
            Self::TraitCall { receiver, args, .. } | Self::MethodCall { receiver, args, .. } => {
                receiver.validate_nominal_record_carriers()?;
                validate_nominal_record_exprs(args)?;
            }
            Self::Map { source, body, .. } | Self::Filter { source, body, .. } => {
                source.validate_nominal_record_carriers()?;
                body.validate_nominal_record_carriers()?;
            }
            Self::Binary { lhs, rhs, .. } => {
                lhs.validate_nominal_record_carriers()?;
                rhs.validate_nominal_record_carriers()?;
            }
            Self::If {
                condition,
                then_expr,
                else_expr,
            } => {
                condition.validate_nominal_record_carriers()?;
                then_expr.validate_nominal_record_carriers()?;
                else_expr.validate_nominal_record_carriers()?;
            }
            Self::IfLet {
                expr,
                guard,
                then_expr,
                else_expr,
                ..
            } => {
                expr.validate_nominal_record_carriers()?;
                if let Some(guard) = guard {
                    guard.validate_nominal_record_carriers()?;
                }
                then_expr.validate_nominal_record_carriers()?;
                else_expr.validate_nominal_record_carriers()?;
            }
            Self::Match { scrutinee, arms } => {
                scrutinee.validate_nominal_record_carriers()?;
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        guard.validate_nominal_record_carriers()?;
                    }
                    arm.value.validate_nominal_record_carriers()?;
                }
            }
            Self::Value(_) | Self::Local(_) | Self::EntityRef(_) => {}
        }
        Ok(())
    }

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
            | Self::Range { .. }
            | Self::Record(_)
            | Self::NominalRecord(_)
            | Self::Variant { .. }
            | Self::Field { .. }
            | Self::ProjectTuple { .. }
            | Self::ProjectRecord { .. }
            | Self::AssignField { .. }
            | Self::Call { .. }
            | Self::Function { .. }
            | Self::Apply { .. }
            | Self::TraitCall { .. }
            | Self::PureCall { .. }
            | Self::SpreadArg(_)
            | Self::MethodCall { .. }
            | Self::Map { .. }
            | Self::Filter { .. }
            | Self::Sum { .. }
            | Self::IfLet { .. }
            | Self::Match { .. } => false,
        }
    }
}

fn validate_nominal_record_exprs(
    expressions: &[RuntimeExpr],
) -> Result<(), RuntimeNominalRecordInitializerError> {
    for expression in expressions {
        expression.validate_nominal_record_carriers()?;
    }
    Ok(())
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
            Self::Range { inclusive, .. } => f.write_str(if *inclusive {
                "range/inclusive"
            } else {
                "range"
            }),
            Self::Record(fields) => write!(f, "record/{}", fields.len()),
            Self::NominalRecord(record) => write!(
                f,
                "nominal_record/{}/{}",
                record.layout().nominal().as_str(),
                record.initializers().len()
            ),
            Self::Variant { name, .. } => write!(f, ".{name}"),
            Self::Field { field, .. } => write!(f, ".{field}"),
            Self::ProjectTuple { ordinal, .. } => write!(f, ".{ordinal}"),
            Self::ProjectRecord { ordinal, .. } => write!(f, ".#{ordinal}"),
            Self::AssignField { field, .. } => write!(f, "assign .{field}"),
            Self::Call { callee, .. } => write!(f, "{callee}()"),
            Self::Function { params, .. } => write!(f, "fn/{}", params.len()),
            Self::Apply { .. } => f.write_str("apply"),
            Self::TraitCall { callable, .. } => write!(f, "trait#{}()", callable.0),
            Self::PureCall { helper, .. } => write!(f, "pure#{}()", helper.0),
            Self::SpreadArg(expr) => write!(f, "{expr}..."),
            Self::MethodCall { method, .. } => write!(f, ".{method}()"),
            Self::Map { .. } => f.write_str("map"),
            Self::Filter { .. } => f.write_str("filter"),
            Self::Sum { .. } => f.write_str("sum"),
            Self::Unary { op, .. } => f.write_str(op.as_label()),
            Self::Binary { op, .. } => f.write_str(op.as_label()),
            Self::If { .. } => f.write_str("if"),
            Self::IfLet { .. } => f.write_str("if let"),
            Self::Match { .. } => f.write_str("match"),
        }
    }
}

impl fmt::Display for RuntimeUnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_label())
    }
}

impl fmt::Display for RuntimeBinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_label())
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

impl RuntimeUnaryOp {
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Not => "!",
            Self::Neg => "-",
        }
    }
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

impl RuntimeBinaryOp {
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::And => "&&",
            Self::Or => "||",
        }
    }

    fn unsupported_error(self, lhs: &RuntimeValue, rhs: &RuntimeValue) -> RuntimeEvalError {
        RuntimeEvalError::UnsupportedBinary {
            op: self.as_label(),
            lhs: runtime_value_label(lhs),
            rhs: runtime_value_label(rhs),
        }
    }
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
    #[error("invalid entity target `{target}`: {reason}")]
    InvalidEntityTarget { target: String, reason: String },
    #[error("expected bracket sequence expression, found {0}")]
    ExpectedBracketSeq(String),
    #[error("runtime range is invalid: {reason}")]
    InvalidRange { reason: String },
    #[error("field `{field}` does not exist on {value}")]
    MissingField { field: String, value: String },
    #[error("cannot assign field `{field}` on {value}")]
    InvalidFieldAssignment { field: String, value: String },
    #[error("spread argument requires a tuple or bracket sequence, found {0}")]
    InvalidSpread(String),
    #[error("spread argument cannot be evaluated outside a call argument list")]
    SpreadOutsideCall,
    #[error("expected runtime function expression, found {0}")]
    ExpectedFunction(String),
    #[error("runtime function expected {expected} argument(s), found {found}")]
    FunctionArgumentCount { expected: usize, found: usize },
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
    #[error("runtime trait method id {0} does not exist")]
    UnknownTraitMethod(usize),
    #[error("trait method `{method}` expected {expected} argument(s), found {found}")]
    TraitMethodArgumentCount {
        method: String,
        expected: usize,
        found: usize,
    },
    #[error("trait method `{method}` cannot update receiver `{receiver}`")]
    InvalidTraitReceiverUpdate { method: String, receiver: String },
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
    #[error("audio command error: {0}")]
    Audio(String),
    #[error("runtime effect error: {0}")]
    Effect(String),
    #[error("audio command expected {expected}, found {actual}")]
    AudioValue {
        expected: &'static str,
        actual: String,
    },
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
            op: op.as_label(),
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
            (lhs, rhs) => Err(op.unsupported_error(&lhs, &rhs)),
        },
        RuntimeBinaryOp::Or => match (lhs, rhs) {
            (RuntimeValue::Bool(lhs), RuntimeValue::Bool(rhs)) => {
                Ok(RuntimeValue::Bool(lhs || rhs))
            }
            (lhs, rhs) => Err(op.unsupported_error(&lhs, &rhs)),
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
                (lhs, rhs) => Err(op.unsupported_error(&lhs, &rhs)),
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
            (lhs, rhs) => Err(op.unsupported_error(&lhs, &rhs)),
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
        (lhs, rhs) => Err(op.unsupported_error(&RuntimeValue::Int(lhs), &RuntimeValue::Int(rhs))),
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
        (lhs, rhs) => Err(op.unsupported_error(&RuntimeValue::UInt(lhs), &RuntimeValue::UInt(rhs))),
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
        (lhs, rhs) => Err(op.unsupported_error(&RuntimeValue::Int(lhs), &RuntimeValue::Int(rhs))),
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
        (lhs, rhs) => Err(op.unsupported_error(&RuntimeValue::UInt(lhs), &RuntimeValue::UInt(rhs))),
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

pub fn evaluate_core_range_intrinsic(
    args: &[RuntimeValue],
) -> Result<RuntimeValue, RuntimeEvalError> {
    let [start, end, RuntimeValue::Bool(inclusive)] = args else {
        return Err(RuntimeEvalError::InvalidRange {
            reason: format!(
                "core.range expected (start|(), end|(), bool), got ({})",
                args.iter()
                    .map(runtime_value_label)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    };
    let start = range_intrinsic_bound(start);
    let end = range_intrinsic_bound(end);
    RuntimeRange::new(start, end, *inclusive).map(RuntimeValue::Range)
}

fn range_intrinsic_bound(value: &RuntimeValue) -> Option<RuntimeValue> {
    if matches!(value, RuntimeValue::Unit) {
        None
    } else {
        Some(value.clone())
    }
}

pub fn evaluate_core_iter_collect_intrinsic(
    value: RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    RuntimeIterator::from_value(value)
        .map(Iterator::collect)
        .map(runtime_sequence_values)
        .map_err(|value| RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(&value)))
}

pub fn evaluate_core_iter_into_iter_intrinsic(
    value: RuntimeValue,
    evidence: &RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    let RuntimeValue::String(label) = evidence else {
        return Err(RuntimeEvalError::ExpectedBracketSeq(format!(
            "iterator evidence must be a string label, found {}",
            runtime_value_label(evidence)
        )));
    };
    let Some(evidence) = RuntimeIteratorEvidence::from_awbc_label(label) else {
        return Err(RuntimeEvalError::ExpectedBracketSeq(format!(
            "unknown iterator evidence `{label}`"
        )));
    };
    RuntimeIterator::from_value_with_evidence(value, &evidence)
        .map(RuntimeValue::Iterator)
        .map_err(|value| RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(&value)))
}

pub fn evaluate_core_iter_next_intrinsic(
    value: RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    let RuntimeValue::Iterator(mut iterator) = value else {
        return Err(RuntimeEvalError::ExpectedBracketSeq(format!(
            "core.iter.next expected iterator, found {}",
            runtime_value_label(&value)
        )));
    };
    let item = iterator
        .next()
        .map_or_else(RuntimeValue::option_none, RuntimeValue::option_some);
    Ok(RuntimeValue::Tuple(vec![
        RuntimeValue::Iterator(iterator),
        item,
    ]))
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

pub(crate) fn runtime_value_into_sequence_values(
    value: RuntimeValue,
) -> Result<Vec<RuntimeValue>, RuntimeValue> {
    match value {
        RuntimeValue::Seq(seq) => Ok(seq.into_values()),
        RuntimeValue::Tuple(values) => Ok(values),
        value => Err(value),
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
        RuntimeValue::Range(value) => value.label(),
        RuntimeValue::Iterator(_) => "iterator".to_owned(),
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
        RuntimeValue::NominalRecord(record) => {
            format!(
                "nominal-record/{}/{}",
                record.type_id().as_str(),
                record.fields().len()
            )
        }
        RuntimeValue::Opaque(value) => format!("opaque/{}", value.producer().as_str()),
        RuntimeValue::Function(function) => format!("function/{}", function.arity()),
        RuntimeValue::Variant { name, payload, .. } => {
            if payload.is_some() {
                format!(".{name}(...)")
            } else {
                format!(".{name}")
            }
        }
    }
}
