//! Runtime pure helper acceleration adapters.
//!
//! This crate owns native acceleration state so `arcweft-core` can stay Sans I/O
//! and dependency-light.

mod accelerator_api;
mod call_backend;
mod compile;
mod external;
pub mod inference;
pub mod math;
#[cfg(test)]
mod tests;

use apache_avro::{Reader, Schema, Writer, types::Value as AvroValue};
use arcweft_core::{
    math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64},
    plan::{
        RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin, RuntimePureInputType,
        RuntimePureOutputType,
    },
    pure::{
        AotPureFunctionBackend, AotPureI64Plan, AotPureScalarPlan, PureFunctionRequest,
        PureFunctionStats, RuntimeExternalCallBackend, RuntimeFixedArgs, RuntimeFloat32Args,
        RuntimeFloat64Args, RuntimeI32Args, RuntimeI64Args, RuntimeMathCallBackend,
        RuntimePureCallBackend, RuntimePureScalar, RuntimePureScalarInteger, VmPureFunctionScratch,
    },
    step::RuntimePureCallStats,
    value::{
        DenseSeq, RuntimeBinding, RuntimeCallTarget, RuntimeEvalError, RuntimeExactInteger,
        RuntimeExactIntegerSlice, RuntimeExactIntegerSliceMut, RuntimeExpr, RuntimeIntrinsic,
        RuntimeSeq, RuntimeValue, runtime_sequence_dense_bytes, runtime_sequence_dense_usize,
    },
};
use arcweft_data::{
    Bytes, BytesFormat, Codec, DataError, DataErrorKind, DataFormat, DecodeOptions, EncodeOptions,
    FieldShape, Number, RecordPolicy, TypeShape, Value,
};
#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
use native_jit::PureObjectInputKind;
use native_jit::{
    CompiledPureF32Inputs, CompiledPureF64Inputs, CompiledPureI8Inputs, CompiledPureI16Inputs,
    CompiledPureI32Inputs, CompiledPureI64Inputs, CompiledPureI128BatchInputs,
    CompiledPureU8Inputs, CompiledPureU16Inputs, CompiledPureU32Inputs, CompiledPureU64Inputs,
    CompiledPureU128BatchInputs, CraneliftPureFunctionBackend,
};
use rayon::{ThreadPool, ThreadPoolBuilder, prelude::*};
use std::collections::BTreeMap;
use std::fmt;

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
mod native_jit {
    pub use arcweft_lang_jit_cranelift::{
        CompiledPureF32Inputs, CompiledPureF64Inputs, CompiledPureI8Inputs, CompiledPureI16Inputs,
        CompiledPureI32Inputs, CompiledPureI64Inputs, CompiledPureI128BatchInputs,
        CompiledPureU8Inputs, CompiledPureU16Inputs, CompiledPureU32Inputs, CompiledPureU64Inputs,
        CompiledPureU128BatchInputs, CraneliftPureFunctionBackend, PureObjectBundleRequest,
        PureObjectInputKind,
    };
}

#[cfg(not(all(feature = "native-jit", not(target_arch = "wasm32"))))]
mod native_jit {
    use arcweft_core::{
        pure::{PureFunctionRequest, RuntimeFixedArgs},
        value::{RuntimeISizeValue, RuntimeUSizeValue},
    };
    use std::fmt;

    #[derive(Debug)]
    pub struct NativeJitUnavailable;

