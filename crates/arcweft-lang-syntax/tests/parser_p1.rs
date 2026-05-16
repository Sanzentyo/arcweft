fn parse_ok(source: impl Into<String>) -> arcweft_lang_syntax::TypedSyntaxTree {
    let parsed = arcweft_lang_syntax::parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "expected source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed.into_typed_tree()
}

use arcweft_lang_syntax::{
    Expr, FlowItem, GenericParam, HirTopLevelDecl, Item, TypeRef, parse_fn_signature,
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
fn dialogue_line_options_are_structured_not_raw_args() {
    let tree = parse_ok(
        r#"
alice(id=@say.opening.dream_hint, text_key=@text.opening.dream_hint, voice=auto, window=@textbox.side, hooks=[@hook.dialogue.read_state_color], style=@style.dream, look=smile, source_locale="ja-JP"): 今日は少しだけ。[p]
"#,
    );

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
    assert!(matches!(options.look(), Some(Expr::Path(path)) if path == "smile"));
    assert!(options.args().is_empty());
    assert_eq!(options.source_locale(), Some("\"ja-JP\""));
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
    let tree = parse_ok(
        r"
pub dialogue defaults @dialogue.defaults {
    window = @textbox.0
    voice = auto
    style = @style.dialogue.default
}
",
    );

    let Item::DialogueDefaults(defaults) = &tree.items()[0] else {
        panic!("expected dialogue defaults");
    };
    assert_eq!(
        defaults.id().expect("defaults id").body(),
        "dialogue.defaults"
    );
    assert_eq!(defaults.options().len(), 3);

    let hir = arcweft_lang_syntax::lower_to_hir(&tree).expect("defaults lower");
    assert!(matches!(
        hir.declarations(),
        [HirTopLevelDecl::DialogueDefaults(_)]
    ));
}
