//! Native Cranelift adapter for Arcweft pure helper functions.
//!
//! The VM remains the semantic reference. This crate is intentionally outside
//! `arcweft-core` so native code generation, executable memory, and the small
//! function-pointer call boundary stay in an adapter layer.

mod native_call;

use arcweft_core::pure::{
    PureFunctionBackend, PureFunctionBackendKind, PureFunctionRequest, PureFunctionResult,
    PureFunctionStats, RuntimeI64Args,
};
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeBinding, RuntimeCallTarget, RuntimeEvalError, RuntimeExpr,
    RuntimeIntrinsic, RuntimeUnaryOp, RuntimeValue,
};
use cranelift::codegen::ir::{BlockArg, MemFlags, UserFuncName};
use cranelift::jit::{JITBuilder, JITModule};
use cranelift::module::{FuncId, Linkage, Module, ModuleError, default_libcall_names};
use cranelift::prelude::{
    AbiParam, Configurable, FloatCC, FunctionBuilder, FunctionBuilderContext, InstBuilder, IntCC,
    Value, settings, types,
};
use std::collections::BTreeMap;
use thiserror::Error;

/// Native Cranelift backend for the current pure helper subset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CraneliftPureFunctionBackend;

/// Error produced while selecting, lowering, compiling, or invoking a helper.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CraneliftJitError {
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
enum LoweredI64Binding {
    Const(i64),
    Value(Value),
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
    ) -> Result<PureFunctionResult, CraneliftJitError> {
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
    ) -> Result<CompiledPureI64, CraneliftJitError> {
        let mut module = jit_module()?;
        let mut ctx = module.make_context();
        let mut func_ctx = FunctionBuilderContext::new();
        let mut signature = module.make_signature();
        signature.returns.push(AbiParam::new(types::I64));

        let func_id = module
            .declare_function("arcweft_pure_helper", Linkage::Local, &signature)
            .map_err(jit_error)?;
        ctx.func.signature = signature;
        ctx.func.name = UserFuncName::user(0, func_id.as_u32());

        let bindings = int_bindings(&request.bindings)?;
        let mut stats = arcweft_core::pure::PureFunctionStats::default();
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
            .define_function(func_id, &mut ctx)
            .map_err(jit_error)?;
        module.clear_context(&mut ctx);
        module.finalize_definitions().map_err(jit_error)?;
        let code = module.get_finalized_function(func_id);

        Ok(CompiledPureI64 {
            _module: module,
            code,
            stats,
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
    ) -> Result<CompiledPureI64Inputs, CraneliftJitError> {
        let param_names = param_names
            .into_iter()
            .map(Into::into)
            .collect::<Vec<String>>();
        validate_param_names(&param_names)?;
        if param_names.len() > 4 {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT integer helper supports at most 4 runtime inputs, got {}",
                param_names.len()
            )));
        }

        let mut module = jit_module()?;
        let mut ctx = module.make_context();
        let mut func_ctx = FunctionBuilderContext::new();
        let mut signature = module.make_signature();
        signature
            .params
            .extend(param_names.iter().map(|_| AbiParam::new(types::I64)));
        signature.returns.push(AbiParam::new(types::I64));

        let func_id = module
            .declare_function("arcweft_pure_helper_inputs", Linkage::Local, &signature)
            .map_err(jit_error)?;
        ctx.func.signature = signature;
        ctx.func.name = UserFuncName::user(0, func_id.as_u32());

        let captured_bindings = int_bindings(&request.bindings)?;
        let mut bindings = captured_bindings.clone();
        let mut stats = arcweft_core::pure::PureFunctionStats::default();
        {
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
            let block = builder.create_block();
            builder.append_block_params_for_function_params(block);
            builder.switch_to_block(block);
            let params = builder.block_params(block);
            for (name, value) in param_names.iter().zip(params.iter().copied()) {
                bindings.insert(name.clone(), LoweredI64Binding::Value(value));
            }
            let value = lower_expr(&mut builder, &bindings, &request.expr, &mut stats)?;
            builder.ins().return_(&[value]);
            builder.seal_all_blocks();
            builder.finalize();
        }

        module
            .define_function(func_id, &mut ctx)
            .map_err(jit_error)?;
        module.clear_context(&mut ctx);
        let batch_code = compile_i64_rows_batch_function(
            &mut module,
            &request.expr,
            &captured_bindings,
            &param_names,
        )?;
        let batch_sum_code = compile_i64_rows_batch_sum_function(
            &mut module,
            &request.expr,
            &captured_bindings,
            &param_names,
        )?;
        module.finalize_definitions().map_err(jit_error)?;
        let code = module.get_finalized_function(func_id);
        let batch_code = module.get_finalized_function(batch_code);
        let batch_sum_code = module.get_finalized_function(batch_sum_code);
        let caller =
            native_call::I64InputCaller::from_code(code, param_names.len()).ok_or_else(|| {
                CraneliftJitError::UnsupportedExpr(format!(
                    "JIT helper arity {} is outside the native call boundary",
                    param_names.len()
                ))
            })?;

        Ok(CompiledPureI64Inputs {
            _module: module,
            caller,
            batch_code,
            batch_sum_code,
            param_names,
            stats,
        })
    }

    /// Compiles a pure helper request to a reusable native `i32` function with
    /// runtime `i32` inputs.
    pub fn compile_i32_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureI32Inputs, CraneliftJitError> {
        let param_names = param_names
            .into_iter()
            .map(Into::into)
            .collect::<Vec<String>>();
        validate_param_names(&param_names)?;
        if param_names.len() > 4 {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT i32 helper supports at most 4 runtime inputs, got {}",
                param_names.len()
            )));
        }

        let mut module = jit_module()?;
        let mut ctx = module.make_context();
        let mut func_ctx = FunctionBuilderContext::new();
        let mut signature = module.make_signature();
        signature
            .params
            .extend(param_names.iter().map(|_| AbiParam::new(types::I32)));
        signature.returns.push(AbiParam::new(types::I32));

        let func_id = module
            .declare_function("arcweft_pure_i32_helper_inputs", Linkage::Local, &signature)
            .map_err(jit_error)?;
        ctx.func.signature = signature;
        ctx.func.name = UserFuncName::user(0, func_id.as_u32());

        let captured_bindings = i32_bindings(&request.bindings)?;
        let mut bindings = captured_bindings.clone();
        let mut stats = arcweft_core::pure::PureFunctionStats::default();
        {
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
            let block = builder.create_block();
            builder.append_block_params_for_function_params(block);
            builder.switch_to_block(block);
            let params = builder.block_params(block);
            for (name, value) in param_names.iter().zip(params.iter().copied()) {
                bindings.insert(name.clone(), LoweredI64Binding::Value(value));
            }
            let value = lower_i32_expr(&mut builder, &bindings, &request.expr, &mut stats)?;
            builder.ins().return_(&[value]);
            builder.seal_all_blocks();
            builder.finalize();
        }

        module
            .define_function(func_id, &mut ctx)
            .map_err(jit_error)?;
        module.clear_context(&mut ctx);
        let batch_code = compile_i32_rows_batch_function(
            &mut module,
            &request.expr,
            &captured_bindings,
            &param_names,
        )?;
        let batch_sum_code = compile_i32_rows_batch_sum_function(
            &mut module,
            &request.expr,
            &captured_bindings,
            &param_names,
        )?;
        module.finalize_definitions().map_err(jit_error)?;
        let code = module.get_finalized_function(func_id);
        let batch_code = module.get_finalized_function(batch_code);
        let batch_sum_code = module.get_finalized_function(batch_sum_code);
        let caller =
            native_call::I32InputCaller::from_code(code, param_names.len()).ok_or_else(|| {
                CraneliftJitError::UnsupportedExpr(format!(
                    "JIT i32 helper arity {} is outside the native call boundary",
                    param_names.len()
                ))
            })?;

        Ok(CompiledPureI32Inputs {
            _module: module,
            caller,
            batch_code,
            batch_sum_code,
            param_names,
            stats,
        })
    }

    /// Compiles a pure helper request to a reusable native `u32` function with
    /// runtime `u32` inputs.
    pub fn compile_u32_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureU32Inputs, CraneliftJitError> {
        let param_names = param_names
            .into_iter()
            .map(Into::into)
            .collect::<Vec<String>>();
        validate_param_names(&param_names)?;
        if param_names.len() > 4 {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT u32 helper supports at most 4 runtime inputs, got {}",
                param_names.len()
            )));
        }

        let mut module = jit_module()?;
        let mut ctx = module.make_context();
        let mut func_ctx = FunctionBuilderContext::new();
        let mut signature = module.make_signature();
        signature
            .params
            .extend(param_names.iter().map(|_| AbiParam::new(types::I32)));
        signature.returns.push(AbiParam::new(types::I32));

        let func_id = module
            .declare_function("arcweft_pure_u32_helper_inputs", Linkage::Local, &signature)
            .map_err(jit_error)?;
        ctx.func.signature = signature;
        ctx.func.name = UserFuncName::user(0, func_id.as_u32());

        let captured_bindings = u32_bindings(&request.bindings)?;
        let mut bindings = captured_bindings.clone();
        let mut stats = arcweft_core::pure::PureFunctionStats::default();
        {
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
            let block = builder.create_block();
            builder.append_block_params_for_function_params(block);
            builder.switch_to_block(block);
            let params = builder.block_params(block);
            for (name, value) in param_names.iter().zip(params.iter().copied()) {
                bindings.insert(name.clone(), LoweredI64Binding::Value(value));
            }
            let value = lower_u32_expr(&mut builder, &bindings, &request.expr, &mut stats)?;
            builder.ins().return_(&[value]);
            builder.seal_all_blocks();
            builder.finalize();
        }

        module
            .define_function(func_id, &mut ctx)
            .map_err(jit_error)?;
        module.clear_context(&mut ctx);
        let batch_code = compile_u32_rows_batch_function(
            &mut module,
            &request.expr,
            &captured_bindings,
            &param_names,
        )?;
        let batch_sum_code = compile_u32_rows_batch_sum_function(
            &mut module,
            &request.expr,
            &captured_bindings,
            &param_names,
        )?;
        module.finalize_definitions().map_err(jit_error)?;
        let code = module.get_finalized_function(func_id);
        let batch_code = module.get_finalized_function(batch_code);
        let batch_sum_code = module.get_finalized_function(batch_sum_code);
        let caller =
            native_call::U32InputCaller::from_code(code, param_names.len()).ok_or_else(|| {
                CraneliftJitError::UnsupportedExpr(format!(
                    "JIT u32 helper arity {} is outside the native call boundary",
                    param_names.len()
                ))
            })?;

        Ok(CompiledPureU32Inputs {
            _module: module,
            caller,
            batch_code,
            batch_sum_code,
            param_names,
            stats,
        })
    }

    /// Compiles a pure helper request to a reusable native `f32` function with
    /// runtime `f32` inputs.
    pub fn compile_f32_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureF32Inputs, CraneliftJitError> {
        let param_names = param_names
            .into_iter()
            .map(Into::into)
            .collect::<Vec<String>>();
        validate_param_names(&param_names)?;
        if param_names.len() > 4 {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT f32 helper supports at most 4 runtime inputs, got {}",
                param_names.len()
            )));
        }

        let mut module = jit_module()?;
        let mut ctx = module.make_context();
        let mut func_ctx = FunctionBuilderContext::new();
        let mut signature = module.make_signature();
        signature
            .params
            .extend(param_names.iter().map(|_| AbiParam::new(types::F32)));
        signature.returns.push(AbiParam::new(types::F32));

        let func_id = module
            .declare_function("arcweft_pure_f32_helper_inputs", Linkage::Local, &signature)
            .map_err(jit_error)?;
        ctx.func.signature = signature;
        ctx.func.name = UserFuncName::user(0, func_id.as_u32());

        let captured_bindings = f32_bindings(&request.bindings)?;
        let mut bindings = captured_bindings.clone();
        let mut stats = arcweft_core::pure::PureFunctionStats::default();
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
            .define_function(func_id, &mut ctx)
            .map_err(jit_error)?;
        module.clear_context(&mut ctx);
        let batch_code = compile_f32_rows_batch_function(
            &mut module,
            &request.expr,
            &captured_bindings,
            &param_names,
        )?;
        module.finalize_definitions().map_err(jit_error)?;
        let code = module.get_finalized_function(func_id);
        let batch_code = module.get_finalized_function(batch_code);
        let caller =
            native_call::F32InputCaller::from_code(code, param_names.len()).ok_or_else(|| {
                CraneliftJitError::UnsupportedExpr(format!(
                    "JIT f32 helper arity {} is outside the native call boundary",
                    param_names.len()
                ))
            })?;

        Ok(CompiledPureF32Inputs {
            _module: module,
            caller,
            batch_code,
            param_names,
            stats,
        })
    }

    /// Compiles a pure helper request to a reusable native `f64` function with
    /// runtime `f64` inputs.
    pub fn compile_f64_with_inputs(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureF64Inputs, CraneliftJitError> {
        let param_names = param_names
            .into_iter()
            .map(Into::into)
            .collect::<Vec<String>>();
        validate_param_names(&param_names)?;
        if param_names.len() > 4 {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT f64 helper supports at most 4 runtime inputs, got {}",
                param_names.len()
            )));
        }

        let mut module = jit_module()?;
        let mut ctx = module.make_context();
        let mut func_ctx = FunctionBuilderContext::new();
        let mut signature = module.make_signature();
        signature
            .params
            .extend(param_names.iter().map(|_| AbiParam::new(types::F64)));
        signature.returns.push(AbiParam::new(types::F64));

        let func_id = module
            .declare_function("arcweft_pure_f64_helper_inputs", Linkage::Local, &signature)
            .map_err(jit_error)?;
        ctx.func.signature = signature;
        ctx.func.name = UserFuncName::user(0, func_id.as_u32());

        let captured_bindings = f64_bindings(&request.bindings)?;
        let mut bindings = captured_bindings.clone();
        let mut stats = arcweft_core::pure::PureFunctionStats::default();
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
            .define_function(func_id, &mut ctx)
            .map_err(jit_error)?;
        module.clear_context(&mut ctx);
        let batch_code = compile_f64_rows_batch_function(
            &mut module,
            &request.expr,
            &captured_bindings,
            &param_names,
        )?;
        module.finalize_definitions().map_err(jit_error)?;
        let code = module.get_finalized_function(func_id);
        let batch_code = module.get_finalized_function(batch_code);
        let caller =
            native_call::F64InputCaller::from_code(code, param_names.len()).ok_or_else(|| {
                CraneliftJitError::UnsupportedExpr(format!(
                    "JIT f64 helper arity {} is outside the native call boundary",
                    param_names.len()
                ))
            })?;

        Ok(CompiledPureF64Inputs {
            _module: module,
            caller,
            batch_code,
            param_names,
            stats,
        })
    }

    /// Compiles a batch benchmark runner for a pure helper request.
    pub fn compile_i64_batch(
        &self,
        request: &PureFunctionRequest,
        param_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<CompiledPureI64Batch, CraneliftJitError> {
        let param_names = param_names
            .into_iter()
            .map(Into::into)
            .collect::<Vec<String>>();
        validate_param_names(&param_names)?;
        if param_names.len() > 4 {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT batch helper supports at most 4 runtime inputs, got {}",
                param_names.len()
            )));
        }

        let mut module = jit_module()?;
        let mut ctx = module.make_context();
        let mut func_ctx = FunctionBuilderContext::new();
        let mut signature = module.make_signature();
        signature.params.extend([
            AbiParam::new(types::I64),
            AbiParam::new(types::I64),
            AbiParam::new(types::I64),
        ]);
        signature.returns.push(AbiParam::new(types::I64));

        let func_id = module
            .declare_function("arcweft_pure_helper_batch", Linkage::Local, &signature)
            .map_err(jit_error)?;
        ctx.func.signature = signature;
        ctx.func.name = UserFuncName::user(0, func_id.as_u32());

        let captured_bindings = int_bindings(&request.bindings)?;
        let mut stats = arcweft_core::pure::PureFunctionStats::default();
        {
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            let seed = builder.block_params(entry)[0];
            let sample = builder.block_params(entry)[1];
            let iterations = builder.block_params(entry)[2];

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
                bindings.insert(name.clone(), LoweredI64Binding::Value(value));
            }
            let value = lower_expr(&mut builder, &bindings, &request.expr, &mut stats)?;
            let next_accumulator = builder.ins().iadd(accumulator, value);
            let one = builder.ins().iconst(types::I64, 1);
            let next_index = builder.ins().iadd(index, one);
            let next_inputs = input_values
                .iter()
                .copied()
                .enumerate()
                .map(|(param_index, value)| {
                    lower_next_input_value(&mut builder, value, param_index)
                })
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
            .define_function(func_id, &mut ctx)
            .map_err(jit_error)?;
        module.clear_context(&mut ctx);
        module.finalize_definitions().map_err(jit_error)?;
        let code = module.get_finalized_function(func_id);

        Ok(CompiledPureI64Batch {
            _module: module,
            code,
            param_names,
            stats,
        })
    }
}