    impl fmt::Display for NativeJitUnavailable {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("native Cranelift JIT is unavailable for this target")
        }
    }

    impl std::error::Error for NativeJitUnavailable {}

    pub struct CraneliftPureFunctionBackend;

    pub struct CompiledPureI64Inputs;
    pub struct CompiledPureI8Inputs;
    pub struct CompiledPureI16Inputs;
    pub struct CompiledPureI32Inputs;
    pub struct CompiledPureI128BatchInputs;
    pub struct CompiledPureU32Inputs;
    pub struct CompiledPureU8Inputs;
    pub struct CompiledPureU16Inputs;
    pub struct CompiledPureU64Inputs;
    pub struct CompiledPureU128BatchInputs;
    pub struct CompiledPureF32Inputs;
    pub struct CompiledPureF64Inputs;
    impl CraneliftPureFunctionBackend {
        pub fn compile_i64_with_inputs<'a, I>(
            &self,
            _request: &PureFunctionRequest,
            _input_names: I,
        ) -> Result<CompiledPureI64Inputs, NativeJitUnavailable>
        where
            I: IntoIterator<Item = &'a str>,
        {
            Err(NativeJitUnavailable)
        }

        pub fn compile_i32_with_inputs<'a, I>(
            &self,
            _request: &PureFunctionRequest,
            _input_names: I,
        ) -> Result<CompiledPureI32Inputs, NativeJitUnavailable>
        where
            I: IntoIterator<Item = &'a str>,
        {
            Err(NativeJitUnavailable)
        }

        pub fn compile_i8_with_inputs<'a, I>(
            &self,
            _request: &PureFunctionRequest,
            _input_names: I,
        ) -> Result<CompiledPureI8Inputs, NativeJitUnavailable>
        where
            I: IntoIterator<Item = &'a str>,
        {
            Err(NativeJitUnavailable)
        }

        pub fn compile_i16_with_inputs<'a, I>(
            &self,
            _request: &PureFunctionRequest,
            _input_names: I,
        ) -> Result<CompiledPureI16Inputs, NativeJitUnavailable>
        where
            I: IntoIterator<Item = &'a str>,
        {
            Err(NativeJitUnavailable)
        }

        pub fn compile_i128_batch_with_inputs<'a, I>(
            &self,
            _request: &PureFunctionRequest,
            _input_names: I,
        ) -> Result<CompiledPureI128BatchInputs, NativeJitUnavailable>
        where
            I: IntoIterator<Item = &'a str>,
        {
            Err(NativeJitUnavailable)
        }

        pub fn compile_u32_with_inputs<'a, I>(
            &self,
            _request: &PureFunctionRequest,
            _input_names: I,
        ) -> Result<CompiledPureU32Inputs, NativeJitUnavailable>
        where
            I: IntoIterator<Item = &'a str>,
        {
            Err(NativeJitUnavailable)
        }

        pub fn compile_u8_with_inputs<'a, I>(
            &self,
            _request: &PureFunctionRequest,
            _input_names: I,
        ) -> Result<CompiledPureU8Inputs, NativeJitUnavailable>
        where
            I: IntoIterator<Item = &'a str>,
        {
            Err(NativeJitUnavailable)
        }

        pub fn compile_u16_with_inputs<'a, I>(
            &self,
            _request: &PureFunctionRequest,
            _input_names: I,
        ) -> Result<CompiledPureU16Inputs, NativeJitUnavailable>
        where
            I: IntoIterator<Item = &'a str>,
        {
            Err(NativeJitUnavailable)
        }

        pub fn compile_u64_with_inputs<'a, I>(
            &self,
            _request: &PureFunctionRequest,
            _input_names: I,
        ) -> Result<CompiledPureU64Inputs, NativeJitUnavailable>
        where
            I: IntoIterator<Item = &'a str>,
        {
            Err(NativeJitUnavailable)
        }

        pub fn compile_u128_batch_with_inputs<'a, I>(
            &self,
            _request: &PureFunctionRequest,
            _input_names: I,
        ) -> Result<CompiledPureU128BatchInputs, NativeJitUnavailable>
        where
            I: IntoIterator<Item = &'a str>,
        {
            Err(NativeJitUnavailable)
        }

        pub fn compile_f32_with_inputs<'a, I>(
            &self,
            _request: &PureFunctionRequest,
            _input_names: I,
        ) -> Result<CompiledPureF32Inputs, NativeJitUnavailable>
        where
            I: IntoIterator<Item = &'a str>,
        {
            Err(NativeJitUnavailable)
        }

        pub fn compile_f64_with_inputs<'a, I>(
            &self,
            _request: &PureFunctionRequest,
            _input_names: I,
        ) -> Result<CompiledPureF64Inputs, NativeJitUnavailable>
        where
            I: IntoIterator<Item = &'a str>,
        {
            Err(NativeJitUnavailable)
        }
    }

    impl CompiledPureI64Inputs {
        pub fn param_names(&self) -> &[String] {
            &[]
        }

        pub fn call(&self, _args: &[i64]) -> Result<i64, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_i64_args(
            &self,
            _args: RuntimeFixedArgs<i64>,
        ) -> Result<i64, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_flat_batch(
            &self,
            _flat_inputs: &[i64],
            _out: &mut [i64],
        ) -> Result<(), NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_flat_batch_sum(
            &self,
            _flat_inputs: &[i64],
            _rows: usize,
        ) -> Result<i64, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_isize(
            &self,
            _args: &[RuntimeISizeValue],
        ) -> Result<RuntimeISizeValue, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_isize_flat_batch(
            &self,
            _flat_inputs: &[RuntimeISizeValue],
            _out: &mut [RuntimeISizeValue],
        ) -> Result<(), NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_isize_flat_batch_sum(
            &self,
            _flat_inputs: &[RuntimeISizeValue],
            _rows: usize,
        ) -> Result<i64, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }
    }

    impl CompiledPureI32Inputs {
        pub fn call(&self, _args: &[i32]) -> Result<i32, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_flat_batch(
            &self,
            _flat_inputs: &[i32],
            _out: &mut [i32],
        ) -> Result<(), NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_flat_batch_sum(
            &self,
            _flat_inputs: &[i32],
            _rows: usize,
        ) -> Result<i64, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }
    }

    macro_rules! impl_small_int_unavailable {
        ($compiled:ty, $ty:ty) => {
            impl $compiled {
                pub fn call(&self, _args: &[$ty]) -> Result<$ty, NativeJitUnavailable> {
                    Err(NativeJitUnavailable)
                }

                pub fn call_flat_batch(
                    &self,
                    _flat_inputs: &[$ty],
                    _out: &mut [$ty],
                ) -> Result<(), NativeJitUnavailable> {
                    Err(NativeJitUnavailable)
                }

                pub fn call_flat_batch_sum(
                    &self,
                    _flat_inputs: &[$ty],
                    _rows: usize,
                ) -> Result<i64, NativeJitUnavailable> {
                    Err(NativeJitUnavailable)
                }
            }
        };
    }

    impl_small_int_unavailable!(CompiledPureI8Inputs, i8);
    impl_small_int_unavailable!(CompiledPureI16Inputs, i16);
    impl_small_int_unavailable!(CompiledPureU8Inputs, u8);
    impl_small_int_unavailable!(CompiledPureU16Inputs, u16);

    macro_rules! impl_wide_int_batch_unavailable {
        ($compiled:ty, $ty:ty) => {
            impl $compiled {
                pub fn call(&self, _args: &[$ty]) -> Result<$ty, NativeJitUnavailable> {
                    Err(NativeJitUnavailable)
                }

                pub fn call_flat_batch(
                    &self,
                    _flat_inputs: &[$ty],
                    _out: &mut [$ty],
                ) -> Result<(), NativeJitUnavailable> {
                    Err(NativeJitUnavailable)
                }

                pub fn call_flat_batch_sum(
                    &self,
                    _flat_inputs: &[$ty],
                    _rows: usize,
                ) -> Result<i64, NativeJitUnavailable> {
                    Err(NativeJitUnavailable)
                }
            }
        };
    }

    impl_wide_int_batch_unavailable!(CompiledPureI128BatchInputs, i128);
    impl_wide_int_batch_unavailable!(CompiledPureU128BatchInputs, u128);

    impl CompiledPureU32Inputs {
        pub fn call(&self, _args: &[u32]) -> Result<u32, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_flat_batch(
            &self,
            _flat_inputs: &[u32],
            _out: &mut [u32],
        ) -> Result<(), NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_flat_batch_sum(
            &self,
            _flat_inputs: &[u32],
            _rows: usize,
        ) -> Result<i64, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }
    }

    impl CompiledPureU64Inputs {
        pub fn call(&self, _args: &[u64]) -> Result<u64, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_flat_batch(
            &self,
            _flat_inputs: &[u64],
            _out: &mut [u64],
        ) -> Result<(), NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_flat_batch_sum(
            &self,
            _flat_inputs: &[u64],
            _rows: usize,
        ) -> Result<i64, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_usize(
            &self,
            _args: &[RuntimeUSizeValue],
        ) -> Result<RuntimeUSizeValue, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_usize_flat_batch(
            &self,
            _flat_inputs: &[RuntimeUSizeValue],
            _out: &mut [RuntimeUSizeValue],
        ) -> Result<(), NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_usize_flat_batch_sum(
            &self,
            _flat_inputs: &[RuntimeUSizeValue],
            _rows: usize,
        ) -> Result<i64, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }
    }

    impl CompiledPureF32Inputs {
        pub fn call(&self, _args: &[f32]) -> Result<f32, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_flat_batch(
            &self,
            _flat_inputs: &[f32],
            _out: &mut [f32],
        ) -> Result<(), NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }
    }

    impl CompiledPureF64Inputs {
        pub fn call(&self, _args: &[f64]) -> Result<f64, NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }

        pub fn call_flat_batch(
            &self,
            _flat_inputs: &[f64],
            _out: &mut [f64],
        ) -> Result<(), NativeJitUnavailable> {
            Err(NativeJitUnavailable)
        }
    }
}

