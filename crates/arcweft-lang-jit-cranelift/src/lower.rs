use super::{
    BTreeMap, Configurable, CraneliftCodegenError, FloatCC, FunctionBuilder, InstBuilder, IntCC,
    JITBuilder, JITModule, LoweredF32Binding, LoweredF64Binding, LoweredIntBinding,
    LoweredSmallIntBinding, ModuleError, ObjectBuilder, ObjectModule, OwnedTargetIsa,
    PureFunctionStats, RuntimeBinaryOp, RuntimeBinding, RuntimeCallTarget, RuntimeExpr, RuntimeInt,
    RuntimeIntrinsic, RuntimeUnaryOp, RuntimeValue, SmallIntKind, SmallIntLiteral, Value,
    default_libcall_names, settings, types,
};

pub(super) fn lower_input_value(
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

pub(super) fn lower_next_input_value(
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

pub(super) fn validate_param_names(param_names: &[String]) -> Result<(), CraneliftCodegenError> {
    for (index, name) in param_names.iter().enumerate() {
        if name.is_empty() {
            return Err(CraneliftCodegenError::UnsupportedExpr(
                "JIT runtime input names must be non-empty".to_owned(),
            ));
        }
        if param_names[..index].contains(name) {
            return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "JIT runtime input `{name}` is duplicated"
            )));
        }
    }
    Ok(())
}

pub(super) fn native_isa(is_pic: bool) -> Result<OwnedTargetIsa, CraneliftCodegenError> {
    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|error| CraneliftCodegenError::Backend(error.to_string()))?;
    flag_builder
        .set("is_pic", if is_pic { "true" } else { "false" })
        .map_err(|error| CraneliftCodegenError::Backend(error.to_string()))?;
    flag_builder
        .set("opt_level", "speed")
        .map_err(|error| CraneliftCodegenError::Backend(error.to_string()))?;
    let isa_builder = cranelift::native::builder()
        .map_err(|message| CraneliftCodegenError::UnsupportedHost(message.to_owned()))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|error| CraneliftCodegenError::Backend(error.to_string()))?;
    Ok(isa)
}

pub(super) fn jit_module() -> Result<JITModule, CraneliftCodegenError> {
    let isa = native_isa(false)?;
    Ok(JITModule::new(JITBuilder::with_isa(
        isa,
        default_libcall_names(),
    )))
}

pub(super) fn object_module() -> Result<ObjectModule, CraneliftCodegenError> {
    let isa = native_isa(true)?;
    let builder = ObjectBuilder::new(isa, "arcweft_pure_object", default_libcall_names())
        .map_err(codegen_error)?;
    Ok(ObjectModule::new(builder))
}

pub(super) fn emit_object_bytes(module: ObjectModule) -> Result<Vec<u8>, CraneliftCodegenError> {
    module
        .finish()
        .emit()
        .map_err(|error| CraneliftCodegenError::Backend(error.to_string()))
}

pub(super) fn sanitize_symbol_component(name: &str) -> String {
    let mut sanitized = String::with_capacity(name.len().max(1));
    let mut previous_underscore = false;
    for ch in name.chars() {
        let mapped = if ch.is_ascii_alphanumeric() { ch } else { '_' };
        if mapped == '_' {
            if !previous_underscore {
                sanitized.push(mapped);
            }
            previous_underscore = true;
        } else {
            sanitized.push(mapped);
            previous_underscore = false;
        }
    }
    let sanitized = sanitized.trim_matches('_');
    if sanitized.is_empty() {
        "helper".to_owned()
    } else {
        sanitized.to_owned()
    }
}

pub(super) fn int_bindings(
    bindings: &[RuntimeBinding],
) -> Result<BTreeMap<String, LoweredIntBinding>, CraneliftCodegenError> {
    bindings
        .iter()
        .map(|binding| match binding.value {
            RuntimeValue::Int(value) => value
                .exact_i64()
                .or(match value {
                    RuntimeInt::ISize(value) => Some(value),
                    _ => None,
                })
                .map(|value| (binding.name.clone(), LoweredIntBinding::Const(value)))
                .ok_or_else(|| {
                    CraneliftCodegenError::UnsupportedExpr(format!(
                        "binding `{}` is not an i64-compatible integer",
                        binding.name
                    ))
                }),
            _ => Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "binding `{}` is not an i64-compatible integer",
                binding.name
            ))),
        })
        .collect()
}