fn compile_i64_rows_batch_function(
    module: &mut JITModule,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<String, LoweredI64Binding>,
    param_names: &[String],
) -> Result<FuncId, CraneliftJitError> {
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftJitError::UnsupportedHost(
            "JIT rows batch currently requires a 64-bit host pointer type".to_owned(),
        ));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature.params.extend([
        AbiParam::new(pointer_type),
        AbiParam::new(types::I64),
        AbiParam::new(pointer_type),
    ]);

    let func_id = module
        .declare_function("arcweft_pure_helper_rows_batch", Linkage::Local, &signature)
        .map_err(jit_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    let mut stats = arcweft_core::pure::PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let inputs_ptr = builder.block_params(entry)[0];
        let rows = builder.block_params(entry)[1];
        let out_ptr = builder.block_params(entry)[2];

        let loop_block = builder.create_block();
        let body_block = builder.create_block();
        let done_block = builder.create_block();
        builder.append_block_param(loop_block, types::I64);

        let zero = builder.ins().iconst(types::I64, 0);
        builder.ins().jump(loop_block, &[BlockArg::from(zero)]);

        builder.switch_to_block(loop_block);
        let row = builder.block_params(loop_block)[0];
        let keep_going = builder.ins().icmp(IntCC::SignedLessThan, row, rows);
        builder
            .ins()
            .brif(keep_going, body_block, &[], done_block, &[]);

        builder.switch_to_block(body_block);
        let mut bindings = captured_bindings.clone();
        for (param_index, name) in param_names.iter().enumerate() {
            let value = load_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                param_names.len(),
                param_index,
            );
            bindings.insert(name.clone(), LoweredI64Binding::Value(value));
        }
        let value = lower_expr(&mut builder, &bindings, expr, &mut stats)?;
        store_batch_output(&mut builder, out_ptr, row, value);
        let one = builder.ins().iconst(types::I64, 1);
        let next_row = builder.ins().iadd(row, one);
        builder.ins().jump(loop_block, &[BlockArg::from(next_row)]);

        builder.switch_to_block(done_block);
        builder.ins().return_(&[]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(jit_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

fn compile_i64_rows_batch_sum_function(
    module: &mut JITModule,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<String, LoweredI64Binding>,
    param_names: &[String],
) -> Result<FuncId, CraneliftJitError> {
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftJitError::UnsupportedHost(
            "JIT rows batch sum currently requires a 64-bit host pointer type".to_owned(),
        ));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature
        .params
        .extend([AbiParam::new(pointer_type), AbiParam::new(types::I64)]);
    signature.returns.push(AbiParam::new(types::I64));

    let func_id = module
        .declare_function(
            "arcweft_pure_helper_rows_batch_sum",
            Linkage::Local,
            &signature,
        )
        .map_err(jit_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    let mut stats = arcweft_core::pure::PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let inputs_ptr = builder.block_params(entry)[0];
        let rows = builder.block_params(entry)[1];

        let loop_block = builder.create_block();
        let body_block = builder.create_block();
        let done_block = builder.create_block();
        builder.append_block_param(loop_block, types::I64);
        builder.append_block_param(loop_block, types::I64);

        let zero = builder.ins().iconst(types::I64, 0);
        builder
            .ins()
            .jump(loop_block, &[BlockArg::from(zero), BlockArg::from(zero)]);

        builder.switch_to_block(loop_block);
        let row = builder.block_params(loop_block)[0];
        let accumulator = builder.block_params(loop_block)[1];
        let keep_going = builder.ins().icmp(IntCC::SignedLessThan, row, rows);
        builder
            .ins()
            .brif(keep_going, body_block, &[], done_block, &[]);

        builder.switch_to_block(body_block);
        let mut bindings = captured_bindings.clone();
        for (param_index, name) in param_names.iter().enumerate() {
            let value = load_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                param_names.len(),
                param_index,
            );
            bindings.insert(name.clone(), LoweredI64Binding::Value(value));
        }
        let value = lower_expr(&mut builder, &bindings, expr, &mut stats)?;
        let next_accumulator = builder.ins().iadd(accumulator, value);
        let one = builder.ins().iconst(types::I64, 1);
        let next_row = builder.ins().iadd(row, one);
        builder.ins().jump(
            loop_block,
            &[BlockArg::from(next_row), BlockArg::from(next_accumulator)],
        );

        builder.switch_to_block(done_block);
        builder.ins().return_(&[accumulator]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(jit_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

fn compile_i32_rows_batch_function(
    module: &mut JITModule,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<String, LoweredI64Binding>,
    param_names: &[String],
) -> Result<FuncId, CraneliftJitError> {
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftJitError::UnsupportedHost(
            "JIT i32 rows batch currently requires a 64-bit host pointer type".to_owned(),
        ));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature.params.extend([
        AbiParam::new(pointer_type),
        AbiParam::new(types::I64),
        AbiParam::new(pointer_type),
    ]);

    let func_id = module
        .declare_function("arcweft_pure_i32_rows_batch", Linkage::Local, &signature)
        .map_err(jit_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    let mut stats = arcweft_core::pure::PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let inputs_ptr = builder.block_params(entry)[0];
        let rows = builder.block_params(entry)[1];
        let out_ptr = builder.block_params(entry)[2];

        let loop_block = builder.create_block();
        let body_block = builder.create_block();
        let done_block = builder.create_block();
        builder.append_block_param(loop_block, types::I64);

        let zero = builder.ins().iconst(types::I64, 0);
        builder.ins().jump(loop_block, &[BlockArg::from(zero)]);

        builder.switch_to_block(loop_block);
        let row = builder.block_params(loop_block)[0];
        let keep_going = builder.ins().icmp(IntCC::SignedLessThan, row, rows);
        builder
            .ins()
            .brif(keep_going, body_block, &[], done_block, &[]);

        builder.switch_to_block(body_block);
        let mut bindings = captured_bindings.clone();
        for (param_index, name) in param_names.iter().enumerate() {
            let value = load_i32_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                param_names.len(),
                param_index,
            );
            bindings.insert(name.clone(), LoweredI64Binding::Value(value));
        }
        let value = lower_i32_expr(&mut builder, &bindings, expr, &mut stats)?;
        store_i32_batch_output(&mut builder, out_ptr, row, value);
        let one = builder.ins().iconst(types::I64, 1);
        let next_row = builder.ins().iadd(row, one);
        builder.ins().jump(loop_block, &[BlockArg::from(next_row)]);

        builder.switch_to_block(done_block);
        builder.ins().return_(&[]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(jit_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

fn compile_i32_rows_batch_sum_function(
    module: &mut JITModule,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<String, LoweredI64Binding>,
    param_names: &[String],
) -> Result<FuncId, CraneliftJitError> {
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftJitError::UnsupportedHost(
            "JIT i32 rows batch sum currently requires a 64-bit host pointer type".to_owned(),
        ));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature
        .params
        .extend([AbiParam::new(pointer_type), AbiParam::new(types::I64)]);
    signature.returns.push(AbiParam::new(types::I64));

    let func_id = module
        .declare_function(
            "arcweft_pure_i32_rows_batch_sum",
            Linkage::Local,
            &signature,
        )
        .map_err(jit_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    let mut stats = arcweft_core::pure::PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let inputs_ptr = builder.block_params(entry)[0];
        let rows = builder.block_params(entry)[1];

        let loop_block = builder.create_block();
        let body_block = builder.create_block();
        let done_block = builder.create_block();
        builder.append_block_param(loop_block, types::I64);
        builder.append_block_param(loop_block, types::I64);

        let zero = builder.ins().iconst(types::I64, 0);
        builder
            .ins()
            .jump(loop_block, &[BlockArg::from(zero), BlockArg::from(zero)]);

        builder.switch_to_block(loop_block);
        let row = builder.block_params(loop_block)[0];
        let accumulator = builder.block_params(loop_block)[1];
        let keep_going = builder.ins().icmp(IntCC::SignedLessThan, row, rows);
        builder
            .ins()
            .brif(keep_going, body_block, &[], done_block, &[]);

        builder.switch_to_block(body_block);
        let mut bindings = captured_bindings.clone();
        for (param_index, name) in param_names.iter().enumerate() {
            let value = load_i32_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                param_names.len(),
                param_index,
            );
            bindings.insert(name.clone(), LoweredI64Binding::Value(value));
        }
        let value = lower_i32_expr(&mut builder, &bindings, expr, &mut stats)?;
        let value = builder.ins().sextend(types::I64, value);
        let next_accumulator = builder.ins().iadd(accumulator, value);
        let one = builder.ins().iconst(types::I64, 1);
        let next_row = builder.ins().iadd(row, one);
        builder.ins().jump(
            loop_block,
            &[BlockArg::from(next_row), BlockArg::from(next_accumulator)],
        );

        builder.switch_to_block(done_block);
        builder.ins().return_(&[accumulator]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(jit_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

fn compile_u32_rows_batch_function(
    module: &mut JITModule,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<String, LoweredI64Binding>,
    param_names: &[String],
) -> Result<FuncId, CraneliftJitError> {
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftJitError::UnsupportedHost(
            "JIT u32 rows batch currently requires a 64-bit host pointer type".to_owned(),
        ));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature.params.extend([
        AbiParam::new(pointer_type),
        AbiParam::new(types::I64),
        AbiParam::new(pointer_type),
    ]);

    let func_id = module
        .declare_function("arcweft_pure_u32_rows_batch", Linkage::Local, &signature)
        .map_err(jit_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    let mut stats = arcweft_core::pure::PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let inputs_ptr = builder.block_params(entry)[0];
        let rows = builder.block_params(entry)[1];
        let out_ptr = builder.block_params(entry)[2];

        let loop_block = builder.create_block();
        let body_block = builder.create_block();
        let done_block = builder.create_block();
        builder.append_block_param(loop_block, types::I64);

        let zero = builder.ins().iconst(types::I64, 0);
        builder.ins().jump(loop_block, &[BlockArg::from(zero)]);

        builder.switch_to_block(loop_block);
        let row = builder.block_params(loop_block)[0];
        let keep_going = builder.ins().icmp(IntCC::UnsignedLessThan, row, rows);
        builder
            .ins()
            .brif(keep_going, body_block, &[], done_block, &[]);

        builder.switch_to_block(body_block);
        let mut bindings = captured_bindings.clone();
        for (param_index, name) in param_names.iter().enumerate() {
            let value = load_u32_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                param_names.len(),
                param_index,
            );
            bindings.insert(name.clone(), LoweredI64Binding::Value(value));
        }
        let value = lower_u32_expr(&mut builder, &bindings, expr, &mut stats)?;
        store_u32_batch_output(&mut builder, out_ptr, row, value);
        let one = builder.ins().iconst(types::I64, 1);
        let next_row = builder.ins().iadd(row, one);
        builder.ins().jump(loop_block, &[BlockArg::from(next_row)]);

        builder.switch_to_block(done_block);
        builder.ins().return_(&[]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(jit_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

fn compile_u32_rows_batch_sum_function(
    module: &mut JITModule,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<String, LoweredI64Binding>,
    param_names: &[String],
) -> Result<FuncId, CraneliftJitError> {
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftJitError::UnsupportedHost(
            "JIT u32 rows batch sum currently requires a 64-bit host pointer type".to_owned(),
        ));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature
        .params
        .extend([AbiParam::new(pointer_type), AbiParam::new(types::I64)]);
    signature.returns.push(AbiParam::new(types::I64));

    let func_id = module
        .declare_function(
            "arcweft_pure_u32_rows_batch_sum",
            Linkage::Local,
            &signature,
        )
        .map_err(jit_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    let mut stats = arcweft_core::pure::PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let inputs_ptr = builder.block_params(entry)[0];
        let rows = builder.block_params(entry)[1];

        let loop_block = builder.create_block();
        let body_block = builder.create_block();
        let done_block = builder.create_block();
        builder.append_block_param(loop_block, types::I64);
        builder.append_block_param(loop_block, types::I64);

        let zero = builder.ins().iconst(types::I64, 0);
        builder
            .ins()
            .jump(loop_block, &[BlockArg::from(zero), BlockArg::from(zero)]);

        builder.switch_to_block(loop_block);
        let row = builder.block_params(loop_block)[0];
        let accumulator = builder.block_params(loop_block)[1];
        let keep_going = builder.ins().icmp(IntCC::UnsignedLessThan, row, rows);
        builder
            .ins()
            .brif(keep_going, body_block, &[], done_block, &[]);

        builder.switch_to_block(body_block);
        let mut bindings = captured_bindings.clone();
        for (param_index, name) in param_names.iter().enumerate() {
            let value = load_u32_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                param_names.len(),
                param_index,
            );
            bindings.insert(name.clone(), LoweredI64Binding::Value(value));
        }
        let value = lower_u32_expr(&mut builder, &bindings, expr, &mut stats)?;
        let value = builder.ins().uextend(types::I64, value);
        let next_accumulator = builder.ins().iadd(accumulator, value);
        let one = builder.ins().iconst(types::I64, 1);
        let next_row = builder.ins().iadd(row, one);
        builder.ins().jump(
            loop_block,
            &[BlockArg::from(next_row), BlockArg::from(next_accumulator)],
        );

        builder.switch_to_block(done_block);
        builder.ins().return_(&[accumulator]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(jit_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

fn compile_f32_rows_batch_function(
    module: &mut JITModule,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<String, LoweredF32Binding>,
    param_names: &[String],
) -> Result<FuncId, CraneliftJitError> {
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftJitError::UnsupportedHost(
            "JIT f32 rows batch currently requires a 64-bit host pointer type".to_owned(),
        ));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature.params.extend([
        AbiParam::new(pointer_type),
        AbiParam::new(types::I64),
        AbiParam::new(pointer_type),
    ]);

    let func_id = module
        .declare_function("arcweft_pure_f32_rows_batch", Linkage::Local, &signature)
        .map_err(jit_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    let mut stats = arcweft_core::pure::PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let inputs_ptr = builder.block_params(entry)[0];
        let rows = builder.block_params(entry)[1];
        let out_ptr = builder.block_params(entry)[2];

        let loop_block = builder.create_block();
        let body_block = builder.create_block();
        let done_block = builder.create_block();
        builder.append_block_param(loop_block, types::I64);

        let zero = builder.ins().iconst(types::I64, 0);
        builder.ins().jump(loop_block, &[BlockArg::from(zero)]);

        builder.switch_to_block(loop_block);
        let row = builder.block_params(loop_block)[0];
        let keep_going = builder.ins().icmp(IntCC::SignedLessThan, row, rows);
        builder
            .ins()
            .brif(keep_going, body_block, &[], done_block, &[]);

        builder.switch_to_block(body_block);
        let mut bindings = captured_bindings.clone();
        for (param_index, name) in param_names.iter().enumerate() {
            let value = load_f32_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                param_names.len(),
                param_index,
            );
            bindings.insert(name.clone(), LoweredF32Binding::Value(value));
        }
        let value = lower_f32_expr(&mut builder, &bindings, expr, &mut stats)?;
        store_f32_batch_output(&mut builder, out_ptr, row, value);
        let one = builder.ins().iconst(types::I64, 1);
        let next_row = builder.ins().iadd(row, one);
        builder.ins().jump(loop_block, &[BlockArg::from(next_row)]);

        builder.switch_to_block(done_block);
        builder.ins().return_(&[]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(jit_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

fn compile_f64_rows_batch_function(
    module: &mut JITModule,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<String, LoweredF64Binding>,
    param_names: &[String],
) -> Result<FuncId, CraneliftJitError> {
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftJitError::UnsupportedHost(
            "JIT f64 rows batch currently requires a 64-bit host pointer type".to_owned(),
        ));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature.params.extend([
        AbiParam::new(pointer_type),
        AbiParam::new(types::I64),
        AbiParam::new(pointer_type),
    ]);

    let func_id = module
        .declare_function("arcweft_pure_f64_rows_batch", Linkage::Local, &signature)
        .map_err(jit_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    let mut stats = arcweft_core::pure::PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let inputs_ptr = builder.block_params(entry)[0];
        let rows = builder.block_params(entry)[1];
        let out_ptr = builder.block_params(entry)[2];

        let loop_block = builder.create_block();
        let body_block = builder.create_block();
        let done_block = builder.create_block();
        builder.append_block_param(loop_block, types::I64);

        let zero = builder.ins().iconst(types::I64, 0);
        builder.ins().jump(loop_block, &[BlockArg::from(zero)]);

        builder.switch_to_block(loop_block);
        let row = builder.block_params(loop_block)[0];
        let keep_going = builder.ins().icmp(IntCC::SignedLessThan, row, rows);
        builder
            .ins()
            .brif(keep_going, body_block, &[], done_block, &[]);

        builder.switch_to_block(body_block);
        let mut bindings = captured_bindings.clone();
        for (param_index, name) in param_names.iter().enumerate() {
            let value = load_f64_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                param_names.len(),
                param_index,
            );
            bindings.insert(name.clone(), LoweredF64Binding::Value(value));
        }
        let value = lower_f64_expr(&mut builder, &bindings, expr, &mut stats)?;
        store_f64_batch_output(&mut builder, out_ptr, row, value);
        let one = builder.ins().iconst(types::I64, 1);
        let next_row = builder.ins().iadd(row, one);
        builder.ins().jump(loop_block, &[BlockArg::from(next_row)]);

        builder.switch_to_block(done_block);
        builder.ins().return_(&[]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(jit_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

fn load_batch_input(
    builder: &mut FunctionBuilder<'_>,
    inputs_ptr: Value,
    row: Value,
    arity: usize,
    param_index: usize,
) -> Value {
    let stride_bytes =
        i64::try_from(arity.saturating_mul(std::mem::size_of::<i64>())).unwrap_or(i64::MAX);
    let row_stride = builder.ins().iconst(types::I64, stride_bytes);
    let row_offset = builder.ins().imul(row, row_stride);
    let param_offset =
        i64::try_from(param_index.saturating_mul(std::mem::size_of::<i64>())).unwrap_or(i64::MAX);
    let byte_offset = if param_offset == 0 {
        row_offset
    } else {
        let param_offset = builder.ins().iconst(types::I64, param_offset);
        builder.ins().iadd(row_offset, param_offset)
    };
    let address = builder.ins().iadd(inputs_ptr, byte_offset);
    builder
        .ins()
        .load(types::I64, MemFlags::trusted(), address, 0)
}

fn load_i32_batch_input(
    builder: &mut FunctionBuilder<'_>,
    inputs_ptr: Value,
    row: Value,
    arity: usize,
    param_index: usize,
) -> Value {
    let stride_bytes =
        i64::try_from(arity.saturating_mul(std::mem::size_of::<i32>())).unwrap_or(i64::MAX);
    let row_stride = builder.ins().iconst(types::I64, stride_bytes);
    let row_offset = builder.ins().imul(row, row_stride);
    let param_offset =
        i64::try_from(param_index.saturating_mul(std::mem::size_of::<i32>())).unwrap_or(i64::MAX);
    let byte_offset = if param_offset == 0 {
        row_offset
    } else {
        let param_offset = builder.ins().iconst(types::I64, param_offset);
        builder.ins().iadd(row_offset, param_offset)
    };
    let address = builder.ins().iadd(inputs_ptr, byte_offset);
    builder
        .ins()
        .load(types::I32, MemFlags::trusted(), address, 0)
}

fn load_u32_batch_input(
    builder: &mut FunctionBuilder<'_>,
    inputs_ptr: Value,
    row: Value,
    arity: usize,
    param_index: usize,
) -> Value {
    let stride_bytes =
        i64::try_from(arity.saturating_mul(std::mem::size_of::<u32>())).unwrap_or(i64::MAX);
    let row_stride = builder.ins().iconst(types::I64, stride_bytes);
    let row_offset = builder.ins().imul(row, row_stride);
    let param_offset =
        i64::try_from(param_index.saturating_mul(std::mem::size_of::<u32>())).unwrap_or(i64::MAX);
    let byte_offset = if param_offset == 0 {
        row_offset
    } else {
        let param_offset = builder.ins().iconst(types::I64, param_offset);
        builder.ins().iadd(row_offset, param_offset)
    };
    let address = builder.ins().iadd(inputs_ptr, byte_offset);
    builder
        .ins()
        .load(types::I32, MemFlags::trusted(), address, 0)
}

fn load_f32_batch_input(
    builder: &mut FunctionBuilder<'_>,
    inputs_ptr: Value,
    row: Value,
    arity: usize,
    param_index: usize,
) -> Value {
    let stride_bytes =
        i64::try_from(arity.saturating_mul(std::mem::size_of::<f32>())).unwrap_or(i64::MAX);
    let row_stride = builder.ins().iconst(types::I64, stride_bytes);
    let row_offset = builder.ins().imul(row, row_stride);
    let param_offset =
        i64::try_from(param_index.saturating_mul(std::mem::size_of::<f32>())).unwrap_or(i64::MAX);
    let byte_offset = if param_offset == 0 {
        row_offset
    } else {
        let param_offset = builder.ins().iconst(types::I64, param_offset);
        builder.ins().iadd(row_offset, param_offset)
    };
    let address = builder.ins().iadd(inputs_ptr, byte_offset);
    builder
        .ins()
        .load(types::F32, MemFlags::trusted(), address, 0)
}

fn load_f64_batch_input(
    builder: &mut FunctionBuilder<'_>,
    inputs_ptr: Value,
    row: Value,
    arity: usize,
    param_index: usize,
) -> Value {
    let stride_bytes =
        i64::try_from(arity.saturating_mul(std::mem::size_of::<f64>())).unwrap_or(i64::MAX);
    let row_stride = builder.ins().iconst(types::I64, stride_bytes);
    let row_offset = builder.ins().imul(row, row_stride);
    let param_offset =
        i64::try_from(param_index.saturating_mul(std::mem::size_of::<f64>())).unwrap_or(i64::MAX);
    let byte_offset = if param_offset == 0 {
        row_offset
    } else {
        let param_offset = builder.ins().iconst(types::I64, param_offset);
        builder.ins().iadd(row_offset, param_offset)
    };
    let address = builder.ins().iadd(inputs_ptr, byte_offset);
    builder
        .ins()
        .load(types::F64, MemFlags::trusted(), address, 0)
}

fn store_batch_output(builder: &mut FunctionBuilder<'_>, out_ptr: Value, row: Value, value: Value) {
    let value_bytes = i64::try_from(std::mem::size_of::<i64>()).unwrap_or(i64::MAX);
    let stride = builder.ins().iconst(types::I64, value_bytes);
    let byte_offset = builder.ins().imul(row, stride);
    let address = builder.ins().iadd(out_ptr, byte_offset);
    builder.ins().store(MemFlags::trusted(), value, address, 0);
}

fn store_f32_batch_output(
    builder: &mut FunctionBuilder<'_>,
    out_ptr: Value,
    row: Value,
    value: Value,
) {
    let value_bytes = i64::try_from(std::mem::size_of::<f32>()).unwrap_or(i64::MAX);
    let stride = builder.ins().iconst(types::I64, value_bytes);
    let byte_offset = builder.ins().imul(row, stride);
    let address = builder.ins().iadd(out_ptr, byte_offset);
    builder.ins().store(MemFlags::trusted(), value, address, 0);
}

fn store_f64_batch_output(
    builder: &mut FunctionBuilder<'_>,
    out_ptr: Value,
    row: Value,
    value: Value,
) {
    let value_bytes = i64::try_from(std::mem::size_of::<f64>()).unwrap_or(i64::MAX);
    let stride = builder.ins().iconst(types::I64, value_bytes);
    let byte_offset = builder.ins().imul(row, stride);
    let address = builder.ins().iadd(out_ptr, byte_offset);
    builder.ins().store(MemFlags::trusted(), value, address, 0);
}

fn store_i32_batch_output(
    builder: &mut FunctionBuilder<'_>,
    out_ptr: Value,
    row: Value,
    value: Value,
) {
    let value_bytes = i64::try_from(std::mem::size_of::<i32>()).unwrap_or(i64::MAX);
    let stride = builder.ins().iconst(types::I64, value_bytes);
    let byte_offset = builder.ins().imul(row, stride);
    let address = builder.ins().iadd(out_ptr, byte_offset);
    builder.ins().store(MemFlags::trusted(), value, address, 0);
}

impl CompiledPureI64 {
    /// Calls the compiled helper.
    pub fn call(&self) -> i64 {
        native_call::call_i64(self.code)
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

fn store_u32_batch_output(
    builder: &mut FunctionBuilder<'_>,
    out_ptr: Value,
    row: Value,
    value: Value,
) {
    let value_bytes = i64::try_from(std::mem::size_of::<u32>()).unwrap_or(i64::MAX);
    let stride = builder.ins().iconst(types::I64, value_bytes);
    let byte_offset = builder.ins().imul(row, stride);
    let address = builder.ins().iadd(out_ptr, byte_offset);
    builder.ins().store(MemFlags::trusted(), value, address, 0);
}

impl CompiledPureI64Inputs {
    /// Calls the compiled helper with runtime integer inputs.
    pub fn call(&self, inputs: &[i64]) -> Result<i64, CraneliftJitError> {
        if inputs.len() != self.param_names.len() {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT helper expected {} input(s), got {}",
                self.param_names.len(),
                inputs.len()
            )));
        }
        self.caller.call(inputs).ok_or_else(|| {
            CraneliftJitError::UnsupportedExpr(format!(
                "JIT helper arity {} is outside the native call boundary",
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper with the runtime fixed-size integer pack.
    pub fn call_i64_args(&self, args: RuntimeI64Args) -> Result<i64, CraneliftJitError> {
        if args.len() != self.param_names.len() {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT helper expected {} input(s), got {}",
                self.param_names.len(),
                args.len()
            )));
        }
        let (values, len) = args.into_parts();
        self.caller.call_packed(values, len).ok_or_else(|| {
            CraneliftJitError::UnsupportedExpr(format!(
                "JIT helper arity {len} is outside the native call boundary"
            ))
        })
    }

    /// Calls the compiled helper for flat row-major `i64` inputs.
    pub fn call_flat_batch(
        &self,
        inputs: &[i64],
        out: &mut [i64],
    ) -> Result<(), CraneliftJitError> {
        if !native_call::call_i64_rows_batch(self.batch_code, inputs, self.param_names.len(), out) {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT rows batch expected {} input value(s), got {} for {} row(s)",
                self.param_names.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Calls the compiled helper for flat row-major `i64` inputs and returns
    /// the sum of all row results without writing an output slice.
    pub fn call_flat_batch_sum(
        &self,
        inputs: &[i64],
        rows: usize,
    ) -> Result<i64, CraneliftJitError> {
        native_call::call_i64_rows_batch_sum(
            self.batch_sum_code,
            inputs,
            self.param_names.len(),
            rows,
        )
        .ok_or_else(|| {
            CraneliftJitError::UnsupportedExpr(format!(
                "JIT rows batch sum expected {} input value(s), got {} for {} row(s)",
                self.param_names.len().saturating_mul(rows),
                inputs.len(),
                rows
            ))
        })
    }

    /// Returns the local binding names used as runtime parameters.
    pub fn param_names(&self) -> &[String] {
        &self.param_names
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

impl CompiledPureI32Inputs {
    /// Calls the compiled helper with runtime `i32` inputs.
    pub fn call(&self, inputs: &[i32]) -> Result<i32, CraneliftJitError> {
        if inputs.len() != self.param_names.len() {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT i32 helper expected {} input(s), got {}",
                self.param_names.len(),
                inputs.len()
            )));
        }
        self.caller.call(inputs).ok_or_else(|| {
            CraneliftJitError::UnsupportedExpr(format!(
                "JIT i32 helper arity {} is outside the native call boundary",
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper for flat row-major `i32` inputs.
    pub fn call_flat_batch(
        &self,
        inputs: &[i32],
        out: &mut [i32],
    ) -> Result<(), CraneliftJitError> {
        if !native_call::call_i32_rows_batch(self.batch_code, inputs, self.param_names.len(), out) {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT i32 rows batch expected {} input value(s), got {} for {} row(s)",
                self.param_names.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Calls the compiled helper for flat row-major `i32` inputs and sums the
    /// `i32` outputs into an `i64` accumulator.
    pub fn call_flat_batch_sum(
        &self,
        inputs: &[i32],
        rows: usize,
    ) -> Result<i64, CraneliftJitError> {
        native_call::call_i32_rows_batch_sum(
            self.batch_sum_code,
            inputs,
            self.param_names.len(),
            rows,
        )
        .ok_or_else(|| {
            CraneliftJitError::UnsupportedExpr(format!(
                "JIT i32 rows batch sum expected {} input value(s), got {} for {rows} row(s)",
                self.param_names.len().saturating_mul(rows),
                inputs.len()
            ))
        })
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

impl CompiledPureU32Inputs {
    /// Calls the compiled helper with runtime `u32` inputs.
    pub fn call(&self, inputs: &[u32]) -> Result<u32, CraneliftJitError> {
        if inputs.len() != self.param_names.len() {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT u32 helper expected {} input(s), got {}",
                self.param_names.len(),
                inputs.len()
            )));
        }
        self.caller.call(inputs).ok_or_else(|| {
            CraneliftJitError::UnsupportedExpr(format!(
                "JIT u32 helper arity {} is outside the native call boundary",
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper for flat row-major `u32` inputs.
    pub fn call_flat_batch(
        &self,
        inputs: &[u32],
        out: &mut [u32],
    ) -> Result<(), CraneliftJitError> {
        if !native_call::call_u32_rows_batch(self.batch_code, inputs, self.param_names.len(), out) {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT u32 rows batch expected {} input value(s), got {} for {} row(s)",
                self.param_names.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Calls the compiled helper for flat row-major `u32` inputs and sums the
    /// `u32` outputs into an `i64` accumulator.
    pub fn call_flat_batch_sum(
        &self,
        inputs: &[u32],
        rows: usize,
    ) -> Result<i64, CraneliftJitError> {
        native_call::call_u32_rows_batch_sum(
            self.batch_sum_code,
            inputs,
            self.param_names.len(),
            rows,
        )
        .ok_or_else(|| {
            CraneliftJitError::UnsupportedExpr(format!(
                "JIT u32 rows batch sum expected {} input value(s), got {} for {rows} row(s)",
                self.param_names.len().saturating_mul(rows),
                inputs.len()
            ))
        })
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

impl CompiledPureF32Inputs {
    /// Calls the compiled helper with runtime `f32` inputs.
    pub fn call(&self, inputs: &[f32]) -> Result<f32, CraneliftJitError> {
        if inputs.len() != self.param_names.len() {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT f32 helper expected {} input(s), got {}",
                self.param_names.len(),
                inputs.len()
            )));
        }
        self.caller.call(inputs).ok_or_else(|| {
            CraneliftJitError::UnsupportedExpr(format!(
                "JIT f32 helper arity {} is outside the native call boundary",
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper for flat row-major `f32` inputs.
    pub fn call_flat_batch(
        &self,
        inputs: &[f32],
        out: &mut [f32],
    ) -> Result<(), CraneliftJitError> {
        if !native_call::call_f32_rows_batch(self.batch_code, inputs, self.param_names.len(), out) {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT f32 rows batch expected {} input value(s), got {} for {} row(s)",
                self.param_names.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

impl CompiledPureF64Inputs {
    /// Calls the compiled helper with runtime `f64` inputs.
    pub fn call(&self, inputs: &[f64]) -> Result<f64, CraneliftJitError> {
        if inputs.len() != self.param_names.len() {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT f64 helper expected {} input(s), got {}",
                self.param_names.len(),
                inputs.len()
            )));
        }
        self.caller.call(inputs).ok_or_else(|| {
            CraneliftJitError::UnsupportedExpr(format!(
                "JIT f64 helper arity {} is outside the native call boundary",
                inputs.len()
            ))
        })
    }

    /// Calls the compiled helper for flat row-major `f64` inputs.
    pub fn call_flat_batch(
        &self,
        inputs: &[f64],
        out: &mut [f64],
    ) -> Result<(), CraneliftJitError> {
        if !native_call::call_f64_rows_batch(self.batch_code, inputs, self.param_names.len(), out) {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT f64 rows batch expected {} input value(s), got {} for {} row(s)",
                self.param_names.len().saturating_mul(out.len()),
                inputs.len(),
                out.len()
            )));
        }
        Ok(())
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

impl CompiledPureI64Batch {
    /// Calls the compiled batch helper for a deterministic input series.
    pub fn call(
        &self,
        seed: u64,
        sample: usize,
        iterations: usize,
    ) -> Result<i64, CraneliftJitError> {
        let seed = i64::try_from(seed).map_err(|_| {
            CraneliftJitError::UnsupportedExpr("JIT batch seed must fit i64".to_owned())
        })?;
        let sample = i64::try_from(sample).map_err(|_| {
            CraneliftJitError::UnsupportedExpr("JIT batch sample index must fit i64".to_owned())
        })?;
        let iterations = i64::try_from(iterations).map_err(|_| {
            CraneliftJitError::UnsupportedExpr("JIT batch iterations must fit i64".to_owned())
        })?;
        Ok(native_call::call_i64_batch(
            self.code, seed, sample, iterations,
        ))
    }

    /// Returns the local binding names used as runtime parameters.
    pub fn param_names(&self) -> &[String] {
        &self.param_names
    }

    /// Returns lowering counters captured during compilation.
    pub const fn stats(&self) -> &PureFunctionStats {
        &self.stats
    }
}

fn lower_input_value(
    builder: &mut FunctionBuilder<'_>,
    seed: Value,
    sample: Value,
    iteration: Value,
    param_index: usize,
) -> Value {
    let input_index = i64::try_from(param_index + 1).unwrap_or(i64::MAX);
    let zero_based = i64::try_from(param_index).unwrap_or(i64::MAX);
    let multiplier = builder.ins().iconst(types::I64, input_index);
    let sample_scale = builder.ins().iconst(types::I64, 3 + zero_based);
    let modulus = builder.ins().iconst(
        types::I64,
        5 + i64::try_from(param_index % 5).unwrap_or_default(),
    );
    let seed_term = builder.ins().imul(seed, multiplier);
    let sample_term = builder.ins().imul(sample, sample_scale);
    let sum = builder.ins().iadd(seed_term, sample_term);
    let sum = builder.ins().iadd(sum, iteration);
    let value = builder.ins().urem(sum, modulus);
    let one = builder.ins().iconst(types::I64, 1);
    builder.ins().iadd(value, one)
}

fn lower_next_input_value(
    builder: &mut FunctionBuilder<'_>,
    current: Value,
    param_index: usize,
) -> Value {
    let modulus = builder.ins().iconst(
        types::I64,
        5 + i64::try_from(param_index % 5).unwrap_or_default(),
    );
    let one = builder.ins().iconst(types::I64, 1);
    let incremented = builder.ins().iadd(current, one);
    let wrapped = builder.ins().icmp(IntCC::Equal, current, modulus);
    builder.ins().select(wrapped, one, incremented)
}

fn validate_param_names(param_names: &[String]) -> Result<(), CraneliftJitError> {
    for (index, name) in param_names.iter().enumerate() {
        if name.is_empty() {
            return Err(CraneliftJitError::UnsupportedExpr(
                "JIT runtime input names must be non-empty".to_owned(),
            ));
        }
        if param_names[..index].contains(name) {
            return Err(CraneliftJitError::UnsupportedExpr(format!(
                "JIT runtime input `{name}` is duplicated"
            )));
        }
    }
    Ok(())
}

fn jit_module() -> Result<JITModule, CraneliftJitError> {
    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|error| CraneliftJitError::Backend(error.to_string()))?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|error| CraneliftJitError::Backend(error.to_string()))?;
    flag_builder
        .set("opt_level", "speed")
        .map_err(|error| CraneliftJitError::Backend(error.to_string()))?;
    let isa_builder = cranelift::native::builder()
        .map_err(|message| CraneliftJitError::UnsupportedHost(message.to_owned()))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|error| CraneliftJitError::Backend(error.to_string()))?;
    Ok(JITModule::new(JITBuilder::with_isa(
        isa,
        default_libcall_names(),
    )))
}

fn int_bindings(
    bindings: &[RuntimeBinding],
) -> Result<BTreeMap<String, LoweredI64Binding>, CraneliftJitError> {
    bindings
        .iter()
        .map(|binding| match binding.value {
            RuntimeValue::Int(value) => value
                .exact_i64()
                .map(|value| (binding.name.clone(), LoweredI64Binding::Const(value)))
                .ok_or_else(|| {
                    CraneliftJitError::UnsupportedExpr(format!(
                        "binding `{}` is not an i64 integer",
                        binding.name
                    ))
                }),
            _ => Err(CraneliftJitError::UnsupportedExpr(format!(
                "binding `{}` is not an i64 integer",
                binding.name
            ))),
        })
        .collect()
}

fn i32_bindings(
    bindings: &[RuntimeBinding],
) -> Result<BTreeMap<String, LoweredI64Binding>, CraneliftJitError> {
    bindings
        .iter()
        .map(|binding| match binding.value {
            RuntimeValue::Int(value) => value
                .exact_i32()
                .map(|value| {
                    (
                        binding.name.clone(),
                        LoweredI64Binding::Const(i64::from(value)),
                    )
                })
                .ok_or_else(|| {
                    CraneliftJitError::UnsupportedExpr(format!(
                        "binding `{}` is not an i32 integer",
                        binding.name
                    ))
                }),
            _ => Err(CraneliftJitError::UnsupportedExpr(format!(
                "binding `{}` is not an i32 integer",
                binding.name
            ))),
        })
        .collect()
}

fn u32_bindings(
    bindings: &[RuntimeBinding],
) -> Result<BTreeMap<String, LoweredI64Binding>, CraneliftJitError> {
    bindings
        .iter()
        .map(|binding| match binding.value {
            RuntimeValue::UInt(arcweft_core::value::RuntimeUInt::U32(value)) => Ok((
                binding.name.clone(),
                LoweredI64Binding::Const(u32_iconst_value(value)),
            )),
            _ => Err(CraneliftJitError::UnsupportedExpr(format!(
                "binding `{}` is not an u32 integer",
                binding.name
            ))),
        })
        .collect()
}

fn f32_bindings(
    bindings: &[RuntimeBinding],
) -> Result<BTreeMap<String, LoweredF32Binding>, CraneliftJitError> {
    bindings
        .iter()
        .map(|binding| match binding.value {
            RuntimeValue::F32(value) => Ok((binding.name.clone(), LoweredF32Binding::Const(value))),
            _ => Err(CraneliftJitError::UnsupportedExpr(format!(
                "binding `{}` is not an f32 value",
                binding.name
            ))),
        })
        .collect()
}

fn f64_bindings(
    bindings: &[RuntimeBinding],
) -> Result<BTreeMap<String, LoweredF64Binding>, CraneliftJitError> {
    bindings
        .iter()
        .map(|binding| match binding.value {
            RuntimeValue::F64(value) => Ok((binding.name.clone(), LoweredF64Binding::Const(value))),
            _ => Err(CraneliftJitError::UnsupportedExpr(format!(
                "binding `{}` is not an f64 value",
                binding.name
            ))),
        })
        .collect()
}

fn lower_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredI64Binding>,
    expr: &RuntimeExpr,
    stats: &mut arcweft_core::pure::PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Int(value)) => value
            .exact_i64()
            .map(|value| builder.ins().iconst(types::I64, value))
            .ok_or_else(|| {
                CraneliftJitError::UnsupportedExpr(format!(
                    "literal `{value}` is not an i64 integer"
                ))
            }),
        RuntimeExpr::Value(value) => Err(CraneliftJitError::UnsupportedExpr(format!(
            "literal {value:?} is not an i64 integer"
        ))),
        RuntimeExpr::Local(name) => match bindings.get(name) {
            Some(LoweredI64Binding::Const(value)) => Ok(builder.ins().iconst(types::I64, *value)),
            Some(LoweredI64Binding::Value(value)) => Ok(*value),
            None => Err(CraneliftJitError::UnsupportedExpr(format!(
                "unknown integer binding `{name}`"
            ))),
        },
        RuntimeExpr::Let { name, expr, body } => {
            let value = lower_expr(builder, bindings, expr, stats)?;
            let mut scoped_bindings = bindings.clone();
            scoped_bindings.insert(name.clone(), LoweredI64Binding::Value(value));
            lower_expr(builder, &scoped_bindings, body, stats)
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Neg,
            expr,
        } => {
            let value = lower_expr(builder, bindings, expr, stats)?;
            Ok(builder.ins().ineg(value))
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Not,
            ..
        } => Err(CraneliftJitError::UnsupportedExpr(
            "boolean negation is not an i64 result".to_owned(),
        )),
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let lhs = lower_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_expr(builder, bindings, rhs, stats)?;
            match op {
                RuntimeBinaryOp::Add => Ok(builder.ins().iadd(lhs, rhs)),
                RuntimeBinaryOp::Sub => Ok(builder.ins().isub(lhs, rhs)),
                RuntimeBinaryOp::Mul => Ok(builder.ins().imul(lhs, rhs)),
                RuntimeBinaryOp::Div => Ok(builder.ins().sdiv(lhs, rhs)),
                _ => Err(CraneliftJitError::UnsupportedExpr(format!(
                    "binary operator `{op}` is outside the JIT subset"
                ))),
            }
        }
        RuntimeExpr::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::Add) && args.len() == 2 =>
        {
            stats.evaluated_calls += 1;
            let lhs = lower_expr(builder, bindings, &args[0], stats)?;
            let rhs = lower_expr(builder, bindings, &args[1], stats)?;
            Ok(builder.ins().iadd(lhs, rhs))
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => lower_if_expr(builder, bindings, condition, then_expr, else_expr, stats),
        RuntimeExpr::Call { callee, .. } => Err(CraneliftJitError::UnsupportedExpr(format!(
            "call `{callee}` is outside the JIT subset"
        ))),
        other => Err(CraneliftJitError::UnsupportedExpr(format!(
            "expression `{other}` is outside the JIT subset"
        ))),
    }
}

fn lower_i32_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredI64Binding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Int(value)) => value
            .exact_i32()
            .map(|value| builder.ins().iconst(types::I32, i64::from(value)))
            .ok_or_else(|| {
                CraneliftJitError::UnsupportedExpr(format!(
                    "literal `{value}` is not an i32 integer"
                ))
            }),
        RuntimeExpr::Value(value) => Err(CraneliftJitError::UnsupportedExpr(format!(
            "literal {value:?} is not an i32 integer"
        ))),
        RuntimeExpr::Local(name) => match bindings.get(name) {
            Some(LoweredI64Binding::Const(value)) => Ok(builder.ins().iconst(types::I32, *value)),
            Some(LoweredI64Binding::Value(value)) => Ok(*value),
            None => Err(CraneliftJitError::UnsupportedExpr(format!(
                "unknown i32 binding `{name}`"
            ))),
        },
        RuntimeExpr::Let { name, expr, body } => {
            let value = lower_i32_expr(builder, bindings, expr, stats)?;
            let mut scoped_bindings = bindings.clone();
            scoped_bindings.insert(name.clone(), LoweredI64Binding::Value(value));
            lower_i32_expr(builder, &scoped_bindings, body, stats)
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Neg,
            expr,
        } => {
            let value = lower_i32_expr(builder, bindings, expr, stats)?;
            Ok(builder.ins().ineg(value))
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Not,
            ..
        } => Err(CraneliftJitError::UnsupportedExpr(
            "boolean negation is not an i32 result".to_owned(),
        )),
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let lhs = lower_i32_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_i32_expr(builder, bindings, rhs, stats)?;
            match op {
                RuntimeBinaryOp::Add => Ok(builder.ins().iadd(lhs, rhs)),
                RuntimeBinaryOp::Sub => Ok(builder.ins().isub(lhs, rhs)),
                RuntimeBinaryOp::Mul => Ok(builder.ins().imul(lhs, rhs)),
                RuntimeBinaryOp::Div => Ok(builder.ins().sdiv(lhs, rhs)),
                _ => Err(CraneliftJitError::UnsupportedExpr(format!(
                    "binary operator `{op}` is outside the i32 JIT subset"
                ))),
            }
        }
        RuntimeExpr::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::Add) && args.len() == 2 =>
        {
            stats.evaluated_calls += 1;
            let lhs = lower_i32_expr(builder, bindings, &args[0], stats)?;
            let rhs = lower_i32_expr(builder, bindings, &args[1], stats)?;
            Ok(builder.ins().iadd(lhs, rhs))
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => lower_i32_if_expr(builder, bindings, condition, then_expr, else_expr, stats),
        RuntimeExpr::Call { callee, .. } => Err(CraneliftJitError::UnsupportedExpr(format!(
            "call `{callee}` is outside the i32 JIT subset"
        ))),
        other => Err(CraneliftJitError::UnsupportedExpr(format!(
            "expression `{other}` is outside the i32 JIT subset"
        ))),
    }
}

fn u32_iconst_value(value: u32) -> i64 {
    i64::from(i32::from_ne_bytes(value.to_ne_bytes()))
}

fn lower_u32_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredI64Binding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::UInt(arcweft_core::value::RuntimeUInt::U32(value))) => {
            Ok(builder.ins().iconst(types::I32, u32_iconst_value(*value)))
        }
        RuntimeExpr::Value(value) => Err(CraneliftJitError::UnsupportedExpr(format!(
            "literal {value:?} is not an u32 integer"
        ))),
        RuntimeExpr::Local(name) => match bindings.get(name) {
            Some(LoweredI64Binding::Const(value)) => Ok(builder.ins().iconst(types::I32, *value)),
            Some(LoweredI64Binding::Value(value)) => Ok(*value),
            None => Err(CraneliftJitError::UnsupportedExpr(format!(
                "unknown u32 binding `{name}`"
            ))),
        },
        RuntimeExpr::Let { name, expr, body } => {
            let value = lower_u32_expr(builder, bindings, expr, stats)?;
            let mut scoped_bindings = bindings.clone();
            scoped_bindings.insert(name.clone(), LoweredI64Binding::Value(value));
            lower_u32_expr(builder, &scoped_bindings, body, stats)
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Neg,
            expr,
        } => {
            let value = lower_u32_expr(builder, bindings, expr, stats)?;
            Ok(builder.ins().ineg(value))
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Not,
            ..
        } => Err(CraneliftJitError::UnsupportedExpr(
            "boolean negation is not an u32 result".to_owned(),
        )),
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let lhs = lower_u32_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_u32_expr(builder, bindings, rhs, stats)?;
            match op {
                RuntimeBinaryOp::Add => Ok(builder.ins().iadd(lhs, rhs)),
                RuntimeBinaryOp::Sub => Ok(builder.ins().isub(lhs, rhs)),
                RuntimeBinaryOp::Mul => Ok(builder.ins().imul(lhs, rhs)),
                RuntimeBinaryOp::Div => Ok(builder.ins().udiv(lhs, rhs)),
                _ => Err(CraneliftJitError::UnsupportedExpr(format!(
                    "binary operator `{op}` is outside the u32 JIT subset"
                ))),
            }
        }
        RuntimeExpr::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::Add) && args.len() == 2 =>
        {
            stats.evaluated_calls += 1;
            let lhs = lower_u32_expr(builder, bindings, &args[0], stats)?;
            let rhs = lower_u32_expr(builder, bindings, &args[1], stats)?;
            Ok(builder.ins().iadd(lhs, rhs))
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => lower_u32_if_expr(builder, bindings, condition, then_expr, else_expr, stats),
        RuntimeExpr::Call { callee, .. } => Err(CraneliftJitError::UnsupportedExpr(format!(
            "call `{callee}` is outside the u32 JIT subset"
        ))),
        other => Err(CraneliftJitError::UnsupportedExpr(format!(
            "expression `{other}` is outside the u32 JIT subset"
        ))),
    }
}

fn lower_f32_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF32Binding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::F32(value)) => Ok(builder.ins().f32const(*value)),
        RuntimeExpr::Value(value) => Err(CraneliftJitError::UnsupportedExpr(format!(
            "literal {value:?} is not an f32 value"
        ))),
        RuntimeExpr::Local(name) => match bindings.get(name) {
            Some(LoweredF32Binding::Const(value)) => Ok(builder.ins().f32const(*value)),
            Some(LoweredF32Binding::Value(value)) => Ok(*value),
            None => Err(CraneliftJitError::UnsupportedExpr(format!(
                "unknown f32 binding `{name}`"
            ))),
        },
        RuntimeExpr::Let { name, expr, body } => {
            let value = lower_f32_expr(builder, bindings, expr, stats)?;
            let mut scoped_bindings = bindings.clone();
            scoped_bindings.insert(name.clone(), LoweredF32Binding::Value(value));
            lower_f32_expr(builder, &scoped_bindings, body, stats)
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Neg,
            expr,
        } => {
            let value = lower_f32_expr(builder, bindings, expr, stats)?;
            Ok(builder.ins().fneg(value))
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Not,
            ..
        } => Err(CraneliftJitError::UnsupportedExpr(
            "boolean negation is not an f32 result".to_owned(),
        )),
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let lhs = lower_f32_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_f32_expr(builder, bindings, rhs, stats)?;
            match op {
                RuntimeBinaryOp::Add => Ok(builder.ins().fadd(lhs, rhs)),
                RuntimeBinaryOp::Sub => Ok(builder.ins().fsub(lhs, rhs)),
                RuntimeBinaryOp::Mul => Ok(builder.ins().fmul(lhs, rhs)),
                RuntimeBinaryOp::Div => Ok(builder.ins().fdiv(lhs, rhs)),
                _ => Err(CraneliftJitError::UnsupportedExpr(format!(
                    "binary operator `{op}` is outside the f32 JIT subset"
                ))),
            }
        }
        RuntimeExpr::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::Add) && args.len() == 2 =>
        {
            stats.evaluated_calls += 1;
            let lhs = lower_f32_expr(builder, bindings, &args[0], stats)?;
            let rhs = lower_f32_expr(builder, bindings, &args[1], stats)?;
            Ok(builder.ins().fadd(lhs, rhs))
        }
        RuntimeExpr::Call { callee, args } => {
            lower_f32_std_float_call(builder, bindings, callee, args, stats).ok_or_else(|| {
                CraneliftJitError::UnsupportedExpr(format!(
                    "call `{callee}` is outside the f32 JIT subset"
                ))
            })?
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => lower_f32_if_expr(builder, bindings, condition, then_expr, else_expr, stats),
        other => Err(CraneliftJitError::UnsupportedExpr(format!(
            "expression `{other}` is outside the f32 JIT subset"
        ))),
    }
}

fn lower_f64_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF64Binding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::F64(value)) => Ok(builder.ins().f64const(*value)),
        RuntimeExpr::Value(value) => Err(CraneliftJitError::UnsupportedExpr(format!(
            "literal {value:?} is not an f64 value"
        ))),
        RuntimeExpr::Local(name) => match bindings.get(name) {
            Some(LoweredF64Binding::Const(value)) => Ok(builder.ins().f64const(*value)),
            Some(LoweredF64Binding::Value(value)) => Ok(*value),
            None => Err(CraneliftJitError::UnsupportedExpr(format!(
                "unknown f64 binding `{name}`"
            ))),
        },
        RuntimeExpr::Let { name, expr, body } => {
            let value = lower_f64_expr(builder, bindings, expr, stats)?;
            let mut scoped_bindings = bindings.clone();
            scoped_bindings.insert(name.clone(), LoweredF64Binding::Value(value));
            lower_f64_expr(builder, &scoped_bindings, body, stats)
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Neg,
            expr,
        } => {
            let value = lower_f64_expr(builder, bindings, expr, stats)?;
            Ok(builder.ins().fneg(value))
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Not,
            ..
        } => Err(CraneliftJitError::UnsupportedExpr(
            "boolean negation is not an f64 result".to_owned(),
        )),
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let lhs = lower_f64_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_f64_expr(builder, bindings, rhs, stats)?;
            match op {
                RuntimeBinaryOp::Add => Ok(builder.ins().fadd(lhs, rhs)),
                RuntimeBinaryOp::Sub => Ok(builder.ins().fsub(lhs, rhs)),
                RuntimeBinaryOp::Mul => Ok(builder.ins().fmul(lhs, rhs)),
                RuntimeBinaryOp::Div => Ok(builder.ins().fdiv(lhs, rhs)),
                _ => Err(CraneliftJitError::UnsupportedExpr(format!(
                    "binary operator `{op}` is outside the f64 JIT subset"
                ))),
            }
        }
        RuntimeExpr::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::Add) && args.len() == 2 =>
        {
            stats.evaluated_calls += 1;
            let lhs = lower_f64_expr(builder, bindings, &args[0], stats)?;
            let rhs = lower_f64_expr(builder, bindings, &args[1], stats)?;
            Ok(builder.ins().fadd(lhs, rhs))
        }
        RuntimeExpr::Call { callee, args } => {
            lower_f64_std_float_call(builder, bindings, callee, args, stats).ok_or_else(|| {
                CraneliftJitError::UnsupportedExpr(format!(
                    "call `{callee}` is outside the f64 JIT subset"
                ))
            })?
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => lower_f64_if_expr(builder, bindings, condition, then_expr, else_expr, stats),
        other => Err(CraneliftJitError::UnsupportedExpr(format!(
            "expression `{other}` is outside the f64 JIT subset"
        ))),
    }
}

fn lower_f32_std_float_call(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF32Binding>,
    callee: &RuntimeCallTarget,
    args: &[RuntimeExpr],
    stats: &mut PureFunctionStats,
) -> Option<Result<Value, CraneliftJitError>> {
    let intrinsic = callee.as_intrinsic()?;
    let result = match (intrinsic, args) {
        (RuntimeIntrinsic::StdF32Abs, [value]) => {
            lower_f32_expr(builder, bindings, value, stats).map(|value| builder.ins().fabs(value))
        }
        (RuntimeIntrinsic::StdF32Floor, [value]) => {
            lower_f32_expr(builder, bindings, value, stats).map(|value| builder.ins().floor(value))
        }
        (RuntimeIntrinsic::StdF32Ceil, [value]) => {
            lower_f32_expr(builder, bindings, value, stats).map(|value| builder.ins().ceil(value))
        }
        (RuntimeIntrinsic::StdF32Trunc, [value]) => {
            lower_f32_expr(builder, bindings, value, stats).map(|value| builder.ins().trunc(value))
        }
        (RuntimeIntrinsic::StdF32Fract, [value]) => lower_f32_expr(builder, bindings, value, stats)
            .map(|value| {
                let trunc = builder.ins().trunc(value);
                builder.ins().fsub(value, trunc)
            }),
        (RuntimeIntrinsic::StdF32Sqrt, [value]) => {
            lower_f32_expr(builder, bindings, value, stats).map(|value| builder.ins().sqrt(value))
        }
        (RuntimeIntrinsic::StdF32MulAdd, [a, b, c]) => (|| {
            let a = lower_f32_expr(builder, bindings, a, stats)?;
            let b = lower_f32_expr(builder, bindings, b, stats)?;
            let c = lower_f32_expr(builder, bindings, c, stats)?;
            Ok(builder.ins().fma(a, b, c))
        })(),
        _ => return None,
    };
    stats.evaluated_calls += 1;
    Some(result)
}

fn lower_f64_std_float_call(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF64Binding>,
    callee: &RuntimeCallTarget,
    args: &[RuntimeExpr],
    stats: &mut PureFunctionStats,
) -> Option<Result<Value, CraneliftJitError>> {
    let intrinsic = callee.as_intrinsic()?;
    let result = match (intrinsic, args) {
        (RuntimeIntrinsic::StdF64Abs, [value]) => {
            lower_f64_expr(builder, bindings, value, stats).map(|value| builder.ins().fabs(value))
        }
        (RuntimeIntrinsic::StdF64Floor, [value]) => {
            lower_f64_expr(builder, bindings, value, stats).map(|value| builder.ins().floor(value))
        }
        (RuntimeIntrinsic::StdF64Ceil, [value]) => {
            lower_f64_expr(builder, bindings, value, stats).map(|value| builder.ins().ceil(value))
        }
        (RuntimeIntrinsic::StdF64Trunc, [value]) => {
            lower_f64_expr(builder, bindings, value, stats).map(|value| builder.ins().trunc(value))
        }
        (RuntimeIntrinsic::StdF64Fract, [value]) => lower_f64_expr(builder, bindings, value, stats)
            .map(|value| {
                let trunc = builder.ins().trunc(value);
                builder.ins().fsub(value, trunc)
            }),
        (RuntimeIntrinsic::StdF64Sqrt, [value]) => {
            lower_f64_expr(builder, bindings, value, stats).map(|value| builder.ins().sqrt(value))
        }
        (RuntimeIntrinsic::StdF64MulAdd, [a, b, c]) => (|| {
            let a = lower_f64_expr(builder, bindings, a, stats)?;
            let b = lower_f64_expr(builder, bindings, b, stats)?;
            let c = lower_f64_expr(builder, bindings, c, stats)?;
            Ok(builder.ins().fma(a, b, c))
        })(),
        _ => return None,
    };
    stats.evaluated_calls += 1;
    Some(result)
}

fn lower_if_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredI64Binding>,
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    let condition = lower_condition(builder, bindings, condition, stats)?;
    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, types::I64);
    builder
        .ins()
        .brif(condition, then_block, &[], else_block, &[]);

    builder.switch_to_block(then_block);
    let then_value = lower_expr(builder, bindings, then_expr, stats)?;
    builder.ins().jump(merge_block, &[then_value.into()]);

    builder.switch_to_block(else_block);
    let else_value = lower_expr(builder, bindings, else_expr, stats)?;
    builder.ins().jump(merge_block, &[else_value.into()]);

    builder.switch_to_block(merge_block);
    Ok(builder.block_params(merge_block)[0])
}

fn lower_i32_if_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredI64Binding>,
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    let condition = lower_i32_condition(builder, bindings, condition, stats)?;
    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, types::I32);
    builder
        .ins()
        .brif(condition, then_block, &[], else_block, &[]);

    builder.switch_to_block(then_block);
    let then_value = lower_i32_expr(builder, bindings, then_expr, stats)?;
    builder.ins().jump(merge_block, &[then_value.into()]);

    builder.switch_to_block(else_block);
    let else_value = lower_i32_expr(builder, bindings, else_expr, stats)?;
    builder.ins().jump(merge_block, &[else_value.into()]);

    builder.switch_to_block(merge_block);
    Ok(builder.block_params(merge_block)[0])
}

fn lower_u32_if_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredI64Binding>,
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    let condition = lower_u32_condition(builder, bindings, condition, stats)?;
    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, types::I32);
    builder
        .ins()
        .brif(condition, then_block, &[], else_block, &[]);

    builder.switch_to_block(then_block);
    let then_value = lower_u32_expr(builder, bindings, then_expr, stats)?;
    builder.ins().jump(merge_block, &[then_value.into()]);

    builder.switch_to_block(else_block);
    let else_value = lower_u32_expr(builder, bindings, else_expr, stats)?;
    builder.ins().jump(merge_block, &[else_value.into()]);

    builder.switch_to_block(merge_block);
    Ok(builder.block_params(merge_block)[0])
}

fn lower_f32_if_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF32Binding>,
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    let condition = lower_f32_condition(builder, bindings, condition, stats)?;
    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, types::F32);
    builder
        .ins()
        .brif(condition, then_block, &[], else_block, &[]);

    builder.switch_to_block(then_block);
    let then_value = lower_f32_expr(builder, bindings, then_expr, stats)?;
    builder.ins().jump(merge_block, &[then_value.into()]);

    builder.switch_to_block(else_block);
    let else_value = lower_f32_expr(builder, bindings, else_expr, stats)?;
    builder.ins().jump(merge_block, &[else_value.into()]);

    builder.switch_to_block(merge_block);
    Ok(builder.block_params(merge_block)[0])
}

fn lower_f64_if_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF64Binding>,
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    let condition = lower_f64_condition(builder, bindings, condition, stats)?;
    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, types::F64);
    builder
        .ins()
        .brif(condition, then_block, &[], else_block, &[]);

    builder.switch_to_block(then_block);
    let then_value = lower_f64_expr(builder, bindings, then_expr, stats)?;
    builder.ins().jump(merge_block, &[then_value.into()]);

    builder.switch_to_block(else_block);
    let else_value = lower_f64_expr(builder, bindings, else_expr, stats)?;
    builder.ins().jump(merge_block, &[else_value.into()]);

    builder.switch_to_block(merge_block);
    Ok(builder.block_params(merge_block)[0])
}

fn lower_condition(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredI64Binding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => {
            Ok(builder.ins().iconst(types::I8, i64::from(*value)))
        }
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let Some(condition) = int_condition(*op) else {
                return Err(CraneliftJitError::UnsupportedExpr(format!(
                    "condition operator `{op}` is outside the JIT subset"
                )));
            };
            let lhs = lower_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_expr(builder, bindings, rhs, stats)?;
            Ok(builder.ins().icmp(condition, lhs, rhs))
        }
        other => Err(CraneliftJitError::UnsupportedExpr(format!(
            "condition `{other}` is outside the JIT subset"
        ))),
    }
}

