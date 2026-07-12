use super::support::*;
use crate::diagnostics::TypeCheckError;

#[test]
fn parses_reusable_flow_body() {
    let tree = parse_ok(
        r"
pub flow @flow.alice_enters alice_enters {
    show(@character.alice, .normal, at = .right, fade = 220ms)
    alice: おはよう。[p]
}
",
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow item");
    };
    assert_eq!(flow.id().expect("flow id").body(), "flow.alice_enters");
    assert!(matches!(
        &flow.body()[0],
        FlowItem::Stmt(Stmt::Expr {
            expr: Expr::Call { .. },
            ..
        })
    ));
    assert!(matches!(&flow.body()[1], FlowItem::SpeakerLine(_)));
}

#[test]
fn parses_colon_form_with_inline_bracket_content() {
    let tree = parse_ok("alice(voice=auto):[今日は少しだけ。[p]]");

    let Item::FlowItem(item) = &tree.items()[0] else {
        panic!("expected speaker line");
    };
    let FlowItem::SpeakerLine(line) = item.as_ref() else {
        panic!("expected speaker line");
    };
    assert_eq!(line.speaker(), "alice");
    assert!(matches!(line.options().voice(), Some(Expr::Path(path)) if path == "auto"));
    assert_eq!(line.content().raw(), "[今日は少しだけ。[p]]");
}

#[test]
fn parses_positional_look_and_extended_line_options() {
    let tree = parse_ok(
        "alice(smile, voice=auto, stage=.main, portrait=.bust, focus=.soft, cleanup=.line):[おはよう。[p]]",
    );

    let Item::FlowItem(item) = &tree.items()[0] else {
        panic!("expected speaker line");
    };
    let FlowItem::SpeakerLine(line) = item.as_ref() else {
        panic!("expected speaker line");
    };
    assert!(matches!(line.options().look(), Some(Expr::Path(path)) if path == "smile"));
    assert!(matches!(line.options().stage(), Some(Expr::ShortVariant(path)) if path == "main"));
    assert!(matches!(line.options().portrait(), Some(Expr::ShortVariant(path)) if path == "bust"));
    assert!(matches!(line.options().focus(), Some(Expr::ShortVariant(path)) if path == "soft"));
    assert!(matches!(line.options().cleanup(), Some(Expr::ShortVariant(path)) if path == "line"));
}

#[test]
fn unreserved_line_options_remain_extension_arguments_without_builtin_meaning() {
    let tree = parse_ok("alice(project_option=soft):[おはよう。[p]]");
    let Item::FlowItem(item) = &tree.items()[0] else {
        panic!("expected speaker line");
    };
    let FlowItem::SpeakerLine(line) = item.as_ref() else {
        panic!("expected speaker line");
    };

    assert!(line.options().look().is_none());
    assert!(matches!(
        line.options().args(),
        [argument]
            if argument.name() == "project_option"
                && matches!(argument.value(), Expr::Path(path) if path == "soft")
    ));
}

#[test]
fn typechecks_character_method_and_speaker_preset_dialogue_callees() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    alice.say(voice=auto)[おはよう。[p]]
    @<character.alice>.say(voice=auto)[おはよう。[p]]
    alice2(voice=auto): おはよう。[p]
    alice2(voice=auto)[おはよう。[p]]
}
",
    );

    let hir = lower_to_hir(&tree).expect("dialogue callee fixture lowers");
    let env = TypeCheckEnv::new()
        .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
        .with_symbol("alice2", TypeKind::Named("SpeakerPreset".to_owned()));

    typecheck_hir(&hir, &env).expect("dialogue callee forms typecheck");
    let flow = &hir.flows()[0];
    let HirFlowItem::Dialogue(delimited) = &flow.body()[1] else {
        panic!("expected delimited character dialogue");
    };
    assert_eq!(
        delimited.id().expect("generated delimited line id").body(),
        "say.opening.alice.002"
    );
}

