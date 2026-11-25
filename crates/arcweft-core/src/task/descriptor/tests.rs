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
        crate::task::TaskHandleType::Need {
            payload: RuntimeTypeSemanticDigest::from_bytes(
                *RuntimeCheckedType::Bool
                    .semantic_identity_digest()
                    .as_bytes(),
            ),
        },
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
        request: (request).into(),
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
            id: crate::value::RuntimeBundleAssetResourceId::try_new("asset.oversize").unwrap(),
            kind: AssetLoadKind::Image,
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
fn asset_request_v1_serde_accepts_only_the_two_resource_kinds() {
    for (name, kind) in [
        ("image", AssetLoadKind::Image),
        ("voice", AssetLoadKind::Voice),
    ] {
        let encoded = format!(r#"{{"AssetLoad":{{"id":"asset.opening","kind":"{name}"}}}}"#);
        let request = HostTaskRequest::<AwbcRuntimeValueSnapshot>::AssetLoad(AssetRequest {
            id: crate::value::RuntimeBundleAssetResourceId::try_new("asset.opening").unwrap(),
            kind,
        });
        let decoded = serde_json::from_str::<HostTaskRequest<AwbcRuntimeValueSnapshot>>(&encoded)
            .expect("valid v1 asset request decodes");
        assert_eq!(decoded, request);
        assert_eq!(serde_json::to_string(&request).unwrap(), encoded);
    }

    let unsupported = r#"{"AssetLoad":{"id":"asset.opening","kind":"shader"}}"#;
    let error = serde_json::from_str::<HostTaskRequest<AwbcRuntimeValueSnapshot>>(unsupported)
        .expect_err("unsupported asset kind fails before constructing a saved request");
    assert!(
        error
            .to_string()
            .contains("expected asset request kind `image` or `voice`")
    );
    let unprojected = r#"{"AssetLoad":{"id":"voice.opening","kind":"voice"}}"#;
    assert!(
        serde_json::from_str::<HostTaskRequest<AwbcRuntimeValueSnapshot>>(unprojected).is_err()
    );
}

#[test]
fn every_request_family_uses_the_same_live_inert_descriptor_and_limits() {
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(
        crate::awbc::schema::AwbcProgram::default(),
    ));
    let original_operands = crate::entry::RuntimeValueDigest::from_bytes([0x85; 32]);
    let descriptor_limits = RuntimeSchemaLimits::engine_default();
    let mut request_identities = std::collections::BTreeSet::new();
    for request in requests() {
        let mut spec = specification(request);
        if matches!(spec.request.as_host(), Some(HostTaskRequest::SystemInfo(_))) {
            spec.debug_label = "oversize".into();
        }
        let saved = crate::task::TaskSpecSaveSnapshot::from_live(&spec, None).unwrap();
        let encoded = serde_json::to_vec(&saved).unwrap();
        let decoded: crate::task::TaskSpecSaveSnapshot = serde_json::from_slice(&encoded).unwrap();
        let live_identity =
            crate::entry::schema::value_encoding::retained_host_template_arguments_digest(
                original_operands,
                spec.request.as_host().unwrap(),
                descriptor_limits.max_encoded_bytes,
            )
            .unwrap();
        let inert_identity =
            crate::entry::schema::value_encoding::retained_host_template_arguments_digest(
                original_operands,
                decoded.request.as_host().unwrap(),
                descriptor_limits.max_encoded_bytes,
            )
            .unwrap();
        assert_eq!(live_identity, inert_identity);
        assert!(request_identities.insert(*live_identity.as_bytes()));
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
fn system_info_retains_the_operation_field_in_live_and_saved_descriptors() {
    #[derive(Default)]
    struct FixedFields(Vec<u8>);

    impl TaskDescriptorVisitor for FixedFields {
        fn node(&mut self, _depth: usize) -> Result<(), RuntimeSchemaError> {
            Ok(())
        }

        fn fixed(&mut self, bytes: &[u8], _depth: usize) -> Result<(), RuntimeSchemaError> {
            self.0.extend_from_slice(bytes);
            Ok(())
        }

        fn string(&mut self, _text: &str, _depth: usize) -> Result<(), RuntimeSchemaError> {
            panic!("SystemInfo has no string fields")
        }

        fn collection(&mut self, _count: usize, _depth: usize) -> Result<(), RuntimeSchemaError> {
            panic!("SystemInfo has no collections")
        }

        fn blob(&mut self, _bytes: &[u8], _depth: usize) -> Result<(), RuntimeSchemaError> {
            panic!("SystemInfo has no byte arrays")
        }
    }

    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(
        crate::awbc::schema::AwbcProgram::default(),
    ));
    for (kind, tag) in [
        (SystemInfoKind::CoreCount, 0),
        (SystemInfoKind::ThreadCount, 1),
        (SystemInfoKind::AvailableParallelism, 2),
    ] {
        let request = HostTaskRequest::SystemInfo(SystemInfoRequest { kind });
        let mut live_fields = FixedFields::default();
        request.visit_descriptor(&mut live_fields, 0).unwrap();
        assert_eq!(live_fields.0, [12, tag]);

        let spec = specification(request);
        let value = RuntimeValue::NeedHandle(RuntimeNeedHandle::try_from(spec).unwrap());
        let saved =
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &owner).unwrap();
        let AwbcRuntimeValueSnapshot::NeedHandle(handle) = &saved else {
            panic!("Need snapshot required")
        };
        let mut saved_fields = FixedFields::default();
        handle
            .spec()
            .request
            .as_host()
            .unwrap()
            .visit_descriptor(&mut saved_fields, 0)
            .unwrap();
        assert_eq!(saved_fields.0, live_fields.0);
        assert_eq!(saved.into_runtime_value_for_program(&owner).unwrap(), value);
    }
}