fn lower_i32_condition(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredI64Binding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => {
            Ok(builder.ins().iconst(types::I8, i64::from(*value)))
        }
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let Some(condition) = int_condition(*op) else {
                return Err(CraneliftJitError::UnsupportedExpr(format!(
                    "condition operator `{op}` is outside the i32 JIT subset"
                )));
            };
            let lhs = lower_i32_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_i32_expr(builder, bindings, rhs, stats)?;
            Ok(builder.ins().icmp(condition, lhs, rhs))
        }
        other => Err(CraneliftJitError::UnsupportedExpr(format!(
            "condition `{other}` is outside the i32 JIT subset"
        ))),
    }
}

fn lower_u32_condition(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredI64Binding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => {
            Ok(builder.ins().iconst(types::I8, i64::from(*value)))
        }
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let Some(condition) = unsigned_int_condition(*op) else {
                return Err(CraneliftJitError::UnsupportedExpr(format!(
                    "condition operator `{op}` is outside the u32 JIT subset"
                )));
            };
            let lhs = lower_u32_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_u32_expr(builder, bindings, rhs, stats)?;
            Ok(builder.ins().icmp(condition, lhs, rhs))
        }
        other => Err(CraneliftJitError::UnsupportedExpr(format!(
            "condition `{other}` is outside the u32 JIT subset"
        ))),
    }
}

