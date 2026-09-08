//! Shared checked-expression to value-program sealer.
//!
//! Fx samplers and reactive View Fx bindings consume the same closed
//! instruction algebra. Context-specific code supplies typed input and context
//! projections; operator and callable identity mapping lives only here.

use arcweft_lang_hir::{
    expr::{HirBinaryOp, HirExpr, HirExprKind, HirUnaryOp},
    identity::ExprId,
};
use arcweft_presentation::fx::{
    FxContextSlot, FxRuntimeParameterRef, FxRuntimeType, FxRuntimeValue, ValueInstruction,
};

use crate::callable::{
    BuiltinCallableId, CallableCandidateId, DomainMethodId, StdFloatOperation, VectorDimensions,
};

pub(super) trait CheckedValueProgramSealContext {
    type Error;

    fn expression(&mut self, owner: ExprId) -> Result<HirExpr, Self::Error>;
    fn inferred_type(&mut self, owner: ExprId) -> Result<FxRuntimeType, Self::Error>;
    fn constant(
        &mut self,
        owner: ExprId,
        expected: FxRuntimeType,
    ) -> Result<Option<FxRuntimeValue>, Self::Error>;
    fn input(&mut self, owner: ExprId) -> Result<Option<FxRuntimeParameterRef>, Self::Error>;
    fn context_slot(&mut self, owner: ExprId) -> Result<Option<FxContextSlot>, Self::Error>;
    fn call(&mut self, owner: ExprId) -> Result<(CallableCandidateId, Vec<ExprId>), Self::Error>;
    fn invalid(&self, owner: ExprId) -> Self::Error;
}

pub(super) fn seal_checked_value_expression<C>(
    context: &mut C,
    owner: ExprId,
) -> Result<(Vec<ValueInstruction>, FxRuntimeType), C::Error>
where
    C: CheckedValueProgramSealContext,
{
    let expression = context.expression(owner)?;
    let inferred = context.inferred_type(owner)?;
    if matches!(expression.kind(), HirExprKind::Literal(_)) {
        let value = context
            .constant(owner, inferred)?
            .ok_or_else(|| context.invalid(owner))?;
        return Ok((vec![ValueInstruction::Constant { value }], inferred));
    }
    match expression.kind() {
        HirExprKind::Path(_) => context
            .input(owner)?
            .map(|parameter| {
                (
                    vec![ValueInstruction::LoadParameter { parameter }],
                    parameter.runtime_type(),
                )
            })
            .ok_or_else(|| context.invalid(owner)),
        HirExprKind::Select(_) => context
            .context_slot(owner)?
            .map(|slot| {
                (
                    vec![ValueInstruction::LoadContext { slot }],
                    slot.value_type(),
                )
            })
            .ok_or_else(|| context.invalid(owner)),
        HirExprKind::Call(_) => seal_call(context, owner),
        HirExprKind::Unary(unary) => {
            let (mut instructions, operand_type) =
                seal_checked_value_expression(context, unary.operand())?;
            instructions.push(match unary.operator() {
                HirUnaryOp::Negate => ValueInstruction::Neg,
                HirUnaryOp::Not => ValueInstruction::Not,
            });
            Ok((instructions, operand_type))
        }
        HirExprKind::Binary(binary) => {
            let (mut left, _) = seal_checked_value_expression(context, binary.left())?;
            let (right, _) = seal_checked_value_expression(context, binary.right())?;
            left.extend(right);
            left.push(binary_instruction(binary.operator()).ok_or_else(|| context.invalid(owner))?);
            if binary.operator() == HirBinaryOp::NotEqual {
                left.push(ValueInstruction::Not);
            }
            Ok((left, inferred))
        }
        _ => Err(context.invalid(owner)),
    }
}

fn seal_call<C>(
    context: &mut C,
    owner: ExprId,
) -> Result<(Vec<ValueInstruction>, FxRuntimeType), C::Error>
where
    C: CheckedValueProgramSealContext,
{
    let (identity, sources) = context.call(owner)?;
    let (instruction, arity, direct_type) =
        call_instruction(&identity).ok_or_else(|| context.invalid(owner))?;
    if sources.len() != arity {
        return Err(context.invalid(owner));
    }
    if let ValueInstruction::LoadContext { slot } = instruction {
        return Ok((vec![instruction], slot.value_type()));
    }
    let mut instructions = Vec::new();
    for source in sources {
        let (part, _) = seal_checked_value_expression(context, source)?;
        instructions.extend(part);
    }
    instructions.push(instruction);
    Ok((
        instructions,
        direct_type.unwrap_or(context.inferred_type(owner)?),
    ))
}

fn binary_instruction(operator: HirBinaryOp) -> Option<ValueInstruction> {
    Some(match operator {
        HirBinaryOp::Add => ValueInstruction::Add,
        HirBinaryOp::Subtract => ValueInstruction::Sub,
        HirBinaryOp::Multiply => ValueInstruction::Mul,
        HirBinaryOp::Divide => ValueInstruction::Div,
        HirBinaryOp::Equal | HirBinaryOp::NotEqual => ValueInstruction::Equal,
        HirBinaryOp::Less => ValueInstruction::Less,
        HirBinaryOp::LessOrEqual => ValueInstruction::LessEqual,
        HirBinaryOp::Greater => ValueInstruction::Greater,
        HirBinaryOp::GreaterOrEqual => ValueInstruction::GreaterEqual,
        HirBinaryOp::And => ValueInstruction::And,
        HirBinaryOp::Or => ValueInstruction::Or,
        _ => return None,
    })
}

fn call_instruction(
    identity: &CallableCandidateId,
) -> Option<(ValueInstruction, usize, Option<FxRuntimeType>)> {
    match identity {
        CallableCandidateId::DomainMethod(DomainMethodId::FxSampleOrdinalPhase) => Some((
            ValueInstruction::LoadContext {
                slot: FxContextSlot::OrdinalPhase,
            },
            0,
            Some(FxRuntimeType::F32),
        )),
        CallableCandidateId::Builtin(BuiltinCallableId::Sin) => {
            Some((ValueInstruction::Sin, 1, Some(FxRuntimeType::F32)))
        }
        CallableCandidateId::Builtin(BuiltinCallableId::Cos) => {
            Some((ValueInstruction::Cos, 1, Some(FxRuntimeType::F32)))
        }
        CallableCandidateId::Builtin(BuiltinCallableId::Vector {
            dimensions: VectorDimensions::Two,
        }) => Some((ValueInstruction::MakeVec2, 2, Some(FxRuntimeType::Vec2))),
        CallableCandidateId::Builtin(BuiltinCallableId::StdFloat(callable)) => {
            let instruction = match callable.operation() {
                StdFloatOperation::Abs => ValueInstruction::Abs,
                StdFloatOperation::Floor => ValueInstruction::Floor,
                StdFloatOperation::Fract => ValueInstruction::Fract,
                StdFloatOperation::Sin => ValueInstruction::Sin,
                StdFloatOperation::Cos => ValueInstruction::Cos,
                _ => return None,
            };
            Some((instruction, 1, Some(FxRuntimeType::F32)))
        }
        _ => None,
    }
}
