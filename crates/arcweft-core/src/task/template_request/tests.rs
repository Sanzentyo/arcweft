use super::*;
use crate::task::AssetRequest;
use crate::value::{
    RuntimeBundleAssetResourceId, RuntimeEntityReference, RuntimeVirtualPathSpace,
    runtime_sequence_values,
};
use arcweft_id::{DeclarationIdentityFamily, PublicId};

fn text(value: &str) -> RuntimeValue {
    RuntimeValue::String(value.into())
}

fn path() -> RuntimeValue {
    RuntimeVirtualPath::new(RuntimeVirtualPathSpace::Save, "fixture.txt").into_value()
}

fn catalog(family: DeclarationIdentityFamily, id: &str) -> RuntimeValue {
    RuntimeValue::EntityRef(
        RuntimeEntityReference::try_project(family, PublicId::try_new(id).unwrap()).unwrap(),
    )
}

#[test]
fn original_template_counts_combined_spreads_before_expansion() {
    use crate::entry::{RuntimeSchemaError, RuntimeSchemaLimits};
    use crate::task::{RuntimeProgramOwner, RuntimeRequestRoleIdentity};

    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(
        crate::awbc::schema::AwbcProgram::default(),
    ));
    let roles = [
        RuntimeHostArgumentTemplate::spread(
            RuntimeRequestRoleIdentity::from_accepted_identity([1; 32]),
            (),
        ),
        RuntimeHostArgumentTemplate::spread(
            RuntimeRequestRoleIdentity::from_accepted_identity([2; 32]),
            (),
        ),
    ];
    let values = || {
        vec![
            runtime_sequence_values(vec![text("first"), text("second")]),
            RuntimeValue::Tuple(vec![text("third"), text("fourth")]),
        ]
    };
    let bank = |max_sequence_items| {
        RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            RuntimeSchemaLimits {
                max_sequence_items,
                ..RuntimeSchemaLimits::engine_default()
            },
        )
    };
    let mut exact = bank(4);
    let args = EvaluatedArguments::admit_and_group(&roles, values(), &mut exact).unwrap();
    assert_eq!(
        args.positional,
        VecDeque::from([text("first"), text("second"), text("third"), text("fourth")])
    );

    let mut one_short = bank(3);
    let expected = RuntimeSchemaError::BudgetExceeded {
        budget: "sequence_items",
    }
    .into();
    let Err(RuntimeTaskProducerSourceError::Admission(actual)) =
        EvaluatedArguments::admit_and_group(&roles, values(), &mut one_short)
    else {
        panic!("two individually admitted spreads exceed the combined limit");
    };
    assert_eq!(actual, expected);
    assert_eq!(one_short.status(), Err(expected));
}

