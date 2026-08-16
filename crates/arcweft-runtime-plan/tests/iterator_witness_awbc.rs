use arcweft_core::awbc::schema::AwbcEntryId;
use arcweft_core::engine::{FlowExit, FlowFiberStatus};
use arcweft_core::entry::{EntryBindingIdentity, RuntimeEntryRoles};
use arcweft_core::executor::ArcweftRuntimeExecutor;
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeBuiltinIteratorEvidence, RuntimeEntryKind,
    RuntimeEntrySpec, RuntimeEntryTarget, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed,
    RuntimeFlowSeed, RuntimeIteratorEvidenceSeed, RuntimeLocalDeclarationSeed, RuntimePatternSeed,
    RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuilder, RuntimePlanSequenceKind,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use arcweft_core::pure::VmRuntimePureCallBackend;
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
};
use arcweft_core::value::{RuntimeSeq, RuntimeSignedIntWidth, RuntimeValue};
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

fn counter_plan() -> RuntimePlan {
    let item_type = type_id(1);
    let sequence_type = type_id(2);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
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
            ],
            [RuntimeLocalDeclarationSeed::new(item_type)],
            [],
            [],
        )
        .expect("test semantic facts admit");
    let item = admission.local_ids()[0].clone();
    let main = flow_id("iterator.main");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            main.clone(),
            [],
            vec![RuntimeFlowOpSeed::For {
                pattern: RuntimePatternSeed::new(
                    item_type,
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local: item.clone(),
                    },
                ),
                source: RuntimeExprSeed::new(
                    sequence_type,
                    RuntimeExprSeedKind::Value(RuntimeValue::Seq(RuntimeSeq::values(vec![
                        RuntimeValue::i64(0),
                        RuntimeValue::i64(1),
                    ]))),
                ),
                evidence: RuntimeIteratorEvidenceSeed::Builtin(RuntimeBuiltinIteratorEvidence::Vec),
                body: vec![RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                    item_type,
                    RuntimeExprSeedKind::Local(item),
                ))],
            }],
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
    let plan = counter_plan();
    let report = AwbcLowerer::new(&plan, &DialogueContentCatalog::new(), "iterator.arcw")
        .lower()
        .expect("builder-sealed iterator plan lowers to AWBC");

    let mut executor = ArcweftRuntimeExecutor::from_awbc_product(report.program, AwbcEntryId(0))
        .expect("AWBC product executor initializes");
    let mut pure_backend = VmRuntimePureCallBackend::default();
    let result = executor.step_with_root_bindings_and_pure_backend(
        RuntimeStepInput::default(),
        &[],
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 128 },
        },
        &mut pure_backend,
    );
    assert_eq!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return("0".to_owned()))
    );
}
