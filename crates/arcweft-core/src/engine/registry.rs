//! Native reads and writes on the registry of the reached execution scope.

use super::audio::RuntimeEffectExecutionError;
use super::code_cursor::NativeSourceOperation;
use super::{Engine, FlowFiberId};
use crate::effect::{
    RuntimeEffectExpr, RuntimeEffectMaterializeError, RuntimeRegistryOwner,
    RuntimeRegistryReadKind, RuntimeRegistryStateUpdate, RuntimeRegistryWriteSource,
    RuntimeScopeRegistry, RuntimeScopeRegistryReadView,
};
use crate::plan::FlowOp;
use crate::pure::RuntimeCallBackend;
use crate::runtime_id::{DialogueActivationId, RuntimePlanTypeId};
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeEvalError, RuntimeRetainedValueAdmission, RuntimeScopeFunctionContext, RuntimeValue,
};
use arcweft_id::{EffectId, LifetimeRegistryKey, LifetimeScopeKind};

/// A read-only view of the original scopes while the activation transaction
/// owns the line and the execution frame has been temporarily switched.
#[derive(Debug, PartialEq)]
pub(super) struct NativeActivationRegistryReads {
    activation: DialogueActivationId,
    line: RuntimeScopeRegistryReadView,
    parent: Option<RuntimeScopeRegistryReadView>,
}

#[derive(Clone, Copy)]
pub(super) enum NativeDialogueRegistryTarget {
    Line,
    ActivationFlow,
    ParentFlow(FlowFiberId),
}

impl NativeDialogueRegistryTarget {
    pub(super) fn origin(
        self,
        activation: crate::runtime_id::DialogueActivationId,
    ) -> (RuntimeRegistryOwner, RuntimeRegistryWriteSource) {
        match self {
            Self::Line => (
                RuntimeRegistryOwner::Dialogue(activation),
                RuntimeRegistryWriteSource::NativeLine { activation },
            ),
            Self::ActivationFlow | Self::ParentFlow(_) => (
                RuntimeRegistryOwner::NativeFlow,
                RuntimeRegistryWriteSource::Native,
            ),
        }
    }
}

pub(super) struct NativeDialogueRegistryWrite {
    pub(super) target: NativeDialogueRegistryTarget,
    pub(super) write: crate::effect::RuntimePreparedRegistryWrite,
}

pub(super) enum NativeLineRegistryExecution<'a> {
    Child(&'a mut Option<NativeDialogueRegistryWrite>),
    Activation(NativeActivationRegistryExecution<'a>),
}

pub(super) struct NativeActivationRegistryExecution<'a> {
    pub(super) activation: crate::runtime_id::DialogueActivationId,
    pub(super) group: crate::runtime_id::RuntimeLineTaskGroupId,
    pub(super) rows: &'a crate::effect::RuntimeScopeRegistry,
    pub(super) parent: FlowFiberId,
    pub(super) parent_rows: Option<&'a crate::effect::RuntimeScopeRegistry>,
    pub(super) admission: &'a mut RuntimeRetainedValueAdmission,
    pub(super) pending: &'a mut Option<NativeDialogueRegistryWrite>,
}

struct NativeLineRegistryOperands<'plan> {
    key: &'plan arcweft_id::LifetimeRegistryKey,
    value: &'plan crate::value::RuntimeExpr,
    ty: crate::pattern::RuntimeSemanticTypeId,
    binding: RuntimeScopeFunctionContext,
}

impl Engine {
    pub(super) fn with_activation_registry_reads<T>(
        &mut self,
        activation: DialogueActivationId,
        parent: FlowFiberId,
        line: &RuntimeScopeRegistry,
        run: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let parent = self
            .dialogue_parent_flow_registry(parent, activation)
            .map(RuntimeScopeRegistry::read_view);
        let previous = self
            .activation_registry_reads
            .replace(NativeActivationRegistryReads {
                activation,
                line: line.read_view(),
                parent,
            });
        let result = run(self);
        self.activation_registry_reads = previous;
        result
    }

