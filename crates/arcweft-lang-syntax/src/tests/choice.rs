use super::support::*;

#[test]
fn parses_choice_block_inside_flow() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    choice @choice.opening.first {
        @choice.opening.listen "聞いてみる" -> @flow.alice_intro
        @choice.opening.silent "黙っている" -> @flow.quiet_intro
    }
}
"#,
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Choice(choice) = &flow.body()[0] else {
        panic!("expected choice");
    };
    assert_eq!(
        choice.id().expect("choice id").body(),
        "choice.opening.first"
    );
    assert_eq!(choice.options().len(), 2);
    assert_eq!(choice.options()[0].label(), "聞いてみる");
}

#[test]
fn rejects_sigiled_choice_keyword_syntax() {
    let errors = parse_errors(
        r#"
flow @flow.opening opening {
    @choice @choice.opening.first {
        @choice.opening.listen "聞いてみる" -> @flow.alice_intro
    }
}
"#,
    );

    assert!(errors[0].message().contains("@choice"));
    assert!(
        errors[0]
            .expected()
            .iter()
            .any(|expected| expected == "choice @choice.id { ... }"),
    );
}

#[test]
fn parses_choice_option_with_condition() {
    let tree = parse_ok(
        r#"
choice @choice.opening.first {
    @choice.opening.listen "聞いてみる" if state.affection[@character.alice] >= 3 -> @flow.alice_intro
}
"#,
    );

    let Item::FlowItem(FlowItem::Choice(choice)) = &tree.items()[0] else {
        panic!("expected choice");
    };
    let option = &choice.options()[0];
    assert_eq!(
        option.id().expect("choice option id").body(),
        "choice.opening.listen"
    );
    assert!(matches!(option.condition(), Some(Expr::Binary { .. })));
    assert_eq!(
        option.target().expect("goto target").body(),
        "flow.alice_intro"
    );
}

#[test]
fn parses_choice_option_block_and_value_output() {
    let tree = parse_ok(
        r#"
choice @choice.opening.first {
    let can_enter_alice = state.affection[@character.alice] >= 3

    option @choice.opening.listen {
        label = "聞いてみる"
        enabled = can_enter_alice
        visible = true
        order = 10
        ui {
            disabled_reason = if can_enter_alice { None } else { Some("好感度が足りません") }
            badge = if can_enter_alice { None } else { Some("LOCKED") }
        }
        select {
            goto @flow.alice_intro
        }
    }

    @choice.opening.silent "黙っている" => @flow.quiet_intro
}
"#,
    );

    let Item::FlowItem(FlowItem::Choice(choice)) = &tree.items()[0] else {
        panic!("expected choice");
    };
    assert_eq!(choice.items().len(), 3);
    assert_eq!(choice.options().len(), 2);
    let option = &choice.options()[0];
    assert_eq!(option.label(), "聞いてみる");
    assert!(option.enabled().is_some());
    assert!(option.visible().is_some());
    assert!(option.order().is_some());
    assert_eq!(option.ui_fields().len(), 2);
    assert_eq!(
        option.target().expect("goto target").body(),
        "flow.alice_intro"
    );
    assert!(matches!(
        choice.options()[1].action(),
        ChoiceAction::Out(Expr::EntityRef(entity)) if entity.body() == "flow.quiet_intro"
    ));
}

#[test]
fn parses_dynamic_choice_options_from_for_loop() {
    let tree = parse_ok(
        r"
choice @choice.opening.routes {
    for route in opening_routes(state) {
        option route.choice_id {
            label = route.label
            enabled = route.enabled
            select { goto route.target }
        }
    }
}
",
    );

    let Item::FlowItem(FlowItem::Choice(choice)) = &tree.items()[0] else {
        panic!("expected choice");
    };
    assert!(matches!(&choice.items()[0], ChoiceItem::For { .. }));
    assert_eq!(choice.options().len(), 1);
    assert!(choice.options()[0].id_expr().is_some());
}

