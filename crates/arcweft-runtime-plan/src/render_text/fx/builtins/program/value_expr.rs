//! Small typed expression builder for validated built-in sampler bytecode.

use arcweft_presentation::fx::{
    Angle, FiniteF32, FxContextSlot, FxRuntimeType, FxRuntimeValue, FxSamplerProgram, Length,
    ValueInstruction, ValueProgramSchema,
};

use crate::errors::RuntimePlanLowerError;

use super::super::fx_error;

#[derive(Clone)]
pub(super) struct ProgramExpr(Vec<ValueInstruction>);

pub(super) struct TransformFields {
    pub(super) translate_x: ProgramExpr,
    pub(super) translate_y: ProgramExpr,
    pub(super) scale_x: ProgramExpr,
    pub(super) scale_y: ProgramExpr,
    pub(super) skew_x: ProgramExpr,
    pub(super) skew_y: ProgramExpr,
    pub(super) rotation: ProgramExpr,
    pub(super) origin_x: ProgramExpr,
    pub(super) origin_y: ProgramExpr,
    pub(super) opacity: ProgramExpr,
}

impl TransformFields {
    pub(super) fn identity() -> Self {
        Self {
            translate_x: value(FxRuntimeValue::Length(Length::ZERO)),
            translate_y: value(FxRuntimeValue::Length(Length::ZERO)),
            scale_x: f32_expr(1.0),
            scale_y: f32_expr(1.0),
            skew_x: value(FxRuntimeValue::Angle(Angle::ZERO)),
            skew_y: value(FxRuntimeValue::Angle(Angle::ZERO)),
            rotation: value(FxRuntimeValue::Angle(Angle::ZERO)),
            origin_x: value(FxRuntimeValue::Length(Length::ZERO)),
            origin_y: value(FxRuntimeValue::Length(Length::ZERO)),
            opacity: f32_expr(1.0),
        }
    }

    pub(super) fn finish(self) -> Result<FxSamplerProgram, RuntimePlanLowerError> {
        sampler(
            FxRuntimeType::Transform2D,
            concatenate([
                self.translate_x,
                self.translate_y,
                self.scale_x,
                self.scale_y,
                self.skew_x,
                self.skew_y,
                self.rotation,
                self.origin_x,
                self.origin_y,
                self.opacity,
                ProgramExpr(vec![ValueInstruction::MakeTransform2D]),
            ]),
        )
    }
}

pub(super) fn sampler(
    return_type: FxRuntimeType,
    mut expression: ProgramExpr,
) -> Result<FxSamplerProgram, RuntimePlanLowerError> {
    expression.0.push(ValueInstruction::Return);
    FxSamplerProgram::validate(
        ValueProgramSchema::new(Vec::new(), Vec::new(), return_type),
        expression.0,
    )
    .map_err(|error| fx_error(format!("invalid built-in sampler: {error}")))
}

pub(super) fn concatenate(expressions: impl IntoIterator<Item = ProgramExpr>) -> ProgramExpr {
    ProgramExpr(
        expressions
            .into_iter()
            .flat_map(|expression| expression.0)
            .collect(),
    )
}

pub(super) fn value(value: FxRuntimeValue) -> ProgramExpr {
    ProgramExpr(vec![ValueInstruction::Constant { value }])
}

pub(super) fn f32_expr(number: f32) -> ProgramExpr {
    value(FxRuntimeValue::F32(
        FiniteF32::try_new(number).expect("built-in constants are finite"),
    ))
}

pub(super) fn i32_expr(number: i32) -> ProgramExpr {
    value(FxRuntimeValue::I32(number))
}

pub(super) fn length_expr(pixels: f32) -> Result<ProgramExpr, RuntimePlanLowerError> {
    Ok(value(FxRuntimeValue::Length(
        Length::try_pixels(pixels).map_err(|error| fx_error(format!("invalid length: {error}")))?,
    )))
}

pub(super) fn angle_expr(degrees: f32) -> Result<ProgramExpr, RuntimePlanLowerError> {
    Ok(value(FxRuntimeValue::Angle(
        Angle::try_degrees(f64::from(degrees))
            .map_err(|error| fx_error(format!("invalid angle: {error}")))?,
    )))
}

pub(super) fn context(slot: FxContextSlot) -> ProgramExpr {
    ProgramExpr(vec![ValueInstruction::LoadContext { slot }])
}

fn unary(mut operand: ProgramExpr, operation: ValueInstruction) -> ProgramExpr {
    operand.0.push(operation);
    operand
}

fn binary(mut left: ProgramExpr, right: ProgramExpr, operation: ValueInstruction) -> ProgramExpr {
    left.0.extend(right.0);
    left.0.push(operation);
    left
}

pub(super) fn add(left: ProgramExpr, right: ProgramExpr) -> ProgramExpr {
    binary(left, right, ValueInstruction::Add)
}

pub(super) fn sub(left: ProgramExpr, right: ProgramExpr) -> ProgramExpr {
    binary(left, right, ValueInstruction::Sub)
}

pub(super) fn mul(left: ProgramExpr, right: ProgramExpr) -> ProgramExpr {
    binary(left, right, ValueInstruction::Mul)
}

pub(super) fn div(left: ProgramExpr, right: ProgramExpr) -> ProgramExpr {
    binary(left, right, ValueInstruction::Div)
}

pub(super) fn max(left: ProgramExpr, right: ProgramExpr) -> ProgramExpr {
    binary(left, right, ValueInstruction::Max)
}

pub(super) fn less_equal(left: ProgramExpr, right: ProgramExpr) -> ProgramExpr {
    binary(left, right, ValueInstruction::LessEqual)
}

pub(super) fn sin(value: ProgramExpr) -> ProgramExpr {
    unary(value, ValueInstruction::Sin)
}

pub(super) fn cos(value: ProgramExpr) -> ProgramExpr {
    unary(value, ValueInstruction::Cos)
}

pub(super) fn floor_to_i32(value: ProgramExpr) -> ProgramExpr {
    unary(value, ValueInstruction::FloorToI32)
}

pub(super) fn hash_noise(value: ProgramExpr) -> ProgramExpr {
    unary(value, ValueInstruction::HashNoise)
}

pub(super) fn signed_noise(bucket: ProgramExpr) -> ProgramExpr {
    sub(mul(hash_noise(bucket), f32_expr(2.0)), f32_expr(1.0))
}

pub(super) fn select(
    condition: ProgramExpr,
    when_true: ProgramExpr,
    when_false: ProgramExpr,
) -> ProgramExpr {
    let mut expression = concatenate([condition, when_true, when_false]);
    expression.0.push(ValueInstruction::Select);
    expression
}

pub(super) fn make_color(
    red: ProgramExpr,
    green: ProgramExpr,
    blue: ProgramExpr,
    alpha: ProgramExpr,
) -> ProgramExpr {
    let mut expression = concatenate([red, green, blue, alpha]);
    expression.0.push(ValueInstruction::MakeColor);
    expression
}
