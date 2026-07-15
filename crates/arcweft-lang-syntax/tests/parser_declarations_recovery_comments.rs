use arcweft_lang_syntax::{
    ast::{
        common::UseTreeKind,
        flow::{FlowItem, Stmt},
        items::{Item, MemoOption, RawSyntaxFamily},
    },
    expr::Expr,
    types::TypeRef,
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
fn flow_body_attributes_are_explicit_recovery_diagnostics() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r"
flow opening {
    #![generated(tool)]
    #[allow(style::redundant_decl_identity)]
    alice: hello[p]
}
",
    );

    assert_eq!(parsed.errors().len(), 2);
    assert!(parsed.errors()[0].message().contains("inner attributes"));
    assert!(parsed.errors()[1].message().contains("outer attributes"));
    let tree = parsed.typed_tree();
    let arcweft_lang_syntax::ast::items::Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    assert_eq!(flow.body().len(), 3);
    assert!(matches!(
        flow.body()[0],
        arcweft_lang_syntax::ast::flow::FlowItem::Raw(_)
    ));
    assert!(matches!(
        flow.body()[1],
        arcweft_lang_syntax::ast::flow::FlowItem::Raw(_)
    ));
}

#[test]
fn unrecognized_import_prefixes_use_generic_top_level_recovery() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r"
deferred use game.heavy.{shader}
scheduled use game.generated.{RouteMap}
use game.prelude.*
",
    );

    assert_eq!(parsed.errors().len(), 2);
    assert!(
        parsed
            .errors()
            .iter()
            .all(|error| error.message() == "unexpected top-level item")
    );
    let tree = parsed.typed_tree();
    assert_eq!(tree.uses().len(), 1);
    assert_eq!(tree.uses()[0].tree().source(), "game.prelude.*");
}

#[test]
fn unknown_braced_top_level_item_uses_generic_recovery() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
pub unknown_panel alice {
    display = "Alice"
}

pub character bob {}
"#,
    );

    assert_eq!(parsed.errors().len(), 1);
    let error = &parsed.errors()[0];
    assert_eq!(error.message(), "unexpected top-level item");
    assert_eq!(error.found(), Some("pub unknown_panel alice {"));
    assert!(
        error
            .recovery()
            .iter()
            .any(|suggestion| suggestion.message().contains("current Arcweft"))
    );
    assert!(matches!(
        parsed.typed_tree().items(),
        [Item::Raw(_), Item::EntityDecl(item)]
            if item.kind() == arcweft_lang_syntax::ast::items::EntityDeclKind::Character
            && item.id().body() == "character.bob"
    ));
}

#[test]
fn arbitrary_unknown_braced_item_recovers_to_the_next_declaration() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        "pub unknown_widget legacy {\n color = rgb(\"#fff\")\n}\npub view DialoguePanel() {\n Text(\"ok\")\n}\n",
    );

    assert_eq!(parsed.errors().len(), 1);
    assert_eq!(parsed.errors()[0].message(), "unexpected top-level item");
    assert_eq!(
        parsed.errors()[0].found(),
        Some("pub unknown_widget legacy {")
    );
    assert!(matches!(
        parsed.typed_tree().items(),
        [Item::Raw(_), Item::EntityDecl(item)]
            if item.kind() == arcweft_lang_syntax::ast::items::EntityDeclKind::View
    ));
}

#[test]
fn namespace_separator_is_rejected_in_module_paths() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        "mod game::opening\nflow @flow.opening opening { return }\n",
    );

    assert_eq!(parsed.errors().len(), 1);
    assert!(
        parsed.errors()[0]
            .message()
            .contains("module paths use `.` separators")
    );
    assert!(parsed.typed_tree().module().is_none());
}

