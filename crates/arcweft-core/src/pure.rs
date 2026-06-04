use crate::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
use crate::plan::{RuntimePureHelper, RuntimePureInputType, RuntimePureOutputType};
use crate::step::RuntimePureCallStats;
use crate::value::{
    RuntimeBinaryOp, RuntimeBinding, RuntimeCallTarget, RuntimeEnv, RuntimeEvalError,
    RuntimeExactInteger, RuntimeExpr, RuntimeFieldValue, RuntimeISizeValue, RuntimeIntrinsic,
    RuntimeSeq, RuntimeUSizeValue, RuntimeUnaryOp, RuntimeValue, evaluate_binary,
    evaluate_numeric_op, evaluate_std_float_intrinsic, evaluate_unary, runtime_binary_op_label,
    runtime_sequence_values, runtime_unary_op_label, runtime_value_into_sequence_values,
    runtime_value_label, sum_i64_sequence_ref,
};
use std::collections::BTreeMap;

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
    expr: AotI64Expr,
    initial_slots: Vec<i64>,
    input_slots: Vec<usize>,
    slot_count: usize,
}

/// Compiled AOT plan for exact-width scalar helpers that are not widened to `i64`.
#[derive(Clone, Debug, PartialEq)]
pub struct AotPureScalarPlan {
    name: String,
    expr: AotScalarExpr,
    initial_slots: Vec<RuntimePureScalar>,
    input_slots: Vec<usize>,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    slot_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
enum AotI64Expr {
    Const(i64),
    Local(usize),
    Let {
        slot: usize,
        expr: Box<AotI64Expr>,
        body: Box<AotI64Expr>,
    },
    AddCall {
        lhs: Box<AotI64Expr>,
        rhs: Box<AotI64Expr>,
    },
    Unary {
        op: RuntimeUnaryOp,
        expr: Box<AotI64Expr>,
    },
    Binary {
        lhs: Box<AotI64Expr>,
        op: RuntimeBinaryOp,
        rhs: Box<AotI64Expr>,
    },
    If {
        condition: AotBoolExpr,
        then_expr: Box<AotI64Expr>,
        else_expr: Box<AotI64Expr>,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum AotBoolExpr {
    Const(bool),
    Compare {
        lhs: Box<AotI64Expr>,
        op: RuntimeBinaryOp,
        rhs: Box<AotI64Expr>,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum AotScalarExpr {
    Const(RuntimePureScalar),
    Local(usize),
    Let {
        slot: usize,
        expr: Box<AotScalarExpr>,
        body: Box<AotScalarExpr>,
    },
    AddCall {
        lhs: Box<AotScalarExpr>,
        rhs: Box<AotScalarExpr>,
    },
    Unary {
        op: RuntimeUnaryOp,
        expr: Box<AotScalarExpr>,
    },
    Binary {
        lhs: Box<AotScalarExpr>,
        op: RuntimeBinaryOp,
        rhs: Box<AotScalarExpr>,
    },
    If {
        condition: AotScalarBoolExpr,
        then_expr: Box<AotScalarExpr>,
        else_expr: Box<AotScalarExpr>,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum AotScalarBoolExpr {
    Const(bool),
    Compare {
        lhs: Box<AotScalarExpr>,
        op: RuntimeBinaryOp,
        rhs: Box<AotScalarExpr>,
    },
}

#[derive(Clone, Debug)]
struct AotCompileContext {
    slots: BTreeMap<String, usize>,
    next_slot: usize,
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

impl AotPureFunctionBackend {
    pub const fn new() -> Self {
        Self
    }

    /// Compiles a deterministic helper request to a typed `i64` AOT plan.
    pub fn compile_i64(
        &self,
        request: &PureFunctionRequest,
    ) -> Result<AotPureI64Plan, RuntimeEvalError> {
        AotPureI64Plan::compile(request)
    }

    /// Compiles a deterministic helper request with selected runtime integer inputs.
    pub fn compile_i64_with_inputs(
        &self,
        request: &PureFunctionRequest,
        input_names: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<AotPureI64Plan, RuntimeEvalError> {
        AotPureI64Plan::compile_with_inputs(request, input_names)
    }

    /// Compiles a deterministic helper request to an exact-width scalar AOT plan.
    pub fn compile_scalar_with_inputs(
        &self,
        request: &PureFunctionRequest,
        input_names: impl IntoIterator<Item = impl AsRef<str>>,
        input_type: RuntimePureInputType,
        output_type: RuntimePureOutputType,
    ) -> Result<AotPureScalarPlan, RuntimeEvalError> {
        AotPureScalarPlan::compile_with_inputs(request, input_names, input_type, output_type)
    }
}

impl PureFunctionBackend for AotPureFunctionBackend {
    fn kind(&self) -> PureFunctionBackendKind {
        PureFunctionBackendKind::Aot
    }

    fn evaluate(
        &self,
        request: &PureFunctionRequest,
    ) -> Result<PureFunctionResult, RuntimeEvalError> {
        let (value, stats) = self.compile_i64(request)?.call();
        Ok(PureFunctionResult {
            backend: self.kind(),
            value: RuntimeValue::i64(value),
            stats,
        })
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

impl RuntimePureCallBackend for VmRuntimePureCallBackend {
    fn call_i8_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[i8],
    ) -> Result<Option<i8>, RuntimeEvalError> {
        self.call_exact_int_slice(helper, args)
    }

    fn call_i8_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i8],
        arity: usize,
        out: &mut [i8],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_i8_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i8],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_i16_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[i16],
    ) -> Result<Option<i16>, RuntimeEvalError> {
        self.call_exact_int_slice(helper, args)
    }

    fn call_i16_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i16],
        arity: usize,
        out: &mut [i16],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_i16_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i16],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_i32(
        &mut self,
        helper: &RuntimePureHelper,
        args: RuntimeI32Args,
    ) -> Result<Option<i32>, RuntimeEvalError> {
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_stack_packs += 1;
        self.stats.arg_bytes_copied += args.len() * std::mem::size_of::<i32>();
        let value = self.scratch.evaluate_i32_args(helper, args)?;
        runtime_value_into_i32_result(helper, value).map(Some)
    }

    fn call_i32_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[i32],
    ) -> Result<Option<i32>, RuntimeEvalError> {
        if args.len() > RuntimeI32Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeI32Args::MAX,
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        let value = self.scratch.evaluate_i32_slice(helper, args)?;
        runtime_value_into_i32_result(helper, value).map(Some)
    }

    fn call_i32_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i32],
        arity: usize,
        out: &mut [i32],
    ) -> Result<(), RuntimeEvalError> {
        if arity > RuntimeI32Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeI32Args::MAX,
                found: arity,
            });
        }
        if flat_inputs.len() != out.len().saturating_mul(arity) {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: format!(
                    "pure flat batch expected {} input value(s), got {}",
                    out.len().saturating_mul(arity),
                    flat_inputs.len()
                ),
            });
        }
        self.stats.batch_calls += 1;
        self.stats.batch_items += out.len();
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += out.len();
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += out.len();
        self.stats.vm_calls += out.len();
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.result_bytes_copied += std::mem::size_of_val(out);
        if arity == 0 {
            return out.iter_mut().try_for_each(|slot| {
                let value = self.scratch.evaluate_i32_slice(helper, &[])?;
                *slot = runtime_value_into_i32_result(helper, value)?;
                Ok(())
            });
        }
        flat_inputs
            .chunks_exact(arity)
            .zip(out.iter_mut())
            .try_for_each(|(row, slot)| {
                let value = self.scratch.evaluate_i32_slice(helper, row)?;
                *slot = runtime_value_into_i32_result(helper, value)?;
                Ok(())
            })
    }

    fn call_i32_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i32],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        if arity > RuntimeI32Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeI32Args::MAX,
                found: arity,
            });
        }
        if flat_inputs.len() != rows.saturating_mul(arity) {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: format!(
                    "pure flat batch expected {} input value(s), got {}",
                    rows.saturating_mul(arity),
                    flat_inputs.len()
                ),
            });
        }
        self.stats.batch_calls += 1;
        self.stats.batch_items += rows;
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += rows;
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += rows;
        self.stats.vm_calls += rows;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        let mut sum = 0_i64;
        if arity == 0 {
            for _ in 0..rows {
                let value = self.scratch.evaluate_i32_slice(helper, &[])?;
                sum += i64::from(runtime_value_into_i32_result(helper, value)?);
            }
            return Ok(sum);
        }
        for row in flat_inputs.chunks_exact(arity) {
            let value = self.scratch.evaluate_i32_slice(helper, row)?;
            sum += i64::from(runtime_value_into_i32_result(helper, value)?);
        }
        Ok(sum)
    }

    fn call_u32_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[u32],
    ) -> Result<Option<u32>, RuntimeEvalError> {
        self.call_exact_int_slice(helper, args)
    }

    fn call_u8_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[u8],
    ) -> Result<Option<u8>, RuntimeEvalError> {
        self.call_exact_int_slice(helper, args)
    }

    fn call_u8_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u8],
        arity: usize,
        out: &mut [u8],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_u8_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u8],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_u16_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[u16],
    ) -> Result<Option<u16>, RuntimeEvalError> {
        self.call_exact_int_slice(helper, args)
    }

    fn call_u16_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u16],
        arity: usize,
        out: &mut [u16],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_u16_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u16],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_u32_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u32],
        arity: usize,
        out: &mut [u32],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_u32_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u32],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_u64_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[u64],
    ) -> Result<Option<u64>, RuntimeEvalError> {
        self.call_exact_int_slice(helper, args)
    }

    fn call_u64_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u64],
        arity: usize,
        out: &mut [u64],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_u64_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_exact_int_slice<T: RuntimePureScalarInteger>(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[T],
    ) -> Result<Option<T>, RuntimeEvalError> {
        if args.len() > RuntimeFixedArgs::<T>::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeFixedArgs::<T>::MAX,
                found: args.len(),
            });
        }
        if helper.output_type != T::OUTPUT_TYPE
            || helper.input_types.len() != helper.input_names.len()
            || !helper
                .input_types
                .iter()
                .all(|input| *input == T::INPUT_TYPE)
        {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: "exact integer call type does not match helper signature".to_owned(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        let value = self.scratch.evaluate_exact_int_slice::<T>(helper, args)?;
        T::try_from_runtime_value(&helper.name, value).map(Some)
    }

    fn call_exact_int_flat_batch<T: RuntimePureScalarInteger>(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[T],
        arity: usize,
        out: &mut [T],
    ) -> Result<(), RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<T>(helper, flat_inputs.len(), arity, out.len())?;
        self.stats.batch_calls += 1;
        self.stats.batch_items += out.len();
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += out.len();
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += out.len();
        self.stats.vm_calls += out.len();
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.result_bytes_copied += std::mem::size_of_val(out);
        if arity == 0 {
            return out.iter_mut().try_for_each(|slot| {
                let value = self.scratch.evaluate_exact_int_slice::<T>(helper, &[])?;
                *slot = T::try_from_runtime_value(&helper.name, value)?;
                Ok(())
            });
        }
        flat_inputs
            .chunks_exact(arity)
            .zip(out.iter_mut())
            .try_for_each(|(row, slot)| {
                let value = self.scratch.evaluate_exact_int_slice::<T>(helper, row)?;
                *slot = T::try_from_runtime_value(&helper.name, value)?;
                Ok(())
            })
    }

    fn call_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[T],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<T>(helper, flat_inputs.len(), arity, rows)?;
        self.stats.batch_calls += 1;
        self.stats.batch_items += rows;
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += rows;
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += rows;
        self.stats.vm_calls += rows;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        let mut sum = 0_i64;
        if arity == 0 {
            for _ in 0..rows {
                let value = self.scratch.evaluate_exact_int_slice::<T>(helper, &[])?;
                sum +=
                    T::try_from_runtime_value(&helper.name, value)?.try_sum_as_i64(&helper.name)?;
            }
            return Ok(sum);
        }
        for row in flat_inputs.chunks_exact(arity) {
            let value = self.scratch.evaluate_exact_int_slice::<T>(helper, row)?;
            sum += T::try_from_runtime_value(&helper.name, value)?.try_sum_as_i64(&helper.name)?;
        }
        Ok(sum)
    }

    fn call_i64(
        &mut self,
        helper: &RuntimePureHelper,
        args: RuntimeI64Args,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_stack_packs += 1;
        self.stats.arg_bytes_copied += args.len() * std::mem::size_of::<i64>();
        let value = self.scratch.evaluate_i64_args(helper, args)?;
        match value {
            RuntimeValue::Int(value) => value.exact_i64().map(Some).ok_or_else(|| {
                RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(value)))
            }),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
        }
    }

    fn call_i64_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[i64],
    ) -> Result<Option<i64>, RuntimeEvalError> {
        if args.len() > RuntimeI64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeI64Args::MAX,
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        let value = self.scratch.evaluate_i64_slice(helper, args)?;
        match value {
            RuntimeValue::Int(value) => value.exact_i64().map(Some).ok_or_else(|| {
                RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(value)))
            }),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
        }
    }

    fn call_i64_batch(
        &mut self,
        helper: &RuntimePureHelper,
        rows: &[RuntimeI64Args],
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError> {
        if rows.len() != out.len() {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: format!(
                    "pure batch expected {} output slot(s), got {}",
                    rows.len(),
                    out.len()
                ),
            });
        }
        self.stats.batch_calls += 1;
        self.stats.batch_items += rows.len();
        self.stats.pure_calls += rows.len();
        self.stats.vm_calls += rows.len();
        self.stats.arg_stack_packs += rows.len();
        self.stats.arg_bytes_copied += rows
            .iter()
            .map(|row| row.len() * std::mem::size_of::<i64>())
            .sum::<usize>();
        self.stats.result_bytes_copied += std::mem::size_of_val(out);
        rows.iter().zip(out.iter_mut()).try_for_each(|(row, slot)| {
            let value = self.scratch.evaluate_i64_args(helper, *row)?;
            match value {
                RuntimeValue::Int(value) => {
                    *slot = value.exact_i64().ok_or_else(|| {
                        RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(
                            value,
                        )))
                    })?;
                    Ok(())
                }
                value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
            }
        })
    }

    fn call_i64_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i64],
        arity: usize,
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError> {
        if arity > RuntimeI64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeI64Args::MAX,
                found: arity,
            });
        }
        if flat_inputs.len() != out.len().saturating_mul(arity) {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: format!(
                    "pure flat batch expected {} input value(s), got {}",
                    out.len().saturating_mul(arity),
                    flat_inputs.len()
                ),
            });
        }
        self.stats.batch_calls += 1;
        self.stats.batch_items += out.len();
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += out.len();
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += out.len();
        self.stats.vm_calls += out.len();
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.result_bytes_copied += std::mem::size_of_val(out);
        if arity == 0 {
            return out.iter_mut().try_for_each(|slot| {
                let value = self.scratch.evaluate_i64_slice(helper, &[])?;
                match value {
                    RuntimeValue::Int(value) => {
                        *slot = value.exact_i64().ok_or_else(|| {
                            RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(
                                value,
                            )))
                        })?;
                        Ok(())
                    }
                    value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
                }
            });
        }
        flat_inputs
            .chunks_exact(arity)
            .zip(out.iter_mut())
            .try_for_each(|(row, slot)| {
                let value = self.scratch.evaluate_i64_slice(helper, row)?;
                match value {
                    RuntimeValue::Int(value) => {
                        *slot = value.exact_i64().ok_or_else(|| {
                            RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(
                                value,
                            )))
                        })?;
                        Ok(())
                    }
                    value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
                }
            })
    }

    fn call_i64_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        if arity > RuntimeI64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeI64Args::MAX,
                found: arity,
            });
        }
        if flat_inputs.len() != rows.saturating_mul(arity) {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: format!(
                    "pure flat batch expected {} input value(s), got {}",
                    rows.saturating_mul(arity),
                    flat_inputs.len()
                ),
            });
        }
        self.stats.batch_calls += 1;
        self.stats.batch_items += rows;
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += rows;
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += rows;
        self.stats.vm_calls += rows;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        let mut sum = 0i64;
        if arity == 0 {
            for _ in 0..rows {
                match self.scratch.evaluate_i64_slice(helper, &[])? {
                    RuntimeValue::Int(value) => {
                        sum += value.exact_i64().ok_or_else(|| {
                            RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(
                                value,
                            )))
                        })?;
                    }
                    value => {
                        return Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value)));
                    }
                }
            }
            return Ok(sum);
        }
        for row in flat_inputs.chunks_exact(arity) {
            match self.scratch.evaluate_i64_slice(helper, row)? {
                RuntimeValue::Int(value) => {
                    sum += value.exact_i64().ok_or_else(|| {
                        RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(
                            value,
                        )))
                    })?;
                }
                value => return Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
            }
        }
        Ok(sum)
    }

    fn call_i64_repeated_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        row: &[i64],
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        if row.len() > RuntimeI64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeI64Args::MAX,
                found: row.len(),
            });
        }
        self.stats.batch_calls += usize::from(rows > 0);
        self.stats.batch_items += rows;
        self.stats.flat_batch_calls += usize::from(rows > 0);
        self.stats.flat_batch_items += rows;
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(row);
        self.stats.pure_calls += rows;
        self.stats.vm_calls += rows;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(row);
        if rows == 0 {
            return Ok(0);
        }
        let value = match self.scratch.evaluate_i64_slice(helper, row)? {
            RuntimeValue::Int(value) => value.exact_i64().ok_or_else(|| {
                RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(value)))
            })?,
            value => return Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
        };
        let rows = i64::try_from(rows).map_err(|_| RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: "pure repeated batch row count must fit i64".to_owned(),
        })?;
        value
            .checked_mul(rows)
            .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: "pure repeated batch sum overflowed i64".to_owned(),
            })
    }

    fn call_f32_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[f32],
    ) -> Result<Option<f32>, RuntimeEvalError> {
        if args.len() > RuntimeFloat32Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeFloat32Args::MAX,
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        let value = self.scratch.evaluate_f32_slice(helper, args)?;
        runtime_value_into_f32_result(helper, value).map(Some)
    }

    fn call_f32_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[f32],
        arity: usize,
        out: &mut [f32],
    ) -> Result<(), RuntimeEvalError> {
        validate_float_flat_batch_shape(
            helper,
            RuntimePureInputType::F32,
            RuntimePureOutputType::F32,
            RuntimeFloat32Args::MAX,
            flat_inputs.len(),
            arity,
            out.len(),
        )?;
        self.stats.batch_calls += 1;
        self.stats.batch_items += out.len();
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += out.len();
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += out.len();
        self.stats.vm_calls += out.len();
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.result_bytes_copied += std::mem::size_of_val(out);
        if arity == 0 {
            return out.iter_mut().try_for_each(|slot| {
                let value = self.scratch.evaluate_f32_slice(helper, &[])?;
                *slot = runtime_value_into_f32_result(helper, value)?;
                Ok(())
            });
        }
        flat_inputs
            .chunks_exact(arity)
            .zip(out.iter_mut())
            .try_for_each(|(row, slot)| {
                let value = self.scratch.evaluate_f32_slice(helper, row)?;
                *slot = runtime_value_into_f32_result(helper, value)?;
                Ok(())
            })
    }

    fn call_f64_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[f64],
    ) -> Result<Option<f64>, RuntimeEvalError> {
        if args.len() > RuntimeFloat64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeFloat64Args::MAX,
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        let value = self.scratch.evaluate_f64_slice(helper, args)?;
        runtime_value_into_f64_result(helper, value).map(Some)
    }

    fn call_f64_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[f64],
        arity: usize,
        out: &mut [f64],
    ) -> Result<(), RuntimeEvalError> {
        validate_float_flat_batch_shape(
            helper,
            RuntimePureInputType::F64,
            RuntimePureOutputType::F64,
            RuntimeFloat64Args::MAX,
            flat_inputs.len(),
            arity,
            out.len(),
        )?;
        self.stats.batch_calls += 1;
        self.stats.batch_items += out.len();
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += out.len();
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += out.len();
        self.stats.vm_calls += out.len();
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.result_bytes_copied += std::mem::size_of_val(out);
        if arity == 0 {
            return out.iter_mut().try_for_each(|slot| {
                let value = self.scratch.evaluate_f64_slice(helper, &[])?;
                *slot = runtime_value_into_f64_result(helper, value)?;
                Ok(())
            });
        }
        flat_inputs
            .chunks_exact(arity)
            .zip(out.iter_mut())
            .try_for_each(|(row, slot)| {
                let value = self.scratch.evaluate_f64_slice(helper, row)?;
                *slot = runtime_value_into_f64_result(helper, value)?;
                Ok(())
            })
    }

    fn call_values(
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
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.fallbacks += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        self.scratch.evaluate_values(helper, args)
    }

    fn stats(&self) -> RuntimePureCallStats {
        self.stats
    }
}

