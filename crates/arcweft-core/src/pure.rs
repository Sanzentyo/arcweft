use crate::plan::RuntimePureHelper;
use crate::step::RuntimePureCallStats;
use crate::value::{
    RuntimeBinaryOp, RuntimeBinding, RuntimeEnv, RuntimeEvalError, RuntimeExpr, RuntimeFieldValue,
    RuntimeUnaryOp, RuntimeValue, evaluate_binary, evaluate_unary, runtime_value_label,
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

/// Fixed-size integer argument pack for runtime pure helper fast paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeI64Args {
    len: usize,
    values: [i64; Self::MAX],
}

/// Runtime-facing backend for deterministic pure helper calls.
pub trait RuntimePureCallBackend {
    fn call_i64(
        &mut self,
        helper: &RuntimePureHelper,
        args: RuntimeI64Args,
    ) -> Result<Option<i64>, RuntimeEvalError>;

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

/// AOT backend for deterministic pure helpers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AotPureFunctionBackend;

/// VM runtime backend used when no external pure accelerator is provided.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VmRuntimePureCallBackend {
    stats: RuntimePureCallStats,
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
        let mut evaluator = PureEvaluator::new(request.bindings.clone());
        let value = evaluator.evaluate_expr(&request.expr)?;
        Ok(PureFunctionResult {
            backend: self.kind(),
            value,
            stats: evaluator.stats,
        })
    }
}

impl VmPureFunctionBackend {
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
        if args.len() != helper.input_names.len() {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: helper.input_names.len(),
                found: args.len(),
            });
        }
        let mut evaluator = PureEvaluator::new_i64_slice(&helper.input_names, args);
        evaluator.evaluate_expr(&helper.expr)
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

impl RuntimeI64Args {
    pub const MAX: usize = 4;

    pub const fn new(values: [i64; Self::MAX], len: usize) -> Self {
        Self { len, values }
    }

    pub const fn len(self) -> usize {
        self.len
    }

    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    pub fn as_slice(&self) -> &[i64] {
        &self.values[..self.len]
    }
}

impl RuntimePureCallBackend for VmRuntimePureCallBackend {
    fn call_i64(
        &mut self,
        helper: &RuntimePureHelper,
        args: RuntimeI64Args,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_stack_packs += 1;
        let value = VmPureFunctionBackend.evaluate_i64_args(helper, args)?;
        match value {
            RuntimeValue::Int(value) => Ok(Some(value)),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
        }
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
        self.stats.arg_vec_allocations += 1;
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
        slots.clear();
        slots.extend_from_slice(&self.initial_slots);
        if slots.len() < self.slot_count {
            slots.resize(self.slot_count, 0);
        }
        for (slot, value) in self.input_slots.iter().zip(inputs.iter().copied()) {
            slots[*slot] = value;
        }
        Ok(self.evaluate_with_slot_slice(slots))
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

impl PureEvaluator {
    fn new(bindings: Vec<RuntimeBinding>) -> Self {
        let mut env = RuntimeEnv::default();
        env.bind_all(bindings);
        Self {
            env,
            stats: PureFunctionStats::default(),
        }
    }

    fn new_i64_slice(input_names: &[String], args: &[i64]) -> Self {
        let mut env = RuntimeEnv::default();
        input_names
            .iter()
            .zip(args.iter().copied())
            .for_each(|(name, value)| env.set(name.clone(), RuntimeValue::Int(value)));
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
            RuntimeExpr::Let { name, expr, body } => {
                let value = self.evaluate_expr(expr)?;
                self.env.push_scope();
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

fn spread_runtime_values(value: RuntimeValue) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
    match value {
        RuntimeValue::Tuple(items) | RuntimeValue::BracketSeq(items) => Ok(items),
        value => Err(RuntimeEvalError::InvalidSpread(runtime_value_label(&value))),
    }
}
