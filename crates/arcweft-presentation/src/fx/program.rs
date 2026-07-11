//! Validated, bounded stack programs shared by Fx samplers and View values.

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use super::{
    evaluator,
    identity::hash_usize,
    state::FxSampleContext,
    value::{FxRuntimeType, FxRuntimeValue, hash_runtime_value},
};

pub const FX_MAX_INSTRUCTIONS_PER_SAMPLER: usize = 1_024;
pub const FX_MAX_CONSTANTS_PER_SAMPLER: usize = 256;
pub const FX_MAX_STACK_VALUES_PER_PROGRAM: usize = 64;
pub const FX_MAX_CAPTURED_OR_PARAMETER_SLOTS: usize = 64;
pub const FX_DEFAULT_EVALUATOR_OPERATIONS: u32 = 262_144;

/// Context values available without a host callback.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FxContextSlot {
    /// Activation-relative logical seconds, represented as dimensionless `F32`.
    Time = 0,
    /// Target-local logical ordinal, represented as an integer-valued `F32`.
    Ordinal = 1,
    /// Golden-angle ordinal phase, represented as dimensionless radians in `F32`.
    OrdinalPhase = 2,
    ReduceMotion = 3,
}

/// Closed instruction inventory for deterministic value programs.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ValueInstruction {
    Constant {
        value: FxRuntimeValue,
    },
    LoadParameter {
        slot: u16,
        ty: FxRuntimeType,
    },
    LoadState {
        slot: u16,
        ty: FxRuntimeType,
    },
    LoadContext {
        slot: FxContextSlot,
    },
    Neg,
    Add,
    Sub,
    Mul,
    Div,
    Abs,
    Min,
    Max,
    Clamp,
    Sin,
    Cos,
    Floor,
    Fract,
    Equal,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Not,
    And,
    Or,
    /// Pops `condition`, `when_true`, and `when_false` in that authored order.
    Select,
    /// Pops one `I32` bucket and hashes it with the context seed and ordinal.
    HashNoise,
    /// Pops dimensionless `x` and `y` values in authored order.
    MakeVec2,
    /// Pops all ten [`super::Transform2D`] fields in declaration order.
    MakeTransform2D,
    Return,
}

/// Declared input and return types for a value program.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ValueProgramSchema {
    parameter_types: Vec<FxRuntimeType>,
    state_types: Vec<FxRuntimeType>,
    return_type: FxRuntimeType,
}

/// Validation limits selected by the owning program kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueProgramLimits {
    instructions: usize,
    constants: usize,
    stack_values: usize,
    parameter_and_state_slots: usize,
}

/// Common validated program body. View owns a distinct wrapper around this type.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValidatedValueProgram {
    schema: ValueProgramSchema,
    instructions: Vec<ValueInstruction>,
}

/// Validated owner for Fx sampler programs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FxSamplerProgram {
    program: ValidatedValueProgram,
}

/// Borrowed runtime inputs for one program evaluation.
#[derive(Clone, Copy, Debug)]
pub struct ValueProgramInputs<'a> {
    pub parameters: &'a [FxRuntimeValue],
    pub state: &'a [FxRuntimeValue],
}

/// Per-instance, per-frame operation budget shared by all sampler evaluations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FxEvaluationBudget {
    remaining: u32,
    limit: u32,
}

