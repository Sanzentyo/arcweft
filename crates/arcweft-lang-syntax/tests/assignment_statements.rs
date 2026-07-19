use arcweft_lang_syntax::{
    ast::{
        flow::Stmt,
        items::{ImplMember, Item},
    },
    expr::Expr,
    parser::parse_source,
    types::FnReceiverKind,
};

#[test]
fn parses_impl_assignment_tail_if_without_raw_fallback() {
    let parsed = parse_source(include_str!(
        "../../../fixtures/iterator-witness/user-defined.arcw"
    ));
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let tree = parsed.typed_tree();
    let (signature, value) = tree
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Impl(item) if item.target() == "CounterIter" => {
                item.members().iter().find_map(|member| match member {
                    ImplMember::Function {
                        signature,
                        body_value: Some(value),
                        ..
                    } if signature.name() == "next" => Some((signature, value)),
                    _ => None,
                })
            }
            _ => None,
        })
        .expect("CounterIter::next method body is parsed");

    let receiver = &signature.param_groups()[0].params()[0];
    assert_eq!(receiver.receiver_kind(), Some(FnReceiverKind::MutRef));

    let Expr::If {
        then_branch,
        else_branch: Some(_),
        ..
    } = value.expr()
    else {
        panic!("expected tail if expression, got {value:?}");
    };
    let Expr::Block {
        statements,
        value: Some(_),
    } = then_branch.as_ref()
    else {
        panic!("expected then branch block, got {then_branch:?}");
    };
    assert!(matches!(
        statements.as_slice(),
        [Stmt::Let { .. }, Stmt::Assign { .. }]
    ));
}

#[test]
fn parses_flow_body_assignment_without_raw_fallback() {
    let parsed = parse_source(
        r"
flow @flow.assignment_demo {
    let out = Box { index: 0 }
    out.index = out.index + 1
}
",
    );
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let tree = parsed.typed_tree();
    let Some(Item::Flow(flow)) = tree.items().first() else {
        panic!("expected flow item");
    };
    assert!(matches!(
        flow.body(),
        [
            _,
            arcweft_lang_syntax::ast::flow::FlowItem::Stmt(Stmt::Assign { .. })
        ]
    ));
}

#[test]
fn function_tail_control_roles_follow_typed_statement_ownership() {
    let parsed = parse_source(
        r"
fn tail_match(value: i64) -> i64 {
    match value {
        0 => 1
        _ => 2
    }
}

fn terminal_loop() {
    loop {
        break
    }
}

fn terminal_select(value: i64) {
    select value
}
",
    );
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let tree = parsed.typed_tree();
    let function = |name: &str| {
        tree.items().iter().find_map(|item| match item {
            Item::Function(function) if function.signature().name() == name => Some(function),
            _ => None,
        })
    };

    let tail_match = function("tail_match").expect("tail_match function");
    assert!(matches!(
        tail_match
            .body_value()
            .map(arcweft_lang_syntax::ast::flow::AuthoredExpr::expr),
        Some(Expr::Match { .. })
    ));
    assert!(tail_match.body_statements().is_empty());

    let terminal_loop = function("terminal_loop").expect("terminal_loop function");
    assert!(terminal_loop.body_value().is_none());
    assert!(matches!(
        terminal_loop.body_statements(),
        [Stmt::Loop { .. }]
    ));

    let terminal_select = function("terminal_select").expect("terminal_select function");
    assert!(terminal_select.body_value().is_none());
    assert!(matches!(
        terminal_select.body_statements(),
        [Stmt::Select(_)]
    ));
}
