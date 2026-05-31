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
    RuntimeBinaryOp, RuntimeBinding, RuntimeEvalError, RuntimeExpr, RuntimeIntrinsic,
    RuntimeUnaryOp, RuntimeValue,
};
use cranelift::codegen::ir::{BlockArg, MemFlags, UserFuncName};
use cranelift::jit::{JITBuilder, JITModule};
use cranelift::module::{FuncId, Linkage, Module, ModuleError, default_libcall_names};
use cranelift::prelude::{
    AbiParam, Configurable, FunctionBuilder, FunctionBuilderContext, InstBuilder, IntCC, Value,
    settings, types,
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

fn store_batch_output(builder: &mut FunctionBuilder<'_>, out_ptr: Value, row: Value, value: Value) {
    let value_bytes = i64::try_from(std::mem::size_of::<i64>()).unwrap_or(i64::MAX);
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
