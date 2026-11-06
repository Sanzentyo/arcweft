//! Native reads and writes on the registry of the reached execution scope.

use super::audio::RuntimeEffectExecutionError;
use super::code_cursor::NativeSourceOperation;
use super::{Engine, FlowFiberId};
use crate::effect::{
    RuntimeDropPolicyExpr, RuntimeEffectExpr, RuntimeEffectMaterializeError,
    RuntimePreparedRegistryDrop, RuntimePreparedRegistryMutation, RuntimeRegistryOwner,
    RuntimeRegistryReadKind, RuntimeRegistryWritePreparation, RuntimeRegistryWriteSource,
    RuntimeScopeRegistry,
};
use crate::plan::FlowOp;
use crate::pure::RuntimeCallBackend;
use crate::runtime_id::{DialogueActivationId, RuntimePlanTypeId};
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeEvalError, RuntimeRetainedValueAdmission, RuntimeScopeFunctionContext, RuntimeValue,
};
use arcweft_id::{EffectId, LifetimeRegistryKey, LifetimeScopeKind};

#[derive(Clone, Copy)]
pub(super) enum NativeDialogueRegistryTarget {
    Line,
    ActivationFlow,
    ParentFlow {
        fiber: FlowFiberId,
        frame: NativeParentFlowFrame,
    },
}

/// The same frame can be in the execution slot or displaced by an inline
/// dialogue at different points of a prepared registry write.
#[derive(Clone, Copy)]
pub(super) enum NativeParentFlowFrame {
    Fiber,
    Inline(DialogueActivationId),
}

impl NativeDialogueRegistryTarget {
    pub(super) fn origin(
        self,
        activation: DialogueActivationId,
    ) -> (RuntimeRegistryOwner, RuntimeRegistryWriteSource) {
        match self {
            Self::Line => (
                RuntimeRegistryOwner::Dialogue(activation),
                RuntimeRegistryWriteSource::NativeLine { activation },
            ),
            Self::ActivationFlow | Self::ParentFlow { .. } => (
                RuntimeRegistryOwner::NativeFlow,
                RuntimeRegistryWriteSource::Native,
            ),
        }
    }
}

pub(super) struct NativeDialogueRegistryMutation {
    pub(super) target: NativeDialogueRegistryTarget,
    pub(super) mutation: RuntimePreparedRegistryMutation,
}

pub(super) enum NativeLineRegistryExecution<'a> {
    Child(&'a mut Option<NativeDialogueRegistryMutation>),
    Activation(NativeActivationRegistryExecution<'a>),
}

pub(super) struct NativeActivationRegistryExecution<'a> {
    pub(super) admission: &'a mut RuntimeRetainedValueAdmission,
    pub(super) pending: &'a mut Option<NativeDialogueRegistryMutation>,
}

struct NativeLineRegistryOperands<'plan> {
    key: &'plan arcweft_id::LifetimeRegistryKey,
    effect: &'plan RuntimeEffectExpr,
    binding: RuntimeScopeFunctionContext,
}

/// The original reached row and exact source operand are admitted together
/// before the owning Engine evaluates a mutation input.
pub(super) enum NativePreparedRegistryMutation<'source, 'budget> {
    Write {
        prepared: RuntimeRegistryWritePreparation<'source, 'budget>,
        input: &'source crate::value::RuntimeExpr,
    },
    Drop {
        prepared: RuntimePreparedRegistryDrop,
        fade: Option<&'source crate::value::RuntimeExpr>,
    },
}

impl<'source, 'budget> NativePreparedRegistryMutation<'source, 'budget> {
    pub(super) fn prepare(
        plan: &crate::plan::RuntimePlan,
        effect: &'source RuntimeEffectExpr,
        rows: &RuntimeScopeRegistry,
        owner: RuntimeRegistryOwner,
        origin: RuntimeRegistryWriteSource,
        binding: &'source RuntimeScopeFunctionContext,
        admission: &'budget mut RuntimeRetainedValueAdmission,
    ) -> Result<Self, RuntimeEffectExecutionError> {
        let missing = RuntimeEffectMaterializeError::RegistryContextRequired;
        match effect {
            RuntimeEffectExpr::RegistryWrite { key, value } => {
                let ty = plan
                    .type_table()
                    .get(value.ty())
                    .ok_or(missing)?
                    .semantic_identity();
                let prepared =
                    rows.prepare_write(owner, key, ty, binding.instantiation(), origin, admission)?;
                Ok(Self::Write {
                    prepared,
                    input: value,
                })
            }
            RuntimeEffectExpr::RegistryDrop { target, policy } => {
                let prepared = rows.prepare_drop(
                    owner,
                    target.key(),
                    target.presence(),
                    *target.item_type(),
                    binding.instantiation(),
                    admission,
                )?;
                let fade = match policy {
                    RuntimeDropPolicyExpr::Stop { fade } => Some(fade),
                    _ => None,
                };
                Ok(Self::Drop { prepared, fade })
            }
            _ => Err(missing.into()),
        }
    }

