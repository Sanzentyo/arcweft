use super::*;
use crate::entry::RuntimeSchemaLimits;
use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};
use crate::pattern::RuntimeCheckedType;
use crate::task::*;
use crate::value::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimePayload, RuntimeValue,
};

fn specification(request: HostTaskRequest) -> TaskSpec {
    let producer = NeedProducerSpec::new(
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
    );
    TaskSpec {
        generation: GenerationId::new(0),
        producer: NeedProducerInstance::try_from(&producer).unwrap(),
        class: TaskClass::Cpu,
        priority: TaskPriority(0),
        cancel_scope: CancelScopeId("c".into()),
        policy: TaskPolicy::JoinSameKey,
        outcome: TaskOutcomeContract::new(RuntimeCheckedType::Bool),
        request,
        debug_label: "d".into(),
    }
}

fn requests() -> [HostTaskRequest; 14] {
    let text = || "oversize".to_owned();
    let body = || Some(RuntimePayload(RuntimeValue::String("ok".into())));
    [
        HostTaskRequest::FileReadText(FileReadTextRequest { path: text() }),
        HostTaskRequest::FileReadBytes(FileReadBytesRequest { path: text() }),
        HostTaskRequest::FileWriteText(FileWriteTextRequest {
            path: text(),
            text: "ok".into(),
        }),
        HostTaskRequest::FileWriteBytes(FileWriteBytesRequest {
            path: text(),
            bytes: vec![1, 2],
        }),
        HostTaskRequest::HttpFetch(HttpFetchRequest {
            url: text(),
            method: "GET".into(),
            headers: vec![("h".into(), "v".into())],
            body: body(),
        }),
        HostTaskRequest::HttpRespond(HttpRespondRequest {
            request_id: text(),
            status: 200,
            headers: vec![("h".into(), "v".into())],
            body: body(),
        }),
        HostTaskRequest::ProcessRun(ProcessRunRequest {
            program: text(),
            args: vec!["a".into()],
            env: vec![("e".into(), "v".into())],
        }),
        HostTaskRequest::AssetLoad(AssetRequest {
            id: text(),
            kind: "image".into(),
        }),
        HostTaskRequest::ShaderCompile(ShaderRequest {
            id: text(),
            entry: Some("e".into()),
        }),
        HostTaskRequest::AudioDecode(AudioDecodeRequest { id: text() }),
        HostTaskRequest::TtsSynthesis(TtsRequest {
            voice: Some(text()),
            text: "ok".into(),
        }),
        HostTaskRequest::WasmCall(WasmCallRequest {
            module: text(),
            function: "f".into(),
            args: vec![RuntimePayload(RuntimeValue::Unit)],
        }),
        HostTaskRequest::SystemInfo(SystemInfoRequest {
            kind: SystemInfoKind::CoreCount,
        }),
        HostTaskRequest::Custom {
            capability: HostCapabilityId("h".into()),
            operation: text(),
            args: vec![RuntimePayload(RuntimeValue::Unit)],
            named_args: vec![NamedHostArg {
                name: "n".into(),
                value: RuntimePayload(RuntimeValue::Bool(true)),
            }],
            manifest_contract: None,
        },
    ]
}

#[test]
fn every_request_family_uses_the_same_live_inert_descriptor_and_limits() {
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(
        crate::awbc::schema::AwbcProgram::default(),
    ));
    for request in requests() {
        let mut spec = specification(request);
        if matches!(spec.request, HostTaskRequest::SystemInfo(_)) {
            spec.debug_label = "oversize".into();
        }
        let saved = TaskSpec::<AwbcRuntimeValueSnapshot>::from_live(&spec, None).unwrap();
        let encoded = serde_json::to_vec(&saved).unwrap();
        let decoded: TaskSpec<AwbcRuntimeValueSnapshot> = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded.into_live(&owner).unwrap(), spec);
        let limits = RuntimeSchemaLimits {
            max_string_bytes: 7,
            ..RuntimeSchemaLimits::engine_default()
        };
        for result in [
            spec.validate_descriptor_inputs(
                &mut ValueResourceAdmission::snapshot(limits, ValueAdmissionOrigin::Live),
                0,
                |v| v.value().view(),
            ),
            saved.validate_descriptor_inputs(
                &mut ValueResourceAdmission::snapshot(limits, ValueAdmissionOrigin::InertSnapshot),
                0,
                AwbcRuntimeValueSnapshot::view,
            ),
        ] {
            assert_eq!(
                result,
                Err(RuntimeSchemaError::BudgetExceeded {
                    budget: "string_bytes"
                })
            );
        }
    }
}

