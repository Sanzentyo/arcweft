use super::{
    AUTO_EAGER_JIT_WORK_UNITS, AUTO_JIT_FLAT_BATCH_WORK_UNITS, AotPureFunctionBackend,
    AotPureI64Plan, CompiledPureI64Inputs, CraneliftPureFunctionBackend, DenseSeq,
    FlatBatchSumPolicy, FlatBatchSumShape, IndexedParallelIterator, IntoParallelIterator,
    IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator, ParallelSlice,
    PureFunctionRequest, RuntimeBinding, RuntimeEvalError, RuntimeExpr, RuntimeFixedArgs,
    RuntimeI64Args, RuntimePureAotPlan, RuntimePureBackendMode, RuntimePureCacheEntry,
    RuntimePureCallStats, RuntimePureCompileStats, RuntimePureHelper, RuntimePureHelperId,
    RuntimePureInputType, RuntimePureNativeKind, RuntimePureOutputType, RuntimePureScalar,
    RuntimePureScalarInteger, RuntimePureWorkerCount, RuntimeSeq, RuntimeValue, ThreadPool,
    ThreadPoolBuilder, VmPureFunctionScratch, helper_native_kind, native_jit, native_jit_enabled,
};

pub(super) fn runtime_value_kind(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "()",
        RuntimeValue::Bool(_) => "bool",
        RuntimeValue::Int(_) => "int",
        RuntimeValue::UInt(_) => "uint",
        RuntimeValue::F32(_) => "f32",
        RuntimeValue::F64(_) => "f64",
        RuntimeValue::MatrixF32(_) => "matrix_f32",
        RuntimeValue::MatrixF64(_) => "matrix_f64",
        RuntimeValue::TensorF32(_) => "tensor_f32",
        RuntimeValue::TensorF64(_) => "tensor_f64",
        RuntimeValue::String(_) => "string",
        RuntimeValue::Char(_) => "char",
        RuntimeValue::Duration(_) => "duration",
        RuntimeValue::EntityRef(_) => "entity_ref",
        RuntimeValue::Tuple(_) => "tuple",
        RuntimeValue::Seq(RuntimeSeq::Values(_)) => "seq_values",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Units(_))) => "seq_units",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I8(_))) => "seq_i8",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I16(_))) => "seq_i16",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I32(_))) => "seq_i32",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I64(_))) => "seq_i64",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I128(_))) => "seq_i128",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::ISize(_))) => "seq_isize",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U8(_))) => "seq_u8",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U16(_))) => "seq_u16",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U32(_))) => "seq_u32",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U64(_))) => "seq_u64",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U128(_))) => "seq_u128",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::USize(_))) => "seq_usize",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::F32(_))) => "seq_f32",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::F64(_))) => "seq_f64",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bool(_))) => "seq_bool",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bytes(_))) => "seq_bytes",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Chars(_))) => "seq_chars",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Durations(_))) => "seq_durations",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Strings(_))) => "seq_strings",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::EntityRefs(_))) => "seq_entity_refs",
        RuntimeValue::Seq(RuntimeSeq::TupleColumns(_)) => "seq_tuple_columns",
        RuntimeValue::Seq(RuntimeSeq::RecordColumns(_)) => "seq_record_columns",
        RuntimeValue::Record(_) => "record",
        RuntimeValue::Variant { .. } => "variant",
    }
    .to_owned()
}

pub(super) fn compile_helper(
    mode: RuntimePureBackendMode,
    helper: &RuntimePureHelper,
    work_units: usize,
    stats: &mut RuntimePureCompileStats,
) -> RuntimePureCacheEntry {
    if mode == RuntimePureBackendMode::Vm {
        return RuntimePureCacheEntry::Vm;
    }
    if helper_native_kind(helper) == Some(RuntimePureNativeKind::I64) {
        let request = compile_request(helper, || RuntimeValue::i64(0));
        return match mode {
            RuntimePureBackendMode::Vm => RuntimePureCacheEntry::Vm,
            RuntimePureBackendMode::Aot => {
                compile_aot_i64(&request, helper, stats).unwrap_or(RuntimePureCacheEntry::Vm)
            }
            RuntimePureBackendMode::Jit => {
                compile_jit(&request, helper, stats).unwrap_or(RuntimePureCacheEntry::Vm)
            }
            RuntimePureBackendMode::Auto => compile_auto(&request, helper, work_units, stats),
        };
    }
    let Some(input_type) = helper_scalar_aot_input_type(helper) else {
        if mode == RuntimePureBackendMode::Auto {
            stats.auto_vm_selected += 1;
        }
        return RuntimePureCacheEntry::Vm;
    };
    let output_type = helper.output_type;
    let zero = scalar_zero_for_input(input_type);
    let request = compile_request(helper, zero);
    match mode {
        RuntimePureBackendMode::Vm => RuntimePureCacheEntry::Vm,
        RuntimePureBackendMode::Aot => {
            compile_aot_scalar(&request, helper, input_type, output_type, stats)
                .unwrap_or(RuntimePureCacheEntry::Vm)
        }
        RuntimePureBackendMode::Jit => {
            if let Some(kind) = helper_native_kind(helper) {
                compile_native_jit(kind, &request, helper, stats).unwrap_or_else(|| {
                    compile_aot_scalar(&request, helper, input_type, output_type, stats)
                        .unwrap_or(RuntimePureCacheEntry::Vm)
                })
            } else {
                stats.jit_attempts = stats.jit_attempts.saturating_add(1);
                stats.jit_failures = stats.jit_failures.saturating_add(1);
                compile_aot_scalar(&request, helper, input_type, output_type, stats)
                    .unwrap_or(RuntimePureCacheEntry::Vm)
            }
        }
        RuntimePureBackendMode::Auto => {
            compile_auto_scalar(&request, helper, input_type, output_type, stats)
        }
    }
}