/// A malformed program rejected before execution.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ValueProgramValidationError {
    #[error("program has {actual} instructions, exceeding the limit of {limit}")]
    TooManyInstructions { actual: usize, limit: usize },
    #[error("program has {actual} constants, exceeding the limit of {limit}")]
    TooManyConstants { actual: usize, limit: usize },
    #[error("program declares {actual} parameter/state slots, exceeding the limit of {limit}")]
    TooManySlots { actual: usize, limit: usize },
    #[error("instruction {instruction} references {kind} slot {slot}, but only {available} exist")]
    SlotOutOfBounds {
        instruction: usize,
        kind: &'static str,
        slot: u16,
        available: usize,
    },
    #[error(
        "instruction {instruction} declares {actual:?} for {kind} slot {slot}, expected {expected:?}"
    )]
    SlotTypeMismatch {
        instruction: usize,
        kind: &'static str,
        slot: u16,
        expected: FxRuntimeType,
        actual: FxRuntimeType,
    },
    #[error(
        "instruction {instruction} requires {required} stack values, but only {available} exist"
    )]
    StackUnderflow {
        instruction: usize,
        required: usize,
        available: usize,
    },
    #[error(
        "instruction {instruction} grows the stack to {actual}, exceeding the limit of {limit}"
    )]
    StackLimit {
        instruction: usize,
        actual: usize,
        limit: usize,
    },
    #[error("instruction {instruction} `{operation}` does not accept operand types {operands:?}")]
    InvalidOperands {
        instruction: usize,
        operation: &'static str,
        operands: Vec<FxRuntimeType>,
    },
    #[error("instruction {instruction} returns {actual:?}, but the program declares {expected:?}")]
    ReturnTypeMismatch {
        instruction: usize,
        expected: FxRuntimeType,
        actual: FxRuntimeType,
    },
    #[error("instruction {instruction} returns with {remaining} additional stack values")]
    ReturnStackNotEmpty {
        instruction: usize,
        remaining: usize,
    },
    #[error("return instruction {instruction} must be the final instruction")]
    ReturnNotLast { instruction: usize },
    #[error("program has no return instruction")]
    MissingReturn,
}

/// Deterministic evaluator failure. No partial application output is committed.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxEvaluationError {
    #[error("program expected {expected} {kind} values, got {actual}")]
    InputCount {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{kind} slot {slot} has type {actual:?}, expected {expected:?}")]
    InputType {
        kind: &'static str,
        slot: usize,
        expected: FxRuntimeType,
        actual: FxRuntimeType,
    },
    #[error("Fx evaluator exhausted its {limit}-operation budget at instruction {instruction}")]
    BudgetExceeded { instruction: usize, limit: u32 },
    #[error("division by zero at instruction {instruction}")]
    DivisionByZero { instruction: usize },
    #[error("`{operation}` produced a non-finite result at instruction {instruction}")]
    NonFiniteResult {
        instruction: usize,
        operation: &'static str,
    },
    #[error("`{operation}` underflowed a non-zero value to zero at instruction {instruction}")]
    Underflow {
        instruction: usize,
        operation: &'static str,
    },
    #[error("integer `{operation}` overflow at instruction {instruction}")]
    IntegerOverflow {
        instruction: usize,
        operation: &'static str,
    },
    #[error(
        "unit/type mismatch for `{operation}` at instruction {instruction}: {left:?} and {right:?}"
    )]
    UnitMismatch {
        instruction: usize,
        operation: &'static str,
        left: FxRuntimeType,
        right: FxRuntimeType,
    },
    #[error("clamp minimum exceeds maximum at instruction {instruction}")]
    InvalidClampBounds { instruction: usize },
    #[error("invalid transform opacity at instruction {instruction}")]
    InvalidOpacity { instruction: usize },
    #[error("validated program stack invariant failed at instruction {instruction}")]
    InvalidProgramState { instruction: usize },
}

impl FxContextSlot {
    pub const fn value_type(self) -> FxRuntimeType {
        match self {
            Self::Time | Self::Ordinal | Self::OrdinalPhase => FxRuntimeType::F32,
            Self::ReduceMotion => FxRuntimeType::Bool,
        }
    }
}

impl ValueProgramSchema {
    pub fn new(
        parameter_types: Vec<FxRuntimeType>,
        state_types: Vec<FxRuntimeType>,
        return_type: FxRuntimeType,
    ) -> Self {
        Self {
            parameter_types,
            state_types,
            return_type,
        }
    }

