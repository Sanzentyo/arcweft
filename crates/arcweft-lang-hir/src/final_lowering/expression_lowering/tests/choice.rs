use super::*;

use std::fmt::Write;

use crate::expr::{
    HirChoiceBody, HirChoiceCompactAction, HirChoiceCompactArm, HirChoiceExpr, HirChoiceItem,
};

fn recovery_operand_ordinals(module: &HirModule, owner: ExprId) -> Vec<u32> {
    let mut ordinals = module
        .arenas()
        .expressions()
        .try_iter(module.slots())
        .expect("Choice expression inventory")
        .filter_map(|(expression, _)| {
            let metadata = module
                .slots()
                .resolve(expression)
                .expect("Choice expression metadata");
            match metadata.origin() {
                HirOrigin::Synthetic(key)
                    if key.owner() == SyntheticOwner::Expr(owner)
                        && key.role() == SyntheticRole::RecoveryOperand =>
                {
                    Some(key.ordinal())
                }
                HirOrigin::Source(_) | HirOrigin::Synthetic(_) => None,
            }
        })
        .collect::<Vec<_>>();
    ordinals.sort_unstable();
    ordinals
}

fn choice_required_expression_limit_fixture(one_over: bool) -> String {
    let maximum = HirLimit::SyntheticDescendantsPerOwner.maximum();
    assert_eq!(maximum % 2, 0, "compact-arm fixture requires an even limit");
    let arm_count = maximum / 2;
    let mut source = String::from("(choice {\n");
    for ordinal in 0..arm_count {
        if ordinal + 1 == arm_count {
            writeln!(&mut source, "@.item_{ordinal} =>").expect("writing to a String cannot fail");
        } else {
            writeln!(&mut source, "@.item_{ordinal} \"Item {ordinal}\" => unit")
                .expect("writing to a String cannot fail");
        }
    }
    if one_over {
        source.push_str("option\n");
    }
    source.push_str("})");
    source
}

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

#[test]
fn choice_required_expression_ordinals_ignore_optional_absence_and_recovery_state() {
    let parsed = parsed_source(
        "choice-required-expression-ordinal-stability",
        &[
            "(choice { @.only \"Only\" => })".into(),
            "(choice { @.only \"Only\" if true => })".into(),
            "(choice { @.only \"Only\" if => })".into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    let [without_condition, authored_condition, recovered_condition] = owners.as_slice() else {
        panic!("fixture must publish three Choice expressions")
    };

    assert_eq!(recovery_operand_ordinals(&module, *without_condition), [1]);
    assert_eq!(recovery_operand_ordinals(&module, *authored_condition), [2]);
    assert_eq!(
        recovery_operand_ordinals(&module, *recovered_condition),
        [1, 2]
    );
}

#[test]
fn choice_source_freeze_rejects_recovery_operand_slot_substitution() {
    assert_expression_source_freeze_rejects(
        "choice-recovery-slot-substitution",
        "(choice { @.only => })",
        |transaction, root| {
            let (scope, state, id, body_scope, plan, arm_id, label, condition, out) = {
                let (slots, arenas) = transaction.storage_mut();
                let root_payload = arenas
                    .expressions()
                    .resolve_staged(slots, root)
                    .expect("staged recovered Choice");
                let HirExprKind::Choice(choice) = root_payload.kind() else {
                    panic!("recovered compact arm must remain a Choice")
                };
                let [HirChoiceItem::CompactArm(arm)] = choice.body().items() else {
                    panic!("fixture must retain one compact arm")
                };
                let HirChoiceCompactAction::Out(out) = arm.action() else {
                    panic!("authored arrow must retain an Out recovery child")
                };
                (
                    root_payload.scope(),
                    root_payload.state().clone(),
                    choice.id().cloned(),
                    choice.body().scope(),
                    choice.plan().cloned(),
                    arm.id().clone(),
                    arm.label(),
                    arm.condition(),
                    *out,
                )
            };
            assert_ne!(label, out, "fixture must allocate two recovery operands");

            let replacement = HirExpr::try_new(
                scope,
                HirExprKind::Choice(HirChoiceExpr::new(
                    id,
                    HirChoiceBody::new(
                        body_scope,
                        Box::new([HirChoiceItem::CompactArm(HirChoiceCompactArm::new(
                            arm_id,
                            out,
                            condition,
                            HirChoiceCompactAction::Out(label),
                        ))]),
                    ),
                    plan,
                )),
                state,
            )
            .expect("same-module recovery-slot substitution is locally constructible");
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .expressions()
                .revise_finalized(slots, root, replacement)
                .expect("test-only Choice recovery-slot substitution");
        },
    );
}

#[test]
fn choice_required_expression_exact_limit_publishes_last_recovery_ordinals() {
    let maximum = HirLimit::SyntheticDescendantsPerOwner.maximum();
    let parsed = parsed_source(
        "choice-required-expression-limit-exact",
        &[choice_required_expression_limit_fixture(false)],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    let [owner] = owners.as_slice() else {
        panic!("fixture must publish one Choice expression")
    };

    assert_eq!(
        recovery_operand_ordinals(&module, *owner),
        [
            u32::try_from(maximum - 2).expect("production limit fits u32"),
            u32::try_from(maximum - 1).expect("production limit fits u32"),
        ]
    );
}

#[test]
fn choice_required_expression_one_over_limit_rolls_back_atomically() {
    let maximum = HirLimit::SyntheticDescendantsPerOwner.maximum();
    let parsed = parsed_source(
        "choice-required-expression-limit-one-over",
        &[choice_required_expression_limit_fixture(true)],
    );
    let attached = attached_expressions(&parsed)
        .pop()
        .expect("one Choice expression");
    let mut database = HirDatabase::try_new().expect("Choice limit database");
    let before = database.test_state();
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let result = transaction.lower_attached_expression(&attached, scope);

    assert!(
        matches!(
            result,
            Err(HirLowerFailure::Limit(error))
                if error.limit() == HirLimit::SyntheticDescendantsPerOwner
                    && error.observed() == maximum + 1
                    && error.maximum() == maximum
        ),
        "unexpected one-over result: {result:?}"
    );
    assert!(transaction.finish(&mut database).is_err());
    assert_eq!(database.test_state(), before);
    assert!(database.current(&module_key(&parsed)).is_none());
}