pub(super) fn small_int_bindings(
    bindings: &[RuntimeBinding],
    kind: SmallIntKind,
) -> Result<BTreeMap<String, LoweredSmallIntBinding>, CraneliftCodegenError> {
    bindings
        .iter()
        .map(|binding| {
            kind.literal(&binding.value)
                .map(|value| (binding.name.clone(), LoweredSmallIntBinding::Const(value)))
                .ok_or_else(|| {
                    CraneliftCodegenError::UnsupportedExpr(format!(
                        "binding `{}` is not an {} integer",
                        binding.name,
                        kind.label()
                    ))
                })
        })
        .collect()
}

pub(super) fn i32_bindings(
    bindings: &[RuntimeBinding],
) -> Result<BTreeMap<String, LoweredIntBinding>, CraneliftCodegenError> {
    bindings
        .iter()
        .map(|binding| match binding.value {
            RuntimeValue::Int(value) => value
                .exact_i32()
                .map(|value| {
                    (
                        binding.name.clone(),
                        LoweredIntBinding::Const(i64::from(value)),
                    )
                })
                .ok_or_else(|| {
                    CraneliftCodegenError::UnsupportedExpr(format!(
                        "binding `{}` is not an i32 integer",
                        binding.name
                    ))
                }),
            _ => Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "binding `{}` is not an i32 integer",
                binding.name
            ))),
        })
        .collect()
}

pub(super) fn u32_bindings(
    bindings: &[RuntimeBinding],
) -> Result<BTreeMap<String, LoweredIntBinding>, CraneliftCodegenError> {
    bindings
        .iter()
        .map(|binding| match binding.value {
            RuntimeValue::UInt(arcweft_core::value::RuntimeUInt::U32(value)) => Ok((
                binding.name.clone(),
                LoweredIntBinding::Const(u32_iconst_value(value)),
            )),
            _ => Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "binding `{}` is not an u32 integer",
                binding.name
            ))),
        })
        .collect()
}

pub(super) fn u64_bindings(
    bindings: &[RuntimeBinding],
) -> Result<BTreeMap<String, LoweredIntBinding>, CraneliftCodegenError> {
    bindings
        .iter()
        .map(|binding| match binding.value {
            RuntimeValue::UInt(arcweft_core::value::RuntimeUInt::U64(value)) => Ok((
                binding.name.clone(),
                LoweredIntBinding::Const(u64_iconst_value(value)),
            )),
            RuntimeValue::UInt(arcweft_core::value::RuntimeUInt::USize(value)) => Ok((
                binding.name.clone(),
                LoweredIntBinding::Const(u64_iconst_value(value)),
            )),
            _ => Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "binding `{}` is not an u64-compatible integer",
                binding.name
            ))),
        })
        .collect()
}

pub(super) fn f32_bindings(
    bindings: &[RuntimeBinding],
) -> Result<BTreeMap<String, LoweredF32Binding>, CraneliftCodegenError> {
    bindings
        .iter()
        .map(|binding| match binding.value {
            RuntimeValue::F32(value) => Ok((binding.name.clone(), LoweredF32Binding::Const(value))),
            _ => Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "binding `{}` is not an f32 value",
                binding.name
            ))),
        })
        .collect()
}

pub(super) fn f64_bindings(
    bindings: &[RuntimeBinding],
) -> Result<BTreeMap<String, LoweredF64Binding>, CraneliftCodegenError> {
    bindings
        .iter()
        .map(|binding| match binding.value {
            RuntimeValue::F64(value) => Ok((binding.name.clone(), LoweredF64Binding::Const(value))),
            _ => Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "binding `{}` is not an f64 value",
                binding.name
            ))),
        })
        .collect()
}

