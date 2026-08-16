use super::lower::{
    codegen_error, jit_module, lower_expr, lower_f32_expr, lower_f64_expr, lower_i32_expr,
    lower_small_int_expr, lower_u32_expr, lower_u64_expr, small_int_bindings,
    validate_input_locals,
};
use super::{
    AbiParam, BTreeMap, BlockArg, CraneliftCodegenError, DefinedPureSmallIntBatchInputs,
    DefinedPureSmallIntInputs, FuncId, FunctionBuilder, FunctionBuilderContext, InstBuilder, IntCC,
    Linkage, LoweredF32Binding, LoweredF64Binding, LoweredIntBinding, LoweredSmallIntBinding,
    MemFlags, Module, PureFunctionRequest, PureFunctionStats, RuntimeExpr,
    RuntimeLocalDeclarationId, SmallIntCompiledParts, SmallIntKind, UserFuncName, Value,
    WideIntBatchCompiledParts, request_helper, types,
};

pub(super) fn small_int_arity_error(kind: SmallIntKind, arity: usize) -> CraneliftCodegenError {
    CraneliftCodegenError::UnsupportedExpr(format!(
        "JIT {} helper arity {arity} is outside the native call boundary",
        kind.label()
    ))
}

pub(super) fn compile_small_int_with_inputs(
    request: &PureFunctionRequest,
    input_locals: impl IntoIterator<Item = RuntimeLocalDeclarationId>,
    kind: SmallIntKind,
) -> Result<SmallIntCompiledParts, CraneliftCodegenError> {
    let mut module = jit_module()?;
    let defined = define_small_int_with_inputs(
        &mut module,
        &format!("arcweft_pure_{}_helper_inputs", kind.label()),
        request,
        input_locals,
        kind,
    )?;
    module.finalize_definitions().map_err(codegen_error)?;
    let code = module.get_finalized_function(defined.entry);
    let batch_code = module.get_finalized_function(defined.batch);
    let batch_sum_code = module.get_finalized_function(defined.batch_sum);
    Ok(SmallIntCompiledParts {
        module,
        code,
        batch_code,
        batch_sum_code,
        input_locals: defined.input_locals,
        stats: defined.stats,
    })
}

pub(super) fn compile_wide_int_batch_with_inputs(
    request: &PureFunctionRequest,
    input_locals: impl IntoIterator<Item = RuntimeLocalDeclarationId>,
    kind: SmallIntKind,
) -> Result<WideIntBatchCompiledParts, CraneliftCodegenError> {
    debug_assert!(matches!(kind, SmallIntKind::I128 | SmallIntKind::U128));
    let mut module = jit_module()?;
    let defined = define_small_int_batch_with_inputs(
        &mut module,
        &format!("arcweft_pure_{}_helper_inputs", kind.label()),
        request,
        input_locals,
        kind,
    )?;
    module.finalize_definitions().map_err(codegen_error)?;
    let batch_code = module.get_finalized_function(defined.batch);
    let batch_sum_code = module.get_finalized_function(defined.batch_sum);
    Ok(WideIntBatchCompiledParts {
        module,
        batch_code,
        batch_sum_code,
        input_locals: defined.input_locals,
        stats: defined.stats,
    })
}

