use arcweft_compiler::lower::lower_source_runtime_plan_with_typecheck_and_options;
use arcweft_core::plan::{FlowOp, RuntimeIteratorEvidence, RuntimeIteratorWitnessExecutable};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::{check::analyze_types, env::TypeCheckEnv};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;

#[test]
fn source_iterator_witness_lowers_trait_methods_and_for_evidence() {
    let parsed = parse_source(include_str!(
        "../../../fixtures/iterator-witness/user-defined.arcw"
    ));
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers to HIR");
    let typecheck = analyze_types(&hir, &TypeCheckEnv::default());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        typecheck.diagnostics
    );

    let plan = lower_source_runtime_plan_with_typecheck_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
    )
    .expect("source fixture lowers to runtime plan");

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