#[test]
fn speaker_preset_options_parse_but_reject_unresolved_atoms() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let alice2 = alice(face=smile, mood=embarrassed, custom_style=soft, view=@view:.side)
    alice2: おはよう。[p]
    alice.face(worried)
}
",
    );
    let hir = lower_to_hir(&tree).expect("extensible speaker options lower");
    let env = TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character));
    let errors = typecheck_hir(&hir, &env).expect_err("unresolved dialogue atoms are rejected");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("unknown symbol `smile`"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("unknown symbol `worried`"))
    );

    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let bad = smile
}
",
    );
    let hir = lower_to_hir(&tree).expect("bare atom fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("bare atom is not global");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("unknown symbol `smile`"))
    );
}

#[test]
fn speaker_preset_options_accept_resolved_variant_atoms() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let alice2 = alice(face=.smile, voice=auto, view=@view:.side)
    alice2: おはよう。[p]
    alice.face(.worried)
}
",
    );
    let hir = lower_to_hir(&tree).expect("resolved speaker options lower");
    let env = TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character));
    typecheck_hir(&hir, &env).expect("short variant atom options typecheck");
}

#[test]
fn parses_bare_block_after_dialogue_as_unnamed_scope() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    alice.say()[おはよう。[p]] {
        let tmp = route_title(state.route)
        log.info("tmp={tmp}", tmp = tmp)
    }
}
"#,
    );

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let [FlowItem::ContentCall(call), FlowItem::Scope(block)] = flow.body() else {
        panic!("expected dialogue call followed by unnamed scope");
    };
    assert!(call.plan().is_none());
    assert_eq!(block.name(), None);
    assert_eq!(block.body().len(), 2);

    let hir = lower_to_hir(&tree).expect("dialogue plus bare block lowers");
    let [HirFlowItem::Dialogue(dialogue), HirFlowItem::Scope(block)] = hir.flows()[0].body() else {
        panic!("expected HIR dialogue followed by unnamed scope");
    };
    assert!(dialogue.plan().is_none());
    assert_eq!(block.name(), None);
    assert_eq!(block.body().len(), 2);
}

#[test]
fn lowers_relative_dialogue_line_options() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    scope rain {
        地の文(id=@.sound):
            扉の向こうから、雨の音がした。[p]

        alice(id=@.comment, text_key=@.comment_text, source_locale=en-US):
            Good morning.[p]

        地の文:
            窓が小さく鳴った。[p]
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("relative dialogue options lower");
    let HirFlowItem::Scope(scope) = &hir.flows()[0].body()[0] else {
        panic!("expected HIR scope");
    };
    let HirFlowItem::Dialogue(narration) = &scope.body()[0] else {
        panic!("expected narration");
    };
    assert_eq!(
        narration.id().expect("narration id").body(),
        "say.opening.narrator.rain.sound"
    );
    assert_eq!(
        narration.text_key().expect("derived text key").body(),
        "text.opening.narrator.rain.sound"
    );

    let HirFlowItem::Dialogue(alice) = &scope.body()[1] else {
        panic!("expected alice line");
    };
    assert_eq!(
        alice.id().expect("alice line id").body(),
        "say.opening.alice.rain.comment"
    );
    assert_eq!(
        alice.text_key().expect("explicit text key").body(),
        "text.opening.alice.rain.comment_text"
    );
    assert_eq!(alice.source_locale(), Some("en-US"));

    let HirFlowItem::Dialogue(generated) = &scope.body()[2] else {
        panic!("expected generated-id narration");
    };
    assert_eq!(
        generated.id().expect("generated line id").body(),
        "say.opening.narrator.rain.001"
    );
    assert_eq!(
        generated.text_key().expect("generated text key").body(),
        "text.opening.narrator.rain.001"
    );

    let registry = registry_from_hir(&hir);
    validate_hir_references(&hir, &registry).expect("dialogue ids resolve");
    validate_typecheck_ready(&hir).expect("dialogue option IDs are typecheck-ready");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("地の文", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character)),
    )
    .expect("typecheck succeeds");
}

