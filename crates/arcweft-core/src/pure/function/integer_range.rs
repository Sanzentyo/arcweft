//! Conservative integer bounds over the original admitted numeric body.
//! These borrowed facts authorize result elision, never callback execution.

use super::{RuntimePureFunctionBodyRef, RuntimePureFunctionRef};
use crate::pattern::{RuntimePattern, RuntimePatternKind};
use crate::plan::{FlowOp, RuntimePureOutputType};
use crate::pure::{RuntimePureControlBindings, RuntimePureScalar};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::scope::{RuntimeScopeExitTarget, RuntimeScopeFrameKind};
use crate::value::{RuntimeBinaryOp, RuntimeExpr, RuntimeExprKind, RuntimeUnaryOp, RuntimeValue};
use std::collections::BTreeMap;

/// `Unknown` includes every value of the original type, including u128 values
/// above `i128::MAX` and any native arithmetic wrap that this analysis cannot bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::pure) enum RuntimeIntegerRange {
    Unknown,
    Interval { lower: i128, upper: i128 },
}

impl RuntimeIntegerRange {
    fn point(value: i128) -> Self {
        Self::Interval {
            lower: value,
            upper: value,
        }
    }

    pub(in crate::pure) fn from_scalar(value: RuntimePureScalar) -> Self {
        use RuntimePureScalar as S;
        let value = match value {
            S::Bool(value) => i128::from(value),
            S::I8(value) => i128::from(value),
            S::I16(value) => i128::from(value),
            S::I32(value) => i128::from(value),
            S::I64(value) | S::ISize(value) => i128::from(value),
            S::I128(value) => value,
            S::U8(value) => i128::from(value),
            S::U16(value) => i128::from(value),
            S::U32(value) => i128::from(value),
            S::U64(value) | S::USize(value) => i128::from(value),
            S::U128(value) => match i128::try_from(value) {
                Ok(value) => value,
                Err(_) => return Self::Unknown,
            },
            S::F32(_) | S::F64(_) => return Self::Unknown,
        };
        Self::point(value)
    }

    fn literal(value: &RuntimeValue) -> Self {
        match value {
            RuntimeValue::Int(value) => Self::point(value.as_i128()),
            RuntimeValue::UInt(value) => {
                i128::try_from(value.as_u128()).map_or(Self::Unknown, Self::point)
            }
            RuntimeValue::Bool(value) => Self::point(i128::from(*value)),
            _ => Self::Unknown,
        }
    }

