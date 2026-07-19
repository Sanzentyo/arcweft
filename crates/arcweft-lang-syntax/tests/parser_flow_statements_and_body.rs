use arcweft_lang_syntax::{
    ast::{
        flow::{AuthoredExpr, FlowItem, Stmt},
        items::Item,
    },
    expr::{ArgumentListTerminatorSyntax, CallRecoveryBoundarySyntax, CallRecoveryTokenKind, Expr},
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
            expr: Expr::Call(call),
            ..
        } if select_path(call.callee()) == Some("metrics.record".to_owned())
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

#[test]
fn function_tail_bracket_owner_retains_recovered_call_and_close_bracket() {
    let source = "fn demo() {\n    [f(α, β]\n}\n";
    let parsed = arcweft_lang_syntax::parser::parse_source(source);
    let Item::Function(function) = &parsed.typed_tree().items()[0] else {
        panic!("expected function");
    };
    let Some(Expr::BracketSeq(items)) = function.body_value().map(AuthoredExpr::expr) else {
        panic!("function tail must retain the bracket owner");
    };
    let [Expr::Call(call)] = items.as_slice() else {
        panic!("bracket item must retain the recovered typed call");
    };
    let boundary_start = source.find(']').expect("authored close bracket");
    let boundary = arcweft_lang_syntax::ast::common::TextRange::new(
        boundary_start,
        boundary_start + ']'.len_utf8(),
    );
    let argument_list = call
        .parenthesized_syntax()
        .expect("parenthesized syntax")
        .argument_list();

    assert_eq!(
        argument_list.terminator(),
        &ArgumentListTerminatorSyntax::RecoveredMissing {
            insertion: boundary_start,
            boundary: CallRecoveryBoundarySyntax::Token {
                kind: CallRecoveryTokenKind::CloseBracket,
                range: boundary,
            },
        }
    );
    assert_eq!(&source[boundary.as_range()], "]");
    let diagnostic = parsed
        .errors()
        .iter()
        .find(|error| error.message().contains("missing closing `)`"))
        .expect("full-source owner retains the recovery diagnostic");
    assert_eq!(
        *diagnostic.range(),
        arcweft_lang_syntax::ast::common::TextRange::new(boundary_start, boundary_start)
    );
    assert_eq!(diagnostic.recovery()[0].edits()[0].replacement(), ")");
}

#[test]
fn function_tail_record_owner_retains_recovered_call_and_close_brace() {
    let source = "fn demo() {\n    Record { value: f(α, β }\n}\n";
    let parsed = arcweft_lang_syntax::parser::parse_source(source);
    let Item::Function(function) = &parsed.typed_tree().items()[0] else {
        panic!("expected function");
    };
    let Some(Expr::Record { fields, .. }) = function.body_value().map(AuthoredExpr::expr) else {
        panic!("function tail must retain the record owner");
    };
    let [(_, Expr::Call(call))] = fields.as_slice() else {
        panic!("record value must retain the recovered typed call");
    };
    let boundary_start = source.find(" }\n}").expect("record close brace") + 1;
    let boundary = arcweft_lang_syntax::ast::common::TextRange::new(
        boundary_start,
        boundary_start + '}'.len_utf8(),
    );
    let argument_list = call
        .parenthesized_syntax()
        .expect("parenthesized syntax")
        .argument_list();

    assert_eq!(
        argument_list.terminator(),
        &ArgumentListTerminatorSyntax::RecoveredMissing {
            insertion: boundary_start,
            boundary: CallRecoveryBoundarySyntax::Token {
                kind: CallRecoveryTokenKind::CloseBrace,
                range: boundary,
            },
        }
    );
    assert_eq!(&source[boundary.as_range()], "}");
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.message().contains("missing closing `)`"))
    );
}

#[test]
fn function_statement_owner_retains_recovered_call_before_semicolon() {
    let source = "fn demo() {\n    f(α, β;\n    let next = next()\n    next\n}\n";
    let parsed = arcweft_lang_syntax::parser::parse_source(source);
    let Item::Function(function) = &parsed.typed_tree().items()[0] else {
        panic!("expected function");
    };
    let [
        Stmt::Expr {
            expr: Expr::Call(call),
            ..
        },
        Stmt::Let { .. },
    ] = function.body_statements()
    else {
        panic!(
            "semicolon owner must retain the recovered expression and following statement: {:?}",
            function.body_statements()
        );
    };
    let boundary_start = source.find(';').expect("authored statement semicolon");
    let boundary = arcweft_lang_syntax::ast::common::TextRange::new(
        boundary_start,
        boundary_start + ';'.len_utf8(),
    );
    let argument_list = call
        .parenthesized_syntax()
        .expect("parenthesized syntax")
        .argument_list();

    assert_eq!(
        argument_list.terminator(),
        &ArgumentListTerminatorSyntax::RecoveredMissing {
            insertion: boundary_start,
            boundary: CallRecoveryBoundarySyntax::Token {
                kind: CallRecoveryTokenKind::Semicolon,
                range: boundary,
            },
        }
    );
    assert_eq!(&source[boundary.as_range()], ";");
    assert!(matches!(
        function.body_value().map(AuthoredExpr::expr),
        Some(Expr::Path(path)) if path == "next"
    ));
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.message().contains("missing closing `)`"))
    );
}

#[test]
fn function_owner_rejects_invalid_call_without_publishing_typed_call() {
    for invalid in ["f(,x)", "f(x,,y)", "f(x y)", "f(name =)", "f(...)"] {
        let source = format!("fn demo() {{\n    {invalid}\n}}\n");
        let parsed = arcweft_lang_syntax::parser::parse_source(source);
        let Item::Function(function) = &parsed.typed_tree().items()[0] else {
            panic!("expected function");
        };

        assert!(
            matches!(
                function.body_value().map(AuthoredExpr::expr),
                Some(Expr::Raw(raw)) if raw == invalid
            ),
            "{invalid} must remain an invalid owner value rather than a typed CallExpr"
        );
        assert!(
            !parsed.errors().is_empty(),
            "{invalid} must retain its full-source parser error"
        );
    }
}
