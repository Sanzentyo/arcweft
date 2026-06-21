//! Native Cranelift adapter for Arcweft pure helper functions.
//!
//! The VM remains the semantic reference. This crate is intentionally outside
//! `arcweft-core` so native code generation, executable memory, and the small
//! function-pointer call boundary stay in an adapter layer.

mod batch;
mod compiled;
mod lower;
mod native_call;
use batch::{
    compile_small_int_with_inputs, compile_wide_int_batch_with_inputs,
    define_f32_rows_batch_function, define_f64_rows_batch_function, define_i32_rows_batch_function,
    define_i32_rows_batch_sum_function, define_i64_rows_batch_function,
    define_i64_rows_batch_sum_function, define_small_int_batch_with_inputs,
    define_small_int_with_inputs, define_u32_rows_batch_function,
    define_u32_rows_batch_sum_function, define_u64_rows_batch_function,
    define_u64_rows_batch_sum_function, small_int_arity_error,
};
use lower::{
    codegen_error, emit_object_bytes, f32_bindings, f64_bindings, i32_bindings, int_bindings,
    jit_module, lower_expr, lower_f32_expr, lower_f64_expr, lower_i32_expr, lower_input_value,
    lower_next_input_value, lower_u32_expr, lower_u64_expr, object_module,
    sanitize_symbol_component, u32_bindings, u64_bindings, validate_param_names,
};

use arcweft_core::pure::{
    PureFunctionBackend, PureFunctionBackendKind, PureFunctionRequest, PureFunctionResult,
    PureFunctionStats, RuntimeI64Args,
};
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeBinding, RuntimeCallTarget, RuntimeEvalError, RuntimeExpr,
    RuntimeISizeValue, RuntimeInt, RuntimeIntrinsic, RuntimeUInt, RuntimeUSizeValue,
    RuntimeUnaryOp, RuntimeValue,
};
use cranelift::codegen::ir::{BlockArg, MemFlags, Type, UserFuncName};
use cranelift::codegen::isa::OwnedTargetIsa;
use cranelift::jit::{JITBuilder, JITModule};
use cranelift::module::{FuncId, Linkage, Module, ModuleError, default_libcall_names};
use cranelift::prelude::{
    AbiParam, Configurable, FloatCC, FunctionBuilder, FunctionBuilderContext, InstBuilder, IntCC,
    Value, settings, types,
};
use cranelift_object::{ObjectBuilder, ObjectModule};
use std::collections::BTreeMap;
use thiserror::Error;

/// Native Cranelift backend for the current pure helper subset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CraneliftPureFunctionBackend;

/// Error produced while selecting, lowering, compiling, or invoking a helper.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CraneliftCodegenError {
    #[error("host is not supported by Cranelift: {0}")]
    UnsupportedHost(String),
    #[error("pure helper expression is not supported by Cranelift: {0}")]
    UnsupportedExpr(String),
    #[error("Cranelift backend error: {0}")]
    Backend(String),
}

/// Compiled no-argument native helper returning an `i64`.
pub struct CompiledPureI64 {
    _module: JITModule,
    code: *const u8,
    stats: PureFunctionStats,
}

/// Compiled native helper returning an `i64` with selected runtime inputs.
///
/// The parameter names are Arcweft local binding names. Non-parameter locals
/// are captured from the request bindings as compile-time constants.
pub struct CompiledPureI64Inputs {
    _module: JITModule,
    caller: native_call::I64InputCaller,
    batch_code: *const u8,
    batch_sum_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

/// Relocatable object output for a parameterized pure helper.
pub struct ObjectPureInputs {
    pub object_bytes: Vec<u8>,
    pub entry_symbol: String,
    pub batch_symbol: String,
    pub batch_sum_symbol: Option<String>,
    pub param_names: Vec<String>,
    pub stats: PureFunctionStats,
}

/// Relocatable object output for a batch-only pure helper.
pub struct ObjectPureBatchInputs {
    pub object_bytes: Vec<u8>,
    pub batch_symbol: String,
    pub batch_sum_symbol: String,
    pub param_names: Vec<String>,
    pub stats: PureFunctionStats,
}

/// Exact scalar storage kind used when emitting pure helper object artifacts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PureObjectInputKind {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    F32,
    F64,
}

/// One pure helper requested for build-time object artifact emission.
pub struct PureObjectBundleRequest<'a> {
    pub request: &'a PureFunctionRequest,
    pub kind: PureObjectInputKind,
    pub param_names: Vec<String>,
}

impl<'a> PureObjectBundleRequest<'a> {
    /// Creates one object-bundle request with Arcweft runtime input names.
    pub fn new(
        request: &'a PureFunctionRequest,
        kind: PureObjectInputKind,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            request,
            kind,
            param_names: param_names.into_iter().map(Into::into).collect(),
        }
    }
}

/// Relocatable object output containing multiple pure helpers.
pub struct ObjectPureBundle {
    pub object_bytes: Vec<u8>,
    pub helpers: Vec<ObjectPureBundleHelper>,
}

/// Symbol metadata for one helper inside an object bundle.
pub struct ObjectPureBundleHelper {
    pub name: String,
    pub kind: PureObjectInputKind,
    pub entrypoints: ObjectPureEntrypoints,
    pub param_names: Vec<String>,
    pub stats: PureFunctionStats,
}

/// Entrypoint shape exported for one pure helper object artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectPureEntrypoints {
    Entry {
        entry_symbol: String,
    },
    Scalar {
        entry_symbol: String,
        batch_symbol: String,
        batch_sum_symbol: Option<String>,
    },
    Batch {
        batch_symbol: String,
        batch_sum_symbol: String,
    },
}

impl ObjectPureEntrypoints {
    /// Returns the scalar entry symbol when this artifact exports one.
    pub fn entry_symbol(&self) -> Option<&str> {
        match self {
            Self::Entry { entry_symbol } | Self::Scalar { entry_symbol, .. } => Some(entry_symbol),
            Self::Batch { .. } => None,
        }
    }

    /// Returns the row-batch symbol when this artifact exports one.
    pub fn batch_symbol(&self) -> Option<&str> {
        match self {
            Self::Entry { .. } => None,
            Self::Scalar { batch_symbol, .. } | Self::Batch { batch_symbol, .. } => {
                Some(batch_symbol)
            }
        }
    }

    /// Returns the row-batch-sum symbol when this artifact exports one.
    pub fn batch_sum_symbol(&self) -> Option<&str> {
        match self {
            Self::Entry { .. } => None,
            Self::Scalar {
                batch_sum_symbol, ..
            } => batch_sum_symbol.as_deref(),
            Self::Batch {
                batch_sum_symbol, ..
            } => Some(batch_sum_symbol),
        }
    }

    /// Visits every exported symbol for this entrypoint shape.
    pub fn for_each_symbol(&self, mut visit: impl FnMut(&str)) {
        if let Some(symbol) = self.entry_symbol() {
            visit(symbol);
        }
        if let Some(symbol) = self.batch_symbol() {
            visit(symbol);
        }
        if let Some(symbol) = self.batch_sum_symbol() {
            visit(symbol);
        }
    }
}

/// Pure no-input `i64` helper function defined into a Cranelift module.
pub struct DefinedPureI64Entry {
    pub entry: FuncId,
    pub stats: PureFunctionStats,
}

/// Pure scalar helper functions defined into a Cranelift module.
///
/// This is the codegen-side artifact shared by JIT and future object/AOT
/// targets. It contains only Cranelift function identifiers and metadata; JIT
/// finalization and native function-pointer lookup happen in the JIT wrapper.
pub struct DefinedPureScalarInputs {
    pub entry: FuncId,
    pub batch: FuncId,
    pub batch_sum: FuncId,
    pub param_names: Vec<String>,
    pub stats: PureFunctionStats,
}

