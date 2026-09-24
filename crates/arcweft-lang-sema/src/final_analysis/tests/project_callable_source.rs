//! Declaration roots and continuation values carry one scoped source authority.

use super::{analyze, fixture, project_specialization::selections};
use crate::{
    callable::{
        CallableGroupIndex, CallableParameterPassing, CallableParameterPresence,
        CheckedProjectFunctionCallableOrigin, select_project_function_value_runtime,
    },
    types::{GenericBinder, TypeKind, UnmeteredTypeProjection},
};

#[test]
fn project_callable_source_bare_root_witness_and_value_call_select_the_same_body() {
    let fixture = fixture(
        r"
fn identity<T>(value: T) -> T { value }
fn apply(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }
flow main() -> i64 {
    let saved = identity
    let value = saved(21i64)
    let direct = identity(21i64)
    return apply(identity, value + direct)
}
",
        None,
    );
    let analysis = analyze(&fixture).expect("bare scheme, alias invocation and callback use");
    let call = selections(&analysis, "identity").remove(0);
    let source = select_project_function_value_runtime(
        call.declaration(),
        analysis.checked_callables(),
        None,
        &mut UnmeteredTypeProjection,
    )
    .unwrap();
    assert_eq!(source.origin(), CheckedProjectFunctionCallableOrigin::Root);
    assert_eq!(source.group(), CallableGroupIndex::ZERO);
    assert!(source.closed_selection().is_none());
    assert!(source.retained_parameters().is_empty());
    let parameter = &source.parameters()[0];
    assert_eq!(parameter.coordinate().group(), CallableGroupIndex::ZERO);
    assert_eq!(parameter.coordinate().parameter().get(), 0);
    assert_eq!(
        parameter.abi_type().scope().binders(),
        [GenericBinder::new(1, 0, 0)]
    );
    assert_eq!(parameter.binding_type(), parameter.abi_type());
    assert_eq!(
        parameter.abi_type().value(),
        &TypeKind::GenericParam(parameter.abi_type().scope().bound_type(0, 0).unwrap(),)
    );
    let witness = analysis
        .expressions()
        .find_map(|(_, fact)| fact.function_specialization())
        .unwrap();
    let witnesses = analysis
        .expressions()
        .filter_map(|(_, fact)| {
            fact.function_specialization()
                .map(|witness| (fact, witness))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        witnesses.len(),
        2,
        "alias callee and callback each retain their own witness"
    );
    assert!(witnesses.iter().any(|(fact, _)| matches!(
        fact.resolution(),
        crate::final_analysis::CheckedExpressionResolution::Value(
            crate::final_analysis::CheckedValueResolution::Local(_)
        )
    )));
    assert_eq!(source.function_type(), witness.source_type());
    let value = source
        .specialize_callable_value_with_control(
            analysis.checked_callables(),
            witness,
            None,
            &mut UnmeteredTypeProjection,
        )
        .unwrap();
    let terminal = call
        .specialize_input_callable_with_control(
            analysis.checked_callables(),
            None,
            &mut UnmeteredTypeProjection,
        )
        .unwrap();
    assert_eq!(value.source(), CheckedProjectFunctionCallableOrigin::Root);
    assert_eq!(value.source_digest(), source.source_digest());
    assert_eq!(value.source_digest(), terminal.source_digest());
    assert_eq!(value.type_arguments(), [TypeKind::I64]);
    assert_eq!(value.closed_selection(), terminal.closed_selection());
}

#[test]
fn project_callable_source_global_root_excludes_unrelated_enclosing_instances() {
    let fixture = fixture(
        r#"
fn identity<T>(value: T) -> T { value }
fn outer<T>(value: T) -> T { identity(value) }
flow main() -> bool { let text = outer("saved"); return outer(true) }
"#,
        None,
    );
    let analysis = analyze(&fixture).unwrap();
    let call = selections(&analysis, "identity").remove(0);
    let instances = selections(&analysis, "outer")
        .into_iter()
        .map(|selection| selection.close_instance(None).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    let roots = instances
        .iter()
        .map(|enclosing| {
            select_project_function_value_runtime(
                call.declaration(),
                analysis.checked_callables(),
                Some(enclosing),
                &mut UnmeteredTypeProjection,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(roots[0], roots[1]);
    assert_eq!(roots[0].source_digest(), roots[1].source_digest());
    let proofs = instances
        .iter()
        .map(|enclosing| {
            call.specialize_input_callable_with_control(
                analysis.checked_callables(),
                Some(enclosing),
                &mut UnmeteredTypeProjection,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(proofs[0].source_digest(), proofs[1].source_digest());
    assert_ne!(proofs[0].type_arguments(), proofs[1].type_arguments());
    assert_ne!(
        proofs[0].closed_selection().solution().instantiation(),
        proofs[1].closed_selection().solution().instantiation()
    );
}

#[test]
fn project_callable_source_continuation_owns_rest_binding_and_retained_coordinates() {
    let fixture = fixture(
        r#"
fn choose<A, B>(first: A)(second: B, remaining: ...B) -> B { second }
flow main() -> i64 { let prefix = choose("saved"); return prefix(42i64, 9i64) }
"#,
        None,
    );
    let analysis = analyze(&fixture).unwrap();
    let prefix = selections(&analysis, "choose")
        .into_iter()
        .find(|selection| {
            matches!(
                selection.outcome(),
                crate::callable::CheckedProjectFunctionRuntimeOutcome::Continue { .. }
            )
        })
        .expect("one prefix selection");
    let source = prefix
        .callable_value_source_with_control(
            analysis.checked_callables(),
            None,
            &mut UnmeteredTypeProjection,
        )
        .unwrap();
    let arrow = prefix
        .application_function_type_with_control(
            analysis.checked_callables(),
            None,
            &mut UnmeteredTypeProjection,
        )
        .unwrap();
    assert!(
        matches!(&arrow, TypeKind::Function { binder, params, return_type, .. }
        if binder.is_empty() && params.as_ref() == [TypeKind::String] && return_type.as_ref() == source.function_type())
    );
    assert!(arrow.semantic_identity_digest().is_ok());
    assert!(matches!(
        source.origin(),
        CheckedProjectFunctionCallableOrigin::Continuation { .. }
    ));
    assert_eq!(source.group().get(), 1);
    assert!(source.closed_selection().is_none());
    let retained = &source.retained_parameters()[0];
    assert_eq!(retained.coordinate().group().get(), 0);
    assert_eq!(retained.coordinate().parameter().get(), 0);
    assert!(retained.binding_type().scope().binders().is_empty());
    assert_eq!(retained.binding_type().value(), &TypeKind::String);
    let rest = &source.parameters()[1];
    assert_eq!(rest.coordinate().group().get(), 1);
    assert_eq!(rest.coordinate().parameter().get(), 1);
    assert_eq!(rest.passing(), CallableParameterPassing::RestPositional);
    assert_eq!(rest.abi_type().scope(), rest.binding_type().scope());
    assert_eq!(
        rest.binding_type().value(),
        &TypeKind::Vec(Box::new(rest.abi_type().value().clone()))
    );
}

#[test]
fn project_callable_source_closed_root_keeps_initial_group_and_terminal_body_distinct() {
    let fixture = fixture(
        r"
fn add(left: i64)(right: i64) -> i64 { left + right }
flow main() -> i64 { return add(20i64)(22i64) }
",
        None,
    );
    let analysis = analyze(&fixture).unwrap();
    let call = selections(&analysis, "add").remove(0);
    let root = select_project_function_value_runtime(
        call.declaration(),
        analysis.checked_callables(),
        None,
        &mut UnmeteredTypeProjection,
    )
    .unwrap();
    assert_eq!(root.group(), CallableGroupIndex::ZERO);
    assert!(root.retained_parameters().is_empty());
    let body = root.closed_selection().unwrap();
    assert_eq!(body.group().get(), 1);
    assert_eq!(body.solution().callable_type(), root.function_type());
    assert!(
        matches!(body.function_type(), TypeKind::Function { binder, params, return_type, .. }
        if binder.is_empty() && params.as_ref() == [TypeKind::I64] && return_type.as_ref() == &TypeKind::I64)
    );
}

#[test]
fn project_callable_source_attached_presence_and_binding_keep_the_source_binder() {
    let fixture = fixture(
        r#"
pub character alice {}
fn maybe<T>(value: T)[body?: DialogueContent] -> DialogueContent {
    match body {
        .Some(content) => content
        .None => panic("missing optional content")
    }
}
fn opening() { alice[#maybe(42i64)]; }
"#,
        None,
    );
    let analysis = analyze(&fixture).unwrap();
    let call = selections(&analysis, "maybe").remove(0);
    let source = select_project_function_value_runtime(
        call.declaration(),
        analysis.checked_callables(),
        None,
        &mut UnmeteredTypeProjection,
    )
    .unwrap();
    let attached = source.attached_source_schema().unwrap();
    assert_eq!(attached.parameter().group(), CallableGroupIndex::ZERO);
    assert_eq!(
        attached.parameter().presence(),
        CallableParameterPresence::Optional
    );
    assert_eq!(
        attached.abi_type().scope(),
        source.parameters()[0].abi_type().scope()
    );
    assert_eq!(attached.abi_type(), attached.binding_type());
    assert_eq!(attached.abi_type().value(), attached.parameter().abi_type());
}
