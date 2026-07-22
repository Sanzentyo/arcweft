use arcweft_lang_syntax::{
    ast::{flow::Stmt, pattern::Pattern},
    expr::{CallArg, Expr, parse_expr},
    types::{AuthoredTypeRef, TypeRef},
};

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

fn assert_selected_call<'a>(expr: &'a Expr, path: &str) -> &'a [CallArg] {
    let Expr::Call(call) = expr else {
        panic!("expected call expression: {expr:?}");
    };
    assert_eq!(select_path(call.callee()), Some(path.to_owned()));
    call.args()
}

#[test]
fn speaker_preset_call_arguments_are_typed_expressions() {
    let expr = parse_expr("alice(face=.smile, voice=auto, view=@view:.side)")
        .expect("speaker preset argument list parses");
    let Expr::Call(call) = expr else {
        panic!("expected call expression");
    };
    assert!(matches!(call.callee(), Expr::Path(path) if path == "alice"));
    assert_eq!(call.args().len(), 3);
    assert!(
        call.args()
            .iter()
            .all(|arg| matches!(arg, CallArg::Named { .. }))
    );
    assert!(
        matches!(&call.args()[0], CallArg::Named { value, .. } if matches!(value.as_ref(), Expr::ShortVariant(path) if path == "smile"))
    );
}

#[test]
fn call_arguments_keep_positional_spread_nodes() {
    let expr =
        parse_expr("log_info(\"loaded\", fields...)").expect("positional spread argument parses");
    let Expr::Call(call) = expr else {
        panic!("expected call expression");
    };

    assert_eq!(call.args().len(), 2);
    assert!(matches!(
        &call.args()[1],
        CallArg::Spread { value } if matches!(value.as_ref(), Expr::Path(path) if path == "fields")
    ));
}

#[test]
fn postfix_callback_block_lowers_to_selected_call_closure_arg() {
    let expr = parse_expr(
        r#"Button("Send").on_click { action.invoke(@action:.feedback.submit, value = name.text) }"#,
    )
    .expect("callback block parses");
    let Expr::Call(call) = expr else {
        panic!("expected selected callback call");
    };
    let Expr::Select(select) = call.callee() else {
        panic!("expected selected callee");
    };
    assert_eq!(select.member().as_str(), "on_click");
    assert!(matches!(select.target(), Expr::Call(_)));
    let [CallArg::Positional(closure)] = call.args() else {
        panic!("expected single closure arg: {:?}", call.args());
    };
    let Expr::Closure { params, body, .. } = closure.as_ref() else {
        panic!("expected closure arg: {closure:?}");
    };
    assert!(params.is_empty());
    assert!(matches!(
        body.as_ref(),
        Expr::Block {
            statements,
            value: Some(value),
        } if statements.is_empty()
            && matches!(value.as_ref(), Expr::Call(call)
                if select_path(call.callee()) == Some("action.invoke".to_owned()))
    ));
}

#[test]
fn postfix_callback_block_supports_parameterized_closure_after_call_select_unification() {
    let expr = parse_expr("items.map { item, index => item.label(index) }")
        .expect("parameterized callback block parses");
    let args = assert_selected_call(&expr, "items.map");

    let [CallArg::Positional(closure)] = args else {
        panic!("expected single closure arg: {args:?}");
    };
    let Expr::Closure { params, body, .. } = closure.as_ref() else {
        panic!("expected closure arg: {closure:?}");
    };
    assert_eq!(
        params
            .iter()
            .map(|param| param.simple_ident())
            .collect::<Vec<_>>(),
        vec![Some("item"), Some("index")]
    );
    assert!(matches!(
        body.as_ref(),
        Expr::Block {
            statements,
            value: Some(value),
        } if statements.is_empty()
            && matches!(value.as_ref(), Expr::Call(call)
                if select_path(call.callee()) == Some("item.label".to_owned()))
    ));
}

#[test]
fn closures_keep_pattern_and_type_ascription_parameters() {
    let expr = parse_expr("|(item, index): Pair| item.label(index)")
        .expect("typed pattern closure parses");
    let Expr::Closure { params, body, .. } = expr else {
        panic!("expected closure");
    };

    assert_eq!(params.len(), 1);
    assert!(matches!(
        params[0].pattern(),
        Pattern::Tuple(items) if items.len() == 2
    ));
    assert!(matches!(
        params[0].ty(),
        Some(ty) if matches!(ty.value(), TypeRef::Path(path) if path.canonical_string() == "Pair")
    ));
    assert!(matches!(
        body.as_ref(),
        Expr::Call(call) if select_path(call.callee()) == Some("item.label".to_owned())
    ));
}

#[test]
fn closures_keep_explicit_return_type_and_block_body() {
    let expr = parse_expr(
        r"
|score: i32| -> bool {
    score >= 80
}
",
    )
    .expect("return-typed closure parses");
    let Expr::Closure {
        params,
        return_type,
        body,
        ..
    } = expr
    else {
        panic!("expected closure");
    };

    assert_eq!(params[0].simple_ident(), Some("score"));
    assert!(matches!(
        return_type,
        Some(ty) if matches!(ty.value(), TypeRef::Path(path) if path.canonical_string() == "bool")
    ));
    assert!(matches!(body.as_ref(), Expr::Block { .. }));
}

