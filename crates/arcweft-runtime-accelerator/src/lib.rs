//! Runtime pure helper acceleration adapters.
//!
//! This crate owns native acceleration state so `arcweft-core` can stay Sans I/O
//! and dependency-light.

use arcweft_core::{
    plan::{RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin},
    pure::{
        AotPureFunctionBackend, AotPureI64Plan, PureFunctionBackend, PureFunctionRequest,
        RuntimeI64Args, RuntimePureCallBackend, VmPureFunctionBackend,
    },
    step::RuntimePureCallStats,
    value::{RuntimeBinding, RuntimeEvalError, RuntimeValue},
};
use arcweft_lang_jit_cranelift::{CompiledPureI64Inputs, CraneliftPureFunctionBackend};
use std::{collections::BTreeMap, fmt};

/// Runtime pure backend selection used by CLI/player adapters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimePureBackendMode {
    Vm,
    Aot,
    Jit,
    #[default]
    Auto,
}

/// Compile-cache backed runtime pure helper accelerator.
pub struct RuntimePureAccelerator {
    mode: RuntimePureBackendMode,
    cache: BTreeMap<RuntimePureHelperId, RuntimePureCacheEntry>,
    stats: RuntimePureCallStats,
}

enum RuntimePureCacheEntry {
    Jit(Box<CompiledPureI64Inputs>),
    Aot(AotPureI64Plan),
    Vm,
}

impl fmt::Debug for RuntimePureAccelerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimePureAccelerator")
            .field("mode", &self.mode)
            .field("cache_entries", &self.cache.len())
            .field("stats", &self.stats)
            .finish()
    }
}

impl RuntimePureAccelerator {
    pub fn new(mode: RuntimePureBackendMode, helpers: &[RuntimePureHelper]) -> Self {
        let cache = helpers
            .iter()
            .map(|helper| (helper.id, compile_helper(mode, helper)))
            .collect();
        Self {
            mode,
            cache,
            stats: RuntimePureCallStats::default(),
        }
    }

    pub const fn mode(&self) -> RuntimePureBackendMode {
        self.mode
    }
}

impl RuntimePureCallBackend for RuntimePureAccelerator {
    fn call_i64(
        &mut self,
        helper: &RuntimePureHelper,
        args: RuntimeI64Args,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        self.stats.pure_calls += 1;
        self.stats.arg_stack_packs += 1;
        match self.cache.get(&helper.id) {
            Some(RuntimePureCacheEntry::Jit(compiled)) => {
                self.stats.jit_calls += 1;
                compiled.call(args.as_slice()).map(Some).map_err(|error| {
                    RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    }
                })
            }
            Some(RuntimePureCacheEntry::Aot(compiled)) => {
                self.stats.aot_calls += 1;
                compiled
                    .call_with_inputs(args.as_slice())
                    .map(|(value, _)| Some(value))
            }
            Some(RuntimePureCacheEntry::Vm) | None => {
                self.stats.vm_calls += 1;
                self.call_vm_i64(helper, args).map(Some)
            }
        }
    }

    fn call_values(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_vec_allocations += 1;
        evaluate_vm(helper, args)
    }

    fn stats(&self) -> RuntimePureCallStats {
        self.stats
    }
}

impl RuntimePureAccelerator {
    fn call_vm_i64(
        &mut self,
        helper: &RuntimePureHelper,
        args: RuntimeI64Args,
    ) -> Result<i64, RuntimeEvalError> {
        self.stats.arg_vec_allocations += 1;
        let values = args
            .as_slice()
            .iter()
            .copied()
            .map(RuntimeValue::Int)
            .collect::<Vec<_>>();
        match evaluate_vm(helper, &values)? {
            RuntimeValue::Int(value) => Ok(value),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
        }
    }
}

fn runtime_value_kind(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "()",
        RuntimeValue::Bool(_) => "bool",
        RuntimeValue::Int(_) => "int",
        RuntimeValue::Float(_) => "float",
        RuntimeValue::String(_) => "string",
        RuntimeValue::Char(_) => "char",
        RuntimeValue::Duration(_) => "duration",
        RuntimeValue::EntityRef(_) => "entity_ref",
        RuntimeValue::Tuple(_) => "tuple",
        RuntimeValue::BracketSeq(_) => "bracket_seq",
        RuntimeValue::Record(_) => "record",
        RuntimeValue::Variant { .. } => "variant",
    }
    .to_owned()
}

