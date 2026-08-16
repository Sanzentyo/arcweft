use super::*;
use crate::expressions::{
    ExpressionProjection, ExpressionRecordFieldPart, SyntaxBinaryOperator,
    SyntaxDialogueContentProjection, SyntaxLifetimeRegistryScope, SyntaxPostfixBracketProjection,
    SyntaxRecordField,
};
use crate::grammar::budget::GrammarBudget;
use crate::grammar::event::{PendingStartProjection, SyntaxEvent};
use crate::incremental::SyntaxLimit;
use crate::parser::lexer::DocumentLexer;

fn expression_events(source: &str) -> Vec<SyntaxEvent> {
    let tokens = DocumentLexer::new(source).lex();
    let mut events = Vec::new();
    let mut budget = GrammarBudget::default();
    {
        let mut parser = DocumentParser::new(source, &tokens, &mut events, &mut budget);
        emit_expression(&mut parser, tokens.len(), SyntaxRole::Element(0));
        assert!(parser.is_at_end(), "{source}");
    }
    events
}

fn projection(events: &[SyntaxEvent], kind: SyntaxKind) -> &PendingExpressionProjection {
    events
        .iter()
        .find_map(|event| match event {
            SyntaxEvent::StartNode {
                kind: actual,
                projection: PendingStartProjection::Expression(projection),
                ..
            } if *actual == kind => Some(projection),
            _ => None,
        })
        .expect("expression leaf owns its projection")
}

#[test]
fn leaf_expression_matrix_projects_e01_through_e07_on_exact_start_events() {
    assert!(matches!(
        projection(&expression_events("()"), SyntaxKind::TupleExpression).projection(),
        ExpressionProjection::Unit
    ));

    let literal_events = expression_events("0x2a_u32");
    assert!(matches!(
        projection(&literal_events, SyntaxKind::LiteralExpression).projection(),
        ExpressionProjection::Literal(literal)
            if literal.numeric_digit_count() == Some(2)
    ));

    let entity_events = expression_events("@scene.entry");
    assert!(matches!(
        projection(
            &entity_events,
            SyntaxKind::EntityReferenceExpression
        )
        .projection(),
        ExpressionProjection::EntityReference(entity) if entity.value().is_ok()
    ));

    let lifetime_events = expression_events("'line.focus?");
    assert!(matches!(
        projection(&lifetime_events, SyntaxKind::LifetimePathExpression).projection(),
        ExpressionProjection::LifetimePath(path)
            if matches!(path.scope(), SyntaxLifetimeRegistryScope::Line)
                && path.segments().len() == 1
                && path.is_optional()
    ));

    assert!(matches!(
        projection(
            &expression_events("game::actor"),
            SyntaxKind::PathExpression
        )
        .projection(),
        ExpressionProjection::Path
    ));
    assert!(matches!(
        projection(
            &expression_events(".Ready"),
            SyntaxKind::ShortVariantExpression
        )
        .projection(),
        ExpressionProjection::ShortVariant(Ok(name)) if name.as_str() == "Ready"
    ));
    assert!(matches!(
        projection(&expression_events("_"), SyntaxKind::PlaceholderExpression).projection(),
        ExpressionProjection::Placeholder(SyntaxPlaceholderKind::PartialApplication)
    ));
    assert!(matches!(
        projection(&expression_events("^"), SyntaxKind::PlaceholderExpression).projection(),
        ExpressionProjection::Placeholder(SyntaxPlaceholderKind::PipeLeft)
    ));
}

