use super::*;
use crate::entry::RuntimeValueDigest;
use crate::pattern::RuntimeCheckedType;
use crate::task::identity::{TaskId, TaskKey};
use crate::task::specification::NeedProducerInstance;
use crate::task::{
    CancelScopeId, GenerationId, HostCapabilityId, HostTaskRequest, NeedProducerContractDigest,
    NeedProducerFamily, NeedProducerSiteDigest, NeedProducerSpec, RuntimeTypeSemanticDigest,
    TaskClass, TaskPlanSemanticDigest, TaskPriority,
};
use crate::value::{RuntimePayload, RuntimeValue};

fn producer_input() -> NeedProducerSpec {
    NeedProducerSpec::new(
        NeedProducerFamily::HostAdapterTask,
        NeedProducerContractDigest::from_bytes([1; 32]),
        TaskPlanSemanticDigest::from_bytes([2; 32]),
        NeedProducerSiteDigest::from_bytes([3; 32]),
        RuntimeTypeSemanticDigest::from_bytes(
            *RuntimeCheckedType::Bool
                .semantic_identity_digest()
                .as_bytes(),
        ),
        RuntimeValue::Tuple(Vec::new()).try_digest(1024).unwrap(),
    )
}

fn specification(policy: TaskPolicy) -> TaskSpec {
    TaskSpec {
        generation: GenerationId::new(0),
        producer: NeedProducerInstance::try_from(&producer_input()).unwrap(),
        class: TaskClass::Cpu,
        priority: TaskPriority(0),
        cancel_scope: CancelScopeId("fixture".to_owned()),
        policy,
        outcome: TaskOutcomeContract::new(RuntimeCheckedType::Bool),
        request: (HostTaskRequest::Custom {
            capability: HostCapabilityId("fixture".to_owned()),
            operation: "pure".to_owned(),
            args: Vec::new(),
            named_args: Vec::new(),
            manifest_contract: None,
        })
        .into(),
        debug_label: "diagnostic".to_owned(),
    }
}

fn accepted(spec: TaskSpec, ordinal: u64) -> RuntimeNeedHandle {
    let ordinal = TaskLaunchOrdinal::try_for_policy(spec.policy, ordinal).unwrap();
    let handle = TaskHandle {
        correlation: spec.correlation(ordinal).unwrap(),
    };
    RuntimeNeedHandle::try_from_accepted_launch(spec, handle).unwrap()
}

#[test]
fn admitted_need_projection_preserves_the_parent_resource_policy_in_both_directions() {
    let live = accepted(specification(TaskPolicy::JoinSameKey), 0);
    let mut image = RuntimeNeedHandleSaveSnapshot::from_live(&live, None).unwrap();
    let defaults = crate::entry::RuntimeSchemaLimits::engine_default();
    let length = defaults.max_string_bytes + 1;
    image.spec.debug_label = "d".repeat(usize::try_from(length).unwrap());
    let owner = crate::task::RuntimeProgramOwner::Plan(std::sync::Arc::new(
        crate::plan::RuntimePlanBuilder::new().finish().unwrap(),
    ));
    let limits = crate::entry::RuntimeSchemaLimits {
        max_string_bytes: length,
        ..defaults
    };
    let value = crate::value::AwbcRuntimeValueSnapshot::NeedHandle(image.clone());
    let mut admission =
        crate::value::RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
    admission.validate_owned_borrowed((&value).into()).unwrap();
    let RuntimeValue::NeedHandle(restored) = value.into_admitted_runtime_value(&owner).unwrap()
    else {
        panic!("Need storage role is preserved")
    };
    assert_eq!(
        restored.spec().debug_label.len(),
        usize::try_from(length).unwrap()
    );
    let mut admission =
        crate::value::RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
    crate::value::RuntimeNeedView::Live(&restored)
        .admit_retained(&mut admission)
        .unwrap();
    let recaptured =
        RuntimeNeedHandleSaveSnapshot::from_live_admitted(&restored, Some(&owner)).unwrap();
    assert_eq!(
        recaptured.spec.debug_label.len(),
        usize::try_from(length).unwrap()
    );
    assert_eq!(recaptured.correlation, image.correlation);
    assert!(matches!(
        RuntimeNeedHandleSaveSnapshot::from_live(&restored, Some(&owner)),
        Err(crate::value::AwbcRuntimeValueSnapshotError::Resource(_)),
    ));
    assert!(matches!(
        image.into_live(&owner),
        Err(crate::value::AwbcRuntimeValueSnapshotError::Resource(_))
    ));
}