impl RuntimeMathCallBackend for VmRuntimePureCallBackend {
    fn call_math_matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        lhs.matmul_scalar(rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathMatmulF32.as_label().to_owned(),
                reason: error.to_string(),
            })
    }

    fn call_math_matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        lhs.add_scalar(rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathMatrixAddF32.as_label().to_owned(),
                reason: error.to_string(),
            })
    }

    fn call_math_tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        lhs.add_scalar(rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathTensorAddF32.as_label().to_owned(),
                reason: error.to_string(),
            })
    }

    fn call_math_matmul_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeEvalError> {
        self.stats.math_calls += 1;
        lhs.matmul_scalar(rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathMatmulF64.as_label().to_owned(),
                reason: error.to_string(),
            })
    }

    fn call_math_matrix_add_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeEvalError> {
        self.stats.math_calls += 1;
        lhs.add_scalar(rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathMatrixAddF64.as_label().to_owned(),
                reason: error.to_string(),
            })
    }

    fn call_math_tensor_add_f64(
        &mut self,
        lhs: &DenseTensorF64,
        rhs: &DenseTensorF64,
    ) -> Result<DenseTensorF64, RuntimeEvalError> {
        self.stats.math_calls += 1;
        lhs.add_scalar(rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathTensorAddF64.as_label().to_owned(),
                reason: error.to_string(),
            })
    }
}

