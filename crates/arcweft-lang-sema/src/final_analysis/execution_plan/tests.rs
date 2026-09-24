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
            super::execution_plan_for_expression(owner, &forged, &calls, &BTreeMap::new(),),
            Err(FinalSemanticAnalysisError::CallFactMismatch)
        ));
    }
}

#[test]
fn dialogue_target_calls_keep_their_value_and_selected_runtime_application() {
    use crate::final_analysis::{CheckedRuntimeValueDisposition, CheckedStructuralExecutionReason};

    for source in [
        r#"
pub character alice {}
flow root { alice(source_locale = "ja-JP")[Hello] }
"#,
        r#"
pub character alice {}
flow root {
    let dialogue = alice()
    dialogue(source_locale = "ja-JP")[Hello]
}
"#,
        r#"
pub character alice {}
fn target() -> CharacterDialogue { alice() }
flow root { target()[Hello] }
"#,
    ] {
        let fixture = fixture(source, None);
        let analysis = analyze(&fixture).expect("checked Dialogue target");
        let (application, target) = analysis
            .expressions()
            .find_map(|(_, expression)| match expression.resolution() {
                CheckedExpressionResolution::DialogueApplication { target, .. } => {
                    Some((expression, target))
                }
                _ => None,
            })
            .expect("one line application");
        let target_expression = analysis
            .expression(target.expression())
            .expect("whole target expression");
        assert_eq!(target_expression.value_type(), Some(&target.ty()));
        let target_plan = target_expression
            .execution_plan()
            .expect("target execution");
        assert_eq!(target_plan.value(), CheckedRuntimeValueDisposition::Retain);
        assert!(target_plan.executes_as_runtime_call());
        assert_eq!(
            target_plan.call_application(),
            Some(
                analysis
                    .call(target.expression())
                    .expect("target call fact")
                    .selected_application()
                    .expect("selected target call")
                    .digest()
            ),
        );
        let application_plan = application.execution_plan().expect("line execution");
        assert_eq!(
            application_plan.value(),
            CheckedRuntimeValueDisposition::Omit
        );
        assert_eq!(
            application_plan.structural_reason(),
            Some(CheckedStructuralExecutionReason::DialogueApplication)
        );
    }
}