#[test]
fn inert_handle_identity_rejects_before_request_callable_construction() {
    let live = accepted(specification(TaskPolicy::JoinSameKey), 0);
    let mut image = RuntimeNeedHandleSaveSnapshot::from_live(&live, None).unwrap();
    image.correlation.need = NeedId::try_from_bytes([9; 32]).unwrap();
    image.spec.request = (super::super::HostTaskRequest::Custom {
        capability: HostCapabilityId("fixture".into()),
        operation: "pure".into(),
        args: vec![crate::value::AwbcRuntimeValueSnapshot::Callable(
            crate::value::AwbcRuntimeCallableSnapshot {
                state: crate::runtime_id::RuntimeCallableStateId::from_zero_based(0).unwrap(),
                retained: vec![],
            },
        )],
        named_args: vec![],
        manifest_contract: None,
    })
    .into();
    let owner = super::super::RuntimeProgramOwner::Awbc(std::sync::Arc::new(
        crate::awbc::schema::AwbcProgram::default(),
    ));
    assert_eq!(
        image.into_live(&owner).unwrap_err(),
        crate::value::AwbcRuntimeValueSnapshotError::Message {
            message: RuntimeNeedHandleError::CorrelationMismatch.to_string(),
        }
    );
}

#[test]
fn need_handle_preparation_distinguishes_reusable_and_accepted_origins() {
    assert_eq!(
        RuntimeNeedHandle::try_from(specification(TaskPolicy::AlwaysStart)),
        Err(RuntimeNeedHandleError::ReusableAlwaysStart)
    );
    let reusable = RuntimeNeedHandle::try_from(specification(TaskPolicy::JoinSameKey)).unwrap();
    assert!(reusable.reusable_spec().is_some());
    assert_eq!(
        reusable.correlation().launch_ordinal,
        TaskLaunchOrdinal::JOIN
    );
    assert_eq!(reusable.need_id(), reusable.correlation().need);
    assert_eq!(
        reusable.outcome(),
        &TaskOutcomeContract::new(RuntimeCheckedType::Bool)
    );
    let join = accepted(specification(TaskPolicy::JoinSameKey), 0);
    assert!(join.reusable_spec().is_none());
    assert_eq!(join.correlation(), reusable.correlation());
    let first = accepted(specification(TaskPolicy::AlwaysStart), 1);
    let second = accepted(specification(TaskPolicy::AlwaysStart), 2);
    assert!(first.reusable_spec().is_none());
    assert_eq!(first.correlation().task_key, second.correlation().task_key);
    assert_ne!(first.need_id(), second.need_id());
    assert_ne!(first.correlation().task_id, second.correlation().task_id);
}