#[test]
fn parses_choice_match_items_and_collects_arm_options() {
    let tree = parse_ok(
        r#"
choice @choice.opening.first {
    match state.route_override {
        .Some(route) when route_enabled => {
            @.listen "聞いてみる" -> @flow.alice_intro
        }
        _ => {
            @.silent "黙っている" -> @flow.quiet_intro
        }
    }
}
"#,
    );

    let Item::FlowItem(FlowItem::Choice(choice)) = &tree.items()[0] else {
        panic!("expected choice");
    };
    let ChoiceItem::Match { expr, arms } = &choice.items()[0] else {
        panic!("expected choice match item");
    };
    assert!(expr_path_eq(expr, "state.route_override"));
    assert_eq!(arms.len(), 2);
    assert!(arms[0].guard().is_some());
    assert!(matches!(
        arms[0].items().first(),
        Some(ChoiceItem::Option(option)) if option.label() == "聞いてみる"
    ));
    assert_eq!(choice.options().len(), 2);
    assert_eq!(
        choice.options()[0].target().expect("listen target").body(),
        "flow.alice_intro"
    );
    assert_eq!(
        choice.options()[1].target().expect("silent target").body(),
        "flow.quiet_intro"
    );

    let hir = lower_to_hir(&tree).expect("choice match lowers");
    validate_typecheck_ready(&hir).expect("choice match is typecheck-ready");
    let env = TypeCheckEnv::new()
        .with_symbol(
            "state.route_override",
            TypeKind::Named("Option<String>".to_owned()),
        )
        .with_symbol("route_enabled", TypeKind::Bool);
    typecheck_hir(&hir, &env).expect("choice match options typecheck");
}

#[test]
fn choice_body_raw_items_are_not_typecheck_ready() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    choice @choice.opening.first {
        unknown choice body syntax
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("choice with raw item lowers");
    let errors = validate_typecheck_ready(&hir).expect_err("raw choice item is rejected");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("raw expression")
                && error.message().contains("unknown choice body syntax"))
    );
}

#[test]
fn parses_choice_plan_option_in_sugar_label_key_and_value() {
    let tree = parse_ok(
        r#"
choice @choice.opening.routes {
    option route in opening_routes(state) {
        id = route.choice_id
        label(id=@text.choice.opening.route) = route.label
        value = route.target
        enabled = route.enabled
        select { out route.target }
    }
}
with {
    window = @choice_window.main
    layout = vertical
    default_focus = @choice.opening.listen
    timeout 10s { select @choice.opening.silent }
    cancel on input .BackToTitle { return Ok(FlowExit::Goto(@flow.title)) }
    on select selected { log info "selected {id:?}" { id = selected.id } }
}
"#,
    );

    let Item::FlowItem(FlowItem::Choice(choice)) = &tree.items()[0] else {
        panic!("expected choice");
    };
    let plan = choice.plan().expect("choice plan");
    assert!(matches!(&plan.items()[0], ChoicePlanItem::Option { name, .. } if name == "window"));
    assert!(matches!(&plan.items()[3], ChoicePlanItem::Timeout { .. }));
    assert!(matches!(&plan.items()[4], ChoicePlanItem::Cancel { .. }));
    assert!(matches!(&plan.items()[5], ChoicePlanItem::OnSelect { .. }));
    assert!(matches!(
        &plan.items()[3],
        ChoicePlanItem::Timeout { body, .. }
            if matches!(body.first(), Some(Stmt::Select(Expr::EntityRef(_))))
    ));
    assert!(matches!(
        &plan.items()[4],
        ChoicePlanItem::Cancel { body, .. }
            if matches!(body.first(), Some(Stmt::Return(Expr::Call { .. })))
    ));
    assert!(matches!(
        &plan.items()[5],
        ChoicePlanItem::OnSelect { body, .. }
            if matches!(body.first(), Some(Stmt::Expr(Expr::Call { .. })))
    ));
    assert!(matches!(&choice.items()[0], ChoiceItem::For { .. }));
    let option = &choice.options()[0];
    assert!(option.label_text_key().is_some());
    assert!(option.value().is_some());
    assert!(matches!(option.action(), ChoiceAction::Out(_)));
}