#[test]
fn parenthesized_call_projection_retains_argument_forms_and_termination() {
    let events = expression_events("callee(first, limit = second, rest...,)");
    let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(call)) =
        projection(&events, SyntaxKind::CallExpression).projection()
    else {
        panic!("parenthesized Call projection");
    };
    assert!(matches!(
        call.callee(),
        crate::expressions::SyntaxCallCalleeProjection::Ordinary
    ));
    assert_eq!(call.arguments().len(), 3);
    assert!(matches!(
        &call.arguments()[0],
        SyntaxCallArgumentProjection::Positional {
            value: SyntaxExpressionSlot::Authored
        }
    ));
    assert!(matches!(
        &call.arguments()[1],
        SyntaxCallArgumentProjection::Named {
            name: Ok(name),
            equals: SyntaxRequiredTokenState::Present,
            value: SyntaxExpressionSlot::Authored,
        } if name.as_str() == "limit"
    ));
    assert!(matches!(
        &call.arguments()[2],
        SyntaxCallArgumentProjection::Spread {
            value: SyntaxExpressionSlot::Authored,
            ellipsis: SyntaxRequiredTokenState::Present,
        }
    ));
    assert_eq!(call.terminator(), SyntaxCallArgumentListTerminator::Closed);
    let components = projection(&events, SyntaxKind::CallExpression).components();
    assert!(components.iter().any(|component| {
        component.role() == ExpressionComponentRole::CallArgumentTrailingSeparator
    }));

    let recovered = expression_events("callee(name =");
    let pending = projection(&recovered, SyntaxKind::CallExpression);
    let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(call)) =
        pending.projection()
    else {
        panic!("recovered parenthesized Call projection");
    };
    assert!(matches!(
        &call.arguments()[0],
        SyntaxCallArgumentProjection::Named {
            name: Ok(name),
            equals: SyntaxRequiredTokenState::Present,
            value: SyntaxExpressionSlot::Missing,
        } if name.as_str() == "name"
    ));
    assert_eq!(
        call.terminator(),
        SyntaxCallArgumentListTerminator::RecoveredMissing
    );
    assert!(pending.has_recovery());
}

#[test]
fn empty_call_after_rejected_statement_owner_does_not_fabricate_a_callee_range() {
    let source = "callee()";
    let tokens = DocumentLexer::new(source).lex();
    let mut events = Vec::new();
    let mut budget = GrammarBudget::with_test_global_count(
        SyntaxLimit::Statements,
        SyntaxLimit::Statements.maximum(),
    );
    let accepted = budget.start(SyntaxKind::ProofCallStatement, SyntaxRole::Statement(0));
    assert!(!accepted);

    {
        let mut parser = DocumentParser::new(source, &tokens, &mut events, &mut budget);
        emit_expression(&mut parser, tokens.len(), SyntaxRole::Callee);
        assert!(parser.is_at_end());
    }

    assert_eq!(budget.failure(), Some(SyntaxLimit::Statements));
    assert!(events.is_empty());
}

#[test]
fn leading_turbofish_does_not_fabricate_a_missing_callee_call() {
    let leading = expression_events("::<T>(value)");
    assert!(!leading.iter().any(|event| matches!(
        event,
        SyntaxEvent::StartNode {
            kind: SyntaxKind::CallExpression,
            ..
        }
    )));

    let grouped = expression_events("(value)");
    assert!(grouped.iter().any(|event| matches!(
        event,
        SyntaxEvent::StartNode {
            kind: SyntaxKind::DelimitedGroup,
            transparent_expression_group: true,
            ..
        }
    )));
    assert!(!grouped.iter().any(|event| matches!(
        event,
        SyntaxEvent::StartNode {
            kind: SyntaxKind::CallExpression,
            ..
        }
    )));
}

