use super::{analyze, fixture};
use crate::final_analysis::{CheckedExpressionResolution, FinalSemanticAnalysis};
use crate::types::TypeKind;

pub(super) fn assert_unselected_call_has_no_execution(
    analysis: &FinalSemanticAnalysis,
    owner: arcweft_lang_hir::identity::ExprId,
) {
    use crate::final_analysis::FinalAnalysisExecutionProjectionError;

    assert!(
        analysis
            .call(owner)
            .unwrap()
            .selected_application()
            .is_none()
    );
    let expression = analysis.expression(owner).expect("tooling call fact");
    assert!(expression.value_type().is_none());
    assert!(expression.execution_plan().is_none());
    assert_eq!(
        analysis.execution_projection().plan(owner),
        Err(FinalAnalysisExecutionProjectionError::UnselectedCall { owner }),
    );
    assert_eq!(
        analysis.execution_projection().expression(owner),
        Err(FinalAnalysisExecutionProjectionError::UnselectedCall { owner }),
    );
}

#[test]
fn unselected_call_outcomes_never_grant_execution() {
    use crate::callable::CallAnalysisOutcome;

    let fixture = super::typed_overload_fixture(
        "fn caller() { choose(true); }\n",
        "choose",
        vec![
            super::TestCallableOverload::strict([TypeKind::I64], TypeKind::I64),
            super::TestCallableOverload::strict([TypeKind::U64], TypeKind::U64),
        ],
    );
    let analysis = analyze(&fixture).expect("rejected overloads remain inspectable by tooling");
    let calls = analysis.calls().collect::<Vec<_>>();
    let [(owner, call)] = calls.as_slice() else {
        panic!("one source call");
    };
    assert!(matches!(call.outcome(), CallAnalysisOutcome::Rejected(_)));
    assert_unselected_call_has_no_execution(&analysis, *owner);
}

#[test]
fn direct_closure_call_preserves_its_expression_role() {
    let fixture = fixture(
        "flow main() -> i64 { return (|value: i64| value)(42i64) }",
        None,
    );
    analyze(&fixture).expect("an immediately invoked closure retains its checked source role");
}