    pub(super) fn read_registry_value(
        &self,
        key: &LifetimeRegistryKey,
        kind: RuntimeRegistryReadKind,
        ty: RuntimePlanTypeId,
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
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner,
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let invalid = |cause| RuntimeEvalError::RegistryReadAdmission(Box::new(cause));
        if let Some(activation) = &self.activation_registry_reads {
            // During inline execution the activation environment is installed
            // temporarily. Its enclosing suspended frame is not the current
            // environment's owner, so admit the actual execution values and
            // the detached registry owners together.
            self.fiber
                .frame
                .env
                .admit_retained(&mut admission)
                .map_err(invalid)?;
            activation
                .line
                .admit_retained(
                    RuntimeRegistryOwner::Dialogue(activation.activation),
                    &mut admission,
                )
                .map_err(invalid)?;
            if let Some(parent) = &activation.parent {
                parent
                    .admit_retained(RuntimeRegistryOwner::NativeFlow, &mut admission)
                    .map_err(invalid)?;
            }
            let reached = match key.scope() {
                LifetimeScopeKind::Line => Some((
                    &activation.line,
                    RuntimeRegistryOwner::Dialogue(activation.activation),
                )),
                LifetimeScopeKind::Flow if self.fiber.frame.env.flow_registry().is_none() => {
                    activation
                        .parent
                        .as_ref()
                        .map(|parent| (parent, RuntimeRegistryOwner::NativeFlow))
                }
                _ => None,
            };
            if let Some((rows, owner)) = reached {
                return rows
                    .read(owner, key, kind, expected, binding, &mut admission)
                    .map_err(invalid);
            }
            if self.fiber.frame.env.flow_registry().is_none() {
                return Err(RuntimeEvalError::RegistryReadContextRequired);
            }
        } else {
            self.admit_live_retained_values(&mut admission)
                .map_err(invalid)?;
        }
        let activation = self.fiber.owner.dialogue_activation().copied();
        let (rows, reached_owner) = match key.scope() {
            LifetimeScopeKind::Flow => {
                let flow = self.fiber.frame.env.flow_registry().or_else(|| {
                    let activation = activation?;
                    let parent = self.dialogue_activations.active_parent(&activation)?;
                    self.dialogue_parent_flow_registry(parent, activation)
                });
                (flow, RuntimeRegistryOwner::NativeFlow)
            }
            LifetimeScopeKind::Line => {
                let activation = activation.ok_or(RuntimeEvalError::RegistryReadContextRequired)?;
                let line = self
                    .dialogue_activations
                    .active_line(&activation)
                    .ok_or(RuntimeEvalError::RegistryReadContextRequired)?;
                line.admit_retained_resources(Some(activation), &mut admission)
                    .map_err(invalid)?;
                (
                    line.line_registry().ok(),
                    RuntimeRegistryOwner::Dialogue(activation),
                )
            }
            _ => return Err(RuntimeEvalError::RegistryReadContextRequired),
        };
        rows.ok_or(RuntimeEvalError::RegistryReadContextRequired)?
            .read(reached_owner, key, kind, expected, binding, &mut admission)
            .map_err(invalid)
    }

