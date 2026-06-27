use arcweft_adapter_context::manifest::{AdapterHostCall, AdapterManifest};
use arcweft_bundle::{
    ArcweftBundle, BundleAdapterHostCall, BundleAdapterManifest, BundleFormat, BundleManifest,
    BundleRuntimeSummary, BundleSource,
};
use arcweft_core::bytecode::{BYTECODE_ABI_VERSION, BytecodeProgram, BytecodeVerificationError};
use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan};
use arcweft_core::task::{
    AwaitTarget, HostTaskArgTemplate, HostTaskRequest, HostTaskRequestTemplate, NeedId, TaskId,
};
use arcweft_core::value::{RuntimeExpr, RuntimePayload, RuntimeValue};
use arcweft_host_adapter::{HostAdapter, HostAdapterError, HostTaskMetrics, HostTaskOutcome};
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_host::{
    BundleRunnerError, BundleRunnerExecutor, BundleRunnerOptions, BundleRunnerStepMode,
    NativeAdapterRegistrar, run_bundle_file_with_native_adapters, run_bundle_with_native_adapters,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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

    assert!(matches!(
        error,
        BundleRunnerError::VerifyBytecode(BytecodeVerificationError::UnsupportedAbi {
            actual,
            expected,
        }) if actual == BYTECODE_ABI_VERSION + 1 && expected == BYTECODE_ABI_VERSION
    ));
}

#[test]
fn bundle_runner_rejects_missing_bytecode_entrypoint_before_execution() {
    let mut bundle = structured_custom_echo_bundle();
    bundle.bytecode.program.entry_flow = None;
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
    .expect_err("bytecode without an entrypoint is rejected before execution");

    assert!(matches!(
        error,
        BundleRunnerError::VerifyBytecode(BytecodeVerificationError::MissingEntrypoint)
    ));
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
        Some(FlowRuntimeId("flow.custom".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.custom".to_owned()),
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
    .expect("custom bundle plan is valid");
    let display = LineDisplayCatalog::default();
    let product_awbc = include_product_awbc.then(|| {
        AwbcLowerer::new(&plan, &display, "custom.arcw")
            .lower()
            .expect("custom product AWBC lowers")
            .program
    });
    let program = BytecodeProgram::from_runtime_plan(plan);
    let stats = program.stats();
    let bundle = ArcweftBundle::new(
        BundleManifest {
            source_label: "custom.arcw".to_owned(),
            profile_id: None,
            profile_kind: None,
            entry: None,
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
        BundleSource {
            label: "custom.arcw".to_owned(),
            text:
                "flow @flow.custom custom { await custom.echo(\"hello\") return \"custom-done\" }"
                    .to_owned(),
        },
        program,
        display,
    )
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