pub(super) fn define_small_int_with_inputs<M>(
    module: &mut M,
    symbol_prefix: &str,
    request: &PureFunctionRequest,
    input_locals: impl IntoIterator<Item = RuntimeLocalDeclarationId>,
    kind: SmallIntKind,
) -> Result<DefinedPureSmallIntInputs, CraneliftCodegenError>
where
    M: Module,
{
    debug_assert!(!matches!(kind, SmallIntKind::I128 | SmallIntKind::U128));
    let input_locals = input_locals
        .into_iter()
        .collect::<Vec<RuntimeLocalDeclarationId>>();
    validate_input_locals(&input_locals)?;
    if input_locals.len() > 4 {
        return Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "Cranelift {} helper supports at most 4 runtime inputs, got {}",
            kind.label(),
            input_locals.len()
        )));
    }

    let ty = kind.cranelift_type();
    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature
        .params
        .extend(input_locals.iter().map(|_| AbiParam::new(ty)));
    signature.returns.push(AbiParam::new(ty));

    let entry_name = format!("{symbol_prefix}_entry");
    let entry = module
        .declare_function(&entry_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, entry.as_u32());

    let captured_bindings = small_int_bindings(request.bindings(), kind)?;
    let mut bindings = captured_bindings.clone();
    let mut stats = PureFunctionStats::default();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let block = builder.create_block();
        builder.append_block_params_for_function_params(block);
        builder.switch_to_block(block);
        let params = builder.block_params(block);
        for (name, value) in input_locals.iter().zip(params.iter().copied()) {
            bindings.insert(*name, LoweredSmallIntBinding::Value(value));
        }
        let value = lower_small_int_expr(
            &mut builder,
            &bindings,
            &request_helper(request)?.expr,
            &mut stats,
            kind,
        )?;
        builder.ins().return_(&[value]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(entry, &mut ctx)
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    let batch = define_small_int_rows_batch_function(
        module,
        &format!("{symbol_prefix}_rows_batch"),
        &request_helper(request)?.expr,
        &captured_bindings,
        &input_locals,
        kind,
    )?;
    let batch_sum = define_small_int_rows_batch_sum_function(
        module,
        &format!("{symbol_prefix}_rows_batch_sum"),
        &request_helper(request)?.expr,
        &captured_bindings,
        &input_locals,
        kind,
    )?;

    Ok(DefinedPureSmallIntInputs {
        entry,
        batch,
        batch_sum,
        input_locals,
        stats,
    })
}

pub(super) fn define_small_int_batch_with_inputs<M>(
    module: &mut M,
    symbol_prefix: &str,
    request: &PureFunctionRequest,
    input_locals: impl IntoIterator<Item = RuntimeLocalDeclarationId>,
    kind: SmallIntKind,
) -> Result<DefinedPureSmallIntBatchInputs, CraneliftCodegenError>
where
    M: Module,
{
    debug_assert!(matches!(kind, SmallIntKind::I128 | SmallIntKind::U128));
    let input_locals = input_locals
        .into_iter()
        .collect::<Vec<RuntimeLocalDeclarationId>>();
    validate_input_locals(&input_locals)?;
    if input_locals.len() > 4 {
        return Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "Cranelift {} helper supports at most 4 runtime inputs, got {}",
            kind.label(),
            input_locals.len()
        )));
    }

    let captured_bindings = small_int_bindings(request.bindings(), kind)?;
    let batch = define_small_int_rows_batch_function(
        module,
        &format!("{symbol_prefix}_rows_batch"),
        &request_helper(request)?.expr,
        &captured_bindings,
        &input_locals,
        kind,
    )?;
    let batch_sum = define_small_int_rows_batch_sum_function(
        module,
        &format!("{symbol_prefix}_rows_batch_sum"),
        &request_helper(request)?.expr,
        &captured_bindings,
        &input_locals,
        kind,
    )?;

    Ok(DefinedPureSmallIntBatchInputs {
        batch,
        batch_sum,
        input_locals,
        stats: PureFunctionStats::default(),
    })
}

pub(super) fn define_small_int_rows_batch_function<M>(
    module: &mut M,
    symbol_name: &str,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<RuntimeLocalDeclarationId, LoweredSmallIntBinding>,
    input_locals: &[RuntimeLocalDeclarationId],
    kind: SmallIntKind,
) -> Result<FuncId, CraneliftCodegenError>
where
    M: Module,
{
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftCodegenError::UnsupportedHost(format!(
            "Cranelift {} rows batch codegen currently requires a 64-bit host pointer type",
            kind.label()
        )));
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
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    let mut stats = PureFunctionStats::default();
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
        for (param_index, name) in input_locals.iter().enumerate() {
            let value = load_small_int_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                input_locals.len(),
                param_index,
                kind,
            );
            bindings.insert(*name, LoweredSmallIntBinding::Value(value));
        }
        let value = lower_small_int_expr(&mut builder, &bindings, expr, &mut stats, kind)?;
        store_small_int_batch_output(&mut builder, out_ptr, row, value, kind);
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
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