#[test]
fn use_tree_exposes_typed_module_prefixes() {
    let tree = parse_ok(
        r"
use parent.shared.{alpha, beta}
pub use crate.game.routes.opening as opening_route
use self.prelude.*
",
    );

    assert_eq!(tree.uses().len(), 3);
    assert_eq!(tree.uses()[0].tree().source(), "super.shared.{alpha, beta}");
    assert_eq!(
        tree.uses()[0].tree().module_path_prefix().to_string(),
        "super.shared"
    );
    assert!(tree.uses()[0].tree().module_path_is_exact());
    let UseTreeKind::Group { module, names } = tree.uses()[0].tree().kind() else {
        panic!("expected grouped use tree");
    };
    assert_eq!(module.to_string(), "super.shared");
    assert_eq!(names.len(), 2);
    assert_eq!(names[0].name().as_str(), "alpha");
    assert_eq!(names[0].binding_name().as_str(), "alpha");
    assert_eq!(
        tree.uses()[1].tree().module_path_prefix().to_string(),
        "crate.game.routes.opening"
    );
    assert!(!tree.uses()[1].tree().module_path_is_exact());
    let UseTreeKind::Path { path, alias } = tree.uses()[1].tree().kind() else {
        panic!("expected aliased path use tree");
    };
    assert_eq!(path.to_string(), "crate.game.routes.opening");
    assert_eq!(
        alias
            .as_ref()
            .map(arcweft_lang_syntax::ast::module_path::ModuleSegment::as_str),
        Some("opening_route")
    );
    assert_eq!(
        tree.uses()[2].tree().module_path_prefix().to_string(),
        "self.prelude"
    );
    assert!(tree.uses()[2].tree().module_path_is_exact());
    let UseTreeKind::Glob { module } = tree.uses()[2].tree().kind() else {
        panic!("expected glob use tree");
    };
    assert_eq!(module.to_string(), "self.prelude");
}

#[test]
fn malformed_grouped_use_reports_a_structured_parse_diagnostic() {
    let parsed = arcweft_lang_syntax::parser::parse_source("use game.effects.{wave,,pulse}\n");

    assert_eq!(parsed.errors().len(), 1);
    assert!(parsed.errors()[0].message().contains("empty name"));
    assert!(parsed.typed_tree().uses().is_empty());
}

#[test]
fn content_declaration_parses_as_typed_entity_body() {
    let tree = parse_ok(
        r"
content chapter_two {
    roots = [
        @flow:.chapter_two,
        @asset:.bg.room,
    ]
}
",
    );

    let arcweft_lang_syntax::ast::items::Item::EntityDecl(content) = &tree.items()[0] else {
        panic!("expected content entity declaration");
    };
    assert_eq!(
        content.kind(),
        arcweft_lang_syntax::ast::items::EntityDeclKind::Content
    );
    assert_eq!(content.id().body(), "content.chapter_two");
    assert!(content.body().is_none());
    let body = content.content_body().expect("content body is typed");
    assert_eq!(body.roots().len(), 2);
    assert_eq!(body.roots()[0].body(), "flow.chapter_two");
    assert_eq!(body.roots()[1].body(), "asset.bg.room");
}

#[test]
fn action_declaration_parses_as_typed_entity() {
    let tree = parse_ok(
        r"
pub action feedback.submit_name(value: String)
",
    );

    let arcweft_lang_syntax::ast::items::Item::EntityDecl(action) = &tree.items()[0] else {
        panic!("expected action entity declaration");
    };
    assert_eq!(
        action.kind(),
        arcweft_lang_syntax::ast::items::EntityDeclKind::Action
    );
    assert_eq!(action.id().body(), "action.feedback.submit_name");
    assert_eq!(action.signature_tail(), "(value: String)");
    assert!(action.body().is_none());
    assert!(action.structured_body().is_none());
}

