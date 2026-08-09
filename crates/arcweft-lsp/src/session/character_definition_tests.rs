use super::*;

use arcweft_lang_sema::character_definition::{
    CharacterDefinitionRequestBudget, CharacterDefinitionWorkKind,
};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionResponse, Position, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, Uri, VersionedTextDocumentIdentifier,
    WorkDoneProgressParams,
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const CHARACTER_REFERENCE_SOURCE: &str = "flow @flow.main main {\n    let hero = show(@character.akane)\n}\n\
entry server @entry.server.main {\n    goto @flow.main\n}\n";
const CHARACTER_MEMBER_SOURCE: &str = "flow @flow.main main {\n    let hero = show(@character.akane, look = .normal)\n}\n\
entry server @entry.server.main {\n    goto @flow.main\n}\n";

#[test]
fn character_definition_cache_hit_replays_identical_shared_work() {
    let project = CharacterDefinitionProject::new("character-cache-receipt");
    let source = CHARACTER_REFERENCE_SOURCE;
    project.write_project(source, character_manifest());
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(
        &LspConfig::new(arcweft_runtime_host::RuntimeHostRunnerKind::Native)
            .with_profile_id("game"),
    );
    open(&mut session, &uri, source);

    let document = session.documents.get(&uri).expect("open source");
    let profile = session.profile_for_uri(&uri);
    let cursor = source.find("akane").expect("character reference");
    let mut miss_budget = CharacterDefinitionRequestBudget::for_request();
    let miss_checkpoint = miss_budget.checkpoint();
    let miss = crate::features::character_definition::character_definition_with_budget(
        profile,
        &session.documents,
        document,
        cursor,
        &mut miss_budget,
    )
    .expect("cache miss definition");
    assert!(matches!(
        miss,
        crate::features::character_definition::CharacterDefinitionDispatch::Character(Some(_))
    ));
    let miss_work = miss_budget
        .receipt_since(miss_checkpoint)
        .expect("complete cache-miss work");

    let mut hit_budget = CharacterDefinitionRequestBudget::for_request();
    let hit_checkpoint = hit_budget.checkpoint();
    let hit = crate::features::character_definition::character_definition_with_budget(
        profile,
        &session.documents,
        document,
        cursor,
        &mut hit_budget,
    )
    .expect("cache hit definition");
    assert!(matches!(
        hit,
        crate::features::character_definition::CharacterDefinitionDispatch::Character(Some(_))
    ));
    let hit_work = hit_budget
        .receipt_since(hit_checkpoint)
        .expect("complete cache-hit work");
    assert_eq!(hit_budget.consumed(), miss_budget.consumed());
    assert_eq!(hit_work, miss_work);

    let mut exhausted = CharacterDefinitionRequestBudget::for_request();
    let precharge = exhausted
        .maximum()
        .checked_sub(miss_budget.consumed())
        .and_then(|remaining| remaining.checked_add(1))
        .expect("request work fits production budget");
    for _ in 0..precharge {
        exhausted
            .charge(CharacterDefinitionWorkKind::IdentityCheck)
            .expect("precharge remains in budget");
    }
    let error = crate::features::character_definition::character_definition_with_budget(
        profile,
        &session.documents,
        document,
        cursor,
        &mut exhausted,
    )
    .expect_err("receipt replay and live adaptation must exhaust the shared budget");
    assert!(matches!(
        error,
        crate::features::character_definition::CharacterDefinitionRequestError::Resource(_)
    ));
    assert_eq!(exhausted.consumed(), exhausted.maximum() + 1);

    let accepted = profile.accepted_environment().expect("accepted generation");
    assert_eq!(accepted.character_cache_entries_for_test(), (true, true));
    accepted.clear_caches();
    assert_eq!(accepted.character_cache_entries_for_test(), (false, false));
}

