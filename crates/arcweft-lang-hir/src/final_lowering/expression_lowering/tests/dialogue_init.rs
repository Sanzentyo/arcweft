use super::*;
use crate::dialogue_application::{
    HirAttachedContentApplication, HirAttachedContentApplicationFamily, HirLinePlan,
    HirLinePlanItem,
};
use crate::expr::HirExpr;

#[test]
fn dialogue_init_lowers_to_a_retained_child_scope_and_owned_statement_roots() {
    let parsed = parsed_source(
        "dialogue-init-scope",
        &[
            "alice()[本文。[p]] with { init { let initialized = 1; log.info(initialized) } }"
                .into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    let application = match expression(&module, owners[0]).kind() {
        HirExprKind::AttachedContentApplication(application) => application,
        kind => panic!("expected attached Dialogue application, found {kind:?}"),
    };
    let HirAttachedContentApplicationFamily::DialogueLine {
        plan: Some(plan), ..
    } = application.family()
    else {
        panic!("Dialogue application retains its line plan")
    };
    let [HirLinePlanItem::Init { scope, statements }] = plan.items() else {
        panic!("line plan retains the direct Init item")
    };
    assert_eq!(statements.len(), 2);
    let init_scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), *scope)
        .expect("Init scope is live in the published module");
    assert_eq!(init_scope.kind(), HirScopeKind::Block);
    assert_eq!(init_scope.parent(), Some(plan.root_scope()));
    assert_eq!(init_scope.owner(), &HirScopeOwner::Expr(owners[0]));
    assert_eq!(init_scope.locals().len(), 1);
    for statement in statements.iter() {
        assert_eq!(
            module
                .arenas()
                .statements()
                .resolve(module.slots(), *statement)
                .expect("Init statement is live")
                .scope(),
            *scope
        );
    }
    assert!(matches!(
        module
            .arenas()
            .statements()
            .resolve(module.slots(), statements[0])
            .expect("Init binding statement is live")
            .kind(),
        HirStmtKind::Let { .. }
    ));
}

#[test]
fn dialogue_init_source_projection_rejects_a_substituted_scope_owner() {
    assert_expression_freeze_rejects(
        "dialogue-init-scope-substitution",
        "alice()[本文。[p]] with { init { let initialized = 1 } }",
        |transaction, owner| {
            let (slots, arenas) = transaction.storage_mut();
            let expression = arenas
                .expressions()
                .resolve_staged(slots, owner)
                .expect("staged attached Dialogue expression");
            let expression_scope = expression.scope();
            let expression_state = expression.state().clone();
            let HirExprKind::AttachedContentApplication(application) = expression.kind() else {
                panic!("Init fixture remains an attached Dialogue application")
            };
            let HirAttachedContentApplicationFamily::DialogueLine {
                target,
                plan: Some(plan),
                coordinates,
            } = application.family()
            else {
                panic!("Init fixture retains its Dialogue line plan")
            };
            let mut items = plan.items().to_vec();
            let [HirLinePlanItem::Init { scope, .. }] = items.as_mut_slice() else {
                panic!("fixture has one direct Init item")
            };
            *scope = plan.root_scope();
            let plan = HirLinePlan::try_new(
                plan.root_scope(),
                plan.label().cloned(),
                items.into_boxed_slice(),
            )
            .expect("substituted scope remains a live typed HIR identifier");
            let family = HirAttachedContentApplicationFamily::DialogueLine {
                target: *target,
                plan: Some(plan),
                coordinates: coordinates.clone(),
            };
            let application = HirAttachedContentApplication::try_new_with_body_presence(
                owner,
                application.content().clone(),
                family,
                application.body_presence(),
            )
            .expect("substituted plan remains an internally valid payload");
            let payload = HirExpr::try_new(
                expression_scope,
                HirExprKind::AttachedContentApplication(application),
                expression_state,
            )
            .expect("substituted application remains a valid expression payload");
            arenas
                .expressions()
                .revise_finalized(slots, owner, payload)
                .expect("transactionally revise the staged expression")
        },
    );
}

#[test]
fn dialogue_init_keeps_an_authored_out_statement_in_its_scope() {
    let parsed = parsed_source(
        "dialogue-init-out-scope",
        &["alice()[本文。[p]] with { init { log.info(\"init-before-out\"); out () } }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert!(
        module.is_analysis_ready(),
        "diagnostics={:?}; recovered={:?}",
        module.diagnostics(),
        module.recovered_owners().collect::<Vec<_>>(),
    );
    let application = match expression(&module, owners[0]).kind() {
        HirExprKind::AttachedContentApplication(application) => application,
        kind => panic!("expected attached Dialogue application, found {kind:?}"),
    };
    let HirAttachedContentApplicationFamily::DialogueLine {
        plan: Some(plan), ..
    } = application.family()
    else {
        panic!("Dialogue application retains its line plan")
    };
    let [HirLinePlanItem::Init { scope, statements }] = plan.items() else {
        panic!("line plan retains the direct Init item")
    };
    assert_eq!(statements.len(), 2);
    assert!(matches!(
        module
            .arenas()
            .statements()
            .resolve(module.slots(), statements[1])
            .expect("Init out statement is live")
            .kind(),
        HirStmtKind::Out { .. }
    ));
    assert!(statements.iter().all(|statement| {
        module
            .arenas()
            .statements()
            .resolve(module.slots(), *statement)
            .is_ok_and(|statement| statement.scope() == *scope)
    }));
}
