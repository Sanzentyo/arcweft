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
    RuntimeBinaryOp, RuntimeBinding, RuntimeEvalError, RuntimeExpr, RuntimeValue,
};
use cranelift::codegen::ir::UserFuncName;
use cranelift::jit::{JITBuilder, JITModule};
use cranelift::module::{Linkage, Module, ModuleError, default_libcall_names};
use cranelift::prelude::{
    AbiParam, Configurable, FunctionBuilder, FunctionBuilderContext, InstBuilder, Value, settings,
    types,
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
    /// literal and bound-local values, including the registered `add` helper.
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

fn int_bindings(bindings: &[RuntimeBinding]) -> Result<BTreeMap<String, i64>, CraneliftJitError> {
    bindings
        .iter()
        .map(|binding| match binding.value {
            RuntimeValue::Int(value) => Ok((binding.name.clone(), value)),
            _ => Err(CraneliftJitError::UnsupportedExpr(format!(
                "binding `{}` is not an i64 integer",
                binding.name
            ))),
        })
        .collect()
}

fn lower_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, i64>,
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
        RuntimeExpr::Local(name) => bindings.get(name).map_or_else(
            || {
                Err(CraneliftJitError::UnsupportedExpr(format!(
                    "unknown integer binding `{name}`"
                )))
            },
            |value| Ok(builder.ins().iconst(types::I64, *value)),
        ),
        RuntimeExpr::Binary { lhs, op, rhs } => {
            stats.evaluated_binary_ops += 1;
            let lhs = lower_expr(builder, bindings, lhs, stats)?;
            let rhs = lower_expr(builder, bindings, rhs, stats)?;
            match op {
                RuntimeBinaryOp::Add => Ok(builder.ins().iadd(lhs, rhs)),
                RuntimeBinaryOp::Sub => Ok(builder.ins().isub(lhs, rhs)),
                RuntimeBinaryOp::Mul => Ok(builder.ins().imul(lhs, rhs)),
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
        RuntimeExpr::Call { callee, .. } => Err(CraneliftJitError::UnsupportedExpr(format!(
            "call `{callee}` is outside the JIT subset"
        ))),
        other => Err(CraneliftJitError::UnsupportedExpr(format!(
            "expression `{other:?}` is outside the JIT subset"
        ))),
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
