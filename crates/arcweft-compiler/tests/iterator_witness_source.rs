use arcweft_compiler::source::compile_source;

#[path = "iterator_witness_source/bounded_calls.rs"]
mod bounded_calls;

use arcweft_core::plan::{
    FlowOp, RuntimeFunctionParameterPassing, RuntimeIteratorEvidence,
    RuntimeIteratorWitnessExecutable, RuntimePureInputType, RuntimeReceiverMode,
};

#[test]
fn source_iterator_witness_lowers_trait_methods_and_for_evidence() {
    let source = format!(
        "entry cli @entry.main {{ goto @flow.counter_demo }}\n{}",
        include_str!("../../../fixtures/iterator-witness/user-defined.arcw")
    );
    let compiled =
        compile_source(&source).expect("source fixture compiles through admitted project lowering");
    let plan = &compiled.plan;

    assert_eq!(
        plan.trait_methods().len(),
        2,
        "only the exact witnessed into_iter and next methods are lowered"
    );
    for method in plan.trait_methods() {
        let [receiver] = method.inputs.as_ref() else {
            panic!("witnessed method has one complete receiver formal")
        };
        assert_eq!(receiver.abi(), RuntimePureInputType::Value);
        let placement = plan
            .local_declarations()
            .get(receiver.local())
            .unwrap()
            .placement();
        assert_eq!(
            placement.owner(),
            arcweft_core::plan::RuntimeLocalOwner::TraitMethod(method.id)
        );
        assert_eq!(
            placement.initialization(),
            arcweft_core::plan::RuntimeLocalInitialization::CallableParameter { position: 0 }
        );
        assert_eq!(
            receiver.passing(),
            match method.receiver {
                RuntimeReceiverMode::Owned => RuntimeFunctionParameterPassing::Value,
                RuntimeReceiverMode::MutRef => RuntimeFunctionParameterPassing::Affine,
                RuntimeReceiverMode::SharedRef => panic!("fixture has no shared receiver"),
            }
        );
    }
    let method_local_domain = plan
        .nominal_record_domains()
        .domains()
        .find(|domain| matches!(domain.fields(), [field] if field.name() == Some("output")))
        .expect("the reached Iterator::next body retains its nominal local layout");
    assert!(
        plan.local_declarations()
            .declarations()
            .any(|local| local.ty() == method_local_domain.owner()),
        "the reached Iterator::next body local is admitted with its exact nominal type"
    );
    assert!(
        plan.nominal_record_domains().domains().all(|domain| domain
            .fields()
            .iter()
            .all(|field| field.name() != Some("unused"))),
        "an unwitnessed Iterator impl does not re-enter runtime reachability"
    );
    let evidence = plan
        .flows()
        .iter()
        .flat_map(|flow| flow.body().ops().iter())
        .find_map(|op| match op {
            FlowOp::For { evidence, .. } => Some(evidence),
            _ => None,
        })
        .expect("fixture flow keeps for-loop evidence");
    let RuntimeIteratorEvidence::Witness(witness) = evidence else {
        panic!("expected witness-backed for evidence, got {evidence:?}");
    };
    assert!(matches!(
        witness.executable,
        RuntimeIteratorWitnessExecutable::TraitCalls { .. }
    ));
    run_witness_plan(&compiled);
}

#[test]
fn source_iterator_identity_witness_executes_on_verified_awbc() {
    let source = format!(
        "entry cli @entry.main {{ goto @flow.identity_counter_demo }}\n{}",
        include_str!("../../../fixtures/iterator-witness/identity.arcw")
    );
    let compiled = compile_source(&source)
        .expect("identity iterator fixture compiles through admitted project lowering");
    assert_eq!(
        compiled.plan.trait_methods().len(),
        1,
        "next is lowered once"
    );
    let evidence = compiled
        .plan
        .flows()
        .iter()
        .flat_map(|flow| flow.body().ops().iter())
        .find_map(|op| match op {
            FlowOp::For { evidence, .. } => Some(evidence),
            _ => None,
        })
        .expect("identity fixture keeps for-loop evidence");
    assert!(matches!(
        evidence,
        RuntimeIteratorEvidence::Witness(witness)
            if matches!(
                witness.executable,
                RuntimeIteratorWitnessExecutable::IdentityIntoIterator { .. }
            )
    ));
    run_witness_plan(&compiled);
}

fn run_witness_plan(compiled: &arcweft_compiler::types::CompiledSource) {
    run_native_witness_plan(compiled.plan.clone());
    let program = arcweft_runtime_plan::awbc_lower::AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "iterator_witness_source.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let program = std::sync::Arc::new(program);
    let next = compiled
        .plan
        .trait_methods()
        .iter()
        .find(|method| method.receiver == RuntimeReceiverMode::MutRef)
        .unwrap();
    let state_type = compiled
        .plan
        .local_declarations()
        .get(next.inputs[0].local())
        .unwrap()
        .ty();
    let state_type = compiled
        .plan
        .type_table()
        .get(state_type)
        .unwrap()
        .semantic_identity();
    for owner in [
        arcweft_core::task::RuntimeProgramOwner::Plan(std::sync::Arc::new(compiled.plan.clone())),
        arcweft_core::task::RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program)),
    ] {
        round_trip_source_witness(&owner, state_type, next.id);
    }
    run_and_restore_awbc_witness_plan(program);
}