fn lower_f32_condition(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF32Binding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => {
            Ok(builder.ins().iconst(types::I8, i64::from(*value)))
        }
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let Some(condition) = float_condition(*op) else {
                return Err(CraneliftJitError::UnsupportedExpr(format!(
                    "condition operator `{op}` is outside the f32 JIT subset"
                )));
            };
            let lhs = lower_f32_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_f32_expr(builder, bindings, rhs, stats)?;
            Ok(builder.ins().fcmp(condition, lhs, rhs))
        }
        other => Err(CraneliftJitError::UnsupportedExpr(format!(
            "condition `{other}` is outside the f32 JIT subset"
        ))),
    }
}

fn lower_f64_condition(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF64Binding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftJitError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => {
            Ok(builder.ins().iconst(types::I8, i64::from(*value)))
        }
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let Some(condition) = float_condition(*op) else {
                return Err(CraneliftJitError::UnsupportedExpr(format!(
                    "condition operator `{op}` is outside the f64 JIT subset"
                )));
            };
            let lhs = lower_f64_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_f64_expr(builder, bindings, rhs, stats)?;
            Ok(builder.ins().fcmp(condition, lhs, rhs))
        }
        other => Err(CraneliftJitError::UnsupportedExpr(format!(
            "condition `{other}` is outside the f64 JIT subset"
        ))),
    }
}

