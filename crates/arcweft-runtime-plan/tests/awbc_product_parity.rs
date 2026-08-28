//! Product-AWBC parity coverage built exclusively through the sealed plan builder.

use std::sync::Arc;

use arcweft_core::awbc::{
    codec::AwbcDecodeBudget,
    product_step::evaluate_pure_program_with_backend,
    schema::AwbcEntryId,
    verify::{AwbcVerifyBudget, AwbcVerifyContext},
};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::entry::{
    EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
    RuntimeFlowSchema,
};
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    EntryRuntimeId, FlowEvent, FlowRuntimeId, RuntimeAwaitPendingObserverSeed,
    RuntimeAwaitTargetSeed, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFlowSeed,
    RuntimeFunctionSiteSeedId, RuntimeHostTaskRequestTemplateSeed, RuntimeLocalDeclarationSeed,
    RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuilder,
    RuntimePlanSequenceKind, RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimePureHelperId,
    RuntimePureHelperOrigin, RuntimePureHelperSeed, RuntimePureOutputType,
    RuntimePureProgramBindingSeed,
};
use arcweft_core::pure::{
    PureFunctionBackend, PureFunctionRequest, RuntimePureCallBackend, VmPureFunctionBackend,
    VmRuntimePureCallBackend,
};
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
};
use arcweft_core::task::{
    HostCapabilityId, LogicalEpoch, NeedId, TaskEvent, TaskEventKind, TaskId, TaskOutcomeContract,
    TaskSequence,
};
use arcweft_core::value::{
    Progress, RuntimeBinaryOp, RuntimeSeq, RuntimeSignedIntWidth, RuntimeStandardMapFamily,
    RuntimeStandardMapOperandOrder, RuntimeValue,
};
use arcweft_id::runtime_program::RuntimePureProgramId;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_text_model::DialogueContentCatalog;

const STRING_TYPE: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([1; 32]);

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::canonical(value).expect("test flow ID is valid")
}

fn type_id(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
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

fn standard_map_i64(value: i64) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        type_id(20),
        RuntimeExprSeedKind::Value(RuntimeValue::i64(value)),
    )
}

fn standard_map_source() -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::values(vec![
        RuntimeValue::i64(1),
        RuntimeValue::i64(2),
        RuntimeValue::i64(3),
    ]))
}

fn standard_map_seed(
    family: RuntimeStandardMapFamily,
    order: RuntimeStandardMapOperandOrder,
    function_ty: RuntimeSemanticTypeId,
    source_ty: RuntimeSemanticTypeId,
    result_ty: RuntimeSemanticTypeId,
    site: RuntimeFunctionSiteSeedId,
    source: RuntimeValue,
) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        result_ty,
        RuntimeExprSeedKind::StandardMap {
            family,
            order,
            mapping: Box::new(RuntimeExprSeed::new(
                function_ty,
                RuntimeExprSeedKind::Function(site),
            )),
            source: Box::new(RuntimeExprSeed::new(
                source_ty,
                RuntimeExprSeedKind::Value(source),
            )),
        },
    )
}

#[derive(Clone)]
struct AwbcStandardMapCase {
    helper: RuntimePureHelperId,
    program: RuntimePureProgramId,
    expected: RuntimeValue,
}

