macro_rules! runtime_record {
    ([$(RuntimeFieldValue { name: $name:expr, value: $value:expr, }),* $(,)?]) => {
        $crate::value::RuntimeValue::try_record(vec![$(($name, $value)),*])
            .expect("test record fields are unique")
    };
}

pub(crate) use runtime_record;

pub(crate) fn host_task_control(
    builder: &mut crate::plan::RuntimePlanBuilder,
    output: crate::pattern::RuntimeSemanticTypeId,
) -> crate::plan::RuntimeControlEffectContractSeedId {
    use crate::plan::*;
    builder
        .push_control_effect_contract(RuntimeControlEffectContractDefinition {
            mode: RuntimeTaskControlMode::MustSuspend,
            effects: Box::new([RuntimeControlEffectRow {
                kind: RuntimeControlEffectKind::HostOperation,
                identity: None,
                inputs: Box::new([]),
                output: Some(output),
                cardinality: RuntimeControlEffectCardinality::ExactlyOnce,
                ordering: RuntimeControlEffectOrdering::SingleTerminal,
                cancellation: RuntimeControlEffectCancellation::NotObserved,
                terminal: RuntimeControlEffectTerminalBehavior::Value,
            }]),
            children: Box::new([]),
        })
        .unwrap()
}

/// A real reusable descriptor whose distinct identity comes from its argument.
/// The fixture supplies a complete producer/spec rather than a caller-made ID.
pub(crate) fn reusable_need(argument: &str) -> crate::task::RuntimeNeedHandle {
    use crate::task::*;
    let outcome = TaskOutcomeContract::new(crate::pattern::RuntimeCheckedType::String);
    reusable_need_with_outcome(argument, outcome)
}

pub(crate) fn reusable_need_with_outcome(
    argument: &str,
    outcome: crate::task::TaskOutcomeContract,
) -> crate::task::RuntimeNeedHandle {
    use crate::task::*;
    let value = crate::value::RuntimeValue::String(argument.to_owned());
    let arguments = crate::value::RuntimeValue::Tuple(vec![value.clone()]);
    let producer = NeedProducerSpec::new(
        NeedProducerFamily::HostAdapterTask,
        NeedProducerContractDigest::from_bytes([1; 32]),
        TaskPlanSemanticDigest::from_bytes([2; 32]),
        NeedProducerSiteDigest::from_bytes([3; 32]),
        RuntimeTypeSemanticDigest::from_bytes(*outcome.payload_semantic_identity().as_bytes()),
        arguments
            .try_digest(4096)
            .expect("fixture arguments encode"),
    );
    RuntimeNeedHandle::try_from(TaskSpec {
        generation: GenerationId::new(1),
        producer: NeedProducerInstance::try_from(&producer).expect("fixture producer is checked"),
        class: TaskClass::Cpu,
        priority: TaskPriority(0),
        cancel_scope: CancelScopeId("fixture".to_owned()),
        policy: TaskPolicy::JoinSameKey,
        outcome,
        request: HostTaskRequest::custom("fixture", "run", [value.into()]),
        debug_label: "reusable fixture".to_owned(),
    })
    .expect("fixture descriptor has one exact complete correlation")
}

pub(crate) fn task_spec(
    outcome: crate::task::TaskOutcomeContract,
    request: crate::task::HostTaskRequest,
) -> crate::task::TaskSpec {
    use crate::task::*;
    let arguments = request.runtime_values().collect::<Vec<_>>();
    let arguments = crate::entry::schema::canonical_runtime_value_view_digest(
        crate::value::RuntimeValueView::Tuple(crate::value::RuntimeTupleView::Borrowed(&arguments)),
        4096,
    )
    .expect("fixture request arguments encode");
    let producer = NeedProducerSpec::new(
        NeedProducerFamily::HostAdapterTask,
        NeedProducerContractDigest::from_bytes([1; 32]),
        TaskPlanSemanticDigest::from_bytes([2; 32]),
        NeedProducerSiteDigest::from_bytes([3; 32]),
        RuntimeTypeSemanticDigest::from_bytes(*outcome.payload_semantic_identity().as_bytes()),
        arguments,
    );
    TaskSpec {
        generation: GenerationId::new(1),
        producer: NeedProducerInstance::try_from(&producer).expect("fixture producer is checked"),
        class: request.task_class(),
        priority: TaskPriority(0),
        cancel_scope: CancelScopeId("fixture".to_owned()),
        policy: TaskPolicy::JoinSameKey,
        outcome,
        debug_label: request.debug_label(),
        request,
    }
}

pub(crate) fn task_submission(spec: crate::task::TaskSpec) -> crate::task::TaskSubmission {
    let mut journal = crate::task::TaskAdmissionJournal::default();
    let handle = journal
        .ensure_task(spec)
        .expect("fixture start is admitted");
    journal
        .submission(handle)
        .expect("fixture retains its accepted receipt")
}