/// Pure floating-point helper functions defined into a Cranelift module.
pub struct DefinedPureFloatInputs {
    pub entry: FuncId,
    pub batch: FuncId,
    pub param_names: Vec<String>,
    pub stats: PureFunctionStats,
}

/// Pure small-integer helper functions defined into a Cranelift module.
pub struct DefinedPureSmallIntInputs {
    pub entry: FuncId,
    pub batch: FuncId,
    pub batch_sum: FuncId,
    pub param_names: Vec<String>,
    pub stats: PureFunctionStats,
}

/// Pure wide-integer batch helper functions defined into a Cranelift module.
pub struct DefinedPureSmallIntBatchInputs {
    pub batch: FuncId,
    pub batch_sum: FuncId,
    pub param_names: Vec<String>,
    pub stats: PureFunctionStats,
}

/// Deterministic benchmark batch function defined into a Cranelift module.
pub struct DefinedPureI64BenchmarkBatch {
    pub entry: FuncId,
    pub param_names: Vec<String>,
    pub stats: PureFunctionStats,
}

/// Compiled native helper returning an `i128` through pointer-based calls.
///
/// Scalar calls are lowered to a single-row batch. No by-value `i128` argument
/// or return type crosses the Rust FFI boundary.
pub struct CompiledPureI128BatchInputs {
    _module: JITModule,
    batch_code: *const u8,
    batch_sum_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

/// Compiled native helper returning an `i8` with selected runtime inputs.
pub struct CompiledPureI8Inputs {
    _module: JITModule,
    caller: native_call::I8InputCaller,
    batch_code: *const u8,
    batch_sum_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

/// Compiled native helper returning an `i16` with selected runtime inputs.
pub struct CompiledPureI16Inputs {
    _module: JITModule,
    caller: native_call::I16InputCaller,
    batch_code: *const u8,
    batch_sum_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

/// Compiled native helper returning an `i32` with selected runtime inputs.
///
/// This keeps non-i64 dense integer batches on a width-preserving JIT ABI
/// instead of widening through the i64 fast path.
pub struct CompiledPureI32Inputs {
    _module: JITModule,
    caller: native_call::I32InputCaller,
    batch_code: *const u8,
    batch_sum_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

/// Compiled native helper returning an `u32` with selected runtime inputs.
///
/// This keeps non-i64 dense integer batches on a width-preserving JIT ABI
/// instead of widening through the i64 fast path.
pub struct CompiledPureU32Inputs {
    _module: JITModule,
    caller: native_call::U32InputCaller,
    batch_code: *const u8,
    batch_sum_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

/// Compiled native helper returning an `u8` with selected runtime inputs.
pub struct CompiledPureU8Inputs {
    _module: JITModule,
    caller: native_call::U8InputCaller,
    batch_code: *const u8,
    batch_sum_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

/// Compiled native helper returning an `u16` with selected runtime inputs.
pub struct CompiledPureU16Inputs {
    _module: JITModule,
    caller: native_call::U16InputCaller,
    batch_code: *const u8,
    batch_sum_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

/// Compiled native helper returning an `u64` with selected runtime inputs.
///
/// This keeps wide unsigned dense integer batches on a width-preserving JIT ABI.
pub struct CompiledPureU64Inputs {
    _module: JITModule,
    caller: native_call::U64InputCaller,
    batch_code: *const u8,
    batch_sum_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

/// Compiled native helper returning an `u128` through pointer-based calls.
///
/// Scalar calls are lowered to a single-row batch. No by-value `u128` argument
/// or return type crosses the Rust FFI boundary.
pub struct CompiledPureU128BatchInputs {
    _module: JITModule,
    batch_code: *const u8,
    batch_sum_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

/// Compiled native helper returning an `f32` with selected runtime inputs.
pub struct CompiledPureF32Inputs {
    _module: JITModule,
    caller: native_call::F32InputCaller,
    batch_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

/// Compiled native helper returning an `f64` with selected runtime inputs.
pub struct CompiledPureF64Inputs {
    _module: JITModule,
    caller: native_call::F64InputCaller,
    batch_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

/// Compiled native batch runner for deterministic benchmark input series.
///
/// The emitted function receives `seed`, `sample`, and `iterations`, generates
/// the same bounded integer input series as `arcw jit check`, evaluates the
/// pure helper in a native loop, and returns the accumulator.
pub struct CompiledPureI64Batch {
    _module: JITModule,
    code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

#[derive(Clone, Copy, Debug)]
enum LoweredIntBinding {
    /// Literal bits for integer codegen. The lowering site selects the
    /// Cranelift type, so this does not imply an `i64` runtime ABI.
    Const(i64),
    Value(Value),
}

#[derive(Clone, Copy, Debug)]
enum LoweredSmallIntBinding {
    Const(SmallIntLiteral),
    Value(Value),
}

#[derive(Clone, Copy, Debug)]
enum SmallIntLiteral {
    Narrow(i64),
    I128(i128),
    U128(u128),
}

#[derive(Clone, Copy, Debug)]
enum LoweredF32Binding {
    Const(f32),
    Value(Value),
}

#[derive(Clone, Copy, Debug)]
enum LoweredF64Binding {
    Const(f64),
    Value(Value),
}

#[derive(Clone, Copy, Debug)]
enum SmallIntKind {
    I8,
    I16,
    I128,
    U8,
    U16,
    U128,
}

impl SmallIntKind {
    const fn label(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I128 => "i128",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U128 => "u128",
        }
    }

    const fn cranelift_type(self) -> Type {
        match self {
            Self::I8 | Self::U8 => types::I8,
            Self::I16 | Self::U16 => types::I16,
            Self::I128 | Self::U128 => types::I128,
        }
    }

    const fn byte_width(self) -> usize {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I128 | Self::U128 => 16,
        }
    }

    const fn signed(self) -> bool {
        matches!(self, Self::I8 | Self::I16 | Self::I128)
    }

    fn literal(self, value: &RuntimeValue) -> Option<SmallIntLiteral> {
        match (self, value) {
            (Self::I8, RuntimeValue::Int(RuntimeInt::I8(value))) => {
                Some(SmallIntLiteral::Narrow(i64::from(*value)))
            }
            (Self::I16, RuntimeValue::Int(RuntimeInt::I16(value))) => {
                Some(SmallIntLiteral::Narrow(i64::from(*value)))
            }
            (Self::I128, RuntimeValue::Int(RuntimeInt::I128(value))) => {
                Some(SmallIntLiteral::I128(*value))
            }
            (Self::U8, RuntimeValue::UInt(RuntimeUInt::U8(value))) => {
                Some(SmallIntLiteral::Narrow(i64::from(*value)))
            }
            (Self::U16, RuntimeValue::UInt(RuntimeUInt::U16(value))) => {
                Some(SmallIntLiteral::Narrow(i64::from(*value)))
            }
            (Self::U128, RuntimeValue::UInt(RuntimeUInt::U128(value))) => {
                Some(SmallIntLiteral::U128(*value))
            }
            _ => None,
        }
    }
}

impl PureFunctionBackend for CraneliftPureFunctionBackend {
    fn kind(&self) -> PureFunctionBackendKind {
        PureFunctionBackendKind::Jit
    }

    fn evaluate(
        &self,
        request: &PureFunctionRequest,
    ) -> Result<PureFunctionResult, RuntimeEvalError> {
        self.evaluate_jit(request)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: request.name.clone(),
                reason: error.to_string(),
            })
    }
}

impl CraneliftPureFunctionBackend {
    /// Compiles and runs a pure helper request through Cranelift.
    ///
    /// The first supported subset is deterministic `i64` arithmetic over
    /// literal and bound-local values, integer comparisons, `if` expressions,
    /// and the registered `add` helper.
    pub fn evaluate_jit(
        &self,
        request: &PureFunctionRequest,
    ) -> Result<PureFunctionResult, CraneliftCodegenError> {
        let compiled = self.compile_i64(request)?;
        let value = compiled.call();
        Ok(PureFunctionResult {
            backend: self.kind(),
            value: RuntimeValue::i64(value),
            stats: compiled.stats().clone(),
        })
    }

