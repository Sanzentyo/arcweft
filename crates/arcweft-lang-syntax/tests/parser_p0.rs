fn parse_ok(source: impl Into<String>) -> arcweft_lang_syntax::TypedSyntaxTree {
    let parsed = arcweft_lang_syntax::parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "expected source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed.into_typed_tree()
}

use arcweft_lang_syntax::{
    BinaryOp, Expr, FlowItem, Item, TypeRef, UnaryOp, parse_expr, parse_type_ref,
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
        FlowItem::Stmt(arcweft_lang_syntax::Stmt::Let {
            expr: Expr::DialogueCall { .. },
            ..
        })
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
    panic "todo"
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