pub(super) fn define_small_int_rows_batch_sum_function<M>(
    module: &mut M,
    symbol_name: &str,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<RuntimeLocalDeclarationId, LoweredSmallIntBinding>,
    input_locals: &[RuntimeLocalDeclarationId],
    kind: SmallIntKind,
) -> Result<FuncId, CraneliftCodegenError>
where
    M: Module,
{
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftCodegenError::UnsupportedHost(format!(
            "Cranelift {} rows batch sum codegen currently requires a 64-bit host pointer type",
            kind.label()
        )));
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut signature = module.make_signature();
    signature
        .params
        .extend([AbiParam::new(pointer_type), AbiParam::new(types::I64)]);
    signature.returns.push(AbiParam::new(types::I64));

    let func_id = module
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
    ctx.func.signature = signature;
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    let mut stats = PureFunctionStats::default();
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
        for (param_index, name) in input_locals.iter().enumerate() {
            let value = load_small_int_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                input_locals.len(),
                param_index,
                kind,
            );
            bindings.insert(*name, LoweredSmallIntBinding::Value(value));
        }
        let value = lower_small_int_expr(&mut builder, &bindings, expr, &mut stats, kind)?;
        let value = if kind.cranelift_type().bits() > 64 {
            builder.ins().ireduce(types::I64, value)
        } else if kind.signed() {
            builder.ins().sextend(types::I64, value)
        } else {
            builder.ins().uextend(types::I64, value)
        };
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
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

