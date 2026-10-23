//! Native same-fiber continuation for source-ordered formatter operands.

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
        }) = self.fiber.frame.control_stack.last()
        else {
            return false;
        };
        let Some(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::FormatAttempt(mut frame),
        }) = self.fiber.frame.control_stack.pop()
        else {
            unreachable!("format attempt frame was checked before removal")
        };
        if let Some(active) = frame.active.take() {
            self.fiber.frame.env.pop_scope();
            self.fiber.frame.pending_ops = active.caller_pending_ops;
            self.fiber.frame.cursor = active.resume;
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
        source: crate::engine::code_cursor::NativeSourceOperation,
        next_op_index: Option<usize>,
    ) -> Result<(), RuntimeEvalError> {
        use crate::engine::NativePendingWork;
        use crate::engine::work::{NativeRuntimeContinuation, NativeSourceRangeRole};
        let plan = std::sync::Arc::clone(&self.plan);
        let Some(FlowOp::FormatOperandAttempt {
            attempt,
            parameter,
            value,
            ..
        }) = source.operation(&plan)
        else {
            return Err(RuntimeEvalError::NativeWorkAdmission {
                source: Box::new(crate::value::AwbcRuntimeValueSnapshotError::Message {
                    message: "formatter operand lost its original source declaration".into(),
                }),
            });
        };
        let attempt = *attempt;
        let parameter = *parameter;
        let manifest = plan.format_attempt(attempt).ok_or_else(|| {
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
        let body = self.original_body_range(&source, crate::plan::RuntimeFlowBodyRole::Body)?;
        let work = [
            Some(NativePendingWork::source(
                body,
                NativeSourceRangeRole::NestedBody,
            )),
            Some(NativePendingWork::resume(
                NativeRuntimeContinuation::CompleteFormatOperand { source },
            )),
        ];
        self.prepare_replacement_native_work(&work)?;
        if ordinal == 0 {
            let mut admission = self.native_work_admission()?;
            (|| {
                admission.validate_metadata_collection(manifest.operands().len())?;
                admission.validate_metadata(&self.format_context)
            })()
            .map_err(|source| RuntimeEvalError::NativeWorkAdmission {
                source: Box::new(source),
            })?;
            self.fiber.frame.control_stack.push(FlowControlStackEntry {
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
        let caller_pending_ops = std::mem::take(&mut self.fiber.frame.pending_ops);
        let Some(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::FormatAttempt(frame),
        }) = self.fiber.frame.control_stack.last_mut()
        else {
            self.fiber.frame.pending_ops = caller_pending_ops;
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "format operand has no current attempt frame".into(),
            ));
        };
        if frame.attempt != attempt || frame.next_operand != ordinal || frame.active.is_some() {
            self.fiber.frame.pending_ops = caller_pending_ops;
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "format operands are out of source order".into(),
            ));
        }
        frame.active = Some(NativeFormatOperandFrame {
            ordinal,
            resume: resume.clone(),
            caller_pending_ops,
        });
        self.fiber.frame.env.push_scope_with_capacity(0);
        self.fiber.frame.cursor = resume;
        self.fiber.frame.pending_ops = work.into_iter().flatten().collect();
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
        if !self
            .fiber
            .frame
            .env
            .value_matches_type(&self.plan, expected, &evaluated)?
        {
            return Err(RuntimeEvalError::InvalidExpressionType(expected));
        }
        let Some(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::FormatAttempt(frame),
        }) = self.fiber.frame.control_stack.last_mut()
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
        self.fiber.frame.env.pop_scope();
        self.fiber.frame.pending_ops = active.caller_pending_ops;
        self.fiber.frame.cursor = active.resume;
        Ok(())
    }

    pub(super) fn recover_format_operand_attempt(
        &mut self,
        failure: &str,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let Some(index) = self.fiber.frame.control_stack.iter().rposition(|entry| {
            matches!(&entry.kind, FlowControlStackEntryKind::FormatAttempt(frame) if frame.active.is_some())
        }) else {
            return false;
        };
        while self.fiber.frame.control_stack.len() > index + 1 {
            let entry = self
                .fiber
                .frame
                .control_stack
                .pop()
                .expect("bounded format unwind");
            match entry.kind {
                FlowControlStackEntryKind::Scope {
                    origin, cleanups, ..
                } => {
                    self.cancel_scheduled_scope_close(origin);
                    self.fiber.frame.env.pop_scope();
                    self.emit_scope_cleanups(cleanups, output, pure_backend);
                }
                FlowControlStackEntryKind::FunctionCall(frame) if frame.function_scope => {
                    self.fiber.frame.env.pop_scope();
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
            &mut self.fiber.frame.control_stack[index].kind
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
        self.fiber.frame.env.pop_scope();
        self.fiber.frame.pending_ops = active.caller_pending_ops;
        self.fiber.frame.cursor = active.resume;
        self.fiber.frame.status = FlowFiberStatus::Running;
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
        }) = self.fiber.frame.control_stack.pop()
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
    use crate::pattern::RuntimeCheckedType;
    use crate::plan::*;
    use crate::runtime_id::{RuntimeDialogueContentTemplateId, RuntimeDialogueValueSlotId};
    use crate::value::{
        RuntimeBinaryOp, RuntimeDialogueOpaqueRole, RuntimeLocalReadMode, RuntimeSignedIntWidth,
    };

    enum FormatFixture {
        Complete,
        Recoverable,
        Fatal,
    }

    fn format_attempt_engine(fixture: FormatFixture) -> (Engine, RuntimeFormatAttemptId) {
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
                    RuntimePlanTypeSeed::new(
                        RuntimeCheckedType::Unit.semantic_identity_digest(),
                        RuntimePlanTypeProjection::Unit,
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
        let attempt = builder
            .reserve_format_attempt_seed(RuntimeFormatAttemptDeclarationSeed {
                template,
                operands: Box::new([
                    RuntimeFormatAttemptOperandSeed::new(RuntimeFmtParameterId::Value, integer),
                    RuntimeFormatAttemptOperandSeed::new(RuntimeFmtParameterId::Style, string),
                ]),
            })
            .expect("typed attempt reserves");
        let (inputs, body): (
            Box<[RuntimeFunctionInputBindingSeed]>,
            Vec<RuntimeFlowOpSeed>,
        ) = match fixture {
            FormatFixture::Complete => (Box::new([]), Vec::new()),
            FormatFixture::Recoverable => (
                Box::new([]),
                vec![RuntimeFlowOpSeed::Let {
                    pattern: RuntimePatternSeed::new(integer, RuntimePatternSeedKind::Discard),
                    expr: RuntimeExprSeed::new(
                        integer,
                        RuntimeExprSeedKind::Binary {
                            lhs: Box::new(RuntimeExprSeed::new(
                                integer,
                                RuntimeExprSeedKind::Value(RuntimeValue::i64(1)),
                            )),
                            op: RuntimeBinaryOp::Div,
                            rhs: Box::new(RuntimeExprSeed::new(
                                integer,
                                RuntimeExprSeedKind::Value(RuntimeValue::i64(0)),
                            )),
                        },
                    ),
                }],
            ),
            FormatFixture::Fatal => {
                let locals = builder
                    .admit_type_batch(
                        [],
                        [
                            RuntimeLocalDeclarationSeed::new(
                                crate::engine::manual_local_source("formatter.input"),
                                integer,
                            ),
                            RuntimeLocalDeclarationSeed::new(
                                crate::engine::manual_local_source("formatter.binding"),
                                integer,
                            ),
                        ],
                    )
                    .unwrap();
                let input = locals.local_ids()[0].clone();
                let binding = locals.local_ids()[1].clone();
                let inputs = Box::new([RuntimeFunctionInputBindingSeed {
                    transfer: RuntimeFunctionInputTransfer::Formal,
                    origin: RuntimeFunctionInputOrigin::Parameter(
                        RuntimeFunctionParameterIdentity::from_accepted_identity([0xb1; 32]),
                    ),
                    ownership: Default::default(),
                    unrestricted_bindings: Box::new([]),
                    source: RuntimeFunctionInputSource::Parameter {
                        position: 0,
                        passing: RuntimeFunctionParameterPassing::Value,
                    },
                    input_local: input,
                    pattern: RuntimePatternSeed::new(
                        integer,
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: binding.clone(),
                        },
                    ),
                }]);
                // The original code is typed; this negative runtime fixture
                // deliberately supplies no value for its required input.
                (
                    inputs,
                    vec![RuntimeFlowOpSeed::Let {
                        pattern: RuntimePatternSeed::new(integer, RuntimePatternSeedKind::Discard),
                        expr: RuntimeExprSeed::new(
                            integer,
                            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                                binding,
                                RuntimeLocalReadMode::Move,
                            )),
                        ),
                    }],
                )
            }
        };
        let site = builder
            .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xb2; 32]),
                role: RuntimeFunctionSemanticRole::Ordinary,
                function_type: None,
                inputs,
                result: RuntimeCheckedType::Unit.semantic_identity_digest(),
                body_kind: RuntimeFunctionSiteBodyKind::Executable,
                effects: RuntimeEffectSet::empty(),
            })
            .unwrap();
        builder
            .define_function_site_seed(
                &site,
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: Box::new([
                        RuntimeFlowOpSeed::FormatOperandAttempt {
                            attempt: attempt.clone(),
                            parameter: RuntimeFmtParameterId::Value,
                            body,
                            value: RuntimeExprSeed::new(
                                integer,
                                RuntimeExprSeedKind::Value(RuntimeValue::i64(17)),
                            ),
                        },
                        RuntimeFlowOpSeed::FormatOperandAttempt {
                            attempt,
                            parameter: RuntimeFmtParameterId::Style,
                            body: Vec::new(),
                            value: RuntimeExprSeed::new(
                                string,
                                RuntimeExprSeedKind::Value(RuntimeValue::String("number".into())),
                            ),
                        },
                    ]),
                }),
            )
            .unwrap();
        let plan = builder.finish().expect("attempt plan seals");
        let mut engine = Engine::new(plan);
        let position = crate::engine::NativeCodeCursor::root(
            &engine.plan,
            crate::plan::code::RuntimeCodeOwner::Function(site.id()),
        )
        .unwrap();
        engine
            .fiber
            .frame
            .pending_ops
            .push_back(crate::engine::work::NativePendingWork::source(
                position,
                crate::engine::work::NativeSourceRangeRole::TaskRoot,
            ));
        (engine, RuntimeFormatAttemptId::from_zero_based(0).unwrap())
    }

    fn completed_format_engine() -> (Engine, RuntimeFormatAttemptId) {
        let (mut engine, attempt) = format_attempt_engine(FormatFixture::Complete);
        let mut output = RuntimeStepOutput::default();
        let mut backend = crate::pure::VmRuntimePureCallBackend::default();
        for _ in 0..4 {
            engine.step_flow(&mut output, &mut backend, &mut None);
        }
        assert!(output.diagnostics.is_empty());
        assert!(engine.fiber.frame.pending_ops.is_empty());
        (engine, attempt)
    }

    #[test]
    fn native_formatter_preflight_rejects_corrupt_abi_frontier_presence_and_types() {
        let (mut engine, _) = completed_format_engine();
        let owner = crate::task::RuntimeProgramOwner::Plan(engine.program_plan());
        let image = engine.inert_rollback_image().unwrap();
        for case in 0..5 {
            let mut corrupt = image.clone();
            let Some(crate::engine::FlowControlStackEntryRollbackImage::FormatAttempt(frame)) =
                corrupt.fiber.frame.control_stack.last_mut()
            else {
                panic!("completed original attempt remains owned")
            };
            match case {
                0 => {
                    frame.values.pop();
                }
                1 => {
                    frame.next_operand += 1;
                }
                2 => {
                    frame.values[0] = None;
                }
                3 => {
                    frame.next_operand = 1;
                }
                4 => {
                    frame.values[0] = Some(
                        crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                            &RuntimeValue::Unit,
                            &owner,
                        )
                        .unwrap(),
                    );
                }
                _ => unreachable!(),
            }
            assert!(
                Engine::from_rollback_image(corrupt, &owner).is_err(),
                "case {case}"
            );
        }
        let Some(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::FormatAttempt(frame),
        }) = engine.fiber.frame.control_stack.last_mut()
        else {
            unreachable!()
        };
        let original = frame.values[0].replace(RuntimeValue::Unit).unwrap();
        assert!(engine.inert_rollback_image().is_err());
        let Some(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::FormatAttempt(frame),
        }) = engine.fiber.frame.control_stack.last_mut()
        else {
            unreachable!()
        };
        frame.values[0] = Some(original);
        assert_eq!(engine.inert_rollback_image().unwrap(), image);
        let restored = Engine::from_rollback_image(image.clone(), &owner).unwrap();
        assert_eq!(restored.inert_rollback_image().unwrap(), image);
    }

    #[test]
    fn native_formatter_values_share_live_inert_and_parent_node_limits() {
        let (engine, _) = completed_format_engine();
        let image = engine.inert_rollback_image().unwrap();
        let owner = crate::task::RuntimeProgramOwner::Plan(engine.program_plan());
        let limits = crate::entry::RuntimeSchemaLimits {
            max_nodes: 2,
            ..crate::entry::RuntimeSchemaLimits::engine_default()
        };
        let mut exact =
            crate::value::RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        engine.fiber.frame.admit_owned_values(&mut exact).unwrap();
        let mut exact =
            crate::value::RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        image.fiber.frame.admit_owned_values(&mut exact).unwrap();
        for inert in [false, true] {
            let mut shared =
                crate::value::RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
            shared.validate(&RuntimeValue::Bool(true)).unwrap();
            let error = if inert {
                image.fiber.frame.admit_owned_values(&mut shared)
            } else {
                engine.fiber.frame.admit_owned_values(&mut shared)
            }
            .unwrap_err();
            assert!(matches!(
                error,
                crate::value::AwbcRuntimeValueSnapshotError::Resource(
                    crate::entry::RuntimeSchemaError::BudgetExceeded { budget: "nodes" }
                )
            ));
            assert_eq!(
                engine
                    .fiber
                    .frame
                    .admit_owned_values(&mut shared)
                    .unwrap_err(),
                error
            );
        }
        assert_eq!(engine.inert_rollback_image().unwrap(), image);
    }

    #[test]
    fn native_flow_attempt_catches_source_failure_then_evaluates_later_operand() {
        let (mut engine, attempt) = format_attempt_engine(FormatFixture::Recoverable);
        let initial_locale = arcweft_id::LocaleTag::try_new("de-DE").unwrap();
        engine.set_format_context(RuntimeFormatContext::new(initial_locale.clone()));
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
        assert!(engine.fiber.frame.pending_ops.is_empty());
        assert_eq!(engine.fiber.frame.status, FlowFiberStatus::Running);
        assert!(output.diagnostics.is_empty());
        let owner = crate::task::RuntimeProgramOwner::Plan(engine.program_plan());
        let image = engine.inert_rollback_image().unwrap();
        let restored = Engine::from_rollback_image(image.clone(), &owner).unwrap();
        assert_eq!(restored.inert_rollback_image().unwrap(), image);
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
        assert!(engine.fiber.frame.control_stack.is_empty());
    }

    #[test]
    fn native_flow_attempt_does_not_convert_fatal_source_error() {
        let (mut engine, _) = format_attempt_engine(FormatFixture::Fatal);
        let mut output = RuntimeStepOutput::default();
        let mut backend = crate::pure::VmRuntimePureCallBackend::default();
        let mut drop_policy = None;
        engine.step_flow(&mut output, &mut backend, &mut drop_policy);
        engine.step_flow(&mut output, &mut backend, &mut drop_policy);
        assert!(matches!(
            engine.fiber.frame.status,
            FlowFiberStatus::Failed(_)
        ));
        assert_eq!(output.diagnostics.len(), 1);
    }
}
