use arcweft_core::awbc::schema::{
    AwbcEntryId, AwbcFunctionKind, AwbcInstruction, AwbcRegisterId, AwbcTraitReceiverMode,
};
use arcweft_core::entry::{EntryBindingIdentity, RuntimeEntryRoles};
use arcweft_core::executor::ArcweftRuntimeExecutor;
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::plan::{
    EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeFlow, RuntimeIteratorEvidence, RuntimeIteratorIdentityWitnessCalls,
    RuntimeIteratorWitnessCalls, RuntimeIteratorWitnessEvidence, RuntimeIteratorWitnessExecutable,
    RuntimePlan, RuntimePureInputType, RuntimePureOutputType, RuntimeReceiverMode,
    RuntimeTraitMethod, RuntimeTraitMethodId, RuntimeTraitMethodIdentity,
};
use arcweft_core::pure::VmRuntimePureCallBackend;
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
};
use arcweft_core::value::{RuntimeBinaryOp, RuntimeExpr, RuntimeValue};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_text_model::DialogueContentCatalog;

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::from_runtime_target_value(value).expect("test flow ID is valid")
}

fn with_test_entry(plan: RuntimePlan) -> RuntimePlan {
    plan.with_entries(vec![RuntimeEntrySpec {
        id: EntryRuntimeId::from_source_entity_body("entry.iterator_witness")
            .expect("test entry ID is valid"),
        kind: RuntimeEntryKind::Cli,
        binding: EntryBindingIdentity::from_bytes([1; 32]),
        target: RuntimeEntryTarget::Flow(flow_id("flow.main")),
        roles: RuntimeEntryRoles::None,
    }])
}

#[test]
fn awbc_program_carries_trait_method_table_entries() {
    let plan = counter_witness_plan();
    let program = lower_plan(&plan);

    assert_eq!(program.trait_methods.len(), 2);
    assert_eq!(
        program.trait_methods[0].receiver,
        AwbcTraitReceiverMode::Owned
    );
    assert_eq!(
        program.trait_methods[1].receiver,
        AwbcTraitReceiverMode::MutRef
    );
    assert_eq!(
        program.trait_methods[1].receiver_state_slot,
        Some(AwbcRegisterId(0))
    );
    assert!(program.trait_methods.iter().all(|method| {
        program.functions[method.function.index()].kind == AwbcFunctionKind::TraitMethod
            && program.functions[method.function.index()].signature == method.signature
    }));
}

#[test]
fn witness_for_lowers_to_trait_method_calls() {
    let plan = counter_witness_plan();
    let program = lower_plan(&plan);

    let calls = program
        .instructions
        .iter()
        .filter(|instruction| matches!(instruction, AwbcInstruction::CallTraitMethod { .. }))
        .count();
    assert!(
        calls >= 2,
        "expected into_iter and next calls: {program:#?}"
    );
    assert!(program.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            AwbcInstruction::CallTraitMethod {
                receiver_out: Some(_),
                ..
            }
        )
    }));
}

#[test]
fn identity_witness_for_executes_on_awbc_product_vm() {
    let plan = counter_identity_witness_plan();
    let program = lower_plan(&plan);
    assert_eq!(program.trait_methods.len(), 1);

    let mut executor = ArcweftRuntimeExecutor::from_awbc_product(program, AwbcEntryId(0))
        .expect("AWBC product executor builds");
    let mut backend = VmRuntimePureCallBackend::default();
    let result = executor.step_with_root_bindings_and_pure_backend(
        RuntimeStepInput::default(),
        &[],
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 32 },
        },
        &mut backend,
    );

    assert!(
        matches!(
            result.fiber_status,
            arcweft_core::engine::FlowFiberStatus::Done(
                arcweft_core::engine::FlowExit::Return(ref value)
            ) if value == "0"
        ),
        "unexpected AWBC runtime result: {result:#?}"
    );
}

fn lower_plan(plan: &RuntimePlan) -> arcweft_core::awbc::schema::AwbcProgram {
    AwbcLowerer::new(
        plan,
        &DialogueContentCatalog::new(),
        "iterator_witness.arcw",
    )
    .lower()
    .expect("runtime plan lowers to verified AWBC")
    .program
}

fn counter_trait_identity(id: usize, method_name: &str) -> RuntimeTraitMethodIdentity {
    RuntimeTraitMethodIdentity {
        impl_id: id,
        trait_id: Some(id),
        witness: Some(id),
        trait_name: Some(
            if method_name == "next" {
                "Iterator"
            } else {
                "IntoIterator"
            }
            .to_owned(),
        ),
        self_type: "CounterIter".to_owned(),
        method_name: method_name.to_owned(),
        monomorph_label: format!("CounterIter::{method_name}"),
    }
}

fn counter_state() -> RuntimeValue {
    RuntimeValue::try_record(vec![
        ("current".to_owned(), RuntimeValue::i64(0)),
        ("end".to_owned(), RuntimeValue::i64(1)),
    ])
    .expect("test record fields are unique")
}

fn self_field(field: &str) -> RuntimeExpr {
    RuntimeExpr::Field {
        target: Box::new(RuntimeExpr::Local("self".to_owned())),
        field: field.to_owned(),
    }
}