    /// Compiles a pure helper request to a reusable native function.
    pub fn compile_i64(
        &self,
        request: &PureFunctionRequest,
    ) -> Result<CompiledPureI64, CraneliftCodegenError> {
        let mut module = jit_module()?;
        let defined = define_i64_entry(&mut module, "arcweft_pure_helper", request)?;
        module.finalize_definitions().map_err(codegen_error)?;
        let code = module.get_finalized_function(defined.entry);

        Ok(CompiledPureI64 {
            _module: module,
            code,
            stats: defined.stats,
        })
    }

    /// Compiles a pure helper request to a reusable native function with runtime
    /// integer inputs.
    ///
    /// `param_names` names local bindings that become runtime `i64`
    /// parameters. Other integer locals are captured from the request.
    pub fn compile_i64_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureI64Inputs, CraneliftCodegenError> {
        let mut module = jit_module()?;
        let defined = define_i64_with_inputs(
            &mut module,
            "arcweft_pure_helper_inputs",
            request,
            param_names,
        )?;
        module.finalize_definitions().map_err(codegen_error)?;
        let code = module.get_finalized_function(defined.entry);
        let batch_code = module.get_finalized_function(defined.batch);
        let batch_sum_code = module.get_finalized_function(defined.batch_sum);
        let caller = native_call::I64InputCaller::from_code(code, defined.param_names.len())
            .ok_or_else(|| {
                CraneliftCodegenError::UnsupportedExpr(format!(
                    "JIT helper arity {} is outside the native call boundary",
                    defined.param_names.len()
                ))
            })?;

        Ok(CompiledPureI64Inputs {
            _module: module,
            caller,
            batch_code,
            batch_sum_code,
            param_names: defined.param_names,
            stats: defined.stats,
        })
    }

    /// Emits a relocatable object containing the parameterized `i64` helper
    /// entrypoint and flat-batch entrypoints.
    pub fn emit_object_i64_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<ObjectPureInputs, CraneliftCodegenError> {
        emit_object_i64_with_inputs(request, param_names)
    }

    /// Emits a relocatable object containing the parameterized `i32` helper
    /// entrypoint and flat-batch entrypoints.
    pub fn emit_object_i32_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<ObjectPureInputs, CraneliftCodegenError> {
        emit_object_i32_with_inputs(request, param_names)
    }

    /// Compiles a pure helper request to a reusable native `i8` function with
    /// runtime `i8` inputs.
    pub fn compile_i8_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureI8Inputs, CraneliftCodegenError> {
        let parts = compile_small_int_with_inputs(request, param_names, SmallIntKind::I8)?;
        let caller = native_call::I8InputCaller::from_code(parts.code, parts.param_names.len())
            .ok_or_else(|| small_int_arity_error(SmallIntKind::I8, parts.param_names.len()))?;
        Ok(CompiledPureI8Inputs {
            _module: parts.module,
            caller,
            batch_code: parts.batch_code,
            batch_sum_code: parts.batch_sum_code,
            param_names: parts.param_names,
            stats: parts.stats,
        })
    }

    /// Emits a relocatable object containing the parameterized `i8` helper
    /// entrypoint and flat-batch entrypoints.
    pub fn emit_object_i8_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<ObjectPureInputs, CraneliftCodegenError> {
        emit_object_i8_with_inputs(request, param_names)
    }

    /// Compiles a pure helper request to reusable native `i128` flat-batch
    /// functions with runtime `i128` inputs.
    ///
    /// The generated functions use pointer-based row buffers only. Scalar
    /// `i128` calls execute through a one-row batch so by-value `i128`
    /// arguments stay out of the native ABI. Runtime inputs are loaded and
    /// stored as full-width `i128` values. Full-width literals and captured
    /// constants are lowered from two 64-bit halves with `iconcat`, avoiding
    /// invalid `iconst.i128` construction.
    pub fn compile_i128_batch_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureI128BatchInputs, CraneliftCodegenError> {
        let parts = compile_wide_int_batch_with_inputs(request, param_names, SmallIntKind::I128)?;
        Ok(CompiledPureI128BatchInputs {
            _module: parts.module,
            batch_code: parts.batch_code,
            batch_sum_code: parts.batch_sum_code,
            param_names: parts.param_names,
            stats: parts.stats,
        })
    }

    /// Emits a relocatable object containing the parameterized `i128`
    /// flat-batch entrypoints.
    pub fn emit_object_i128_batch_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<ObjectPureBatchInputs, CraneliftCodegenError> {
        emit_object_i128_batch_with_inputs(request, param_names)
    }

    /// Compiles a pure helper request to a reusable native `i16` function with
    /// runtime `i16` inputs.
    pub fn compile_i16_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureI16Inputs, CraneliftCodegenError> {
        let parts = compile_small_int_with_inputs(request, param_names, SmallIntKind::I16)?;
        let caller = native_call::I16InputCaller::from_code(parts.code, parts.param_names.len())
            .ok_or_else(|| small_int_arity_error(SmallIntKind::I16, parts.param_names.len()))?;
        Ok(CompiledPureI16Inputs {
            _module: parts.module,
            caller,
            batch_code: parts.batch_code,
            batch_sum_code: parts.batch_sum_code,
            param_names: parts.param_names,
            stats: parts.stats,
        })
    }

    /// Emits a relocatable object containing the parameterized `i16` helper
    /// entrypoint and flat-batch entrypoints.
    pub fn emit_object_i16_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<ObjectPureInputs, CraneliftCodegenError> {
        emit_object_i16_with_inputs(request, param_names)
    }

    /// Compiles a pure helper request to a reusable native `i32` function with
    /// runtime `i32` inputs.
    pub fn compile_i32_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureI32Inputs, CraneliftCodegenError> {
        let mut module = jit_module()?;
        let defined = define_i32_with_inputs(
            &mut module,
            "arcweft_pure_i32_helper_inputs",
            request,
            param_names,
        )?;
        module.finalize_definitions().map_err(codegen_error)?;
        let code = module.get_finalized_function(defined.entry);
        let batch_code = module.get_finalized_function(defined.batch);
        let batch_sum_code = module.get_finalized_function(defined.batch_sum);
        let caller = native_call::I32InputCaller::from_code(code, defined.param_names.len())
            .ok_or_else(|| {
                CraneliftCodegenError::UnsupportedExpr(format!(
                    "JIT i32 helper arity {} is outside the native call boundary",
                    defined.param_names.len()
                ))
            })?;

        Ok(CompiledPureI32Inputs {
            _module: module,
            caller,
            batch_code,
            batch_sum_code,
            param_names: defined.param_names,
            stats: defined.stats,
        })
    }

    /// Compiles a pure helper request to a reusable native `u32` function with
    /// runtime `u32` inputs.
    pub fn compile_u32_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureU32Inputs, CraneliftCodegenError> {
        let mut module = jit_module()?;
        let defined = define_u32_with_inputs(
            &mut module,
            "arcweft_pure_u32_helper_inputs",
            request,
            param_names,
        )?;
        module.finalize_definitions().map_err(codegen_error)?;
        let code = module.get_finalized_function(defined.entry);
        let batch_code = module.get_finalized_function(defined.batch);
        let batch_sum_code = module.get_finalized_function(defined.batch_sum);
        let caller = native_call::U32InputCaller::from_code(code, defined.param_names.len())
            .ok_or_else(|| {
                CraneliftCodegenError::UnsupportedExpr(format!(
                    "JIT u32 helper arity {} is outside the native call boundary",
                    defined.param_names.len()
                ))
            })?;

        Ok(CompiledPureU32Inputs {
            _module: module,
            caller,
            batch_code,
            batch_sum_code,
            param_names: defined.param_names,
            stats: defined.stats,
        })
    }