fn int_condition(op: RuntimeBinaryOp) -> Option<IntCC> {
    match op {
        RuntimeBinaryOp::Eq => Some(IntCC::Equal),
        RuntimeBinaryOp::Ne => Some(IntCC::NotEqual),
        RuntimeBinaryOp::Lt => Some(IntCC::SignedLessThan),
        RuntimeBinaryOp::Le => Some(IntCC::SignedLessThanOrEqual),
        RuntimeBinaryOp::Gt => Some(IntCC::SignedGreaterThan),
        RuntimeBinaryOp::Ge => Some(IntCC::SignedGreaterThanOrEqual),
        RuntimeBinaryOp::Add
        | RuntimeBinaryOp::Sub
        | RuntimeBinaryOp::Mul
        | RuntimeBinaryOp::Div
        | RuntimeBinaryOp::And
        | RuntimeBinaryOp::Or => None,
    }
}

fn unsigned_int_condition(op: RuntimeBinaryOp) -> Option<IntCC> {
    match op {
        RuntimeBinaryOp::Eq => Some(IntCC::Equal),
        RuntimeBinaryOp::Ne => Some(IntCC::NotEqual),
        RuntimeBinaryOp::Lt => Some(IntCC::UnsignedLessThan),
        RuntimeBinaryOp::Le => Some(IntCC::UnsignedLessThanOrEqual),
        RuntimeBinaryOp::Gt => Some(IntCC::UnsignedGreaterThan),
        RuntimeBinaryOp::Ge => Some(IntCC::UnsignedGreaterThanOrEqual),
        RuntimeBinaryOp::Add
        | RuntimeBinaryOp::Sub
        | RuntimeBinaryOp::Mul
        | RuntimeBinaryOp::Div
        | RuntimeBinaryOp::And
        | RuntimeBinaryOp::Or => None,
    }
}

