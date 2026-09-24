//! Runtime body selection consumes sealed specialization evidence, with no
//! fresh inference and no chosen source expression hidden in a type match.

use super::{analyze, fixture};
use crate::{
    callable::{
        CallableCandidateId, CheckedProjectFunctionInstanceProjectionError,
        CheckedProjectFunctionRuntimeOutcome, CheckedProjectFunctionRuntimeSelection,
        CheckedProjectFunctionRuntimeSelectionError, select_project_function_runtime,
    },
    final_analysis::FinalSemanticAnalysis,
    types::{TypeKind, UnmeteredTypeProjection},
};
use arcweft_lang_hir::symbol::CallableDeclarationKey;

pub(super) fn selections(
    analysis: &FinalSemanticAnalysis,
    name: &str,
) -> Vec<CheckedProjectFunctionRuntimeSelection> {
    analysis
        .calls()
        .filter_map(|(owner, call)| {
            let application = call.selected_application()?;
            let CallableCandidateId::Project(CallableDeclarationKey::Existing(declaration)) =
                application.core().candidates().selected().id()
            else {
                return None;
            };
            (declaration.name() == name).then(|| {
                select_project_function_runtime(
                    application,
                    analysis
                        .checked_callable_join(owner)
                        .expect("exact call join"),
                    analysis.checked_callables(),
                )
                .expect("project runtime selection")
                .expect("ordinary project function")
            })
        })
        .collect()
}

