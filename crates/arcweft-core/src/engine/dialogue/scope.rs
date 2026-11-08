//! Activation-owned scope exit, including a sole result retained during cleanup.

use super::{
    ActivationScopeStep, DialogueActivationExecutionSlot, DialogueActivationFrame,
    DialogueActivationStep, DialogueExecutionError, Engine, NativeDialogueActivationState,
    RuntimeValue, advance_activation_pc, flush_commands, preview_result_destination_local,
    unique_affine_line_handles,
};
use crate::line_task::{LineRuntimeError, RuntimeHandleOwnerSlot, ScopeExit};
use crate::pattern::RuntimePattern;
use crate::plan::FlowOp;
use crate::presentation::RuntimeCommandQueue;
use crate::pure::RuntimeCallBackend;
use crate::runtime_id::{DialogueActivationId, RuntimeLineHandleToken, RuntimeLineTaskGroupId};
use crate::value::ownership::RuntimeOwnedSlotId;
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission, RuntimeStoredValue,
};

/// The original activation operation remains parked until this scope drains.
/// Its value is neither bound to the outer environment nor discarded early.
#[derive(Debug, PartialEq)]
pub(in crate::engine) struct PendingActivationScopeResult<V = RuntimeValue> {
    pub(super) operation: usize,
    pub(super) scope_depth: usize,
    pub(super) value: V,
}

impl Clone for PendingActivationScopeResult<crate::value::AwbcRuntimeValueSnapshot> {
    fn clone(&self) -> Self {
        Self {
            operation: self.operation,
            scope_depth: self.scope_depth,
            value: self.value.clone(),
        }
    }
}

impl<V: RuntimeStoredValue> PendingActivationScopeResult<V> {
    pub(super) fn admit(
        &self,
        group: RuntimeLineTaskGroupId,
        operation: usize,
        scope_depth: usize,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let invalid = || AwbcRuntimeValueSnapshotError::Message {
            message: "native dialogue scope result differs from its original exit".into(),
        };
        admission.validate_metadata(&(self.operation, self.scope_depth))?;
        if self.operation != operation || self.scope_depth != scope_depth || scope_depth == 0 {
            return Err(invalid());
        }
        let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner() else {
            return Err(invalid());
        };
        let Some(FlowOp::ExitScopeBind { expr, .. }) = plan
            .line_task_groups()
            .get(group.index())
            .and_then(|group| group.activation_ops().get(operation))
        else {
            return Err(invalid());
        };
        let expected = plan
            .type_table()
            .get(expr.ty())
            .ok_or_else(invalid)?
            .semantic_identity();
        admission.validate_owned_typed_borrowed_in_invocation(
            self.value.borrowed_value(),
            expected,
            binding,
        )
    }
}