#[test]
fn associated_call_projection_uses_the_same_attached_type_transaction() {
    for (source, explicit) in [
        ("String.with_capacity(8)", false),
        ("Vec<I32>.with_capacity(8)", false),
        ("Vec<I32>::with_capacity(8)", true),
    ] {
        let events = expression_events(source);
        let pending = projection(&events, SyntaxKind::CallExpression);
        let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(call)) =
            pending.projection()
        else {
            panic!("associated parenthesized Call projection: {source}");
        };
        match (call.callee(), explicit) {
            (
                crate::expressions::SyntaxCallCalleeProjection::UnresolvedDot {
                    member: Ok(member),
                    ..
                },
                false,
            )
            | (
                crate::expressions::SyntaxCallCalleeProjection::Associated {
                    member: Ok(member),
                    ..
                },
                true,
            ) => assert_eq!(member.as_str(), "with_capacity"),
            _ => panic!("wrong associated Call callee: {source}"),
        }
        assert_eq!(call.arguments().len(), 1);
        assert!(pending.components().iter().any(|component| {
            component.role() == ExpressionComponentRole::CallAssociatedReceiver
        }));
        assert!(pending.components().iter().any(|component| {
            component.role() == ExpressionComponentRole::CallAssociatedSeparator
        }));
        assert!(pending.components().iter().any(|component| {
            component.role() == ExpressionComponentRole::CallAssociatedMember
        }));
        assert!(
            !pending
                .components()
                .iter()
                .any(|component| { component.role() == ExpressionComponentRole::CallCallee })
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    SyntaxEvent::StartNode {
                        role: SyntaxRole::Type,
                        projection: PendingStartProjection::Type(type_projection),
                        ..
                    } if type_projection.path().steps().is_empty()
                ))
                .count(),
            1,
            "{source}"
        );
    }
}

#[test]
fn qualified_value_path_call_is_not_reclassified_as_an_associated_type_call() {
    let events = expression_events("pkg::service::invoke(x)");
    let pending = projection(&events, SyntaxKind::CallExpression);
    let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(call)) =
        pending.projection()
    else {
        panic!("qualified ordinary Call projection");
    };
    assert!(matches!(
        call.callee(),
        crate::expressions::SyntaxCallCalleeProjection::Ordinary
    ));
    assert!(
        pending
            .components()
            .iter()
            .any(|component| component.role() == ExpressionComponentRole::CallCallee)
    );
    assert!(!pending.components().iter().any(|component| matches!(
        component.role(),
        ExpressionComponentRole::CallAssociatedReceiver
            | ExpressionComponentRole::CallAssociatedSeparator
            | ExpressionComponentRole::CallAssociatedMember
    )));
}

#[test]
fn associated_call_recovery_starts_only_after_an_authored_separator() {
    use crate::expressions::{
        SyntaxAssociatedCallSyntax, SyntaxAssociatedReceiver, SyntaxAssociatedSeparator,
        SyntaxCallCalleeProjection,
    };

    let cases = ["Bad<>::member(x)", "Vec<I32>. (8)", "Vec<I32>.9bad(8)"];
    let calls = cases.map(|source| {
        let events = expression_events(source);
        let pending = projection(&events, SyntaxKind::CallExpression).clone();
        assert!(
            pending.validates_components(SourceRange::new(0, source.len())),
            "{source}: {:?}",
            pending.components()
        );
        let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(call)) =
            pending.projection()
        else {
            panic!("associated recovery Call projection: {source}");
        };
        (source, pending.clone(), call.clone())
    });

    assert!(matches!(
        calls[0].2.callee(),
        SyntaxCallCalleeProjection::Associated {
            receiver: SyntaxAssociatedReceiver::Present,
            separator: SyntaxAssociatedSeparator::Present(
                SyntaxAssociatedCallSyntax::ExplicitDoubleColon,
            ),
            member: Ok(member),
        } if member.as_str() == "member"
    ));

    assert!(matches!(
        calls[1].2.callee(),
        SyntaxCallCalleeProjection::Associated {
            receiver: SyntaxAssociatedReceiver::Present,
            separator: SyntaxAssociatedSeparator::Present(SyntaxAssociatedCallSyntax::DotFallback,),
            member: Err(crate::name::SyntaxNameIssue::Missing),
        }
    ));

    assert!(matches!(
        calls[2].2.callee(),
        SyntaxCallCalleeProjection::Associated {
            receiver: SyntaxAssociatedReceiver::Present,
            separator: SyntaxAssociatedSeparator::Present(SyntaxAssociatedCallSyntax::DotFallback,),
            member: Err(crate::name::SyntaxNameIssue::InvalidStart { .. }),
        }
    ));

    for (source, pending, _) in calls {
        for role in [
            ExpressionComponentRole::CallAssociatedReceiver,
            ExpressionComponentRole::CallAssociatedSeparator,
            ExpressionComponentRole::CallAssociatedMember,
        ] {
            assert!(
                pending
                    .components()
                    .iter()
                    .any(|component| component.role() == role),
                "{source}: missing {role:?}"
            );
        }
    }

    for source in [
        "::member(x)",
        "Vec<I32> with_capacity(8)",
        "Vec<I32>..with_capacity(8)",
    ] {
        let events = expression_events(source);
        assert!(
            !events.iter().any(|event| {
                let SyntaxEvent::StartNode {
                    projection: PendingStartProjection::Expression(projection),
                    ..
                } = event
                else {
                    return false;
                };
                matches!(
                    projection.projection(),
                    ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(call))
                        if matches!(call.callee(), SyntaxCallCalleeProjection::Associated { .. })
                )
            }),
            "{source} must not be reclassified as an associated Call"
        );
    }
}

