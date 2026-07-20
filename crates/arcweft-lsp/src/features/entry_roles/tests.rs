#![allow(
    clippy::mutable_key_type,
    reason = "WorkspaceEdit exposes the LSP Uri type as its required changes-map key"
)]

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use lsp_types::{
    DidOpenTextDocumentParams, DocumentSymbolResponse, SymbolKind, TextDocumentItem, Uri,
    WorkspaceSymbolResponse,
};

use super::*;
use crate::{documents::DocumentStore, positions::PositionEncoding, profiles::LspProfileResolver};
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
let smoke = || ()
smoke()
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
    let profile = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("agent".to_owned()))
        .resolve_for_document_path(&source_path);
    assert!(
        profile.diagnostics().is_empty(),
        "{:?}",
        profile.diagnostics()
    );
    let document = open(&source_path, SOURCE);
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
    let first = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("agent".to_owned()))
        .resolve_for_document_path(&first_project.path("src/main.arcw"));
    assert!(first.diagnostics().is_empty(), "{:?}", first.diagnostics());

    let second_project = TestProject::new("workspace-symbol-second");
    second_project.write_manifest();
    let second_source = SOURCE.replace("smoke", "beta_smoke");
    second_project.write("src/main.arcw", &second_source);
    let second = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("agent".to_owned()))
        .resolve_for_document_path(&second_project.path("src/main.arcw"));
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
    let profile = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("agent".to_owned()))
        .resolve_for_document_path(&source_path);
    let stale = format!("// unsaved\n{SOURCE}");
    let document = open(&source_path, &stale);
    let offset = stale.rfind("smoke").expect("controller role");

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
    let DocumentSymbolResponse::Nested(symbols) = document_symbols(&profile, &document) else {
        panic!("ordinary outline remains available for stale editor bytes");
    };
    let smoke = symbols
        .iter()
        .find(|symbol| symbol.name == "smoke")
        .expect("ordinary function symbol");
    assert!(
        smoke
            .detail
            .as_deref()
            .is_some_and(|detail| !detail.contains("bound as")),
        "role annotations require exact accepted bytes"
    );
}

#[test]
fn document_outline_preserves_ordinary_declarations_without_a_manifest() {
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
    let profile = LspProfileResolver::new(RuntimeHostRunnerKind::Native, None)
        .resolve_for_document_path(&source_path);
    let document = open(&source_path, OUTLINE);

    let DocumentSymbolResponse::Nested(symbols) = document_symbols(&profile, &document) else {
        panic!("ordinary outline");
    };
    assert_eq!(
        symbols
            .iter()
            .map(|symbol| (symbol.name.as_str(), symbol.kind))
            .collect::<Vec<_>>(),
        [
            ("GameState", SymbolKind::STRUCT),
            ("GameEvent", SymbolKind::ENUM),
            ("update", SymbolKind::FUNCTION),
            ("opening", SymbolKind::FUNCTION),
            ("entry.game.main", SymbolKind::OBJECT),
        ]
    );
}

#[test]
fn rename_aborts_when_a_secondary_open_manifest_is_stale() {
    let project = TestProject::new("entry-rename-stale-secondary");
    project.write_manifest();
    project.write("src/main.arcw", SOURCE);
    let source_path = project.path("src/main.arcw");
    let profile = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("agent".to_owned()))
        .resolve_for_document_path(&source_path);
    assert!(
        profile.diagnostics().is_empty(),
        "{:?}",
        profile.diagnostics()
    );

    let mut documents = DocumentStore::default();
    let document = documents.open(
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: file_uri(&source_path),
                language_id: "arcweft".to_owned(),
                version: 4,
                text: SOURCE.to_owned(),
            },
        },
        PositionEncoding::Utf16,
    );
    documents.open(
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: file_uri(&project.path("arcw.toml")),
                language_id: "toml".to_owned(),
                version: 8,
                text: format!("# unsaved\n{}", TestProject::manifest()),
            },
        },
        PositionEncoding::Utf16,
    );
    let offset = SOURCE
        .rfind("@entry.agent.main")
        .expect("entry declaration")
        + 1;

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
initial_game_state()
}

fn reduce_game(state: &GameState, event: GameEvent)
-> Result<Reduction<GameState>, ReducerError>
effects {}
{
reduce_game(state, event)
}

flow @flow.opening opening(state: GameState) {
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
        r#"[package]
name = "entry-nominal-tooling"

[profiles.game]
kind = "game"
entry = "entry.game.main"
source = "src/main.arcw"
"#,
    );
    project.write("src/main.arcw", STATEFUL);
    let source_path = project.path("src/main.arcw");
    let profile = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("game".to_owned()))
        .resolve_for_document_path(&source_path);
    assert!(
        profile.diagnostics().is_empty(),
        "{:?}",
        profile.diagnostics()
    );
    let document = open(&source_path, STATEFUL);

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

    let reducer_declaration = STATEFUL.find("reduce_game(state").expect("reducer name");
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
    let profile = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("agent".to_owned()))
        .resolve_for_document_path(&source_path);
    assert!(
        profile.diagnostics().is_empty(),
        "{:?}",
        profile.diagnostics()
    );
    let document = open(&source_path, MAIN);
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
fn manifest_entry_token_defines_and_renames_the_source_entry() {
    let project = TestProject::new("entry-manifest-tooling");
    project.write_manifest();
    project.write("src/main.arcw", SOURCE);
    let source_path = project.path("src/main.arcw");
    let profile = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("agent".to_owned()))
        .resolve_for_document_path(&source_path);
    let manifest = TestProject::manifest();
    let document = open(&project.path("arcw.toml"), &manifest);
    let offset = manifest.find("entry.agent.main").expect("entry selection");

    let GotoDefinitionResponse::Scalar(manifest_definition) =
        definition(&profile, &document, offset).expect("manifest entry definition")
    else {
        panic!("expected scalar manifest entry definition");
    };
    let source_document = open(&source_path, SOURCE);
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
    let profile = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("agent".to_owned()))
        .resolve_for_document_path(&source_path);
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
        let document = open_with_encoding(&source_path, &source, encoding);
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

fn open_with_encoding(path: &Path, source: &str, encoding: PositionEncoding) -> DocumentSnapshot {
    let mut store = DocumentStore::default();
    store.open(
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
        r#"[package]
name = "entry-role-tooling"

[profiles.agent]
kind = "agent"
entry = "entry.agent.main"
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
