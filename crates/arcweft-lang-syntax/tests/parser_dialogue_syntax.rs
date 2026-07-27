use arcweft_lang_syntax::{
    ast::{
        dialogue::{DialogueContent, DialogueToken},
        flow::{FlowItem, Stmt},
        items::Item,
    },
    expr::Expr,
};

fn parse_ok(source: impl Into<String>) -> arcweft_lang_syntax::source::ParsedSource {
    let parsed = parse_dialogue_fixture(source);
    assert!(
        parsed.errors().is_empty(),
        "expected source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed
}

#[test]
fn dialogue_trailing_brace_plan_avoids_owned_block_for_same_line() {
    let same_line = parse_dialogue_fixture(
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
fn let_dialogue_call_expr_source_includes_same_line_plan() {
    let source = r"
flow @flow.opening opening {
    let result = alice.say()[Pick one.] with { out score + 1i64 }
}
";
    let parsed = parse_ok(source);
    let tree = parsed.typed_tree();
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::Let {
        expr: Expr::DialogueCall {
            plan: Some(plan), ..
        },
        expr_source: Some(expr_source),
        expr_range: Some(expr_range),
        ..
    }) = &flow.body()[0]
    else {
        panic!("expected dialogue call let binding with source");
    };

    assert_eq!(
        expr_source,
        "alice.say()[Pick one.] with { out score + 1i64 }"
    );
    assert_eq!(
        &source[expr_range.as_range()],
        "alice.say()[Pick one.] with { out score + 1i64 }"
    );
    assert_eq!(
        &source[plan.range().as_range()],
        "with { out score + 1i64 }"
    );
}

#[test]
fn let_dialogue_call_expr_source_includes_following_line_plan() {
    let source = r"
flow @flow.opening opening {
    let result = alice.say()[Pick one.]
    with:
        out score + 1i64
}
";
    let parsed = parse_ok(source);
    let tree = parsed.typed_tree();
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Stmt(Stmt::Let {
        expr: Expr::DialogueCall {
            plan: Some(plan), ..
        },
        expr_source: Some(expr_source),
        expr_range: Some(expr_range),
        ..
    }) = &flow.body()[0]
    else {
        panic!("expected dialogue call let binding with source");
    };

    assert_eq!(
        expr_source,
        "alice.say()[Pick one.]\n    with:\n        out score + 1i64"
    );
    assert_eq!(
        &source[expr_range.as_range()],
        "alice.say()[Pick one.]\n    with:\n        out score + 1i64"
    );
    assert_eq!(
        &source[plan.range().as_range()],
        "    with:\n        out score + 1i64"
    );
}

#[test]
fn multiline_let_dialogue_call_expr_range_slices_lf_and_crlf_source() {
    let source_lf = "flow @flow.opening opening {\n    let result = alice.say()[\n        Intro\n        [.sparkle amp=2px]effect[/][p]\n    ]\n}\n";
    for source in [source_lf.to_owned(), source_lf.replace('\n', "\r\n")] {
        let parsed = parse_ok(source.clone());
        let tree = parsed.typed_tree();
        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Stmt(Stmt::Let {
            expr: Expr::DialogueCall { content, .. },
            expr_source: Some(expr_source),
            expr_range: Some(expr_range),
            ..
        }) = &flow.body()[0]
        else {
            panic!("expected dialogue call let binding with source");
        };

        assert_eq!(
            source[expr_range.as_range()].replace("\r\n", "\n"),
            *expr_source
        );
        assert!(source[expr_range.as_range()].ends_with(']'));
        let sparkle = content
            .tokens()
            .iter()
            .find_map(|token| match token {
                DialogueToken::InferredTag(tag) if tag.name() == ".sparkle" => Some(tag),
                _ => None,
            })
            .expect("typed sparkle tag");
        let sparkle_source = content
            .source_range(sparkle.range())
            .expect("expression dialogue provenance");
        assert_eq!(&source[sparkle_source.as_range()], "[.sparkle amp=2px]");
    }
}

#[test]
fn dialogue_line_options_are_structured_not_raw_args() {
    let source = r#"
flow @flow.opening opening {
    alice(id=@say.opening.dream_hint, text_key=@text.opening.dream_hint, voice=auto, view=@view.side, hooks=[@hook.dialogue.read_state_color], style=@style.dream, rich_text=rich_text_style(ruby=ruby_style(size=11px)), look=smile, source_locale="ja-JP", custom=foo(size=12px)): 今日は少しだけ。[p]
}
"#;
    let parsed = parse_ok(source);
    let tree = parsed.typed_tree();

    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
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
    assert_eq!(options.view().expect("view").body(), "view.side");
    assert_eq!(options.hooks().len(), 1);
    assert!(matches!(options.style(), Some(Expr::EntityRef(id)) if id.body() == "style.dream"));
    assert_eq!(options.style_raw(), Some("@style.dream"));
    assert_eq!(
        &source[options.style_range().expect("style range").as_range()],
        "@style.dream"
    );
    assert!(matches!(options.rich_text(), Some(Expr::Call(_))));
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
fn spaced_dialogue_line_option_call_keeps_document_coordinates() {
    let source = r"
flow opening {
    alice(look = standard_value(1i32)): hello
}
";
    let parsed = parse_ok(source);
    let tree = parsed.typed_tree();
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
        panic!("expected speaker line");
    };
    let Some(Expr::Call(call)) = line.options().look() else {
        panic!("expected ordinary call in the look option");
    };

    assert_eq!(&source[call.range().as_range()], "standard_value(1i32)");
    assert_eq!(&source[call.callee_range().as_range()], "standard_value");
}

#[test]
fn flow_body_dialogue_ranges_use_document_offsets() {
    let source = r"
pub character alice {}

flow opening {
    alice: |[夢](ゆめ)[p]
}
";
    let parsed = parse_ok(source);
    let tree = parsed.typed_tree();

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
    let parsed = parse_ok(
        r"
flow opening {
    narrator: Iteration #[
        i_to_string(i)
    ] of #[a].
}
",
    );
    let tree = parsed.typed_tree();
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
    let parsed = parse_ok(source);
    let tree = parsed.typed_tree();
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
    assert_eq!(
        &line.content().raw()[expr_tokens[0].range().as_range()],
        "score + 1i64"
    );
    assert_eq!(
        &source[line
            .content()
            .source_range(expr_tokens[0].range())
            .expect("binary interpolation source range")
            .as_range()],
        "score + 1i64"
    );
    assert!(matches!(expr_tokens[1].expr(), Expr::Path(path) if path == "player_name"));
    assert_eq!(expr_tokens[1].source(), "player_name");
    assert_eq!(
        &line.content().raw()[expr_tokens[1].range().as_range()],
        "player_name"
    );
    assert_eq!(
        &source[line
            .content()
            .source_range(expr_tokens[1].range())
            .expect("path interpolation source range")
            .as_range()],
        "player_name"
    );
}

fn assert_later_line_dialogue_ranges(source: &str, content: &DialogueContent) {
    let expression = content
        .tokens()
        .iter()
        .find_map(|token| match token {
            DialogueToken::Expr(expression) => Some(expression),
            _ => None,
        })
        .expect("later-line dialogue expression");
    assert_eq!(
        &content.raw()[expression.range().as_range()],
        "score + 1i64"
    );
    assert_eq!(
        &source[content
            .source_range(expression.range())
            .expect("projected expression range")
            .as_range()],
        "score + 1i64"
    );

    let tag = content
        .tokens()
        .iter()
        .find_map(|token| match token {
            DialogueToken::Tag(tag) if tag.name() == "effect" => Some(tag),
            _ => None,
        })
        .expect("later-line effect tag");
    assert_eq!(
        &content.raw()[tag.range().as_range()],
        "[effect .warning mood=\"very urgent\"]"
    );
    assert_eq!(
        &source[content
            .source_range(tag.range())
            .expect("projected tag range")
            .as_range()],
        "[effect .warning mood=\"very urgent\"]"
    );
    let mood = tag
        .arguments()
        .iter()
        .find(|argument| argument.name() == Some("mood"))
        .expect("mood argument");
    let mood_value = mood.value().expect("mood value");
    assert_eq!(
        &content.raw()[mood_value.range().as_range()],
        "\"very urgent\""
    );
    assert_eq!(
        &source[content
            .source_range(mood_value.range())
            .expect("projected tag value range")
            .as_range()],
        "\"very urgent\""
    );
}

fn speaker_content(tree: &arcweft_lang_syntax::ast::items::TypedSyntaxTree) -> &DialogueContent {
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::SpeakerLine(line) = &flow.body()[0] else {
        panic!("expected speaker line");
    };
    line.content()
}

#[test]
fn multiline_dialogue_ranges_project_across_lf_normalization() {
    let source = r#"flow opening {
    narrator: Iteration #[
        score + 1i64
    ] [effect .warning mood="very urgent"]text[/effect]
}
"#;
    let parsed = parse_ok(source);
    let tree = parsed.typed_tree();
    let content = speaker_content(tree);
    assert!(content.raw().contains("#[\nscore + 1i64\n]"));
    assert_later_line_dialogue_ranges(source, content);
}

