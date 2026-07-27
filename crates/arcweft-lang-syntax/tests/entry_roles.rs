use arcweft_lang_syntax::{
    ast::items::{EntryDeclItem, EntryItem, EntryKind, Item},
    parser::recovery::ParseErrorKind,
    source::ParsedSource,
    types::TypeRef,
};

fn entry(parsed: &ParsedSource) -> &EntryDeclItem {
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Entry(entry) => Some(entry),
            _ => None,
        })
        .expect("entry parses")
}

fn source_range(source: &str, fragment: &str) -> std::ops::Range<usize> {
    let start = source.find(fragment).expect("fixture fragment exists");
    start..start + fragment.len()
}

fn assert_parser_kind(source: &str, kind: ParseErrorKind) {
    let parsed = parse_entry_role_fixture(source);
    assert!(
        parsed.errors().iter().any(|error| error.kind() == kind),
        "expected {kind:?} for {source:?}, got {:?}",
        parsed.errors()
    );
}

fn assert_parser_range(source: &str, kind: ParseErrorKind, expected: std::ops::Range<usize>) {
    let parsed = parse_entry_role_fixture(source);
    let error = parsed
        .errors()
        .iter()
        .find(|error| error.kind() == kind)
        .unwrap_or_else(|| {
            panic!(
                "expected {kind:?} for {source:?}, got {:?}",
                parsed.errors()
            )
        });
    assert_eq!(error.range().as_range(), expected);
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
    let parsed = parse_entry_role_fixture(source);
    let entry = entry(&parsed);
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
    assert!(matches!(ty.value(), TypeRef::Path(path) if path.canonical_string() == "GameState"));
    assert_eq!(value_range.as_range(), source_range(source, "GameState"));
    assert_eq!(range.as_range(), source_range(source, "state = GameState"));
    assert_eq!(target.body(), "flow.opening");
    assert!(matches!(
        event,
        EntryItem::EventType {
            ty,
            ..
        } if matches!(ty.value(), TypeRef::Path(name) if name.canonical_string() == "GameEvent")
    ));
    assert!(matches!(
        initializer,
        EntryItem::Initializer { path, .. } if path.as_str() == "game.initial_state"
    ));
}

#[test]
fn editor_test_and_agent_are_direct_entry_kinds() {
    let editor_parsed = parse_entry_role_fixture(
        "entry editor @entry.editor.main {\nstate = EditorState\ninitializer = init\nevent = EditorEvent\nreducer = reduce\ngoto @flow.home\n}",
    );
    let editor = entry(&editor_parsed);
    let test_parsed = parse_entry_role_fixture(
        "entry test @entry.test.main {\nstate = TestState\ninitializer = init\nevent = TestEvent\nreducer = reduce\ngoto @flow.test\n}",
    );
    let test = entry(&test_parsed);
    let agent_parsed = parse_entry_role_fixture(
        "entry agent @entry.agent.smoke {\ncontroller = agents.opening_smoke\n}",
    );
    let agent = entry(&agent_parsed);

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
        let parsed = parse_entry_role_fixture(source);
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
        let parsed = parse_entry_role_fixture(&source);
        let duplicate = parsed
            .errors()
            .iter()
            .find(|error| error.kind() == ParseErrorKind::EntryDuplicateRole)
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
    let parsed = parse_entry_role_fixture(source);
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
            error.kind(),
            ParseErrorKind::Generic
                | ParseErrorKind::EntryRolePath
                | ParseErrorKind::EntryRoleBinding
        )
    }));
}

#[test]
fn entry_kind_rejects_incompatible_typed_roles_and_routes() {
    let parsed = parse_entry_role_fixture(
        "entry game @entry.game.main {\ncontroller = smoke\nroute GET \"/\" -> @flow.main\n}",
    );
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.kind() == ParseErrorKind::EntryIncompatibleRole)
    );
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.kind() == ParseErrorKind::EntryIncompatibleRoute)
    );
}