pub(super) fn lower_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredIntBinding>,
    expr: &RuntimeExpr,
    stats: &mut arcweft_core::pure::PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Int(value)) => value
            .exact_i64()
            .or(match *value {
                RuntimeInt::ISize(value) => Some(value),
                _ => None,
            })
            .map(|value| builder.ins().iconst(types::I64, value))
            .ok_or_else(|| {
                CraneliftCodegenError::UnsupportedExpr(format!(
                    "literal `{value}` is not an i64-compatible integer"
                ))
            }),
        RuntimeExpr::Value(value) => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "literal {value:?} is not an i64-compatible integer"
        ))),
        RuntimeExpr::Local(name) => match bindings.get(name) {
            Some(LoweredIntBinding::Const(value)) => Ok(builder.ins().iconst(types::I64, *value)),
            Some(LoweredIntBinding::Value(value)) => Ok(*value),
            None => Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "unknown integer binding `{name}`"
            ))),
        },
        RuntimeExpr::Let { name, expr, body } => {
            let value = lower_expr(builder, bindings, expr, stats)?;
            let mut scoped_bindings = bindings.clone();
            scoped_bindings.insert(name.clone(), LoweredIntBinding::Value(value));
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
        } => Err(CraneliftCodegenError::UnsupportedExpr(
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
                _ => Err(CraneliftCodegenError::UnsupportedExpr(format!(
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
        RuntimeExpr::Call { callee, .. } => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "call `{callee}` is outside the JIT subset"
        ))),
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "expression `{other}` is outside the JIT subset"
        ))),
    }
}

pub(super) fn lower_i32_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredIntBinding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Int(value)) => value
            .exact_i32()
            .map(|value| builder.ins().iconst(types::I32, i64::from(value)))
            .ok_or_else(|| {
                CraneliftCodegenError::UnsupportedExpr(format!(
                    "literal `{value}` is not an i32 integer"
                ))
            }),
        RuntimeExpr::Value(value) => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "literal {value:?} is not an i32 integer"
        ))),
        RuntimeExpr::Local(name) => match bindings.get(name) {
            Some(LoweredIntBinding::Const(value)) => Ok(builder.ins().iconst(types::I32, *value)),
            Some(LoweredIntBinding::Value(value)) => Ok(*value),
            None => Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "unknown i32 binding `{name}`"
            ))),
        },
        RuntimeExpr::Let { name, expr, body } => {
            let value = lower_i32_expr(builder, bindings, expr, stats)?;
            let mut scoped_bindings = bindings.clone();
            scoped_bindings.insert(name.clone(), LoweredIntBinding::Value(value));
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
        } => Err(CraneliftCodegenError::UnsupportedExpr(
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
                _ => Err(CraneliftCodegenError::UnsupportedExpr(format!(
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
        RuntimeExpr::Call { callee, .. } => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "call `{callee}` is outside the i32 JIT subset"
        ))),
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "expression `{other}` is outside the i32 JIT subset"
        ))),
    }
}

pub(super) fn u32_iconst_value(value: u32) -> i64 {
    i64::from(i32::from_ne_bytes(value.to_ne_bytes()))
}

pub(super) fn u64_iconst_value(value: u64) -> i64 {
    i64::from_ne_bytes(value.to_ne_bytes())
}

