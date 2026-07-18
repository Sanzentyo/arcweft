use super::{BinaryOp, Expr, ExprParseError, Placeholder, parse_expr};
use crate::ast::common::TextRange;
use crate::reference::BorrowKind;

fn strict_error(source: &str) -> ExprParseError {
    parse_expr(source).expect_err("fixture must fail strict expression parsing")
}

#[test]
fn generic_strict_failure_preserves_type_code_range_and_message() {
    let error: ExprParseError = strict_error("");

    assert_eq!(error.code(), "syntax.expr.parse");
    assert_eq!(error.range(), TextRange::new(0, 0));
    assert_eq!(error.to_string(), "expected expression");
}

#[test]
fn parses_field_access_comparison() {
    let parsed = parse_expr("self.current < self.end")
        .expect("field access comparison parses as an expression");
    let Expr::Binary { lhs, op, rhs } = parsed else {
        panic!("expected binary expression");
    };
    assert_eq!(op, BinaryOp::Lt);
    assert!(matches!(*lhs, Expr::Select(_)));
    assert!(matches!(*rhs, Expr::Select(_)));
}

#[test]
fn parses_pipe_rhs_if_let_expression() {
    let parsed =
        parse_expr("maybe |> if let .Some(value) = ^ when value > 1i64 { value } else { 1i64 }")
            .expect("pipe rhs if-let parses as an expression");
    assert!(matches!(
        parsed,
        Expr::Pipe { rhs, .. }
            if matches!(
                rhs.as_ref(),
                Expr::IfLet {
                    expr,
                    guard: Some(_),
                    else_branch: Some(_),
                    ..
                } if matches!(expr.as_ref(), Expr::Placeholder(Placeholder::PipeLeft))
            )
    ));
}

#[test]
fn parses_pipe_rhs_match_expression() {
    let parsed = parse_expr("ready |> match ^ { true => 7i64 false => 1i64 }")
        .expect("pipe rhs match parses as an expression");
    assert!(matches!(
        parsed,
        Expr::Pipe { rhs, .. }
            if matches!(
                rhs.as_ref(),
                Expr::Match { scrutinee, arms }
                    if matches!(scrutinee.as_ref(), Expr::Placeholder(Placeholder::PipeLeft))
                        && arms.len() == 2
            )
    ));
}

#[test]
fn reference_prefixes_obey_postfix_and_infix_precedence() {
    let parsed = parse_expr("&f(x)[i].field").expect("borrowed postfix chain");
    let Expr::Borrow(borrow) = parsed else {
        panic!("expected shared borrow");
    };
    assert_eq!(borrow.kind(), BorrowKind::Shared);
    assert_eq!(borrow.operator_range(), TextRange::new(0, 1));
    assert!(matches!(borrow.operand(), Expr::Select(_)));

    let parsed = parse_expr("*p * q").expect("deref followed by multiplication");
    assert!(matches!(
        parsed,
        Expr::Binary {
            lhs,
            op: BinaryOp::Mul,
            ..
        } if matches!(lhs.as_ref(), Expr::Deref(_))
    ));

    let parsed = parse_expr("&x & y").expect("borrow followed by merge");
    assert!(matches!(
        parsed,
        Expr::Binary {
            lhs,
            op: BinaryOp::Merge,
            ..
        } if matches!(lhs.as_ref(), Expr::Borrow(_))
    ));
}

#[test]
fn reference_prefixes_keep_keyword_and_logical_and_boundaries_distinct() {
    let Expr::Borrow(shared) = parse_expr("&mutable").expect("shared borrow of mutable path")
    else {
        panic!("expected shared borrow");
    };
    assert_eq!(shared.kind(), BorrowKind::Shared);
    assert!(matches!(shared.operand(), Expr::Path(path) if path.as_str() == "mutable"));

    let Expr::Borrow(mutable) = parse_expr("& mut value").expect("spaced mutable borrow") else {
        panic!("expected mutable borrow");
    };
    assert_eq!(mutable.kind(), BorrowKind::Mutable);

    let Expr::Binary {
        op: BinaryOp::And,
        rhs,
        ..
    } = parse_expr("ready && &value").expect("logical and with borrowed rhs")
    else {
        panic!("expected logical and");
    };
    assert!(matches!(rhs.as_ref(), Expr::Borrow(_)));
}

#[test]
fn mutable_borrow_and_nested_deref_keep_typed_operator_ranges() {
    let parsed = parse_expr("*&mut value").expect("nested reference prefixes");
    let Expr::Deref(deref) = parsed else {
        panic!("expected outer dereference");
    };
    assert_eq!(deref.operator_range(), TextRange::new(0, 1));
    let Expr::Borrow(borrow) = deref.operand() else {
        panic!("expected inner mutable borrow");
    };
    assert_eq!(borrow.kind(), BorrowKind::Mutable);
    assert_eq!(borrow.operator_range(), TextRange::new(1, 5));
}

#[test]
fn reference_prefix_ranges_use_original_expression_offsets() {
    let Expr::Borrow(leading) = parse_expr("  &mut value  ").expect("leading trivia parses") else {
        panic!("expected leading-trivia borrow");
    };
    assert_eq!(leading.operator_range(), TextRange::new(2, 6));

    let Expr::Closure { body, .. } = parse_expr("|value| &mut value").expect("closure parses")
    else {
        panic!("expected closure");
    };
    let Expr::Borrow(nested) = body.as_ref() else {
        panic!("expected borrowed closure body");
    };
    assert_eq!(nested.operator_range(), TextRange::new(8, 12));

    let Expr::Index { index, .. } = parse_expr("items[  *pointer]").expect("index parses") else {
        panic!("expected index expression");
    };
    let Expr::Deref(nested) = index.as_ref() else {
        panic!("expected dereferenced index");
    };
    assert_eq!(nested.operator_range(), TextRange::new(8, 9));
}