#[test]
fn character_owner_definition_returns_exact_manifest_location_link() {
    let project = CharacterDefinitionProject::new("character-owner-link");
    let source = CHARACTER_REFERENCE_SOURCE;
    let manifest = character_manifest();
    project.write_project(source, manifest);
    let uri = file_uri(&project.path("src/main.arcw"));
    let manifest_uri = file_uri(&project.path("assets/akane.awchar/character.awchar.json"));
    let mut session = ArcweftLspSession::new(
        &LspConfig::new(arcweft_runtime_host::RuntimeHostRunnerKind::Native)
            .with_profile_id("game"),
    );
    open(&mut session, &uri, source);

    let response = definition_request(&mut session, uri, position_of(source, "akane"));
    assert!(response.error.is_none(), "{:?}", response.error);
    let definition = serde_json::from_value::<GotoDefinitionResponse>(
        response.result.expect("definition result"),
    )
    .expect("definition response");
    let GotoDefinitionResponse::Link(links) = definition else {
        panic!("character definition must use LocationLink");
    };
    assert_eq!(links.len(), 1);
    let link = &links[0];
    assert_eq!(link.target_uri, manifest_uri);
    assert_eq!(
        link.target_range.start,
        position_of(manifest, "\"character.akane\"")
    );
    assert_eq!(
        link.target_selection_range.start,
        position_of(manifest, "character.akane")
    );
    assert_eq!(
        link.target_selection_range.end.character,
        link.target_selection_range.start.character + 15
    );
}

#[test]
fn character_definition_rejects_changed_target_without_partial_location() {
    let project = CharacterDefinitionProject::new("character-stale-target");
    let source = CHARACTER_REFERENCE_SOURCE;
    let manifest = character_manifest();
    project.write_project(source, manifest);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(
        &LspConfig::new(arcweft_runtime_host::RuntimeHostRunnerKind::Native)
            .with_profile_id("game"),
    );
    open(&mut session, &uri, source);
    fs::write(
        project.path("assets/akane.awchar/character.awchar.json"),
        manifest.replace("character.akane", "character.aoi"),
    )
    .expect("change target after accepted publication");

    let response = definition_request(&mut session, uri, position_of(source, "akane"));
    let error = response.error.expect("changed target is stale");
    assert_eq!(error.code, -32_801);
    assert_eq!(
        error.data,
        Some(serde_json::json!({
            "code": "aw.character.definition.stale_target"
        }))
    );
    assert!(response.result.is_none());
}

#[test]
fn stale_target_request_schedules_a_complete_profile_rebuild() {
    let project = CharacterDefinitionProject::new("character-stale-rebuild");
    let source = CHARACTER_REFERENCE_SOURCE;
    let manifest = character_manifest();
    project.write_project(source, manifest);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(
        &LspConfig::new(arcweft_runtime_host::RuntimeHostRunnerKind::Native)
            .with_profile_id("game"),
    );
    open(&mut session, &uri, source);
    let before = session
        .profile_for_uri(&uri)
        .accepted_environment()
        .expect("initial generation")
        .generation();
    let reformatted = format!("\n\n{manifest}");
    fs::write(
        project.path("assets/akane.awchar/character.awchar.json"),
        &reformatted,
    )
    .expect("reformat target after accepted publication");

    let stale = definition_request(&mut session, uri.clone(), position_of(source, "akane"));
    assert_eq!(stale.error.expect("first request is stale").code, -32_801);
    let after = session
        .profile_for_uri(&uri)
        .accepted_environment()
        .expect("scheduled rebuild publishes")
        .generation();
    assert!(after.get() > before.get());

    let fresh = definition_request(&mut session, uri, position_of(source, "akane"));
    assert!(fresh.error.is_none(), "{:?}", fresh.error);
    let definition = serde_json::from_value::<GotoDefinitionResponse>(
        fresh.result.expect("definition from rebuilt generation"),
    )
    .expect("definition response");
    let GotoDefinitionResponse::Link(links) = definition else {
        panic!("character definition must use LocationLink");
    };
    assert_eq!(
        links[0].target_selection_range.start,
        position_of(&reformatted, "character.akane")
    );
}

#[test]
fn valid_open_manifest_overlay_rebuilds_one_complete_definition_generation() {
    let project = CharacterDefinitionProject::new("character-manifest-overlay");
    let source = CHARACTER_REFERENCE_SOURCE;
    let disk_manifest = character_manifest();
    project.write_project(source, disk_manifest);
    let source_uri = file_uri(&project.path("src/main.arcw"));
    let manifest_uri = file_uri(&project.path("assets/akane.awchar/character.awchar.json"));
    let mut session = ArcweftLspSession::new(
        &LspConfig::new(arcweft_runtime_host::RuntimeHostRunnerKind::Native)
            .with_profile_id("game"),
    );
    open(&mut session, &source_uri, source);
    let before = session
        .profile_for_uri(&source_uri)
        .accepted_environment()
        .expect("disk generation")
        .generation();

    let overlay_manifest = format!("\n\n{disk_manifest}");
    open(&mut session, &manifest_uri, &overlay_manifest);
    let after = session
        .profile_for_uri(&source_uri)
        .accepted_environment()
        .expect("overlay generation")
        .generation();
    assert!(after.get() > before.get());

    let response = definition_request(&mut session, source_uri, position_of(source, "akane"));
    assert!(response.error.is_none(), "{:?}", response.error);
    let definition = serde_json::from_value::<GotoDefinitionResponse>(
        response.result.expect("overlay definition"),
    )
    .expect("definition response");
    let GotoDefinitionResponse::Link(links) = definition else {
        panic!("character definition must use LocationLink");
    };
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target_uri, manifest_uri);
    assert_eq!(
        links[0].target_selection_range.start,
        position_of(&overlay_manifest, "character.akane")
    );
}

