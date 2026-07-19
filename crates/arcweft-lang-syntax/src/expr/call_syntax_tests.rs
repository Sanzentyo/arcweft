use super::call_syntax::{
    ArgumentListSyntaxInit, CallArgumentSyntaxInit, CallSyntaxInvariantError,
    CallbackBlockSyntaxInit, CallbackParameterHeaderSyntaxInit, CallbackParameterSyntaxInit,
};
use super::{
    ArgumentListSyntax, ArgumentListTerminatorSyntax, CallArg, CallArgumentFormSyntax,
    CallArgumentRecoverySyntax, CallExpr, CallRecoveryBoundarySyntax, CallRecoveryTokenKind,
    CallbackBlockCallSyntax, CallbackBlockSyntax, CallbackParameterTypeSyntax, ClosureParam,
    DottedPath, Expr, ParenthesizedCallSyntax,
};
use crate::ast::common::TextRange;
use crate::pattern::parse_pattern;
use crate::types::parse_type_ref;

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
    ParenthesizedCallSyntax::try_from_parser(callee, arguments)
}

#[test]
fn parenthesized_empty_call_has_exact_ranges() {
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
fn parenthesized_positional_utf8_ranges_are_bytes() {
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
            CallArg::Positional(Expr::Path("α".into())),
            CallArg::Positional(Expr::Literal(super::Literal::String("猫".to_owned()))),
        ],
        syntax,
    )
    .expect("semantic shape matches");

    let list = call.parenthesized_syntax().unwrap().argument_list();
    assert_eq!(call.range(), range(0, 12));
    assert_eq!(list.arguments()[0].range(), range(2, 4));
    assert_eq!(list.arguments()[1].value_range(), range(6, 11));
    assert_eq!(list.separators(), &[range(4, 5)]);
    assert_eq!(list.active_argument_slot(4), Some(0));
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
    assert_eq!(list.active_argument_slot(4), Some(0));
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
    let error = ParenthesizedCallSyntax::try_from_parser(range(0, 2), arguments).unwrap_err();
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
