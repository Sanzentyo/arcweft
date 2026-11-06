//! Native registry writes at the original executable operation.

use super::Engine;
use super::audio::RuntimeEffectExecutionError;
use super::code_cursor::NativeSourceOperation;
use crate::effect::{
    RuntimeEffectExpr, RuntimeEffectMaterializeError, RuntimeRegistryStateUpdate,
    RuntimeRegistryWriteSource,
};
use crate::plan::FlowOp;
use crate::pure::RuntimeCallBackend;
use crate::task::RuntimeProgramOwner;
use crate::value::{RuntimeRetainedValueAdmission, RuntimeScopeFunctionContext};
use arcweft_id::{EffectId, LifetimeScopeKind};

impl Engine {
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
        let position =
            registry.prepare_write(key, RuntimeRegistryWriteSource::Native, &mut admission)?;
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
                                key,
                                value: value(false),
                            },
                        ),
                        RuntimeFlowOpSeed::ReturnExpr(value(false)),
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