#[test]
fn stateful_entry_requires_exactly_one_initial_goto() {
    let missing = parse_entry_role_fixture(
        "entry game @entry.game.main {\nstate = GameState\ninitializer = init\nevent = GameEvent\nreducer = reduce\n}",
    );
    assert!(
        missing
            .errors()
            .iter()
            .any(|error| error.kind() == ParseErrorKind::EntryMissingGoto)
    );

    let source = "entry game @entry.game.main {\nstate = GameState\ninitializer = init\nevent = GameEvent\nreducer = reduce\ngoto @flow.first\ngoto @flow.second\n}";
    let duplicate = parse_entry_role_fixture(source);
    let error = duplicate
        .errors()
        .iter()
        .find(|error| error.kind() == ParseErrorKind::EntryDuplicateGoto)
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

#[test]
fn entry_producers_cover_every_typed_entry_diagnostic_kind() {
    let fixtures = [
        (
            "entry @entry.game.main {\n}\n",
            ParseErrorKind::EntryMissingKind,
        ),
        ("entry game {\n}\n", ParseErrorKind::EntryMissingId),
        (
            "entry game @flow.main {\n}\n",
            ParseErrorKind::EntryIdFamily,
        ),
        (
            "entry game @entry.game.main trailing {\n}\n",
            ParseErrorKind::EntryTrailingHead,
        ),
        (
            "entry game @entry.game.main {\nstate = GameState\nstate = OtherState\n}\n",
            ParseErrorKind::EntryDuplicateRole,
        ),
        (
            "entry agent @entry.agent.main {\nstate = GameState\n}\n",
            ParseErrorKind::EntryIncompatibleRole,
        ),
        (
            "entry game @entry.game.main {\ngoto @flow.first\ngoto @flow.second\n}\n",
            ParseErrorKind::EntryDuplicateGoto,
        ),
        (
            "entry agent @entry.agent.main {\ngoto @flow.first\n}\n",
            ParseErrorKind::EntryIncompatibleGoto,
        ),
        (
            "entry game @entry.game.main {\nroute GET \"/\" -> @flow.first\n}\n",
            ParseErrorKind::EntryIncompatibleRoute,
        ),
        (
            "entry agent @entry.agent.main {\n}\n",
            ParseErrorKind::EntryMissingRole,
        ),
        (
            "entry game @entry.game.main {\nstate = GameState\ninitializer = init\nevent = GameEvent\nreducer = reduce\n}\n",
            ParseErrorKind::EntryMissingGoto,
        ),
        (
            "entry agent @entry.agent.main {\ncontroller smoke\n}\n",
            ParseErrorKind::EntryRoleBinding,
        ),
        (
            "entry agent @entry.agent.main {\ncontroller =\n}\n",
            ParseErrorKind::EntryRoleValue,
        ),
        (
            "entry agent @entry.agent.main {\ncontroller = smoke()\n}\n",
            ParseErrorKind::EntryRolePath,
        ),
    ];

    for (source, kind) in fixtures {
        assert_parser_kind(source, kind);
    }
    assert_parser_kind(
        "entry agent @entry.agent.main {\ncontroller alias = smoke\n}\n",
        ParseErrorKind::EntryRoleBinding,
    );
}

#[test]
fn entry_head_diagnostics_use_exact_source_ranges_after_whitespace_normalization() {
    let missing_kind = "   entry    @entry.game.main {\n}\n";
    assert_parser_range(
        missing_kind,
        ParseErrorKind::EntryMissingKind,
        source_range(missing_kind, "@entry.game.main"),
    );

    let missing_id = "   entry    game    {\n}\n";
    let missing_id_offset = missing_id.find("game").expect("entry kind") + "game".len();
    assert_parser_range(
        missing_id,
        ParseErrorKind::EntryMissingId,
        missing_id_offset..missing_id_offset,
    );

    let trailing = "   entry   game   @entry.game.main     trailing   {\n}\n";
    assert_parser_range(
        trailing,
        ParseErrorKind::EntryTrailingHead,
        source_range(trailing, "trailing"),
    );
}

#[test]
fn malformed_nominal_generic_parameters_have_a_typed_group_range() {
    for source in [
        "struct Broken<,> {\nvalue: i32\n}\n",
        "enum Broken<,> {\nValue\n}\n",
    ] {
        let parsed = parse_entry_role_fixture(source);
        let error = parsed
            .errors()
            .iter()
            .find(|error| error.kind() == ParseErrorKind::NominalInvalidGenericParameters)
            .expect("typed nominal generic diagnostic");
        assert_eq!(error.code(), "syntax.nominal.invalid_generic_parameters");
        assert_eq!(error.range().as_range(), source_range(source, "<,>"));
    }
}

fn parse_entry_role_fixture(
    source: impl Into<String>,
) -> arcweft_lang_syntax::source::ParsedSource {
    let document = std::sync::Arc::new(
        arcweft_source::SourceDocument::try_new(
            arcweft_source::SourceDocumentId::try_new("arcweft-test://syntax/entry-roles")
                .expect("fixed test document ID is valid"),
            arcweft_source::SourceName::path("entry-roles.arcw"),
            source.into(),
        )
        .expect("test source document"),
    );
    arcweft_lang_syntax::parser::parse_document_with_source(
        document,
        arcweft_lang_syntax::parser::ParseOptions::default(),
    )
}