fn round_trip_source_witness(
    owner: &arcweft_core::task::RuntimeProgramOwner,
    state_type: arcweft_core::pattern::RuntimeSemanticTypeId,
    next: arcweft_core::plan::RuntimeTraitMethodId,
) {
    use arcweft_core::{
        entry::RuntimeSchemaLimits,
        value::{AwbcRuntimeValueSnapshot, RuntimeIterator, RuntimeValue},
    };
    let state = owner
        .types()
        .try_record_value(
            state_type,
            vec![RuntimeValue::i64(0), RuntimeValue::i64(3)],
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
    let state_image =
        AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&state, owner).unwrap();
    let limits = RuntimeSchemaLimits::engine_default();
    owner
        .types()
        .validate_inert_snapshot_value(state_type, &state_image, limits)
        .unwrap();
    assert!(
        owner
            .types()
            .validate_inert_snapshot_value(
                state_type,
                &AwbcRuntimeValueSnapshot::Bool(true),
                limits
            )
            .is_err()
    );
    let state = state_image
        .into_typed_runtime_value_for_program_with_limits(owner, state_type, limits)
        .unwrap();
    let value =
        RuntimeValue::Iterator(RuntimeIterator::try_witness(owner.clone(), state, next).unwrap());
    let image = AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, owner).unwrap();
    let bytes = serde_json::to_vec(&image).unwrap();
    let decoded: AwbcRuntimeValueSnapshot = serde_json::from_slice(&bytes).unwrap();
    // This for-loop keeps the nominal receiver and method signature rather
    // than a first-class Iterator row; do not invent a type for its wire.
    let restored = decoded.into_runtime_value_for_program(owner).unwrap();
    assert_eq!(
        restored, value,
        "the exact original receiver and method survive the inert wire"
    );
    assert!(RuntimeIterator::try_witness(owner.clone(), RuntimeValue::Bool(true), next).is_err());
    assert!(
        RuntimeIterator::try_witness(
            owner.clone(),
            RuntimeValue::Unit,
            arcweft_core::plan::RuntimeTraitMethodId(usize::MAX)
        )
        .is_err()
    );
    let mut forged = serde_json::to_value(&image).unwrap();
    forged["Iterator"]["Witness"]["state"] = serde_json::json!({"Bool":true});
    let forged: AwbcRuntimeValueSnapshot = serde_json::from_value(forged).unwrap();
    assert!(forged.into_runtime_value_for_program(owner).is_err());
}

fn run_native_witness_plan(plan: arcweft_core::plan::RuntimePlan) {
    use arcweft_core::{
        engine::{Engine, FlowExit, FlowFiberStatus},
        step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
    };
    let entry = plan.entries()[0].id.clone();
    let mut engine = Engine::for_entry(plan, &entry).unwrap();
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..RuntimeStepOptions::default()
    };
    for _ in 0..2048 {
        if matches!(engine.fiber().frame.status, FlowFiberStatus::Done(_)) {
            break;
        }
        let output = engine.step(RuntimeStepInput::default(), options);
        assert!(
            output.output.diagnostics.is_empty(),
            "{:?}",
            output.output.diagnostics
        );
    }
    assert_eq!(
        engine.fiber().frame.status,
        FlowFiberStatus::Done(FlowExit::Return("2".into()))
    );
}

fn run_and_restore_awbc_witness_plan(
    program: std::sync::Arc<arcweft_core::awbc::schema::AwbcProgram>,
) {
    use arcweft_core::{
        awbc::{
            product_step::{AwbcProductExecutorSaveSnapshot, AwbcProductStepExecutor},
            schema::AwbcEntryId,
        },
        engine::{FlowExit, FlowFiberStatus},
        step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
    };
    let mut executor =
        AwbcProductStepExecutor::for_entry_arc(program, AwbcEntryId(0), 4096).unwrap();
    let next_function = executor
        .program()
        .trait_methods
        .iter()
        .find(|method| method.receiver == arcweft_core::awbc::schema::AwbcTraitReceiverMode::MutRef)
        .unwrap()
        .function;
    let mut restored = false;
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..RuntimeStepOptions::default()
    };
    for _ in 0..2048 {
        if !restored && executor.compact_fiber().cursor.function == next_function {
            let snapshot = executor.snapshot_for_save().unwrap();
            let image =
                AwbcProductExecutorSaveSnapshot::from_live(&snapshot, executor.admitted_program())
                    .unwrap();
            let bytes = serde_json::to_vec(&image).unwrap();
            let decoded: AwbcProductExecutorSaveSnapshot = serde_json::from_slice(&bytes).unwrap();
            executor = executor
                .restore_inert_snapshot_owned(decoded)
                .unwrap_or_else(|(_, error)| panic!("witnessed iterator restore failed: {error}"));
            restored = true;
        }
        if matches!(executor.fiber().frame.status, FlowFiberStatus::Done(_)) {
            break;
        }
        let output = executor.step(RuntimeStepInput::default(), options);
        assert!(
            output.output.diagnostics.is_empty(),
            "{:?}",
            output.output.diagnostics
        );
    }
    assert!(
        restored,
        "the actual next method frame must stop and restore"
    );
    assert_eq!(
        executor.fiber().frame.status,
        FlowFiberStatus::Done(FlowExit::Return("2".into()))
    );
}
