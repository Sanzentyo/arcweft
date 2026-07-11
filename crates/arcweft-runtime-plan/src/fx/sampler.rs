//! Typed compilation of pure Fx sampler closures into the shared value-program IR.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::syntax::expr::{BinaryOp, CallArg, Expr, Literal, UnaryOp, UnitNumberSuffix};
use arcweft_presentation::fx::{
    Angle, FiniteF32, FxContextSlot, FxRuntimeType, FxRuntimeValue, FxSamplerProgram,
    FxStaticValue, Length, ValueInstruction, ValueProgramSchema,
};

use crate::{errors::RuntimePlanLowerError, labels::expr_label};

use super::value_lowering::lower_closed_runtime_value;

pub(super) fn lower_sampler(
    expr: &Expr,
    bindings: &BTreeMap<String, FxStaticValue>,
    parameter_types: &[FxRuntimeType],
) -> Result<FxSamplerProgram, RuntimePlanLowerError> {
    let Expr::Closure { params, body, .. } = expr else {
        return Err(error(format!(
            "sampler `{}` must be a closure",
            expr_label(expr)
        )));
    };
    let [parameter] = params.as_slice() else {
        return Err(error(
            "sampler closure requires exactly one FxSampleContext parameter",
        ));
    };
    let context_name = parameter
        .simple_ident()
        .ok_or_else(|| error("sampler context parameter must be a simple identifier"))?;
    let body = block_value(body)?;
    let mut compiler = SamplerCompiler {
        context_name,
        bindings,
        instructions: Vec::new(),
    };
    compiler.emit_expr(body, FxRuntimeType::Transform2D)?;
    compiler.instructions.push(ValueInstruction::Return);
    FxSamplerProgram::validate(
        ValueProgramSchema::new(
            parameter_types.to_vec(),
            Vec::new(),
            FxRuntimeType::Transform2D,
        ),
        compiler.instructions,
    )
    .map_err(|source| error(format!("invalid sampler program: {source}")))
}

struct SamplerCompiler<'a> {
    context_name: &'a str,
    bindings: &'a BTreeMap<String, FxStaticValue>,
    instructions: Vec<ValueInstruction>,
}

