use arcweft_core::awbc::schema::AwbcEntryId;
use arcweft_core::engine::{FlowExit, FlowFiberStatus};
use arcweft_core::entry::{
    EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
    RuntimeFlowSchema,
};
use arcweft_core::executor::ArcweftRuntimeExecutor;
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeBuiltinIteratorEvidenceSeed,
    RuntimeBuiltinIteratorFamily, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFlowSeed,
    RuntimeIteratorEvidenceSeed, RuntimeLocalDeclarationSeed, RuntimeLocalReadSeed,
    RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuilder,
    RuntimePlanSequenceKind, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use arcweft_core::pure::VmRuntimePureCallBackend;
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
};
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeLocalReadMode, RuntimeSeq, RuntimeSignedIntWidth, RuntimeValue,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_text_model::DialogueContentCatalog;

fn type_id(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::canonical(value).expect("test flow ID is valid")
}

fn entry_id(value: &str) -> EntryRuntimeId {
    EntryRuntimeId::canonical(value).expect("test entry ID is valid")
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum IteratorReturn {
    First,
    Second,
    Exhausted,
}

#[allow(
    clippy::too_many_lines,
    reason = "The iterator paths share one sealed type, flow, and entry fixture."
)]
fn counter_plan(return_when: IteratorReturn) -> RuntimePlan {
    let item_type = type_id(1);
    let sequence_type = type_id(2);
    let iterator_type = type_id(3);
    let next_value_type = type_id(4);
    let step_type = type_id(5);
    let next_payload_type = type_id(6);
    let bool_type = type_id(7);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    item_type,
                    RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
                ),
                RuntimePlanTypeSeed::new(
                    sequence_type,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: item_type,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    iterator_type,
                    RuntimePlanTypeProjection::Iterator(item_type),
                ),
                RuntimePlanTypeSeed::new(
                    next_payload_type,
                    RuntimePlanTypeProjection::Tuple(Box::new([item_type])),
                ),
                RuntimePlanTypeSeed::new(
                    next_value_type,
                    RuntimePlanTypeProjection::Option {
                        item: item_type,
                        some_payload: next_payload_type,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    step_type,
                    RuntimePlanTypeProjection::Tuple(vec![iterator_type, next_value_type].into()),
                ),
                RuntimePlanTypeSeed::new(bool_type, RuntimePlanTypeProjection::Bool),
            ],
            [RuntimeLocalDeclarationSeed::new(
                manual_local_origin("arcweft-runtime-plan.fixture.tests.iterator_witness_awbc.counter_plan.binding_a"),
                item_type,
            )],
        )
        .expect("test semantic facts admit");
    let item = admission.local_ids()[0].clone();
    let main = flow_id("iterator.main");
    builder
        .push_flow_executable(RuntimeFlowExecutable {
            flow: main.clone(),
            contract: FlowContractHash::from_bytes([0xf2; 32]),
            controller: None,
        })
        .expect("typed flow executable admits");
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: main.clone(),
            parameters: Vec::new(),
        })
        .expect("typed flow schema admits");
    let item_read = || {
        RuntimeExprSeed::new(
            item_type,
            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                item.clone(),
                RuntimeLocalReadMode::Copy,
            )),
        )
    };
    let body = match return_when {
        IteratorReturn::First => vec![RuntimeFlowOpSeed::ReturnExpr(item_read())],
        IteratorReturn::Second => vec![RuntimeFlowOpSeed::If {
            condition: RuntimeExprSeed::new(
                bool_type,
                RuntimeExprSeedKind::Binary {
                    lhs: Box::new(item_read()),
                    op: RuntimeBinaryOp::Eq,
                    rhs: Box::new(RuntimeExprSeed::new(
                        item_type,
                        RuntimeExprSeedKind::Value(RuntimeValue::i64(1)),
                    )),
                },
            ),
            then_ops: vec![RuntimeFlowOpSeed::ReturnExpr(item_read())],
            else_ops: Vec::new(),
        }],
        IteratorReturn::Exhausted => Vec::new(),
    };
    let mut ops = vec![RuntimeFlowOpSeed::For {
        pattern: RuntimePatternSeed::new(
            item_type,
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local: item,
            },
        ),
        source: RuntimeExprSeed::new(
            sequence_type,
            RuntimeExprSeedKind::Value(RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::i64(0),
                RuntimeValue::i64(1),
            ]))),
        ),
        evidence: RuntimeIteratorEvidenceSeed::Builtin(RuntimeBuiltinIteratorEvidenceSeed {
            family: RuntimeBuiltinIteratorFamily::Vec,
            item: item_type,
            iterator: iterator_type,
            next_value: next_value_type,
            step: step_type,
        }),
        body,
    }];
    if return_when != IteratorReturn::First {
        ops.push(RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
            item_type,
            RuntimeExprSeedKind::Value(RuntimeValue::i64(-1)),
        )));
    }
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([61; 32]),
            main.clone(),
            [],
            arcweft_core::plan::RuntimeEffectSet::empty(),
            ops,
        ))
        .expect("typed flow seed admits");
    builder
        .push_entry(RuntimeEntrySpec {
            id: entry_id("iterator"),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([1; 32]),
            target: RuntimeEntryTarget::Flow(main),
            roles: RuntimeEntryRoles::None,
        })
        .expect("entry admits");
    builder.finish().expect("typed runtime plan seals")
}