#[test]
fn surplus_function_arguments_leave_no_selected_application() {
    let fixture = fixture(
        r#"
fn increment(value: i64) -> i64 { value + 1i64 }
flow main() -> i64 {
    let factory = |offset: i64| { |value: i64| increment(value) + offset }
    return factory(1i64, 40i64)
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("an invalid group retains its tooling evidence");
    let rejected = analysis
        .calls()
        .filter_map(|(owner, call)| match call.outcome() {
            crate::callable::CallAnalysisOutcome::Rejected(evidence) => Some((owner, evidence)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(owner, evidence)] = rejected.as_slice() else {
        panic!("one rejected function-value application");
    };
    assert_eq!(evidence.candidates().len(), 1);
    let schema = evidence.candidates()[0].schema();
    assert_eq!(schema.groups().len(), 1);
    assert_eq!(schema.groups()[0].parameters().len(), 1);
    assert_unselected_call_has_no_execution(&analysis, *owner);
}

#[test]
fn block_argument_mutation_retains_the_assignment_role() {
    let fixture = fixture(
        r#"
struct State { value: i64 }
fn identity(value: i64) -> i64 { value }
flow main() -> i64 {
    let state = State { value = 1i64 }
    return identity({
        let input = 41i64
        state.value = 2i64
        identity(input)
    })
}
"#,
        None,
    );
    analyze(&fixture)
        .expect("a call operand block retains its checked nominal assignment evidence");
}

#[test]
fn direct_closure_call_accepts_a_mutating_block_argument() {
    let fixture = fixture(
        r#"
struct State { value: i64 }
fn identity(value: i64) -> i64 { value }
flow main() -> i64 {
    let state = State { value = 1i64 }
    return (|value: i64| value + state.value)({
        let input = 41i64
        state.value = 2i64
        identity(input)
    })
}
"#,
        None,
    );
    analyze(&fixture).expect("callee and operand source roles remain independently valid");
}

#[test]
fn named_block_operands_publish_selected_call_evidence() {
    let fixture = fixture(
        r#"
struct State { value: i64 }
fn reorder(first: i64, second: i64) -> i64 { first * 10i64 + second }
flow main() -> i64 {
    let state = State { value = 0i64 }
    let ordered = reorder(
        second = { let value = state.value + 1i64
            state.value = value
            value },
        first = { let value = state.value + 1i64
            state.value = value
            value },
    )
    return ordered * 10i64 + state.value
}
"#,
        None,
    );
    let analysis =
        analyze(&fixture).expect("block-local inference completes before call selection");
    let calls = analysis.calls().collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let (owner, call) = calls[0];
    assert!(call.selected_application().is_some());
    let checked = analysis.expression(owner).expect("checked call expression");
    assert_eq!(checked.result().value_type(), Some(&TypeKind::I64));
    assert!(
        checked
            .execution_plan()
            .expect("selected call has an execution plan")
            .call_application()
            .is_some()
    );
}

#[test]
fn block_left_pipe_publishes_one_typed_binding_for_both_uses() {
    let fixture = fixture(
        r#"
struct State { value: i64 }
flow main() -> i64 {
    let state = State { value = 0i64 }
    let doubled = {
        let value = state.value + 1i64
        state.value = value
        value
    } |> ^ + ^
    return doubled + state.value
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("pipe LHS block owns its local binding inference");
    let pipes = analysis
        .expressions()
        .filter_map(|(_, checked)| match checked.resolution() {
            CheckedExpressionResolution::Pipe(pipe) => Some(pipe),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [pipe] = pipes.as_slice() else {
        panic!("one checked pipe")
    };
    assert_eq!(pipe.occurrences().len(), 2);
    for (ordinal, occurrence) in pipe.occurrences().iter().enumerate() {
        let checked = analysis
            .expression(occurrence.lookup_expression())
            .expect("checked placeholder");
        let CheckedExpressionResolution::PipeLeft(left) = checked.resolution() else {
            panic!("a pipe occurrence keeps its typed binding identity")
        };
        assert_eq!(left.binding_identity(), pipe.binding_identity());
        assert_eq!(left.occurrence_ordinal() as usize, ordinal);
        assert_eq!(checked.result().value_type(), Some(&TypeKind::I64));
    }
}

#[test]
fn contextual_closure_block_keeps_its_local_in_the_candidate_scope() {
    let fixture = fixture(
        r#"
fn apply(callback: i64 -> i64 effects {}) -> i64 { callback(41i64) }
flow main() -> i64 {
    let answer = apply({ let offset = 1i64
        |value| value + offset })
    return answer
}
"#,
        None,
    );
    let analysis =
        analyze(&fixture).expect("closure expectation and local facts share a candidate scope");
    assert_eq!(analysis.calls().count(), 2);
    assert!(
        analysis
            .calls()
            .all(|(_, call)| call.selected_application().is_some())
    );
    assert!(analysis.expressions().any(|(_, checked)| matches!(
        checked.resolution(),
        CheckedExpressionResolution::Closure(_)
    )));
}

#[test]
fn callback_returns_a_nonterminal_prefix() {
    let fixture = fixture(
        r#"
fn sum(first: i64)(second: i64)(third: i64) -> i64 { first + second + third }
fn advance(handler: i64 -> (i64 -> i64 effects {}) effects {}, value: i64) -> (i64 -> i64 effects {}) {
    handler(value)
}
flow main() -> i64 {
    let prefix = sum(1i64)
    let left = advance(prefix, 20i64)
    let right = advance(prefix, 30i64)
    return left(21i64) + right(11i64)
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("a callback may return the next curried group");
    assert_eq!(analysis.calls().count(), 6);
    for (owner, call) in analysis.calls() {
        let application = call
            .selected_application()
            .expect("every call has a selected application");
        assert_eq!(
            analysis
                .execution_projection()
                .plan(owner)
                .expect("every selected call is executable")
                .call_application(),
            Some(application.digest()),
        );
    }
}