pub(super) fn exact_i64_result(value: RuntimeValue) -> Result<i64, RuntimeEvalError> {
    match value {
        RuntimeValue::Int(value) => value
            .exact_i64()
            .ok_or_else(|| RuntimeEvalError::ExpectedInt(value.to_string())),
        value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
    }
}

pub(super) fn validate_flat_batch_shape(
    helper: &RuntimePureHelper,
    flat_input_len: usize,
    arity: usize,
    rows: usize,
) -> Result<(), RuntimeEvalError> {
    if arity > RuntimeI64Args::MAX {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: helper.name.clone(),
            max: RuntimeI64Args::MAX,
            found: arity,
        });
    }
    let expected = rows.saturating_mul(arity);
    if flat_input_len != expected {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure flat batch expected {expected} input value(s), got {flat_input_len}"
            ),
        });
    }
    Ok(())
}

pub(super) fn validate_exact_int_flat_batch_shape<T: RuntimePureScalarInteger>(
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
    if arity != helper.input_names.len() {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure batch arity expected {} input value(s), got {arity}",
                helper.input_names.len()
            ),
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

pub(super) fn validate_exact_int_slice_shape<T: RuntimePureScalarInteger>(
    helper: &RuntimePureHelper,
    arg_len: usize,
) -> Result<(), RuntimeEvalError> {
    if arg_len > RuntimeFixedArgs::<T>::MAX {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: helper.name.clone(),
            max: RuntimeFixedArgs::<T>::MAX,
            found: arg_len,
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
            reason: "exact integer slice type does not match helper signature".to_owned(),
        });
    }
    if arg_len != helper.input_names.len() {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure slice expected {} input value(s), got {arg_len}",
                helper.input_names.len()
            ),
        });
    }
    Ok(())
}

pub(super) fn validate_float_flat_batch_shape(
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

pub(super) fn compile_auto(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    work_units: usize,
    stats: &mut RuntimePureCompileStats,
) -> RuntimePureCacheEntry {
    if native_jit_enabled() && work_units >= AUTO_EAGER_JIT_WORK_UNITS {
        stats.auto_jit_selected += 1;
        return compile_jit(request, helper, stats)
            .or_else(|| compile_aot_i64(request, helper, stats))
            .unwrap_or_else(|| {
                stats.auto_vm_selected += 1;
                RuntimePureCacheEntry::Vm
            });
    }
    match compile_aot_i64(request, helper, stats) {
        Some(RuntimePureCacheEntry::Aot(aot)) => {
            stats.auto_aot_selected += 1;
            stats.auto_jit_deferred += 1;
            RuntimePureCacheEntry::AutoAot {
                aot,
                jit: None,
                jit_failed: false,
            }
        }
        Some(_) => unreachable!("compile_aot only returns AOT entries"),
        None => {
            stats.auto_vm_selected += 1;
            RuntimePureCacheEntry::Vm
        }
    }
}

pub(super) fn helper_has_native_jit_entry(helper: &RuntimePureHelper) -> bool {
    helper_native_kind(helper).is_some()
}

pub(super) fn auto_jit_flat_batch_threshold(helper: &RuntimePureHelper, rows: usize) -> usize {
    if helper_has_native_jit_entry(helper) && rows >= 64 {
        0
    } else {
        AUTO_JIT_FLAT_BATCH_WORK_UNITS.max(1)
    }
}

pub(super) fn finish_native_jit_compile<T>(
    compiled: Option<T>,
    stats: &mut RuntimePureCompileStats,
    make_entry: impl FnOnce(T) -> RuntimePureCacheEntry,
) -> Option<RuntimePureCacheEntry> {
    stats.jit_attempts += 1;
    if compiled.is_some() {
        stats.jit_successes += 1;
    } else {
        stats.jit_failures += 1;
    }
    compiled.map(make_entry)
}

pub(super) fn compile_native_jit(
    kind: RuntimePureNativeKind,
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    if helper_native_kind(helper) != Some(kind) {
        return None;
    }
    if !native_jit_enabled() {
        stats.jit_attempts += 1;
        stats.jit_failures += 1;
        return None;
    }

    let input_names = || helper.input_names.iter().map(String::as_str);
    compile_signed_native_jit(kind, request, input_names(), stats)
        .or_else(|| compile_unsigned_native_jit(kind, request, input_names(), stats))
        .or_else(|| compile_float_native_jit(kind, request, input_names(), stats))
}

pub(super) fn compile_signed_native_jit<'a>(
    kind: RuntimePureNativeKind,
    request: &PureFunctionRequest,
    input_names: impl IntoIterator<Item = &'a str>,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    match kind {
        RuntimePureNativeKind::I8 => finish_native_jit_compile(
            CraneliftPureFunctionBackend
                .compile_i8_with_inputs(request, input_names)
                .ok(),
            stats,
            |compiled| RuntimePureCacheEntry::JitI8(Box::new(compiled)),
        ),
        RuntimePureNativeKind::I16 => finish_native_jit_compile(
            CraneliftPureFunctionBackend
                .compile_i16_with_inputs(request, input_names)
                .ok(),
            stats,
            |compiled| RuntimePureCacheEntry::JitI16(Box::new(compiled)),
        ),
        RuntimePureNativeKind::I32 => finish_native_jit_compile(
            CraneliftPureFunctionBackend
                .compile_i32_with_inputs(request, input_names)
                .ok(),
            stats,
            |compiled| RuntimePureCacheEntry::JitI32(Box::new(compiled)),
        ),
        RuntimePureNativeKind::I64 | RuntimePureNativeKind::ISize => finish_native_jit_compile(
            CraneliftPureFunctionBackend
                .compile_i64_with_inputs(request, input_names)
                .ok(),
            stats,
            |compiled| match kind {
                RuntimePureNativeKind::I64 => RuntimePureCacheEntry::Jit(Box::new(compiled)),
                RuntimePureNativeKind::ISize => RuntimePureCacheEntry::JitISize(Box::new(compiled)),
                _ => unreachable!("i64-compatible native JIT arm received {kind:?}"),
            },
        ),
        RuntimePureNativeKind::I128 => finish_native_jit_compile(
            CraneliftPureFunctionBackend
                .compile_i128_batch_with_inputs(request, input_names)
                .ok(),
            stats,
            |compiled| RuntimePureCacheEntry::JitI128Batch(Box::new(compiled)),
        ),
        _ => None,
    }
}

