//! One deterministic reference evaluator used by every renderer path.

use std::cmp::Ordering;

use super::{
    program::{
        FxContextSlot, FxEvaluationBudget, FxEvaluationError, ValidatedValueProgram,
        ValueInstruction, ValueProgramInputs,
    },
    state::FxSampleContext,
    value::{
        Angle, FiniteF32, FxColor, FxRuntimeType, FxRuntimeValue, FxVec2, Length, Opacity, Seconds,
        Transform2D,
    },
};

pub(super) fn evaluate(
    program: &ValidatedValueProgram,
    inputs: ValueProgramInputs<'_>,
    context: FxSampleContext,
    budget: &mut FxEvaluationBudget,
) -> Result<FxRuntimeValue, FxEvaluationError> {
    validate_inputs(
        "parameter",
        program.schema().parameter_types(),
        inputs.parameters,
    )?;
    validate_inputs("state", program.schema().state_types(), inputs.state)?;

    let mut stack = Vec::with_capacity(program.instructions().len().min(64));
    for (index, instruction) in program.instructions().iter().enumerate() {
        budget.charge(index)?;
        match instruction {
            ValueInstruction::Constant { value } => stack.push(*value),
            ValueInstruction::LoadParameter { slot, .. } => stack.push(
                inputs
                    .parameters
                    .get(usize::from(*slot))
                    .copied()
                    .ok_or(FxEvaluationError::InvalidProgramState { instruction: index })?,
            ),
            ValueInstruction::LoadState { slot, .. } => stack.push(
                inputs
                    .state
                    .get(usize::from(*slot))
                    .copied()
                    .ok_or(FxEvaluationError::InvalidProgramState { instruction: index })?,
            ),
            ValueInstruction::LoadContext { slot } => stack.push(context_value(*slot, context)?),
            ValueInstruction::Return => return pop_one(index, &mut stack),
            _ => execute_operator(index, instruction, &mut stack, context)?,
        }
    }
    Err(FxEvaluationError::InvalidProgramState {
        instruction: program.instructions().len(),
    })
}

fn execute_operator(
    index: usize,
    instruction: &ValueInstruction,
    stack: &mut Vec<FxRuntimeValue>,
    context: FxSampleContext,
) -> Result<(), FxEvaluationError> {
    match instruction {
        ValueInstruction::Neg
        | ValueInstruction::Abs
        | ValueInstruction::Sin
        | ValueInstruction::Cos
        | ValueInstruction::Floor
        | ValueInstruction::Fract => execute_unary(index, instruction, stack)?,
        ValueInstruction::Add
        | ValueInstruction::Sub
        | ValueInstruction::Mul
        | ValueInstruction::Div
        | ValueInstruction::Min
        | ValueInstruction::Max => execute_binary(index, instruction, stack)?,
        ValueInstruction::Equal
        | ValueInstruction::Less
        | ValueInstruction::LessEqual
        | ValueInstruction::Greater
        | ValueInstruction::GreaterEqual => execute_comparison(index, instruction, stack)?,
        ValueInstruction::Not | ValueInstruction::And | ValueInstruction::Or => {
            execute_boolean(index, instruction, stack)?;
        }
        ValueInstruction::Clamp => {
            let (value, minimum, maximum) = pop_three(index, stack)?;
            stack.push(clamp(index, value, minimum, maximum)?);
        }
        ValueInstruction::Select => {
            let (condition, when_true, when_false) = pop_three(index, stack)?;
            let FxRuntimeValue::Bool(condition) = condition else {
                return Err(FxEvaluationError::InvalidProgramState { instruction: index });
            };
            stack.push(if condition { when_true } else { when_false });
        }
        ValueInstruction::HashNoise => {
            let FxRuntimeValue::I32(bucket) = pop_one(index, stack)? else {
                return Err(FxEvaluationError::InvalidProgramState { instruction: index });
            };
            let noise = context.deterministic_noise(bucket).map_err(|_| {
                FxEvaluationError::NonFiniteResult {
                    instruction: index,
                    operation: "hash_noise",
                }
            })?;
            stack.push(FxRuntimeValue::F32(noise));
        }
        ValueInstruction::FloorToI32 => {
            let FxRuntimeValue::F32(value) = pop_one(index, stack)? else {
                return Err(FxEvaluationError::InvalidProgramState { instruction: index });
            };
            stack.push(FxRuntimeValue::I32(floor_to_i32(index, value)?));
        }
        ValueInstruction::MakeVec2 => {
            let (FxRuntimeValue::F32(x), FxRuntimeValue::F32(y)) = pop_two(index, stack)? else {
                return Err(FxEvaluationError::InvalidProgramState { instruction: index });
            };
            stack.push(FxRuntimeValue::Vec2(FxVec2 { x, y }));
        }
        ValueInstruction::MakeColor => {
            let color = make_color(index, stack)?;
            stack.push(FxRuntimeValue::Color(color));
        }
        ValueInstruction::MakeTransform2D => {
            let transform = make_transform(index, stack)?;
            stack.push(FxRuntimeValue::Transform2D(transform));
        }
        ValueInstruction::Constant { .. }
        | ValueInstruction::LoadParameter { .. }
        | ValueInstruction::LoadState { .. }
        | ValueInstruction::LoadContext { .. }
        | ValueInstruction::Return => {
            return Err(FxEvaluationError::InvalidProgramState { instruction: index });
        }
    }
    Ok(())
}

