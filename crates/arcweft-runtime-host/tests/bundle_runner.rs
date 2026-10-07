use arcweft_adapter_context::manifest::{
    AdapterCallableGroupIndex, AdapterCallableParameterIndex, AdapterFunctionParam,
    AdapterFunctionSignature, AdapterHostCall, AdapterManifest, AdapterParameterGroup,
    AdapterParameterPassing, AdapterParameterPresence, AdapterTypeKind,
};
use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_bundle::{
    ArcweftBundle, BundleAdapterHostCall, BundleAdapterManifest, BundleFormat, BundleManifest,
    BundleRuntimeSummary,
};
use arcweft_core::entry::{
    EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
    RuntimeFlowSchema,
};
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeAwaitTargetSeed, RuntimeEntryKind, RuntimeEntrySpec,
    RuntimeEntryTarget, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFlowSeed,
    RuntimeLocalDeclarationSeed, RuntimeLocalReadSeed, RuntimeNeedProducerStartTargetSeed,
    RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
};
use arcweft_core::task::{
    CancelScopeId, HostCapabilityId, HostRestartPolicy, HostTaskRequest, LogicalEpoch,
    NeedProducerContractDigest, NeedProducerRequestProjection, NeedProducerSiteDigest,
    NeedProducerTaskPlan, TaskClass, TaskDispatchIdentity, TaskEvent, TaskEventKind, TaskPolicy,
    TaskPriority, TaskPublicationRevision, TaskSequence,
};
use arcweft_core::value::{RuntimeLocalReadMode, RuntimePayload, RuntimeValue};
use arcweft_host_adapter::{
    HostAdapter, HostAdapterError, HostAdapterRegistry, HostTaskCompletion, HostTaskMetrics,
    HostTaskOutcome,
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
fn runtime_plan_vm_starts_and_awaits_a_typed_need_producer() {
    let mut executor = ArcweftRuntimeExecutor::from_runtime_plan(
        custom_echo_plan(),
        ArcweftExecutionTier::RuntimePlanVm,
    )
    .expect("runtime-plan producer executor builds");
    let entry = EntryRuntimeId::from_source_entity_body("entry.custom")
        .expect("custom entry identity is valid");
    executor
        .start_structured_entry(&entry)
        .expect("custom entry starts");
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::Drain,
        budget: RuntimeStepBudget { max_ops: 16 },
        ..RuntimeStepOptions::default()
    };

    let started = executor.step(RuntimeStepInput::default(), options);
    assert_eq!(
        started.output.requests.tasks.len(),
        1,
        "producer start did not emit a task: stop={:?}, status={:?}, diagnostics={:?}",
        started.stop_reason,
        started.fiber_status,
        started.output.diagnostics
    );
    let task = started.output.requests.tasks[0].clone();
    assert!(matches!(
        &task.spec().request,
        HostTaskRequest::Custom { capability, operation, .. }
            if capability.0 == "custom" && operation == "echo"
    ));
    assert_eq!(
        executor.task_generation(&task.task_id()),
        Some(executor.generation())
    );
    assert_eq!(executor.restartable_dispatches().len(), 1);

    let event = TaskEvent::from_dispatch(
        TaskDispatchIdentity::new(task.handle().correlation, LogicalEpoch(1), TaskSequence(1)),
        TaskPublicationRevision::FIRST,
        TaskEventKind::Ready(RuntimePayload::new(RuntimeValue::String(
            "echo-ok".to_owned(),
        ))),
    );
    let completed = executor.step(
        RuntimeStepInput {
            task_events: vec![event],
            ..RuntimeStepInput::default()
        },
        options,
    );
    assert!(matches!(
        completed.fiber_status,
        arcweft_core::engine::FlowFiberStatus::Done(
            arcweft_core::engine::FlowExit::Return(ref value)
        ) if value == "custom-done"
    ));
    assert!(executor.restartable_dispatches().is_empty());
}

#[test]
fn bundle_file_runner_executes_decoded_need_producer_awbc() {
    let path = temp_bundle_path("decoded-need-producer", "awfb");
    fs::write(
        &path,
        custom_echo_bundle()
            .to_format_bytes(BundleFormat::Awfb)
            .expect("typed producer bundle encodes as AWFB"),
    )
    .expect("fixture writes");
    let registrars: [NativeAdapterRegistrar; 1] =
        [|_, builder| builder.register(CustomEchoAdapter::new())];
    let result = run_bundle_file_with_native_adapters(
        &path,
        &BundleRunnerOptions {
            steps: 8,
            mode: BundleRunnerStepMode::Drain,
            ..BundleRunnerOptions::default()
        },
        &registrars,
    );
    let _ = fs::remove_file(&path);
    let report = result.expect("decoded producer bundle runs");

    assert_eq!(
        report.native_io.completed_tasks, 1,
        "decoded producer run did not complete its task: {report:?}"
    );
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
        BundleRunnerError::ContainerArtifactIdentity(
            arcweft_bundle::container::ContainerError::BadMagic
        )
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
                .with_host_call(custom_echo_host_call()),
        }
    }
}

