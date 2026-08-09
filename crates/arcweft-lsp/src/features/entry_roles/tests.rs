#![allow(
    clippy::mutable_key_type,
    reason = "WorkspaceEdit exposes the LSP Uri type as its required changes-map key"
)]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use arcweft_lang_hir::{
    item::HirItemFamily,
    source_index::{
        HirCallableSourceOwner, HirCallableSourceRole, HirDeclarationSourceRole, HirItemSourceRole,
    },
    symbol::CallableDeclarationOwner,
};
use arcweft_lang_syntax::attachment::TypedItemNode;
use lsp_types::{
    DidOpenTextDocumentParams, DocumentSymbolResponse, TextDocumentItem, Uri,
    WorkspaceSymbolResponse,
};

use super::*;
use crate::{
    documents::{AcceptedOpenDocument, DocumentStore},
    positions::PositionEncoding,
    profiles::{LspProfile, LspProfileResolver, LspProfileTestHarness},
};
use arcweft_runtime_host::RuntimeHostRunnerKind;

const SOURCE: &str = r"
fn smoke() -> Result<Unit, AgentError>
effects {}
{
Ok(())
}

fn invoke() -> Result<Unit, AgentError> {
smoke()
}

fn shadowed() -> Unit {
let smoke: (Unit) -> Unit effects {} = |_unit: Unit| -> Unit { () }
smoke(())
}

fn selected_entry() -> Unit {
let selected = @entry.agent.main
()
}

entry agent @entry.agent.main {
controller = smoke
}
";

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end tooling scenario verifies one shared callable across navigation and rename surfaces"
)]
fn role_rhs_uses_the_ordinary_callable_for_definition_hover_and_rename() {
    let project = TestProject::new("entry-role-tooling");
    project.write_manifest();
    project.write("src/main.arcw", SOURCE);
    let source_path = project.path("src/main.arcw");
    let profile = LspProfileTestHarness::new(LspProfileResolver::new(
        RuntimeHostRunnerKind::Native,
        Some("agent".to_owned()),
    ))
    .resolve_for_document_path(&source_path)
    .expect("profile construction")
    .publish_for_test();
    assert!(
        profile.diagnostics().is_empty(),
        "{:?}",
        profile.diagnostics()
    );
    let document = open_accepted(&profile, &source_path, SOURCE, PositionEncoding::Utf16);
    let offset = SOURCE.rfind("smoke").expect("controller role");

    assert!(definition(&profile, &document, offset).is_some());
    let HoverContents::Scalar(MarkedString::String(hover_text)) =
        hover(&profile, &document, offset).expect("hover").contents
    else {
        panic!("expected string hover");
    };
    assert!(hover_text.contains("bound as `controller`"));
    let DocumentSymbolResponse::Nested(outline) = document_symbols(&profile, &document) else {
        panic!("expected nested outline");
    };
    let smoke = outline
        .iter()
        .filter(|symbol| symbol.name == "smoke")
        .collect::<Vec<_>>();
    assert_eq!(smoke.len(), 1, "one ordinary callable symbol");
    assert!(
        smoke[0]
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("bound as `controller`"))
    );
    let WorkspaceSymbolResponse::Nested(workspace) =
        workspace_symbols(&profile, "smoke", PositionEncoding::Utf16).expect("workspace symbols")
    else {
        panic!("expected nested workspace symbols");
    };
    assert_eq!(workspace.len(), 1, "one ordinary workspace symbol");
    assert!(
        workspace[0]
            .container_name
            .as_deref()
            .is_some_and(|detail| detail.contains("bound as `controller`"))
    );
    let completions = callable_completions(&profile)
        .into_iter()
        .filter(|item| item.label == "smoke")
        .collect::<Vec<_>>();
    assert_eq!(completions.len(), 1, "one ordinary callable completion");
    assert!(
        completions[0]
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("bound as `controller`"))
    );
    let edits = rename(
        &profile,
        &DocumentStore::default(),
        &document,
        offset,
        "inspect",
    )
    .and_then(|edit| edit.changes)
    .expect("rename edits");
    assert_eq!(
        edits.values().map(Vec::len).sum::<usize>(),
        3,
        "ordinary declaration, ordinary call, and entry role are renamed"
    );
    assert_eq!(
        references(&profile, &document, offset)
            .expect("ordinary references")
            .len(),
        3,
        "a local closure shadowing the project function is not linked to it"
    );
}

