use super::*;

#[test]
fn attached_closure_retains_parameter_type_and_body_identities() {
    let expression = expression(
        "|item: Label, fallback| -> Text { item.text }",
        SyntaxKind::ClosureExpression,
    );
    let ExpressionProjection::Closure(closure) = expression.projection() else {
        panic!("closure expression must retain the typed Closure projection");
    };
    assert_eq!(closure.parameters().len(), 2);
    assert!(closure.parameters()[0].has_type());
    assert!(!closure.parameters()[1].has_type());
    assert!(closure.has_result_type());
    assert_eq!(closure.body(), SyntaxExpressionSlot::Authored);

    let [item, fallback] = expression.closure_parameters() else {
        panic!("closure must attach two ordered parameter identities");
    };
    assert!(item.ty().is_some());
    assert!(fallback.ty().is_none());
    assert!(expression.closure_result_type().is_some());
    assert_eq!(expression.children().len(), 1);
    assert_eq!(expression.children()[0].ordinal(), 0);
    assert!(
        expression.children()[0]
            .authored_semantic()
            .expect("closure body access")
            .is_some()
    );
}

#[test]
fn attached_callback_call_retains_one_central_closure_argument() {
    let expression = expression(
        "items.map { item: Label, index => item.text }",
        SyntaxKind::CallExpression,
    );
    let ExpressionProjection::Call(SyntaxCallProjection::CallbackBlock(callback)) =
        expression.projection()
    else {
        panic!("callback block must use the central Call projection");
    };
    assert_eq!(callback.callback(), SyntaxExpressionSlot::Authored);
    assert_eq!(
        expression
            .children()
            .iter()
            .map(AttachedExpressionChild::ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );

    let callback = expression.children()[1]
        .authored_semantic()
        .expect("callback child access")
        .expect("authored callback Closure");
    let ExpressionProjection::Closure(closure) = callback.projection() else {
        panic!("the sole callback argument must be the central Closure projection");
    };
    assert!(matches!(
        closure.syntax(),
        SyntaxClosureSyntax::CallbackBlock {
            explicit_header: true,
            ..
        }
    ));
    assert_eq!(closure.parameters().len(), 2);
    assert!(closure.parameters()[0].has_type());
    assert!(!closure.parameters()[1].has_type());
    assert_eq!(callback.closure_parameters().len(), 2);
    assert!(callback.closure_parameters()[0].ty().is_some());
    assert!(callback.closure_parameters()[1].ty().is_none());
    assert!(
        callback
            .component(ExpressionComponentRole::ClosureOpenDelimiter)
            .is_some()
    );
    assert!(
        callback
            .component(ExpressionComponentRole::ClosureCloseDelimiter)
            .is_some()
    );
    assert!(
        callback
            .component(ExpressionComponentRole::ClosureFatArrow)
            .is_some()
    );
    assert!(
        callback
            .component(ExpressionComponentRole::ClosureParameterSeparator { following: 1 })
            .is_some()
    );
}

#[test]
fn attached_implicit_callback_body_uses_the_central_block_projection() {
    let expression = expression(
        "items.each { let value = 1; value }",
        SyntaxKind::CallExpression,
    );
    let ExpressionProjection::Call(SyntaxCallProjection::CallbackBlock(_)) =
        expression.projection()
    else {
        panic!("implicit callback block must use the central Call projection");
    };
    let callback = expression.children()[1]
        .authored_semantic()
        .expect("callback child access")
        .expect("authored callback Closure");
    let ExpressionProjection::Closure(closure) = callback.projection() else {
        panic!("callback argument must be a Closure");
    };
    assert!(matches!(
        closure.syntax(),
        SyntaxClosureSyntax::CallbackBlock {
            explicit_header: false,
            ..
        }
    ));
    assert!(closure.parameters().is_empty());
    let body = callback.children()[0]
        .authored_semantic()
        .expect("callback body access")
        .expect("authored callback body");
    assert!(matches!(body.projection(), ExpressionProjection::Block));
}

#[test]
fn attached_computation_blocks_retain_the_selected_kind_and_block_owner() {
    let cases = [
        ("result { 1 }", SyntaxComputationBlockKind::Result),
        ("task { 1 }", SyntaxComputationBlockKind::Task),
        ("seq { 1 }", SyntaxComputationBlockKind::Seq),
        ("stream { 1 }", SyntaxComputationBlockKind::Stream),
    ];
    for (source, expected) in cases {
        let expression = expression(source, SyntaxKind::ComputationBlockExpression);
        assert_eq!(
            expression.projection(),
            &ExpressionProjection::ComputationBlock(expected)
        );
        assert!(expression.block().is_some());
        assert!(expression.children().is_empty());
    }
}

#[test]
fn attached_scope_omission_is_a_block_and_invalid_present_name_stays_typed() {
    let named = expression(
        "scope retry { let value = 1; value }",
        SyntaxKind::NamedBlockExpression,
    );
    let ExpressionProjection::NamedBlock(Ok(name)) = named.projection() else {
        panic!("authored valid scope name must remain a named block");
    };
    assert_eq!(name.as_str(), "retry");
    assert!(named.block().is_some());
    assert!(named.component(ExpressionComponentRole::Name).is_some());

    let unnamed = expression("scope { 1 }", SyntaxKind::NamedBlockExpression);
    assert!(matches!(unnamed.projection(), ExpressionProjection::Block));
    assert!(unnamed.block().is_some());
    assert!(unnamed.component(ExpressionComponentRole::Name).is_none());

    let invalid = expression("scope 9bad { 1 }", SyntaxKind::NamedBlockExpression);
    assert!(matches!(
        invalid.projection(),
        ExpressionProjection::NamedBlock(Err(SyntaxNameIssue::InvalidStart { spelling }))
            if spelling.as_ref() == "9bad"
    ));
    assert!(invalid.block().is_some());
    assert!(invalid.component(ExpressionComponentRole::Name).is_some());
}

#[test]
fn attached_if_retains_required_children_and_omitted_else_insertion() {
    let expression = expression("if true { 1 }", SyntaxKind::IfExpression);
    let ExpressionProjection::If {
        condition,
        then_branch,
        else_branch,
    } = expression.projection()
    else {
        panic!("if expression must retain the typed If projection");
    };
    assert_eq!(*condition, SyntaxExpressionSlot::Authored);
    assert_eq!(*then_branch, SyntaxExpressionSlot::Authored);
    assert_eq!(*else_branch, None);
    assert_eq!(expression.children().len(), 2);
    assert_eq!(expression.children()[0].ordinal(), 0);
    assert_eq!(expression.children()[1].ordinal(), 1);

    let then_source = expression
        .component(ExpressionComponentRole::ThenBranch)
        .expect("then-branch source");
    let omitted_else = expression
        .component(ExpressionComponentRole::ElseBranch)
        .expect("omitted else insertion");
    assert_eq!(omitted_else.range().start(), then_source.range().end());
    assert_eq!(omitted_else.range().start(), omitted_else.range().end());
}

#[test]
fn attached_if_distinguishes_missing_required_slots_from_omission() {
    let missing_condition = expression("if { 1 }", SyntaxKind::IfExpression);
    let ExpressionProjection::If {
        condition,
        then_branch,
        else_branch,
    } = missing_condition.projection()
    else {
        panic!("if expression must retain the typed If projection");
    };
    assert_eq!(*condition, SyntaxExpressionSlot::Missing);
    assert_eq!(*then_branch, SyntaxExpressionSlot::Authored);
    assert_eq!(*else_branch, None);
    assert!(matches!(
        &missing_condition.children()[0],
        AttachedExpressionChild::Missing { ordinal: 0, .. }
    ));

    let missing_then = expression("if true", SyntaxKind::IfExpression);
    let ExpressionProjection::If {
        condition,
        then_branch,
        else_branch,
    } = missing_then.projection()
    else {
        panic!("if expression must retain the typed If projection");
    };
    assert_eq!(*condition, SyntaxExpressionSlot::Authored);
    assert_eq!(*then_branch, SyntaxExpressionSlot::Missing);
    assert_eq!(*else_branch, None);
    assert!(matches!(
        &missing_then.children()[1],
        AttachedExpressionChild::Missing { ordinal: 1, .. }
    ));

    let missing_authored_else = expression("if true { 1 } else", SyntaxKind::IfExpression);
    let ExpressionProjection::If { else_branch, .. } = missing_authored_else.projection() else {
        panic!("if expression must retain the typed If projection");
    };
    assert_eq!(*else_branch, Some(SyntaxExpressionSlot::Missing));
    assert!(matches!(
        &missing_authored_else.children()[2],
        AttachedExpressionChild::Missing { ordinal: 2, .. }
    ));
}

#[test]
fn attached_if_retains_authored_else_if_as_one_semantic_child() {
    let expression = expression(
        "if true { 1 } else if false { 2 } else { 3 }",
        SyntaxKind::IfExpression,
    );
    let ExpressionProjection::If {
        else_branch: Some(SyntaxExpressionSlot::Authored),
        ..
    } = expression.projection()
    else {
        panic!("outer if must retain one authored else child");
    };
    assert_eq!(expression.children().len(), 3);
    let nested = expression.children()[2]
        .authored_semantic()
        .expect("nested else-if access")
        .expect("authored nested else-if");
    assert!(matches!(
        nested.projection(),
        ExpressionProjection::If { .. }
    ));
}

#[test]
fn attached_if_stops_condition_before_else_and_rejects_forged_omission_site() {
    let missing_then = expression("if true else { 2 }", SyntaxKind::IfExpression);
    let ExpressionProjection::If {
        condition,
        then_branch,
        else_branch,
    } = missing_then.projection()
    else {
        panic!("if expression must retain the typed If projection");
    };
    assert_eq!(*condition, SyntaxExpressionSlot::Authored);
    assert_eq!(*then_branch, SyntaxExpressionSlot::Missing);
    assert_eq!(*else_branch, Some(SyntaxExpressionSlot::Authored));

    let forged = [
        SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
        SyntaxEvent::expression_start(
            SyntaxKind::IfExpression,
            SyntaxRole::Element(0),
            PendingExpressionProjection::new(
                ExpressionProjection::If {
                    condition: SyntaxExpressionSlot::Missing,
                    then_branch: SyntaxExpressionSlot::Missing,
                    else_branch: None,
                },
                vec![
                    PendingExpressionComponent::new(
                        ExpressionComponentRole::Condition,
                        SourceRange::new(0, 0),
                    ),
                    PendingExpressionComponent::new(
                        ExpressionComponentRole::ThenBranch,
                        SourceRange::new(1, 1),
                    ),
                    PendingExpressionComponent::new(
                        ExpressionComponentRole::ElseBranch,
                        SourceRange::new(0, 0),
                    ),
                ],
            ),
        ),
        SyntaxEvent::start(SyntaxKind::MissingExpression, SyntaxRole::Condition),
        SyntaxEvent::FinishNode,
        SyntaxEvent::start(SyntaxKind::MissingExpression, SyntaxRole::ThenBranch),
        SyntaxEvent::FinishNode,
        SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(0, 1)),
        SyntaxEvent::FinishNode,
        SyntaxEvent::FinishNode,
    ];
    let source_document = document("x");
    let build = build_grammar(&source_document, &forged).unwrap();
    assert_eq!(
        attach_build(source_document, &build).unwrap_err(),
        AttachmentFailure::SnapshotInvariant,
        "an omitted else must be inserted exactly at the then-branch end"
    );
}

