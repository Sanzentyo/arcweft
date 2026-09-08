//! Small typed expression builder for validated builtin Fx samplers.

use crate::fx::{
    Angle, FiniteF32, FxContextSlot, FxRuntimeParameterRef, FxRuntimeType, FxRuntimeValue,
    FxSamplerProgram, Length, ValueInstruction, ValueProgramSchema,
};

use super::super::BuiltinFxBuildError;

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
            translate_x: length(Length::ZERO),
            translate_y: length(Length::ZERO),
            scale_x: f32(FiniteF32::ONE),
            scale_y: f32(FiniteF32::ONE),
            skew_x: angle(Angle::ZERO),
            skew_y: angle(Angle::ZERO),
            rotation: angle(Angle::ZERO),
            origin_x: length(Length::ZERO),
            origin_y: length(Length::ZERO),
            opacity: f32(FiniteF32::ONE),
        }
    }

    pub(super) fn finish(
        self,
        parameter_types: &[FxRuntimeType],
    ) -> Result<FxSamplerProgram, BuiltinFxBuildError> {
        sampler(
            parameter_types,
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
    parameter_types: &[FxRuntimeType],
    return_type: FxRuntimeType,
    mut expression: ProgramExpr,
) -> Result<FxSamplerProgram, BuiltinFxBuildError> {
    expression.0.push(ValueInstruction::Return);
    Ok(FxSamplerProgram::validate(
        ValueProgramSchema::new(parameter_types.to_vec(), Vec::new(), return_type),
        expression.0,
    )?)
}

pub(super) fn concatenate(expressions: impl IntoIterator<Item = ProgramExpr>) -> ProgramExpr {
    ProgramExpr(
        expressions
            .into_iter()
            .flat_map(|expression| expression.0)
            .collect(),
    )
}

pub(super) fn value(runtime_value: FxRuntimeValue) -> ProgramExpr {
    ProgramExpr(vec![ValueInstruction::Constant {
        value: runtime_value,
    }])
}

pub(super) fn parameter(reference: FxRuntimeParameterRef) -> ProgramExpr {
    ProgramExpr(vec![ValueInstruction::LoadParameter {
        parameter: reference,
    }])
}

pub(super) fn f32(number: FiniteF32) -> ProgramExpr {
    value(FxRuntimeValue::F32(number))
}

pub(super) fn f32_const(value: f32) -> ProgramExpr {
    f32(FiniteF32::try_new(value).expect("checked-in builtin constant is finite"))
}

pub(super) fn i32(number: i32) -> ProgramExpr {
    value(FxRuntimeValue::I32(number))
}

pub(super) fn length(distance: Length) -> ProgramExpr {
    value(FxRuntimeValue::Length(distance))
}

pub(super) fn angle(rotation: Angle) -> ProgramExpr {
    value(FxRuntimeValue::Angle(rotation))
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
pub(super) fn bitcast_u32_to_i32(value: ProgramExpr) -> ProgramExpr {
    unary(value, ValueInstruction::BitcastU32ToI32)
}
pub(super) fn seconds_value(value: ProgramExpr) -> ProgramExpr {
    unary(value, ValueInstruction::SecondsValue)
}
pub(super) fn vec2_x(value: ProgramExpr) -> ProgramExpr {
    unary(value, ValueInstruction::Vec2X)
}
pub(super) fn vec2_y(value: ProgramExpr) -> ProgramExpr {
    unary(value, ValueInstruction::Vec2Y)
}
pub(super) fn hash_noise(value: ProgramExpr) -> ProgramExpr {
    unary(value, ValueInstruction::HashNoise)
}
pub(super) fn signed_noise(bucket: ProgramExpr) -> ProgramExpr {
    sub(mul(hash_noise(bucket), f32_const(2.0)), f32(FiniteF32::ONE))
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

pub(super) fn make_vec2(x: ProgramExpr, y: ProgramExpr) -> ProgramExpr {
    let mut expression = concatenate([x, y]);
    expression.0.push(ValueInstruction::MakeVec2);
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
