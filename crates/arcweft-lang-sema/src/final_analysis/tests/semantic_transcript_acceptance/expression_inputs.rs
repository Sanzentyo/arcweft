use super::*;
use crate::final_analysis::{CheckedExecutionInput, CheckedLocalReadMode, CheckedLocalUseSite};

#[test]
fn expression_input_abi_distinguishes_place_mutation_from_value_evaluation() {
    let world = super::fixture(
        "struct Queue { items: Vec<i64> }\nfn root(input: Queue) -> Option<i64> { { let mut queue = input; queue.items.pop() } }",
        None,
    );
    let report = super::analyze(&world).unwrap();
    let call = report
        .calls()
        .find_map(|(owner, call)| {
            call.selected_application()
                .filter(|application| {
                    application
                        .core()
                        .candidates()
                        .selected()
                        .capacity_operation()
                        == Some(crate::callable::CheckedCapacityOperation::Pop)
                })
                .map(|_| owner)
        })
        .unwrap();
    let abi = input_abi(&report, &world, call).unwrap();
    let [receiver] = abi.places() else {
        panic!("one addressed receiver")
    };
    assert!(!abi.expressions().contains(receiver));
    let [input] = abi.inputs() else {
        panic!("one external place owner")
    };
    let [usage] = input.uses() else {
        panic!("one place access, no receiver value read")
    };
    assert_eq!(usage.site(), CheckedLocalUseSite::Place(*receiver));
    let access = usage.access().place_access().unwrap();
    assert_eq!(
        access.mode(),
        crate::final_analysis::CheckedLocalPlaceMode::Mutate
    );
    assert!(access.place().nominal_field().is_some());
    assert!(usage.access().value_transfer().is_none());
    assert!(usage.latent_requirement().is_none());
    assert!(
        report
            .checked_local_uses()
            .value_transfer_at(usage.site())
            .is_none()
    );
    let whole_body = report
        .expressions()
        .filter_map(|(owner, _)| input_abi(&report, &world, owner).ok())
        .find(|region| !region.statements().is_empty() && region.places().contains(receiver))
        .unwrap();
    assert_eq!(whole_body.inputs().len(), 1);
    assert_ne!(
        whole_body.inputs()[0].binding().local(),
        input.binding().local()
    );
    assert!(
        whole_body.inputs()[0]
            .uses()
            .iter()
            .all(|usage| usage.access().place_access().is_none())
    );
}

#[test]
fn expression_input_abi_records_replacement_without_reading_the_previous_value() {
    let world = super::fixture(
        "fn root(input: Vec<Need<i64>>, replacement: Vec<Need<i64>>) -> i64 { let mut items = input; { let marker = 0i64; items = replacement; 42i64 } }",
        None,
    );
    let report = super::analyze(&world).unwrap();
    let abi = report
        .expressions()
        .filter_map(|(owner, _)| input_abi(&report, &world, owner).ok())
        .find(|abi| abi.places().len() == 1 && abi.inputs().len() == 2)
        .unwrap();
    let place = abi.places()[0];
    assert!(!abi.expressions().contains(&place));
    let usage = abi
        .inputs()
        .iter()
        .flat_map(CheckedExecutionInput::uses)
        .find(|usage| usage.site() == CheckedLocalUseSite::Place(place))
        .unwrap();
    let access = usage.access().place_access().unwrap();
    assert_eq!(
        access.mode(),
        crate::final_analysis::CheckedLocalPlaceMode::Assign
    );
    assert!(access.place().nominal_field().is_none());
    assert!(usage.access().value_transfer().is_none());
}