    pub(super) fn input(&self) -> Option<&'source crate::value::RuntimeExpr> {
        match self {
            Self::Write { input, .. } => Some(input),
            Self::Drop { fade, .. } => *fade,
        }
    }

    pub(super) fn finish(
        self,
        value: Option<RuntimeValue>,
    ) -> Result<RuntimePreparedRegistryMutation, RuntimeEffectExecutionError> {
        match self {
            Self::Write { prepared, .. } => Ok(RuntimePreparedRegistryMutation::Write(
                prepared.capture(&value.expect("the write has one input"))?,
            )),
            Self::Drop { prepared, fade } => {
                if fade.is_some() && !matches!(value, Some(RuntimeValue::Duration(_))) {
                    return Err(crate::line_task::LineRuntimeError::InvalidDropPolicy.into());
                }
                Ok(RuntimePreparedRegistryMutation::Drop(prepared))
            }
        }
    }
}

impl Engine {
    pub(super) fn read_registry_value(
        &self,
        key: &LifetimeRegistryKey,
        kind: RuntimeRegistryReadKind,
        ty: RuntimePlanTypeId,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let owner = RuntimeProgramOwner::Plan(self.plan.clone());
        if !self.fiber.frame.program.same_program(&owner) {
            return Err(RuntimeEvalError::RegistryReadContextRequired);
        }
        let expected = self
            .plan
            .type_table()
            .get(ty)
            .ok_or(RuntimeEvalError::RegistryReadContextRequired)?
            .semantic_identity();
        let binding = self.fiber.frame.env.function_instantiation();
        let inline = self.dialogue_activations.inline_stack.last();
        let activation = inline
            .map(|transaction| *transaction.activation())
            .or_else(|| self.fiber.owner.dialogue_activation().copied());
        let (rows, reached_owner) = match key.scope() {
            LifetimeScopeKind::Flow => {
                let flow = self.fiber.frame.env.flow_registry().or_else(|| {
                    let activation = activation?;
                    let parent = match inline {
                        Some(transaction) => transaction.frame().parent,
                        None => self.dialogue_activations.active_parent(&activation)?,
                    };
                    self.dialogue_parent_flow_registry(parent, activation)
                        .map(|(_, rows)| rows)
                });
                (flow, RuntimeRegistryOwner::NativeFlow)
            }
            LifetimeScopeKind::Line => {
                let activation = activation.ok_or(RuntimeEvalError::RegistryReadContextRequired)?;
                let line = match inline {
                    Some(transaction) => transaction.line(),
                    None => self
                        .dialogue_activations
                        .active_line(&activation)
                        .ok_or(RuntimeEvalError::RegistryReadContextRequired)?,
                };
                (
                    line.line_registry().ok(),
                    RuntimeRegistryOwner::Dialogue(activation),
                )
            }
            _ => return Err(RuntimeEvalError::RegistryReadContextRequired),
        };
        let rows = rows.ok_or(RuntimeEvalError::RegistryReadContextRequired)?;
        let mut admission = RuntimeRetainedValueAdmission::for_program(owner, limits);
        let invalid = |cause| RuntimeEvalError::RegistryReadAdmission(Box::new(cause));
        self.admit_live_fiber_values(&mut admission, None, None)
            .map_err(invalid)?;
        self.admit_live_dialogue_values(&mut admission)
            .map_err(invalid)?;
        admission.finish_presentation_custody().map_err(invalid)?;
        rows.read(reached_owner, key, kind, expected, binding, &mut admission)
            .map_err(invalid)
    }

