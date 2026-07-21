use arcweft_compiler::source::compile_source;
use arcweft_core::plan::{FlowOp, RuntimeIteratorEvidence, RuntimeIteratorWitnessExecutable};

#[test]
fn source_iterator_witness_lowers_trait_methods_and_for_evidence() {
    let compiled = compile_source(include_str!(
        "../../../fixtures/iterator-witness/user-defined.arcw"
    ))
    .expect("source fixture compiles through admitted project lowering");
    let plan = compiled.plan;

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
}
