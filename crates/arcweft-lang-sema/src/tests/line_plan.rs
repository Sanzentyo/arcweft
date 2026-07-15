use super::support::*;

#[test]
fn parses_colon_speaker_with_indented_line_plan() {
    let tree = parse_ok(
        r"
alice(voice=auto, look=smile):
    今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
with:
    at(0.42s): alice.stage.face(worried)
    cancel on input(.SkipLine) => continue
    out (actor, voice)
",
    );

    let Item::FlowItem(item) = &tree.items()[0] else {
        panic!("expected top-level speaker flow item");
    };
    let FlowItem::SpeakerLine(line) = item.as_ref() else {
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
    assert!(rule.trigger().label().contains("SkipLine"));
    assert!(matches!(rule.action(), [Stmt::Continue { label: None }]));
    assert!(matches!(&plan.items()[2], LinePlanItem::Out(_)));
}

#[test]
fn parses_bracket_speaker_call_with_with_colon_plan() {
    let tree = parse_ok(
        r"
alice[
    おはよう。[p]
]
with:
    at(0.42s): alice.stage.face(smile)
",
    );

    let Item::FlowItem(item) = &tree.items()[0] else {
        panic!("expected content call");
    };
    let FlowItem::ContentCall(call) = item.as_ref() else {
        panic!("expected content call");
    };
    assert_eq!(call.callee(), "alice");
    assert_eq!(call.content().raw(), "おはよう。[p]");
    let plan = call.plan().expect("line plan");
    assert!(matches!(
        &plan.items()[0],
        LinePlanItem::TimedCue {
            anchor: Expr::Literal(_),
            body: Expr::Call { .. }
        }
    ));
}

#[test]
fn parses_same_line_line_plan_attachments() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    alice.say()[聞いて。[p]] with: out (voice, face)
    alice.say()[もう一度。[p]] with 'line { out .Done }
    let handles = alice.say()[結果を返す。[p]] with: out (voice, face)
}
",
    );

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
        [LinePlanItem::Out(Expr::ShortVariant(path))] if path == "Done"
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
    let tree = parse_ok(
        r"
flow @flow.opening opening {
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
    );

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
        [LinePlanItem::Out(Expr::Literal(Literal::Bool(true)))]
    ));
    assert!(matches!(
        expr.as_ref(),
        Expr::DialogueCall {
            plan: Some(plan),
            ..
        } if matches!(plan.items(), [LinePlanItem::Out(Expr::ShortVariant(path))] if path == "Done")
    ));

    let hir = lower_to_hir(&tree).expect("multiline line result bindings lower");
    validate_typecheck_ready(&hir).expect("multiline line result bindings are typecheck-ready");

    let check_tree = parse_ok(
        r"
flow @flow.opening opening {
    let handles = alice.say(voice=auto)[
        今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
    ]
    with:
        out true

    let result = try alice.say(voice=auto)[
        キャンセルできる行です。[p]
    ]
    with:
        cancel on input(.SkipLine):
            out Err(LineCancel.Skipped)

        out Ok(())
}
",
    );
    let check_hir =
        lower_to_hir(&check_tree).expect("typecheck multiline line result binding lowers");
    let env = TypeCheckEnv::new()
        .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
        .with_symbol("auto", TypeKind::Named("VoicePolicy".to_owned()))
        .with_symbol(
            "LineCancel.Skipped",
            TypeKind::Named("LineCancel".to_owned()),
        )
        .with_method(
            TypeKind::entity_ref(EntityKind::Character),
            "say",
            TypeKind::Named("DialogueLine".to_owned()),
        );
    typecheck_hir(&check_hir, &env).expect("multiline line result binding typechecks");
}