    pub(super) fn dialogue_parent_flow_registry(
        &self,
        parent: FlowFiberId,
        activation: DialogueActivationId,
    ) -> Option<(
        NativeDialogueRegistryTarget,
        &crate::effect::RuntimeScopeRegistry,
    )> {
        let owner = RuntimeProgramOwner::Plan(self.plan.clone());
        let fiber = self
            .session_fibers()
            .find(|fiber| fiber.id == parent && fiber.persistent_id == activation.owner_fiber())?;
        let inline = self.inline_dialogue_execution();
        let candidates = inline
            .frames()
            .rev()
            .filter(|(id, _)| fiber.id == self.fiber.id && *id != activation)
            .map(|(id, frame)| (NativeParentFlowFrame::Inline(id), frame))
            .chain(std::iter::once((
                NativeParentFlowFrame::Fiber,
                self.session_frame(fiber, inline.suspended_frame()),
            )));
        candidates
            .filter(|(_, frame)| frame.program.same_program(&owner))
            .find_map(|(owner, frame)| {
                Some((
                    NativeDialogueRegistryTarget::ParentFlow {
                        fiber: parent,
                        frame: owner,
                    },
                    frame.env.flow_registry()?,
                ))
            })
    }

    fn dialogue_parent_flow_frame_mut(
        &mut self,
        parent: FlowFiberId,
        activation: DialogueActivationId,
        target: NativeParentFlowFrame,
    ) -> Option<&mut super::NativeFlowExecutionFrame> {
        let selected =
            self.fiber.id == parent && self.fiber.persistent_id == activation.owner_fiber();
        match target {
            NativeParentFlowFrame::Fiber if selected => {
                match self.dialogue_activations.inline_stack.first_mut() {
                    Some(first) => Some(&mut first.frame_mut().execution),
                    None => Some(&mut self.fiber.frame),
                }
            }
            NativeParentFlowFrame::Fiber => self
                .parked_main
                .iter_mut()
                .chain(&mut self.child_fibers)
                .find(|fiber| fiber.id == parent && fiber.persistent_id == activation.owner_fiber())
                .map(|fiber| &mut fiber.frame),
            NativeParentFlowFrame::Inline(owner) if selected && owner != activation => {
                let index = self
                    .dialogue_activations
                    .inline_stack
                    .iter()
                    .position(|row| *row.activation() == owner)?;
                match self.dialogue_activations.inline_stack.get_mut(index + 1) {
                    Some(next) => Some(&mut next.frame_mut().execution),
                    None => Some(&mut self.fiber.frame),
                }
            }
            NativeParentFlowFrame::Inline(_) => None,
        }
    }