fn standard_map_awbc_plan() -> (Arc<RuntimePlan>, Vec<AwbcStandardMapCase>) {
    let item_ty = type_id(20);
    let error_ty = type_id(21);
    let function_ty = type_id(22);
    let vec_ty = type_id(23);
    let seq_ty = type_id(24);
    let array_ty = type_id(25);
    let slice_ty = type_id(26);
    let option_ty = type_id(27);
    let result_ty = type_id(28);
    let unit_ty = type_id(29);
    let item_payload_ty = type_id(30);
    let error_payload_ty = type_id(31);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(
                    item_ty,
                    RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
                ),
                RuntimePlanTypeSeed::new(error_ty, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    function_ty,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([item_ty]),
                        result: item_ty,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    vec_ty,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: item_ty,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    seq_ty,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Seq,
                        item: item_ty,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    array_ty,
                    RuntimePlanTypeProjection::Array {
                        item: item_ty,
                        length: 3,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    slice_ty,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Slice,
                        item: item_ty,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    item_payload_ty,
                    RuntimePlanTypeProjection::Tuple(Box::new([item_ty])),
                ),
                RuntimePlanTypeSeed::new(
                    error_payload_ty,
                    RuntimePlanTypeProjection::Tuple(Box::new([error_ty])),
                ),
                RuntimePlanTypeSeed::new(
                    option_ty,
                    RuntimePlanTypeProjection::Option {
                        item: item_ty,
                        some_payload: item_payload_ty,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    result_ty,
                    RuntimePlanTypeProjection::Result {
                        value: item_ty,
                        error: error_ty,
                        value_payload: item_payload_ty,
                        error_payload: error_payload_ty,
                    },
                ),
                RuntimePlanTypeSeed::new(unit_ty, RuntimePlanTypeProjection::Unit),
            ],
            [RuntimeLocalDeclarationSeed::new(item_ty)],
            [],
            [],
        )
        .expect("standard map AWBC type graph");
    let callback_local = admission.local_ids()[0].clone();
    let callback_site = builder
        .push_function_site_seed(
            [callback_local.clone()],
            [],
            RuntimeExprSeed::new(
                item_ty,
                RuntimeExprSeedKind::Binary {
                    lhs: Box::new(RuntimeExprSeed::new(
                        item_ty,
                        RuntimeExprSeedKind::Local(callback_local),
                    )),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(standard_map_i64(1)),
                },
            ),
        )
        .expect("standard map AWBC callback site");

    let cases = [
        (
            RuntimeStandardMapFamily::Vec,
            RuntimeStandardMapOperandOrder::MappingThenReceiver,
            vec_ty,
            vec_ty,
            standard_map_source(),
            RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::i64(2),
                RuntimeValue::i64(3),
                RuntimeValue::i64(4),
            ])),
        ),
        (
            RuntimeStandardMapFamily::Seq,
            RuntimeStandardMapOperandOrder::ReceiverThenMapping,
            seq_ty,
            seq_ty,
            standard_map_source(),
            RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::i64(2),
                RuntimeValue::i64(3),
                RuntimeValue::i64(4),
            ])),
        ),
        (
            RuntimeStandardMapFamily::Array,
            RuntimeStandardMapOperandOrder::MappingThenReceiver,
            array_ty,
            array_ty,
            standard_map_source(),
            RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::i64(2),
                RuntimeValue::i64(3),
                RuntimeValue::i64(4),
            ])),
        ),
        (
            RuntimeStandardMapFamily::Slice,
            RuntimeStandardMapOperandOrder::ReceiverThenMapping,
            slice_ty,
            vec_ty,
            standard_map_source(),
            RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::i64(2),
                RuntimeValue::i64(3),
                RuntimeValue::i64(4),
            ])),
        ),
        (
            RuntimeStandardMapFamily::Option,
            RuntimeStandardMapOperandOrder::MappingThenReceiver,
            option_ty,
            option_ty,
            RuntimeValue::option_some(RuntimeValue::i64(7)),
            RuntimeValue::option_some(RuntimeValue::i64(8)),
        ),
        (
            RuntimeStandardMapFamily::Option,
            RuntimeStandardMapOperandOrder::ReceiverThenMapping,
            option_ty,
            option_ty,
            RuntimeValue::option_none(),
            RuntimeValue::option_none(),
        ),
        (
            RuntimeStandardMapFamily::Result,
            RuntimeStandardMapOperandOrder::MappingThenReceiver,
            result_ty,
            result_ty,
            RuntimeValue::result_ok(RuntimeValue::i64(9)),
            RuntimeValue::result_ok(RuntimeValue::i64(10)),
        ),
        (
            RuntimeStandardMapFamily::Result,
            RuntimeStandardMapOperandOrder::ReceiverThenMapping,
            result_ty,
            result_ty,
            RuntimeValue::result_err(RuntimeValue::String("preserve".to_owned())),
            RuntimeValue::result_err(RuntimeValue::String("preserve".to_owned())),
        ),
    ];

    let mut expectations = Vec::with_capacity(cases.len());
    for (index, (family, order, source_ty, result_ty, source, expected)) in
        cases.into_iter().enumerate()
    {
        let helper = builder
            .push_pure_helper_seed(RuntimePureHelperSeed {
                name: format!("standard_map_awbc_{index}"),
                inputs: Box::new([]),
                input_abi: Vec::new(),
                output_abi: RuntimePureOutputType::Value,
                body: standard_map_seed(
                    family,
                    order,
                    function_ty,
                    source_ty,
                    result_ty,
                    callback_site.clone(),
                    source,
                ),
                scalar_eval_supported: false,
                origin: RuntimePureHelperOrigin::Annotated,
            })
            .expect("standard map AWBC helper");
        let program = RuntimePureProgramId::from_checked_digest([index as u8 + 1; 32]);
        builder
            .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program, helper })
            .expect("standard map AWBC pure-program binding");
        expectations.push((program, expected));
    }

    let flow = flow_id("standard_map.main");
    admit_flow_authority(&mut builder, &flow);
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            [],
            vec![RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                unit_ty,
                RuntimeExprSeedKind::Value(RuntimeValue::Unit),
            ))],
        ))
        .expect("standard map AWBC flow");
    builder
        .push_entry(RuntimeEntrySpec {
            id: entry_id(),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([3; 32]),
            target: RuntimeEntryTarget::Flow(flow),
            roles: RuntimeEntryRoles::None,
        })
        .expect("standard map AWBC entry");

    let plan = Arc::new(builder.finish().expect("standard map AWBC plan"));
    let cases = expectations
        .into_iter()
        .map(|(program, expected)| {
            let helper = plan
                .pure_programs()
                .iter()
                .find(|binding| binding.program() == program)
                .expect("standard map AWBC binding has its helper")
                .helper();
            AwbcStandardMapCase {
                helper,
                program,
                expected,
            }
        })
        .collect();
    (plan, cases)
}