#[test]
fn entity_headers_accept_the_shared_typed_tail_grammar() {
    let tree = parse_ok(
        r#"
#[generated(tool)]
pub view GenericPanel<T: Display>(value: T) {
    Text("ok")
}

pub action feedback.submit<T>(value: T)
pub signal current_flow: Watch<Ref<Flow>>
pub layer overlay: NativeView {}
pub audio bus music parent @bus.master {}
pub character @character.alice Alice as alice {}
"#,
    );

    let entities = tree
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::EntityDecl(entity) => Some(entity),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(entities.len(), 6);
    assert_eq!(entities[0].signature_tail(), "<T: Display>(value: T)");
    assert_eq!(entities[1].signature_tail(), "<T>(value: T)");
    assert_eq!(entities[2].signature_tail(), ": Watch<Ref<Flow>>");
    assert_eq!(entities[3].signature_tail(), ": NativeView");
    assert_eq!(entities[4].signature_tail(), "parent @bus.master");
    assert_eq!(entities[5].surface_alias(), Some("alice"));
}

#[test]
fn invalid_entity_block_header_is_not_an_ast_node_and_recovers_after_its_block() {
    let source = r#"pub asset set foo {
    file = "obsolete.png"
}
pub character bob {}
"#;
    let parsed = arcweft_lang_syntax::parser::parse_source(source);

    assert_eq!(parsed.errors().len(), 1);
    let error = &parsed.errors()[0];
    assert_eq!(
        error.message(),
        "unexpected token in entity declaration header"
    );
    assert_eq!(error.found(), Some("foo"));
    assert_eq!(&source[error.range().as_range()], "foo");
    assert!(matches!(
        parsed.typed_tree().items(),
        [Item::EntityDecl(item)]
            if item.kind() == arcweft_lang_syntax::ast::items::EntityDeclKind::Character
                && item.id().body() == "character.bob"
    ));
}

#[test]
fn invalid_entity_line_header_recovers_at_the_next_declaration() {
    let source =
        "pub action feedback.submit payload junk\npub view StatusPanel() { Text(\"ok\") }\n";
    let parsed = arcweft_lang_syntax::parser::parse_source(source);

    assert_eq!(parsed.errors().len(), 1);
    let error = &parsed.errors()[0];
    assert_eq!(
        error.message(),
        "unexpected token in entity declaration header"
    );
    assert_eq!(error.found(), Some("payload"));
    assert_eq!(&source[error.range().as_range()], "payload");
    assert!(matches!(
        parsed.typed_tree().items(),
        [Item::EntityDecl(item)]
            if item.kind() == arcweft_lang_syntax::ast::items::EntityDeclKind::View
                && item.id().body() == "view.StatusPanel"
    ));
}

#[test]
fn at_is_entity_ref_and_slash_comments_are_comments() {
    let tree = parse_ok(
        r"
// ordinary comment
flow @flow.opening opening {
    goto @flow.title
}
",
    );
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    assert_eq!(flow.id().expect("flow id").body(), "flow.opening");

    let tree = parse_ok("// ordinary comment only");
    assert!(tree.items().is_empty());
}

#[test]
fn block_comments_are_comments() {
    let tree = parse_ok(
        r"
/*
ordinary block comment
*/
flow @flow.opening opening {
    goto @flow.title
}
",
    );
    let arcweft_lang_syntax::ast::items::Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    assert_eq!(flow.id().expect("flow id").body(), "flow.opening");
}

#[test]
fn doc_comments_attach_to_function_and_parameters() {
    let tree = parse_ok(
        r#"
/// Opens a route.
pub fn open_route(
    /// Current game state.
    state: GameState,
) -> ! {
    panic("todo")
}
"#,
    );
    let Item::Function(function) = &tree.items()[0] else {
        panic!("expected function");
    };
    assert_eq!(
        function.doc().expect("function doc").text(),
        "Opens a route."
    );
    assert_eq!(
        function.signature().param_groups()[0].params()[0]
            .doc()
            .expect("param doc")
            .text(),
        "Current game state."
    );
    assert!(matches!(
        function.signature().return_type(),
        Some(TypeRef::Never)
    ));
}