#[test]
fn typechecks_bound_timed_cue_line_result_and_outer_use() {
    let tree = parse_ok(
        r#"
flow @flow.line_handles line_handles {
    let (_, cue) = alice.say(voice=auto)[聞いて。[p]]
    with:
        let actor = alice.stage.acquire(scope=line)
        let cue = at(0.42s):
            actor.look(.worried, crossfade=120ms)
        let voice = line.voice_handle()
        out (voice, cue)

    log.info("cue kept", cue = cue)
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("bound timed cue line result lowers");
    validate_typecheck_ready(&hir).expect("bound timed cue line result is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("auto", TypeKind::Named("VoicePolicy".to_owned()))
            .with_function("log.info", TypeKind::Unit),
    )
    .expect("bound timed cue line result typechecks");
}

#[test]
fn rejects_inline_parallel_group_heads_without_block_delimiters() {
    for (source, raw_fragment) in [
        (
            r"
alice[おはよう。[p]]
with:
    start cue_move()
",
            "start cue_move()",
        ),
        (
            r"
alice[おはよう。[p]]
with:
    together cue_move()
",
            "together cue_move()",
        ),
    ] {
        let tree = parse_ok(source);
        let hir = lower_to_hir(&tree).expect("lossy line plan still lowers");
        let errors =
            validate_typecheck_ready(&hir).expect_err("inline line-plan group head is rejected");
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("raw")
                    && error.message().contains(raw_fragment)),
            "expected raw line plan error for `{raw_fragment}` in {errors:?}"
        );
    }
}

#[test]
fn reports_unclosed_line_plan_block_after_cue() {
    let errors = parse_errors(
        r"
alice[おはよう。[p]]
with {
    at(0.42s) { alice.stage.face(worried)
",
    );

    assert_eq!(errors.len(), 1);
    assert!(errors[0].message().contains("line plan"));
    assert!(!errors[0].recovery().is_empty());
}

#[test]
fn line_plan_items_keep_typed_expressions() {
    let tree = parse_ok(
        r"
alice:
    聞いて。[p]
with:
    reveal = voice
    let voice = line.voice_handle()
    out (actor, voice)
",
    );

    let Item::FlowItem(item) = &tree.items()[0] else {
        panic!("expected speaker line");
    };
    let FlowItem::SpeakerLine(line) = item.as_ref() else {
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
            expr: Expr::Call { .. },
            ..
        }
    ));
    assert!(matches!(
        &plan.items()[2],
        LinePlanItem::Out(Expr::Tuple(_))
    ));
}

#[test]
fn parses_line_marks_handlers_threads_and_lifetime_registry() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    alice(focus=.soft)[待って。[mark .release_focus][p]]
    with:
        init:
            'line.focus.main <- acquire_focus()
            wait(mark(.release_focus))
        on mark(.release_focus):
            'line.focus |> drop
            out .Released
        thread motion:
            wait(0.35s)
            tick_motion()
            defer { cleanup_motion() }
        defer:
            cleanup_line()
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::ContentCall(call) = &flow.body()[0] else {
        panic!("expected content call");
    };
    assert!(call.content().tokens().iter().any(
        |token| matches!(token, DialogueToken::Mark(mark) if mark.name() == ".release_focus")
    ));
    let plan = call.plan().expect("line plan");
    assert!(matches!(&plan.items()[0], LinePlanItem::Init(_)));
    assert!(matches!(&plan.items()[1], LinePlanItem::On { .. }));
    assert!(matches!(
        &plan.items()[2],
        LinePlanItem::Thread(thread) if thread.name() == Some("motion") && thread.body().len() == 3
    ));
    assert!(matches!(
        &plan.items()[3],
        LinePlanItem::Stmt(stmt)
            if matches!(stmt.as_ref(), Stmt::DeferBlock {
                outcome: DeferOutcome::Always,
                statements
            } if statements.len() == 1)
    ));

    let hir = lower_to_hir(&tree).expect("line plan fixture lowers");
    validate_typecheck_ready(&hir).expect("line plan fixture is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol(".soft", TypeKind::Named("FocusPolicy".to_owned()))
            .with_symbol(".Released", TypeKind::Named("LineExit".to_owned()))
            .with_function("acquire_focus", TypeKind::Named("FocusHandle".to_owned()))
            .with_function("tick_motion", TypeKind::Unit)
            .with_function("cleanup_motion", TypeKind::Unit)
            .with_function("cleanup_line", TypeKind::Unit),
    )
    .expect("line plan fixture typechecks");
}

