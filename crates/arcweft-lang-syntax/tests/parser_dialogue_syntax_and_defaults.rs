use arcweft_lang_syntax::{
    ast::{dialogue::DialogueToken, flow::FlowItem, items::Item},
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

#[test]
fn dialogue_trailing_brace_plan_avoids_owned_block_for_same_line() {
    let same_line = arcweft_lang_syntax::parser::parse_source(
        r"
flow @flow.opening opening {
    alice.say()[本文です。[p]] with { out handles }
}
",
    );
    assert!(
        same_line.errors().is_empty(),
        "expected same-line line plan to parse, got {:?}",
        same_line.errors()
    );
    assert_eq!(same_line.syntax_stats().block_owned_bytes, 0);
}

#[test]
fn dialogue_line_options_are_structured_not_raw_args() {
    let source = r#"
alice(id=@say.opening.dream_hint, text_key=@text.opening.dream_hint, voice=auto, window=@textbox.side, hooks=[@hook.dialogue.read_state_color], style=@style.dream, rich_text=rich_text_style(ruby=ruby_style(size=11px)), look=smile, source_locale="ja-JP", custom=foo(size=12px)): 今日は少しだけ。[p]
"#;
    let tree = parse_ok(source);

    let Item::FlowItem(item) = &tree.items()[0] else {
        panic!("expected speaker line");
    };
    let FlowItem::SpeakerLine(line) = item.as_ref() else {
        panic!("expected speaker line");
    };
    let options = line.options();
    assert_eq!(
        options.id().expect("line id").body(),
        "say.opening.dream_hint"
    );
    assert_eq!(
        options.text_key().expect("text key").body(),
        "text.opening.dream_hint"
    );
    assert!(matches!(options.voice(), Some(Expr::Path(path)) if path == "auto"));
    assert_eq!(options.window().expect("window").body(), "textbox.side");
    assert_eq!(options.hooks().len(), 1);
    assert!(matches!(options.style(), Some(Expr::EntityRef(id)) if id.body() == "style.dream"));
    assert_eq!(options.style_raw(), Some("@style.dream"));
    assert_eq!(
        &source[options.style_range().expect("style range").as_range()],
        "@style.dream"
    );
    assert!(matches!(options.rich_text(), Some(Expr::Call { .. })));
    assert_eq!(
        options.rich_text_raw(),
        Some("rich_text_style(ruby=ruby_style(size=11px))")
    );
    assert_eq!(
        &source[options
            .rich_text_range()
            .expect("rich text range")
            .as_range()],
        "rich_text_style(ruby=ruby_style(size=11px))"
    );
    assert!(matches!(options.look(), Some(Expr::Path(path)) if path == "smile"));
    assert_eq!(options.args().len(), 1);
    assert_eq!(options.args()[0].name(), "custom");
    assert_eq!(options.args()[0].raw_value(), "foo(size=12px)");
    assert_eq!(
        &source[options.args()[0].value_range().as_range()],
        "foo(size=12px)"
    );
    assert_eq!(options.source_locale(), Some("\"ja-JP\""));
}

#[test]
fn flow_body_dialogue_ranges_use_document_offsets() {
    let source = r"
pub character alice {}

flow opening {
    alice: |[夢](ゆめ)[p]
}
";
    let tree = parse_ok(source);

    let Item::Flow(flow) = &tree.items()[1] else {
        panic!("expected flow");
    };
    let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
        panic!("expected speaker line");
    };
    let dream_offset = source.find("夢").expect("dialogue content offset");
    let content_range = line.content().range();
    assert!(content_range.start() <= dream_offset);
    assert!(dream_offset < content_range.end());
    assert_eq!(&source[content_range.as_range()], "|[夢](ゆめ)[p]");
    assert_eq!(
        &source[line.range().as_range()],
        "    alice: |[夢](ゆめ)[p]"
    );
}

#[test]
fn speaker_line_inline_interpolation_may_span_lines() {
    let tree = parse_ok(
        r"
flow opening {
    narrator: Iteration #[
        i_to_string(i)
    ] of #[a].
}
",
    );
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
        panic!("expected speaker line");
    };
    let expr_count = line
        .content()
        .tokens()
        .iter()
        .filter(|token| matches!(token, DialogueToken::Expr(_)))
        .count();
    assert_eq!(expr_count, 2);
    assert_eq!(flow.body().len(), 1);
}