#[test]
fn lowers_at_relative_dialogue_line_options_with_parent_scopes() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    scope outer {
        scope inner {
            alice(id=@...shared, text_key=@super.inner_text):
                Good morning.[p]
        }
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("at-relative dialogue options lower");
    let HirFlowItem::Scope(outer) = &hir.flows()[0].body()[0] else {
        panic!("expected outer scope");
    };
    let HirFlowItem::Scope(inner) = &outer.body()[0] else {
        panic!("expected inner scope");
    };
    let HirFlowItem::Dialogue(alice) = &inner.body()[0] else {
        panic!("expected alice line");
    };

    assert_eq!(
        alice.id().expect("alice line id").body(),
        "say.opening.alice.shared"
    );
    assert_eq!(
        alice.text_key().expect("explicit text key").body(),
        "text.opening.alice.outer.inner_text"
    );
}

#[test]
fn parses_character_content_call_with_brace_plan() {
    let tree = parse_ok(
        r##"
alice.say(id=@say.opening.dream_hint, voice=auto)[
    今日は少しだけ、#[fmt("変な夢", color=rgb("#a8b5ff"))]を見たんだ。[p]
]
with {
    at(0.42s) { alice.stage.face(worried, crossfade=120ms) }
}
"##,
    );

    let Item::FlowItem(item) = &tree.items()[0] else {
        panic!("expected content call");
    };
    let FlowItem::ContentCall(call) = item.as_ref() else {
        panic!("expected content call");
    };
    assert_eq!(call.callee(), "alice.say");
    assert!(call.plan().is_some());
}

#[test]
fn dialogue_tokenizer_covers_content_interpolations_and_escapes() {
    let tokens = parse_dialogue_tokens(
        r#"今日は\｜少し、｜変な夢《へんなゆめ》#[fmt("夢", color=blue)][p][mark .release]"#,
    );

    assert!(tokens.iter().any(|token| matches!(token, DialogueToken::Ruby { base, ruby } if base == "変な夢" && ruby == "へんなゆめ")));
    assert!(
        tokens
            .iter()
            .any(|token| matches!(token, DialogueToken::Expr(expr) if matches!(expr.expr(), Expr::Call { .. })))
    );
    assert!(
        tokens
            .iter()
            .any(|token| matches!(token, DialogueToken::Tag(tag) if tag.name() == "p"))
    );
    assert!(
        tokens
            .iter()
            .any(|token| matches!(token, DialogueToken::Mark(mark) if mark.name() == ".release"))
    );
}

#[test]
fn dialogue_tokenizer_normalizes_bracket_ruby_to_ruby_token() {
    let tokens = parse_dialogue_tokens(
        r#"今日は少しだけ、[ruby rt="へんなゆめ"]変な夢[/ruby]を見たんだ。[p]"#,
    );

    assert!(tokens.iter().any(
            |token| matches!(token, DialogueToken::Ruby { base, ruby } if base == "変な夢" && ruby == "へんなゆめ")
        ));
    assert!(
        tokens.iter().all(|token| !matches!(
            token,
            DialogueToken::Tag(tag) if tag.name() == "ruby"
        )),
        "bracket ruby should normalize to Ruby token, got {tokens:?}"
    );
    assert!(
        tokens
            .iter()
            .all(|token| !matches!(token, DialogueToken::EndTag(tag) if tag.name() == "ruby")),
        "bracket ruby end interpolation should be consumed, got {tokens:?}"
    );
}

