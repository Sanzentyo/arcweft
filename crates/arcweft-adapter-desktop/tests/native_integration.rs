use arcweft_core::{
    pattern::{RuntimeBuiltinVariantCaseIdentity, RuntimeCheckedType},
    task::{
        CancelScopeId, HostTaskRequest, TaskClass, TaskOutcomeContract, TaskPolicy, TaskPriority,
        TaskSpec,
    },
    value::RuntimeValue,
};
#[cfg(target_os = "windows")]
use arcweft_desktop_contract::PlatformKind;
use arcweft_desktop_contract::{DesktopFeature, DesktopResponse, SupportLevel};
use arcweft_host_adapter::{
    HostAdapterRegistry, HostTaskCompletion, HostTaskSubmission, HostTaskSubmissionContext,
};

#[test]
fn native_desktop_capabilities_complete_through_host_registry() {
    let adapter_set = arcweft_adapter_desktop::DesktopAdapterSet::bind_current_thread(
        arcweft_desktop_native::NativeDesktopBackend::builder().build(),
    );
    let (builder, coordinator) = adapter_set
        .register(HostAdapterRegistry::builder())
        .expect("desktop host calls are uniquely owned");
    let registry = builder.build();
    let task = task("desktop.platform", "capabilities");
    let mut journal = arcweft_core::task::TaskAdmissionJournal::default();
    let handle = journal.ensure_task(task).expect("task admission");
    let task = arcweft_core::task::BoundTaskSpec::bind(
        journal.submission(handle).unwrap(),
        None,
        arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
    )
    .expect("standalone bound receipt");

    let submission = registry
        .submit(
            &task,
            HostTaskSubmissionContext::new(
                arcweft_core::task::TaskDispatchIdentity::new(
                    task.handle().correlation,
                    arcweft_core::task::LogicalEpoch(1),
                    arcweft_core::task::TaskSequence(1),
                ),
                arcweft_core::task::TaskPublicationRevision::FIRST,
            ),
        )
        .expect("desktop platform adapter owns capabilities");
    let HostTaskSubmission::Completed(outcome) = submission else {
        panic!("capabilities should complete without a window pump");
    };
    let HostTaskCompletion::Ready(payload) = outcome.completion else {
        panic!("capabilities request succeeds");
    };
    let Some((RuntimeBuiltinVariantCaseIdentity::ResultOk, Some(payload))) =
        payload.value().builtin_variant_case()
    else {
        panic!("desktop response is a Result::Ok payload");
    };
    let RuntimeValue::String(payload) = payload else {
        panic!("desktop response payload is JSON text");
    };
    let response: DesktopResponse =
        serde_json::from_str(payload).expect("desktop response is JSON");
    let DesktopResponse::Capabilities(capabilities) = response else {
        panic!("expected capabilities response");
    };

    #[cfg(target_os = "windows")]
    assert_eq!(capabilities.platform, PlatformKind::Windows);
    assert_eq!(coordinator.pending_count(), 0);
    assert_eq!(
        capabilities
            .support(DesktopFeature::PersistentFileGrant)
            .map(|support| support.level),
        Some(SupportLevel::Unsupported)
    );
}

fn task(capability: &str, operation: &str) -> TaskSpec {
    use arcweft_core::task::{
        GenerationId, NeedProducerContractDigest, NeedProducerFamily, NeedProducerInstance,
        NeedProducerSiteDigest, NeedProducerSpec, RuntimeTypeSemanticDigest,
        TaskPlanSemanticDigest,
    };
    let outcome = TaskOutcomeContract::new(RuntimeCheckedType::Result {
        ok: Box::new(RuntimeCheckedType::String),
        error: Box::new(RuntimeCheckedType::String),
    });
    let producer = NeedProducerSpec::new(
        NeedProducerFamily::HostAdapterTask,
        NeedProducerContractDigest::from_bytes([1; 32]),
        TaskPlanSemanticDigest::from_bytes([2; 32]),
        NeedProducerSiteDigest::from_bytes([3; 32]),
        RuntimeTypeSemanticDigest::from_bytes(*outcome.payload_semantic_identity().as_bytes()),
        RuntimeValue::Tuple(vec![]).try_digest(1024).unwrap(),
    );
    TaskSpec {
        generation: GenerationId::new(1),
        producer: NeedProducerInstance::try_from(&producer).unwrap(),
        class: TaskClass::Background,
        priority: TaskPriority(0),
        cancel_scope: CancelScopeId("desktop-test".into()),
        policy: TaskPolicy::JoinSameKey,
        outcome,
        request: HostTaskRequest::custom(capability, operation, []),
        debug_label: format!("{capability}.{operation}"),
    }
}