fn counter_next_body() -> RuntimeExpr {
    RuntimeExpr::If {
        condition: Box::new(RuntimeExpr::Binary {
            lhs: Box::new(self_field("current")),
            op: RuntimeBinaryOp::Lt,
            rhs: Box::new(self_field("end")),
        }),
        then_expr: Box::new(RuntimeExpr::Let {
            name: "value".to_owned(),
            expr: Box::new(self_field("current")),
            body: Box::new(RuntimeExpr::AssignField {
                target: Box::new(RuntimeExpr::Local("self".to_owned())),
                field: "current".to_owned(),
                expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("value".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(1))),
                }),
                body: Box::new(RuntimeExpr::Variant {
                    owner: arcweft_core::pattern::RuntimeCheckedType::Option(Box::new(
                        arcweft_core::pattern::RuntimeCheckedType::Signed(
                            arcweft_core::value::RuntimeSignedIntWidth::I64,
                        ),
                    )),
                    ordinal: 0,
                    name: "Some".to_owned(),
                    payload: Some(Box::new(RuntimeExpr::Local("value".to_owned()))),
                }),
            }),
        }),
        else_expr: Box::new(RuntimeExpr::Variant {
            owner: arcweft_core::pattern::RuntimeCheckedType::Option(Box::new(
                arcweft_core::pattern::RuntimeCheckedType::Signed(
                    arcweft_core::value::RuntimeSignedIntWidth::I64,
                ),
            )),
            ordinal: 1,
            name: "None".to_owned(),
            payload: None,
        }),
    }
}

fn counter_trait_methods() -> Vec<RuntimeTraitMethod> {
    vec![
        RuntimeTraitMethod {
            id: RuntimeTraitMethodId(0),
            identity: counter_trait_identity(0, "into_iter"),
            receiver: RuntimeReceiverMode::Owned,
            input_names: vec!["self".to_owned()],
            input_types: vec![RuntimePureInputType::Value],
            output_type: RuntimePureOutputType::Value,
            body: RuntimeExpr::Local("self".to_owned()),
        },
        RuntimeTraitMethod {
            id: RuntimeTraitMethodId(1),
            identity: counter_trait_identity(1, "next"),
            receiver: RuntimeReceiverMode::MutRef,
            input_names: vec!["self".to_owned()],
            input_types: vec![RuntimePureInputType::Value],
            output_type: RuntimePureOutputType::Value,
            body: counter_next_body(),
        },
    ]
}

fn counter_witness_plan() -> RuntimePlan {
    with_test_entry(
        RuntimePlan::new(
            vec![RuntimeFlow {
                id: flow_id("flow.main"),
                ops: vec![FlowOp::For {
                    pattern: RuntimePattern::Ident("item".to_owned()),
                    source: RuntimeExpr::Value(counter_state()),
                    evidence: RuntimeIteratorEvidence::Witness(RuntimeIteratorWitnessEvidence {
                        item_type: "i64".to_owned(),
                        into_iter_type: "CounterIter".to_owned(),
                        executable: RuntimeIteratorWitnessExecutable::TraitCalls(
                            RuntimeIteratorWitnessCalls {
                                into_iter: RuntimeTraitMethodId(0),
                                next: RuntimeTraitMethodId(1),
                            },
                        ),
                    }),
                    body: vec![FlowOp::ReturnExpr(RuntimeExpr::Local("item".to_owned()))],
                }],
            }],
            Vec::new(),
        )
        .expect("flow plan is valid"),
    )
    .with_trait_methods(counter_trait_methods())
}

fn counter_identity_trait_methods() -> Vec<RuntimeTraitMethod> {
    vec![RuntimeTraitMethod {
        id: RuntimeTraitMethodId(0),
        identity: counter_trait_identity(0, "next"),
        receiver: RuntimeReceiverMode::MutRef,
        input_names: vec!["self".to_owned()],
        input_types: vec![RuntimePureInputType::Value],
        output_type: RuntimePureOutputType::Value,
        body: counter_next_body(),
    }]
}

fn counter_identity_witness_plan() -> RuntimePlan {
    with_test_entry(
        RuntimePlan::new(
            vec![RuntimeFlow {
                id: flow_id("flow.main"),
                ops: vec![FlowOp::For {
                    pattern: RuntimePattern::Ident("item".to_owned()),
                    source: RuntimeExpr::Value(counter_state()),
                    evidence: RuntimeIteratorEvidence::Witness(RuntimeIteratorWitnessEvidence {
                        item_type: "i64".to_owned(),
                        into_iter_type: "Counter".to_owned(),
                        executable: RuntimeIteratorWitnessExecutable::IdentityIntoIterator(
                            RuntimeIteratorIdentityWitnessCalls {
                                next: RuntimeTraitMethodId(0),
                            },
                        ),
                    }),
                    body: vec![FlowOp::ReturnExpr(RuntimeExpr::Local("item".to_owned()))],
                }],
            }],
            Vec::new(),
        )
        .expect("flow plan is valid"),
    )
    .with_trait_methods(counter_identity_trait_methods())
}