#[test]
fn select_projection_keeps_missing_member_without_postfix_try() {
    let selected = expression_events("target.member");
    let pending = projection(&selected, SyntaxKind::SelectExpression);
    assert!(matches!(
        pending.projection(),
        ExpressionProjection::Select(SyntaxSelectedMember::Name(member))
            if member.as_str() == "member"
    ));
    assert!(pending.components().iter().any(|component| {
        component.role() == ExpressionComponentRole::Target
            && component.range() == SourceRange::new(0, 6)
    }));
    assert!(pending.components().iter().any(|component| {
        component.role() == ExpressionComponentRole::SelectedMember
            && component.range() == SourceRange::new(7, 13)
    }));

    let missing = expression_events("target.   ");
    let pending = projection(&missing, SyntaxKind::SelectExpression);
    assert!(matches!(
        pending.projection(),
        ExpressionProjection::Select(SyntaxSelectedMember::Missing)
    ));
    assert!(pending.components().iter().any(|component| {
        component.role() == ExpressionComponentRole::SelectedMember
            && component.range() == SourceRange::new(10, 10)
    }));
}

#[test]
fn numeric_select_member_keeps_the_inner_missing_select_before_generic_recovery() {
    for (source, insertion, recovery) in [
        ("target.42", 7, SourceRange::new(7, 9)),
        ("target. 42", 8, SourceRange::new(8, 10)),
    ] {
        let events = expression_events(source);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    SyntaxEvent::StartNode {
                        kind: SyntaxKind::SelectExpression,
                        ..
                    }
                ))
                .count(),
            1,
            "{source}"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    SyntaxEvent::StartNode {
                        kind: SyntaxKind::ErrorExpression,
                        ..
                    }
                ))
                .count(),
            1,
            "{source}"
        );

        let select = projection(&events, SyntaxKind::SelectExpression);
        assert!(matches!(
            select.projection(),
            ExpressionProjection::Select(SyntaxSelectedMember::Missing)
        ));
        assert!(select.components().iter().any(|component| {
            component.role() == ExpressionComponentRole::SelectedMember
                && component.range() == SourceRange::new(insertion, insertion)
        }));

        let error = projection(&events, SyntaxKind::ErrorExpression);
        assert!(matches!(error.projection(), ExpressionProjection::Error));
        assert!(error.components().iter().any(|component| {
            component.role() == ExpressionComponentRole::Recovery && component.range() == recovery
        }));
    }
}