#[test]
fn multiline_dialogue_ranges_project_across_crlf_normalization() {
    let source = "flow opening {\r\n    narrator: Iteration #[\r\n        score + 1i64\r\n    ] [effect .warning mood=\"very urgent\"]text[/effect]\r\n}\r\n";
    let parsed = parse_ok(source);
    let tree = parsed.typed_tree();
    let content = speaker_content(tree);
    assert!(content.raw().contains("#[\nscore + 1i64\n]"));
    assert_later_line_dialogue_ranges(source, content);
}

#[test]
fn indented_dialogue_ranges_project_from_trimmed_lines() {
    let source = r#"flow opening {
    narrator:
        Intro
        #[score + 1i64] [effect .warning mood="very urgent"]text[/effect]
}
"#;
    let parsed = parse_ok(source);
    let tree = parsed.typed_tree();
    let content = speaker_content(tree);
    assert_eq!(
        content.raw(),
        "Intro\n#[score + 1i64] [effect .warning mood=\"very urgent\"]text[/effect]"
    );
    assert_later_line_dialogue_ranges(source, content);
}

#[test]
fn bracket_content_call_ranges_project_from_normalized_lines() {
    let source = r#"flow opening {
    alice.say()[Intro
        #[score + 1i64] [effect .warning mood="very urgent"]text[/effect]
    ]
}
"#;
    let parsed = parse_ok(source);
    let tree = parsed.typed_tree();
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::ContentCall(call) = &flow.body()[0] else {
        panic!("expected content call");
    };
    assert_later_line_dialogue_ranges(source, call.content());
}

fn parse_dialogue_fixture(source: impl Into<String>) -> arcweft_lang_syntax::source::ParsedSource {
    let document = std::sync::Arc::new(
        arcweft_source::SourceDocument::try_new(
            arcweft_source::SourceDocumentId::try_new(
                "arcweft-test://syntax/parser-dialogue-syntax",
            )
            .expect("fixed test document ID is valid"),
            arcweft_source::SourceName::path("parser-dialogue-syntax.arcw"),
            source.into(),
        )
        .expect("test source document"),
    );
    arcweft_lang_syntax::parser::parse_document_with_source(
        document,
        arcweft_lang_syntax::parser::ParseOptions::default(),
    )
}
