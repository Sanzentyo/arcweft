use super::support::*;

#[test]
fn parses_colon_speaker_with_indented_line_plan() {
    let tree = parse_source(
        r"
alice(voice=auto, face=smile):
    今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
with:
    at(0.42s): alice.stage.face(worried)
    cancel on input .SkipLine => continue
    out (actor, voice)
",
    )
    .expect("speaker line with line plan parses");

    let Item::FlowItem(FlowItem::SpeakerLine(line)) = &tree.items()[0] else {
        panic!("expected top-level speaker flow item");
    };
    assert_eq!(line.speaker(), "alice");
    assert_eq!(line.content().tokens().len(), 4);
    assert!(line.plan().is_some());
    let plan = line.plan().expect("line plan");
    assert!(matches!(&plan.items()[0], LinePlanItem::TimedCue { .. }));
    let LinePlanItem::CancelRule(rule) = &plan.items()[1] else {
        panic!("expected cancel rule");
    };
    assert_eq!(rule.trigger(), "input .SkipLine");
    assert!(matches!(rule.action(), [Stmt::Continue { label: None }]));
    assert!(matches!(&plan.items()[2], LinePlanItem::Out(_)));
}

#[test]
fn parses_bracket_speaker_call_with_with_colon_plan() {
    let tree = parse_source(
        r"
alice[
    おはよう。[p]
]
with:
    at(0.42s): alice.stage.face(smile)
",
    )
    .expect("bracket speaker call with with-colon plan parses");

    let Item::FlowItem(FlowItem::ContentCall(call)) = &tree.items()[0] else {
        panic!("expected content call");
    };
    assert_eq!(call.callee(), "alice");
    assert_eq!(call.content().raw(), "おはよう。[p]");
    let plan = call.plan().expect("line plan");
    assert!(matches!(
        &plan.items()[0],
        LinePlanItem::TimedCue {
            anchor: Expr::Literal(_),
            body: Expr::MethodCall { .. }
        }
    ));
}

#[test]
fn parses_same_line_line_plan_attachments() {
    let tree = parse_source(
        r"
flow #flow.opening opening {
    alice.say()[聞いて。[p]] with: out (voice, face)
    alice.say()[もう一度。[p]] with 'line { out .Done }
    let handles = alice.say()[結果を返す。[p]] with: out (voice, face)
}
",
    )
    .expect("same-line with attachments parse");

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let [
        FlowItem::ContentCall(inline_call),
        FlowItem::ContentCall(brace_call),
        FlowItem::Stmt(Stmt::Let {
            expr:
                Expr::DialogueCall {
                    plan: Some(bound_plan),
                    ..
                },
            ..
        }),
    ] = flow.body()
    else {
        panic!("expected content calls and line result binding");
    };
    let inline_plan = inline_call.plan().expect("inline plan");
    assert!(matches!(
        inline_plan.items(),
        [LinePlanItem::Out(Expr::Tuple(items))] if items.len() == 2
    ));
    let plan = brace_call.plan().expect("brace plan");
    assert_eq!(plan.label(), Some("line"));
    assert!(matches!(
        plan.items(),
        [LinePlanItem::Out(Expr::Path(path))] if path == ".Done"
    ));
    assert!(matches!(
        bound_plan.items(),
        [LinePlanItem::Out(Expr::Tuple(items))] if items.len() == 2
    ));

    let hir = lower_to_hir(&tree).expect("same-line line plans lower");
    validate_typecheck_ready(&hir).expect("same-line line plans are typecheck-ready");
}

#[test]
fn parses_multiline_line_result_binding_with_plan() {
    let tree = parse_source(
        r"
flow #flow.opening opening {
    let handles = alice.say(voice=auto)[
        今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
    ]
    with:
        out true

    let result = try alice.say()[
        もう一度。[p]
    ] with: out .Done
}
",
    )
    .expect("multiline line result binding parses");

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let [
        FlowItem::Stmt(Stmt::Let {
            expr:
                Expr::DialogueCall {
                    plan: Some(bound_plan),
                    ..
                },
            ..
        }),
        FlowItem::Stmt(Stmt::Let {
            expr: Expr::Try { expr },
            ..
        }),
    ] = flow.body()
    else {
        panic!("expected multiline dialogue result bindings");
    };
    assert!(matches!(
        bound_plan.items(),
        [LinePlanItem::Out(Expr::Literal(
            crate::expr::Literal::Bool(true)
        ))]
    ));
    assert!(matches!(
        expr.as_ref(),
        Expr::DialogueCall {
            plan: Some(plan),
            ..
        } if matches!(plan.items(), [LinePlanItem::Out(Expr::Path(path))] if path == ".Done")
    ));

    let hir = lower_to_hir(&tree).expect("multiline line result bindings lower");
    validate_typecheck_ready(&hir).expect("multiline line result bindings are typecheck-ready");

    let check_tree = parse_source(
        r"
flow #flow.opening opening {
    let handles = alice.say(voice=auto)[
        今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
    ]
    with:
        out true

    let result = try alice.say(voice=auto)[
        キャンセルできる行です。[p]
    ]
    with:
        cancel on input .SkipLine:
            out Err(LineCancel::Skipped)

        out Ok(())
}
",
    )
    .expect("typecheck multiline line result binding parses");
    let check_hir =
        lower_to_hir(&check_tree).expect("typecheck multiline line result binding lowers");
    let env = TypeCheckEnv::new()
        .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
        .with_symbol("auto", TypeKind::Named("VoicePolicy".to_owned()))
        .with_symbol(
            "LineCancel::Skipped",
            TypeKind::Named("LineCancel".to_owned()),
        )
        .with_method(
            TypeKind::Ref(EntityKind::Character),
            "say",
            TypeKind::Named("DialogueLine".to_owned()),
        );
    typecheck_hir(&check_hir, &env).expect("multiline line result binding typechecks");
}