fn floor_to_i32(index: usize, value: FiniteF32) -> Result<i32, FxEvaluationError> {
    let floored = f64::from(value.get().floor());
    if floored < f64::from(i32::MIN) || floored > f64::from(i32::MAX) {
        return Err(FxEvaluationError::IntegerConversion {
            instruction: index,
            operation: "floor_to_i32",
        });
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the finite floored value is checked against the complete i32 range"
    )]
    Ok(floored as i32)
}

fn make_color(index: usize, stack: &mut Vec<FxRuntimeValue>) -> Result<FxColor, FxEvaluationError> {
    let values = pop_many(index, stack, 4)?;
    let [
        FxRuntimeValue::F32(red),
        FxRuntimeValue::F32(green),
        FxRuntimeValue::F32(blue),
        FxRuntimeValue::F32(alpha),
    ] = values.as_slice()
    else {
        return Err(FxEvaluationError::InvalidProgramState { instruction: index });
    };
    let channel = |value| {
        Opacity::try_new(value)
            .map_err(|_| FxEvaluationError::InvalidOpacity { instruction: index })
    };
    Ok(FxColor::new(
        channel(*red)?,
        channel(*green)?,
        channel(*blue)?,
        channel(*alpha)?,
    ))
}

fn execute_unary(
    index: usize,
    instruction: &ValueInstruction,
    stack: &mut Vec<FxRuntimeValue>,
) -> Result<(), FxEvaluationError> {
    let value = pop_one(index, stack)?;
    let result = match instruction {
        ValueInstruction::Neg => neg(index, value)?,
        ValueInstruction::Abs => abs(index, value)?,
        ValueInstruction::Sin => trig(index, value, true)?,
        ValueInstruction::Cos => trig(index, value, false)?,
        ValueInstruction::Floor => floor_fract(index, value, true)?,
        ValueInstruction::Fract => floor_fract(index, value, false)?,
        _ => return Err(FxEvaluationError::InvalidProgramState { instruction: index }),
    };
    stack.push(result);
    Ok(())
}

fn execute_binary(
    index: usize,
    instruction: &ValueInstruction,
    stack: &mut Vec<FxRuntimeValue>,
) -> Result<(), FxEvaluationError> {
    let (left, right) = pop_two(index, stack)?;
    let result = match instruction {
        ValueInstruction::Add => add(index, left, right)?,
        ValueInstruction::Sub => sub(index, left, right)?,
        ValueInstruction::Mul => mul(index, left, right)?,
        ValueInstruction::Div => div(index, left, right)?,
        ValueInstruction::Min => min_max(index, left, right, false)?,
        ValueInstruction::Max => min_max(index, left, right, true)?,
        _ => return Err(FxEvaluationError::InvalidProgramState { instruction: index }),
    };
    stack.push(result);
    Ok(())
}