#[test]
fn invalid_manifest_overlay_preserves_the_last_accepted_generation() {
    let project = CharacterDefinitionProject::new("character-invalid-overlay");
    let source = CHARACTER_REFERENCE_SOURCE;
    let disk_manifest = character_manifest();
    project.write_project(source, disk_manifest);
    let source_uri = file_uri(&project.path("src/main.arcw"));
    let manifest_uri = file_uri(&project.path("assets/akane.awchar/character.awchar.json"));
    let mut session = ArcweftLspSession::new(
        &LspConfig::new(arcweft_runtime_host::RuntimeHostRunnerKind::Native)
            .with_profile_id("game"),
    );
    open(&mut session, &source_uri, source);
    open(&mut session, &manifest_uri, disk_manifest);
    let accepted = session
        .profile_for_uri(&source_uri)
        .accepted_environment()
        .expect("valid overlay generation");

    change(&mut session, manifest_uri, 2, "{");
    let current = session
        .profile_for_uri(&source_uri)
        .accepted_environment()
        .expect("previous generation remains accepted");
    assert!(Arc::ptr_eq(&accepted, &current));

    let response = definition_request(&mut session, source_uri, position_of(source, "akane"));
    let error = response.error.expect("invalid target overlay is stale");
    assert_eq!(error.code, -32_801);
    assert_eq!(
        error.data,
        Some(serde_json::json!({
            "code": "aw.character.definition.stale_target"
        }))
    );
    assert!(response.result.is_none());
}

#[test]
fn closing_manifest_overlay_rebuilds_remaining_profiles_from_disk() {
    let project = CharacterDefinitionProject::new("character-overlay-close");
    let source = CHARACTER_REFERENCE_SOURCE;
    let disk_manifest = character_manifest();
    project.write_project(source, disk_manifest);
    let source_uri = file_uri(&project.path("src/main.arcw"));
    let manifest_uri = file_uri(&project.path("assets/akane.awchar/character.awchar.json"));
    let mut session = ArcweftLspSession::new(
        &LspConfig::new(arcweft_runtime_host::RuntimeHostRunnerKind::Native)
            .with_profile_id("game"),
    );
    open(&mut session, &source_uri, source);
    let overlay_manifest = format!("\n\n{disk_manifest}");
    open(&mut session, &manifest_uri, &overlay_manifest);
    let overlay_generation = session
        .profile_for_uri(&source_uri)
        .accepted_environment()
        .expect("overlay generation")
        .generation();

    close(&mut session, manifest_uri);
    let disk_generation = session
        .profile_for_uri(&source_uri)
        .accepted_environment()
        .expect("disk generation after close")
        .generation();
    assert!(disk_generation.get() > overlay_generation.get());

    let response = definition_request(&mut session, source_uri, position_of(source, "akane"));
    assert!(response.error.is_none(), "{:?}", response.error);
    let definition = serde_json::from_value::<GotoDefinitionResponse>(
        response.result.expect("disk definition after close"),
    )
    .expect("definition response");
    let GotoDefinitionResponse::Link(links) = definition else {
        panic!("character definition must use LocationLink");
    };
    assert_eq!(
        links[0].target_selection_range.start,
        position_of(disk_manifest, "character.akane")
    );
}

#[test]
fn globally_unique_local_member_resolves_through_typed_member_index() {
    let project = CharacterDefinitionProject::new("character-local-member");
    let source = CHARACTER_MEMBER_SOURCE;
    let manifest = character_manifest();
    project.write_project(source, manifest);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(
        &LspConfig::new(arcweft_runtime_host::RuntimeHostRunnerKind::Native)
            .with_profile_id("game"),
    );
    open(&mut session, &uri, source);

    let response = definition_request(&mut session, uri, position_of(source, "normal"));
    assert!(response.error.is_none(), "{:?}", response.error);
    let definition = serde_json::from_value::<GotoDefinitionResponse>(
        response.result.expect("member definition"),
    )
    .expect("definition response");
    let GotoDefinitionResponse::Link(links) = definition else {
        panic!("character definition must use LocationLink");
    };
    assert_eq!(links.len(), 1);
    assert_eq!(
        links[0].target_selection_range.start,
        position_of_last(manifest, "normal")
    );
}