#[test]
fn need_handle_environment_move_preserves_complete_specification_without_starting_work() {
    let move_through_local = |handle| {
        let local = crate::runtime_id::RuntimeLocalDeclarationId::from_accepted_ordinal(
            std::num::NonZeroU32::new(1).unwrap(),
        );
        let mut env = crate::value::RuntimeEnv::default();
        env.set(local, RuntimeValue::NeedHandle(handle));
        let Some(RuntimeValue::NeedHandle(moved)) = env.take(local) else {
            panic!("owned need handle fixture");
        };
        assert!(env.take(local).is_none());
        moved
    };
    for (policy, ordinal) in [(TaskPolicy::JoinSameKey, 0), (TaskPolicy::AlwaysStart, 1)] {
        let mut spec = specification(policy);
        spec.request = (HostTaskRequest::Custom {
            capability: HostCapabilityId("fixture".to_owned()),
            operation: "pure".to_owned(),
            args: vec![RuntimePayload::new(RuntimeValue::String(
                "owned argument".to_owned(),
            ))],
            named_args: Vec::new(),
            manifest_contract: None,
        })
        .into();
        spec.producer = NeedProducerInstance::try_from(&NeedProducerSpec::new(
            producer_input().family(),
            spec.producer.contract(),
            spec.producer.plan(),
            producer_input().producer_site(),
            spec.producer.payload_type(),
            RuntimeValue::Tuple(vec![RuntimeValue::String("owned argument".to_owned())])
                .try_digest(1024)
                .unwrap(),
        ))
        .unwrap();
        let handle = accepted(spec.clone(), ordinal);
        let correlation = handle.correlation();
        let spec_address = std::ptr::from_ref(&handle.0.spec);
        let HostTaskRequest::Custom { args, .. } = handle
            .0
            .spec
            .request
            .as_host()
            .expect("host request fixture")
        else {
            panic!("custom request fixture");
        };
        let RuntimeValue::String(argument) = args[0].value() else {
            panic!("owned string fixture");
        };
        let argument_address = argument.as_ptr();
        let restored = move_through_local(handle);
        assert_eq!(restored.correlation(), correlation);
        assert_eq!(
            restored.0.spec.producer.arguments(),
            RuntimeValue::Tuple(vec![RuntimeValue::String("owned argument".to_owned())])
                .try_digest(1024)
                .unwrap()
        );
        assert_eq!(std::ptr::from_ref(&restored.0.spec), spec_address);
        let HostTaskRequest::Custom { args, .. } = restored
            .0
            .spec
            .request
            .as_host()
            .expect("host request fixture")
        else {
            panic!("restored request fixture");
        };
        let RuntimeValue::String(argument) = args[0].value() else {
            panic!("restored argument fixture");
        };
        assert_eq!(argument.as_ptr(), argument_address);
        assert_eq!(restored.0.spec, spec);
        assert!(restored.reusable_spec().is_none());
    }
    let handle = RuntimeNeedHandle::try_from(specification(TaskPolicy::JoinSameKey)).unwrap();
    let restored = move_through_local(handle);
    assert!(restored.reusable_spec().is_some());
}

#[test]
fn need_handle_preparation_rejects_every_forged_correlation_field() {
    let spec = specification(TaskPolicy::AlwaysStart);
    let ordinal = TaskLaunchOrdinal::try_for_policy(spec.policy, 1).unwrap();
    let row = spec.correlation(ordinal).unwrap();
    let other = specification(TaskPolicy::JoinSameKey).producer.key();
    let other = crate::task::NeedProducerInstanceKey::try_from_bytes({
        let mut bytes = *other.as_bytes();
        bytes[0] ^= 1;
        bytes
    })
    .unwrap();
    for (forged, expected) in [
        (
            TaskCorrelation {
                generation: GenerationId::new(1),
                ..row
            },
            RuntimeNeedHandleError::CorrelationMismatch,
        ),
        (
            TaskCorrelation {
                producer: other,
                ..row
            },
            RuntimeNeedHandleError::CorrelationMismatch,
        ),
        (
            TaskCorrelation {
                producer_contract: NeedProducerContractDigest::from_bytes([9; 32]),
                ..row
            },
            RuntimeNeedHandleError::ProducerContractMismatch,
        ),
        (
            TaskCorrelation {
                need: NeedId::try_from_bytes([9; 32]).unwrap(),
                ..row
            },
            RuntimeNeedHandleError::CorrelationMismatch,
        ),
        (
            TaskCorrelation {
                task_key: TaskKey::try_from_bytes([9; 32]).unwrap(),
                ..row
            },
            RuntimeNeedHandleError::CorrelationMismatch,
        ),
        (
            TaskCorrelation {
                task_id: TaskId::try_from_bytes([9; 32]).unwrap(),
                ..row
            },
            RuntimeNeedHandleError::CorrelationMismatch,
        ),
        (
            TaskCorrelation {
                launch_ordinal: TaskLaunchOrdinal::try_for_policy(spec.policy, 2).unwrap(),
                ..row
            },
            RuntimeNeedHandleError::CorrelationMismatch,
        ),
    ] {
        assert_eq!(
            RuntimeNeedHandle::try_from_accepted_launch(
                spec.clone(),
                TaskHandle {
                    correlation: forged
                }
            ),
            Err(expected.clone())
        );
        let mut snapshot =
            RuntimeNeedHandleSaveSnapshot::from_live(&accepted(spec.clone(), 1), None).unwrap();
        snapshot.correlation = forged;
        assert_eq!(snapshot.validate_identity(), Err(expected));
    }
}