#[test]
fn workspace_symbols_union_distinct_worlds_deduplicate_and_ignore_profile_order() {
    let first_project = TestProject::new("workspace-symbol-first");
    first_project.write_manifest();
    let first_source = SOURCE.replace("smoke", "alpha_smoke");
    first_project.write("src/main.arcw", &first_source);
    let first = LspProfileTestHarness::new(LspProfileResolver::new(
        RuntimeHostRunnerKind::Native,
        Some("agent".to_owned()),
    ))
    .resolve_for_document_path(&first_project.path("src/main.arcw"))
    .expect("first profile construction")
    .publish_for_test();
    assert!(first.diagnostics().is_empty(), "{:?}", first.diagnostics());

    let second_project = TestProject::new("workspace-symbol-second");
    second_project.write_manifest();
    let second_source = SOURCE.replace("smoke", "beta_smoke");
    second_project.write("src/main.arcw", &second_source);
    let second = LspProfileTestHarness::new(LspProfileResolver::new(
        RuntimeHostRunnerKind::Native,
        Some("agent".to_owned()),
    ))
    .resolve_for_document_path(&second_project.path("src/main.arcw"))
    .expect("second profile construction")
    .publish_for_test();
    assert!(
        second.diagnostics().is_empty(),
        "{:?}",
        second.diagnostics()
    );

    let forward =
        workspace_symbols_for_profiles([&first, &first, &second], "smoke", PositionEncoding::Utf16)
            .expect("workspace union");
    let reverse = workspace_symbols_for_profiles(
        [&second, &first, &second],
        "smoke",
        PositionEncoding::Utf16,
    )
    .expect("reversed workspace union");
    assert_eq!(forward, reverse);
    let WorkspaceSymbolResponse::Nested(symbols) = forward else {
        panic!("nested workspace symbols");
    };
    assert_eq!(
        symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>(),
        ["alpha_smoke", "beta_smoke"]
    );
}

#[test]
fn stale_open_bytes_do_not_reuse_accepted_entry_role_spans() {
    let project = TestProject::new("entry-role-stale");
    project.write_manifest();
    project.write("src/main.arcw", SOURCE);
    let source_path = project.path("src/main.arcw");
    let profile = LspProfileTestHarness::new(LspProfileResolver::new(
        RuntimeHostRunnerKind::Native,
        Some("agent".to_owned()),
    ))
    .resolve_for_document_path(&source_path)
    .expect("profile construction")
    .publish_for_test();
    let stale = format!("// unsaved\n{SOURCE}");
    let document = open(&source_path, &stale);
    let offset = stale.rfind("smoke").expect("controller role");
    let accepted = profile.accepted_environment().expect("accepted profile");
    let accepted_source = accepted
        .project()
        .sources()
        .by_uri(document.uri())
        .expect("accepted source");
    assert!(
        !Arc::ptr_eq(accepted_source.document(), document.source_document()),
        "unsaved bytes must form a distinct source-document lease"
    );

    assert!(definition(&profile, &document, offset).is_none());
    assert!(
        rename(
            &profile,
            &DocumentStore::default(),
            &document,
            offset,
            "inspect"
        )
        .is_none()
    );
    assert!(
        matches!(
            document_symbols(&profile, &document),
            DocumentSymbolResponse::Nested(symbols) if symbols.is_empty()
        ),
        "a stale editor lineage must not receive any accepted-project symbols"
    );
}