#[test]
fn original_template_projects_each_typed_request_family_and_preserves_fields() {
    use HostTaskTemplateOperation as Operation;
    let logical_path = "save:fixture.txt";
    let examples = [
        (
            Operation::FileReadText,
            vec![path()],
            vec![],
            HostTaskRequest::FileReadText(FileReadTextRequest {
                path: logical_path.into(),
            }),
        ),
        (
            Operation::FileReadBytes,
            vec![path()],
            vec![],
            HostTaskRequest::FileReadBytes(FileReadBytesRequest {
                path: logical_path.into(),
            }),
        ),
        (
            Operation::FileWriteText,
            vec![path(), text("test")],
            vec![],
            HostTaskRequest::FileWriteText(FileWriteTextRequest {
                path: logical_path.into(),
                text: "test".into(),
            }),
        ),
        (
            Operation::FileWriteBytes,
            vec![
                path(),
                runtime_sequence_values(vec![RuntimeValue::u8(19), RuntimeValue::u8(27)]),
            ],
            vec![],
            HostTaskRequest::FileWriteBytes(FileWriteBytesRequest {
                path: logical_path.into(),
                bytes: vec![19, 27],
            }),
        ),
        (
            Operation::HttpFetch,
            vec![text("https://example.invalid")],
            vec![
                ("method", text("POST")),
                (
                    "headers",
                    runtime_sequence_values(vec![RuntimeValue::Tuple(vec![
                        text("x-test"),
                        text("value"),
                    ])]),
                ),
                ("body", RuntimeValue::Bool(true)),
            ],
            HostTaskRequest::HttpFetch(HttpFetchRequest {
                url: "https://example.invalid".into(),
                method: "POST".into(),
                headers: vec![("x-test".into(), "value".into())],
                body: Some(RuntimePayload::new(RuntimeValue::Bool(true))),
            }),
        ),
        (
            Operation::HttpRespond,
            vec![text("request-1")],
            vec![("status", RuntimeValue::u32(201))],
            HostTaskRequest::HttpRespond(HttpRespondRequest {
                request_id: "request-1".into(),
                status: 201,
                headers: vec![],
                body: None,
            }),
        ),
        (
            Operation::ProcessRun,
            vec![text("fixture-worker")],
            vec![(
                "args",
                runtime_sequence_values(vec![text("first"), text("second")]),
            )],
            HostTaskRequest::ProcessRun(ProcessRunRequest {
                program: "fixture-worker".into(),
                args: vec!["first".into(), "second".into()],
                env: vec![],
            }),
        ),
        (
            Operation::AssetLoad(AssetLoadKind::Image),
            vec![catalog(
                DeclarationIdentityFamily::Asset,
                "asset.bg.fixture",
            )],
            vec![],
            HostTaskRequest::AssetLoad(AssetRequest {
                id: RuntimeBundleAssetResourceId::try_new("asset.bg.fixture").unwrap(),
                kind: AssetLoadKind::Image,
            }),
        ),
        (
            Operation::AssetLoad(AssetLoadKind::Voice),
            vec![catalog(DeclarationIdentityFamily::Voice, "voice.alice.001")],
            vec![],
            HostTaskRequest::AssetLoad(AssetRequest {
                id: RuntimeBundleAssetResourceId::try_new("asset.voice.alice.001").unwrap(),
                kind: AssetLoadKind::Voice,
            }),
        ),
        (
            Operation::ShaderCompile,
            vec![text("shader.fixture")],
            vec![("entry", text("main"))],
            HostTaskRequest::ShaderCompile(ShaderRequest {
                id: "shader.fixture".into(),
                entry: Some("main".into()),
            }),
        ),
        (
            Operation::AudioDecode,
            vec![text("audio.fixture")],
            vec![],
            HostTaskRequest::AudioDecode(AudioDecodeRequest {
                id: "audio.fixture".into(),
            }),
        ),
        (
            Operation::TtsSynthesis,
            vec![],
            vec![("text", text("test")), ("voice", text("fixture"))],
            HostTaskRequest::TtsSynthesis(TtsRequest {
                voice: Some("fixture".into()),
                text: "test".into(),
            }),
        ),
        (
            Operation::WasmCall,
            vec![text("module"), text("function"), RuntimeValue::Bool(false)],
            vec![],
            HostTaskRequest::WasmCall(WasmCallRequest {
                module: "module".into(),
                function: "function".into(),
                args: vec![RuntimePayload::new(RuntimeValue::Bool(false))],
            }),
        ),
        (
            Operation::SystemInfo(SystemInfoKind::AvailableParallelism),
            vec![],
            vec![],
            HostTaskRequest::SystemInfo(SystemInfoRequest {
                kind: SystemInfoKind::AvailableParallelism,
            }),
        ),
        (
            Operation::Custom {
                capability: "shader".into(),
                operation: "compile".into(),
            },
            vec![text("shader.fixture")],
            vec![("entry", text("main"))],
            HostTaskRequest::custom_with_named_args(
                "shader",
                "compile",
                [RuntimePayload::new(text("shader.fixture"))],
                [("entry".into(), RuntimePayload::new(text("main")))],
            ),
        ),
    ];
    let mut kinds = std::collections::BTreeSet::new();
    for (operation, positional, named, expected) in examples {
        let arguments = EvaluatedArguments {
            positional: positional.into(),
            named: named
                .into_iter()
                .map(|(name, value)| (name.into(), value))
                .collect(),
        };
        assert_eq!(arguments.project(&operation).unwrap(), expected);
        kinds.insert(operation.kind());
    }
    assert_eq!(kinds.len(), 14);
}

#[test]
fn original_template_rejects_malformed_supplied_fields() {
    use HostTaskTemplateOperation as Operation;
    for (name, value) in [
        ("status", text("201")),
        ("unexpected", RuntimeValue::u32(201)),
    ] {
        let arguments = EvaluatedArguments {
            positional: vec![text("request-1")].into(),
            named: vec![(name.into(), value)],
        };
        assert!(arguments.project(&Operation::HttpRespond).is_err());
    }
    let arguments = EvaluatedArguments {
        positional: vec![catalog(DeclarationIdentityFamily::Voice, "voice.alice.001")].into(),
        named: vec![],
    };
    assert!(
        arguments
            .project(&Operation::AssetLoad(AssetLoadKind::Image))
            .is_err()
    );
}