#[test]
fn parses_flat_fence_line_sugar_with_line_plan() {
    let tree = parse_ok(
        r"
flow @flow.flat flat {
=== line alice(.smile, focus = .soft) ===
聞いて。[mark .release_focus]
=== with ===
wait(mark(.release_focus))
=== /with ===
=== /line ===
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let [FlowItem::ContentCall(call)] = flow.body() else {
        panic!("expected flat line to lower to content call sugar");
    };
    assert_eq!(call.callee(), "alice");
    assert!(call.content().raw().contains("聞いて。"));
    let plan = call.plan().expect("flat with plan");
    assert!(matches!(
        plan.items(),
        [LinePlanItem::Stmt(stmt)]
            if matches!(stmt.as_ref(), Stmt::Wait(WaitTarget::Expr(_)))
    ));
}

#[test]
fn rejects_unknown_flow_items_and_malformed_flat_fences() {
    for (source, message) in [
        (
            "flow @flow.x x { launch load_avatar() }",
            "unsupported flow item",
        ),
        (
            "flow @flow.x x {\n=== thread worker ===\nfoo()\n=== /scope ===\n}",
            "flat fence close mismatch",
        ),
        (
            "flow @flow.x x {\n=== line alice ===\nhello\n}",
            "missing close fence",
        ),
        (
            "flow @flow.x x {\n=== ===\nfoo()\n=== / ===\n}",
            "unknown flat fence kind",
        ),
    ] {
        let errors = parse_errors(source);
        assert!(
            errors.iter().any(|error| error.message().contains(message)),
            "expected `{message}` in {errors:?}"
        );
    }
}

#[test]
fn rejects_duplicate_marks_and_missing_handlers() {
    let duplicate = parse_ok(
        r"
flow @flow.opening opening {
    alice[重複。[mark .x][mark .x][p]]
}
",
    );
    let duplicate_hir = lower_to_hir(&duplicate).expect("duplicate mark fixture lowers");
    let duplicate_errors = typecheck_hir(
        &duplicate_hir,
        &TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character)),
    )
    .expect_err("duplicate marks are rejected");
    assert!(
        duplicate_errors
            .iter()
            .any(|error| error.message().contains("duplicate dialogue mark"))
    );

    let missing = parse_ok(
        r"
flow @flow.opening opening {
    alice[待って。[p]]
    with:
        on mark(.missing):
            out .Missing
}
",
    );
    let missing_hir = lower_to_hir(&missing).expect("missing mark fixture lowers");
    let missing_errors = typecheck_hir(
        &missing_hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol(".Missing", TypeKind::Named("LineExit".to_owned())),
    )
    .expect_err("missing handler mark is rejected");
    assert!(missing_errors.iter().any(|error| {
        error
            .message()
            .contains("does not name a `[mark .missing]`")
    }));
}

#[test]
fn unregistered_dialogue_tags_do_not_gain_builtin_hook_semantics() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    alice[拡張タグ。[project_extension payload][p]]
}
",
    );
    let hir = lower_to_hir(&tree).expect("open tag fixture lowers");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character)),
    )
    .expect("an unregistered tag is preserved without acquiring mark or hook behavior");
}

#[test]
fn line_plan_cancel_actions_keep_typed_statements() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    alice[
        聞いて。[p]
    ]
    with 'line {
        cancel on input(.SkipLine) { out 'line .Skipped }
        cancel on input(.BackToTitle) => goto @flow.title
    }
}
",
    );

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
        [Stmt::Out { label: Some(label), expr }]
            if label == "line"
                && matches!(expr.expr(), Expr::ShortVariant(path) if path == "Skipped")
    ));
    let LinePlanItem::CancelRule(back_rule) = &plan.items()[1] else {
        panic!("expected back-to-title cancel rule");
    };
    assert!(matches!(
        back_rule.action(),
        [Stmt::Goto(target)]
            if matches!(target.expr(), Expr::EntityRef(target) if target.body() == "flow.title")
    ));

    let hir = lower_to_hir(&tree).expect("line plan cancel actions lower");
    validate_typecheck_ready(&hir).expect("line plan cancel actions are typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol(".Skipped", TypeKind::Named("LineExit".to_owned())),
    )
    .expect("typecheck succeeds");
}