impl SamplerCompiler<'_> {
    fn emit_expr(
        &mut self,
        expr: &Expr,
        expected: FxRuntimeType,
    ) -> Result<(), RuntimePlanLowerError> {
        let actual = self.infer_type(expr)?;
        if actual != expected {
            return Err(error(format!(
                "sampler expression `{}` has type {actual:?}, expected {expected:?}",
                expr_label(expr)
            )));
        }
        if let Some(value) = self.bound_value(expr).cloned() {
            return self.emit_bound_value(&value);
        }
        if let Some(slot) = self.context_slot(expr) {
            self.instructions
                .push(ValueInstruction::LoadContext { slot });
            return Ok(());
        }
        match expr {
            Expr::Literal(_) => self.emit_constant(expr, expected),
            Expr::Unary { op, expr: operand } => {
                if matches!(op, UnaryOp::Neg) && matches!(operand.as_ref(), Expr::Literal(_)) {
                    return self.emit_constant(expr, expected);
                }
                self.emit_expr(operand, self.infer_type(operand)?)?;
                self.instructions.push(match op {
                    UnaryOp::Neg => ValueInstruction::Neg,
                    UnaryOp::Not => ValueInstruction::Not,
                });
                Ok(())
            }
            Expr::Binary { lhs, op, rhs } => self.emit_binary(lhs, *op, rhs),
            Expr::Call { callee, args } => self.emit_call(callee, args),
            Expr::Record { path, fields } if path == "Transform2D" => self.emit_transform(fields),
            Expr::If {
                condition,
                then_branch,
                else_branch: Some(else_branch),
            } => {
                self.emit_expr(condition, FxRuntimeType::Bool)?;
                self.emit_expr(then_branch, expected)?;
                self.emit_expr(else_branch, expected)?;
                self.instructions.push(ValueInstruction::Select);
                Ok(())
            }
            Expr::Block { .. } => self.emit_expr(block_value(expr)?, expected),
            _ => Err(error(format!(
                "sampler expression `{}` is outside the closed instruction set",
                expr_label(expr)
            ))),
        }
    }

    fn emit_bound_value(&mut self, value: &FxStaticValue) -> Result<(), RuntimePlanLowerError> {
        match value {
            FxStaticValue::Runtime(value) => {
                self.instructions
                    .push(ValueInstruction::Constant { value: *value });
                Ok(())
            }
            FxStaticValue::Parameter(slot) => {
                self.instructions.push(ValueInstruction::LoadParameter {
                    slot: slot.index,
                    ty: slot.ty,
                });
                Ok(())
            }
            _ => Err(error(format!(
                "sampler capture has non-runtime type {}",
                value.static_type().as_str()
            ))),
        }
    }

    fn emit_constant(
        &mut self,
        expr: &Expr,
        expected: FxRuntimeType,
    ) -> Result<(), RuntimePlanLowerError> {
        let value = lower_closed_runtime_value(expr, expected)?;
        self.instructions.push(ValueInstruction::Constant { value });
        Ok(())
    }

    fn emit_binary(
        &mut self,
        lhs: &Expr,
        op: BinaryOp,
        rhs: &Expr,
    ) -> Result<(), RuntimePlanLowerError> {
        let left = self.infer_type(lhs)?;
        let right = self.infer_type(rhs)?;
        self.emit_expr(lhs, left)?;
        self.emit_expr(rhs, right)?;
        let instruction = match op {
            BinaryOp::Add => ValueInstruction::Add,
            BinaryOp::Sub => ValueInstruction::Sub,
            BinaryOp::Mul => ValueInstruction::Mul,
            BinaryOp::Div => ValueInstruction::Div,
            BinaryOp::Eq | BinaryOp::NotEq => ValueInstruction::Equal,
            BinaryOp::Lt => ValueInstruction::Less,
            BinaryOp::Lte => ValueInstruction::LessEqual,
            BinaryOp::Gt => ValueInstruction::Greater,
            BinaryOp::Gte => ValueInstruction::GreaterEqual,
            BinaryOp::And => ValueInstruction::And,
            BinaryOp::Or => ValueInstruction::Or,
            BinaryOp::Implies | BinaryOp::In | BinaryOp::Merge | BinaryOp::Rem => {
                return Err(error(format!(
                    "operator `{op:?}` is outside the Fx sampler instruction set"
                )));
            }
        };
        self.instructions.push(instruction);
        if op == BinaryOp::NotEq {
            self.instructions.push(ValueInstruction::Not);
        }
        Ok(())
    }

    fn emit_call(&mut self, callee: &Expr, args: &[CallArg]) -> Result<(), RuntimePlanLowerError> {
        if let Some(slot) = self.context_call_slot(callee, args) {
            self.instructions
                .push(ValueInstruction::LoadContext { slot });
            return Ok(());
        }
        let name = simple_path(callee).ok_or_else(|| {
            error(format!(
                "sampler call `{}` is not a closed intrinsic",
                expr_label(callee)
            ))
        })?;
        let operands = positional_args(name, args)?;
        match name {
            "sin" | "cos" | "abs" | "floor" | "fract" => {
                let [operand] = operands.as_slice() else {
                    return Err(error(format!("`{name}` requires one operand")));
                };
                self.emit_expr(operand, self.infer_type(operand)?)?;
                self.instructions.push(match name {
                    "sin" => ValueInstruction::Sin,
                    "cos" => ValueInstruction::Cos,
                    "abs" => ValueInstruction::Abs,
                    "floor" => ValueInstruction::Floor,
                    "fract" => ValueInstruction::Fract,
                    _ => unreachable!("intrinsic was matched above"),
                });
            }
            "min" | "max" => {
                let [first, second] = operands.as_slice() else {
                    return Err(error(format!("`{name}` requires two operands")));
                };
                let ty = self.infer_type(first)?;
                self.emit_expr(first, ty)?;
                self.emit_expr(second, ty)?;
                self.instructions.push(if name == "min" {
                    ValueInstruction::Min
                } else {
                    ValueInstruction::Max
                });
            }
            "clamp" => {
                let [value, minimum, maximum] = operands.as_slice() else {
                    return Err(error("`clamp` requires value, minimum, and maximum"));
                };
                let ty = self.infer_type(value)?;
                self.emit_expr(value, ty)?;
                self.emit_expr(minimum, ty)?;
                self.emit_expr(maximum, ty)?;
                self.instructions.push(ValueInstruction::Clamp);
            }
            "hash_noise" => {
                let [bucket] = operands.as_slice() else {
                    return Err(error("`hash_noise` requires one i32 bucket"));
                };
                self.emit_expr(bucket, FxRuntimeType::I32)?;
                self.instructions.push(ValueInstruction::HashNoise);
            }
            "vec2" => {
                let [x, y] = operands.as_slice() else {
                    return Err(error("`vec2` requires two f32 operands"));
                };
                self.emit_expr(x, FxRuntimeType::F32)?;
                self.emit_expr(y, FxRuntimeType::F32)?;
                self.instructions.push(ValueInstruction::MakeVec2);
            }
            _ => return Err(error(format!("unknown Fx sampler intrinsic `{name}`"))),
        }
        Ok(())
    }

    fn emit_transform(&mut self, fields: &[(String, Expr)]) -> Result<(), RuntimePlanLowerError> {
        let mut authored = BTreeMap::new();
        for (name, value) in fields {
            if authored.insert(name.as_str(), value).is_some() {
                return Err(error(format!("Transform2D repeats field `{name}`")));
            }
        }
        let known = transform_fields()
            .into_iter()
            .map(|(name, _, _)| name)
            .collect::<BTreeSet<_>>();
        if let Some(unknown) = authored.keys().find(|name| !known.contains(**name)) {
            return Err(error(format!("Transform2D has no field `{unknown}`")));
        }
        for (name, ty, default) in transform_fields() {
            if let Some(value) = authored.get(name) {
                self.emit_expr(value, ty)?;
            } else {
                self.instructions
                    .push(ValueInstruction::Constant { value: default });
            }
        }
        self.instructions.push(ValueInstruction::MakeTransform2D);
        Ok(())
    }

    fn infer_type(&self, expr: &Expr) -> Result<FxRuntimeType, RuntimePlanLowerError> {
        if let Some(value) = self.bound_value(expr) {
            return value.runtime_type().ok_or_else(|| {
                error(format!(
                    "sampler capture `{}` has non-runtime type {}",
                    expr_label(expr),
                    value.static_type().as_str()
                ))
            });
        }
        if let Some(slot) = self.context_slot(expr) {
            return Ok(slot.value_type());
        }
        match expr {
            Expr::Literal(Literal::Bool(_)) => Ok(FxRuntimeType::Bool),
            Expr::Literal(Literal::Int(_)) => Ok(FxRuntimeType::I32),
            Expr::Literal(Literal::Float { .. }) => Ok(FxRuntimeType::F32),
            Expr::Literal(Literal::UnitNumber { suffix, .. }) => match suffix {
                UnitNumberSuffix::Px => Ok(FxRuntimeType::Length),
                UnitNumberSuffix::Deg | UnitNumberSuffix::Rad | UnitNumberSuffix::Turn => {
                    Ok(FxRuntimeType::Angle)
                }
                _ => Err(error(format!(
                    "unit `{}` is not accepted by Fx samplers",
                    suffix.as_str()
                ))),
            },
            Expr::Literal(Literal::Duration { .. }) => Ok(FxRuntimeType::Seconds),
            Expr::Unary { op, expr } => match op {
                UnaryOp::Neg => self.infer_type(expr),
                UnaryOp::Not => Ok(FxRuntimeType::Bool),
            },
            Expr::Binary { lhs, op, rhs } => {
                binary_result(self.infer_type(lhs)?, *op, self.infer_type(rhs)?).ok_or_else(|| {
                    error(format!(
                        "sampler operator `{op:?}` does not accept `{}` and `{}`",
                        expr_label(lhs),
                        expr_label(rhs)
                    ))
                })
            }
            Expr::Call { callee, args } => self.infer_call(callee, args),
            Expr::Record { path, .. } if path == "Transform2D" => Ok(FxRuntimeType::Transform2D),
            Expr::If {
                then_branch,
                else_branch: Some(else_branch),
                ..
            } => {
                let then_type = self.infer_type(then_branch)?;
                let else_type = self.infer_type(else_branch)?;
                if then_type == else_type {
                    Ok(then_type)
                } else {
                    Err(error("sampler if branches must have one runtime type"))
                }
            }
            Expr::Block { .. } => self.infer_type(block_value(expr)?),
            _ => Err(error(format!(
                "cannot infer a sampler runtime type for `{}`",
                expr_label(expr)
            ))),
        }
    }

    fn infer_call(
        &self,
        callee: &Expr,
        args: &[CallArg],
    ) -> Result<FxRuntimeType, RuntimePlanLowerError> {
        if let Some(slot) = self.context_call_slot(callee, args) {
            return Ok(slot.value_type());
        }
        let name = simple_path(callee)
            .ok_or_else(|| error("Fx sampler calls must use a closed intrinsic"))?;
        let operands = positional_args(name, args)?;
        match name {
            "sin" | "cos" | "floor" | "fract" | "hash_noise" => Ok(FxRuntimeType::F32),
            "abs" | "min" | "max" | "clamp" => operands
                .first()
                .map(|value| self.infer_type(value))
                .transpose()?
                .ok_or_else(|| error(format!("`{name}` requires operands"))),
            "vec2" => Ok(FxRuntimeType::Vec2),
            _ => Err(error(format!("unknown Fx sampler intrinsic `{name}`"))),
        }
    }

    fn bound_value(&self, expr: &Expr) -> Option<&FxStaticValue> {
        let Expr::Path(path) = expr else {
            return None;
        };
        (path.segments().len() == 1)
            .then(|| self.bindings.get(path.as_label()))
            .flatten()
    }

    fn context_slot(&self, expr: &Expr) -> Option<FxContextSlot> {
        let Expr::Select(select) = expr else {
            return None;
        };
        if !matches!(select.target(), Expr::Path(path) if path.is_single(self.context_name)) {
            return None;
        }
        match select.member().as_str() {
            "time" => Some(FxContextSlot::Time),
            "ordinal" => Some(FxContextSlot::Ordinal),
            "ordinal_phase" => Some(FxContextSlot::OrdinalPhase),
            "reduce_motion" => Some(FxContextSlot::ReduceMotion),
            _ => None,
        }
    }

    fn context_call_slot(&self, callee: &Expr, args: &[CallArg]) -> Option<FxContextSlot> {
        args.is_empty().then(|| self.context_slot(callee)).flatten()
    }
}

