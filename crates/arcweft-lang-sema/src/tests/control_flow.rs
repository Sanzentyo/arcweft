use super::support::*;

#[test]
fn typecheck_rejects_locals_escaping_named_and_bare_scopes() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    scope rain {
        let scoped_name = true
    }

    {
        let block_name = true
    }

    scope {
        let unnamed_scope_name = true
    }

    let from_scope = scoped_name
    let from_block = block_name
    let from_unnamed_scope = unnamed_scope_name
}
",
    );

    let hir = lower_to_hir(&tree).expect("scope escape fixture lowers");
    validate_typecheck_ready(&hir).expect("scope escape fixture is typecheck-ready");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect_err("scoped locals must not escape named or bare lexical scopes");
    let messages = errors
        .iter()
        .map(|error| error.message().to_owned())
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("unknown symbol `scoped_name`")),
        "expected scoped_name to be unavailable outside scope, got {messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("unknown symbol `block_name`")),
        "expected block_name to be unavailable outside block, got {messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("unknown symbol `unnamed_scope_name`")),
        "expected unnamed_scope_name to be unavailable outside explicit unnamed scope, got {messages:?}"
    );
}

#[test]
fn lowers_scope_expression_let_binding() {
    let tree = parse_ok(
        r"
flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    let can_enter = scope alice_route_check {
        let affection_ok = state.affection[@character.alice] >= 3
        let has_key = state.inventory.contains(@item.alice_key)
        affection_ok && has_key
    }

    if can_enter {
        goto @flow.alice_intro
    }
}

flow @flow.alice_intro alice_intro {
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected source flow");
    };
    let FlowItem::Stmt(Stmt::LetScope { pattern, scope }) = &flow.body()[0] else {
        panic!("expected AST scope expression binding");
    };
    assert_eq!(pattern, &Pattern::Ident("can_enter".to_owned()));
    assert_eq!(scope.name(), Some("alice_route_check"));
    assert_eq!(scope.statements().len(), 2);
    assert!(scope.value().is_some());

    let hir = lower_to_hir(&tree).expect("scope expression fixture lowers");
    let HirFlowItem::LetScope { pattern, scope } = &hir.flows()[0].body()[0] else {
        panic!("expected HIR scope expression binding");
    };
    assert_eq!(pattern, &Pattern::Ident("can_enter".to_owned()));
    assert_eq!(scope.name(), Some("alice_route_check"));
    assert_eq!(scope.statements().len(), 2);
    assert!(scope.value().is_some());

    let registry = registry_from_hir(&hir)
        .with_entity("character.alice", EntityKind::Character)
        .with_entity("item.alice_key", EntityKind::Other("item".to_owned()));
    validate_hir_references(&hir, &registry).expect("scope expression refs resolve");
    validate_typecheck_ready(&hir).expect("scope expression is typecheck-ready");
    let env = TypeCheckEnv::new()
        .with_symbol(
            "state.affection",
            TypeKind::Named("OrderedMap<Character, i64>".to_owned()),
        )
        .with_symbol("state.inventory", TypeKind::Named("Inventory".to_owned()))
        .with_method(
            TypeKind::Named("Inventory".to_owned()),
            "contains",
            TypeKind::Bool,
        )
        .with_index(
            TypeKind::Named("OrderedMap<Character, i64>".to_owned()),
            TypeKind::I64,
        );
    typecheck_hir(&hir, &env).expect("scope expression typechecks");
}

#[test]
fn lowers_unnamed_scope_expression_let_binding() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let ready = scope {
        let local = true
        local
    }

    if ready {
        goto @flow.next
    }
}

flow @flow.next next {
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected source flow");
    };
    let FlowItem::Stmt(Stmt::LetScope { pattern, scope }) = &flow.body()[0] else {
        panic!("expected AST scope expression binding");
    };
    assert_eq!(pattern, &Pattern::Ident("ready".to_owned()));
    assert_eq!(scope.name(), None);
    assert_eq!(scope.statements().len(), 1);
    assert!(scope.value().is_some());

    let hir = lower_to_hir(&tree).expect("unnamed scope expression fixture lowers");
    let HirFlowItem::LetScope { pattern, scope } = &hir.flows()[0].body()[0] else {
        panic!("expected HIR scope expression binding");
    };
    assert_eq!(pattern, &Pattern::Ident("ready".to_owned()));
    assert_eq!(scope.name(), None);
    assert_eq!(scope.statements().len(), 1);
    assert!(scope.value().is_some());

    let registry = registry_from_hir(&hir);
    validate_hir_references(&hir, &registry).expect("unnamed scope expression refs resolve");
    validate_typecheck_ready(&hir).expect("unnamed scope expression is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("unnamed scope expression typechecks");
}

