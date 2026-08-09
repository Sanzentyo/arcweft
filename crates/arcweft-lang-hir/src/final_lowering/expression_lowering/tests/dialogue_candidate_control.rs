use super::*;
use crate::dialogue_application::HirPostfixBracketCandidates;
use crate::pattern::{HirPatternBinding, HirPatternKind};
use crate::scope::CaptureAccess;
use crate::type_ref::HirTypeKind;

fn ambiguous_closure_tuple_candidate(plain_element_count: usize) -> String {
    let mut source = String::from("items[(|value: Pair| value");
    for _ in 0..plain_element_count {
        source.push_str(",a");
    }
    source.push_str(")]");
    source
}

fn index_candidate(module: &HirModule, owner: ExprId) -> (ExprId, &crate::expr::HirIndexExpr) {
    let HirExprKind::PostfixBracket(postfix) = expression(module, owner).kind() else {
        panic!("fixture root must remain the ambiguous E34 postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { index, .. } = postfix.candidates() else {
        panic!("ordinary index interpretation must remain typed");
    };
    let HirExprKind::Index(index_payload) = expression(module, *index).kind() else {
        panic!("ordinary interpretation must retain its Index root");
    };
    (*index, index_payload)
}

fn assert_candidate_origin<I: HirTypedId>(module: &HirModule, id: I, outer: ExprId, ordinal: u32) {
    let metadata = module.slots().resolve(id).expect("candidate slot metadata");
    assert!(matches!(
        metadata.origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Expr(outer)
                && key.role() == SyntheticRole::PostfixIndexCandidateExpression
                && key.ordinal() == ordinal
    ));
}

#[test]
fn closure_candidate_uses_shared_pattern_type_scope_and_local_arenas() {
    let parsed = parsed_source(
        "dialogue-candidate-closure",
        &["items[|value: Pair| value]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let outer = owners[0];
    let (index_id, index) = index_candidate(&module, outer);
    let closure_id = index.index();
    let HirExprKind::Closure(closure) = expression(&module, closure_id).kind() else {
        panic!("candidate primary must retain the central Closure payload");
    };
    let [parameter] = closure.parameters() else {
        panic!("candidate Closure must retain one parameter");
    };
    let ty = parameter.ty().expect("candidate parameter annotation");
    let pattern = module
        .arenas()
        .patterns()
        .resolve(module.slots(), parameter.pattern())
        .expect("candidate parameter Pattern");
    let HirPatternKind::Binding(binding) = pattern.kind() else {
        panic!("Closure annotation must not rewrite its binding Pattern family");
    };
    let HirPatternBinding::Bound {
        name,
        local: local_id,
    } = binding
    else {
        panic!("candidate parameter must own one admitted Local");
    };
    assert_eq!(name.as_str(), "value");
    let local = module
        .arenas()
        .locals()
        .resolve(module.slots(), *local_id)
        .expect("candidate parameter Local");
    assert_eq!(local.kind(), HirLocalKind::ClosureParameter);
    assert_eq!(local.scope(), closure.scope());
    assert_eq!(local.pattern(), Some(parameter.pattern()));
    assert_eq!(
        local.annotation(),
        None,
        "Closure parameter annotation belongs to HirClosureParameter, not the Pattern Local"
    );
    assert!(matches!(
        module
            .arenas()
            .types()
            .resolve(module.slots(), ty)
            .expect("candidate parameter Type")
            .kind(),
        HirTypeKind::Path(path) if path.segments().len() == 1
    ));

    let scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), closure.scope())
        .expect("candidate Closure scope");
    assert_eq!(scope.kind(), HirScopeKind::Closure);
    assert_eq!(scope.parent(), Some(expression(&module, outer).scope()));
    assert_eq!(scope.owner(), &HirScopeOwner::Expr(closure_id));
    assert_eq!(scope.locals(), [*local_id]);
    assert_eq!(expression(&module, closure.body()).scope(), closure.scope());

    assert_candidate_origin(&module, index_id, outer, 0);
    assert_candidate_origin(&module, closure_id, outer, 1);
    assert_candidate_origin(&module, closure.body(), outer, 2);
    assert_candidate_origin(&module, parameter.pattern(), outer, 0);
    assert_candidate_origin(&module, ty, outer, 0);
    assert_candidate_origin(&module, closure.scope(), outer, 0);
    assert_candidate_origin(&module, *local_id, outer, 0);
}

#[test]
fn closure_candidate_defers_capture_identity_until_source_order_is_known() {
    let parsed = parsed_source(
        "dialogue-candidate-closure-captures",
        &["result { let left = 1; let right = 2; items[|| right + left + right] }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let HirExprKind::ComputationBlock(block) = expression(&module, owners[0]).kind() else {
        panic!("fixture root must retain the Result computation block");
    };
    let postfix_id = block.tail();
    let (_, index) = index_candidate(&module, postfix_id);
    let closure_id = index.index();
    let HirExprKind::Closure(closure) = expression(&module, closure_id).kind() else {
        panic!("index candidate must retain its Closure primary");
    };
    let [right_id, left_id] = closure.captures() else {
        panic!("candidate Closure must retain exactly two unique captures");
    };

    let right = module
        .resolve_capture(*right_id)
        .expect("right candidate capture");
    let left = module
        .resolve_capture(*left_id)
        .expect("left candidate capture");
    assert_eq!(
        module.resolve_local(right.local()).unwrap().name().as_str(),
        "right"
    );
    assert_eq!(
        module.resolve_local(left.local()).unwrap().name().as_str(),
        "left"
    );
    assert_eq!(right.access(), CaptureAccess::Read);
    assert_eq!(left.access(), CaptureAccess::Read);
    assert!(right.first_use().range().start() < left.first_use().range().start());

    for (ordinal, capture_id) in closure.captures().iter().copied().enumerate() {
        let capture = module.resolve_capture(capture_id).unwrap();
        let metadata = module.metadata(capture_id).unwrap();
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(closure_id)
                    && key.role() == SyntheticRole::ClosureCapture
                    && key.ordinal() == u32::try_from(ordinal).unwrap()
        ));
        assert!(matches!(
            metadata.source_site(),
            HirSourceSite::Insertion(insertion)
                if insertion.offset() == capture.first_use().range().start()
                    && insertion.source_identity() == capture.first_use().source()
        ));
    }
}

