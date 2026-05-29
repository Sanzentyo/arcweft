use crate::value::{
    RuntimeBinding, RuntimeEnv, RuntimeEvalError, RuntimeExpr, RuntimeFieldValue, RuntimeValue,
    evaluate_binary, evaluate_unary, runtime_value_label,
};

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

/// AOT boundary that currently delegates to the VM fallback.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AotPureFunctionBackend {
    vm: VmPureFunctionBackend,
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
        let mut evaluator = PureEvaluator::new(request.bindings.clone());
        let value = evaluator.evaluate_expr(&request.expr)?;
        Ok(PureFunctionResult {
            backend: self.kind(),
            value,
            stats: evaluator.stats,
        })
    }
}

impl AotPureFunctionBackend {
    pub const fn new() -> Self {
        Self {
            vm: VmPureFunctionBackend,
        }
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
        let mut result = self.vm.evaluate(request)?;
        result.backend = self.kind();
        Ok(result)
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

impl PureEvaluator {
    fn new(bindings: Vec<RuntimeBinding>) -> Self {
        let mut env = RuntimeEnv::default();
        env.bind_all(bindings);
        Self {
            env,
            stats: PureFunctionStats::default(),
        }
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
            RuntimeExpr::Tuple(items) => items
                .iter()
                .map(|item| self.evaluate_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::Tuple),
            RuntimeExpr::BracketSeq(items) => items
                .iter()
                .map(|item| self.evaluate_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::BracketSeq),
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
            RuntimeExpr::Field { target, field } => {
                let value = self.evaluate_expr(target)?;
                match value {
                    RuntimeValue::Record(fields) => fields
                        .into_iter()
                        .find(|candidate| candidate.name == *field)
                        .map(|field| field.value)
                        .ok_or_else(|| RuntimeEvalError::MissingField {
                            field: field.clone(),
                            value: "record".to_owned(),
                        }),
                    value => Err(RuntimeEvalError::MissingField {
                        field: field.clone(),
                        value: runtime_value_label(&value),
                    }),
                }
            }
            RuntimeExpr::Call { callee, args } => self.evaluate_call_expr(callee, args),
            RuntimeExpr::MethodCall {
                receiver,
                method,
                args,
            } => self.evaluate_method_call_expr(receiver, method, args),
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

    fn evaluate_call_expr(
        &mut self,
        callee: &str,
        args: &[RuntimeExpr],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.evaluated_calls += 1;
        let args = args
            .iter()
            .map(|arg| self.evaluate_expr(arg))
            .collect::<Result<Vec<_>, _>>()?;
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

    fn evaluate_method_call_expr(
        &mut self,
        receiver: &RuntimeExpr,
        method: &str,
        args: &[RuntimeExpr],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.evaluated_method_calls += 1;
        let receiver = self.evaluate_expr(receiver)?;
        let args = args
            .iter()
            .map(|arg| self.evaluate_expr(arg))
            .collect::<Result<Vec<_>, _>>()?;
        match (receiver, method, args.as_slice()) {
            (RuntimeValue::String(value), "trim", []) => {
                Ok(RuntimeValue::String(value.trim().to_owned()))
            }
            (RuntimeValue::String(value), "to_string", []) => Ok(RuntimeValue::String(value)),
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
}
