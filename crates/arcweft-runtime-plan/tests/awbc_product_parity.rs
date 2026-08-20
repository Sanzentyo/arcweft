//! Product-AWBC parity coverage built exclusively through the sealed plan builder.

use arcweft_core::awbc::{codec::AwbcDecodeBudget, schema::AwbcEntryId};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::entry::{EntryBindingIdentity, RuntimeEntryRoles};
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    EntryRuntimeId, FlowEvent, FlowRuntimeId, RuntimeAwaitPendingObserverSeed,
    RuntimeAwaitTargetSeed, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFlowSeed,
    RuntimeHostTaskRequestTemplateSeed, RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlan,
    RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use arcweft_core::pure::VmRuntimePureCallBackend;
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
};
use arcweft_core::task::{
    HostCapabilityId, LogicalEpoch, NeedId, TaskEvent, TaskEventKind, TaskId, TaskOutcomeContract,
    TaskSequence,
};
use arcweft_core::value::{Progress, RuntimeValue};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_text_model::DialogueContentCatalog;

const STRING_TYPE: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([1; 32]);

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::canonical(value).expect("test flow ID is valid")
}

fn entry_id() -> EntryRuntimeId {
    EntryRuntimeId::canonical("parity.start").expect("test entry ID is valid")
}

fn string(value: &str) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        STRING_TYPE,
        RuntimeExprSeedKind::Value(RuntimeValue::String(value.to_owned())),
    )
}

fn plan_with_return(value: &str) -> RuntimePlan {
    let flow = flow_id("parity.main");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [RuntimePlanTypeSeed::new(
                STRING_TYPE,
                RuntimePlanTypeProjection::String,
            )],
            [],
            [],
            [],
        )
        .expect("semantic facts admit");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            [],
            vec![RuntimeFlowOpSeed::ReturnExpr(string(value))],
        ))
        .expect("flow admits");
    builder
        .push_entry(RuntimeEntrySpec {
            id: entry_id(),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([2; 32]),
            target: RuntimeEntryTarget::Flow(flow),
            roles: RuntimeEntryRoles::None,
        })
        .expect("entry admits");
    builder.finish().expect("builder seals plan")
}

fn plan_with_await_observer() -> RuntimePlan {
    let flow = flow_id("parity.await_observer");
    let progress_type = RuntimeSemanticTypeId::from_bytes([4; 32]);
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(STRING_TYPE, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(progress_type, RuntimePlanTypeProjection::Progress),
            ],
            [],
            [],
            [],
        )
        .expect("Await observer facts admit");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            [],
            vec![RuntimeFlowOpSeed::Await {
                binding: None,
                target: RuntimeAwaitTargetSeed {
                    need: NeedId("need.parity.observe".to_owned()),
                    task: TaskId("task.parity.observe".to_owned()),
                    outcome: TaskOutcomeContract::new(
                        arcweft_core::pattern::RuntimeCheckedType::String,
                    ),
                    request: RuntimeHostTaskRequestTemplateSeed {
                        capability: HostCapabilityId("test".to_owned()),
                        operation: "observe".to_owned(),
                        args: Vec::new(),
                    },
                },
                observers: vec![
                    RuntimeAwaitPendingObserverSeed {
                        pattern: RuntimePatternSeed::new(
                            progress_type,
                            RuntimePatternSeedKind::Discard,
                        ),
                        ops: vec![RuntimeFlowOpSeed::Return("first".to_owned())],
                    },
                    RuntimeAwaitPendingObserverSeed {
                        pattern: RuntimePatternSeed::new(
                            progress_type,
                            RuntimePatternSeedKind::Discard,
                        ),
                        ops: vec![RuntimeFlowOpSeed::Return("second".to_owned())],
                    },
                ],
            }],
        ))
        .expect("Await observer flow admits");
    builder
        .push_entry(RuntimeEntrySpec {
            id: entry_id(),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([2; 32]),
            target: RuntimeEntryTarget::Flow(flow),
            roles: RuntimeEntryRoles::None,
        })
        .expect("Await observer entry admits");
    builder.finish().expect("Await observer plan seals")
}

fn lower(plan: &RuntimePlan) -> arcweft_core::awbc::schema::AwbcProgram {
    AwbcLowerer::new(
        plan,
        &DialogueContentCatalog::new(),
        "awbc_product_parity.arcw",
    )
    .lower()
    .expect("sealed plan lowers to AWBC")
    .program
}