    pub(in crate::pure) fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Interval { lower: a, upper: b }, Self::Interval { lower: c, upper: d }) => {
                Self::Interval {
                    lower: a.min(c),
                    upper: b.max(d),
                }
            }
            _ => Self::Unknown,
        }
    }

    fn fits(self, lower_bound: i128, upper_bound: i128) -> bool {
        matches!(self, Self::Interval { lower, upper }
            if lower >= lower_bound && upper <= upper_bound)
    }

    fn arithmetic(self, op: RuntimeBinaryOp, rhs: Self) -> Self {
        if op == RuntimeBinaryOp::Mul && (self == Self::point(0) || rhs == Self::point(0)) {
            return Self::point(0);
        }
        let (Self::Interval { lower: a, upper: b }, Self::Interval { lower: c, upper: d }) =
            (self, rhs)
        else {
            return Self::Unknown;
        };
        let interval = match op {
            RuntimeBinaryOp::Add => a.checked_add(c).zip(b.checked_add(d)),
            RuntimeBinaryOp::Sub => a.checked_sub(d).zip(b.checked_sub(c)),
            RuntimeBinaryOp::Mul => {
                let products = [
                    a.checked_mul(c),
                    a.checked_mul(d),
                    b.checked_mul(c),
                    b.checked_mul(d),
                ];
                match products {
                    [Some(ac), Some(ad), Some(bc), Some(bd)] => {
                        Some((ac.min(ad).min(bc).min(bd), ac.max(ad).max(bc).max(bd)))
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        interval.map_or(Self::Unknown, |(lower, upper)| Self::Interval {
            lower,
            upper,
        })
    }
}

#[derive(Clone)]
struct RangeContext<'a> {
    function: RuntimePureFunctionRef<'a>,
    control:
        RuntimePureControlBindings<'a, BTreeMap<RuntimeLocalDeclarationId, RuntimeIntegerRange>>,
}

#[derive(Clone, Copy)]
enum RangeFlow {
    Continue,
    Return(RuntimeIntegerRange),
}

impl RuntimePureFunctionRef<'_> {
    pub(in crate::pure) fn integer_result_fits_i64(
        self,
        inputs: impl ExactSizeIterator<Item = RuntimeIntegerRange>,
    ) -> bool {
        if inputs.len() != self.inputs.len() || !self.supports_total_numeric_completion() {
            return false;
        }
        let initial = self
            .inputs
            .iter()
            .zip(inputs)
            .map(|(input, range)| (input.local(), range))
            .collect();
        let mut context = RangeContext {
            function: self,
            control: RuntimePureControlBindings::new(self, initial),
        };
        let result = match self.body {
            RuntimePureFunctionBodyRef::Expression(value) => context.consume(value, 0),
            RuntimePureFunctionBodyRef::Executable(body) => match context.operations(body.ops(), 0)
            {
                Some(RangeFlow::Return(range)) => Some(range),
                _ => None,
            },
        };
        result.is_some_and(|range| range.fits(i128::from(i64::MIN), i128::from(i64::MAX)))
    }
}

impl RangeContext<'_> {
    fn expression(&mut self, value: &RuntimeExpr, depth: usize) -> Option<RuntimeIntegerRange> {
        use RuntimeIntegerRange as R;
        if depth > 128 {
            return None;
        }
        let range = match value.kind() {
            RuntimeExprKind::Value(value) => R::literal(value),
            RuntimeExprKind::Local(read) if read.fields().is_empty() => {
                *self.control.numeric().get(&read.local())?
            }
            RuntimeExprKind::Scope { body, .. } => self.expression(body, depth + 1)?,
            RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => {
                let input = self.expression(expr, depth + 1)?;
                let previous = self.control.numeric_mut().insert(*binding, input);
                let result = self.expression(body, depth + 1);
                match previous {
                    Some(previous) => {
                        self.control.numeric_mut().insert(*binding, previous);
                    }
                    None => {
                        self.control.numeric_mut().remove(binding);
                    }
                }
                return result;
            }
            RuntimeExprKind::Unary {
                op: RuntimeUnaryOp::Neg,
                expr,
            } => match self.expression(expr, depth + 1)? {
                R::Interval { lower, upper } => upper
                    .checked_neg()
                    .zip(lower.checked_neg())
                    .map_or(R::Unknown, |(lower, upper)| R::Interval { lower, upper }),
                R::Unknown => R::Unknown,
            },
            RuntimeExprKind::Binary { lhs, op, rhs } => self
                .expression(lhs, depth + 1)?
                .arithmetic(*op, self.expression(rhs, depth + 1)?),
            RuntimeExprKind::If {
                then_expr,
                else_expr,
                ..
            } => self
                .expression(then_expr, depth + 1)?
                .join(self.expression(else_expr, depth + 1)?),
            RuntimeExprKind::Call { callee, args }
                if callee.as_intrinsic() == Some(crate::value::RuntimeIntrinsic::Add)
                    && args.len() == 2 =>
            {
                self.expression(args[0].value(), depth + 1)?.arithmetic(
                    RuntimeBinaryOp::Add,
                    self.expression(args[1].value(), depth + 1)?,
                )
            }
            _ => R::Unknown,
        };
        // Checked interval arithmetic must also fit the original native ABI.
        // Otherwise native wrapping can put a result outside the computed range.
        let bounds = match super::super::pure_scalar_projection(self.function.plan(), value.ty())? {
            RuntimePureOutputType::I8 => (i128::from(i8::MIN), i128::from(i8::MAX)),
            RuntimePureOutputType::I16 => (i128::from(i16::MIN), i128::from(i16::MAX)),
            RuntimePureOutputType::I32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
            RuntimePureOutputType::I64 | RuntimePureOutputType::ISize => {
                (i128::from(i64::MIN), i128::from(i64::MAX))
            }
            RuntimePureOutputType::I128 => (i128::MIN, i128::MAX),
            RuntimePureOutputType::U8 => (0, i128::from(u8::MAX)),
            RuntimePureOutputType::U16 => (0, i128::from(u16::MAX)),
            RuntimePureOutputType::U32 => (0, i128::from(u32::MAX)),
            RuntimePureOutputType::U64 | RuntimePureOutputType::USize => (0, i128::from(u64::MAX)),
            RuntimePureOutputType::U128 => (0, i128::MAX),
            RuntimePureOutputType::Bool => (0, 1),
            _ => return Some(R::Unknown),
        };
        Some(if range.fits(bounds.0, bounds.1) {
            range
        } else {
            R::Unknown
        })
    }

    fn consume(&mut self, value: &RuntimeExpr, depth: usize) -> Option<RuntimeIntegerRange> {
        let range = self.expression(value, depth)?;
        self.control.consume_numeric_expression(value).ok()?;
        Some(range)
    }

    fn bind(&mut self, pattern: &RuntimePattern, range: RuntimeIntegerRange) -> Option<()> {
        match pattern.kind() {
            RuntimePatternKind::Bind { binding, .. } | RuntimePatternKind::Typed { binding } => {
                self.control.numeric_mut().insert(binding.local(), range);
            }
            RuntimePatternKind::Discard => {}
            _ => return None,
        }
        Some(())
    }

    fn operations(&mut self, operations: &[FlowOp], depth: usize) -> Option<RangeFlow> {
        if depth > 128 {
            return None;
        }
        for operation in operations {
            match operation {
                FlowOp::Let { pattern, expr } => {
                    if let Some(unit) = self.control.evaluate_unit(expr).ok()? {
                        self.control.bind_unit(pattern, unit).ok()?;
                    } else {
                        let range = self.consume(expr, depth + 1)?;
                        self.bind(pattern, range)?;
                    }
                }
                FlowOp::EnterScope { .. } => {
                    self.control
                        .enter_scope(RuntimeScopeFrameKind::EmittedLexical);
                }
                FlowOp::ExitScope => self
                    .control
                    .exit_scope(RuntimeScopeExitTarget::EmittedLexical)
                    .ok()?,
                FlowOp::ExitScopeBind { pattern, expr } => {
                    let unit = self.control.evaluate_unit(expr).ok()?;
                    let range = if unit.is_none() {
                        Some(self.consume(expr, depth + 1)?)
                    } else {
                        None
                    };
                    self.control
                        .exit_scope(RuntimeScopeExitTarget::EmittedLexical)
                        .ok()?;
                    if let Some(unit) = unit {
                        self.control.bind_unit(pattern, unit).ok()?;
                    }
                    if let Some(range) = range {
                        self.bind(pattern, range)?;
                    }
                }
                FlowOp::ReturnExpr(value) => {
                    return Some(RangeFlow::Return(self.consume(value, depth + 1)?));
                }
                FlowOp::Scope { body, .. } => {
                    if !body.is_empty() {
                        let scope = self
                            .control
                            .enter_scope(RuntimeScopeFrameKind::EmittedLexical);
                        let result = self.operations(body, depth + 1)?;
                        if matches!(result, RangeFlow::Return(_)) {
                            return Some(result);
                        }
                        if self.control.contains_scope(scope) {
                            self.control
                                .exit_scope(RuntimeScopeExitTarget::Frame(scope))
                                .ok()?;
                        }
                    }
                }
                FlowOp::If {
                    condition,
                    then_ops,
                    else_ops,
                } => {
                    self.consume(condition, depth + 1)?;
                    let branch = |operations: &[FlowOp]| {
                        let mut context = self.clone();
                        let scope = (!operations.is_empty())
                            .then(|| context.control.enter_scope(RuntimeScopeFrameKind::Control));
                        let result = context.operations(operations, depth + 1)?;
                        if matches!(result, RangeFlow::Continue)
                            && let Some(scope) = scope
                            && context.control.contains_scope(scope)
                        {
                            context
                                .control
                                .exit_scope(RuntimeScopeExitTarget::Frame(scope))
                                .ok()?;
                        }
                        Some((context, result))
                    };
                    let (mut left, left_result) = branch(then_ops)?;
                    let (right, right_result) = branch(else_ops)?;
                    match (left_result, right_result) {
                        (RangeFlow::Return(left), RangeFlow::Return(right)) => {
                            return Some(RangeFlow::Return(left.join(right)));
                        }
                        (RangeFlow::Continue, RangeFlow::Continue) => {
                            if !left.control.compatible_after_branch(&right.control)
                                || !left
                                    .control
                                    .numeric()
                                    .keys()
                                    .eq(right.control.numeric().keys())
                            {
                                return None;
                            }
                            for (local, range) in left.control.numeric_mut() {
                                *range = range.join(*right.control.numeric().get(local)?);
                            }
                            *self = left;
                        }
                        _ => return None,
                    }
                }
                FlowOp::Noop => {}
                _ => return None,
            }
        }
        Some(RangeFlow::Continue)
    }
}