    pub fn parameter_types(&self) -> &[FxRuntimeType] {
        &self.parameter_types
    }

    pub fn state_types(&self) -> &[FxRuntimeType] {
        &self.state_types
    }

    pub const fn return_type(&self) -> FxRuntimeType {
        self.return_type
    }
}

impl ValueProgramLimits {
    pub const SAMPLER: Self = Self {
        instructions: FX_MAX_INSTRUCTIONS_PER_SAMPLER,
        constants: FX_MAX_CONSTANTS_PER_SAMPLER,
        stack_values: FX_MAX_STACK_VALUES_PER_PROGRAM,
        parameter_and_state_slots: FX_MAX_CAPTURED_OR_PARAMETER_SLOTS,
    };

    pub const VIEW: Self = Self {
        instructions: 4_096,
        constants: 1_024,
        stack_values: FX_MAX_STACK_VALUES_PER_PROGRAM,
        parameter_and_state_slots: 256,
    };
}

impl ValidatedValueProgram {
    /// Validates limits, slots, stack shape, operand types, and the single return.
    pub fn validate(
        schema: ValueProgramSchema,
        instructions: Vec<ValueInstruction>,
        limits: ValueProgramLimits,
    ) -> Result<Self, ValueProgramValidationError> {
        validate_program(&schema, &instructions, limits)?;
        Ok(Self {
            schema,
            instructions,
        })
    }

    pub const fn schema(&self) -> &ValueProgramSchema {
        &self.schema
    }

    pub fn instructions(&self) -> &[ValueInstruction] {
        &self.instructions
    }

    pub fn evaluate(
        &self,
        inputs: ValueProgramInputs<'_>,
        context: FxSampleContext,
        budget: &mut FxEvaluationBudget,
    ) -> Result<FxRuntimeValue, FxEvaluationError> {
        evaluator::evaluate(self, inputs, context, budget)
    }

    pub(crate) fn hash_into(&self, hasher: &mut blake3::Hasher) {
        hash_usize(hasher, self.schema.parameter_types.len());
        for ty in &self.schema.parameter_types {
            hasher.update(&[*ty as u8]);
        }
        hash_usize(hasher, self.schema.state_types.len());
        for ty in &self.schema.state_types {
            hasher.update(&[*ty as u8]);
        }
        hasher.update(&[self.schema.return_type as u8]);
        hash_usize(hasher, self.instructions.len());
        for instruction in &self.instructions {
            hash_instruction(hasher, instruction);
        }
    }
}

impl FxSamplerProgram {
    pub fn validate(
        schema: ValueProgramSchema,
        instructions: Vec<ValueInstruction>,
    ) -> Result<Self, ValueProgramValidationError> {
        ValidatedValueProgram::validate(schema, instructions, ValueProgramLimits::SAMPLER)
            .map(|program| Self { program })
    }

    pub const fn program(&self) -> &ValidatedValueProgram {
        &self.program
    }

    pub const fn return_type(&self) -> FxRuntimeType {
        self.program.schema.return_type
    }

    pub fn evaluate(
        &self,
        inputs: ValueProgramInputs<'_>,
        context: FxSampleContext,
        budget: &mut FxEvaluationBudget,
    ) -> Result<FxRuntimeValue, FxEvaluationError> {
        self.program.evaluate(inputs, context, budget)
    }

    pub(crate) fn hash_into(&self, hasher: &mut blake3::Hasher) {
        self.program.hash_into(hasher);
    }
}

#[derive(Deserialize)]
struct FxSamplerProgramWire {
    program: RawValueProgram,
}

#[derive(Deserialize)]
struct RawValueProgram {
    schema: ValueProgramSchema,
    instructions: Vec<ValueInstruction>,
}

impl<'de> Deserialize<'de> for FxSamplerProgram {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxSamplerProgramWire::deserialize(deserializer)?;
        Self::validate(wire.program.schema, wire.program.instructions).map_err(D::Error::custom)
    }
}