#[test]
fn parses_and_typechecks_plain_block_expression_binding() {
    let tree = parse_ok(
        r"
flow @flow.block_expr block_expr {
    let total = {
        let a = 1i32
        let b = 2i32
        a + b
    }
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::Let {
        pattern,
        expr: Expr::Block { statements, value },
        ..
    }) = &flow.body()[0]
    else {
        panic!("expected let binding with block expression");
    };
    assert_eq!(pattern, &Pattern::Ident("total".to_owned()));
    assert_eq!(statements.len(), 2);
    assert!(value.is_some());

    let hir = lower_to_hir(&tree).expect("plain block expression fixture lowers");
    validate_typecheck_ready(&hir).expect("plain block expression is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("plain block expression typechecks");
}

#[test]
fn parses_and_typechecks_let_else_binding() {
    let tree = parse_ok(
        r"
flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    let .Some(route) = state.route_override else {
        goto @flow.title
    }

    goto route
}

flow @flow.title title {
}
",
    );
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected source flow");
    };
    let FlowItem::Stmt(Stmt::LetElse {
        pattern,
        expr,
        else_body,
        ..
    }) = &flow.body()[0]
    else {
        panic!("expected structured let-else");
    };
    assert!(variant_tuple_binding(pattern, "Some", "route"));
    assert!(expr_path_eq(expr.expr(), "state.route_override"));
    assert!(matches!(else_body.as_slice(), [Stmt::Goto(_)]));

    let hir = lower_to_hir(&tree).expect("let-else fixture lowers");
    let registry = registry_from_hir(&hir);
    validate_hir_references(&hir, &registry).expect("let-else refs resolve");
    validate_typecheck_ready(&hir).expect("let-else is typecheck-ready");
    let env = TypeCheckEnv::new().with_symbol(
        "state.route_override",
        TypeKind::Named("Option<Ref<Flow>>".to_owned()),
    );
    typecheck_hir(&hir, &env).expect("let-else typechecks and binds route");
}

#[test]
fn typecheck_rejects_non_diverging_let_else() {
    let tree = parse_ok(
        r"
flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    let .Some(route) = state.route_override else {
        @flow.title
    }
}

flow @flow.title title {
}
",
    );
    let hir = lower_to_hir(&tree).expect("non-diverging let-else fixture lowers");
    let env = TypeCheckEnv::new().with_symbol(
        "state.route_override",
        TypeKind::Named("Option<Ref<Flow>>".to_owned()),
    );
    let errors = typecheck_hir(&hir, &env).expect_err("non-diverging let-else is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("let-else else block must leave the current continuation")
    }));
}

#[test]
fn typecheck_rejects_out_outside_line_plan_scope() {
    let tree = parse_ok(
        r"
flow @flow.bad_out bad_out {
    out .Done
}
",
    );
    let hir = lower_to_hir(&tree).expect("flow out fixture lowers");
    validate_typecheck_ready(&hir).expect("flow out fixture is typecheck-ready");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("flow-level out is rejected");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("`out` can only be used")),
        "expected flow-level out rejection, got {errors:?}"
    );
}

#[test]
fn typechecks_let_else_panic_and_fail_as_diverging() {
    for diverging in [
        r#"panic("missing route")"#,
        "fail(.MissingRoute)",
        r#"bail("missing route")"#,
    ] {
        let source = format!(
            r"
flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {{
    let .Some(route) = state.route_override else {{
        {diverging}
    }}

    goto route
}}
"
        );
        let tree = parse_ok(source);
        let hir = lower_to_hir(&tree).expect("diverging let-else fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol(
                "state.route_override",
                TypeKind::Named("Option<Ref<Flow>>".to_owned()),
            )
            .with_symbol(".MissingRoute", TypeKind::Named("ErrorKind".to_owned()));
        typecheck_hir(&hir, &env).expect("panic/fail let-else branches diverge");
    }
}