#[test]
fn builtin_iterator_lowers_and_executes_on_awbc_product_vm() {
    let plan = counter_plan(IteratorReturn::First);
    let report = AwbcLowerer::new(&plan, &DialogueContentCatalog::new(), "iterator.arcw")
        .lower()
        .expect("builder-sealed iterator plan lowers to AWBC");

    let mut executor = ArcweftRuntimeExecutor::from_awbc_product(report.program, AwbcEntryId(0))
        .expect("AWBC product executor initializes");
    let mut pure_backend = VmRuntimePureCallBackend::default();
    let result = executor.step_with_pure_backend(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 128 },
            ..RuntimeStepOptions::default()
        },
        &mut pure_backend,
    );
    assert_eq!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return("0".to_owned()))
    );
}

#[test]
fn builtin_iterator_reuses_owned_state_on_backedge() {
    let plan = counter_plan(IteratorReturn::Second);
    let report = AwbcLowerer::new(&plan, &DialogueContentCatalog::new(), "iterator.arcw")
        .lower()
        .expect("builder-sealed iterator plan lowers to AWBC");

    let mut executor = ArcweftRuntimeExecutor::from_awbc_product(report.program, AwbcEntryId(0))
        .expect("AWBC product executor initializes");
    let mut pure_backend = VmRuntimePureCallBackend::default();
    let result = executor.step_with_pure_backend(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 128 },
            ..RuntimeStepOptions::default()
        },
        &mut pure_backend,
    );
    assert_eq!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return("1".to_owned()))
    );
}

#[test]
fn builtin_iterator_exits_after_owned_source_is_exhausted() {
    let plan = counter_plan(IteratorReturn::Exhausted);
    let report = AwbcLowerer::new(&plan, &DialogueContentCatalog::new(), "iterator.arcw")
        .lower()
        .expect("builder-sealed iterator plan lowers to AWBC");

    let mut executor = ArcweftRuntimeExecutor::from_awbc_product(report.program, AwbcEntryId(0))
        .expect("AWBC product executor initializes");
    let mut pure_backend = VmRuntimePureCallBackend::default();
    let result = executor.step_with_pure_backend(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 128 },
            ..RuntimeStepOptions::default()
        },
        &mut pure_backend,
    );
    assert_eq!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return("-1".to_owned()))
    );
}

fn manual_local_origin(declaration: &str) -> arcweft_core::plan::RuntimeLocalOrigin {
    // This fixture declares a semantic binding name independent of its value,
    // type, source offset, and builder-issued local ordinal.
    let mut identity = blake3::Hasher::new();
    identity.update(b"arcweft.manual-fixture-binding.v1\0");
    identity.update(declaration.as_bytes());
    arcweft_core::plan::RuntimeLocalOrigin::Binding(*identity.finalize().as_bytes())
}
