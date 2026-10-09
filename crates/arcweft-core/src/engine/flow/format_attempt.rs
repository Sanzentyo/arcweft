//! Native same-fiber continuation for source-ordered formatter operands.

use std::collections::VecDeque;

use crate::engine::{
    Engine, FlowControlStackEntry, FlowControlStackEntryKind, FlowFiberStatus,
    NativeFormatAttemptFrame, NativeFormatOperandFrame, RuntimeCallBackend, RuntimeEvalError,
    RuntimeStepOutput,
};
use crate::plan::FlowOp;
use crate::runtime_id::{RuntimeFormatAttemptId, RuntimePlanTypeId};
use crate::value::{RuntimeExpr, RuntimeFmtParameterId, RuntimeFormatContext, RuntimeValue};

pub(in crate::engine) struct NativeFormatAttemptValues {
    pub context: RuntimeFormatContext,
    pub primary_type: RuntimePlanTypeId,
    pub values: Vec<(RuntimeFmtParameterId, Option<RuntimeValue>)>,
    pub first_recoverable: Option<String>,
}

impl Engine {
    /// Drops the current operand transaction when source control transfer
    /// deliberately exits the formatter before its completion marker.
    pub(super) fn abandon_format_attempt_for_transfer(&mut self) -> bool {
        let Some(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::FormatAttempt(_),
        }) = self.fiber.control_stack.last()
        else {
            return false;
        };
        let Some(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::FormatAttempt(mut frame),
        }) = self.fiber.control_stack.pop()
        else {
            unreachable!("format attempt frame was checked before removal")
        };
        if let Some(active) = frame.active.take() {
            self.fiber.env.pop_scope();
            self.fiber.pending_ops = active.caller_pending_ops;
            self.fiber.cursor = active.resume;
        }
        true
    }

    pub(in crate::engine) fn fail_format_aware_eval(
        &mut self,
        error: RuntimeEvalError,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        if let RuntimeEvalError::RecoverableExpression(failure) = &error
            && self.recover_format_operand_attempt(&failure.to_string(), output, pure_backend)
        {
            return;
        }
        self.fail_eval(error, output);
    }

    pub(super) fn start_format_operand_attempt(
        &mut self,
        attempt: RuntimeFormatAttemptId,
        parameter: RuntimeFmtParameterId,
        body: Vec<FlowOp>,
        value: RuntimeExpr,
        next_op_index: Option<usize>,
    ) -> Result<(), RuntimeEvalError> {
        let manifest = self.plan.format_attempt(attempt).ok_or_else(|| {
            RuntimeEvalError::DialogueContentConstruction(format!(
                "unknown format attempt {attempt}"
            ))
        })?;
        let ordinal = manifest.operand_index(parameter).ok_or_else(|| {
            RuntimeEvalError::DialogueContentConstruction(format!(
                "format attempt {attempt} has no {parameter:?} operand"
            ))
        })?;
        if value.ty() != manifest.operands()[ordinal].ty() {
            return Err(RuntimeEvalError::InvalidExpressionType(value.ty()));
        }
        if ordinal == 0 {
            self.fiber.control_stack.push(FlowControlStackEntry {
                kind: FlowControlStackEntryKind::FormatAttempt(NativeFormatAttemptFrame {
                    attempt,
                    context: self.format_context.clone(),
                    values: vec![None; manifest.operands().len()],
                    first_recoverable: None,
                    next_operand: 0,
                    active: None,
                }),
            });
        }
        let resume = self.resume_cursor(next_op_index);
        let caller_pending_ops = std::mem::take(&mut self.fiber.pending_ops);
        let Some(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::FormatAttempt(frame),
        }) = self.fiber.control_stack.last_mut()
        else {
            self.fiber.pending_ops = caller_pending_ops;
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "format operand has no current attempt frame".to_owned(),
            ));
        };
        if frame.attempt != attempt || frame.next_operand != ordinal || frame.active.is_some() {
            self.fiber.pending_ops = caller_pending_ops;
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "format operands are out of source order".to_owned(),
            ));
        }
        frame.active = Some(NativeFormatOperandFrame {
            ordinal,
            resume,
            caller_pending_ops,
        });
        self.fiber.env.push_scope_with_capacity(0);
        self.fiber.cursor = resume;
        self.fiber.pending_ops = VecDeque::from(body);
        self.fiber
            .pending_ops
            .push_back(FlowOp::CompleteFormatOperand {
                attempt,
                parameter,
                value,
            });
        Ok(())
    }

    pub(super) fn complete_format_operand_attempt(
        &mut self,
        attempt: RuntimeFormatAttemptId,
        parameter: RuntimeFmtParameterId,
        value: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), RuntimeEvalError> {
        let evaluated = self.evaluate_expr_with_backend(value, pure_backend)?;
        let manifest = self.plan.format_attempt(attempt).ok_or_else(|| {
            RuntimeEvalError::DialogueContentConstruction(format!(
                "unknown format attempt {attempt}"
            ))
        })?;
        let ordinal = manifest.operand_index(parameter).ok_or_else(|| {
            RuntimeEvalError::DialogueContentConstruction(format!(
                "format attempt {attempt} has no {parameter:?} operand"
            ))
        })?;
        let expected = manifest.operands()[ordinal].ty();
        if !self.plan.value_matches_type(expected, &evaluated)? {
            return Err(RuntimeEvalError::InvalidExpressionType(expected));
        }
        let Some(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::FormatAttempt(frame),
        }) = self.fiber.control_stack.last_mut()
        else {
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "format completion has no current attempt frame".to_owned(),
            ));
        };
        if frame.attempt != attempt
            || frame.next_operand != ordinal
            || frame
                .active
                .as_ref()
                .is_none_or(|active| active.ordinal != ordinal)
        {
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "format completion does not match its active operand".to_owned(),
            ));
        }
        let active = frame.active.take().expect("checked active format operand");
        frame.values[ordinal] = Some(evaluated);
        frame.next_operand = ordinal + 1;
        self.fiber.env.pop_scope();
        self.fiber.pending_ops = active.caller_pending_ops;
        self.fiber.cursor = active.resume;
        Ok(())
    }

    pub(super) fn recover_format_operand_attempt(
        &mut self,
        failure: &str,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let Some(index) = self.fiber.control_stack.iter().rposition(|entry| {
            matches!(&entry.kind, FlowControlStackEntryKind::FormatAttempt(frame) if frame.active.is_some())
        }) else {
            return false;
        };
        while self.fiber.control_stack.len() > index + 1 {
            let entry = self
                .fiber
                .control_stack
                .pop()
                .expect("bounded format unwind");
            match entry.kind {
                FlowControlStackEntryKind::Scope {
                    origin, cleanups, ..
                } => {
                    self.cancel_scheduled_scope_close(origin);
                    self.fiber.env.pop_scope();
                    self.emit_scope_cleanups(cleanups, output, pure_backend);
                }
                FlowControlStackEntryKind::FunctionCall(frame) if frame.function_scope => {
                    self.fiber.env.pop_scope();
                }
                FlowControlStackEntryKind::FunctionCall(_)
                | FlowControlStackEntryKind::Loop { .. }
                | FlowControlStackEntryKind::While { .. }
                | FlowControlStackEntryKind::WhileLet { .. } => {}
                FlowControlStackEntryKind::FormatAttempt(_) => {
                    unreachable!("innermost active format attempt owns this failure")
                }
            }
        }
        let FlowControlStackEntryKind::FormatAttempt(frame) =
            &mut self.fiber.control_stack[index].kind
        else {
            unreachable!("selected format frame is stable during unwind")
        };
        let active = frame
            .active
            .take()
            .expect("selected format operand is active");
        frame.values[active.ordinal] = None;
        frame.next_operand = active.ordinal + 1;
        if frame.first_recoverable.is_none() {
            frame.first_recoverable = Some(failure.to_owned());
        }
        self.fiber.env.pop_scope();
        self.fiber.pending_ops = active.caller_pending_ops;
        self.fiber.cursor = active.resume;
        self.fiber.status = FlowFiberStatus::Running;
        true
    }

    pub(in crate::engine) fn take_format_attempt_values(
        &mut self,
        attempt: RuntimeFormatAttemptId,
    ) -> Result<NativeFormatAttemptValues, RuntimeEvalError> {
        let manifest = self.plan.format_attempt(attempt).ok_or_else(|| {
            RuntimeEvalError::DialogueContentConstruction(format!(
                "unknown format attempt {attempt}"
            ))
        })?;
        let primary_type = manifest
            .operands()
            .iter()
            .find(|operand| operand.parameter() == RuntimeFmtParameterId::Value)
            .map(|operand| operand.ty())
            .ok_or_else(|| {
                RuntimeEvalError::DialogueContentConstruction(
                    "format attempt has no Value operand".to_owned(),
                )
            })?;
        let count = manifest.operands().len();
        let Some(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::FormatAttempt(frame),
        }) = self.fiber.control_stack.pop()
        else {
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "FormatContent has no completed attempt frame".to_owned(),
            ));
        };
        if frame.attempt != attempt || frame.active.is_some() || frame.next_operand != count {
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "FormatContent attempt is incomplete or mismatched".to_owned(),
            ));
        }
        Ok(NativeFormatAttemptValues {
            context: frame.context,
            primary_type,
            values: manifest
                .operands()
                .iter()
                .map(|operand| operand.parameter())
                .zip(frame.values)
                .collect(),
            first_recoverable: frame.first_recoverable,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::RuntimeDialogueContentTemplateDigest;
    use crate::pattern::{RuntimeCheckedType, RuntimePattern, RuntimePatternKind};
    use crate::plan::{
        RuntimeDialogueContentSlotSeed, RuntimeDialogueContentTemplateManifestSeed,
        RuntimeDialogueValueRole, RuntimeFormatAttemptDeclarationSeed,
        RuntimeFormatAttemptOperandSeed, RuntimePlanBuilder, RuntimePlanTypeProjection,
        RuntimePlanTypeSeed,
    };
    use crate::runtime_id::{RuntimeDialogueContentTemplateId, RuntimeDialogueValueSlotId};
    use crate::value::{
        RuntimeBinaryOp, RuntimeDialogueOpaqueRole, RuntimeExprKind, RuntimeSignedIntWidth,
    };

    fn format_attempt_engine() -> (Engine, RuntimeFormatAttemptId) {
        let integer =
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64).semantic_identity_digest();
        let string = RuntimeCheckedType::String.semantic_identity_digest();
        let content = RuntimeDialogueOpaqueRole::Content.semantic_identity();
        let content_owner = RuntimeDialogueOpaqueRole::Content.exact_owner();
        let template = RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap();
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(
                        integer,
                        RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
                    ),
                    RuntimePlanTypeSeed::new(string, RuntimePlanTypeProjection::String),
                    RuntimePlanTypeSeed::new(
                        content,
                        RuntimePlanTypeProjection::Opaque {
                            producer: content_owner.producer().clone(),
                            admission: content_owner.admission(),
                            value_class: content_owner.value_class(),
                            persistence: content_owner.persistence(),
                            arguments: Box::new([]),
                        },
                    ),
                ],
                [],
            )
            .expect("attempt types admit");
        builder
            .register_dialogue_content_template_seed(RuntimeDialogueContentTemplateManifestSeed {
                id: template,
                digest: RuntimeDialogueContentTemplateDigest::from_bytes([0x51; 32]),
                slots: Box::new([RuntimeDialogueContentSlotSeed {
                    slot: RuntimeDialogueValueSlotId::from_zero_based(0).unwrap(),
                    role: RuntimeDialogueValueRole::Formatted,
                    semantic_type: content,
                }]),
                effects: Box::new([]),
            })
            .expect("attempt template registers");
        builder
            .reserve_format_attempt_seed(RuntimeFormatAttemptDeclarationSeed {
                template,
                operands: Box::new([
                    RuntimeFormatAttemptOperandSeed::new(RuntimeFmtParameterId::Value, integer),
                    RuntimeFormatAttemptOperandSeed::new(RuntimeFmtParameterId::Style, string),
                ]),
            })
            .expect("typed attempt reserves");
        let plan = builder.finish().expect("attempt plan seals");
        (
            Engine::new(plan),
            RuntimeFormatAttemptId::from_zero_based(0).unwrap(),
        )
    }

    fn literal(ty: RuntimePlanTypeId, value: RuntimeValue) -> RuntimeExpr {
        RuntimeExpr::from_admitted_parts(ty, RuntimeExprKind::Value(value))
    }

    #[test]
    fn native_flow_attempt_catches_source_failure_then_evaluates_later_operand() {
        let (mut engine, attempt) = format_attempt_engine();
        let initial_locale = arcweft_id::LocaleTag::try_new("de-DE").unwrap();
        engine.set_format_context(RuntimeFormatContext::new(initial_locale.clone()));
        let integer = engine.plan.format_attempt(attempt).unwrap().operands()[0].ty();
        let string = engine.plan.format_attempt(attempt).unwrap().operands()[1].ty();
        let failed_source = RuntimeExpr::from_admitted_parts(
            integer,
            RuntimeExprKind::Binary {
                lhs: Box::new(literal(integer, RuntimeValue::i64(1))),
                op: RuntimeBinaryOp::Div,
                rhs: Box::new(literal(integer, RuntimeValue::i64(0))),
            },
        );
        engine.fiber.pending_ops = VecDeque::from(vec![
            FlowOp::FormatOperandAttempt {
                attempt,
                parameter: RuntimeFmtParameterId::Value,
                body: vec![FlowOp::Let {
                    pattern: RuntimePattern::from_admitted_parts(
                        integer,
                        RuntimePatternKind::Discard,
                    ),
                    expr: failed_source,
                }],
                value: literal(integer, RuntimeValue::i64(17)),
            },
            FlowOp::FormatOperandAttempt {
                attempt,
                parameter: RuntimeFmtParameterId::Style,
                body: Vec::new(),
                value: literal(string, RuntimeValue::String("number".to_owned())),
            },
        ]);
        let mut output = RuntimeStepOutput::default();
        let mut backend = crate::pure::VmRuntimePureCallBackend::default();
        let mut drop_policy = None;
        engine.step_flow(&mut output, &mut backend, &mut drop_policy);
        engine.set_format_context(RuntimeFormatContext::new(
            arcweft_id::LocaleTag::try_new("ja-JP").unwrap(),
        ));
        for _ in 0..3 {
            engine.step_flow(&mut output, &mut backend, &mut drop_policy);
        }
        assert!(engine.fiber.pending_ops.is_empty());
        assert_eq!(engine.fiber.status, FlowFiberStatus::Running);
        assert!(output.diagnostics.is_empty());
        let values = engine.take_format_attempt_values(attempt).unwrap();
        assert_eq!(values.context.active_locale(), &initial_locale);
        assert_eq!(
            values.first_recoverable.as_deref(),
            Some("division by zero")
        );
        assert_eq!(
            values.values,
            vec![
                (RuntimeFmtParameterId::Value, None),
                (
                    RuntimeFmtParameterId::Style,
                    Some(RuntimeValue::String("number".to_owned())),
                ),
            ]
        );
        assert!(engine.fiber.control_stack.is_empty());
    }

    #[test]
    fn native_flow_attempt_does_not_convert_fatal_source_error() {
        let (mut engine, attempt) = format_attempt_engine();
        let integer = engine.plan.format_attempt(attempt).unwrap().operands()[0].ty();
        let string = engine.plan.format_attempt(attempt).unwrap().operands()[1].ty();
        let invalid_source = RuntimeExpr::from_admitted_parts(
            integer,
            RuntimeExprKind::Binary {
                lhs: Box::new(literal(integer, RuntimeValue::i64(1))),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(literal(string, RuntimeValue::String("bad".to_owned()))),
            },
        );
        engine.fiber.pending_ops = VecDeque::from(vec![
            FlowOp::FormatOperandAttempt {
                attempt,
                parameter: RuntimeFmtParameterId::Value,
                body: vec![FlowOp::Let {
                    pattern: RuntimePattern::from_admitted_parts(
                        integer,
                        RuntimePatternKind::Discard,
                    ),
                    expr: invalid_source,
                }],
                value: literal(integer, RuntimeValue::i64(17)),
            },
            FlowOp::FormatOperandAttempt {
                attempt,
                parameter: RuntimeFmtParameterId::Style,
                body: Vec::new(),
                value: literal(string, RuntimeValue::String("number".to_owned())),
            },
        ]);
        let mut output = RuntimeStepOutput::default();
        let mut backend = crate::pure::VmRuntimePureCallBackend::default();
        let mut drop_policy = None;
        engine.step_flow(&mut output, &mut backend, &mut drop_policy);
        engine.step_flow(&mut output, &mut backend, &mut drop_policy);
        assert!(matches!(engine.fiber.status, FlowFiberStatus::Failed(_)));
        assert_eq!(output.diagnostics.len(), 1);
    }
}