fn execute_comparison(
    index: usize,
    instruction: &ValueInstruction,
    stack: &mut Vec<FxRuntimeValue>,
) -> Result<(), FxEvaluationError> {
    let (left, right) = pop_two(index, stack)?;
    let result = match instruction {
        ValueInstruction::Equal => left == right,
        ValueInstruction::Less => compare(index, left, right, OrderingTest::Less)?,
        ValueInstruction::LessEqual => compare(index, left, right, OrderingTest::LessEqual)?,
        ValueInstruction::Greater => compare(index, left, right, OrderingTest::Greater)?,
        ValueInstruction::GreaterEqual => compare(index, left, right, OrderingTest::GreaterEqual)?,
        _ => return Err(FxEvaluationError::InvalidProgramState { instruction: index }),
    };
    stack.push(FxRuntimeValue::Bool(result));
    Ok(())
}

fn execute_boolean(
    index: usize,
    instruction: &ValueInstruction,
    stack: &mut Vec<FxRuntimeValue>,
) -> Result<(), FxEvaluationError> {
    let result = match instruction {
        ValueInstruction::Not => {
            let FxRuntimeValue::Bool(value) = pop_one(index, stack)? else {
                return Err(FxEvaluationError::InvalidProgramState { instruction: index });
            };
            !value
        }
        ValueInstruction::And | ValueInstruction::Or => {
            let (FxRuntimeValue::Bool(left), FxRuntimeValue::Bool(right)) = pop_two(index, stack)?
            else {
                return Err(FxEvaluationError::InvalidProgramState { instruction: index });
            };
            if matches!(instruction, ValueInstruction::And) {
                left && right
            } else {
                left || right
            }
        }
        _ => return Err(FxEvaluationError::InvalidProgramState { instruction: index }),
    };
    stack.push(FxRuntimeValue::Bool(result));
    Ok(())
}

fn validate_inputs(
    kind: &'static str,
    declared: &[FxRuntimeType],
    values: &[FxRuntimeValue],
) -> Result<(), FxEvaluationError> {
    if declared.len() != values.len() {
        return Err(FxEvaluationError::InputCount {
            kind,
            expected: declared.len(),
            actual: values.len(),
        });
    }
    declared
        .iter()
        .zip(values)
        .enumerate()
        .find_map(|(slot, (expected, value))| {
            (*expected != value.value_type()).then_some(FxEvaluationError::InputType {
                kind,
                slot,
                expected: *expected,
                actual: value.value_type(),
            })
        })
        .map_or(Ok(()), Err)
}

fn context_value(
    slot: FxContextSlot,
    context: FxSampleContext,
) -> Result<FxRuntimeValue, FxEvaluationError> {
    Ok(match slot {
        FxContextSlot::Time => FxRuntimeValue::F32(context.time()),
        FxContextSlot::Ordinal => FxRuntimeValue::F32(context.ordinal_value().map_err(|_| {
            FxEvaluationError::NonFiniteResult {
                instruction: 0,
                operation: "load_ordinal",
            }
        })?),
        FxContextSlot::OrdinalPhase => {
            FxRuntimeValue::F32(context.ordinal_phase().map_err(|_| {
                FxEvaluationError::NonFiniteResult {
                    instruction: 0,
                    operation: "ordinal_phase",
                }
            })?)
        }
        FxContextSlot::ReduceMotion => FxRuntimeValue::Bool(context.reduce_motion()),
    })
}

fn neg(index: usize, value: FxRuntimeValue) -> Result<FxRuntimeValue, FxEvaluationError> {
    Ok(match value {
        FxRuntimeValue::I32(value) => FxRuntimeValue::I32(value.checked_neg().ok_or(
            FxEvaluationError::IntegerOverflow {
                instruction: index,
                operation: "neg",
            },
        )?),
        FxRuntimeValue::F32(value) => FxRuntimeValue::F32(finite(index, "neg", -value.get())?),
        FxRuntimeValue::Length(value) => {
            FxRuntimeValue::Length(length(index, "neg", -value.pixels())?)
        }
        FxRuntimeValue::Angle(value) => {
            FxRuntimeValue::Angle(angle(index, "neg", -value.radians())?)
        }
        FxRuntimeValue::Seconds(value) => {
            FxRuntimeValue::Seconds(seconds(index, "neg", -value.seconds())?)
        }
        FxRuntimeValue::Vec2(value) => FxRuntimeValue::Vec2(FxVec2 {
            x: finite(index, "neg", -value.x.get())?,
            y: finite(index, "neg", -value.y.get())?,
        }),
        _ => return Err(FxEvaluationError::InvalidProgramState { instruction: index }),
    })
}