pub(super) fn compile_unsigned_native_jit<'a>(
    kind: RuntimePureNativeKind,
    request: &PureFunctionRequest,
    input_names: impl IntoIterator<Item = &'a str>,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    match kind {
        RuntimePureNativeKind::U8 => finish_native_jit_compile(
            CraneliftPureFunctionBackend
                .compile_u8_with_inputs(request, input_names)
                .ok(),
            stats,
            |compiled| RuntimePureCacheEntry::JitU8(Box::new(compiled)),
        ),
        RuntimePureNativeKind::U16 => finish_native_jit_compile(
            CraneliftPureFunctionBackend
                .compile_u16_with_inputs(request, input_names)
                .ok(),
            stats,
            |compiled| RuntimePureCacheEntry::JitU16(Box::new(compiled)),
        ),
        RuntimePureNativeKind::U32 => finish_native_jit_compile(
            CraneliftPureFunctionBackend
                .compile_u32_with_inputs(request, input_names)
                .ok(),
            stats,
            |compiled| RuntimePureCacheEntry::JitU32(Box::new(compiled)),
        ),
        RuntimePureNativeKind::U64 | RuntimePureNativeKind::USize => finish_native_jit_compile(
            CraneliftPureFunctionBackend
                .compile_u64_with_inputs(request, input_names)
                .ok(),
            stats,
            |compiled| match kind {
                RuntimePureNativeKind::U64 => RuntimePureCacheEntry::JitU64(Box::new(compiled)),
                RuntimePureNativeKind::USize => RuntimePureCacheEntry::JitUSize(Box::new(compiled)),
                _ => unreachable!("u64-compatible native JIT arm received {kind:?}"),
            },
        ),
        RuntimePureNativeKind::U128 => finish_native_jit_compile(
            CraneliftPureFunctionBackend
                .compile_u128_batch_with_inputs(request, input_names)
                .ok(),
            stats,
            |compiled| RuntimePureCacheEntry::JitU128Batch(Box::new(compiled)),
        ),
        _ => None,
    }
}

pub(super) fn compile_float_native_jit<'a>(
    kind: RuntimePureNativeKind,
    request: &PureFunctionRequest,
    input_names: impl IntoIterator<Item = &'a str>,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    match kind {
        RuntimePureNativeKind::F32 => finish_native_jit_compile(
            CraneliftPureFunctionBackend
                .compile_f32_with_inputs(request, input_names)
                .ok(),
            stats,
            |compiled| RuntimePureCacheEntry::JitF32(Box::new(compiled)),
        ),
        RuntimePureNativeKind::F64 => finish_native_jit_compile(
            CraneliftPureFunctionBackend
                .compile_f64_with_inputs(request, input_names)
                .ok(),
            stats,
            |compiled| RuntimePureCacheEntry::JitF64(Box::new(compiled)),
        ),
        _ => None,
    }
}

pub(super) fn compile_jit(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    compile_native_jit(RuntimePureNativeKind::I64, request, helper, stats)
}

pub(super) fn compile_auto_scalar(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    stats: &mut RuntimePureCompileStats,
) -> RuntimePureCacheEntry {
    match compile_aot_scalar(request, helper, input_type, output_type, stats) {
        Some(RuntimePureCacheEntry::Aot(aot)) => {
            stats.auto_aot_selected += 1;
            let native_jit_candidate = native_jit_enabled() && helper_has_native_jit_entry(helper);
            if native_jit_candidate {
                stats.auto_jit_deferred += 1;
            }
            RuntimePureCacheEntry::AutoAot {
                aot,
                jit: None,
                jit_failed: !native_jit_candidate,
            }
        }
        Some(_) => unreachable!("compile_aot_scalar only returns AOT entries"),
        None => {
            stats.auto_vm_selected += 1;
            RuntimePureCacheEntry::Vm
        }
    }
}

pub(super) fn compile_aot_i64(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    stats.aot_attempts += 1;
    let compiled = (helper_native_kind(helper) == Some(RuntimePureNativeKind::I64))
        .then(|| {
            AotPureFunctionBackend::new()
                .compile_i64_with_inputs(request, helper.input_names.iter().map(String::as_str))
                .map(RuntimePureAotPlan::I64)
                .ok()
        })
        .flatten();
    if compiled.is_some() {
        stats.aot_successes += 1;
    } else {
        stats.aot_failures += 1;
    }
    compiled.map(RuntimePureCacheEntry::Aot)
}