fn custom_echo_host_call() -> AdapterHostCall {
    let signature = AdapterFunctionSignature::try_new(
        vec![
            AdapterParameterGroup::try_new(
                AdapterCallableGroupIndex::try_from_usize(0).expect("initial group index fits"),
                vec![
                    AdapterFunctionParam::try_new(
                        AdapterCallableParameterIndex::try_from_usize(0)
                            .expect("initial parameter index fits"),
                        None,
                        AdapterTypeKind::String,
                        AdapterParameterPassing::PositionalOnly,
                        AdapterParameterPresence::Required,
                    )
                    .expect("custom echo positional parameter is valid"),
                ],
            )
            .expect("custom echo parameter group is valid"),
        ],
        AdapterTypeKind::Need {
            item: Box::new(AdapterTypeKind::String),
        },
    )
    .expect("typed host-call signature is valid");
    AdapterHostCall::with_signature("custom.echo", signature, [])
}

fn custom_echo_payload_type() -> RuntimeSemanticTypeId {
    HostAdapterRegistry::builder()
        .register(CustomEchoAdapter::new())
        .expect("custom echo adapter registers")
        .build()
        .host_call_result_type("custom.echo")
        .expect("custom echo result type is registered")
}

impl HostAdapter for CustomEchoAdapter {
    fn manifest(&self) -> &AdapterManifest {
        &self.manifest
    }

    fn complete(
        &self,
        task: &arcweft_core::task::TaskSpec,
        bound: &arcweft_core::task::BoundTaskOutcome,
    ) -> Option<HostTaskOutcome> {
        matches!(&task.request, HostTaskRequest::Custom { capability, operation, .. }
            if capability.0 == "custom" && operation == "echo")
        .then(|| HostTaskOutcome {
            completion: bound
                .try_payload(RuntimeValue::String("echo-ok".to_owned()))
                .map_or_else(
                    |error| HostTaskCompletion::Failed(error.to_string()),
                    HostTaskCompletion::Ready,
                ),
            metrics: HostTaskMetrics::default(),
        })
    }

    fn can_complete_in_parallel(&self, _request: &HostTaskRequest) -> bool {
        true
    }
}

fn custom_echo_plan() -> RuntimePlan {
    let string_ty = custom_echo_payload_type();
    let need_ty = RuntimeSemanticTypeId::from_bytes([2; 32]);
    let host_contract = custom_echo_host_call().contract_digest();
    let producer_plan = NeedProducerTaskPlan::try_new(
        NeedProducerContractDigest::from_bytes(*host_contract.as_bytes()),
        NeedProducerSiteDigest::from_bytes([0x4b; 32]),
        NeedProducerRequestProjection::ExternCapability {
            capability: HostCapabilityId("custom".to_owned()),
            operation: "echo".to_owned(),
            contract: host_contract,
            argument_names: Box::new([None]),
        },
        vec![string_ty].into_boxed_slice(),
        string_ty,
        TaskPolicy::AlwaysStart,
        HostRestartPolicy::Restartable,
        TaskClass::Io,
        TaskPriority(0),
        CancelScopeId("flow.custom".to_owned()),
    )
    .expect("custom host Need producer plan is valid");
    let flow = flow_id("flow.custom");
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(string_ty, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(need_ty, RuntimePlanTypeProjection::Need(string_ty)),
            ],
            [RuntimeLocalDeclarationSeed::new(
                manual_local_origin(
                    "arcweft-runtime-host.fixture.tests.bundle_runner.custom_echo_plan.binding_a",
                ),
                need_ty,
            )],
        )
        .expect("string type admits");
    let need_local = admitted.local_ids()[0].clone();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([61; 32]),
            flow.clone(),
            [],
            arcweft_core::plan::RuntimeEffectSet::empty(),
            vec![
                RuntimeFlowOpSeed::StartNeedProducer {
                    binding: RuntimePatternSeed::new(
                        need_ty,
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: need_local.clone(),
                        },
                    ),
                    target: RuntimeNeedProducerStartTargetSeed {
                        plan: producer_plan,
                        arguments: vec![arcweft_core::plan::RuntimeHostArgumentSeed::Positional(
                            arcweft_core::task::RuntimeRequestRoleIdentity::from_accepted_identity(
                                [42; 32],
                            ),
                            RuntimeExprSeed::new(
                                string_ty,
                                RuntimeExprSeedKind::Value(RuntimeValue::String(
                                    "hello".to_owned(),
                                )),
                            ),
                        )],
                    },
                },
                RuntimeFlowOpSeed::Await {
                    binding: None,
                    target: RuntimeAwaitTargetSeed {
                        source: RuntimeExprSeed::new(
                            need_ty,
                            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                                need_local,
                                RuntimeLocalReadMode::Move,
                            )),
                        ),
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
    builder.finish().expect("custom bundle plan is valid")
}

fn custom_echo_bundle() -> ArcweftBundle {
    let plan = custom_echo_plan();
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
            locale: arcweft_manifest_model::ProjectLocaleSpec::default(),
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
            "flow custom { let pending = custom.echo(\"hello\") await pending return \"custom-done\" }",
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

fn manual_local_origin(declaration: &str) -> arcweft_core::plan::RuntimeLocalOrigin {
    // This fixture declares a semantic binding name independent of its value,
    // type, source offset, and builder-issued local ordinal.
    let mut identity = blake3::Hasher::new();
    identity.update(b"arcweft.manual-fixture-binding.v1\0");
    identity.update(declaration.as_bytes());
    arcweft_core::plan::RuntimeLocalOrigin::Binding(*identity.finalize().as_bytes())
}