fn abs(index: usize, value: FxRuntimeValue) -> Result<FxRuntimeValue, FxEvaluationError> {
    Ok(match value {
        FxRuntimeValue::I32(value) => FxRuntimeValue::I32(value.checked_abs().ok_or(
            FxEvaluationError::IntegerOverflow {
                instruction: index,
                operation: "abs",
            },
        )?),
        FxRuntimeValue::F32(value) => FxRuntimeValue::F32(finite(index, "abs", value.get().abs())?),
        FxRuntimeValue::Length(value) => {
            FxRuntimeValue::Length(length(index, "abs", value.pixels().abs())?)
        }
        FxRuntimeValue::Angle(value) => {
            FxRuntimeValue::Angle(angle(index, "abs", value.radians().abs())?)
        }
        FxRuntimeValue::Seconds(value) => {
            FxRuntimeValue::Seconds(seconds(index, "abs", value.seconds().abs())?)
        }
        FxRuntimeValue::Vec2(value) => FxRuntimeValue::Vec2(FxVec2 {
            x: finite(index, "abs", value.x.get().abs())?,
            y: finite(index, "abs", value.y.get().abs())?,
        }),
        _ => return Err(FxEvaluationError::InvalidProgramState { instruction: index }),
    })
}

fn add(
    index: usize,
    left: FxRuntimeValue,
    right: FxRuntimeValue,
) -> Result<FxRuntimeValue, FxEvaluationError> {
    Ok(match (left, right) {
        (FxRuntimeValue::I32(left), FxRuntimeValue::I32(right)) => FxRuntimeValue::I32(
            left.checked_add(right)
                .ok_or(FxEvaluationError::IntegerOverflow {
                    instruction: index,
                    operation: "add",
                })?,
        ),
        (FxRuntimeValue::F32(left), FxRuntimeValue::F32(right)) => {
            FxRuntimeValue::F32(finite(index, "add", left.get() + right.get())?)
        }
        (FxRuntimeValue::Length(left), FxRuntimeValue::Length(right)) => {
            FxRuntimeValue::Length(length(index, "add", left.pixels() + right.pixels())?)
        }
        (FxRuntimeValue::Angle(left), FxRuntimeValue::Angle(right)) => {
            FxRuntimeValue::Angle(angle(index, "add", left.radians() + right.radians())?)
        }
        (FxRuntimeValue::Seconds(left), FxRuntimeValue::Seconds(right)) => {
            FxRuntimeValue::Seconds(seconds(index, "add", left.seconds() + right.seconds())?)
        }
        (FxRuntimeValue::Vec2(left), FxRuntimeValue::Vec2(right)) => FxRuntimeValue::Vec2(FxVec2 {
            x: finite(index, "add", left.x.get() + right.x.get())?,
            y: finite(index, "add", left.y.get() + right.y.get())?,
        }),
        (left, right) => return unit_mismatch(index, "add", left, right),
    })
}

fn sub(
    index: usize,
    left: FxRuntimeValue,
    right: FxRuntimeValue,
) -> Result<FxRuntimeValue, FxEvaluationError> {
    Ok(match (left, right) {
        (FxRuntimeValue::I32(left), FxRuntimeValue::I32(right)) => FxRuntimeValue::I32(
            left.checked_sub(right)
                .ok_or(FxEvaluationError::IntegerOverflow {
                    instruction: index,
                    operation: "sub",
                })?,
        ),
        (FxRuntimeValue::F32(left), FxRuntimeValue::F32(right)) => {
            FxRuntimeValue::F32(finite(index, "sub", left.get() - right.get())?)
        }
        (FxRuntimeValue::Length(left), FxRuntimeValue::Length(right)) => {
            FxRuntimeValue::Length(length(index, "sub", left.pixels() - right.pixels())?)
        }
        (FxRuntimeValue::Angle(left), FxRuntimeValue::Angle(right)) => {
            FxRuntimeValue::Angle(angle(index, "sub", left.radians() - right.radians())?)
        }
        (FxRuntimeValue::Seconds(left), FxRuntimeValue::Seconds(right)) => {
            FxRuntimeValue::Seconds(seconds(index, "sub", left.seconds() - right.seconds())?)
        }
        (FxRuntimeValue::Vec2(left), FxRuntimeValue::Vec2(right)) => FxRuntimeValue::Vec2(FxVec2 {
            x: finite(index, "sub", left.x.get() - right.x.get())?,
            y: finite(index, "sub", left.y.get() - right.y.get())?,
        }),
        (left, right) => return unit_mismatch(index, "sub", left, right),
    })
}

