fn parse_ok(source: impl Into<String>) -> arcweft_lang_syntax::ast::items::TypedSyntaxTree {
    let parsed = arcweft_lang_syntax::parser::parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "expected source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed.into_typed_tree()
}

#[test]
fn flow_body_attributes_are_explicit_recovery_diagnostics() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r"
flow opening {
    #![generated(tool)]
    #[allow(style::redundant_decl_identity)]
    alice: hello[p]
}
",
    );

    assert_eq!(parsed.errors().len(), 2);
    assert!(parsed.errors()[0].message().contains("inner attributes"));
    assert!(parsed.errors()[1].message().contains("outer attributes"));
    let tree = parsed.typed_tree();
    let arcweft_lang_syntax::ast::items::Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    assert_eq!(flow.body().len(), 3);
    assert!(matches!(
        flow.body()[0],
        arcweft_lang_syntax::ast::flow::FlowItem::Raw(_)
    ));
    assert!(matches!(
        flow.body()[1],
        arcweft_lang_syntax::ast::flow::FlowItem::Raw(_)
    ));
}

use arcweft_lang_syntax::{
    ast::{
        flow::{FlowItem, Stmt},
        items::{Item, RawSyntaxFamily},
    },
    expr::{BinaryOp, CallArg, Expr, Literal, UnaryOp, parse_expr},
    types::{TypeRef, parse_type_ref},
};

fn field_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => Some(path.clone()),
        Expr::Field { target, field } => Some(format!("{}.{}", field_path(target)?, field)),
        _ => None,
    }
}

#[test]
fn pratt_parser_keeps_documented_precedence() {
    let expr = parse_expr("a + b * c").expect("multiplicative precedence parses");
    assert!(matches!(
        expr,
        Expr::Binary {
            op: BinaryOp::Add,
            rhs,
            ..
        } if matches!(rhs.as_ref(), Expr::Binary { op: BinaryOp::Mul, .. })
    ));

    let expr = parse_expr("a < b + c").expect("comparison with additive rhs parses");
    assert!(matches!(
        expr,
        Expr::Binary {
            op: BinaryOp::Lt,
            rhs,
            ..
        } if matches!(rhs.as_ref(), Expr::Binary { op: BinaryOp::Add, .. })
    ));

    let expr = parse_expr("-score").expect("unary negation parses");
    assert!(matches!(
        expr,
        Expr::Unary {
            op: UnaryOp::Neg,
            ..
        }
    ));
}

#[test]
fn generic_expr_brackets_are_indexes_not_dialogue_calls() {
    let expr = parse_expr("alice.say()[text]").expect("bracket postfix parses");
    assert!(matches!(expr, Expr::Index { .. }));

    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let handles = alice.say()[本文です。[p]]
}
",
    );
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    assert!(matches!(
        &flow.body()[0],
        FlowItem::Stmt(Stmt::Let {
            expr: Expr::DialogueCall { .. },
            ..
        })
    ));
}

#[test]
fn dialogue_trailing_brace_plan_avoids_owned_block_for_same_line() {
    let same_line = arcweft_lang_syntax::parser::parse_source(
        r"
flow @flow.opening opening {
    alice.say()[本文です。[p]] with { out handles }
}
",
    );
    assert!(
        same_line.errors().is_empty(),
        "expected same-line line plan to parse, got {:?}",
        same_line.errors()
    );
    assert_eq!(same_line.syntax_stats().block_owned_bytes, 0);
}

#[test]
fn indented_defer_body_groups_multiline_statements_from_cst_lines() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    defer:
        let saved = compute(
            1
        )
        metrics
            .record(saved)
}
",
    );
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::DeferBlock { statements, .. }) = &flow.body()[0] else {
        panic!("expected defer block");
    };
    assert_eq!(statements.len(), 2);
    assert!(matches!(&statements[0], Stmt::Let { .. }));
    assert!(matches!(
        &statements[1],
        Stmt::Expr(Expr::MethodCall { method, .. }) if method == "record"
    ));
}