/// Runtime pure backend selection used by CLI/player adapters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimePureBackendMode {
    Vm,
    Aot,
    Jit,
    #[default]
    Auto,
}

/// Runtime pure batch worker-count policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimePureWorkerCount {
    #[default]
    Auto,
    Fixed(usize),
}

/// Adapter-owned pure helper acceleration settings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePureAcceleratorConfig {
    pub backend: RuntimePureBackendMode,
    pub workers: RuntimePureWorkerCount,
    /// Minimum rows per resolved worker before an AOT/VM batch uses the pool.
    pub batch_min_len: usize,
    /// Emit a bundled Cranelift object artifact for supported AOT helpers.
    ///
    /// This is off by default so ordinary runtime startup does not pay
    /// build-time AOT artifact generation cost.
    pub emit_object_artifacts: bool,
    pub math: math::RuntimeMathAcceleratorConfig,
}

/// Compile-cache and runtime cache counters for pure acceleration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimePureCompileStats {
    pub jit_attempts: usize,
    pub jit_successes: usize,
    pub jit_failures: usize,
    pub aot_attempts: usize,
    pub aot_successes: usize,
    pub aot_failures: usize,
    pub auto_jit_selected: usize,
    pub auto_aot_selected: usize,
    pub auto_vm_selected: usize,
    pub auto_jit_deferred: usize,
    pub auto_jit_promotions: usize,
    pub auto_jit_skipped_small: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub object_attempts: usize,
    pub object_successes: usize,
    pub object_failures: usize,
    pub object_bytes: usize,
    pub compile_elapsed_ns: u128,
}

/// Compile-cache backed runtime pure helper accelerator.
pub struct RuntimePureAccelerator {
    config: RuntimePureAcceleratorConfig,
    cache: Vec<Option<RuntimePureCacheEntry>>,
    stats: RuntimePureCallStats,
    compile_stats: RuntimePureCompileStats,
    helper_summary: RuntimePureAccelerationSummary,
    helper_work_units: Vec<usize>,
    auto_scalar_work_units: Vec<usize>,
    pool: Option<ThreadPool>,
    resolved_workers: usize,
    flat_i64_inputs: Vec<i64>,
    aot_i64_slots: Vec<i64>,
    aot_scalar_slots: Vec<RuntimePureScalar>,
    vm_scratch: VmPureFunctionScratch,
    math: math::RuntimeMathAccelerator,
    math_prepare_cache: RuntimeMathPrepareCache,
}