fn mul(
    index: usize,
    left: FxRuntimeValue,
    right: FxRuntimeValue,
) -> Result<FxRuntimeValue, FxEvaluationError> {
    Ok(match (left, right) {
        (FxRuntimeValue::I32(left), FxRuntimeValue::I32(right)) => FxRuntimeValue::I32(
            left.checked_mul(right)
                .ok_or(FxEvaluationError::IntegerOverflow {
                    instruction: index,
                    operation: "mul",
                })?,
        ),
        (FxRuntimeValue::F32(left), FxRuntimeValue::F32(right)) => {
            FxRuntimeValue::F32(finite_mul_div(
                index,
                "mul",
                left.get(),
                right.get(),
                left.get() * right.get(),
            )?)
        }
        (FxRuntimeValue::Length(value), FxRuntimeValue::F32(scale))
        | (FxRuntimeValue::F32(scale), FxRuntimeValue::Length(value)) => FxRuntimeValue::Length(
            Length::try_pixels(
                finite_mul_div(
                    index,
                    "mul",
                    value.pixels(),
                    scale.get(),
                    value.pixels() * scale.get(),
                )?
                .get(),
            )
            .map_err(|_| FxEvaluationError::NonFiniteResult {
                instruction: index,
                operation: "mul",
            })?,
        ),
        (FxRuntimeValue::Angle(value), FxRuntimeValue::F32(scale))
        | (FxRuntimeValue::F32(scale), FxRuntimeValue::Angle(value)) => {
            FxRuntimeValue::Angle(angle(
                index,
                "mul",
                finite_mul_div(
                    index,
                    "mul",
                    value.radians(),
                    scale.get(),
                    value.radians() * scale.get(),
                )?
                .get(),
            )?)
        }
        (FxRuntimeValue::Seconds(value), FxRuntimeValue::F32(scale))
        | (FxRuntimeValue::F32(scale), FxRuntimeValue::Seconds(value)) => {
            FxRuntimeValue::Seconds(seconds(
                index,
                "mul",
                finite_mul_div(
                    index,
                    "mul",
                    value.seconds(),
                    scale.get(),
                    value.seconds() * scale.get(),
                )?
                .get(),
            )?)
        }
        (FxRuntimeValue::Vec2(value), FxRuntimeValue::F32(scale))
        | (FxRuntimeValue::F32(scale), FxRuntimeValue::Vec2(value)) => {
            FxRuntimeValue::Vec2(FxVec2 {
                x: finite_mul_div(
                    index,
                    "mul",
                    value.x.get(),
                    scale.get(),
                    value.x.get() * scale.get(),
                )?,
                y: finite_mul_div(
                    index,
                    "mul",
                    value.y.get(),
                    scale.get(),
                    value.y.get() * scale.get(),
                )?,
            })
        }
        (left, right) => return unit_mismatch(index, "mul", left, right),
    })
}

fn div(
    index: usize,
    left: FxRuntimeValue,
    right: FxRuntimeValue,
) -> Result<FxRuntimeValue, FxEvaluationError> {
    if is_zero(&right) {
        return Err(FxEvaluationError::DivisionByZero { instruction: index });
    }
    Ok(match (left, right) {
        (FxRuntimeValue::I32(left), FxRuntimeValue::I32(right)) => FxRuntimeValue::I32(
            left.checked_div(right)
                .ok_or(FxEvaluationError::IntegerOverflow {
                    instruction: index,
                    operation: "div",
                })?,
        ),
        (FxRuntimeValue::F32(left), FxRuntimeValue::F32(right)) => {
            FxRuntimeValue::F32(finite_mul_div(
                index,
                "div",
                left.get(),
                right.get(),
                left.get() / right.get(),
            )?)
        }
        (unit, FxRuntimeValue::F32(divisor)) => divide_by_scalar(index, unit, divisor)?,
        (left, right) => divide_equal_units(index, left, right)?,
    })
}