#[test]
fn parses_and_typechecks_bail_and_ensure_calls() {
    let tree = parse_ok(
        r#"
flow @flow.validate validate {
    ensure(score >= 0, "score must be non-negative")
    if !valid {
        bail("invalid score")
    }
    goto @flow.title
}
"#,
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    assert!(matches!(
        &flow.body()[0],
        FlowItem::Stmt(Stmt::Expr { expr: _, .. })
    ));
    let hir = lower_to_hir(&tree).expect("bail and ensure fixture lowers");
    validate_typecheck_ready(&hir).expect("bail and ensure are typecheck-ready");
    let env = TypeCheckEnv::new()
        .with_symbol("score", TypeKind::I64)
        .with_symbol("valid", TypeKind::Bool);
    typecheck_hir(&hir, &env).expect("bail and ensure typecheck");
}

#[test]
fn parses_and_typechecks_result_computation_block_binding() {
    let tree = parse_ok(
        r#"
flow @flow.compute compute {
    let route = result {
        let id = parse_choice_id(raw)?
        ensure(id_valid, "choice id must be valid")
        Ok(@flow.title)
    }
    goto @flow.title
}
"#,
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::Let {
        expr:
            Expr::ComputationBlock {
                kind,
                statements,
                value: Some(_),
            },
        ..
    }) = &flow.body()[0]
    else {
        panic!("expected result computation block binding");
    };
    assert_eq!(kind, &ComputationBlockKind::Result);
    assert_eq!(statements.len(), 2);

    let hir = lower_to_hir(&tree).expect("result computation block fixture lowers");
    validate_typecheck_ready(&hir).expect("result computation block is typecheck-ready");
    let env = TypeCheckEnv::new()
        .with_symbol("raw", TypeKind::String)
        .with_symbol("id_valid", TypeKind::Bool)
        .with_function(
            "parse_choice_id",
            TypeKind::Result {
                ok: Box::new(TypeKind::String),
                error: Box::new(TypeKind::Named("ParseError".to_owned())),
            },
        )
        .with_function(
            "Ok",
            TypeKind::Result {
                ok: Box::new(TypeKind::entity_ref(EntityKind::Flow)),
                error: Box::new(TypeKind::Named("ArcError".to_owned())),
            },
        );
    typecheck_hir(&hir, &env).expect("result computation block typechecks");
}

#[test]
fn parses_and_typechecks_stream_computation_block_binding() {
    let tree = parse_ok(
        r"
flow @flow.stream stream_example {
    let levels = stream {
        for frame in frames {
            yield rms(frame)
        }
    }
    goto @flow.title
}

flow @flow.title title {}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::Let {
        expr:
            Expr::ComputationBlock {
                kind,
                statements,
                value: None,
            },
        ..
    }) = &flow.body()[0]
    else {
        panic!("expected stream computation block binding");
    };
    assert_eq!(kind, &ComputationBlockKind::Stream);
    assert_eq!(statements.len(), 1);

    let hir = lower_to_hir(&tree).expect("stream computation block fixture lowers");
    validate_typecheck_ready(&hir).expect("stream computation block is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol(
                "frames",
                TypeKind::Stream {
                    item: Box::new(TypeKind::Named("Frame".to_owned())),
                    error: Box::new(TypeKind::Named("CaptureError".to_owned())),
                },
            )
            .with_function("rms", TypeKind::I64),
    )
    .expect("typecheck succeeds");
}

#[test]
fn typecheck_rejects_yield_outside_generation_context() {
    let tree = parse_ok(
        r"
flow @flow.bad bad {
    yield state
}
",
    );
    let hir = lower_to_hir(&tree).expect("yield fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("yield in flow is rejected");

    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("`yield` is only valid in `seq`, `stream`, or `source` contexts")
    }));
}

#[test]
fn typecheck_rejects_yield_in_dialogue_line_plan() {
    let tree = parse_ok(
        r"
flow @flow.bad bad {
    alice[待って。[p]]
    with:
        yield .Done
}
",
    );
    let hir = lower_to_hir(&tree).expect("line-plan yield fixture lowers");
    let env = TypeCheckEnv::new().with_symbol("alice", TypeKind::Speaker(EntityKind::Character));
    let errors = typecheck_hir(&hir, &env).expect_err("line-plan yield is rejected");

    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("`yield` cannot be used in a dialogue line plan")
    }));
}