impl Default for FxEvaluationBudget {
    fn default() -> Self {
        Self::new(FX_DEFAULT_EVALUATOR_OPERATIONS)
    }
}

impl FxEvaluationBudget {
    pub const fn new(limit: u32) -> Self {
        Self {
            remaining: limit,
            limit,
        }
    }

    pub const fn remaining(self) -> u32 {
        self.remaining
    }

    pub(crate) fn charge(&mut self, instruction: usize) -> Result<(), FxEvaluationError> {
        if self.remaining == 0 {
            Err(FxEvaluationError::BudgetExceeded {
                instruction,
                limit: self.limit,
            })
        } else {
            self.remaining -= 1;
            Ok(())
        }
    }
}

fn validate_program(
    schema: &ValueProgramSchema,
    instructions: &[ValueInstruction],
    limits: ValueProgramLimits,
) -> Result<(), ValueProgramValidationError> {
    validate_declared_limits(schema, instructions, limits)?;

    let mut stack = Vec::with_capacity(limits.stack_values.min(instructions.len()));
    let mut constants = 0;
    let mut returned = false;
    for (index, instruction) in instructions.iter().enumerate() {
        match instruction {
            ValueInstruction::Constant { value } => {
                constants += 1;
                if constants > limits.constants {
                    return Err(ValueProgramValidationError::TooManyConstants {
                        actual: constants,
                        limit: limits.constants,
                    });
                }
                stack.push(value.value_type());
            }
            ValueInstruction::LoadParameter { slot, ty } => {
                validate_slot(index, "parameter", *slot, *ty, &schema.parameter_types)?;
                stack.push(*ty);
            }
            ValueInstruction::LoadState { slot, ty } => {
                validate_slot(index, "state", *slot, *ty, &schema.state_types)?;
                stack.push(*ty);
            }
            ValueInstruction::LoadContext { slot } => stack.push(slot.value_type()),
            ValueInstruction::Neg => validate_unary(index, "neg", &mut stack, neg_result)?,
            ValueInstruction::Add => validate_binary(index, "add", &mut stack, add_sub_result)?,
            ValueInstruction::Sub => validate_binary(index, "sub", &mut stack, add_sub_result)?,
            ValueInstruction::Mul => validate_binary(index, "mul", &mut stack, mul_result)?,
            ValueInstruction::Div => validate_binary(index, "div", &mut stack, div_result)?,
            ValueInstruction::Abs => validate_unary(index, "abs", &mut stack, abs_result)?,
            ValueInstruction::Min => validate_binary(index, "min", &mut stack, order_result)?,
            ValueInstruction::Max => validate_binary(index, "max", &mut stack, order_result)?,
            ValueInstruction::Clamp => validate_clamp(index, &mut stack)?,
            ValueInstruction::Sin => validate_unary(index, "sin", &mut stack, trig_result)?,
            ValueInstruction::Cos => validate_unary(index, "cos", &mut stack, trig_result)?,
            ValueInstruction::Floor => validate_unary(index, "floor", &mut stack, f32_result)?,
            ValueInstruction::Fract => validate_unary(index, "fract", &mut stack, f32_result)?,
            ValueInstruction::Equal => validate_binary(index, "equal", &mut stack, equal_result)?,
            ValueInstruction::Less => validate_binary(index, "less", &mut stack, compare_result)?,
            ValueInstruction::LessEqual => {
                validate_binary(index, "less_equal", &mut stack, compare_result)?;
            }
            ValueInstruction::Greater => {
                validate_binary(index, "greater", &mut stack, compare_result)?;
            }
            ValueInstruction::GreaterEqual => {
                validate_binary(index, "greater_equal", &mut stack, compare_result)?;
            }
            ValueInstruction::Not => validate_unary(index, "not", &mut stack, bool_result)?,
            ValueInstruction::And => validate_binary(index, "and", &mut stack, bools_result)?,
            ValueInstruction::Or => validate_binary(index, "or", &mut stack, bools_result)?,
            ValueInstruction::Select => validate_select(index, &mut stack)?,
            ValueInstruction::HashNoise => {
                validate_unary(index, "hash_noise", &mut stack, hash_noise_result)?;
            }
            ValueInstruction::MakeVec2 => validate_make_vec2(index, &mut stack)?,
            ValueInstruction::MakeTransform2D => validate_make_transform(index, &mut stack)?,
            ValueInstruction::Return => {
                if index + 1 != instructions.len() {
                    return Err(ValueProgramValidationError::ReturnNotLast { instruction: index });
                }
                let actual = pop_types(index, &mut stack, 1)?[0];
                if actual != schema.return_type {
                    return Err(ValueProgramValidationError::ReturnTypeMismatch {
                        instruction: index,
                        expected: schema.return_type,
                        actual,
                    });
                }
                if !stack.is_empty() {
                    return Err(ValueProgramValidationError::ReturnStackNotEmpty {
                        instruction: index,
                        remaining: stack.len(),
                    });
                }
                returned = true;
            }
        }
        if stack.len() > limits.stack_values {
            return Err(ValueProgramValidationError::StackLimit {
                instruction: index,
                actual: stack.len(),
                limit: limits.stack_values,
            });
        }
    }
    if returned {
        Ok(())
    } else {
        Err(ValueProgramValidationError::MissingReturn)
    }
}