#[test]
fn known_leaf_recovery_keeps_its_typed_family() {
    let unterminated = expression_events("\"unfinished");
    assert!(matches!(
        projection(&unterminated, SyntaxKind::LiteralExpression).projection(),
        ExpressionProjection::Literal(literal)
            if matches!(
                literal.value(),
                crate::literal::SyntaxLiteralValue::Invalid(_)
            )
    ));

    let missing_id = expression_events("@");
    assert!(matches!(
        projection(
            &missing_id,
            SyntaxKind::EntityReferenceExpression
        )
        .projection(),
        ExpressionProjection::EntityReference(entity) if entity.value().is_err()
    ));

    let malformed_lifetime = expression_events("'line..focus");
    assert!(matches!(
        projection(
            &malformed_lifetime,
            SyntaxKind::LifetimePathExpression
        )
        .projection(),
        ExpressionProjection::LifetimePath(path)
            if path.has_recovery() && path.segments().len() == 2
    ));

    let missing_variant = expression_events(".");
    assert!(matches!(
        projection(&missing_variant, SyntaxKind::ShortVariantExpression).projection(),
        ExpressionProjection::ShortVariant(Err(SyntaxNameIssue::Missing))
    ));
}

#[test]
fn question_marker_is_not_reclassified_as_try() {
    let lifetime = expression_events("'line.focus?");
    assert!(!lifetime.iter().any(|event| matches!(
        event,
        SyntaxEvent::StartNode {
            kind: SyntaxKind::TryExpression,
            ..
        }
    )));

    let ordinary = expression_events("value?");
    assert!(!ordinary.iter().any(|event| matches!(
        event,
        SyntaxEvent::StartNode {
            kind: SyntaxKind::TryExpression,
            ..
        }
    )));
}

#[test]
fn pratt_e14_through_e17_projections_retain_slots_forms_and_components() {
    let index = expression_events("items[0]");
    let index = projection(&index, SyntaxKind::PostfixBracketExpression);
    assert!(matches!(
        index.projection(),
        ExpressionProjection::Index(index)
            if index.target() == SyntaxExpressionSlot::Authored
                && index.index() == SyntaxExpressionSlot::Authored
    ));
    assert_eq!(index.components().len(), 2);

    let missing_content = expression_events("items[]");
    assert!(matches!(
        projection(
            &missing_content,
            SyntaxKind::DialogueContentApplicationExpression
        )
        .projection(),
        ExpressionProjection::DialogueContentApplication(application)
            if matches!(application.content(), SyntaxDialogueContentProjection::Missing { .. })
    ));

    let ambiguous = expression_events("items[key]");
    assert!(matches!(
        projection(&ambiguous, SyntaxKind::PostfixBracketExpression).projection(),
        ExpressionProjection::PostfixBracket(SyntaxPostfixBracketProjection::Ambiguous { .. })
    ));

    let pipe = expression_events("left |> right");
    let pipe = projection(&pipe, SyntaxKind::PipeExpression);
    assert!(matches!(
        pipe.projection(),
        ExpressionProjection::Pipe([
            SyntaxExpressionSlot::Authored,
            SyntaxExpressionSlot::Authored,
        ])
    ));
    assert_eq!(pipe.components().len(), 3);

    assert!(matches!(
        projection(&expression_events("try value"), SyntaxKind::TryExpression).projection(),
        ExpressionProjection::Try {
            operand: SyntaxExpressionSlot::Authored,
        }
    ));
    assert!(matches!(
        projection(
            &expression_events("await value"),
            SyntaxKind::AwaitExpression
        )
        .projection(),
        ExpressionProjection::Await {
            operand: SyntaxExpressionSlot::Authored,
            branches: None,
        }
    ));
    let nested = expression_events("try await value");
    assert!(matches!(
        projection(&nested, SyntaxKind::TryExpression).projection(),
        ExpressionProjection::Try {
            operand: SyntaxExpressionSlot::Authored,
        }
    ));
    assert!(matches!(
        projection(&nested, SyntaxKind::AwaitExpression).projection(),
        ExpressionProjection::Await {
            operand: SyntaxExpressionSlot::Authored,
            branches: None,
        }
    ));
}

