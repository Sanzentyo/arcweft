//! An ordinary LocalOnly Program Need captures its channel exactly once.
use super::*;
use arcweft_core::plan::{RuntimeProgramTaskCapture, RuntimeProgramTaskTemplate};
use arcweft_core::task::{
    CancelScopeId, NeedProducerContractDigest, NeedProducerDefinition, NeedProducerFamily,
    ProgramTaskDelivery, RuntimeTypeSemanticDigest, TaskClass, TaskHandleType, TaskPolicy,
    TaskPriority,
};

impl FinalFlowLowerer<'_> {
    pub(super) fn lower_action_receive_value(
        &mut self,
        owner: ExprId,
        call: &RuntimeResolvedCall,
        continuation: RuntimeFlowValueContinuation,
        overrides: RuntimeExpressionOverrides,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let (channel_type, payload) = call.action_receive().ok_or_else(|| {
            RuntimePlanLowerError::new("Action receive has no checked channel contract")
        })?;
        let (channel, result) = self
            .control
            .action_receivers
            .get(&owner)
            .cloned()
            .ok_or_else(|| RuntimePlanLowerError::new("Action receive has no admitted locals"))?;
        let need = call
            .need_producer()
            .expect("admitted Program producer")
            .need_type();
        let need_local = self
            .control
            .expression_values
            .get(&owner)
            .cloned()
            .ok_or_else(|| {
                RuntimePlanLowerError::new("Action receive has no admitted Need local")
            })?;
        let admission = self.semantic_facts.call_producer(owner).ok_or_else(|| {
            RuntimePlanLowerError::new("Action receive has no original checked producer definition")
        })?;
        let function = self
            .semantic_facts
            .expression_coordinate(owner)
            .ok_or_else(|| {
                RuntimePlanLowerError::new("Action receive has no accepted function coordinate")
            })?
            .runtime_function_definition()
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        let operand = &call.operands()[0];
        let source = self
            .expr_lowerer()
            .with_overrides(overrides)
            .lower_scalar_operand_source(operand.source(), channel_type)
            .map_err(RuntimePlanLowerError::new)?;
        let mut digest = blake3::Hasher::new();
        digest.update(b"arcweft.action.receive.producer-contract.v1\0");
        digest.update(channel_type.identity().as_bytes());
        digest.update(payload.identity().as_bytes());
        let plan = self
            .task_plans
            .reserve_original_source()
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        let producer = NeedProducerDefinition {
            function,
            family: NeedProducerFamily::SelectedCallable,
            contract: NeedProducerContractDigest::from_bytes(*digest.finalize().as_bytes()),
            plan,
            producer_site: admission.site(),
            handle_type: TaskHandleType::Need {
                payload: RuntimeTypeSemanticDigest::from_bytes(*payload.identity().as_bytes()),
            },
            class: TaskClass::Cpu,
            priority: TaskPriority(0),
            cancel_scope: CancelScopeId("flow".into()),
            policy: TaskPolicy::AlwaysStart,
            debug_label: "source Action receive".into(),
        };
        let mut ops = vec![
            RuntimeFlowOpSeed::Let {
                pattern: bind_seed(channel_type, channel.clone()),
                expr: source,
            },
            RuntimeFlowOpSeed::ProgramTask {
                name: None,
                binding: bind_seed(need, need_local.clone()),
                template: RuntimeProgramTaskTemplate {
                    producer,
                    delivery: ProgramTaskDelivery::LocalOnly,
                    effects: RuntimeEffectSet::try_from_effects([
                        arcweft_id::EffectId::action_receive(),
                    ])
                    .expect("one engine effect"),
                    captures: vec![RuntimeProgramTaskCapture::copy(channel.clone())],
                    body: arcweft_core::plan::RuntimeProgramTaskBody::Flow(vec![
                        RuntimeFlowOpSeed::ReceiveAction {
                            channel,
                            result: result.clone(),
                        },
                        RuntimeFlowOpSeed::ReturnExpr(local_seed(
                            payload,
                            result,
                            RuntimeLocalReadMode::Move,
                        )),
                    ]),
                },
            },
        ];
        ops.extend(self.apply_value_continuation(
            local_seed(need, need_local, RuntimeLocalReadMode::Move),
            continuation,
        )?);
        Ok(ops)
    }
}