#[test]
fn later_request_payload_budget_failure_precedes_earlier_callable_construction() {
    let spec = specification(HostTaskRequest::custom("h", "f", []));
    let mut saved = crate::task::TaskSpecSaveSnapshot::from_live(&spec, None).unwrap();
    let HostTaskRequest::Custom { args, .. } =
        saved.request.as_host_mut().expect("host request fixture")
    else {
        unreachable!()
    };
    *args = vec![
        AwbcRuntimeValueSnapshot::Callable(crate::value::AwbcRuntimeCallableSnapshot {
            state: crate::runtime_id::RuntimeCallableStateId::from_zero_based(0).unwrap(),
            retained_order: crate::value::RuntimeInputOrder::canonical(0),
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
fn later_request_original_state_failure_precedes_earlier_progress_construction() {
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(
        crate::awbc::schema::AwbcProgram::default(),
    ));
    let invalid_progress = AwbcRuntimeValueSnapshot::Progress {
        ratio: -1.0,
        label: None,
    };
    assert!(
        invalid_progress
            .clone()
            .into_runtime_value_for_program(&owner)
            .is_err()
    );
    let spec = specification(HostTaskRequest::custom("h", "f", []));
    let mut saved = crate::task::TaskSpecSaveSnapshot::from_live(&spec, None).unwrap();
    let state = crate::runtime_id::RuntimeCallableStateId::from_zero_based(0).unwrap();
    let HostTaskRequest::Custom { args, .. } =
        saved.request.as_host_mut().expect("host request fixture")
    else {
        unreachable!()
    };
    *args = vec![
        invalid_progress,
        AwbcRuntimeValueSnapshot::Callable(crate::value::AwbcRuntimeCallableSnapshot {
            state,
            retained_order: crate::value::RuntimeInputOrder::canonical(0),
            retained: vec![],
        }),
    ];
    assert_eq!(
        saved.into_live(&owner).unwrap_err(),
        AwbcRuntimeValueSnapshotError::Message {
            message: format!("callable state {state} is absent from the selected program"),
        }
    );
}

#[test]
fn original_outcome_row_precedes_request_construction() {
    let spec = specification(HostTaskRequest::custom("h", "f", []));
    let mut saved = crate::task::TaskSpecSaveSnapshot::from_live(&spec, None).unwrap();
    let payload = crate::pattern::RuntimeSemanticTypeId::from_bytes([99; 32]);
    saved.outcome = TaskOutcomeContract::program(payload);
    let producer = NeedProducerSpec::new(
        NeedProducerFamily::HostAdapterTask,
        saved.producer.contract(),
        saved.producer.plan(),
        NeedProducerSiteDigest::from_bytes([3; 32]),
        crate::task::TaskHandleType::Need {
            payload: RuntimeTypeSemanticDigest::from_bytes(*payload.as_bytes()),
        },
        saved.producer.arguments(),
    );
    saved.producer = NeedProducerInstance::try_from(&producer).unwrap();
    let HostTaskRequest::Custom { args, .. } =
        saved.request.as_host_mut().expect("host request fixture")
    else {
        unreachable!()
    };
    args.push(AwbcRuntimeValueSnapshot::Callable(
        crate::value::AwbcRuntimeCallableSnapshot {
            state: crate::runtime_id::RuntimeCallableStateId::from_zero_based(0).unwrap(),
            retained_order: crate::value::RuntimeInputOrder::canonical(0),
            retained: vec![],
        },
    ));
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(
        crate::awbc::schema::AwbcProgram::default(),
    ));
    assert_eq!(
        saved.into_live(&owner).unwrap_err(),
        AwbcRuntimeValueSnapshotError::TaskOutcome(TaskOutcomeBindingError::ProgramType(
            crate::program_types::RuntimeProgramTypeError::Missing {
                semantic_type: payload
            },
        ))
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
        Err(first.clone())
    );
    assert_eq!(
        AwbcRuntimeValueSnapshot::validate_admitted_storage_view(
            RuntimeValue::Unit.view(),
            None,
            &resources
        ),
        Err(AwbcRuntimeValueSnapshotError::Resource(first))
    );
}

#[test]
fn submission_identity_is_admitted_before_request_callable_construction() {
    let mut journal = TaskAdmissionJournal::default();
    let handle = journal
        .ensure_task(specification(HostTaskRequest::custom("h", "f", [])))
        .unwrap();
    let submission = journal.submission(handle).unwrap();
    let saved = TaskSubmissionSaveSnapshot {
        spec: crate::task::TaskSpecSaveSnapshot::from_live(submission.spec(), None).unwrap(),
        handle: submission.handle(),
    };
    let mut wire = serde_json::to_value(saved).unwrap();
    let mut forged = handle;
    forged.correlation.task_id = TaskId::try_from_bytes([9; 32]).unwrap();
    wire["handle"] = serde_json::to_value(forged).unwrap();
    wire["spec"]["request"]["Host"]["Custom"]["args"] =
        serde_json::to_value(vec![AwbcRuntimeValueSnapshot::Callable(
            crate::value::AwbcRuntimeCallableSnapshot {
                state: crate::runtime_id::RuntimeCallableStateId::from_zero_based(0).unwrap(),
                retained_order: crate::value::RuntimeInputOrder::canonical(0),
                retained: vec![],
            },
        )])
        .unwrap();
    let decoded: TaskSubmissionSaveSnapshot = serde_json::from_value(wire).unwrap();
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(
        crate::awbc::schema::AwbcProgram::default(),
    ));
    assert_eq!(
        decoded
            .admit_retained(
                &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                    owner,
                    RuntimeSchemaLimits::engine_default(),
                )
            )
            .unwrap_err(),
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