    pub(super) fn dialogue_registry_rows_mut<'a>(
        &'a mut self,
        transaction: &'a mut super::dialogue::DialogueActivationTransaction,
        target: NativeDialogueRegistryTarget,
    ) -> Result<&'a mut crate::effect::RuntimeScopeRegistry, RuntimeEffectExecutionError> {
        let missing = RuntimeEffectMaterializeError::RegistryContextRequired;
        match target {
            NativeDialogueRegistryTarget::Line => Ok(transaction.line_mut().line_registry_mut()?),
            NativeDialogueRegistryTarget::ActivationFlow => transaction
                .frame_mut()
                .execution
                .env
                .flow_registry_mut()
                .ok_or_else(|| missing.into()),
            NativeDialogueRegistryTarget::ParentFlow { fiber, frame } => {
                let activation = *transaction.activation();
                if transaction.frame().parent != fiber {
                    return Err(missing.into());
                }
                let owner = RuntimeProgramOwner::Plan(self.plan.clone());
                self.dialogue_parent_flow_frame_mut(fiber, activation, frame)
                    .filter(|frame| frame.program.same_program(&owner))
                    .and_then(|frame| frame.env.flow_registry_mut())
                    .ok_or_else(|| missing.into())
            }
        }
    }

    pub(super) fn commit_dialogue_registry_mutation(
        &mut self,
        transaction: &mut super::dialogue::DialogueActivationTransaction,
        pending: NativeDialogueRegistryMutation,
    ) -> Result<(), RuntimeEffectExecutionError> {
        let rows = self.dialogue_registry_rows_mut(transaction, pending.target)?;
        pending.mutation.reserve(rows)?;
        pending.mutation.commit(rows)?;
        Ok(())
    }

    pub(super) fn stage_borrowed_line_registry_mutation(
        &mut self,
        source: &NativeSourceOperation,
        execution: NativeActivationRegistryExecution<'_>,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), RuntimeEffectExecutionError> {
        let plan = self.plan.clone();
        let missing = RuntimeEffectMaterializeError::RegistryContextRequired;
        let transaction = self
            .dialogue_activations
            .inline_stack
            .last()
            .ok_or(missing.clone())?;
        let activation = *transaction.activation();
        let parent = transaction.frame().parent;
        let operands =
            self.line_registry_operands(&plan, source, transaction.frame().task_group)?;
        let (target, rows) = match operands.key.scope() {
            LifetimeScopeKind::Line => (
                NativeDialogueRegistryTarget::Line,
                transaction.line().line_registry()?,
            ),
            LifetimeScopeKind::Flow => match self.fiber.frame.env.flow_registry() {
                Some(rows) => (NativeDialogueRegistryTarget::ActivationFlow, rows),
                None => self
                    .dialogue_parent_flow_registry(parent, activation)
                    .ok_or(missing.clone())?,
            },
            _ => return Err(RuntimeEffectMaterializeError::RegistryContextRequired.into()),
        };
        let (owner, origin) = target.origin(activation);
        let prepared = NativePreparedRegistryMutation::prepare(
            &plan,
            operands.effect,
            rows,
            owner,
            origin,
            &operands.binding,
            execution.admission,
        )?;
        let value = prepared
            .input()
            .map(|input| self.evaluate_expr_with_backend(input, backend))
            .transpose()?;
        *execution.pending = Some(NativeDialogueRegistryMutation {
            target,
            mutation: prepared.finish(value)?,
        });

        Ok(())
    }

    pub(super) fn stage_child_line_registry_mutation(
        &mut self,
        source: &NativeSourceOperation,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<NativeDialogueRegistryMutation, RuntimeEffectExecutionError> {
        let missing = RuntimeEffectMaterializeError::RegistryContextRequired;
        let super::FlowFiberOwner::LineTask(child) = &self.fiber.owner else {
            return Err(missing.into());
        };
        let activation = *child.tag.activation_id();
        let group = self
            .dialogue_activations
            .active_task_group(&activation)
            .ok_or(missing.clone())?;
        let plan = self.plan.clone();
        let owner = RuntimeProgramOwner::Plan(plan.clone());
        let operands = self.line_registry_operands(&plan, source, group)?;
        let (target, rows) = match operands.key.scope() {
            LifetimeScopeKind::Line => (
                NativeDialogueRegistryTarget::Line,
                self.dialogue_activations
                    .active_line(&activation)
                    .ok_or(missing.clone())?
                    .line_registry()?,
            ),
            LifetimeScopeKind::Flow => {
                let parent = self
                    .dialogue_activations
                    .active_parent(&activation)
                    .ok_or(missing.clone())?;
                self.dialogue_parent_flow_registry(parent, activation)
                    .ok_or(missing.clone())?
            }
            _ => return Err(missing.into()),
        };
        let (image, mut admission) = self
            .inert_rollback_image_with_dialogue(
                None,
                crate::entry::RuntimeSchemaLimits::engine_default(),
            )
            .map_err(|message| RuntimeEffectExecutionError::Checkpoint { message })?;
        let (destination, source) = target.origin(activation);
        let prepared = NativePreparedRegistryMutation::prepare(
            &plan,
            operands.effect,
            rows,
            destination,
            source,
            &operands.binding,
            &mut admission,
        )?;
        let evaluated = prepared
            .input()
            .map(|input| self.evaluate_expr_with_backend(input, backend))
            .transpose();
        match evaluated
            .map_err(RuntimeEffectExecutionError::from)
            .and_then(|value| prepared.finish(value))
        {
            Ok(mutation) => Ok(NativeDialogueRegistryMutation { target, mutation }),

            Err(cause) => {
                *self = Self::from_rollback_image(image, &owner).map_err(|message| {
                    RuntimeEffectExecutionError::Rollback {
                        cause: Box::new(cause.clone()),
                        message,
                    }
                })?;
                Err(cause)
            }
        }
    }

    fn line_registry_operands<'plan>(
        &self,
        plan: &'plan crate::plan::RuntimePlan,
        source: &NativeSourceOperation,
        group: crate::runtime_id::RuntimeLineTaskGroupId,
    ) -> Result<NativeLineRegistryOperands<'plan>, RuntimeEffectExecutionError> {
        use crate::plan::code::RuntimeCodeOwner;

        let missing = RuntimeEffectMaterializeError::RegistryContextRequired;
        let owner = RuntimeProgramOwner::Plan(self.plan.clone());
        if !self.fiber.frame.program.same_program(&owner) {
            return Err(missing.into());
        }
        let Some(FlowOp::EvaluatedEffect(effect)) = source.operation(plan) else {
            return Err(missing.into());
        };
        let key = effect.registry_mutation_key().ok_or(missing.clone())?;
        let binding = self.fiber.frame.env.source_function_context();
        let effects = match source.owner() {
            RuntimeCodeOwner::LineTask { group: source, .. } if source == group => plan
                .line_task_groups()
                .get(group.index())
                .ok_or(missing.clone())?
                .effects(),
            RuntimeCodeOwner::Function(site)
                if matches!(
                    &binding, RuntimeScopeFunctionContext::Function { site: active, .. } if *active == site
                ) =>
            {
                plan.function_sites()
                    .get(site)
                    .ok_or(missing.clone())?
                    .invocation_effects()
            }
            _ => return Err(missing.into()),
        };
        let required = EffectId::state_write(key);
        if !effects.iter().any(|effect| effect.covers(&required)) {
            return Err(RuntimeEffectMaterializeError::RegistryWriteNotAuthorized {
                effect: required,
            }
            .into());
        }
        Ok(NativeLineRegistryOperands {
            key,
            effect,
            binding,
        })
    }

    pub(super) fn execute_registry_mutation(
        &mut self,
        source: &NativeSourceOperation,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), RuntimeEffectExecutionError> {
        let plan = self.plan.clone();
        let owner = RuntimeProgramOwner::Plan(plan.clone());
        let missing = RuntimeEffectMaterializeError::RegistryContextRequired;
        if !self.fiber.frame.program.same_program(&owner) {
            return Err(missing.into());
        }
        let Some(FlowOp::EvaluatedEffect(effect)) = source.operation(&plan) else {
            return Err(missing.into());
        };
        let key = effect.registry_mutation_key().ok_or(missing.clone())?;
        if key.scope() != &LifetimeScopeKind::Flow {
            return Err(missing.into());
        }
        let registry = self
            .fiber
            .frame
            .env
            .flow_registry()
            .ok_or(missing.clone())?;
        let binding = self.fiber.frame.env.source_function_context();
        let RuntimeScopeFunctionContext::Function { site, .. } = &binding else {
            return Err(missing.into());
        };
        if source.owner() != crate::plan::code::RuntimeCodeOwner::Function(*site) {
            return Err(missing.into());
        }
        let declaration = plan.function_sites().get(*site).ok_or(missing.clone())?;
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        self.admit_live_retained_values(&mut admission)?;
        let prepared = NativePreparedRegistryMutation::prepare(
            &plan,
            effect,
            registry,
            RuntimeRegistryOwner::NativeFlow,
            RuntimeRegistryWriteSource::Native,
            &binding,
            &mut admission,
        )?;

        let required = EffectId::state_write(key);
        if !declaration
            .invocation_effects()
            .iter()
            .any(|effect| effect.covers(&required))
        {
            return Err(RuntimeEffectMaterializeError::RegistryWriteNotAuthorized {
                effect: required,
            }
            .into());
        }
        let image = self
            .inert_rollback_image()
            .map_err(|message| RuntimeEffectExecutionError::Checkpoint { message })?;
        let result = (|| {
            let value = prepared
                .input()
                .map(|input| self.evaluate_expr_with_backend(input, backend))
                .transpose()?;
            let mutation = prepared.finish(value)?;
            let registry = self.fiber.frame.env.flow_registry_mut().ok_or(missing)?;
            mutation.reserve(registry)?;
            mutation.commit(registry)?;
            Ok::<_, RuntimeEffectExecutionError>(())
        })();
        if let Err(cause) = result {
            *self = Self::from_rollback_image(image, &owner).map_err(|message| {
                RuntimeEffectExecutionError::Rollback {
                    cause: Box::new(cause.clone()),
                    message,
                }
            })?;
            return Err(cause);
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::pattern::RuntimeCheckedType;
    use crate::plan::{
        FlowRuntimeId, RuntimeEffectSet, RuntimeEvaluatedEffectSeed, RuntimeExecutableBodySeed,
        RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFlowSeed,
        RuntimeFunctionDefinitionIdentity, RuntimeFunctionSiteDeclarationSeed, RuntimePlan,
        RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
    };
    use crate::step::{RuntimeStepInput, RuntimeStepOptions};
    use crate::value::RuntimeValue;
    use arcweft_id::{DeclarationName, LifetimeRegistryKey};

    pub(in crate::engine) fn flow_registry_plan(authorized: bool) -> (RuntimePlan, FlowRuntimeId) {
        let flow = FlowRuntimeId::canonical("registry").unwrap();
        let key = LifetimeRegistryKey::new(
            LifetimeScopeKind::Flow,
            vec![DeclarationName::try_new("flag").unwrap()].into_boxed_slice(),
        );
        let boolean = RuntimeCheckedType::Bool.semantic_identity_digest();
        let value = |flag| {
            RuntimeExprSeed::new(
                boolean,
                RuntimeExprSeedKind::Value(RuntimeValue::Bool(flag)),
            )
        };
        let effects = if authorized {
            RuntimeEffectSet::try_from_effects([EffectId::state_write(&key)]).unwrap()
        } else {
            RuntimeEffectSet::empty()
        };
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    boolean,
                    RuntimePlanTypeProjection::Bool,
                )],
                [],
            )
            .unwrap();
        builder
            .push_flow_schema(crate::entry::RuntimeFlowSchema {
                flow: flow.clone(),
                parameters: vec![],
            })
            .unwrap();
        builder
            .push_flow_seed(RuntimeFlowSeed::new(
                flow.clone(),
                RuntimeFunctionSiteDeclarationSeed::flow(
                    RuntimeFunctionDefinitionIdentity::from_accepted_identity([53; 32]),
                    None,
                    Box::new([]),
                    boolean,
                    effects.clone(),
                ),
                RuntimeExecutableBodySeed {
                    effects,
                    ops: Box::new([
                        RuntimeFlowOpSeed::EvaluatedEffect(
                            RuntimeEvaluatedEffectSeed::RegistryWrite {
                                key: key.clone(),
                                value: value(true),
                            },
                        ),
                        RuntimeFlowOpSeed::EvaluatedEffect(
                            RuntimeEvaluatedEffectSeed::RegistryWrite {
                                key: key.clone(),
                                value: value(false),
                            },
                        ),
                        RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                            boolean,
                            RuntimeExprSeedKind::RegistryRead {
                                key,
                                kind: RuntimeRegistryReadKind::Required,
                            },
                        )),
                    ]),
                },
            ))
            .unwrap();
        (builder.finish().unwrap(), flow)
    }

    #[test]
    fn native_flow_registry_retains_latest_value_across_rollback_and_retires_on_return() {
        let (plan, flow) = flow_registry_plan(true);
        let mut engine = Engine::for_flow(plan, &flow).unwrap();
        let owner = RuntimeProgramOwner::Plan(engine.plan.clone());
        for flag in [true, false] {
            let step = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
            assert_eq!(step.stats.executed_ops, 1);
            assert!(step.output.diagnostics.is_empty(), "{step:?}");
            let image = engine.inert_rollback_image().unwrap();
            engine = Engine::from_rollback_image(image, &owner).unwrap();
            let entries =
                serde_json::to_value(engine.fiber.frame.env.flow_registry().unwrap()).unwrap();
            assert_eq!(entries.as_array().unwrap().len(), 1);
            assert_eq!(entries[0]["row"]["value"]["Bool"], flag);
        }
        let step = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert_eq!(step.stop_reason, crate::step::RuntimeStepStopReason::Done);
        assert_eq!(
            engine.fiber.frame.status,
            crate::engine::FlowFiberStatus::Done(crate::engine::FlowExit::Return("false".into()))
        );
        assert!(engine.fiber.frame.env.flow_registry().is_none());
    }

    #[test]
    fn native_flow_registry_requires_the_original_function_effect() {
        let (plan, flow) = flow_registry_plan(false);
        let mut engine = Engine::for_flow(plan, &flow).unwrap();
        let step = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert_eq!(step.stop_reason, crate::step::RuntimeStepStopReason::Failed);
        assert_eq!(step.stats.executed_ops, 1);
        assert!(engine.fiber.frame.env.flow_registry().unwrap().is_empty());
    }
}