fn compile_helper(
    mode: RuntimePureBackendMode,
    helper: &RuntimePureHelper,
) -> RuntimePureCacheEntry {
    match mode {
        RuntimePureBackendMode::Vm => RuntimePureCacheEntry::Vm,
        RuntimePureBackendMode::Aot => compile_aot(helper).unwrap_or(RuntimePureCacheEntry::Vm),
        RuntimePureBackendMode::Jit => compile_jit(helper)
            .unwrap_or_else(|| compile_aot(helper).unwrap_or(RuntimePureCacheEntry::Vm)),
        RuntimePureBackendMode::Auto => compile_jit(helper)
            .or_else(|| compile_aot(helper))
            .unwrap_or(RuntimePureCacheEntry::Vm),
    }
}

fn compile_jit(helper: &RuntimePureHelper) -> Option<RuntimePureCacheEntry> {
    CraneliftPureFunctionBackend
        .compile_i64_with_inputs(
            &compile_request(helper),
            helper.input_names.iter().map(String::as_str),
        )
        .ok()
        .map(Box::new)
        .map(RuntimePureCacheEntry::Jit)
}

fn compile_aot(helper: &RuntimePureHelper) -> Option<RuntimePureCacheEntry> {
    AotPureFunctionBackend::new()
        .compile_i64_with_inputs(
            &compile_request(helper),
            helper.input_names.iter().map(String::as_str),
        )
        .ok()
        .map(RuntimePureCacheEntry::Aot)
}

fn compile_request(helper: &RuntimePureHelper) -> PureFunctionRequest {
    PureFunctionRequest::new(
        helper.name.clone(),
        helper.expr.clone(),
        helper
            .input_names
            .iter()
            .cloned()
            .map(|name| RuntimeBinding {
                name,
                value: RuntimeValue::Int(0),
            }),
    )
}

fn evaluate_vm(
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
    let request = PureFunctionRequest::new(
        helper.name.clone(),
        helper.expr.clone(),
        helper
            .input_names
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .map(|(name, value)| RuntimeBinding { name, value }),
    );
    VmPureFunctionBackend
        .evaluate(&request)
        .map(|result| result.value)
}

/// Summary of helpers selected for acceleration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePureAccelerationSummary {
    pub annotated: usize,
    pub inferred: usize,
    pub jit: usize,
    pub aot: usize,
    pub vm: usize,
}

impl RuntimePureAccelerator {
    pub fn summary(&self, helpers: &[RuntimePureHelper]) -> RuntimePureAccelerationSummary {
        let annotated = helpers
            .iter()
            .filter(|helper| helper.origin == RuntimePureHelperOrigin::Annotated)
            .count();
        let inferred = helpers.len().saturating_sub(annotated);
        let mut jit = 0;
        let mut aot = 0;
        let mut vm = 0;
        for entry in self.cache.values() {
            match entry {
                RuntimePureCacheEntry::Jit(_) => jit += 1,
                RuntimePureCacheEntry::Aot(_) => aot += 1,
                RuntimePureCacheEntry::Vm => vm += 1,
            }
        }
        RuntimePureAccelerationSummary {
            annotated,
            inferred,
            jit,
            aot,
            vm,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::{
        plan::RuntimePureHelperId,
        value::{RuntimeBinaryOp, RuntimeExpr},
    };

    #[test]
    fn auto_accelerator_calls_jit_without_value_vec_allocation() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "score".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(2))),
                }),
            },
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::new(
            RuntimePureBackendMode::Auto,
            std::slice::from_ref(&helper),
        );

        let value = accelerator
            .call_i64(&helper, RuntimeI64Args::new([3, 4, 0, 0], 2))
            .expect("accelerated call succeeds");

        assert_eq!(value, Some(18));
        assert_eq!(accelerator.stats().pure_calls, 1);
        assert_eq!(accelerator.stats().arg_stack_packs, 1);
        assert_eq!(accelerator.stats().arg_vec_allocations, 0);
        assert_eq!(accelerator.summary(&[helper]).jit, 1);
    }
}