pub(super) fn compile_aot_scalar(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    stats.aot_attempts += 1;
    let compiled = AotPureFunctionBackend::new()
        .compile_scalar_with_inputs(
            request,
            helper.input_names.iter().map(String::as_str),
            input_type,
            output_type,
        )
        .map(RuntimePureAotPlan::Scalar)
        .ok();
    if compiled.is_some() {
        stats.aot_successes += 1;
    } else {
        stats.aot_failures += 1;
    }
    compiled.map(RuntimePureCacheEntry::Aot)
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
pub(super) fn record_aot_object_artifact_bundle(
    helpers: &[RuntimePureHelper],
    cache: &[Option<RuntimePureCacheEntry>],
    stats: &mut RuntimePureCompileStats,
) {
    use native_jit::PureObjectBundleRequest;

    let prepared = helpers
        .iter()
        .filter(|helper| {
            cache
                .get(helper.id.0)
                .and_then(Option::as_ref)
                .is_some_and(RuntimePureCacheEntry::uses_aot_plan)
        })
        .filter_map(|helper| {
            let kind = helper_native_kind(helper)?;
            let request = compile_request(helper, || kind.zero_value());
            Some((kind, helper, request))
        })
        .collect::<Vec<_>>();

    if prepared.is_empty() {
        return;
    }

    stats.object_attempts = stats.object_attempts.saturating_add(prepared.len());
    let requests = prepared.iter().map(|(kind, helper, request)| {
        PureObjectBundleRequest::new(
            request,
            kind.object_input_kind(),
            helper.input_names.iter().map(String::as_str),
        )
    });
    match CraneliftPureFunctionBackend.emit_object_bundle(requests) {
        Ok(bundle) => {
            stats.object_successes = stats.object_successes.saturating_add(bundle.helpers.len());
            stats.object_failures = stats
                .object_failures
                .saturating_add(prepared.len().saturating_sub(bundle.helpers.len()));
            stats.object_bytes = stats.object_bytes.saturating_add(bundle.object_bytes.len());
        }
        Err(_) => {
            stats.object_failures = stats.object_failures.saturating_add(prepared.len());
        }
    }
}

#[cfg(not(all(feature = "native-jit", not(target_arch = "wasm32"))))]
pub(super) fn record_aot_object_artifact_bundle(
    _helpers: &[RuntimePureHelper],
    _cache: &[Option<RuntimePureCacheEntry>],
    _stats: &mut RuntimePureCompileStats,
) {
}

pub(super) fn helper_scalar_aot_input_type(
    helper: &RuntimePureHelper,
) -> Option<RuntimePureInputType> {
    if helper.input_names.len() != helper.input_types.len()
        || matches!(helper.output_type, RuntimePureOutputType::Value)
    {
        return None;
    }
    let expected = scalar_input_type_for_output(helper.output_type)?;
    helper
        .input_types
        .iter()
        .all(|input| *input == expected)
        .then_some(expected)
}

const fn scalar_input_type_for_output(
    output: RuntimePureOutputType,
) -> Option<RuntimePureInputType> {
    match output {
        RuntimePureOutputType::I8 => Some(RuntimePureInputType::I8),
        RuntimePureOutputType::I16 => Some(RuntimePureInputType::I16),
        RuntimePureOutputType::I32 => Some(RuntimePureInputType::I32),
        RuntimePureOutputType::I64 => Some(RuntimePureInputType::I64),
        RuntimePureOutputType::I128 => Some(RuntimePureInputType::I128),
        RuntimePureOutputType::ISize => Some(RuntimePureInputType::ISize),
        RuntimePureOutputType::U8 => Some(RuntimePureInputType::U8),
        RuntimePureOutputType::U16 => Some(RuntimePureInputType::U16),
        RuntimePureOutputType::U32 => Some(RuntimePureInputType::U32),
        RuntimePureOutputType::U64 => Some(RuntimePureInputType::U64),
        RuntimePureOutputType::U128 => Some(RuntimePureInputType::U128),
        RuntimePureOutputType::USize => Some(RuntimePureInputType::USize),
        RuntimePureOutputType::F32 => Some(RuntimePureInputType::F32),
        RuntimePureOutputType::F64 => Some(RuntimePureInputType::F64),
        RuntimePureOutputType::Bool | RuntimePureOutputType::Value => None,
    }
}

const fn scalar_zero_for_input(input: RuntimePureInputType) -> fn() -> RuntimeValue {
    match input {
        RuntimePureInputType::I8 => zero_i8,
        RuntimePureInputType::I16 => zero_i16,
        RuntimePureInputType::I32 => zero_i32,
        RuntimePureInputType::I64 => zero_i64,
        RuntimePureInputType::I128 => zero_i128,
        RuntimePureInputType::ISize => zero_isize,
        RuntimePureInputType::U8 => zero_u8,
        RuntimePureInputType::U16 => zero_u16,
        RuntimePureInputType::U32 => zero_u32,
        RuntimePureInputType::U64 => zero_u64,
        RuntimePureInputType::U128 => zero_u128,
        RuntimePureInputType::USize => zero_usize,
        RuntimePureInputType::F32 => zero_f32,
        RuntimePureInputType::F64 => zero_f64,
        RuntimePureInputType::Value => zero_unit,
    }
}

pub(super) fn zero_unit() -> RuntimeValue {
    RuntimeValue::Unit
}

pub(super) fn zero_i8() -> RuntimeValue {
    RuntimeValue::i8(0)
}

pub(super) fn zero_i16() -> RuntimeValue {
    RuntimeValue::i16(0)
}

pub(super) fn zero_i32() -> RuntimeValue {
    RuntimeValue::i32(0)
}

pub(super) fn zero_i64() -> RuntimeValue {
    RuntimeValue::i64(0)
}

pub(super) fn zero_i128() -> RuntimeValue {
    RuntimeValue::i128(0)
}

pub(super) fn zero_isize() -> RuntimeValue {
    RuntimeValue::isize(0)
}

pub(super) fn zero_u8() -> RuntimeValue {
    RuntimeValue::u8(0)
}

pub(super) fn zero_u16() -> RuntimeValue {
    RuntimeValue::u16(0)
}

pub(super) fn zero_u32() -> RuntimeValue {
    RuntimeValue::u32(0)
}

pub(super) fn zero_u64() -> RuntimeValue {
    RuntimeValue::u64(0)
}

pub(super) fn zero_u128() -> RuntimeValue {
    RuntimeValue::u128(0)
}

pub(super) fn zero_usize() -> RuntimeValue {
    RuntimeValue::usize(0)
}

pub(super) fn zero_f32() -> RuntimeValue {
    RuntimeValue::F32(0.0)
}

pub(super) fn zero_f64() -> RuntimeValue {
    RuntimeValue::F64(0.0)
}

pub(super) fn resolve_worker_count(workers: RuntimePureWorkerCount) -> usize {
    match workers {
        RuntimePureWorkerCount::Auto => std::thread::available_parallelism()
            .ok()
            .map_or(1, std::num::NonZeroUsize::get),
        RuntimePureWorkerCount::Fixed(value) => value.max(1),
    }
}

pub(super) fn build_thread_pool(worker_count: usize) -> Option<ThreadPool> {
    (worker_count > 1)
        .then(|| {
            ThreadPoolBuilder::new()
                .num_threads(worker_count)
                .build()
                .ok()
        })
        .flatten()
}

pub(super) fn helper_cache_slots(
    helpers: &[RuntimePureHelper],
) -> Vec<Option<RuntimePureCacheEntry>> {
    let slots = helpers
        .iter()
        .map(|helper| helper.id.0)
        .max()
        .map_or(0, |max_id| max_id + 1);
    let mut cache = Vec::with_capacity(slots);
    cache.resize_with(slots, || None);
    cache
}

pub(super) fn helper_work_unit_slots(helpers: &[RuntimePureHelper]) -> Vec<usize> {
    let slots = helpers
        .iter()
        .map(|helper| helper.id.0)
        .max()
        .map_or(0, |max_id| max_id + 1);
    let mut weights = vec![0; slots];
    for helper in helpers {
        weights[helper.id.0] = runtime_expr_work_units(&helper.expr);
    }
    weights
}

pub(super) fn runtime_expr_work_units(expr: &RuntimeExpr) -> usize {
    match expr {
        RuntimeExpr::Value(_) | RuntimeExpr::Local(_) | RuntimeExpr::EntityRef(_) => 1,
        RuntimeExpr::Unary { expr, .. } => 1 + runtime_expr_work_units(expr),
        RuntimeExpr::Binary { lhs, rhs, .. } => {
            2 + runtime_expr_work_units(lhs) + runtime_expr_work_units(rhs)
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            3 + runtime_expr_work_units(condition)
                + runtime_expr_work_units(then_expr)
                + runtime_expr_work_units(else_expr)
        }
        RuntimeExpr::Let { expr, body, .. } => {
            2 + runtime_expr_work_units(expr) + runtime_expr_work_units(body)
        }
        RuntimeExpr::Tuple(items) | RuntimeExpr::BracketSeq(items) => {
            1 + items.iter().map(runtime_expr_work_units).sum::<usize>()
        }
        RuntimeExpr::RepeatSeq { value, .. } => 2 + runtime_expr_work_units(value),
        RuntimeExpr::Record(fields) => {
            1 + fields
                .iter()
                .map(|field| runtime_expr_work_units(&field.value))
                .sum::<usize>()
        }
        RuntimeExpr::Variant { payload, .. } => {
            1 + payload.as_deref().map_or(0, runtime_expr_work_units)
        }
        RuntimeExpr::SpreadArg(payload) => 1 + runtime_expr_work_units(payload),
        RuntimeExpr::Field { target, .. }
        | RuntimeExpr::ProjectTuple { target, .. }
        | RuntimeExpr::ProjectRecord { target, .. } => 1 + runtime_expr_work_units(target),
        RuntimeExpr::Call { args, .. } | RuntimeExpr::PureCall { args, .. } => {
            8 + args.iter().map(runtime_expr_work_units).sum::<usize>()
        }
        RuntimeExpr::MethodCall { receiver, args, .. } => {
            8 + runtime_expr_work_units(receiver)
                + args.iter().map(runtime_expr_work_units).sum::<usize>()
        }
        RuntimeExpr::Map { source, body, .. } => {
            8 + runtime_expr_work_units(source) + runtime_expr_work_units(body)
        }
        RuntimeExpr::Sum { source } => 4 + runtime_expr_work_units(source),
        RuntimeExpr::IfLet {
            expr,
            guard,
            then_expr,
            else_expr,
            ..
        } => {
            4 + runtime_expr_work_units(expr)
                + guard.as_deref().map_or(0, runtime_expr_work_units)
                + runtime_expr_work_units(then_expr)
                + runtime_expr_work_units(else_expr)
        }
        RuntimeExpr::Match { scrutinee, arms } => {
            6 + runtime_expr_work_units(scrutinee)
                + arms
                    .iter()
                    .map(|arm| {
                        arm.guard.as_ref().map_or(0, runtime_expr_work_units)
                            + runtime_expr_work_units(&arm.value)
                    })
                    .sum::<usize>()
        }
    }
}

pub(super) fn cache_entry(
    cache: &[Option<RuntimePureCacheEntry>],
    id: RuntimePureHelperId,
) -> Option<&RuntimePureCacheEntry> {
    cache.get(id.0).and_then(Option::as_ref)
}

pub(super) fn call_jit_batch(
    compiled: &CompiledPureI64Inputs,
    rows: &[RuntimeI64Args],
    out: &mut [i64],
    helper: &RuntimePureHelper,
    flat_inputs: &mut Vec<i64>,
) -> Result<(), RuntimeEvalError> {
    let arity = compiled.param_names().len();
    flat_inputs.clear();
    flat_inputs.reserve(rows.len().saturating_mul(arity));
    for row in rows {
        flat_inputs.extend_from_slice(row.as_slice());
    }
    compiled
        .call_flat_batch(flat_inputs, out)
        .map_err(|error| RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: error.to_string(),
        })
}

