use arcweft_core::{
    pattern::{RuntimeCheckedType, RuntimeVariantIdentity},
    task::{
        CancelScopeId, HostTaskRequest, TaskClass, TaskId, TaskKey, TaskOutcomeContract,
        TaskPolicy, TaskPriority, TaskSpec,
    },
    value::RuntimeValue,
};
#[cfg(target_os = "windows")]
use arcweft_desktop_contract::PlatformKind;
use arcweft_desktop_contract::{DesktopFeature, DesktopResponse, SupportLevel};
use arcweft_host_adapter::{HostAdapterRegistry, HostTaskCompletion, HostTaskSubmission};

#[test]
fn native_desktop_capabilities_complete_through_host_registry() {
    let adapter_set = arcweft_adapter_desktop::DesktopAdapterSet::bind_current_thread(
        arcweft_desktop_native::NativeDesktopBackend::builder().build(),
    );
    let (builder, coordinator) = adapter_set
        .register(HostAdapterRegistry::builder())
        .expect("desktop host calls are uniquely owned");
    let registry = builder.build();

    let submission = registry
        .submit(&task("desktop.platform", "capabilities"))
        .expect("desktop platform adapter owns capabilities");
    let HostTaskSubmission::Completed(outcome) = submission else {
        panic!("capabilities should complete without a window pump");
    };
    let HostTaskCompletion::Ready(payload) = outcome.completion else {
        panic!("capabilities request succeeds");
    };
    let RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Result,
        ordinal: 0,
        payload: Some(payload),
        ..
    } = payload.value()
    else {
        panic!("desktop response is a Result::Ok payload");
    };
    let RuntimeValue::String(payload) = payload.as_ref() else {
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
    let id = format!("{capability}.{operation}");
    TaskSpec::new(
        TaskId(id.clone()),
        TaskKey(id),
        TaskClass::Background,
        TaskPriority(0),
        CancelScopeId("desktop-test".to_owned()),
        TaskPolicy::JoinSameKey,
        HostTaskRequest::custom(capability, operation, []),
    )
    .with_outcome(TaskOutcomeContract::new(RuntimeCheckedType::Result {
        ok: Box::new(RuntimeCheckedType::String),
        error: Box::new(RuntimeCheckedType::String),
    }))
}