enum RuntimePureCacheEntry {
    Jit(Box<CompiledPureI64Inputs>),
    JitI8(Box<CompiledPureI8Inputs>),
    JitI16(Box<CompiledPureI16Inputs>),
    JitI128Batch(Box<CompiledPureI128BatchInputs>),
    JitI32(Box<CompiledPureI32Inputs>),
    JitISize(Box<CompiledPureI64Inputs>),
    JitU8(Box<CompiledPureU8Inputs>),
    JitU16(Box<CompiledPureU16Inputs>),
    JitU32(Box<CompiledPureU32Inputs>),
    JitU64(Box<CompiledPureU64Inputs>),
    JitU128Batch(Box<CompiledPureU128BatchInputs>),
    JitUSize(Box<CompiledPureU64Inputs>),
    JitF32(Box<CompiledPureF32Inputs>),
    JitF64(Box<CompiledPureF64Inputs>),
    Aot(RuntimePureAotPlan),
    AutoAot {
        aot: RuntimePureAotPlan,
        jit: Option<Box<CompiledPureI64Inputs>>,
        jit_failed: bool,
    },
    Vm,
}

impl RuntimePureCacheEntry {
    #[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
    fn uses_aot_plan(&self) -> bool {
        matches!(self, Self::Aot(_) | Self::AutoAot { .. })
    }

