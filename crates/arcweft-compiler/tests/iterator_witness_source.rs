use arcweft_compiler::source::compile_source;
use arcweft_core::plan::{FlowOp, RuntimeIteratorEvidence, RuntimeIteratorWitnessExecutable};

#[test]
fn source_iterator_witness_lowers_trait_methods_and_for_evidence() {
    let compiled = compile_source(include_str!(
        "../../../fixtures/iterator-witness/user-defined.arcw"
    ))
    .expect("source fixture compiles through admitted project lowering");
    let plan = &compiled.plan;

    assert_eq!(
        plan.trait_methods().len(),
        2,
        "only the exact witnessed into_iter and next methods are lowered"
    );
    let method_local_domain = plan
        .nominal_record_domains()
        .domains()
        .find(|domain| matches!(domain.fields(), [field] if field.name() == "output"))
        .expect("the reached Iterator::next body retains its nominal local layout");
    assert!(
        plan.local_declarations()
            .declarations()
            .any(|local| local.ty() == method_local_domain.owner()),
        "the reached Iterator::next body local is admitted with its exact nominal type"
    );
    assert!(
        plan.nominal_record_domains()
            .domains()
            .all(|domain| domain.fields().iter().all(|field| field.name() != "unused")),
        "an unwitnessed Iterator impl does not re-enter runtime reachability"
    );
    let evidence = plan
        .flows()
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
        RuntimeIteratorWitnessExecutable::TraitCalls { .. }
    ));
}

#[test]
fn source_iterator_identity_witness_executes_on_verified_awbc() {
    let compiled = compile_source(include_str!(
        "../../../fixtures/iterator-witness/identity.arcw"
    ))
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
                RuntimeIteratorWitnessExecutable::IdentityIntoIterator { .. }
            )
    ));
}
