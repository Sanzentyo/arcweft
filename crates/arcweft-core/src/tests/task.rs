use crate::{
    pattern::{RuntimeCheckedType, RuntimeVariantIdentity},
    task::*,
    value::{RuntimePayload, RuntimeValue},
};

#[test]
fn task_outcome_contract_owns_one_exact_ready_payload() {
    let direct = TaskOutcomeContract::new(RuntimeCheckedType::String);
    assert_eq!(
        direct
            .try_payload(RuntimeValue::String("ready".to_owned()))
            .expect("direct payload is admitted")
            .value(),
        &RuntimeValue::String("ready".to_owned())
    );
    assert!(
        direct
            .try_result_ok(RuntimeValue::String("ready".to_owned()))
            .is_err()
    );

    let fallible = TaskOutcomeContract::new(RuntimeCheckedType::Result {
        ok: Box::new(RuntimeCheckedType::String),
        error: Box::new(RuntimeCheckedType::String),
    });
    let ready = fallible
        .try_result_ok(RuntimeValue::String("ready".to_owned()))
        .expect("Result::Ok payload is admitted");
    assert!(matches!(
        ready.value(),
        RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Builtin(
                crate::pattern::RuntimeBuiltinVariantIdentity::Result,
            ),
            ordinal: 0,
            ..
        }
    ));
    let error = fallible
        .try_result_err(RuntimeValue::String("domain".to_owned()))
        .expect("Result::Err payload is admitted");
    assert!(matches!(
        error.value(),
        RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Builtin(
                crate::pattern::RuntimeBuiltinVariantIdentity::Result,
            ),
            ordinal: 1,
            ..
        }
    ));
    assert!(fallible.try_result_ok(RuntimeValue::Bool(true)).is_err());
    assert!(fallible.try_result_err(RuntimeValue::Bool(false)).is_err());
    assert!(
        fallible
            .try_payload(RuntimeValue::String("unwrapped".to_owned()))
            .is_err()
    );
}

#[test]
fn task_dispatch_start_continues_the_saved_revision_frontier() {
    let identity = TaskDispatchIdentity::new(
        crate::tests::reusable_need("task.resume").correlation(),
        LogicalEpoch(4),
        TaskSequence(7),
    );
    let initial = TaskDispatchStart::new(identity.clone(), None);
    assert_eq!(
        initial.next_publication_revision(),
        Some(TaskPublicationRevision::FIRST)
    );

    let last =
        TaskPublicationRevision::new(std::num::NonZeroU64::new(2).expect("revision is nonzero"));
    let resumed = TaskDispatchStart::new(identity.clone(), Some(last));
    assert_eq!(resumed.identity(), &identity);
    assert_eq!(resumed.last_publication_revision(), Some(last));
    assert_eq!(
        resumed
            .next_publication_revision()
            .map(TaskPublicationRevision::get),
        Some(3)
    );

    let exhausted = TaskDispatchStart::new(
        identity,
        Some(TaskPublicationRevision::new(
            std::num::NonZeroU64::new(u64::MAX).expect("maximum revision is nonzero"),
        )),
    );
    assert_eq!(exhausted.next_publication_revision(), None);
}

#[test]
fn normalizes_task_events_by_replay_stable_keys() {
    let correlation = crate::tests::reusable_need("ordered producer input").correlation();
    let early = TaskEvent {
        correlation: crate::tests::reusable_need("other producer input").correlation(),
        cursor: TaskPublicationCursor {
            logical_epoch: LogicalEpoch(0),
            sequence: TaskSequence(9),
        },
        kind: TaskEventKind::Ready(RuntimePayload::from("early")),
    };
    let pending = TaskEvent {
        correlation,
        cursor: TaskPublicationCursor {
            logical_epoch: LogicalEpoch(1),
            sequence: TaskSequence(1),
        },
        kind: TaskEventKind::Progress(crate::value::Progress::new(0.5).unwrap()),
    };
    let ready = TaskEvent {
        correlation,
        cursor: TaskPublicationCursor {
            logical_epoch: LogicalEpoch(1),
            sequence: TaskSequence(2),
        },
        kind: TaskEventKind::Ready(RuntimePayload::from("ready")),
    };
    assert_eq!(
        normalize_task_events(vec![ready.clone(), pending.clone(), early.clone()]),
        vec![early, pending, ready]
    );
}

