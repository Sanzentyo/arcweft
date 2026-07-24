use super::call_syntax::{
    ArgumentListSyntaxInit, CallArgumentSyntaxInit, CallSyntaxInvariantError,
    CallbackBlockSyntaxInit, CallbackParameterHeaderSyntaxInit, CallbackParameterSyntaxInit,
};
use super::{
    ArgumentListSyntax, ArgumentListTerminatorSyntax, BinaryOp, CallArg, CallArgumentFormSyntax,
    CallArgumentRecoverySyntax, CallExpr, CallRecoveryBoundarySyntax, CallRecoveryTokenKind,
    CallbackBlockCallSyntax, CallbackBlockSyntax, CallbackParameterTypeSyntax, ClosureExprSource,
    ClosureParam, DottedPath, ExplicitCallTypeApplicationSyntax, Expr, ExprParseScope, Lexer,
    MAX_CALL_ARGUMENTS, MAX_CALLBACK_PARAMETERS, MAX_EXPR_DIAGNOSTICS, MAX_NESTED_CALLS,
    ParenthesizedCallSyntax, ParenthesizedCalleeSyntax, ParsedExpr, PathMemberCalleeSyntax, Token,
    collect_expr_source_ranges, parse_expr, parse_expr_fragment_recovering_at,
    parse_expr_recovering_at,
};
use crate::ast::common::TextRange;
use crate::pattern::parse_pattern;
use crate::types::{TypeRef, TypeRefLexemeKind, TypeRefNodeStep, parse_type_ref};

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(start, end)
}

fn closed(close_start: usize) -> ArgumentListTerminatorSyntax {
    ArgumentListTerminatorSyntax::Closed {
        close_paren: range(close_start, close_start + 1),
    }
}

fn positional(start: usize, end: usize) -> CallArgumentSyntaxInit {
    CallArgumentSyntaxInit {
        range: range(start, end),
        value: range(start, end),
        form: CallArgumentFormSyntax::Positional,
        recovery: CallArgumentRecoverySyntax::Parsed,
    }
}

fn named(
    full: TextRange,
    name: TextRange,
    equals: TextRange,
    value: TextRange,
) -> CallArgumentSyntaxInit {
    CallArgumentSyntaxInit {
        range: full,
        value,
        form: CallArgumentFormSyntax::Named { name, equals },
        recovery: CallArgumentRecoverySyntax::Parsed,
    }
}

fn parenthesized(
    source: &str,
    callee: TextRange,
    init: ArgumentListSyntaxInit,
) -> Result<ParenthesizedCallSyntax, CallSyntaxInvariantError> {
    let arguments = ArgumentListSyntax::try_from_parser(source, 0, init)?;
    ParenthesizedCallSyntax::try_from_parser(ParenthesizedCalleeSyntax::ordinary(callee), arguments)
}

fn parsed_call(source: &str) -> CallExpr {
    match parse_expr(source).expect("fixture must parse") {
        Expr::Call(call) => call,
        other => panic!("expected call expression, found {other:?}"),
    }
}

#[test]
fn selected_generic_member_uses_typed_token_transaction() {
    let call = parsed_call("visible_choices.collect<Vec<ChoiceView>>()");
    let Expr::Select(selected) = call.callee() else {
        panic!("expected selected generic member callee");
    };
    assert_eq!(selected.member().as_str(), "collect");
    let application = call
        .explicit_type_application()
        .expect("generic member retains typed application syntax");
    assert!(!application.is_turbofish());
    assert!(matches!(
        application.arguments(),
        [TypeRef::Generic { base, args }]
            if base.canonical_string() == "Vec"
                && matches!(args.as_slice(), [TypeRef::Path(path)] if path.canonical_string() == "ChoiceView")
    ));
    assert_eq!(application.range(), range(16, 40));
    assert_eq!(call.callee_range(), range(0, 40));

    let comparison = parse_expr("visible_choices.collect < limit")
        .expect("selector followed by comparison remains expression grammar");
    assert!(matches!(comparison, Expr::Binary { .. }));

    let chained = parse_expr("state.route_override.context(\"missing\")")
        .expect("ordinary selected value chain remains expression grammar");
    let Expr::Call(chained) = chained else {
        panic!("expected selected value call");
    };
    assert!(matches!(
        chained.callee(),
        Expr::Select(selected) if selected.member().as_str() == "context"
    ));

    let fixture_chain = parsed_call("xs.map(|x| x + 1i64).collect<Vec<i64>>()");
    assert!(matches!(
        fixture_chain.callee(),
        Expr::Select(selected) if selected.member().as_str() == "collect"
    ));
    assert!(matches!(
        fixture_chain
            .explicit_type_application()
            .map(ExplicitCallTypeApplicationSyntax::arguments),
        Some([TypeRef::Generic { base, args }])
            if base.canonical_string() == "Vec"
                && matches!(args.as_slice(), [TypeRef::Path(path)]
                    if path.canonical_string() == "i64")
    ));

    let turbofish = parsed_call("items.map::<Result<T, E>>(value)");
    assert!(matches!(
        turbofish.callee(),
        Expr::Select(selected) if selected.member().as_str() == "map"
    ));
    assert!(
        turbofish
            .explicit_type_application()
            .is_some_and(ExplicitCallTypeApplicationSyntax::is_turbofish)
    );
}

fn path_member(call: &CallExpr) -> &PathMemberCalleeSyntax {
    call.path_member_callee_syntax()
        .expect("fixture has a typed path-member callee")
}

fn assert_builtin_associated_dot_ranges(source: &str, receiver: &str, argument: TextRange) {
    let call = parsed_call(source);
    let callee = path_member(&call);
    assert!(matches!(
        callee.receiver().value(),
        TypeRef::Path(path) if path.canonical_string() == receiver
    ));
    assert_eq!(
        callee.receiver().root_source().whole(),
        &range(0, receiver.len())
    );
    assert_eq!(
        callee.separator().range(),
        range(receiver.len(), receiver.len() + 1)
    );
    assert_eq!(callee.member().as_str(), "with_capacity");
    assert_eq!(
        callee.member_range(),
        range(receiver.len() + 1, receiver.len() + 14)
    );
    assert_eq!(callee.range(), range(0, receiver.len() + 14));
    let arguments = call
        .parenthesized_syntax()
        .expect("fixture is parenthesized")
        .argument_list();
    assert_eq!(
        arguments.open_paren(),
        range(receiver.len() + 14, receiver.len() + 15)
    );
    assert_eq!(
        arguments.close_paren(),
        Some(range(source.len() - 1, source.len()))
    );
    assert_eq!(arguments.range(), range(receiver.len() + 14, source.len()));
    assert_eq!(arguments.arguments()[0].value_range(), argument);
    assert_eq!(call.range(), range(0, source.len()));
}

fn assert_type_lexeme(
    receiver: &crate::types::AuthoredTypeRef,
    kind: TypeRefLexemeKind,
    expected: TextRange,
) {
    assert!(
        receiver
            .source()
            .lexemes()
            .iter()
            .any(|lexeme| { lexeme.kind() == &kind && lexeme.range() == &expected }),
        "missing {kind:?} at {expected:?}"
    );
}

fn assert_type_node(
    receiver: &crate::types::AuthoredTypeRef,
    owner: &[TypeRefNodeStep],
    expected: TextRange,
) {
    let matches = receiver
        .source()
        .nodes()
        .iter()
        .filter(|(path, source)| path.steps() == owner && source.whole() == &expected)
        .count();
    assert_eq!(
        matches, 1,
        "expected exactly one type node {owner:?} at {expected:?}"
    );
}

fn assert_owned_type_lexeme(
    receiver: &crate::types::AuthoredTypeRef,
    owner: &[TypeRefNodeStep],
    kind: TypeRefLexemeKind,
    expected: TextRange,
) {
    let matches = receiver
        .source()
        .lexemes()
        .iter()
        .filter(|lexeme| {
            lexeme.owner().steps() == owner && lexeme.kind() == &kind && lexeme.range() == &expected
        })
        .count();
    assert_eq!(
        matches, 1,
        "expected exactly one {kind:?} owned by {owner:?} at {expected:?}"
    );
}

fn relative_range(source: TextRange, base: usize) -> (usize, usize) {
    (
        source
            .start()
            .checked_sub(base)
            .expect("range starts at base"),
        source
            .end()
            .checked_sub(base)
            .expect("range ends after base"),
    )
}

fn assert_relative_argument_form(
    associated: &CallArgumentFormSyntax,
    associated_base: usize,
    ordinary: &CallArgumentFormSyntax,
    ordinary_base: usize,
) {
    match (associated, ordinary) {
        (CallArgumentFormSyntax::Positional, CallArgumentFormSyntax::Positional) => {}
        (
            CallArgumentFormSyntax::Named {
                name: associated_name,
                equals: associated_equals,
            },
            CallArgumentFormSyntax::Named {
                name: ordinary_name,
                equals: ordinary_equals,
            },
        ) => {
            assert_eq!(
                relative_range(*associated_name, associated_base),
                relative_range(*ordinary_name, ordinary_base)
            );
            assert_eq!(
                relative_range(*associated_equals, associated_base),
                relative_range(*ordinary_equals, ordinary_base)
            );
        }
        (
            CallArgumentFormSyntax::Spread {
                ellipsis: associated_ellipsis,
            },
            CallArgumentFormSyntax::Spread {
                ellipsis: ordinary_ellipsis,
            },
        ) => assert_eq!(
            relative_range(*associated_ellipsis, associated_base),
            relative_range(*ordinary_ellipsis, ordinary_base)
        ),
        (associated, ordinary) => {
            panic!("argument form changed: {associated:?} != {ordinary:?}")
        }
    }
}

fn assert_relative_recovery_boundary(
    associated: CallRecoveryBoundarySyntax,
    associated_base: usize,
    ordinary: CallRecoveryBoundarySyntax,
    ordinary_base: usize,
) {
    match (associated, ordinary) {
        (
            CallRecoveryBoundarySyntax::EndOfExpression,
            CallRecoveryBoundarySyntax::EndOfExpression,
        ) => {}
        (
            CallRecoveryBoundarySyntax::Token {
                kind: associated_kind,
                range: associated_range,
            },
            CallRecoveryBoundarySyntax::Token {
                kind: ordinary_kind,
                range: ordinary_range,
            },
        ) => {
            assert_eq!(associated_kind, ordinary_kind);
            assert_eq!(
                relative_range(associated_range, associated_base),
                relative_range(ordinary_range, ordinary_base)
            );
        }
        (associated, ordinary) => {
            panic!("recovery boundary changed: {associated:?} != {ordinary:?}")
        }
    }
}

fn assert_relative_terminator(
    associated: &ArgumentListTerminatorSyntax,
    associated_base: usize,
    ordinary: &ArgumentListTerminatorSyntax,
    ordinary_base: usize,
) {
    match (associated, ordinary) {
        (
            ArgumentListTerminatorSyntax::Closed {
                close_paren: associated_close,
            },
            ArgumentListTerminatorSyntax::Closed {
                close_paren: ordinary_close,
            },
        ) => assert_eq!(
            relative_range(*associated_close, associated_base),
            relative_range(*ordinary_close, ordinary_base)
        ),
        (
            ArgumentListTerminatorSyntax::RecoveredMissing {
                insertion: associated_insertion,
                boundary: associated_boundary,
            },
            ArgumentListTerminatorSyntax::RecoveredMissing {
                insertion: ordinary_insertion,
                boundary: ordinary_boundary,
            },
        ) => {
            assert_eq!(
                associated_insertion - associated_base,
                ordinary_insertion - ordinary_base
            );
            assert_relative_recovery_boundary(
                *associated_boundary,
                associated_base,
                *ordinary_boundary,
                ordinary_base,
            );
        }
        (associated, ordinary) => {
            panic!("argument terminator changed: {associated:?} != {ordinary:?}")
        }
    }
}