#[test]
fn attached_if_let_retains_pattern_and_fixed_semantic_child_ordinals() {
    let unguarded = expression(
        "if let value = 1 { value } else { 0 }",
        SyntaxKind::IfLetExpression,
    );
    let ExpressionProjection::IfLet {
        scrutinee,
        guard,
        then_branch,
        else_branch,
    } = unguarded.projection()
    else {
        panic!("if-let expression must retain the typed IfLet projection");
    };
    assert_eq!(*scrutinee, SyntaxExpressionSlot::Authored);
    assert_eq!(*guard, None);
    assert_eq!(*then_branch, SyntaxExpressionSlot::Authored);
    assert_eq!(*else_branch, Some(SyntaxExpressionSlot::Authored));
    assert!(unguarded.pattern().is_some());
    assert_eq!(
        unguarded
            .children()
            .iter()
            .map(AttachedExpressionChild::ordinal)
            .collect::<Vec<_>>(),
        vec![0, 2, 3]
    );
    assert!(
        unguarded
            .component(ExpressionComponentRole::Guard)
            .is_none()
    );

    let guarded = expression(
        "if let value = 1 when true { value } else { 0 }",
        SyntaxKind::IfLetExpression,
    );
    let ExpressionProjection::IfLet {
        guard: Some(SyntaxExpressionSlot::Authored),
        ..
    } = guarded.projection()
    else {
        panic!("guarded if-let must retain one authored Guard slot");
    };
    assert_eq!(
        guarded
            .children()
            .iter()
            .map(AttachedExpressionChild::ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert!(guarded.component(ExpressionComponentRole::Guard).is_some());
}

#[test]
fn attached_if_let_distinguishes_each_missing_slot_from_omitted_else() {
    let cases = [
        ("if let value { 1 } else { 2 }", 0),
        ("if let value = 1 when { 1 } else { 2 }", 1),
        ("if let value = 1 else { 2 }", 2),
        ("if let value = 1 { 1 } else", 3),
    ];
    for (source, missing_ordinal) in cases {
        let expression = expression(source, SyntaxKind::IfLetExpression);
        assert!(matches!(
            expression
                .children()
                .iter()
                .find(|child| child.ordinal() == missing_ordinal),
            Some(AttachedExpressionChild::Missing { .. })
        ));
    }

    let omitted = expression("if let value = 1 { value }", SyntaxKind::IfLetExpression);
    let ExpressionProjection::IfLet {
        else_branch: None, ..
    } = omitted.projection()
    else {
        panic!("omitted if-let else must remain distinct from a missing authored else");
    };
    assert!(omitted.children().iter().all(|child| child.ordinal() != 3));
    let then_source = omitted
        .component(ExpressionComponentRole::ThenBranch)
        .expect("if-let then source");
    let omitted_else = omitted
        .component(ExpressionComponentRole::ElseBranch)
        .expect("if-let required-tail insertion");
    assert_eq!(omitted_else.range().start(), then_source.range().end());
    assert_eq!(omitted_else.range().start(), omitted_else.range().end());
}