impl Engine {
    pub(super) fn execute_activation_scope_binding(
        &mut self,
        slot: &mut DialogueActivationExecutionSlot,
        pattern: &RuntimePattern,
        expression: &crate::value::RuntimeExpr,
        before: &std::collections::BTreeMap<RuntimeLineHandleToken, RuntimeHandleOwnerSlot>,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<DialogueActivationStep, DialogueExecutionError> {
        if slot.transaction().frame().pending_scope_result.is_none() {
            let owner = crate::task::RuntimeProgramOwner::Plan(self.plan.clone());
            let image = self
                .inert_rollback_image_with_dialogue(
                    Some(slot.transaction()),
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                )
                .map_err(|message| DialogueExecutionError::ActivationExecutionFailed { message })?
                .0;
            let result = (|| {
                let value = self.evaluate_dialogue_expr(slot, expression, pure_backend)?;
                let frame = slot.transaction_mut().frame_mut();
                if frame.scopes.is_empty()
                    || !crate::pattern::inspect_runtime_pattern_owned(
                        &self.plan,
                        pattern,
                        &value,
                        frame.execution.env.function_instantiation(),
                    )?
                {
                    return Err(LineRuntimeError::InvalidActivationOperation.into());
                }
                frame.pending_scope_result = Some(PendingActivationScopeResult {
                    operation: frame.activation_pc,
                    scope_depth: frame.scopes.len(),
                    value,
                });
                self.reconcile_activation_instruction(slot.transaction_mut(), before, None)?;
                Ok(DialogueActivationStep::Continue)
            })();
            return result.map_err(|cause| {
                self.restore_failed_dialogue_instruction(
                    slot.transaction_mut(),
                    image,
                    &owner,
                    cause,
                )
            });
        }
        let transaction = slot.transaction_mut();
        let id = *transaction.activation();
        let (frame, line) = transaction.parts_mut();
        let step = self.prepare_activation_scope_exit(&id, frame, line, ScopeExit::Completed)?;
        match step {
            ActivationScopeStep::Waiting(received) => {
                transaction.stage_received_handles(received);
                Ok(DialogueActivationStep::Continue)
            }
            ActivationScopeStep::Deferred(batch, received) => {
                transaction.stage_received_handles(received);
                Ok(DialogueActivationStep::Deferred(batch))
            }
            ActivationScopeStep::Finished(received) => {
                advance_activation_pc(transaction.frame_mut())?;
                transaction.stage_received_handles(received);
                Ok(DialogueActivationStep::Continue)
            }
        }
    }

    /// Advances one actual lexical scope without exposing its pending result.
    pub(super) fn prepare_activation_scope_exit(
        &self,
        activation_id: &DialogueActivationId,
        frame: &mut DialogueActivationFrame,
        activation: &mut NativeDialogueActivationState,
        exit: ScopeExit,
    ) -> Result<ActivationScopeStep, DialogueExecutionError> {
        let scope = frame
            .scopes
            .last_mut()
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        let progress = self.step_line_defer_scope(
            activation_id,
            activation,
            scope,
            super::super::defer::NativeDeferScopeContext {
                parent: frame.parent,
                execution: self.fiber.execution,
                owner: super::super::defer::NativeDeferParent::Activation,
            },
            exit,
        )?;
        match progress.step {
            super::super::defer::NativeDeferScopeStep::Waiting => {
                return Ok(ActivationScopeStep::Waiting(progress.received));
            }
            super::super::defer::NativeDeferScopeStep::Deferred(batch) => {
                return Ok(ActivationScopeStep::Deferred(batch, progress.received));
            }
            super::super::defer::NativeDeferScopeStep::Drained => {}
        }
        let received =
            self.release_activation_scope_locals(activation_id, frame, activation, exit)?;
        frame.scopes.pop();
        Ok(ActivationScopeStep::Finished(received))
    }

    /// Preflights the complete scope drop and result transfer together. The one
    /// issuer proof covers both, before any values leave their physical cells.
    fn release_activation_scope_locals(
        &self,
        activation_id: &DialogueActivationId,
        frame: &mut DialogueActivationFrame,
        activation: &mut NativeDialogueActivationState,
        exit: ScopeExit,
    ) -> Result<
        Option<crate::line_task::PreparedRuntimeParentFiberReconciliation>,
        DialogueExecutionError,
    > {
        let plan = self.plan.clone();
        let pending = frame.pending_scope_result.as_ref();
        let pattern = if let Some(pending) = pending {
            if pending.operation != frame.activation_pc || pending.scope_depth != frame.scopes.len()
            {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            let Some(FlowOp::ExitScopeBind { pattern, .. }) = plan
                .line_task_groups()
                .get(frame.task_group.index())
                .and_then(|group| group.activation_ops().get(pending.operation))
            else {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            };
            if matches!(exit, ScopeExit::Completed) {
                if !crate::pattern::inspect_runtime_pattern_owned(
                    &plan,
                    pattern,
                    &pending.value,
                    frame.execution.env.function_instantiation(),
                )? {
                    return Err(LineRuntimeError::InvalidActivationOperation.into());
                }
                if pattern
                    .binding_declarations()
                    .any(|binding| frame.execution.env.get_stored(binding.local()).is_some())
                {
                    return Err(LineRuntimeError::InvalidActivationOperation.into());
                }
                Some(pattern)
            } else {
                None
            }
        } else {
            None
        };
        let mut ledger = activation.ledger().clone();
        let mut commands = RuntimeCommandQueue::new(*activation_id, activation.command_sequence());
        let mut seen = std::collections::BTreeSet::new();
        let mut received_before = std::collections::BTreeMap::new();
        let mut received_after = std::collections::BTreeMap::new();
        for (local, value) in frame.execution.env.current_scope_bindings() {
            for handle in unique_affine_line_handles(value)? {
                if !seen.insert(handle.token().clone()) {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                }
                let owner = RuntimeHandleOwnerSlot::in_dialogue_fiber(
                    self.published_receiver_execution(),
                    activation_id,
                    handle.token(),
                    frame
                        .execution
                        .owned_local_slot(self.fiber.execution, local),
                );
                if matches!(owner, RuntimeHandleOwnerSlot::ParentFiber { .. }) {
                    received_before.insert(handle.token().clone(), owner);
                } else {
                    let lease = ledger
                        .lease(handle.token())
                        .ok_or(LineRuntimeError::UnknownHandle)?;
                    if lease.resource().kind() != handle.kind() {
                        return Err(LineRuntimeError::WrongOpaqueProducer.into());
                    }
                    if !matches!(lease.owner(), RuntimeHandleOwnerSlot::DialogueResult(_)) {
                        ledger.drop_owned(handle.token(), &owner, &mut commands)?;
                    }
                }
            }
        }
        if let Some(pending) = pending {
            for handle in unique_affine_line_handles(&pending.value)? {
                if !seen.insert(handle.token().clone()) {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                }
                let source = RuntimeHandleOwnerSlot::in_dialogue_fiber(
                    self.published_receiver_execution(),
                    activation_id,
                    handle.token(),
                    RuntimeOwnedSlotId::NativeDialogueScopeResult {
                        execution: self.fiber.execution,
                        activation: *activation_id,
                    },
                );
                let destination = pattern
                    .map(|pattern| {
                        preview_result_destination_local(pattern, &pending.value, handle.token())
                    })
                    .transpose()?
                    .flatten()
                    .map(|local| {
                        RuntimeHandleOwnerSlot::in_dialogue_fiber(
                            self.published_receiver_execution(),
                            activation_id,
                            handle.token(),
                            frame
                                .execution
                                .owned_local_slot(self.fiber.execution, local),
                        )
                    });
                if matches!(source, RuntimeHandleOwnerSlot::ParentFiber { .. }) {
                    received_before.insert(handle.token().clone(), source);
                    if let Some(destination) = destination {
                        received_after.insert(handle.token().clone(), destination);
                    }
                } else {
                    let lease = ledger
                        .lease(handle.token())
                        .ok_or(LineRuntimeError::UnknownHandle)?;
                    if lease.resource().kind() != handle.kind() {
                        return Err(LineRuntimeError::WrongOpaqueProducer.into());
                    }
                    if let Some(destination) = destination {
                        ledger.transfer(handle.token(), &source, destination)?;
                    } else {
                        ledger.drop_owned(handle.token(), &source, &mut commands)?;
                    }
                }
            }
        }
        let received = self
            .dialogue_activations
            .inspect_received_fiber_reconciliation(
                self.published_receiver_execution(),
                &received_before,
                &received_after,
                &crate::line_task::RuntimeHandleDropAuthorization::at_boundary(Some(
                    crate::effect::RuntimeDropPolicy::Default,
                )),
                None,
                None,
            )?;
        flush_commands(activation_id, activation, commands)?;
        activation.commit_ledger(ledger);
        let _ = frame.execution.env.pop_scope_bindings();
        if let Some(pending) = frame.pending_scope_result.take()
            && let Some(pattern) = pattern
        {
            let bindings = crate::pattern::match_runtime_pattern_owned(
                &plan,
                pattern,
                pending.value,
                frame.execution.env.function_instantiation(),
            )
            .expect("scope result passed borrowed pattern preflight")
            .expect("scope result matched its original pattern");
            frame.execution.env.bind_all(bindings);
        }
        Ok(received)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::RuntimeCheckedType;
    use crate::plan::*;
    use crate::runtime_id::{RuntimeDialogueContentTemplateId, RuntimeDialogueEffectSiteCount};
    use crate::value::AwbcRuntimeValueSnapshot;

    fn plan() -> std::sync::Arc<RuntimePlan> {
        let unit = RuntimeCheckedType::Unit.semantic_identity_digest();
        let boolean = RuntimeCheckedType::Bool.semantic_identity_digest();
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                    RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                ],
                [],
            )
            .unwrap();
        let group = builder
            .push_line_task_group_seed(RuntimeLineTaskGroupSeed {
                definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([0x93; 32]),
                effects: RuntimeEffectSet::empty(),
                initial_inputs: Box::new([]),
                activation_ops: vec![
                    RuntimeFlowOpSeed::EnterScope {
                        identity: crate::scope::RuntimeScopeIdentity::Anonymous,
                    },
                    RuntimeFlowOpSeed::ExitScopeBind {
                        pattern: RuntimePatternSeed::new(boolean, RuntimePatternSeedKind::Discard),
                        expr: RuntimeExprSeed::new(
                            boolean,
                            RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                        ),
                    },
                ],
                result_type: unit,
                handle_sites: Box::new([]),
                root: RuntimeLineTaskNodeSeed::Action(vec![]),
                cancel_rules: Box::new([]),
                cleanup_completed: vec![],
                cleanup_cancelled: vec![],
                cleanup_failed: vec![],
                cleanup_policy: crate::line_task::LineCleanupPolicy::default(),
            })
            .unwrap();
        let template = RuntimeDialogueContentTemplateManifestSeed {
            id: RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap(),
            digest: crate::entry::RuntimeDialogueContentTemplateDigest::from_bytes([0x94; 32]),
            slots: Box::new([]),
            effects: Box::new([]),
        };
        let key = RuntimeDialogueContentTemplateKey::new(template.id, template.digest);
        builder
            .register_dialogue_content_template_seed(template)
            .unwrap();
        let content = builder
            .push_dialogue_content_seed(RuntimeDialogueContentPlanSeed {
                line: RuntimeLineId::from_source_entity_body("line.scope_result").unwrap(),
                template: key,
                values: Box::new([]),
                effect_sites: Box::new([]),
                marks: Box::new([]),
                effect_site_count: RuntimeDialogueEffectSiteCount::try_from_len(0).unwrap(),
            })
            .unwrap();
        builder
            .attach_line_task_group_seed(&content, &group)
            .unwrap();
        std::sync::Arc::new(builder.finish().unwrap())
    }

    #[test]
    fn scope_result_admission_checks_original_exit_frontier_type_and_shared_budget() {
        let plan = plan();
        let owner = crate::task::RuntimeProgramOwner::Plan(plan.clone());
        let group = RuntimeLineTaskGroupId::from_zero_based(0).unwrap();
        let pending = PendingActivationScopeResult {
            operation: 1,
            scope_depth: 1,
            value: RuntimeValue::Bool(true),
        };
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        pending.admit(group, 1, 1, None, &mut admission).unwrap();
        let inert = PendingActivationScopeResult {
            operation: 1,
            scope_depth: 1,
            value: AwbcRuntimeValueSnapshot::from_admitted_runtime_value(&pending.value, &owner)
                .unwrap(),
        };
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        inert.admit(group, 1, 1, None, &mut admission).unwrap();
        for mutation in 0..5 {
            let mut forged = inert.clone();
            match mutation {
                0 => forged.operation = 0,
                1 => forged.scope_depth = 2,
                2 => {
                    forged.value = AwbcRuntimeValueSnapshot::from_admitted_runtime_value(
                        &RuntimeValue::Unit,
                        &owner,
                    )
                    .unwrap()
                }
                3 => {}
                4 => {}
                _ => unreachable!(),
            }
            let (pc, depth) = match mutation {
                3 => (0, 1),
                4 => (1, 0),
                _ => (1, 1),
            };
            let mut admission = RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                crate::entry::RuntimeSchemaLimits::engine_default(),
            );
            assert!(
                forged
                    .admit(group, pc, depth, None, &mut admission)
                    .is_err()
            );
            assert_eq!(pending.value, RuntimeValue::Bool(true));
        }
        let mut limits = crate::entry::RuntimeSchemaLimits::engine_default();
        limits.max_nodes = 0;
        let mut admission = RuntimeRetainedValueAdmission::for_program(owner, limits);
        assert!(inert.admit(group, 1, 1, None, &mut admission).is_err());
    }