fn assert_relative_argument_surface(
    associated: &ArgumentListSyntax,
    associated_base: usize,
    ordinary: &ArgumentListSyntax,
    ordinary_base: usize,
    surface_len: usize,
) {
    assert_eq!(
        relative_range(associated.range(), associated_base),
        relative_range(ordinary.range(), ordinary_base)
    );
    assert_eq!(
        relative_range(associated.open_paren(), associated_base),
        relative_range(ordinary.open_paren(), ordinary_base)
    );
    assert_eq!(associated.arguments().len(), ordinary.arguments().len());
    for (associated, ordinary) in associated.arguments().iter().zip(ordinary.arguments()) {
        assert_eq!(
            relative_range(associated.range(), associated_base),
            relative_range(ordinary.range(), ordinary_base)
        );
        assert_eq!(
            relative_range(associated.value_range(), associated_base),
            relative_range(ordinary.value_range(), ordinary_base)
        );
        assert_eq!(associated.recovery(), ordinary.recovery());
        assert_relative_argument_form(
            associated.form(),
            associated_base,
            ordinary.form(),
            ordinary_base,
        );
    }
    assert_eq!(
        associated
            .separators()
            .iter()
            .map(|range| relative_range(*range, associated_base))
            .collect::<Vec<_>>(),
        ordinary
            .separators()
            .iter()
            .map(|range| relative_range(*range, ordinary_base))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        associated
            .trailing_comma()
            .map(|range| relative_range(range, associated_base)),
        ordinary
            .trailing_comma()
            .map(|range| relative_range(range, ordinary_base))
    );
    assert_relative_terminator(
        associated.terminator(),
        associated_base,
        ordinary.terminator(),
        ordinary_base,
    );
    for offset in 0..=surface_len {
        assert_eq!(
            associated.active_argument_slot(associated_base + offset),
            ordinary.active_argument_slot(ordinary_base + offset),
            "active argument changed at relative offset {offset}"
        );
    }
}

fn recovered_call(source: &str) -> (CallExpr, ParsedExpr) {
    let parsed = parse_expr_recovering_at(
        source,
        ExprParseScope {
            source_range: range(0, source.len()),
            end_boundary: CallRecoveryBoundarySyntax::EndOfExpression,
        },
    )
    .expect("fixture must recover");
    let Expr::Call(call) = parsed.expr.clone() else {
        panic!("expected recovered call expression");
    };
    (call, parsed)
}

fn parsed_callback(source: &str) -> CallExpr {
    let call = parsed_call(source);
    assert!(
        call.callback_block_syntax().is_some(),
        "fixture must produce callback-block syntax"
    );
    call
}

#[test]
fn parenthesized_empty_call_has_exact_ranges() {
    let call = parsed_call("f()");
    let list = call
        .parenthesized_syntax()
        .expect("parenthesized surface")
        .argument_list();

    assert_eq!(call.range(), range(0, 3));
    assert_eq!(call.callee_range(), range(0, 1));
    assert_eq!(list.open_paren(), range(1, 2));
    assert_eq!(list.content_range(), range(2, 2));
    assert_eq!(list.close_paren(), Some(range(2, 3)));
    assert!(list.arguments().is_empty());
    assert!(list.separators().is_empty());
    assert_eq!(list.trailing_comma(), None);
}

#[test]
fn parenthesized_positional_utf8_ranges_are_bytes() {
    let call = parsed_call("f(α, \"猫\")");
    let list = call
        .parenthesized_syntax()
        .expect("parenthesized surface")
        .argument_list();

    assert_eq!(call.range(), range(0, 12));
    assert_eq!(call.callee_range(), range(0, 1));
    assert_eq!(list.open_paren(), range(1, 2));
    assert_eq!(list.arguments()[0].range(), range(2, 4));
    assert_eq!(list.arguments()[0].value_range(), range(2, 4));
    assert_eq!(list.separators(), &[range(4, 5)]);
    assert_eq!(list.arguments()[1].range(), range(6, 11));
    assert_eq!(list.arguments()[1].value_range(), range(6, 11));
    assert_eq!(list.close_paren(), Some(range(11, 12)));
}

#[test]
fn parenthesized_named_argument_ranges_exclude_trivia() {
    let call = parsed_call("paint(look = .smile)");
    let list = call
        .parenthesized_syntax()
        .expect("parenthesized surface")
        .argument_list();
    let argument = &list.arguments()[0];

    assert_eq!(call.range(), range(0, 20));
    assert_eq!(call.callee_range(), range(0, 5));
    assert_eq!(list.open_paren(), range(5, 6));
    assert_eq!(argument.range(), range(6, 19));
    assert_eq!(argument.value_range(), range(13, 19));
    assert!(matches!(
        argument.form(),
        CallArgumentFormSyntax::Named { name, equals }
            if *name == range(6, 10) && *equals == range(11, 12)
    ));
    assert_eq!(list.close_paren(), Some(range(19, 20)));
}

#[test]
fn parenthesized_postfix_spread_has_exact_ellipsis() {
    let call = parsed_call("log(fields...)");
    let list = call
        .parenthesized_syntax()
        .expect("parenthesized surface")
        .argument_list();
    let argument = &list.arguments()[0];

    assert_eq!(call.range(), range(0, 14));
    assert_eq!(call.callee_range(), range(0, 3));
    assert_eq!(list.open_paren(), range(3, 4));
    assert_eq!(argument.range(), range(4, 13));
    assert_eq!(argument.value_range(), range(4, 10));
    assert!(matches!(
        argument.form(),
        CallArgumentFormSyntax::Spread { ellipsis } if *ellipsis == range(10, 13)
    ));
    assert_eq!(list.close_paren(), Some(range(13, 14)));
}

#[test]
fn parenthesized_trailing_comma_is_not_separator() {
    let call = parsed_call("f(α,)");
    let list = call
        .parenthesized_syntax()
        .expect("parenthesized surface")
        .argument_list();

    assert_eq!(call.range(), range(0, 6));
    assert_eq!(list.open_paren(), range(1, 2));
    assert_eq!(list.arguments()[0].range(), range(2, 4));
    assert!(list.separators().is_empty());
    assert_eq!(list.trailing_comma(), Some(range(4, 5)));
    assert_eq!(list.close_paren(), Some(range(5, 6)));
}

#[test]
fn nested_parenthesized_calls_keep_independent_ranges() {
    let outer = parsed_call("outer(inner(猫), β)");
    let outer_list = outer
        .parenthesized_syntax()
        .expect("outer parenthesized surface")
        .argument_list();
    let Expr::Call(inner) = outer.args()[0].value() else {
        panic!("first outer argument must be the nested call");
    };
    let inner_list = inner
        .parenthesized_syntax()
        .expect("inner parenthesized surface")
        .argument_list();

    assert_eq!(outer.range(), range(0, 21));
    assert_eq!(outer.callee_range(), range(0, 5));
    assert_eq!(outer_list.open_paren(), range(5, 6));
    assert_eq!(outer_list.arguments()[0].range(), range(6, 16));
    assert_eq!(outer_list.separators(), &[range(16, 17)]);
    assert_eq!(outer_list.arguments()[1].range(), range(18, 20));
    assert_eq!(outer_list.close_paren(), Some(range(20, 21)));

    assert_eq!(inner.range(), range(6, 16));
    assert_eq!(inner.callee_range(), range(6, 11));
    assert_eq!(inner_list.open_paren(), range(11, 12));
    assert_eq!(inner_list.arguments()[0].range(), range(12, 15));
    assert_eq!(inner_list.close_paren(), Some(range(15, 16)));
}