#[test]
fn need_handle_preparation_rejects_changed_producer_instance_on_restore() {
    let spec = specification(TaskPolicy::AlwaysStart);
    let base = producer_input();
    for changed in [
        NeedProducerSpec::new(
            NeedProducerFamily::Timeout,
            base.contract(),
            base.plan(),
            base.producer_site(),
            base.payload_type(),
            base.arguments(),
        ),
        NeedProducerSpec::new(
            base.family(),
            NeedProducerContractDigest::from_bytes([9; 32]),
            base.plan(),
            base.producer_site(),
            base.payload_type(),
            base.arguments(),
        ),
        NeedProducerSpec::new(
            base.family(),
            base.contract(),
            TaskPlanSemanticDigest::from_bytes([9; 32]),
            base.producer_site(),
            base.payload_type(),
            base.arguments(),
        ),
        NeedProducerSpec::new(
            base.family(),
            base.contract(),
            base.plan(),
            NeedProducerSiteDigest::from_bytes([9; 32]),
            base.payload_type(),
            base.arguments(),
        ),
        NeedProducerSpec::new(
            base.family(),
            base.contract(),
            base.plan(),
            base.producer_site(),
            RuntimeTypeSemanticDigest::from_bytes([9; 32]),
            base.arguments(),
        ),
        NeedProducerSpec::new(
            base.family(),
            base.contract(),
            base.plan(),
            base.producer_site(),
            base.payload_type(),
            RuntimeValueDigest::from_bytes([9; 32]),
        ),
    ] {
        let mut snapshot =
            RuntimeNeedHandleSaveSnapshot::from_live(&accepted(spec.clone(), 1), None).unwrap();
        let expected = if changed.contract() == base.contract() {
            RuntimeNeedHandleError::CorrelationMismatch
        } else {
            RuntimeNeedHandleError::ProducerContractMismatch
        };
        snapshot.spec.producer = NeedProducerInstance::try_from(&changed).unwrap();
        assert_eq!(snapshot.validate_identity(), Err(expected));
    }
}

#[test]
fn need_handle_preparation_checks_policy_origin_and_generation_on_restore() {
    let spec = specification(TaskPolicy::AlwaysStart);
    let mut snapshot =
        RuntimeNeedHandleSaveSnapshot::from_live(&accepted(spec.clone(), 1), None).unwrap();
    snapshot.origin = RuntimeNeedHandleOrigin::ReusableJoin;
    assert_eq!(
        snapshot.validate_identity(),
        Err(RuntimeNeedHandleError::ReusableAlwaysStart)
    );
    let mut snapshot =
        RuntimeNeedHandleSaveSnapshot::from_live(&accepted(spec.clone(), 1), None).unwrap();
    snapshot.spec.policy = TaskPolicy::JoinSameKey;
    assert_eq!(
        snapshot.validate_identity(),
        Err(RuntimeNeedHandleError::Identity(
            TaskIdentityError::NonZeroJoinOrdinal
        ))
    );
    let mut snapshot =
        RuntimeNeedHandleSaveSnapshot::from_live(&accepted(spec.clone(), 1), None).unwrap();
    snapshot.spec.generation = GenerationId::new(1);
    assert_eq!(
        snapshot.validate_identity(),
        Err(RuntimeNeedHandleError::CorrelationMismatch)
    );
    let mut rebased_spec = spec;
    rebased_spec.generation = GenerationId::new(1);
    let rebased = accepted(rebased_spec, 1);
    let original = accepted(specification(TaskPolicy::AlwaysStart), 1);
    assert_eq!(rebased.need_id(), original.need_id());
    assert_ne!(
        rebased.correlation().task_key,
        original.correlation().task_key
    );
    assert_ne!(
        rebased.correlation().task_id,
        original.correlation().task_id
    );
}