#[test]
fn postfix_bracket_candidates_classify_without_source_name_heuristics() {
    assert!(matches!(
        projection(
            &expression_events("items[0]"),
            SyntaxKind::PostfixBracketExpression
        )
        .projection(),
        ExpressionProjection::Index(_)
    ));
    assert!(matches!(
        projection(
            &expression_events("items[key]"),
            SyntaxKind::PostfixBracketExpression
        )
        .projection(),
        ExpressionProjection::PostfixBracket(SyntaxPostfixBracketProjection::Ambiguous { .. })
    ));
    assert!(matches!(
        projection(
            &expression_events("alice[こんにちは。]"),
            SyntaxKind::DialogueContentApplicationExpression
        )
        .projection(),
        ExpressionProjection::DialogueContentApplication(application)
            if matches!(application.content(), SyntaxDialogueContentProjection::Present(_))
    ));
    let invalid_events = expression_events("items[,]");
    let invalid = projection(&invalid_events, SyntaxKind::PostfixBracketExpression);
    assert!(
        matches!(
            invalid.projection(),
            ExpressionProjection::PostfixBracket(SyntaxPostfixBracketProjection::Invalid { .. })
        ),
        "{:?}",
        invalid.projection()
    );
}

#[test]
fn prefix_e23_e24_e26_projections_retain_semantics_components_and_recovery() {
    for (source, expected) in [
        ("& value", SyntaxBorrowKind::Shared),
        ("& mut value", SyntaxBorrowKind::Mutable),
    ] {
        let events = expression_events(source);
        let projection = projection(&events, SyntaxKind::BorrowExpression);
        assert!(matches!(
            projection.projection(),
            ExpressionProjection::Borrow {
                operand: SyntaxExpressionSlot::Authored,
                kind,
            } if *kind == expected
        ));
        assert_eq!(projection.components().len(), 2);
    }

    let mutable = expression_events("& mut value");
    let mutable = projection(&mutable, SyntaxKind::BorrowExpression);
    assert_eq!(
        mutable
            .components()
            .iter()
            .find(|component| component.role() == ExpressionComponentRole::Operator)
            .expect("mutable borrow operator component")
            .range(),
        SourceRange::new(0, 5)
    );

    assert!(matches!(
        projection(
            &expression_events("*value"),
            SyntaxKind::DereferenceExpression
        )
        .projection(),
        ExpressionProjection::Dereference {
            operand: SyntaxExpressionSlot::Authored,
        }
    ));
    for (source, expected) in [
        ("!value", SyntaxUnaryOperator::Not),
        ("-value", SyntaxUnaryOperator::Negate),
    ] {
        assert!(matches!(
            projection(&expression_events(source), SyntaxKind::UnaryExpression).projection(),
            ExpressionProjection::Unary {
                operand: SyntaxExpressionSlot::Authored,
                operator,
            } if *operator == expected
        ));
    }

    for (source, kind) in [
        ("&", SyntaxKind::BorrowExpression),
        ("*", SyntaxKind::DereferenceExpression),
        ("!", SyntaxKind::UnaryExpression),
        ("-", SyntaxKind::UnaryExpression),
    ] {
        assert!(matches!(
            projection(&expression_events(source), kind).projection(),
            ExpressionProjection::Borrow {
                operand: SyntaxExpressionSlot::Missing,
                ..
            } | ExpressionProjection::Dereference {
                operand: SyntaxExpressionSlot::Missing,
            } | ExpressionProjection::Unary {
                operand: SyntaxExpressionSlot::Missing,
                ..
            }
        ));
    }
}