#[test]
fn line_plan_cancel_commands_keep_structured_arguments() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    alice[
        聞いて。[p]
    ]
    with {
        cancel on input(.SkipLine) {
            voice.stop(fade = 40ms)
            text.flush(mode = .Instant)
            continue
        }
    }
}
",
    );

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
        [
            Stmt::Expr { expr: stop, .. },
            Stmt::Expr { expr: flush, .. },
            Stmt::Continue { label: None }
        ] if selected_call_member(stop) == Some("stop") && selected_call_member(flush) == Some("flush")
    ));

    let hir = lower_to_hir(&tree).expect("line plan cancel commands lower");
    validate_typecheck_ready(&hir).expect("line plan cancel commands are typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("voice", TypeKind::Named("VoiceHandle".to_owned()))
            .with_symbol("text", TypeKind::Named("DialogueText".to_owned())),
    )
    .expect("typecheck succeeds");
}

#[test]
fn line_plan_assertions_keep_typed_conditions() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    alice:
        聞いて。[p]
    with {
        assert(dialogue_view_ready)
        debug_assert(route_count > 0)
    }
}
",
    );
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
        panic!("expected speaker line");
    };
    let plan = line.plan().expect("line plan");
    assert!(matches!(
        &plan.items()[0],
        LinePlanItem::TimelineAssert(assertion)
            if assertion.policy()
                == arcweft_lang_syntax::ast::line_plan::TimelineAssertPolicy::Always
                && matches!(assertion.condition(), Expr::Path(path) if path == "dialogue_view_ready")
    ));

    let hir = lower_to_hir(&tree).expect("line plan assertions lower");
    validate_typecheck_ready(&hir).expect("line plan assertions are typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("dialogue_view_ready", TypeKind::Bool)
            .with_symbol("route_count", TypeKind::I64),
    )
    .expect("typecheck succeeds");
}

#[test]
fn line_plan_parallel_groups_keep_typed_items() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
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
    );
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
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_function("cue_move", TypeKind::Unit)
            .with_function("cue_face", TypeKind::Unit)
            .with_function("cue_se", TypeKind::Unit),
    )
    .expect("typecheck succeeds");
}

#[test]
fn line_plan_block_items_share_brace_and_colon_parsing() {
    fn parse_plan_items(source: &str) -> Vec<LinePlanItem> {
        let tree = parse_ok(source);
        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
            panic!("expected speaker line");
        };
        line.plan().expect("line plan").items().to_vec()
    }

    let brace_items = parse_plan_items(
        r"
flow @flow.opening opening {
    alice:
        合図。[mark .release_focus][p]
    with {
        init { setup_line() }
        on mark(.release_focus) { release_focus() }
        cancel on input(.SkipLine) { continue }
        defer on completed { cleanup_line() }
        start {
            together {
                cue_move()
                cue_face()
            }
        }
    }
}
",
    );
    let colon_items = parse_plan_items(
        r"
flow @flow.opening opening {
    alice:
        合図。[mark .release_focus][p]
    with:
        init:
            setup_line()
        on mark(.release_focus):
            release_focus()
        cancel on input(.SkipLine):
            continue
        defer on completed:
            cleanup_line()
        start:
            together:
                cue_move()
                cue_face()
}
",
    );

    for items in [&brace_items, &colon_items] {
        assert!(matches!(&items[0], LinePlanItem::Init(statements) if statements.len() == 1));
        assert!(matches!(&items[1], LinePlanItem::On { body, .. } if body.len() == 1));
        assert!(matches!(&items[2], LinePlanItem::CancelRule(rule) if rule.action().len() == 1));
        assert!(matches!(
            &items[3],
            LinePlanItem::Stmt(stmt)
                if matches!(stmt.as_ref(), Stmt::DeferBlock {
                    outcome: DeferOutcome::Completed,
                    statements,
                } if statements.len() == 1)
        ));
        assert!(matches!(
            &items[4],
            LinePlanItem::StartGroup(start_items)
                if matches!(start_items.as_slice(), [LinePlanItem::TogetherGroup(together_items)] if together_items.len() == 2)
        ));
    }
}

