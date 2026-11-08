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