/// Builds a real original selected declaration, including its complete Task
/// slot, using the caller's actual payload/type graph.
pub(crate) fn selected_producer_plan(
    mut builder: crate::plan::RuntimePlanBuilder,
    metadata: crate::task::NeedProducerTaskPlan,
    arguments: Vec<crate::task::RuntimeHostArgumentTemplate<crate::plan::RuntimeExprSeed>>,
) -> std::sync::Arc<crate::plan::RuntimePlan> {
    use crate::plan::*;
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let need = crate::pattern::RuntimeSemanticTypeId::from_bytes([0xf6; 32]);
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    need,
                    RuntimePlanTypeProjection::Need(metadata.payload_type()),
                ),
            ],
            [],
        )
        .unwrap();
    let function = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xf7; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs: Box::new([]),
            result: unit,
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    let task_plan = builder
        .task_plan_construction()
        .reserve_original_source()
        .unwrap();
    builder
        .define_function_site_seed(
            &function,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([
                    RuntimeFlowOpSeed::StartNeedProducer {
                        binding: RuntimePatternSeed::new(need, RuntimePatternSeedKind::Discard),
                        target: RuntimeNeedProducerStartTargetSeed {
                            plan: metadata,
                            task_plan,
                            arguments,
                        },
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        unit,
                        RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Unit),
                    )),
                ]),
            }),
        )
        .unwrap();
    std::sync::Arc::new(builder.finish().unwrap())
}

pub(crate) fn selected_producer_start(
    plan: &std::sync::Arc<crate::plan::RuntimePlan>,
    generation: crate::task::GenerationId,
    arguments: Vec<crate::task::NeedProducerRuntimeArgument>,
) -> crate::task::RuntimeNeedProducerStart {
    let crate::plan::FlowOp::StartNeedProducer { target, .. } = &plan
        .function_sites()
        .iter()
        .next()
        .unwrap()
        .body()
        .executable()
        .unwrap()
        .ops()[0]
    else {
        panic!("fixture must retain its actual selected declaration")
    };
    let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
        crate::task::RuntimeProgramOwner::Plan(std::sync::Arc::clone(plan)),
        crate::entry::RuntimeSchemaLimits::engine_default(),
    );
    let limits = crate::plan::RuntimeTaskPlanSealLimits::default();
    let mut meter = admission.task_source_meter(limits).unwrap();
    let source = plan
        .task_source(target.task_plan(), limits, &mut meter)
        .unwrap();
    source
        .selected_start(
            &crate::plan::task_source::RuntimeTaskSourceProgram::native(std::sync::Arc::clone(
                plan,
            )),
            generation,
            arguments,
            None,
            &mut admission,
        )
        .unwrap()
}

pub(crate) fn pending_string_need(
    payload: crate::pattern::RuntimeSemanticTypeId,
) -> (
    crate::task::NeedProducerRegistry,
    crate::task::RuntimeNeedHandle,
) {
    use crate::task::*;
    let plan = NeedProducerTaskPlan::try_new(
        NeedProducerContractDigest::from_bytes([7; 32]),
        NeedProducerSiteDigest::from_bytes([8; 32]),
        NeedProducerRequestProjection::ExternCapability {
            capability: HostCapabilityId("fixture".to_owned()),
            operation: "pending".to_owned(),
            contract: crate::step::HostCallContractDigest::from_bytes([7; 32]),
            argument_names: Box::new([]),
        },
        Box::new([]),
        payload,
        TaskPolicy::JoinSameKey,
        HostRestartPolicy::Restartable,
        TaskClass::Io,
        TaskPriority(0),
        CancelScopeId("fixture".to_owned()),
    )
    .expect("fixture selected producer admits");
    let mut builder = crate::plan::RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [crate::plan::RuntimePlanTypeSeed::new(
                payload,
                crate::plan::RuntimePlanTypeProjection::String,
            )],
            [],
        )
        .unwrap();
    let source = selected_producer_plan(builder, plan, Vec::new());
    let start = selected_producer_start(&source, GenerationId::new(0), Vec::new());
    let mut registry = NeedProducerRegistry::default();
    let invocation = registry
        .begin_invocation(
            GenerationId::new(0),
            crate::runtime_id::RuntimePersistentFiberId::from_allocated(1),
            start.producer().producer_site(),
        )
        .unwrap();
    let admission = registry.admit_start(invocation, start).unwrap();
    let handle =
        crate::task::RuntimeNeedHandle::try_from(registry.submission_for_admission(&admission))
            .unwrap();
    (registry, handle)
}

pub(crate) fn host_producer() -> crate::task::HostCallProducerDefinition {
    crate::task::HostCallProducerDefinition {
        contract: crate::task::NeedProducerContractDigest::from_bytes([1; 32]),
        plan: crate::task::TaskPlanSemanticDigest::from_bytes([2; 32]),
        site: crate::task::NeedProducerSiteDigest::from_bytes([3; 32]),
    }
}

mod affine_intrinsics;
mod flow;
pub(crate) mod function_application;
mod line_task_reducer;
pub(crate) mod program_custody;
mod pure;
mod step_stats_delta;
mod stream;
mod task;
mod value;