#[test]
fn dialogue_tokenizer_normalizes_ascii_ruby_forms_to_ruby_token() {
    let tokens = parse_dialogue_tokens(
        "今日は|[変な夢](へんなゆめ)と|悪夢{あくむ}と[rb rt=まぼろし]幻[/rb]を見た。[p]",
    );

    assert!(tokens.iter().any(
        |token| matches!(token, DialogueToken::Ruby { base, ruby } if base == "変な夢" && ruby == "へんなゆめ")
    ));
    assert!(tokens.iter().any(
        |token| matches!(token, DialogueToken::Ruby { base, ruby } if base == "悪夢" && ruby == "あくむ")
    ));
    assert!(tokens.iter().any(
        |token| matches!(token, DialogueToken::Ruby { base, ruby } if base == "幻" && ruby == "まぼろし")
    ));
}

#[test]
fn dialogue_tokenizer_reports_invalid_compact_ruby_without_consuming_text() {
    let parsed = parse_dialogue_text("今日は|変 な夢{へんなゆめ}を見た。[p]");

    assert!(parsed.diagnostics().iter().any(|diagnostic| {
        diagnostic.message().contains("invalid compact ruby")
            && diagnostic.recovery().contains("|[base](ruby)")
    }));
    assert!(parsed.tokens().iter().any(
        |token| matches!(token, DialogueToken::Text(text) if text.contains("|変 な夢{へんなゆめ}"))
    ));
}

#[test]
fn dialogue_tokenizer_normalizes_authoring_sugar_tags() {
    let tokens = parse_dialogue_tokens(
        r"$(player_name)[! flash(color=#ffffff)][.keyword][w 500ms][page][wait][nl][em:夢][strong:声][color #a8b5ff:夜][raw: [p] literal][p]",
    );

    assert!(tokens.iter().any(
        |token| matches!(token, DialogueToken::Expr(expr) if matches!(expr.expr(), Expr::Path(path) if path == "player_name"))
    ));
    assert!(tokens.iter().any(
        |token| matches!(token, DialogueToken::Tag(tag) if tag.name() == "call" && tag.attrs() == "flash(color=#ffffff)")
    ));
    assert!(
        tokens.iter().any(
            |token| matches!(token, DialogueToken::InferredTag(tag) if tag.name() == ".keyword")
        )
    );
    assert!(tokens.iter().any(
        |token| matches!(token, DialogueToken::Tag(tag) if tag.name() == "w" && tag.attrs() == "time=500ms")
    ));
    assert!(
        tokens
            .iter()
            .any(|token| matches!(token, DialogueToken::Tag(tag) if tag.name() == "p"))
    );
    assert!(
        tokens
            .iter()
            .any(|token| matches!(token, DialogueToken::Tag(tag) if tag.name() == "l"))
    );
    assert!(
        tokens
            .iter()
            .any(|token| matches!(token, DialogueToken::Tag(tag) if tag.name() == "r"))
    );
    assert!(tokens.windows(3).any(|window| matches!(
        window,
        [
            DialogueToken::Tag(tag),
            DialogueToken::Text(text),
            DialogueToken::EndTag(end)
        ] if tag.name() == "em" && text == "夢" && end.name() == "em"
    )));
    assert!(tokens.windows(3).any(|window| matches!(
        window,
        [
            DialogueToken::Tag(tag),
            DialogueToken::Text(text),
            DialogueToken::EndTag(end)
        ] if tag.name() == "color" && tag.attrs() == "value=\"#a8b5ff\"" && text == "夜" && end.name() == "color"
    )));
    assert!(
        tokens
            .iter()
            .any(|token| matches!(token, DialogueToken::Raw(raw) if raw == "[p] literal"))
    );
    assert_eq!(
        tokens
            .iter()
            .filter(|token| matches!(token, DialogueToken::Tag(tag) if tag.name() == "p"))
            .count(),
        2
    );
}