#[test]
fn rejects_at_bracket_timed_cue_as_raw_line_plan_item() {
    let tree = parse_source(
        r"
alice[おはよう。[p]]
with:
    at(0.42s)[alice.stage.face(worried)]
",
    )
    .expect("old at bracket cue parses lossily");

    let Item::FlowItem(FlowItem::ContentCall(call)) = &tree.items()[0] else {
        panic!("expected content call");
    };
    let plan = call.plan().expect("line plan");
    assert!(matches!(&plan.items()[0], LinePlanItem::Raw(_)));

    let hir = lower_to_hir(&tree).expect("lossy line plan still lowers");
    let errors = validate_typecheck_ready(&hir).expect_err("old at bracket cue is rejected");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("raw expression")
                && error.message().contains("at(0.42s)["))
    );
}

#[test]
fn reports_unclosed_line_plan_block_after_cue() {
    let errors = parse_source(
        r"
alice[おはよう。[p]]
with {
    at(0.42s) { alice.stage.face(worried)
",
    )
    .expect_err("unclosed line plan fails");

    assert_eq!(errors.len(), 1);
    assert!(errors[0].message().contains("line plan"));
    assert!(!errors[0].recovery().is_empty());
}

#[test]
fn line_plan_items_keep_typed_expressions() {
    let tree = parse_source(
        r"
alice:
    聞いて。[p]
with:
    reveal = voice
    let voice = line.voice_handle()
    out (actor, voice)
",
    )
    .expect("line plan exprs parse");

    let Item::FlowItem(FlowItem::SpeakerLine(line)) = &tree.items()[0] else {
        panic!("expected speaker line");
    };
    let plan = line.plan().expect("line plan");
    assert!(matches!(
        &plan.items()[0],
        LinePlanItem::Option { value: Expr::Path(path), .. } if path == "voice"
    ));
    assert!(matches!(
        &plan.items()[1],
        LinePlanItem::Let {
            expr: Expr::MethodCall { .. },
            ..
        }
    ));
    assert!(matches!(
        &plan.items()[2],
        LinePlanItem::Out(Expr::Tuple(_))
    ));
}

#[test]
fn line_plan_cancel_actions_keep_typed_statements() {
    let tree = parse_source(
        r"
flow #flow.opening opening {
    alice[
        聞いて。[p]
    ]
    with 'line {
        cancel on input .SkipLine { out 'line .Skipped }
        cancel on input .BackToTitle => goto #flow.title
    }
}
",
    )
    .expect("line plan cancel actions parse");

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::ContentCall(call) = &flow.body()[0] else {
        panic!("expected content call");
    };
    let plan = call.plan().expect("line plan");
    assert_eq!(plan.label(), Some("line"));
    let LinePlanItem::CancelRule(skip_rule) = &plan.items()[0] else {
        panic!("expected skip cancel rule");
    };
    assert!(matches!(
        skip_rule.action(),
        [Stmt::Out { label: Some(label), expr: Expr::Path(path) }]
            if label == "line" && path == ".Skipped"
    ));
    let LinePlanItem::CancelRule(back_rule) = &plan.items()[1] else {
        panic!("expected back-to-title cancel rule");
    };
    assert!(matches!(
        back_rule.action(),
        [Stmt::Goto(Expr::EntityRef(target))] if target.body() == "flow.title"
    ));

    let hir = lower_to_hir(&tree).expect("line plan cancel actions lower");
    validate_typecheck_ready(&hir).expect("line plan cancel actions are typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
            .with_symbol(".Skipped", TypeKind::Named("LineExit".to_owned())),
    )
    .expect("line plan cancel actions typecheck");
}

#[test]
fn line_plan_cancel_commands_keep_structured_arguments() {
    let tree = parse_source(
        r"
flow #flow.opening opening {
    alice[
        聞いて。[p]
    ]
    with {
        cancel on input .SkipLine {
            stop voice fade=40ms
            flush text instant
            continue
        }
    }
}
",
    )
    .expect("line plan cancel commands parse");

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::ContentCall(call) = &flow.body()[0] else {
        panic!("expected content call");
    };
    let plan = call.plan().expect("line plan");
    let LinePlanItem::CancelRule(rule) = &plan.items()[0] else {
        panic!("expected cancel rule");
    };
    assert!(matches!(
        rule.action(),
        [Stmt::Command(stop), Stmt::Command(flush), Stmt::Continue { label: None }]
            if stop.name() == "stop" && flush.name() == "flush"
    ));

    let hir = lower_to_hir(&tree).expect("line plan cancel commands lower");
    validate_typecheck_ready(&hir).expect("line plan cancel commands are typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
            .with_symbol("voice", TypeKind::Named("VoiceHandle".to_owned()))
            .with_symbol("text", TypeKind::Named("DialogueText".to_owned()))
            .with_symbol("instant", TypeKind::Named("FlushPolicy".to_owned())),
    )
    .expect("line plan cancel commands typecheck");
}