#[test]
fn range_e19_projection_preserves_optional_endpoints_and_inclusive_marker() {
    for (source, has_start, has_end, inclusive) in [
        ("start..end", true, true, false),
        ("start..=end", true, true, true),
        ("..end", false, true, false),
        ("..=end", false, true, true),
        ("start..", true, false, false),
        ("start..=", true, false, true),
        ("..", false, false, false),
        ("..=", false, false, true),
    ] {
        let events = expression_events(source);
        let range = projection(&events, SyntaxKind::RangeExpression);
        assert!(matches!(
            range.projection(),
            ExpressionProjection::Range {
                start,
                end,
                inclusive: actual,
            } if start.is_some() == has_start
                && end.is_some() == has_end
                && *actual == inclusive
        ));
        assert_eq!(
            range
                .components()
                .iter()
                .any(|component| { component.role() == ExpressionComponentRole::RangeStart }),
            has_start,
            "{source}"
        );
        assert_eq!(
            range
                .components()
                .iter()
                .any(|component| { component.role() == ExpressionComponentRole::RangeEnd }),
            has_end,
            "{source}"
        );
        assert_eq!(
            range.components().iter().any(|component| {
                component.role() == ExpressionComponentRole::RangeInclusiveMarker
            }),
            inclusive,
            "{source}"
        );
    }

    let inclusive = expression_events("start..=end");
    let inclusive = projection(&inclusive, SyntaxKind::RangeExpression);
    assert_eq!(
        inclusive
            .components()
            .iter()
            .find(|component| { component.role() == ExpressionComponentRole::RangeInclusiveMarker })
            .expect("inclusive range marker")
            .range(),
        SourceRange::new(5, 8)
    );
}

#[test]
fn binary_e22_projection_uses_the_closed_operator_vocabulary() {
    for (operator, expected) in [
        ("=>", SyntaxBinaryOperator::Implies),
        ("||", SyntaxBinaryOperator::Or),
        ("&&", SyntaxBinaryOperator::And),
        ("in", SyntaxBinaryOperator::In),
        ("==", SyntaxBinaryOperator::Equal),
        ("!=", SyntaxBinaryOperator::NotEqual),
        (">=", SyntaxBinaryOperator::GreaterOrEqual),
        ("<=", SyntaxBinaryOperator::LessOrEqual),
        (">", SyntaxBinaryOperator::Greater),
        ("<", SyntaxBinaryOperator::Less),
        ("&", SyntaxBinaryOperator::Merge),
        ("+", SyntaxBinaryOperator::Add),
        ("-", SyntaxBinaryOperator::Subtract),
        ("*", SyntaxBinaryOperator::Multiply),
        ("/", SyntaxBinaryOperator::Divide),
        ("%", SyntaxBinaryOperator::Remainder),
    ] {
        let source = format!("left {operator} right");
        let events = expression_events(&source);
        let binary = projection(&events, SyntaxKind::BinaryExpression);
        assert!(matches!(
            binary.projection(),
            ExpressionProjection::Binary {
                left: SyntaxExpressionSlot::Authored,
                operator: actual,
                right: SyntaxExpressionSlot::Authored,
            } if *actual == expected
        ));
        assert_eq!(
            binary
                .components()
                .iter()
                .find(|component| component.role() == ExpressionComponentRole::Operator)
                .expect("binary operator component")
                .range()
                .as_range()
                .len(),
            operator.len(),
            "{source}"
        );
    }

    assert!(matches!(
        projection(&expression_events("left +"), SyntaxKind::BinaryExpression).projection(),
        ExpressionProjection::Binary {
            left: SyntaxExpressionSlot::Authored,
            operator: SyntaxBinaryOperator::Add,
            right: SyntaxExpressionSlot::Missing,
        }
    ));
    assert!(matches!(
        projection(&expression_events("+ right"), SyntaxKind::BinaryExpression).projection(),
        ExpressionProjection::Binary {
            left: SyntaxExpressionSlot::Missing,
            operator: SyntaxBinaryOperator::Add,
            right: SyntaxExpressionSlot::Authored,
        }
    ));
    assert!(binary_binding_power("??").is_none());
}