#[test]
fn signature_cursor_selects_innermost_parenthesized_list() {
    let source = "outer(inner(猫), β)";
    let expr = parse_expr(source).expect("nested call fixture");
    let calls = collect_expr_source_ranges(&expr, source, range(0, source.len()))
        .into_iter()
        .filter_map(|entry| match entry.expr() {
            Expr::Call(call) if call.parenthesized_syntax().is_some() => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    let selected = calls
        .iter()
        .filter(|call| {
            call.parenthesized_syntax()
                .is_some_and(|syntax| syntax.argument_list().contains_signature_cursor(13))
        })
        .min_by_key(|call| call.range().end() - call.range().start())
        .expect("one innermost call");

    assert_eq!(selected.range(), range(6, 16));
}

#[test]
fn missing_close_retains_parenthesized_call_at_owner_end() {
    let source = "f(α, β";
    let (call, parsed) = recovered_call(source);
    let list = call
        .parenthesized_syntax()
        .expect("recovered parenthesized surface")
        .argument_list();

    assert_eq!(call.range(), range(0, 8));
    assert_eq!(call.callee_range(), range(0, 1));
    assert_eq!(list.open_paren(), range(1, 2));
    assert_eq!(list.arguments()[0].range(), range(2, 4));
    assert_eq!(list.separators(), &[range(4, 5)]);
    assert_eq!(list.arguments()[1].range(), range(6, 8));
    assert_eq!(list.close_paren(), None);
    assert_eq!(
        list.terminator(),
        &ArgumentListTerminatorSyntax::RecoveredMissing {
            insertion: 8,
            boundary: CallRecoveryBoundarySyntax::EndOfExpression,
        }
    );
    assert_eq!(parsed.diagnostics.len(), 1);
    assert_eq!(
        parsed.diagnostics[0].code(),
        "syntax.expr.missing_call_close"
    );
    assert_eq!(parsed.diagnostics[0].range(), range(8, 8));
    assert_eq!(parsed.diagnostics[0].related_ranges(), &[range(1, 2)]);

    let strict = parse_expr(source).expect_err("strict parsing rejects recovered syntax");
    assert_eq!(strict.code(), "syntax.expr.missing_call_close");
}

#[test]
fn missing_close_after_open_paren_recovers_an_empty_list() {
    let source = "f(";
    let (call, parsed) = recovered_call(source);
    let list = call
        .parenthesized_syntax()
        .expect("recovered parenthesized surface")
        .argument_list();

    assert!(call.args().is_empty());
    assert!(list.arguments().is_empty());
    assert!(list.separators().is_empty());
    assert_eq!(
        list.terminator(),
        &ArgumentListTerminatorSyntax::RecoveredMissing {
            insertion: 2,
            boundary: CallRecoveryBoundarySyntax::EndOfExpression,
        }
    );
    assert_eq!(parsed.diagnostics.len(), 1);
    assert_eq!(
        parse_expr(source)
            .expect_err("strict parser rejects recovery")
            .code(),
        "syntax.expr.missing_call_close"
    );
}

fn assert_missing_close_before_owner_token(
    source: &str,
    source_range: TextRange,
    kind: CallRecoveryTokenKind,
    boundary: TextRange,
) {
    let parsed = parse_expr_recovering_at(
        source,
        ExprParseScope {
            source_range,
            end_boundary: CallRecoveryBoundarySyntax::Token {
                kind,
                range: boundary,
            },
        },
    )
    .expect("owner-scoped expression must recover");
    let Expr::Call(call) = parsed.expr else {
        panic!("owner-scoped expression must remain a typed call");
    };
    let list = call
        .parenthesized_syntax()
        .expect("recovered parenthesized surface")
        .argument_list();

    assert_eq!(
        list.terminator(),
        &ArgumentListTerminatorSyntax::RecoveredMissing {
            insertion: boundary.start(),
            boundary: CallRecoveryBoundarySyntax::Token {
                kind,
                range: boundary,
            },
        }
    );
    assert_eq!(list.close_paren(), None);
    assert_eq!(
        source.get(boundary.as_range()),
        Some(match kind {
            CallRecoveryTokenKind::Semicolon => ";",
            CallRecoveryTokenKind::Colon => ":",
            CallRecoveryTokenKind::CloseBracket => "]",
            CallRecoveryTokenKind::CloseBrace => "}",
            _ => panic!("fixture uses one of the required direct owner tokens"),
        })
    );
    assert_eq!(parsed.diagnostics.len(), 1);
    assert_eq!(
        parsed.diagnostics[0].code(),
        "syntax.expr.missing_call_close"
    );
    assert!(
        parse_expr(&source[source_range.as_range()]).is_err(),
        "the strict fragment API rejects the same recovered expression"
    );
}

#[test]
fn missing_close_stops_before_close_bracket_owner_token() {
    let source = "[f(α, β]";
    let parsed = parse_expr_recovering_at(
        source,
        ExprParseScope {
            source_range: range(0, source.len()),
            end_boundary: CallRecoveryBoundarySyntax::EndOfExpression,
        },
    );
    let parsed = parsed.expect("bracket owner must retain its closing delimiter");
    let Expr::BracketSeq(items) = parsed.expr else {
        panic!("outer bracket sequence must remain typed");
    };
    let [Expr::Call(call)] = items.as_slice() else {
        panic!("bracket item must remain a typed recovered call");
    };
    let list = call
        .parenthesized_syntax()
        .expect("recovered parenthesized surface")
        .argument_list();
    let boundary = range(source.len() - 1, source.len());

    assert_eq!(parsed.range, range(0, source.len()));
    assert_eq!(
        list.terminator(),
        &ArgumentListTerminatorSyntax::RecoveredMissing {
            insertion: boundary.start(),
            boundary: CallRecoveryBoundarySyntax::Token {
                kind: CallRecoveryTokenKind::CloseBracket,
                range: boundary,
            },
        }
    );
    assert_eq!(&source[boundary.as_range()], "]");
    assert_eq!(parsed.diagnostics.len(), 1);
}

#[test]
fn missing_close_stops_before_close_brace_owner_token() {
    let source = "Record { value: f(α, β }";
    let parsed = parse_expr_recovering_at(
        source,
        ExprParseScope {
            source_range: range(0, source.len()),
            end_boundary: CallRecoveryBoundarySyntax::EndOfExpression,
        },
    );
    let parsed = parsed.expect("record owner must retain its closing delimiter");
    let Expr::Record { fields, .. } = parsed.expr else {
        panic!("outer record must remain typed");
    };
    let [(_, Expr::Call(call))] = fields.as_slice() else {
        panic!("record value must remain a typed recovered call");
    };
    let list = call
        .parenthesized_syntax()
        .expect("recovered parenthesized surface")
        .argument_list();
    let boundary = range(source.len() - 1, source.len());

    assert_eq!(parsed.range, range(0, source.len()));
    assert_eq!(
        list.terminator(),
        &ArgumentListTerminatorSyntax::RecoveredMissing {
            insertion: boundary.start(),
            boundary: CallRecoveryBoundarySyntax::Token {
                kind: CallRecoveryTokenKind::CloseBrace,
                range: boundary,
            },
        }
    );
    assert_eq!(&source[boundary.as_range()], "}");
    assert_eq!(parsed.diagnostics.len(), 1);
}

#[test]
fn missing_close_stops_before_semicolon_owner_token() {
    assert_missing_close_before_owner_token(
        "f(α, β; next())",
        range(0, 8),
        CallRecoveryTokenKind::Semicolon,
        range(8, 9),
    );
}

#[test]
fn missing_close_stops_before_speaker_colon_owner_token() {
    assert_missing_close_before_owner_token(
        "alice(look = .smile: hello",
        range(0, 19),
        CallRecoveryTokenKind::Colon,
        range(19, 20),
    );
}

#[test]
fn nested_closed_call_keeps_its_close_when_outer_call_is_missing_close() {
    let source = "outer(inner(1, 2)";
    let (outer, parsed) = recovered_call(source);
    let outer_list = outer
        .parenthesized_syntax()
        .expect("outer parenthesized surface")
        .argument_list();
    let Expr::Call(inner) = outer.args()[0].value() else {
        panic!("outer argument must remain the closed inner call");
    };
    let inner_list = inner
        .parenthesized_syntax()
        .expect("inner parenthesized surface")
        .argument_list();

    assert_eq!(inner_list.close_paren(), Some(range(16, 17)));
    assert_eq!(inner.range(), range(6, 17));
    assert_eq!(
        outer_list.terminator(),
        &ArgumentListTerminatorSyntax::RecoveredMissing {
            insertion: 17,
            boundary: CallRecoveryBoundarySyntax::EndOfExpression,
        }
    );
    assert_eq!(parsed.diagnostics.len(), 1);
}

#[test]
fn malformed_middle_argument_recovers_one_exact_slot() {
    let source = "f(α, @@@, β)";
    let (call, parsed) = recovered_call(source);
    let list = call
        .parenthesized_syntax()
        .expect("parenthesized surface")
        .argument_list();

    assert_eq!(call.range(), range(0, 14));
    assert_eq!(list.arguments().len(), 3);
    assert_eq!(list.arguments()[0].range(), range(2, 4));
    assert_eq!(list.separators(), &[range(4, 5), range(9, 10)]);
    assert_eq!(list.arguments()[1].range(), range(6, 9));
    assert_eq!(list.arguments()[1].value_range(), range(6, 9));
    assert_eq!(
        list.arguments()[1].recovery(),
        CallArgumentRecoverySyntax::Recovered {
            diagnostic: range(6, 9),
        }
    );
    assert!(matches!(call.args()[1].value(), Expr::Raw(raw) if raw == "@@@"));
    assert_eq!(list.arguments()[2].range(), range(11, 13));
    assert_eq!(list.close_paren(), Some(range(13, 14)));
    assert_eq!(parsed.diagnostics.len(), 1);
    assert_eq!(
        parsed.diagnostics[0].code(),
        "syntax.expr.recovered_call_argument"
    );
    assert_eq!(parsed.diagnostics[0].range(), range(6, 9));
    assert!(parse_expr(source).is_err());
}

#[test]
fn malformed_named_value_preserves_named_form() {
    let source = "f(look = @@@, stage = main)";
    let (call, parsed) = recovered_call(source);
    let list = call
        .parenthesized_syntax()
        .expect("parenthesized surface")
        .argument_list();

    assert_eq!(list.arguments().len(), 2);
    assert_eq!(list.arguments()[0].range(), range(2, 12));
    assert_eq!(list.arguments()[0].value_range(), range(9, 12));
    assert!(matches!(
        list.arguments()[0].form(),
        CallArgumentFormSyntax::Named { name, equals }
            if *name == range(2, 6) && *equals == range(7, 8)
    ));
    assert!(matches!(
        list.arguments()[0].recovery(),
        CallArgumentRecoverySyntax::Recovered { diagnostic }
            if diagnostic == range(9, 12)
    ));
    assert!(matches!(call.args()[0].value(), Expr::Raw(raw) if raw == "@@@"));
    assert!(matches!(
        list.arguments()[1].form(),
        CallArgumentFormSyntax::Named { name, equals }
            if *name == range(14, 19) && *equals == range(20, 21)
    ));
    assert_eq!(list.arguments()[1].value_range(), range(22, 26));
    assert_eq!(parsed.diagnostics.len(), 1);
}

#[test]
fn call_rejects_empty_consecutive_unseparated_and_incomplete_arguments() {
    for source in ["f(,x)", "f(x,,y)", "f(x y)", "f(name =)", "f(...)"] {
        let error = parse_expr(source).expect_err("invalid call must not publish a CallExpr");
        assert!(
            error.code().starts_with("syntax.expr."),
            "{source} returned an unstructured expression error"
        );
    }
}

#[test]
fn callback_block_zero_params_has_exact_braces_and_body() {
    let call = parsed_callback("items.tap { emit() }");
    let callback = call
        .callback_block_syntax()
        .expect("callback-block surface")
        .callback();

    assert_eq!(call.range(), range(0, 20));
    assert_eq!(call.callee_range(), range(0, 9));
    assert_eq!(callback.open_brace(), range(10, 11));
    assert!(callback.parameters().is_implicit_zero());
    assert_eq!(callback.body_range(), range(12, 18));
    assert_eq!(callback.close_brace(), range(19, 20));
    assert_eq!(callback.closure_range(), range(10, 20));
    assert_eq!(call.args().len(), 1);
    let Expr::Closure { body, .. } = call.args()[0].value() else {
        panic!("callback call must carry exactly one closure argument");
    };
    let Expr::Block {
        statements,
        value: Some(value),
    } = body.as_ref()
    else {
        panic!("callback braces must retain a semantic block body");
    };
    assert!(statements.is_empty());
    let Expr::Call(nested) = value.as_ref() else {
        panic!("callback block value must retain nested emit call");
    };
    let nested_list = nested
        .parenthesized_syntax()
        .expect("nested parenthesized surface")
        .argument_list();
    assert_eq!(nested.callee_range(), range(12, 16));
    assert_eq!(nested_list.open_paren(), range(16, 17));
    assert_eq!(nested_list.close_paren(), Some(range(17, 18)));
}

#[test]
fn callback_block_crlf_body_retains_its_value_expression() {
    let call = parsed_callback("items.tap {\r\nx\r\n}");
    let Expr::Closure { body, .. } = call.args()[0].value() else {
        panic!("callback call must carry exactly one closure argument");
    };
    let Expr::Block {
        statements,
        value: Some(value),
    } = body.as_ref()
    else {
        panic!("callback braces must retain a semantic block body");
    };

    assert!(statements.is_empty());
    assert!(matches!(value.as_ref(), Expr::Path(path) if path.as_label() == "x"));
}

#[test]
fn callback_block_one_param_has_exact_header() {
    let call = parsed_callback("items.map { item => item.label }");
    let callback = call
        .callback_block_syntax()
        .expect("callback-block surface")
        .callback();
    let header = callback.parameters();

    assert_eq!(call.range(), range(0, 32));
    assert_eq!(call.callee_range(), range(0, 9));
    assert_eq!(callback.open_brace(), range(10, 11));
    assert_eq!(header.parameters().len(), 1);
    assert_eq!(header.parameters()[0].range(), range(12, 16));
    assert_eq!(header.parameters()[0].pattern_range(), range(12, 16));
    assert!(header.separators().is_empty());
    assert_eq!(header.fat_arrow(), Some(range(17, 19)));
    assert_eq!(callback.body_range(), range(20, 30));
    assert_eq!(callback.close_brace(), range(31, 32));
}

#[test]
fn callback_block_multiple_params_and_nested_call_keep_ranges() {
    let call = parsed_callback("items.zip { item, index => item.label(index) }");
    let callback = call
        .callback_block_syntax()
        .expect("callback-block surface")
        .callback();
    let header = callback.parameters();

    assert_eq!(call.range(), range(0, 46));
    assert_eq!(call.callee_range(), range(0, 9));
    assert_eq!(callback.open_brace(), range(10, 11));
    assert_eq!(header.parameters()[0].range(), range(12, 16));
    assert_eq!(header.separators(), &[range(16, 17)]);
    assert_eq!(header.parameters()[1].range(), range(18, 23));
    assert_eq!(header.fat_arrow(), Some(range(24, 26)));
    assert_eq!(callback.body_range(), range(27, 44));
    assert_eq!(callback.close_brace(), range(45, 46));

    let Expr::Closure { body, .. } = call.args()[0].value() else {
        panic!("callback call must carry a closure");
    };
    let Expr::Block {
        statements,
        value: Some(value),
    } = body.as_ref()
    else {
        panic!("callback braces must retain a semantic block body");
    };
    assert!(statements.is_empty());
    let Expr::Call(nested) = value.as_ref() else {
        panic!("callback block value must retain nested call");
    };
    let nested_list = nested
        .parenthesized_syntax()
        .expect("nested parenthesized surface")
        .argument_list();
    assert_eq!(nested.range(), range(27, 44));
    assert_eq!(nested.callee_range(), range(27, 37));
    assert_eq!(nested_list.open_paren(), range(37, 38));
    assert_eq!(nested_list.arguments()[0].range(), range(38, 43));
    assert_eq!(nested_list.close_paren(), Some(range(43, 44)));
}

#[test]
fn return_typed_closure_body_call_keeps_owner_absolute_ranges() {
    let expression = "wrap(prefix, |x| -> T {\n  nested(x)\n})";
    let owner = format!("let result = {expression};");
    let expression_start = owner.find(expression).expect("authored expression");
    let expression_end = expression_start + expression.len();
    let parsed = parse_expr_recovering_at(
        &owner,
        ExprParseScope {
            source_range: range(expression_start, expression_end),
            end_boundary: CallRecoveryBoundarySyntax::EndOfExpression,
        },
    )
    .expect("owner-scoped closure fixture must parse");
    assert!(parsed.diagnostics.is_empty());
    let Expr::Call(outer) = parsed.expr else {
        panic!("fixture must retain the outer call");
    };
    let Expr::Closure { body, .. } = outer.args()[1].value() else {
        panic!("second argument must remain a closure");
    };
    let Expr::Block {
        value: Some(value), ..
    } = body.as_ref()
    else {
        panic!("return-typed closure must retain its block value");
    };
    let Expr::Call(nested) = value.as_ref() else {
        panic!("closure block value must remain a typed call");
    };
    let nested_start = owner.find("nested").expect("nested call spelling");
    let nested_list = nested
        .parenthesized_syntax()
        .expect("nested parenthesized call")
        .argument_list();

    assert_eq!(outer.range(), range(expression_start, expression_end));
    assert_eq!(nested.range(), range(nested_start, nested_start + 9));
    assert_eq!(nested.callee_range(), range(nested_start, nested_start + 6));
    assert_eq!(
        nested_list.open_paren(),
        range(nested_start + 6, nested_start + 7)
    );
    assert_eq!(
        nested_list.arguments()[0].range(),
        range(nested_start + 7, nested_start + 8)
    );
    assert_eq!(
        nested_list.close_paren(),
        Some(range(nested_start + 8, nested_start + 9))
    );
}

#[test]
fn callback_block_typed_param_has_exact_type_ascription() {
    let call = parsed_callback("items.map { item: Label => item.text }");
    let callback = call
        .callback_block_syntax()
        .expect("callback-block surface")
        .callback();
    let header = callback.parameters();
    let parameter = &header.parameters()[0];
    let ty = parameter.type_ascription().expect("typed parameter");

    assert_eq!(call.range(), range(0, 38));
    assert_eq!(call.callee_range(), range(0, 9));
    assert_eq!(callback.open_brace(), range(10, 11));
    assert_eq!(parameter.range(), range(12, 23));
    assert_eq!(parameter.pattern_range(), range(12, 16));
    assert_eq!(ty.colon(), range(16, 17));
    assert_eq!(ty.ty_range(), range(18, 23));
    assert_eq!(header.fat_arrow(), Some(range(24, 26)));
    assert_eq!(callback.body_range(), range(27, 36));
    assert_eq!(callback.close_brace(), range(37, 38));
}

#[test]
fn selected_callback_after_parenthesized_call_keeps_both_surfaces() {
    let source =
        "Button(\"Send\").on_click {\n  let label = name.text\n  action.invoke(value = label)\n}";
    let outer = parsed_callback(source);
    let callback = outer
        .callback_block_syntax()
        .expect("callback-block surface")
        .callback();

    assert_eq!(outer.range(), range(0, 82));
    assert_eq!(outer.callee_range(), range(0, 23));
    assert_eq!(callback.open_brace(), range(24, 25));
    assert_eq!(callback.body_range(), range(28, 80));
    assert_eq!(callback.close_brace(), range(81, 82));

    let expr = Expr::Call(outer);
    let call_ranges = collect_expr_source_ranges(&expr, source, range(0, source.len()))
        .into_iter()
        .filter_map(|entry| match entry.expr() {
            Expr::Call(call) => Some(call.range()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(call_ranges.contains(&range(0, 14)));
    assert!(call_ranges.contains(&range(52, 80)));
    assert!(call_ranges.contains(&range(0, 82)));
}

#[test]
fn callback_multistatement_body_range_excludes_brace_trivia() {
    let source =
        "Button(\"Send\").on_click {\n  let label = name.text\n  action.invoke(value = label)\n}";
    let call = parsed_callback(source);
    let callback = call
        .callback_block_syntax()
        .expect("callback-block surface")
        .callback();
    assert_eq!(callback.body_range(), range(28, 80));
    assert_eq!(
        &source[callback.body_range().as_range()],
        "let label = name.text\n  action.invoke(value = label)"
    );
}

#[test]
fn callback_rejects_malformed_headers_bodies_and_unclosed_braces() {
    for source in [
        "items.map { => item }",
        "items.map { item => }",
        "items.map { item, => item }",
        "items.map { item: => item }",
        "items.map { item => item",
        "items.map { }",
    ] {
        assert!(
            parse_expr(source).is_err(),
            "{source} must not produce a callback-call surface"
        );
    }
}

#[test]
fn associated_string_dot_ranges() {
    assert_builtin_associated_dot_ranges("String.with_capacity(64)", "String", range(21, 23));
}

#[test]
fn associated_bytes_dot_ranges() {
    assert_builtin_associated_dot_ranges("Bytes.with_capacity(4096)", "Bytes", range(20, 24));
}

#[test]
fn associated_bare_vec_retains_typed_path_candidate() {
    let call = parsed_call("Vec.with_capacity(8)");
    let receiver = path_member(&call).receiver();
    assert!(matches!(
        receiver.value(),
        TypeRef::Path(path) if path.canonical_string() == "Vec"
    ));
    assert!(receiver.source().nodes().iter().all(|(path, _)| {
        !path
            .steps()
            .iter()
            .any(|step| matches!(step, TypeRefNodeStep::GenericArgument(_)))
    }));
}

#[test]
fn associated_vec_generic_dot_ranges() {
    let call = parsed_call("Vec<I32>.with_capacity(8)");
    let callee = path_member(&call);
    let receiver = callee.receiver();
    assert!(matches!(
        receiver.value(),
        TypeRef::Generic { base, args }
            if base.canonical_string() == "Vec"
                && matches!(args.as_slice(), [TypeRef::Path(path)] if path.canonical_string() == "I32")
    ));
    assert_eq!(callee.receiver().root_source().whole(), &range(0, 8));
    assert_type_node(receiver, &[], range(0, 8));
    assert_type_node(
        receiver,
        &[TypeRefNodeStep::GenericArgument(0)],
        range(4, 7),
    );
    assert_owned_type_lexeme(
        receiver,
        &[],
        TypeRefLexemeKind::PathSegment { ordinal: 0 },
        range(0, 3),
    );
    assert_owned_type_lexeme(receiver, &[], TypeRefLexemeKind::OpenAngle, range(3, 4));
    assert_owned_type_lexeme(
        receiver,
        &[TypeRefNodeStep::GenericArgument(0)],
        TypeRefLexemeKind::PathSegment { ordinal: 0 },
        range(4, 7),
    );
    assert_owned_type_lexeme(receiver, &[], TypeRefLexemeKind::CloseAngle, range(7, 8));
    assert_eq!(callee.separator().range(), range(8, 9));
    assert_eq!(callee.member_range(), range(9, 22));
    assert_eq!(callee.range(), range(0, 22));
    let arguments = call
        .parenthesized_syntax()
        .expect("parenthesized call")
        .argument_list();
    assert_eq!(arguments.open_paren(), range(22, 23));
    assert_eq!(arguments.arguments()[0].range(), range(23, 24));
    assert_eq!(arguments.arguments()[0].value_range(), range(23, 24));
    assert_eq!(arguments.close_paren(), Some(range(24, 25)));
    assert_eq!(call.range(), range(0, 25));
}

#[test]
fn associated_vec_generic_parameter_dot_ranges() {
    let call = parsed_call("Vec<T>.with_capacity(8)");
    let receiver = path_member(&call).receiver();
    let (_, source) = receiver
        .source()
        .nodes()
        .iter()
        .find(|(path, _)| path.steps() == [TypeRefNodeStep::GenericArgument(0)])
        .expect("generic parameter has one structural source node");
    assert_eq!(source.whole(), &range(4, 5));
}

#[test]
fn associated_qualified_receiver_lexemes() {
    let call = parsed_call("pkg::types::Vec<I32>.with_capacity(8)");
    let callee = path_member(&call);
    let receiver = callee.receiver();
    assert_eq!(receiver.root_source().whole(), &range(0, 20));
    for (kind, expected) in [
        (TypeRefLexemeKind::PathSegment { ordinal: 0 }, range(0, 3)),
        (TypeRefLexemeKind::PathSeparator { before: 1 }, range(3, 5)),
        (TypeRefLexemeKind::PathSegment { ordinal: 1 }, range(5, 10)),
        (
            TypeRefLexemeKind::PathSeparator { before: 2 },
            range(10, 12),
        ),
        (TypeRefLexemeKind::PathSegment { ordinal: 2 }, range(12, 15)),
        (TypeRefLexemeKind::OpenAngle, range(15, 16)),
        (TypeRefLexemeKind::CloseAngle, range(19, 20)),
    ] {
        assert_type_lexeme(receiver, kind, expected);
    }
    assert_eq!(callee.separator().range(), range(20, 21));
    assert_eq!(callee.member_range(), range(21, 34));
}

#[test]
fn associated_alias_receiver_lexemes() {
    let call = parsed_call("Alias<I32>.with_capacity(8)");
    let callee = path_member(&call);
    let receiver = callee.receiver();
    assert!(matches!(
        receiver.value(),
        TypeRef::Generic { base, args }
            if base.canonical_string() == "Alias"
                && matches!(args.as_slice(), [TypeRef::Path(path)] if path.canonical_string() == "I32")
    ));
    assert_eq!(receiver.root_source().whole(), &range(0, 10));
    assert_type_node(receiver, &[], range(0, 10));
    assert_type_node(
        receiver,
        &[TypeRefNodeStep::GenericArgument(0)],
        range(6, 9),
    );
    assert_owned_type_lexeme(
        receiver,
        &[],
        TypeRefLexemeKind::PathSegment { ordinal: 0 },
        range(0, 5),
    );
    assert_owned_type_lexeme(receiver, &[], TypeRefLexemeKind::OpenAngle, range(5, 6));
    assert_owned_type_lexeme(
        receiver,
        &[TypeRefNodeStep::GenericArgument(0)],
        TypeRefLexemeKind::PathSegment { ordinal: 0 },
        range(6, 9),
    );
    assert_owned_type_lexeme(receiver, &[], TypeRefLexemeKind::CloseAngle, range(9, 10));
    assert_eq!(callee.separator().range(), range(10, 11));
    assert_eq!(callee.member_range(), range(11, 24));
}

#[test]
fn associated_nested_generic_lexeme_tree() {
    let source = "Vec<Option<Result<T,E>>>.with_capacity(8)";
    let call = parsed_call(source);
    let callee = path_member(&call);
    let TypeRef::Generic { args, .. } = callee.receiver().value() else {
        panic!("receiver must retain its generic type tree")
    };
    assert!(matches!(args.as_slice(), [TypeRef::Generic { .. }]));
    assert_eq!(callee.receiver().root_source().whole(), &range(0, 24));
    assert_eq!(callee.separator().range(), range(24, 25));
    assert_eq!(callee.member_range(), range(25, 38));
    assert_eq!(callee.range(), range(0, 38));

    let receiver = callee.receiver();
    let option = [TypeRefNodeStep::GenericArgument(0)];
    let result = [
        TypeRefNodeStep::GenericArgument(0),
        TypeRefNodeStep::GenericArgument(0),
    ];
    let result_t = [
        TypeRefNodeStep::GenericArgument(0),
        TypeRefNodeStep::GenericArgument(0),
        TypeRefNodeStep::GenericArgument(0),
    ];
    let result_e = [
        TypeRefNodeStep::GenericArgument(0),
        TypeRefNodeStep::GenericArgument(0),
        TypeRefNodeStep::GenericArgument(1),
    ];
    for (owner, expected) in [
        (&[][..], range(0, 24)),
        (&option[..], range(4, 23)),
        (&result[..], range(11, 22)),
        (&result_t[..], range(18, 19)),
        (&result_e[..], range(20, 21)),
    ] {
        assert_type_node(receiver, owner, expected);
    }
    for (owner, kind, expected) in [
        (
            &[][..],
            TypeRefLexemeKind::PathSegment { ordinal: 0 },
            range(0, 3),
        ),
        (&[][..], TypeRefLexemeKind::OpenAngle, range(3, 4)),
        (
            &option[..],
            TypeRefLexemeKind::PathSegment { ordinal: 0 },
            range(4, 10),
        ),
        (&option[..], TypeRefLexemeKind::OpenAngle, range(10, 11)),
        (
            &result[..],
            TypeRefLexemeKind::PathSegment { ordinal: 0 },
            range(11, 17),
        ),
        (&result[..], TypeRefLexemeKind::OpenAngle, range(17, 18)),
        (
            &result_t[..],
            TypeRefLexemeKind::PathSegment { ordinal: 0 },
            range(18, 19),
        ),
        (
            &result[..],
            TypeRefLexemeKind::ArgumentSeparator { before: 1 },
            range(19, 20),
        ),
        (
            &result_e[..],
            TypeRefLexemeKind::PathSegment { ordinal: 0 },
            range(20, 21),
        ),
        (&result[..], TypeRefLexemeKind::CloseAngle, range(21, 22)),
        (&option[..], TypeRefLexemeKind::CloseAngle, range(22, 23)),
        (&[][..], TypeRefLexemeKind::CloseAngle, range(23, 24)),
    ] {
        assert_owned_type_lexeme(receiver, owner, kind, expected);
    }
    assert_eq!(receiver.source().nodes().len(), 5);
    assert_eq!(receiver.source().lexemes().len(), 12);
}

#[test]
fn associated_existing_generic_path_ranges() {
    let call = parsed_call("Vec<I32>::with_capacity(8)");
    let callee = path_member(&call);
    assert_eq!(callee.receiver().root_source().whole(), &range(0, 8));
    assert!(matches!(
        callee.separator(),
        super::AssociatedMemberSeparatorSyntax::Path { range: separator }
            if separator == range(8, 10)
    ));
    assert_eq!(callee.member_range(), range(10, 23));
    assert_eq!(callee.range(), range(0, 23));
    let arguments = call
        .parenthesized_syntax()
        .expect("parenthesized call")
        .argument_list();
    assert_eq!(arguments.open_paren(), range(23, 24));
    assert_eq!(arguments.arguments()[0].range(), range(24, 25));
    assert_eq!(arguments.arguments()[0].value_range(), range(24, 25));
    assert_eq!(arguments.close_paren(), Some(range(25, 26)));
    assert_eq!(call.range(), range(0, 26));
}

#[test]
fn associated_generic_parameter_path_ranges() {
    let call = parsed_call("Vec<T>::with_capacity(8)");
    let callee = path_member(&call);
    let (_, source) = callee
        .receiver()
        .source()
        .nodes()
        .iter()
        .find(|(path, _)| path.steps() == [TypeRefNodeStep::GenericArgument(0)])
        .expect("generic parameter source");
    assert_eq!(source.whole(), &range(4, 5));
    assert_eq!(callee.separator().range(), range(6, 8));
    assert_eq!(callee.member_range(), range(8, 21));
}

#[test]
fn associated_turbofish_dot_ranges() {
    let call = parsed_call("Vec::<I32>.with_capacity(8)");
    let callee = path_member(&call);
    let receiver = callee.receiver();
    assert_eq!(receiver.root_source().whole(), &range(0, 10));
    assert_type_node(receiver, &[], range(0, 10));
    assert_type_node(
        receiver,
        &[TypeRefNodeStep::GenericArgument(0)],
        range(6, 9),
    );
    for (owner, kind, expected) in [
        (
            &[][..],
            TypeRefLexemeKind::PathSegment { ordinal: 0 },
            range(0, 3),
        ),
        (&[][..], TypeRefLexemeKind::TurbofishSeparator, range(3, 5)),
        (&[][..], TypeRefLexemeKind::OpenAngle, range(5, 6)),
        (
            &[TypeRefNodeStep::GenericArgument(0)][..],
            TypeRefLexemeKind::PathSegment { ordinal: 0 },
            range(6, 9),
        ),
        (&[][..], TypeRefLexemeKind::CloseAngle, range(9, 10)),
    ] {
        assert_owned_type_lexeme(receiver, owner, kind, expected);
    }
    assert_eq!(
        receiver
            .source()
            .lexemes()
            .iter()
            .filter(|lexeme| matches!(lexeme.kind(), TypeRefLexemeKind::TurbofishSeparator))
            .count(),
        1
    );
    assert_eq!(callee.separator().range(), range(10, 11));
    assert_eq!(callee.member_range(), range(11, 24));
    assert_eq!(callee.range(), range(0, 24));
    assert_eq!(call.range(), range(0, 27));
}

#[test]
fn associated_turbofish_path_ranges() {
    let call = parsed_call("Vec::<I32>::with_capacity(8)");
    let callee = path_member(&call);
    let receiver = callee.receiver();
    assert_eq!(receiver.root_source().whole(), &range(0, 10));
    assert_type_node(receiver, &[], range(0, 10));
    assert_type_node(
        receiver,
        &[TypeRefNodeStep::GenericArgument(0)],
        range(6, 9),
    );
    for (owner, kind, expected) in [
        (
            &[][..],
            TypeRefLexemeKind::PathSegment { ordinal: 0 },
            range(0, 3),
        ),
        (&[][..], TypeRefLexemeKind::TurbofishSeparator, range(3, 5)),
        (&[][..], TypeRefLexemeKind::OpenAngle, range(5, 6)),
        (
            &[TypeRefNodeStep::GenericArgument(0)][..],
            TypeRefLexemeKind::PathSegment { ordinal: 0 },
            range(6, 9),
        ),
        (&[][..], TypeRefLexemeKind::CloseAngle, range(9, 10)),
    ] {
        assert_owned_type_lexeme(receiver, owner, kind, expected);
    }
    assert_eq!(
        receiver
            .source()
            .lexemes()
            .iter()
            .filter(|lexeme| matches!(lexeme.kind(), TypeRefLexemeKind::TurbofishSeparator))
            .count(),
        1
    );
    assert_eq!(callee.separator().range(), range(10, 12));
    assert_eq!(callee.member_range(), range(12, 25));
    assert_eq!(callee.range(), range(0, 25));
    assert_eq!(call.range(), range(0, 28));
}

#[test]
fn nongeneric_path_separator_aliases_not_introduced() {
    for source in [
        "String::with_capacity(64)",
        "Bytes::with_capacity(8)",
        "Vec::with_capacity(8)",
    ] {
        let call = parsed_call(source);
        assert!(
            call.parenthesized_syntax()
                .expect("parenthesized call")
                .callee()
                .path_member()
                .is_none(),
            "{source} must remain an ordinary nongeneric path call"
        );
    }
}

#[test]
fn ordinary_expression_receiver_remains_ordinary() {
    let source = "factory().with_capacity(8)";
    let call = parsed_call(source);
    assert!(matches!(
        call.parenthesized_syntax()
            .expect("outer parenthesized call")
            .callee(),
        ParenthesizedCalleeSyntax::Ordinary { range: callee, .. }
            if *callee == range(0, 23)
    ));
    assert_eq!(call.callee_range(), range(0, 23));
    assert_eq!(call.range(), range(0, 26));
    let call_ranges = collect_expr_source_ranges(&Expr::Call(call), source, range(0, source.len()))
        .into_iter()
        .filter_map(|entry| matches!(entry.expr(), Expr::Call(_)).then_some(entry.range()))
        .collect::<Vec<_>>();
    assert_eq!(call_ranges, [range(0, 26), range(0, 9)]);
}

#[test]
fn call_argument_surface_unchanged_for_associated_callee() {
    const ASSOCIATED: &str = "Vec<I32>.with_capacity";
    const ORDINARY: &str = "ordinary";
    for (surface, recovered) in [
        ("(1)", false),
        ("(capacity = n)", false),
        ("(values...)", false),
        ("(1,)", false),
        ("(1, capacity = n, values...,)", false),
        ("(1, capacity = n", true),
    ] {
        let associated_source = format!("{ASSOCIATED}{surface}");
        let ordinary_source = format!("{ORDINARY}{surface}");
        let associated = if recovered {
            recovered_call(&associated_source).0
        } else {
            parsed_call(&associated_source)
        };
        let ordinary = if recovered {
            recovered_call(&ordinary_source).0
        } else {
            parsed_call(&ordinary_source)
        };
        assert!(associated.path_member_callee_syntax().is_some());
        assert!(matches!(
            ordinary
                .parenthesized_syntax()
                .expect("ordinary parenthesized call")
                .callee(),
            ParenthesizedCalleeSyntax::Ordinary { .. }
        ));
        let associated_list = associated
            .parenthesized_syntax()
            .expect("associated parenthesized call")
            .argument_list();
        let ordinary_list = ordinary
            .parenthesized_syntax()
            .expect("ordinary parenthesized call")
            .argument_list();
        assert_relative_argument_surface(
            associated_list,
            ASSOCIATED.len(),
            ordinary_list,
            ORDINARY.len(),
            surface.len(),
        );
    }
}

#[test]
fn static_generic_calls_use_pratt_owned_argument_ranges() {
    let call = parsed_call("foo::<T>(value)");
    let list = call
        .parenthesized_syntax()
        .expect("static generic call is parenthesized")
        .argument_list();
    assert_eq!(call.callee_range(), range(0, 8));
    assert_eq!(list.open_paren(), range(8, 9));
    assert_eq!(list.arguments()[0].range(), range(9, 14));
    assert_eq!(list.close_paren(), Some(range(14, 15)));
    assert_eq!(
        call.callee().dotted_selector_label().as_deref(),
        Some("foo")
    );
    let application = call
        .explicit_type_application()
        .expect("static generic call retains typed application");
    assert!(application.is_turbofish());
    assert!(matches!(
        application.arguments(),
        [TypeRef::Path(path)] if path.canonical_string() == "T"
    ));

    let nested_source = "registry::resolve::<Option<Result<T, E>>>(value)";
    let nested = parsed_call(nested_source);
    let open = nested_source.find('(').expect("authored call open");
    assert_eq!(nested.callee_range(), range(0, open));
    assert_eq!(
        nested
            .parenthesized_syntax()
            .expect("nested static generic call")
            .argument_list()
            .open_paren(),
        range(open, open + 1)
    );

    let trailing = parsed_call("foo::<T,>()");
    assert_eq!(trailing.callee_range(), range(0, 9));
    assert_eq!(
        trailing.callee().dotted_selector_label().as_deref(),
        Some("foo")
    );
    assert!(
        trailing
            .explicit_type_application()
            .is_some_and(ExplicitCallTypeApplicationSyntax::is_turbofish)
    );
}

#[test]
fn static_generic_current_fixture_parses_without_source_scan() {
    let call = parsed_call("Vec<i32>::with_capacity(4usize)");
    let callee = call
        .path_member_callee_syntax()
        .expect("current collection fixture has a typed path-member callee");
    assert_eq!(callee.receiver().root_source().whole(), &range(0, 8));
    assert!(callee.separator().is_explicit_path());
    assert_eq!(callee.member().as_str(), "with_capacity");
    assert_eq!(call.args().len(), 1);
}

#[test]
fn comparison_lookahead_unchanged_by_associated_receiver() {
    let simple = parse_expr("a<b").expect("simple comparison");
    assert!(matches!(
        &simple,
        Expr::Binary {
            lhs,
            op: BinaryOp::Lt,
            rhs,
        } if matches!(lhs.as_ref(), Expr::Path(_)) && matches!(rhs.as_ref(), Expr::Path(_))
    ));
    assert_eq!(
        collect_expr_source_ranges(&simple, "a<b", range(0, 3))
            .into_iter()
            .map(|entry| entry.range())
            .collect::<Vec<_>>(),
        [range(0, 3), range(0, 1), range(2, 3)]
    );

    let additive = parse_expr("a<b+c").expect("comparison with additive rhs");
    assert!(matches!(
        &additive,
        Expr::Binary {
            lhs,
            op: BinaryOp::Lt,
            rhs,
        } if matches!(lhs.as_ref(), Expr::Path(_))
            && matches!(
                rhs.as_ref(),
                Expr::Binary {
                    lhs,
                    op: BinaryOp::Add,
                    rhs,
                } if matches!(lhs.as_ref(), Expr::Path(_))
                    && matches!(rhs.as_ref(), Expr::Path(_))
            )
    ));
    assert_eq!(
        collect_expr_source_ranges(&additive, "a<b+c", range(0, 5))
            .into_iter()
            .map(|entry| entry.range())
            .collect::<Vec<_>>(),
        [
            range(0, 5),
            range(0, 1),
            range(2, 5),
            range(2, 3),
            range(4, 5),
        ]
    );

    let chained_source = "a < b > (c)";
    let chained = parse_expr(chained_source).expect("spaced comparison chain");
    assert!(matches!(
        &chained,
        Expr::Binary {
            lhs,
            op: BinaryOp::Gt,
            rhs,
        } if matches!(
            lhs.as_ref(),
            Expr::Binary {
                lhs,
                op: BinaryOp::Lt,
                rhs,
            } if matches!(lhs.as_ref(), Expr::Path(_))
                && matches!(rhs.as_ref(), Expr::Path(_))
        ) && matches!(rhs.as_ref(), Expr::Path(_))
    ));
    assert_eq!(
        collect_expr_source_ranges(&chained, chained_source, range(0, chained_source.len()),)
            .into_iter()
            .map(|entry| entry.range())
            .collect::<Vec<_>>(),
        [
            range(0, 11),
            range(0, 5),
            range(0, 1),
            range(4, 5),
            range(8, 11),
        ]
    );
}

#[test]
fn malformed_static_generic_rolls_back_atomically() {
    for (source, expected_identifiers) in [
        (
            "Vec::<T::>().with_capacity(8)",
            &["Vec", "T", "with_capacity"][..],
        ),
        (
            "Vec<,T>.with_capacity(8)",
            &["Vec", "T", "with_capacity"][..],
        ),
    ] {
        let tokens = Lexer::new(source).tokenize();
        let identifiers = tokens
            .iter()
            .filter_map(|token| match &token.token {
                Token::Ident(name) => Some(name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(identifiers, expected_identifiers);
        assert!(
            parse_expr(source).is_err(),
            "{source} must fail through ordinary expression grammar"
        );
        let receiver_end = source.find(".with_capacity").expect("member separator");
        assert!(
            parse_type_ref(&source[..receiver_end]).is_err(),
            "{source} must publish no partial AuthoredTypeRef"
        );
        let call_open = source.rfind('(').expect("terminal call open");
        let (call, parsed) = recovered_call(source);
        assert_eq!(parsed.diagnostics.len(), 1);
        assert!(matches!(call.callee(), Expr::Raw(raw) if raw == &source[..call_open]));
        let syntax = call
            .parenthesized_syntax()
            .expect("recovered parenthesized surface");
        assert!(matches!(
            syntax.callee(),
            ParenthesizedCalleeSyntax::Ordinary { range: callee, .. }
                if *callee == range(0, call_open)
        ));
        assert!(call.path_member_callee_syntax().is_none());
        assert_eq!(
            syntax.argument_list().open_paren(),
            range(call_open, call_open + 1)
        );
        assert_eq!(call.args().len(), 1);
        assert_eq!(
            syntax.argument_list().arguments()[0].value_range(),
            range(call_open + 1, call_open + 2)
        );
        assert_eq!(call.range(), range(0, source.len()));
    }
}

#[test]
fn missing_associated_member_recovers_ordinary_call() {
    for source in ["Vec<i32>.(8)", "Vec<i32>::(8)"] {
        let call_open = source.find('(').expect("call open");
        let (call, parsed) = recovered_call(source);
        assert_eq!(parsed.diagnostics.len(), 1);
        let diagnostic_range = parsed.diagnostics[0].range();
        assert!(diagnostic_range.start() < diagnostic_range.end());
        assert!(matches!(call.callee(), Expr::Raw(raw) if raw == &source[..call_open]));
        let syntax = call
            .parenthesized_syntax()
            .expect("recovered parenthesized surface");
        assert!(matches!(
            syntax.callee(),
            ParenthesizedCalleeSyntax::Ordinary { range: callee, .. }
                if *callee == range(0, call_open)
        ));
        assert!(call.path_member_callee_syntax().is_none());
        assert_eq!(
            syntax.argument_list().open_paren(),
            range(call_open, call_open + 1)
        );
        assert_eq!(call.args().len(), 1);
        assert_eq!(
            syntax.argument_list().arguments()[0].value_range(),
            range(call_open + 1, call_open + 2)
        );
        assert_eq!(call.range(), range(0, source.len()));
    }
}

#[test]
fn associated_call_exact_argument_limit() {
    let source = format!(
        "Vec<I32>.with_capacity({})",
        (0..MAX_CALL_ARGUMENTS)
            .map(|index| format!("a{index}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let call = parsed_call(&source);
    let callee = path_member(&call);
    assert_eq!(callee.receiver().root_source().whole(), &range(0, 8));
    assert_eq!(callee.member().as_str(), "with_capacity");
    let list = call
        .parenthesized_syntax()
        .expect("parenthesized surface")
        .argument_list();
    assert_eq!(call.args().len(), MAX_CALL_ARGUMENTS);
    assert_eq!(list.arguments().len(), MAX_CALL_ARGUMENTS);
    assert_eq!(list.separators().len(), MAX_CALL_ARGUMENTS - 1);
}

#[test]
fn associated_receiver_exact_generic_argument_limit() {
    const MAX_GENERIC_ARGUMENTS: usize = 256;
    let receiver = format!(
        "Many<{}>",
        std::iter::repeat_n("T", MAX_GENERIC_ARGUMENTS)
            .collect::<Vec<_>>()
            .join(",")
    );
    let source = format!("{receiver}.with_capacity(8)");
    let call = parsed_call(&source);
    let callee = path_member(&call);
    assert_eq!(
        callee.receiver().root_source().whole(),
        &range(0, receiver.len())
    );
    assert_eq!(
        callee.receiver().source().nodes().len(),
        MAX_GENERIC_ARGUMENTS + 1
    );
    assert_eq!(
        callee.receiver().source().lexemes().len(),
        2 * MAX_GENERIC_ARGUMENTS + 2
    );
    for index in 0..MAX_GENERIC_ARGUMENTS {
        let index = u16::try_from(index).expect("generic argument index");
        assert!(
            callee
                .receiver()
                .source()
                .nodes()
                .iter()
                .any(|(path, _)| { path.steps() == [TypeRefNodeStep::GenericArgument(index)] })
        );
    }
    assert_eq!(
        callee.separator().range(),
        range(receiver.len(), receiver.len() + 1)
    );
    assert_eq!(callee.member().as_str(), "with_capacity");
    assert_eq!(callee.range(), range(0, receiver.len() + 14));
    assert_eq!(call.args().len(), 1);
    assert_eq!(call.range(), range(0, source.len()));
}

#[test]
fn associated_receiver_one_over_generic_argument_limit() {
    const ONE_OVER_GENERIC_ARGUMENTS: usize = 257;
    let receiver = format!(
        "Many<{}>",
        std::iter::repeat_n("T", ONE_OVER_GENERIC_ARGUMENTS)
            .collect::<Vec<_>>()
            .join(",")
    );
    let source = format!("{receiver}.with_capacity(8)");
    let error = parse_expr(&source).expect_err("one-over receiver must fail atomically");
    assert_eq!(
        error.to_string(),
        "type constructor exceeds the 256 argument limit"
    );
    assert_eq!(error.code(), "syntax.type.generic_argument_limit");
    assert!(
        parse_expr_recovering_at(
            &source,
            ExprParseScope {
                source_range: range(0, source.len()),
                end_boundary: CallRecoveryBoundarySyntax::EndOfExpression,
            },
        )
        .is_err(),
        "one-over receiver must not publish a partial call or type map"
    );
}

#[test]
fn associated_receiver_exact_type_node_limit() {
    const MAX_TYPE_NODES: usize = 4_096;
    let tuple = format!(
        "({})",
        std::iter::repeat_n("T", MAX_TYPE_NODES - 2)
            .collect::<Vec<_>>()
            .join(",")
    );
    let receiver = format!("Many<{tuple}>");
    let source = format!("{receiver}.with_capacity(8)");
    let call = parsed_call(&source);
    let callee = path_member(&call);
    assert_eq!(callee.receiver().source().nodes().len(), MAX_TYPE_NODES);
    assert_eq!(
        callee.receiver().root_source().whole(),
        &range(0, receiver.len())
    );
    assert_eq!(
        callee.separator().range(),
        range(receiver.len(), receiver.len() + 1)
    );
    assert_eq!(callee.member().as_str(), "with_capacity");
    assert_eq!(callee.range(), range(0, receiver.len() + 14));
    assert_eq!(call.args().len(), 1);
    assert_eq!(call.range(), range(0, source.len()));
}

#[test]
fn associated_receiver_one_over_type_node_limit() {
    const ONE_OVER_TYPE_NODES: usize = 4_097;
    let tuple = format!(
        "({})",
        std::iter::repeat_n("T", ONE_OVER_TYPE_NODES - 2)
            .collect::<Vec<_>>()
            .join(",")
    );
    let receiver = format!("Many<{tuple}>");
    let source = format!("{receiver}.with_capacity(8)");
    let error = parse_expr(&source).expect_err("one-over receiver must fail atomically");
    assert_eq!(error.to_string(), "type exceeds the 4096 node limit");
    assert_eq!(error.code(), "syntax.type.node_limit");
    assert!(
        parse_expr_recovering_at(
            &source,
            ExprParseScope {
                source_range: range(0, source.len()),
                end_boundary: CallRecoveryBoundarySyntax::EndOfExpression,
            },
        )
        .is_err(),
        "one-over receiver must not publish a partial call or type map"
    );
}

#[test]
fn associated_call_one_over_argument_limit() {
    let source = format!(
        "Vec<I32>.with_capacity({})",
        (0..=MAX_CALL_ARGUMENTS)
            .map(|index| format!("a{index}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let error = parse_expr(&source).expect_err("one-over call argument limit must fail");
    assert_eq!(error.code(), "syntax.expr.call_argument_limit");
}

#[test]
fn call_surface_exact_limit_callback_parameters_succeeds() {
    let source = format!(
        "items.map {{ {} => p0 }}",
        (0..MAX_CALLBACK_PARAMETERS)
            .map(|index| format!("p{index}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let call = parsed_callback(&source);
    let parameters = call
        .callback_block_syntax()
        .expect("callback surface")
        .callback()
        .parameters();
    assert_eq!(parameters.parameters().len(), MAX_CALLBACK_PARAMETERS);
    assert_eq!(parameters.separators().len(), MAX_CALLBACK_PARAMETERS - 1);
}

#[test]
fn call_surface_one_over_callback_parameter_limit_fails_atomically() {
    let source = format!(
        "items.map {{ {} => p0 }}",
        (0..=MAX_CALLBACK_PARAMETERS)
            .map(|index| format!("p{index}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let error = parse_expr(&source).expect_err("one-over callback parameter limit must fail");
    assert_eq!(error.code(), "syntax.expr.callback_parameter_limit");
}

#[test]
fn call_surface_exact_limit_recovered_diagnostics_succeeds() {
    let source = format!("f({})", vec!["@@@"; MAX_EXPR_DIAGNOSTICS].join(","));
    let (call, parsed) = recovered_call(&source);
    let list = call
        .parenthesized_syntax()
        .expect("parenthesized surface")
        .argument_list();

    assert_eq!(parsed.diagnostics.len(), MAX_EXPR_DIAGNOSTICS);
    assert_eq!(call.args().len(), MAX_EXPR_DIAGNOSTICS);
    assert_eq!(list.arguments().len(), MAX_EXPR_DIAGNOSTICS);
    assert!(list.arguments().iter().all(|argument| matches!(
        argument.recovery(),
        CallArgumentRecoverySyntax::Recovered { .. }
    )));
}

#[test]
fn call_surface_one_over_diagnostic_limit_fails_atomically() {
    let source = format!("f({})", vec!["@@@"; MAX_EXPR_DIAGNOSTICS + 1].join(","));
    let error = parse_expr_recovering_at(
        &source,
        ExprParseScope {
            source_range: range(0, source.len()),
            end_boundary: CallRecoveryBoundarySyntax::EndOfExpression,
        },
    )
    .expect_err("one-over diagnostic limit must fail");
    assert_eq!(error.code(), "syntax.expr.diagnostic_limit");
}

#[test]
fn call_surface_exact_limit_nested_calls_succeeds() {
    let mut source = "value".to_owned();
    for _ in 0..MAX_NESTED_CALLS {
        source = format!("f({source})");
    }
    let call = parsed_call(&source);
    assert_eq!(call.range(), range(0, source.len()));
}

#[test]
fn call_surface_one_over_nested_call_limit_fails_atomically() {
    let mut source = "value".to_owned();
    for _ in 0..=MAX_NESTED_CALLS {
        source = format!("f({source})");
    }
    let error = parse_expr(&source).expect_err("one-over nested call limit must fail");
    assert_eq!(error.code(), "syntax.expr.call_nesting_limit");
}

#[test]
fn call_surface_checked_fragment_base_overflow_fails_without_value() {
    let error = parse_expr_fragment_recovering_at(
        "f()",
        usize::MAX,
        CallRecoveryBoundarySyntax::EndOfExpression,
    )
    .expect_err("source base overflow must fail before a syntax value is published");
    assert_eq!(error.code(), "syntax.expr.offset_overflow");
}

#[test]
fn parser_invariant_accepts_parenthesized_empty_call_ranges() {
    let syntax = parenthesized(
        "f()",
        range(0, 1),
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: Vec::new(),
            separators: Vec::new(),
            trailing_comma: None,
            terminator: closed(2),
        },
    )
    .expect("valid empty call syntax");
    let call = CallExpr::try_parenthesized(Expr::Path("f".into()), Vec::new(), syntax)
        .expect("semantic shape matches");

    assert_eq!(call.range(), range(0, 3));
    assert_eq!(call.callee_range(), range(0, 1));
    let list = call.parenthesized_syntax().unwrap().argument_list();
    assert_eq!(list.open_paren(), range(1, 2));
    assert_eq!(list.content_range(), range(2, 2));
    assert_eq!(list.close_paren(), Some(range(2, 3)));
    assert!(list.arguments().is_empty());
    assert!(list.separators().is_empty());
    assert_eq!(list.trailing_comma(), None);
    assert!(list.contains_signature_cursor(2));
    assert_eq!(list.active_argument_slot(2), Some(0));
    assert!(!list.contains_signature_cursor(1));
    assert!(!list.contains_signature_cursor(3));
}

#[test]
fn parser_invariant_accepts_parenthesized_utf8_ranges() {
    let source = "f(α, \"猫\")";
    let syntax = parenthesized(
        source,
        range(0, 1),
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: vec![positional(2, 4), positional(6, 11)],
            separators: vec![range(4, 5)],
            trailing_comma: None,
            terminator: closed(11),
        },
    )
    .expect("valid UTF-8 call syntax");
    let call = CallExpr::try_parenthesized(
        Expr::Path("f".into()),
        vec![
            CallArg::Positional(Box::new(Expr::Path("α".into()))),
            CallArg::Positional(Box::new(Expr::Literal(super::Literal::String(
                "猫".to_owned(),
            )))),
        ],
        syntax,
    )
    .expect("semantic shape matches");

    let list = call.parenthesized_syntax().unwrap().argument_list();
    assert_eq!(call.range(), range(0, 12));
    assert_eq!(list.arguments()[0].range(), range(2, 4));
    assert_eq!(list.arguments()[1].value_range(), range(6, 11));
    assert_eq!(list.separators(), &[range(4, 5)]);
    assert_eq!(list.active_argument_slot(4), Some(1));
    assert_eq!(list.active_argument_slot(5), Some(1));
}

#[test]
fn comma_start_focuses_the_following_argument_slot() {
    let call = parsed_call("f(a , b)");
    let list = call
        .parenthesized_syntax()
        .expect("parenthesized call syntax")
        .argument_list();

    assert_eq!(list.separators(), &[range(4, 5)]);
    assert_eq!(list.active_argument_slot(3), Some(0));
    assert_eq!(list.active_argument_slot(4), Some(1));
    assert_eq!(list.active_argument_slot(5), Some(1));
}

#[test]
fn parenthesized_named_spread_and_trailing_comma_preserve_forms() {
    let named_syntax = parenthesized(
        "paint(look = .smile)",
        range(0, 5),
        ArgumentListSyntaxInit {
            open_paren: range(5, 6),
            arguments: vec![named(
                range(6, 19),
                range(6, 10),
                range(11, 12),
                range(13, 19),
            )],
            separators: Vec::new(),
            trailing_comma: None,
            terminator: closed(19),
        },
    )
    .expect("valid named call syntax");
    let named_call = CallExpr::try_parenthesized(
        Expr::Path("paint".into()),
        vec![CallArg::Named {
            name: "look".to_owned(),
            value: Box::new(Expr::ShortVariant(super::Name::new("smile"))),
        }],
        named_syntax,
    )
    .expect("named semantic form matches");
    assert_eq!(named_call.range(), range(0, 20));
    assert!(matches!(
        named_call.parenthesized_syntax().unwrap().argument_list().arguments()[0].form(),
        CallArgumentFormSyntax::Named { name, equals }
            if *name == range(6, 10) && *equals == range(11, 12)
    ));

    let spread_syntax = parenthesized(
        "log(fields...)",
        range(0, 3),
        ArgumentListSyntaxInit {
            open_paren: range(3, 4),
            arguments: vec![CallArgumentSyntaxInit {
                range: range(4, 13),
                value: range(4, 10),
                form: CallArgumentFormSyntax::Spread {
                    ellipsis: range(10, 13),
                },
                recovery: CallArgumentRecoverySyntax::Parsed,
            }],
            separators: Vec::new(),
            trailing_comma: None,
            terminator: closed(13),
        },
    )
    .expect("valid spread call syntax");
    let spread_call = CallExpr::try_parenthesized(
        Expr::Path("log".into()),
        vec![CallArg::Spread {
            value: Box::new(Expr::Path("fields".into())),
        }],
        spread_syntax,
    )
    .expect("spread semantic form matches");
    assert!(matches!(
        spread_call.parenthesized_syntax().unwrap().argument_list().arguments()[0].form(),
        CallArgumentFormSyntax::Spread { ellipsis } if *ellipsis == range(10, 13)
    ));

    let trailing = parenthesized(
        "f(α,)",
        range(0, 1),
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: vec![positional(2, 4)],
            separators: Vec::new(),
            trailing_comma: Some(range(4, 5)),
            terminator: closed(5),
        },
    )
    .expect("valid trailing comma");
    let list = trailing.argument_list();
    assert_eq!(list.separators(), &[]);
    assert_eq!(list.trailing_comma(), Some(range(4, 5)));
    assert_eq!(list.active_argument_slot(4), Some(1));
    assert_eq!(list.active_argument_slot(5), Some(1));
}

#[test]
fn missing_close_records_insertion_without_fabricated_parenthesis() {
    let source = "f(α, β";
    let syntax = parenthesized(
        source,
        range(0, 1),
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: vec![positional(2, 4), positional(6, 8)],
            separators: vec![range(4, 5)],
            trailing_comma: None,
            terminator: ArgumentListTerminatorSyntax::RecoveredMissing {
                insertion: 8,
                boundary: CallRecoveryBoundarySyntax::EndOfExpression,
            },
        },
    )
    .expect("valid missing-close recovery");
    let list = syntax.argument_list();
    assert_eq!(syntax.range(), range(0, 8));
    assert_eq!(list.close_paren(), None);
    assert_eq!(
        list.recovery_boundary(),
        Some(CallRecoveryBoundarySyntax::EndOfExpression)
    );
    assert_eq!(list.content_range(), range(2, 8));
    assert!(list.contains_signature_cursor(8));
    assert_eq!(list.active_argument_slot(8), Some(1));
    assert!(!list.contains_signature_cursor(9));
}

#[test]
fn missing_close_records_exact_owner_token() {
    let source = "f(value]";
    let syntax = parenthesized(
        source,
        range(0, 1),
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: vec![positional(2, 7)],
            separators: Vec::new(),
            trailing_comma: None,
            terminator: ArgumentListTerminatorSyntax::RecoveredMissing {
                insertion: 7,
                boundary: CallRecoveryBoundarySyntax::Token {
                    kind: CallRecoveryTokenKind::CloseBracket,
                    range: range(7, 8),
                },
            },
        },
    )
    .expect("valid owner-token recovery");
    assert_eq!(syntax.range(), range(0, 7));
    assert_eq!(
        syntax.argument_list().recovery_boundary(),
        Some(CallRecoveryBoundarySyntax::Token {
            kind: CallRecoveryTokenKind::CloseBracket,
            range: range(7, 8),
        })
    );
}

#[test]
fn recovered_argument_requires_one_nonempty_diagnostic_inside_value() {
    let source = "f(@@@)";
    let syntax = parenthesized(
        source,
        range(0, 1),
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: vec![CallArgumentSyntaxInit {
                range: range(2, 5),
                value: range(2, 5),
                form: CallArgumentFormSyntax::Positional,
                recovery: CallArgumentRecoverySyntax::Recovered {
                    diagnostic: range(2, 5),
                },
            }],
            separators: Vec::new(),
            trailing_comma: None,
            terminator: closed(5),
        },
    )
    .expect("valid recovered argument");
    assert_eq!(
        syntax.argument_list().arguments()[0].recovery(),
        CallArgumentRecoverySyntax::Recovered {
            diagnostic: range(2, 5),
        }
    );
}

#[test]
fn callback_block_has_exact_typed_header_and_body() {
    let source = "items.map { item: Label => item.text }";
    let callback = CallbackBlockSyntax::try_from_parser(
        source,
        0,
        CallbackBlockSyntaxInit {
            open_brace: range(10, 11),
            parameters: CallbackParameterHeaderSyntaxInit::Explicit {
                parameters: vec![CallbackParameterSyntaxInit {
                    range: range(12, 23),
                    pattern: range(12, 16),
                    type_ascription: Some(CallbackParameterTypeSyntax::new(
                        range(16, 17),
                        range(18, 23),
                    )),
                }],
                separators: Vec::new(),
                fat_arrow: range(24, 26),
            },
            body: range(27, 36),
            close_brace: range(37, 38),
        },
    )
    .expect("valid callback syntax");
    let surface = CallbackBlockCallSyntax::try_from_parser(range(0, 9), callback)
        .expect("valid callback call surface");
    let closure = Expr::Closure {
        params: vec![ClosureParam::new(
            parse_pattern("item"),
            Some(parse_type_ref("Label").expect("valid type")),
        )],
        return_type: None,
        body: Box::new(Expr::Path(DottedPath::parse_dotted("item.text"))),
        source: ClosureExprSource::new(range(10, 38), range(10, 26), None, range(27, 36)),
    };
    let call = CallExpr::try_callback_block(
        Expr::Path(DottedPath::parse_dotted("items.map")),
        closure,
        surface,
    )
    .expect("semantic callback shape matches");

    assert_eq!(call.range(), range(0, 38));
    assert_eq!(call.args().len(), 1);
    let callback = call.callback_block_syntax().unwrap().callback();
    assert_eq!(callback.open_brace(), range(10, 11));
    assert_eq!(callback.body_range(), range(27, 36));
    assert_eq!(callback.close_brace(), range(37, 38));
    assert_eq!(callback.closure_range(), range(10, 38));
    let header = callback.parameters();
    assert!(!header.is_implicit_zero());
    assert_eq!(header.fat_arrow(), Some(range(24, 26)));
    assert_eq!(header.parameters()[0].range(), range(12, 23));
    assert_eq!(header.parameters()[0].pattern_range(), range(12, 16));
    let ty = header.parameters()[0].type_ascription().unwrap();
    assert_eq!(ty.colon(), range(16, 17));
    assert_eq!(ty.ty_range(), range(18, 23));
}

#[test]
fn callback_block_implicit_zero_has_no_parameter_punctuation() {
    let source = "tap { emit() }";
    let callback = CallbackBlockSyntax::try_from_parser(
        source,
        0,
        CallbackBlockSyntaxInit {
            open_brace: range(4, 5),
            parameters: CallbackParameterHeaderSyntaxInit::ImplicitZero,
            body: range(6, 12),
            close_brace: range(13, 14),
        },
    )
    .expect("valid implicit-zero callback");
    assert!(callback.parameters().is_implicit_zero());
    assert!(callback.parameters().parameters().is_empty());
    assert!(callback.parameters().separators().is_empty());
    assert_eq!(callback.parameters().fat_arrow(), None);
}

#[test]
fn invalid_utf8_boundary_is_rejected() {
    let error = ArgumentListSyntax::try_from_parser(
        "f(α)",
        0,
        ArgumentListSyntaxInit {
            open_paren: range(2, 3),
            arguments: Vec::new(),
            separators: Vec::new(),
            trailing_comma: None,
            terminator: closed(4),
        },
    )
    .unwrap_err();
    assert_eq!(error, CallSyntaxInvariantError::InvalidUtf8Boundary);
}

#[test]
fn invalid_punctuation_range_is_rejected() {
    let error = ArgumentListSyntax::try_from_parser(
        "f()",
        0,
        ArgumentListSyntaxInit {
            open_paren: range(0, 1),
            arguments: Vec::new(),
            separators: Vec::new(),
            trailing_comma: None,
            terminator: closed(2),
        },
    )
    .unwrap_err();
    assert_eq!(error, CallSyntaxInvariantError::InvalidTokenRange);

    let oversized = ArgumentListSyntax::try_from_parser(
        "f()",
        0,
        ArgumentListSyntaxInit {
            open_paren: range(1, 3),
            arguments: Vec::new(),
            separators: Vec::new(),
            trailing_comma: None,
            terminator: closed(2),
        },
    )
    .unwrap_err();
    assert_eq!(oversized, CallSyntaxInvariantError::InvalidTokenRange);
}

#[test]
fn range_order_is_rejected() {
    let error = ArgumentListSyntax::try_from_parser(
        "f(x)",
        0,
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: vec![positional(3, 2)],
            separators: Vec::new(),
            trailing_comma: None,
            terminator: closed(3),
        },
    )
    .unwrap_err();
    assert_eq!(error, CallSyntaxInvariantError::RangeOrder);

    let delimiter_order = ArgumentListSyntax::try_from_parser(
        ")(",
        0,
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: Vec::new(),
            separators: Vec::new(),
            trailing_comma: None,
            terminator: ArgumentListTerminatorSyntax::Closed {
                close_paren: range(0, 1),
            },
        },
    )
    .unwrap_err();
    assert_eq!(delimiter_order, CallSyntaxInvariantError::RangeOrder);
}

#[test]
fn argument_count_and_form_mismatch_are_rejected() {
    let syntax = parenthesized(
        "f(x)",
        range(0, 1),
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: vec![positional(2, 3)],
            separators: Vec::new(),
            trailing_comma: None,
            terminator: closed(3),
        },
    )
    .expect("valid syntax");
    let count_error =
        CallExpr::try_parenthesized(Expr::Path("f".into()), Vec::new(), syntax.clone())
            .unwrap_err();
    assert_eq!(count_error, CallSyntaxInvariantError::ArgumentCountMismatch);

    let form_error = CallExpr::try_parenthesized(
        Expr::Path("f".into()),
        vec![CallArg::Spread {
            value: Box::new(Expr::Path("x".into())),
        }],
        syntax,
    )
    .unwrap_err();
    assert_eq!(form_error, CallSyntaxInvariantError::ArgumentFormMismatch);

    let malformed_form = ArgumentListSyntax::try_from_parser(
        "f(x)",
        0,
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: vec![CallArgumentSyntaxInit {
                range: range(2, 3),
                value: range(2, 2),
                form: CallArgumentFormSyntax::Positional,
                recovery: CallArgumentRecoverySyntax::Parsed,
            }],
            separators: Vec::new(),
            trailing_comma: None,
            terminator: closed(3),
        },
    )
    .unwrap_err();
    assert_eq!(malformed_form, CallSyntaxInvariantError::RangeOrder);
}

#[test]
fn separator_count_and_trailing_comma_are_rejected() {
    let separator_error = ArgumentListSyntax::try_from_parser(
        "f(x,y)",
        0,
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: vec![positional(2, 3), positional(4, 5)],
            separators: Vec::new(),
            trailing_comma: None,
            terminator: closed(5),
        },
    )
    .unwrap_err();
    assert_eq!(
        separator_error,
        CallSyntaxInvariantError::SeparatorCountMismatch
    );

    let trailing_error = ArgumentListSyntax::try_from_parser(
        "f(,)",
        0,
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: Vec::new(),
            separators: Vec::new(),
            trailing_comma: Some(range(2, 3)),
            terminator: closed(3),
        },
    )
    .unwrap_err();
    assert_eq!(
        trailing_error,
        CallSyntaxInvariantError::InvalidTrailingComma
    );
}

#[test]
fn invalid_recovery_boundary_is_rejected() {
    let wrong_end = ArgumentListSyntax::try_from_parser(
        "f(x",
        0,
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: vec![positional(2, 3)],
            separators: Vec::new(),
            trailing_comma: None,
            terminator: ArgumentListTerminatorSyntax::RecoveredMissing {
                insertion: 2,
                boundary: CallRecoveryBoundarySyntax::EndOfExpression,
            },
        },
    )
    .unwrap_err();
    assert_eq!(wrong_end, CallSyntaxInvariantError::InvalidRecoveryBoundary);

    let wrong_token_start = ArgumentListSyntax::try_from_parser(
        "f(x]",
        0,
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: vec![positional(2, 3)],
            separators: Vec::new(),
            trailing_comma: None,
            terminator: ArgumentListTerminatorSyntax::RecoveredMissing {
                insertion: 2,
                boundary: CallRecoveryBoundarySyntax::Token {
                    kind: CallRecoveryTokenKind::CloseBracket,
                    range: range(3, 4),
                },
            },
        },
    )
    .unwrap_err();
    assert_eq!(
        wrong_token_start,
        CallSyntaxInvariantError::InvalidRecoveryBoundary
    );
}

#[test]
fn invalid_callee_range_is_rejected() {
    let arguments = ArgumentListSyntax::try_from_parser(
        "f()",
        0,
        ArgumentListSyntaxInit {
            open_paren: range(1, 2),
            arguments: Vec::new(),
            separators: Vec::new(),
            trailing_comma: None,
            terminator: closed(2),
        },
    )
    .expect("valid arguments");
    let error = ParenthesizedCallSyntax::try_from_parser(
        ParenthesizedCalleeSyntax::ordinary(range(0, 2)),
        arguments,
    )
    .unwrap_err();
    assert_eq!(error, CallSyntaxInvariantError::InvalidCalleeRange);
}

#[test]
fn invalid_callback_argument_and_header_are_rejected() {
    let empty_header = CallbackBlockSyntax::try_from_parser(
        "f { => x }",
        0,
        CallbackBlockSyntaxInit {
            open_brace: range(2, 3),
            parameters: CallbackParameterHeaderSyntaxInit::Explicit {
                parameters: Vec::new(),
                separators: Vec::new(),
                fat_arrow: range(4, 6),
            },
            body: range(7, 8),
            close_brace: range(9, 10),
        },
    )
    .unwrap_err();
    assert_eq!(
        empty_header,
        CallSyntaxInvariantError::InvalidCallbackParameterHeader
    );

    let callback = CallbackBlockSyntax::try_from_parser(
        "f { x }",
        0,
        CallbackBlockSyntaxInit {
            open_brace: range(2, 3),
            parameters: CallbackParameterHeaderSyntaxInit::ImplicitZero,
            body: range(4, 5),
            close_brace: range(6, 7),
        },
    )
    .expect("valid callback syntax");
    let surface =
        CallbackBlockCallSyntax::try_from_parser(range(0, 1), callback).expect("valid surface");
    let error = CallExpr::try_callback_block(
        Expr::Path("f".into()),
        Expr::Path("not_a_closure".into()),
        surface,
    )
    .unwrap_err();
    assert_eq!(error, CallSyntaxInvariantError::InvalidCallbackArgument);
}

#[test]
fn invalid_callback_body_is_rejected() {
    let error = CallbackBlockSyntax::try_from_parser(
        "f { }",
        0,
        CallbackBlockSyntaxInit {
            open_brace: range(2, 3),
            parameters: CallbackParameterHeaderSyntaxInit::ImplicitZero,
            body: range(4, 4),
            close_brace: range(4, 5),
        },
    )
    .unwrap_err();
    assert_eq!(error, CallSyntaxInvariantError::RangeOrder);

    let outside = CallbackBlockSyntax::try_from_parser(
        "f { x }",
        0,
        CallbackBlockSyntaxInit {
            open_brace: range(2, 3),
            parameters: CallbackParameterHeaderSyntaxInit::ImplicitZero,
            body: range(1, 2),
            close_brace: range(6, 7),
        },
    )
    .unwrap_err();
    assert_eq!(outside, CallSyntaxInvariantError::InvalidCallbackBody);
}

#[test]
fn source_base_overflow_is_rejected() {
    let error = ArgumentListSyntax::try_from_parser(
        "(",
        usize::MAX,
        ArgumentListSyntaxInit {
            open_paren: range(0, 1),
            arguments: Vec::new(),
            separators: Vec::new(),
            trailing_comma: None,
            terminator: ArgumentListTerminatorSyntax::RecoveredMissing {
                insertion: 1,
                boundary: CallRecoveryBoundarySyntax::EndOfExpression,
            },
        },
    )
    .unwrap_err();
    assert_eq!(error, CallSyntaxInvariantError::OffsetOverflow);
}