fn validate_declared_limits(
    schema: &ValueProgramSchema,
    instructions: &[ValueInstruction],
    limits: ValueProgramLimits,
) -> Result<(), ValueProgramValidationError> {
    if instructions.len() > limits.instructions {
        return Err(ValueProgramValidationError::TooManyInstructions {
            actual: instructions.len(),
            limit: limits.instructions,
        });
    }
    let slot_count = schema.parameter_types.len() + schema.state_types.len();
    if slot_count > limits.parameter_and_state_slots {
        return Err(ValueProgramValidationError::TooManySlots {
            actual: slot_count,
            limit: limits.parameter_and_state_slots,
        });
    }
    Ok(())
}

fn validate_slot(
    instruction: usize,
    kind: &'static str,
    slot: u16,
    ty: FxRuntimeType,
    declared: &[FxRuntimeType],
) -> Result<(), ValueProgramValidationError> {
    let Some(expected) = declared.get(usize::from(slot)).copied() else {
        return Err(ValueProgramValidationError::SlotOutOfBounds {
            instruction,
            kind,
            slot,
            available: declared.len(),
        });
    };
    if expected != ty {
        return Err(ValueProgramValidationError::SlotTypeMismatch {
            instruction,
            kind,
            slot,
            expected,
            actual: ty,
        });
    }
    Ok(())
}

fn validate_unary(
    instruction: usize,
    operation: &'static str,
    stack: &mut Vec<FxRuntimeType>,
    result: fn(FxRuntimeType) -> Option<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    let operand = pop_types(instruction, stack, 1)?[0];
    result(operand).map_or_else(
        || {
            Err(ValueProgramValidationError::InvalidOperands {
                instruction,
                operation,
                operands: vec![operand],
            })
        },
        |ty| {
            stack.push(ty);
            Ok(())
        },
    )
}

fn validate_binary(
    instruction: usize,
    operation: &'static str,
    stack: &mut Vec<FxRuntimeType>,
    result: fn(FxRuntimeType, FxRuntimeType) -> Option<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    let operands = pop_types(instruction, stack, 2)?;
    result(operands[0], operands[1]).map_or_else(
        || {
            Err(ValueProgramValidationError::InvalidOperands {
                instruction,
                operation,
                operands,
            })
        },
        |ty| {
            stack.push(ty);
            Ok(())
        },
    )
}