#[test]
fn document_outline_without_an_accepted_project_fails_closed() {
    const OUTLINE: &str = r"
struct GameState {}
enum GameEvent { Start }
fn update() -> Unit { () }
flow @flow.opening opening {}
entry game @entry.game.main {
goto @flow.opening
}
";
    let project = TestProject::new("ordinary-outline");
    let source_path = project.path("src/main.arcw");
    project.write("src/main.arcw", OUTLINE);
    let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
    let document = open(&source_path, OUTLINE);
    assert!(
        profile.accepted_environment().is_none(),
        "a manifest-free document has no compiler-owned tooling lease"
    );

    let DocumentSymbolResponse::Nested(symbols) = document_symbols(&profile, &document) else {
        panic!("ordinary outline");
    };
    assert!(
        symbols.is_empty(),
        "document symbols require the compiler-owned tooling lease; the LSP must not lower a parallel local HIR"
    );
}

#[test]
fn rename_aborts_when_a_secondary_open_source_is_stale() {
    const HELPERS: &str = r"
mod crate.helpers

pub fn selected_entry() -> Unit {
let selected = @entry.agent.main
()
}
";
    let project = TestProject::new("entry-rename-stale-secondary");
    project.write_manifest();
    let main = format!("use crate.helpers.selected_entry\n{SOURCE}");
    project.write("src/main.arcw", &main);
    project.write("src/helpers.arcw", HELPERS);
    let source_path = project.path("src/main.arcw");
    let profile = LspProfileTestHarness::new(LspProfileResolver::new(
        RuntimeHostRunnerKind::Native,
        Some("agent".to_owned()),
    ))
    .resolve_for_document_path(&source_path)
    .expect("profile construction")
    .publish_for_test();
    assert!(
        profile.diagnostics().is_empty(),
        "{:?}",
        profile.diagnostics()
    );

    let mut documents = DocumentStore::default();
    let accepted = profile.accepted_environment().expect("accepted profile");
    let accepted_source = accepted
        .project()
        .sources()
        .by_uri(&file_uri(&source_path))
        .expect("accepted source");
    let authority = AcceptedOpenDocument::new(Arc::clone(accepted_source.document()), None);
    let document = documents
        .open_with_authority(
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: file_uri(&source_path),
                    language_id: "arcweft".to_owned(),
                    version: 4,
                    text: main.clone(),
                },
            },
            PositionEncoding::Utf16,
            Some(&authority),
        )
        .expect("source document parse");
    let helpers_path = project.path("src/helpers.arcw");
    let helpers_uri = file_uri(&helpers_path);
    let accepted_helper = accepted
        .project()
        .sources()
        .by_uri(&helpers_uri)
        .expect("accepted helper source");
    let helper_authority = AcceptedOpenDocument::new(Arc::clone(accepted_helper.document()), None);
    documents
        .open_with_authority(
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: helpers_uri,
                    language_id: "arcweft".to_owned(),
                    version: 8,
                    text: format!("{HELPERS}\n// unsaved editor revision\n"),
                },
            },
            PositionEncoding::Utf16,
            Some(&helper_authority),
        )
        .expect("helper document parse");
    let offset = main.rfind("@entry.agent.main").expect("entry declaration") + 1;

    assert!(
        rename(&profile, &documents, &document, offset, "renamed").is_none(),
        "a multi-document rename must fail closed instead of returning partial edits"
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one nominal-role scenario verifies state and event ownership across all LSP surfaces"
)]
fn state_and_event_roles_define_their_ordinary_nominal_declarations() {
    const STATEFUL: &str = r"
struct GameState {
score: i32
}

enum GameEvent {
Start
}

fn initial_game_state() -> GameState
effects {}
{
GameState { score = 0i32 }
}

fn reduce_game(current: &GameState, event: GameEvent)
-> Result<Reduction<GameState>, ReducerError>
effects {}
{
Ok(Reduction.unchanged(current))
}

fn preview_reduction(current: &GameState, event: GameEvent)
-> Result<Reduction<GameState>, ReducerError>
effects {}
{
reduce_game(current, event)
}

flow @flow.opening opening(current: GameState) {
}

entry game @entry.game.main {
state = GameState
initializer = initial_game_state
event = GameEvent
reducer = reduce_game
goto @flow.opening
}
";
    let project = TestProject::new("entry-nominal-tooling");
    project.write(
        "arcw.toml",
        r#"schema = 1

[package]
id = "org.arcweft.tests.entry-nominal-tooling"
version = "0.1.0"

[profiles.game]
kind = "game"
entry = "@entry.game.main"
source = "src/main.arcw"
"#,
    );
    project.write("src/main.arcw", STATEFUL);
    let source_path = project.path("src/main.arcw");
    let profile = LspProfileTestHarness::new(LspProfileResolver::new(
        RuntimeHostRunnerKind::Native,
        Some("game".to_owned()),
    ))
    .resolve_for_document_path(&source_path)
    .expect("profile construction")
    .publish_for_test();
    assert!(
        profile.diagnostics().is_empty(),
        "{:?}",
        profile.diagnostics()
    );
    let document = open_accepted(&profile, &source_path, STATEFUL, PositionEncoding::Utf16);

    for (role, declaration) in [
        ("state = GameState", "struct GameState"),
        ("event = GameEvent", "enum GameEvent"),
    ] {
        let role_offset = STATEFUL.find(role).expect("role") + role.find('=').unwrap() + 2;
        let GotoDefinitionResponse::Scalar(location) =
            definition(&profile, &document, role_offset).expect("nominal definition")
        else {
            panic!("expected scalar definition");
        };
        let declaration_offset =
            STATEFUL.find(declaration).expect("declaration") + declaration.rfind(' ').unwrap() + 1;
        let expected = document
            .line_index()
            .position_from_byte_offset(declaration_offset);
        assert_eq!(location.range.start, expected);
    }

    for (role, declaration) in [
        ("initializer = initial_game_state", "fn initial_game_state"),
        ("reducer = reduce_game", "fn reduce_game"),
    ] {
        let role_offset = STATEFUL.find(role).expect("callable role") + role.find('=').unwrap() + 2;
        let GotoDefinitionResponse::Scalar(location) =
            definition(&profile, &document, role_offset).expect("callable definition")
        else {
            panic!("expected scalar definition");
        };
        let declaration_offset =
            STATEFUL.find(declaration).expect("declaration") + declaration.rfind(' ').unwrap() + 1;
        assert_eq!(
            location.range.start,
            document
                .line_index()
                .position_from_byte_offset(declaration_offset)
        );
    }

    let reducer_declaration = STATEFUL.find("reduce_game(current").expect("reducer name");
    let reducer_role = STATEFUL.rfind("reduce_game").expect("reducer role");
    assert_eq!(
        rename(
            &profile,
            &DocumentStore::default(),
            &document,
            reducer_declaration,
            "apply_game",
        ),
        rename(
            &profile,
            &DocumentStore::default(),
            &document,
            reducer_role,
            "apply_game",
        ),
        "rename is declaration-owned regardless of initiation site"
    );
    assert_eq!(
        references(&profile, &document, reducer_role)
            .expect("reducer references")
            .len(),
        3,
        "ordinary declaration, ordinary call, and reducer role are one reference set"
    );
    let DocumentSymbolResponse::Nested(symbols) = document_symbols(&profile, &document) else {
        panic!("outline");
    };
    let reducer = symbols
        .iter()
        .find(|symbol| symbol.name == "reduce_game")
        .expect("ordinary reducer function symbol");
    assert!(
        reducer
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("bound as `reducer`"))
    );
    let reducer_completions = callable_completions(&profile)
        .into_iter()
        .filter(|completion| completion.label == "reduce_game")
        .collect::<Vec<_>>();
    assert_eq!(
        reducer_completions.len(),
        1,
        "one ordinary callable completion represents the reducer"
    );
    assert!(
        reducer_completions[0]
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("bound as `reducer`")),
        "the ordinary callable completion carries the role annotation"
    );
}