    fn is_non_i64_runtime_fallback(&self) -> bool {
        matches!(
            self,
            Self::JitI8(_)
                | Self::JitI16(_)
                | Self::JitI128Batch(_)
                | Self::JitI32(_)
                | Self::JitISize(_)
                | Self::JitU8(_)
                | Self::JitU16(_)
                | Self::JitU32(_)
                | Self::JitU64(_)
                | Self::JitU128Batch(_)
                | Self::JitUSize(_)
                | Self::JitF32(_)
                | Self::JitF64(_)
                | Self::Vm
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimePureNativeKind {
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
}

impl RuntimePureNativeKind {
    const fn input_type(self) -> RuntimePureInputType {
        match self {
            Self::I8 => RuntimePureInputType::I8,
            Self::I16 => RuntimePureInputType::I16,
            Self::I32 => RuntimePureInputType::I32,
            Self::I64 => RuntimePureInputType::I64,
            Self::I128 => RuntimePureInputType::I128,
            Self::ISize => RuntimePureInputType::ISize,
            Self::U8 => RuntimePureInputType::U8,
            Self::U16 => RuntimePureInputType::U16,
            Self::U32 => RuntimePureInputType::U32,
            Self::U64 => RuntimePureInputType::U64,
            Self::U128 => RuntimePureInputType::U128,
            Self::USize => RuntimePureInputType::USize,
            Self::F32 => RuntimePureInputType::F32,
            Self::F64 => RuntimePureInputType::F64,
        }
    }

    const fn zero_value(self) -> RuntimeValue {
        match self {
            Self::I8 => RuntimeValue::i8(0),
            Self::I16 => RuntimeValue::i16(0),
            Self::I32 => RuntimeValue::i32(0),
            Self::I64 => RuntimeValue::i64(0),
            Self::I128 => RuntimeValue::i128(0),
            Self::ISize => RuntimeValue::isize(0),
            Self::U8 => RuntimeValue::u8(0),
            Self::U16 => RuntimeValue::u16(0),
            Self::U32 => RuntimeValue::u32(0),
            Self::U64 => RuntimeValue::u64(0),
            Self::U128 => RuntimeValue::u128(0),
            Self::USize => RuntimeValue::usize(0),
            Self::F32 => RuntimeValue::F32(0.0),
            Self::F64 => RuntimeValue::F64(0.0),
        }
    }

    #[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
    const fn object_input_kind(self) -> PureObjectInputKind {
        match self {
            Self::I8 => PureObjectInputKind::I8,
            Self::I16 => PureObjectInputKind::I16,
            Self::I32 => PureObjectInputKind::I32,
            Self::I64 | Self::ISize => PureObjectInputKind::I64,
            Self::I128 => PureObjectInputKind::I128,
            Self::U8 => PureObjectInputKind::U8,
            Self::U16 => PureObjectInputKind::U16,
            Self::U32 => PureObjectInputKind::U32,
            Self::U64 | Self::USize => PureObjectInputKind::U64,
            Self::U128 => PureObjectInputKind::U128,
            Self::F32 => PureObjectInputKind::F32,
            Self::F64 => PureObjectInputKind::F64,
        }
    }
}

fn helper_native_kind(helper: &RuntimePureHelper) -> Option<RuntimePureNativeKind> {
    let kind = match helper.output_type {
        RuntimePureOutputType::I8 => RuntimePureNativeKind::I8,
        RuntimePureOutputType::I16 => RuntimePureNativeKind::I16,
        RuntimePureOutputType::I32 => RuntimePureNativeKind::I32,
        RuntimePureOutputType::I64 => RuntimePureNativeKind::I64,
        RuntimePureOutputType::I128 => RuntimePureNativeKind::I128,
        RuntimePureOutputType::ISize => RuntimePureNativeKind::ISize,
        RuntimePureOutputType::U8 => RuntimePureNativeKind::U8,
        RuntimePureOutputType::U16 => RuntimePureNativeKind::U16,
        RuntimePureOutputType::U32 => RuntimePureNativeKind::U32,
        RuntimePureOutputType::U64 => RuntimePureNativeKind::U64,
        RuntimePureOutputType::U128 => RuntimePureNativeKind::U128,
        RuntimePureOutputType::USize => RuntimePureNativeKind::USize,
        RuntimePureOutputType::F32 => RuntimePureNativeKind::F32,
        RuntimePureOutputType::F64 => RuntimePureNativeKind::F64,
        RuntimePureOutputType::Bool | RuntimePureOutputType::Value => return None,
    };
    (helper.input_names.len() == helper.input_types.len()
        && helper
            .input_types
            .iter()
            .all(|input_type| *input_type == kind.input_type()))
    .then_some(kind)
}

fn call_jit_exact_int_slice<T: RuntimePureScalarInteger>(
    entry: &RuntimePureCacheEntry,
    helper: &RuntimePureHelper,
    args: &[T],
) -> Option<Result<Option<T>, RuntimeEvalError>> {
    let value = match (T::exact_slice(args), entry) {
        (RuntimeExactIntegerSlice::I8(args), RuntimePureCacheEntry::JitI8(compiled)) => {
            compiled.call(args).map(RuntimeValue::i8)
        }
        (RuntimeExactIntegerSlice::I16(args), RuntimePureCacheEntry::JitI16(compiled)) => {
            compiled.call(args).map(RuntimeValue::i16)
        }
        (RuntimeExactIntegerSlice::I32(args), RuntimePureCacheEntry::JitI32(compiled)) => {
            compiled.call(args).map(RuntimeValue::i32)
        }
        (RuntimeExactIntegerSlice::I128(args), RuntimePureCacheEntry::JitI128Batch(compiled)) => {
            compiled.call(args).map(RuntimeValue::i128)
        }
        (RuntimeExactIntegerSlice::ISize(args), RuntimePureCacheEntry::JitISize(compiled)) => {
            compiled
                .call_isize(args)
                .map(|value| RuntimeValue::isize(value.get()))
        }
        (RuntimeExactIntegerSlice::U8(args), RuntimePureCacheEntry::JitU8(compiled)) => {
            compiled.call(args).map(RuntimeValue::u8)
        }
        (RuntimeExactIntegerSlice::U16(args), RuntimePureCacheEntry::JitU16(compiled)) => {
            compiled.call(args).map(RuntimeValue::u16)
        }
        (RuntimeExactIntegerSlice::U32(args), RuntimePureCacheEntry::JitU32(compiled)) => {
            compiled.call(args).map(RuntimeValue::u32)
        }
        (RuntimeExactIntegerSlice::U64(args), RuntimePureCacheEntry::JitU64(compiled)) => {
            compiled.call(args).map(RuntimeValue::u64)
        }
        (RuntimeExactIntegerSlice::U128(args), RuntimePureCacheEntry::JitU128Batch(compiled)) => {
            compiled.call(args).map(RuntimeValue::u128)
        }
        (RuntimeExactIntegerSlice::USize(args), RuntimePureCacheEntry::JitUSize(compiled)) => {
            compiled
                .call_usize(args)
                .map(|value| RuntimeValue::usize(value.get()))
        }
        _ => return None,
    };
    Some(
        value
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: error.to_string(),
            })
            .and_then(|value| T::try_from_runtime_value(&helper.name, value).map(Some)),
    )
}

fn call_jit_exact_int_flat_batch<T: RuntimePureScalarInteger>(
    entry: &RuntimePureCacheEntry,
    helper: &RuntimePureHelper,
    flat_inputs: &[T],
    out: &mut [T],
) -> Option<Result<(), RuntimeEvalError>> {
    let result = match (T::exact_slice(flat_inputs), T::exact_slice_mut(out), entry) {
        (
            RuntimeExactIntegerSlice::ISize(inputs),
            RuntimeExactIntegerSliceMut::ISize(out),
            RuntimePureCacheEntry::JitISize(compiled),
        ) => compiled.call_isize_flat_batch(inputs, out),
        (
            RuntimeExactIntegerSlice::USize(inputs),
            RuntimeExactIntegerSliceMut::USize(out),
            RuntimePureCacheEntry::JitUSize(compiled),
        ) => compiled.call_usize_flat_batch(inputs, out),
        _ => return None,
    };
    Some(result.map_err(|error| RuntimeEvalError::UnsupportedPure {
        name: helper.name.clone(),
        reason: error.to_string(),
    }))
}

fn call_jit_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
    entry: &RuntimePureCacheEntry,
    helper: &RuntimePureHelper,
    flat_inputs: &[T],
    rows: usize,
) -> Option<Result<i64, RuntimeEvalError>> {
    let result = match (T::exact_slice(flat_inputs), entry) {
        (RuntimeExactIntegerSlice::ISize(inputs), RuntimePureCacheEntry::JitISize(compiled)) => {
            compiled.call_isize_flat_batch_sum(inputs, rows)
        }
        (RuntimeExactIntegerSlice::USize(inputs), RuntimePureCacheEntry::JitUSize(compiled)) => {
            compiled.call_usize_flat_batch_sum(inputs, rows)
        }
        _ => return None,
    };
    Some(result.map_err(|error| RuntimeEvalError::UnsupportedPure {
        name: helper.name.clone(),
        reason: error.to_string(),
    }))
}