    /// Emits a relocatable object containing the parameterized `u32` helper
    /// entrypoint and flat-batch entrypoints.
    pub fn emit_object_u32_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<ObjectPureInputs, CraneliftCodegenError> {
        emit_object_u32_with_inputs(request, param_names)
    }

    /// Compiles a pure helper request to a reusable native `u8` function with
    /// runtime `u8` inputs.
    pub fn compile_u8_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureU8Inputs, CraneliftCodegenError> {
        let parts = compile_small_int_with_inputs(request, param_names, SmallIntKind::U8)?;
        let caller = native_call::U8InputCaller::from_code(parts.code, parts.param_names.len())
            .ok_or_else(|| small_int_arity_error(SmallIntKind::U8, parts.param_names.len()))?;
        Ok(CompiledPureU8Inputs {
            _module: parts.module,
            caller,
            batch_code: parts.batch_code,
            batch_sum_code: parts.batch_sum_code,
            param_names: parts.param_names,
            stats: parts.stats,
        })
    }

    /// Emits a relocatable object containing the parameterized `u8` helper
    /// entrypoint and flat-batch entrypoints.
    pub fn emit_object_u8_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<ObjectPureInputs, CraneliftCodegenError> {
        emit_object_u8_with_inputs(request, param_names)
    }

    /// Compiles a pure helper request to a reusable native `u16` function with
    /// runtime `u16` inputs.
    pub fn compile_u16_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureU16Inputs, CraneliftCodegenError> {
        let parts = compile_small_int_with_inputs(request, param_names, SmallIntKind::U16)?;
        let caller = native_call::U16InputCaller::from_code(parts.code, parts.param_names.len())
            .ok_or_else(|| small_int_arity_error(SmallIntKind::U16, parts.param_names.len()))?;
        Ok(CompiledPureU16Inputs {
            _module: parts.module,
            caller,
            batch_code: parts.batch_code,
            batch_sum_code: parts.batch_sum_code,
            param_names: parts.param_names,
            stats: parts.stats,
        })
    }

    /// Emits a relocatable object containing the parameterized `u16` helper
    /// entrypoint and flat-batch entrypoints.
    pub fn emit_object_u16_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<ObjectPureInputs, CraneliftCodegenError> {
        emit_object_u16_with_inputs(request, param_names)
    }

    /// Compiles a pure helper request to reusable native `u128` flat-batch
    /// functions with runtime `u128` inputs.
    ///
    /// The generated functions use pointer-based row buffers only. Scalar
    /// `u128` calls execute through a one-row batch so by-value `u128`
    /// arguments stay out of the native ABI. Runtime inputs are loaded and
    /// stored as full-width `u128` values. Full-width literals and captured
    /// constants are lowered from two 64-bit halves with `iconcat`, avoiding
    /// invalid `iconst.i128` construction.
    pub fn compile_u128_batch_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureU128BatchInputs, CraneliftCodegenError> {
        let parts = compile_wide_int_batch_with_inputs(request, param_names, SmallIntKind::U128)?;
        Ok(CompiledPureU128BatchInputs {
            _module: parts.module,
            batch_code: parts.batch_code,
            batch_sum_code: parts.batch_sum_code,
            param_names: parts.param_names,
            stats: parts.stats,
        })
    }

    /// Emits a relocatable object containing the parameterized `u128`
    /// flat-batch entrypoints.
    pub fn emit_object_u128_batch_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<ObjectPureBatchInputs, CraneliftCodegenError> {
        emit_object_u128_batch_with_inputs(request, param_names)
    }

    /// Compiles a pure helper request to a reusable native `u64` function with
    /// runtime `u64` inputs.
    pub fn compile_u64_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureU64Inputs, CraneliftCodegenError> {
        let mut module = jit_module()?;
        let defined = define_u64_with_inputs(
            &mut module,
            "arcweft_pure_u64_helper_inputs",
            request,
            param_names,
        )?;
        module.finalize_definitions().map_err(codegen_error)?;
        let code = module.get_finalized_function(defined.entry);
        let batch_code = module.get_finalized_function(defined.batch);
        let batch_sum_code = module.get_finalized_function(defined.batch_sum);
        let caller = native_call::U64InputCaller::from_code(code, defined.param_names.len())
            .ok_or_else(|| {
                CraneliftCodegenError::UnsupportedExpr(format!(
                    "JIT u64 helper arity {} is outside the native call boundary",
                    defined.param_names.len()
                ))
            })?;

        Ok(CompiledPureU64Inputs {
            _module: module,
            caller,
            batch_code,
            batch_sum_code,
            param_names: defined.param_names,
            stats: defined.stats,
        })
    }

    /// Emits a relocatable object containing the parameterized `u64` helper
    /// entrypoint and flat-batch entrypoints.
    pub fn emit_object_u64_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<ObjectPureInputs, CraneliftCodegenError> {
        emit_object_u64_with_inputs(request, param_names)
    }

    /// Compiles a pure helper request to a reusable native `f32` function with
    /// runtime `f32` inputs.
    pub fn compile_f32_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureF32Inputs, CraneliftCodegenError> {
        let mut module = jit_module()?;
        let defined = define_f32_with_inputs(
            &mut module,
            "arcweft_pure_f32_helper_inputs",
            request,
            param_names,
        )?;
        module.finalize_definitions().map_err(codegen_error)?;
        let code = module.get_finalized_function(defined.entry);
        let batch_code = module.get_finalized_function(defined.batch);
        let caller = native_call::F32InputCaller::from_code(code, defined.param_names.len())
            .ok_or_else(|| {
                CraneliftCodegenError::UnsupportedExpr(format!(
                    "JIT f32 helper arity {} is outside the native call boundary",
                    defined.param_names.len()
                ))
            })?;

        Ok(CompiledPureF32Inputs {
            _module: module,
            caller,
            batch_code,
            param_names: defined.param_names,
            stats: defined.stats,
        })
    }

    /// Emits a relocatable object containing the parameterized `f32` helper
    /// entrypoint and flat-batch entrypoint.
    pub fn emit_object_f32_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<ObjectPureInputs, CraneliftCodegenError> {
        emit_object_f32_with_inputs(request, param_names)
    }

    /// Compiles a pure helper request to a reusable native `f64` function with
    /// runtime `f64` inputs.
    pub fn compile_f64_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureF64Inputs, CraneliftCodegenError> {
        let mut module = jit_module()?;
        let defined = define_f64_with_inputs(
            &mut module,
            "arcweft_pure_f64_helper_inputs",
            request,
            param_names,
        )?;
        module.finalize_definitions().map_err(codegen_error)?;
        let code = module.get_finalized_function(defined.entry);
        let batch_code = module.get_finalized_function(defined.batch);
        let caller = native_call::F64InputCaller::from_code(code, defined.param_names.len())
            .ok_or_else(|| {
                CraneliftCodegenError::UnsupportedExpr(format!(
                    "JIT f64 helper arity {} is outside the native call boundary",
                    defined.param_names.len()
                ))
            })?;

        Ok(CompiledPureF64Inputs {
            _module: module,
            caller,
            batch_code,
            param_names: defined.param_names,
            stats: defined.stats,
        })
    }

    /// Emits a relocatable object containing the parameterized `f64` helper
    /// entrypoint and flat-batch entrypoint.
    pub fn emit_object_f64_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<ObjectPureInputs, CraneliftCodegenError> {
        emit_object_f64_with_inputs(request, param_names)
    }

    /// Emits one relocatable object containing multiple pure helper artifacts.
    pub fn emit_object_bundle<'a>(
        &self,
        helpers: impl IntoIterator<Item = PureObjectBundleRequest<'a>>,
    ) -> Result<ObjectPureBundle, CraneliftCodegenError> {
        emit_object_bundle(helpers)
    }