pub(super) fn lower_small_int_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredSmallIntBinding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
    kind: SmallIntKind,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(value) => kind
            .literal(value)
            .map(|value| small_int_const(builder, kind, value))
            .ok_or_else(|| {
                CraneliftCodegenError::UnsupportedExpr(format!(
                    "literal {value:?} is not an {} integer",
                    kind.label()
                ))
            }),
        RuntimeExpr::Local(name) => match bindings.get(name) {
            Some(LoweredSmallIntBinding::Const(value)) => {
                Ok(small_int_const(builder, kind, *value))
            }
            Some(LoweredSmallIntBinding::Value(value)) => Ok(*value),
            None => Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "unknown {} binding `{name}`",
                kind.label()
            ))),
        },
        RuntimeExpr::Let { name, expr, body } => {
            let value = lower_small_int_expr(builder, bindings, expr, stats, kind)?;
            let mut scoped_bindings = bindings.clone();
            scoped_bindings.insert(name.clone(), LoweredSmallIntBinding::Value(value));
            lower_small_int_expr(builder, &scoped_bindings, body, stats, kind)
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Neg,
            expr,
        } => {
            let value = lower_small_int_expr(builder, bindings, expr, stats, kind)?;
            Ok(builder.ins().ineg(value))
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Not,
            ..
        } => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "boolean negation is not an {} result",
            kind.label()
        ))),
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let lhs = lower_small_int_expr(builder, bindings, lhs, stats, kind)?;
            let rhs = lower_small_int_expr(builder, bindings, rhs, stats, kind)?;
            match op {
                RuntimeBinaryOp::Add => Ok(builder.ins().iadd(lhs, rhs)),
                RuntimeBinaryOp::Sub => Ok(builder.ins().isub(lhs, rhs)),
                RuntimeBinaryOp::Mul => Ok(builder.ins().imul(lhs, rhs)),
                RuntimeBinaryOp::Div if kind.signed() => Ok(builder.ins().sdiv(lhs, rhs)),
                RuntimeBinaryOp::Div => Ok(builder.ins().udiv(lhs, rhs)),
                _ => Err(CraneliftCodegenError::UnsupportedExpr(format!(
                    "binary operator `{op}` is outside the {} JIT subset",
                    kind.label()
                ))),
            }
        }
        RuntimeExpr::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::Add) && args.len() == 2 =>
        {
            stats.evaluated_calls += 1;
            let lhs = lower_small_int_expr(builder, bindings, &args[0], stats, kind)?;
            let rhs = lower_small_int_expr(builder, bindings, &args[1], stats, kind)?;
            Ok(builder.ins().iadd(lhs, rhs))
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => lower_small_int_if_expr(
            builder, bindings, condition, then_expr, else_expr, stats, kind,
        ),
        RuntimeExpr::Call { callee, .. } => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "call `{callee}` is outside the {} JIT subset",
            kind.label()
        ))),
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "expression `{other}` is outside the {} JIT subset",
            kind.label()
        ))),
    }
}

pub(super) fn small_int_const(
    builder: &mut FunctionBuilder<'_>,
    kind: SmallIntKind,
    value: SmallIntLiteral,
) -> Value {
    let ty = kind.cranelift_type();
    match value {
        SmallIntLiteral::Narrow(value) if ty.bits() <= 64 => builder.ins().iconst(ty, value),
        SmallIntLiteral::Narrow(value) => {
            let value = builder.ins().iconst(types::I64, value);
            if kind.signed() {
                builder.ins().sextend(ty, value)
            } else {
                builder.ins().uextend(ty, value)
            }
        }
        SmallIntLiteral::I128(value) if matches!(kind, SmallIntKind::I128) => {
            i128_const(builder, value)
        }
        SmallIntLiteral::U128(value) if matches!(kind, SmallIntKind::U128) => {
            u128_const(builder, value)
        }
        SmallIntLiteral::I128(_) | SmallIntLiteral::U128(_) => {
            unreachable!("literal kind is validated by SmallIntKind::literal")
        }
    }
}

pub(super) fn i128_const(builder: &mut FunctionBuilder<'_>, value: i128) -> Value {
    u128_const(builder, value as u128)
}

pub(super) fn u128_const(builder: &mut FunctionBuilder<'_>, value: u128) -> Value {
    let lo = builder
        .ins()
        .iconst(types::I64, bitpattern_i64(value as u64));
    let hi = builder
        .ins()
        .iconst(types::I64, bitpattern_i64((value >> 64) as u64));
    builder.ins().iconcat(lo, hi)
}

pub(super) fn bitpattern_i64(value: u64) -> i64 {
    i64::from_ne_bytes(value.to_ne_bytes())
}

pub(super) fn lower_u32_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredIntBinding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::UInt(arcweft_core::value::RuntimeUInt::U32(value))) => {
            Ok(builder.ins().iconst(types::I32, u32_iconst_value(*value)))
        }
        RuntimeExpr::Value(value) => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "literal {value:?} is not an u32 integer"
        ))),
        RuntimeExpr::Local(name) => match bindings.get(name) {
            Some(LoweredIntBinding::Const(value)) => Ok(builder.ins().iconst(types::I32, *value)),
            Some(LoweredIntBinding::Value(value)) => Ok(*value),
            None => Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "unknown u32 binding `{name}`"
            ))),
        },
        RuntimeExpr::Let { name, expr, body } => {
            let value = lower_u32_expr(builder, bindings, expr, stats)?;
            let mut scoped_bindings = bindings.clone();
            scoped_bindings.insert(name.clone(), LoweredIntBinding::Value(value));
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
        } => Err(CraneliftCodegenError::UnsupportedExpr(
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
                _ => Err(CraneliftCodegenError::UnsupportedExpr(format!(
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
        RuntimeExpr::Call { callee, .. } => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "call `{callee}` is outside the u32 JIT subset"
        ))),
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "expression `{other}` is outside the u32 JIT subset"
        ))),
    }
}

