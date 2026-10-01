use super::*;
use crate::final_analysis::{CheckedCallableBoundary, CheckedFunctionSiteBoundary};

#[test]
fn return_targets_the_callable_frame_through_a_carrier_block() {
    let world = super::fixture(
        "fn root() -> i64 { let unused = result { return 7i64; 0i64 }; return 9i64 }",
        None,
    );
    let report = super::analyze(&world).unwrap();
    let targets = report
        .statements()
        .filter_map(|(_, statement)| match statement.payload() {
            CheckedStatementPayload::ControlTransfer(target) => target.return_target(),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [
        CheckedCallableBoundary::Declaration(inner),
        CheckedCallableBoundary::Declaration(outer),
    ] = targets.as_slice()
    else {
        panic!("both returns belong to the function, including the return inside Result")
    };
    assert_eq!(inner, outer);
}

#[test]
fn closure_return_keeps_its_frame_and_declared_result() {
    let world = super::fixture(
        "flow root() -> i64 { let callback = || -> String { return \"inner\" }; return 9i64 }",
        None,
    );
    let report = super::analyze(&world).unwrap();
    let target = report
        .statements()
        .find_map(|(_, statement)| match statement.payload() {
            CheckedStatementPayload::ControlTransfer(target) => match target.return_target() {
                Some(CheckedCallableBoundary::FunctionSite(
                    CheckedFunctionSiteBoundary::Explicit(site),
                )) => Some(site),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    let closure = report.expression(target.lookup_owner()).unwrap();
    let Some(TypeKind::Function { return_type, .. }) = closure.value_type() else {
        panic!("the closure has complete callable value evidence")
    };
    assert_eq!(return_type.as_ref(), &TypeKind::String);
    assert!(
        report
            .calls()
            .all(|(_, call)| call.selected_application().is_some())
    );
}

#[test]
fn implicit_body_return_uses_the_selected_callable_identity() {
    let world = super::fixture(
        "fn root() -> i64 { let callback: (i64) -> i64 = { return _; 0i64 }; return 9i64 }",
        None,
    );
    let report = super::analyze(&world).unwrap();
    let (owner, callable) = report
        .expressions()
        .find_map(|(owner, expression)| match expression.resolution() {
            CheckedExpressionResolution::ImplicitCallable(callable) => Some((owner, callable)),
            _ => None,
        })
        .unwrap();
    let target = report
        .statements()
        .find_map(|(_, statement)| match statement.payload() {
            CheckedStatementPayload::ControlTransfer(target) => match target.return_target() {
                Some(CheckedCallableBoundary::FunctionSite(
                    CheckedFunctionSiteBoundary::Implicit { site, callable },
                )) => Some((site, callable)),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    assert_eq!(target.0.lookup_owner(), owner);
    assert_eq!(*target.1, callable.identity());
    assert_eq!(callable.result(), &TypeKind::I64);
    assert_eq!(
        input_abi(&report, &world, owner).unwrap().control(),
        crate::final_analysis::CheckedExecutableControlRole::ExpressionCompatible
    );
    assert_eq!(
        report.callable_body_control(owner),
        Some(crate::final_analysis::CheckedExecutableControlRole::FlowRequired)
    );
}

#[test]
fn returning_an_implicit_callable_value_does_not_invoke_it() {
    let world = super::fixture("fn root() -> (i64) -> i64 { return _ + 1i64 }", None);
    let report = super::analyze(&world).unwrap();
    assert!(report.expressions().any(|(_, expression)| matches!(
        expression.resolution(),
        CheckedExpressionResolution::ImplicitCallable(_)
    )));
    let statements = report.statements().collect::<Vec<_>>();
    let [(_, statement)] = statements.as_slice() else {
        panic!("one Return statement")
    };
    let CheckedStatementPayload::ControlTransfer(target) = statement.payload() else {
        panic!("Return has its selected frame")
    };
    assert!(matches!(
        target.return_target(),
        Some(CheckedCallableBoundary::Declaration(_))
    ));
}

#[test]
fn nested_return_rejects_a_value_of_the_wrong_type() {
    let world = super::fixture(
        "fn root() -> i64 { let unused = { return true; 0i64 }; 9i64 }",
        None,
    );
    assert!(
        matches!(super::analyze(&world), Err(FinalSemanticAnalysisError::ReturnValueTypeMismatch {
        expected, actual, ..
    }) if expected.as_ref() == &TypeKind::I64 && actual.as_ref() == &TypeKind::Bool)
    );
}
