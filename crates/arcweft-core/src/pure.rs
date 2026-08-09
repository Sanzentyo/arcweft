use crate::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
use crate::pattern::{RuntimePattern, match_runtime_pattern};
use crate::plan::{RuntimePureHelper, RuntimePureInputType, RuntimePureOutputType};
use crate::step::RuntimePureCallStats;
use crate::value::{
    RuntimeBinaryOp, RuntimeBinding, RuntimeCallTarget, RuntimeEnv, RuntimeEvalError,
    RuntimeExactInteger, RuntimeExpr, RuntimeExprMatchArm, RuntimeFieldExpr, RuntimeFieldValue,
    RuntimeFunctionValue, RuntimeISizeValue, RuntimeIntrinsic, RuntimeIterator, RuntimeSeq,
    RuntimeUSizeValue, RuntimeUnaryOp, RuntimeValue, evaluate_binary,
    evaluate_core_iter_collect_intrinsic, evaluate_core_iter_into_iter_intrinsic,
    evaluate_core_iter_next_intrinsic, evaluate_core_option_is_some_intrinsic,
    evaluate_core_option_unwrap_intrinsic, evaluate_core_range_intrinsic, evaluate_numeric_op,
    evaluate_std_float_intrinsic, evaluate_unary, runtime_sequence_values,
    runtime_value_into_sequence_values, runtime_value_label, sum_i64_sequence_ref,
};

mod aot;
mod runtime_backend;

/// Request for evaluating a deterministic pure helper expression.
#[derive(Clone, Debug, PartialEq)]
pub struct PureFunctionRequest {
    pub name: String,
    pub expr: RuntimeExpr,
    pub bindings: Vec<RuntimeBinding>,
}

/// Result of one pure helper backend evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct PureFunctionResult {
    pub backend: PureFunctionBackendKind,
    pub value: RuntimeValue,
    pub stats: PureFunctionStats,
}

/// Backend family used for pure helper evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PureFunctionBackendKind {
    Vm,
    Aot,
    Jit,
}

/// Deterministic counters for pure helper evaluation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PureFunctionStats {
    pub evaluated_exprs: usize,
    pub evaluated_calls: usize,
    pub evaluated_method_calls: usize,
    pub evaluated_binary_ops: usize,
}

/// Fixed-size scalar argument pack for runtime pure helper fast paths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimeFixedArgs<T> {
    len: usize,
    values: [T; 4],
}

pub type RuntimeI32Args = RuntimeFixedArgs<i32>;
pub type RuntimeI64Args = RuntimeFixedArgs<i64>;
pub type RuntimeFloat32Args = RuntimeFixedArgs<f32>;
pub type RuntimeFloat64Args = RuntimeFixedArgs<f64>;

/// Exact integer scalar that preserves the helper ABI width during VM pure evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RuntimePureScalar {
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    ISize(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    USize(u64),
    F32(f32),
    F64(f64),
}

/// Integer widths that can stay typed across pure helper VM fast paths.
pub trait RuntimePureScalarInteger: RuntimeExactInteger {
    fn into_pure_scalar(self) -> RuntimePureScalar;
}

macro_rules! impl_runtime_pure_scalar_integer {
    ($ty:ty, $variant:ident) => {
        impl RuntimePureScalarInteger for $ty {
            fn into_pure_scalar(self) -> RuntimePureScalar {
                RuntimePureScalar::$variant(self)
            }
        }
    };
}

impl_runtime_pure_scalar_integer!(i8, I8);
impl_runtime_pure_scalar_integer!(i16, I16);
impl_runtime_pure_scalar_integer!(i32, I32);
impl_runtime_pure_scalar_integer!(i128, I128);
impl_runtime_pure_scalar_integer!(u8, U8);
impl_runtime_pure_scalar_integer!(u16, U16);
impl_runtime_pure_scalar_integer!(u32, U32);
impl_runtime_pure_scalar_integer!(u64, U64);
impl_runtime_pure_scalar_integer!(u128, U128);

impl RuntimePureScalarInteger for RuntimeISizeValue {
    fn into_pure_scalar(self) -> RuntimePureScalar {
        RuntimePureScalar::ISize(self.get())
    }
}

impl RuntimePureScalarInteger for RuntimeUSizeValue {
    fn into_pure_scalar(self) -> RuntimePureScalar {
        RuntimePureScalar::USize(self.get())
    }
}

/// Runtime-facing backend for deterministic built-in math calls.
pub trait RuntimeMathCallBackend {
    fn call_math_matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeEvalError>;

    fn call_math_matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeEvalError>;

    fn call_math_tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeEvalError>;

    fn call_math_matmul_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeEvalError>;

    fn call_math_matrix_add_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeEvalError>;

    fn call_math_tensor_add_f64(
        &mut self,
        lhs: &DenseTensorF64,
        rhs: &DenseTensorF64,
    ) -> Result<DenseTensorF64, RuntimeEvalError>;
}

/// Adapter boundary for runtime calls that are not Arcweft Core intrinsics.
///
/// This mirrors an FFI boundary: Core evaluates argument expressions and keeps
/// their typed `RuntimeValue` shape, while adapter crates decide which named
/// calls they own and how to execute them.
pub trait RuntimeExternalCallBackend {
    fn call_external(
        &mut self,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>>;
}

/// Stable compact-AWBC pure helper identity presented to runtime backends.
///
/// Compact product execution cannot reconstruct the structured helper expression,
/// so backends receive the canonical helper identity and ABI shape directly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCompactPureHelper {
    pub id: u32,
    pub name: String,
    pub arity: usize,
    pub scalar_eval_supported: bool,
}

/// Runtime-facing backend for deterministic pure helper calls.
pub trait RuntimePureCallBackend {
    fn call_i8_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[i8],
    ) -> Result<Option<i8>, RuntimeEvalError>;

    fn call_i8_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i8],
        arity: usize,
        out: &mut [i8],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i8_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i8],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_i16_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[i16],
    ) -> Result<Option<i16>, RuntimeEvalError>;

    fn call_i16_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i16],
        arity: usize,
        out: &mut [i16],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i16_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i16],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_i128_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i128],
        arity: usize,
        out: &mut [i128],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i128_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i128],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_i32(
        &mut self,
        helper: &RuntimePureHelper,
        args: RuntimeI32Args,
    ) -> Result<Option<i32>, RuntimeEvalError>;

    fn call_i32_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[i32],
    ) -> Result<Option<i32>, RuntimeEvalError>;

    fn call_i32_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i32],
        arity: usize,
        out: &mut [i32],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i32_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i32],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u32_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[u32],
    ) -> Result<Option<u32>, RuntimeEvalError>;

    fn call_u8_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[u8],
    ) -> Result<Option<u8>, RuntimeEvalError>;

    fn call_u8_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u8],
        arity: usize,
        out: &mut [u8],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u8_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u8],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u16_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[u16],
    ) -> Result<Option<u16>, RuntimeEvalError>;

    fn call_u16_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u16],
        arity: usize,
        out: &mut [u16],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u16_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u16],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u128_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u128],
        arity: usize,
        out: &mut [u128],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u128_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u128],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u32_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u32],
        arity: usize,
        out: &mut [u32],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u32_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u32],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u64_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[u64],
    ) -> Result<Option<u64>, RuntimeEvalError>;

    fn call_u64_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u64],
        arity: usize,
        out: &mut [u64],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u64_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[T],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_exact_int_slice<T: RuntimePureScalarInteger>(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[T],
    ) -> Result<Option<T>, RuntimeEvalError>;

    fn call_exact_int_flat_batch<T: RuntimePureScalarInteger>(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[T],
        arity: usize,
        out: &mut [T],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i64(
        &mut self,
        helper: &RuntimePureHelper,
        args: RuntimeI64Args,
    ) -> Result<Option<i64>, RuntimeEvalError>;

    fn call_i64_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[i64],
    ) -> Result<Option<i64>, RuntimeEvalError>;

    fn call_i64_batch(
        &mut self,
        helper: &RuntimePureHelper,
        rows: &[RuntimeI64Args],
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i64_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i64],
        arity: usize,
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i64_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_i64_repeated_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        row: &[i64],
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_f32_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[f32],
    ) -> Result<Option<f32>, RuntimeEvalError>;

    fn call_f32_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[f32],
        arity: usize,
        out: &mut [f32],
    ) -> Result<(), RuntimeEvalError>;

    fn call_f64_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[f64],
    ) -> Result<Option<f64>, RuntimeEvalError>;

    fn call_f64_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[f64],
        arity: usize,
        out: &mut [f64],
    ) -> Result<(), RuntimeEvalError>;

    fn call_values(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError>;

    /// Optionally evaluates a canonical compact-AWBC helper directly.
    ///
    /// Returning `None` selects the verified compact VM fallback owned by the
    /// product executor. Returning `Some` preserves backend selection and
    /// deterministic success/failure at the shared runtime boundary.
    fn call_compact_values(
        &mut self,
        helper: &RuntimeCompactPureHelper,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        let _ = (helper, args);
        None
    }

    fn stats(&self) -> RuntimePureCallStats;
}

