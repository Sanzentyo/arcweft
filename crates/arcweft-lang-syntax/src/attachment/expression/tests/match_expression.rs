use super::*;
use crate::attachment::AttachedMatchArmExpression;
use crate::expressions::{SyntaxMatchArmPart, SyntaxMatchBodyTerminator, SyntaxRequiredTokenState};

#[test]
fn attached_match_retains_ordered_arm_owners_and_exact_parts() {
    let expression = expression(
        "match value { ready when true => 1, _ => { 0 } }",
        SyntaxKind::MatchExpression,
    );
    let ExpressionProjection::Match(projection) = expression.projection() else {
        panic!("Match expression must retain the central Match projection");
    };
    assert_eq!(projection.scrutinee(), SyntaxExpressionSlot::Authored);
    assert_eq!(projection.terminator(), SyntaxMatchBodyTerminator::Closed);
    assert_eq!(projection.arms().len(), 2);
    assert_eq!(
        expression.children().len(),
        1,
        "only the scrutinee is a root child"
    );

    let [guarded, fallback] = expression.match_arms() else {
        panic!("Match must retain two source-ordered attached arms");
    };
    assert_eq!(guarded.projection(), &projection.arms()[0]);
    assert_eq!(fallback.projection(), &projection.arms()[1]);
    assert!(
        guarded
            .guard()
            .is_some_and(|guard| guard.authored().is_some())
    );
    assert!(fallback.guard().is_none());
    assert!(guarded.value().authored().is_some());
    assert!(fallback.value().authored().is_some());
    assert!(
        guarded.pattern().whole_source_span().range().start()
            < guarded.pattern().whole_source_span().range().end()
    );

    let whole = guarded
        .component(SyntaxMatchArmPart::Whole)
        .expect("arm whole source");
    let pattern = guarded
        .component(SyntaxMatchArmPart::Pattern)
        .expect("arm Pattern source");
    let guard = guarded
        .component(SyntaxMatchArmPart::Guard)
        .expect("arm Guard source");
    let arrow = guarded
        .component(SyntaxMatchArmPart::Arrow)
        .expect("arm arrow source");
    let value = guarded
        .component(SyntaxMatchArmPart::Value)
        .expect("arm Value source");
    assert_eq!(whole.range(), guarded.whole_source_span().range());
    assert_eq!(
        pattern.range(),
        guarded.pattern().whole_source_span().range()
    );
    assert_eq!(
        guard.range(),
        guarded.guard().unwrap().source_span().range()
    );
    assert_eq!(arrow.range().end() - arrow.range().start(), 2);
    assert_eq!(value.range(), guarded.value().source_span().range());
    assert!(
        fallback
            .value()
            .authored_semantic()
            .expect("fallback value access")
            .is_some_and(|value| matches!(value.projection(), ExpressionProjection::Block))
    );
}

#[test]
fn attached_match_preserves_missing_body_guard_arrow_value_and_close() {
    let missing_body = expression("match value", SyntaxKind::MatchExpression);
    let ExpressionProjection::Match(projection) = missing_body.projection() else {
        panic!("missing Match body must retain the Match projection");
    };
    assert_eq!(
        projection.terminator(),
        SyntaxMatchBodyTerminator::MissingBody
    );
    assert!(projection.arms().is_empty());
    assert!(missing_body.match_arms().is_empty());

    let missing_scrutinee = expression("match { _ => 1 }", SyntaxKind::MatchExpression);
    let ExpressionProjection::Match(projection) = missing_scrutinee.projection() else {
        panic!("missing scrutinee must retain the Match projection");
    };
    assert_eq!(projection.scrutinee(), SyntaxExpressionSlot::Missing);
    assert!(matches!(
        missing_scrutinee.children(),
        [AttachedExpressionChild::Missing { ordinal: 0, .. }]
    ));

    let missing_guard = expression("match value { _ when => 1 }", SyntaxKind::MatchExpression);
    let ExpressionProjection::Match(projection) = missing_guard.projection() else {
        panic!("missing guard must retain the Match projection");
    };
    assert_eq!(
        projection.arms()[0].guard(),
        Some(SyntaxExpressionSlot::Missing)
    );
    assert!(matches!(
        missing_guard.match_arms()[0].guard(),
        Some(AttachedMatchArmExpression::Missing { .. })
    ));

    let missing_arrow_and_value = expression("match value { _ }", SyntaxKind::MatchExpression);
    let ExpressionProjection::Match(projection) = missing_arrow_and_value.projection() else {
        panic!("recovered arm must retain the Match projection");
    };
    assert!(projection.has_recovery());
    assert_eq!(
        projection.arms()[0].arrow(),
        SyntaxRequiredTokenState::Missing
    );
    assert_eq!(projection.arms()[0].value(), SyntaxExpressionSlot::Missing);
    let arm = &missing_arrow_and_value.match_arms()[0];
    assert!(matches!(
        arm.value(),
        AttachedMatchArmExpression::Missing { .. }
    ));
    let arrow = arm.component(SyntaxMatchArmPart::Arrow).unwrap();
    let value = arm.component(SyntaxMatchArmPart::Value).unwrap();
    assert_eq!(arrow.range().start(), arrow.range().end());
    assert_eq!(arrow.range(), value.range());

    let missing_close = expression("match value { _ => 1", SyntaxKind::MatchExpression);
    let ExpressionProjection::Match(projection) = missing_close.projection() else {
        panic!("missing close must retain the Match projection");
    };
    assert_eq!(
        projection.terminator(),
        SyntaxMatchBodyTerminator::RecoveredMissingClose
    );
    assert_eq!(missing_close.match_arms().len(), 1);
}