fn validate_clamp(
    instruction: usize,
    stack: &mut Vec<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    let operands = pop_types(instruction, stack, 3)?;
    if operands[0] == operands[1]
        && operands[1] == operands[2]
        && order_result(operands[0], operands[1]).is_some()
    {
        stack.push(operands[0]);
        Ok(())
    } else {
        Err(ValueProgramValidationError::InvalidOperands {
            instruction,
            operation: "clamp",
            operands,
        })
    }
}

fn validate_select(
    instruction: usize,
    stack: &mut Vec<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    let operands = pop_types(instruction, stack, 3)?;
    if operands[0] == FxRuntimeType::Bool && operands[1] == operands[2] {
        stack.push(operands[1]);
        Ok(())
    } else {
        Err(ValueProgramValidationError::InvalidOperands {
            instruction,
            operation: "select",
            operands,
        })
    }
}

fn validate_make_vec2(
    instruction: usize,
    stack: &mut Vec<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    let operands = pop_types(instruction, stack, 2)?;
    if operands == [FxRuntimeType::F32, FxRuntimeType::F32] {
        stack.push(FxRuntimeType::Vec2);
        Ok(())
    } else {
        Err(ValueProgramValidationError::InvalidOperands {
            instruction,
            operation: "make_vec2",
            operands,
        })
    }
}

fn validate_make_transform(
    instruction: usize,
    stack: &mut Vec<FxRuntimeType>,
) -> Result<(), ValueProgramValidationError> {
    const FIELDS: [FxRuntimeType; 10] = [
        FxRuntimeType::Length,
        FxRuntimeType::Length,
        FxRuntimeType::F32,
        FxRuntimeType::F32,
        FxRuntimeType::Angle,
        FxRuntimeType::Angle,
        FxRuntimeType::Angle,
        FxRuntimeType::Length,
        FxRuntimeType::Length,
        FxRuntimeType::F32,
    ];
    let operands = pop_types(instruction, stack, FIELDS.len())?;
    if operands == FIELDS {
        stack.push(FxRuntimeType::Transform2D);
        Ok(())
    } else {
        Err(ValueProgramValidationError::InvalidOperands {
            instruction,
            operation: "make_transform_2d",
            operands,
        })
    }
}

fn pop_types(
    instruction: usize,
    stack: &mut Vec<FxRuntimeType>,
    count: usize,
) -> Result<Vec<FxRuntimeType>, ValueProgramValidationError> {
    if stack.len() < count {
        return Err(ValueProgramValidationError::StackUnderflow {
            instruction,
            required: count,
            available: stack.len(),
        });
    }
    Ok(stack.split_off(stack.len() - count))
}

fn neg_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    matches!(
        ty,
        FxRuntimeType::I32
            | FxRuntimeType::F32
            | FxRuntimeType::Length
            | FxRuntimeType::Angle
            | FxRuntimeType::Seconds
            | FxRuntimeType::Vec2
    )
    .then_some(ty)
}

fn abs_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    neg_result(ty)
}

fn add_sub_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    (left == right).then(|| neg_result(left)).flatten()
}

fn mul_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    match (left, right) {
        (FxRuntimeType::I32, FxRuntimeType::I32) => Some(FxRuntimeType::I32),
        (FxRuntimeType::F32, FxRuntimeType::F32) => Some(FxRuntimeType::F32),
        (unit, FxRuntimeType::F32) | (FxRuntimeType::F32, unit)
            if matches!(
                unit,
                FxRuntimeType::Length
                    | FxRuntimeType::Angle
                    | FxRuntimeType::Seconds
                    | FxRuntimeType::Vec2
            ) =>
        {
            Some(unit)
        }
        _ => None,
    }
}