enum RuntimePureAotPlan {
    I64(AotPureI64Plan),
    Scalar(AotPureScalarPlan),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeBatchBackendKind {
    Jit,
    Aot,
    Vm,
    Missing,
}

#[derive(Clone, Copy)]
struct FlatBatchSumShape<'a> {
    flat_inputs: &'a [i64],
    arity: usize,
    rows: usize,
}

#[derive(Clone, Copy)]
struct FlatBatchSumPolicy<'a> {
    pool: Option<&'a ThreadPool>,
    wants_parallel: bool,
    parallel_jobs: usize,
}

#[derive(Default)]
struct RuntimeMathPrepareCache {
    matmul: Option<PreparedMatrixMatmulCache>,
    matmul_bias_add: Option<PreparedMatrixMatmulBiasAddCache>,
    matrix_add: Option<PreparedMatrixAddCache>,
    tensor_add: Option<PreparedTensorAddCache>,
}

struct PreparedMatrixMatmulCache {
    signature: MatrixBinaryShapeSignature,
    capacity_signature: MatrixBinaryShapeSignature,
    value_signature: MatrixBinaryValueSignature,
    prepared: math::RuntimePreparedMatrixMatmulF32,
}

struct PreparedMatrixMatmulBiasAddCache {
    signature: MatrixMatmulBiasShapeSignature,
    capacity_signature: MatrixMatmulBiasShapeSignature,
    value_signature: MatrixMatmulBiasValueSignature,
    prepared: math::RuntimePreparedMatrixMatmulBiasAddF32,
}

struct PreparedMatrixAddCache {
    signature: MatrixBinaryShapeSignature,
    capacity_signature: MatrixBinaryShapeSignature,
    value_signature: MatrixBinaryValueSignature,
    prepared: math::RuntimePreparedMatrixAddF32,
}

