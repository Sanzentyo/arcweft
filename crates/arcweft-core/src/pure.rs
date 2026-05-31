use crate::plan::RuntimePureHelper;
use crate::step::RuntimePureCallStats;
use crate::value::{
    RuntimeBinaryOp, RuntimeBinding, RuntimeEnv, RuntimeEvalError, RuntimeExactInteger,
    RuntimeExpr, RuntimeF32, RuntimeF64, RuntimeFieldValue, RuntimeSeq, RuntimeUnaryOp,
    RuntimeValue, evaluate_binary, evaluate_unary, runtime_binary_op_label,
    runtime_sequence_values, runtime_unary_op_label, runtime_value_into_sequence_values,
    runtime_value_label, sum_i64_sequence_ref,
};
use std::collections::BTreeMap;

/// Request for evaluating a deterministic pure helper expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PureFunctionRequest {
    pub name: String,
    pub expr: RuntimeExpr,
    pub bindings: Vec<RuntimeBinding>,
}

/// Result of one pure helper backend evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PureFunctionResult {
    pub backend: PureFunctionBackendKind,
    pub value: RuntimeValue,
    pub stats: PureFunctionStats,
}

/// Backend family used for pure helper evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PureFunctionBackendKind {
    Vm,
    Aot,
    Jit,
}

/// Deterministic counters for pure helper evaluation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PureFunctionStats {
    pub evaluated_exprs: usize,
    pub evaluated_calls: usize,
    pub evaluated_method_calls: usize,
    pub evaluated_binary_ops: usize,
}

/// Fixed-size scalar argument pack for runtime pure helper fast paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeFixedArgs<T> {
    len: usize,
    values: [T; 4],
}

pub type RuntimeI32Args = RuntimeFixedArgs<i32>;
pub type RuntimeI64Args = RuntimeFixedArgs<i64>;
pub type RuntimeF32Args = RuntimeFixedArgs<RuntimeF32>;
pub type RuntimeF64Args = RuntimeFixedArgs<RuntimeF64>;

/// Exact integer scalar that preserves the helper ABI width during VM pure evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    F32(RuntimeF32),
    F64(RuntimeF64),
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

/// Runtime-facing backend for deterministic pure helper calls.
pub trait RuntimePureCallBackend {
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

    fn call_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[T],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

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
        args: &[RuntimeF32],
    ) -> Result<Option<RuntimeF32>, RuntimeEvalError>;

    fn call_f64_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[RuntimeF64],
    ) -> Result<Option<RuntimeF64>, RuntimeEvalError>;

    fn call_values(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError>;

    fn stats(&self) -> RuntimePureCallStats;
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VmPureFunctionBackend;

/// Reusable VM fallback storage for repeated `i64` pure-helper evaluation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VmPureFunctionScratch {
    env: RuntimeEnv,
}

/// AOT backend for deterministic pure helpers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AotPureFunctionBackend;

/// VM runtime backend used when no external pure accelerator is provided.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VmRuntimePureCallBackend {
    stats: RuntimePureCallStats,
    scratch: VmPureFunctionScratch,
}

/// Compiled AOT plan for the current deterministic `i64` pure-helper subset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AotPureI64Plan {
    name: String,
    expr: AotI64Expr,
    initial_slots: Vec<i64>,
    input_slots: Vec<usize>,
    slot_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum AotBoolExpr {
    Const(bool),
    Compare {
        lhs: Box<AotI64Expr>,
        op: RuntimeBinaryOp,
        rhs: Box<AotI64Expr>,
    },
}

#[derive(Clone, Debug)]
struct AotCompileContext {
    slots: BTreeMap<String, usize>,
    next_slot: usize,
}

