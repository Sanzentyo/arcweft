use arcweft_adapter_context::manifest::{AdapterHostCall, AdapterManifest};
use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_bundle::{
    ArcweftBundle, BundleAdapterHostCall, BundleAdapterManifest, BundleFormat, BundleManifest,
    BundleRuntimeSummary,
};
use arcweft_core::entry::{
    EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
    RuntimeFlowSchema,
};
use arcweft_core::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeAwaitTargetSeed, RuntimeEntryKind, RuntimeEntrySpec,
    RuntimeEntryTarget, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFlowSeed,
    RuntimeHostArgumentSeed, RuntimeHostTaskRequestTemplateSeed, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use arcweft_core::task::{HostCapabilityId, HostTaskRequest, NeedId, TaskId, TaskOutcomeContract};
use arcweft_core::value::{RuntimePayload, RuntimeValue};
use arcweft_host_adapter::{
    HostAdapter, HostAdapterError, HostTaskCompletion, HostTaskMetrics, HostTaskOutcome,
};
use arcweft_runtime_host::{
    BundleRunnerError, BundleRunnerOptions, BundleRunnerStepMode, NativeAdapterRegistrar,
    run_bundle_file_with_native_adapters, run_bundle_with_native_adapters,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_text_model::DialogueContentCatalog;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_runtime_artifact_fingerprint() -> arcweft_core::effect::RuntimeArtifactFingerprint {
    arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x6a; 32])
        .expect("fixture runtime artifact fingerprint is non-zero")
}

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::from_runtime_target_value(value).expect("test flow ID is valid")
}

fn cli_entry(entry: &str, flow: &str) -> RuntimeEntrySpec {
    RuntimeEntrySpec {
        id: EntryRuntimeId::from_source_entity_body(entry).expect("test entry ID is valid"),
        kind: RuntimeEntryKind::Cli,
        binding: EntryBindingIdentity::from_bytes([1; 32]),
        target: RuntimeEntryTarget::Flow(flow_id(flow)),
        roles: RuntimeEntryRoles::None,
    }
}

#[test]
fn bundle_runner_executes_custom_adapter_without_cli() {
    let bundle = custom_echo_bundle();
    let registrars: [NativeAdapterRegistrar; 1] =
        [|_, builder| builder.register(CustomEchoAdapter::new())];
    let report = run_bundle_with_native_adapters(
        &bundle,
        &BundleRunnerOptions {
            steps: 8,
            mode: BundleRunnerStepMode::Drain,
            ..BundleRunnerOptions::default()
        },
        &registrars,
    )
    .expect("custom adapter bundle runs");

    assert_eq!(report.source, "custom.arcw");
    assert_eq!(report.adapter_manifests, 1);
    assert_eq!(report.native_io.completed_tasks, 1);
    assert_eq!(report.native_io.failed_tasks, 0);
    assert_eq!(report.final_status, "done return custom-done");
    assert!(report.steps.iter().any(|step| step.task_requests == 1));
}

#[test]
fn bundle_runner_reports_custom_adapter_missing_from_host() {
    let bundle = custom_echo_bundle();
    let error = run_bundle_with_native_adapters(
        &bundle,
        &BundleRunnerOptions {
            steps: 8,
            mode: BundleRunnerStepMode::Drain,
            ..BundleRunnerOptions::default()
        },
        &[],
    )
    .expect_err("missing custom adapters are rejected before bundle execution");

    assert!(matches!(
        error,
        BundleRunnerError::NativeAdapter(HostAdapterError::MissingHostCallImplementations {
            host_call_ids
        }) if host_call_ids == vec!["custom.echo".to_owned()]
    ));
}

#[test]
fn bundle_runner_rejects_missing_exact_entry_selection_before_execution() {
    let mut bundle = custom_echo_bundle();
    bundle.manifest.entry = None;
    let registrars: [NativeAdapterRegistrar; 1] =
        [|_, builder| builder.register(CustomEchoAdapter::new())];

    let error = run_bundle_with_native_adapters(
        &bundle,
        &BundleRunnerOptions {
            steps: 8,
            mode: BundleRunnerStepMode::Drain,
            ..BundleRunnerOptions::default()
        },
        &registrars,
    )
    .expect_err("bundle without exact entry selection is rejected before execution");

    assert!(matches!(error, BundleRunnerError::MissingEntrySelection));
}

#[test]
fn bundle_file_runner_rejects_json_bytes_in_awfb_path() {
    let path = temp_bundle_path("legacy-json", "awfb");
    fs::write(
        &path,
        custom_echo_bundle()
            .to_format_bytes(BundleFormat::Json)
            .expect("legacy JSON encodes"),
    )
    .expect("fixture writes");

    let error = run_bundle_file_with_native_adapters(&path, &BundleRunnerOptions::default(), &[])
        .expect_err("AWFB product path must require AWFB magic");
    let _ = fs::remove_file(&path);

    assert!(matches!(
        error,
        BundleRunnerError::DecodeBundle(arcweft_bundle::BundleCodecError::DecodeAwfb {
            message
        }) if message.contains("magic")
    ));
}