struct PreparedTensorAddCache {
    signature: TensorBinaryShapeSignature,
    capacity_signature: TensorBinaryShapeSignature,
    value_signature: TensorBinaryValueSignature,
    prepared: math::RuntimePreparedTensorAddF32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MatrixBinaryShapeSignature {
    lhs: MatrixShapeSignature,
    rhs: MatrixShapeSignature,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MatrixShapeSignature {
    rows: usize,
    cols: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MatrixBinaryValueSignature {
    lhs: Vec<u32>,
    rhs: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MatrixMatmulBiasShapeSignature {
    lhs: MatrixShapeSignature,
    rhs: MatrixShapeSignature,
    bias: TensorShapeSignature,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MatrixMatmulBiasValueSignature {
    lhs: Vec<u32>,
    rhs: Vec<u32>,
    bias: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TensorBinaryShapeSignature {
    lhs: TensorShapeSignature,
    rhs: TensorShapeSignature,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TensorShapeSignature {
    dims: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TensorBinaryValueSignature {
    lhs: Vec<u32>,
    rhs: Vec<u32>,
}

impl MatrixBinaryShapeSignature {
    fn new(lhs: &DenseMatrixF32, rhs: &DenseMatrixF32) -> Self {
        Self {
            lhs: MatrixShapeSignature::new(lhs),
            rhs: MatrixShapeSignature::new(rhs),
        }
    }

    fn capacity_for_matrix_add(lhs: &DenseMatrixF32, rhs: &DenseMatrixF32) -> Self {
        Self {
            lhs: MatrixShapeSignature::capacity_for(lhs),
            rhs: MatrixShapeSignature::capacity_for(rhs),
        }
    }

    fn capacity_for_matmul(lhs: &DenseMatrixF32, rhs: &DenseMatrixF32) -> Self {
        Self {
            lhs: MatrixShapeSignature::capacity_for(lhs),
            rhs: MatrixShapeSignature::capacity_for(rhs),
        }
    }

    fn contains(&self, shape: &Self) -> bool {
        self.lhs.contains(&shape.lhs) && self.rhs.contains(&shape.rhs)
    }
}

impl MatrixShapeSignature {
    fn new(matrix: &DenseMatrixF32) -> Self {
        Self {
            rows: matrix.rows(),
            cols: matrix.cols(),
        }
    }

    fn capacity_for(matrix: &DenseMatrixF32) -> Self {
        Self {
            rows: power_of_two_capacity(matrix.rows()),
            cols: power_of_two_capacity(matrix.cols()),
        }
    }

    fn contains(&self, shape: &Self) -> bool {
        self.rows >= shape.rows && self.cols >= shape.cols
    }
}

impl MatrixBinaryValueSignature {
    fn new(lhs: &DenseMatrixF32, rhs: &DenseMatrixF32) -> Self {
        Self {
            lhs: f32_value_bits(lhs.values()),
            rhs: f32_value_bits(rhs.values()),
        }
    }

    fn matches(&self, lhs: &DenseMatrixF32, rhs: &DenseMatrixF32) -> bool {
        f32_value_bits_match(&self.lhs, lhs.values())
            && f32_value_bits_match(&self.rhs, rhs.values())
    }

    fn update(&mut self, lhs: &DenseMatrixF32, rhs: &DenseMatrixF32) {
        update_f32_value_bits(&mut self.lhs, lhs.values());
        update_f32_value_bits(&mut self.rhs, rhs.values());
    }
}

impl MatrixMatmulBiasShapeSignature {
    fn new(lhs: &DenseMatrixF32, rhs: &DenseMatrixF32, bias: &DenseTensorF32) -> Self {
        Self {
            lhs: MatrixShapeSignature::new(lhs),
            rhs: MatrixShapeSignature::new(rhs),
            bias: TensorShapeSignature::new(bias),
        }
    }

    fn capacity_for(lhs: &DenseMatrixF32, rhs: &DenseMatrixF32, bias: &DenseTensorF32) -> Self {
        Self {
            lhs: MatrixShapeSignature::capacity_for(lhs),
            rhs: MatrixShapeSignature::capacity_for(rhs),
            bias: TensorShapeSignature::capacity_for(bias),
        }
    }

    fn contains(&self, shape: &Self) -> bool {
        self.lhs.contains(&shape.lhs)
            && self.rhs.contains(&shape.rhs)
            && self.bias.contains(&shape.bias)
    }
}

impl MatrixMatmulBiasValueSignature {
    fn new(lhs: &DenseMatrixF32, rhs: &DenseMatrixF32, bias: &DenseTensorF32) -> Self {
        Self {
            lhs: f32_value_bits(lhs.values()),
            rhs: f32_value_bits(rhs.values()),
            bias: f32_value_bits(bias.values()),
        }
    }

    fn matches(&self, lhs: &DenseMatrixF32, rhs: &DenseMatrixF32, bias: &DenseTensorF32) -> bool {
        f32_value_bits_match(&self.lhs, lhs.values())
            && f32_value_bits_match(&self.rhs, rhs.values())
            && f32_value_bits_match(&self.bias, bias.values())
    }

    fn update(&mut self, lhs: &DenseMatrixF32, rhs: &DenseMatrixF32, bias: &DenseTensorF32) {
        update_f32_value_bits(&mut self.lhs, lhs.values());
        update_f32_value_bits(&mut self.rhs, rhs.values());
        update_f32_value_bits(&mut self.bias, bias.values());
    }
}

impl TensorBinaryShapeSignature {
    fn new(lhs: &DenseTensorF32, rhs: &DenseTensorF32) -> Self {
        Self {
            lhs: TensorShapeSignature::new(lhs),
            rhs: TensorShapeSignature::new(rhs),
        }
    }

    fn capacity_for_add(lhs: &DenseTensorF32, rhs: &DenseTensorF32) -> Self {
        Self {
            lhs: TensorShapeSignature::capacity_for(lhs),
            rhs: TensorShapeSignature::capacity_for(rhs),
        }
    }

    fn contains(&self, shape: &Self) -> bool {
        self.lhs.contains(&shape.lhs) && self.rhs.contains(&shape.rhs)
    }
}

impl TensorShapeSignature {
    fn new(tensor: &DenseTensorF32) -> Self {
        Self {
            dims: tensor.shape().dims().to_vec(),
        }
    }

    fn capacity_for(tensor: &DenseTensorF32) -> Self {
        Self {
            dims: vec![power_of_two_capacity(tensor.values().len())],
        }
    }

    fn element_count(&self) -> usize {
        self.dims
            .iter()
            .copied()
            .fold(1_usize, usize::saturating_mul)
    }

    fn contains(&self, shape: &Self) -> bool {
        self.element_count() >= shape.element_count()
    }
}

impl TensorBinaryValueSignature {
    fn new(lhs: &DenseTensorF32, rhs: &DenseTensorF32) -> Self {
        Self {
            lhs: f32_value_bits(lhs.values()),
            rhs: f32_value_bits(rhs.values()),
        }
    }

    fn matches(&self, lhs: &DenseTensorF32, rhs: &DenseTensorF32) -> bool {
        f32_value_bits_match(&self.lhs, lhs.values())
            && f32_value_bits_match(&self.rhs, rhs.values())
    }

    fn update(&mut self, lhs: &DenseTensorF32, rhs: &DenseTensorF32) {
        update_f32_value_bits(&mut self.lhs, lhs.values());
        update_f32_value_bits(&mut self.rhs, rhs.values());
    }
}

pub(crate) fn f32_value_bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|value| value.to_bits()).collect()
}

pub(crate) fn f32_value_bits_match(bits: &[u32], values: &[f32]) -> bool {
    bits.len() == values.len()
        && bits
            .iter()
            .zip(values)
            .all(|(stored, value)| *stored == value.to_bits())
}

pub(crate) fn update_f32_value_bits(bits: &mut Vec<u32>, values: &[f32]) {
    bits.clear();
    bits.extend(values.iter().map(|value| value.to_bits()));
}

pub(crate) fn power_of_two_capacity(value: usize) -> usize {
    value.checked_next_power_of_two().unwrap_or(value).max(1)
}

const AUTO_EAGER_JIT_WORK_UNITS: usize = 64;
const AUTO_JIT_FLAT_BATCH_WORK_UNITS: usize = 512;
const AUTO_JIT_SCALAR_WORK_UNITS: usize = AUTO_JIT_FLAT_BATCH_WORK_UNITS;

const fn native_jit_enabled() -> bool {
    cfg_select! {
        all(feature = "native-jit", not(target_arch = "wasm32")) => { true }
        _ => { false }
    }
}

impl RuntimePureAotPlan {
    fn i64_plan(&self) -> Option<&AotPureI64Plan> {
        match self {
            Self::I64(plan) => Some(plan),
            Self::Scalar(_) => None,
        }
    }

    fn call_i64_with_inputs_scratch(
        &self,
        inputs: &[i64],
        slots: &mut Vec<i64>,
    ) -> Result<(i64, PureFunctionStats), RuntimeEvalError> {
        match self {
            Self::I64(plan) => plan.call_with_inputs_scratch(inputs, slots),
            Self::Scalar(plan) => Err(RuntimeEvalError::UnsupportedPure {
                name: plan.name().to_owned(),
                reason: "AOT scalar plan is not an i64 plan".to_owned(),
            }),
        }
    }

    fn call_exact_int_with_inputs_scratch<T: RuntimePureScalarInteger>(
        &self,
        inputs: &[T],
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(T, PureFunctionStats), RuntimeEvalError> {
        match self {
            Self::Scalar(plan) => plan.call_exact_int_with_inputs_scratch(inputs, slots),
            Self::I64(plan) => Err(RuntimeEvalError::UnsupportedPure {
                name: plan.name().to_owned(),
                reason: "AOT i64 plan is not an exact scalar plan".to_owned(),
            }),
        }
    }

    fn call_f32_with_inputs_scratch(
        &self,
        inputs: &[f32],
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(f32, PureFunctionStats), RuntimeEvalError> {
        match self {
            Self::Scalar(plan) => plan.call_f32_with_inputs_scratch(inputs, slots),
            Self::I64(plan) => Err(RuntimeEvalError::UnsupportedPure {
                name: plan.name().to_owned(),
                reason: "AOT i64 plan is not an f32 scalar plan".to_owned(),
            }),
        }
    }

    fn call_f64_with_inputs_scratch(
        &self,
        inputs: &[f64],
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(f64, PureFunctionStats), RuntimeEvalError> {
        match self {
            Self::Scalar(plan) => plan.call_f64_with_inputs_scratch(inputs, slots),
            Self::I64(plan) => Err(RuntimeEvalError::UnsupportedPure {
                name: plan.name().to_owned(),
                reason: "AOT i64 plan is not an f64 scalar plan".to_owned(),
            }),
        }
    }

    fn require_i64(&self, helper: &RuntimePureHelper) -> Result<&AotPureI64Plan, RuntimeEvalError> {
        self.i64_plan()
            .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: "AOT scalar plan cannot serve an i64 batch call".to_owned(),
            })
    }
}

impl fmt::Debug for RuntimePureAccelerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimePureAccelerator")
            .field("config", &self.config)
            .field("cache_entries", &self.cache_entries())
            .field("stats", &self.stats)
            .field("compile_stats", &self.compile_stats)
            .field("helper_summary", &self.helper_summary)
            .field("has_pool", &self.pool.is_some())
            .field("resolved_workers", &self.resolved_workers)
            .field("math_stats", &self.math.stats())
            .finish_non_exhaustive()
    }
}

/// Summary of helpers selected for acceleration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimePureAccelerationSummary {
    pub annotated: usize,
    pub inferred: usize,
    pub jit: usize,
    pub aot: usize,
    pub vm: usize,
}

impl RuntimePureAccelerator {
    pub fn summary(&self) -> RuntimePureAccelerationSummary {
        let mut jit = 0;
        let mut aot = 0;
        let mut vm = 0;
        for entry in self.cache.iter().filter_map(Option::as_ref) {
            match entry {
                RuntimePureCacheEntry::Jit(_)
                | RuntimePureCacheEntry::JitI8(_)
                | RuntimePureCacheEntry::JitI16(_)
                | RuntimePureCacheEntry::JitI128Batch(_)
                | RuntimePureCacheEntry::JitI32(_)
                | RuntimePureCacheEntry::JitISize(_)
                | RuntimePureCacheEntry::JitU8(_)
                | RuntimePureCacheEntry::JitU16(_)
                | RuntimePureCacheEntry::JitU32(_)
                | RuntimePureCacheEntry::JitU64(_)
                | RuntimePureCacheEntry::JitU128Batch(_)
                | RuntimePureCacheEntry::JitUSize(_)
                | RuntimePureCacheEntry::JitF32(_)
                | RuntimePureCacheEntry::JitF64(_)
                | RuntimePureCacheEntry::AutoAot { jit: Some(_), .. } => jit += 1,
                RuntimePureCacheEntry::Aot(_)
                | RuntimePureCacheEntry::AutoAot { jit: None, .. } => {
                    aot += 1;
                }
                RuntimePureCacheEntry::Vm => vm += 1,
            }
        }
        RuntimePureAccelerationSummary {
            annotated: self.helper_summary.annotated,
            inferred: self.helper_summary.inferred,
            jit,
            aot,
            vm,
        }
    }
}

fn helper_summary_from_helpers(helpers: &[RuntimePureHelper]) -> RuntimePureAccelerationSummary {
    let annotated = helpers
        .iter()
        .filter(|helper| helper.origin == RuntimePureHelperOrigin::Annotated)
        .count();
    RuntimePureAccelerationSummary {
        annotated,
        inferred: helpers.len().saturating_sub(annotated),
        jit: 0,
        aot: 0,
        vm: 0,
    }
}

impl Default for RuntimePureAcceleratorConfig {
    fn default() -> Self {
        Self {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Auto,
            batch_min_len: 1024,
            emit_object_artifacts: false,
            math: math::RuntimeMathAcceleratorConfig::default(),
        }
    }
}