/// Backend accepted by runtime expression evaluation.
pub trait RuntimeCallBackend:
    RuntimePureCallBackend + RuntimeMathCallBackend + RuntimeExternalCallBackend
{
}

impl<T> RuntimeCallBackend for T where
    T: RuntimePureCallBackend + RuntimeMathCallBackend + RuntimeExternalCallBackend
{
}

/// Backend contract for pure deterministic helper evaluation.
pub trait PureFunctionBackend {
    fn kind(&self) -> PureFunctionBackendKind;

    fn evaluate(
        &self,
        request: &PureFunctionRequest,
    ) -> Result<PureFunctionResult, RuntimeEvalError>;
}

/// VM fallback backend for pure helpers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VmPureFunctionBackend;

/// Reusable VM fallback storage for repeated `i64` pure-helper evaluation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VmPureFunctionScratch {
    env: RuntimeEnv,
}

/// AOT backend for deterministic pure helpers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AotPureFunctionBackend;

/// VM runtime backend used when no external pure accelerator is provided.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VmRuntimePureCallBackend {
    stats: RuntimePureCallStats,
    scratch: VmPureFunctionScratch,
}

/// Compiled AOT plan for the current deterministic `i64` pure-helper subset.
#[derive(Clone, Debug, PartialEq)]
pub struct AotPureI64Plan {
    name: String,
    expr: aot::AotI64Expr,
    initial_slots: Vec<i64>,
    input_slots: Vec<usize>,
    slot_count: usize,
}

/// Compiled AOT plan for exact-width scalar helpers that are not widened to `i64`.
#[derive(Clone, Debug, PartialEq)]
pub struct AotPureScalarPlan {
    name: String,
    expr: aot::AotScalarExpr,
    initial_slots: Vec<RuntimePureScalar>,
    input_slots: Vec<usize>,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    slot_count: usize,
}

/// VM/JIT conformance result for deterministic helper execution.
#[derive(Clone, Debug, PartialEq)]
pub struct PureFunctionConformance {
    pub vm: PureFunctionResult,
    pub candidate: PureFunctionResult,
    pub matches_vm: bool,
}

impl PureFunctionRequest {
    pub fn new(
        name: impl Into<String>,
        expr: RuntimeExpr,
        bindings: impl IntoIterator<Item = RuntimeBinding>,
    ) -> Self {
        Self {
            name: name.into(),
            expr,
            bindings: bindings.into_iter().collect(),
        }
    }
}

impl PureFunctionBackend for VmPureFunctionBackend {
    fn kind(&self) -> PureFunctionBackendKind {
        PureFunctionBackendKind::Vm
    }

    fn evaluate(
        &self,
        request: &PureFunctionRequest,
    ) -> Result<PureFunctionResult, RuntimeEvalError> {
        let mut evaluator = PureEvaluator::new_ref(&request.bindings);
        let value = evaluator.evaluate_expr(&request.expr)?;
        Ok(PureFunctionResult {
            backend: self.kind(),
            value,
            stats: evaluator.stats,
        })
    }
}

impl VmPureFunctionBackend {
    pub fn evaluate_i32_args(
        &self,
        helper: &RuntimePureHelper,
        args: RuntimeI32Args,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_i32_slice(helper, args.as_slice())
    }

    pub fn evaluate_i32_slice(
        &self,
        helper: &RuntimePureHelper,
        args: &[i32],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut scratch = VmPureFunctionScratch::default();
        scratch.evaluate_i32_slice(helper, args)
    }

    pub fn evaluate_i64_args(
        &self,
        helper: &RuntimePureHelper,
        args: RuntimeI64Args,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_i64_slice(helper, args.as_slice())
    }

    pub fn evaluate_i64_slice(
        &self,
        helper: &RuntimePureHelper,
        args: &[i64],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut scratch = VmPureFunctionScratch::default();
        scratch.evaluate_i64_slice(helper, args)
    }

    pub fn evaluate_f32_slice(
        &self,
        helper: &RuntimePureHelper,
        args: &[f32],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut scratch = VmPureFunctionScratch::default();
        scratch.evaluate_f32_slice(helper, args)
    }

    pub fn evaluate_f64_slice(
        &self,
        helper: &RuntimePureHelper,
        args: &[f64],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut scratch = VmPureFunctionScratch::default();
        scratch.evaluate_f64_slice(helper, args)
    }
}

impl VmPureFunctionScratch {
    pub fn evaluate_i32_args(
        &mut self,
        helper: &RuntimePureHelper,
        args: RuntimeI32Args,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_i32_slice(helper, args.as_slice())
    }

    pub fn evaluate_i32_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[i32],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if args.len() != helper.input_names.len() {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: helper.input_names.len(),
                found: args.len(),
            });
        }
        self.env
            .replace_root_i32_bindings(&helper.input_names, args);
        let mut evaluator = PureEvaluator::with_env(std::mem::take(&mut self.env));
        let result = if helper.scalar_eval_supported {
            evaluator
                .evaluate_scalar_expr(&helper.expr)
                .map(RuntimePureScalar::into_runtime_value)
        } else {
            evaluator.evaluate_expr(&helper.expr)
        };
        self.env = evaluator.into_env();
        result
    }

    pub fn evaluate_exact_int_slice<T: RuntimePureScalarInteger>(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[T],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if args.len() != helper.input_names.len() {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: helper.input_names.len(),
                found: args.len(),
            });
        }
        if helper.scalar_eval_supported {
            let mut evaluator = PureScalarEvaluator::new_exact(&helper.input_names, args);
            return evaluator
                .evaluate(&helper.expr)
                .map(RuntimePureScalar::into_runtime_value);
        }
        self.env
            .replace_root_exact_int_bindings(&helper.input_names, args);
        let mut evaluator = PureEvaluator::with_env(std::mem::take(&mut self.env));
        let result = if helper.scalar_eval_supported {
            evaluator
                .evaluate_scalar_expr(&helper.expr)
                .map(RuntimePureScalar::into_runtime_value)
        } else {
            evaluator.evaluate_expr(&helper.expr)
        };
        self.env = evaluator.into_env();
        result
    }

    pub fn evaluate_i64_args(
        &mut self,
        helper: &RuntimePureHelper,
        args: RuntimeI64Args,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_i64_slice(helper, args.as_slice())
    }

    pub fn evaluate_i64_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[i64],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if args.len() != helper.input_names.len() {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: helper.input_names.len(),
                found: args.len(),
            });
        }
        self.env
            .replace_root_i64_bindings(&helper.input_names, args);
        let mut evaluator = PureEvaluator::with_env(std::mem::take(&mut self.env));
        let result = if helper.scalar_eval_supported {
            evaluator
                .evaluate_scalar_expr(&helper.expr)
                .map(RuntimePureScalar::into_runtime_value)
        } else {
            evaluator.evaluate_expr(&helper.expr)
        };
        self.env = evaluator.into_env();
        result
    }

    pub fn evaluate_f32_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[f32],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if args.len() != helper.input_names.len() {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: helper.input_names.len(),
                found: args.len(),
            });
        }
        self.env
            .replace_root_f32_bindings(&helper.input_names, args);
        let mut evaluator = PureEvaluator::with_env(std::mem::take(&mut self.env));
        let result = if helper.scalar_eval_supported {
            evaluator
                .evaluate_scalar_expr(&helper.expr)
                .map(RuntimePureScalar::into_runtime_value)
        } else {
            evaluator.evaluate_expr(&helper.expr)
        };
        self.env = evaluator.into_env();
        result
    }

    pub fn evaluate_f64_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[f64],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if args.len() != helper.input_names.len() {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: helper.input_names.len(),
                found: args.len(),
            });
        }
        self.env
            .replace_root_f64_bindings(&helper.input_names, args);
        let mut evaluator = PureEvaluator::with_env(std::mem::take(&mut self.env));
        let result = if helper.scalar_eval_supported {
            evaluator
                .evaluate_scalar_expr(&helper.expr)
                .map(RuntimePureScalar::into_runtime_value)
        } else {
            evaluator.evaluate_expr(&helper.expr)
        };
        self.env = evaluator.into_env();
        result
    }

    pub fn evaluate_values(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if args.len() != helper.input_names.len() {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: helper.input_names.len(),
                found: args.len(),
            });
        }
        self.env
            .replace_root_value_bindings_ref(&helper.input_names, args);
        let mut evaluator = PureEvaluator::with_env(std::mem::take(&mut self.env));
        let result = if helper.scalar_eval_supported {
            evaluator
                .evaluate_scalar_expr(&helper.expr)
                .map(RuntimePureScalar::into_runtime_value)
        } else {
            evaluator.evaluate_expr(&helper.expr)
        };
        self.env = evaluator.into_env();
        result
    }
}