#[test]
fn typecheck_rejects_non_bool_ensure_condition() {
    let tree = parse_ok(
        r#"
flow @flow.validate validate {
    ensure(score, "score must be non-negative")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("non-bool ensure fixture lowers");
    let errors = typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("score", TypeKind::I64),
    )
    .expect_err("non-bool ensure condition is rejected");

    assert!(errors.iter().any(|error| {
        matches!(
            error.kind(),
            crate::diagnostics::TypeCheckErrorKind::ArgumentTypeMismatch {
                function,
                argument,
                expected: TypeKind::Bool,
                ..
            } if function == "ensure" && argument == "condition"
        )
    }));
}

#[test]
fn parses_and_typechecks_while_loop() {
    let tree = parse_ok(
        r"
flow @flow.loading loading {
    while loading {
        continue
    }
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::While(block) = &flow.body()[0] else {
        panic!("expected while block");
    };
    assert!(matches!(block.condition(), Expr::Path(path) if path == "loading"));

    let hir = lower_to_hir(&tree).expect("while fixture lowers");
    let HirFlowItem::While(block) = &hir.flows()[0].body()[0] else {
        panic!("expected HIR while block");
    };
    assert!(matches!(block.condition(), Expr::Path(path) if path == "loading"));

    validate_typecheck_ready(&hir).expect("while block is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("loading", TypeKind::Bool),
    )
    .expect("typecheck succeeds");
}

#[test]
fn parses_and_typechecks_if_let_guard_block() {
    let tree = parse_ok(
        r"
flow @flow.branching branching {
    if let .Some(route) = state.route_override when route_available {
        goto route
    }
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::IfLet(block) = &flow.body()[0] else {
        panic!("expected if-let block");
    };
    assert!(variant_tuple_binding(block.pattern(), "Some", "route"));
    assert!(expr_path_eq(block.expr(), "state.route_override"));
    assert!(block.guard().is_some());

    let hir = lower_to_hir(&tree).expect("if-let fixture lowers");
    let HirFlowItem::IfLet(block) = &hir.flows()[0].body()[0] else {
        panic!("expected HIR if-let block");
    };
    assert!(variant_tuple_binding(block.pattern(), "Some", "route"));

    validate_typecheck_ready(&hir).expect("if-let block is typecheck-ready");
    let env = TypeCheckEnv::new()
        .with_symbol(
            "state.route_override",
            TypeKind::Named("Option<Ref<Flow>>".to_owned()),
        )
        .with_symbol("route_available", TypeKind::Bool);
    typecheck_hir(&hir, &env).expect("if-let block typechecks and binds route in body");
}

#[test]
fn parses_and_typechecks_value_if_expression_binding() {
    let tree = parse_ok(
        r#"
flow @flow.branching branching {
    let face = if ready {
        "smile"
    } else {
        "worried"
    }
}
"#,
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::Let {
        pattern,
        expr:
            Expr::If {
                condition,
                then_branch,
                else_branch: Some(_),
            },
        ..
    }) = &flow.body()[0]
    else {
        panic!("expected let binding with value if expression");
    };
    assert_eq!(pattern, &Pattern::Ident("face".to_owned()));
    assert!(matches!(condition.as_ref(), Expr::Path(path) if path == "ready"));
    assert!(matches!(
        then_branch.as_ref(),
        Expr::Block {
            value: Some(value),
            ..
        } if matches!(value.as_ref(), Expr::Literal(Literal::String(value)) if value == "smile")
    ));

    let hir = lower_to_hir(&tree).expect("value if expression fixture lowers");
    validate_typecheck_ready(&hir).expect("value if expression is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("ready", TypeKind::Bool),
    )
    .expect("typecheck succeeds");
}

#[test]
fn typecheck_joins_value_if_branch_types_as_anonymous_sum() {
    let tree = parse_ok(
        r#"
flow @flow.branching branching {
    let face = if ready {
        "smile"
    } else {
        1i64
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("anonymous sum value if fixture lowers");
    let report = analyze_types(
        &hir,
        &TypeCheckEnv::new().with_symbol("ready", TypeKind::Bool),
    );
    assert!(
        report.diagnostics.is_empty(),
        "anonymous sum value if typechecks: {:?}",
        report.diagnostics
    );
    assert!(report.judgments.iter().any(|judgment| {
        judgment.subject
            == TypeJudgmentSubject::LetBinding {
                pattern: "Ident(\"face\")".to_owned(),
            }
            && judgment.ty == TypeKind::Choice(vec![TypeKind::String, TypeKind::I64])
    }));
}

#[test]
fn parses_and_typechecks_value_if_let_expression_binding() {
    let tree = parse_ok(
        r"
flow @flow.branching branching {
    let route = if let .Some(route) = state.route_override when route_enabled {
        route
    } else {
        @flow.title
    }
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::Let {
        pattern,
        expr:
            Expr::IfLet {
                pattern: binding,
                expr,
                guard: Some(_),
                then_branch,
                else_branch: Some(_),
            },
        ..
    }) = &flow.body()[0]
    else {
        panic!("expected let binding with value if-let expression");
    };
    assert_eq!(pattern, &Pattern::Ident("route".to_owned()));
    assert!(variant_tuple_binding(binding.as_ref(), "Some", "route"));
    assert!(expr_path_eq(expr.as_ref(), "state.route_override"));
    assert!(matches!(
        then_branch.as_ref(),
        Expr::Block {
            value: Some(value),
            ..
        } if matches!(value.as_ref(), Expr::Path(path) if path == "route")
    ));

    let hir = lower_to_hir(&tree).expect("value if-let expression fixture lowers");
    validate_typecheck_ready(&hir).expect("value if-let expression is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol(
                "state.route_override",
                TypeKind::Named("Option<Ref<Flow>>".to_owned()),
            )
            .with_symbol("route_enabled", TypeKind::Bool),
    )
    .expect("typecheck succeeds");
}

#[test]
fn value_if_let_guard_can_use_pattern_binding() {
    let tree = parse_ok(
        r"
flow @flow.branching branching {
    let chosen = if let .Some(value) = maybe when value > fallback {
        value
    } else {
        fallback
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("value if-let guard binding fixture lowers");
    validate_typecheck_ready(&hir).expect("value if-let guard binding is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("maybe", TypeKind::Option(Box::new(TypeKind::I64)))
            .with_symbol("fallback", TypeKind::I64),
    )
    .expect("value if-let guard sees pattern binding");
}

#[test]
fn typecheck_rejects_value_if_let_non_bool_guard() {
    let tree = parse_ok(
        r"
flow @flow.branching branching {
    let route = if let .Some(route) = state.route_override when route_count {
        route
    } else {
        @flow.title
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("non-bool value if-let fixture lowers");
    let errors = typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol(
                "state.route_override",
                TypeKind::Named("Option<Ref<Flow>>".to_owned()),
            )
            .with_symbol("route_count", TypeKind::I64),
    )
    .expect_err("non-bool value if-let guard is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("if-let expression guard must have type bool")
    }));
}

#[test]
fn parses_and_typechecks_value_match_expression_binding() {
    let tree = parse_ok(
        r"
flow @flow.branching branching {
    let route = match selected {
        @choice.opening.listen when can_listen => @flow.alice_intro
        @choice.opening.silent => @flow.quiet_intro
        _ => @flow.title
    }
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::Let {
        pattern,
        expr: Expr::Match { scrutinee, arms },
        ..
    }) = &flow.body()[0]
    else {
        panic!("expected let binding with value match expression");
    };
    assert_eq!(pattern, &Pattern::Ident("route".to_owned()));
    assert!(matches!(scrutinee.as_ref(), Expr::Path(path) if path == "selected"));
    assert_eq!(arms.len(), 3);
    assert!(arms[0].guard().is_some());
    assert!(matches!(
        arms[0].value(),
        Expr::EntityRef(entity) if entity.body() == "flow.alice_intro"
    ));

    let hir = lower_to_hir(&tree).expect("value match expression fixture lowers");
    validate_typecheck_ready(&hir).expect("value match expression is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("selected", TypeKind::entity_ref(EntityKind::ChoiceOption))
            .with_symbol("can_listen", TypeKind::Bool),
    )
    .expect("typecheck succeeds");
}

#[test]
fn typecheck_joins_value_match_branch_types_as_anonymous_sum() {
    let tree = parse_ok(
        r#"
flow @flow.branching branching {
    let route = match selected {
        @choice.opening.listen => @flow.alice_intro
        _ => "fallback"
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("anonymous sum value match fixture lowers");
    let report = analyze_types(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("selected", TypeKind::entity_ref(EntityKind::ChoiceOption)),
    );
    assert!(
        report.diagnostics.is_empty(),
        "anonymous sum value match typechecks: {:?}",
        report.diagnostics
    );
    assert!(report.judgments.iter().any(|judgment| {
        judgment.subject
            == TypeJudgmentSubject::LetBinding {
                pattern: "Ident(\"route\")".to_owned(),
            }
            && judgment.ty
                == TypeKind::Choice(vec![
                    TypeKind::entity_ref(EntityKind::Flow),
                    TypeKind::String,
                ])
    }));
}

#[test]
fn parses_and_typechecks_postfix_try_expression() {
    let tree = parse_ok(
        r"
flow @flow.trying trying {
    let config = load_config()?
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::Let {
        pattern,
        expr: Expr::Try { expr },
        ..
    }) = &flow.body()[0]
    else {
        panic!("expected let binding with postfix try expression");
    };
    assert_eq!(pattern, &Pattern::Ident("config".to_owned()));
    assert!(matches!(expr.as_ref(), Expr::Call(_)));

    let hir = lower_to_hir(&tree).expect("postfix try fixture lowers");
    validate_typecheck_ready(&hir).expect("postfix try expression is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_function(
            "load_config",
            TypeKind::Result {
                ok: Box::new(TypeKind::Named("Config".to_owned())),
                error: Box::new(TypeKind::Named("ConfigError".to_owned())),
            },
        ),
    )
    .expect("typecheck succeeds");
}

#[test]
fn parses_and_typechecks_prefix_try_expression() {
    let tree = parse_ok(
        r"
flow @flow.trying trying {
    let config = try load_config()
}
",
    );
    let hir = lower_to_hir(&tree).expect("prefix try fixture lowers");
    validate_typecheck_ready(&hir).expect("prefix try expression is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_function(
            "load_config",
            TypeKind::Named("Result<Config, Error>".to_owned()),
        ),
    )
    .expect("typecheck succeeds");
}

#[test]
fn typecheck_rejects_non_bool_if_let_guard() {
    let tree = parse_ok(
        r"
flow @flow.branching branching {
    if let .Some(route) = state.route_override when route_count {
        goto route
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("non-bool if-let guard fixture lowers");
    let env = TypeCheckEnv::new()
        .with_symbol(
            "state.route_override",
            TypeKind::Named("Option<Ref<Flow>>".to_owned()),
        )
        .with_symbol("route_count", TypeKind::I64);
    let errors = typecheck_hir(&hir, &env).expect_err("non-bool if-let guard is rejected");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("if-let guard must have type bool"))
    );
}

#[test]
fn parses_and_typechecks_while_let_loop() {
    let tree = parse_ok(
        r"
flow @flow.events events {
    while let .Some(event) = next_event when event_ready {
        goto event
    }
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::WhileLet(block) = &flow.body()[0] else {
        panic!("expected while-let block");
    };
    assert!(variant_tuple_binding(block.pattern(), "Some", "event"));
    assert!(matches!(block.expr(), Expr::Path(path) if path == "next_event"));
    assert!(block.guard().is_some());

    let hir = lower_to_hir(&tree).expect("while-let fixture lowers");
    let HirFlowItem::WhileLet(block) = &hir.flows()[0].body()[0] else {
        panic!("expected HIR while-let block");
    };
    assert!(variant_tuple_binding(block.pattern(), "Some", "event"));

    validate_typecheck_ready(&hir).expect("while-let block is typecheck-ready");
    let env = TypeCheckEnv::new()
        .with_symbol(
            "next_event",
            TypeKind::Named("Option<Ref<Flow>>".to_owned()),
        )
        .with_symbol("event_ready", TypeKind::Bool);
    typecheck_hir(&hir, &env).expect("while-let block typechecks");
}

#[test]
fn typecheck_rejects_non_bool_while_condition() {
    let tree = parse_ok(
        r"
flow @flow.loading loading {
    while loading_count {
        continue
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("non-bool while fixture lowers");
    let errors = typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("loading_count", TypeKind::I64),
    )
    .expect_err("non-bool while condition is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("while condition must have type bool")
    }));
}

#[test]
fn parses_and_typechecks_loop_expression_binding() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let next = 'events: loop {
        break 'events @flow.title
    }

    goto next
}

flow @flow.title title {
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::LetLoop { pattern, block }) = &flow.body()[0] else {
        panic!("expected loop expression binding");
    };
    assert_eq!(pattern, &Pattern::Ident("next".to_owned()));
    assert_eq!(block.label(), Some("events"));
    assert!(matches!(
        block.body(),
        [FlowItem::Stmt(Stmt::Break { label: Some(label), expr: Some(expr) })]
            if label == "events"
                && matches!(expr.expr(), Expr::EntityRef(entity) if entity.body() == "flow.title")
    ));

    let hir = lower_to_hir(&tree).expect("loop expression fixture lowers");
    let HirFlowItem::LetLoop { pattern, block } = &hir.flows()[0].body()[0] else {
        panic!("expected HIR loop expression binding");
    };
    assert_eq!(pattern, &Pattern::Ident("next".to_owned()));
    assert_eq!(block.label(), Some("events"));
    assert!(matches!(
        block.body(),
        [HirFlowItem::Stmt(Stmt::Break { label: Some(label), expr: Some(expr) })]
            if label == "events"
                && matches!(expr.expr(), Expr::EntityRef(entity) if entity.body() == "flow.title")
    ));

    let registry = registry_from_hir(&hir);
    validate_hir_references(&hir, &registry).expect("loop expression refs resolve");
    validate_typecheck_ready(&hir).expect("loop expression is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("loop expression typechecks");
}

#[test]
fn typecheck_rejects_break_value_in_while() {
    let tree = parse_ok(
        r"
flow @flow.loading loading {
    while is_loading {
        break @flow.title
    }
}

flow @flow.title title {
}
",
    );
    let hir = lower_to_hir(&tree).expect("while break-value fixture lowers");
    let errors = typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("is_loading", TypeKind::Bool),
    )
    .expect_err("break expr in while is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("break expr is allowed only in loop")
    }));
}

#[test]
fn typecheck_rejects_break_outside_loop() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    break
}
",
    );
    let hir = lower_to_hir(&tree).expect("bare break fixture lowers");
    let errors =
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("break outside loops is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("break is only allowed inside loop")
    }));
}

#[test]
fn typecheck_rejects_unresolved_control_transfer_labels() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let next = 'events: loop {
        if done {
            break 'missing @flow.title
        }
        continue 'missing
    }

    alice[
        聞いて。[p]
    ]
    with 'line {
        cancel on input(.SkipLine) { out 'missing .Skipped }
    }
}

flow @flow.title title {}
",
    );
    let hir = lower_to_hir(&tree).expect("unresolved label fixture lowers");
    validate_typecheck_ready(&hir).expect("unresolved label fixture is typecheck-ready");
    let errors = typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("done", TypeKind::Bool)
            .with_symbol(".Skipped", TypeKind::Named("LineExit".to_owned())),
    )
    .expect_err("unresolved labels are rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("break label `'missing` does not name an active loop")
    }));
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("continue label `'missing` does not name an active loop")
    }));
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("out label `'missing` does not name an active line-plan scope")
    }));
}