fn float_condition(op: RuntimeBinaryOp) -> Option<FloatCC> {
    match op {
        RuntimeBinaryOp::Eq => Some(FloatCC::Equal),
        RuntimeBinaryOp::Ne => Some(FloatCC::NotEqual),
        RuntimeBinaryOp::Lt => Some(FloatCC::LessThan),
        RuntimeBinaryOp::Le => Some(FloatCC::LessThanOrEqual),
        RuntimeBinaryOp::Gt => Some(FloatCC::GreaterThan),
        RuntimeBinaryOp::Ge => Some(FloatCC::GreaterThanOrEqual),
        RuntimeBinaryOp::Add
        | RuntimeBinaryOp::Sub
        | RuntimeBinaryOp::Mul
        | RuntimeBinaryOp::Div
        | RuntimeBinaryOp::And
        | RuntimeBinaryOp::Or => None,
    }
}

fn jit_error(error: ModuleError) -> CraneliftJitError {
    CraneliftJitError::Backend(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::pure::{
        PureFunctionBackendKind, VmPureFunctionBackend, compare_pure_function_backend,
    };
    use arcweft_core::value::RuntimeCallTarget;

    fn int_binding(name: &str, value: i64) -> RuntimeBinding {
        RuntimeBinding {
            name: name.to_owned(),
            value: RuntimeValue::i64(value),
        }
    }

    fn i32_binding(name: &str, value: i32) -> RuntimeBinding {
        RuntimeBinding {
            name: name.to_owned(),
            value: RuntimeValue::i32(value),
        }
    }

    fn u32_binding(name: &str, value: u32) -> RuntimeBinding {
        RuntimeBinding {
            name: name.to_owned(),
            value: RuntimeValue::u32(value),
        }
    }

    fn f32_binding(name: &str, value: f32) -> RuntimeBinding {
        RuntimeBinding {
            name: name.to_owned(),
            value: RuntimeValue::F32(value),
        }
    }

    fn f64_binding(name: &str, value: f64) -> RuntimeBinding {
        RuntimeBinding {
            name: name.to_owned(),
            value: RuntimeValue::F64(value),
        }
    }

    #[test]
    fn cranelift_jit_evaluates_integer_helper_and_matches_vm() {
        let request = PureFunctionRequest::new(
            "score",
            RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Call {
                    callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                    args: vec![
                        RuntimeExpr::Local("bonus".to_owned()),
                        RuntimeExpr::Value(RuntimeValue::i64(2)),
                    ],
                }),
            },
            [int_binding("base", 3), int_binding("bonus", 4)],
        );

        let conformance = compare_pure_function_backend(
            &VmPureFunctionBackend,
            &CraneliftPureFunctionBackend,
            &request,
        )
        .expect("Cranelift JIT matches VM for supported pure integer helper");

        assert!(conformance.matches_vm);
        assert_eq!(conformance.candidate.backend, PureFunctionBackendKind::Jit);
        assert_eq!(conformance.candidate.value, RuntimeValue::i64(18));
        assert_eq!(conformance.candidate.stats.evaluated_calls, 1);
        assert_eq!(conformance.candidate.stats.evaluated_binary_ops, 1);
    }

    #[test]
    fn cranelift_compiled_helper_can_be_called_repeatedly() {
        let request = PureFunctionRequest::new(
            "score",
            RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(21))),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(21))),
            },
            [],
        );

        let compiled = CraneliftPureFunctionBackend
            .compile_i64(&request)
            .expect("Cranelift compiles integer helper");

        assert_eq!(compiled.call(), 42);
        assert_eq!(compiled.call(), 42);
        assert_eq!(compiled.stats().evaluated_binary_ops, 1);
    }

    #[test]
    fn cranelift_compiled_helper_accepts_runtime_integer_inputs() {
        let request = PureFunctionRequest::new(
            "score_inputs",
            RuntimeExpr::If {
                condition: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Ge,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(3))),
                }),
                then_expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Mul,
                    rhs: Box::new(RuntimeExpr::Call {
                        callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                        args: vec![
                            RuntimeExpr::Local("bonus".to_owned()),
                            RuntimeExpr::Value(RuntimeValue::i64(2)),
                        ],
                    }),
                }),
                else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::i64(0))),
            },
            [int_binding("base", 3), int_binding("bonus", 4)],
        );

        let compiled = CraneliftPureFunctionBackend
            .compile_i64_with_inputs(&request, ["base", "bonus"])
            .expect("Cranelift compiles parameterized integer helper");

        assert_eq!(compiled.param_names(), ["base", "bonus"]);
        assert_eq!(compiled.call(&[3, 4]).expect("call succeeds"), 18);
        assert_eq!(
            compiled
                .call_i64_args(RuntimeI64Args::new([3, 4, 0, 0], 2))
                .expect("packed call succeeds"),
            18
        );
        assert_eq!(compiled.call(&[2, 99]).expect("call succeeds"), 0);
        assert_eq!(compiled.call(&[7, 1]).expect("call succeeds"), 21);
        let mut out = [0; 3];
        compiled
            .call_flat_batch(&[3, 4, 2, 99, 7, 1], &mut out)
            .expect("flat rows batch succeeds");
        assert_eq!(out, [18, 0, 21]);
        assert_eq!(
            compiled
                .call_flat_batch_sum(&[3, 4, 2, 99, 7, 1], 3)
                .expect("flat rows batch sum succeeds"),
            39
        );
    }

    #[test]
    fn cranelift_compiled_helper_accepts_runtime_i32_inputs_without_widening() {
        let request = PureFunctionRequest::new(
            "score_i32",
            RuntimeExpr::If {
                condition: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Ge,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i32(3))),
                }),
                then_expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Mul,
                    rhs: Box::new(RuntimeExpr::Binary {
                        lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                        op: RuntimeBinaryOp::Add,
                        rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i32(2))),
                    }),
                }),
                else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::i32(0))),
            },
            [i32_binding("base", 0), i32_binding("bonus", 0)],
        );

        let compiled = CraneliftPureFunctionBackend
            .compile_i32_with_inputs(&request, ["base", "bonus"])
            .expect("Cranelift compiles parameterized i32 helper");

        assert_eq!(compiled.call(&[3, 4]).expect("i32 call succeeds"), 18);
        assert_eq!(compiled.call(&[2, 99]).expect("i32 call succeeds"), 0);
        let mut out = [0; 3];
        compiled
            .call_flat_batch(&[3, 4, 2, 99, 7, 1], &mut out)
            .expect("i32 flat rows batch succeeds");
        assert_eq!(out, [18, 0, 21]);
        assert_eq!(
            compiled
                .call_flat_batch_sum(&[3, 4, 2, 99, 7, 1], 3)
                .expect("i32 flat rows batch sum succeeds"),
            39
        );
    }

    #[test]
    fn cranelift_compiled_helper_accepts_runtime_u32_inputs_without_widening() {
        let request = PureFunctionRequest::new(
            "score_u32",
            RuntimeExpr::If {
                condition: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Ge,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u32(u32::MAX - 4))),
                }),
                then_expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Div,
                    rhs: Box::new(RuntimeExpr::Binary {
                        lhs: Box::new(RuntimeExpr::Local("divisor".to_owned())),
                        op: RuntimeBinaryOp::Add,
                        rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u32(1))),
                    }),
                }),
                else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::u32(0))),
            },
            [u32_binding("base", 0), u32_binding("divisor", 0)],
        );

        let compiled = CraneliftPureFunctionBackend
            .compile_u32_with_inputs(&request, ["base", "divisor"])
            .expect("Cranelift compiles parameterized u32 helper");

        assert_eq!(
            compiled
                .call(&[u32::MAX - 1, 1])
                .expect("u32 call succeeds"),
            (u32::MAX - 1) / 2
        );
        assert_eq!(compiled.call(&[3, 99]).expect("u32 call succeeds"), 0);
        let mut out = [0; 3];
        compiled
            .call_flat_batch(&[u32::MAX - 1, 1, 3, 99, u32::MAX, 4], &mut out)
            .expect("u32 flat rows batch succeeds");
        assert_eq!(out, [(u32::MAX - 1) / 2, 0, u32::MAX / 5]);
        assert_eq!(
            compiled
                .call_flat_batch_sum(&[u32::MAX - 1, 1, 3, 99, u32::MAX, 4], 3)
                .expect("u32 flat rows batch sum succeeds"),
            i64::from((u32::MAX - 1) / 2) + i64::from(u32::MAX / 5)
        );
    }

    #[test]
    fn cranelift_compiled_helper_accepts_runtime_f32_inputs_without_value_boundary() {
        let request = PureFunctionRequest::new(
            "score_f32",
            RuntimeExpr::If {
                condition: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Gt,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F32(2.0))),
                }),
                then_expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Mul,
                    rhs: Box::new(RuntimeExpr::Binary {
                        lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                        op: RuntimeBinaryOp::Add,
                        rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F32(0.5))),
                    }),
                }),
                else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::F32(0.0))),
            },
            [f32_binding("base", 0.0), f32_binding("scale", 0.0)],
        );

        let compiled = CraneliftPureFunctionBackend
            .compile_f32_with_inputs(&request, ["base", "scale"])
            .expect("Cranelift compiles parameterized f32 helper");

        assert_eq!(compiled.call(&[3.0, 1.5]).expect("f32 call succeeds"), 6.0);
        assert_eq!(compiled.call(&[2.0, 99.0]).expect("f32 call succeeds"), 0.0);
        let mut out = [0.0; 3];
        compiled
            .call_flat_batch(&[3.0, 1.5, 2.0, 99.0, 4.0, 0.5], &mut out)
            .expect("f32 flat rows batch succeeds");
        assert_eq!(out, [6.0, 0.0, 4.0]);
    }

    #[test]
    fn cranelift_compiled_helper_accepts_runtime_f64_inputs_without_value_boundary() {
        let request = PureFunctionRequest::new(
            "score_f64",
            RuntimeExpr::If {
                condition: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Gt,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F64(2.0))),
                }),
                then_expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Mul,
                    rhs: Box::new(RuntimeExpr::Binary {
                        lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                        op: RuntimeBinaryOp::Add,
                        rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F64(0.5))),
                    }),
                }),
                else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::F64(0.0))),
            },
            [f64_binding("base", 0.0), f64_binding("scale", 0.0)],
        );

        let compiled = CraneliftPureFunctionBackend
            .compile_f64_with_inputs(&request, ["base", "scale"])
            .expect("Cranelift compiles parameterized f64 helper");

        assert_eq!(compiled.call(&[3.0, 1.5]).expect("f64 call succeeds"), 6.0);
        assert_eq!(compiled.call(&[2.0, 99.0]).expect("f64 call succeeds"), 0.0);
        let mut out = [0.0; 3];
        compiled
            .call_flat_batch(&[3.0, 1.5, 2.0, 99.0, 4.0, 0.5], &mut out)
            .expect("f64 flat rows batch succeeds");
        assert_eq!(out.map(f64::to_bits), [6.0f64, 0.0, 4.0].map(f64::to_bits));
    }

    #[test]
    fn cranelift_compiled_helper_lowers_supported_std_f32_intrinsics() {
        let request = PureFunctionRequest::new(
            "std_f32_intrinsics",
            RuntimeExpr::Call {
                callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF32MulAdd),
                args: vec![
                    RuntimeExpr::Call {
                        callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF32Sqrt),
                        args: vec![RuntimeExpr::Local("base".to_owned())],
                    },
                    RuntimeExpr::Call {
                        callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF32Abs),
                        args: vec![RuntimeExpr::Local("scale".to_owned())],
                    },
                    RuntimeExpr::Call {
                        callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF32Fract),
                        args: vec![RuntimeExpr::Local("offset".to_owned())],
                    },
                ],
            },
            [
                f32_binding("base", 0.0),
                f32_binding("scale", 0.0),
                f32_binding("offset", 0.0),
            ],
        );

        let compiled = CraneliftPureFunctionBackend
            .compile_f32_with_inputs(&request, ["base", "scale", "offset"])
            .expect("Cranelift compiles supported std.f32 intrinsics");

        assert_eq!(
            compiled
                .call(&[9.0, -2.0, 1.25])
                .expect("f32 call succeeds")
                .to_bits(),
            6.25f32.to_bits()
        );
        let mut out = [0.0; 2];
        compiled
            .call_flat_batch(&[9.0, -2.0, 1.25, 16.0, -0.5, 2.75], &mut out)
            .expect("f32 flat rows batch succeeds");
        assert_eq!(out.map(f32::to_bits), [6.25f32, 2.75].map(f32::to_bits));
    }

    #[test]
    fn cranelift_compiled_helper_lowers_supported_std_f64_intrinsics() {
        let request = PureFunctionRequest::new(
            "std_f64_intrinsics",
            RuntimeExpr::Call {
                callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF64MulAdd),
                args: vec![
                    RuntimeExpr::Call {
                        callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF64Sqrt),
                        args: vec![RuntimeExpr::Local("base".to_owned())],
                    },
                    RuntimeExpr::Call {
                        callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF64Ceil),
                        args: vec![RuntimeExpr::Local("scale".to_owned())],
                    },
                    RuntimeExpr::Call {
                        callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::StdF64Fract),
                        args: vec![RuntimeExpr::Local("offset".to_owned())],
                    },
                ],
            },
            [
                f64_binding("base", 0.0),
                f64_binding("scale", 0.0),
                f64_binding("offset", 0.0),
            ],
        );

        let compiled = CraneliftPureFunctionBackend
            .compile_f64_with_inputs(&request, ["base", "scale", "offset"])
            .expect("Cranelift compiles supported std.f64 intrinsics");

        assert_eq!(
            compiled
                .call(&[25.0, 1.2, 3.5])
                .expect("f64 call succeeds")
                .to_bits(),
            10.5f64.to_bits()
        );
        let mut out = [0.0; 2];
        compiled
            .call_flat_batch(&[25.0, 1.2, 3.5, 16.0, 2.0, 7.25], &mut out)
            .expect("f64 flat rows batch succeeds");
        assert_eq!(out.map(f64::to_bits), [10.5f64, 8.25].map(f64::to_bits));
    }

    #[test]
    fn cranelift_compiled_batch_matches_repeated_input_calls() {
        let request = PureFunctionRequest::new(
            "score_inputs",
            RuntimeExpr::If {
                condition: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Ge,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(3))),
                }),
                then_expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Mul,
                    rhs: Box::new(RuntimeExpr::Call {
                        callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                        args: vec![
                            RuntimeExpr::Local("bonus".to_owned()),
                            RuntimeExpr::Value(RuntimeValue::i64(2)),
                        ],
                    }),
                }),
                else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::i64(0))),
            },
            [int_binding("base", 3), int_binding("bonus", 4)],
        );

        let backend = CraneliftPureFunctionBackend;
        let compiled = backend
            .compile_i64_with_inputs(&request, ["base", "bonus"])
            .expect("Cranelift compiles parameterized integer helper");
        let batch = backend
            .compile_i64_batch(&request, ["base", "bonus"])
            .expect("Cranelift compiles batch helper");
        let expected = (0..8)
            .map(|iteration| {
                let base = i64::from((7 + iteration) % 5) + 1;
                let bonus = i64::from((14 + iteration) % 6) + 1;
                compiled.call(&[base, bonus]).expect("call succeeds")
            })
            .sum::<i64>();

        assert_eq!(batch.param_names(), ["base", "bonus"]);
        assert_eq!(batch.call(7, 0, 8).expect("batch call succeeds"), expected);
        assert_eq!(batch.stats().evaluated_binary_ops, 2);
    }

    #[test]
    fn cranelift_compiled_helper_evaluates_lexical_let() {
        let request = PureFunctionRequest::new(
            "score_with_local",
            RuntimeExpr::Let {
                name: "boosted".to_owned(),
                expr: Box::new(RuntimeExpr::Call {
                    callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                    args: vec![
                        RuntimeExpr::Local("bonus".to_owned()),
                        RuntimeExpr::Value(RuntimeValue::i64(2)),
                    ],
                }),
                body: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Mul,
                    rhs: Box::new(RuntimeExpr::Local("boosted".to_owned())),
                }),
            },
            [int_binding("base", 0), int_binding("bonus", 0)],
        );

        let compiled = CraneliftPureFunctionBackend
            .compile_i64_with_inputs(&request, ["base", "bonus"])
            .expect("Cranelift compiles lexical let");

        assert_eq!(compiled.call(&[3, 4]).expect("call succeeds"), 18);
        assert_eq!(compiled.call(&[5, 1]).expect("call succeeds"), 15);
    }

    #[test]
    fn cranelift_compiled_helper_accepts_four_runtime_integer_inputs() {
        let request = PureFunctionRequest::new(
            "sum4",
            RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("a".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Local("b".to_owned())),
                }),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("c".to_owned())),
                    op: RuntimeBinaryOp::Sub,
                    rhs: Box::new(RuntimeExpr::Local("d".to_owned())),
                }),
            },
            [
                int_binding("a", 0),
                int_binding("b", 0),
                int_binding("c", 0),
                int_binding("d", 0),
            ],
        );

        let compiled = CraneliftPureFunctionBackend
            .compile_i64_with_inputs(&request, ["a", "b", "c", "d"])
            .expect("Cranelift compiles four-input integer helper");

        assert_eq!(compiled.call(&[2, 3, 10, 4]).expect("call succeeds"), 30);
    }

    #[test]
    fn cranelift_compiled_helper_evaluates_division_and_negation() {
        let request = PureFunctionRequest::new(
            "normalized_delta",
            RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Unary {
                    op: RuntimeUnaryOp::Neg,
                    expr: Box::new(RuntimeExpr::Binary {
                        lhs: Box::new(RuntimeExpr::Local("score".to_owned())),
                        op: RuntimeBinaryOp::Sub,
                        rhs: Box::new(RuntimeExpr::Local("baseline".to_owned())),
                    }),
                }),
                op: RuntimeBinaryOp::Div,
                rhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
            },
            [
                int_binding("score", 0),
                int_binding("baseline", 0),
                int_binding("scale", 1),
            ],
        );

        let compiled = CraneliftPureFunctionBackend
            .compile_i64_with_inputs(&request, ["score", "baseline", "scale"])
            .expect("Cranelift compiles i64 div and unary negation");

        assert_eq!(compiled.call(&[21, 9, 3]).expect("call succeeds"), -4);
        assert_eq!(compiled.call(&[8, 20, 4]).expect("call succeeds"), 3);
        assert_eq!(compiled.stats().evaluated_binary_ops, 2);
    }

    #[test]
    fn cranelift_jit_evaluates_integer_if_and_matches_vm() {
        let request = PureFunctionRequest::new(
            "score_branch",
            RuntimeExpr::If {
                condition: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("score".to_owned())),
                    op: RuntimeBinaryOp::Ge,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(10))),
                }),
                then_expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("score".to_owned())),
                    op: RuntimeBinaryOp::Mul,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
                }),
                else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::i64(0))),
            },
            [int_binding("score", 12)],
        );

        let conformance = compare_pure_function_backend(
            &VmPureFunctionBackend,
            &CraneliftPureFunctionBackend,
            &request,
        )
        .expect("Cranelift JIT matches VM for integer if helper");

        assert!(conformance.matches_vm);
        assert_eq!(conformance.candidate.value, RuntimeValue::i64(24));
    }

    #[test]
    fn cranelift_jit_rejects_non_integer_helpers() {
        let request = PureFunctionRequest::new(
            "trim_label",
            RuntimeExpr::Value(RuntimeValue::String("x".to_owned())),
            [],
        );

        let error = CraneliftPureFunctionBackend
            .evaluate_jit(&request)
            .expect_err("string-heavy helpers are outside the JIT subset");

        assert!(matches!(error, CraneliftJitError::UnsupportedExpr(_)));
    }

    #[test]
    fn cranelift_jit_unsupported_expr_uses_display_label() {
        let request = PureFunctionRequest::new("tuple_value", RuntimeExpr::Tuple(vec![]), []);

        let error = CraneliftPureFunctionBackend
            .evaluate_jit(&request)
            .expect_err("tuple helpers are outside the current JIT subset")
            .to_string();

        assert!(error.contains("expression `tuple/0` is outside the JIT subset"));
        assert!(!error.contains("RuntimeExpr"));
    }

    #[test]
    fn cranelift_jit_unsupported_operator_uses_display_label() {
        let request = PureFunctionRequest::new(
            "bool_and",
            RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(1))),
                op: RuntimeBinaryOp::And,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(1))),
            },
            [],
        );

        let error = CraneliftPureFunctionBackend
            .evaluate_jit(&request)
            .expect_err("boolean operators are outside the i64 JIT subset")
            .to_string();

        assert!(error.contains("binary operator `&&` is outside the JIT subset"));
    }
}