#[test]
fn mutable_borrow_accepts_comment_and_newline_trivia() {
    for source in ["&/* policy */mut value", "&// policy\nmut value"] {
        let Expr::Borrow(borrow) = parse_expr(source).expect("mutable borrow parses") else {
            panic!("expected mutable borrow");
        };
        assert_eq!(borrow.kind(), BorrowKind::Mutable);
        assert_eq!(
            borrow.operator_range(),
            TextRange::new(0, source.find("mut").expect("mut token") + 3)
        );
    }
}

#[test]
fn borrow_prefix_stops_before_range_unless_grouped() {
    let parsed = parse_expr("&x..y").expect("borrowed range start");
    assert!(matches!(
        parsed,
        Expr::Range {
            start: Some(start),
            ..
        } if matches!(start.as_ref(), Expr::Borrow(_))
    ));

    let parsed = parse_expr("&(x..y)").expect("borrowed grouped range");
    assert!(matches!(
        parsed,
        Expr::Borrow(borrow) if matches!(borrow.operand(), Expr::Range { .. })
    ));
}

#[test]
fn prefix_depth_limit_is_inclusive_and_typed() {
    let maximum = format!("{}value", "& ".repeat(64));
    assert!(parse_expr(&maximum).is_ok());

    let over_limit = format!("{}value", "& ".repeat(65));
    let error: ExprParseError = strict_error(&over_limit);
    assert_eq!(error.code(), "syntax.expr.prefix_depth_limit");
    assert_eq!(error.range(), TextRange::new(128, 129));
    assert_eq!(
        error.to_string(),
        "expression prefix nesting exceeds the inclusive limit of 64"
    );
}

#[test]
fn missing_prefix_operand_has_a_zero_width_typed_failure() {
    let error: ExprParseError = strict_error("&mut");
    assert_eq!(error.code(), "syntax.expr.missing_prefix_operand");
    assert_eq!(error.range(), TextRange::new(4, 4));
    assert_eq!(error.to_string(), "prefix operator requires an operand");
}

#[test]
fn missing_prefix_operands_anchor_at_each_expression_sync_token() {
    for source in ["&,", "&;", "&)", "&]", "&}", "&"] {
        let error: ExprParseError = strict_error(source);
        assert_eq!(error.code(), "syntax.expr.missing_prefix_operand");
        assert_eq!(error.range(), TextRange::new(1, 1));
        assert_eq!(error.to_string(), "prefix operator requires an operand");
    }
}

#[test]
fn borrow_closures_multiline_postfix_and_utf8_ranges_remain_exact() {
    let Expr::Borrow(closure) = parse_expr("&|value| value").expect("borrowed closure") else {
        panic!("expected borrowed closure");
    };
    assert!(matches!(closure.operand(), Expr::Closure { .. }));

    let Expr::Closure { body, .. } = parse_expr("|value|\n  &value").expect("multiline closure")
    else {
        panic!("expected closure");
    };
    assert!(matches!(body.as_ref(), Expr::Borrow(_)));

    let multiline = "items[\n  *pointer\n].field";
    let Expr::Select(select) = parse_expr(multiline).expect("multiline postfix operand") else {
        panic!("expected selected index expression");
    };
    let Expr::Index { index, .. } = select.target() else {
        panic!("expected index target");
    };
    let Expr::Deref(deref) = index.as_ref() else {
        panic!("expected dereferenced index");
    };
    let star = multiline.find('*').expect("star token");
    assert_eq!(deref.operator_range(), TextRange::new(star, star + 1));

    let utf8 = "前 + &mut 値";
    let Expr::Binary { rhs, .. } = parse_expr(utf8).expect("UTF-8 borrow expression") else {
        panic!("expected binary expression");
    };
    let Expr::Borrow(borrow) = rhs.as_ref() else {
        panic!("expected borrowed rhs");
    };
    let ampersand = utf8.find('&').expect("ampersand token");
    assert_eq!(
        borrow.operator_range(),
        TextRange::new(ampersand, ampersand + "&mut".len())
    );
}

#[test]
fn unsupported_cast_reports_the_current_unexpected_token_range() {
    let error: ExprParseError = strict_error("&value as Type");
    assert_eq!(error.code(), "syntax.expr.unexpected_token");
    assert_eq!(error.range(), TextRange::new(7, 9));
    assert_eq!(
        error.to_string(),
        "unexpected token after expression: Ident(\"as\")"
    );
}

#[test]
fn typed_assertion_family_is_reserved_for_statement_position() {
    let error: ExprParseError = strict_error("wrap(assert.check(true))");
    assert_eq!(error.code(), "syntax.assert.statement_only");
    assert_eq!(error.range(), TextRange::new(5, 23));
    assert_eq!(
        error.to_string(),
        "assert.check is a statement and cannot be used as an expression"
    );

    for mode in ["prove", "check", "debug"] {
        let source = format!("assert.{mode}(true)");
        let error: ExprParseError = strict_error(&source);
        assert_eq!(error.code(), "syntax.assert.statement_only");
        assert_eq!(error.range(), TextRange::new(0, source.len()));
        assert_eq!(
            error.to_string(),
            format!("assert.{mode} is a statement and cannot be used as an expression")
        );
    }

    let unknown: ExprParseError = strict_error("assert.assume(true)");
    assert_eq!(unknown.code(), "syntax.assert.unknown_mode");
    assert_eq!(unknown.range(), TextRange::new(0, 19));
    assert_eq!(unknown.to_string(), "unknown assertion mode");

    assert!(parse_expr("assert(true)").is_ok());
    assert!(parse_expr("object.assert.check(true)").is_ok());
}
