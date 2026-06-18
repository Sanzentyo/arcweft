use arcweft_adapter_context::manifest::{AdapterHostCall, AdapterManifest};
use arcweft_bundle::{
    ArcweftBundle, BundleAdapterHostCall, BundleAdapterManifest, BundleManifest,
    BundleRuntimeSummary, BundleSource,
};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan};
use arcweft_core::task::{
    AwaitTarget, HostTaskArgTemplate, HostTaskRequest, HostTaskRequestTemplate, NeedId, TaskId,
};
use arcweft_core::value::{RuntimeExpr, RuntimePayload, RuntimeValue};
use arcweft_host_adapter::{HostAdapter, HostTaskMetrics, HostTaskOutcome};
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_host::{
    BundleRunnerOptions, BundleRunnerStepMode, NativeAdapterRegistrar,
    run_bundle_with_native_adapters,
};

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
    let report = run_bundle_with_native_adapters(
        &bundle,
        &BundleRunnerOptions {
            steps: 8,
            mode: BundleRunnerStepMode::Drain,
            ..BundleRunnerOptions::default()
        },
        &[],
    )
    .expect("bundle runner reports runtime diagnostics");

    assert_eq!(report.native_io.completed_tasks, 0);
    assert_eq!(report.native_io.failed_tasks, 1);
    assert!(report.steps.iter().any(|step| {
        step.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .contains("host call `custom.echo` is provided by the active adapter manifest")
        })
    }));
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
    let program = BytecodeProgram::from_runtime_plan(plan);
    let stats = program.stats();
    ArcweftBundle::new(
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
        LineDisplayCatalog::default(),
    )
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