#[test]
fn records_e20_e21_project_authored_fields_and_exact_source_parts() {
    let events = expression_events("Point { x = value, y: , shorthand }");
    let record = projection(&events, SyntaxKind::RecordExpression);
    let ExpressionProjection::Record(fields) = record.projection() else {
        panic!("E20 projection");
    };
    assert!(matches!(
        &fields[0],
        SyntaxRecordField::Explicit {
            name: Ok(name),
            value: SyntaxExpressionSlot::Authored,
        } if name.as_str() == "x"
    ));
    assert!(matches!(
        &fields[1],
        SyntaxRecordField::Explicit {
            name: Ok(name),
            value: SyntaxExpressionSlot::Missing,
        } if name.as_str() == "y"
    ));
    assert!(matches!(
        &fields[2],
        SyntaxRecordField::Shorthand { name: Ok(name) }
            if name.as_str() == "shorthand"
    ));
    assert!(
        record
            .components()
            .iter()
            .any(|component| component.role() == ExpressionComponentRole::RecordPath)
    );
    for part in [
        ExpressionRecordFieldPart::Whole,
        ExpressionRecordFieldPart::Name,
        ExpressionRecordFieldPart::Colon,
        ExpressionRecordFieldPart::Value,
    ] {
        assert!(record.components().iter().any(|component| {
            component.role() == ExpressionComponentRole::RecordField { field: 1, part }
        }));
    }
    assert!(record.components().iter().all(|component| {
        component.role()
            != ExpressionComponentRole::RecordField {
                field: 2,
                part: ExpressionRecordFieldPart::Colon,
            }
    }));

    let literal_events = expression_events("{ first = value, second: }");
    assert!(matches!(
        projection(&literal_events, SyntaxKind::RecordLiteralExpression).projection(),
        ExpressionProjection::RecordLiteral(fields)
            if fields.len() == 2
                && matches!(
                    &fields[1],
                    SyntaxRecordField::Explicit {
                        value: SyntaxExpressionSlot::Missing,
                        ..
                    }
                )
    ));

    for source in [
        "{ assert ready; () }",
        "{ close resource; () }",
        "{ select stream; () }",
        "{ unsafe lifetime @unsafe.audit reason = \"bounded\" { value; }; () }",
    ] {
        assert!(
            matches!(
                projection(&expression_events(source), SyntaxKind::BlockExpression).projection(),
                ExpressionProjection::Block
            ),
            "{source}"
        );
    }
}

#[test]
fn e35_generic_error_projects_only_the_unclassified_recovery_source() {
    let standalone = expression_events(":");
    let standalone = projection(&standalone, SyntaxKind::ErrorExpression);
    assert!(matches!(
        standalone.projection(),
        ExpressionProjection::Error
    ));
    assert_eq!(standalone.components().len(), 1);
    assert_eq!(
        standalone.components()[0].role(),
        ExpressionComponentRole::Recovery
    );
    assert_eq!(standalone.components()[0].range(), SourceRange::new(0, 1));

    let wrapped = expression_events("value : bad");
    let wrapped = projection(&wrapped, SyntaxKind::ErrorExpression);
    assert!(matches!(wrapped.projection(), ExpressionProjection::Error));
    let recovery = wrapped
        .components()
        .iter()
        .find(|component| component.role() == ExpressionComponentRole::Recovery)
        .expect("E35 recovery component")
        .range();
    assert!(recovery.start() > 0);
    assert_eq!(recovery.end(), "value : bad".len());
}

#[test]
fn pratt_start_insertion_preserves_each_child_leaf_projection() {
    let events = expression_events("1 + 2");
    assert!(matches!(
        events.first(),
        Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::BinaryExpression,
            ..
        })
    ));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                SyntaxEvent::StartNode {
                    kind: SyntaxKind::LiteralExpression,
                    projection: PendingStartProjection::Expression(_),
                    ..
                }
            ))
            .count(),
        2
    );
}