#[test]
fn direct_and_aliased_import_calls_share_typed_references_but_rename_only_authored_name() {
    const MAIN: &str = r"
use crate.helpers.smoke
use crate.helpers.smoke as inspect

fn direct() -> Result<Unit, AgentError> {
smoke()
}


fn aliased() -> Result<Unit, AgentError> {
inspect()
}

entry agent @entry.agent.main {
controller = inspect
}
";
    const HELPERS: &str = r"
mod crate.helpers

pub fn smoke() -> Result<Unit, AgentError>
effects {}
{
Ok(())
}
";
    let project = TestProject::new("entry-call-reference-tooling");
    project.write_manifest();
    project.write("src/main.arcw", MAIN);
    project.write("src/helpers.arcw", HELPERS);
    let source_path = project.path("src/main.arcw");
    let profile = LspProfileTestHarness::new(LspProfileResolver::new(
        RuntimeHostRunnerKind::Native,
        Some("agent".to_owned()),
    ))
    .resolve_for_document_path(&source_path)
    .expect("profile construction")
    .publish_for_test();
    assert!(
        profile.diagnostics().is_empty(),
        "{:?}",
        profile.diagnostics()
    );
    let document = open_accepted(&profile, &source_path, MAIN, PositionEncoding::Utf16);
    let alias_offset = MAIN.find("inspect()").expect("aliased call");

    let GotoDefinitionResponse::Scalar(definition_location) =
        definition(&profile, &document, alias_offset).expect("definition")
    else {
        panic!("expected scalar definition");
    };
    assert!(
        definition_location
            .uri
            .as_str()
            .ends_with("/src/helpers.arcw")
    );
    let role_offset = MAIN.rfind("inspect").expect("aliased controller role");
    assert!(definition(&profile, &document, role_offset).is_some());
    let locations =
        references(&profile, &document, alias_offset).expect("typed callable references");
    assert_eq!(
        locations.len(),
        6,
        "declaration, role, two imports, and two call sites"
    );

    let edits = rename(
        &profile,
        &DocumentStore::default(),
        &document,
        alias_offset,
        "observe",
    )
    .and_then(|edit| edit.changes)
    .expect("rename edits");
    assert_eq!(
        edits.values().map(Vec::len).sum::<usize>(),
        4,
        "declaration, both import targets, and direct call are renamed; alias call and role stay"
    );
    assert!(
        edits
            .values()
            .flatten()
            .all(|edit| edit.new_text == "observe")
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exact matrix row proves the three ordinary callable families across every navigation surface and both syntax/HIR identities"
)]
fn lsp_navigation_uses_typed_syntax_and_module_hir_ids() {
    const MAIN: &str = r"
use crate.helpers.helper

fn controller() -> Result<Unit, AgentError>
effects {}
{
Ok(())
}

fn serve() -> Unit { () }

fn invoke() -> Unit {
serve()
}

predicate allows(value: bool) = value

predicate combined(value: bool) = allows(value)

proof witness() = ()

proof combined_witness() {
witness();
}

entry agent @entry.agent.main {
controller = controller
}
";
    const HELPERS: &str = r"
mod crate.helpers

pub fn helper() -> Unit { () }
";

    let project = TestProject::new("typed-callable-navigation");
    project.write_manifest();
    project.write("src/main.arcw", MAIN);
    project.write("src/helpers.arcw", HELPERS);
    let main_path = project.path("src/main.arcw");
    let helpers_path = project.path("src/helpers.arcw");
    let profile = LspProfileTestHarness::new(LspProfileResolver::new(
        RuntimeHostRunnerKind::Native,
        Some("agent".to_owned()),
    ))
    .resolve_for_document_path(&main_path)
    .expect("profile construction")
    .publish_for_test();
    assert!(
        profile.diagnostics().is_empty(),
        "{:?}",
        profile.diagnostics()
    );

    let accepted = profile
        .accepted_environment()
        .expect("accepted environment");
    let accepted_project = accepted.project();
    let navigation_uri = file_uri(&main_path);
    let helpers_uri = file_uri(&helpers_path);
    let accepted_source = accepted_project
        .sources()
        .by_uri(&navigation_uri)
        .expect("accepted navigation source");
    let module_key = accepted_project
        .module_key(accepted_source.document().identity())
        .expect("module-preserving source key");
    let parsed = accepted_project
        .parsed_source(&module_key)
        .expect("compiler-retained ParsedSource");
    let hir = accepted_project
        .hir(&module_key)
        .expect("compiler-retained HIR module");
    assert!(Arc::ptr_eq(
        hir,
        accepted_project
            .hir_project()
            .view()
            .module(module_key.module())
            .expect("same module in shared HIR project")
    ));
    assert!(Arc::ptr_eq(
        parsed.document_lease(),
        accepted_source.document()
    ));
    assert!(Arc::ptr_eq(
        hir.provenance().document(),
        accepted_source.document()
    ));
    assert_eq!(hir.provenance().syntax_snapshot(), parsed.snapshot_id());
    let helpers_source = accepted_project
        .sources()
        .by_uri(&helpers_uri)
        .expect("accepted helpers source");
    let helpers_key = accepted_project
        .module_key(helpers_source.document().identity())
        .expect("module-preserving helpers key");
    let helpers_hir = accepted_project
        .hir(&helpers_key)
        .expect("compiler-retained helpers HIR module");
    assert_ne!(module_key.module(), helpers_key.module());
    assert_eq!(
        hir.module_id().database(),
        helpers_hir.module_id().database()
    );
    assert!(Arc::ptr_eq(
        helpers_hir,
        accepted_project
            .hir_project()
            .view()
            .module(helpers_key.module())
            .expect("same helpers module in shared HIR project")
    ));

    let attached = parsed.items().expect("attached source items");
    let expected = [
        (
            "serve",
            CallableDeclarationOwner::Function,
            HirItemFamily::Function,
        ),
        (
            "allows",
            CallableDeclarationOwner::Predicate,
            HirItemFamily::Predicate,
        ),
        (
            "witness",
            CallableDeclarationOwner::Proof,
            HirItemFamily::Proof,
        ),
    ];
    for (name, owner, family) in expected {
        let syntax = attached
            .iter()
            .find(|item| {
                item.name()
                    .ok()
                    .flatten()
                    .is_some_and(|source_name| source_name.source_text() == name)
            })
            .expect("typed callable syntax");
        assert!(matches!(
            (owner, syntax),
            (
                CallableDeclarationOwner::Function,
                TypedItemNode::Function(_)
            ) | (
                CallableDeclarationOwner::Predicate,
                TypedItemNode::Predicate(_)
            ) | (CallableDeclarationOwner::Proof, TypedItemNode::Proof(_))
        ));
        assert_eq!(syntax.snapshot_id(), parsed.snapshot_id());

        let symbol = accepted_project
            .project_symbols()
            .callable_symbols()
            .find(|symbol| symbol.owner() == owner && symbol.declaration().name() == name)
            .expect("typed callable symbol");
        assert_eq!(symbol.declaration().module(), module_key.module());
        assert_eq!(symbol.source_snapshot(), hir.snapshot_id());
        assert_eq!(symbol.source_owner(), HirCallableSourceOwner::Item);
        assert_eq!(
            hir.resolve_item(symbol.source_item())
                .expect("module-local ItemId")
                .family(),
            family
        );
        let whole = item_source_span(
            hir,
            symbol.source_item(),
            HirItemSourceRole::Declaration(HirDeclarationSourceRole::Whole),
        )
        .expect("typed declaration source");
        let syntax_span = syntax.source_span();
        assert_eq!(whole, syntax_span);
        let name_span = item_source_span(
            hir,
            symbol.source_item(),
            HirItemSourceRole::Callable(HirCallableSourceRole::Name {
                owner: HirCallableSourceOwner::Item,
            }),
        )
        .expect("typed callable name source");
        assert_eq!(name_span, *symbol.name_span());
    }

    let mut documents = DocumentStore::default();
    let authority = AcceptedOpenDocument::new(Arc::clone(accepted_source.document()), None);
    let document = documents
        .open_with_authority(
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: navigation_uri.clone(),
                    language_id: "arcweft".to_owned(),
                    version: 1,
                    text: MAIN.to_owned(),
                },
            },
            PositionEncoding::Utf16,
            Some(&authority),
        )
        .expect("accepted helper document");

    let DocumentSymbolResponse::Nested(outline) = document_symbols(&profile, &document) else {
        panic!("expected nested outline");
    };
    let outlined = outline
        .iter()
        .filter(|symbol| ["serve", "allows", "witness"].contains(&symbol.name.as_str()))
        .map(|symbol| {
            (
                symbol.name.as_str(),
                symbol
                    .detail
                    .as_deref()
                    .expect("typed callable source label"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        outlined,
        [
            ("serve", "fn serve() -> Unit"),
            ("allows", "predicate allows(value: bool)"),
            ("witness", "proof witness()"),
        ]
    );

    for (name, keyword, expected_references) in [
        ("serve", "fn", 2),
        ("allows", "predicate", 2),
        ("witness", "proof", 2),
    ] {
        let offset = MAIN
            .find(&format!("{keyword} {name}"))
            .expect("callable declaration")
            + keyword.len()
            + 1;
        let GotoDefinitionResponse::Scalar(location) =
            definition(&profile, &document, offset).expect("callable definition")
        else {
            panic!("expected scalar definition");
        };
        assert_eq!(location.uri, navigation_uri);
        let HoverContents::Scalar(MarkedString::String(hover_text)) =
            hover(&profile, &document, offset)
                .expect("callable hover")
                .contents
        else {
            panic!("expected string hover");
        };
        assert!(
            hover_text.contains(&format!("{name}(")),
            "{keyword} hover must come from the typed signature: {hover_text}"
        );
        assert_eq!(
            references(&profile, &document, offset)
                .expect("typed callable references")
                .len(),
            expected_references,
            "exact typed declaration and use inventory for {name}"
        );
        let edits = rename(
            &profile,
            &documents,
            &document,
            offset,
            &format!("renamed_{name}"),
        )
        .and_then(|edit| edit.changes)
        .expect("typed callable rename edits");
        assert_eq!(
            edits.values().map(Vec::len).sum::<usize>(),
            expected_references,
            "exact typed declaration and use rename inventory for {name}"
        );
    }
}

#[test]
fn manifest_entry_token_defines_and_renames_the_source_entry() {
    let project = TestProject::new("entry-manifest-tooling");
    project.write_manifest();
    project.write("src/main.arcw", SOURCE);
    let source_path = project.path("src/main.arcw");
    let profile = LspProfileTestHarness::new(LspProfileResolver::new(
        RuntimeHostRunnerKind::Native,
        Some("agent".to_owned()),
    ))
    .resolve_for_document_path(&source_path)
    .expect("profile construction")
    .publish_for_test();
    let manifest = TestProject::manifest();
    let document = open(&project.path("arcw.toml"), &manifest);
    let offset = manifest.find("entry.agent.main").expect("entry selection");

    let GotoDefinitionResponse::Scalar(manifest_definition) =
        definition(&profile, &document, offset).expect("manifest entry definition")
    else {
        panic!("expected scalar manifest entry definition");
    };
    let source_document = open_accepted(&profile, &source_path, SOURCE, PositionEncoding::Utf16);
    let source_entry_start = SOURCE
        .rfind("@entry.agent.main")
        .expect("source entry declaration");
    assert_eq!(manifest_definition.uri, *source_document.uri());
    assert_eq!(
        manifest_definition.range,
        source_document.line_index().range_from_byte_span(
            source_entry_start,
            source_entry_start + "@entry.agent.main".len(),
        ),
        "the TOML token must navigate to the exact authored source entry ID"
    );
    let edits = rename(
        &profile,
        &DocumentStore::default(),
        &document,
        offset,
        "inspect",
    )
    .and_then(|edit| edit.changes)
    .expect("entry rename edits");
    assert_eq!(
        edits.values().map(Vec::len).sum::<usize>(),
        3,
        "source declaration, typed source reference, and manifest selection are renamed"
    );

    let reference_offset = SOURCE.find("@entry.agent.main").expect("entry reference") + 1;
    let GotoDefinitionResponse::Scalar(source_reference_definition) =
        definition(&profile, &source_document, reference_offset)
            .expect("typed source entry reference definition")
    else {
        panic!("expected scalar source entry definition");
    };
    assert_eq!(
        source_reference_definition, manifest_definition,
        "typed source and TOML entry references resolve to the same exact declaration range"
    );
    assert_eq!(
        references(&profile, &source_document, reference_offset)
            .expect("entry references")
            .len(),
        3,
        "declaration, typed source reference, and manifest selection"
    );
}

#[test]
fn entry_reference_ranges_follow_utf8_utf16_and_utf32_encodings() {
    let source = SOURCE.replace(
        "let selected = @entry.agent.main",
        "let selected = (\"😀\", @entry.agent.main)",
    );
    let project = TestProject::new("entry-reference-encoding");
    project.write_manifest();
    project.write("src/main.arcw", &source);
    let source_path = project.path("src/main.arcw");
    let profile = LspProfileTestHarness::new(LspProfileResolver::new(
        RuntimeHostRunnerKind::Native,
        Some("agent".to_owned()),
    ))
    .resolve_for_document_path(&source_path)
    .expect("profile construction")
    .publish_for_test();
    assert!(
        profile.diagnostics().is_empty(),
        "{:?}",
        profile.diagnostics()
    );
    let offset = source.find("@entry.agent.main").expect("entry reference") + 1;

    for encoding in [
        PositionEncoding::Utf8,
        PositionEncoding::Utf16,
        PositionEncoding::Utf32,
    ] {
        let document = open_accepted(&profile, &source_path, &source, encoding);
        let expected = document.line_index().position_from_byte_offset(offset);
        let edits = rename(
            &profile,
            &DocumentStore::default(),
            &document,
            offset,
            "inspect",
        )
        .and_then(|edit| edit.changes)
        .expect("entry rename edits");
        let current = edits.get(document.uri()).expect("current source edits");
        assert!(
            current.iter().any(|edit| edit.range.start == expected),
            "{encoding:?} must preserve the exact entry-reference start"
        );
    }
}

fn open(path: &Path, source: &str) -> DocumentSnapshot {
    open_with_encoding(path, source, PositionEncoding::Utf16)
}

fn open_accepted(
    profile: &LspProfile,
    path: &Path,
    source: &str,
    encoding: PositionEncoding,
) -> DocumentSnapshot {
    let uri = file_uri(path);
    let accepted = profile.accepted_environment().expect("accepted profile");
    let accepted_source = accepted
        .project()
        .sources()
        .by_uri(&uri)
        .expect("accepted source");
    let authority = AcceptedOpenDocument::new(Arc::clone(accepted_source.document()), None);
    let mut store = DocumentStore::default();
    store
        .open_with_authority(
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri,
                    language_id: "arcweft".to_owned(),
                    version: 1,
                    text: source.to_owned(),
                },
            },
            encoding,
            Some(&authority),
        )
        .expect("accepted document open")
}

fn open_with_encoding(path: &Path, source: &str, encoding: PositionEncoding) -> DocumentSnapshot {
    let mut store = DocumentStore::default();
    store
        .open(
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: file_uri(path),
                    language_id: "arcweft".to_owned(),
                    version: 1,
                    text: source.to_owned(),
                },
            },
            encoding,
        )
        .expect("document parse")
}

fn file_uri(path: &Path) -> Uri {
    format!(
        "file:///{}",
        path.to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    )
    .parse()
    .expect("file URI")
}

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{label}-{unique}"));
        fs::create_dir_all(&root).expect("root");
        Self { root }
    }

    fn path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.root.join(path)
    }

    fn manifest() -> String {
        r#"schema = 1

[package]
id = "org.arcweft.tests.entry-role-tooling"
version = "0.1.0"

[profiles.agent]
kind = "agent"
entry = "@entry.agent.main"
source = "src/main.arcw"
"#
        .to_owned()
    }

    fn write_manifest(&self) {
        self.write("arcw.toml", &Self::manifest());
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.path(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, contents).expect("write");
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