#[test]
fn detects_already_normalized_task_events_without_reordering() {
    let correlation = crate::tests::reusable_need("ordered producer input").correlation();
    let events = vec![
        TaskEvent {
            correlation,
            cursor: TaskPublicationCursor {
                logical_epoch: LogicalEpoch(0),
                sequence: TaskSequence(1),
            },
            kind: TaskEventKind::Progress(crate::value::Progress::new(0.5).unwrap()),
        },
        TaskEvent {
            correlation,
            cursor: TaskPublicationCursor {
                logical_epoch: LogicalEpoch(0),
                sequence: TaskSequence(2),
            },
            kind: TaskEventKind::Ready(RuntimePayload::from("ready")),
        },
    ];
    assert!(task_events_are_normalized(&events));
    assert_eq!(normalize_task_events(events.clone()), events);
}

#[test]
fn task_spec_uses_typed_request_and_debug_label() {
    let mut spec = crate::tests::task_spec(
        TaskOutcomeContract::new(RuntimeCheckedType::String),
        HostTaskRequest::AssetLoad(AssetRequest {
            id: "asset.bg.room".to_owned(),
            kind: "image".to_owned(),
        }),
    );
    spec.priority = TaskPriority(3);
    spec.cancel_scope = CancelScopeId("flow.opening".to_owned());
    assert_eq!(spec.debug_label, "asset.load image asset.bg.room");
    assert!(
        matches!(&spec.request, HostTaskRequest::AssetLoad(AssetRequest { id, kind })
        if id == "asset.bg.room" && kind == "image")
    );
    let receipt = crate::tests::task_submission(spec);
    assert_eq!(
        receipt.handle().correlation,
        receipt.spec().correlation(TaskLaunchOrdinal::JOIN).unwrap()
    );
}

#[test]
fn host_task_request_covers_sans_io_adapter_work() {
    let requests = [
        HostTaskRequest::FileReadText(FileReadTextRequest {
            path: "game/config.arcw".to_owned(),
        }),
        HostTaskRequest::FileReadBytes(FileReadBytesRequest {
            path: "game/blob.bin".to_owned(),
        }),
        HostTaskRequest::FileWriteText(FileWriteTextRequest {
            path: "save/slot.json".to_owned(),
            text: "{}".to_owned(),
        }),
        HostTaskRequest::FileWriteBytes(FileWriteBytesRequest {
            path: "save/slot.bin".to_owned(),
            bytes: vec![1, 2, 3],
        }),
        HostTaskRequest::HttpFetch(HttpFetchRequest {
            url: "https://example.invalid/api".to_owned(),
            method: "GET".to_owned(),
            headers: vec![("accept".to_owned(), "application/json".to_owned())],
            body: None,
        }),
        HostTaskRequest::HttpRespond(HttpRespondRequest {
            request_id: "req-1".to_owned(),
            status: 200,
            headers: Vec::new(),
            body: Some("ok".into()),
        }),
        HostTaskRequest::ProcessRun(ProcessRunRequest {
            program: "tool".to_owned(),
            args: vec!["--version".to_owned()],
            env: Vec::new(),
        }),
        HostTaskRequest::ShaderCompile(ShaderRequest {
            id: "shader.text".to_owned(),
            entry: Some("main".to_owned()),
        }),
        HostTaskRequest::AudioDecode(AudioDecodeRequest {
            id: "voice.alice.001".to_owned(),
        }),
        HostTaskRequest::TtsSynthesis(TtsRequest {
            voice: Some("alice".to_owned()),
            text: "hello".to_owned(),
        }),
        HostTaskRequest::WasmCall(WasmCallRequest {
            module: "score".to_owned(),
            function: "rank".to_owned(),
            args: vec![RuntimePayload::from("choice")],
        }),
        HostTaskRequest::SystemInfo(SystemInfoRequest {
            kind: SystemInfoKind::CoreCount,
        }),
        HostTaskRequest::custom("custom.capability", "op", [RuntimePayload::from("arg")]),
    ];

    assert!(
        requests
            .iter()
            .all(|request| !request.debug_label().is_empty())
    );
    assert_eq!(requests[0].host_call_id(), "fs.read_text");
    assert_eq!(requests[3].host_call_id(), "fs.write_bytes");
    assert_eq!(requests[5].host_call_id(), "http.respond");
    assert_eq!(requests[11].host_call_id(), "system.core_count");
    assert_eq!(requests[12].host_call_id(), "custom.capability.op");
}

#[test]
fn host_request_serialization_contains_only_owned_runtime_fields() {
    let request = HostTaskRequest::custom("custom.capability", "read", []);
    assert_eq!(
        serde_json::to_value(&request).expect("host request serializes"),
        serde_json::json!({
            "Custom": {
                "capability": "custom.capability",
                "operation": "read",
                "args": []
            }
        })
    );
}
