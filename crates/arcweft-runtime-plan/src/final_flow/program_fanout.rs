//! Source parallel owns a Traversal and starts its original Program Need.
//! Each worker invokes the actual retained function and awaits its result.
use super::*;
use arcweft_core::plan::{
    RuntimeCallArgumentSeed, RuntimeGeneratedFunctionRole, RuntimeProgramFanout,
    RuntimeProgramTaskBody, RuntimeProgramTaskCapture, RuntimeProgramTaskTemplate,
};
use arcweft_core::task::{
    CancelScopeId, NeedProducerContractDigest, NeedProducerDefinition, NeedProducerFamily,
    NeedProducerSiteDigest, ProgramTaskDelivery, RuntimeTypeSemanticDigest, TaskClass,
    TaskHandleType, TaskPolicy, TaskPriority,
};

impl FinalFlowLowerer<'_> {
    pub(super) fn lower_program_fanout_value(
        &mut self,
        owner: ExprId,
        call: &RuntimeResolvedCall,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let contract = call.program_fanout().ok_or_else(|| {
            RuntimePlanLowerError::new("Program fanout has no exact checked call contract")
        })?;
        let locals = self.control.fanouts.get(&owner).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new("Program fanout has no admitted capture and worker locals")
        })?;
        let result_local = self
            .control
            .expression_values
            .get(&owner)
            .cloned()
            .ok_or_else(|| {
                RuntimePlanLowerError::new("Program fanout has no admitted Need result local")
            })?;
        let admission = self.semantic_facts.call_producer(owner).ok_or_else(|| {
            RuntimePlanLowerError::new("Program fanout has no original checked producer definition")
        })?;
        let definition = self
            .semantic_facts
            .expression_coordinate(owner)
            .ok_or_else(|| {
                RuntimePlanLowerError::new("Program fanout has no accepted function coordinate")
            })?
            .runtime_function_definition()
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        let lowerer = self.expr_lowerer().with_overrides(overrides);
        let [receiver, limit] = call.operands() else {
            return Err(RuntimePlanLowerError::new(
                "Program fanout lost its admitted operand row",
            ));
        };
        let source = lowerer
            .lower_scalar_operand_source(receiver.source(), receiver.ty())
            .map_err(RuntimePlanLowerError::new)?;
        let limit_value = lowerer
            .lower_scalar_operand_source(limit.source(), limit.ty())
            .map_err(RuntimePlanLowerError::new)?;
        let mut digest = blake3::Hasher::new();
        digest.update(b"arcweft.parallel.producer-contract.v1\0");
        digest.update(contract.source.identity().as_bytes());
        digest.update(contract.mapping.identity().as_bytes());
        digest.update(contract.aggregate.identity().as_bytes());
        let contract_digest = NeedProducerContractDigest::from_bytes(*digest.finalize().as_bytes());
        let mut worker_site = blake3::Hasher::new();
        worker_site.update(b"arcweft.parallel.worker-site.v1\0");
        worker_site.update(admission.site().as_bytes());
        let parent_coordinate = self
            .task_plans
            .reserve_original_source()
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        let worker_coordinate = self
            .task_plans
            .reserve_original_source()
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        let base = NeedProducerDefinition {
            function: definition,
            family: NeedProducerFamily::AwaitManyBase,
            contract: contract_digest,
            plan: parent_coordinate,
            producer_site: admission.site(),
            handle_type: TaskHandleType::Need {
                payload: RuntimeTypeSemanticDigest::from_bytes(
                    *contract.aggregate.identity().as_bytes(),
                ),
            },
            class: TaskClass::Cpu,
            priority: TaskPriority(0),
            cancel_scope: CancelScopeId("flow".into()),
            policy: TaskPolicy::AlwaysStart,
            debug_label: "source parallel".into(),
        };
        let worker = NeedProducerDefinition {
            function: definition.generated_child(RuntimeGeneratedFunctionRole::FanoutWorker),
            family: NeedProducerFamily::AwaitManyChild,
            contract: contract_digest,
            plan: worker_coordinate,
            producer_site: NeedProducerSiteDigest::from_bytes(*worker_site.finalize().as_bytes()),
            handle_type: TaskHandleType::Need {
                payload: RuntimeTypeSemanticDigest::from_bytes(
                    *contract.result.identity().as_bytes(),
                ),
            },
            class: TaskClass::Cpu,
            priority: TaskPriority(0),
            cancel_scope: CancelScopeId("flow".into()),
            policy: TaskPolicy::AlwaysStart,
            debug_label: "source parallel worker".into(),
        };
        let worker_body = vec![
            RuntimeFlowOpSeed::ApplyFunction {
                callee: local_seed(
                    contract.mapping,
                    locals.callable.clone(),
                    RuntimeLocalReadMode::Move,
                ),
                args: Box::new([RuntimeCallArgumentSeed::new(
                    local_seed(
                        contract.item,
                        locals.item.clone(),
                        RuntimeLocalReadMode::Move,
                    ),
                    RuntimeCallArgumentMode::Value,
                    0,
                )]),
                result: bind_seed(contract.need, locals.need.clone()),
            },
            RuntimeFlowOpSeed::Await {
                binding: Some(bind_seed(contract.result, locals.result.clone())),
                target: arcweft_core::plan::RuntimeAwaitTargetSeed {
                    source: local_seed(contract.need, locals.need, RuntimeLocalReadMode::Move),
                },
                observers: vec![],
            },
            RuntimeFlowOpSeed::ReturnExpr(local_seed(
                contract.result,
                locals.result,
                RuntimeLocalReadMode::Move,
            )),
        ];
        let mut ops = vec![
            RuntimeFlowOpSeed::Let {
                pattern: RuntimePatternSeed::new(
                    receiver.ty().identity(),
                    RuntimePatternSeedKind::Tuple(Box::new([
                        bind_seed(contract.source, locals.source.clone()),
                        bind_seed(contract.mapping, locals.callable.clone()),
                    ])),
                ),
                expr: source,
            },
            RuntimeFlowOpSeed::Let {
                pattern: bind_seed(contract.limit, locals.limit.clone()),
                expr: limit_value,
            },
            RuntimeFlowOpSeed::ProgramTask {
                name: None,
                binding: bind_seed(
                    call.need_producer()
                        .expect("admitted Program producer")
                        .need_type(),
                    result_local.clone(),
                ),
                template: RuntimeProgramTaskTemplate {
                    producer: base,
                    delivery: ProgramTaskDelivery::LocalOnly,
                    captures: vec![
                        RuntimeProgramTaskCapture::moved(locals.source.clone()),
                        RuntimeProgramTaskCapture::copy(locals.limit.clone()),
                        RuntimeProgramTaskCapture::moved(locals.callable.clone()),
                    ],
                    body: RuntimeProgramTaskBody::Fanout(Box::new(RuntimeProgramFanout {
                        source: locals.source,
                        limit: locals.limit,
                        item: locals.item.clone(),
                        worker: RuntimeProgramTaskTemplate {
                            producer: worker,
                            delivery: ProgramTaskDelivery::LocalOnly,
                            captures: vec![
                                RuntimeProgramTaskCapture::moved(locals.callable),
                                RuntimeProgramTaskCapture::moved(locals.item),
                            ],
                            body: worker_body,
                        },
                    })),
                },
            },
        ];
        ops.extend(
            self.apply_value_continuation(
                local_seed(
                    call.need_producer()
                        .expect("admitted Program producer")
                        .need_type(),
                    result_local,
                    RuntimeLocalReadMode::Move,
                ),
                continuation,
            )?,
        );
        Ok(ops)
    }
}