#[test]
fn bundle_file_runner_requires_awfb_extension() {
    let path = temp_bundle_path("wrong-extension", "json");
    fs::write(
        &path,
        custom_echo_bundle()
            .to_format_bytes(BundleFormat::Awfb)
            .expect("AWFB encodes"),
    )
    .expect("fixture writes");

    let error = run_bundle_file_with_native_adapters(&path, &BundleRunnerOptions::default(), &[])
        .expect_err("product runner requires .awfb extension");
    let _ = fs::remove_file(&path);

    assert!(matches!(
        error,
        BundleRunnerError::ExpectedAwfbProduct { .. }
    ));
}

#[derive(Clone, Debug)]
struct CustomEchoAdapter {
    manifest: AdapterManifest,
}

impl CustomEchoAdapter {
    fn new() -> Self {
        Self {
            manifest: AdapterManifest::new("custom-echo", "Custom Echo")
                .with_host_call(AdapterHostCall::new("custom.echo", [])),
        }
    }
}

impl HostAdapter for CustomEchoAdapter {
    fn manifest(&self) -> &AdapterManifest {
        &self.manifest
    }

    fn complete(&self, task: &arcweft_core::task::TaskSpec) -> Option<HostTaskOutcome> {
        matches!(&task.request, HostTaskRequest::Custom { capability, operation, .. }
            if capability.0 == "custom" && operation == "echo")
        .then(|| HostTaskOutcome {
            completion: HostTaskCompletion::Ready(RuntimePayload::from("echo-ok")),
            metrics: HostTaskMetrics::default(),
        })
    }

    fn can_complete_in_parallel(&self, _request: &HostTaskRequest) -> bool {
        true
    }
}

fn custom_echo_bundle() -> ArcweftBundle {
    let string_ty = RuntimeSemanticTypeId::from_bytes([1; 32]);
    let flow = flow_id("flow.custom");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [RuntimePlanTypeSeed::new(
                string_ty,
                RuntimePlanTypeProjection::String,
            )],
            [],
            [],
            [],
        )
        .expect("string type admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            [],
            vec![
                RuntimeFlowOpSeed::Await {
                    binding: None,
                    target: RuntimeAwaitTargetSeed {
                        need: NeedId("need.custom.echo".to_owned()),
                        task: TaskId("task.custom.echo".to_owned()),
                        outcome: TaskOutcomeContract::new(RuntimeCheckedType::String),
                        request: RuntimeHostTaskRequestTemplateSeed {
                            capability: HostCapabilityId("custom".to_owned()),
                            operation: "echo".to_owned(),
                            args: vec![RuntimeHostArgumentSeed::Positional(RuntimeExprSeed::new(
                                string_ty,
                                RuntimeExprSeedKind::Value(RuntimeValue::String(
                                    "hello".to_owned(),
                                )),
                            ))],
                        },
                    },
                    observers: Vec::new(),
                },
                RuntimeFlowOpSeed::Return("custom-done".to_owned()),
            ],
        ))
        .expect("custom flow admits");
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: Vec::new(),
        })
        .expect("custom flow schema admits");
    builder
        .push_flow_executable(RuntimeFlowExecutable {
            flow: flow.clone(),
            contract: FlowContractHash::from_bytes([0x7c; 32]),
            controller: None,
        })
        .expect("custom flow executable admits");
    builder
        .push_entry(cli_entry("entry.custom", "flow.custom"))
        .expect("custom entry admits");
    let plan = builder.finish().expect("custom bundle plan is valid");
    let dialogue_content = DialogueContentCatalog::new();
    let product_awbc = AwbcLowerer::new(&plan, &dialogue_content, "custom.arcw")
        .lower()
        .expect("custom product AWBC lowers")
        .program;
    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some("entry.custom".to_owned()),
            adapter: Some("custom-echo".to_owned()),
            adapter_manifest_ids: vec!["custom-echo".to_owned()],
            required_host_calls: vec!["custom.echo".to_owned()],
            runtime: BundleRuntimeSummary {
                artifact_fingerprint: fixture_runtime_artifact_fingerprint(),
                entry_flow: Some("flow.custom".to_owned()),
                flows: product_awbc.flow_bindings.len(),
                bytecode_instructions: product_awbc.instructions.len(),
                line_task_groups: product_awbc.line_task_groups.len(),
                stream_plans: product_awbc.stream_plans.len(),
            },
        },
        source_map(
            "custom.arcw",
            "flow custom { await custom.echo(\"hello\") return \"custom-done\" }",
        ),
        product_awbc,
        dialogue_content,
    )
    .expect("standard dialogue source joins source map")
    .with_adapter_manifests([BundleAdapterManifest {
        id: "custom-echo".to_owned(),
        display_name: "Custom Echo".to_owned(),
        effects: Vec::new(),
        host_calls: vec![BundleAdapterHostCall {
            id: "custom.echo".to_owned(),
            effects: Vec::new(),
        }],
    }])
}

fn source_map(label: &str, text: &str) -> SourceMapSection {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new(label).expect("source ID"),
        SourceName::path(label),
        text,
    )
    .expect("source document");
    SourceMapSection::try_from_documents(&[&document]).expect("source map")
}

fn temp_bundle_path(label: &str, extension: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "arcweft-runtime-host-{label}-{}-{nanos}.{extension}",
        std::process::id()
    ))
}