impl RuntimeExternalCallBackend for VmRuntimePureCallBackend {
    fn call_external(
        &mut self,
        _callee: &RuntimeCallTarget,
        _args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        None
    }
}

impl AotPureI64Plan {
    fn compile(request: &PureFunctionRequest) -> Result<Self, RuntimeEvalError> {
        Self::compile_with_inputs(request, std::iter::empty::<&str>())
    }

    fn compile_with_inputs(
        request: &PureFunctionRequest,
        input_names: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, RuntimeEvalError> {
        let mut ctx = AotCompileContext::from_request(request)?;
        let expr = compile_aot_i64_expr(&request.name, &request.expr, &mut ctx)?;
        let slot_count = ctx.next_slot;
        let input_slots = input_names
            .into_iter()
            .map(|name| ctx.input_slot(&request.name, name.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut initial_slots = AotCompileContext::initial_slots(request)?;
        initial_slots.resize(slot_count, 0);
        Ok(Self {
            name: request.name.clone(),
            expr,
            initial_slots,
            input_slots,
            slot_count,
        })
    }

    /// Calls the compiled helper and returns the integer value plus evaluation stats.
    pub fn call(&self) -> (i64, PureFunctionStats) {
        self.evaluate_with_slots(self.initial_slots.clone())
    }

    /// Calls the compiled helper with runtime integer inputs.
    pub fn call_with_inputs(
        &self,
        inputs: &[i64],
    ) -> Result<(i64, PureFunctionStats), RuntimeEvalError> {
        if inputs.len() != self.input_slots.len() {
            return Err(unsupported_aot(
                &self.name,
                format!(
                    "AOT helper expected {} input(s), got {}",
                    self.input_slots.len(),
                    inputs.len()
                ),
            ));
        }
        let mut slots = self.initial_slots.clone();
        for (slot, value) in self.input_slots.iter().zip(inputs.iter().copied()) {
            slots[*slot] = value;
        }
        Ok(self.evaluate_with_slots(slots))
    }

    /// Calls the compiled helper with caller-owned slot storage.
    pub fn call_with_inputs_scratch(
        &self,
        inputs: &[i64],
        slots: &mut Vec<i64>,
    ) -> Result<(i64, PureFunctionStats), RuntimeEvalError> {
        if inputs.len() != self.input_slots.len() {
            return Err(unsupported_aot(
                &self.name,
                format!(
                    "AOT helper expected {} input(s), got {}",
                    self.input_slots.len(),
                    inputs.len()
                ),
            ));
        }
        self.reset_scratch_slots(slots);
        for (slot, value) in self.input_slots.iter().zip(inputs.iter().copied()) {
            slots[*slot] = value;
        }
        Ok(self.evaluate_with_slot_slice(slots))
    }

    fn reset_scratch_slots(&self, slots: &mut Vec<i64>) {
        if slots.len() == self.slot_count {
            slots.copy_from_slice(&self.initial_slots);
        } else {
            slots.clear();
            slots.extend_from_slice(&self.initial_slots);
            if slots.len() < self.slot_count {
                slots.resize(self.slot_count, 0);
            }
        }
    }

    fn evaluate_with_slots(&self, mut slots: Vec<i64>) -> (i64, PureFunctionStats) {
        self.evaluate_with_slot_slice(&mut slots)
    }

    fn evaluate_with_slot_slice(&self, slots: &mut [i64]) -> (i64, PureFunctionStats) {
        let mut evaluator = AotI64Evaluator {
            slots,
            stats: PureFunctionStats::default(),
        };
        let value = evaluator.eval_i64(&self.expr);
        (value, evaluator.stats)
    }

    /// Helper name captured from the original request.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl AotPureScalarPlan {
    fn compile_with_inputs(
        request: &PureFunctionRequest,
        input_names: impl IntoIterator<Item = impl AsRef<str>>,
        input_type: RuntimePureInputType,
        output_type: RuntimePureOutputType,
    ) -> Result<Self, RuntimeEvalError> {
        let mut ctx = AotCompileContext::from_scalar_request(request)?;
        let expr = compile_aot_scalar_expr(&request.name, &request.expr, &mut ctx)?;
        let slot_count = ctx.next_slot;
        let input_slots = input_names
            .into_iter()
            .map(|name| ctx.input_slot(&request.name, name.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut initial_slots = AotCompileContext::initial_scalar_slots(request)?;
        initial_slots.resize(
            slot_count,
            RuntimePureScalar::default_for_output(output_type)?,
        );
        Ok(Self {
            name: request.name.clone(),
            expr,
            initial_slots,
            input_slots,
            input_type,
            output_type,
            slot_count,
        })
    }

    /// Calls the compiled helper with exact-width integer inputs.
    pub fn call_exact_int_with_inputs_scratch<T: RuntimePureScalarInteger>(
        &self,
        inputs: &[T],
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(T, PureFunctionStats), RuntimeEvalError> {
        if self.input_type != T::INPUT_TYPE || self.output_type != T::OUTPUT_TYPE {
            return Err(unsupported_aot(
                &self.name,
                "AOT scalar helper type does not match exact integer call",
            ));
        }
        let (value, stats) = self.call_with_scalar_inputs_scratch(
            inputs
                .iter()
                .copied()
                .map(RuntimePureScalarInteger::into_pure_scalar),
            inputs.len(),
            slots,
        )?;
        T::try_from_runtime_value(&self.name, value.into_runtime_value())
            .map(|value| (value, stats))
    }

    /// Calls the compiled helper with `f32` inputs.
    pub fn call_f32_with_inputs_scratch(
        &self,
        inputs: &[f32],
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(f32, PureFunctionStats), RuntimeEvalError> {
        if self.input_type != RuntimePureInputType::F32
            || self.output_type != RuntimePureOutputType::F32
        {
            return Err(unsupported_aot(
                &self.name,
                "AOT scalar helper type does not match f32 call",
            ));
        }
        let (value, stats) = self.call_with_scalar_inputs_scratch(
            inputs.iter().copied().map(RuntimePureScalar::F32),
            inputs.len(),
            slots,
        )?;
        match value {
            RuntimePureScalar::F32(value) => Ok((value, stats)),
            value => Err(unsupported_aot(
                &self.name,
                format!("AOT scalar f32 result expected f32, got {}", value.label()),
            )),
        }
    }

    /// Calls the compiled helper with `f64` inputs.
    pub fn call_f64_with_inputs_scratch(
        &self,
        inputs: &[f64],
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(f64, PureFunctionStats), RuntimeEvalError> {
        if self.input_type != RuntimePureInputType::F64
            || self.output_type != RuntimePureOutputType::F64
        {
            return Err(unsupported_aot(
                &self.name,
                "AOT scalar helper type does not match f64 call",
            ));
        }
        let (value, stats) = self.call_with_scalar_inputs_scratch(
            inputs.iter().copied().map(RuntimePureScalar::F64),
            inputs.len(),
            slots,
        )?;
        match value {
            RuntimePureScalar::F64(value) => Ok((value, stats)),
            value => Err(unsupported_aot(
                &self.name,
                format!("AOT scalar f64 result expected f64, got {}", value.label()),
            )),
        }
    }

    fn call_with_scalar_inputs_scratch(
        &self,
        inputs: impl IntoIterator<Item = RuntimePureScalar>,
        input_len: usize,
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(RuntimePureScalar, PureFunctionStats), RuntimeEvalError> {
        if input_len != self.input_slots.len() {
            return Err(unsupported_aot(
                &self.name,
                format!(
                    "AOT helper expected {} input(s), got {}",
                    self.input_slots.len(),
                    input_len
                ),
            ));
        }
        self.reset_scratch_slots(slots)?;
        for (slot, value) in self.input_slots.iter().zip(inputs) {
            slots[*slot] = value;
        }
        self.evaluate_with_slot_slice(slots)
    }

    fn reset_scratch_slots(
        &self,
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(), RuntimeEvalError> {
        if slots.len() == self.slot_count {
            slots.copy_from_slice(&self.initial_slots);
        } else {
            slots.clear();
            slots.extend_from_slice(&self.initial_slots);
            if slots.len() < self.slot_count {
                slots.resize(
                    self.slot_count,
                    RuntimePureScalar::default_for_output(self.output_type)?,
                );
            }
        }
        Ok(())
    }

    fn evaluate_with_slot_slice(
        &self,
        slots: &mut [RuntimePureScalar],
    ) -> Result<(RuntimePureScalar, PureFunctionStats), RuntimeEvalError> {
        let mut evaluator = AotScalarEvaluator {
            name: &self.name,
            slots,
            stats: PureFunctionStats::default(),
        };
        let value = evaluator.eval_scalar(&self.expr)?;
        Ok((value, evaluator.stats))
    }

    /// Helper name captured from the original request.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl AotCompileContext {
    fn from_request(request: &PureFunctionRequest) -> Result<Self, RuntimeEvalError> {
        let mut slots = BTreeMap::new();
        for binding in &request.bindings {
            if !matches!(
                binding.value,
                RuntimeValue::Int(crate::value::RuntimeInt::I64(_))
            ) {
                return Err(unsupported_aot(
                    &request.name,
                    format!("binding `{}` is not an i64 integer", binding.name),
                ));
            }
            if slots.insert(binding.name.clone(), slots.len()).is_some() {
                return Err(unsupported_aot(
                    &request.name,
                    format!("binding `{}` is duplicated", binding.name),
                ));
            }
        }
        let next_slot = slots.len();
        Ok(Self { slots, next_slot })
    }

    fn initial_slots(request: &PureFunctionRequest) -> Result<Vec<i64>, RuntimeEvalError> {
        request
            .bindings
            .iter()
            .map(|binding| match binding.value {
                RuntimeValue::Int(value) => value.exact_i64().ok_or_else(|| {
                    unsupported_aot(
                        &request.name,
                        format!("binding `{}` is not an i64 integer", binding.name),
                    )
                }),
                _ => Err(unsupported_aot(
                    &request.name,
                    format!("binding `{}` is not an i64 integer", binding.name),
                )),
            })
            .collect()
    }

    fn from_scalar_request(request: &PureFunctionRequest) -> Result<Self, RuntimeEvalError> {
        let mut slots = BTreeMap::new();
        for binding in &request.bindings {
            if runtime_value_as_scalar(&binding.value).is_none() {
                return Err(unsupported_aot(
                    &request.name,
                    format!("binding `{}` is not a scalar value", binding.name),
                ));
            }
            if slots.insert(binding.name.clone(), slots.len()).is_some() {
                return Err(unsupported_aot(
                    &request.name,
                    format!("binding `{}` is duplicated", binding.name),
                ));
            }
        }
        let next_slot = slots.len();
        Ok(Self { slots, next_slot })
    }

    fn initial_scalar_slots(
        request: &PureFunctionRequest,
    ) -> Result<Vec<RuntimePureScalar>, RuntimeEvalError> {
        request
            .bindings
            .iter()
            .map(|binding| {
                runtime_value_as_scalar(&binding.value).ok_or_else(|| {
                    unsupported_aot(
                        &request.name,
                        format!("binding `{}` is not a scalar value", binding.name),
                    )
                })
            })
            .collect()
    }

    fn local_slot(&self, name: &str) -> Option<usize> {
        self.slots.get(name).copied()
    }

    fn input_slot(&self, helper_name: &str, name: &str) -> Result<usize, RuntimeEvalError> {
        if name.is_empty() {
            return Err(unsupported_aot(
                helper_name,
                "AOT runtime input names must be non-empty",
            ));
        }
        self.local_slot(name).ok_or_else(|| {
            unsupported_aot(
                helper_name,
                format!("AOT runtime input `{name}` is not a helper binding"),
            )
        })
    }

    fn with_let_binding<T>(
        &mut self,
        name: &str,
        compile_body: impl FnOnce(&mut Self, usize) -> Result<T, RuntimeEvalError>,
    ) -> Result<T, RuntimeEvalError> {
        let slot = self.next_slot;
        self.next_slot += 1;
        let previous = self.slots.insert(name.to_owned(), slot);
        let body = compile_body(self, slot);
        if let Some(previous) = previous {
            self.slots.insert(name.to_owned(), previous);
        } else {
            self.slots.remove(name);
        }
        body
    }
}

struct AotI64Evaluator<'a> {
    slots: &'a mut [i64],
    stats: PureFunctionStats,
}

impl AotI64Evaluator<'_> {
    fn eval_i64(&mut self, expr: &AotI64Expr) -> i64 {
        self.stats.evaluated_exprs += 1;
        match expr {
            AotI64Expr::Const(value) => *value,
            AotI64Expr::Local(slot) => self.slots[*slot],
            AotI64Expr::Let { slot, expr, body } => {
                let value = self.eval_i64(expr);
                let previous = self.slots[*slot];
                self.slots[*slot] = value;
                let result = self.eval_i64(body);
                self.slots[*slot] = previous;
                result
            }
            AotI64Expr::AddCall { lhs, rhs } => {
                self.stats.evaluated_calls += 1;
                self.eval_i64(lhs).wrapping_add(self.eval_i64(rhs))
            }
            AotI64Expr::Unary { op, expr } => match op {
                RuntimeUnaryOp::Neg => self.eval_i64(expr).wrapping_neg(),
                RuntimeUnaryOp::Not => unreachable!("bool unary is not compiled as i64"),
            },
            AotI64Expr::Binary { lhs, op, rhs } => {
                self.stats.evaluated_binary_ops += 1;
                let lhs = self.eval_i64(lhs);
                let rhs = self.eval_i64(rhs);
                match op {
                    RuntimeBinaryOp::Add => lhs.wrapping_add(rhs),
                    RuntimeBinaryOp::Sub => lhs.wrapping_sub(rhs),
                    RuntimeBinaryOp::Mul => lhs.wrapping_mul(rhs),
                    RuntimeBinaryOp::Div => lhs.wrapping_div(rhs),
                    RuntimeBinaryOp::Eq
                    | RuntimeBinaryOp::Ne
                    | RuntimeBinaryOp::Lt
                    | RuntimeBinaryOp::Le
                    | RuntimeBinaryOp::Gt
                    | RuntimeBinaryOp::Ge
                    | RuntimeBinaryOp::And
                    | RuntimeBinaryOp::Or => unreachable!("non-i64 binary op in AOT i64 expr"),
                }
            }
            AotI64Expr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                if self.eval_bool(condition) {
                    self.eval_i64(then_expr)
                } else {
                    self.eval_i64(else_expr)
                }
            }
        }
    }

    fn eval_bool(&mut self, expr: &AotBoolExpr) -> bool {
        self.stats.evaluated_exprs += 1;
        match expr {
            AotBoolExpr::Const(value) => *value,
            AotBoolExpr::Compare { lhs, op, rhs } => {
                self.stats.evaluated_binary_ops += 1;
                let lhs = self.eval_i64(lhs);
                let rhs = self.eval_i64(rhs);
                match op {
                    RuntimeBinaryOp::Eq => lhs == rhs,
                    RuntimeBinaryOp::Ne => lhs != rhs,
                    RuntimeBinaryOp::Lt => lhs < rhs,
                    RuntimeBinaryOp::Le => lhs <= rhs,
                    RuntimeBinaryOp::Gt => lhs > rhs,
                    RuntimeBinaryOp::Ge => lhs >= rhs,
                    RuntimeBinaryOp::Add
                    | RuntimeBinaryOp::Sub
                    | RuntimeBinaryOp::Mul
                    | RuntimeBinaryOp::Div
                    | RuntimeBinaryOp::And
                    | RuntimeBinaryOp::Or => unreachable!("non-comparison op in AOT bool expr"),
                }
            }
        }
    }
}

struct AotScalarEvaluator<'a> {
    name: &'a str,
    slots: &'a mut [RuntimePureScalar],
    stats: PureFunctionStats,
}

impl AotScalarEvaluator<'_> {
    fn eval_scalar(&mut self, expr: &AotScalarExpr) -> Result<RuntimePureScalar, RuntimeEvalError> {
        self.stats.evaluated_exprs += 1;
        match expr {
            AotScalarExpr::Const(value) => Ok(*value),
            AotScalarExpr::Local(slot) => Ok(self.slots[*slot]),
            AotScalarExpr::Let { slot, expr, body } => {
                let value = self.eval_scalar(expr)?;
                let previous = self.slots[*slot];
                self.slots[*slot] = value;
                let result = self.eval_scalar(body);
                self.slots[*slot] = previous;
                result
            }
            AotScalarExpr::AddCall { lhs, rhs } => {
                self.stats.evaluated_calls += 1;
                evaluate_scalar_binary(
                    self.eval_scalar(lhs)?,
                    RuntimeBinaryOp::Add,
                    self.eval_scalar(rhs)?,
                )
            }
            AotScalarExpr::Unary { op, expr } => {
                evaluate_scalar_unary(*op, self.eval_scalar(expr)?)
            }
            AotScalarExpr::Binary { lhs, op, rhs } => {
                self.stats.evaluated_binary_ops += 1;
                evaluate_scalar_binary(self.eval_scalar(lhs)?, *op, self.eval_scalar(rhs)?)
            }
            AotScalarExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                if self.eval_bool(condition)? {
                    self.eval_scalar(then_expr)
                } else {
                    self.eval_scalar(else_expr)
                }
            }
        }
        .map_err(|error| match error {
            RuntimeEvalError::UnsupportedPure { .. } => error,
            other => unsupported_aot(self.name, other.to_string()),
        })
    }

    fn eval_bool(&mut self, expr: &AotScalarBoolExpr) -> Result<bool, RuntimeEvalError> {
        self.stats.evaluated_exprs += 1;
        match expr {
            AotScalarBoolExpr::Const(value) => Ok(*value),
            AotScalarBoolExpr::Compare { lhs, op, rhs } => {
                self.stats.evaluated_binary_ops += 1;
                match evaluate_scalar_binary(self.eval_scalar(lhs)?, *op, self.eval_scalar(rhs)?)? {
                    RuntimePureScalar::Bool(value) => Ok(value),
                    value => Err(unsupported_aot(
                        self.name,
                        format!("condition expected bool, got {}", value.label()),
                    )),
                }
            }
        }
    }
}

fn compile_aot_i64_expr(
    helper_name: &str,
    expr: &RuntimeExpr,
    ctx: &mut AotCompileContext,
) -> Result<AotI64Expr, RuntimeEvalError> {
    match expr {
        RuntimeExpr::Value(RuntimeValue::Int(value)) => {
            value.exact_i64().map(AotI64Expr::Const).ok_or_else(|| {
                unsupported_aot(
                    helper_name,
                    format!("literal `{value}` is not an i64 integer"),
                )
            })
        }
        RuntimeExpr::Value(value) => Err(unsupported_aot(
            helper_name,
            format!("literal {value:?} is not an i64 integer"),
        )),
        RuntimeExpr::Local(name) => ctx
            .local_slot(name)
            .map(AotI64Expr::Local)
            .ok_or_else(|| RuntimeEvalError::UnknownBinding(name.clone())),
        RuntimeExpr::Let { name, expr, body } => {
            let expr = compile_aot_i64_expr(helper_name, expr, ctx)?;
            ctx.with_let_binding(name, |ctx, slot| {
                Ok(AotI64Expr::Let {
                    slot,
                    expr: Box::new(expr),
                    body: Box::new(compile_aot_i64_expr(helper_name, body, ctx)?),
                })
            })
        }
        RuntimeExpr::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::Add) && args.len() == 2 =>
        {
            Ok(AotI64Expr::AddCall {
                lhs: Box::new(compile_aot_i64_expr(helper_name, &args[0], ctx)?),
                rhs: Box::new(compile_aot_i64_expr(helper_name, &args[1], ctx)?),
            })
        }
        RuntimeExpr::SpreadArg(_) => Err(unsupported_aot(
            helper_name,
            "spread arguments are expanded by the VM call boundary",
        )),
        RuntimeExpr::Unary { op, expr } => match op {
            RuntimeUnaryOp::Neg => Ok(AotI64Expr::Unary {
                op: *op,
                expr: Box::new(compile_aot_i64_expr(helper_name, expr, ctx)?),
            }),
            RuntimeUnaryOp::Not => Err(unsupported_aot(
                helper_name,
                "boolean negation is not an i64 result",
            )),
        },
        RuntimeExpr::Binary { lhs, op, rhs } if is_aot_i64_binary(*op) => Ok(AotI64Expr::Binary {
            lhs: Box::new(compile_aot_i64_expr(helper_name, lhs, ctx)?),
            op: *op,
            rhs: Box::new(compile_aot_i64_expr(helper_name, rhs, ctx)?),
        }),
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => Ok(AotI64Expr::If {
            condition: compile_aot_bool_expr(helper_name, condition, ctx)?,
            then_expr: Box::new(compile_aot_i64_expr(helper_name, then_expr, ctx)?),
            else_expr: Box::new(compile_aot_i64_expr(helper_name, else_expr, ctx)?),
        }),
        RuntimeExpr::Call { callee, .. } => Err(unsupported_aot(
            helper_name,
            format!("call `{callee}` is outside the AOT i64 subset"),
        )),
        RuntimeExpr::PureCall { .. } => Err(unsupported_aot(
            helper_name,
            "nested runtime pure calls are outside the AOT i64 subset",
        )),
        other => Err(unsupported_aot(
            helper_name,
            format!("expression `{other}` is outside the AOT i64 subset"),
        )),
    }
}

fn compile_aot_bool_expr(
    helper_name: &str,
    expr: &RuntimeExpr,
    ctx: &mut AotCompileContext,
) -> Result<AotBoolExpr, RuntimeEvalError> {
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => Ok(AotBoolExpr::Const(*value)),
        RuntimeExpr::Binary { lhs, op, rhs } if is_aot_comparison(*op) => {
            Ok(AotBoolExpr::Compare {
                lhs: Box::new(compile_aot_i64_expr(helper_name, lhs, ctx)?),
                op: *op,
                rhs: Box::new(compile_aot_i64_expr(helper_name, rhs, ctx)?),
            })
        }
        other => Err(unsupported_aot(
            helper_name,
            format!("condition `{other}` is outside the AOT i64 subset"),
        )),
    }
}

fn compile_aot_scalar_expr(
    helper_name: &str,
    expr: &RuntimeExpr,
    ctx: &mut AotCompileContext,
) -> Result<AotScalarExpr, RuntimeEvalError> {
    match expr {
        RuntimeExpr::Value(value) => runtime_value_as_scalar(value)
            .map(AotScalarExpr::Const)
            .ok_or_else(|| {
                unsupported_aot(
                    helper_name,
                    format!("literal {value:?} is not a scalar value"),
                )
            }),
        RuntimeExpr::Local(name) => ctx
            .local_slot(name)
            .map(AotScalarExpr::Local)
            .ok_or_else(|| RuntimeEvalError::UnknownBinding(name.clone())),
        RuntimeExpr::Let { name, expr, body } => {
            let expr = compile_aot_scalar_expr(helper_name, expr, ctx)?;
            ctx.with_let_binding(name, |ctx, slot| {
                Ok(AotScalarExpr::Let {
                    slot,
                    expr: Box::new(expr),
                    body: Box::new(compile_aot_scalar_expr(helper_name, body, ctx)?),
                })
            })
        }
        RuntimeExpr::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::Add) && args.len() == 2 =>
        {
            Ok(AotScalarExpr::AddCall {
                lhs: Box::new(compile_aot_scalar_expr(helper_name, &args[0], ctx)?),
                rhs: Box::new(compile_aot_scalar_expr(helper_name, &args[1], ctx)?),
            })
        }
        RuntimeExpr::SpreadArg(_) => Err(unsupported_aot(
            helper_name,
            "spread arguments are expanded by the VM call boundary",
        )),
        RuntimeExpr::Unary { op, expr } => match op {
            RuntimeUnaryOp::Neg => Ok(AotScalarExpr::Unary {
                op: *op,
                expr: Box::new(compile_aot_scalar_expr(helper_name, expr, ctx)?),
            }),
            RuntimeUnaryOp::Not => Err(unsupported_aot(
                helper_name,
                "boolean negation is not a scalar result",
            )),
        },
        RuntimeExpr::Binary { lhs, op, rhs } if is_aot_scalar_binary(*op) => {
            Ok(AotScalarExpr::Binary {
                lhs: Box::new(compile_aot_scalar_expr(helper_name, lhs, ctx)?),
                op: *op,
                rhs: Box::new(compile_aot_scalar_expr(helper_name, rhs, ctx)?),
            })
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => Ok(AotScalarExpr::If {
            condition: compile_aot_scalar_bool_expr(helper_name, condition, ctx)?,
            then_expr: Box::new(compile_aot_scalar_expr(helper_name, then_expr, ctx)?),
            else_expr: Box::new(compile_aot_scalar_expr(helper_name, else_expr, ctx)?),
        }),
        RuntimeExpr::Call { callee, .. } => Err(unsupported_aot(
            helper_name,
            format!("call `{callee}` is outside the AOT scalar subset"),
        )),
        RuntimeExpr::PureCall { .. } => Err(unsupported_aot(
            helper_name,
            "nested runtime pure calls are outside the AOT scalar subset",
        )),
        other => Err(unsupported_aot(
            helper_name,
            format!("expression `{other}` is outside the AOT scalar subset"),
        )),
    }
}

fn compile_aot_scalar_bool_expr(
    helper_name: &str,
    expr: &RuntimeExpr,
    ctx: &mut AotCompileContext,
) -> Result<AotScalarBoolExpr, RuntimeEvalError> {
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => Ok(AotScalarBoolExpr::Const(*value)),
        RuntimeExpr::Binary { lhs, op, rhs } if is_aot_comparison(*op) => {
            Ok(AotScalarBoolExpr::Compare {
                lhs: Box::new(compile_aot_scalar_expr(helper_name, lhs, ctx)?),
                op: *op,
                rhs: Box::new(compile_aot_scalar_expr(helper_name, rhs, ctx)?),
            })
        }
        other => Err(unsupported_aot(
            helper_name,
            format!("condition `{other}` is outside the AOT scalar subset"),
        )),
    }
}

fn is_aot_i64_binary(op: RuntimeBinaryOp) -> bool {
    matches!(
        op,
        RuntimeBinaryOp::Add | RuntimeBinaryOp::Sub | RuntimeBinaryOp::Mul | RuntimeBinaryOp::Div
    )
}

fn is_aot_scalar_binary(op: RuntimeBinaryOp) -> bool {
    matches!(
        op,
        RuntimeBinaryOp::Add | RuntimeBinaryOp::Sub | RuntimeBinaryOp::Mul | RuntimeBinaryOp::Div
    )
}

fn is_aot_comparison(op: RuntimeBinaryOp) -> bool {
    matches!(
        op,
        RuntimeBinaryOp::Eq
            | RuntimeBinaryOp::Ne
            | RuntimeBinaryOp::Lt
            | RuntimeBinaryOp::Le
            | RuntimeBinaryOp::Gt
            | RuntimeBinaryOp::Ge
    )
}

fn unsupported_aot(name: &str, reason: impl Into<String>) -> RuntimeEvalError {
    RuntimeEvalError::UnsupportedPure {
        name: name.to_owned(),
        reason: reason.into(),
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
            RuntimePureOutputType::Value => Err(unsupported_aot(
                "pure",
                "AOT scalar slots require a concrete scalar output type",
            )),
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
        runtime_value_label(&self.into_runtime_value())
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

fn runtime_value_into_i32_result(
    helper: &RuntimePureHelper,
    value: RuntimeValue,
) -> Result<i32, RuntimeEvalError> {
    i32::try_from_runtime_value(&helper.name, value)
}

fn runtime_value_into_f32_result(
    helper: &RuntimePureHelper,
    value: RuntimeValue,
) -> Result<f32, RuntimeEvalError> {
    match value {
        RuntimeValue::F32(value) => Ok(value),
        value => Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure f32 result expected f32, got {}",
                runtime_value_label(&value)
            ),
        }),
    }
}