fn divide_by_scalar(
    index: usize,
    value: FxRuntimeValue,
    divisor: FiniteF32,
) -> Result<FxRuntimeValue, FxEvaluationError> {
    let divisor = divisor.get();
    Ok(match value {
        FxRuntimeValue::Length(value) => FxRuntimeValue::Length(length(
            index,
            "div",
            finite_mul_div(
                index,
                "div",
                value.pixels(),
                divisor,
                value.pixels() / divisor,
            )?
            .get(),
        )?),
        FxRuntimeValue::Angle(value) => FxRuntimeValue::Angle(angle(
            index,
            "div",
            finite_mul_div(
                index,
                "div",
                value.radians(),
                divisor,
                value.radians() / divisor,
            )?
            .get(),
        )?),
        FxRuntimeValue::Seconds(value) => FxRuntimeValue::Seconds(seconds(
            index,
            "div",
            finite_mul_div(
                index,
                "div",
                value.seconds(),
                divisor,
                value.seconds() / divisor,
            )?
            .get(),
        )?),
        FxRuntimeValue::Vec2(value) => FxRuntimeValue::Vec2(FxVec2 {
            x: finite_mul_div(
                index,
                "div",
                value.x.get(),
                divisor,
                value.x.get() / divisor,
            )?,
            y: finite_mul_div(
                index,
                "div",
                value.y.get(),
                divisor,
                value.y.get() / divisor,
            )?,
        }),
        other => {
            return unit_mismatch(
                index,
                "div",
                other,
                FxRuntimeValue::F32(FiniteF32::try_new(divisor).map_err(|_| {
                    FxEvaluationError::NonFiniteResult {
                        instruction: index,
                        operation: "div",
                    }
                })?),
            );
        }
    })
}

fn divide_equal_units(
    index: usize,
    left: FxRuntimeValue,
    right: FxRuntimeValue,
) -> Result<FxRuntimeValue, FxEvaluationError> {
    let (left_value, right_value) = match (left, right) {
        (FxRuntimeValue::Length(left), FxRuntimeValue::Length(right)) => {
            (left.pixels(), right.pixels())
        }
        (FxRuntimeValue::Angle(left), FxRuntimeValue::Angle(right)) => {
            (left.radians(), right.radians())
        }
        (FxRuntimeValue::Seconds(left), FxRuntimeValue::Seconds(right)) => {
            (left.seconds(), right.seconds())
        }
        (left, right) => return unit_mismatch(index, "div", left, right),
    };
    Ok(FxRuntimeValue::F32(finite_mul_div(
        index,
        "div",
        left_value,
        right_value,
        left_value / right_value,
    )?))
}

fn min_max(
    index: usize,
    left: FxRuntimeValue,
    right: FxRuntimeValue,
    maximum: bool,
) -> Result<FxRuntimeValue, FxEvaluationError> {
    let choose_right = compare_values(
        index,
        &left,
        &right,
        if maximum {
            OrderingTest::Less
        } else {
            OrderingTest::Greater
        },
    )?;
    Ok(if choose_right { right } else { left })
}

fn clamp(
    index: usize,
    value: FxRuntimeValue,
    minimum: FxRuntimeValue,
    maximum: FxRuntimeValue,
) -> Result<FxRuntimeValue, FxEvaluationError> {
    if compare_values(index, &minimum, &maximum, OrderingTest::Greater)? {
        return Err(FxEvaluationError::InvalidClampBounds { instruction: index });
    }
    if compare_values(index, &value, &minimum, OrderingTest::Less)? {
        Ok(minimum)
    } else if compare_values(index, &value, &maximum, OrderingTest::Greater)? {
        Ok(maximum)
    } else {
        Ok(value)
    }
}