#[test]
fn zero_arg_closure_keeps_explicit_return_type() {
    let expr = parse_expr(
        r#"
|| -> String {
    "now"
}
"#,
    )
    .expect("zero-arg return-typed closure parses");
    let Expr::Closure {
        params,
        return_type,
        body,
        ..
    } = expr
    else {
        panic!("expected closure");
    };

    assert!(params.is_empty());
    assert!(matches!(
        return_type,
        Some(ty) if matches!(ty.value(), TypeRef::Path(path) if path.canonical_string() == "String")
    ));
    assert!(matches!(body.as_ref(), Expr::Block { .. }));
}

#[test]
fn call_arg_closure_keeps_explicit_return_type() {
    let expr = parse_expr("items.filter(|choice: Choice| -> bool { choice.enabled })")
        .expect("return-typed closure arg parses");
    let args = assert_selected_call(&expr, "items.filter");
    let [CallArg::Positional(closure)] = args else {
        panic!("expected closure arg: {args:?}");
    };
    let Expr::Closure {
        params,
        return_type,
        body,
        ..
    } = closure.as_ref()
    else {
        panic!("expected closure arg: {closure:?}");
    };

    assert_eq!(params[0].simple_ident(), Some("choice"));
    assert!(matches!(
        return_type,
        Some(ty) if matches!(ty.value(), TypeRef::Path(path) if path.canonical_string() == "bool")
    ));
    assert!(matches!(body.as_ref(), Expr::Block { .. }));
}

#[test]
fn parenthesized_closure_can_be_called_immediately() {
    let expr = parse_expr(r#"(|name: String| -> String { name })("arc")"#)
        .expect("parenthesized closure call parses");
    let Expr::Call(call) = expr else {
        panic!("expected closure call");
    };
    assert!(matches!(
        call.callee(),
        Expr::Closure {
            params,
            return_type,
            body,
            ..
        } if params.len() == 1
            && matches!(return_type.as_ref().map(AuthoredTypeRef::value), Some(TypeRef::Path(path)) if path.canonical_string() == "String")
            && matches!(body.as_ref(), Expr::Block { .. })
    ));
    assert!(matches!(
        call.args(),
        [CallArg::Positional(value)] if matches!(value.as_ref(), Expr::Literal(_))
    ));
}

#[test]
fn parenthesized_zero_arg_closure_can_be_called_immediately() {
    let expr = parse_expr(
        r#"
(|| -> String {
    "arc"
})()
"#,
    )
    .expect("parenthesized zero-arg closure call parses");
    let Expr::Call(call) = expr else {
        panic!("expected zero-arg closure call");
    };
    assert!(matches!(
        call.callee(),
        Expr::Closure {
            params,
            return_type,
            body,
            ..
        } if params.is_empty()
            && matches!(return_type.as_ref().map(AuthoredTypeRef::value), Some(TypeRef::Path(path)) if path.canonical_string() == "String")
            && matches!(body.as_ref(), Expr::Block { .. })
    ));
    assert!(call.args().is_empty());
}

#[test]
fn closure_return_type_requires_block_body() {
    let error = parse_expr("|score: i32| -> bool score >= 80")
        .expect_err("return-typed closure without block is rejected");
    assert!(
        error
            .to_string()
            .contains("closure return type annotation requires a block body"),
        "unexpected error: {error}"
    );
}

#[test]
fn callback_block_closure_keeps_typed_parameters() {
    let expr =
        parse_expr("items.map { item: Label => item.text }").expect("typed callback block parses");
    let args = assert_selected_call(&expr, "items.map");
    let [CallArg::Positional(closure)] = args else {
        panic!("expected closure arg: {args:?}");
    };
    let Expr::Closure { params, body, .. } = closure.as_ref() else {
        panic!("expected closure arg: {closure:?}");
    };

    assert_eq!(params[0].simple_ident(), Some("item"));
    assert!(matches!(
        params[0].ty(),
        Some(ty) if matches!(ty.value(), TypeRef::Path(path) if path.canonical_string() == "Label")
    ));
    assert!(matches!(body.as_ref(), Expr::Block { .. }));
}

#[test]
fn postfix_callback_block_preserves_multi_statement_body() {
    let expr = parse_expr(
        r#"Button("Send").on_click {
  let label = name.text
  action.invoke(@action:.feedback.submit, value = label)
}"#,
    )
    .expect("multi-statement callback block parses");
    let Expr::Call(call) = expr else {
        panic!("expected selected callback call");
    };
    let Expr::Select(select) = call.callee() else {
        panic!("expected selected callee");
    };

    assert_eq!(select.member().as_str(), "on_click");
    let [CallArg::Positional(closure)] = call.args() else {
        panic!("expected single closure arg: {:?}", call.args());
    };
    let Expr::Closure { params, body, .. } = closure.as_ref() else {
        panic!("expected closure arg: {closure:?}");
    };
    assert!(params.is_empty());
    let Expr::Block {
        statements,
        value: Some(value),
    } = body.as_ref()
    else {
        panic!("expected callback body block, got {body:?}");
    };
    assert!(matches!(statements.as_slice(), [Stmt::Let { .. }]));
    assert!(matches!(
        value.as_ref(),
        Expr::Call(call) if select_path(call.callee()) == Some("action.invoke".to_owned())
    ));
}
