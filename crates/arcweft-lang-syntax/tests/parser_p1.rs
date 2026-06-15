fn parse_ok(source: impl Into<String>) -> arcweft_lang_syntax::ast::items::TypedSyntaxTree {
    let parsed = arcweft_lang_syntax::parser::parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "expected source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed.into_typed_tree()
}

use arcweft_lang_syntax::{
    ast::{flow::FlowItem, items::Item},
    expr::Expr,
    types::{FnParamKind, GenericParam, TypeRef, parse_fn_signature},
};

#[test]
fn function_signatures_keep_generics_curried_groups_and_where_clauses() {
    let signature = parse_fn_signature(
        "fn bind<'a, T>(state: &'a State)(route: T) -> ArcResult<T> where T: Clone + Debug",
    )
    .expect("curried generic signature parses");

    assert_eq!(signature.name(), "bind");
    assert!(matches!(
        &signature.generic_params()[0],
        GenericParam::Lifetime(lifetime) if lifetime.name() == "a"
    ));
    assert!(matches!(
        &signature.generic_params()[1],
        GenericParam::Type(name) if name == "T"
    ));
    assert_eq!(signature.param_groups().len(), 2);
    assert_eq!(signature.param_groups()[0].params().len(), 1);
    assert_eq!(signature.param_groups()[1].params().len(), 1);
    assert!(matches!(
        signature.return_type(),
        Some(TypeRef::Generic { base, args }) if base == "ArcResult" && args.len() == 1
    ));
    assert_eq!(signature.where_clauses().len(), 1);
    assert_eq!(signature.where_clauses()[0].bounds().len(), 2);
}

#[test]
fn function_signatures_reject_trailing_garbage() {
    let error = parse_fn_signature("fn f(x: i32) -> i32 unexpected")
        .expect_err("trailing tokens after return type are rejected");

    assert!(error.to_string().contains("unexpected"));
}

#[test]
fn function_signatures_keep_rest_parameters() {
    let signature = parse_fn_signature("fn log(message: String, fields: ...LogField) -> Unit")
        .expect("rest parameter signature parses");
    let params = signature.param_groups()[0].params();

    assert_eq!(params[0].kind(), FnParamKind::Fixed);
    assert_eq!(params[1].kind(), FnParamKind::Rest);
    assert!(matches!(params[1].ty(), TypeRef::Path(path) if path == "LogField"));
}

#[test]
fn function_signatures_reject_misplaced_rest_parameters() {
    let in_middle = parse_fn_signature("fn f(xs: ...Int, y: Int) -> Unit")
        .expect_err("rest in the middle is rejected");
    let curried = parse_fn_signature("fn f(xs: ...Int)(y: Int) -> Unit")
        .expect_err("rest before a curried group is rejected");
    let defaulted = parse_fn_signature("fn f(xs: ...Int = []) -> Unit")
        .expect_err("defaulted rest is rejected");

    assert!(in_middle.to_string().contains("last parameter"));
    assert!(curried.to_string().contains("final group"));
    assert!(defaulted.to_string().contains("default"));
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
fn hook_headers_keep_when_priority_once_and_effects() {
    let tree = parse_ok(
        r"
hook @hook.choice_visible
on @choice.opening.listen
phase AfterLayout
when choice_enabled(state)
priority -5
once
effects signal.choice_visible, ui.patch
{
    signal.set(@signal.choice_visible, true)
}
",
    );

    let Item::Hook(hook) = &tree.items()[0] else {
        panic!("expected hook");
    };
    assert!(hook.when().is_some());
    assert_eq!(hook.priority(), Some(-5));
    assert!(hook.once());
    assert_eq!(hook.effects().len(), 2);
}

#[test]
fn dialogue_defaults_are_preserved_as_top_level_declarations() {
    let source = r"
pub dialogue defaults @dialogue.defaults {
    window = @textbox.0
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