pub(super) fn call_jit_flat_batch_sum(
    compiled: &CompiledPureI64Inputs,
    helper: &RuntimePureHelper,
    flat_inputs: &[i64],
    rows: usize,
) -> Result<i64, RuntimeEvalError> {
    compiled
        .call_flat_batch_sum(flat_inputs, rows)
        .map_err(|error| RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: error.to_string(),
        })
}

pub(super) fn call_aot_batch(
    compiled: &AotPureI64Plan,
    rows: &[RuntimeI64Args],
    out: &mut [i64],
    slots: &mut Vec<i64>,
) -> Result<(), RuntimeEvalError> {
    rows.iter().zip(out.iter_mut()).try_for_each(|(row, slot)| {
        compiled
            .call_with_inputs_scratch(row.as_slice(), slots)
            .map(|(value, _)| *slot = value)
    })
}

pub(super) fn call_aot_batch_parallel(
    pool: Option<&ThreadPool>,
    compiled: &AotPureI64Plan,
    rows: &[RuntimeI64Args],
    out: &mut [i64],
) -> Result<(), RuntimeEvalError> {
    let mut run = || {
        rows.par_iter()
            .zip(out.par_iter_mut())
            .try_for_each_init(Vec::new, |slots, (row, slot)| {
                compiled
                    .call_with_inputs_scratch(row.as_slice(), slots)
                    .map(|(value, _)| *slot = value)
            })
    };
    match pool {
        Some(pool) => pool.install(run),
        None => run(),
    }
}