fn trig(
    index: usize,
    value: FxRuntimeValue,
    sine: bool,
) -> Result<FxRuntimeValue, FxEvaluationError> {
    let radians = match value {
        FxRuntimeValue::F32(value) => value.get(),
        FxRuntimeValue::Angle(value) => value.radians(),
        _ => return Err(FxEvaluationError::InvalidProgramState { instruction: index }),
    };
    Ok(FxRuntimeValue::F32(finite(
        index,
        if sine { "sin" } else { "cos" },
        if sine { radians.sin() } else { radians.cos() },
    )?))
}

fn floor_fract(
    index: usize,
    value: FxRuntimeValue,
    floor: bool,
) -> Result<FxRuntimeValue, FxEvaluationError> {
    let FxRuntimeValue::F32(value) = value else {
        return Err(FxEvaluationError::InvalidProgramState { instruction: index });
    };
    Ok(FxRuntimeValue::F32(finite(
        index,
        if floor { "floor" } else { "fract" },
        if floor {
            value.get().floor()
        } else {
            value.get().fract()
        },
    )?))
}

fn compare(
    index: usize,
    left: FxRuntimeValue,
    right: FxRuntimeValue,
    test: OrderingTest,
) -> Result<bool, FxEvaluationError> {
    compare_values(index, &left, &right, test)
}

fn compare_values(
    index: usize,
    left: &FxRuntimeValue,
    right: &FxRuntimeValue,
    test: OrderingTest,
) -> Result<bool, FxEvaluationError> {
    let ordering = match (left, right) {
        (FxRuntimeValue::I32(left), FxRuntimeValue::I32(right)) => left.cmp(right),
        (FxRuntimeValue::F32(left), FxRuntimeValue::F32(right)) => {
            left.get().total_cmp(&right.get())
        }
        (FxRuntimeValue::Length(left), FxRuntimeValue::Length(right)) => {
            left.pixels().total_cmp(&right.pixels())
        }
        (FxRuntimeValue::Angle(left), FxRuntimeValue::Angle(right)) => {
            left.radians().total_cmp(&right.radians())
        }
        (FxRuntimeValue::Seconds(left), FxRuntimeValue::Seconds(right)) => {
            left.seconds().total_cmp(&right.seconds())
        }
        _ => {
            return Err(FxEvaluationError::UnitMismatch {
                instruction: index,
                operation: "compare",
                left: left.value_type(),
                right: right.value_type(),
            });
        }
    };
    Ok(test.matches(ordering))
}

