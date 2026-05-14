use super::support::*;

#[test]
fn parses_fragment_as_flow_like_body() {
    let tree = parse_ok(
        r"
pub fragment @frag.alice_enters alice_enters: FlowFragment {
    @show alice normal at=right fade=220ms
    alice: おはよう。[p]
}
",
    );

    let Item::Flow(fragment) = &tree.items()[0] else {
        panic!("expected fragment as flow-like item");
    };
    assert_eq!(fragment.kind(), FlowKind::Fragment);
    assert_eq!(
        fragment.id().expect("fragment id").body(),
        "frag.alice_enters"
    );
    assert!(matches!(&fragment.body()[0], FlowItem::ScenarioCommand(_)));
    assert!(matches!(&fragment.body()[1], FlowItem::SpeakerLine(_)));
}

#[test]
fn parses_colon_form_with_inline_bracket_content() {
    let tree = parse_ok("alice(voice=auto):[今日は少しだけ。[p]]");

    let Item::FlowItem(FlowItem::SpeakerLine(line)) = &tree.items()[0] else {
        panic!("expected speaker line");
    };
    assert_eq!(line.speaker(), "alice");
    assert!(matches!(line.options().voice(), Some(Expr::Path(path)) if path == "auto"));
    assert_eq!(line.content().raw(), "[今日は少しだけ。[p]]");
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
        .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
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
fn parses_bare_block_after_dialogue_as_unnamed_scope() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    alice.say()[おはよう。[p]] {
        let tmp = route_title(state.route)
        log info "tmp={tmp}" { tmp = tmp }
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
            .with_symbol("地の文", TypeKind::Ref(EntityKind::Character))
            .with_symbol("alice", TypeKind::Ref(EntityKind::Character)),
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

    let Item::FlowItem(FlowItem::ContentCall(call)) = &tree.items()[0] else {
        panic!("expected content call");
    };
    assert_eq!(call.callee(), "alice.say");
    assert!(call.plan().is_some());
}

#[test]
fn dialogue_tokenizer_covers_content_interpolations_and_escapes() {
    let tokens = parse_dialogue_tokens(
        r#"今日は\｜少し、｜変な夢《へんなゆめ》#[fmt("夢", color=blue)][p][hook mark]"#,
    );

    assert!(tokens.iter().any(|token| matches!(token, DialogueToken::Ruby { base, ruby } if base == "変な夢" && ruby == "へんなゆめ")));
    assert!(
        tokens
            .iter()
            .any(|token| matches!(token, DialogueToken::Expr(Expr::Call { .. })))
    );
    assert!(
        tokens
            .iter()
            .any(|token| matches!(token, DialogueToken::Tag(tag) if tag.name() == "p"))
    );
    assert!(
        tokens
            .iter()
            .any(|token| matches!(token, DialogueToken::Tag(tag) if tag.name() == "hook"))
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
            .all(|token| !matches!(token, DialogueToken::EndTag(name) if name == "ruby")),
        "bracket ruby end interpolation should be consumed, got {tokens:?}"
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
            DialogueToken::Expr(Expr::Call { callee, .. })
                if matches!(callee.as_ref(), Expr::Path(path) if path == "ruby")
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
    Content::empty()
}

stream fn camera_frames() -> Source<VideoFrame, CameraError> {
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
        "fn camera_frames() -> Source<VideoFrame, CameraError>"
    );
    assert!(matches!(stream.body_statements()[0], Stmt::Yield(_)));

    let hir = lower_to_hir(&tree).expect("dialogue and stream functions lower");
    assert_eq!(hir.functions()[0].kind(), FunctionKind::Dialogue);
    assert_eq!(hir.functions()[1].kind(), FunctionKind::Stream);
    validate_typecheck_ready(&hir).expect("function-kind bodies are structured");
}