#[test]
fn dialogue_interpolation_tokens_carry_document_source_ranges() {
    let source = r"
flow opening {
    alice: Score #[score + 1i64] / $( player_name )[p]
}
";
    let tree = parse_ok(source);
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
        panic!("expected speaker line");
    };
    let expr_tokens = line
        .content()
        .tokens()
        .iter()
        .filter_map(|token| match token {
            DialogueToken::Expr(expr) => Some(expr),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(expr_tokens.len(), 2);
    assert!(matches!(expr_tokens[0].expr(), Expr::Binary { .. }));
    assert_eq!(expr_tokens[0].source(), "score + 1i64");
    assert_eq!(&source[expr_tokens[0].range().as_range()], "score + 1i64");
    assert!(matches!(expr_tokens[1].expr(), Expr::Path(path) if path == "player_name"));
    assert_eq!(expr_tokens[1].source(), "player_name");
    assert_eq!(&source[expr_tokens[1].range().as_range()], "player_name");
}

#[test]
fn dialogue_defaults_are_preserved_as_top_level_declarations() {
    let source = r"
pub dialogue defaults @dialogue.defaults {
    window = @textbox.main
    voice = auto
    rich_text {
        ruby {
            size = 14px
            gap += 1px
        }
    }
}
";
    let tree = parse_ok(source);

    let Item::DialogueDefaults(defaults) = &tree.items()[0] else {
        panic!("expected dialogue defaults");
    };
    assert_eq!(
        defaults.id().expect("defaults id").body(),
        "dialogue.defaults"
    );
    let assignments = defaults.assignments();
    assert_eq!(assignments.len(), 4);
    assert_eq!(assignments[0].path().dotted(), "window");
    assert_eq!(assignments[2].path().dotted(), "rich_text.ruby.size");
    assert_eq!(assignments[3].path().dotted(), "rich_text.ruby.gap");
    assert_eq!(
        source[assignments[2].range().as_range()].trim(),
        "size = 14px"
    );
    assert_eq!(
        source[assignments[2].path_range().as_range()].trim(),
        "size"
    );
    assert_eq!(
        source[assignments[2].value_range().as_range()].trim(),
        "14px"
    );
    assert_eq!(assignments[2].raw_value(), "14px");
    assert_eq!(
        source[assignments[3].range().as_range()].trim(),
        "gap += 1px"
    );
    assert_eq!(source[assignments[3].path_range().as_range()].trim(), "gap");
    assert_eq!(
        source[assignments[3].value_range().as_range()].trim(),
        "1px"
    );
    assert_eq!(assignments[3].raw_value(), "1px");
}

#[test]
fn dialogue_defaults_preserve_attached_attributes() {
    let tree = parse_ok(
        r"
#[generated]
#[allow(style::explicit_decl_id)]
pub dialogue defaults @dialogue:.defaults.mobile {
    rich_text {
        ruby {
            size = 11px
        }
    }
}
",
    );

    let Item::DialogueDefaults(defaults) = &tree.items()[0] else {
        panic!("expected dialogue defaults");
    };
    let attrs = defaults.attrs();
    assert_eq!(attrs.len(), 2);
    assert_eq!(attrs[0].name(), "generated");
    assert_eq!(attrs[1].name(), "allow");
    assert_eq!(attrs[1].args(), Some("style::explicit_decl_id"));
    assert_eq!(
        defaults.id().expect("defaults id").body(),
        "dialogue.defaults.mobile"
    );
    assert_eq!(
        defaults.assignments()[0].path().dotted(),
        "rich_text.ruby.size"
    );
}

#[test]
fn dialogue_defaults_reject_relative_profile_ids_and_one_line_nested_blocks() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r"
pub dialogue defaults @.mobile {
    rich_text { ruby { size = 11px } }
}
",
    );

    let errors = parsed.errors();
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("dialogue defaults profiles cannot use relative IDs")
    }));
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("one-line nested dialogue defaults blocks are not canonical")
    }));
}

#[test]
fn dialogue_defaults_accept_family_relative_profile_ids() {
    let tree = parse_ok(
        r"
pub dialogue defaults @dialogue:.defaults.mobile {
    rich_text {
        ruby {
            size = 11px
        }
    }
}
",
    );

    let Item::DialogueDefaults(defaults) = &tree.items()[0] else {
        panic!("expected dialogue defaults");
    };
    assert_eq!(
        defaults.id().expect("defaults id").body(),
        "dialogue.defaults.mobile"
    );
}