#[derive(Clone, Copy)]
enum OrderingTest {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl OrderingTest {
    const fn matches(self, ordering: Ordering) -> bool {
        match self {
            Self::Less => matches!(ordering, Ordering::Less),
            Self::LessEqual => !matches!(ordering, Ordering::Greater),
            Self::Greater => matches!(ordering, Ordering::Greater),
            Self::GreaterEqual => !matches!(ordering, Ordering::Less),
        }
    }
}

fn make_transform(
    index: usize,
    stack: &mut Vec<FxRuntimeValue>,
) -> Result<Transform2D, FxEvaluationError> {
    let values = pop_many(index, stack, 10)?;
    let [
        FxRuntimeValue::Length(translate_x),
        FxRuntimeValue::Length(translate_y),
        FxRuntimeValue::F32(scale_x),
        FxRuntimeValue::F32(scale_y),
        FxRuntimeValue::Angle(skew_x),
        FxRuntimeValue::Angle(skew_y),
        FxRuntimeValue::Angle(rotation),
        FxRuntimeValue::Length(origin_x),
        FxRuntimeValue::Length(origin_y),
        FxRuntimeValue::F32(opacity),
    ] = values.as_slice()
    else {
        return Err(FxEvaluationError::InvalidProgramState { instruction: index });
    };
    let transform = Transform2D {
        translate_x: *translate_x,
        translate_y: *translate_y,
        scale_x: *scale_x,
        scale_y: *scale_y,
        skew_x: *skew_x,
        skew_y: *skew_y,
        rotation: *rotation,
        origin_x: *origin_x,
        origin_y: *origin_y,
        opacity: *opacity,
    };
    transform
        .validate()
        .map_err(|_| FxEvaluationError::InvalidOpacity { instruction: index })?;
    Ok(transform)
}

fn is_zero(value: &FxRuntimeValue) -> bool {
    match value {
        FxRuntimeValue::I32(value) => *value == 0,
        FxRuntimeValue::F32(value) => value.get() == 0.0,
        FxRuntimeValue::Length(value) => value.pixels() == 0.0,
        FxRuntimeValue::Angle(value) => value.radians() == 0.0,
        FxRuntimeValue::Seconds(value) => value.seconds() == 0.0,
        _ => false,
    }
}

fn finite(
    index: usize,
    operation: &'static str,
    value: f32,
) -> Result<FiniteF32, FxEvaluationError> {
    FiniteF32::try_new(value).map_err(|_| FxEvaluationError::NonFiniteResult {
        instruction: index,
        operation,
    })
}

fn finite_mul_div(
    index: usize,
    operation: &'static str,
    left: f32,
    right: f32,
    result: f32,
) -> Result<FiniteF32, FxEvaluationError> {
    if result == 0.0 && left != 0.0 && right != 0.0 {
        return Err(FxEvaluationError::Underflow {
            instruction: index,
            operation,
        });
    }
    finite(index, operation, result)
}

fn length(index: usize, operation: &'static str, value: f32) -> Result<Length, FxEvaluationError> {
    Length::try_pixels(value).map_err(|_| FxEvaluationError::NonFiniteResult {
        instruction: index,
        operation,
    })
}

fn angle(index: usize, operation: &'static str, value: f32) -> Result<Angle, FxEvaluationError> {
    Angle::try_radians(value).map_err(|_| FxEvaluationError::NonFiniteResult {
        instruction: index,
        operation,
    })
}

fn seconds(
    index: usize,
    operation: &'static str,
    value: f32,
) -> Result<Seconds, FxEvaluationError> {
    Seconds::try_seconds(value).map_err(|_| FxEvaluationError::NonFiniteResult {
        instruction: index,
        operation,
    })
}

fn unit_mismatch<T>(
    index: usize,
    operation: &'static str,
    left: FxRuntimeValue,
    right: FxRuntimeValue,
) -> Result<T, FxEvaluationError> {
    Err(FxEvaluationError::UnitMismatch {
        instruction: index,
        operation,
        left: left.value_type(),
        right: right.value_type(),
    })
}

fn pop_one(
    instruction: usize,
    stack: &mut Vec<FxRuntimeValue>,
) -> Result<FxRuntimeValue, FxEvaluationError> {
    stack
        .pop()
        .ok_or(FxEvaluationError::InvalidProgramState { instruction })
}

fn pop_two(
    instruction: usize,
    stack: &mut Vec<FxRuntimeValue>,
) -> Result<(FxRuntimeValue, FxRuntimeValue), FxEvaluationError> {
    let values = pop_many(instruction, stack, 2)?;
    let mut values = values.into_iter();
    Ok((
        values
            .next()
            .ok_or(FxEvaluationError::InvalidProgramState { instruction })?,
        values
            .next()
            .ok_or(FxEvaluationError::InvalidProgramState { instruction })?,
    ))
}

fn pop_three(
    instruction: usize,
    stack: &mut Vec<FxRuntimeValue>,
) -> Result<(FxRuntimeValue, FxRuntimeValue, FxRuntimeValue), FxEvaluationError> {
    let values = pop_many(instruction, stack, 3)?;
    let mut values = values.into_iter();
    Ok((
        values
            .next()
            .ok_or(FxEvaluationError::InvalidProgramState { instruction })?,
        values
            .next()
            .ok_or(FxEvaluationError::InvalidProgramState { instruction })?,
        values
            .next()
            .ok_or(FxEvaluationError::InvalidProgramState { instruction })?,
    ))
}

fn pop_many(
    instruction: usize,
    stack: &mut Vec<FxRuntimeValue>,
    count: usize,
) -> Result<Vec<FxRuntimeValue>, FxEvaluationError> {
    if stack.len() < count {
        Err(FxEvaluationError::InvalidProgramState { instruction })
    } else {
        Ok(stack.split_off(stack.len() - count))
    }
}
