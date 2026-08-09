use arcweft_compiler::source::compile_source;
use arcweft_core::awbc::schema::{AwbcEntryId, AwbcInstruction};
use arcweft_core::entry::{EntryBindingIdentity, RuntimeEntryRoles};
use arcweft_core::executor::ArcweftRuntimeExecutor;
use arcweft_core::plan::{
    EntryRuntimeId, FlowOp, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeIteratorEvidence, RuntimeIteratorWitnessExecutable,
};
use arcweft_core::pure::VmRuntimePureCallBackend;
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepStopReason,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

#[test]
fn source_iterator_witness_lowers_trait_methods_and_for_evidence() {
    let compiled = compile_source(include_str!(
        "../../../fixtures/iterator-witness/user-defined.arcw"
    ))
    .expect("source fixture compiles through admitted project lowering");
    let plan = &compiled.plan;

    assert_eq!(
        plan.trait_methods.len(),
        2,
        "into_iter and next are lowered"
    );
    let evidence = plan
        .flows
        .iter()
        .flat_map(|flow| flow.ops.iter())
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
        RuntimeIteratorWitnessExecutable::TraitCalls(_)
    ));

    let executable_plan = plan.clone().with_entries(vec![RuntimeEntrySpec {
        id: EntryRuntimeId::from_source_entity_body("entry.iterator_witness")
            .expect("test entry ID"),
        kind: RuntimeEntryKind::Cli,
        binding: EntryBindingIdentity::from_bytes([1; 32]),
        target: RuntimeEntryTarget::Flow(plan.flows[0].id.clone()),
        roles: RuntimeEntryRoles::None,
    }]);
    let awbc = AwbcLowerer::new(
        &executable_plan,
        &compiled.dialogue_content,
        "iterator-witness/user-defined.arcw",
    )
    .lower()
    .expect("generated trait methods lower to verified AWBC");
    assert_eq!(awbc.program.trait_methods.len(), 2);
    assert!(
        awbc.program
            .instructions
            .iter()
            .filter(|instruction| {
                matches!(instruction, AwbcInstruction::CallTraitMethod { .. })
            })
            .count()
            >= 2,
        "For lowering calls both into_iter and next"
    );
}

#[test]
fn source_iterator_identity_witness_executes_on_verified_awbc() {
    let compiled = compile_source(include_str!(
        "../../../fixtures/iterator-witness/identity.arcw"
    ))
    .expect("identity iterator fixture compiles through admitted project lowering");
    assert_eq!(compiled.plan.trait_methods.len(), 1, "next is lowered once");
    let evidence = compiled
        .plan
        .flows
        .iter()
        .flat_map(|flow| flow.ops.iter())
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
                RuntimeIteratorWitnessExecutable::IdentityIntoIterator(_)
            )
    ));

    let executable_plan = compiled.plan.clone().with_entries(vec![RuntimeEntrySpec {
        id: EntryRuntimeId::from_source_entity_body("entry.iterator_identity")
            .expect("test entry ID"),
        kind: RuntimeEntryKind::Cli,
        binding: EntryBindingIdentity::from_bytes([2; 32]),
        target: RuntimeEntryTarget::Flow(compiled.plan.flows[0].id.clone()),
        roles: RuntimeEntryRoles::None,
    }]);
    let awbc = AwbcLowerer::new(
        &executable_plan,
        &compiled.dialogue_content,
        "iterator-witness/identity.arcw",
    )
    .lower()
    .expect("identity witness lowers to verified AWBC");
    let mut executor = ArcweftRuntimeExecutor::from_awbc_product(awbc.program, AwbcEntryId(0))
        .expect("identity witness AWBC executor builds");
    let mut backend = VmRuntimePureCallBackend::default();
    let mut result = None;
    let mut observed_budget_yield = false;
    for _ in 0..8 {
        let step = executor.step_with_root_bindings_and_pure_backend(
            RuntimeStepInput::default(),
            &[],
            RuntimeStepOptions {
                mode: RuntimeStepMode::Drain,
                budget: RuntimeStepBudget { max_ops: 128 },
            },
            &mut backend,
        );
        observed_budget_yield |= step.stop_reason == RuntimeStepStopReason::BudgetExhausted;
        let terminal = !matches!(
            step.fiber_status,
            arcweft_core::engine::FlowFiberStatus::Running
        );
        result = Some(step);
        if terminal {
            break;
        }
    }
    let result = result.expect("identity witness executes at least one runtime step");
    assert!(
        observed_budget_yield,
        "identity witness fixture must exercise atomic trait-call budget retry"
    );
    assert!(
        matches!(
            result.fiber_status,
            arcweft_core::engine::FlowFiberStatus::Done(
                arcweft_core::engine::FlowExit::Return(ref value)
            ) if value == "2"
        ),
        "unexpected identity witness result: {result:#?}"
    );
}