    /// Compiles a batch benchmark runner for a pure helper request.
    pub fn compile_i64_batch(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureI64Batch, CraneliftCodegenError> {
        let mut module = jit_module()?;
        let defined = define_i64_benchmark_batch(
            &mut module,
            "arcweft_pure_helper_batch",
            request,
            param_names,
        )?;
        module.finalize_definitions().map_err(codegen_error)?;
        let code = module.get_finalized_function(defined.entry);

        Ok(CompiledPureI64Batch {
            _module: module,
            code,
            param_names: defined.param_names,
            stats: defined.stats,
        })
    }
}

/// Defines an `i64` pure helper entrypoint and row-batch entrypoints into a
/// Defines a no-input `i64` pure helper entrypoint into a Cranelift module
/// without finalizing or looking up native function pointers.
pub fn define_i64_entry<M>(
    module: &mut M,
    symbol_name: &str,
    request: &PureFunctionRequest,
) -> Result<DefinedPureI64Entry, CraneliftCodegenError>
where
    M: Module,
{
    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature.returns.push(AbiParam::new(types::I64));

    let entry = module
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, entry.as_u32());

    let bindings = int_bindings(&request.bindings)?;
    let mut stats = PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let block = builder.create_block();
        builder.switch_to_block(block);
        let value = lower_expr(&mut builder, &bindings, &request.expr, &mut stats)?;
        builder.ins().return_(&[value]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(entry, &mut ctx)
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);

    Ok(DefinedPureI64Entry { entry, stats })
}

/// Defines an `i64` pure helper entrypoint and row-batch entrypoints into a
/// Cranelift module without finalizing or looking up native function pointers.
pub fn define_i64_with_inputs<M>(
    module: &mut M,
    symbol_prefix: &str,
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<DefinedPureScalarInputs, CraneliftCodegenError>
where
    M: Module,
{
    let param_names = param_names
        .into_iter()
        .map(Into::into)
        .collect::<Vec<String>>();
    validate_param_names(&param_names)?;
    if param_names.len() > 4 {
        return Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "Cranelift i64 helper supports at most 4 runtime inputs, got {}",
            param_names.len()
        )));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature
        .params
        .extend(param_names.iter().map(|_| AbiParam::new(types::I64)));
    signature.returns.push(AbiParam::new(types::I64));

    let entry_name = format!("{symbol_prefix}_entry");
    let entry = module
        .declare_function(&entry_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, entry.as_u32());

    let captured_bindings = int_bindings(&request.bindings)?;
    let mut bindings = captured_bindings.clone();
    let mut stats = PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let block = builder.create_block();
        builder.append_block_params_for_function_params(block);
        builder.switch_to_block(block);
        let params = builder.block_params(block);
        for (name, value) in param_names.iter().zip(params.iter().copied()) {
            bindings.insert(name.clone(), LoweredIntBinding::Value(value));
        }
        let value = lower_expr(&mut builder, &bindings, &request.expr, &mut stats)?;
        builder.ins().return_(&[value]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(entry, &mut ctx)
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    let batch = define_i64_rows_batch_function(
        module,
        &format!("{symbol_prefix}_rows_batch"),
        &request.expr,
        &captured_bindings,
        &param_names,
    )?;
    let batch_sum = define_i64_rows_batch_sum_function(
        module,
        &format!("{symbol_prefix}_rows_batch_sum"),
        &request.expr,
        &captured_bindings,
        &param_names,
    )?;

    Ok(DefinedPureScalarInputs {
        entry,
        batch,
        batch_sum,
        param_names,
        stats,
    })
}

/// Defines the deterministic `arcw jit check` benchmark loop into a module.
///
/// This is intentionally separate from JIT finalization so the same Cranelift
/// IR can be emitted by object/AOT targets if the benchmark runner needs a
/// relocatable artifact.
pub fn define_i64_benchmark_batch<M>(
    module: &mut M,
    symbol_name: &str,
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<DefinedPureI64BenchmarkBatch, CraneliftCodegenError>
where
    M: Module,
{
    let param_names = param_names
        .into_iter()
        .map(Into::into)
        .collect::<Vec<String>>();
    validate_param_names(&param_names)?;
    if param_names.len() > 4 {
        return Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "Cranelift i64 benchmark batch supports at most 4 runtime inputs, got {}",
            param_names.len()
        )));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature.params.extend([
        AbiParam::new(types::I64),
        AbiParam::new(types::I64),
        AbiParam::new(types::I64),
    ]);
    signature.returns.push(AbiParam::new(types::I64));

    let entry = module
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, entry.as_u32());

    let captured_bindings = int_bindings(&request.bindings)?;
    let mut stats = PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        let seed = builder.block_params(entry_block)[0];
        let sample = builder.block_params(entry_block)[1];
        let iterations = builder.block_params(entry_block)[2];

        let loop_block = builder.create_block();
        let body_block = builder.create_block();
        let done_block = builder.create_block();
        builder.append_block_param(loop_block, types::I64);
        builder.append_block_param(loop_block, types::I64);
        for _ in &param_names {
            builder.append_block_param(loop_block, types::I64);
        }

        let zero = builder.ins().iconst(types::I64, 0);
        let initial_inputs = (0..param_names.len())
            .map(|param_index| lower_input_value(&mut builder, seed, sample, zero, param_index))
            .collect::<Vec<_>>();
        let mut initial_args = vec![BlockArg::from(zero), BlockArg::from(zero)];
        initial_args.extend(initial_inputs.iter().copied().map(BlockArg::from));
        builder.ins().jump(loop_block, &initial_args);

        builder.switch_to_block(loop_block);
        let index = builder.block_params(loop_block)[0];
        let accumulator = builder.block_params(loop_block)[1];
        let input_values = builder.block_params(loop_block)[2..].to_vec();
        let keep_going = builder
            .ins()
            .icmp(IntCC::UnsignedLessThan, index, iterations);
        builder
            .ins()
            .brif(keep_going, body_block, &[], done_block, &[]);

        builder.switch_to_block(body_block);
        let mut bindings = captured_bindings.clone();
        for (name, value) in param_names.iter().zip(input_values.iter().copied()) {
            bindings.insert(name.clone(), LoweredIntBinding::Value(value));
        }
        let value = lower_expr(&mut builder, &bindings, &request.expr, &mut stats)?;
        let next_accumulator = builder.ins().iadd(accumulator, value);
        let one = builder.ins().iconst(types::I64, 1);
        let next_index = builder.ins().iadd(index, one);
        let next_inputs = input_values
            .iter()
            .copied()
            .enumerate()
            .map(|(param_index, value)| lower_next_input_value(&mut builder, value, param_index))
            .collect::<Vec<_>>();
        let mut next_args = vec![BlockArg::from(next_index), BlockArg::from(next_accumulator)];
        next_args.extend(next_inputs.iter().copied().map(BlockArg::from));
        builder.ins().jump(loop_block, &next_args);

        builder.switch_to_block(done_block);
        builder.ins().return_(&[accumulator]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(entry, &mut ctx)
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);