#[test]
fn line_plan_assertions_keep_typed_conditions() {
    let tree = parse_source(
        r"
flow #flow.opening opening {
    alice:
        聞いて。[p]
    with {
        assert textbox_ready
        debug_assert route_count > 0
    }
}
",
    )
    .expect("line plan assertions parse");
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
        panic!("expected speaker line");
    };
    let plan = line.plan().expect("line plan");
    assert!(matches!(
        &plan.items()[0],
        LinePlanItem::Assert {
            debug: false,
            expr: Expr::Path(path)
        } if path == "textbox_ready"
    ));

    let hir = lower_to_hir(&tree).expect("line plan assertions lower");
    validate_typecheck_ready(&hir).expect("line plan assertions are typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
            .with_symbol("textbox_ready", TypeKind::Bool)
            .with_symbol("route_count", TypeKind::Int),
    )
    .expect("line plan assertions typecheck");
}

#[test]
fn line_plan_parallel_groups_keep_typed_items() {
    let tree = parse_source(
        r"
flow #flow.opening opening {
    alice:
        走って！[p]
    with {
        start {
            together {
                cue_move()
                cue_face()
                cue_se()
            }
        }
    }
}
",
    )
    .expect("line plan parallel groups parse");
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
        panic!("expected speaker line");
    };
    let plan = line.plan().expect("line plan");
    let [LinePlanItem::StartGroup(start_items)] = plan.items() else {
        panic!("expected start group");
    };
    let [LinePlanItem::TogetherGroup(together_items)] = start_items.as_slice() else {
        panic!("expected together group inside start group");
    };
    assert_eq!(together_items.len(), 3);
    assert!(matches!(
        &together_items[0],
        LinePlanItem::Expr(Expr::Call { .. })
    ));

    let hir = lower_to_hir(&tree).expect("line plan parallel groups lower");
    validate_typecheck_ready(&hir).expect("line plan parallel groups are typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
            .with_function("cue_move", TypeKind::Unit)
            .with_function("cue_face", TypeKind::Unit)
            .with_function("cue_se", TypeKind::Unit),
    )
    .expect("line plan parallel groups typecheck");
}

#[test]
fn line_plan_memo_keeps_typed_options() {
    let tree = parse_source(
        r"
flow #flow.opening opening {
    alice:
        聞いて。[p]
    with {
        memo rich_text key=(line.id, locale, theme.text_hash) cache=flow
    }
}
",
    )
    .expect("line plan memo parses");
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
        panic!("expected speaker line");
    };
    let plan = line.plan().expect("line plan");
    let [LinePlanItem::Memo { name, options }] = plan.items() else {
        panic!("expected memo item");
    };
    assert_eq!(name, "rich_text");
    assert_eq!(options.len(), 2);
    assert!(matches!(&options[0].1, Expr::Tuple(items) if items.len() == 3));

    let hir = lower_to_hir(&tree).expect("line plan memo lowers");
    validate_typecheck_ready(&hir).expect("line plan memo is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
            .with_symbol("line.id", TypeKind::Ref(EntityKind::DialogueLine))
            .with_symbol("locale", TypeKind::String)
            .with_symbol("theme.text_hash", TypeKind::Named("TextHash".to_owned()))
            .with_symbol("flow", TypeKind::Named("CacheScope".to_owned())),
    )
    .expect("line plan memo typechecks");
}

#[test]
fn parses_multiline_timed_cue_body_as_expression() {
    let tree = parse_source(
        r"
alice[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.face(smile)
",
    )
    .expect("multiline timed cue parses");

    let Item::FlowItem(FlowItem::ContentCall(call)) = &tree.items()[0] else {
        panic!("expected content call");
    };
    let plan = call.plan().expect("line plan");
    assert!(matches!(
        &plan.items()[0],
        LinePlanItem::TimedCue {
            anchor: Expr::Literal(_),
            body: Expr::MethodCall { .. }
        }
    ));
}
