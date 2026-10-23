mod callable;
mod format_attempt;
mod function_call;
pub(super) mod match_guard;
mod resume;

use super::NativePendingWork;
use super::code_cursor::NativeSourceOperation;
use super::dialogue::{DialogueActivationFrame, DialogueLineTaskState};
use super::work::{NativePendingWorkKind, NativeRuntimeContinuation, NativeSourceRangeRole};
use super::{
    ChoiceState, Engine, FlowControlStackEntry, FlowControlStackEntryKind, FlowEvent,
    FlowFiberStatus, FlowOp, FlowScopeCleanup, HostCallState, NativeCodeCursor, RuntimeDiagnostic,
    RuntimeEvalError, RuntimeExpr, RuntimeIterator, RuntimePattern, RuntimeStepOutput,
    RuntimeValue, runtime_value_label,
};
use crate::effect::LineEffectRequest;
use crate::pattern::{RuntimeBuiltinVariantCaseIdentity, pattern_binding_capacity};
use crate::plan::{
    RuntimeIteratorEvidence, RuntimeIteratorWitnessExecutable,
    RuntimeProjectCallOrdinaryMaterialization, RuntimeReceiverMode,
};
use crate::pure::RuntimeCallBackend;
use crate::scope::RuntimeScopeIdentity;
use crate::step::{RuntimeHostCallId, RuntimeHostCallRequest};
use crate::task::{HostTaskRequest, NamedHostArg, RuntimeHostArgumentTemplate};
use crate::time::LogicalDuration;
use crate::value::{RuntimeEnv, RuntimeLocalBinding};
use std::borrow::Cow;
use std::sync::Arc;