    Ok(DefinedPureI64BenchmarkBatch {
        entry,
        param_names,
        stats,
    })
}

/// Emits one relocatable object containing multiple parameterized pure helpers.
pub fn emit_object_bundle<'a>(
    helpers: impl IntoIterator<Item = PureObjectBundleRequest<'a>>,
) -> Result<ObjectPureBundle, CraneliftCodegenError> {
    let mut module = object_module()?;
    let mut metadata = Vec::new();
    for (index, helper) in helpers.into_iter().enumerate() {
        let symbol_prefix = format!(
            "arcweft_pure_bundle_{index}_{}",
            sanitize_symbol_component(&helper.request.name)
        );
        metadata.push(define_object_bundle_helper(
            &mut module,
            &symbol_prefix,
            helper,
        )?);
    }
    if metadata.is_empty() {
        return Err(CraneliftCodegenError::UnsupportedExpr(
            "object bundle must contain at least one pure helper".to_owned(),
        ));
    }
    Ok(ObjectPureBundle {
        object_bytes: emit_object_bytes(module)?,
        helpers: metadata,
    })
}

fn define_object_bundle_helper<M>(
    module: &mut M,
    symbol_prefix: &str,
    helper: PureObjectBundleRequest<'_>,
) -> Result<ObjectPureBundleHelper, CraneliftCodegenError>
where
    M: Module,
{
    let name = helper.request.name.clone();
    let kind = helper.kind;
    match kind {
        PureObjectInputKind::I64 => {
            let defined =
                define_i64_with_inputs(module, symbol_prefix, helper.request, helper.param_names)?;
            Ok(scalar_bundle_helper(symbol_prefix, name, kind, defined))
        }
        PureObjectInputKind::I32 => {
            let defined =
                define_i32_with_inputs(module, symbol_prefix, helper.request, helper.param_names)?;
            Ok(scalar_bundle_helper(symbol_prefix, name, kind, defined))
        }
        PureObjectInputKind::U32 => {
            let defined =
                define_u32_with_inputs(module, symbol_prefix, helper.request, helper.param_names)?;
            Ok(scalar_bundle_helper(symbol_prefix, name, kind, defined))
        }
        PureObjectInputKind::U64 => {
            let defined =
                define_u64_with_inputs(module, symbol_prefix, helper.request, helper.param_names)?;
            Ok(scalar_bundle_helper(symbol_prefix, name, kind, defined))
        }
        PureObjectInputKind::F32 => {
            let defined =
                define_f32_with_inputs(module, symbol_prefix, helper.request, helper.param_names)?;
            Ok(float_bundle_helper(symbol_prefix, name, kind, defined))
        }
        PureObjectInputKind::F64 => {
            let defined =
                define_f64_with_inputs(module, symbol_prefix, helper.request, helper.param_names)?;
            Ok(float_bundle_helper(symbol_prefix, name, kind, defined))
        }
        PureObjectInputKind::I8 => small_int_bundle_helper(
            module,
            symbol_prefix,
            helper.request,
            helper.param_names,
            name,
            kind,
            SmallIntKind::I8,
        ),
        PureObjectInputKind::I16 => small_int_bundle_helper(
            module,
            symbol_prefix,
            helper.request,
            helper.param_names,
            name,
            kind,
            SmallIntKind::I16,
        ),
        PureObjectInputKind::U8 => small_int_bundle_helper(
            module,
            symbol_prefix,
            helper.request,
            helper.param_names,
            name,
            kind,
            SmallIntKind::U8,
        ),
        PureObjectInputKind::U16 => small_int_bundle_helper(
            module,
            symbol_prefix,
            helper.request,
            helper.param_names,
            name,
            kind,
            SmallIntKind::U16,
        ),
        PureObjectInputKind::I128 => wide_int_bundle_helper(
            module,
            symbol_prefix,
            helper.request,
            helper.param_names,
            name,
            kind,
            SmallIntKind::I128,
        ),
        PureObjectInputKind::U128 => wide_int_bundle_helper(
            module,
            symbol_prefix,
            helper.request,
            helper.param_names,
            name,
            kind,
            SmallIntKind::U128,
        ),
    }
}

fn scalar_bundle_helper(
    symbol_prefix: &str,
    name: String,
    kind: PureObjectInputKind,
    defined: DefinedPureScalarInputs,
) -> ObjectPureBundleHelper {
    ObjectPureBundleHelper {
        name,
        kind,
        entrypoints: ObjectPureEntrypoints::Scalar {
            entry_symbol: format!("{symbol_prefix}_entry"),
            batch_symbol: format!("{symbol_prefix}_rows_batch"),
            batch_sum_symbol: Some(format!("{symbol_prefix}_rows_batch_sum")),
        },
        param_names: defined.param_names,
        stats: defined.stats,
    }
}

fn float_bundle_helper(
    symbol_prefix: &str,
    name: String,
    kind: PureObjectInputKind,
    defined: DefinedPureFloatInputs,
) -> ObjectPureBundleHelper {
    ObjectPureBundleHelper {
        name,
        kind,
        entrypoints: ObjectPureEntrypoints::Scalar {
            entry_symbol: format!("{symbol_prefix}_entry"),
            batch_symbol: format!("{symbol_prefix}_rows_batch"),
            batch_sum_symbol: None,
        },
        param_names: defined.param_names,
        stats: defined.stats,
    }
}

fn small_int_bundle_helper<M>(
    module: &mut M,
    symbol_prefix: &str,
    request: &PureFunctionRequest,
    param_names: Vec<String>,
    name: String,
    kind: PureObjectInputKind,
    small_kind: SmallIntKind,
) -> Result<ObjectPureBundleHelper, CraneliftCodegenError>
where
    M: Module,
{
    let defined =
        define_small_int_with_inputs(module, symbol_prefix, request, param_names, small_kind)?;
    Ok(ObjectPureBundleHelper {
        name,
        kind,
        entrypoints: ObjectPureEntrypoints::Scalar {
            entry_symbol: format!("{symbol_prefix}_entry"),
            batch_symbol: format!("{symbol_prefix}_rows_batch"),
            batch_sum_symbol: Some(format!("{symbol_prefix}_rows_batch_sum")),
        },
        param_names: defined.param_names,
        stats: defined.stats,
    })
}

fn wide_int_bundle_helper<M>(
    module: &mut M,
    symbol_prefix: &str,
    request: &PureFunctionRequest,
    param_names: Vec<String>,
    name: String,
    kind: PureObjectInputKind,
    small_kind: SmallIntKind,
) -> Result<ObjectPureBundleHelper, CraneliftCodegenError>
where
    M: Module,
{
    let defined = define_small_int_batch_with_inputs(
        module,
        symbol_prefix,
        request,
        param_names,
        small_kind,
    )?;
    Ok(ObjectPureBundleHelper {
        name,
        kind,
        entrypoints: ObjectPureEntrypoints::Batch {
            batch_symbol: format!("{symbol_prefix}_rows_batch"),
            batch_sum_symbol: format!("{symbol_prefix}_rows_batch_sum"),
        },
        param_names: defined.param_names,
        stats: defined.stats,
    })
}

/// Emits a relocatable object containing the parameterized `i64` helper
/// entrypoint and flat-batch entrypoints.
pub fn emit_object_i64_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    let mut module = object_module()?;
    let symbol_prefix = format!("arcweft_pure_{}", sanitize_symbol_component(&request.name));
    let defined = define_i64_with_inputs(&mut module, &symbol_prefix, request, param_names)?;
    scalar_object_result(module, symbol_prefix, defined)
}

/// Emits a relocatable object containing the parameterized `i32` helper
/// entrypoint and flat-batch entrypoints.
pub fn emit_object_i32_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    let mut module = object_module()?;
    let symbol_prefix = format!("arcweft_pure_{}", sanitize_symbol_component(&request.name));
    let defined = define_i32_with_inputs(&mut module, &symbol_prefix, request, param_names)?;
    scalar_object_result(module, symbol_prefix, defined)
}

/// Emits a relocatable object containing the parameterized `u32` helper
/// entrypoint and flat-batch entrypoints.
pub fn emit_object_u32_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    let mut module = object_module()?;
    let symbol_prefix = format!("arcweft_pure_{}", sanitize_symbol_component(&request.name));
    let defined = define_u32_with_inputs(&mut module, &symbol_prefix, request, param_names)?;
    scalar_object_result(module, symbol_prefix, defined)
}

/// Emits a relocatable object containing the parameterized `u64` helper
/// entrypoint and flat-batch entrypoints.
pub fn emit_object_u64_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    let mut module = object_module()?;
    let symbol_prefix = format!("arcweft_pure_{}", sanitize_symbol_component(&request.name));
    let defined = define_u64_with_inputs(&mut module, &symbol_prefix, request, param_names)?;
    scalar_object_result(module, symbol_prefix, defined)
}