fn options() -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: RuntimeStepMode::Drain,
        budget: RuntimeStepBudget { max_ops: 32 },
    }
}

#[test]
fn typed_runtime_plan_and_product_awbc_return_the_same_value() {
    let plan = plan_with_return("done");
    let program = lower(&plan);
    let mut native =
        ArcweftRuntimeExecutor::from_runtime_plan(plan, ArcweftExecutionTier::RuntimePlanVm)
            .expect("runtime-plan VM builds");
    native
        .start_structured_entry(&entry_id())
        .expect("runtime-plan entry starts");
    let mut product = ArcweftRuntimeExecutor::from_awbc_product(program, AwbcEntryId(0))
        .expect("AWBC product builds");
    let mut native_backend = VmRuntimePureCallBackend::default();
    let mut product_backend = VmRuntimePureCallBackend::default();

    let native_result = native.step_with_root_bindings_and_pure_backend(
        RuntimeStepInput::default(),
        &[],
        options(),
        &mut native_backend,
    );
    let product_result = product.step_with_root_bindings_and_pure_backend(
        RuntimeStepInput::default(),
        &[],
        options(),
        &mut product_backend,
    );

    assert_eq!(product_result.output, native_result.output);
    assert_eq!(product_result.stop_reason, native_result.stop_reason);
    assert_eq!(product_result.fiber_status, native_result.fiber_status);
    assert!(matches!(
        product_result.fiber_status,
        FlowFiberStatus::Done(_)
    ));
}

#[test]
fn canonical_awbc_roundtrip_preserves_product_execution() {
    let program = lower(&plan_with_return("roundtrip"));
    let bytes = program
        .encode_canonical()
        .expect("AWBC encodes canonically");
    let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
        &bytes,
        AwbcDecodeBudget::default(),
    )
    .expect("canonical AWBC decodes");
    let mut executor = ArcweftRuntimeExecutor::from_awbc_product(decoded, AwbcEntryId(0))
        .expect("decoded AWBC product builds");
    let result = executor.step(RuntimeStepInput::default(), options());

    assert!(matches!(result.fiber_status, FlowFiberStatus::Done(_)));
}

#[test]
fn canonical_awbc_rejects_tampered_payload() {
    let mut bytes = lower(&plan_with_return("tamper"))
        .encode_canonical()
        .expect("AWBC encodes canonically");
    let last = bytes.last_mut().expect("AWBC payload is nonempty");
    *last ^= 0x80;

    assert!(
        arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
            &bytes,
            AwbcDecodeBudget::default(),
        )
        .is_err()
    );
}

#[test]
fn product_awbc_matches_first_progress_observer_and_consumes_publication_once() {
    let plan = plan_with_await_observer();
    let program = lower(&plan);
    let mut native =
        ArcweftRuntimeExecutor::from_runtime_plan(plan, ArcweftExecutionTier::RuntimePlanVm)
            .expect("runtime-plan VM builds");
    native
        .start_structured_entry(&entry_id())
        .expect("runtime-plan entry starts");
    let mut product = ArcweftRuntimeExecutor::from_awbc_product(program, AwbcEntryId(0))
        .expect("AWBC product builds");
    let _ = native.step(RuntimeStepInput::default(), options());
    let _ = product.step(RuntimeStepInput::default(), options());
    let publication = TaskEvent {
        logical_epoch: LogicalEpoch(1),
        task_id: TaskId("task.parity.observe".to_owned()),
        sequence: TaskSequence(1),
        kind: TaskEventKind::Progress(Progress::new(0.5).expect("fixture Progress is valid")),
    };

    let native_result = native.step(
        RuntimeStepInput {
            task_events: vec![publication.clone()],
            ..RuntimeStepInput::default()
        },
        options(),
    );
    let product_result = product.step(
        RuntimeStepInput {
            task_events: vec![publication],
            ..RuntimeStepInput::default()
        },
        options(),
    );

    for result in [&native_result, &product_result] {
        assert_eq!(
            result
                .output
                .flow_events
                .iter()
                .filter(|event| matches!(event, FlowEvent::AwaitProgress { .. }))
                .count(),
            1
        );
        assert!(
            matches!(
                result.fiber_status,
                FlowFiberStatus::Done(arcweft_core::engine::FlowExit::Return(ref value))
                    if value == "first"
            ),
            "unexpected Await observer status: {:?}; diagnostics: {:?}",
            result.fiber_status,
            result.output.diagnostics
        );
    }
}