impl Engine {
    pub(in crate::engine) fn native_work_admission(
        &self,
    ) -> Result<crate::value::RuntimeRetainedValueAdmission, RuntimeEvalError> {
        let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&self.plan));
        self.fiber
            .frame
            .ensure_program_owner(&owner)
            .map_err(|message| RuntimeEvalError::NativeWorkAdmission {
                source: Box::new(crate::value::AwbcRuntimeValueSnapshotError::Message { message }),
            })?;
        Ok(crate::value::RuntimeRetainedValueAdmission::for_program(
            owner,
            crate::entry::RuntimeSchemaLimits::engine_default(),
        ))
    }

    pub(in crate::engine) fn prepare_native_work<const N: usize>(
        &self,
        work: &[Option<NativePendingWork>; N],
    ) -> Result<(), RuntimeEvalError> {
        self.prepare_native_work_with_prefix(self.fiber.frame.pending_ops.iter(), work)
    }

    fn prepare_replacement_native_work<const N: usize>(
        &self,
        work: &[Option<NativePendingWork>; N],
    ) -> Result<(), RuntimeEvalError> {
        self.prepare_native_work_with_prefix(std::iter::empty(), work)
    }

    fn prepare_native_work_with_prefix<'a, const N: usize>(
        &self,
        current: impl ExactSizeIterator<Item = &'a NativePendingWork>,
        work: &'a [Option<NativePendingWork>; N],
    ) -> Result<(), RuntimeEvalError> {
        let mut admission = self.native_work_admission()?;
        let invalid_count = || RuntimeEvalError::NativeWorkAdmission {
            source: Box::new(crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: "native pending work count exceeds its metadata domain".into(),
            }),
        };
        let physical = current
            .len()
            .checked_add(work.iter().flatten().count())
            .ok_or_else(invalid_count)?;
        let logical = current
            .chain(work.iter().flatten())
            .try_fold(0_usize, |total, item| {
                total.checked_add(item.logical_pending_count()?)
            })
            .ok_or_else(invalid_count)?;
        let admitted = (|| {
            admission.validate_metadata_collection(physical)?;
            admission.validate_metadata_collection(logical)?;
            for item in work.iter().flatten() {
                item.admit_metadata(
                    &mut admission,
                    self.fiber.frame.env.function_instantiation(),
                )?;
            }
            Ok::<_, crate::value::AwbcRuntimeValueSnapshotError>(())
        })();
        admitted.map_err(|source| RuntimeEvalError::NativeWorkAdmission {
            source: Box::new(source),
        })
    }

    pub(in crate::engine) fn prepend_admitted_native_work<const N: usize>(
        &mut self,
        work: [Option<NativePendingWork>; N],
    ) {
        self.fiber
            .frame
            .pending_ops
            .reserve(work.iter().flatten().count());
        for item in work.into_iter().rev().flatten() {
            self.fiber.frame.pending_ops.push_front(item);
        }
    }

    fn prepare_native_scope(
        &self,
        kind: crate::scope::RuntimeScopeFrameKind,
    ) -> Result<(crate::scope::RuntimeScheduledScopeToken, u64), RuntimeEvalError> {
        self.native_work_admission()?;
        let ordinal = std::num::NonZeroU64::new(self.next_scheduled_scope_sequence)
            .ok_or(crate::scope::RuntimeScopeExitError::IdentityCapacityExhausted)?;
        let next = self
            .next_scheduled_scope_sequence
            .checked_add(1)
            .ok_or(crate::scope::RuntimeScopeExitError::IdentityCapacityExhausted)?;
        Ok((
            crate::scope::RuntimeScheduledScopeToken::from_runtime(
                self.fiber.execution,
                self.fiber.persistent_id,
                ordinal,
                kind,
            ),
            next,
        ))
    }

    fn push_scoped_source(
        &mut self,
        body: NativeCodeCursor,
        bindings: Vec<RuntimeLocalBinding>,
        identity: RuntimeScopeIdentity,
        kind: crate::scope::RuntimeScopeFrameKind,
        tail: Option<NativeRuntimeContinuation>,
    ) -> Result<(), RuntimeEvalError> {
        if body.remaining_operation_count() == Some(0) && bindings.is_empty() && tail.is_none() {
            return Ok(());
        }
        let (token, next) = self.prepare_native_scope(kind)?;
        let work = [
            Some(NativePendingWork::resume(
                NativeRuntimeContinuation::EnterScheduledScope { identity, token },
            )),
            (!bindings.is_empty())
                .then(|| NativePendingWork::resume(NativeRuntimeContinuation::Bind(bindings))),
            Some(NativePendingWork::source(
                body,
                NativeSourceRangeRole::NestedBody,
            )),
            Some(NativePendingWork::resume(
                NativeRuntimeContinuation::ExitScheduledScope { token },
            )),
            tail.map(NativePendingWork::resume),
        ];
        self.prepare_native_work(&work)?;
        self.prepend_admitted_native_work(work);
        self.next_scheduled_scope_sequence = next;
        Ok(())
    }

    fn push_source_range(&mut self, body: NativeCodeCursor) -> Result<(), RuntimeEvalError> {
        if body.remaining_operation_count() == Some(0) {
            return Ok(());
        }
        let work = [Some(NativePendingWork::source(
            body,
            NativeSourceRangeRole::NestedBody,
        ))];
        self.prepare_native_work(&work)?;
        self.prepend_admitted_native_work(work);
        Ok(())
    }

    pub(in crate::engine) fn push_scoped_source_body(
        &mut self,
        source: &NativeSourceOperation,
        role: crate::plan::RuntimeFlowBodyRole,
        bindings: Vec<RuntimeLocalBinding>,
    ) -> Result<(), RuntimeEvalError> {
        let body = self.original_body_range(source, role)?;
        self.push_scoped_source(
            body,
            bindings,
            RuntimeScopeIdentity::Anonymous,
            crate::scope::RuntimeScopeFrameKind::Control,
            None,
        )
    }

    pub(in crate::engine) fn original_body_range(
        &self,
        source: &super::code_cursor::NativeSourceOperation,
        role: crate::plan::RuntimeFlowBodyRole,
    ) -> Result<NativeCodeCursor, RuntimeEvalError> {
        let mut admission = self.native_work_admission()?;
        source.child_body(role, &mut admission).map_err(|source| {
            RuntimeEvalError::NativeWorkAdmission {
                source: Box::new(source),
            }
        })
    }

    /// Prepares an original task root before its capture packet is moved.
    /// Task lexical boundaries are runtime continuations, never invented AST.
    pub(in crate::engine) fn prepare_task_root_work(
        &self,
        position: NativeCodeCursor,
        scope_empty: bool,
    ) -> Result<std::collections::VecDeque<NativePendingWork>, RuntimeEvalError> {
        let mut admission = self.native_work_admission()?;
        let work = NativePendingWork::source(position, NativeSourceRangeRole::TaskRoot);
        let logical =
            work.logical_pending_count()
                .ok_or_else(|| RuntimeEvalError::NativeWorkAdmission {
                    source: Box::new(crate::value::AwbcRuntimeValueSnapshotError::Message {
                        message: "native task root has an invalid operation range".into(),
                    }),
                })?;
        let scoped = scope_empty || logical != 0;
        let boundaries = if scoped { 2 } else { 0 };
        let physical = usize::from(logical != 0) + boundaries;
        let count = logical.checked_add(boundaries).ok_or_else(|| {
            RuntimeEvalError::NativeWorkAdmission {
                source: Box::new(crate::value::AwbcRuntimeValueSnapshotError::Message {
                    message: "native task root operation count overflows".into(),
                }),
            }
        })?;
        let admit = (|| {
            admission.validate_metadata_collection(count)?;
            admission.validate_metadata_collection(physical)?;
            work.admit_metadata(&mut admission, None)?;
            if scoped {
                for boundary in [
                    NativeRuntimeContinuation::EnterTaskScope,
                    NativeRuntimeContinuation::ExitTaskScope,
                ] {
                    let boundary: NativePendingWork = NativePendingWork::resume(boundary);
                    boundary.admit_metadata(&mut admission, None)?;
                }
            }
            Ok::<_, crate::value::AwbcRuntimeValueSnapshotError>(())
        })();
        admit.map_err(|source| RuntimeEvalError::NativeWorkAdmission {
            source: Box::new(source),
        })?;
        let mut queue = std::collections::VecDeque::with_capacity(physical);
        if scoped {
            queue.push_back(NativePendingWork::resume(
                NativeRuntimeContinuation::EnterTaskScope,
            ));
        }
        if logical != 0 {
            queue.push_back(work);
        }
        if scoped {
            queue.push_back(NativePendingWork::resume(
                NativeRuntimeContinuation::ExitTaskScope,
            ));
        }
        Ok(queue)
    }

    /// Admits the complete replacement root before function inputs are moved.
    /// The queue owns both executable roots and their completion transitions.
    pub(in crate::engine) fn prepare_function_root_work(
        &self,
        site: crate::runtime_id::RuntimeFunctionSiteId,
    ) -> Result<std::collections::VecDeque<NativePendingWork>, RuntimeEvalError> {
        self.native_work_admission()?;
        let position = NativeCodeCursor::root(
            &self.plan,
            crate::plan::code::RuntimeCodeOwner::Function(site),
        )
        .ok_or(crate::value::RuntimeFunctionApplyError::UnknownStructuredSite { site })?;
        let work = [Some(NativePendingWork::source(
            position,
            NativeSourceRangeRole::FunctionRoot,
        ))];
        self.prepare_replacement_native_work(&work)?;
        Ok(work.into_iter().flatten().collect())
    }

    pub(super) fn step_flow(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
        drop_policy: &mut Option<crate::effect::RuntimeDropPolicy>,
    ) {
        let plan = Arc::clone(&self.plan);
        // Empty body ranges are structural transitions, not executable polls.
        self.fiber.frame.normalize_pending_source_ranges(&plan);
        if self
            .fiber
            .frame
            .pending_ops
            .front()
            .is_some_and(|work| matches!(work.kind, NativePendingWorkKind::Resume(_)))
        {
            let work = self
                .fiber
                .frame
                .pending_ops
                .pop_front()
                .expect("selected native resume");
            let NativePendingWorkKind::Resume(resume) = work.kind else {
                unreachable!("the selected work is a runtime continuation")
            };
            self.execute_native_resume(resume, output, pure_backend);
            return;
        }
        let (op, source_site, next_op_index) =
            if let Some(work) = self.fiber.frame.pending_ops.front_mut() {
                let NativePendingWorkKind::Source { position, role } = &mut work.kind else {
                    unreachable!("runtime continuations were dispatched before source lookup")
                };
                if let Some(expression) = position.expression(&plan) {
                    if *role != NativeSourceRangeRole::FunctionRoot {
                        self.fiber.frame.pending_ops.pop_front();
                    }
                    match self.evaluate_expr_with_backend(expression, pure_backend) {
                        Ok(value) => {
                            let label = runtime_value_label(&value);
                            if !self.return_function_call_value(value, output, pure_backend) {
                                self.return_value(label, output, pure_backend);
                            }
                        }
                        Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                    }
                    return;
                }
                let Some(operations) = position.operations(&plan) else {
                    self.fail_eval("native queued source lost its original body", output);
                    return;
                };
                let Some(op) = operations.first() else {
                    if self.fiber.frame.active_function().is_some() {
                        self.complete_function_call_fallthrough(output, pure_backend);
                    } else {
                        self.finish(output, pure_backend);
                    }
                    return;
                };
                let source = position
                    .source_operation(&plan)
                    .expect("selected original operation");
                let next = position
                    .operation_index()
                    .expect("original operation ordinal")
                    + 1;
                let advance = if *role == NativeSourceRangeRole::FunctionRoot {
                    Some(next)
                } else {
                    assert!(
                        position.set_operation(next),
                        "queued source stays in its original body"
                    );
                    None
                };
                // The selected source operation has been consumed before it
                // can suspend or save its caller. Only function-root EOF is
                // an executable completion transition.
                self.fiber.frame.normalize_pending_source_ranges(&plan);
                (Cow::Borrowed(op), source, advance)
            } else {
                if self.fiber.frame.active_function().is_some() {
                    self.fail_eval("active function lost its queued original root", output);
                }
                return;
            };
        self.execute_flow_operation(
            op,
            source_site,
            next_op_index,
            output,
            pure_backend,
            drop_policy,
        );
    }

    // Authored dispatch borrows original plan rows. Runtime state is consumed
    // separately by the closed native continuation dispatcher.
    #[allow(clippy::too_many_lines)]
    pub(super) fn execute_flow_operation(
        &mut self,
        op: Cow<'_, FlowOp>,
        source_site: NativeSourceOperation,
        next_op_index: Option<usize>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
        drop_policy: &mut Option<crate::effect::RuntimeDropPolicy>,
    ) {
        match op.as_ref() {
            FlowOp::Let { pattern, expr } => {
                self.evaluate_let_with_backend(pattern, expr, output, pure_backend);
                self.advance_if_needed(next_op_index);
            }
            FlowOp::StandardMap(control) => {
                let admitted = self
                    .fiber
                    .frame
                    .env
                    .get(control.mapping())
                    .ok_or(RuntimeEvalError::UnknownLocal(control.mapping()))
                    .and_then(|value| control.admit_mapping(value));
                if let Err(error) = admitted {
                    self.fail_format_aware_eval(error, output, pure_backend);
                    return;
                }
                self.advance_if_needed(next_op_index);
                if let Err(error) = self
                    .original_body_range(&source_site, crate::plan::RuntimeFlowBodyRole::Body)
                    .and_then(|body| self.push_source_range(body))
                {
                    self.fail_format_aware_eval(error, output, pure_backend);
                }
            }
            FlowOp::FormatOperandAttempt { .. } => {
                if let Err(error) = self.start_format_operand_attempt(source_site, next_op_index) {
                    self.fail_format_aware_eval(error, output, pure_backend);
                }
            }
            FlowOp::LetElse { pattern, expr, .. } => {
                match self
                    .evaluate_expr_with_backend(expr, pure_backend)
                    .and_then(|value| self.try_bind_pattern_owned(pattern, value))
                {
                    Ok(None) => self.advance_if_needed(next_op_index),
                    Ok(Some(value)) => {
                        self.advance_if_needed(next_op_index);
                        if let Err(error) = self
                            .original_body_range(
                                &source_site,
                                crate::plan::RuntimeFlowBodyRole::Else,
                            )
                            .and_then(|body| self.push_source_range(body))
                        {
                            self.fail_format_aware_eval(error, output, pure_backend);
                            return;
                        }
                        output.diagnostics.push(RuntimeDiagnostic::new(format!(
                            "let-else pattern did not match {}",
                            runtime_value_label(&value)
                        )));
                    }
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::Assign { place, value } => {
                match self.evaluate_expr_with_backend(value, pure_backend) {
                    Ok(value) => match self.fiber.frame.env.assign_place(place, value) {
                        Ok(_) => self.advance_if_needed(next_op_index),
                        Err(error) => self.fail_eval(error.into_parts().0, output),
                    },
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::LineOperation { .. } | FlowOp::CommitDialogueResult { .. } => {
                self.fiber.frame.status = FlowFiberStatus::Failed(
                    "line operation escaped its dialogue activation authority".to_owned(),
                );
            }
            FlowOp::SelectDialogueResult { value } => {
                if !matches!(
                    &self.fiber.owner,
                    super::FlowFiberOwner::LineTask(owner)
                        if matches!(
                            owner.tag.work(),
                            crate::line_task::LineTaskWork::Cancellation(_)
                                | crate::line_task::LineTaskWork::Node(_)
                        )
                ) || self.fiber.frame.selected_dialogue_result.is_some()
                {
                    self.fail_eval(
                        crate::line_task::LineRuntimeError::InvalidActivationOperation,
                        output,
                    );
                    return;
                }
                match self.evaluate_expr_with_backend(value, pure_backend) {
                    Ok(selected) => {
                        self.fiber.frame.selected_dialogue_result = Some(selected);
                        *drop_policy = Some(crate::effect::RuntimeDropPolicy::Default);
                        self.fiber.frame.pending_ops.clear();
                        self.unwind_control_stack(output, pure_backend);
                        self.fiber.frame.status = FlowFiberStatus::Done(super::FlowExit::Done);
                    }
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::Dialogue { .. } => {
                let FlowOp::Dialogue {
                    target,
                    content,
                    result,
                } = op.into_owned()
                else {
                    unreachable!("selected dialogue activation");
                };
                let target_value = match self.evaluate_expr_with_backend(&target, pure_backend) {
                    Ok(value) => value,
                    Err(error) => {
                        self.fail_format_aware_eval(error, output, pure_backend);
                        return;
                    }
                };
                if let Err(error) = self.plan.accepts_value(
                    target.ty(),
                    &target_value,
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                ) {
                    self.fail_eval(
                        crate::value::RuntimeEvalError::CharacterDialogueConstruction(
                            error.to_string(),
                        ),
                        output,
                    );
                    return;
                }
                let crate::value::RuntimeValue::Opaque(target_value) = target_value else {
                    self.fail_eval(
                        crate::value::RuntimeEvalError::CharacterDialogueConstruction(
                            "dialogue target is not an opaque CharacterDialogue value".to_owned(),
                        ),
                        output,
                    );
                    return;
                };
                let plan = Arc::clone(&self.plan);
                let Some(content_plan) = plan.dialogue_content().get(content) else {
                    self.fiber.frame.status =
                        FlowFiberStatus::Failed(format!("missing dialogue content plan {content}"));
                    return;
                };
                let mut values = Vec::with_capacity(content_plan.values().len());
                for site in content_plan.values() {
                    match self.evaluate_dialogue_site(site, pure_backend) {
                        Ok(value) => values.push(crate::plan::RuntimeDialogueValueBinding {
                            slot: site.slot(),
                            role: site.role(),
                            value,
                        }),
                        Err(error) => {
                            self.fail_eval(error, output);
                            return;
                        }
                    }
                }
                let mut effect_callbacks = Vec::with_capacity(content_plan.effect_sites().len());
                for site in content_plan.effect_sites() {
                    let callback = match self.evaluate_callable_expr(
                        site.state(),
                        site.captures(),
                        pure_backend,
                    ) {
                        Ok(RuntimeValue::Callable(callback)) => callback,
                        Err(error) => {
                            self.fail_eval(error, output);
                            return;
                        }
                        Ok(_) => unreachable!(
                            "structured function-site construction returned a non-function"
                        ),
                    };
                    let remaining = match callback.remaining_arity() {
                        Ok(remaining) => remaining,
                        Err(error) => {
                            self.fail_eval(error, output);
                            return;
                        }
                    };
                    if remaining != 0 || !callback.is_structured_executable_callback() {
                        self.fiber.frame.status = FlowFiberStatus::Failed(
                            "dialogue effect callback is not an executable zero-argument function"
                                .to_owned(),
                        );
                        return;
                    }
                    effect_callbacks.push(crate::value::RuntimeDialogueContentEffectBinding::new(
                        site.site(),
                        callback,
                    ));
                }
                let line = content_plan.line().clone();
                let Some(task_group) = content_plan.line_task_group() else {
                    self.fiber.frame.status = FlowFiberStatus::Failed(
                        crate::line_task::LineRuntimeError::MissingTaskGroup.to_string(),
                    );
                    return;
                };
                let Some(group) = plan.line_task_groups().get(task_group.index()) else {
                    self.fiber.frame.status = FlowFiberStatus::Failed(
                        crate::line_task::LineRuntimeError::UnknownTaskGroup.to_string(),
                    );
                    return;
                };
                if group.result_type() != result.ty() {
                    self.fiber.frame.status = FlowFiberStatus::Failed(
                        crate::line_task::LineRuntimeError::DialogueResultTypeMismatch.to_string(),
                    );
                    return;
                }
                let captures = match self.capture_line_task_locals(group) {
                    Ok(captures) => captures,
                    Err(error) => {
                        self.fiber.frame.status = FlowFiberStatus::Failed(error.to_string());
                        return;
                    }
                };
                let activation = match self.allocate_dialogue_activation(content) {
                    Ok(activation) => activation,
                    Err(message) => {
                        self.fiber.frame.status = FlowFiberStatus::Failed(message.to_string());
                        return;
                    }
                };
                self.advance_if_needed(next_op_index);
                let mut locals = RuntimeEnv::default();
                let capture_locals = captures
                    .iter()
                    .map(|capture| capture.local)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                locals.bind_all(captures.into_vec());
                let dialogue = DialogueActivationFrame {
                    parent: self.fiber.id,
                    line,
                    content,
                    target: Some(target_value),
                    task_group,
                    captures: capture_locals,
                    task_inputs: Box::new([]),
                    execution: super::NativeFlowExecutionFrame::running(
                        self.fiber.frame.program.clone(),
                        locals,
                    ),
                    line_task: DialogueLineTaskState::NotStarted,
                    elapsed: LogicalDuration::default(),
                    phase: super::DialogueRuntimePhase::Activating,
                    result_target: result,
                    voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
                    values: values.into_boxed_slice(),
                    effect_callbacks: effect_callbacks.into_boxed_slice(),
                    activation_pc: 0,
                    exiting_for_result: false,
                    scopes: Vec::new(),
                    pending_line_operation: None,
                    pending_host_call: None,
                    failure: None,
                };
                if let Err(error) = self
                    .dialogue_activations
                    .begin(activation.clone(), dialogue)
                {
                    self.fiber.frame.status = FlowFiberStatus::Failed(error.to_string());
                    return;
                }
                self.fiber.frame.status =
                    FlowFiberStatus::Dialogue(super::DialogueExecutionStatus::new(
                        activation,
                        super::DialogueRuntimePhase::Activating,
                    ));
            }
            FlowOp::Choice { .. } => {
                let FlowOp::Choice { id, options } = op.into_owned() else {
                    unreachable!("selected choice");
                };
                output.flow_events.push(FlowEvent::ChoicePresented {
                    id: id.clone(),
                    options: options.clone(),
                });
                self.advance_if_needed(next_op_index);
                self.fiber.frame.status = FlowFiberStatus::Choice(ChoiceState { id, options });
            }
            FlowOp::Await { .. } => {
                self.start_need_await(source_site, next_op_index, output, pure_backend);
            }
            FlowOp::StartNeedProducer { .. } => {
                let FlowOp::StartNeedProducer { binding, target } = op.into_owned() else {
                    unreachable!("selected producer start");
                };
                self.start_need_producer(binding, target, next_op_index, output, pure_backend);
            }
            FlowOp::AwaitMany { .. } => {
                let FlowOp::AwaitMany {
                    binding,
                    target,
                    pending,
                } = op.into_owned()
                else {
                    unreachable!("selected aggregate await");
                };
                self.emit_line_effects(pending, output, pure_backend);
                self.start_await_many_state(binding, target, next_op_index, output, pure_backend);
            }
            FlowOp::HostCall { binding, target } => {
                let arguments = match self.evaluate_host_call_arguments(&target.args, pure_backend)
                {
                    Ok(arguments) => arguments,
                    Err(error) => {
                        self.fiber.frame.status = FlowFiberStatus::Failed(error.clone());
                        output.diagnostics.push(RuntimeDiagnostic::new(error));
                        return;
                    }
                };
                let (args, named_args) = arguments;
                let Some(result) = self
                    .plan
                    .type_table()
                    .get(target.result)
                    .map(|declaration| declaration.semantic_identity())
                else {
                    let error = format!(
                        "host call {} result type is absent from the selected plan",
                        target.public_id
                    );
                    self.fiber.frame.status = FlowFiberStatus::Failed(error.clone());
                    output.diagnostics.push(RuntimeDiagnostic::new(error));
                    return;
                };
                let id = self.next_host_call_id(&target.public_id);
                let request = crate::task::HostTaskRequest::Custom {
                    capability: crate::task::HostCapabilityId(target.capability.clone()),
                    operation: target.operation.clone(),
                    manifest_contract: target.contract,
                    args,
                    named_args,
                };
                let request = match target.producer.sealed_definition().and_then(|producer| {
                    RuntimeHostCallRequest::admit(
                        id.clone(),
                        producer,
                        self.generation,
                        request,
                        result,
                        target.mode,
                        target.deterministic,
                        &mut self.need_producers,
                    )
                }) {
                    Ok(request) => request,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                };
                output.requests.host_calls.push(request);
                self.advance_if_needed(next_op_index);
                self.fiber.frame.status = FlowFiberStatus::HostCall(HostCallState {
                    binding: binding.clone(),
                    id,
                    result_type: super::HostCallResultType::Plan(target.result),
                });
            }
            FlowOp::ProjectCall { site } => {
                self.start_project_call(*site, next_op_index, output, pure_backend);
            }
            FlowOp::ApplyGroup {
                callee,
                args,
                result,
            } => {
                let application = (|| {
                    let callee = self.evaluate_expr_with_backend(callee, pure_backend)?;
                    let args = self.evaluate_function_call_args(args, pure_backend)?;
                    self.start_function_value_call(
                        callee,
                        args,
                        result.clone(),
                        next_op_index,
                        output,
                        pure_backend,
                    )
                })();
                if let Err(error) = application {
                    self.fail_format_aware_eval(error, output, pure_backend);
                }
            }
            FlowOp::If { condition, .. } => {
                match self.evaluate_bool_with_backend(condition, pure_backend) {
                    Ok(selected) => {
                        self.advance_if_needed(next_op_index);
                        let role = if selected {
                            crate::plan::RuntimeFlowBodyRole::Then
                        } else {
                            crate::plan::RuntimeFlowBodyRole::Else
                        };
                        if let Err(error) =
                            self.push_scoped_source_body(&source_site, role, Vec::new())
                        {
                            self.fail_format_aware_eval(error, output, pure_backend);
                        }
                    }
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::IfLet {
                pattern,
                expr,
                guard,
                ..
            } => {
                match self.evaluate_if_let_with_backend(pattern, expr, guard.as_ref(), pure_backend)
                {
                    Ok(bindings) => {
                        self.advance_if_needed(next_op_index);
                        let role = if bindings.is_some() {
                            crate::plan::RuntimeFlowBodyRole::Then
                        } else {
                            crate::plan::RuntimeFlowBodyRole::Else
                        };
                        if let Err(error) = self.push_scoped_source_body(
                            &source_site,
                            role,
                            bindings.unwrap_or_default(),
                        ) {
                            self.fail_format_aware_eval(error, output, pure_backend);
                        }
                    }
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::Match { scrutinee, .. } => {
                match self.evaluate_expr_with_backend(scrutinee, pure_backend) {
                    Ok(value) => {
                        self.advance_if_needed(next_op_index);
                        if let Err(error) = self.select_match_candidate(value, source_site) {
                            self.fail_format_aware_eval(error, output, pure_backend);
                        }
                    }
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::Loop { .. } => {
                self.advance_if_needed(next_op_index);
                self.fiber.frame.control_stack.push(FlowControlStackEntry {
                    kind: FlowControlStackEntryKind::Loop {
                        source: source_site.clone(),
                    },
                });
                if let Err(error) = self.push_loop_iteration(&source_site) {
                    self.fail_format_aware_eval(error, output, pure_backend);
                }
            }
            FlowOp::While { condition, .. } => {
                match self.evaluate_bool_with_backend(condition, pure_backend) {
                    Ok(true) => {
                        self.advance_if_needed(next_op_index);
                        self.fiber.frame.control_stack.push(FlowControlStackEntry {
                            kind: FlowControlStackEntryKind::While {
                                source: source_site.clone(),
                            },
                        });
                        if let Err(error) = self.push_while_iteration(&source_site) {
                            self.fail_format_aware_eval(error, output, pure_backend);
                        }
                    }
                    Ok(false) => self.advance_if_needed(next_op_index),
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::WhileLet {
                pattern,
                expr,
                guard,
                ..
            } => {
                match self.evaluate_if_let_with_backend(pattern, expr, guard.as_ref(), pure_backend)
                {
                    Ok(Some(bindings)) => {
                        self.advance_if_needed(next_op_index);
                        self.fiber.frame.control_stack.push(FlowControlStackEntry {
                            kind: FlowControlStackEntryKind::WhileLet {
                                source: source_site.clone(),
                            },
                        });
                        if let Err(error) = self.push_while_let_iteration(&source_site, bindings) {
                            self.fail_format_aware_eval(error, output, pure_backend);
                        }
                    }
                    Ok(None) => self.advance_if_needed(next_op_index),
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::For {
                source, evidence, ..
            } => {
                self.advance_if_needed(next_op_index);
                match self.evaluate_expr_with_backend(source, pure_backend) {
                    Ok(value) => match self.runtime_iterator_from_value_with_backend(
                        value,
                        evidence,
                        pure_backend,
                    ) {
                        Ok(iterator) => {
                            self.push_for_next(&source_site, iterator, output, pure_backend);
                        }
                        Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                    },
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowOp::Thread { .. } => {
                if self.task_request_quota_remaining == 0 {
                    self.fail_task_request_quota(
                        "Thread start exceeds the remaining task request quota",
                        output,
                    );
                    return;
                }
                let super::thread::PreparedThread {
                    spec,
                    mut child,
                    next_fiber_id,
                } = match self.prepare_thread(&source_site) {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                };
                let accepted = self
                    .need_producers
                    .ensure_task_with_publication(spec, |accepted| {
                        child.launch = super::FlowFiberLaunch::Task(accepted.handle());
                        self.child_fibers.push_back(child);
                        self.next_fiber_id = next_fiber_id;
                        self.run_child_next = true;
                    });
                let accepted = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                };
                self.task_request_quota_remaining -= 1;
                self.advance_if_needed(next_op_index);
                output.requests.tasks.push(accepted);
            }
            FlowOp::Scope { identity, .. } => {
                self.advance_if_needed(next_op_index);
                let inserted = self
                    .original_body_range(&source_site, crate::plan::RuntimeFlowBodyRole::Body)
                    .and_then(|body| {
                        self.push_scoped_source(
                            body,
                            Vec::new(),
                            identity.clone(),
                            crate::scope::RuntimeScopeFrameKind::EmittedLexical,
                            None,
                        )
                    });
                if let Err(error) = inserted {
                    self.fail_format_aware_eval(error, output, pure_backend);
                }
            }
            FlowOp::LetScope { .. } => {
                let body = match self
                    .original_body_range(&source_site, crate::plan::RuntimeFlowBodyRole::Body)
                {
                    Ok(body) => body,
                    Err(error) => {
                        self.fail_format_aware_eval(error, output, pure_backend);
                        return;
                    }
                };
                let work = [
                    Some(NativePendingWork::resume(
                        NativeRuntimeContinuation::EnterLetScope {
                            source: source_site.clone(),
                        },
                    )),
                    Some(NativePendingWork::source(
                        body,
                        NativeSourceRangeRole::NestedBody,
                    )),
                    Some(NativePendingWork::resume(
                        NativeRuntimeContinuation::CompleteLetScope {
                            source: source_site,
                        },
                    )),
                ];
                if let Err(error) = self.prepare_native_work(&work) {
                    self.fail_format_aware_eval(error, output, pure_backend);
                    return;
                }
                self.advance_if_needed(next_op_index);
                self.prepend_admitted_native_work(work);
            }
            FlowOp::Break(expr) => {
                let value = match expr {
                    Some(expr) => match self.evaluate_expr_with_backend(expr, pure_backend) {
                        Ok(value) => value,
                        Err(error) => {
                            self.fail_format_aware_eval(error, output, pure_backend);
                            return;
                        }
                    },
                    None => RuntimeValue::Unit,
                };
                let mut value = Some(value);
                let mut handled = self.break_nearest_loop(&mut value, output, pure_backend);
                while !handled && self.abandon_format_attempt_for_transfer() {
                    handled = self.break_nearest_loop(&mut value, output, pure_backend);
                }
                if handled {
                    self.advance_if_needed(next_op_index);
                } else {
                    self.fail_eval(RuntimeEvalError::MisplacedLoopControl("break"), output);
                }
            }
            FlowOp::Continue => {
                let mut handled = self.continue_nearest_loop(output, pure_backend);
                while !handled && self.abandon_format_attempt_for_transfer() {
                    handled = self.continue_nearest_loop(output, pure_backend);
                }
                if handled {
                    self.advance_if_needed(next_op_index);
                } else {
                    self.fail_eval(RuntimeEvalError::MisplacedLoopControl("continue"), output);
                }
            }
            FlowOp::Goto(target) => self.goto(target, output, pure_backend),
            FlowOp::GotoExpr(expr) => match self.evaluate_entity_target(expr) {
                Ok(target) => self.goto(&target, output, pure_backend),
                Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
            },
            FlowOp::Return(_) => {
                let FlowOp::Return(value) = op.into_owned() else {
                    unreachable!("selected return value");
                };
                self.return_value(value, output, pure_backend);
            }
            FlowOp::ReturnExpr(expr) => match self.evaluate_expr_with_backend(expr, pure_backend) {
                Ok(value) => {
                    let label = runtime_value_label(&value);
                    if !self.return_function_call_value(value, output, pure_backend) {
                        self.return_value(label, output, pure_backend);
                    }
                }
                Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
            },
            FlowOp::Effect(_) => {
                let FlowOp::Effect(effect) = op.into_owned() else {
                    unreachable!("selected effect");
                };
                self.emit_line_effect(effect, output, pure_backend);
                if !self.apply_control_effects(output, pure_backend) {
                    self.advance_if_needed(next_op_index);
                }
            }
            FlowOp::EvaluatedEffect(effect) => {
                if !self.execute_evaluated_effect(effect, drop_policy, output, pure_backend) {
                    return;
                }
                if !self.apply_control_effects(output, pure_backend) {
                    self.advance_if_needed(next_op_index);
                }
            }
            FlowOp::RegisterDefer { site, .. } => {
                self.fail_eval(
                    RuntimeEvalError::UnknownDeferredSite { site: *site },
                    output,
                );
            }
            FlowOp::RegisterCleanup { .. } => {
                let FlowOp::RegisterCleanup { key, effect } = op.into_owned() else {
                    unreachable!("selected cleanup registration");
                };
                self.register_scope_cleanup(key, effect);
                self.advance_if_needed(next_op_index);
            }
            FlowOp::CancelCleanup { key } => {
                self.cancel_scope_cleanup(key);
                self.advance_if_needed(next_op_index);
            }
            FlowOp::EnterScope { identity } => {
                self.push_scope_frame(identity.clone());
                self.advance_if_needed(next_op_index);
            }
            FlowOp::ExitScope => {
                if let Err(error) = self.exit_emitted_scope(output, pure_backend) {
                    self.fail_eval(error, output);
                    return;
                }
                self.advance_if_needed(next_op_index);
            }
            FlowOp::ExitScopeBind { pattern, expr } => {
                let value = match self.evaluate_expr_with_backend(expr, pure_backend) {
                    Ok(value) => value,
                    Err(error) => {
                        self.fail_format_aware_eval(error, output, pure_backend);
                        return;
                    }
                };
                if let Err(error) = self.exit_emitted_scope(output, pure_backend) {
                    self.fail_eval(error, output);
                    return;
                }
                self.bind_value(pattern, value, output);
                self.advance_if_needed(next_op_index);
            }
            FlowOp::Noop => {
                self.advance_if_needed(next_op_index);
            }
        }
    }

    pub(in crate::engine) fn has_active_project_call(&self) -> bool {
        self.fiber.frame.active_function().is_some()
    }

    fn start_project_call(
        &mut self,
        site: crate::runtime_id::RuntimeProjectCallSiteId,
        next_op_index: Option<usize>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let owner = Arc::clone(&self.plan);
        let Some(row) = owner.project_call_sites().get(site) else {
            self.fiber.frame.status =
                FlowFiberStatus::Failed(format!("missing project-call site {site}"));
            return;
        };
        let plan = row.plan();
        let prepared = (|| {
            let value = self.evaluate_expr_with_backend(plan.callee(), pure_backend)?;
            let RuntimeValue::Callable(callable) = value else {
                return Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                    &value,
                )));
            };
            callable
                .validate_for_owner(&crate::task::RuntimeProgramOwner::Plan(Arc::clone(&owner)))?;
            if callable.state() != plan.state() {
                return Err(crate::value::RuntimeCallableValueError::UnexpectedState {
                    expected: plan.state(),
                    actual: callable.state(),
                }
                .into());
            }
            let operands = self.evaluate_project_call_operands(plan, pure_backend)?;
            self.inspect_project_call_materialization(plan, &operands)?;
            let (arguments, attached) = self.materialize_project_call_owned(plan, operands);
            Ok::<_, RuntimeEvalError>((callable, arguments, attached))
        })();
        let (callable, arguments, attached) = match prepared {
            Ok(values) => values,
            Err(error) => {
                self.fail_format_aware_eval(error, output, pure_backend);
                return;
            }
        };
        if let Err(error) = self.start_callable_group(
            callable,
            arguments,
            attached,
            row.result().clone(),
            next_op_index,
            output,
            pure_backend,
        ) {
            self.fail_format_aware_eval(error, output, pure_backend);
        }
    }
    fn evaluate_project_call_operands(
        &mut self,
        plan: &crate::plan::RuntimeProjectCallPlan,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Vec<Vec<RuntimeValue>>, RuntimeEvalError> {
        plan.operands()
            .iter()
            .map(|operand| {
                let value = self.evaluate_expr_with_backend(operand.value(), pure_backend)?;
                match operand.mode() {
                    crate::value::RuntimeCallArgumentMode::Value => Ok(vec![value]),
                    crate::value::RuntimeCallArgumentMode::Spread => {
                        crate::value::runtime_value_into_sequence_values(value).map_err(|value| {
                            RuntimeEvalError::InvalidSpread(runtime_value_label(&value))
                        })
                    }
                }
            })
            .collect()
    }

    fn inspect_project_call_materialization(
        &self,
        plan: &crate::plan::RuntimeProjectCallPlan,
        operands: &[Vec<RuntimeValue>],
    ) -> Result<(), RuntimeEvalError> {
        let source = |index: u32| {
            usize::try_from(index)
                .ok()
                .and_then(|index| operands.get(index))
                .map(Vec::as_slice)
                .ok_or_else(|| {
                    RuntimeEvalError::InvalidSpread("project-call source is absent".to_owned())
                })
        };
        let mut seen = std::collections::BTreeSet::new();
        for row in plan.ordinary() {
            match row {
                RuntimeProjectCallOrdinaryMaterialization::Fixed(row) => {
                    let values = source(row.source_index())?;
                    if !seen.insert(row.source_index()) || values.len() != 1 {
                        return Err(RuntimeEvalError::InvalidSpread(
                            "fixed project-call source must be unique and singular".to_owned(),
                        ));
                    }
                    self.require_project_call_value(row.binding_ty(), &values[0])?;
                }
                RuntimeProjectCallOrdinaryMaterialization::Rest(row) => {
                    for &index in row.source_indices() {
                        if !seen.insert(index) {
                            return Err(RuntimeEvalError::InvalidSpread(
                                "project-call source is referenced twice".to_owned(),
                            ));
                        }
                        for value in source(index)? {
                            self.require_project_call_value(row.abi_ty(), value)?;
                        }
                    }
                }
            }
        }
        if let Some(index) = plan.attached().and_then(|row| row.source_index()) {
            let values = source(index)?;
            if !seen.insert(index) || values.len() != 1 {
                return Err(RuntimeEvalError::InvalidSpread(
                    "attached project-call source must be unique and singular".to_owned(),
                ));
            }
            let expected = plan
                .operands()
                .get(index as usize)
                .expect("sealed attached source index names an operand")
                .value()
                .ty();
            self.require_project_call_value(expected, &values[0])?;
        }
        if seen.len() != operands.len() {
            return Err(RuntimeEvalError::InvalidSpread(
                "project-call operand has no owned destination".to_owned(),
            ));
        }
        Ok(())
    }

    fn materialize_project_call_owned(
        &self,
        plan: &crate::plan::RuntimeProjectCallPlan,
        operands: Vec<Vec<RuntimeValue>>,
    ) -> (Vec<RuntimeValue>, Option<RuntimeValue>) {
        let mut sources = operands.into_iter().map(Some).collect::<Vec<_>>();
        let mut take = |index: u32| {
            sources
                .get_mut(index as usize)
                .and_then(Option::take)
                .expect("borrowed project-call proof sealed one source destination")
        };
        let mut arguments = Vec::with_capacity(plan.ordinary().len());
        for row in plan.ordinary() {
            match row {
                RuntimeProjectCallOrdinaryMaterialization::Fixed(row) => {
                    let value = take(row.source_index())
                        .pop()
                        .expect("fixed project-call source was singular");
                    arguments.push(value);
                }
                RuntimeProjectCallOrdinaryMaterialization::Rest(row) => {
                    let mut elements = Vec::new();
                    for &index in row.source_indices() {
                        elements.extend(take(index));
                    }
                    arguments.push(crate::value::runtime_sequence_values(elements));
                }
            }
        }
        let attached = plan
            .attached()
            .and_then(|row| row.source_index())
            .map(|index| {
                take(index)
                    .pop()
                    .expect("attached project-call source was singular")
            });
        assert!(sources.iter().all(Option::is_none));
        (arguments, attached)
    }

    fn require_project_call_value(
        &self,
        expected: crate::runtime_id::RuntimePlanTypeId,
        value: &RuntimeValue,
    ) -> Result<(), RuntimeEvalError> {
        if self.plan.value_matches_type(expected, value)? {
            Ok(())
        } else {
            Err(RuntimeEvalError::InvalidExpressionType(expected))
        }
    }

    pub(super) fn evaluate_host_call_arguments(
        &mut self,
        arguments: &[RuntimeHostArgumentTemplate],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<
        (
            Vec<crate::value::RuntimePayload>,
            Vec<NamedHostArg<crate::value::RuntimePayload>>,
        ),
        String,
    > {
        let mut positional = Vec::new();
        let mut named = Vec::new();
        for argument in arguments {
            let value = self
                .evaluate_expr_with_backend(argument.value(), pure_backend)
                .map_err(|error| error.to_string())?;
            match argument {
                RuntimeHostArgumentTemplate::Positional(..) => {
                    positional.push(crate::value::RuntimePayload::from(value));
                }
                RuntimeHostArgumentTemplate::Named(_, argument) => named.push(NamedHostArg {
                    name: argument.name.clone(),
                    value: crate::value::RuntimePayload::from(value),
                }),
                RuntimeHostArgumentTemplate::Spread(..) => {
                    let values = crate::value::runtime_value_into_sequence_values(value).map_err(
                        |value| {
                            format!(
                                "spread host argument requires a tuple or bracket sequence, found {}",
                                runtime_value_label(&value)
                            )
                        },
                    )?;
                    positional.extend(values.into_iter().map(crate::value::RuntimePayload::from));
                }
            }
        }
        Ok((positional, named))
    }

    pub(super) fn bind_value(
        &mut self,
        pattern: &RuntimePattern,
        value: RuntimeValue,
        output: &mut RuntimeStepOutput,
    ) {
        match self.try_bind_pattern_owned(pattern, value) {
            Ok(None) => {}
            Ok(Some(value)) => self.fail_eval(
                RuntimeEvalError::PatternMismatch(runtime_value_label(&value)),
                output,
            ),
            Err(error) => self.fail_eval(error, output),
        }
    }

    pub(super) fn advance_if_needed(&mut self, next_op_index: Option<usize>) {
        if let Some(next_op_index) = next_op_index {
            let cursor = self
                .fiber
                .frame
                .function_root_position_mut()
                .expect("a root operation retains its queued source until commit");
            assert!(
                cursor.set_operation(next_op_index),
                "source operation stays inside its admitted code range"
            );
        }
    }

    pub(super) fn next_host_call_id(&mut self, public_id: &str) -> RuntimeHostCallId {
        let id = self.preview_host_call_id(public_id);
        self.advance_host_call_sequence();
        id
    }

    pub(super) fn preview_host_call_id(&self, public_id: &str) -> RuntimeHostCallId {
        let sequence = self.next_host_call_sequence;
        RuntimeHostCallId(if sequence == 0 {
            public_id.to_owned()
        } else {
            format!("{public_id}.{sequence}")
        })
    }

    pub(super) fn advance_host_call_sequence(&mut self) {
        self.next_host_call_sequence = self.next_host_call_sequence.saturating_add(1);
    }

    pub(super) fn push_scope_frame(&mut self, identity: RuntimeScopeIdentity) {
        self.push_scope_frame_with_origin(
            identity,
            crate::scope::RuntimeScopeFrameOrigin::EmittedLexical,
        );
    }

    fn push_scope_frame_with_origin(
        &mut self,
        identity: RuntimeScopeIdentity,
        origin: crate::scope::RuntimeScopeFrameOrigin,
    ) {
        self.fiber.frame.env.push_scope_with_identity(identity);
        self.fiber.frame.control_stack.push(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::Scope {
                origin,
                cleanups: Vec::new(),
                match_guard: None,
            },
        });
    }

    fn allocate_scheduled_scope(
        &mut self,
        kind: crate::scope::RuntimeScopeFrameKind,
    ) -> Option<crate::scope::RuntimeScheduledScopeToken> {
        let ordinal = std::num::NonZeroU64::new(self.next_scheduled_scope_sequence)?;
        let Some(next) = self.next_scheduled_scope_sequence.checked_add(1) else {
            self.fiber.frame.status = FlowFiberStatus::Failed(
                crate::scope::RuntimeScopeExitError::IdentityCapacityExhausted.to_string(),
            );
            return None;
        };
        self.next_scheduled_scope_sequence = next;
        Some(crate::scope::RuntimeScheduledScopeToken::from_runtime(
            self.fiber.execution,
            self.fiber.persistent_id,
            ordinal,
            kind,
        ))
    }
    pub(super) fn push_control_scope_frame(
        &mut self,
    ) -> Option<crate::scope::RuntimeScheduledScopeToken> {
        let token = self.allocate_scheduled_scope(crate::scope::RuntimeScopeFrameKind::Control)?;
        self.push_scope_frame_with_origin(
            RuntimeScopeIdentity::Anonymous,
            crate::scope::RuntimeScopeFrameOrigin::Scheduled(token),
        );
        Some(token)
    }
    fn active_scope_targets(
        &self,
    ) -> impl Iterator<
        Item = (
            Option<crate::scope::RuntimeScheduledScopeToken>,
            crate::scope::RuntimeScopeFrameKind,
        ),
    > + '_ {
        self.fiber
            .frame
            .control_stack
            .iter()
            .rev()
            .map_while(|frame| match &frame.kind {
                FlowControlStackEntryKind::Scope { origin, .. } => {
                    Some((origin.token(), origin.kind()))
                }
                _ => None,
            })
    }
    pub(super) fn exit_emitted_scope(
        &mut self,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), crate::scope::RuntimeScopeExitError> {
        let exit = crate::scope::RuntimeScopeExitTarget::EmittedLexical
            .resolve(self.active_scope_targets())?;
        for _ in exit.targets() {
            self.pop_scope_frame(output, backend);
        }
        Ok(())
    }

    pub(super) fn register_scope_cleanup(
        &mut self,
        key: impl Into<String>,
        effect: LineEffectRequest,
    ) {
        let cleanup = FlowScopeCleanup::new(key, effect);
        if let Some(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::Scope { cleanups, .. },
        }) = self
            .fiber
            .frame
            .control_stack
            .iter_mut()
            .rev()
            .find(|entry| matches!(&entry.kind, FlowControlStackEntryKind::Scope { .. }))
        {
            cleanups.push(cleanup);
        } else {
            self.fiber.frame.root_cleanups.push(cleanup);
        }
    }

    pub(super) fn cancel_scope_cleanup(&mut self, key: &str) {
        self.fiber
            .frame
            .root_cleanups
            .retain(|cleanup| cleanup.key != key);
        for entry in &mut self.fiber.frame.control_stack {
            if let FlowControlStackEntryKind::Scope { cleanups, .. } = &mut entry.kind {
                cleanups.retain(|cleanup| cleanup.key != key);
            }
        }
    }

    pub(super) fn push_await_observer_ops(
        &mut self,
        bindings: Vec<RuntimeLocalBinding>,
        source: &NativeSourceOperation,
        ordinal: usize,
    ) -> Result<(), RuntimeEvalError> {
        let body = self.original_body_range(
            source,
            crate::plan::RuntimeFlowBodyRole::AwaitObserver { ordinal },
        )?;
        self.push_scoped_source(
            body,
            bindings,
            RuntimeScopeIdentity::Anonymous,
            crate::scope::RuntimeScopeFrameKind::Control,
            Some(NativeRuntimeContinuation::CompleteAwaitObserver),
        )
    }

    pub(super) fn push_loop_iteration(
        &mut self,
        source: &NativeSourceOperation,
    ) -> Result<(), RuntimeEvalError> {
        let body = self.original_body_range(source, crate::plan::RuntimeFlowBodyRole::Body)?;
        self.push_scoped_source(
            body,
            Vec::new(),
            RuntimeScopeIdentity::Anonymous,
            crate::scope::RuntimeScopeFrameKind::Control,
            Some(NativeRuntimeContinuation::LoopNext {
                source: source.clone(),
            }),
        )
    }

    pub(super) fn push_while_iteration(
        &mut self,
        source: &NativeSourceOperation,
    ) -> Result<(), RuntimeEvalError> {
        let body = self.original_body_range(source, crate::plan::RuntimeFlowBodyRole::Body)?;
        self.push_scoped_source(
            body,
            Vec::new(),
            RuntimeScopeIdentity::Anonymous,
            crate::scope::RuntimeScopeFrameKind::Control,
            Some(NativeRuntimeContinuation::WhileNext {
                source: source.clone(),
            }),
        )
    }

    pub(super) fn push_while_let_iteration(
        &mut self,
        source: &NativeSourceOperation,
        bindings: Vec<RuntimeLocalBinding>,
    ) -> Result<(), RuntimeEvalError> {
        let body = self.original_body_range(source, crate::plan::RuntimeFlowBodyRole::Body)?;
        self.push_scoped_source(
            body,
            bindings,
            RuntimeScopeIdentity::Anonymous,
            crate::scope::RuntimeScopeFrameKind::Control,
            Some(NativeRuntimeContinuation::WhileLetNext {
                source: source.clone(),
            }),
        )
    }
    pub(super) fn push_for_next(
        &mut self,
        source: &NativeSourceOperation,
        mut iterator: RuntimeIterator,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let plan = Arc::clone(&self.plan);
        let Some(FlowOp::For { pattern, .. }) = source.operation(&plan) else {
            self.fail_eval(
                "native iterator continuation lost its original For declaration",
                output,
            );
            return;
        };
        let item = match self.next_runtime_iterator_item(&mut iterator, pure_backend) {
            Ok(Some(item)) => item,
            Ok(None) => return,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let prepared = (|| {
            let body = self.original_body_range(source, crate::plan::RuntimeFlowBodyRole::Body)?;
            let (token, next) =
                self.prepare_native_scope(crate::scope::RuntimeScopeFrameKind::Control)?;
            let work = [
                Some(NativePendingWork::source(
                    body,
                    NativeSourceRangeRole::NestedBody,
                )),
                Some(NativePendingWork::resume(
                    NativeRuntimeContinuation::ExitScheduledScope { token },
                )),
                Some(NativePendingWork::resume(
                    NativeRuntimeContinuation::ForNext {
                        source: source.clone(),
                        iterator,
                    },
                )),
            ];
            self.prepare_native_work(&work)?;
            Ok::<_, RuntimeEvalError>((token, next, work))
        })();
        let (token, next, work) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.fail_format_aware_eval(error, output, pure_backend);
                return;
            }
        };
        self.next_scheduled_scope_sequence = next;
        self.push_scope_frame_with_origin(
            RuntimeScopeIdentity::Anonymous,
            crate::scope::RuntimeScopeFrameOrigin::Scheduled(token),
        );
        self.prepend_admitted_native_work(work);
        match self.try_bind_pattern_owned(pattern, item) {
            Ok(None) => {}
            Ok(Some(item)) => self.fail_eval(
                RuntimeEvalError::PatternMismatch(runtime_value_label(&item)),
                output,
            ),
            Err(error) => self.fail_eval(error, output),
        }
    }
    fn runtime_iterator_from_value_with_backend(
        &mut self,
        value: RuntimeValue,
        evidence: &RuntimeIteratorEvidence,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeIterator, RuntimeEvalError> {
        if let RuntimeIteratorEvidence::Witness(witness) = evidence {
            return match &witness.executable {
                RuntimeIteratorWitnessExecutable::TraitCalls { into_iter, next } => {
                    let outcome = self.evaluate_trait_method_values(
                        *into_iter,
                        RuntimeReceiverMode::Owned,
                        value,
                        Vec::new(),
                        pure_backend,
                    )?;
                    Ok(RuntimeIterator::try_witness(
                        crate::task::RuntimeProgramOwner::Plan(self.program_plan()),
                        outcome.value,
                        *next,
                    )?)
                }
                RuntimeIteratorWitnessExecutable::IdentityIntoIterator { next } => {
                    Ok(RuntimeIterator::try_witness(
                        crate::task::RuntimeProgramOwner::Plan(self.program_plan()),
                        value,
                        *next,
                    )?)
                }
            };
        }
        RuntimeIterator::from_value_with_evidence(value, evidence)
            .map_err(|value| RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(&value)))
    }

    fn next_runtime_iterator_item(
        &mut self,
        iterator: &mut RuntimeIterator,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let RuntimeIterator::Witness(witness) = iterator else {
            return Ok(iterator.next());
        };
        witness.validate_for_owner(&crate::task::RuntimeProgramOwner::Plan(self.program_plan()))?;
        let outcome = self.evaluate_trait_method_values(
            witness.next_method(),
            RuntimeReceiverMode::MutRef,
            std::mem::replace(witness.state_mut(), RuntimeValue::Unit),
            Vec::new(),
            pure_backend,
        )?;
        if let Some(updated_receiver) = outcome.updated_receiver {
            *witness.state_mut() = updated_receiver;
        }
        let label = runtime_value_label(&outcome.value);
        match outcome.value.try_into_builtin_variant_case() {
            Ok((RuntimeBuiltinVariantCaseIdentity::OptionNone, None)) => Ok(None),
            Ok((RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value))) => Ok(Some(value)),
            _ => Err(RuntimeEvalError::ExpectedBracketSeq(format!(
                "Iterator::next expected Option, found {label}"
            ))),
        }
    }

    pub(super) fn cancel_scheduled_scope_close(
        &mut self,
        origin: crate::scope::RuntimeScopeFrameOrigin,
    ) {
        if let crate::scope::RuntimeScopeFrameOrigin::Scheduled(token) = origin {
            self.fiber.frame.pending_ops.retain(|work| {
                !matches!(&work.kind, NativePendingWorkKind::Resume(
                    NativeRuntimeContinuation::ExitScheduledScope { token: queued }
                ) if *queued == token)
            });
        }
    }

    pub(super) fn pop_scope_frame(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        if !matches!(
            self.fiber.frame.control_stack.last(),
            Some(FlowControlStackEntry {
                kind: FlowControlStackEntryKind::Scope { .. }
            })
        ) {
            return;
        }
        let Some(FlowControlStackEntry {
            kind:
                FlowControlStackEntryKind::Scope {
                    origin, cleanups, ..
                },
        }) = self.fiber.frame.control_stack.pop()
        else {
            return;
        };
        self.cancel_scheduled_scope_close(origin);
        self.fiber.frame.env.pop_scope();
        self.emit_scope_cleanups(cleanups, output, pure_backend);
    }

    pub(super) fn pop_scope_frames_until_loop(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        while matches!(
            self.fiber.frame.control_stack.last(),
            Some(FlowControlStackEntry {
                kind: FlowControlStackEntryKind::Scope { .. }
            })
        ) {
            self.pop_scope_frame(output, pure_backend);
        }
    }

    pub(super) fn pop_loop_frame(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Option<FlowControlStackEntryKind> {
        self.pop_scope_frames_until_loop(output, pure_backend);
        let is_loop = self.fiber.frame.control_stack.last().is_some_and(|entry| {
            matches!(
                entry.kind,
                FlowControlStackEntryKind::Loop { .. }
                    | FlowControlStackEntryKind::While { .. }
                    | FlowControlStackEntryKind::WhileLet { .. }
            )
        });
        if is_loop {
            self.fiber.frame.control_stack.pop().map(|entry| entry.kind)
        } else {
            None
        }
    }

    pub(super) fn discard_pending_until_loop_next(&mut self) {
        while let Some(op) = self.fiber.frame.pending_ops.pop_front() {
            if matches!(
                op.kind,
                NativePendingWorkKind::Resume(
                    NativeRuntimeContinuation::LoopNext { .. }
                        | NativeRuntimeContinuation::WhileNext { .. }
                        | NativeRuntimeContinuation::WhileLetNext { .. }
                )
            ) {
                break;
            }
        }
    }

    pub(super) fn break_nearest_loop(
        &mut self,
        value: &mut Option<RuntimeValue>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        self.discard_pending_until_loop_next();
        let Some(kind) = self.pop_loop_frame(output, pure_backend) else {
            return false;
        };
        self.fiber.frame.await_observer = None;
        let plan = Arc::clone(&self.plan);
        match kind {
            FlowControlStackEntryKind::Loop { source } => {
                let Some(FlowOp::Loop { result, .. }) = source.operation(&plan) else {
                    self.fail_eval("native value loop lost its original declaration", output);
                    return true;
                };
                if let Some(pattern) = result {
                    self.bind_value(
                        pattern,
                        value.take().expect("selected break value remains owned"),
                        output,
                    );
                }
            }
            FlowControlStackEntryKind::While { .. }
            | FlowControlStackEntryKind::WhileLet { .. } => {
                if value.as_ref() != Some(&RuntimeValue::Unit) {
                    self.fail_eval(RuntimeEvalError::BreakValueOutsideValueLoop, output);
                }
            }
            FlowControlStackEntryKind::Scope { .. } => return false,
            FlowControlStackEntryKind::FormatAttempt(_) => return false,
            FlowControlStackEntryKind::FunctionCall(_) => return false,
        }
        true
    }

    pub(super) fn continue_nearest_loop(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        self.pop_scope_frames_until_loop(output, pure_backend);
        self.discard_pending_until_loop_next();
        let Some(kind) = self
            .fiber
            .frame
            .control_stack
            .last()
            .map(|frame| &frame.kind)
        else {
            return false;
        };
        self.fiber.frame.await_observer = None;
        match kind {
            FlowControlStackEntryKind::Loop { source } => {
                let source = source.clone();
                if let Err(error) = self.push_loop_iteration(&source) {
                    self.fail_format_aware_eval(error, output, pure_backend);
                }
            }
            FlowControlStackEntryKind::While { source } => {
                let work = [Some(NativePendingWork::resume(
                    NativeRuntimeContinuation::WhileNext {
                        source: source.clone(),
                    },
                ))];
                match self.prepare_native_work(&work) {
                    Ok(()) => self.prepend_admitted_native_work(work),
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowControlStackEntryKind::WhileLet { source } => {
                let work = [Some(NativePendingWork::resume(
                    NativeRuntimeContinuation::WhileLetNext {
                        source: source.clone(),
                    },
                ))];
                match self.prepare_native_work(&work) {
                    Ok(()) => self.prepend_admitted_native_work(work),
                    Err(error) => self.fail_format_aware_eval(error, output, pure_backend),
                }
            }
            FlowControlStackEntryKind::Scope { .. } => {
                self.fail_eval(RuntimeEvalError::MisplacedLoopControl("continue"), output);
                return false;
            }
            FlowControlStackEntryKind::FormatAttempt(_) => return false,
            FlowControlStackEntryKind::FunctionCall(_) => {
                self.fail_eval(RuntimeEvalError::MisplacedLoopControl("continue"), output);
                return false;
            }
        }
        true
    }

    pub(super) fn drain_root_cleanups(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let cleanups = std::mem::take(&mut self.fiber.frame.root_cleanups);
        self.emit_scope_cleanups(cleanups, output, pure_backend);
    }

    pub(super) fn emit_scope_cleanups(
        &mut self,
        mut cleanups: Vec<FlowScopeCleanup>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        while let Some(cleanup) = cleanups.pop() {
            self.emit_line_effect(cleanup.effect, output, pure_backend);
        }
    }
}

#[cfg(test)]
#[path = "flow/iterator_tests.rs"]
mod iterator_tests;

#[cfg(test)]
mod ownership_tests {
    use super::Engine;
    use crate::pattern::RuntimeSemanticTypeId;
    use crate::plan::{
        RuntimePlanBuilder, RuntimePlanSequenceKind, RuntimePlanTypeProjection,
        RuntimePlanTypeSeed, RuntimeProjectCallOperand, RuntimeProjectCallOrdinaryMaterialization,
        RuntimeProjectCallPlan, RuntimeProjectCallRestMaterialization,
    };
    use crate::runtime_id::{RuntimeCallableStateId, RuntimeLocalDeclarationId};
    use crate::task::NeedId;
    use crate::value::{
        RuntimeCallArgumentMode, RuntimeExpr, RuntimeExprKind, RuntimeLocalRead,
        RuntimeLocalReadMode, RuntimeValue, runtime_value_into_sequence_values,
    };
    use std::num::NonZeroU32;

    #[test]
    fn project_call_rest_materialization_moves_two_affine_sources_once() {
        let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
        let need = RuntimeSemanticTypeId::from_bytes([0x82; 32]);
        let sequence = RuntimeSemanticTypeId::from_bytes([0x83; 32]);
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                    RuntimePlanTypeSeed::new(need, RuntimePlanTypeProjection::Need(unit)),
                    RuntimePlanTypeSeed::new(
                        sequence,
                        RuntimePlanTypeProjection::Sequence {
                            kind: RuntimePlanSequenceKind::Vec,
                            item: need,
                        },
                    ),
                ],
                [],
            )
            .expect("affine rest types");
        let plan = builder.finish().expect("closed type table");
        let unit_ty = plan.type_table().id_for_semantic(unit).expect("unit");
        let need_ty = plan.type_table().id_for_semantic(need).expect("Need");
        let sequence_ty = plan
            .type_table()
            .id_for_semantic(sequence)
            .expect("Vec<Need>");
        let local = RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::MIN);
        let operand = || {
            RuntimeProjectCallOperand::from_admitted_parts(
                RuntimeExpr::from_admitted_parts(
                    need_ty,
                    RuntimeExprKind::Local(RuntimeLocalRead::from_admitted_parts(
                        local,
                        RuntimeLocalReadMode::Move,
                    )),
                ),
                RuntimeCallArgumentMode::Value,
            )
        };
        let call = RuntimeProjectCallPlan::try_from_admitted_parts(
            RuntimeExpr::from_admitted_parts(unit_ty, RuntimeExprKind::Value(RuntimeValue::Unit)),
            RuntimeCallableStateId::from_zero_based(0).expect("state id"),
            0,
            vec![operand(), operand()].into_boxed_slice(),
            vec![RuntimeProjectCallOrdinaryMaterialization::Rest(
                RuntimeProjectCallRestMaterialization::from_admitted_parts(
                    0,
                    need_ty,
                    sequence_ty,
                    Box::new([0, 1]),
                ),
            )]
            .into_boxed_slice(),
            None,
        )
        .expect("each physical source has one destination");
        let engine = Engine::new(plan);
        let first = crate::tests::reusable_need_with_outcome(
            "need.project.rest.first",
            crate::task::TaskOutcomeContract::new(crate::pattern::RuntimeCheckedType::Unit),
        );
        let second = crate::tests::reusable_need_with_outcome(
            "need.project.rest.second",
            crate::task::TaskOutcomeContract::new(crate::pattern::RuntimeCheckedType::Unit),
        );
        let operands = vec![
            vec![RuntimeValue::NeedHandle(first.clone())],
            vec![RuntimeValue::NeedHandle(second.clone())],
        ];
        engine
            .inspect_project_call_materialization(&call, &operands)
            .expect("borrowed affine source preflight");
        let (arguments, attached) = engine.materialize_project_call_owned(&call, operands);
        assert!(attached.is_none());
        let mut arguments = arguments.into_iter();
        let sequence = arguments.next().expect("one logical rest argument");
        assert!(arguments.next().is_none());
        let values = runtime_value_into_sequence_values(sequence).expect("owned rest pack");
        assert_eq!(
            values,
            vec![
                RuntimeValue::NeedHandle(first),
                RuntimeValue::NeedHandle(second)
            ]
        );
    }
}

#[cfg(test)]
#[path = "flow/scope_tests.rs"]
mod scope_tests;