/// Emits a relocatable object containing the parameterized `f32` helper
/// entrypoint and flat-batch entrypoint.
pub fn emit_object_f32_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    let mut module = object_module()?;
    let symbol_prefix = format!("arcweft_pure_{}", sanitize_symbol_component(&request.name));
    let defined = define_f32_with_inputs(&mut module, &symbol_prefix, request, param_names)?;
    float_object_result(module, symbol_prefix, defined)
}

/// Emits a relocatable object containing the parameterized `f64` helper
/// entrypoint and flat-batch entrypoint.
pub fn emit_object_f64_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    let mut module = object_module()?;
    let symbol_prefix = format!("arcweft_pure_{}", sanitize_symbol_component(&request.name));
    let defined = define_f64_with_inputs(&mut module, &symbol_prefix, request, param_names)?;
    float_object_result(module, symbol_prefix, defined)
}

/// Emits a relocatable object containing the parameterized `i8` helper
/// entrypoint and flat-batch entrypoints.
pub fn emit_object_i8_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    emit_object_small_int_with_inputs(request, param_names, SmallIntKind::I8)
}

/// Emits a relocatable object containing the parameterized `i16` helper
/// entrypoint and flat-batch entrypoints.
pub fn emit_object_i16_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    emit_object_small_int_with_inputs(request, param_names, SmallIntKind::I16)
}

/// Emits a relocatable object containing the parameterized `u8` helper
/// entrypoint and flat-batch entrypoints.
pub fn emit_object_u8_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    emit_object_small_int_with_inputs(request, param_names, SmallIntKind::U8)
}

/// Emits a relocatable object containing the parameterized `u16` helper
/// entrypoint and flat-batch entrypoints.
pub fn emit_object_u16_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    emit_object_small_int_with_inputs(request, param_names, SmallIntKind::U16)
}

/// Emits a relocatable object containing the parameterized `i128` flat-batch
/// entrypoints.
pub fn emit_object_i128_batch_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<ObjectPureBatchInputs, CraneliftCodegenError> {
    emit_object_wide_int_batch_with_inputs(request, param_names, SmallIntKind::I128)
}

/// Emits a relocatable object containing the parameterized `u128` flat-batch
/// entrypoints.
pub fn emit_object_u128_batch_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<ObjectPureBatchInputs, CraneliftCodegenError> {
    emit_object_wide_int_batch_with_inputs(request, param_names, SmallIntKind::U128)
}

fn emit_object_small_int_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
    kind: SmallIntKind,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    let mut module = object_module()?;
    let symbol_prefix = format!("arcweft_pure_{}", sanitize_symbol_component(&request.name));
    let defined =
        define_small_int_with_inputs(&mut module, &symbol_prefix, request, param_names, kind)?;
    small_int_object_result(module, symbol_prefix, defined)
}

fn emit_object_wide_int_batch_with_inputs(
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
    kind: SmallIntKind,
) -> Result<ObjectPureBatchInputs, CraneliftCodegenError> {
    let mut module = object_module()?;
    let symbol_prefix = format!("arcweft_pure_{}", sanitize_symbol_component(&request.name));
    let defined = define_small_int_batch_with_inputs(
        &mut module,
        &symbol_prefix,
        request,
        param_names,
        kind,
    )?;
    batch_object_result(module, symbol_prefix, defined)
}

fn scalar_object_result(
    module: ObjectModule,
    symbol_prefix: String,
    defined: DefinedPureScalarInputs,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    Ok(ObjectPureInputs {
        object_bytes: emit_object_bytes(module)?,
        entry_symbol: format!("{symbol_prefix}_entry"),
        batch_symbol: format!("{symbol_prefix}_rows_batch"),
        batch_sum_symbol: Some(format!("{symbol_prefix}_rows_batch_sum")),
        param_names: defined.param_names,
        stats: defined.stats,
    })
}

fn small_int_object_result(
    module: ObjectModule,
    symbol_prefix: String,
    defined: DefinedPureSmallIntInputs,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    Ok(ObjectPureInputs {
        object_bytes: emit_object_bytes(module)?,
        entry_symbol: format!("{symbol_prefix}_entry"),
        batch_symbol: format!("{symbol_prefix}_rows_batch"),
        batch_sum_symbol: Some(format!("{symbol_prefix}_rows_batch_sum")),
        param_names: defined.param_names,
        stats: defined.stats,
    })
}

fn batch_object_result(
    module: ObjectModule,
    symbol_prefix: String,
    defined: DefinedPureSmallIntBatchInputs,
) -> Result<ObjectPureBatchInputs, CraneliftCodegenError> {
    Ok(ObjectPureBatchInputs {
        object_bytes: emit_object_bytes(module)?,
        batch_symbol: format!("{symbol_prefix}_rows_batch"),
        batch_sum_symbol: format!("{symbol_prefix}_rows_batch_sum"),
        param_names: defined.param_names,
        stats: defined.stats,
    })
}

fn float_object_result(
    module: ObjectModule,
    symbol_prefix: String,
    defined: DefinedPureFloatInputs,
) -> Result<ObjectPureInputs, CraneliftCodegenError> {
    Ok(ObjectPureInputs {
        object_bytes: emit_object_bytes(module)?,
        entry_symbol: format!("{symbol_prefix}_entry"),
        batch_symbol: format!("{symbol_prefix}_rows_batch"),
        batch_sum_symbol: None,
        param_names: defined.param_names,
        stats: defined.stats,
    })
}

/// Defines an `i32` pure helper entrypoint and row-batch entrypoints into a
/// Cranelift module without finalizing or looking up native function pointers.
pub fn define_i32_with_inputs<M>(
    module: &mut M,
    symbol_prefix: &str,
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<DefinedPureScalarInputs, CraneliftCodegenError>
where
    M: Module,
{
    let param_names = param_names
        .into_iter()
        .map(Into::into)
        .collect::<Vec<String>>();
    validate_param_names(&param_names)?;
    if param_names.len() > 4 {
        return Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "Cranelift i32 helper supports at most 4 runtime inputs, got {}",
            param_names.len()
        )));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature
        .params
        .extend(param_names.iter().map(|_| AbiParam::new(types::I32)));
    signature.returns.push(AbiParam::new(types::I32));

    let entry_name = format!("{symbol_prefix}_entry");
    let entry = module
        .declare_function(&entry_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, entry.as_u32());

    let captured_bindings = i32_bindings(&request.bindings)?;
    let mut bindings = captured_bindings.clone();
    let mut stats = PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let block = builder.create_block();
        builder.append_block_params_for_function_params(block);
        builder.switch_to_block(block);
        let params = builder.block_params(block);
        for (name, value) in param_names.iter().zip(params.iter().copied()) {
            bindings.insert(name.clone(), LoweredIntBinding::Value(value));
        }
        let value = lower_i32_expr(&mut builder, &bindings, &request.expr, &mut stats)?;
        builder.ins().return_(&[value]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(entry, &mut ctx)
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    let batch = define_i32_rows_batch_function(
        module,
        &format!("{symbol_prefix}_rows_batch"),
        &request.expr,
        &captured_bindings,
        &param_names,
    )?;
    let batch_sum = define_i32_rows_batch_sum_function(
        module,
        &format!("{symbol_prefix}_rows_batch_sum"),
        &request.expr,
        &captured_bindings,
        &param_names,
    )?;

    Ok(DefinedPureScalarInputs {
        entry,
        batch,
        batch_sum,
        param_names,
        stats,
    })
}

