//! Transfer input admission through production parsing, HIR and final analysis.

use super::{FinalSemanticAnalysisError, analyze, fixture};

#[test]
fn bare_goto_rejects_complete_unused_formal_inventory() {
    for parameters in ["unused: i64, other: String", "_: i64, _: String"] {
        let world = fixture(
            &format!("flow main {{ goto @flow.next }}\nflow next({parameters}) {{}}\n"),
            None,
        );
        let error = analyze(&world).expect_err("bare goto supplies no arguments");
        assert!(
            matches!(
                error,
                FinalSemanticAnalysisError::FlowTransferArgumentCount {
                    expected: 2,
                    actual: 0,
                    ..
                }
            ),
            "{error:?}"
        );
        assert_eq!(error.diagnostic_code(), "sema.goto.argument_count");
        assert!(error.source_diagnostic().is_some());
    }
}

#[test]
fn compact_choice_rejects_target_requiring_inputs() {
    let world = fixture(
        "flow main { choice @choice.menu {\n @choice.next \"Next\" -> @flow.next\n } }\nflow next(unused: i64) {}\n",
        None,
    );
    let error = analyze(&world).expect_err("compact Choice supplies no arguments");
    assert!(
        matches!(
            error,
            FinalSemanticAnalysisError::FlowTransferArgumentCount {
                expected: 1,
                actual: 0,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(error.source_diagnostic().is_some());
}

#[test]
fn zero_input_static_dynamic_and_choice_transfers_remain_accepted() {
    for transfer in [
        "goto @flow.next",
        "let route = if true { @flow.next } else { @flow.other }; goto route",
        "choice @choice.menu {\n @choice.next \"Next\" -> @flow.next\n }",
    ] {
        let world = fixture(
            &format!("flow main {{ {transfer} }}\nflow next {{}}\nflow other {{}}\n"),
            None,
        );
        analyze(&world).unwrap_or_else(|error| panic!("{transfer}: {error:?}"));
    }
}

#[test]
fn explicit_goto_binds_the_original_flow_inputs() {
    for arguments in ["7", "value = 7"] {
        let world = fixture(
            &format!(
                "flow main {{ goto @flow.next({arguments}) }}\nflow next(value: i64) -> i64 {{ return value }}\n"
            ),
            None,
        );
        let analysis = analyze(&world).unwrap_or_else(|error| panic!("{arguments}: {error:?}"));
        assert!(
            analysis.call_diagnostics().next().is_none(),
            "{:?}",
            analysis.call_diagnostics().collect::<Vec<_>>()
        );
    }
}

#[test]
fn an_inert_flow_reference_does_not_grant_returning_call_rights() {
    let world = fixture(
        "flow main -> i64 { let route = @flow.next; let result = route(7); return result }\nflow next(value: i64) -> i64 { return value }\n",
        None,
    );
    analyze(&world).expect_err("FlowRef grants only reference and transfer semantics");
}

#[test]
fn a_direct_flow_reference_application_is_only_admitted_as_a_goto_target() {
    let world = fixture(
        "flow main -> i64 { return @flow.next(7) }\nflow next(value: i64) -> i64 { return value }\n",
        None,
    );
    if let Ok(analysis) = analyze(&world) {
        assert!(analysis.call_diagnostics().next().is_some());
    }
}

#[test]
fn explicit_transfer_enters_the_targets_effect_scope() {
    let world = fixture(
        "flow main effects {} { goto @flow.next(7) }\nflow next(value: i64) effects { log.write } { log.info(\"target\") }\n",
        None,
    );
    let analysis = analyze(&world).expect("target execution owns its own effects");
    assert!(analysis.call_diagnostics().next().is_none());
}

#[test]
fn explicit_transfer_arguments_still_require_the_current_effect_permission() {
    let world = fixture(
        "fn input() -> i64 effects { log.write } { log.info(\"argument\"); 7 }\nflow main effects {} { goto @flow.next(input()) }\nflow next(value: i64) {}\n",
        None,
    );
    let error = analyze(&world).expect_err("argument evaluation precedes the Flow boundary");
    let FinalSemanticAnalysisError::EffectUpperBoundExceeded { missing, .. } = error else {
        panic!("{error:?}");
    };
    assert_eq!(missing.to_labels(), ["log.write"]);
}

#[test]
fn a_self_transfer_is_not_returning_callable_recursion() {
    let world = fixture("flow main(value: i64) { goto @flow.main(value) }\n", None);
    let analysis = analyze(&world).expect("a transfer replaces the current Flow invocation");
    assert!(analysis.call_diagnostics().next().is_none());
}

#[test]
fn goto_rejects_values_from_other_target_domains() {
    for target in ["7i64", "true", "\"flow.next\"", "choose()"] {
        let world = fixture(
            &format!("fn choose() -> i64 {{ 7i64 }}\nflow main {{ goto {target} }}\n"),
            None,
        );
        let error = analyze(&world).expect_err("Goto requires an actual Flow reference");
        assert!(
            matches!(
                error,
                FinalSemanticAnalysisError::StatementOperandTypeMismatch { .. }
            ),
            "{target}: {error:?}"
        );
    }
}

#[test]
fn a_returning_route_selection_keeps_its_call_effects() {
    let world = fixture(
        "fn choose() -> Ref<Flow> effects { log.write } { log.info(\"route\"); @flow.next }\nflow main effects {} { goto choose() }\nflow next {}\n",
        None,
    );
    let error = analyze(&world).expect_err("route selection still runs in the source scope");
    let FinalSemanticAnalysisError::EffectUpperBoundExceeded { missing, .. } = error else {
        panic!("{error:?}");
    };
    assert_eq!(missing.to_labels(), ["log.write"]);
}
