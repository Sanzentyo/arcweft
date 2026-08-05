use arcweft_adapter_context::manifest::{AdapterHostCall, AdapterManifest};
use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_bundle::{
    ArcweftBundle, BundleAdapterHostCall, BundleAdapterManifest, BundleFormat, BundleManifest,
    BundleRuntimeSummary,
};
use arcweft_core::bytecode::{BYTECODE_ABI_VERSION, BytecodeProgram, BytecodeVerificationError};
use arcweft_core::entry::{EntryBindingIdentity, RuntimeEntryRoles};
use arcweft_core::plan::{
    EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeFlow, RuntimePlan,
};
use arcweft_core::task::{
    AwaitTarget, HostTaskArgTemplate, HostTaskRequest, HostTaskRequestTemplate, NeedId, TaskId,
};
use arcweft_core::value::{RuntimeExpr, RuntimePayload, RuntimeValue};
use arcweft_dialogue::DialogueProfileRevision;
use arcweft_host_adapter::{HostAdapter, HostAdapterError, HostTaskMetrics, HostTaskOutcome};
use arcweft_render_text::LineDisplayCatalog;
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_runtime_host::{
    BundleRunnerError, BundleRunnerExecutor, BundleRunnerOptions, BundleRunnerStepMode,
    NativeAdapterRegistrar, run_bundle_file_with_native_adapters, run_bundle_with_native_adapters,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn dialogue_revision() -> DialogueProfileRevision {
    let manifest = SourceDocument::try_new(
        SourceDocumentId::try_new("runtime-host-integration-test").expect("document ID"),
        SourceName::Memory,
        "test manifest",
    )
    .expect("test document");
    let sources =
        SourceSetRevision::try_for_identities([manifest.identity()]).expect("source revision");
    DialogueProfileRevision::from_admitted_parts(
        manifest.identity().clone(),
        sources,
        sources,
        ViewProgramId::try_new("view_program.runtime-host-integration-test")
            .expect("View program ID"),
        AcceptedViewProgramRevision::try_from_bytes([0x5a; 32]).expect("View program revision"),
        ResourceTypeRegistry::empty().digest(),
    )
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
fn bundle_runner_rejects_unverified_bytecode_before_execution() {
    let mut bundle = structured_custom_echo_bundle();
    bundle.bytecode.program.abi_version = BYTECODE_ABI_VERSION + 1;
    let registrars: [NativeAdapterRegistrar; 1] =
        [|_, builder| builder.register(CustomEchoAdapter::new())];

    let error = run_bundle_with_native_adapters(
        &bundle,
        &BundleRunnerOptions {
            steps: 8,
            executor: BundleRunnerExecutor::BytecodeVm,
            mode: BundleRunnerStepMode::Drain,
            ..BundleRunnerOptions::default()
        },
        &registrars,
    )
    .expect_err("invalid bytecode is rejected before execution");

    assert!(
        matches!(
            &error,
            BundleRunnerError::VerifyBytecode(BytecodeVerificationError::UnsupportedAbi {
                actual,
                expected,
            }) if *actual == BYTECODE_ABI_VERSION + 1 && *expected == BYTECODE_ABI_VERSION
        ),
        "unexpected invalid-bytecode error: {error:?}"
    );
}

#[test]
fn bundle_runner_rejects_missing_exact_entry_selection_before_execution() {
    let mut bundle = structured_custom_echo_bundle();
    bundle.manifest.entry = None;
    let registrars: [NativeAdapterRegistrar; 1] =
        [|_, builder| builder.register(CustomEchoAdapter::new())];

    let error = run_bundle_with_native_adapters(
        &bundle,
        &BundleRunnerOptions {
            steps: 8,
            executor: BundleRunnerExecutor::BytecodeVm,
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
            result: Ok(RuntimePayload::from("echo-ok")),
            metrics: HostTaskMetrics::default(),
        })
    }

    fn can_complete_in_parallel(&self, _request: &HostTaskRequest) -> bool {
        true
    }
}

fn custom_echo_bundle() -> ArcweftBundle {
    custom_echo_bundle_with_product_awbc(true)
}

fn structured_custom_echo_bundle() -> ArcweftBundle {
    custom_echo_bundle_with_product_awbc(false)
}

fn custom_echo_bundle_with_product_awbc(include_product_awbc: bool) -> ArcweftBundle {
    let plan = RuntimePlan::new(
        vec![RuntimeFlow {
            id: flow_id("flow.custom"),
            ops: vec![
                FlowOp::Await {
                    binding: None,
                    target: AwaitTarget::new(
                        NeedId("need.custom.echo".to_owned()),
                        TaskId("task.custom.echo".to_owned()),
                        HostTaskRequestTemplate::new(
                            "custom",
                            "echo",
                            [HostTaskArgTemplate::positional(RuntimeExpr::Value(
                                RuntimeValue::String("hello".to_owned()),
                            ))],
                        ),
                    ),
                    pending: Vec::new(),
                },
                FlowOp::Return("custom-done".to_owned()),
            ],
        }],
        Vec::new(),
    )
    .expect("custom bundle plan is valid")
    .with_entries(vec![cli_entry("entry.custom", "flow.custom")]);
    let display = LineDisplayCatalog::new(dialogue_revision());
    let product_awbc = include_product_awbc.then(|| {
        AwbcLowerer::new(&plan, &display, "custom.arcw")
            .lower()
            .expect("custom product AWBC lowers")
            .program
    });
    let program = BytecodeProgram::from_runtime_plan(plan);
    let stats = program.stats();
    let bundle = ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some("entry.custom".to_owned()),
            adapter: Some("custom-echo".to_owned()),
            adapter_manifest_ids: vec!["custom-echo".to_owned()],
            required_host_calls: vec!["custom.echo".to_owned()],
            runtime: BundleRuntimeSummary {
                entry_flow: Some("flow.custom".to_owned()),
                flows: stats.flows,
                bytecode_instructions: stats.instructions,
                line_task_groups: stats.line_task_groups,
                stream_plans: stats.stream_plans,
                source_plans: stats.source_plans,
            },
        },
        source_map(
            "custom.arcw",
            "flow custom { await custom.echo(\"hello\") return \"custom-done\" }",
        ),
        program,
        display,
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
    }]);

    if let Some(product_awbc) = product_awbc {
        bundle.with_product_awbc(product_awbc)
    } else {
        bundle
    }
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
