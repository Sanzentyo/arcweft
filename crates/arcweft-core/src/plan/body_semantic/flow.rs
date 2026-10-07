//! Exhaustive Flow metadata and balanced body traversal.

use super::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::plan::construction::task_coordinates::{
    RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanCoordinateOwner,
};
use crate::plan::{self, FlowOp};
use crate::task::semantic::TaskSemanticEncoder;

/// Current producer leaves seen by the private visitor. The final sealer
/// replaces their caller digest with its owner-issued build coordinate; this
/// visitor never reads that completed digest as executable body metadata.
pub(crate) enum RuntimeBodyTaskSource<'a> {
    Producer(&'a crate::task::NeedProducerTemplate),
    Host(&'a plan::RuntimeHostCallTarget),
    Start(&'a plan::RuntimeNeedProducerStartTarget),
}

impl RuntimeBodySemanticContext<'_> {
    pub(crate) fn write_flow(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        ops: &[FlowOp],
        task_owner: &RuntimeTaskPlanCoordinateOwner,
        task_reference: &mut impl FnMut(
            RuntimeBodyTaskSource<'_>,
        ) -> Result<
            RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
    ) -> Result<(), RuntimeBodySemanticError> {
        use plan::flow_ops::RuntimeFlowTreeEvent as Event;
        plan::flow_ops::try_visit_ops_events(ops, &mut |event| {
            encoder.status()?;
            match event {
                Event::EnterBody { role, ops } => {
                    encoder.enter_element();
                    encoder.tag(0);
                    role.encode_semantic_path(encoder);
                    encoder.count(ops.len());
                }
                Event::ExitBody => encoder.tag(1),
                Event::EnterOperation { ordinal, op } => {
                    encoder.enter_element();
                    encoder.tag(2);
                    encoder.count(ordinal);
                    self.write_flow_metadata(encoder, op, task_owner, task_reference)?;
                    op.try_visit_value_roots(&mut |role, node| {
                        encoder.enter_element();
                        role.encode_semantic_path(encoder);
                        self.write_node(encoder, node)
                    })?;
                }
                Event::ExitOperation => encoder.tag(3),
            }
            encoder.status().map_err(Into::into)
        })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive Flow operation metadata algebra; owned body/value traversal remains on shared owners"
    )]
    fn write_flow_metadata(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        op: &FlowOp,
        task_owner: &RuntimeTaskPlanCoordinateOwner,
        task_reference: &mut impl FnMut(
            RuntimeBodyTaskSource<'_>,
        ) -> Result<
            RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.tag(op.semantic_tag());
        match op {
            FlowOp::Bind(_)
            | FlowOp::ForNext { .. }
            | FlowOp::LoopNext { .. }
            | FlowOp::WhileNext { .. }
            | FlowOp::WhileLetNext { .. }
            | FlowOp::CompleteFormatOperand { .. }
            | FlowOp::CompleteAwaitObserver => {
                encoder.reject_owner();
                return Err(RuntimeBodySemanticError::RuntimeFlowContinuation);
            }
            FlowOp::FormatOperandAttempt {
                attempt, parameter, ..
            } => {
                self.write_format_attempt(encoder, *attempt)?;
                encoder.count(parameter.index());
            }
            FlowOp::Assign { place, .. } => self.write_assignment(encoder, place)?,
            FlowOp::LineOperation { binding, operation } => {
                encoder.tag(u8::from(binding.is_some()));
                self.write_line_operation(encoder, operation)?;
            }
            FlowOp::Dialogue {
                content, result, ..
            } => {
                self.write_dialogue_content(encoder, *content)?;
                self.write_type(encoder, result.ty())?;
            }
            FlowOp::Choice { id, options } => {
                Self::write_optional_string(encoder, id.as_deref());
                encoder.count(options.len());
                for option in options {
                    encoder.enter_element();
                    Self::write_optional_string(encoder, option.id.as_deref());
                    encoder.string(&option.label);
                    encoder.tag(u8::from(option.target.is_some()));
                    if let Some(target) = &option.target {
                        self.write_flow_target(encoder, target)?;
                    }
                    encoder.tag(u8::from(option.out.is_some()));
                    if let Some(out) = &option.out {
                        Self::write_optional_string(encoder, out.label.as_deref());
                        encoder.string(&out.value);
                    }
                    encoder.count(option.effects.len());
                    for effect in &option.effects {
                        encoder.enter_element();
                        effect.encode_body_metadata(encoder);
                    }
                }
            }
            FlowOp::Await {
                binding, observers, ..
            } => {
                encoder.tag(u8::from(binding.is_some()));
                encoder.count(observers.len());
            }
            FlowOp::StartNeedProducer { target, .. } => {
                self.write_host_arguments_metadata(encoder, target.arguments())?;
                Self::write_task_reference(
                    encoder,
                    RuntimeBodyTaskSource::Start(target),
                    task_owner,
                    task_reference,
                )?;
            }
            FlowOp::HostCall { binding, target } => {
                encoder.tag(u8::from(binding.is_some()));
                Self::write_task_reference(
                    encoder,
                    RuntimeBodyTaskSource::Host(target),
                    task_owner,
                    task_reference,
                )?;
                self.write_type(encoder, target.result)?;
                self.write_host_arguments_metadata(encoder, &target.args)?;
            }
            FlowOp::Thread {
                producer, captures, ..
            } => {
                self.write_host_arguments_metadata(encoder, &producer.request.args)?;
                Self::write_task_reference(
                    encoder,
                    RuntimeBodyTaskSource::Producer(producer),
                    task_owner,
                    task_reference,
                )?;
                encoder.count(captures.len());
                for capture in captures {
                    encoder.enter_element();
                    self.write_local(encoder, *capture)?;
                }
            }
            FlowOp::AwaitMany {
                binding,
                target,
                pending,
            } => {
                encoder.tag(u8::from(binding.is_some()));
                self.write_local(encoder, target.item_binding)?;
                encoder.ordinal(target.limit);
                Self::write_task_reference(
                    encoder,
                    RuntimeBodyTaskSource::Producer(&target.base),
                    task_owner,
                    task_reference,
                )?;
                Self::write_task_reference(
                    encoder,
                    RuntimeBodyTaskSource::Producer(&target.child),
                    task_owner,
                    task_reference,
                )?;
                encoder.count(pending.len());
                for effect in pending {
                    encoder.enter_element();
                    effect.encode_body_metadata(encoder);
                }
            }
            FlowOp::ProjectCall { site } => self.write_project_call(encoder, *site)?,
            FlowOp::ApplyGroup { args, .. } => self.write_call_arguments(encoder, args)?,
            FlowOp::IfLet { guard, .. } | FlowOp::WhileLet { guard, .. } => {
                encoder.tag(u8::from(guard.is_some()));
            }
            FlowOp::Match { arms, .. } => {
                encoder.count(arms.len());
                for arm in arms {
                    encoder.enter_element();
                    encoder.tag(u8::from(arm.guard.is_some()));
                    if let Some(guard) = &arm.guard {
                        self.write_local(encoder, guard.candidate)?;
                        encoder.tag(u8::from(guard.condition.is_some()));
                        encoder.count(guard.copy_locals.len());
                        for local in &guard.copy_locals {
                            encoder.enter_element();
                            self.write_local(encoder, *local)?;
                        }
                    }
                }
            }
            FlowOp::Loop { result, .. } => encoder.tag(u8::from(result.is_some())),
            FlowOp::For { evidence, .. } => self.write_iterator(encoder, evidence)?,
            FlowOp::Break(value) => encoder.tag(u8::from(value.is_some())),
            FlowOp::Goto(target) => self.write_flow_target(encoder, target)?,
            FlowOp::Return(value) | FlowOp::CancelCleanup { key: value } => encoder.string(value),
            FlowOp::Effect(effect) => effect.encode_body_metadata(encoder),
            FlowOp::EvaluatedEffect(effect) => effect.encode_body_metadata(encoder),
            FlowOp::RegisterCleanup { key, effect } => {
                encoder.string(key);
                effect.encode_body_metadata(encoder);
            }
            FlowOp::RegisterDefer {
                site,
                outcome,
                captures,
                owner,
            } => {
                let function = self.plan.defer_function_site(*site).ok_or_else(|| {
                    encoder.reject_owner();
                    RuntimeBodySemanticError::MissingRow {
                        table: "defer sites",
                        ordinal: site.index(),
                    }
                })?;
                self.write_function_reference(encoder, function)?;
                encoder.tag(match outcome {
                    crate::line_task::RuntimeDeferOutcomeFilter::Always => 0,
                    crate::line_task::RuntimeDeferOutcomeFilter::Completed => 1,
                    crate::line_task::RuntimeDeferOutcomeFilter::Cancelled => 2,
                    crate::line_task::RuntimeDeferOutcomeFilter::Failed => 3,
                });
                encoder.tag(match owner {
                    plan::RuntimeDeferOwner::CurrentScope => 0,
                    plan::RuntimeDeferOwner::LineRoot => 1,
                });
                encoder.count(captures.len());
            }
            FlowOp::Let { .. }
            | FlowOp::LetElse { .. }
            | FlowOp::CommitDialogueResult { .. }
            | FlowOp::SelectDialogueResult { .. }
            | FlowOp::If { .. }
            | FlowOp::While { .. }
            | FlowOp::Scope { .. }
            | FlowOp::LetScope { .. }
            | FlowOp::Continue
            | FlowOp::GotoExpr(_)
            | FlowOp::ReturnExpr(_)
            | FlowOp::EnterScope { .. }
            | FlowOp::ExitScope
            | FlowOp::ExitScopeBind { .. }
            | FlowOp::Noop => {}
        }
        encoder.status().map_err(Into::into)
    }

    fn write_task_reference(
        encoder: &mut TaskSemanticEncoder<'_>,
        source: RuntimeBodyTaskSource<'_>,
        owner: &RuntimeTaskPlanCoordinateOwner,
        resolver: &mut impl FnMut(
            RuntimeBodyTaskSource<'_>,
        )
            -> Result<RuntimeTaskPlanBuildCoordinate, RuntimeBodySemanticError>,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        encoder.enter_element();
        encoder.tag(0);
        encoder.status()?;
        let ordinal = resolver(source).inspect_err(|_| {
            encoder.reject_owner();
        })?;
        if !owner.contains(&ordinal) {
            encoder.reject_owner();
            return Err(RuntimeBodySemanticError::ForeignTaskCoordinate);
        }
        encoder.ordinal(ordinal.ordinal());
        encoder.status().map_err(Into::into)
    }

    fn write_host_arguments_metadata(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        arguments: &[crate::task::RuntimeHostArgumentTemplate],
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.count(arguments.len());
        for (ordinal, argument) in arguments.iter().enumerate() {
            encoder.enter_element();
            encoder.count(ordinal);
            encoder.tag(argument.semantic_tag());
            encoder.digest(argument.identity().as_bytes());
            self.write_type(encoder, argument.value().ty())?;
        }
        encoder.status().map_err(Into::into)
    }

    fn write_optional_string(encoder: &mut TaskSemanticEncoder<'_>, value: Option<&str>) {
        encoder.tag(u8::from(value.is_some()));
        if let Some(value) = value {
            encoder.string(value);
        }
    }

    fn write_flow_target(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        target: &plan::FlowRuntimeId,
    ) -> Result<(), RuntimeBodySemanticError> {
        let flow = self
            .plan
            .flows()
            .iter()
            .find(|flow| &flow.id == target)
            .ok_or_else(|| {
                encoder.reject_owner();
                RuntimeBodySemanticError::MissingRow {
                    table: "flow targets",
                    ordinal: self.plan.flows().len(),
                }
            })?;
        encoder.digest(flow.definition().as_bytes());
        encoder.status().map_err(Into::into)
    }
}
