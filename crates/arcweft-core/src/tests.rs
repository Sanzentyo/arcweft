macro_rules! runtime_record {
    ([$(RuntimeFieldValue { name: $name:expr, value: $value:expr, }),* $(,)?]) => {
        $crate::value::RuntimeValue::try_record(vec![$(($name, $value)),*])
            .expect("test record fields are unique")
    };
}

pub(crate) use runtime_record;

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

pub(crate) fn pending_need(
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
    let mut registry = NeedProducerRegistry::default();
    let invocation = registry
        .begin_invocation(
            GenerationId::new(0),
            crate::runtime_id::RuntimePersistentFiberId::from_allocated(1),
            plan.site(),
        )
        .unwrap();
    let admission = registry.admit_start(invocation, plan, Vec::new()).unwrap();
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