#[test]
fn typechecks_dynamic_choice_option_fields_in_for_sugar() {
    let tree = parse_ok(
        r"
choice @choice.opening.routes {
    option route in opening_routes(state) {
        id = route.choice_id
        label(id=@text.choice.opening.route) = route.label
        value = route.target
        enabled = route.enabled
        visible = route.visible
        order = route.order

        ui {
            disabled_reason = route.disabled_reason
            badge = route.badge
        }

        select { out route.target }
    }
}
",
    );

    let hir = lower_to_hir(&tree).expect("dynamic choice option fixture lowers");
    validate_typecheck_ready(&hir).expect("dynamic choice option fixture is typecheck-ready");
    let env = TypeCheckEnv::new()
        .with_symbol("state", TypeKind::Named("GameState".to_owned()))
        .with_function(
            "opening_routes",
            TypeKind::Named("List<RouteChoice>".to_owned()),
        );
    typecheck_hir(&hir, &env).expect("dynamic choice option fields typecheck");
}

#[test]
fn rejects_dynamic_id_in_compact_choice_arm() {
    let tree = parse_ok(
        r#"
choice @choice.opening.routes {
    route.choice_id "Dynamic label" -> @flow.alice_intro
}
"#,
    );

    let hir = lower_to_hir(&tree).expect("choice with dynamic compact arm lowers");
    let errors =
        validate_typecheck_ready(&hir).expect_err("dynamic compact arm is not typecheck-ready");

    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("raw expression is not ready for type checking")),
        "expected raw choice item diagnostic, got {errors:?}"
    );
}

#[test]
fn rejects_bare_dot_ids_in_id_bearing_choice_contexts() {
    let errors = parse_errors(
        r#"
flow @flow.opening opening {
    choice .first {
        .listen "聞いてみる" -> @flow.alice_intro
    }
}
"#,
    );

    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("relative IDs must start with `@.`")),
        "expected bare relative ID diagnostic, got {errors:?}"
    );
}

#[test]
fn typechecks_choice_plan_structured_bodies() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    choice @choice.opening.first {
        @choice.opening.listen "聞いてみる" -> @flow.alice_intro
    }
    with {
        timeout 10s { select @choice.opening.listen }
        cancel on input .BackToTitle { return Ok(FlowExit::Goto(@flow.title)) }
        on select selected { log info "selected {id:?}" { id = selected.id } }
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("choice plan lowers");
    validate_typecheck_ready(&hir).expect("choice plan bodies have structured expressions");
    let env = TypeCheckEnv::new()
        .with_function("Ok", TypeKind::Named("Result".to_owned()))
        .with_function("FlowExit::Goto", TypeKind::Named("FlowExit".to_owned()))
        .with_function("log.info", TypeKind::Unit);
    typecheck_hir(&hir, &env).expect("choice plan bodies typecheck");
}

#[test]
fn typechecks_choice_option_select_block_statements() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    choice @choice.opening.first {
        option @choice.opening.listen {
            label = "聞いてみる"
            select {
                if can_emit {
                    emit GameEvent::ChoiceSelected { id = @choice.opening.listen }
                }
                match selected_route {
                    .Some(route) => out route
                    _ => out @flow.title
                }
            }
        }
    }
}
"#,
    );
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Choice(choice) = &flow.body()[0] else {
        panic!("expected choice");
    };
    assert!(matches!(
        choice.options()[0].action(),
        ChoiceAction::SelectBlock(statements)
            if matches!(
                statements.as_slice(),
                [Stmt::If { .. }, Stmt::Match { .. }]
            )
    ));

    let hir = lower_to_hir(&tree).expect("choice option select block lowers");
    validate_typecheck_ready(&hir).expect("choice option select block is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol(
                "GameEvent::ChoiceSelected",
                TypeKind::Named("GameEvent".to_owned()),
            )
            .with_symbol("can_emit", TypeKind::Bool)
            .with_symbol(
                "selected_route",
                TypeKind::Named("Option<Ref<Flow>>".to_owned()),
            ),
    )
    .expect("typecheck succeeds");
}