pub(super) fn call_aot_flat_batch(
    compiled: &AotPureI64Plan,
    flat_inputs: &[i64],
    arity: usize,
    out: &mut [i64],
    slots: &mut Vec<i64>,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            compiled
                .call_with_inputs_scratch(&[], slots)
                .map(|(value, _)| *slot = value)
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            compiled
                .call_with_inputs_scratch(row, slots)
                .map(|(value, _)| *slot = value)
        })
}

pub(super) fn call_aot_flat_batch_parallel(
    pool: Option<&ThreadPool>,
    compiled: &AotPureI64Plan,
    flat_inputs: &[i64],
    arity: usize,
    out: &mut [i64],
) -> Result<(), RuntimeEvalError> {
    let mut run = || {
        if arity == 0 {
            return out
                .par_iter_mut()
                .try_for_each_init(Vec::new, |slots, slot| {
                    compiled
                        .call_with_inputs_scratch(&[], slots)
                        .map(|(value, _)| *slot = value)
                });
        }
        flat_inputs
            .par_chunks_exact(arity)
            .zip(out.par_iter_mut())
            .try_for_each_init(Vec::new, |slots, (row, slot)| {
                compiled
                    .call_with_inputs_scratch(row, slots)
                    .map(|(value, _)| *slot = value)
            })
    };
    match pool {
        Some(pool) => pool.install(run),
        None => run(),
    }
}

pub(super) fn call_aot_flat_batch_sum(
    compiled: &AotPureI64Plan,
    flat_inputs: &[i64],
    arity: usize,
    rows: usize,
    slots: &mut Vec<i64>,
) -> Result<i64, RuntimeEvalError> {
    let mut sum = 0i64;
    if arity == 0 {
        for _ in 0..rows {
            let (value, _) = compiled.call_with_inputs_scratch(&[], slots)?;
            sum += value;
        }
        return Ok(sum);
    }
    for row in flat_inputs.chunks_exact(arity) {
        let (value, _) = compiled.call_with_inputs_scratch(row, slots)?;
        sum += value;
    }
    Ok(sum)
}

pub(super) fn call_aot_flat_batch_sum_parallel(
    pool: Option<&ThreadPool>,
    compiled: &AotPureI64Plan,
    flat_inputs: &[i64],
    arity: usize,
    rows: usize,
) -> Result<i64, RuntimeEvalError> {
    let run = || {
        if arity == 0 {
            return (0..rows)
                .into_par_iter()
                .try_fold(
                    || (Vec::new(), 0i64),
                    |(mut slots, sum), _| {
                        let (value, _) = compiled.call_with_inputs_scratch(&[], &mut slots)?;
                        Ok::<(Vec<i64>, i64), RuntimeEvalError>((slots, sum + value))
                    },
                )
                .map(|result| result.map(|(_, sum)| sum))
                .try_reduce(|| 0, |lhs, rhs| Ok(lhs + rhs));
        }
        flat_inputs
            .par_chunks_exact(arity)
            .try_fold(
                || (Vec::new(), 0i64),
                |(mut slots, sum), row| {
                    let (value, _) = compiled.call_with_inputs_scratch(row, &mut slots)?;
                    Ok::<(Vec<i64>, i64), RuntimeEvalError>((slots, sum + value))
                },
            )
            .map(|result| result.map(|(_, sum)| sum))
            .try_reduce(|| 0, |lhs, rhs| Ok(lhs + rhs))
    };
    match pool {
        Some(pool) => pool.install(run),
        None => run(),
    }
}

pub(super) fn call_aot_flat_batch_sum_with_policy(
    policy: FlatBatchSumPolicy<'_>,
    stats: &mut RuntimePureCallStats,
    compiled: &AotPureI64Plan,
    shape: FlatBatchSumShape<'_>,
    slots: &mut Vec<i64>,
) -> Result<i64, RuntimeEvalError> {
    if policy.wants_parallel {
        stats.thread_pool_jobs += policy.parallel_jobs;
        call_aot_flat_batch_sum_parallel(
            policy.pool,
            compiled,
            shape.flat_inputs,
            shape.arity,
            shape.rows,
        )
    } else {
        call_aot_flat_batch_sum(compiled, shape.flat_inputs, shape.arity, shape.rows, slots)
    }
}

pub(super) fn call_vm_batch(
    helper: &RuntimePureHelper,
    rows: &[RuntimeI64Args],
    out: &mut [i64],
    scratch: &mut VmPureFunctionScratch,
) -> Result<(), RuntimeEvalError> {
    rows.iter().zip(out.iter_mut()).try_for_each(|(row, slot)| {
        *slot = exact_i64_result(scratch.evaluate_i64_args(helper, *row)?)?;
        Ok(())
    })
}

pub(super) fn call_vm_batch_parallel(
    pool: Option<&ThreadPool>,
    helper: &RuntimePureHelper,
    rows: &[RuntimeI64Args],
    out: &mut [i64],
) -> Result<(), RuntimeEvalError> {
    let mut run = || {
        rows.par_iter().zip(out.par_iter_mut()).try_for_each_init(
            VmPureFunctionScratch::default,
            |scratch, (row, slot)| {
                *slot = exact_i64_result(scratch.evaluate_i64_args(helper, *row)?)?;
                Ok(())
            },
        )
    };
    match pool {
        Some(pool) => pool.install(run),
        None => run(),
    }
}

