use super::{
    AotPureFunctionBackend, AotPureI64Plan, AotPureScalarPlan, PureFunctionBackend,
    PureFunctionBackendKind, PureFunctionRequest, PureFunctionResult, PureFunctionStats,
    RuntimePureScalar, RuntimePureScalarInteger, evaluate_scalar_binary, evaluate_scalar_unary,
    runtime_value_as_scalar,
};
use crate::plan::{RuntimePureHelper, RuntimePureInputType, RuntimePureOutputType};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::value::{
    RuntimeBinaryOp, RuntimeCallArgumentMode, RuntimeEvalError, RuntimeExpr, RuntimeExprKind,
    RuntimeIntrinsic, RuntimeUnaryOp, RuntimeValue,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum AotI64Expr {
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

#[derive(Clone, Debug, PartialEq)]
pub(super) enum AotBoolExpr {
    Const(bool),
    Compare {
        lhs: Box<AotI64Expr>,
        op: RuntimeBinaryOp,
        rhs: Box<AotI64Expr>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum AotScalarExpr {
    Const(RuntimePureScalar),
    Local(usize),
    Let {
        slot: usize,
        expr: Box<AotScalarExpr>,
        body: Box<AotScalarExpr>,
    },
    AddCall {
        lhs: Box<AotScalarExpr>,
        rhs: Box<AotScalarExpr>,
    },
    Unary {
        op: RuntimeUnaryOp,
        expr: Box<AotScalarExpr>,
    },
    Binary {
        lhs: Box<AotScalarExpr>,
        op: RuntimeBinaryOp,
        rhs: Box<AotScalarExpr>,
    },
    If {
        condition: AotScalarBoolExpr,
        then_expr: Box<AotScalarExpr>,
        else_expr: Box<AotScalarExpr>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum AotScalarBoolExpr {
    Const(bool),
    Compare {
        lhs: Box<AotScalarExpr>,
        op: RuntimeBinaryOp,
        rhs: Box<AotScalarExpr>,
    },
}

#[derive(Clone, Debug)]
struct AotCompileContext {
    slots: BTreeMap<RuntimeLocalDeclarationId, usize>,
    next_slot: usize,
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
        input_locals: impl IntoIterator<Item = RuntimeLocalDeclarationId>,
    ) -> Result<AotPureI64Plan, RuntimeEvalError> {
        AotPureI64Plan::compile_with_inputs(request, input_locals)
    }

    /// Compiles a deterministic helper request to an exact-width scalar AOT plan.
    pub fn compile_scalar_with_inputs(
        &self,
        request: &PureFunctionRequest,
        input_locals: impl IntoIterator<Item = RuntimeLocalDeclarationId>,
        input_type: RuntimePureInputType,
        output_type: RuntimePureOutputType,
    ) -> Result<AotPureScalarPlan, RuntimeEvalError> {
        AotPureScalarPlan::compile_with_inputs(request, input_locals, input_type, output_type)
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
            value: RuntimeValue::i64(value),
            stats,
        })
    }
}

impl AotPureI64Plan {
    fn compile(request: &PureFunctionRequest) -> Result<Self, RuntimeEvalError> {
        Self::compile_with_inputs(request, std::iter::empty())
    }

    fn compile_with_inputs(
        request: &PureFunctionRequest,
        input_locals: impl IntoIterator<Item = RuntimeLocalDeclarationId>,
    ) -> Result<Self, RuntimeEvalError> {
        let helper = request.helper_ref()?.declaration();
        validate_output_abi(helper, RuntimePureOutputType::I64)?;
        let input_locals = input_locals.into_iter().collect::<Vec<_>>();
        for &local in &input_locals {
            validate_input_abi(request, helper, local, RuntimePureInputType::I64)?;
        }
        let mut ctx = AotCompileContext::from_request(request)?;
        let expr = compile_aot_i64_expr(&helper.name, &helper.expr, &mut ctx)?;
        let slot_count = ctx.next_slot;
        let input_slots = input_locals
            .into_iter()
            .map(|local| ctx.input_slot(&helper.name, local))
            .collect::<Result<Vec<_>, _>>()?;
        let mut initial_slots = AotCompileContext::initial_slots(request)?;
        initial_slots.resize(slot_count, 0);
        Ok(Self {
            plan: std::sync::Arc::clone(request.plan()),
            helper: request.helper_id(),
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
                self.name(),
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
                self.name(),
                format!(
                    "AOT helper expected {} input(s), got {}",
                    self.input_slots.len(),
                    inputs.len()
                ),
            ));
        }
        self.reset_scratch_slots(slots);
        for (slot, value) in self.input_slots.iter().zip(inputs.iter().copied()) {
            slots[*slot] = value;
        }
        Ok(self.evaluate_with_slot_slice(slots))
    }

    fn reset_scratch_slots(&self, slots: &mut Vec<i64>) {
        if slots.len() == self.slot_count {
            slots.copy_from_slice(&self.initial_slots);
        } else {
            slots.clear();
            slots.extend_from_slice(&self.initial_slots);
            if slots.len() < self.slot_count {
                slots.resize(self.slot_count, 0);
            }
        }
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
        self.plan.pure_helpers()[self.helper.0].name.as_str()
    }
}

impl AotPureScalarPlan {
    fn compile_with_inputs(
        request: &PureFunctionRequest,
        input_locals: impl IntoIterator<Item = RuntimeLocalDeclarationId>,
        input_type: RuntimePureInputType,
        output_type: RuntimePureOutputType,
    ) -> Result<Self, RuntimeEvalError> {
        let helper = request.helper_ref()?.declaration();
        validate_output_abi(helper, output_type)?;
        let input_locals = input_locals.into_iter().collect::<Vec<_>>();
        for &local in &input_locals {
            validate_input_abi(request, helper, local, input_type)?;
        }
        let mut ctx = AotCompileContext::from_scalar_request(request)?;
        let expr = compile_aot_scalar_expr(&helper.name, &helper.expr, &mut ctx)?;
        let slot_count = ctx.next_slot;
        let input_slots = input_locals
            .into_iter()
            .map(|local| ctx.input_slot(&helper.name, local))
            .collect::<Result<Vec<_>, _>>()?;
        let mut initial_slots = AotCompileContext::initial_scalar_slots(request)?;
        initial_slots.resize(
            slot_count,
            RuntimePureScalar::default_for_output(output_type)?,
        );
        Ok(Self {
            plan: std::sync::Arc::clone(request.plan()),
            helper: request.helper_id(),
            expr,
            initial_slots,
            input_slots,
            input_type,
            output_type,
            slot_count,
        })
    }

    /// Calls the compiled helper with exact-width integer inputs.
    pub fn call_exact_int_with_inputs_scratch<T: RuntimePureScalarInteger>(
        &self,
        inputs: &[T],
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(T, PureFunctionStats), RuntimeEvalError> {
        if self.input_type != T::INPUT_TYPE || self.output_type != T::OUTPUT_TYPE {
            return Err(unsupported_aot(
                self.name(),
                "AOT scalar helper type does not match exact integer call",
            ));
        }
        let (value, stats) = self.call_with_scalar_inputs_scratch(
            inputs
                .iter()
                .copied()
                .map(RuntimePureScalarInteger::into_pure_scalar),
            inputs.len(),
            slots,
        )?;
        T::try_from_runtime_value(self.name(), value.into_runtime_value())
            .map(|value| (value, stats))
    }

    /// Calls the compiled helper with `f32` inputs.
    pub fn call_f32_with_inputs_scratch(
        &self,
        inputs: &[f32],
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(f32, PureFunctionStats), RuntimeEvalError> {
        if self.input_type != RuntimePureInputType::F32
            || self.output_type != RuntimePureOutputType::F32
        {
            return Err(unsupported_aot(
                self.name(),
                "AOT scalar helper type does not match f32 call",
            ));
        }
        let (value, stats) = self.call_with_scalar_inputs_scratch(
            inputs.iter().copied().map(RuntimePureScalar::F32),
            inputs.len(),
            slots,
        )?;
        match value {
            RuntimePureScalar::F32(value) => Ok((value, stats)),
            value => Err(unsupported_aot(
                self.name(),
                format!("AOT scalar f32 result expected f32, got {}", value.label()),
            )),
        }
    }

    /// Calls the compiled helper with `f64` inputs.
    pub fn call_f64_with_inputs_scratch(
        &self,
        inputs: &[f64],
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(f64, PureFunctionStats), RuntimeEvalError> {
        if self.input_type != RuntimePureInputType::F64
            || self.output_type != RuntimePureOutputType::F64
        {
            return Err(unsupported_aot(
                self.name(),
                "AOT scalar helper type does not match f64 call",
            ));
        }
        let (value, stats) = self.call_with_scalar_inputs_scratch(
            inputs.iter().copied().map(RuntimePureScalar::F64),
            inputs.len(),
            slots,
        )?;
        match value {
            RuntimePureScalar::F64(value) => Ok((value, stats)),
            value => Err(unsupported_aot(
                self.name(),
                format!("AOT scalar f64 result expected f64, got {}", value.label()),
            )),
        }
    }

    fn call_with_scalar_inputs_scratch(
        &self,
        inputs: impl IntoIterator<Item = RuntimePureScalar>,
        input_len: usize,
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(RuntimePureScalar, PureFunctionStats), RuntimeEvalError> {
        if input_len != self.input_slots.len() {
            return Err(unsupported_aot(
                self.name(),
                format!(
                    "AOT helper expected {} input(s), got {}",
                    self.input_slots.len(),
                    input_len
                ),
            ));
        }
        self.reset_scratch_slots(slots)?;
        for (slot, value) in self.input_slots.iter().zip(inputs) {
            slots[*slot] = value;
        }
        self.evaluate_with_slot_slice(slots)
    }

    fn reset_scratch_slots(
        &self,
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(), RuntimeEvalError> {
        if slots.len() == self.slot_count {
            slots.copy_from_slice(&self.initial_slots);
        } else {
            slots.clear();
            slots.extend_from_slice(&self.initial_slots);
            if slots.len() < self.slot_count {
                slots.resize(
                    self.slot_count,
                    RuntimePureScalar::default_for_output(self.output_type)?,
                );
            }
        }
        Ok(())
    }

    fn evaluate_with_slot_slice(
        &self,
        slots: &mut [RuntimePureScalar],
    ) -> Result<(RuntimePureScalar, PureFunctionStats), RuntimeEvalError> {
        let mut evaluator = AotScalarEvaluator {
            name: self.name(),
            slots,
            stats: PureFunctionStats::default(),
        };
        let value = evaluator.eval_scalar(&self.expr)?;
        Ok((value, evaluator.stats))
    }

    /// Helper name captured from the original request.
    pub fn name(&self) -> &str {
        self.plan.pure_helpers()[self.helper.0].name.as_str()
    }
}

impl AotCompileContext {
    fn from_request(request: &PureFunctionRequest) -> Result<Self, RuntimeEvalError> {
        let helper = request.helper_ref()?.declaration();
        let mut slots = BTreeMap::new();
        for binding in request.bindings() {
            if !matches!(
                binding.value,
                RuntimeValue::Int(crate::value::RuntimeInt::I64(_))
            ) {
                return Err(unsupported_aot(
                    &helper.name,
                    format!("local {} is not an i64 integer", binding.local),
                ));
            }
            if slots.insert(binding.local, slots.len()).is_some() {
                return Err(unsupported_aot(
                    &helper.name,
                    format!("local {} is duplicated", binding.local),
                ));
            }
        }
        let next_slot = slots.len();
        Ok(Self { slots, next_slot })
    }

    fn initial_slots(request: &PureFunctionRequest) -> Result<Vec<i64>, RuntimeEvalError> {
        let helper = request.helper_ref()?.declaration();
        request
            .bindings()
            .iter()
            .map(|binding| match binding.value {
                RuntimeValue::Int(value) => value.exact_i64().ok_or_else(|| {
                    unsupported_aot(
                        &helper.name,
                        format!("local {} is not an i64 integer", binding.local),
                    )
                }),
                _ => Err(unsupported_aot(
                    &helper.name,
                    format!("local {} is not an i64 integer", binding.local),
                )),
            })
            .collect()
    }

    fn from_scalar_request(request: &PureFunctionRequest) -> Result<Self, RuntimeEvalError> {
        let helper = request.helper_ref()?.declaration();
        let mut slots = BTreeMap::new();
        for binding in request.bindings() {
            if runtime_value_as_scalar(&binding.value).is_none() {
                return Err(unsupported_aot(
                    &helper.name,
                    format!("local {} is not a scalar value", binding.local),
                ));
            }
            if slots.insert(binding.local, slots.len()).is_some() {
                return Err(unsupported_aot(
                    &helper.name,
                    format!("local {} is duplicated", binding.local),
                ));
            }
        }
        let next_slot = slots.len();
        Ok(Self { slots, next_slot })
    }

    fn initial_scalar_slots(
        request: &PureFunctionRequest,
    ) -> Result<Vec<RuntimePureScalar>, RuntimeEvalError> {
        let helper = request.helper_ref()?.declaration();
        request
            .bindings()
            .iter()
            .map(|binding| {
                runtime_value_as_scalar(&binding.value).ok_or_else(|| {
                    unsupported_aot(
                        &helper.name,
                        format!("local {} is not a scalar value", binding.local),
                    )
                })
            })
            .collect()
    }

    fn local_slot(&self, local: RuntimeLocalDeclarationId) -> Option<usize> {
        self.slots.get(&local).copied()
    }

    fn input_slot(
        &self,
        helper_name: &str,
        local: RuntimeLocalDeclarationId,
    ) -> Result<usize, RuntimeEvalError> {
        self.local_slot(local).ok_or_else(|| {
            unsupported_aot(
                helper_name,
                format!("AOT runtime input local {local} is not a helper binding"),
            )
        })
    }

    fn with_let_binding<T>(
        &mut self,
        local: RuntimeLocalDeclarationId,
        compile_body: impl FnOnce(&mut Self, usize) -> Result<T, RuntimeEvalError>,
    ) -> Result<T, RuntimeEvalError> {
        let slot = self.next_slot;
        self.next_slot += 1;
        let previous = self.slots.insert(local, slot);
        let body = compile_body(self, slot);
        if let Some(previous) = previous {
            self.slots.insert(local, previous);
        } else {
            self.slots.remove(&local);
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
                self.eval_i64(lhs).wrapping_add(self.eval_i64(rhs))
            }
            AotI64Expr::Unary { op, expr } => match op {
                RuntimeUnaryOp::Neg => self.eval_i64(expr).wrapping_neg(),
                RuntimeUnaryOp::Not => unreachable!("bool unary is not compiled as i64"),
            },
            AotI64Expr::Binary { lhs, op, rhs } => {
                self.stats.evaluated_binary_ops += 1;
                let lhs = self.eval_i64(lhs);
                let rhs = self.eval_i64(rhs);
                match op {
                    RuntimeBinaryOp::Add => lhs.wrapping_add(rhs),
                    RuntimeBinaryOp::Sub => lhs.wrapping_sub(rhs),
                    RuntimeBinaryOp::Mul => lhs.wrapping_mul(rhs),
                    RuntimeBinaryOp::Div => lhs.wrapping_div(rhs),
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

struct AotScalarEvaluator<'a> {
    name: &'a str,
    slots: &'a mut [RuntimePureScalar],
    stats: PureFunctionStats,
}

impl AotScalarEvaluator<'_> {
    fn eval_scalar(&mut self, expr: &AotScalarExpr) -> Result<RuntimePureScalar, RuntimeEvalError> {
        self.stats.evaluated_exprs += 1;
        match expr {
            AotScalarExpr::Const(value) => Ok(*value),
            AotScalarExpr::Local(slot) => Ok(self.slots[*slot]),
            AotScalarExpr::Let { slot, expr, body } => {
                let value = self.eval_scalar(expr)?;
                let previous = self.slots[*slot];
                self.slots[*slot] = value;
                let result = self.eval_scalar(body);
                self.slots[*slot] = previous;
                result
            }
            AotScalarExpr::AddCall { lhs, rhs } => {
                self.stats.evaluated_calls += 1;
                evaluate_scalar_binary(
                    self.eval_scalar(lhs)?,
                    RuntimeBinaryOp::Add,
                    self.eval_scalar(rhs)?,
                )
            }
            AotScalarExpr::Unary { op, expr } => {
                evaluate_scalar_unary(*op, self.eval_scalar(expr)?)
            }
            AotScalarExpr::Binary { lhs, op, rhs } => {
                self.stats.evaluated_binary_ops += 1;
                evaluate_scalar_binary(self.eval_scalar(lhs)?, *op, self.eval_scalar(rhs)?)
            }
            AotScalarExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                if self.eval_bool(condition)? {
                    self.eval_scalar(then_expr)
                } else {
                    self.eval_scalar(else_expr)
                }
            }
        }
        .map_err(|error| match error {
            RuntimeEvalError::UnsupportedPure { .. } => error,
            other => unsupported_aot(self.name, other.to_string()),
        })
    }

    fn eval_bool(&mut self, expr: &AotScalarBoolExpr) -> Result<bool, RuntimeEvalError> {
        self.stats.evaluated_exprs += 1;
        match expr {
            AotScalarBoolExpr::Const(value) => Ok(*value),
            AotScalarBoolExpr::Compare { lhs, op, rhs } => {
                self.stats.evaluated_binary_ops += 1;
                match evaluate_scalar_binary(self.eval_scalar(lhs)?, *op, self.eval_scalar(rhs)?)? {
                    RuntimePureScalar::Bool(value) => Ok(value),
                    value => Err(unsupported_aot(
                        self.name,
                        format!("condition expected bool, got {}", value.label()),
                    )),
                }
            }
        }
    }
}

fn validate_input_abi(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    local: RuntimeLocalDeclarationId,
    expected: RuntimePureInputType,
) -> Result<(), RuntimeEvalError> {
    let declaration = request
        .plan()
        .local_declarations()
        .get(local)
        .ok_or(RuntimeEvalError::UnknownLocal(local))?;
    let Some(position) = helper
        .input_locals
        .iter()
        .position(|candidate| *candidate == local)
    else {
        return Err(RuntimeEvalError::InvalidExpressionType(declaration.ty()));
    };
    if helper.input_types.get(position).copied() == Some(expected) {
        Ok(())
    } else {
        Err(RuntimeEvalError::InvalidExpressionType(declaration.ty()))
    }
}

fn validate_output_abi(
    helper: &RuntimePureHelper,
    expected: RuntimePureOutputType,
) -> Result<(), RuntimeEvalError> {
    if helper.output_type == expected {
        Ok(())
    } else {
        Err(RuntimeEvalError::InvalidExpressionType(helper.expr.ty()))
    }
}

fn compile_aot_i64_expr(
    helper_name: &str,
    expr: &RuntimeExpr,
    ctx: &mut AotCompileContext,
) -> Result<AotI64Expr, RuntimeEvalError> {
    match expr.kind() {
        RuntimeExprKind::Value(RuntimeValue::Int(value)) => {
            value.exact_i64().map(AotI64Expr::Const).ok_or_else(|| {
                unsupported_aot(
                    helper_name,
                    format!("literal `{value}` is not an i64 integer"),
                )
            })
        }
        RuntimeExprKind::Value(value) => Err(unsupported_aot(
            helper_name,
            format!("literal {value:?} is not an i64 integer"),
        )),
        RuntimeExprKind::Local(local) => ctx
            .local_slot(*local)
            .map(AotI64Expr::Local)
            .ok_or(RuntimeEvalError::UnknownLocal(*local)),
        RuntimeExprKind::Let {
            binding,
            expr,
            body,
        } => {
            let expr = compile_aot_i64_expr(helper_name, expr, ctx)?;
            ctx.with_let_binding(*binding, |ctx, slot| {
                Ok(AotI64Expr::Let {
                    slot,
                    expr: Box::new(expr),
                    body: Box::new(compile_aot_i64_expr(helper_name, body, ctx)?),
                })
            })
        }
        RuntimeExprKind::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::Add)
                && args.len() == 2
                && args
                    .iter()
                    .all(|argument| argument.mode() == RuntimeCallArgumentMode::Value) =>
        {
            Ok(AotI64Expr::AddCall {
                lhs: Box::new(compile_aot_i64_expr(helper_name, args[0].value(), ctx)?),
                rhs: Box::new(compile_aot_i64_expr(helper_name, args[1].value(), ctx)?),
            })
        }
        RuntimeExprKind::Unary { op, expr } => match op {
            RuntimeUnaryOp::Neg => Ok(AotI64Expr::Unary {
                op: *op,
                expr: Box::new(compile_aot_i64_expr(helper_name, expr, ctx)?),
            }),
            RuntimeUnaryOp::Not => Err(unsupported_aot(
                helper_name,
                "boolean negation is not an i64 result",
            )),
        },
        RuntimeExprKind::Binary { lhs, op, rhs } if is_aot_i64_binary(*op) => {
            Ok(AotI64Expr::Binary {
                lhs: Box::new(compile_aot_i64_expr(helper_name, lhs, ctx)?),
                op: *op,
                rhs: Box::new(compile_aot_i64_expr(helper_name, rhs, ctx)?),
            })
        }
        RuntimeExprKind::If {
            condition,
            then_expr,
            else_expr,
        } => Ok(AotI64Expr::If {
            condition: compile_aot_bool_expr(helper_name, condition, ctx)?,
            then_expr: Box::new(compile_aot_i64_expr(helper_name, then_expr, ctx)?),
            else_expr: Box::new(compile_aot_i64_expr(helper_name, else_expr, ctx)?),
        }),
        RuntimeExprKind::Call { callee, .. } => Err(unsupported_aot(
            helper_name,
            format!("call `{callee}` is outside the AOT i64 subset"),
        )),
        RuntimeExprKind::PureCall { .. } => Err(unsupported_aot(
            helper_name,
            "nested runtime pure calls are outside the AOT i64 subset",
        )),
        _ => Err(unsupported_aot(
            helper_name,
            format!(
                "expression with plan type {} is outside the AOT i64 subset",
                expr.ty()
            ),
        )),
    }
}

fn compile_aot_bool_expr(
    helper_name: &str,
    expr: &RuntimeExpr,
    ctx: &mut AotCompileContext,
) -> Result<AotBoolExpr, RuntimeEvalError> {
    match expr.kind() {
        RuntimeExprKind::Value(RuntimeValue::Bool(value)) => Ok(AotBoolExpr::Const(*value)),
        RuntimeExprKind::Binary { lhs, op, rhs } if is_aot_comparison(*op) => {
            Ok(AotBoolExpr::Compare {
                lhs: Box::new(compile_aot_i64_expr(helper_name, lhs, ctx)?),
                op: *op,
                rhs: Box::new(compile_aot_i64_expr(helper_name, rhs, ctx)?),
            })
        }
        _ => Err(unsupported_aot(
            helper_name,
            format!(
                "condition with plan type {} is outside the AOT i64 subset",
                expr.ty()
            ),
        )),
    }
}

fn compile_aot_scalar_expr(
    helper_name: &str,
    expr: &RuntimeExpr,
    ctx: &mut AotCompileContext,
) -> Result<AotScalarExpr, RuntimeEvalError> {
    match expr.kind() {
        RuntimeExprKind::Value(value) => runtime_value_as_scalar(value)
            .map(AotScalarExpr::Const)
            .ok_or_else(|| {
                unsupported_aot(
                    helper_name,
                    format!("literal {value:?} is not a scalar value"),
                )
            }),
        RuntimeExprKind::Local(local) => ctx
            .local_slot(*local)
            .map(AotScalarExpr::Local)
            .ok_or(RuntimeEvalError::UnknownLocal(*local)),
        RuntimeExprKind::Let {
            binding,
            expr,
            body,
        } => {
            let expr = compile_aot_scalar_expr(helper_name, expr, ctx)?;
            ctx.with_let_binding(*binding, |ctx, slot| {
                Ok(AotScalarExpr::Let {
                    slot,
                    expr: Box::new(expr),
                    body: Box::new(compile_aot_scalar_expr(helper_name, body, ctx)?),
                })
            })
        }
        RuntimeExprKind::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::Add)
                && args.len() == 2
                && args
                    .iter()
                    .all(|argument| argument.mode() == RuntimeCallArgumentMode::Value) =>
        {
            Ok(AotScalarExpr::AddCall {
                lhs: Box::new(compile_aot_scalar_expr(helper_name, args[0].value(), ctx)?),
                rhs: Box::new(compile_aot_scalar_expr(helper_name, args[1].value(), ctx)?),
            })
        }
        RuntimeExprKind::Unary { op, expr } => match op {
            RuntimeUnaryOp::Neg => Ok(AotScalarExpr::Unary {
                op: *op,
                expr: Box::new(compile_aot_scalar_expr(helper_name, expr, ctx)?),
            }),
            RuntimeUnaryOp::Not => Err(unsupported_aot(
                helper_name,
                "boolean negation is not a scalar result",
            )),
        },
        RuntimeExprKind::Binary { lhs, op, rhs } if is_aot_scalar_binary(*op) => {
            Ok(AotScalarExpr::Binary {
                lhs: Box::new(compile_aot_scalar_expr(helper_name, lhs, ctx)?),
                op: *op,
                rhs: Box::new(compile_aot_scalar_expr(helper_name, rhs, ctx)?),
            })
        }
        RuntimeExprKind::If {
            condition,
            then_expr,
            else_expr,
        } => Ok(AotScalarExpr::If {
            condition: compile_aot_scalar_bool_expr(helper_name, condition, ctx)?,
            then_expr: Box::new(compile_aot_scalar_expr(helper_name, then_expr, ctx)?),
            else_expr: Box::new(compile_aot_scalar_expr(helper_name, else_expr, ctx)?),
        }),
        RuntimeExprKind::Call { callee, .. } => Err(unsupported_aot(
            helper_name,
            format!("call `{callee}` is outside the AOT scalar subset"),
        )),
        RuntimeExprKind::PureCall { .. } => Err(unsupported_aot(
            helper_name,
            "nested runtime pure calls are outside the AOT scalar subset",
        )),
        _ => Err(unsupported_aot(
            helper_name,
            format!(
                "expression with plan type {} is outside the AOT scalar subset",
                expr.ty()
            ),
        )),
    }
}

fn compile_aot_scalar_bool_expr(
    helper_name: &str,
    expr: &RuntimeExpr,
    ctx: &mut AotCompileContext,
) -> Result<AotScalarBoolExpr, RuntimeEvalError> {
    match expr.kind() {
        RuntimeExprKind::Value(RuntimeValue::Bool(value)) => Ok(AotScalarBoolExpr::Const(*value)),
        RuntimeExprKind::Binary { lhs, op, rhs } if is_aot_comparison(*op) => {
            Ok(AotScalarBoolExpr::Compare {
                lhs: Box::new(compile_aot_scalar_expr(helper_name, lhs, ctx)?),
                op: *op,
                rhs: Box::new(compile_aot_scalar_expr(helper_name, rhs, ctx)?),
            })
        }
        _ => Err(unsupported_aot(
            helper_name,
            format!(
                "condition with plan type {} is outside the AOT scalar subset",
                expr.ty()
            ),
        )),
    }
}

fn is_aot_i64_binary(op: RuntimeBinaryOp) -> bool {
    matches!(
        op,
        RuntimeBinaryOp::Add | RuntimeBinaryOp::Sub | RuntimeBinaryOp::Mul | RuntimeBinaryOp::Div
    )
}

fn is_aot_scalar_binary(op: RuntimeBinaryOp) -> bool {
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