/// VM/JIT conformance result for deterministic helper execution.
#[derive(Clone, Debug, Eq, PartialEq)]
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
        args: &[RuntimeF32],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut scratch = VmPureFunctionScratch::default();
        scratch.evaluate_f32_slice(helper, args)
    }

    pub fn evaluate_f64_slice(
        &self,
        helper: &RuntimePureHelper,
        args: &[RuntimeF64],
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
        args: &[RuntimeF32],
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
        args: &[RuntimeF64],
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
            value: RuntimeValue::Int(value),
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
            RuntimeValue::Int(value) => Ok(Some(value)),
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
            RuntimeValue::Int(value) => Ok(Some(value)),
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
                    *slot = value;
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
                        *slot = value;
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
                        *slot = value;
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
                    RuntimeValue::Int(value) => sum += value,
                    value => {
                        return Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value)));
                    }
                }
            }
            return Ok(sum);
        }
        for row in flat_inputs.chunks_exact(arity) {
            match self.scratch.evaluate_i64_slice(helper, row)? {
                RuntimeValue::Int(value) => sum += value,
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
            RuntimeValue::Int(value) => value,
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
        args: &[RuntimeF32],
    ) -> Result<Option<RuntimeF32>, RuntimeEvalError> {
        if args.len() > RuntimeF32Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeF32Args::MAX,
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        let value = self.scratch.evaluate_f32_slice(helper, args)?;
        runtime_value_into_f32_result(helper, value).map(Some)
    }

    fn call_f64_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[RuntimeF64],
    ) -> Result<Option<RuntimeF64>, RuntimeEvalError> {
        if args.len() > RuntimeF64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeF64Args::MAX,
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        let value = self.scratch.evaluate_f64_slice(helper, args)?;
        runtime_value_into_f64_result(helper, value).map(Some)
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

impl AotCompileContext {
    fn from_request(request: &PureFunctionRequest) -> Result<Self, RuntimeEvalError> {
        let mut slots = BTreeMap::new();
        for binding in &request.bindings {
            if !matches!(binding.value, RuntimeValue::Int(_)) {
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
                RuntimeValue::Int(value) => Ok(value),
                _ => Err(unsupported_aot(
                    &request.name,
                    format!("binding `{}` is not an i64 integer", binding.name),
                )),
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

    fn with_let_binding(
        &mut self,
        name: &str,
        compile_body: impl FnOnce(&mut Self, usize) -> Result<AotI64Expr, RuntimeEvalError>,
    ) -> Result<AotI64Expr, RuntimeEvalError> {
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
                self.eval_i64(lhs).saturating_add(self.eval_i64(rhs))
            }
            AotI64Expr::Unary { op, expr } => match op {
                RuntimeUnaryOp::Neg => -self.eval_i64(expr),
                RuntimeUnaryOp::Not => unreachable!("bool unary is not compiled as i64"),
            },
            AotI64Expr::Binary { lhs, op, rhs } => {
                self.stats.evaluated_binary_ops += 1;
                let lhs = self.eval_i64(lhs);
                let rhs = self.eval_i64(rhs);
                match op {
                    RuntimeBinaryOp::Add => lhs + rhs,
                    RuntimeBinaryOp::Sub => lhs - rhs,
                    RuntimeBinaryOp::Mul => lhs * rhs,
                    RuntimeBinaryOp::Div => lhs / rhs,
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

fn compile_aot_i64_expr(
    helper_name: &str,
    expr: &RuntimeExpr,
    ctx: &mut AotCompileContext,
) -> Result<AotI64Expr, RuntimeEvalError> {
    match expr {
        RuntimeExpr::Value(RuntimeValue::Int(value)) => Ok(AotI64Expr::Const(*value)),
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
        RuntimeExpr::Call { callee, args } if callee == "add" && args.len() == 2 => {
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
            format!("expression `{other:?}` is outside the AOT i64 subset"),
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
            format!("condition `{other:?}` is outside the AOT i64 subset"),
        )),
    }
}

fn is_aot_i64_binary(op: RuntimeBinaryOp) -> bool {
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
    const fn into_runtime_value(self) -> RuntimeValue {
        match self {
            Self::Bool(value) => RuntimeValue::Bool(value),
            Self::I8(value) => RuntimeValue::Int(value as i64),
            Self::I16(value) => RuntimeValue::Int(value as i64),
            Self::I32(value) => RuntimeValue::Int(value as i64),
            Self::I64(value) => RuntimeValue::Int(value),
            Self::I128(value) => RuntimeValue::I128(value),
            Self::ISize(value) => RuntimeValue::ISize(value),
            Self::U8(value) => RuntimeValue::UInt(value as u64),
            Self::U16(value) => RuntimeValue::UInt(value as u64),
            Self::U32(value) => RuntimeValue::UInt(value as u64),
            Self::U64(value) => RuntimeValue::UInt(value),
            Self::U128(value) => RuntimeValue::U128(value),
            Self::USize(value) => RuntimeValue::USize(value),
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
        RuntimeValue::Int(value) => Some(RuntimePureScalar::I64(*value)),
        RuntimeValue::I128(value) => Some(RuntimePureScalar::I128(*value)),
        RuntimeValue::ISize(value) => Some(RuntimePureScalar::ISize(*value)),
        RuntimeValue::UInt(value) => Some(RuntimePureScalar::U64(*value)),
        RuntimeValue::U128(value) => Some(RuntimePureScalar::U128(*value)),
        RuntimeValue::USize(value) => Some(RuntimePureScalar::USize(*value)),
        RuntimeValue::F32(value) => Some(RuntimePureScalar::F32(*value)),
        RuntimeValue::F64(value) => Some(RuntimePureScalar::F64(*value)),
        _ => None,
    }
}

fn runtime_value_into_scalar(value: RuntimeValue) -> Result<RuntimePureScalar, RuntimeEvalError> {
    match value {
        RuntimeValue::Bool(value) => Ok(RuntimePureScalar::Bool(value)),
        RuntimeValue::Int(value) => Ok(RuntimePureScalar::I64(value)),
        RuntimeValue::I128(value) => Ok(RuntimePureScalar::I128(value)),
        RuntimeValue::ISize(value) => Ok(RuntimePureScalar::ISize(value)),
        RuntimeValue::UInt(value) => Ok(RuntimePureScalar::U64(value)),
        RuntimeValue::U128(value) => Ok(RuntimePureScalar::U128(value)),
        RuntimeValue::USize(value) => Ok(RuntimePureScalar::USize(value)),
        RuntimeValue::F32(value) => Ok(RuntimePureScalar::F32(value)),
        RuntimeValue::F64(value) => Ok(RuntimePureScalar::F64(value)),
        value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
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
) -> Result<RuntimeF32, RuntimeEvalError> {
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
) -> Result<RuntimeF64, RuntimeEvalError> {
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

fn evaluate_scalar_unary(
    op: RuntimeUnaryOp,
    value: RuntimePureScalar,
) -> Result<RuntimePureScalar, RuntimeEvalError> {
    match (op, value) {
        (RuntimeUnaryOp::Not, RuntimePureScalar::Bool(value)) => {
            Ok(RuntimePureScalar::Bool(!value))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I8(value)) => Ok(RuntimePureScalar::I8(-value)),
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I16(value)) => Ok(RuntimePureScalar::I16(-value)),
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I32(value)) => Ok(RuntimePureScalar::I32(-value)),
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I64(value)) => Ok(RuntimePureScalar::I64(-value)),
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I128(value)) => {
            Ok(RuntimePureScalar::I128(-value))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::ISize(value)) => {
            Ok(RuntimePureScalar::ISize(-value))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::F32(value)) => Ok(RuntimePureScalar::F32(
            RuntimeF32::from_f32(-value.to_f32()),
        )),
        (RuntimeUnaryOp::Neg, RuntimePureScalar::F64(value)) => Ok(RuntimePureScalar::F64(
            RuntimeF64::from_f64(-value.to_f64()),
        )),
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
            compare_scalar_float(&lhs.to_f32(), op, &rhs.to_f32()),
        )),
        (RuntimePureScalar::F64(lhs), RuntimePureScalar::F64(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_float(&lhs.to_f64(), op, &rhs.to_f64()),
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
            RuntimeF32::from_f32(evaluate_scalar_numeric(lhs.to_f32(), op, rhs.to_f32())),
        )),
        (RuntimePureScalar::F64(lhs), RuntimePureScalar::F64(rhs)) => Ok(RuntimePureScalar::F64(
            RuntimeF64::from_f64(evaluate_scalar_numeric(lhs.to_f64(), op, rhs.to_f64())),
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

fn evaluate_scalar_numeric<T>(lhs: T, op: RuntimeBinaryOp, rhs: T) -> T
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
            RuntimeExpr::Let { name, expr, body } => {
                let value = self.evaluate_expr(expr)?;
                self.env.push_scope_with_capacity(1);
                self.env.set(name.clone(), value);
                let result = self.evaluate_expr(body);
                self.env.pop_scope();
                result
            }
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

    fn evaluate_scalar_expr(
        &mut self,
        expr: &RuntimeExpr,
    ) -> Result<RuntimePureScalar, RuntimeEvalError> {
        self.stats.evaluated_exprs += 1;
        match expr {
            RuntimeExpr::Value(RuntimeValue::Bool(value)) => Ok(RuntimePureScalar::Bool(*value)),
            RuntimeExpr::Value(RuntimeValue::Int(value)) => Ok(RuntimePureScalar::I64(*value)),
            RuntimeExpr::Value(RuntimeValue::I128(value)) => Ok(RuntimePureScalar::I128(*value)),
            RuntimeExpr::Value(RuntimeValue::ISize(value)) => Ok(RuntimePureScalar::ISize(*value)),
            RuntimeExpr::Value(RuntimeValue::UInt(value)) => Ok(RuntimePureScalar::U64(*value)),
            RuntimeExpr::Value(RuntimeValue::U128(value)) => Ok(RuntimePureScalar::U128(*value)),
            RuntimeExpr::Value(RuntimeValue::USize(value)) => Ok(RuntimePureScalar::USize(*value)),
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
            return Ok(RuntimeValue::Int(sum));
        }
        let value = self.evaluate_expr(source)?;
        if let RuntimeValue::Seq(seq) = &value
            && let Some(sum) = seq.sum_as_i64()
        {
            return Ok(RuntimeValue::Int(sum));
        }
        let items = match runtime_value_into_sequence_values(value) {
            Ok(items) => items,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
        items
            .into_iter()
            .try_fold(RuntimeValue::Int(0), |acc, item| {
                evaluate_binary(acc, RuntimeBinaryOp::Add, item)
            })
    }

    fn evaluate_i64_local_sequence_sum(&self, name: &str) -> Result<Option<i64>, RuntimeEvalError> {
        let Some(value) = self.env.get(name) else {
            return Ok(None);
        };
        match value {
            RuntimeValue::Seq(seq) => match seq {
                RuntimeSeq::Values(items) => sum_i64_sequence_ref(items).map(Some),
                RuntimeSeq::Dense(items) => Ok(items.sum_as_i64()),
            },
            RuntimeValue::Tuple(items) => sum_i64_sequence_ref(items).map(Some),
            _ => Ok(None),
        }
    }

    fn evaluate_call_expr(
        &mut self,
        callee: &str,
        args: &[RuntimeExpr],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.evaluated_calls += 1;
        let args = self.evaluate_call_args(args)?;
        match (callee, args.as_slice()) {
            ("add", [RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)]) => {
                Ok(RuntimeValue::Int(lhs.saturating_add(*rhs)))
            }
            _ => Err(RuntimeEvalError::UnsupportedPure {
                name: callee.to_owned(),
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
            value => Err(RuntimeEvalError::MissingField {
                field: field.to_owned(),
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
    RuntimeValue::UInt(u64::try_from(len).unwrap_or(u64::MAX))
}

fn spread_runtime_values(value: RuntimeValue) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
    match runtime_value_into_sequence_values(value) {
        Ok(items) => Ok(items),
        Err(value) => Err(RuntimeEvalError::InvalidSpread(runtime_value_label(&value))),
    }
}
