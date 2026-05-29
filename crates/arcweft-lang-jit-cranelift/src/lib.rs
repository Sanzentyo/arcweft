//! Native Cranelift adapter for Arcweft pure helper functions.
//!
//! The VM remains the semantic reference. This crate is intentionally outside
//! `arcweft-core` so native code generation, executable memory, and the small
//! function-pointer call boundary stay in an adapter layer.

mod native_call;

use arcweft_core::pure::{
    PureFunctionBackend, PureFunctionBackendKind, PureFunctionRequest, PureFunctionResult,
    PureFunctionStats,
};
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeBinding, RuntimeEvalError, RuntimeExpr, RuntimeUnaryOp, RuntimeValue,
};
use cranelift::codegen::ir::UserFuncName;
use cranelift::jit::{JITBuilder, JITModule};
use cranelift::module::{Linkage, Module, ModuleError, default_libcall_names};
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
            value: RuntimeValue::Int(value),
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

        let mut bindings = int_bindings(&request.bindings)?;
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
        module.finalize_definitions().map_err(jit_error)?;
        let code = module.get_finalized_function(func_id);

        Ok(CompiledPureI64Inputs {
            _module: module,
            code,
            param_names,
            stats,
        })
    }
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
        native_call::call_i64_inputs(self.code, inputs).ok_or_else(|| {
            CraneliftJitError::UnsupportedExpr(format!(
                "JIT helper arity {} is outside the native call boundary",
                inputs.len()
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
            RuntimeValue::Int(value) => Ok((binding.name.clone(), LoweredI64Binding::Const(value))),
            _ => Err(CraneliftJitError::UnsupportedExpr(format!(
                "binding `{}` is not an i64 integer",
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
        RuntimeExpr::Value(RuntimeValue::Int(value)) => {
            Ok(builder.ins().iconst(types::I64, *value))
        }
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
                    "binary operator `{op:?}` is outside the JIT subset"
                ))),
            }
        }
        RuntimeExpr::Call { callee, args } if callee == "add" && args.len() == 2 => {
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
            "expression `{other:?}` is outside the JIT subset"
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
                    "condition operator `{op:?}` is outside the JIT subset"
                )));
            };
            let lhs = lower_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_expr(builder, bindings, rhs, stats)?;
            Ok(builder.ins().icmp(condition, lhs, rhs))
        }
        other => Err(CraneliftJitError::UnsupportedExpr(format!(
            "condition `{other:?}` is outside the JIT subset"
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

    fn int_binding(name: &str, value: i64) -> RuntimeBinding {
        RuntimeBinding {
            name: name.to_owned(),
            value: RuntimeValue::Int(value),
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
                    callee: "add".to_owned(),
                    args: vec![
                        RuntimeExpr::Local("bonus".to_owned()),
                        RuntimeExpr::Value(RuntimeValue::Int(2)),
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
        assert_eq!(conformance.candidate.value, RuntimeValue::Int(18));
        assert_eq!(conformance.candidate.stats.evaluated_calls, 1);
        assert_eq!(conformance.candidate.stats.evaluated_binary_ops, 1);
    }

    #[test]
    fn cranelift_compiled_helper_can_be_called_repeatedly() {
        let request = PureFunctionRequest::new(
            "score",
            RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(21))),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(21))),
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
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(3))),
                }),
                then_expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Mul,
                    rhs: Box::new(RuntimeExpr::Call {
                        callee: "add".to_owned(),
                        args: vec![
                            RuntimeExpr::Local("bonus".to_owned()),
                            RuntimeExpr::Value(RuntimeValue::Int(2)),
                        ],
                    }),
                }),
                else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::Int(0))),
            },
            [int_binding("base", 3), int_binding("bonus", 4)],
        );

        let compiled = CraneliftPureFunctionBackend
            .compile_i64_with_inputs(&request, ["base", "bonus"])
            .expect("Cranelift compiles parameterized integer helper");

        assert_eq!(compiled.param_names(), ["base", "bonus"]);
        assert_eq!(compiled.call(&[3, 4]).expect("call succeeds"), 18);
        assert_eq!(compiled.call(&[2, 99]).expect("call succeeds"), 0);
        assert_eq!(compiled.call(&[7, 1]).expect("call succeeds"), 21);
    }

    #[test]
    fn cranelift_compiled_helper_evaluates_lexical_let() {
        let request = PureFunctionRequest::new(
            "score_with_local",
            RuntimeExpr::Let {
                name: "boosted".to_owned(),
                expr: Box::new(RuntimeExpr::Call {
                    callee: "add".to_owned(),
                    args: vec![
                        RuntimeExpr::Local("bonus".to_owned()),
                        RuntimeExpr::Value(RuntimeValue::Int(2)),
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
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(10))),
                }),
                then_expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("score".to_owned())),
                    op: RuntimeBinaryOp::Mul,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(2))),
                }),
                else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::Int(0))),
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
        assert_eq!(conformance.candidate.value, RuntimeValue::Int(24));
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
}