#[test]
fn parses_for_and_select_flow_blocks() {
    let tree = parse_ok(
        r"
flow @flow.stream stream {
    for c in choices {
        option(c.id, label = c.label)
    }
    select {
        audio = frames.next? => {
            signal.set(@signal.voice_level, audio.rms)
        }

        frame _ => {
            scene.show(@scene.listening)
            continue
        }

        event .Back => {
            close frames
            return Ok(FlowExit.Goto(@flow.title))
        }
    }
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::For(for_block) = &flow.body()[0] else {
        panic!("expected for block");
    };
    assert!(matches!(for_block.pattern(), Pattern::Ident(name) if name == "c"));
    assert!(matches!(for_block.source(), Expr::Path(path) if path == "choices"));
    assert!(matches!(
        &for_block.body()[0],
        FlowItem::Stmt(Stmt::Expr {
            expr: Expr::Call(_),
            ..
        })
    ));

    let FlowItem::Select(select) = &flow.body()[1] else {
        panic!("expected select block");
    };
    assert_eq!(select.branches().len(), 3);
    assert!(matches!(
        select.branches()[0].head(),
        SelectBranchHead::Bind {
            name,
            propagates_error: true,
            ..
        } if name == "audio"
    ));
    assert!(matches!(
        select.branches()[1].head(),
        SelectBranchHead::Frame(Pattern::Discard)
    ));
    assert!(matches!(
        select.branches()[2].head(),
        SelectBranchHead::Event(Pattern::Variant { name, payload: None, .. }) if name == "Back"
    ));

    let hir = lower_to_hir(&tree).expect("for and select lower");
    assert!(matches!(&hir.flows()[0].body()[0], HirFlowItem::For(_)));
    assert!(matches!(&hir.flows()[0].body()[1], HirFlowItem::Select(_)));
    validate_typecheck_ready(&hir).expect("for and select are typecheck-ready");
}

#[test]
fn typecheck_rejects_borrow_across_yield_thread_and_defer_boundaries() {
    for boundary in [
        "yield frame",
        "thread load_avatar { load_avatar() }",
        "defer cleanup()",
    ] {
        let tree = parse_ok(format!(
            r"
flow @flow.borrow borrow {{
    let pixels: &'asset [Rgba8] = bg.pixels()
    {boundary}
}}
"
        ));
        let hir = lower_to_hir(&tree).expect("borrow boundary fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
            .with_symbol("frame", TypeKind::Named("Frame".to_owned()))
            .with_function("load_avatar", TypeKind::Named("Task".to_owned()))
            .with_function("cleanup", TypeKind::Unit)
            .with_method(
                TypeKind::Named("ImageHandle".to_owned()),
                "pixels",
                TypeKind::Named("Pixels".to_owned()),
            );
        let errors = typecheck_hir(&hir, &env).expect_err("borrow cannot cross boundary");

        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("suspension boundary")),
            "expected suspension-boundary error for {boundary}"
        );
    }
}

#[test]
fn parses_if_and_match_flow_blocks_for_hir() {
    let tree = parse_ok(
        r"
flow @flow.branching branching {
    if state.ready {
        goto @flow.ready
    }
    match next {
        None => goto @flow.title
        _ => goto @flow.fallback
    }
}

",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    assert!(
        matches!(&flow.body()[0], FlowItem::If(block) if expr_path_eq(block.condition(), "state.ready"))
    );
    assert!(matches!(&flow.body()[1], FlowItem::Match(block) if block.arms().len() == 2));

    let hir = lower_to_hir(&tree).expect("if and match lower");
    assert!(matches!(&hir.flows()[0].body()[0], HirFlowItem::If(_)));
    assert!(matches!(&hir.flows()[0].body()[1], HirFlowItem::Match(_)));
}

#[test]
fn parses_flow_statement_if_else_blocks() {
    let source = r#"
flow @flow.main main(input: i32) -> String {
    if input > 0 {
        return "ok"
    } else {
        bail("Input must be greater than 0")
    }
}
"#;
    let parsed = parse_source(source.to_owned());
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let tree = parsed.into_typed_tree();
    let hir = lower_to_hir(&tree).expect("if-else lowers");
    let HirFlowItem::If(block) = &hir.flows()[0].body()[0] else {
        panic!("expected if block");
    };
    assert_eq!(block.body().len(), 1);
    assert_eq!(block.else_body().len(), 1);
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("if-else typechecks");
}

#[test]
fn typechecks_if_and_match_flow_blocks() {
    let tree = parse_ok(
        r"
flow @flow.branching branching {
    if !state.ready {
        goto @flow.ready
    }
    match next {
        None => goto @flow.title
        _ => goto @flow.fallback
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("if and match fixture lowers");
    let env = TypeCheckEnv::new()
        .with_symbol("state.ready", TypeKind::Bool)
        .with_symbol("next", TypeKind::Named("Option<Ref<Flow>>".to_owned()));

    typecheck_hir(&hir, &env).expect("if and match fixture typechecks");
}

#[test]
fn typechecks_statement_match_arm_guards_and_bindings() {
    let tree = parse_ok(
        r"
flow @flow.branching branching {
    match state.route_override {
        .Some(route) when route_enabled => goto route
        _ => goto @flow.title
    }
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Match(block) = &flow.body()[0] else {
        panic!("expected statement match block");
    };
    assert!(block.arms()[0].guard().is_some());

    let hir = lower_to_hir(&tree).expect("guarded match fixture lowers");
    validate_typecheck_ready(&hir).expect("guarded match is typecheck-ready");
    let env = TypeCheckEnv::new()
        .with_symbol(
            "state.route_override",
            TypeKind::Named("Option<Ref<Flow>>".to_owned()),
        )
        .with_symbol("route_enabled", TypeKind::Bool);
    typecheck_hir(&hir, &env).expect("guarded match binds route and typechecks goto");
}

#[test]
fn typecheck_rejects_statement_match_non_bool_guard() {
    let tree = parse_ok(
        r"
flow @flow.branching branching {
    match state.route_override {
        .Some(route) when route_count => goto route
        _ => goto @flow.title
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("non-bool guarded match fixture lowers");
    let env = TypeCheckEnv::new()
        .with_symbol(
            "state.route_override",
            TypeKind::Named("Option<Ref<Flow>>".to_owned()),
        )
        .with_symbol("route_count", TypeKind::I64);
    let errors = typecheck_hir(&hir, &env).expect_err("non-bool match guard is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("match arm guard must have type bool")
    }));
}