impl<T> RuntimeFixedArgs<T> {
    pub const MAX: usize = 4;

    pub const fn new(values: [T; 4], len: usize) -> Self {
        Self { len, values }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_slice(&self) -> &[T] {
        &self.values[..self.len]
    }

    pub fn into_parts(self) -> ([T; 4], usize) {
        (self.values, self.len)
    }
}

pub fn compare_pure_function_backend(
    vm: &impl PureFunctionBackend,
    candidate: &impl PureFunctionBackend,
    request: &PureFunctionRequest,
) -> Result<PureFunctionConformance, RuntimeEvalError> {
    let vm = vm.evaluate(request)?;
    let candidate = candidate.evaluate(request)?;
    let matches_vm = candidate.value == vm.value;
    Ok(PureFunctionConformance {
        vm,
        candidate,
        matches_vm,
    })
}

struct PureEvaluator {
    env: RuntimeEnv,
    stats: PureFunctionStats,
}

impl RuntimePureScalar {
    fn default_for_output(output_type: RuntimePureOutputType) -> Result<Self, RuntimeEvalError> {
        match output_type {
            RuntimePureOutputType::Bool => Ok(Self::Bool(false)),
            RuntimePureOutputType::I8 => Ok(Self::I8(0)),
            RuntimePureOutputType::I16 => Ok(Self::I16(0)),
            RuntimePureOutputType::I32 => Ok(Self::I32(0)),
            RuntimePureOutputType::I64 => Ok(Self::I64(0)),
            RuntimePureOutputType::I128 => Ok(Self::I128(0)),
            RuntimePureOutputType::ISize => Ok(Self::ISize(0)),
            RuntimePureOutputType::U8 => Ok(Self::U8(0)),
            RuntimePureOutputType::U16 => Ok(Self::U16(0)),
            RuntimePureOutputType::U32 => Ok(Self::U32(0)),
            RuntimePureOutputType::U64 => Ok(Self::U64(0)),
            RuntimePureOutputType::U128 => Ok(Self::U128(0)),
            RuntimePureOutputType::USize => Ok(Self::USize(0)),
            RuntimePureOutputType::F32 => Ok(Self::F32(0.0)),
            RuntimePureOutputType::F64 => Ok(Self::F64(0.0)),
            RuntimePureOutputType::Value => Err(RuntimeEvalError::UnsupportedPure {
                name: "pure".to_owned(),
                reason: "AOT scalar slots require a concrete scalar output type".to_owned(),
            }),
        }
    }

    const fn into_runtime_value(self) -> RuntimeValue {
        match self {
            Self::Bool(value) => RuntimeValue::Bool(value),
            Self::I8(value) => RuntimeValue::i8(value),
            Self::I16(value) => RuntimeValue::i16(value),
            Self::I32(value) => RuntimeValue::i32(value),
            Self::I64(value) => RuntimeValue::i64(value),
            Self::I128(value) => RuntimeValue::i128(value),
            Self::ISize(value) => RuntimeValue::isize(value),
            Self::U8(value) => RuntimeValue::u8(value),
            Self::U16(value) => RuntimeValue::u16(value),
            Self::U32(value) => RuntimeValue::u32(value),
            Self::U64(value) => RuntimeValue::u64(value),
            Self::U128(value) => RuntimeValue::u128(value),
            Self::USize(value) => RuntimeValue::usize(value),
            Self::F32(value) => RuntimeValue::F32(value),
            Self::F64(value) => RuntimeValue::F64(value),
        }
    }

    fn label(self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::I8(value) => value.to_string(),
            Self::I16(value) => value.to_string(),
            Self::I32(value) => value.to_string(),
            Self::I64(value) | Self::ISize(value) => value.to_string(),
            Self::I128(value) => value.to_string(),
            Self::U8(value) => value.to_string(),
            Self::U16(value) => value.to_string(),
            Self::U32(value) => value.to_string(),
            Self::U64(value) | Self::USize(value) => value.to_string(),
            Self::U128(value) => value.to_string(),
            Self::F32(value) => value.to_string(),
            Self::F64(value) => value.to_string(),
        }
    }
}

fn runtime_value_as_scalar(value: &RuntimeValue) -> Option<RuntimePureScalar> {
    match value {
        RuntimeValue::Bool(value) => Some(RuntimePureScalar::Bool(*value)),
        RuntimeValue::Int(value) => Some(runtime_int_as_scalar(*value)),
        RuntimeValue::UInt(value) => Some(runtime_uint_as_scalar(*value)),
        RuntimeValue::F32(value) => Some(RuntimePureScalar::F32(*value)),
        RuntimeValue::F64(value) => Some(RuntimePureScalar::F64(*value)),
        _ => None,
    }
}

fn runtime_value_into_scalar(value: RuntimeValue) -> Result<RuntimePureScalar, RuntimeEvalError> {
    match value {
        RuntimeValue::Bool(value) => Ok(RuntimePureScalar::Bool(value)),
        RuntimeValue::Int(value) => Ok(runtime_int_as_scalar(value)),
        RuntimeValue::UInt(value) => Ok(runtime_uint_as_scalar(value)),
        RuntimeValue::F32(value) => Ok(RuntimePureScalar::F32(value)),
        RuntimeValue::F64(value) => Ok(RuntimePureScalar::F64(value)),
        value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
    }
}

fn runtime_int_as_scalar(value: crate::value::RuntimeInt) -> RuntimePureScalar {
    match value {
        crate::value::RuntimeInt::I8(value) => RuntimePureScalar::I8(value),
        crate::value::RuntimeInt::I16(value) => RuntimePureScalar::I16(value),
        crate::value::RuntimeInt::I32(value) => RuntimePureScalar::I32(value),
        crate::value::RuntimeInt::I64(value) => RuntimePureScalar::I64(value),
        crate::value::RuntimeInt::I128(value) => RuntimePureScalar::I128(value),
        crate::value::RuntimeInt::ISize(value) => RuntimePureScalar::ISize(value),
    }
}