    #[test]
    fn scope_result_is_discarded_on_failed_or_cancelled_scope_exit() {
        use crate::engine::{DialogueRuntimePhase, GenerationId};
        use crate::runtime_id::{RuntimeDialogueContentPlanId, RuntimePersistentFiberId};
        use std::num::NonZeroU32;
        let plan = plan();
        let engine = Engine::new_with_generation(plan.clone(), GenerationId::new(0));
        let group = RuntimeLineTaskGroupId::from_zero_based(0).unwrap();
        let unit = plan.line_task_groups()[group.index()].result_type();
        let id = DialogueActivationId::new(
            crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x95; 32]).unwrap(),
            RuntimePersistentFiberId::from_allocated(1),
            RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
            1,
        );
        for exit in [ScopeExit::Failed, ScopeExit::Cancelled] {
            let mut env = crate::value::RuntimeEnv::default();
            env.push_scope();
            let depth = env.scope_depth();
            let mut frame = DialogueActivationFrame {
                parent: crate::engine::FlowFiberId::default(),
                line: RuntimeLineId::from_source_entity_body("line.scope_result").unwrap(),
                content: id.content(),
                target: None,
                task_group: group,
                task_inputs: Box::new([]),
                execution: crate::engine::NativeFlowExecutionFrame::running(
                    crate::task::RuntimeProgramOwner::Plan(plan.clone()),
                    env,
                ),
                line_task: super::super::DialogueLineTaskState::NotStarted,
                elapsed: Default::default(),
                phase: DialogueRuntimePhase::Closing,
                result_target: RuntimeDialogueResultTarget::new(
                    unit,
                    RuntimePattern::from_admitted_parts(
                        unit,
                        crate::pattern::RuntimePatternKind::Discard,
                    ),
                ),
                voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
                values: Box::new([]),
                effect_callbacks: Box::new([]),
                activation_pc: 1,
                exiting_for_result: false,
                scopes: vec![super::super::DialogueActivationScope::new()],
                pending_scope_result: Some(PendingActivationScopeResult {
                    operation: 1,
                    scope_depth: 1,
                    value: RuntimeValue::Bool(true),
                }),
                pending_line_operation: None,
                pending_host_call: None,
                failure: None,
            };
            let mut line = NativeDialogueActivationState::new();
            assert!(matches!(
                engine
                    .prepare_activation_scope_exit(&id, &mut frame, &mut line, exit)
                    .unwrap(),
                ActivationScopeStep::Finished(_)
            ));
            assert!(frame.pending_scope_result.is_none());
            assert!(frame.scopes.is_empty());
            assert_eq!(frame.execution.env.scope_depth(), depth - 1);
            assert!(!line.has_pending_commands());
        }
    }
}