#[test]
fn expression_input_abi_retains_callable_copy_ingress_obligations() {
    let world = super::fixture(
        "fn twice(callback: i64 -> i64, value: i64) -> (i64, i64) { (callback(value), callback(value)) }\nfn caller() -> (i64, i64) { twice(|value: i64| value, 42i64) }",
        None,
    );
    let report = super::analyze(&world).unwrap();
    let required_local = report
        .checked_local_uses()
        .copy_requirements()
        .next()
        .unwrap()
        .local();
    let source = report
        .expressions()
        .find_map(|(owner, expression)| {
            (expression.execution_local_use() == Some(required_local)).then_some(owner)
        })
        .unwrap();
    let instance = super::super::project_specialization::selections(&report, "twice")
        .remove(0)
        .close_instance(None)
        .unwrap();
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source,
            Some(crate::final_analysis::CheckedLocalUseInstantiation::ProjectFunction(&instance)),
        )
        .unwrap();
    let abi = context.checked_execution_input_abi(source).unwrap();
    let [input] = abi.inputs() else {
        panic!("one callable ingress")
    };
    assert_eq!(
        input.uses()[0].access().value_transfer().unwrap().mode(),
        CheckedLocalReadMode::Copy
    );
    assert!(input.copy_evidence().is_none());
    let requirement = input
        .copy_requirement()
        .expect("deep unrestricted carrier must be checked at ingress");
    assert_eq!(requirement.local(), input.binding().local());
    assert_eq!(
        requirement.ty(),
        input.binding().ty().semantic_identity_digest().unwrap()
    );
}

#[test]
fn expression_input_abi_does_not_invent_modes_for_an_open_generic_body() {
    let world = super::fixture("fn root<T>(value: T) -> T { value }", None);
    let report = super::analyze(&world).unwrap();
    let owner = report
        .expressions()
        .find_map(|(owner, expression)| expression.execution_local_use().map(|_| owner))
        .unwrap();
    assert!(matches!(
        input_abi(&report, &world, owner),
        Err(crate::final_analysis::CheckedExecutionContextError::OpenDeclaration { .. })
    ));
}

#[test]
fn expression_input_abi_cleanup_keeps_creation_inputs_outside_the_latent_frame() {
    let source = "struct Label { value: i64 }\nfn root(value: i64) -> i64 { { defer { let label = Label { value }; () }; value } }";
    let world = super::fixture(source, None);
    let report = super::analyze(&world).unwrap();
    let (defer_owner, _) = report
        .statements()
        .find(|(_, statement)| {
            matches!(
                statement.payload(),
                crate::final_analysis::CheckedStatementPayload::Defer(_)
            )
        })
        .unwrap();
    let abi = report
        .expressions()
        .filter_map(|(owner, _)| input_abi(&report, &world, owner).ok())
        .find(|abi| abi.statements().contains(&defer_owner))
        .unwrap();
    let [input] = abi.inputs() else {
        panic!("one outer input")
    };
    assert_eq!(input.uses().len(), 2);
    assert_eq!(input.uses().iter().filter(|usage| {
        matches!(usage.site(), CheckedLocalUseSite::StatementCapture { owner, .. } if owner == defer_owner)
    }).count(), 1);
    assert!(
        input
            .uses()
            .iter()
            .all(|usage| !matches!(usage.site(), CheckedLocalUseSite::RecordField { .. }))
    );
}

#[test]
fn expression_input_abi_keeps_every_read_and_excludes_inner_bindings() {
    let source = r#"
struct Pair { first: i64, second: i64 }
view Main(first: i64, second: i64,
    result: Pair = { let inner = first + second; Pair { first, second = inner + first } }) {
    Text("value")
}
"#;
    let world = super::fixture(source, None);
    let report = super::analyze(&world).unwrap();
    let default = report
        .checked_callables()
        .records()
        .find_map(|facts| facts.parameter_defaults().values().next())
        .unwrap();
    let abi = input_abi(&report, &world, default.source()).unwrap();
    assert_eq!(abi.inputs().len(), 2);
    assert!(!abi.statements().is_empty());
    assert_eq!(
        abi.inputs()
            .iter()
            .map(|input| input.uses().len())
            .collect::<Vec<_>>(),
        [3, 1]
    );
    assert_eq!(
        abi.inputs()
            .iter()
            .flat_map(CheckedExecutionInput::uses)
            .filter(|usage| {
                matches!(
                    usage.site(),
                    CheckedLocalUseSite::RecordField {
                        source_ordinal: 0,
                        ..
                    }
                )
            })
            .count(),
        1
    );
    assert!(
        abi.inputs()
            .iter()
            .flat_map(CheckedExecutionInput::uses)
            .all(|usage| {
                usage.access().value_transfer().unwrap().mode() == CheckedLocalReadMode::Copy
            })
    );
}