fn runtime_value_into_f64_result(
    helper: &RuntimePureHelper,
    value: RuntimeValue,
) -> Result<f64, RuntimeEvalError> {
    match value {
        RuntimeValue::F64(value) => Ok(value),
        value => Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure f64 result expected f64, got {}",
                runtime_value_label(&value)
            ),
        }),
    }
}

fn validate_exact_int_flat_batch_shape<T: RuntimePureScalarInteger>(
    helper: &RuntimePureHelper,
    flat_input_len: usize,
    arity: usize,
    rows: usize,
) -> Result<(), RuntimeEvalError> {
    if arity > RuntimeFixedArgs::<T>::MAX {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: helper.name.clone(),
            max: RuntimeFixedArgs::<T>::MAX,
            found: arity,
        });
    }
    if helper.output_type != T::OUTPUT_TYPE
        || helper.input_types.len() != helper.input_names.len()
        || !helper
            .input_types
            .iter()
            .all(|input| *input == T::INPUT_TYPE)
    {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: "exact integer batch type does not match helper signature".to_owned(),
        });
    }
    if flat_input_len != rows.saturating_mul(arity) {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure flat batch expected {} input value(s), got {}",
                rows.saturating_mul(arity),
                flat_input_len
            ),
        });
    }
    Ok(())
}