pub(super) fn define_i64_rows_batch_function<M>(
    module: &mut M,
    symbol_name: &str,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<RuntimeLocalDeclarationId, LoweredIntBinding>,
    input_locals: &[RuntimeLocalDeclarationId],
) -> Result<FuncId, CraneliftCodegenError>
where
    M: Module,
{
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftCodegenError::UnsupportedHost(
            "Cranelift i64 rows batch codegen currently requires a 64-bit host pointer type"
                .to_owned(),
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
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
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
        for (param_index, name) in input_locals.iter().enumerate() {
            let value = load_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                input_locals.len(),
                param_index,
            );
            bindings.insert(*name, LoweredIntBinding::Value(value));
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
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

pub(super) fn define_i64_rows_batch_sum_function<M>(
    module: &mut M,
    symbol_name: &str,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<RuntimeLocalDeclarationId, LoweredIntBinding>,
    input_locals: &[RuntimeLocalDeclarationId],
) -> Result<FuncId, CraneliftCodegenError>
where
    M: Module,
{
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftCodegenError::UnsupportedHost(
            "Cranelift i64 rows batch sum codegen currently requires a 64-bit host pointer type"
                .to_owned(),
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
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
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
        for (param_index, name) in input_locals.iter().enumerate() {
            let value = load_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                input_locals.len(),
                param_index,
            );
            bindings.insert(*name, LoweredIntBinding::Value(value));
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
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

pub(super) fn define_i32_rows_batch_function<M>(
    module: &mut M,
    symbol_name: &str,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<RuntimeLocalDeclarationId, LoweredIntBinding>,
    input_locals: &[RuntimeLocalDeclarationId],
) -> Result<FuncId, CraneliftCodegenError>
where
    M: Module,
{
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftCodegenError::UnsupportedHost(
            "Cranelift i32 rows batch codegen currently requires a 64-bit host pointer type"
                .to_owned(),
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
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
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
        for (param_index, name) in input_locals.iter().enumerate() {
            let value = load_i32_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                input_locals.len(),
                param_index,
            );
            bindings.insert(*name, LoweredIntBinding::Value(value));
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
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

pub(super) fn define_i32_rows_batch_sum_function<M>(
    module: &mut M,
    symbol_name: &str,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<RuntimeLocalDeclarationId, LoweredIntBinding>,
    input_locals: &[RuntimeLocalDeclarationId],
) -> Result<FuncId, CraneliftCodegenError>
where
    M: Module,
{
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftCodegenError::UnsupportedHost(
            "Cranelift i32 rows batch sum codegen currently requires a 64-bit host pointer type"
                .to_owned(),
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
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
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
        for (param_index, name) in input_locals.iter().enumerate() {
            let value = load_i32_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                input_locals.len(),
                param_index,
            );
            bindings.insert(*name, LoweredIntBinding::Value(value));
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
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

pub(super) fn define_u32_rows_batch_function<M>(
    module: &mut M,
    symbol_name: &str,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<RuntimeLocalDeclarationId, LoweredIntBinding>,
    input_locals: &[RuntimeLocalDeclarationId],
) -> Result<FuncId, CraneliftCodegenError>
where
    M: Module,
{
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftCodegenError::UnsupportedHost(
            "Cranelift u32 rows batch codegen currently requires a 64-bit host pointer type"
                .to_owned(),
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
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
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
        for (param_index, name) in input_locals.iter().enumerate() {
            let value = load_u32_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                input_locals.len(),
                param_index,
            );
            bindings.insert(*name, LoweredIntBinding::Value(value));
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
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

pub(super) fn define_u32_rows_batch_sum_function<M>(
    module: &mut M,
    symbol_name: &str,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<RuntimeLocalDeclarationId, LoweredIntBinding>,
    input_locals: &[RuntimeLocalDeclarationId],
) -> Result<FuncId, CraneliftCodegenError>
where
    M: Module,
{
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftCodegenError::UnsupportedHost(
            "Cranelift u32 rows batch sum codegen currently requires a 64-bit host pointer type"
                .to_owned(),
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
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
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
        for (param_index, name) in input_locals.iter().enumerate() {
            let value = load_u32_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                input_locals.len(),
                param_index,
            );
            bindings.insert(*name, LoweredIntBinding::Value(value));
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
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

pub(super) fn define_u64_rows_batch_function<M>(
    module: &mut M,
    symbol_name: &str,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<RuntimeLocalDeclarationId, LoweredIntBinding>,
    input_locals: &[RuntimeLocalDeclarationId],
) -> Result<FuncId, CraneliftCodegenError>
where
    M: Module,
{
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftCodegenError::UnsupportedHost(
            "Cranelift u64 rows batch codegen currently requires a 64-bit host pointer type"
                .to_owned(),
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
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
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
        for (param_index, name) in input_locals.iter().enumerate() {
            let value = load_u64_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                input_locals.len(),
                param_index,
            );
            bindings.insert(*name, LoweredIntBinding::Value(value));
        }
        let value = lower_u64_expr(&mut builder, &bindings, expr, &mut stats)?;
        store_u64_batch_output(&mut builder, out_ptr, row, value);
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
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

pub(super) fn define_u64_rows_batch_sum_function<M>(
    module: &mut M,
    symbol_name: &str,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<RuntimeLocalDeclarationId, LoweredIntBinding>,
    input_locals: &[RuntimeLocalDeclarationId],
) -> Result<FuncId, CraneliftCodegenError>
where
    M: Module,
{
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftCodegenError::UnsupportedHost(
            "Cranelift u64 rows batch sum codegen currently requires a 64-bit host pointer type"
                .to_owned(),
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
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
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
        for (param_index, name) in input_locals.iter().enumerate() {
            let value = load_u64_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                input_locals.len(),
                param_index,
            );
            bindings.insert(*name, LoweredIntBinding::Value(value));
        }
        let value = lower_u64_expr(&mut builder, &bindings, expr, &mut stats)?;
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
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

pub(super) fn define_f32_rows_batch_function<M>(
    module: &mut M,
    symbol_name: &str,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<RuntimeLocalDeclarationId, LoweredF32Binding>,
    input_locals: &[RuntimeLocalDeclarationId],
) -> Result<FuncId, CraneliftCodegenError>
where
    M: Module,
{
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftCodegenError::UnsupportedHost(
            "Cranelift f32 rows batch codegen currently requires a 64-bit host pointer type"
                .to_owned(),
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
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
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
        for (param_index, name) in input_locals.iter().enumerate() {
            let value = load_f32_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                input_locals.len(),
                param_index,
            );
            bindings.insert(*name, LoweredF32Binding::Value(value));
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
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

pub(super) fn define_f64_rows_batch_function<M>(
    module: &mut M,
    symbol_name: &str,
    expr: &RuntimeExpr,
    captured_bindings: &BTreeMap<RuntimeLocalDeclarationId, LoweredF64Binding>,
    input_locals: &[RuntimeLocalDeclarationId],
) -> Result<FuncId, CraneliftCodegenError>
where
    M: Module,
{
    let pointer_type = module.target_config().pointer_type();
    if pointer_type != types::I64 {
        return Err(CraneliftCodegenError::UnsupportedHost(
            "Cranelift f64 rows batch codegen currently requires a 64-bit host pointer type"
                .to_owned(),
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
        .declare_function(symbol_name, Linkage::Local, &signature)
        .map_err(codegen_error)?;
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
        for (param_index, name) in input_locals.iter().enumerate() {
            let value = load_f64_batch_input(
                &mut builder,
                inputs_ptr,
                row,
                input_locals.len(),
                param_index,
            );
            bindings.insert(*name, LoweredF64Binding::Value(value));
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
        .map_err(codegen_error)?;
    module.clear_context(&mut ctx);
    Ok(func_id)
}

pub(super) fn load_batch_input(
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

pub(super) fn load_i32_batch_input(
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

pub(super) fn load_small_int_batch_input(
    builder: &mut FunctionBuilder<'_>,
    inputs_ptr: Value,
    row: Value,
    arity: usize,
    param_index: usize,
    kind: SmallIntKind,
) -> Value {
    let stride_bytes = i64::try_from(arity.saturating_mul(kind.byte_width())).unwrap_or(i64::MAX);
    let row_stride = builder.ins().iconst(types::I64, stride_bytes);
    let row_offset = builder.ins().imul(row, row_stride);
    let param_offset =
        i64::try_from(param_index.saturating_mul(kind.byte_width())).unwrap_or(i64::MAX);
    let byte_offset = if param_offset == 0 {
        row_offset
    } else {
        let param_offset = builder.ins().iconst(types::I64, param_offset);
        builder.ins().iadd(row_offset, param_offset)
    };
    let address = builder.ins().iadd(inputs_ptr, byte_offset);
    builder
        .ins()
        .load(kind.cranelift_type(), MemFlags::trusted(), address, 0)
}

pub(super) fn load_u32_batch_input(
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

pub(super) fn load_u64_batch_input(
    builder: &mut FunctionBuilder<'_>,
    inputs_ptr: Value,
    row: Value,
    arity: usize,
    param_index: usize,
) -> Value {
    load_batch_input(builder, inputs_ptr, row, arity, param_index)
}

pub(super) fn load_f32_batch_input(
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

pub(super) fn load_f64_batch_input(
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

pub(super) fn store_batch_output(
    builder: &mut FunctionBuilder<'_>,
    out_ptr: Value,
    row: Value,
    value: Value,
) {
    let value_bytes = i64::try_from(std::mem::size_of::<i64>()).unwrap_or(i64::MAX);
    let stride = builder.ins().iconst(types::I64, value_bytes);
    let byte_offset = builder.ins().imul(row, stride);
    let address = builder.ins().iadd(out_ptr, byte_offset);
    builder.ins().store(MemFlags::trusted(), value, address, 0);
}

pub(super) fn store_f32_batch_output(
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

pub(super) fn store_f64_batch_output(
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

pub(super) fn store_i32_batch_output(
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

pub(super) fn store_u32_batch_output(
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

pub(super) fn store_u64_batch_output(
    builder: &mut FunctionBuilder<'_>,
    out_ptr: Value,
    row: Value,
    value: Value,
) {
    store_batch_output(builder, out_ptr, row, value);
}

pub(super) fn store_small_int_batch_output(
    builder: &mut FunctionBuilder<'_>,
    out_ptr: Value,
    row: Value,
    value: Value,
    kind: SmallIntKind,
) {
    let value_bytes = i64::try_from(kind.byte_width()).unwrap_or(i64::MAX);
    let stride = builder.ins().iconst(types::I64, value_bytes);
    let byte_offset = builder.ins().imul(row, stride);
    let address = builder.ins().iadd(out_ptr, byte_offset);
    builder.ins().store(MemFlags::trusted(), value, address, 0);
}