pub(super) fn lower_u64_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredIntBinding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::UInt(arcweft_core::value::RuntimeUInt::U64(value))) => {
            Ok(builder.ins().iconst(types::I64, u64_iconst_value(*value)))
        }
        RuntimeExpr::Value(RuntimeValue::UInt(arcweft_core::value::RuntimeUInt::USize(value))) => {
            Ok(builder.ins().iconst(types::I64, u64_iconst_value(*value)))
        }
        RuntimeExpr::Value(value) => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "literal {value:?} is not an u64-compatible integer"
        ))),
        RuntimeExpr::Local(name) => match bindings.get(name) {
            Some(LoweredIntBinding::Const(value)) => Ok(builder.ins().iconst(types::I64, *value)),
            Some(LoweredIntBinding::Value(value)) => Ok(*value),
            None => Err(CraneliftCodegenError::UnsupportedExpr(format!(
                "unknown u64 binding `{name}`"
            ))),
        },
        RuntimeExpr::Let { name, expr, body } => {
            let value = lower_u64_expr(builder, bindings, expr, stats)?;
            let mut scoped_bindings = bindings.clone();
            scoped_bindings.insert(name.clone(), LoweredIntBinding::Value(value));
            lower_u64_expr(builder, &scoped_bindings, body, stats)
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Neg,
            expr,
        } => {
            let value = lower_u64_expr(builder, bindings, expr, stats)?;
            Ok(builder.ins().ineg(value))
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Not,
            ..
        } => Err(CraneliftCodegenError::UnsupportedExpr(
            "boolean negation is not an u64 result".to_owned(),
        )),
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let lhs = lower_u64_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_u64_expr(builder, bindings, rhs, stats)?;
            match op {
                RuntimeBinaryOp::Add => Ok(builder.ins().iadd(lhs, rhs)),
                RuntimeBinaryOp::Sub => Ok(builder.ins().isub(lhs, rhs)),
                RuntimeBinaryOp::Mul => Ok(builder.ins().imul(lhs, rhs)),
                RuntimeBinaryOp::Div => Ok(builder.ins().udiv(lhs, rhs)),
                _ => Err(CraneliftCodegenError::UnsupportedExpr(format!(
                    "binary operator `{op}` is outside the u64 JIT subset"
                ))),
            }
        }
        RuntimeExpr::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::Add) && args.len() == 2 =>
        {
            stats.evaluated_calls += 1;
            let lhs = lower_u64_expr(builder, bindings, &args[0], stats)?;
            let rhs = lower_u64_expr(builder, bindings, &args[1], stats)?;
            Ok(builder.ins().iadd(lhs, rhs))
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => lower_u64_if_expr(builder, bindings, condition, then_expr, else_expr, stats),
        RuntimeExpr::Call { callee, .. } => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "call `{callee}` is outside the u64 JIT subset"
        ))),
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "expression `{other}` is outside the u64 JIT subset"
        ))),
    }
}

