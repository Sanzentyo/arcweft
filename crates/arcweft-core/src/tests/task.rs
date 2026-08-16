use crate::{task::*, value::RuntimePayload};

#[test]
fn normalizes_task_events_by_replay_stable_keys() {
    let events = vec![
        TaskEvent {
            logical_epoch: LogicalEpoch(1),
            task_id: TaskId("b".to_owned()),
            sequence: TaskSequence(0),
            kind: TaskEventKind::Ready(RuntimePayload::from("b")),
        },
        TaskEvent {
            logical_epoch: LogicalEpoch(0),
            task_id: TaskId("z".to_owned()),
            sequence: TaskSequence(9),
            kind: TaskEventKind::Ready(RuntimePayload::from("z")),
        },
        TaskEvent {
            logical_epoch: LogicalEpoch(1),
            task_id: TaskId("a".to_owned()),
            sequence: TaskSequence(1),
            kind: TaskEventKind::Ready(RuntimePayload::from("a")),
        },
    ];

    let normalized = normalize_task_events(events);
    let keys: Vec<_> = normalized
        .iter()
        .map(|event| {
            (
                event.logical_epoch,
                event.task_id.0.as_str(),
                event.sequence,
            )
        })
        .collect();
    assert_eq!(
        keys,
        vec![
            (LogicalEpoch(0), "z", TaskSequence(9)),
            (LogicalEpoch(1), "a", TaskSequence(1)),
            (LogicalEpoch(1), "b", TaskSequence(0)),
        ]
    );
}

#[test]
fn detects_already_normalized_task_events_without_reordering() {
    let events = vec![
        TaskEvent {
            logical_epoch: LogicalEpoch(0),
            task_id: TaskId("a".to_owned()),
            sequence: TaskSequence(0),
            kind: TaskEventKind::Ready(RuntimePayload::from("a")),
        },
        TaskEvent {
            logical_epoch: LogicalEpoch(0),
            task_id: TaskId("b".to_owned()),
            sequence: TaskSequence(1),
            kind: TaskEventKind::Ready(RuntimePayload::from("b")),
        },
    ];

    assert!(task_events_are_normalized(&events));
    assert_eq!(normalize_task_events(events.clone()), events);
}

#[test]
fn task_spec_uses_typed_request_and_debug_label() {
    let spec = TaskSpec::new(
        TaskId("task.asset.bg".to_owned()),
        TaskKey("asset.bg".to_owned()),
        TaskClass::AssetDecode,
        TaskPriority(3),
        CancelScopeId("flow.opening".to_owned()),
        TaskPolicy::JoinSameKey,
        HostTaskRequest::AssetLoad(AssetRequest {
            id: "asset.bg.room".to_owned(),
            kind: "image".to_owned(),
        }),
    );

    assert_eq!(spec.debug_label, "asset.load image asset.bg.room");
    assert!(matches!(
        spec.request,
        HostTaskRequest::AssetLoad(AssetRequest { id, kind })
            if id == "asset.bg.room" && kind == "image"
    ));
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