pub(super) fn call_vm_flat_batch(
    helper: &RuntimePureHelper,
    flat_inputs: &[i64],
    arity: usize,
    out: &mut [i64],
    scratch: &mut VmPureFunctionScratch,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            *slot = exact_i64_result(scratch.evaluate_i64_slice(helper, &[])?)?;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            *slot = exact_i64_result(scratch.evaluate_i64_slice(helper, row)?)?;
            Ok(())
        })
}

pub(super) fn call_vm_i32_flat_batch(
    helper: &RuntimePureHelper,
    flat_inputs: &[i32],
    arity: usize,
    out: &mut [i32],
    scratch: &mut VmPureFunctionScratch,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            *slot = vm_i32_result(helper, scratch.evaluate_i32_slice(helper, &[])?)?;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            *slot = vm_i32_result(helper, scratch.evaluate_i32_slice(helper, row)?)?;
            Ok(())
        })
}

pub(super) fn call_vm_i32_flat_batch_sum(
    helper: &RuntimePureHelper,
    flat_inputs: &[i32],
    arity: usize,
    rows: usize,
    scratch: &mut VmPureFunctionScratch,
) -> Result<i64, RuntimeEvalError> {
    let mut sum = 0_i64;
    if arity == 0 {
        for _ in 0..rows {
            sum += i64::from(vm_i32_result(
                helper,
                scratch.evaluate_i32_slice(helper, &[])?,
            )?);
        }
        return Ok(sum);
    }
    for row in flat_inputs.chunks_exact(arity) {
        sum += i64::from(vm_i32_result(
            helper,
            scratch.evaluate_i32_slice(helper, row)?,
        )?);
    }
    Ok(sum)
}

pub(super) fn call_vm_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
    helper: &RuntimePureHelper,
    flat_inputs: &[T],
    arity: usize,
    rows: usize,
    scratch: &mut VmPureFunctionScratch,
) -> Result<i64, RuntimeEvalError> {
    let mut sum = 0_i64;
    if arity == 0 {
        for _ in 0..rows {
            let value = scratch.evaluate_exact_int_slice::<T>(helper, &[])?;
            sum += T::try_from_runtime_value(&helper.name, value)?.try_sum_as_i64(&helper.name)?;
        }
        return Ok(sum);
    }
    for row in flat_inputs.chunks_exact(arity) {
        let value = scratch.evaluate_exact_int_slice::<T>(helper, row)?;
        sum += T::try_from_runtime_value(&helper.name, value)?.try_sum_as_i64(&helper.name)?;
    }
    Ok(sum)
}

pub(super) fn call_vm_exact_int_flat_batch<T: RuntimePureScalarInteger>(
    helper: &RuntimePureHelper,
    flat_inputs: &[T],
    arity: usize,
    out: &mut [T],
    scratch: &mut VmPureFunctionScratch,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            let value = scratch.evaluate_exact_int_slice::<T>(helper, &[])?;
            *slot = T::try_from_runtime_value(&helper.name, value)?;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            let value = scratch.evaluate_exact_int_slice::<T>(helper, row)?;
            *slot = T::try_from_runtime_value(&helper.name, value)?;
            Ok(())
        })
}

pub(super) fn call_aot_exact_int_flat_batch<T: RuntimePureScalarInteger>(
    compiled: &RuntimePureAotPlan,
    flat_inputs: &[T],
    arity: usize,
    out: &mut [T],
    slots: &mut Vec<RuntimePureScalar>,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            let (value, _) = compiled.call_exact_int_with_inputs_scratch(&[], slots)?;
            *slot = value;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            let (value, _) = compiled.call_exact_int_with_inputs_scratch(row, slots)?;
            *slot = value;
            Ok(())
        })
}

pub(super) fn call_aot_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
    compiled: &RuntimePureAotPlan,
    flat_inputs: &[T],
    arity: usize,
    rows: usize,
    slots: &mut Vec<RuntimePureScalar>,
) -> Result<i64, RuntimeEvalError> {
    let mut sum = 0_i64;
    if arity == 0 {
        for _ in 0..rows {
            let (value, _) = compiled.call_exact_int_with_inputs_scratch::<T>(&[], slots)?;
            sum += value.try_sum_as_i64("aot_exact_int_batch_sum")?;
        }
        return Ok(sum);
    }
    for row in flat_inputs.chunks_exact(arity) {
        let (value, _) = compiled.call_exact_int_with_inputs_scratch(row, slots)?;
        sum += value.try_sum_as_i64("aot_exact_int_batch_sum")?;
    }
    Ok(sum)
}

pub(super) fn call_vm_f32_flat_batch(
    helper: &RuntimePureHelper,
    flat_inputs: &[f32],
    arity: usize,
    out: &mut [f32],
    scratch: &mut VmPureFunctionScratch,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            *slot = vm_f32_result(helper, scratch.evaluate_f32_slice(helper, &[])?)?;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            *slot = vm_f32_result(helper, scratch.evaluate_f32_slice(helper, row)?)?;
            Ok(())
        })
}

pub(super) fn call_aot_f32_flat_batch(
    compiled: &RuntimePureAotPlan,
    flat_inputs: &[f32],
    arity: usize,
    out: &mut [f32],
    slots: &mut Vec<RuntimePureScalar>,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            let (value, _) = compiled.call_f32_with_inputs_scratch(&[], slots)?;
            *slot = value;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            let (value, _) = compiled.call_f32_with_inputs_scratch(row, slots)?;
            *slot = value;
            Ok(())
        })
}

pub(super) fn call_vm_f64_flat_batch(
    helper: &RuntimePureHelper,
    flat_inputs: &[f64],
    arity: usize,
    out: &mut [f64],
    scratch: &mut VmPureFunctionScratch,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            *slot = vm_f64_result(helper, scratch.evaluate_f64_slice(helper, &[])?)?;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            *slot = vm_f64_result(helper, scratch.evaluate_f64_slice(helper, row)?)?;
            Ok(())
        })
}