#[test]
fn dialogue_wait_duration_is_typed_at_the_language_boundary() {
    let tokens = parse_dialogue_tokens("[w 125ms][w time=0.5s][w 2s]");
    let durations = tokens
        .iter()
        .filter_map(|token| match token {
            DialogueToken::Tag(tag) if tag.name() == "w" => {
                Some(tag.wait_duration().expect("valid wait duration").millis())
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(durations, [125, 500, 2_000]);

    let speeds = parse_dialogue_tokens("[speed slow][speed cps=28.5][speed fast]")
        .into_iter()
        .filter_map(|token| match token {
            DialogueToken::Tag(tag) if tag.name() == "speed" => {
                Some(tag.reveal_speed().expect("valid reveal speed"))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        speeds
            .iter()
            .map(|speed| speed.milli_cps())
            .collect::<Vec<_>>(),
        [14_000, 28_500, 56_000]
    );
    assert_eq!(speeds[1].canonical_cps(), "28.5");
}

#[test]
fn typechecker_rejects_invalid_dialogue_waits_and_control_attributes() {
    let tree = parse_ok(
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[w]B[w 0ms]C[w 0.0001s]D[p unexpected]E[speed]F[speed 241]
}
",
    );
    let hir = lower_to_hir(&tree).expect("invalid controls still lower for diagnostics");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect_err("invalid dialogue controls must fail type checking");
    let messages = errors
        .iter()
        .map(TypeCheckError::message)
        .collect::<Vec<_>>();

    assert!(
        messages
            .iter()
            .any(|message| message.contains("requires a duration"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("greater than zero"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("precision below one millisecond"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("`[p]` does not accept attributes"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("dialogue speed requires"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("between 1 and 240"))
    );
}

#[test]
fn parser_surfaces_dialogue_text_diagnostics() {
    let errors = parse_errors("alice: 今日は|変 な夢{へんなゆめ}を見た。[p]");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("invalid compact ruby"))
    );
}

#[test]
fn typechecker_uses_shorthand_marks_for_line_plan_handlers() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    alice[待って。[.seen][p]]
    with:
        on mark(.seen):
            log.info("seen")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("shorthand mark lowers");

    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character)),
    )
    .expect("shorthand mark is visible to line plan handler");
}

#[test]
fn typechecker_does_not_register_custom_effect_selectors_as_marks() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    alice[One [.sparkle amp=1px]effect[/], two [.sparkle amp=2px]effects[/].[p]]
}
",
    );
    let hir = lower_to_hir(&tree).expect("custom effect selectors lower");

    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character)),
    )
    .expect("parameterized custom effect selectors are spans, not duplicate marks");
}

#[test]
fn lowers_family_relative_dialogue_id_declarations() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    scope dream {
        alice(id=@say:.hint, text_key=@text:.hint):
            今日は少しだけ。[p]
    }
}
",
    );

    let hir = lower_to_hir(&tree).expect("family-relative dialogue IDs lower");
    let HirFlowItem::Scope(scope) = &hir.flows()[0].body()[0] else {
        panic!("expected scope");
    };
    let HirFlowItem::Dialogue(line) = &scope.body()[0] else {
        panic!("expected dialogue");
    };
    assert_eq!(
        line.id().expect("line id").body(),
        "say.opening.alice.dream.hint"
    );
    assert_eq!(
        line.text_key().expect("text key").body(),
        "text.opening.alice.dream.hint"
    );
}

#[test]
fn lowers_narrator_aliases_and_family_relative_windows() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    scope rain {
        ナレーション(id=@.voiceover, view=@view:.narrator):
            扉の向こうから、雨の音がした。[p]
    }
}
",
    );

    let hir = lower_to_hir(&tree).expect("narrator alias line lowers");
    let HirFlowItem::Scope(scope) = &hir.flows()[0].body()[0] else {
        panic!("expected scope");
    };
    let HirFlowItem::Dialogue(line) = &scope.body()[0] else {
        panic!("expected dialogue");
    };

    assert_eq!(
        line.id().expect("line id").body(),
        "say.opening.narrator.rain.voiceover"
    );
    assert_eq!(
        line.view().expect("view").body(),
        "view.opening.rain.narrator"
    );
}