pub(super) fn lower_f32_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF32Binding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::F32(value)) => Ok(builder.ins().f32const(*value)),
        RuntimeExpr::Value(value) => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "literal {value:?} is not an f32 value"
        ))),
        RuntimeExpr::Local(name) => match bindings.get(name) {
            Some(LoweredF32Binding::Const(value)) => Ok(builder.ins().f32const(*value)),
            Some(LoweredF32Binding::Value(value)) => Ok(*value),
            None => Err(CraneliftCodegenError::UnsupportedExpr(format!(
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
        } => Err(CraneliftCodegenError::UnsupportedExpr(
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
                _ => Err(CraneliftCodegenError::UnsupportedExpr(format!(
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
                CraneliftCodegenError::UnsupportedExpr(format!(
                    "call `{callee}` is outside the f32 JIT subset"
                ))
            })?
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => lower_f32_if_expr(builder, bindings, condition, then_expr, else_expr, stats),
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "expression `{other}` is outside the f32 JIT subset"
        ))),
    }
}

pub(super) fn lower_f64_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF64Binding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::F64(value)) => Ok(builder.ins().f64const(*value)),
        RuntimeExpr::Value(value) => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "literal {value:?} is not an f64 value"
        ))),
        RuntimeExpr::Local(name) => match bindings.get(name) {
            Some(LoweredF64Binding::Const(value)) => Ok(builder.ins().f64const(*value)),
            Some(LoweredF64Binding::Value(value)) => Ok(*value),
            None => Err(CraneliftCodegenError::UnsupportedExpr(format!(
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
        } => Err(CraneliftCodegenError::UnsupportedExpr(
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
                _ => Err(CraneliftCodegenError::UnsupportedExpr(format!(
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
                CraneliftCodegenError::UnsupportedExpr(format!(
                    "call `{callee}` is outside the f64 JIT subset"
                ))
            })?
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => lower_f64_if_expr(builder, bindings, condition, then_expr, else_expr, stats),
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "expression `{other}` is outside the f64 JIT subset"
        ))),
    }
}

pub(super) fn lower_f32_std_float_call(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF32Binding>,
    callee: &RuntimeCallTarget,
    args: &[RuntimeExpr],
    stats: &mut PureFunctionStats,
) -> Option<Result<Value, CraneliftCodegenError>> {
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

pub(super) fn lower_f64_std_float_call(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF64Binding>,
    callee: &RuntimeCallTarget,
    args: &[RuntimeExpr],
    stats: &mut PureFunctionStats,
) -> Option<Result<Value, CraneliftCodegenError>> {
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

pub(super) fn lower_if_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredIntBinding>,
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
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

pub(super) fn lower_i32_if_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredIntBinding>,
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
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

pub(super) fn lower_small_int_if_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredSmallIntBinding>,
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
    kind: SmallIntKind,
) -> Result<Value, CraneliftCodegenError> {
    let condition = lower_small_int_condition(builder, bindings, condition, stats, kind)?;
    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, kind.cranelift_type());
    builder
        .ins()
        .brif(condition, then_block, &[], else_block, &[]);

    builder.switch_to_block(then_block);
    let then_value = lower_small_int_expr(builder, bindings, then_expr, stats, kind)?;
    builder.ins().jump(merge_block, &[then_value.into()]);

    builder.switch_to_block(else_block);
    let else_value = lower_small_int_expr(builder, bindings, else_expr, stats, kind)?;
    builder.ins().jump(merge_block, &[else_value.into()]);

    builder.switch_to_block(merge_block);
    Ok(builder.block_params(merge_block)[0])
}

pub(super) fn lower_u32_if_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredIntBinding>,
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
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

pub(super) fn lower_u64_if_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredIntBinding>,
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
    let condition = lower_u64_condition(builder, bindings, condition, stats)?;
    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, types::I64);
    builder
        .ins()
        .brif(condition, then_block, &[], else_block, &[]);

    builder.switch_to_block(then_block);
    let then_value = lower_u64_expr(builder, bindings, then_expr, stats)?;
    builder.ins().jump(merge_block, &[then_value.into()]);

    builder.switch_to_block(else_block);
    let else_value = lower_u64_expr(builder, bindings, else_expr, stats)?;
    builder.ins().jump(merge_block, &[else_value.into()]);

    builder.switch_to_block(merge_block);
    Ok(builder.block_params(merge_block)[0])
}

pub(super) fn lower_f32_if_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF32Binding>,
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
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

pub(super) fn lower_f64_if_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF64Binding>,
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
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

pub(super) fn lower_condition(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredIntBinding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => {
            Ok(builder.ins().iconst(types::I8, i64::from(*value)))
        }
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let Some(condition) = int_condition(*op) else {
                return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                    "condition operator `{op}` is outside the JIT subset"
                )));
            };
            let lhs = lower_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_expr(builder, bindings, rhs, stats)?;
            Ok(builder.ins().icmp(condition, lhs, rhs))
        }
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "condition `{other}` is outside the JIT subset"
        ))),
    }
}