/// Defines a `u32` pure helper entrypoint and row-batch entrypoints into a
/// Cranelift module without finalizing or looking up native function pointers.
pub fn define_u32_with_inputs<M>(
    module: &mut M,
    symbol_prefix: &str,
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<DefinedPureScalarInputs, CraneliftCodegenError>
where
    M: Module,
{
    let param_names = param_names
        .into_iter()
        .map(Into::into)
        .collect::<Vec<String>>();
    validate_param_names(&param_names)?;
    if param_names.len() > 4 {
        return Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "Cranelift u32 helper supports at most 4 runtime inputs, got {}",
            param_names.len()
        )));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature
        .params
        .extend(param_names.iter().map(|_| AbiParam::new(types::I32)));
    signature.returns.push(AbiParam::new(types::I32));

    let entry_name = format!("{symbol_prefix}_entry");
    let entry = module
        .declare_function(&entry_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, entry.as_u32());

    let captured_bindings = u32_bindings(&request.bindings)?;
    let mut bindings = captured_bindings.clone();
    let mut stats = PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let block = builder.create_block();
        builder.append_block_params_for_function_params(block);
        builder.switch_to_block(block);
        let params = builder.block_params(block);
        for (name, value) in param_names.iter().zip(params.iter().copied()) {
            bindings.insert(name.clone(), LoweredIntBinding::Value(value));
        }
        let value = lower_u32_expr(&mut builder, &bindings, &request.expr, &mut stats)?;
        builder.ins().return_(&[value]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(entry, &mut ctx)
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    let batch = define_u32_rows_batch_function(
        module,
        &format!("{symbol_prefix}_rows_batch"),
        &request.expr,
        &captured_bindings,
        &param_names,
    )?;
    let batch_sum = define_u32_rows_batch_sum_function(
        module,
        &format!("{symbol_prefix}_rows_batch_sum"),
        &request.expr,
        &captured_bindings,
        &param_names,
    )?;

    Ok(DefinedPureScalarInputs {
        entry,
        batch,
        batch_sum,
        param_names,
        stats,
    })
}

/// Defines a `u64` pure helper entrypoint and row-batch entrypoints into a
/// Cranelift module without finalizing or looking up native function pointers.
pub fn define_u64_with_inputs<M>(
    module: &mut M,
    symbol_prefix: &str,
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<DefinedPureScalarInputs, CraneliftCodegenError>
where
    M: Module,
{
    let param_names = param_names
        .into_iter()
        .map(Into::into)
        .collect::<Vec<String>>();
    validate_param_names(&param_names)?;
    if param_names.len() > 4 {
        return Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "Cranelift u64 helper supports at most 4 runtime inputs, got {}",
            param_names.len()
        )));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature
        .params
        .extend(param_names.iter().map(|_| AbiParam::new(types::I64)));
    signature.returns.push(AbiParam::new(types::I64));

    let entry_name = format!("{symbol_prefix}_entry");
    let entry = module
        .declare_function(&entry_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, entry.as_u32());

    let captured_bindings = u64_bindings(&request.bindings)?;
    let mut bindings = captured_bindings.clone();
    let mut stats = PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let block = builder.create_block();
        builder.append_block_params_for_function_params(block);
        builder.switch_to_block(block);
        let params = builder.block_params(block);
        for (name, value) in param_names.iter().zip(params.iter().copied()) {
            bindings.insert(name.clone(), LoweredIntBinding::Value(value));
        }
        let value = lower_u64_expr(&mut builder, &bindings, &request.expr, &mut stats)?;
        builder.ins().return_(&[value]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(entry, &mut ctx)
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    let batch = define_u64_rows_batch_function(
        module,
        &format!("{symbol_prefix}_rows_batch"),
        &request.expr,
        &captured_bindings,
        &param_names,
    )?;
    let batch_sum = define_u64_rows_batch_sum_function(
        module,
        &format!("{symbol_prefix}_rows_batch_sum"),
        &request.expr,
        &captured_bindings,
        &param_names,
    )?;

    Ok(DefinedPureScalarInputs {
        entry,
        batch,
        batch_sum,
        param_names,
        stats,
    })
}

/// Defines an `f32` pure helper entrypoint and row-batch entrypoint into a
/// Cranelift module without finalizing or looking up native function pointers.
pub fn define_f32_with_inputs<M>(
    module: &mut M,
    symbol_prefix: &str,
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<DefinedPureFloatInputs, CraneliftCodegenError>
where
    M: Module,
{
    let param_names = param_names
        .into_iter()
        .map(Into::into)
        .collect::<Vec<String>>();
    validate_param_names(&param_names)?;
    if param_names.len() > 4 {
        return Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "Cranelift f32 helper supports at most 4 runtime inputs, got {}",
            param_names.len()
        )));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature
        .params
        .extend(param_names.iter().map(|_| AbiParam::new(types::F32)));
    signature.returns.push(AbiParam::new(types::F32));

    let entry_name = format!("{symbol_prefix}_entry");
    let entry = module
        .declare_function(&entry_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, entry.as_u32());

    let captured_bindings = f32_bindings(&request.bindings)?;
    let mut bindings = captured_bindings.clone();
    let mut stats = PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let block = builder.create_block();
        builder.append_block_params_for_function_params(block);
        builder.switch_to_block(block);
        let params = builder.block_params(block);
        for (name, value) in param_names.iter().zip(params.iter().copied()) {
            bindings.insert(name.clone(), LoweredF32Binding::Value(value));
        }
        let value = lower_f32_expr(&mut builder, &bindings, &request.expr, &mut stats)?;
        builder.ins().return_(&[value]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(entry, &mut ctx)
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    let batch = define_f32_rows_batch_function(
        module,
        &format!("{symbol_prefix}_rows_batch"),
        &request.expr,
        &captured_bindings,
        &param_names,
    )?;

    Ok(DefinedPureFloatInputs {
        entry,
        batch,
        param_names,
        stats,
    })
}

/// Defines an `f64` pure helper entrypoint and row-batch entrypoint into a
/// Cranelift module without finalizing or looking up native function pointers.
pub fn define_f64_with_inputs<M>(
    module: &mut M,
    symbol_prefix: &str,
    request: &PureFunctionRequest,
    param_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<DefinedPureFloatInputs, CraneliftCodegenError>
where
    M: Module,
{
    let param_names = param_names
        .into_iter()
        .map(Into::into)
        .collect::<Vec<String>>();
    validate_param_names(&param_names)?;
    if param_names.len() > 4 {
        return Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "Cranelift f64 helper supports at most 4 runtime inputs, got {}",
            param_names.len()
        )));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature
        .params
        .extend(param_names.iter().map(|_| AbiParam::new(types::F64)));
    signature.returns.push(AbiParam::new(types::F64));

    let entry_name = format!("{symbol_prefix}_entry");
    let entry = module
        .declare_function(&entry_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, entry.as_u32());

    let captured_bindings = f64_bindings(&request.bindings)?;
    let mut bindings = captured_bindings.clone();
    let mut stats = PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let block = builder.create_block();
        builder.append_block_params_for_function_params(block);
        builder.switch_to_block(block);
        let params = builder.block_params(block);
        for (name, value) in param_names.iter().zip(params.iter().copied()) {
            bindings.insert(name.clone(), LoweredF64Binding::Value(value));
        }
        let value = lower_f64_expr(&mut builder, &bindings, &request.expr, &mut stats)?;
        builder.ins().return_(&[value]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(entry, &mut ctx)
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    let batch = define_f64_rows_batch_function(
        module,
        &format!("{symbol_prefix}_rows_batch"),
        &request.expr,
        &captured_bindings,
        &param_names,
    )?;

    Ok(DefinedPureFloatInputs {
        entry,
        batch,
        param_names,
        stats,
    })
}

struct SmallIntCompiledParts {
    module: JITModule,
    code: *const u8,
    batch_code: *const u8,
    batch_sum_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

struct WideIntBatchCompiledParts {
    module: JITModule,
    batch_code: *const u8,
    batch_sum_code: *const u8,
    param_names: Vec<String>,
    stats: PureFunctionStats,
}

#[cfg(test)]
mod tests;