fn div_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    match (left, right) {
        (FxRuntimeType::I32, FxRuntimeType::I32) => Some(FxRuntimeType::I32),
        (FxRuntimeType::F32, FxRuntimeType::F32) => Some(FxRuntimeType::F32),
        (unit, FxRuntimeType::F32)
            if matches!(
                unit,
                FxRuntimeType::Length
                    | FxRuntimeType::Angle
                    | FxRuntimeType::Seconds
                    | FxRuntimeType::Vec2
            ) =>
        {
            Some(unit)
        }
        (left, right)
            if left == right
                && matches!(
                    left,
                    FxRuntimeType::Length | FxRuntimeType::Angle | FxRuntimeType::Seconds
                ) =>
        {
            Some(FxRuntimeType::F32)
        }
        _ => None,
    }
}

fn order_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    (left == right
        && matches!(
            left,
            FxRuntimeType::I32
                | FxRuntimeType::F32
                | FxRuntimeType::Length
                | FxRuntimeType::Angle
                | FxRuntimeType::Seconds
        ))
    .then_some(left)
}

fn trig_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    matches!(ty, FxRuntimeType::F32 | FxRuntimeType::Angle).then_some(FxRuntimeType::F32)
}

fn f32_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    (ty == FxRuntimeType::F32).then_some(FxRuntimeType::F32)
}

fn bool_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    (ty == FxRuntimeType::Bool).then_some(FxRuntimeType::Bool)
}

fn bools_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    (left == FxRuntimeType::Bool && right == FxRuntimeType::Bool).then_some(FxRuntimeType::Bool)
}

fn equal_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    (left == right).then_some(FxRuntimeType::Bool)
}

fn compare_result(left: FxRuntimeType, right: FxRuntimeType) -> Option<FxRuntimeType> {
    order_result(left, right).map(|_| FxRuntimeType::Bool)
}

fn hash_noise_result(ty: FxRuntimeType) -> Option<FxRuntimeType> {
    (ty == FxRuntimeType::I32).then_some(FxRuntimeType::F32)
}

fn hash_instruction(hasher: &mut blake3::Hasher, instruction: &ValueInstruction) {
    let tag = match instruction {
        ValueInstruction::Constant { value } => {
            hasher.update(&[0]);
            hash_runtime_value(hasher, value);
            return;
        }
        ValueInstruction::LoadParameter { slot, ty } => {
            hasher.update(&[1]);
            hasher.update(&slot.to_le_bytes());
            hasher.update(&[*ty as u8]);
            return;
        }
        ValueInstruction::LoadState { slot, ty } => {
            hasher.update(&[2]);
            hasher.update(&slot.to_le_bytes());
            hasher.update(&[*ty as u8]);
            return;
        }
        ValueInstruction::LoadContext { slot } => {
            hasher.update(&[3, *slot as u8]);
            return;
        }
        ValueInstruction::Neg => 4,
        ValueInstruction::Add => 5,
        ValueInstruction::Sub => 6,
        ValueInstruction::Mul => 7,
        ValueInstruction::Div => 8,
        ValueInstruction::Abs => 9,
        ValueInstruction::Min => 10,
        ValueInstruction::Max => 11,
        ValueInstruction::Clamp => 12,
        ValueInstruction::Sin => 13,
        ValueInstruction::Cos => 14,
        ValueInstruction::Floor => 15,
        ValueInstruction::Fract => 16,
        ValueInstruction::Equal => 17,
        ValueInstruction::Less => 18,
        ValueInstruction::LessEqual => 19,
        ValueInstruction::Greater => 20,
        ValueInstruction::GreaterEqual => 21,
        ValueInstruction::Not => 22,
        ValueInstruction::And => 23,
        ValueInstruction::Or => 24,
        ValueInstruction::Select => 25,
        ValueInstruction::HashNoise => 26,
        ValueInstruction::MakeVec2 => 27,
        ValueInstruction::MakeTransform2D => 28,
        ValueInstruction::Return => 29,
    };
    hasher.update(&[tag]);
}
