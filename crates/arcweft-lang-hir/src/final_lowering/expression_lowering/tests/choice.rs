use super::*;

use crate::expr::{HirChoiceCompactAction, HirChoiceItem};

#[test]
fn attached_choice_compact_arm_lowers_to_the_sole_choice_expression_owner() {
    let parsed = parsed_source(
        "choice-compact-arm",
        &["(choice { @.first \"First\" => unit })".into()],
    );
    let (module, owners, attached) = lower_and_publish(&parsed);
    let [owner] = owners.as_slice() else {
        panic!("fixture must publish one Choice expression")
    };
    assert!(matches!(
        attached[0].projection(),
        arcweft_lang_syntax::expressions::ExpressionProjection::Choice
    ));

    let root = expression(&module, *owner);
    assert!(!root.is_poisoned());
    let HirExprKind::Choice(choice) = root.kind() else {
        panic!("recognized Choice must not lower through generic Error")
    };
    assert!(choice.id().is_none());
    assert!(choice.plan().is_none());
    let [HirChoiceItem::CompactArm(arm)] = choice.body().items() else {
        panic!("compact arm must remain one typed Choice candidate")
    };
    assert!(matches!(arm.action(), HirChoiceCompactAction::Out(_)));

    let scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), choice.body().scope())
        .expect("published Choice body scope");
    assert_eq!(scope.parent(), Some(root.scope()));
    assert_eq!(scope.owner(), &HirScopeOwner::Expr(*owner));
}

#[test]
fn missing_choice_body_keeps_choice_payload_and_poisoned_outer_owner() {
    let parsed = parsed_source("choice-missing-body", &["choice".into()]);
    let (module, owners, _) = lower_and_publish(&parsed);
    let [owner] = owners.as_slice() else {
        panic!("fixture must publish one recovered Choice expression")
    };
    let root = expression(&module, *owner);
    assert!(root.is_poisoned());
    let HirExprKind::Choice(choice) = root.kind() else {
        panic!("missing body must poison the known Choice family")
    };
    assert!(choice.body().items().is_empty());
    assert!(module.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        HirDiagnostic::Recovery(diagnostic)
            if diagnostic.owner() == SyntheticOwner::Expr(*owner)
    )));
}
