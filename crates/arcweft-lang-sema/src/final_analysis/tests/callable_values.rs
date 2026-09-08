use super::{analyze, fixture};
use crate::final_analysis::CheckedExpressionResolution;
use crate::types::TypeKind;

#[test]
fn direct_closure_call_preserves_its_expression_role() {
    let fixture = fixture(
        "flow main() -> i64 { return (|value: i64| value)(42i64) }",
        None,
    );
    analyze(&fixture).expect("an immediately invoked closure retains its checked source role");
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
    assert!(checked.execution_plan().call_application().is_some());
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