fn admit_flow_authority(builder: &mut RuntimePlanBuilder, flow: &FlowRuntimeId) {
    builder
        .push_flow_executable(RuntimeFlowExecutable {
            flow: flow.clone(),
            contract: FlowContractHash::from_bytes([0xf1; 32]),
            controller: None,
        })
        .expect("flow executable admits");
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: Vec::new(),
        })
        .expect("flow schema admits");
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
    admit_flow_authority(&mut builder, &flow);
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
    admit_flow_authority(&mut builder, &flow);
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

    let native_result =
        native.step_with_pure_backend(RuntimeStepInput::default(), options(), &mut native_backend);
    let product_result = product.step_with_pure_backend(
        RuntimeStepInput::default(),
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

#[test]
fn product_awbc_standard_map_helpers_match_structured_results() {
    let (plan, cases) = standard_map_awbc_plan();
    let program = lower(&plan);
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("standard map AWBC product verifies");

    for case in cases {
        let structured = VmPureFunctionBackend
            .evaluate(
                &PureFunctionRequest::try_new(Arc::clone(&plan), case.helper, [])
                    .expect("standard map structured request"),
            )
            .expect("standard map structured execution")
            .value;
        assert_eq!(structured, case.expected);

        let mut product_backend = VmRuntimePureCallBackend::default();
        let product =
            evaluate_pure_program_with_backend(&program, case.program, &[], &mut product_backend)
                .expect("standard map AWBC helper execution");
        match (product, structured) {
            (RuntimeValue::Seq(product), RuntimeValue::Seq(structured)) => {
                assert_eq!(product.into_values(), structured.into_values());
            }
            (product, structured) => assert_eq!(product, structured),
        }
        assert_eq!(product_backend.stats().awbc_pure_program_calls, 1);
    }
}