#[test]
fn line_plan_flat_blocks_share_block_item_parsing() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    alice:
        合図。[mark .release_focus][p]
    with:
        === init ===
        setup_line()
        === /init ===
        === on mark(.release_focus) ===
        release_focus()
        === /on ===
        === cancel on input(.SkipLine) ===
        continue
        === /cancel ===
        === start ===
        === together ===
        cue_move()
        cue_face()
        === /together ===
        === /start ===
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
        panic!("expected speaker line");
    };
    let plan = line.plan().expect("line plan");
    assert!(matches!(&plan.items()[0], LinePlanItem::Init(statements) if statements.len() == 1));
    assert!(matches!(&plan.items()[1], LinePlanItem::On { body, .. } if body.len() == 1));
    assert!(matches!(&plan.items()[2], LinePlanItem::CancelRule(rule) if rule.action().len() == 1));
    assert!(matches!(
        &plan.items()[3],
        LinePlanItem::StartGroup(start_items)
            if matches!(start_items.as_slice(), [LinePlanItem::TogetherGroup(together_items)] if together_items.len() == 2)
    ));

    let hir = lower_to_hir(&tree).expect("flat line plan blocks lower");
    validate_typecheck_ready(&hir).expect("flat line plan blocks are typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_function("setup_line", TypeKind::Unit)
            .with_function("release_focus", TypeKind::Unit)
            .with_function("cue_move", TypeKind::Unit)
            .with_function("cue_face", TypeKind::Unit),
    )
    .expect("typecheck succeeds");
}

#[test]
fn rejects_malformed_line_plan_flat_fences() {
    for (source, message) in [
        (
            r"
flow @flow.opening opening {
    alice:
        合図。[p]
    with:
        === init ===
        setup_line()
}
",
            "missing close fence",
        ),
        (
            r"
flow @flow.opening opening {
    alice:
        合図。[p]
    with:
        === start ===
        === together ===
        cue_move()
        === /start ===
        === /together ===
}
",
            "flat fence close mismatch",
        ),
        (
            r"
flow @flow.opening opening {
    alice:
        合図。[p]
    with {
        === pulse ===
        cue_move()
        === /pulse ===
    }
}
",
            "unknown flat fence kind",
        ),
    ] {
        let errors = parse_errors(source);
        assert!(
            errors.iter().any(|error| error.message().contains(message)),
            "expected `{message}` in {errors:?}"
        );
    }
}

#[test]
fn line_plan_memo_keeps_typed_options() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    alice:
        聞いて。[p]
    with {
        memo(.rich_text, key=(line.id, locale, theme.text_hash), cache=.flow)
    }
}
",
    );
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
        panic!("expected speaker line");
    };
    let plan = line.plan().expect("line plan");
    let [LinePlanItem::Expr(Expr::Call { callee, args })] = plan.items() else {
        panic!("expected memo call item");
    };
    assert!(matches!(callee.as_ref(), Expr::Path(path) if path == "memo"));
    assert_eq!(args.len(), 3);
    assert!(
        matches!(&args[0], CallArg::Positional(Expr::ShortVariant(path)) if path == "rich_text")
    );
    assert!(
        matches!(&args[1], CallArg::Named { name, value } if name == "key" && matches!(value.as_ref(), Expr::Tuple(items) if items.len() == 3))
    );
    assert!(
        matches!(&args[2], CallArg::Named { name, value } if name == "cache" && matches!(value.as_ref(), Expr::ShortVariant(path) if path == "flow"))
    );

    let hir = lower_to_hir(&tree).expect("line plan memo lowers");
    validate_typecheck_ready(&hir).expect("line plan memo is typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("line.id", TypeKind::entity_ref(EntityKind::DialogueLine))
            .with_symbol("locale", TypeKind::String)
            .with_symbol("theme.text_hash", TypeKind::Named("TextHash".to_owned()))
            .with_symbol(".flow", TypeKind::Named("CacheScope".to_owned()))
            .with_function("memo", TypeKind::Unit),
    )
    .expect("typecheck succeeds");
}

#[test]
fn parses_multiline_timed_cue_body_as_expression() {
    let tree = parse_ok(
        r"
alice[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.face(smile)
",
    );

    let Item::FlowItem(item) = &tree.items()[0] else {
        panic!("expected content call");
    };
    let FlowItem::ContentCall(call) = item.as_ref() else {
        panic!("expected content call");
    };
    let plan = call.plan().expect("line plan");
    assert!(matches!(
        &plan.items()[0],
        LinePlanItem::TimedCue {
            anchor: Expr::Literal(_),
            body: Expr::Call { .. }
        }
    ));
}
