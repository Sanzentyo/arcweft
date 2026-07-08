use arcweft_lang_syntax::{
    ast::{
        flow::{FlowItem, Stmt},
        items::{Item, RawSyntaxFamily},
    },
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
fn removed_import_execution_modes_are_parse_diagnostics() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r"
lazy use game.heavy.{shader}
eager use game.generated.{RouteMap}
use game.prelude.*
",
    );

    assert_eq!(parsed.errors().len(), 2);
    assert!(parsed.errors().iter().all(|error| {
        error
            .message()
            .contains("`lazy use` and `eager use` were removed")
    }));
    let tree = parsed.typed_tree();
    assert_eq!(tree.uses().len(), 1);
    assert_eq!(tree.uses()[0].tree().source(), "game.prelude.*");
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
    assert_eq!(
        tree.uses()[1].tree().module_path_prefix().to_string(),
        "crate.game.routes.opening"
    );
    assert!(!tree.uses()[1].tree().module_path_is_exact());
    assert_eq!(
        tree.uses()[2].tree().module_path_prefix().to_string(),
        "self.prelude"
    );
    assert!(tree.uses()[2].tree().module_path_is_exact());
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
fn asset_set_is_not_v1_source_syntax() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r"
asset set @asset_set.route_portraits {
    members = [
        @asset:.portrait.alice,
    ]
}
",
    );

    assert_eq!(parsed.errors().len(), 1);
    assert!(
        parsed.errors()[0]
            .message()
            .contains("`asset set` is not part of the v1 Arcweft source grammar")
    );
    assert!(parsed.typed_tree().items().is_empty());
}

#[test]
fn hot_checkpoint_is_not_v1_source_syntax() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r"
hot checkpoint before_boss {
    roots = [@flow.chapter_two]
}
",
    );

    assert_eq!(parsed.errors().len(), 1);
    assert!(
        parsed.errors()[0]
            .message()
            .contains("`hot checkpoint` is not part of the v1 Arcweft source grammar")
    );
    assert!(parsed.typed_tree().items().is_empty());
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
    let tree = parse_ok(
        r"
flow @flow.raw_example {
    unknown surface form
}
",
    );
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
