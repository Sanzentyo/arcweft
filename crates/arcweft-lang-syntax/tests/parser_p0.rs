fn parse_ok(source: impl Into<String>) -> arcweft_lang_syntax::ast::items::TypedSyntaxTree {
    let parsed = arcweft_lang_syntax::parser::parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "expected source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed.into_typed_tree()
}

use arcweft_lang_syntax::{
    ast::{
        flow::{FlowItem, Stmt},
        items::{Item, RawSyntaxFamily},
    },
    expr::{BinaryOp, Expr, UnaryOp, parse_expr},
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
fn speaker_preset_call_arguments_are_typed_expressions() {
    let expr = parse_expr("alice(face=.smile, voice=auto, window=@textbox:.side)")
        .expect("speaker preset argument list parses");
    let Expr::Call { callee, args } = expr else {
        panic!("expected call expression");
    };
    assert!(matches!(callee.as_ref(), Expr::Path(path) if path == "alice"));
    assert_eq!(args.len(), 3);
    assert!(args.iter().all(|arg| matches!(arg, Expr::NamedArg { .. })));
    assert!(
        matches!(&args[0], Expr::NamedArg { value, .. } if matches!(value.as_ref(), Expr::Path(path) if path == ".smile"))
    );
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