fn transform_fields() -> [(&'static str, FxRuntimeType, FxRuntimeValue); 10] {
    [
        (
            "translate_x",
            FxRuntimeType::Length,
            FxRuntimeValue::Length(Length::ZERO),
        ),
        (
            "translate_y",
            FxRuntimeType::Length,
            FxRuntimeValue::Length(Length::ZERO),
        ),
        (
            "scale_x",
            FxRuntimeType::F32,
            FxRuntimeValue::F32(FiniteF32::ONE),
        ),
        (
            "scale_y",
            FxRuntimeType::F32,
            FxRuntimeValue::F32(FiniteF32::ONE),
        ),
        (
            "skew_x",
            FxRuntimeType::Angle,
            FxRuntimeValue::Angle(Angle::ZERO),
        ),
        (
            "skew_y",
            FxRuntimeType::Angle,
            FxRuntimeValue::Angle(Angle::ZERO),
        ),
        (
            "rotation",
            FxRuntimeType::Angle,
            FxRuntimeValue::Angle(Angle::ZERO),
        ),
        (
            "origin_x",
            FxRuntimeType::Length,
            FxRuntimeValue::Length(Length::ZERO),
        ),
        (
            "origin_y",
            FxRuntimeType::Length,
            FxRuntimeValue::Length(Length::ZERO),
        ),
        (
            "opacity",
            FxRuntimeType::F32,
            FxRuntimeValue::F32(FiniteF32::ONE),
        ),
    ]
}

fn binary_result(left: FxRuntimeType, op: BinaryOp, right: FxRuntimeType) -> Option<FxRuntimeType> {
    use FxRuntimeType::{Angle, Bool, F32, I32, Length, Seconds, Vec2};
    match op {
        BinaryOp::Add | BinaryOp::Sub
            if left == right && matches!(left, I32 | F32 | Length | Angle | Seconds | Vec2) =>
        {
            Some(left)
        }
        BinaryOp::Mul => match (left, right) {
            (I32, I32) => Some(I32),
            (F32, F32) => Some(F32),
            (unit, F32) | (F32, unit) if matches!(unit, Length | Angle | Seconds | Vec2) => {
                Some(unit)
            }
            _ => None,
        },
        BinaryOp::Div => match (left, right) {
            (I32, I32) => Some(I32),
            (F32, F32) => Some(F32),
            (unit, F32) if matches!(unit, Length | Angle | Seconds | Vec2) => Some(unit),
            (left, right) if left == right && matches!(left, Length | Angle | Seconds) => Some(F32),
            _ => None,
        },
        BinaryOp::Eq | BinaryOp::NotEq if left == right => Some(Bool),
        BinaryOp::Lt | BinaryOp::Lte | BinaryOp::Gt | BinaryOp::Gte
            if left == right && matches!(left, I32 | F32 | Length | Angle | Seconds) =>
        {
            Some(Bool)
        }
        BinaryOp::And | BinaryOp::Or if left == Bool && right == Bool => Some(Bool),
        BinaryOp::Implies
        | BinaryOp::In
        | BinaryOp::Merge
        | BinaryOp::Rem
        | BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::Lte
        | BinaryOp::Gt
        | BinaryOp::Gte
        | BinaryOp::And
        | BinaryOp::Or => None,
    }
}

fn block_value(expr: &Expr) -> Result<&Expr, RuntimePlanLowerError> {
    match expr {
        Expr::Block {
            statements,
            value: Some(value),
        } if statements.is_empty() => Ok(value),
        Expr::Block { .. } => Err(error(
            "sampler block bodies must contain one tail expression and no statements",
        )),
        value => Ok(value),
    }
}

fn positional_args<'a>(
    name: &str,
    args: &'a [CallArg],
) -> Result<Vec<&'a Expr>, RuntimePlanLowerError> {
    args.iter()
        .map(|arg| match arg {
            CallArg::Positional(value) => Ok(value),
            CallArg::Named { .. } | CallArg::Spread { .. } => Err(error(format!(
                "Fx sampler intrinsic `{name}` accepts positional arguments only"
            ))),
        })
        .collect()
}

fn simple_path(expr: &Expr) -> Option<&str> {
    let Expr::Path(path) = expr else {
        return None;
    };
    (path.segments().len() == 1).then(|| path.as_label())
}

fn error(message: impl Into<String>) -> RuntimePlanLowerError {
    RuntimePlanLowerError::new(format!("Fx sampler: {}", message.into()))
}