#[test]
fn ambiguous_candidate_passes_reuse_one_outer_closure_capture_ledger() {
    let parsed = parsed_source(
        "dialogue-candidate-parent-capture-reuse",
        &[
            "result { let beta = 1; let alpha = 2; let gamma = 3; || items[(beta, alpha, beta, gamma)] }"
                .into(),
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let HirExprKind::ComputationBlock(block) = expression(&module, owners[0]).kind() else {
        panic!("fixture root must retain the Result computation block");
    };
    let closure_id = block.tail();
    let HirExprKind::Closure(closure) = expression(&module, closure_id).kind() else {
        panic!("fixture tail must retain the ordinary outer Closure");
    };
    assert!(matches!(
        expression(&module, closure.body()).kind(),
        HirExprKind::PostfixBracket(_)
    ));
    let [beta_id, alpha_id, gamma_id] = closure.captures() else {
        panic!("both candidate passes must reuse three outer CaptureIds");
    };
    let names = [beta_id, alpha_id, gamma_id].map(|capture_id| {
        let capture = module.resolve_capture(*capture_id).unwrap();
        module
            .resolve_local(capture.local())
            .unwrap()
            .name()
            .as_str()
    });
    assert_eq!(names, ["beta", "alpha", "gamma"]);
    assert_eq!(module.captures().count(), 3);
    let starts = [beta_id, alpha_id, gamma_id].map(|capture_id| {
        module
            .resolve_capture(*capture_id)
            .unwrap()
            .first_use()
            .range()
            .start()
    });
    assert!(starts[0] < starts[1] && starts[1] < starts[2]);
}

#[test]
fn candidate_direct_assignment_upgrades_the_reused_capture() {
    let parsed = parsed_source(
        "dialogue-candidate-capture-reassign",
        &["result { let mut outer = 0; items[|| { outer = outer + 1; outer }] }".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let HirExprKind::ComputationBlock(block) = expression(&module, owners[0]).kind() else {
        panic!("fixture root must retain the Result computation block");
    };
    let (_, index) = index_candidate(&module, block.tail());
    let closure_id = index.index();
    let HirExprKind::Closure(closure) = expression(&module, closure_id).kind() else {
        panic!("index candidate must retain its Closure primary");
    };
    let [capture_id] = closure.captures() else {
        panic!("candidate assignment must reuse one capture");
    };
    let capture = module.resolve_capture(*capture_id).unwrap();
    assert_eq!(capture.access(), CaptureAccess::Reassign);
    assert_eq!(
        module
            .resolve_local(capture.local())
            .unwrap()
            .name()
            .as_str(),
        "outer"
    );
}

#[test]
fn if_let_candidate_keeps_binding_scope_and_missing_then_recovery() {
    let parsed = parsed_source(
        "dialogue-candidate-if-let",
        &["items[if let value = source else fallback]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let outer = owners[0];
    let (_, index) = index_candidate(&module, outer);
    let if_let_id = index.index();
    let HirExprKind::IfLet(if_let) = expression(&module, if_let_id).kind() else {
        panic!("candidate primary must retain IfLet");
    };
    assert!(if_let.guard().is_none());
    assert!(matches!(
        expression(&module, if_let.scrutinee()).kind(),
        HirExprKind::Path(_)
    ));
    assert!(matches!(
        expression(&module, if_let.else_branch()).kind(),
        HirExprKind::Path(_)
    ));
    assert_eq!(
        expression(&module, if_let.then_branch()).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::ThenBranch,
        })
    );
    assert_eq!(
        expression(&module, if_let_id).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::ThenBranch,
        })
    );

    let scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), if_let.scope())
        .expect("candidate IfLet binding scope");
    assert_eq!(scope.kind(), HirScopeKind::Conditional);
    assert_eq!(scope.parent(), Some(expression(&module, outer).scope()));
    assert_eq!(scope.owner(), &HirScopeOwner::Expr(if_let_id));
    assert_eq!(scope.locals().len(), 1);
    let local = module
        .arenas()
        .locals()
        .resolve(module.slots(), scope.locals()[0])
        .expect("candidate IfLet Local");
    assert_eq!(local.kind(), HirLocalKind::PatternBinding);
    assert_eq!(local.pattern(), Some(if_let.pattern()));
    assert_eq!(
        expression(&module, if_let.scrutinee()).scope(),
        scope.parent().unwrap()
    );
    assert_eq!(
        expression(&module, if_let.then_branch()).scope(),
        if_let.scope()
    );
    assert_eq!(
        expression(&module, if_let.else_branch()).scope(),
        scope.parent().unwrap()
    );

    assert_candidate_origin(&module, if_let_id, outer, 1);
    assert_candidate_origin(&module, if_let.scrutinee(), outer, 2);
    assert_candidate_origin(&module, if_let.then_branch(), outer, 3);
    assert_candidate_origin(&module, if_let.else_branch(), outer, 4);
    assert_candidate_origin(&module, if_let.pattern(), outer, 0);
    assert_candidate_origin(&module, if_let.scope(), outer, 0);
    assert_candidate_origin(&module, scope.locals()[0], outer, 0);
}