fn definition_request(session: &mut ArcweftLspSession, uri: Uri, position: Position) -> Response {
    session.handle_request(Request {
        id: RequestId::from(99),
        method: GotoDefinition::METHOD.to_owned(),
        params: serde_json::json!(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        }),
    })
}

fn open(session: &mut ArcweftLspSession, uri: &Uri, source: &str) {
    session
        .handle_notification(Notification::new(
            DidOpenTextDocument::METHOD.to_owned(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "arcweft".to_owned(),
                    version: 1,
                    text: source.to_owned(),
                },
            },
        ))
        .expect("open source");
    assert!(
        session.profile_for_uri(uri).diagnostics().is_empty(),
        "profile diagnostics: {:?}",
        session.profile_for_uri(uri).diagnostics(),
    );
}

fn change(session: &mut ArcweftLspSession, uri: Uri, version: i32, source: &str) {
    session
        .handle_notification(Notification::new(
            DidChangeTextDocument::METHOD.to_owned(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier { uri, version },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: source.to_owned(),
                }],
            },
        ))
        .expect("change source");
}

fn close(session: &mut ArcweftLspSession, uri: Uri) {
    session
        .handle_notification(Notification::new(
            DidCloseTextDocument::METHOD.to_owned(),
            DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri },
            },
        ))
        .expect("close source");
}

fn position_of(source: &str, needle: &str) -> Position {
    let offset = source.find(needle).expect("needle");
    let before = &source[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count();
    let character = before
        .rsplit_once('\n')
        .map_or(before.len(), |(_, tail)| tail.len());
    Position::new(
        u32::try_from(line).expect("line"),
        u32::try_from(character).expect("character"),
    )
}

fn position_of_last(source: &str, needle: &str) -> Position {
    let offset = source.rfind(needle).expect("needle");
    let before = &source[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count();
    let character = before
        .rsplit_once('\n')
        .map_or(before.len(), |(_, tail)| tail.len());
    Position::new(
        u32::try_from(line).expect("line"),
        u32::try_from(character).expect("character"),
    )
}

fn file_uri(path: &Path) -> Uri {
    let normalized = path.to_string_lossy().replace('\\', "/");
    format!("file:///{normalized}").parse().expect("file URI")
}

fn character_manifest() -> &'static str {
    r#"{
  "format": "arcweft.character",
  "version": 1,
  "character": "character.akane",
  "canvas": { "width": 96, "height": 128 },
  "anchor": { "x": 48, "y": 128 },
  "default_look": "normal",
  "parts": [{
    "id": "body",
    "z": 0,
    "variants": [{
      "id": "default",
      "asset": "layers/body.png",
      "rect": { "x": 0, "y": 0, "width": 96, "height": 128 },
      "opacity": 255,
      "blend": "normal",
      "clipping": false
    }]
  }],
  "looks": [{ "id": "normal", "select": [{ "part": "body", "variant": "default" }] }]
}"#
}

struct CharacterDefinitionProject {
    root: PathBuf,
}

impl CharacterDefinitionProject {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("arcweft-{name}-{unique}"));
        fs::create_dir_all(&root).expect("project root");
        Self { root }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn write_project(&self, source: &str, manifest: &str) {
        self.write(
            "arcw.toml",
            r#"schema = 1

[package]
id = "org.arcweft.tests.character-definition"
version = "0.1.0"

[content-units.characters]
roots = ["@character.akane"]
visibility = "package"
demand = "required"

[profiles.game]
kind = "server"
entry = "@entry.server.main"
source = "src/main.arcw"

[profiles.game.content.characters]
residency = "startup"
placement = "embedded"
compression = "none"
"#,
        );
        self.write("src/main.arcw", source);
        self.write("assets/akane.awchar/character.awchar.json", manifest);
        self.write_bytes(
            "assets/akane.awchar/layers/body.png",
            include_bytes!(
                "../../../arcweft-character/tests/fixtures/zundamon.awchar/layers/body--default.png"
            ),
        );
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent");
        }
        fs::write(path, contents).expect("fixture write");
    }

    fn write_bytes(&self, relative: &str, contents: &[u8]) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent");
        }
        fs::write(path, contents).expect("fixture write");
    }
}

impl Drop for CharacterDefinitionProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