pub(super) fn call_aot_f64_flat_batch(
    compiled: &RuntimePureAotPlan,
    flat_inputs: &[f64],
    arity: usize,
    out: &mut [f64],
    slots: &mut Vec<RuntimePureScalar>,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            let (value, _) = compiled.call_f64_with_inputs_scratch(&[], slots)?;
            *slot = value;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            let (value, _) = compiled.call_f64_with_inputs_scratch(row, slots)?;
            *slot = value;
            Ok(())
        })
}

pub(super) fn vm_i32_result(
    helper: &RuntimePureHelper,
    value: RuntimeValue,
) -> Result<i32, RuntimeEvalError> {
    match value {
        RuntimeValue::Int(value) => {
            value
                .exact_i32()
                .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: format!("pure i32 result `{value}` is outside i32 range"),
                })
        }
        value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
    }
}

pub(super) fn vm_f32_result(
    helper: &RuntimePureHelper,
    value: RuntimeValue,
) -> Result<f32, RuntimeEvalError> {
    match value {
        RuntimeValue::F32(value) => Ok(value),
        value => Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure f32 result expected f32, got {}",
                runtime_value_kind(&value)
            ),
        }),
    }
}

pub(super) fn vm_f64_result(
    helper: &RuntimePureHelper,
    value: RuntimeValue,
) -> Result<f64, RuntimeEvalError> {
    match value {
        RuntimeValue::F64(value) => Ok(value),
        value => Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure f64 result expected f64, got {}",
                runtime_value_kind(&value)
            ),
        }),
    }
}

pub(super) fn call_vm_flat_batch_parallel(
    pool: Option<&ThreadPool>,
    helper: &RuntimePureHelper,
    flat_inputs: &[i64],
    arity: usize,
    out: &mut [i64],
) -> Result<(), RuntimeEvalError> {
    let mut run = || {
        if arity == 0 {
            return out.par_iter_mut().try_for_each_init(
                VmPureFunctionScratch::default,
                |scratch, slot| {
                    *slot = exact_i64_result(scratch.evaluate_i64_slice(helper, &[])?)?;
                    Ok(())
                },
            );
        }
        flat_inputs
            .par_chunks_exact(arity)
            .zip(out.par_iter_mut())
            .try_for_each_init(VmPureFunctionScratch::default, |scratch, (row, slot)| {
                *slot = exact_i64_result(scratch.evaluate_i64_slice(helper, row)?)?;
                Ok(())
            })
    };
    match pool {
        Some(pool) => pool.install(run),
        None => run(),
    }
}

pub(super) fn call_vm_flat_batch_sum(
    helper: &RuntimePureHelper,
    flat_inputs: &[i64],
    arity: usize,
    rows: usize,
    scratch: &mut VmPureFunctionScratch,
) -> Result<i64, RuntimeEvalError> {
    let mut sum = 0i64;
    if arity == 0 {
        for _ in 0..rows {
            sum += exact_i64_result(scratch.evaluate_i64_slice(helper, &[])?)?;
        }
        return Ok(sum);
    }
    for row in flat_inputs.chunks_exact(arity) {
        sum += exact_i64_result(scratch.evaluate_i64_slice(helper, row)?)?;
    }
    Ok(sum)
}

pub(super) fn call_vm_flat_batch_sum_parallel(
    pool: Option<&ThreadPool>,
    helper: &RuntimePureHelper,
    flat_inputs: &[i64],
    arity: usize,
    rows: usize,
) -> Result<i64, RuntimeEvalError> {
    let run = || {
        if arity == 0 {
            return (0..rows)
                .into_par_iter()
                .try_fold(
                    || (VmPureFunctionScratch::default(), 0i64),
                    |(mut scratch, sum), _| {
                        let value = exact_i64_result(scratch.evaluate_i64_slice(helper, &[])?)?;
                        Ok::<(VmPureFunctionScratch, i64), RuntimeEvalError>((scratch, sum + value))
                    },
                )
                .map(|result| result.map(|(_, sum)| sum))
                .try_reduce(|| 0, |lhs, rhs| Ok(lhs + rhs));
        }
        flat_inputs
            .par_chunks_exact(arity)
            .try_fold(
                || (VmPureFunctionScratch::default(), 0i64),
                |(mut scratch, sum), row| {
                    let value = exact_i64_result(scratch.evaluate_i64_slice(helper, row)?)?;
                    Ok::<(VmPureFunctionScratch, i64), RuntimeEvalError>((scratch, sum + value))
                },
            )
            .map(|result| result.map(|(_, sum)| sum))
            .try_reduce(|| 0, |lhs, rhs| Ok(lhs + rhs))
    };
    match pool {
        Some(pool) => pool.install(run),
        None => run(),
    }
}

pub(super) fn call_vm_flat_batch_sum_with_policy(
    policy: FlatBatchSumPolicy<'_>,
    stats: &mut RuntimePureCallStats,
    helper: &RuntimePureHelper,
    shape: FlatBatchSumShape<'_>,
    scratch: &mut VmPureFunctionScratch,
) -> Result<i64, RuntimeEvalError> {
    if policy.wants_parallel {
        stats.thread_pool_jobs += policy.parallel_jobs;
        call_vm_flat_batch_sum_parallel(
            policy.pool,
            helper,
            shape.flat_inputs,
            shape.arity,
            shape.rows,
        )
    } else {
        call_vm_flat_batch_sum(helper, shape.flat_inputs, shape.arity, shape.rows, scratch)
    }
}

pub(super) fn compile_request(
    helper: &RuntimePureHelper,
    zero: impl Fn() -> RuntimeValue + Copy,
) -> PureFunctionRequest {
    PureFunctionRequest::new(
        helper.name.clone(),
        helper.expr.clone(),
        helper
            .input_names
            .iter()
            .cloned()
            .map(|name| RuntimeBinding {
                name,
                value: zero(),
            }),
    )
}