#[test]
fn match_candidate_keeps_per_arm_scopes_patterns_guards_and_global_preorders() {
    let parsed = parsed_source(
        "dialogue-candidate-match",
        &["items[match source { value: I32 when ready => result, _ => fallback }]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let outer = owners[0];
    let (_, index) = index_candidate(&module, outer);
    let match_id = index.index();
    let HirExprKind::Match(match_expression) = expression(&module, match_id).kind() else {
        panic!("candidate primary must retain Match");
    };
    let [first, second] = match_expression.arms() else {
        panic!("candidate Match must retain two ordered arms");
    };
    assert!(first.guard().is_some());
    assert!(second.guard().is_none());
    assert_ne!(first.scope(), second.scope());
    assert_eq!(first.locals().len(), 1);
    assert!(second.locals().is_empty());

    for arm in [first, second] {
        let scope = module
            .arenas()
            .scopes()
            .resolve(module.slots(), arm.scope())
            .expect("candidate Match-arm scope");
        assert_eq!(scope.kind(), HirScopeKind::MatchArm);
        assert_eq!(scope.parent(), Some(expression(&module, outer).scope()));
        assert_eq!(scope.owner(), &HirScopeOwner::Expr(match_id));
        assert_eq!(scope.locals(), arm.locals());
        assert_eq!(expression(&module, arm.value()).scope(), arm.scope());
        if let Some(guard) = arm.guard() {
            assert_eq!(expression(&module, guard).scope(), arm.scope());
        }
    }
    assert_eq!(
        expression(&module, match_expression.scrutinee()).scope(),
        expression(&module, outer).scope()
    );

    let local = module
        .arenas()
        .locals()
        .resolve(module.slots(), first.locals()[0])
        .expect("candidate Match binding Local");
    assert_eq!(local.kind(), HirLocalKind::MatchBinding);
    assert_eq!(local.pattern(), Some(first.pattern()));
    let first_type = match module
        .arenas()
        .patterns()
        .resolve(module.slots(), first.pattern())
        .expect("first arm Pattern")
        .kind()
    {
        HirPatternKind::TypedBinding { ty, .. } => *ty,
        _ => panic!("first arm must retain its typed binding"),
    };
    assert_eq!(local.annotation(), Some(first_type));
    assert!(matches!(
        module
            .arenas()
            .patterns()
            .resolve(module.slots(), second.pattern())
            .expect("second arm Pattern")
            .kind(),
        HirPatternKind::Discard
    ));

    assert_candidate_origin(&module, match_id, outer, 1);
    assert_candidate_origin(&module, match_expression.scrutinee(), outer, 2);
    assert_candidate_origin(&module, first.guard().unwrap(), outer, 3);
    assert_candidate_origin(&module, first.value(), outer, 4);
    assert_candidate_origin(&module, second.value(), outer, 5);
    assert_candidate_origin(&module, first.pattern(), outer, 0);
    assert_candidate_origin(&module, second.pattern(), outer, 1);
    assert_candidate_origin(&module, first_type, outer, 0);
    assert_candidate_origin(&module, first.scope(), outer, 0);
    assert_candidate_origin(&module, second.scope(), outer, 1);
    assert_candidate_origin(&module, first.locals()[0], outer, 0);
}

#[test]
fn mixed_control_candidate_exact_aggregate_descendant_limit_publishes() {
    const FIXED_DESCENDANTS: usize = 9;

    let maximum = HirLimit::SyntheticDescendantsPerOwner.maximum();
    let plain_element_count = maximum
        .checked_sub(FIXED_DESCENDANTS)
        .expect("mixed candidate fixed descendants fit the production limit");
    let parsed = parsed_source(
        "dialogue-control-candidate-descendants-exact",
        &[ambiguous_closure_tuple_candidate(plain_element_count)],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let owner = owners[0];
    let candidate_origin = |metadata: &crate::slot::HirSlotMetadata| {
        matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && matches!(
                        key.role(),
                        SyntheticRole::PostfixIndexCandidateExpression
                            | SyntheticRole::DialogueContentCandidateExpression
                    )
        )
    };
    let expression_count = module
        .arenas()
        .expressions()
        .try_iter(module.slots())
        .expect("candidate expression inventory")
        .filter(|(id, _)| module.slots().resolve(*id).is_ok_and(&candidate_origin))
        .count();
    let type_count = module
        .arenas()
        .types()
        .try_iter(module.slots())
        .expect("candidate Type inventory")
        .filter(|(id, _)| module.slots().resolve(*id).is_ok_and(&candidate_origin))
        .count();
    let pattern_count = module
        .arenas()
        .patterns()
        .try_iter(module.slots())
        .expect("candidate Pattern inventory")
        .filter(|(id, _)| module.slots().resolve(*id).is_ok_and(&candidate_origin))
        .count();
    let scope_count = module
        .arenas()
        .scopes()
        .try_iter(module.slots())
        .expect("candidate Scope inventory")
        .filter(|(id, _)| module.slots().resolve(*id).is_ok_and(&candidate_origin))
        .count();
    let local_count = module
        .arenas()
        .locals()
        .try_iter(module.slots())
        .expect("candidate Local inventory")
        .filter(|(id, _)| module.slots().resolve(*id).is_ok_and(&candidate_origin))
        .count();
    assert_eq!(type_count, 1);
    assert_eq!(pattern_count, 1);
    assert_eq!(scope_count, 1);
    assert_eq!(local_count, 1);
    assert_eq!(
        expression_count + type_count + pattern_count + scope_count + local_count,
        maximum
    );
}

#[test]
fn mixed_control_candidate_one_over_aggregate_descendant_limit_rolls_back() {
    const FIXED_DESCENDANTS: usize = 9;

    let maximum = HirLimit::SyntheticDescendantsPerOwner.maximum();
    let plain_element_count = maximum
        .checked_sub(FIXED_DESCENDANTS)
        .and_then(|count| count.checked_add(1))
        .expect("mixed candidate one-over fixture");
    let parsed = parsed_source(
        "dialogue-control-candidate-descendants-one-over",
        &[ambiguous_closure_tuple_candidate(plain_element_count)],
    );
    let attached = attached_expressions(&parsed)
        .pop()
        .expect("one mixed ambiguous postfix expression");
    let mut database = HirDatabase::try_new().expect("mixed candidate-limit database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    assert!(matches!(
        transaction.lower_attached_expression(&attached, scope),
        Err(HirLowerFailure::Limit(error))
            if error.limit() == HirLimit::SyntheticDescendantsPerOwner
                && error.observed() == maximum + 1
                && error.maximum() == maximum
    ));
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&parsed)).is_none());
}