#[test]
fn speaker_preset_call_arguments_are_typed_expressions() {
    let expr = parse_expr("alice(face=.smile, voice=auto, window=@textbox:.side)")
        .expect("speaker preset argument list parses");
    let Expr::Call { callee, args } = expr else {
        panic!("expected call expression");
    };
    assert!(matches!(callee.as_ref(), Expr::Path(path) if path == "alice"));
    assert_eq!(args.len(), 3);
    assert!(args.iter().all(|arg| matches!(arg, CallArg::Named { .. })));
    assert!(
        matches!(&args[0], CallArg::Named { value, .. } if matches!(value.as_ref(), Expr::Path(path) if path == ".smile"))
    );
}

#[test]
fn call_arguments_keep_positional_spread_nodes() {
    let expr =
        parse_expr("log_info(\"loaded\", fields...)").expect("positional spread argument parses");
    let Expr::Call { args, .. } = expr else {
        panic!("expected call expression");
    };

    assert_eq!(args.len(), 2);
    assert!(matches!(
        &args[1],
        CallArg::Spread { value } if matches!(value.as_ref(), Expr::Path(path) if path == "fields")
    ));
}

#[test]
fn at_is_entity_ref_and_slash_comments_are_comments() {
    let tree = parse_ok(
        r"
// ordinary comment
flow @flow.opening opening {
    goto @flow.title
}
",
    );
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    assert_eq!(flow.id().expect("flow id").body(), "flow.opening");

    let tree = parse_ok("// ordinary comment only");
    assert!(tree.items().is_empty());
}

#[test]
fn doc_comments_attach_to_function_and_parameters() {
    let tree = parse_ok(
        r#"
/// Opens a route.
pub fn open_route(
    /// Current game state.
    state: GameState,
) -> ! {
    panic("todo")
}
"#,
    );
    let Item::Function(function) = &tree.items()[0] else {
        panic!("expected function");
    };
    assert_eq!(
        function.doc().expect("function doc").text(),
        "Opens a route."
    );
    assert_eq!(
        function.signature().param_groups()[0].params()[0]
            .doc()
            .expect("param doc")
            .text(),
        "Current game state."
    );
    assert!(matches!(
        function.signature().return_type(),
        Some(TypeRef::Never)
    ));
}

#[test]
fn field_and_index_are_structured_for_later_typechecking() {
    let expr = parse_expr("state.affection[@character.alice]").expect("field index parses");
    let Expr::Index { target, index } = expr else {
        panic!("expected index");
    };
    assert_eq!(field_path(&target), Some("state.affection".to_owned()));
    assert!(matches!(index.as_ref(), Expr::EntityRef(_)));

    assert!(matches!(
        parse_type_ref("!").expect("never type parses"),
        TypeRef::Never
    ));
}

#[test]
fn array_types_and_repeat_literals_are_structured() {
    assert!(matches!(
        parse_type_ref("Array<i32, 3>").expect("array type parses"),
        TypeRef::Generic { base, args }
            if base == "Array"
                && args.len() == 2
                && matches!(&args[1], TypeRef::ConstInt(3))
    ));

    let expr = parse_expr("[0; 4]").expect("array repeat literal parses");
    assert!(matches!(
        expr,
        Expr::ArrayRepeat { value, len }
            if matches!(value.as_ref(), Expr::Literal(_))
                && matches!(len.as_ref(), Expr::Literal(_))
    ));
}