#[test]
fn lowers_named_scope_and_relative_choice_ids() {
    let tree = parse_ok(
        r#"
mod crate::game::routes::opening
use self::characters::{alice}
use parent::common::{route_gate}

flow @flow.opening opening {
    scope dream {
        choice @.first {
            @.listen "聞いてみる" -> @flow.alice_intro
            @.silent "黙っている" -> @flow.quiet_intro
        }
    }
}

flow @flow.alice_intro alice_intro {}
flow @flow.quiet_intro quiet_intro {}
"#,
    );

    assert_eq!(
        tree.module().expect("module").path(),
        "crate::game::routes::opening"
    );
    assert_eq!(tree.uses()[0].tree(), "self::characters::{alice}");
    assert_eq!(tree.uses()[1].tree(), "super::common::{route_gate}");

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Scope(scope) = &flow.body()[0] else {
        panic!("expected named scope");
    };
    assert_eq!(scope.name(), Some("dream"));
    let FlowItem::Choice(choice) = &scope.body()[0] else {
        panic!("expected scoped choice");
    };
    assert!(choice.id().expect("choice id").is_relative());
    assert!(choice.options()[0].id().expect("option id").is_relative());

    let hir = lower_to_hir(&tree).expect("relative choice ids lower");
    let HirFlowItem::Scope(scope) = &hir.flows()[0].body()[0] else {
        panic!("expected HIR scope");
    };
    let HirFlowItem::Choice(choice) = &scope.body()[0] else {
        panic!("expected HIR choice");
    };
    assert_eq!(
        choice.id().expect("normalized choice id").body(),
        "choice.opening.dream.first"
    );
    assert_eq!(
        choice.options()[0]
            .id()
            .expect("normalized option id")
            .body(),
        "choice.opening.dream.first.listen"
    );

    let registry = registry_from_hir(&hir);
    validate_hir_references(&hir, &registry).expect("normalized scoped ids resolve");
    validate_typecheck_ready(&hir).expect("scoped relative ids are typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("can_enter", TypeKind::Bool),
    )
    .expect("typecheck succeeds");
}

#[test]
fn lowers_choice_expression_let_binding() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    let next_flow = choice @.first {
        @.listen "聞いてみる" => @flow.alice_intro
        @.silent "黙っている" => @flow.quiet_intro
    }

    goto next_flow
}
flow @flow.alice_intro alice_intro {
}

flow @flow.quiet_intro quiet_intro {
}
"#,
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected source flow");
    };
    let FlowItem::Stmt(Stmt::LetChoice { pattern, choice }) = &flow.body()[0] else {
        panic!("expected AST choice expression binding");
    };
    assert_eq!(pattern, &Pattern::Ident("next_flow".to_owned()));
    assert!(choice.id().expect("choice id").is_relative());

    let hir = lower_to_hir(&tree).expect("choice expression fixture lowers");
    let HirFlowItem::LetChoice { pattern, choice } = &hir.flows()[0].body()[0] else {
        panic!("expected HIR choice expression binding");
    };
    assert_eq!(pattern, &Pattern::Ident("next_flow".to_owned()));
    assert_eq!(
        choice.id().expect("normalized choice id").body(),
        "choice.opening.first"
    );
    assert_eq!(
        choice.options()[0]
            .id()
            .expect("normalized first option id")
            .body(),
        "choice.opening.first.listen"
    );
    assert!(matches!(
        choice.options()[0].action(),
        ChoiceAction::Out(Expr::EntityRef(entity)) if entity.body() == "flow.alice_intro"
    ));

    let registry = registry_from_hir(&hir);
    validate_hir_references(&hir, &registry).expect("choice expression refs resolve");
    validate_typecheck_ready(&hir).expect("choice expression is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("choice expression typechecks");
}

#[test]
fn lowers_current_and_parent_relative_choice_ids() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    scope outer {
        scope inner {
            choice @...first {
                @.listen "聞いてみる" -> @flow.alice_intro
            }
        }
    }
}

flow @flow.alice_intro alice_intro {}
"#,
    );

    let hir = lower_to_hir(&tree).expect("parent relative choice ids lower");
    let HirFlowItem::Scope(outer) = &hir.flows()[0].body()[0] else {
        panic!("expected outer scope");
    };
    let HirFlowItem::Scope(inner) = &outer.body()[0] else {
        panic!("expected inner scope");
    };
    let HirFlowItem::Choice(choice) = &inner.body()[0] else {
        panic!("expected choice");
    };

    assert_eq!(
        choice.id().expect("normalized choice id").body(),
        "choice.opening.first"
    );
    assert_eq!(
        choice.options()[0]
            .id()
            .expect("normalized option id")
            .body(),
        "choice.opening.first.listen"
    );
}