#[test]
fn expression_input_abi_callable_creation_stops_before_the_latent_body() {
    for value in ["|value: i64| value + first", "_ + first"] {
        let source =
            format!("view Main(first: i64, callback: i64 -> i64 = {value}) {{ Text(\"value\") }}");
        let world = super::fixture(&source, None);
        let report = super::analyze(&world).unwrap();
        let default = report
            .checked_callables()
            .records()
            .find_map(|facts| facts.parameter_defaults().values().next())
            .unwrap();
        let abi = input_abi(&report, &world, default.source()).unwrap();
        assert_eq!(abi.expressions(), [default.source()]);
        assert!(abi.statements().is_empty());
        assert_eq!(abi.inputs().len(), 1);
        let [usage] = abi.inputs()[0].uses() else {
            panic!("one creation capture")
        };
        assert!(
            matches!(usage.site(), CheckedLocalUseSite::Capture { owner, .. } if owner == default.source())
        );
        assert!(abi.effects().is_empty());
        assert_eq!(abi.suspension(), CheckedSuspensionRole::NonSuspending);
    }
}

#[test]
fn expression_input_abi_is_stable_across_source_and_arena_revisions() {
    fn observe(
        source: &str,
    ) -> (
        crate::semantic_coordinate::CheckedSemanticPath,
        Vec<(
            crate::semantic_coordinate::StableCheckedBindingCoordinate,
            Vec<crate::semantic_coordinate::CheckedLocalInputCoordinate>,
        )>,
    ) {
        let world = super::fixture(source, None);
        let report = super::analyze(&world).unwrap();
        let default = report
            .checked_callables()
            .records()
            .find_map(|facts| facts.parameter_defaults().values().next())
            .unwrap();
        let abi = input_abi(&report, &world, default.source()).unwrap();
        (
            abi.coordinate().path().clone(),
            abi.inputs()
                .iter()
                .map(|input| {
                    (
                        input.binding().origin().clone(),
                        input
                            .uses()
                            .iter()
                            .map(|usage| usage.coordinate().clone())
                            .collect(),
                    )
                })
                .collect(),
        )
    }
    let source = "struct Pair { first: i64, second: i64 }\nview Main(first: i64, second: i64, result: Pair = Pair { first, second = first + second }) { Text(\"value\") }";
    let original = observe(source);
    let revised = format!(
        "fn unrelated() -> i64 {{ 99i64 }}\n{}",
        source.replace("first + second", "first  +  second")
    );
    assert_eq!(original, observe(&revised));
    let changed = observe(&source.replace("second = first + second", "second = second + first"));
    assert_ne!(original, changed);
}

#[test]
fn expression_input_abi_rejects_a_foreign_generation_owner() {
    let first = super::fixture(
        "view Main(first: i64, value: i64 = first + 1) { Text(value) }",
        None,
    );
    let second = super::fixture(
        "fn unrelated() -> i64 { 99i64 }\nview Main(first: i64, value: i64 = first + 1) { Text(value) }",
        None,
    );
    let first_report = super::analyze(&first).unwrap();
    let second_report = super::analyze(&second).unwrap();
    let owner = first_report
        .checked_callables()
        .records()
        .find_map(|facts| {
            facts
                .parameter_defaults()
                .values()
                .next()
                .map(crate::callable::CheckedDeclarationDefault::source)
        })
        .unwrap();
    assert!(matches!(
        input_abi(&second_report, &second, owner),
        Err(
            crate::final_analysis::CheckedExecutionContextError::Semantic(
                FinalSemanticAnalysisError::InvalidOwner
            )
        )
    ));
}