#[test]
fn large_flat_literal_sequences_parse_as_bracket_seq() {
    let values = (0..128)
        .map(|value| format!("{value}i64"))
        .collect::<Vec<_>>()
        .join(", ");
    let expr = parse_expr(&format!("[{values}]")).expect("large literal sequence parses");
    let Expr::NumericBracketSeq(seq) = expr else {
        panic!("expected numeric bracket sequence");
    };
    assert_eq!(seq.len(), 128);
    assert_eq!(seq.suffix(), Some("i64"));
    assert_eq!(seq.values()[0], 0);
    assert_eq!(seq.values()[127], 127);

    let repeat = parse_expr("[0i64; 4]").expect("array repeat still parses");
    assert!(matches!(repeat, Expr::ArrayRepeat { .. }));

    let indexed = parse_expr("[1i64, 2i64][0i64]").expect("literal sequence index parses");
    assert!(matches!(
        indexed,
        Expr::Index { target, index }
            if matches!(target.as_ref(), Expr::NumericBracketSeq(_))
                && matches!(index.as_ref(), Expr::Literal(Literal::Int { .. }))
    ));

    let mixed = parse_expr("[1i64, false]").expect("mixed sequence falls back");
    assert!(matches!(mixed, Expr::BracketSeq(_)));

    let mixed_suffix = parse_expr("[1i32, 2i64]").expect("mixed suffix sequence falls back");
    assert!(matches!(
        mixed_suffix,
        Expr::BracketSeq(items)
            if items.len() == 2
                && matches!(&items[0], Expr::Literal(Literal::Int { suffix, .. }) if suffix.as_deref() == Some("i32"))
                && matches!(&items[1], Expr::Literal(Literal::Int { suffix, .. }) if suffix.as_deref() == Some("i64"))
    ));
}

#[test]
fn float_suffix_and_unit_number_literals_are_typed_syntax() {
    let f32_lit = parse_expr("1.5f32").expect("f32 literal parses");
    assert!(matches!(
        f32_lit,
        Expr::Literal(Literal::Float {
            suffix: Some(arcweft_lang_syntax::expr::FloatSuffix::F32),
            ..
        })
    ));

    let f64_lit = parse_expr("1e3f64").expect("exponent f64 literal parses");
    assert!(matches!(
        f64_lit,
        Expr::Literal(Literal::Float {
            suffix: Some(arcweft_lang_syntax::expr::FloatSuffix::F64),
            ..
        })
    ));

    let pt_lit = parse_expr("12pt").expect("point unit literal parses");
    assert!(matches!(
        pt_lit,
        Expr::Literal(Literal::UnitNumber {
            suffix: arcweft_lang_syntax::expr::UnitNumberSuffix::Pt,
            ..
        })
    ));

    let rad_lit = parse_expr("2rad").expect("radian unit literal parses");
    assert!(matches!(
        rad_lit,
        Expr::Literal(Literal::UnitNumber {
            suffix: arcweft_lang_syntax::expr::UnitNumberSuffix::Rad,
            ..
        })
    ));

    assert!(parse_expr("1.0NaN").is_err());
    assert!(parse_expr("1.0Inf").is_err());
}

#[test]
fn flow_recovery_nodes_keep_family_and_source_range() {
    let tree = parse_ok(
        r"
flow @flow.raw_example {
    unknown surface form
}
",
    );
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Raw(raw) = &flow.body()[0] else {
        panic!("expected recovery node");
    };
    assert_eq!(raw.family(), RawSyntaxFamily::FlowItem);
    assert_eq!(raw.source(), "unknown surface form");
    assert!(raw.range().is_some());
}

#[test]
fn statement_recovery_nodes_keep_family_and_source_range() {
    let tree = parse_ok(
        r"
fn bad_stmt() -> Unit {
    let broken
}
",
    );
    let Item::Function(function) = &tree.items()[0] else {
        panic!("expected function");
    };
    let Stmt::Raw(raw) = &function.body_statements()[0] else {
        panic!("expected raw statement recovery node");
    };
    assert_eq!(raw.family(), RawSyntaxFamily::Stmt);
    assert_eq!(raw.source(), "let broken");
    assert!(raw.range().is_some());
}
