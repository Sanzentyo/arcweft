use arcweft_core::task::{
    CancelScopeId, HostTaskRequest, TaskClass, TaskId, TaskKey, TaskPolicy, TaskPriority, TaskSpec,
};
use arcweft_desktop_contract::{DesktopFeature, DesktopResponse, PlatformKind, SupportLevel};
use arcweft_host_adapter::{HostAdapterRegistry, HostTaskSubmission};

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
        panic!("capabilities should complete without a UI pump");
    };
    let payload = outcome.result.expect("capabilities request succeeds");
    let response: DesktopResponse =
        serde_json::from_str(&payload.label()).expect("desktop response is JSON");
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
}