fn runtime_uint_as_scalar(value: crate::value::RuntimeUInt) -> RuntimePureScalar {
    match value {
        crate::value::RuntimeUInt::U8(value) => RuntimePureScalar::U8(value),
        crate::value::RuntimeUInt::U16(value) => RuntimePureScalar::U16(value),
        crate::value::RuntimeUInt::U32(value) => RuntimePureScalar::U32(value),
        crate::value::RuntimeUInt::U64(value) => RuntimePureScalar::U64(value),
        crate::value::RuntimeUInt::U128(value) => RuntimePureScalar::U128(value),
        crate::value::RuntimeUInt::USize(value) => RuntimePureScalar::USize(value),
    }
}

fn evaluate_scalar_unary(
    op: RuntimeUnaryOp,
    value: RuntimePureScalar,
) -> Result<RuntimePureScalar, RuntimeEvalError> {
    match (op, value) {
        (RuntimeUnaryOp::Not, RuntimePureScalar::Bool(value)) => {
            Ok(RuntimePureScalar::Bool(!value))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I8(value)) => {
            Ok(RuntimePureScalar::I8(value.wrapping_neg()))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I16(value)) => {
            Ok(RuntimePureScalar::I16(value.wrapping_neg()))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I32(value)) => {
            Ok(RuntimePureScalar::I32(value.wrapping_neg()))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I64(value)) => {
            Ok(RuntimePureScalar::I64(value.wrapping_neg()))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I128(value)) => {
            Ok(RuntimePureScalar::I128(value.wrapping_neg()))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::ISize(value)) => {
            Ok(RuntimePureScalar::ISize(value.wrapping_neg()))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::F32(value)) => Ok(RuntimePureScalar::F32(-value)),
        (RuntimeUnaryOp::Neg, RuntimePureScalar::F64(value)) => Ok(RuntimePureScalar::F64(-value)),
        (
            RuntimeUnaryOp::Neg,
            value @ (RuntimePureScalar::U8(_)
            | RuntimePureScalar::U16(_)
            | RuntimePureScalar::U32(_)
            | RuntimePureScalar::U64(_)
            | RuntimePureScalar::U128(_)
            | RuntimePureScalar::USize(_)),
        ) => Err(RuntimeEvalError::UnsupportedUnary {
            op: op.as_label(),
            value: value.label(),
        }),
        (op, value) => Err(RuntimeEvalError::UnsupportedUnary {
            op: op.as_label(),
            value: value.label(),
        }),
    }
}

fn evaluate_scalar_binary(
    lhs: RuntimePureScalar,
    op: RuntimeBinaryOp,
    rhs: RuntimePureScalar,
) -> Result<RuntimePureScalar, RuntimeEvalError> {
    match op {
        RuntimeBinaryOp::Eq => Ok(RuntimePureScalar::Bool(lhs == rhs)),
        RuntimeBinaryOp::Ne => Ok(RuntimePureScalar::Bool(lhs != rhs)),
        RuntimeBinaryOp::And => match (lhs, rhs) {
            (RuntimePureScalar::Bool(lhs), RuntimePureScalar::Bool(rhs)) => {
                Ok(RuntimePureScalar::Bool(lhs && rhs))
            }
            (lhs, rhs) => Err(RuntimeEvalError::UnsupportedBinary {
                op: op.as_label(),
                lhs: lhs.label(),
                rhs: rhs.label(),
            }),
        },
        RuntimeBinaryOp::Or => match (lhs, rhs) {
            (RuntimePureScalar::Bool(lhs), RuntimePureScalar::Bool(rhs)) => {
                Ok(RuntimePureScalar::Bool(lhs || rhs))
            }
            (lhs, rhs) => Err(RuntimeEvalError::UnsupportedBinary {
                op: op.as_label(),
                lhs: lhs.label(),
                rhs: rhs.label(),
            }),
        },
        RuntimeBinaryOp::Lt | RuntimeBinaryOp::Le | RuntimeBinaryOp::Gt | RuntimeBinaryOp::Ge => {
            evaluate_scalar_comparison(lhs, op, rhs)
        }
        RuntimeBinaryOp::Add
        | RuntimeBinaryOp::Sub
        | RuntimeBinaryOp::Mul
        | RuntimeBinaryOp::Div => evaluate_scalar_arithmetic(lhs, op, rhs),
    }
}