#[test]
fn later_request_payload_budget_failure_precedes_earlier_callable_construction() {
    let spec = specification(HostTaskRequest::custom("h", "f", []));
    let mut saved = TaskSpec::<AwbcRuntimeValueSnapshot>::from_live(&spec, None).unwrap();
    let HostTaskRequest::Custom { args, .. } = &mut saved.request else {
        unreachable!()
    };
    *args = vec![
        AwbcRuntimeValueSnapshot::Callable(crate::value::AwbcRuntimeCallableSnapshot {
            state: crate::runtime_id::RuntimeCallableStateId::from_zero_based(0).unwrap(),
            retained: vec![],
        }),
        AwbcRuntimeValueSnapshot::String("x".repeat(
            usize::try_from(RuntimeSchemaLimits::engine_default().max_string_bytes).unwrap() + 1,
        )),
    ];
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(
        crate::awbc::schema::AwbcProgram::default(),
    ));
    assert_eq!(
        saved.into_live(&owner).unwrap_err(),
        AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded {
            budget: "string_bytes"
        })
    );
}

#[test]
fn descriptor_and_all_payloads_consume_one_encoded_byte_allowance() {
    let spec = specification(HostTaskRequest::custom(
        "h",
        "f",
        [
            RuntimePayload(RuntimeValue::String("x".repeat(600))),
            RuntimePayload(RuntimeValue::String("x".repeat(600))),
        ],
    ));
    let limits = RuntimeSchemaLimits {
        max_encoded_bytes: 1024,
        ..RuntimeSchemaLimits::engine_default()
    };
    let error = spec
        .validate_descriptor_inputs(
            &mut ValueResourceAdmission::snapshot(limits, ValueAdmissionOrigin::Live),
            0,
            |v| v.value().view(),
        )
        .unwrap_err();
    assert_eq!(
        error,
        RuntimeSchemaError::BudgetExceeded {
            budget: "encoded_bytes"
        }
    );
}

#[test]
fn descriptor_failure_poison_is_shared_with_following_value_admission() {
    let spec = specification(HostTaskRequest::custom("oversize", "f", []));
    let limits = RuntimeSchemaLimits {
        max_string_bytes: 7,
        ..RuntimeSchemaLimits::engine_default()
    };
    let mut resources = ValueResourceAdmission::snapshot(limits, ValueAdmissionOrigin::Live);
    let first = spec
        .validate_descriptor_inputs(&mut resources, 0, |v| v.value().view())
        .unwrap_err();
    assert_eq!(
        first,
        RuntimeSchemaError::BudgetExceeded {
            budget: "string_bytes"
        }
    );
    assert_eq!(
        resources.validate_view(RuntimeValue::Unit.view(), 0, None),
        Err(first)
    );
}

#[test]
fn submission_identity_is_admitted_before_request_callable_construction() {
    let mut journal = TaskAdmissionJournal::default();
    let handle = journal
        .ensure_task(specification(HostTaskRequest::custom("h", "f", [])))
        .unwrap();
    let submission = journal.submission(handle).unwrap();
    let saved = TaskSubmissionSaveSnapshot::from_live(&submission, None).unwrap();
    let mut wire = serde_json::to_value(saved).unwrap();
    let mut forged = handle;
    forged.correlation.task_id = TaskId::try_from_bytes([9; 32]).unwrap();
    wire["handle"] = serde_json::to_value(forged).unwrap();
    wire["spec"]["request"]["Custom"]["args"] =
        serde_json::to_value(vec![AwbcRuntimeValueSnapshot::Callable(
            crate::value::AwbcRuntimeCallableSnapshot {
                state: crate::runtime_id::RuntimeCallableStateId::from_zero_based(0).unwrap(),
                retained: vec![],
            },
        )])
        .unwrap();
    let decoded: TaskSubmissionSaveSnapshot = serde_json::from_value(wire).unwrap();
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(
        crate::awbc::schema::AwbcProgram::default(),
    ));
    assert_eq!(
        decoded.into_live(&owner).unwrap_err(),
        AwbcRuntimeValueSnapshotError::Message {
            message: TaskEnsureError::TaskIdSpecificationConflict {
                task_id: forged.correlation.task_id
            }
            .to_string(),
        }
    );
}

#[test]
fn standalone_type_limits_precede_need_allocation_and_outcome_hashing() {
    let mut spec = specification(HostTaskRequest::custom("h", "f", []));
    let mut ty = RuntimeCheckedType::Bool;
    for _ in 0..=RuntimeSchemaLimits::engine_default().max_depth {
        ty = RuntimeCheckedType::Option(Box::new(ty));
    }
    spec.outcome = TaskOutcomeContract::new(ty);
    assert!(
        matches!(RuntimeNeedHandle::try_from(spec), Err(RuntimeNeedHandleError::Descriptor(error))
        if matches!(*error, RuntimeSchemaError::BudgetExceeded { budget: "depth" }))
    );
}
