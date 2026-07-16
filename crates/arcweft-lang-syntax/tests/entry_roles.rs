use arcweft_lang_syntax::{
    ast::items::{EntryItem, EntryKind, Item},
    parser::parse_source,
    types::TypeRef,
};

fn entry(source: &str) -> arcweft_lang_syntax::ast::items::EntryDeclItem {
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    parsed
        .into_typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Entry(entry) => Some(entry.clone()),
            _ => None,
        })
        .expect("entry parses")
}

fn source_range(source: &str, fragment: &str) -> std::ops::Range<usize> {
    let start = source.find(fragment).expect("fixture fragment exists");
    start..start + fragment.len()
}

#[test]
fn stateful_entry_roles_are_typed_and_keep_value_and_member_ranges() {
    let source = r"entry game @entry.game.main {
    reducer = game.reduce
    state = GameState
    goto @flow.opening
    event = GameEvent
    initializer = game.initial_state
}";
    let entry = entry(source);
    assert_eq!(entry.kind(), &EntryKind::Game);

    let [reducer, state, EntryItem::Goto(target), event, initializer] = entry.items() else {
        panic!("unexpected entry items: {:?}", entry.items());
    };
    let EntryItem::Reducer {
        path,
        value_range,
        range,
    } = reducer
    else {
        panic!("expected reducer role");
    };
    assert_eq!(path.as_str(), "game.reduce");
    assert_eq!(value_range.as_range(), source_range(source, "game.reduce"));
    assert_eq!(
        range.as_range(),
        source_range(source, "reducer = game.reduce")
    );

    let EntryItem::StateType {
        ty,
        value_range,
        range,
    } = state
    else {
        panic!("expected state role");
    };
    assert_eq!(ty, &TypeRef::Path("GameState".to_owned()));
    assert_eq!(value_range.as_range(), source_range(source, "GameState"));
    assert_eq!(range.as_range(), source_range(source, "state = GameState"));
    assert_eq!(target.body(), "flow.opening");
    assert!(matches!(
        event,
        EntryItem::EventType {
            ty: TypeRef::Path(name),
            ..
        } if name == "GameEvent"
    ));
    assert!(matches!(
        initializer,
        EntryItem::Initializer { path, .. } if path.as_str() == "game.initial_state"
    ));
}

#[test]
fn editor_test_and_agent_are_direct_entry_kinds() {
    let editor = entry(
        "entry editor @entry.editor.main {\nstate = EditorState\ninitializer = init\nevent = EditorEvent\nreducer = reduce\ngoto @flow.home\n}",
    );
    let test = entry(
        "entry test @entry.test.main {\nstate = TestState\ninitializer = init\nevent = TestEvent\nreducer = reduce\ngoto @flow.test\n}",
    );
    let agent = entry("entry agent @entry.agent.smoke {\ncontroller = agents.opening_smoke\n}");

    assert_eq!(editor.kind(), &EntryKind::Editor);
    assert_eq!(test.kind(), &EntryKind::Test);
    assert_eq!(agent.kind(), &EntryKind::Agent);
    assert!(matches!(
        agent.items(),
        [EntryItem::Controller { path, .. }] if path.as_str() == "agents.opening_smoke"
    ));
}

#[test]
fn entry_kind_and_id_are_both_explicit() {
    for source in [
        "entry @entry.game.main { goto @flow.main }",
        "entry game { goto @flow.main }",
        "entry game @flow.main { goto @flow.main }",
    ] {
        let parsed = parse_source(source);
        assert!(!parsed.errors().is_empty(), "{source}");
        assert!(
            parsed
                .typed_tree()
                .items()
                .iter()
                .all(|item| !matches!(item, Item::Entry(_))),
            "invalid entry must not become executable typed syntax: {source}"
        );
    }
}

#[test]
fn duplicate_roles_relate_the_first_and_duplicate_members() {
    for (role, first, second) in [
        ("state", "GameState", "OtherState"),
        ("reducer", "reduce_game", "replace_game"),
        ("controller", "run_agent", "inspect_agent"),
    ] {
        let first_member = format!("{role} = {first}");
        let duplicate_member = format!("{role} = {second}");
        let source =
            format!("entry game @entry.game.main {{\n{first_member}\n{duplicate_member}\n}}");
        let parsed = parse_source(&source);
        let duplicate = parsed
            .errors()
            .iter()
            .find(|error| error.code() == "syntax.entry.duplicate_role")
            .expect("duplicate diagnostic");
        assert_eq!(
            duplicate.range().as_range(),
            source_range(&source, &duplicate_member)
        );
        assert_eq!(duplicate.related().len(), 1);
        assert_eq!(
            duplicate.related()[0].range().as_range(),
            source_range(&source, &first_member)
        );
    }
}

#[test]
fn reserved_role_names_never_fall_through_to_generic_options() {
    let source = r"entry game @entry.game.main {
state = Vec<
reducer = reduce()
controller controller_fn
}";
    let parsed = parse_source(source);
    let entry = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Entry(entry) => Some(entry),
            _ => None,
        })
        .expect("entry remains available for recovery");
    assert!(entry.items().iter().all(|item| {
        !matches!(
            item,
            EntryItem::Option { name, .. }
                if matches!(
                    name.as_str(),
                    "state" | "initializer" | "event" | "reducer" | "controller"
                )
        )
    }));
    assert!(parsed.errors().iter().any(|error| {
        matches!(
            error.code(),
            "syntax.parse" | "syntax.entry.role_path" | "syntax.entry.role_binding"
        )
    }));
}

#[test]
fn entry_kind_rejects_incompatible_typed_roles_and_routes() {
    let parsed = parse_source(
        "entry game @entry.game.main {\ncontroller = smoke\nroute GET \"/\" -> @flow.main\n}",
    );
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.code() == "syntax.entry.incompatible_role")
    );
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.code() == "syntax.entry.incompatible_route")
    );
}

#[test]
fn stateful_entry_requires_exactly_one_initial_goto() {
    let missing = parse_source(
        "entry game @entry.game.main {\nstate = GameState\ninitializer = init\nevent = GameEvent\nreducer = reduce\n}",
    );
    assert!(
        missing
            .errors()
            .iter()
            .any(|error| error.code() == "syntax.entry.missing_goto")
    );

    let source = "entry game @entry.game.main {\nstate = GameState\ninitializer = init\nevent = GameEvent\nreducer = reduce\ngoto @flow.first\ngoto @flow.second\n}";
    let duplicate = parse_source(source);
    let error = duplicate
        .errors()
        .iter()
        .find(|error| error.code() == "syntax.entry.duplicate_goto")
        .expect("duplicate goto diagnostic");
    assert_eq!(
        error.range().as_range(),
        source_range(source, "@flow.second")
    );
    assert_eq!(error.related().len(), 1);
    assert_eq!(
        error.related()[0].range().as_range(),
        source_range(source, "@flow.first")
    );
}