#[test]
fn dialogue_tokenizer_normalizes_function_ruby_to_ruby_token() {
    let tokens =
        parse_dialogue_tokens(r#"今日は少しだけ、#[ruby("変な夢", "へんなゆめ")]を見たんだ。[p]"#);

    assert!(tokens.iter().any(
            |token| matches!(token, DialogueToken::Ruby { base, ruby } if base == "変な夢" && ruby == "へんなゆめ")
        ));
    assert!(
        tokens.iter().all(|token| !matches!(
            token,
            DialogueToken::Expr(expr)
                if matches!(expr.expr(), Expr::Call { callee, .. } if matches!(callee.as_ref(), Expr::Path(path) if path == "ruby"))
        )),
        "ruby(...) interpolation should normalize to Ruby token, got {tokens:?}"
    );
}

#[test]
fn dialogue_tokenizer_preserves_escaped_brackets_hash_and_braces() {
    let tokens = parse_dialogue_tokens(r"\[literal\] \#not_expr \{cue\}");

    assert!(matches!(tokens[0], DialogueToken::Escape('[')));
    assert!(matches!(tokens[2], DialogueToken::Escape(']')));
    assert!(matches!(tokens[4], DialogueToken::Escape('#')));
    assert!(matches!(tokens[6], DialogueToken::Escape('{')));
    assert!(matches!(tokens[8], DialogueToken::Escape('}')));
    assert!(
        tokens
            .iter()
            .all(|token| !matches!(token, DialogueToken::Tag(_) | DialogueToken::Expr(_)))
    );
}

#[test]
fn dialogue_tokenizer_preserves_raw_spans_without_inner_parsing() {
    let tokens = parse_dialogue_tokens("[raw]これは[p]も#[expr]も文字。[/raw][p]");

    assert!(matches!(
        &tokens[0],
        DialogueToken::Raw(raw) if raw == "これは[p]も#[expr]も文字。"
    ));
    assert!(
        tokens[..1]
            .iter()
            .all(|token| !matches!(token, DialogueToken::Tag(_) | DialogueToken::Expr(_)))
    );
    assert!(matches!(
        tokens.last(),
        Some(DialogueToken::Tag(tag)) if tag.name() == "p"
    ));

    let block = parse_dialogue_tokens("[raw]\n[p] も文字として表示する。\n[/raw]");
    assert!(matches!(
        &block[0],
        DialogueToken::Raw(raw) if raw.contains("[p] も文字")
    ));
}

#[test]
fn reports_unclosed_dialogue_content_block() {
    let errors = parse_errors("alice[おはよう。[p]");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message().contains("dialogue content"));
    assert_eq!(errors[0].expected(), &["]"]);
    assert!(!errors[0].recovery().is_empty());
}

#[test]
fn parses_dialogue_and_stream_function_kinds() {
    let tree = parse_ok(
        r"
pub dialogue fn flash(color: Color) -> Content {
    Content.empty()
}

stream fn camera_frames() -> Stream<VideoFrame, CameraError> {
    yield next_frame()
}
",
    );

    let Item::Function(dialogue) = &tree.items()[0] else {
        panic!("expected dialogue function");
    };
    assert_eq!(dialogue.kind(), FunctionKind::Dialogue);
    assert_eq!(
        dialogue.signature_text(),
        "fn flash(color: Color) -> Content"
    );

    let Item::Function(stream) = &tree.items()[1] else {
        panic!("expected stream function");
    };
    assert_eq!(stream.kind(), FunctionKind::Stream);
    assert_eq!(
        stream.signature_text(),
        "fn camera_frames() -> Stream<VideoFrame, CameraError>"
    );
    assert!(matches!(stream.body_statements()[0], Stmt::Yield(_)));

    let hir = lower_to_hir(&tree).expect("dialogue and stream functions lower");
    assert_eq!(hir.functions()[0].kind(), FunctionKind::Dialogue);
    assert_eq!(hir.functions()[1].kind(), FunctionKind::Stream);
    validate_typecheck_ready(&hir).expect("function-kind bodies are structured");
}
