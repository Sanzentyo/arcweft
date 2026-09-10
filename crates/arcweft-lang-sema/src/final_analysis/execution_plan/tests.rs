//! Execution-plan publication requires consistent result and call evidence.

use std::collections::BTreeMap;

use crate::{
    effects::EffectSet,
    final_analysis::{
        CheckedExpression, CheckedExpressionResolution, CheckedTypeSelection,
        FinalSemanticAnalysisError,
        tests::{analyze, fixture},
    },
    types::TypeKind,
};

#[test]
fn call_execution_requires_result_availability_to_match_selection() {
    for (source, forged) in [
        (
            "fn target() -> i64 { 42i64 }\nfn caller() { target(); }\n",
            CheckedExpression::unavailable_call(),
        ),
        (
            "fn caller(speaker: Ref<Character>) { show(speaker, look = 1i64); }\n",
            CheckedExpression::value(
                TypeKind::Unit,
                CheckedTypeSelection::Inferred,
                EffectSet::new(),
                CheckedExpressionResolution::Call,
            ),
        ),
    ] {
        let fixture = fixture(source, None);
        let analysis = analyze(&fixture).expect("call evidence");
        let calls = analysis
            .calls()
            .map(|(owner, call)| (owner, call.clone()))
            .collect::<BTreeMap<_, _>>();
        let owner = *calls.keys().next().expect("one call");
        assert!(matches!(
            super::execution_plan_for_expression(owner, &forged, &calls),
            Err(FinalSemanticAnalysisError::CallFactMismatch)
        ));
    }
}