fn evaluate_scalar_comparison(
    lhs: RuntimePureScalar,
    op: RuntimeBinaryOp,
    rhs: RuntimePureScalar,
) -> Result<RuntimePureScalar, RuntimeEvalError> {
    match (lhs, rhs) {
        (RuntimePureScalar::I8(lhs), RuntimePureScalar::I8(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_ordered(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::I16(lhs), RuntimePureScalar::I16(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_ordered(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::I32(lhs), RuntimePureScalar::I32(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_ordered(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::I64(lhs), RuntimePureScalar::I64(rhs))
        | (RuntimePureScalar::ISize(lhs), RuntimePureScalar::ISize(rhs)) => Ok(
            RuntimePureScalar::Bool(compare_scalar_ordered(&lhs, op, &rhs)),
        ),
        (RuntimePureScalar::I128(lhs), RuntimePureScalar::I128(rhs)) => Ok(
            RuntimePureScalar::Bool(compare_scalar_ordered(&lhs, op, &rhs)),
        ),
        (RuntimePureScalar::U8(lhs), RuntimePureScalar::U8(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_ordered(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::U16(lhs), RuntimePureScalar::U16(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_ordered(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::U32(lhs), RuntimePureScalar::U32(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_ordered(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::U64(lhs), RuntimePureScalar::U64(rhs))
        | (RuntimePureScalar::USize(lhs), RuntimePureScalar::USize(rhs)) => Ok(
            RuntimePureScalar::Bool(compare_scalar_ordered(&lhs, op, &rhs)),
        ),
        (RuntimePureScalar::U128(lhs), RuntimePureScalar::U128(rhs)) => Ok(
            RuntimePureScalar::Bool(compare_scalar_ordered(&lhs, op, &rhs)),
        ),
        (RuntimePureScalar::F32(lhs), RuntimePureScalar::F32(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_float(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::F64(lhs), RuntimePureScalar::F64(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_float(&lhs, op, &rhs),
        )),
        (lhs, rhs) => Err(RuntimeEvalError::UnsupportedBinary {
            op: op.as_label(),
            lhs: lhs.label(),
            rhs: rhs.label(),
        }),
    }
}

fn evaluate_scalar_arithmetic(
    lhs: RuntimePureScalar,
    op: RuntimeBinaryOp,
    rhs: RuntimePureScalar,
) -> Result<RuntimePureScalar, RuntimeEvalError> {
    match (lhs, rhs) {
        (RuntimePureScalar::I8(lhs), RuntimePureScalar::I8(rhs)) => {
            Ok(RuntimePureScalar::I8(evaluate_scalar_numeric(lhs, op, rhs)))
        }
        (RuntimePureScalar::I16(lhs), RuntimePureScalar::I16(rhs)) => Ok(RuntimePureScalar::I16(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::I32(lhs), RuntimePureScalar::I32(rhs)) => Ok(RuntimePureScalar::I32(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::I64(lhs), RuntimePureScalar::I64(rhs)) => Ok(RuntimePureScalar::I64(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::I128(lhs), RuntimePureScalar::I128(rhs)) => Ok(
            RuntimePureScalar::I128(evaluate_scalar_numeric(lhs, op, rhs)),
        ),
        (RuntimePureScalar::ISize(lhs), RuntimePureScalar::ISize(rhs)) => Ok(
            RuntimePureScalar::ISize(evaluate_scalar_numeric(lhs, op, rhs)),
        ),
        (RuntimePureScalar::U8(lhs), RuntimePureScalar::U8(rhs)) => {
            Ok(RuntimePureScalar::U8(evaluate_scalar_numeric(lhs, op, rhs)))
        }
        (RuntimePureScalar::U16(lhs), RuntimePureScalar::U16(rhs)) => Ok(RuntimePureScalar::U16(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::U32(lhs), RuntimePureScalar::U32(rhs)) => Ok(RuntimePureScalar::U32(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::U64(lhs), RuntimePureScalar::U64(rhs)) => Ok(RuntimePureScalar::U64(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::U128(lhs), RuntimePureScalar::U128(rhs)) => Ok(
            RuntimePureScalar::U128(evaluate_scalar_numeric(lhs, op, rhs)),
        ),
        (RuntimePureScalar::USize(lhs), RuntimePureScalar::USize(rhs)) => Ok(
            RuntimePureScalar::USize(evaluate_scalar_numeric(lhs, op, rhs)),
        ),
        (RuntimePureScalar::F32(lhs), RuntimePureScalar::F32(rhs)) => Ok(RuntimePureScalar::F32(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::F64(lhs), RuntimePureScalar::F64(rhs)) => Ok(RuntimePureScalar::F64(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (lhs, rhs) => Err(RuntimeEvalError::UnsupportedBinary {
            op: op.as_label(),
            lhs: lhs.label(),
            rhs: rhs.label(),
        }),
    }
}

fn compare_scalar_ordered<T: Ord>(lhs: &T, op: RuntimeBinaryOp, rhs: &T) -> bool {
    match op {
        RuntimeBinaryOp::Lt => lhs < rhs,
        RuntimeBinaryOp::Le => lhs <= rhs,
        RuntimeBinaryOp::Gt => lhs > rhs,
        RuntimeBinaryOp::Ge => lhs >= rhs,
        _ => unreachable!(),
    }
}

fn compare_scalar_float<T: PartialOrd>(lhs: &T, op: RuntimeBinaryOp, rhs: &T) -> bool {
    match op {
        RuntimeBinaryOp::Lt => lhs < rhs,
        RuntimeBinaryOp::Le => lhs <= rhs,
        RuntimeBinaryOp::Gt => lhs > rhs,
        RuntimeBinaryOp::Ge => lhs >= rhs,
        _ => unreachable!(),
    }
}

fn evaluate_scalar_numeric<T: crate::value::RuntimeDeterministicNumeric>(
    lhs: T,
    op: RuntimeBinaryOp,
    rhs: T,
) -> T {
    evaluate_numeric_op(lhs, op, rhs)
}

struct PureScalarEvaluator<'a, T> {
    input_names: &'a [String],
    args: &'a [T],
    locals: Vec<(String, RuntimePureScalar)>,
}

impl<'a, T: RuntimePureScalarInteger> PureScalarEvaluator<'a, T> {
    fn new_exact(input_names: &'a [String], args: &'a [T]) -> Self {
        Self {
            input_names,
            args,
            locals: Vec::new(),
        }
    }

    fn evaluate(&mut self, expr: &RuntimeExpr) -> Result<RuntimePureScalar, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(value) => runtime_value_as_scalar(value)
                .ok_or_else(|| RuntimeEvalError::ExpectedInt(runtime_value_label(value))),
            RuntimeExpr::Local(name) => self
                .get(name)
                .ok_or_else(|| RuntimeEvalError::UnknownBinding(name.clone())),
            RuntimeExpr::Let { name, expr, body } => {
                let value = self.evaluate(expr)?;
                self.locals.push((name.clone(), value));
                let result = self.evaluate(body);
                self.locals.pop();
                result
            }
            RuntimeExpr::Unary { op, expr } => evaluate_scalar_unary(*op, self.evaluate(expr)?),
            RuntimeExpr::Binary { lhs, op, rhs } => {
                let lhs = self.evaluate(lhs)?;
                let rhs = self.evaluate(rhs)?;
                evaluate_scalar_binary(lhs, *op, rhs)
            }
            RuntimeExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                if self.evaluate_bool(condition)? {
                    self.evaluate(then_expr)
                } else {
                    self.evaluate(else_expr)
                }
            }
            expr => Err(RuntimeEvalError::UnsupportedPure {
                name: "scalar pure".to_owned(),
                reason: format!("{expr} is not in the exact scalar subset"),
            }),
        }
    }

    fn evaluate_bool(&mut self, expr: &RuntimeExpr) -> Result<bool, RuntimeEvalError> {
        match self.evaluate(expr)? {
            RuntimePureScalar::Bool(value) => Ok(value),
            value => Err(RuntimeEvalError::ExpectedBool(value.label())),
        }
    }

    fn get(&self, name: &str) -> Option<RuntimePureScalar> {
        self.locals
            .iter()
            .rev()
            .find_map(|(local, value)| (local == name).then_some(*value))
            .or_else(|| {
                self.input_names
                    .iter()
                    .zip(self.args.iter().copied())
                    .find_map(|(input_name, value)| {
                        (input_name == name).then_some(value.into_pure_scalar())
                    })
            })
    }
}

impl PureEvaluator {
    fn new_ref(bindings: &[RuntimeBinding]) -> Self {
        let mut env = RuntimeEnv::default();
        env.bind_all_ref(bindings);
        Self {
            env,
            stats: PureFunctionStats::default(),
        }
    }

    fn with_env(env: RuntimeEnv) -> Self {
        Self {
            env,
            stats: PureFunctionStats::default(),
        }
    }

    fn into_env(self) -> RuntimeEnv {
        self.env
    }

    fn evaluate_expr(&mut self, expr: &RuntimeExpr) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.evaluated_exprs += 1;
        match expr {
            RuntimeExpr::Value(value) => Ok(value.clone()),
            RuntimeExpr::Local(name) => self
                .env
                .get(name)
                .cloned()
                .ok_or_else(|| RuntimeEvalError::UnknownBinding(name.clone())),
            RuntimeExpr::EntityRef(target) => Ok(RuntimeValue::EntityRef(target.clone())),
            RuntimeExpr::Let { name, expr, body } => self.evaluate_let_expr(name, expr, body),
            RuntimeExpr::Tuple(items) => items
                .iter()
                .map(|item| self.evaluate_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::Tuple),
            RuntimeExpr::BracketSeq(items) => items
                .iter()
                .map(|item| self.evaluate_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map(runtime_sequence_values),
            RuntimeExpr::RepeatSeq { value, len } => self.evaluate_repeat_seq_expr(value, *len),
            RuntimeExpr::Range {
                start,
                end,
                inclusive,
            } => self.evaluate_range_expr(start.as_deref(), end.as_deref(), *inclusive),
            RuntimeExpr::Record(fields) => self.evaluate_record_expr(fields),
            RuntimeExpr::Variant {
                owner,
                ordinal,
                name,
                payload,
            } => self.evaluate_variant_expr(owner, *ordinal, name, payload.as_deref()),
            RuntimeExpr::Field { target, field } => self.evaluate_field_expr(target, field),
            RuntimeExpr::ProjectTuple { target, ordinal } => {
                self.evaluate_project_tuple_expr(target, *ordinal)
            }
            RuntimeExpr::ProjectRecord { target, ordinal } => {
                self.evaluate_project_record_expr(target, *ordinal)
            }
            RuntimeExpr::Call { callee, args } => self.evaluate_call_expr(callee, args),
            RuntimeExpr::Function { params, body } => Ok(self.evaluate_function_expr(params, body)),
            RuntimeExpr::Apply { callee, args } => self.evaluate_apply_expr(callee, args),
            RuntimeExpr::AssignField { .. } | RuntimeExpr::TraitCall { .. } => {
                Self::unsupported_flow_runtime_expr()
            }
            RuntimeExpr::PureCall { .. } => Err(RuntimeEvalError::UnsupportedPure {
                name: "pure call".to_owned(),
                reason: "nested runtime pure calls require a runtime pure backend".to_owned(),
            }),
            RuntimeExpr::SpreadArg(_) => Err(RuntimeEvalError::SpreadOutsideCall),
            RuntimeExpr::MethodCall {
                receiver,
                method,
                args,
            } => self.evaluate_method_call_expr(receiver, method, args),
            RuntimeExpr::Map {
                source,
                param,
                body,
            } => self.evaluate_map_expr(source, param, body),
            RuntimeExpr::Filter {
                source,
                param,
                body,
            } => self.evaluate_filter_expr(source, param, body),
            RuntimeExpr::Sum { source } => self.evaluate_sum_expr(source),
            RuntimeExpr::Unary { op, expr } => {
                let value = self.evaluate_expr(expr)?;
                evaluate_unary(*op, value)
            }
            RuntimeExpr::Binary { lhs, op, rhs } => {
                self.stats.evaluated_binary_ops += 1;
                let lhs = self.evaluate_expr(lhs)?;
                let rhs = self.evaluate_expr(rhs)?;
                evaluate_binary(lhs, *op, rhs)
            }
            RuntimeExpr::If {
                condition,
                then_expr,
                else_expr,
            } => self.evaluate_if_expr(condition, then_expr, else_expr),
            RuntimeExpr::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => self.evaluate_if_let_expr(pattern, expr, guard.as_deref(), then_expr, else_expr),
            RuntimeExpr::Match { scrutinee, arms } => self.evaluate_match_expr(scrutinee, arms),
        }
    }

    fn evaluate_record_expr(
        &mut self,
        fields: &[RuntimeFieldExpr],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        fields
            .iter()
            .map(|field| {
                Ok(RuntimeFieldValue {
                    name: field.name.clone(),
                    value: self.evaluate_expr(&field.value)?,
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeValue::Record)
    }

    fn evaluate_variant_expr(
        &mut self,
        owner: &crate::pattern::RuntimeCheckedType,
        ordinal: u32,
        name: &str,
        payload: Option<&RuntimeExpr>,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if !owner.accepts_variant_case(ordinal, name) {
            return Err(RuntimeEvalError::PatternMismatch(format!(
                "variant owner {owner:?} case {ordinal} `{name}`"
            )));
        }
        let owner = owner.variant_identity().ok_or_else(|| {
            RuntimeEvalError::PatternMismatch(format!("non-variant checked owner {owner:?}"))
        })?;
        Ok(RuntimeValue::Variant {
            owner,
            ordinal,
            name: name.to_owned(),
            payload: payload
                .map(|expr| self.evaluate_expr(expr).map(Box::new))
                .transpose()?,
        })
    }

    fn evaluate_repeat_seq_expr(
        &mut self,
        value: &RuntimeExpr,
        len: usize,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let RuntimeExpr::Value(value) = value {
            return Ok(runtime_sequence_values(vec![value.clone(); len]));
        }
        (0..len)
            .map(|_| self.evaluate_expr(value))
            .collect::<Result<Vec<_>, _>>()
            .map(runtime_sequence_values)
    }

    fn evaluate_if_expr(
        &mut self,
        condition: &RuntimeExpr,
        then_expr: &RuntimeExpr,
        else_expr: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if self.evaluate_bool(condition)? {
            self.evaluate_expr(then_expr)
        } else {
            self.evaluate_expr(else_expr)
        }
    }

    fn evaluate_if_let_expr(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
        then_expr: &RuntimeExpr,
        else_expr: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(expr)?;
        let Some(bindings) = match_runtime_pattern(pattern, &value)? else {
            return self.evaluate_expr(else_expr);
        };
        let guard_matched = if let Some(guard) = guard {
            self.with_temp_bindings_ref(&bindings, |this| this.evaluate_bool(guard))?
        } else {
            true
        };
        if guard_matched {
            self.with_temp_bindings(bindings, |this| this.evaluate_expr(then_expr))
        } else {
            self.evaluate_expr(else_expr)
        }
    }

    fn evaluate_match_expr(
        &mut self,
        scrutinee: &RuntimeExpr,
        arms: &[RuntimeExprMatchArm],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(scrutinee)?;
        for arm in arms {
            let Some(bindings) = match_runtime_pattern(&arm.pattern, &value)? else {
                continue;
            };
            if let Some(guard) = arm.guard.as_ref()
                && !self.with_temp_bindings_ref(&bindings, |this| this.evaluate_bool(guard))?
            {
                continue;
            }
            return self.with_temp_bindings(bindings, |this| this.evaluate_expr(&arm.value));
        }
        Err(RuntimeEvalError::PatternMismatch(runtime_value_label(
            &value,
        )))
    }

    fn with_temp_bindings<T>(
        &mut self,
        bindings: Vec<RuntimeBinding>,
        f: impl FnOnce(&mut Self) -> Result<T, RuntimeEvalError>,
    ) -> Result<T, RuntimeEvalError> {
        self.env.push_scope_with_capacity(bindings.len());
        self.env.bind_all(bindings);
        let result = f(self);
        self.env.pop_scope();
        result
    }

    fn with_temp_bindings_ref<T>(
        &mut self,
        bindings: &[RuntimeBinding],
        f: impl FnOnce(&mut Self) -> Result<T, RuntimeEvalError>,
    ) -> Result<T, RuntimeEvalError> {
        self.env.push_scope_with_capacity(bindings.len());
        self.env.bind_all_ref(bindings);
        let result = f(self);
        self.env.pop_scope();
        result
    }

    fn unsupported_flow_runtime_expr() -> Result<RuntimeValue, RuntimeEvalError> {
        Err(RuntimeEvalError::UnsupportedPure {
            name: "trait method".to_owned(),
            reason: "trait dispatch and mutation require the flow runtime".to_owned(),
        })
    }

    fn evaluate_range_expr(
        &mut self,
        start: Option<&RuntimeExpr>,
        end: Option<&RuntimeExpr>,
        inclusive: bool,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let start = start.map(|expr| self.evaluate_expr(expr)).transpose()?;
        let end = end.map(|expr| self.evaluate_expr(expr)).transpose()?;
        crate::value::RuntimeRange::new(start, end, inclusive).map(RuntimeValue::Range)
    }

    fn evaluate_let_expr(
        &mut self,
        name: &str,
        expr: &RuntimeExpr,
        body: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(expr)?;
        self.env.push_scope_with_capacity(1);
        self.env.set(name.to_owned(), value);
        let result = self.evaluate_expr(body);
        self.env.pop_scope();
        result
    }

    fn evaluate_scalar_expr(
        &mut self,
        expr: &RuntimeExpr,
    ) -> Result<RuntimePureScalar, RuntimeEvalError> {
        self.stats.evaluated_exprs += 1;
        match expr {
            RuntimeExpr::Value(RuntimeValue::Bool(value)) => Ok(RuntimePureScalar::Bool(*value)),
            RuntimeExpr::Value(RuntimeValue::Int(value)) => Ok(runtime_int_as_scalar(*value)),
            RuntimeExpr::Value(RuntimeValue::UInt(value)) => Ok(runtime_uint_as_scalar(*value)),
            RuntimeExpr::Value(RuntimeValue::F32(value)) => Ok(RuntimePureScalar::F32(*value)),
            RuntimeExpr::Value(RuntimeValue::F64(value)) => Ok(RuntimePureScalar::F64(*value)),
            RuntimeExpr::Local(name) => match self.env.get(name) {
                Some(value) => runtime_value_as_scalar(value)
                    .ok_or_else(|| RuntimeEvalError::ExpectedInt(runtime_value_label(value))),
                None => Err(RuntimeEvalError::UnknownBinding(name.clone())),
            },
            RuntimeExpr::Let { name, expr, body } => {
                let value = self.evaluate_scalar_expr(expr)?.into_runtime_value();
                self.env.push_scope_with_capacity(1);
                self.env.set(name.clone(), value);
                let result = self.evaluate_scalar_expr(body);
                self.env.pop_scope();
                result
            }
            RuntimeExpr::Unary { op, expr } => {
                let value = self.evaluate_scalar_expr(expr)?;
                evaluate_scalar_unary(*op, value)
            }
            RuntimeExpr::Binary { lhs, op, rhs } => {
                self.stats.evaluated_binary_ops += 1;
                let lhs = self.evaluate_scalar_expr(lhs)?;
                let rhs = self.evaluate_scalar_expr(rhs)?;
                evaluate_scalar_binary(lhs, *op, rhs)
            }
            RuntimeExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                if self.evaluate_scalar_bool(condition)? {
                    self.evaluate_scalar_expr(then_expr)
                } else {
                    self.evaluate_scalar_expr(else_expr)
                }
            }
            _ => self.evaluate_expr(expr).and_then(runtime_value_into_scalar),
        }
    }

    fn evaluate_scalar_bool(&mut self, expr: &RuntimeExpr) -> Result<bool, RuntimeEvalError> {
        match self.evaluate_scalar_expr(expr)? {
            RuntimePureScalar::Bool(value) => Ok(value),
            value => Err(RuntimeEvalError::ExpectedBool(value.label())),
        }
    }

    fn evaluate_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let iterator = match RuntimeIterator::from_value(self.evaluate_expr(source)?) {
            Ok(iterator) => iterator,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
        iterator
            .collect::<Vec<_>>()
            .into_iter()
            .map(|item| {
                self.env.push_scope_with_capacity(1);
                self.env.set(param.to_owned(), item);
                let result = self.evaluate_expr(body);
                self.env.pop_scope();
                result
            })
            .collect::<Result<Vec<_>, _>>()
            .map(runtime_sequence_values)
    }

    fn evaluate_filter_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let iterator = match RuntimeIterator::from_value(self.evaluate_expr(source)?) {
            Ok(iterator) => iterator,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
        let mut filtered = Vec::new();
        for item in iterator.collect::<Vec<_>>() {
            self.env.push_scope_with_capacity(1);
            self.env.set(param.to_owned(), item.clone());
            let keep = self.evaluate_bool(body);
            self.env.pop_scope();
            if keep? {
                filtered.push(item);
            }
        }
        Ok(runtime_sequence_values(filtered))
    }

    fn evaluate_sum_expr(
        &mut self,
        source: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let RuntimeExpr::Local(name) = source
            && let Some(sum) = self.evaluate_i64_local_sequence_sum(name)?
        {
            return Ok(RuntimeValue::i64(sum));
        }
        let value = self.evaluate_expr(source)?;
        if let RuntimeValue::Seq(seq) = &value
            && let Some(sum) = seq.sum_as_i64()
        {
            return Ok(RuntimeValue::i64(sum));
        }
        let iterator = match RuntimeIterator::from_value(value) {
            Ok(iterator) => iterator,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
        let items = iterator.collect::<Vec<_>>();
        sum_i64_sequence_ref(&items).map(RuntimeValue::i64)
    }

    fn evaluate_i64_local_sequence_sum(&self, name: &str) -> Result<Option<i64>, RuntimeEvalError> {
        let Some(value) = self.env.get(name) else {
            return Ok(None);
        };
        match value {
            RuntimeValue::Seq(seq) => match seq {
                RuntimeSeq::Values(items) => sum_i64_sequence_ref(items).map(Some),
                RuntimeSeq::Dense(items) => Ok(items.sum_as_i64()),
                RuntimeSeq::TupleColumns(_) | RuntimeSeq::RecordColumns(_) => Ok(None),
            },
            RuntimeValue::Tuple(items) => sum_i64_sequence_ref(items).map(Some),
            _ => Ok(None),
        }
    }

    fn evaluate_call_expr(
        &mut self,
        callee: &crate::value::RuntimeCallTarget,
        args: &[RuntimeExpr],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.evaluated_calls += 1;
        let args = self.evaluate_call_args(args)?;
        if let Some(intrinsic) = callee.as_intrinsic()
            && let Some(value) = evaluate_std_float_intrinsic(intrinsic, &args)?
        {
            return Ok(value);
        }
        match (callee.as_intrinsic(), args.as_slice()) {
            (Some(RuntimeIntrinsic::Add), [RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)]) => {
                evaluate_binary(
                    RuntimeValue::Int(*lhs),
                    RuntimeBinaryOp::Add,
                    RuntimeValue::Int(*rhs),
                )
            }
            (Some(RuntimeIntrinsic::CoreRange), _) => evaluate_core_range_intrinsic(&args),
            (Some(RuntimeIntrinsic::CoreIterCollect), [value]) => {
                evaluate_core_iter_collect_intrinsic(value.clone())
            }
            (Some(RuntimeIntrinsic::CoreIterIntoIter), [value, evidence]) => {
                evaluate_core_iter_into_iter_intrinsic(value.clone(), evidence)
            }
            (Some(RuntimeIntrinsic::CoreIterNext), [value]) => {
                evaluate_core_iter_next_intrinsic(value.clone())
            }
            (Some(RuntimeIntrinsic::CoreOptionIsSome), [value]) => {
                evaluate_core_option_is_some_intrinsic(value)
            }
            (Some(RuntimeIntrinsic::CoreOptionUnwrap), [value]) => {
                evaluate_core_option_unwrap_intrinsic(value.clone())
            }
            (
                Some(RuntimeIntrinsic::MathMatmulF32),
                [RuntimeValue::MatrixF32(lhs), RuntimeValue::MatrixF32(rhs)],
            ) => lhs
                .matmul_scalar(rhs)
                .map(RuntimeValue::matrix_f32)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: error.to_string(),
                }),
            (
                Some(RuntimeIntrinsic::MathMatrixAddF32),
                [RuntimeValue::MatrixF32(lhs), RuntimeValue::MatrixF32(rhs)],
            ) => lhs
                .add_scalar(rhs)
                .map(RuntimeValue::matrix_f32)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: error.to_string(),
                }),
            (
                Some(RuntimeIntrinsic::MathTensorAddF32),
                [RuntimeValue::TensorF32(lhs), RuntimeValue::TensorF32(rhs)],
            ) => lhs
                .add_scalar(rhs)
                .map(RuntimeValue::tensor_f32)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: error.to_string(),
                }),
            (
                Some(RuntimeIntrinsic::MathMatmulF64),
                [RuntimeValue::MatrixF64(lhs), RuntimeValue::MatrixF64(rhs)],
            ) => lhs
                .matmul_scalar(rhs)
                .map(RuntimeValue::matrix_f64)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: error.to_string(),
                }),
            (
                Some(RuntimeIntrinsic::MathMatrixAddF64),
                [RuntimeValue::MatrixF64(lhs), RuntimeValue::MatrixF64(rhs)],
            ) => lhs
                .add_scalar(rhs)
                .map(RuntimeValue::matrix_f64)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: error.to_string(),
                }),
            (
                Some(RuntimeIntrinsic::MathTensorAddF64),
                [RuntimeValue::TensorF64(lhs), RuntimeValue::TensorF64(rhs)],
            ) => lhs
                .add_scalar(rhs)
                .map(RuntimeValue::tensor_f64)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: error.to_string(),
                }),
            _ => Err(RuntimeEvalError::UnsupportedPure {
                name: callee.as_label().to_owned(),
                reason: "call is not registered as a pure helper".to_owned(),
            }),
        }
    }

    fn evaluate_field_expr(
        &mut self,
        target: &RuntimeExpr,
        field: &str,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(target)?;
        match value {
            RuntimeValue::Record(fields) => fields
                .into_iter()
                .find(|candidate| candidate.name == field)
                .map(|field| field.value)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: field.to_owned(),
                    value: "record".to_owned(),
                }),
            RuntimeValue::Seq(RuntimeSeq::RecordColumns(records)) => records
                .field_by_name(field)
                .cloned()
                .map(RuntimeValue::Seq)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: field.to_owned(),
                    value: "record sequence".to_owned(),
                }),
            RuntimeValue::EntityRef(id) => {
                Self::entity_ref_field(&id, field).ok_or_else(|| RuntimeEvalError::MissingField {
                    field: field.to_owned(),
                    value: "entity reference".to_owned(),
                })
            }
            value => Err(RuntimeEvalError::MissingField {
                field: field.to_owned(),
                value: runtime_value_label(&value),
            }),
        }
    }

    fn entity_ref_field(id: &str, field: &str) -> Option<RuntimeValue> {
        Some(match field {
            "id" => RuntimeValue::String(id.to_owned()),
            "family" => RuntimeValue::String(Self::entity_ref_family(id).to_owned()),
            "name" => RuntimeValue::String(Self::entity_ref_name(id).to_owned()),
            _ => return None,
        })
    }

    fn entity_ref_family(id: &str) -> &str {
        id.split_once('.').map_or(id, |(family, _)| family)
    }

    fn entity_ref_name(id: &str) -> &str {
        id.split_once('.').map_or("", |(_, name)| name)
    }

    fn evaluate_project_tuple_expr(
        &mut self,
        target: &RuntimeExpr,
        ordinal: usize,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(target)?;
        match value {
            RuntimeValue::Tuple(items) => {
                items
                    .into_iter()
                    .nth(ordinal)
                    .ok_or_else(|| RuntimeEvalError::MissingField {
                        field: ordinal.to_string(),
                        value: "tuple".to_owned(),
                    })
            }
            RuntimeValue::Seq(RuntimeSeq::TupleColumns(columns)) => columns
                .column(ordinal)
                .cloned()
                .map(RuntimeValue::Seq)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: ordinal.to_string(),
                    value: "tuple sequence".to_owned(),
                }),
            value => Err(RuntimeEvalError::MissingField {
                field: ordinal.to_string(),
                value: runtime_value_label(&value),
            }),
        }
    }

    fn evaluate_project_record_expr(
        &mut self,
        target: &RuntimeExpr,
        ordinal: usize,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(target)?;
        match value {
            RuntimeValue::Record(fields) => fields.into_iter().nth(ordinal).map_or_else(
                || {
                    Err(RuntimeEvalError::MissingField {
                        field: ordinal.to_string(),
                        value: "record".to_owned(),
                    })
                },
                |field| Ok(field.value),
            ),
            RuntimeValue::Seq(RuntimeSeq::RecordColumns(records)) => records
                .field_by_ordinal(ordinal)
                .cloned()
                .map(RuntimeValue::Seq)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: ordinal.to_string(),
                    value: "record sequence".to_owned(),
                }),
            value => Err(RuntimeEvalError::MissingField {
                field: ordinal.to_string(),
                value: runtime_value_label(&value),
            }),
        }
    }

    fn evaluate_function_expr(&self, params: &[String], body: &RuntimeExpr) -> RuntimeValue {
        RuntimeValue::Function(RuntimeFunctionValue::new(
            params.to_vec(),
            body.clone(),
            self.env.bindings_snapshot(),
        ))
    }

    fn evaluate_apply_expr(
        &mut self,
        callee: &RuntimeExpr,
        args: &[RuntimeExpr],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let callee = self.evaluate_expr(callee)?;
        let args = self.evaluate_call_args(args)?;
        match callee {
            RuntimeValue::Function(function) => self.apply_runtime_function(&function, &args),
            value => Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &value,
            ))),
        }
    }

    fn apply_runtime_function(
        &mut self,
        function: &RuntimeFunctionValue,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if args.len() < function.arity() {
            return Ok(RuntimeValue::Function(function.partially_apply(args)));
        }

        let (call_args, remaining_args) = args.split_at(function.arity());
        let value = self.call_runtime_function(function, call_args)?;
        if remaining_args.is_empty() {
            return Ok(value);
        }
        match value {
            RuntimeValue::Function(next) => self.apply_runtime_function(&next, remaining_args),
            _ => Err(RuntimeEvalError::FunctionArgumentCount {
                expected: function.arity(),
                found: args.len(),
            }),
        }
    }

    fn call_runtime_function(
        &mut self,
        function: &RuntimeFunctionValue,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.env
            .push_scope_with_capacity(function.captures.len() + args.len());
        self.env.bind_all_ref(&function.captures);
        for (param, value) in function.params.iter().zip(args) {
            self.env.set_ref(param, value);
        }
        let Some(body) = function.expr_body() else {
            self.env.pop_scope();
            return Err(RuntimeEvalError::UnsupportedPure {
                name: "awbc.function".to_owned(),
                reason: "pure evaluator cannot evaluate an AWBC function body".to_owned(),
            });
        };
        let result = self.evaluate_expr(body);
        self.env.pop_scope();
        result
    }

    fn evaluate_method_call_expr(
        &mut self,
        receiver: &RuntimeExpr,
        method: &str,
        args: &[RuntimeExpr],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.evaluated_method_calls += 1;
        let receiver = self.evaluate_expr(receiver)?;
        let args = self.evaluate_call_args(args)?;
        match (receiver, method, args.as_slice()) {
            (RuntimeValue::String(value), "trim", []) => {
                Ok(RuntimeValue::String(value.trim().to_owned()))
            }
            (RuntimeValue::String(value), "to_string", []) => Ok(RuntimeValue::String(value)),
            (RuntimeValue::String(value), "len", []) => {
                Ok(RuntimeValue::from_collection_len(value.chars().count()))
            }
            (RuntimeValue::Seq(seq), "len", []) => Ok(RuntimeValue::from_collection_len(seq.len())),
            (RuntimeValue::Seq(seq), "contains", [needle]) => Ok(RuntimeValue::Bool(
                seq.into_values().iter().any(|item| item == needle),
            )),
            (RuntimeValue::Seq(seq), "require_role", [RuntimeValue::String(role)]) => Ok(seq
                .into_values()
                .into_iter()
                .find(|item| {
                    Self::runtime_record_string_field(item, "role").as_deref() == Some(role)
                })
                .unwrap_or(RuntimeValue::Unit)),
            (RuntimeValue::Seq(seq), "__index", [index]) => Ok(seq.value_at_runtime_index(index)),
            (RuntimeValue::Tuple(items), "len", []) => {
                Ok(RuntimeValue::from_collection_len(items.len()))
            }
            (RuntimeValue::Tuple(items), "contains", [needle]) => {
                Ok(RuntimeValue::Bool(items.iter().any(|item| item == needle)))
            }
            (RuntimeValue::Tuple(items), "__index", [index]) => Ok(index
                .to_collection_index()
                .and_then(|index| items.get(index).cloned())
                .unwrap_or(RuntimeValue::Unit)),
            (
                RuntimeValue::Record(fields),
                "get",
                [RuntimeValue::String(key) | RuntimeValue::EntityRef(key)],
            ) => Ok(fields
                .iter()
                .find(|field| field.name == *key)
                .map_or(RuntimeValue::Unit, |field| field.value.clone())),
            (receiver, _, _) => Err(RuntimeEvalError::UnsupportedPure {
                name: method.to_owned(),
                reason: format!(
                    "method is not registered for {}",
                    runtime_value_label(&receiver)
                ),
            }),
        }
    }

    fn runtime_record_string_field(value: &RuntimeValue, field: &str) -> Option<String> {
        let RuntimeValue::Record(fields) = value else {
            return None;
        };
        fields
            .iter()
            .find(|candidate| candidate.name == field)
            .and_then(|candidate| match &candidate.value {
                RuntimeValue::String(value) | RuntimeValue::EntityRef(value) => Some(value.clone()),
                _ => None,
            })
    }

    fn evaluate_bool(&mut self, expr: &RuntimeExpr) -> Result<bool, RuntimeEvalError> {
        match self.evaluate_expr(expr)? {
            RuntimeValue::Bool(value) => Ok(value),
            value => Err(RuntimeEvalError::ExpectedBool(runtime_value_label(&value))),
        }
    }

    fn evaluate_call_args(
        &mut self,
        args: &[RuntimeExpr],
    ) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
        let mut values = Vec::new();
        for arg in args {
            match arg {
                RuntimeExpr::SpreadArg(expr) => {
                    let spread = self.evaluate_expr(expr)?;
                    values.extend(spread_runtime_values(spread)?);
                }
                expr => values.push(self.evaluate_expr(expr)?),
            }
        }
        Ok(values)
    }
}

fn spread_runtime_values(value: RuntimeValue) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
    match runtime_value_into_sequence_values(value) {
        Ok(items) => Ok(items),
        Err(value) => Err(RuntimeEvalError::InvalidSpread(runtime_value_label(&value))),
    }
}