#[test]
fn project_specialization_value_and_terminal_call_close_the_same_body_instance() {
    let fixture = fixture(
        r#"
fn choose<A, B>(first: A)(second: B) -> B { second }
fn apply(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }
flow main() -> i64 {
    let prefix = choose("saved")
    let text = prefix("text")
    let number = prefix(21i64)
    return apply(prefix, number)
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("one saved scheme with independent uses");
    let selected = selections(&analysis, "choose");
    assert_eq!(selected.len(), 3);
    let prefix = selected
        .iter()
        .find(|selection| {
            matches!(
                selection.outcome(),
                CheckedProjectFunctionRuntimeOutcome::Continue { .. }
            )
        })
        .expect("prefix selection");
    let witness = analysis
        .expressions()
        .find_map(|(_, fact)| fact.function_specialization())
        .expect("callback specialization witness");
    let value = prefix
        .callable_value_source_with_control(
            analysis.checked_callables(),
            None,
            &mut UnmeteredTypeProjection,
        )
        .unwrap()
        .specialize_callable_value_with_control(
            analysis.checked_callables(),
            witness,
            None,
            &mut UnmeteredTypeProjection,
        )
        .expect("witness and prefix compose without another solve");
    assert_eq!(value.type_arguments(), [TypeKind::I64]);
    assert!(value.const_arguments().is_empty());
    assert!(value.effect_arguments().is_empty());
    assert_eq!(value.source_type(), witness.source_type());
    assert_eq!(value.specialized_type(), witness.specialized_type());
    assert!(
        matches!(value.closed_selection().solution().callable_type(),
        TypeKind::Function { params, .. } if params.as_ref() == [TypeKind::String])
    );
    let mut terminal = selected
        .iter()
        .filter(|selection| {
            matches!(
                selection.outcome(),
                CheckedProjectFunctionRuntimeOutcome::Invoke { .. }
            )
        })
        .map(|selection| {
            selection
                .specialize_input_callable_with_control(
                    analysis.checked_callables(),
                    None,
                    &mut UnmeteredTypeProjection,
                )
                .expect("terminal solution projects the same residual origin")
        })
        .collect::<Vec<_>>();
    assert_eq!(terminal.len(), 2);
    let number = terminal.remove(
        terminal
            .iter()
            .position(|proof| proof.type_arguments() == [TypeKind::I64])
            .unwrap(),
    );
    let text = terminal.pop().unwrap();
    assert_eq!(value.source(), number.source());
    assert_eq!(value.source(), text.source());
    assert_eq!(value.next_group(), number.next_group());
    assert_eq!(value.closed_selection(), number.closed_selection());
    assert_eq!(text.type_arguments(), [TypeKind::String]);
    assert_ne!(
        value.closed_selection().solution().instantiation(),
        text.closed_selection().solution().instantiation()
    );
    assert!(
        !prefix.solution().is_fully_instantiated(),
        "source remains polymorphic"
    );
}

#[test]
fn project_specialization_rejects_a_different_source_scheme() {
    let fixture = fixture(
        r#"
fn choose<A, B>(first: A)(second: B) -> B { second }
fn different<A, B>(first: A)(second: B) -> A { first }
fn apply(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }
flow main() -> i64 {
    let prefix = choose("saved")
    let other = different("saved")
    return apply(prefix, 42i64)
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("both schemes are valid independently");
    let witness = analysis
        .expressions()
        .find_map(|(_, fact)| fact.function_specialization())
        .unwrap();
    let other = selections(&analysis, "different").remove(0);
    let other = other
        .callable_value_source_with_control(
            analysis.checked_callables(),
            None,
            &mut UnmeteredTypeProjection,
        )
        .unwrap();
    assert!(matches!(
        other.specialize_callable_value_with_control(
            analysis.checked_callables(),
            witness,
            None,
            &mut UnmeteredTypeProjection,
        ),
        Err(CheckedProjectFunctionInstanceProjectionError::Selection(
            CheckedProjectFunctionRuntimeSelectionError::SpecializationSourceMismatch,
        ))
    ));
}

#[test]
fn project_specialization_closes_a_prefix_in_its_enclosing_instance_once() {
    let fixture = fixture(
        r"
fn choose<A, B>(first: A)(second: B) -> B { second }
fn apply<T>(handler: T -> T effects {}, value: T) -> T { handler(value) }
fn outer<T>(value: T) -> T {
    let prefix = choose(value)
    return apply(prefix, value)
}

flow main() -> i64 { return outer(42i64) }
",
        None,
    );
    let analysis = analyze(&fixture).expect("residual argument may be caller-owned");
    let caller = selections(&analysis, "outer")
        .remove(0)
        .close_instance(None)
        .unwrap();
    let prefix = selections(&analysis, "choose").remove(0);
    let witness = analysis
        .expressions()
        .find_map(|(_, fact)| fact.function_specialization())
        .unwrap();
    let proof = prefix
        .callable_value_source_with_control(
            analysis.checked_callables(),
            Some(&caller),
            &mut UnmeteredTypeProjection,
        )
        .unwrap()
        .specialize_callable_value_with_control(
            analysis.checked_callables(),
            witness,
            Some(&caller),
            &mut UnmeteredTypeProjection,
        )
        .expect("both prefix and witness RHS are closed in outer's instance");
    assert_eq!(proof.type_arguments(), [TypeKind::I64]);
    assert!(
        matches!(proof.closed_selection().solution().callable_type(),
        TypeKind::Function { params, .. } if params.as_ref() == [TypeKind::I64])
    );
    assert!(
        matches!(proof.specialized_type(), TypeKind::Function { params, return_type, .. }
        if params.as_ref() == [TypeKind::I64] && **return_type == TypeKind::I64)
    );
}

#[test]
fn project_specialization_keeps_source_and_value_use_enclosing_instances_separate() {
    let fixture = fixture(
        r#"
fn choose<A, B>(first: A)(second: B) -> B { second }
fn apply<T>(handler: T -> T effects {}, value: T) -> T { handler(value) }
fn outer<T>(saved: T, value: T) -> T {
    let prefix = choose(saved)
    return apply(prefix, value)
}
flow main() -> bool { let text = outer("saved", "text"); return outer(true, false) }
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("each use retains the same rigid declaration key");
    let instances = selections(&analysis, "outer")
        .into_iter()
        .map(|selection| selection.close_instance(None).unwrap())
        .collect::<Vec<_>>();
    let source_enclosing = instances
        .iter()
        .find(|instance| {
            matches!(instance.function_type(),
        TypeKind::Function { params, .. } if params[0] == TypeKind::String)
        })
        .unwrap();
    let witness_enclosing = instances
        .iter()
        .find(|instance| {
            matches!(instance.function_type(),
        TypeKind::Function { params, .. } if params[0] == TypeKind::Bool)
        })
        .unwrap();
    let prefix = selections(&analysis, "choose").remove(0);
    let witness = analysis
        .expressions()
        .find_map(|(_, fact)| fact.function_specialization())
        .unwrap();
    // Both instances use the same free T identity. A source-state registry
    // must close it in the producing context and demand context independently.
    let source = prefix
        .callable_value_source_with_control(
            analysis.checked_callables(),
            Some(source_enclosing),
            &mut UnmeteredTypeProjection,
        )
        .unwrap();
    let other_source = prefix
        .callable_value_source_with_control(
            analysis.checked_callables(),
            Some(witness_enclosing),
            &mut UnmeteredTypeProjection,
        )
        .unwrap();
    assert_eq!(source.origin(), other_source.origin());
    assert_eq!(source.function_type(), other_source.function_type());
    assert_ne!(
        source.source_digest(),
        other_source.source_digest(),
        "the shared source arrow does not erase known earlier substitutions"
    );
    let proof = source
        .specialize_callable_value_with_control(
            analysis.checked_callables(),
            witness,
            Some(witness_enclosing),
            &mut UnmeteredTypeProjection,
        )
        .expect("source<T=String> composed with use<T=bool> demand");
    assert_eq!(proof.type_arguments(), [TypeKind::Bool]);
    assert!(
        matches!(proof.closed_selection().solution().callable_type(),
        TypeKind::Function { params, .. } if params.as_ref() == [TypeKind::String])
    );
    assert!(
        matches!(proof.specialized_type(), TypeKind::Function { params, return_type, .. }
        if params.as_ref() == [TypeKind::Bool] && **return_type == TypeKind::Bool)
    );
}
