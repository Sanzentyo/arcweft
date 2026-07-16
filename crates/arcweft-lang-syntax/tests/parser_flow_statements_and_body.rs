use arcweft_lang_syntax::{
    ast::{
        flow::{AuthoredExpr, FlowItem, Stmt},
        items::Item,
    },
    expr::Expr,
};

fn parse_ok(source: impl Into<String>) -> arcweft_lang_syntax::ast::items::TypedSyntaxTree {
    let parsed = arcweft_lang_syntax::parser::parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "expected source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed.into_typed_tree()
}

fn select_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => Some(path.as_label().to_owned()),
        Expr::Select(select) => Some(format!(
            "{}.{}",
            select_path(select.target())?,
            select.member().as_str()
        )),
        _ => None,
    }
}

#[test]
fn entry_goto_is_the_structured_flow_dispatch_item() {
    let tree = parse_ok(
        r#"
entry game @entry.main {
    state = GameState
    initializer = initial_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.opening
}

flow @flow.opening opening {
    return "ok"
}
"#,
    );
    let arcweft_lang_syntax::ast::items::Item::Entry(entry) = &tree.items()[0] else {
        panic!("expected entry");
    };
    let target = entry
        .items()
        .iter()
        .find_map(|item| match item {
            arcweft_lang_syntax::ast::items::EntryItem::Goto(target) => Some(target),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected goto entry item: {:?}", entry.items()));
    assert_eq!(target.body(), "flow.opening");
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
        Stmt::Expr {
            expr: Expr::Call { callee, .. },
            ..
        } if select_path(callee) == Some("metrics.record".to_owned())
    ));
}

#[test]
fn flow_receive_action_statement_is_structured() {
    let tree = parse_ok(
        r"
pub action feedback.submit(value: String)

flow test {
  let event = receive action(@action:.feedback.submit)
  return event.value
}
",
    );
    let flow = tree
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Flow(flow) => Some(flow),
            _ => None,
        })
        .expect("flow parsed");

    let FlowItem::Stmt(Stmt::LetActionReceive { action, .. }) = &flow.body()[0] else {
        panic!("expected receive action statement");
    };
    assert!(
        matches!(action.expr(), Expr::EntityRef(reference) if reference.canonical_body() == "action.feedback.submit")
    );
}

#[test]
fn flow_if_comparison_condition_is_structured() {
    let tree = parse_ok(
        r#"
flow main {
    let count = 3usize
    if count < 5usize {
        narrator: short.
    }
    return "done"
}
"#,
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    assert!(
        flow.body()
            .iter()
            .any(|item| matches!(item, FlowItem::If(_)))
    );
}

#[test]
fn value_if_else_if_is_nested_if_not_raw_recovery() {
    let tree = parse_ok(
        r#"
fn label(i: i32) -> String {
    if i == 0 {
        return "first"
    } else if i == 1 {
        return "second"
    } else {
        return "last"
    }
}
"#,
    );
    let Item::Function(function) = &tree.items()[0] else {
        panic!("expected function");
    };
    assert!(function.body_statements().is_empty());
    let Some(Expr::If {
        else_branch: Some(else_branch),
        ..
    }) = function.body_value().map(AuthoredExpr::expr)
    else {
        panic!("expected final if expression");
    };
    assert!(matches!(
        else_branch.as_ref(),
        Expr::If {
            else_branch: Some(_),
            ..
        }
    ));
}
