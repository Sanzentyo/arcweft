use super::*;

pub(super) fn runtime_evaluated_effect(
    effect: &CheckedEvaluatedEffect,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeEvaluatedEffectFact, RuntimeSemanticProjectionError> {
    let operand = |operand: &CheckedEvaluatedEffectOperand| {
        runtime_evaluated_effect_operand(operand, symbols, world, analysis)
    };
    let lower_fields =
        |fields: &[CheckedEffectField]| runtime_effect_fields(fields, symbols, world, analysis);
    let operation = match effect.operation() {
        CheckedEvaluatedEffectOperation::Log {
            level,
            message,
            fields,
        } => RuntimeEvaluatedEffect::Log {
            level: runtime_log_level(*level),
            message: operand(message)?,
            fields: lower_fields(fields)?,
        },
        CheckedEvaluatedEffectOperation::SignalWrite { target, value } => {
            RuntimeEvaluatedEffect::SignalWrite {
                target: operand(target)?,
                value: operand(value)?,
            }
        }
        CheckedEvaluatedEffectOperation::MetricWrite { target, value } => {
            RuntimeEvaluatedEffect::MetricWrite {
                target: operand(target)?,
                value: operand(value)?,
            }
        }
        CheckedEvaluatedEffectOperation::EmitEvent { event, fields } => {
            RuntimeEvaluatedEffect::EmitEvent {
                event: operand(event)?,
                fields: lower_fields(fields)?,
            }
        }
        CheckedEvaluatedEffectOperation::Panic { message } => RuntimeEvaluatedEffect::Panic {
            message: operand(message)?,
        },
        CheckedEvaluatedEffectOperation::Fail { message } => RuntimeEvaluatedEffect::Fail {
            message: operand(message)?,
        },
        CheckedEvaluatedEffectOperation::Bail { message } => RuntimeEvaluatedEffect::Bail {
            message: operand(message)?,
        },
        CheckedEvaluatedEffectOperation::Ensure { condition, message } => {
            RuntimeEvaluatedEffect::Ensure {
                condition: operand(condition)?,
                message: operand(message)?,
            }
        }
        CheckedEvaluatedEffectOperation::Drop { target, invocation } => {
            RuntimeEvaluatedEffect::Drop {
                target: operand(target)?,
                policy: match invocation {
                    CheckedDropInvocation::Drop | CheckedDropInvocation::DropOptional => {
                        RuntimeDropPolicyFact::Default
                    }
                    CheckedDropInvocation::DropWithPolicy { policy, .. } => match policy {
                        CheckedExplicitDropPolicy::Cancel => RuntimeDropPolicyFact::Cancel,
                        CheckedExplicitDropPolicy::Stop { fade } => RuntimeDropPolicyFact::Stop {
                            fade: match fade {
                                CheckedDropFade::Constant(value) => {
                                    RuntimeDropFadeFact::Constant(*value)
                                }
                                CheckedDropFade::Operand(value) => {
                                    RuntimeDropFadeFact::Operand(operand(value.operand())?)
                                }
                            },
                        },
                        CheckedExplicitDropPolicy::Finish => RuntimeDropPolicyFact::Finish,
                        CheckedExplicitDropPolicy::Release => RuntimeDropPolicyFact::Release,
                        CheckedExplicitDropPolicy::Detach => RuntimeDropPolicyFact::Detach,
                    },
                },
            }
        }
    };
    Ok(RuntimeEvaluatedEffectFact::new(
        effect.application().raw().expression(),
        operation,
    ))
}

fn runtime_evaluated_effect_operand(
    operand: &CheckedEvaluatedEffectOperand,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeEvaluatedEffectOperandFact, RuntimeSemanticProjectionError> {
    Ok(RuntimeEvaluatedEffectOperandFact::new(
        runtime_call_operand_source(operand.source().raw()),
        runtime_type(operand.ty(), symbols, world, analysis)?,
    ))
}

fn runtime_effect_fields(
    fields: &[CheckedEffectField],
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<Box<[RuntimeEffectFieldFact]>, RuntimeSemanticProjectionError> {
    fields
        .iter()
        .map(|field| {
            Ok(RuntimeEffectFieldFact::new(
                field.open_argument().binding().as_str(),
                runtime_evaluated_effect_operand(field.operand(), symbols, world, analysis)?,
            ))
        })
        .collect()
}

const fn runtime_log_level(level: CallableLogLevel) -> RuntimeLogLevel {
    match level {
        CallableLogLevel::Trace => RuntimeLogLevel::Trace,
        CallableLogLevel::Debug => RuntimeLogLevel::Debug,
        CallableLogLevel::Info => RuntimeLogLevel::Info,
        CallableLogLevel::Warn => RuntimeLogLevel::Warn,
        CallableLogLevel::Error => RuntimeLogLevel::Error,
    }
}