    pub(super) fn dialogue_parent_flow_registry(
        &self,
        parent: FlowFiberId,
        activation: crate::runtime_id::DialogueActivationId,
    ) -> Option<&crate::effect::RuntimeScopeRegistry> {
        let owner = RuntimeProgramOwner::Plan(self.plan.clone());
        self.session_fibers()
            .find(|fiber| fiber.id == parent && fiber.persistent_id == activation.owner_fiber())
            .filter(|fiber| fiber.frame.program.same_program(&owner))?
            .frame
            .env
            .flow_registry()
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
            NativeDialogueRegistryTarget::ParentFlow(parent) => {
                let activation = *transaction.activation();
                if transaction.frame().parent != parent {
                    return Err(missing.into());
                }
                let owner = RuntimeProgramOwner::Plan(self.plan.clone());
                std::iter::once(&mut self.fiber)
                    .chain(&mut self.parked_main)
                    .chain(&mut self.child_fibers)
                    .find(|fiber| {
                        fiber.id == parent && fiber.persistent_id == activation.owner_fiber()
                    })
                    .filter(|fiber| fiber.frame.program.same_program(&owner))
                    .and_then(|fiber| fiber.frame.env.flow_registry_mut())
                    .ok_or_else(|| missing.into())
            }
        }
    }

    pub(super) fn commit_dialogue_registry_write(
        &mut self,
        transaction: &mut super::dialogue::DialogueActivationTransaction,
        pending: NativeDialogueRegistryWrite,
    ) -> Result<(), RuntimeEffectExecutionError> {
        let rows = self.dialogue_registry_rows_mut(transaction, pending.target)?;
        pending.write.reserve(rows)?;
        pending.write.commit(rows);
        Ok(())
    }

    pub(super) fn stage_borrowed_line_registry_write(
        &mut self,
        source: &NativeSourceOperation,
        execution: NativeActivationRegistryExecution<'_>,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), RuntimeEffectExecutionError> {
        use crate::effect::RuntimePreparedRegistryWrite;

        let plan = self.plan.clone();
        let operands = self.line_registry_operands(&plan, source, execution.group)?;
        let (target, rows) = match operands.key.scope() {
            LifetimeScopeKind::Line => (NativeDialogueRegistryTarget::Line, execution.rows),
            LifetimeScopeKind::Flow => match self.fiber.frame.env.flow_registry() {
                Some(rows) => (NativeDialogueRegistryTarget::ActivationFlow, rows),
                None => (
                    NativeDialogueRegistryTarget::ParentFlow(execution.parent),
                    execution
                        .parent_rows
                        .ok_or(RuntimeEffectMaterializeError::RegistryContextRequired)?,
                ),
            },
            _ => return Err(RuntimeEffectMaterializeError::RegistryContextRequired.into()),
        };
        let (owner, origin) = target.origin(execution.activation);
        let position = rows.prepare_write(
            owner,
            operands.key,
            operands.ty,
            origin,
            execution.admission,
        )?;
        let prepared = RuntimeRegistryStateUpdate::prepare(
            operands.key,
            operands.ty,
            operands.binding.instantiation(),
            execution.admission,
        )?;
        let value = self.evaluate_expr_with_backend(operands.value, backend)?;
        let update = prepared.capture(&value)?;
        *execution.pending = Some(NativeDialogueRegistryWrite {
            target,
            write: RuntimePreparedRegistryWrite::new(position, origin, update),
        });
        Ok(())
    }

    pub(super) fn stage_child_line_registry_write(
        &mut self,
        source: &NativeSourceOperation,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<NativeDialogueRegistryWrite, RuntimeEffectExecutionError> {
        use crate::effect::RuntimePreparedRegistryWrite;

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
                (
                    NativeDialogueRegistryTarget::ParentFlow(parent),
                    self.dialogue_parent_flow_registry(parent, activation)
                        .ok_or(missing.clone())?,
                )
            }
            _ => return Err(missing.into()),
        };
        let (image, mut admission) = self
            .inert_rollback_image_with_dialogue(None)
            .map_err(|message| RuntimeEffectExecutionError::Checkpoint { message })?;
        let (destination, source) = target.origin(activation);
        let position = rows.prepare_write(
            destination,
            operands.key,
            operands.ty,
            source,
            &mut admission,
        )?;
        let prepared = RuntimeRegistryStateUpdate::prepare(
            operands.key,
            operands.ty,
            operands.binding.instantiation(),
            &mut admission,
        )?;
        let evaluated = self.evaluate_expr_with_backend(operands.value, backend);
        match evaluated
            .map_err(RuntimeEffectExecutionError::from)
            .and_then(|value| prepared.capture(&value).map_err(Into::into))
        {
            Ok(update) => Ok(NativeDialogueRegistryWrite {
                target,
                write: RuntimePreparedRegistryWrite::new(position, source, update),
            }),
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
        let Some(FlowOp::EvaluatedEffect(RuntimeEffectExpr::RegistryWrite { key, value })) =
            source.operation(plan)
        else {
            return Err(missing.into());
        };
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
        let ty = plan
            .type_table()
            .get(value.ty())
            .ok_or(missing.clone())?
            .semantic_identity();
        Ok(NativeLineRegistryOperands {
            key,
            value,
            ty,
            binding,
        })
    }

    pub(super) fn execute_registry_write(
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
        let Some(FlowOp::EvaluatedEffect(RuntimeEffectExpr::RegistryWrite { key, value })) =
            source.operation(&plan)
        else {
            return Err(missing.into());
        };
        if key.scope() != &LifetimeScopeKind::Flow {
            return Err(missing.into());
        }
        let registry = self
            .fiber
            .frame
            .env
            .flow_registry()
            .ok_or(missing.clone())?;
        let ty = plan
            .type_table()
            .get(value.ty())
            .ok_or(missing.clone())?
            .semantic_identity();
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
        let position = registry.prepare_write(
            RuntimeRegistryOwner::NativeFlow,
            key,
            ty,
            RuntimeRegistryWriteSource::Native,
            &mut admission,
        )?;
        let prepared =
            RuntimeRegistryStateUpdate::prepare(key, ty, binding.instantiation(), &mut admission)?;
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
        self.fiber
            .frame
            .env
            .flow_registry_mut()
            .ok_or(missing.clone())?
            .reserve_write(position)?;
        let result = (|| {
            let value = self.evaluate_expr_with_backend(value, backend)?;
            let update = prepared.capture(&value)?;
            let registry = self.fiber.frame.env.flow_registry_mut().ok_or(missing)?;
            registry.commit(position, RuntimeRegistryWriteSource::Native, update);
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
mod tests {
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

    fn plan(authorized: bool) -> (RuntimePlan, FlowRuntimeId) {
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
        let (plan, flow) = plan(true);
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
        let (plan, flow) = plan(false);
        let mut engine = Engine::for_flow(plan, &flow).unwrap();
        let step = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert_eq!(step.stop_reason, crate::step::RuntimeStepStopReason::Failed);
        assert_eq!(step.stats.executed_ops, 1);
        assert!(engine.fiber.frame.env.flow_registry().unwrap().is_empty());
    }
}