pub(super) fn lower_i32_condition(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredIntBinding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => {
            Ok(builder.ins().iconst(types::I8, i64::from(*value)))
        }
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let Some(condition) = int_condition(*op) else {
                return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                    "condition operator `{op}` is outside the i32 JIT subset"
                )));
            };
            let lhs = lower_i32_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_i32_expr(builder, bindings, rhs, stats)?;
            Ok(builder.ins().icmp(condition, lhs, rhs))
        }
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "condition `{other}` is outside the i32 JIT subset"
        ))),
    }
}

pub(super) fn lower_small_int_condition(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredSmallIntBinding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
    kind: SmallIntKind,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => {
            Ok(builder.ins().iconst(types::I8, i64::from(*value)))
        }
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let condition = if kind.signed() {
                int_condition(*op)
            } else {
                unsigned_int_condition(*op)
            }
            .ok_or_else(|| {
                CraneliftCodegenError::UnsupportedExpr(format!(
                    "condition operator `{op}` is outside the {} JIT subset",
                    kind.label()
                ))
            })?;
            let lhs = lower_small_int_expr(builder, bindings, lhs, stats, kind)?;
            let rhs = lower_small_int_expr(builder, bindings, rhs, stats, kind)?;
            Ok(builder.ins().icmp(condition, lhs, rhs))
        }
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "condition `{other}` is outside the {} JIT subset",
            kind.label()
        ))),
    }
}

pub(super) fn lower_u32_condition(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredIntBinding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => {
            Ok(builder.ins().iconst(types::I8, i64::from(*value)))
        }
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let Some(condition) = unsigned_int_condition(*op) else {
                return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                    "condition operator `{op}` is outside the u32 JIT subset"
                )));
            };
            let lhs = lower_u32_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_u32_expr(builder, bindings, rhs, stats)?;
            Ok(builder.ins().icmp(condition, lhs, rhs))
        }
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "condition `{other}` is outside the u32 JIT subset"
        ))),
    }
}

pub(super) fn lower_u64_condition(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredIntBinding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => {
            Ok(builder.ins().iconst(types::I8, i64::from(*value)))
        }
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let Some(condition) = unsigned_int_condition(*op) else {
                return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                    "condition operator `{op}` is outside the u64 JIT subset"
                )));
            };
            let lhs = lower_u64_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_u64_expr(builder, bindings, rhs, stats)?;
            Ok(builder.ins().icmp(condition, lhs, rhs))
        }
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "condition `{other}` is outside the u64 JIT subset"
        ))),
    }
}

pub(super) fn lower_f32_condition(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF32Binding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => {
            Ok(builder.ins().iconst(types::I8, i64::from(*value)))
        }
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let Some(condition) = float_condition(*op) else {
                return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                    "condition operator `{op}` is outside the f32 JIT subset"
                )));
            };
            let lhs = lower_f32_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_f32_expr(builder, bindings, rhs, stats)?;
            Ok(builder.ins().fcmp(condition, lhs, rhs))
        }
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "condition `{other}` is outside the f32 JIT subset"
        ))),
    }
}

pub(super) fn lower_f64_condition(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, LoweredF64Binding>,
    expr: &RuntimeExpr,
    stats: &mut PureFunctionStats,
) -> Result<Value, CraneliftCodegenError> {
    stats.evaluated_exprs += 1;
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => {
            Ok(builder.ins().iconst(types::I8, i64::from(*value)))
        }
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let Some(condition) = float_condition(*op) else {
                return Err(CraneliftCodegenError::UnsupportedExpr(format!(
                    "condition operator `{op}` is outside the f64 JIT subset"
                )));
            };
            let lhs = lower_f64_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_f64_expr(builder, bindings, rhs, stats)?;
            Ok(builder.ins().fcmp(condition, lhs, rhs))
        }
        other => Err(CraneliftCodegenError::UnsupportedExpr(format!(
            "condition `{other}` is outside the f64 JIT subset"
        ))),
    }
}

pub(super) fn int_condition(op: RuntimeBinaryOp) -> Option<IntCC> {
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

pub(super) fn unsigned_int_condition(op: RuntimeBinaryOp) -> Option<IntCC> {
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

pub(super) fn float_condition(op: RuntimeBinaryOp) -> Option<FloatCC> {
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

pub(super) fn codegen_error(error: ModuleError) -> CraneliftCodegenError {
    CraneliftCodegenError::Backend(error.to_string())
}