fn validate_float_flat_batch_shape(
    helper: &RuntimePureHelper,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    max_arity: usize,
    flat_input_len: usize,
    arity: usize,
    rows: usize,
) -> Result<(), RuntimeEvalError> {
    if arity > max_arity {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: helper.name.clone(),
            max: max_arity,
            found: arity,
        });
    }
    if helper.output_type != output_type
        || helper.input_types.len() != helper.input_names.len()
        || !helper.input_types.iter().all(|input| *input == input_type)
    {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: "float batch type does not match helper signature".to_owned(),
        });
    }
    if flat_input_len != rows.saturating_mul(arity) {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure flat batch expected {} input value(s), got {}",
                rows.saturating_mul(arity),
                flat_input_len
            ),
        });
    }
    Ok(())
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
            op: runtime_unary_op_label(op),
            value: value.label(),
        }),
        (op, value) => Err(RuntimeEvalError::UnsupportedUnary {
            op: runtime_unary_op_label(op),
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
            (lhs, rhs) => unsupported_scalar_binary(op, lhs, rhs),
        },
        RuntimeBinaryOp::Or => match (lhs, rhs) {
            (RuntimePureScalar::Bool(lhs), RuntimePureScalar::Bool(rhs)) => {
                Ok(RuntimePureScalar::Bool(lhs || rhs))
            }
            (lhs, rhs) => unsupported_scalar_binary(op, lhs, rhs),
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
        (lhs, rhs) => unsupported_scalar_binary(op, lhs, rhs),
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
        (lhs, rhs) => unsupported_scalar_binary(op, lhs, rhs),
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

fn unsupported_scalar_binary(
    op: RuntimeBinaryOp,
    lhs: RuntimePureScalar,
    rhs: RuntimePureScalar,
) -> Result<RuntimePureScalar, RuntimeEvalError> {
    Err(RuntimeEvalError::UnsupportedBinary {
        op: runtime_binary_op_label(op),
        lhs: lhs.label(),
        rhs: rhs.label(),
    })
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
            RuntimeExpr::Record(fields) => fields
                .iter()
                .map(|field| {
                    Ok(RuntimeFieldValue {
                        name: field.name.clone(),
                        value: self.evaluate_expr(&field.value)?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::Record),
            RuntimeExpr::Variant {
                path,
                name,
                payload,
            } => Ok(RuntimeValue::Variant {
                path: path.clone(),
                name: name.clone(),
                payload: payload
                    .as_ref()
                    .map(|expr| self.evaluate_expr(expr).map(Box::new))
                    .transpose()?,
            }),
            RuntimeExpr::Field { target, field } => self.evaluate_field_expr(target, field),
            RuntimeExpr::ProjectTuple { target, ordinal } => {
                self.evaluate_project_tuple_expr(target, *ordinal)
            }
            RuntimeExpr::ProjectRecord { target, ordinal } => {
                self.evaluate_project_record_expr(target, *ordinal)
            }
            RuntimeExpr::Call { callee, args } => self.evaluate_call_expr(callee, args),
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
            } => {
                if self.evaluate_bool(condition)? {
                    self.evaluate_expr(then_expr)
                } else {
                    self.evaluate_expr(else_expr)
                }
            }
            RuntimeExpr::IfLet { .. } | RuntimeExpr::Match { .. } => {
                Err(RuntimeEvalError::UnsupportedPure {
                    name: "control".to_owned(),
                    reason: "pattern control is not in the pure helper subset".to_owned(),
                })
            }
        }
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
        let items = match runtime_value_into_sequence_values(self.evaluate_expr(source)?) {
            Ok(items) => items,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
        items
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
        let items = match runtime_value_into_sequence_values(value) {
            Ok(items) => items,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
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
            value => Err(RuntimeEvalError::MissingField {
                field: field.to_owned(),
                value: runtime_value_label(&value),
            }),
        }
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
            (RuntimeValue::Seq(seq), "len", []) => Ok(runtime_len_value(seq.len())),
            (RuntimeValue::Tuple(items), "len", []) => Ok(runtime_len_value(items.len())),
            (receiver, _, _) => Err(RuntimeEvalError::UnsupportedPure {
                name: method.to_owned(),
                reason: format!(
                    "method is not registered for {}",
                    runtime_value_label(&receiver)
                ),
            }),
        }
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

fn runtime_len_value(len: usize) -> RuntimeValue {
    RuntimeValue::usize(u64::try_from(len).unwrap_or(u64::MAX))
}

fn spread_runtime_values(value: RuntimeValue) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
    match runtime_value_into_sequence_values(value) {
        Ok(items) => Ok(items),
        Err(value) => Err(RuntimeEvalError::InvalidSpread(runtime_value_label(&value))),
    }
}