#[test]
fn flow_recovery_nodes_keep_family_and_source_range() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r"
flow @flow.raw_example {
    unknown surface form
}
",
    );
    assert_eq!(parsed.errors().len(), 1);
    assert_eq!(parsed.errors()[0].message(), "unsupported flow item");
    let tree = parsed.typed_tree();
    let Item::Flow(flow) = &tree.items()[0] else {
        panic!("expected flow");
    };
    let FlowItem::Raw(raw) = &flow.body()[0] else {
        panic!("expected recovery node");
    };
    assert_eq!(raw.family(), RawSyntaxFamily::FlowItem);
    assert_eq!(raw.source(), "unknown surface form");
    assert!(raw.range().is_some());
}

#[test]
fn statement_recovery_nodes_keep_family_and_source_range() {
    let tree = parse_ok(
        r"
fn bad_stmt() -> Unit {
    let broken
}
",
    );
    let Item::Function(function) = &tree.items()[0] else {
        panic!("expected function");
    };
    let Stmt::Raw(raw) = &function.body_statements()[0] else {
        panic!("expected raw statement recovery node");
    };
    assert_eq!(raw.family(), RawSyntaxFamily::Stmt);
    assert_eq!(raw.source(), "let broken");
    assert!(raw.range().is_some());
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
effects signal.choice_visible, view.patch
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
fn hook_targets_accept_only_current_grammar_families() {
    let tree = parse_ok(
        r"
hook @hook.state_target
on state .flags.ready
phase StateChanged
{
}

hook @hook.signal_target
on signal @signal.ready
phase SignalChanged
{
}

hook @hook.query_target
on query ChoiceOption where parent == @choice.opening
phase AfterViewDiff
{
}
",
    );

    assert_eq!(tree.items().len(), 3);
    assert!(
        tree.items()
            .iter()
            .all(|item| matches!(item, Item::Hook(_)))
    );
}

#[test]
fn memo_headers_keep_only_valid_typed_options() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r"
memo fn compute(value: Value) -> Result
scope = scene
key = value.id
eviction = eager
track auto
{
    build(value)
}
",
    );

    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.message() == "unknown memo option")
    );
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.message() == "invalid memo option")
    );
    let [Item::MemoFn(memo)] = parsed.typed_tree().items() else {
        panic!("expected one recovered memo function");
    };
    assert!(matches!(
        memo.options(),
        [MemoOption::Scope(Expr::Path(scope)), MemoOption::Key(Expr::Select(_))]
            if scope == "scene"
    ));
}

#[test]
fn repeated_header_diagnostics_use_each_authored_line_range() {
    let hook_source = r"
hook @hook.range_check
on @choice.opening
phase AfterViewDiff
channel pointer
channel pointer
{
}
";
    let parsed = arcweft_lang_syntax::parser::parse_source(hook_source);
    let actual_hook_ranges = parsed
        .errors()
        .iter()
        .filter(|error| error.message() == "unknown hook header")
        .map(|error| (error.range().start(), error.range().end()))
        .collect::<Vec<_>>();
    let expected_hook_ranges = hook_source
        .match_indices("channel pointer")
        .map(|(start, line)| (start, start + line.len()))
        .collect::<Vec<_>>();
    assert_eq!(actual_hook_ranges, expected_hook_ranges);

    let memo_source = r"
memo fn range_check(value: Value) -> Result
scope = scene
scope = scene
{
    build(value)
}
";
    let parsed = arcweft_lang_syntax::parser::parse_source(memo_source);
    let duplicate = parsed
        .errors()
        .iter()
        .find(|error| error.message() == "duplicate memo option")
        .expect("duplicate memo option diagnostic");
    let expected_start = memo_source
        .rfind("scope = scene")
        .expect("second option line");
    assert_eq!(duplicate.range().start(), expected_start);
    assert_eq!(
        duplicate.range().end(),
        expected_start + "scope = scene".len()
    );
}
